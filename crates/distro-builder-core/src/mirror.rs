//! Eigener Spiegelserver: Caching-Proxy für pacman, vollständiger rsync-Spiegel,
//! eigenes Paket-Repository und ein Server-Paket (nginx/compose/systemd) zum Selbsthosten.
//!
//! Der eingebaute Server spricht nur das Nötigste von HTTP/1.1 (GET/HEAD, Range ab Byte N)
//! und bedient die pacman-Pfade `/<repo>/os/<arch>/<datei>`.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::config::{self as C, Config};
use crate::profile::{paths, quote_str};

/// Wie lange Repo-Datenbanken (core.db …) aus dem Cache kommen, bevor neu geladen wird.
const DB_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct MirrorPaths {
    pub root: PathBuf,
    /// Cache des Proxys (gleiches Layout wie ein Arch-Mirror)
    pub cache: PathBuf,
    /// eigene Repos: <repo>/<id>/os/<arch>
    pub repo: PathBuf,
    /// vollständiger rsync-Spiegel
    pub full: PathBuf,
    /// exportiertes Server-Paket
    pub kit: PathBuf,
}

pub fn mirror_paths(cfg: &Config) -> MirrorPaths {
    let root = paths(cfg).base.join("mirror");
    MirrorPaths {
        cache: root.join("cache"),
        repo: root.join("repo"),
        full: root.join("archlinux"),
        kit: root.join("server-kit"),
        root,
    }
}

/// Basis-URL für pacman.conf, ohne abschließenden Schrägstrich.
pub fn base_url(cfg: &Config) -> String {
    cfg.mirror_server
        .url
        .trim()
        .trim_end_matches('/')
        .to_string()
}

/// `Server =`-Zeile für ein Repo auf dem eigenen Spiegel.
pub fn server_line(cfg: &Config) -> String {
    format!("Server = {}/$repo/os/$arch", base_url(cfg))
}

// ---------------------------------------------------------------- Netzwerk-Helfer

/// LAN-Adresse dieses Rechners (ohne dass Pakete verschickt werden).
pub fn local_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    (!ip.is_loopback()).then(|| ip.to_string())
}

/// Ist der eingetragene Spiegel per TCP erreichbar? (kurzer Verbindungstest)
pub fn reachable(cfg: &Config) -> bool {
    let url = base_url(cfg);
    let (https, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (scheme == "https", rest),
        None => (false, url.as_str()),
    };
    let hostport = rest.split('/').next().unwrap_or_default();
    let addr = if hostport.contains(':') {
        hostport.to_string()
    } else {
        format!("{hostport}:{}", if https { 443 } else { 80 })
    };
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
        .is_some_and(|sa| TcpStream::connect_timeout(&sa, Duration::from_secs(2)).is_ok())
}

/// Braucht der Build den Spiegel zwingend (eigenes Repo oder keine Ausweichquelle)?
pub fn required_for_build(cfg: &Config) -> bool {
    let m = &cfg.mirror_server;
    m.enabled && (m.own_repo || !m.fallback)
}

pub fn suggested_url(port: u16) -> String {
    format!(
        "http://{}:{port}",
        local_ip().unwrap_or_else(|| "localhost".into())
    )
}

// ---------------------------------------------------------------- Eigenes Repo

pub fn own_repo_dir(cfg: &Config) -> PathBuf {
    mirror_paths(cfg)
        .repo
        .join(&cfg.general.id)
        .join("os")
        .join(C::pacman_arch(cfg))
}

pub fn own_repo_db(cfg: &Config) -> PathBuf {
    own_repo_dir(cfg).join(format!("{}.db", cfg.general.id))
}

fn is_package_file(name: &str) -> bool {
    name.contains(".pkg.tar") && !name.ends_with(".sig")
}

/// Paketdateien im eigenen Repo (sortiert).
pub fn list_packages(cfg: &Config) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(own_repo_dir(cfg))
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| is_package_file(n))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Paketname aus einem Dateinamen wie `name-1.2-3-x86_64.pkg.tar.zst`.
pub fn package_name(file: &str) -> String {
    let stem = file.split(".pkg.tar").next().unwrap_or(file);
    let parts: Vec<&str> = stem.rsplitn(4, '-').collect();
    if parts.len() == 4 {
        parts[3].to_string()
    } else {
        stem.to_string()
    }
}

fn run_tool(cmd: &mut Command, log: &mut dyn FnMut(&str)) -> Result<(), MirrorError> {
    let out = cmd.output().map_err(|e| {
        MirrorError::Msg(format!(
            "{} konnte nicht gestartet werden ({e}) – ist pacman installiert?",
            cmd.get_program().to_string_lossy()
        ))
    })?;
    for line in String::from_utf8_lossy(&out.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&out.stderr).lines())
    {
        if !line.trim().is_empty() {
            log(line);
        }
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(MirrorError::Msg(format!(
            "{} ist fehlgeschlagen.",
            cmd.get_program().to_string_lossy()
        )))
    }
}

/// Kopiert Pakete ins eigene Repo und trägt sie mit `repo-add` ein (ältere Versionen fliegen raus).
pub fn add_packages(
    cfg: &Config,
    files: &[PathBuf],
    log: &mut dyn FnMut(&str),
) -> Result<usize, MirrorError> {
    let dir = own_repo_dir(cfg);
    fs::create_dir_all(&dir)?;
    let mut added = Vec::new();
    for src in files {
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !is_package_file(&name) {
            log(&format!("Übersprungen (kein Paket): {name}"));
            continue;
        }
        let dest = dir.join(&name);
        fs::copy(src, &dest)?;
        let sig = PathBuf::from(format!("{}.sig", src.display()));
        if sig.is_file() {
            fs::copy(&sig, dir.join(format!("{name}.sig")))?;
        }
        added.push(dest);
    }
    if added.is_empty() {
        return Ok(0);
    }
    let db = dir.join(format!("{}.db.tar.gz", cfg.general.id));
    run_tool(
        Command::new("repo-add")
            .arg("--new")
            .arg("--remove")
            .arg(&db)
            .args(&added),
        log,
    )?;
    Ok(added.len())
}

/// Entfernt ein Paket aus dem eigenen Repo.
pub fn remove_package(
    cfg: &Config,
    file: &str,
    log: &mut dyn FnMut(&str),
) -> Result<(), MirrorError> {
    let dir = own_repo_dir(cfg);
    let db = dir.join(format!("{}.db.tar.gz", cfg.general.id));
    run_tool(
        Command::new("repo-remove").arg(&db).arg(package_name(file)),
        log,
    )?;
    let _ = fs::remove_file(dir.join(file));
    let _ = fs::remove_file(dir.join(format!("{file}.sig")));
    Ok(())
}

// ---------------------------------------------------------------- Server

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    /// proxy | full
    pub mode: String,
    pub upstream: String,
    pub cache: PathBuf,
    pub full: PathBuf,
    pub repo: PathBuf,
    /// Name des eigenen Repos (leer = keins)
    pub own_repo: String,
    pub title: String,
}

impl ServerConfig {
    pub fn from_config(cfg: &Config) -> Self {
        let p = mirror_paths(cfg);
        let m = &cfg.mirror_server;
        Self {
            port: m.port,
            mode: m.mode.clone(),
            upstream: C::mirror_upstream(cfg),
            cache: p.cache,
            full: p.full,
            repo: p.repo,
            own_repo: if m.own_repo {
                cfg.general.id.clone()
            } else {
                String::new()
            },
            title: cfg.general.name.clone(),
        }
    }
}

pub struct ServerHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    pub port: u16,
    pub served_bytes: Arc<AtomicU64>,
}

impl ServerHandle {
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub type LogFn = Arc<dyn Fn(String) + Send + Sync>;

/// Startet den Spiegelserver in einem Hintergrund-Thread (lauscht auf allen Adressen).
pub fn start_server(sc: ServerConfig, log: LogFn) -> io::Result<ServerHandle> {
    let listener = TcpListener::bind(("0.0.0.0", sc.port))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let served = Arc::new(AtomicU64::new(0));
    let sc = Arc::new(sc);
    let thread = {
        let stop = stop.clone();
        let served = served.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        let sc = sc.clone();
                        let log = log.clone();
                        let served = served.clone();
                        std::thread::spawn(move || {
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                            if let Some((line, bytes)) = handle(stream, &sc) {
                                served.fetch_add(bytes, Ordering::Relaxed);
                                log(format!("{} {line}", peer.ip()));
                            }
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        })
    };
    Ok(ServerHandle {
        stop,
        thread: Some(thread),
        port,
        served_bytes: served,
    })
}

struct Request {
    method: String,
    path: String,
    range_start: Option<u64>,
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut range_start = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("range") {
                range_start = v
                    .trim()
                    .strip_prefix("bytes=")
                    .and_then(|r| r.strip_suffix('-'))
                    .and_then(|n| n.parse().ok());
            }
        }
    }
    let raw_path = target.split('?').next().unwrap_or("/");
    Some(Request {
        method,
        path: percent_decode(raw_path),
        range_start,
    })
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && !s.contains('\\')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+-:@~".contains(c))
}

fn is_db_file(name: &str) -> bool {
    [
        ".db",
        ".files",
        ".db.sig",
        ".files.sig",
        ".db.tar.gz",
        ".files.tar.gz",
    ]
    .iter()
    .any(|s| name.ends_with(s))
}

/// Bearbeitet eine Verbindung. Gibt (Logzeile, gesendete Bytes) zurück.
fn handle(mut stream: TcpStream, sc: &ServerConfig) -> Option<(String, u64)> {
    let req = read_request(&stream)?;
    let head = req.method == "HEAD";
    if req.method != "GET" && !head {
        respond_text(&mut stream, 405, "Nur GET und HEAD");
        return Some((format!("{} {} → 405", req.method, req.path), 0));
    }
    if req.path == "/" {
        let body = index_page(sc);
        respond(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            body.as_bytes(),
            head,
        );
        return Some(("GET / → Übersicht".into(), body.len() as u64));
    }
    let segs: Vec<&str> = req.path.trim_start_matches('/').split('/').collect();
    if segs.len() != 4 || segs[1] != "os" || !segs.iter().all(|s| safe_segment(s)) {
        respond_text(&mut stream, 404, "Nicht gefunden");
        return Some((format!("{} → 404", req.path), 0));
    }
    let (repo, arch, file) = (segs[0], segs[2], segs[3]);
    let label = format!("{repo}/{file}");

    // 1. eigenes Repo
    if !sc.own_repo.is_empty() && repo == sc.own_repo {
        let path = sc.repo.join(repo).join("os").join(arch).join(file);
        return Some(serve_file(
            &mut stream,
            &path,
            req.range_start,
            head,
            &label,
            "eigenes Repo",
        ));
    }
    // 2. vollständiger Spiegel
    if sc.mode == "full" {
        let path = sc.full.join(repo).join("os").join(arch).join(file);
        return Some(serve_file(
            &mut stream,
            &path,
            req.range_start,
            head,
            &label,
            "Spiegel",
        ));
    }
    // 3. Caching-Proxy
    let path = sc.cache.join(repo).join("os").join(arch).join(file);
    let fresh = fs::metadata(&path)
        .ok()
        .map(|m| {
            !is_db_file(file)
                || m.modified()
                    .ok()
                    .and_then(|t| SystemTime::now().duration_since(t).ok())
                    .is_some_and(|age| age < DB_TTL)
        })
        .unwrap_or(false);
    let mut source = "Cache";
    if !fresh {
        let url = format!(
            "{}/{file}",
            sc.upstream.replace("$repo", repo).replace("$arch", arch)
        );
        match download(&url, &path) {
            Ok(()) => source = "geladen",
            Err(code) if path.exists() => {
                // Upstream nicht erreichbar: veraltete Datenbank ist besser als keine
                source = if code == 404 {
                    "Cache"
                } else {
                    "Cache (veraltet)"
                };
            }
            Err(code) => {
                respond_text(&mut stream, code, "Upstream liefert die Datei nicht");
                return Some((format!("{label} → {code} (Upstream)"), 0));
            }
        }
    }
    Some(serve_file(
        &mut stream,
        &path,
        req.range_start,
        head,
        &label,
        source,
    ))
}

/// Lädt `url` nach `dest` (über eine .part-Datei). Fehler = HTTP-Status für den Client.
fn download(url: &str, dest: &Path) -> Result<(), u16> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .user_agent(concat!("distro-builder-mirror/", env!("CARGO_PKG_VERSION")))
        .build();
    let resp = match agent.get(url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => return Err(code),
        Err(_) => return Err(502),
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| 500u16)?;
    }
    let tmp = dest.with_file_name(format!(
        ".{}.{:?}.part",
        dest.file_name().unwrap_or_default().to_string_lossy(),
        std::thread::current().id()
    ));
    let result = (|| -> io::Result<()> {
        let mut out = fs::File::create(&tmp)?;
        io::copy(&mut resp.into_reader(), &mut out)?;
        out.sync_all()?;
        fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(502);
    }
    Ok(())
}

fn serve_file(
    stream: &mut TcpStream,
    path: &Path,
    range_start: Option<u64>,
    head: bool,
    label: &str,
    source: &str,
) -> (String, u64) {
    let Ok(mut file) = fs::File::open(path) else {
        respond_text(stream, 404, "Nicht gefunden");
        return (format!("{label} → 404"), 0);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = range_start.unwrap_or(0);
    if start > len || (start == len && len > 0) {
        let _ = write!(
            stream,
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        return (format!("{label} → 416"), 0);
    }
    let partial = range_start.is_some() && start > 0;
    let mut header = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n",
        if partial { "206 Partial Content" } else { "200 OK" },
        len - start
    );
    if partial {
        header.push_str(&format!(
            "Content-Range: bytes {start}-{}/{len}\r\n",
            len.saturating_sub(1)
        ));
    }
    header.push_str("\r\n");
    if stream.write_all(header.as_bytes()).is_err() {
        return (format!("{label} → abgebrochen"), 0);
    }
    let mut sent = 0;
    if !head && file.seek(SeekFrom::Start(start)).is_ok() {
        sent = io::copy(&mut (&mut file).take(len - start), stream).unwrap_or(0);
    }
    (
        format!(
            "{label} → {} ({source}, {})",
            if partial { 206 } else { 200 },
            human_size(len)
        ),
        sent,
    )
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8], head: bool) {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Bad Gateway",
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if !head {
        let _ = stream.write_all(body);
    }
}

fn respond_text(stream: &mut TcpStream, code: u16, text: &str) {
    respond(
        stream,
        code,
        "text/plain; charset=utf-8",
        text.as_bytes(),
        false,
    );
}

fn index_page(sc: &ServerConfig) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let (what, size) = if sc.mode == "full" {
        ("Vollständiger Spiegel", dir_size(&sc.full))
    } else {
        ("Caching-Proxy", dir_size(&sc.cache))
    };
    let mut html = format!(
        "<!doctype html><meta charset=utf-8><title>{t} – Spiegelserver</title>\
         <body style=\"font-family:sans-serif;max-width:40em;margin:2em auto\">\
         <h1>{t} – Spiegelserver</h1><p>{what}, {} auf der Platte.</p>",
        human_size(size),
        t = esc(&sc.title)
    );
    if sc.mode != "full" {
        html.push_str(&format!(
            "<p>Quelle: <code>{}</code></p>",
            esc(&sc.upstream)
        ));
    }
    html.push_str("<p>In <code>/etc/pacman.conf</code>:</p><pre>[core]\nServer = http://&lt;diese Adresse&gt;/$repo/os/$arch</pre>");
    if !sc.own_repo.is_empty() {
        html.push_str(&format!(
            "<p>Eigenes Repository: <code>[{}]</code></p>",
            esc(&sc.own_repo)
        ));
    }
    html.push_str("<p><small>Distro Builder</small></p></body>");
    html
}

pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024f64.powi(3) {
        format!("{:.2} GB", b / 1024f64.powi(3))
    } else if b >= 1024f64.powi(2) {
        format!("{:.1} MB", b / 1024f64.powi(2))
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}

// ---------------------------------------------------------------- vollständiger Spiegel

/// rsync-Skript für einen vollständigen Spiegel der genutzten Repos (nur x86_64).
pub fn sync_script(cfg: &Config) -> String {
    let m = &cfg.mirror_server;
    let repos = C::official_repos(cfg)
        .iter()
        .map(|r| quote_str(r))
        .collect::<Vec<_>>()
        .join(" ");
    let default_dir = format!("/srv/{}-mirror/archlinux", cfg.general.id);
    format!(
        r#"#!/bin/bash
# Erzeugt von Distro Builder – synchronisiert einen Arch-Linux-Spiegel für {name} per rsync.
# Nur die Repos, die die Distro nutzt. Aufruf z. B. stündlich per systemd-Timer.
set -euo pipefail
MIRROR_DIR="${{MIRROR_DIR:-{default_dir}}}"
UPSTREAM="${{UPSTREAM:-{upstream}}}"
BWLIMIT="${{BWLIMIT:-0}}"
REPOS=({repos})
ARCH=x86_64

mkdir -p -- "$MIRROR_DIR"
exec 9>"$MIRROR_DIR/.sync.lock"
flock -n 9 || {{ echo "Synchronisation läuft bereits."; exit 0; }}

FILTER=()
for r in "${{REPOS[@]}}"; do
  FILTER+=(--include="/$r/" --include="/$r/os/" --include="/$r/os/$ARCH/" --include="/$r/os/$ARCH/**")
done
echo "==> Synchronisiere ${{REPOS[*]}} von $UPSTREAM nach $MIRROR_DIR"
rsync -rlptH --safe-links --delete-delay --delay-updates --timeout=600 --contimeout=60 \
  --no-motd --info=progress2 --bwlimit="$BWLIMIT" \
  "${{FILTER[@]}}" --include=/lastupdate --include=/lastsync --exclude='*' \
  "$UPSTREAM" "$MIRROR_DIR/"
date +%s > "$MIRROR_DIR/lastsync_local"
echo "==> Fertig: $(du -sh "$MIRROR_DIR" | cut -f1)"
"#,
        name = cfg.general.name,
        upstream = m.rsync_upstream.trim(),
    )
}

// ---------------------------------------------------------------- Server-Paket

/// nginx-Konfiguration: Caching-Proxy bzw. statischer Spiegel plus eigenes Repo.
pub fn nginx_conf(cfg: &Config) -> String {
    let m = &cfg.mirror_server;
    let gid = &cfg.general.id;
    let own = if m.own_repo {
        format!(
            "\n    # Eigenes Paket-Repository [{gid}]\n    location ^~ /{gid}/ {{\n      root /srv/repo;\n      autoindex on;\n    }}\n"
        )
    } else {
        String::new()
    };
    if m.mode == "full" {
        return format!(
            "# Erzeugt von Distro Builder – statischer Arch-Linux-Spiegel\nevents {{}}\nhttp {{\n  include /etc/nginx/mime.types;\n  sendfile on;\n  server {{\n    listen 80;\n    root /srv/mirror;\n    autoindex on;\n{own}  }}\n}}\n"
        );
    }
    // Upstream-Vorlage (…/$repo/os/$arch) in Host und Pfad zerlegen und als rewrite abbilden
    let upstream = C::mirror_upstream(cfg);
    let (scheme_host, path_tpl) = match upstream.find("://") {
        Some(i) => match upstream[i + 3..].find('/') {
            Some(j) => (
                upstream[..i + 3 + j].to_string(),
                upstream[i + 3 + j..].to_string(),
            ),
            None => (upstream.clone(), String::new()),
        },
        None => (upstream.clone(), String::new()),
    };
    let host = scheme_host
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let rewrite = format!(
        "{}/$3",
        path_tpl.replace("$repo", "$1").replace("$arch", "$2")
    );
    format!(
        r#"# Erzeugt von Distro Builder – Caching-Proxy für pacman
# Pakete werden 90 Tage gecacht, Repo-Datenbanken nie (sie ändern sich ständig).
events {{}}
http {{
  proxy_cache_path /var/cache/nginx/pacman levels=1:2 keys_zone=pacman:20m
                   max_size=100g inactive=90d use_temp_path=off;
  resolver 1.1.1.1 9.9.9.9 valid=300s ipv6=off;
  server {{
    listen 80;
{own}
    # Repo-Datenbanken immer frisch holen
    location ~ "^/([^/]+)/os/([^/]+)/(.+\.(db|files)(\.sig|\.tar\.gz)?)$" {{
      rewrite "^/([^/]+)/os/([^/]+)/(.*)$" "{rewrite}" break;
      proxy_pass {scheme_host};
      proxy_set_header Host {host};
      proxy_ssl_server_name on;
      proxy_cache off;
    }}
    # Pakete und Signaturen cachen
    location ~ "^/([^/]+)/os/([^/]+)/(.+)$" {{
      rewrite "^/([^/]+)/os/([^/]+)/(.*)$" "{rewrite}" break;
      proxy_pass {scheme_host};
      proxy_set_header Host {host};
      proxy_ssl_server_name on;
      proxy_cache pacman;
      proxy_cache_valid 200 90d;
      proxy_cache_lock on;
      proxy_cache_use_stale error timeout updating;
      add_header X-Cache $upstream_cache_status;
    }}
  }}
}}
"#
    )
}

pub fn compose_yaml(cfg: &Config) -> String {
    let m = &cfg.mirror_server;
    let mut volumes = vec!["      - ./nginx.conf:/etc/nginx/nginx.conf:ro".to_string()];
    if m.mode == "full" {
        volumes.push("      - ./archlinux:/srv/mirror:ro".into());
    } else {
        volumes.push("      - cache:/var/cache/nginx".into());
    }
    if m.own_repo {
        volumes.push("      - ./repo:/srv/repo:ro".into());
    }
    let mut out = format!(
        "# Erzeugt von Distro Builder – Spiegelserver für {}\nservices:\n  mirror:\n    image: docker.io/library/nginx:alpine\n    ports:\n      - \"{}:80\"\n    volumes:\n{}\n    restart: unless-stopped\n",
        cfg.general.name,
        m.port,
        volumes.join("\n")
    );
    if m.mode != "full" {
        out.push_str("volumes:\n  cache: {}\n");
    }
    out
}

fn kit_readme(cfg: &Config) -> String {
    let m = &cfg.mirror_server;
    let gid = &cfg.general.id;
    let mut s = format!(
        "# Spiegelserver für {name}\n\nErzeugt von Distro Builder. Dieser Ordner enthält alles, um den Spiegel auf einem\neigenen Server (oder einem Rechner im LAN) dauerhaft zu betreiben.\n\n",
        name = cfg.general.name
    );
    if m.mode == "full" {
        s.push_str(&format!(
            "## Vollständiger Spiegel\n\n1. Ordner auf den Server kopieren, z. B. nach `/srv/{gid}-mirror`.\n2. Erste Synchronisation (dauert, 80 GB und mehr):\n   ```bash\n   MIRROR_DIR=/srv/{gid}-mirror/archlinux bash sync-mirror.sh\n   ```\n3. Stündlich aktualisieren:\n   ```bash\n   sudo cp {gid}-mirror-sync.service {gid}-mirror-sync.timer /etc/systemd/system/\n   sudo systemctl enable --now {gid}-mirror-sync.timer\n   ```\n4. Ausliefern: `docker compose up -d` (nginx auf Port {port}).\n\n",
            port = m.port
        ));
    } else {
        s.push_str(&format!(
            "## Caching-Proxy\n\nnginx lädt jedes Paket beim ersten Abruf von `{}` und hält es danach 90 Tage vor.\nRepo-Datenbanken werden nie gecacht.\n\n```bash\ndocker compose up -d      # Port {}\n```\n\nOhne Docker: `nginx.conf` nach `/etc/nginx/nginx.conf` kopieren und nginx starten.\n\n",
            C::mirror_upstream(cfg),
            m.port
        ));
    }
    if m.own_repo {
        s.push_str(&format!(
            "## Eigenes Repository\n\n`repo/{gid}/os/{arch}` enthält deine Pakete samt Datenbank. Neue Pakete fügst du in Distro\nBuilder hinzu und exportierst das Server-Paket erneut, oder direkt auf dem Server:\n\n```bash\nrepo-add --new --remove repo/{gid}/os/{arch}/{gid}.db.tar.gz neues-paket.pkg.tar.zst\n```\n\n",
            arch = C::pacman_arch(cfg)
        ));
    }
    s.push_str(&format!(
        "## In der Distro\n\nDistro Builder trägt den Spiegel vor den offiziellen Mirrors in `pacman.conf` ein:\n\n```ini\n[core]\nServer = {}/$repo/os/$arch\n{}\n```\n\nAlternativ ohne Docker/nginx: `distro-builder --mirror-server projekt.json` startet den\neingebauten Server (Caching-Proxy, vollständiger Spiegel und eigenes Repo).\n",
        if m.url.trim().is_empty() {
            "http://<adresse>".to_string()
        } else {
            base_url(cfg)
        },
        if m.fallback {
            "Include = /etc/pacman.d/mirrorlist"
        } else {
            ""
        }
    ));
    s
}

/// Schreibt das Server-Paket nach `<projekt>/mirror/server-kit` und gibt den Ordner zurück.
pub fn write_server_kit(cfg: &Config) -> Result<PathBuf, MirrorError> {
    let p = mirror_paths(cfg);
    let kit = p.kit;
    if kit.exists() {
        fs::remove_dir_all(&kit)?;
    }
    fs::create_dir_all(&kit)?;
    let gid = &cfg.general.id;
    fs::write(kit.join("README.md"), kit_readme(cfg))?;
    fs::write(kit.join("nginx.conf"), nginx_conf(cfg))?;
    fs::write(kit.join("compose.yaml"), compose_yaml(cfg))?;
    if cfg.mirror_server.mode == "full" {
        let script = kit.join("sync-mirror.sh");
        fs::write(&script, sync_script(cfg))?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
        fs::write(
            kit.join(format!("{gid}-mirror-sync.service")),
            format!(
                "[Unit]\nDescription={} Spiegel synchronisieren\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=oneshot\nEnvironment=MIRROR_DIR=/srv/{gid}-mirror/archlinux\nExecStart=/bin/bash /srv/{gid}-mirror/sync-mirror.sh\nNice=10\nIOSchedulingClass=idle\n",
                cfg.general.name
            ),
        )?;
        fs::write(
            kit.join(format!("{gid}-mirror-sync.timer")),
            format!(
                "[Unit]\nDescription={} Spiegel stündlich synchronisieren\n\n[Timer]\nOnCalendar=hourly\nRandomizedDelaySec=15m\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
                cfg.general.name
            ),
        )?;
    }
    if cfg.mirror_server.own_repo && p.repo.is_dir() {
        copy_tree(&p.repo, &kit.join("repo"))?;
    }
    Ok(kit)
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path())?, target)?;
        } else if ty.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn http_get(port: u16, path: &str, range: Option<u64>) -> (u16, Vec<u8>, String) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut req = format!("GET {path} HTTP/1.1\r\nHost: x\r\n");
        if let Some(r) = range {
            req.push_str(&format!("Range: bytes={r}-\r\n"));
        }
        req.push_str("\r\n");
        s.write_all(req.as_bytes()).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&buf[..split]).into_owned();
        let code = head[9..12].parse().unwrap();
        (code, buf[split + 4..].to_vec(), head)
    }

    fn test_config(tmp: &Path, upstream: &str) -> ServerConfig {
        ServerConfig {
            port: 0,
            mode: "proxy".into(),
            upstream: upstream.into(),
            cache: tmp.join("cache"),
            full: tmp.join("full"),
            repo: tmp.join("repo"),
            own_repo: "meinos".into(),
            title: "Test".into(),
        }
    }

    #[test]
    fn reachability() {
        let tmp = tempfile::tempdir().unwrap();
        let server = start_server(test_config(tmp.path(), ""), Arc::new(|_| {})).unwrap();
        let mut cfg = C::default_config();
        cfg.mirror_server.enabled = true;
        cfg.mirror_server.own_repo = true;
        cfg.mirror_server.url = format!("http://127.0.0.1:{}/", server.port);
        assert!(required_for_build(&cfg));
        assert!(reachable(&cfg));
        let port = server.port;
        server.stop();
        cfg.mirror_server.url = format!("http://127.0.0.1:{port}");
        assert!(!reachable(&cfg));
        cfg.mirror_server.own_repo = false;
        assert!(
            !required_for_build(&cfg),
            "mit Ausweichquelle nicht zwingend"
        );
    }

    #[test]
    fn package_names() {
        assert_eq!(package_name("yay-12.4.2-1-x86_64.pkg.tar.zst"), "yay");
        assert_eq!(
            package_name("python-foo-bar-1:2.0-3-any.pkg.tar.zst"),
            "python-foo-bar"
        );
        assert!(is_package_file("a-1-1-any.pkg.tar.zst"));
        assert!(!is_package_file("a-1-1-any.pkg.tar.zst.sig"));
    }

    #[test]
    fn paths_are_sanitized() {
        assert!(safe_segment("core.db"));
        assert!(safe_segment("glibc-2.40+r16-1-x86_64.pkg.tar.zst"));
        assert!(!safe_segment(".."));
        assert!(!safe_segment(""));
        assert!(!safe_segment("a b"));
        assert_eq!(
            percent_decode("/core/os/x86_64/a%3A1"),
            "/core/os/x86_64/a:1"
        );
        assert_eq!(percent_decode("/ä%"), "/ä%");
    }

    /// Ein zweiter eingebauter Server spielt den Upstream; der Proxy lädt, cacht und
    /// liefert Range-Anfragen aus – ganz ohne Internet.
    #[test]
    fn proxy_caches_and_serves_ranges() {
        let tmp = tempfile::tempdir().unwrap();
        // "Upstream" = vollständiger Spiegel mit einer Paketdatei
        let up_dir = tmp.path().join("upstream");
        let pkg_dir = up_dir.join("full/core/os/x86_64");
        fs::create_dir_all(&pkg_dir).unwrap();
        let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(pkg_dir.join("hello-1-1-x86_64.pkg.tar.zst"), &payload).unwrap();
        fs::write(pkg_dir.join("core.db"), b"db-v1").unwrap();
        let mut up_cfg = test_config(&up_dir, "");
        up_cfg.mode = "full".into();
        let log: LogFn = Arc::new(|_| {});
        let upstream = start_server(up_cfg, log.clone()).unwrap();

        let proxy_dir = tmp.path().join("proxy");
        let proxy_cfg = test_config(
            &proxy_dir,
            &format!("http://127.0.0.1:{}/$repo/os/$arch", upstream.port),
        );
        // eigenes Repo mit einer Datei
        let own = proxy_dir.join("repo/meinos/os/x86_64");
        fs::create_dir_all(&own).unwrap();
        fs::write(own.join("meinos.db"), b"own-db").unwrap();
        let proxy = start_server(proxy_cfg, log).unwrap();

        let (code, body, _) = http_get(
            proxy.port,
            "/core/os/x86_64/hello-1-1-x86_64.pkg.tar.zst",
            None,
        );
        assert_eq!(code, 200);
        assert_eq!(body, payload);
        assert!(proxy_dir
            .join("cache/core/os/x86_64/hello-1-1-x86_64.pkg.tar.zst")
            .is_file());
        let (code, body, _) = http_get(proxy.port, "/core/os/x86_64/core.db", None);
        assert_eq!((code, body.as_slice()), (200, &b"db-v1"[..]));

        // Upstream weg -> Paket kommt weiter aus dem Cache, Range wird unterstützt
        upstream.stop();
        let (code, body, head) = http_get(
            proxy.port,
            "/core/os/x86_64/hello-1-1-x86_64.pkg.tar.zst",
            Some(150_000),
        );
        assert_eq!(code, 206, "{head}");
        assert_eq!(body, payload[150_000..]);
        assert!(head.contains("Content-Range: bytes 150000-199999/200000"));

        // Datenbank: im Cache noch frisch genug, obwohl der Upstream weg ist
        let (code, body, _) = http_get(proxy.port, "/core/os/x86_64/core.db", None);
        assert_eq!((code, body.as_slice()), (200, &b"db-v1"[..]));

        // eigenes Repo, unbekannte Datei, böse Pfade
        let (code, body, _) = http_get(proxy.port, "/meinos/os/x86_64/meinos.db", None);
        assert_eq!((code, body.as_slice()), (200, &b"own-db"[..]));
        assert_eq!(
            http_get(proxy.port, "/core/os/x86_64/fehlt.pkg.tar.zst", None).0,
            502
        );
        assert_eq!(
            http_get(proxy.port, "/../../etc/os/x86_64/passwd", None).0,
            404
        );
        assert_eq!(http_get(proxy.port, "/", None).0, 200);
        proxy.stop();
    }

    /// Baut ein minimales Paket (nur .PKGINFO) – reicht repo-add vollkommen.
    fn fake_package(dir: &Path, name: &str, ver: &str) -> PathBuf {
        let src = dir.join(format!("src-{name}-{ver}"));
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join(".PKGINFO"),
            format!("pkgname = {name}\npkgbase = {name}\npkgver = {ver}\npkgdesc = Test\nbuilddate = 1700000000\npackager = Test\nsize = 0\narch = any\n"),
        )
        .unwrap();
        let pkg = dir.join(format!("{name}-{ver}-any.pkg.tar.gz"));
        let ok = Command::new("tar")
            .arg("-czf")
            .arg(&pkg)
            .arg("-C")
            .arg(&src)
            .arg(".PKGINFO")
            .status()
            .unwrap()
            .success();
        assert!(ok);
        pkg
    }

    #[test]
    fn own_repo_with_repo_add() {
        if Command::new("repo-add").arg("--help").output().is_err() {
            eprintln!("übersprungen: repo-add nicht vorhanden");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = C::default_config();
        cfg.build.project_dir = tmp.path().join("proj").to_string_lossy().into_owned();
        cfg.mirror_server.own_repo = true;
        let mut log = |_: &str| {};
        let v1 = fake_package(tmp.path(), "hallo", "1.0-1");
        let v2 = fake_package(tmp.path(), "hallo", "2.0-1");
        assert_eq!(add_packages(&cfg, &[v1], &mut log).unwrap(), 1);
        assert_eq!(add_packages(&cfg, &[v2], &mut log).unwrap(), 1);
        assert!(own_repo_db(&cfg).exists());
        // --remove: die alte Version fliegt raus
        assert_eq!(
            list_packages(&cfg),
            vec!["hallo-2.0-1-any.pkg.tar.gz".to_string()]
        );

        let mut sc = ServerConfig::from_config(&cfg);
        sc.port = 0;
        let server = start_server(sc, Arc::new(|_| {})).unwrap();
        let (code, body, _) = http_get(server.port, "/meinos/os/x86_64/meinos.db", None);
        assert_eq!(code, 200);
        assert!(body.len() > 20, "Datenbank ist leer");
        server.stop();

        remove_package(&cfg, "hallo-2.0-1-any.pkg.tar.gz", &mut log).unwrap();
        assert!(list_packages(&cfg).is_empty());
    }

    #[test]
    fn kit_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = C::default_config();
        cfg.build.project_dir = tmp.path().to_string_lossy().into_owned();
        cfg.mirror_server.enabled = true;
        cfg.mirror_server.url = "http://192.168.1.10:8080".into();
        let kit = write_server_kit(&cfg).unwrap();
        let nginx = fs::read_to_string(kit.join("nginx.conf")).unwrap();
        assert!(nginx.contains("proxy_pass https://geo.mirror.pkgbuild.com;"));
        assert!(nginx.contains(r#""/$1/os/$2/$3""#), "{nginx}");
        assert!(fs::read_to_string(kit.join("compose.yaml"))
            .unwrap()
            .contains("\"8080:80\""));

        cfg.mirror_server.mode = "full".into();
        let kit = write_server_kit(&cfg).unwrap();
        let script = kit.join("sync-mirror.sh");
        let out = Command::new("bash")
            .arg("-n")
            .arg(&script)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = fs::read_to_string(&script).unwrap();
        assert!(text.contains("REPOS=(\"core\" \"extra\")"), "{text}");
        assert!(kit.join("meinos-mirror-sync.timer").is_file());

        // ARM-Upstream: anderes Pfad-Layout ($arch/$repo) wird per rewrite abgebildet
        cfg.mirror_server.mode = "proxy".into();
        cfg.build.arch = "rpi".into();
        let nginx = nginx_conf(&cfg);
        assert!(nginx.contains(r#""/$2/$1/$3""#), "{nginx}");
    }
}
