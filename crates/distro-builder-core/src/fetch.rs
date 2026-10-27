//! Lädt Zusatzdaten für die ISO herunter: Ollama-Modelle und Container-Images.
//!
//! Alles wird in `<Projekt>/cache` zwischengespeichert, damit wiederholte Builds
//! nichts neu laden.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{self as C, Config};
use crate::profile::Paths;

const OLLAMA_REGISTRY: &str = "registry.ollama.ai";
const MANIFEST_ACCEPT: &str = "application/vnd.docker.distribution.manifest.v2+json";
const UA: &str = "distro-builder/1.0";

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl FetchError {
    fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// 'llama3.2:3b' -> (registry, namespace, model, tag); auch 'hf.co/user/repo:Q4_K_M'.
pub fn parse_model(name: &str) -> (String, String, String, String) {
    let (name, tag) = match name.split_once(':') {
        Some((n, t)) => (n, t),
        None => (name, ""),
    };
    let tag = if tag.is_empty() { "latest" } else { tag };
    let mut parts: Vec<&str> = name.split('/').collect();
    let mut host = OLLAMA_REGISTRY.to_string();
    if parts.len() > 1 && parts[0].contains('.') {
        host = parts.remove(0).to_string();
    }
    if parts.len() == 1 {
        parts.insert(0, "library");
    }
    let namespace = parts[0].to_string();
    let model = parts[1..].join("/");
    (host, namespace, model, tag.to_string())
}

fn request(url: &str, extra_headers: &[(&str, String)]) -> Result<ureq::Response, FetchError> {
    let mut req = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(std::time::Duration::from_secs(60));
    for (k, v) in extra_headers {
        req = req.set(k, v);
    }
    req.call().map_err(|e| match e {
        ureq::Error::Status(code, resp) => {
            FetchError::msg(format!("HTTP {code} für {url}: {}", resp.status_text()))
        }
        ureq::Error::Transport(t) => FetchError::msg(format!("Registry nicht erreichbar: {t}")),
    })
}

#[derive(Debug, Deserialize)]
struct ManifestLayer {
    digest: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    config: Option<ManifestLayer>,
    #[serde(default)]
    layers: Vec<ManifestLayer>,
}

struct ParsedManifest {
    raw: Vec<u8>,
    manifest: Manifest,
    host: String,
    namespace: String,
    model: String,
    tag: String,
}

fn model_manifest(name: &str) -> Result<ParsedManifest, FetchError> {
    let (host, ns, model, tag) = parse_model(name);
    let url = format!("https://{host}/v2/{ns}/{model}/manifests/{tag}");
    let resp = request(&url, &[("Accept", MANIFEST_ACCEPT.to_string())])
        .map_err(|_| FetchError::msg(format!("Modell '{name}' nicht gefunden")))?;
    let mut raw = Vec::new();
    resp.into_reader().read_to_end(&mut raw)?;
    let manifest: Manifest = serde_json::from_slice(&raw)
        .map_err(|e| FetchError::msg(format!("Ungültiges Manifest für {name}: {e}")))?;
    Ok(ParsedManifest {
        raw,
        manifest,
        host,
        namespace: ns,
        model,
        tag,
    })
}

/// Größe eines Modells in Bytes (ohne Download).
pub fn model_size(name: &str) -> Result<u64, FetchError> {
    let m = model_manifest(name)?;
    let layers: u64 = m.manifest.layers.iter().map(|l| l.size).sum();
    Ok(layers + m.manifest.config.map(|c| c.size).unwrap_or(0))
}

/// Lädt ein Modell im Ollama-Speicherformat nach `<store>/{manifests,blobs}`.
pub fn pull_model(
    name: &str,
    store: &Path,
    log: &mut dyn FnMut(&str),
    mut progress: Option<&mut dyn FnMut(f64, &str)>,
) -> Result<(), FetchError> {
    let m = model_manifest(name)?;
    let mut blobs = Vec::new();
    if let Some(c) = &m.manifest.config {
        blobs.push(ManifestLayer {
            digest: c.digest.clone(),
            size: c.size,
        });
    }
    for l in &m.manifest.layers {
        blobs.push(ManifestLayer {
            digest: l.digest.clone(),
            size: l.size,
        });
    }
    let total: u64 = blobs.iter().map(|b| b.size).sum::<u64>().max(1);
    let mut done: u64 = 0;
    fs::create_dir_all(store.join("blobs"))?;
    log(&format!(
        "Modell {name}: {:.2} GB in {} Teilen",
        total as f64 / 1024f64.powi(3),
        blobs.len()
    ));
    for b in &blobs {
        let dest = store.join("blobs").join(b.digest.replace(':', "-"));
        if dest.exists() && fs::metadata(&dest)?.len() == b.size {
            done += b.size;
            continue;
        }
        let part = dest.with_extension("part");
        let have = if part.exists() {
            fs::metadata(&part)?.len()
        } else {
            0
        };
        let url = format!(
            "https://{}/v2/{}/{}/blobs/{}",
            m.host, m.namespace, m.model, b.digest
        );
        let headers: Vec<(&str, String)> = if have > 0 {
            vec![("Range", format!("bytes={have}-"))]
        } else {
            vec![]
        };
        let resp = request(&url, &headers)
            .map_err(|e| FetchError::msg(format!("Download von {name} fehlgeschlagen: {e}")))?;
        let partial = resp.status() == 206;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(partial && have > 0)
            .truncate(!(partial && have > 0))
            .open(&part)?;
        let start = if partial && have > 0 { have } else { 0 };
        let mut got = start;
        let mut reader = resp.into_reader();
        let mut buf = [0u8; 1024 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(FetchError::Io)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            got += n as u64;
            if let Some(p) = progress.as_mut() {
                p(
                    (done + got) as f64 / total as f64,
                    &format!(
                        "{name}: {:.2} / {:.2} GB",
                        (done + got) as f64 / 1024f64.powi(3),
                        total as f64 / 1024f64.powi(3)
                    ),
                );
            }
        }
        drop(file);
        let mut hasher = Sha256::new();
        let mut verify = fs::File::open(&part)?;
        let mut vbuf = [0u8; 4 * 1024 * 1024];
        loop {
            let n = verify.read(&mut vbuf)?;
            if n == 0 {
                break;
            }
            hasher.update(&vbuf[..n]);
        }
        let digest_hex = format!("sha256:{:x}", hasher.finalize());
        if digest_hex != b.digest {
            let _ = fs::remove_file(&part);
            return Err(FetchError::msg(format!(
                "Prüfsumme von {name} stimmt nicht – bitte erneut bauen."
            )));
        }
        fs::rename(&part, &dest)?;
        done += b.size;
    }
    let mdir = store
        .join("manifests")
        .join(&m.host)
        .join(&m.namespace)
        .join(&m.model);
    fs::create_dir_all(&mdir)?;
    fs::write(mdir.join(&m.tag), &m.raw)?;
    log(&format!("Modell {name} fertig \u{2714}"));
    Ok(())
}

fn image_file(image: &str) -> String {
    let re = regex::Regex::new(r"[^a-zA-Z0-9._-]+").unwrap();
    format!("{}.tar", re.replace_all(image, "_"))
}

pub fn full_ref(image: &str) -> String {
    let mut image = image.to_string();
    let first = image.split('/').next().unwrap_or("").to_string();
    if !image.contains('/') {
        image = format!("docker.io/library/{image}");
    } else if !first.contains('.') && !first.contains(':') && first != "localhost" {
        image = format!("docker.io/{image}");
    }
    let last = image.rsplit('/').next().unwrap_or("").to_string();
    if !last.contains(':') && !image.contains('@') {
        image.push_str(":latest");
    }
    image
}

pub fn container_tool() -> Option<&'static str> {
    for tool in ["skopeo", "docker", "podman"] {
        if which(tool) {
            return Some(tool);
        }
    }
    None
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(prog).is_file()))
        .unwrap_or(false)
}

pub fn pull_image(
    image: &str,
    dest_dir: &Path,
    log: &mut dyn FnMut(&str),
) -> Result<PathBuf, FetchError> {
    fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join(image_file(image));
    if dest.exists() && fs::metadata(&dest)?.len() > 0 {
        log(&format!("Image {image}: aus Cache"));
        return Ok(dest);
    }
    let reference = full_ref(image);
    let Some(tool) = container_tool() else {
        return Err(FetchError::msg(
            "Zum Vorladen von Container-Images wird skopeo, docker oder podman benötigt (empfohlen: sudo pacman -S skopeo).",
        ));
    };
    log(&format!("Lade Image {reference} mit {tool} …"));
    let tmp = dest.with_extension("part");
    let _ = fs::remove_file(&tmp);
    let commands: Vec<Vec<String>> = if tool == "skopeo" {
        vec![vec![
            "skopeo".into(),
            "copy".into(),
            "--override-os".into(),
            "linux".into(),
            "--override-arch".into(),
            "amd64".into(),
            format!("docker://{reference}"),
            format!("docker-archive:{}:{reference}", tmp.display()),
        ]]
    } else {
        vec![
            vec![tool.to_string(), "pull".into(), reference.clone()],
            vec![
                tool.to_string(),
                "save".into(),
                "-o".into(),
                tmp.display().to_string(),
                reference.clone(),
            ],
        ]
    };
    for cmd in commands {
        let out = Command::new(&cmd[0]).args(&cmd[1..]).output()?;
        if !out.status.success() {
            let msg = if !out.stderr.is_empty() {
                out.stderr
            } else {
                out.stdout
            };
            let mut text = String::from_utf8_lossy(&msg).into_owned();
            if text.len() > 600 {
                text = text[text.len() - 600..].to_string();
            }
            return Err(FetchError::msg(format!(
                "Image {image} konnte nicht geladen werden:\n{text}"
            )));
        }
    }
    fs::rename(&tmp, &dest)?;
    log(&format!(
        "Image {image}: {:.0} MB \u{2714}",
        fs::metadata(&dest)?.len() as f64 / 1024f64.powi(2)
    ));
    Ok(dest)
}

/// Kopiert src nach dst – per Hardlink, wenn möglich (spart Platz und Zeit).
fn link_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let rel = entry.file_name();
        let dest = dst.join(&rel);
        if path.is_dir() {
            fs::create_dir_all(&dest)?;
            link_tree(&path, &dest)?;
        } else {
            if path.extension().map(|e| e == "part").unwrap_or(false) {
                continue;
            }
            if dest.exists() {
                fs::remove_file(&dest)?;
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            if fs::hard_link(&path, &dest).is_err() {
                fs::copy(&path, &dest)?;
            }
        }
    }
    Ok(())
}

/// Lädt Modelle/Images in den Cache und legt sie ins airootfs des Profils.
pub fn prepare_assets(
    cfg: &Config,
    pth: &Paths,
    log: &mut dyn FnMut(&str),
    mut progress: Option<&mut dyn FnMut(f64, &str)>,
) -> Result<(), FetchError> {
    let cache = pth.base.join("cache");
    let air = pth.profile.join("airootfs");
    let gid = &cfg.general.id;
    let models: Vec<String> = if cfg.ai.ollama {
        cfg.ai.models.clone()
    } else {
        Vec::new()
    };
    let images: Vec<String> = if cfg.containers.preload {
        C::container_images(cfg)
    } else {
        Vec::new()
    };
    let steps = models.len() + images.len();
    if steps == 0 {
        return Ok(());
    }
    let mut step = 0usize;

    if !models.is_empty() {
        let store = cache.join("ollama");
        for m in &models {
            let steps_f = steps as f64;
            let step_f = step as f64;
            let mut sub = progress.as_mut().map(|p| {
                let p: &mut dyn FnMut(f64, &str) = *p;
                move |frac: f64, text: &str| p((step_f + frac) / steps_f, text)
            });
            pull_model(
                m,
                &store,
                log,
                sub.as_mut().map(|f| f as &mut dyn FnMut(f64, &str)),
            )?;
            step += 1;
        }
        link_tree(&store, &air.join("var/lib/ollama/models"))?;
    }
    if !images.is_empty() {
        let idir = cache.join("images");
        let mut wanted = std::collections::HashSet::new();
        for img in &images {
            if let Some(p) = progress.as_mut() {
                p(
                    step as f64 / steps as f64,
                    &format!("Container-Image {img}"),
                );
            }
            let dest = pull_image(img, &idir, log)?;
            wanted.insert(dest.file_name().unwrap().to_os_string());
            step += 1;
        }
        let target = air.join("var/lib").join(gid).join("container-images");
        fs::create_dir_all(&target)?;
        for fname in wanted {
            let d = target.join(&fname);
            if !d.exists() {
                if fs::hard_link(idir.join(&fname), &d).is_err() {
                    fs::copy(idir.join(&fname), &d)?;
                }
            }
        }
    }
    if let Some(p) = progress.as_mut() {
        p(1.0, "Downloads fertig");
    }
    Ok(())
}

/// Gibt (Modelle in Bytes, Fehler) zurück – für die Größenanzeige.
pub fn estimate_sizes(cfg: &Config) -> (u64, Vec<String>) {
    let mut total = 0u64;
    let mut errors = Vec::new();
    for m in &cfg.ai.models {
        match model_size(m) {
            Ok(size) => total += size,
            Err(e) => errors.push(e.to_string()),
        }
    }
    (total, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model() {
        assert_eq!(
            parse_model("llama3.2:3b"),
            (
                "registry.ollama.ai".to_string(),
                "library".to_string(),
                "llama3.2".to_string(),
                "3b".to_string()
            )
        );
        assert_eq!(
            parse_model("phi4-mini"),
            (
                "registry.ollama.ai".to_string(),
                "library".to_string(),
                "phi4-mini".to_string(),
                "latest".to_string()
            )
        );
        assert_eq!(
            parse_model("hf.co/user/Repo-GGUF:Q4_K_M"),
            (
                "hf.co".to_string(),
                "user".to_string(),
                "Repo-GGUF".to_string(),
                "Q4_K_M".to_string()
            )
        );
    }

    #[test]
    fn test_full_ref() {
        assert_eq!(full_ref("nginx"), "docker.io/library/nginx:latest");
        assert_eq!(
            full_ref("portainer/portainer-ce"),
            "docker.io/portainer/portainer-ce:latest"
        );
        assert_eq!(full_ref("ghcr.io/a/b:main"), "ghcr.io/a/b:main");
    }
}
