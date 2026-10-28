"""Lädt Zusatzdaten für die ISO herunter: Ollama-Modelle und Container-Images.

Alles wird in <Projekt>/cache zwischengespeichert, damit wiederholte Builds nichts neu laden.
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import urllib.error
import urllib.request

from . import config as C

OLLAMA_REGISTRY = "registry.ollama.ai"
MANIFEST_ACCEPT = "application/vnd.docker.distribution.manifest.v2+json"
UA = "distro-builder/1.0"
# Pfad im Live-/Zielsystem (per systemd-Drop-in als OLLAMA_MODELS gesetzt)
OLLAMA_MODELS_DIR = "/var/lib/ollama/models"


class FetchError(Exception):
    pass


def parse_model(name):
    """'llama3.2:3b' -> (registry, namespace, model, tag); auch 'hf.co/user/repo:Q4_K_M'."""
    name, _, tag = name.partition(":")
    tag = tag or "latest"
    parts = name.split("/")
    host = OLLAMA_REGISTRY
    if len(parts) > 1 and "." in parts[0]:
        host = parts.pop(0)
    if len(parts) == 1:
        parts.insert(0, "library")
    return host, parts[0], "/".join(parts[1:]), tag


def _request(url, headers=None):
    req = urllib.request.Request(url, headers={"User-Agent": UA, **(headers or {})})
    return urllib.request.urlopen(req, timeout=60)


def model_manifest(name):
    host, ns, model, tag = parse_model(name)
    url = f"https://{host}/v2/{ns}/{model}/manifests/{tag}"
    try:
        with _request(url, {"Accept": MANIFEST_ACCEPT}) as r:
            raw = r.read()
    except urllib.error.HTTPError as e:
        raise FetchError(f"Modell '{name}' nicht gefunden ({e.code})") from e
    except urllib.error.URLError as e:
        raise FetchError(f"Registry nicht erreichbar: {e.reason}") from e
    return raw, json.loads(raw), (host, ns, model, tag)


def model_size(name):
    """Größe eines Modells in Bytes (ohne Download)."""
    _raw, man, _ = model_manifest(name)
    return sum(l.get("size", 0) for l in man.get("layers", [])) + man.get("config", {}).get("size", 0)


def pull_model(name, store, log, progress=None):
    """Lädt ein Modell im Ollama-Speicherformat nach <store>/{manifests,blobs}."""
    raw, man, (host, ns, model, tag) = model_manifest(name)
    blobs = [man["config"]] + man.get("layers", [])
    total = sum(b.get("size", 0) for b in blobs) or 1
    done = 0
    os.makedirs(os.path.join(store, "blobs"), exist_ok=True)
    log(f"Modell {name}: {total / 1024**3:.2f} GB in {len(blobs)} Teilen")
    for b in blobs:
        digest = b["digest"]
        dest = os.path.join(store, "blobs", digest.replace(":", "-"))
        size = b.get("size", 0)
        if os.path.exists(dest) and os.path.getsize(dest) == size:
            done += size
            continue
        part = dest + ".part"
        have = os.path.getsize(part) if os.path.exists(part) else 0
        headers = {"Range": f"bytes={have}-"} if have else {}
        url = f"https://{host}/v2/{ns}/{model}/blobs/{digest}"
        try:
            with _request(url, headers) as r, open(part, "ab" if have and r.status == 206 else "wb") as f:
                if r.status != 206:
                    have = 0
                got = have
                while chunk := r.read(1024 * 1024):
                    f.write(chunk)
                    got += len(chunk)
                    if progress:
                        progress((done + got) / total, f"{name}: {(done + got) / 1024**3:.2f} / "
                                                       f"{total / 1024**3:.2f} GB")
        except urllib.error.URLError as e:
            raise FetchError(f"Download von {name} fehlgeschlagen: {e}") from e
        h = hashlib.sha256()
        with open(part, "rb") as f:
            while chunk := f.read(4 * 1024 * 1024):
                h.update(chunk)
        if "sha256:" + h.hexdigest() != digest:
            os.remove(part)
            raise FetchError(f"Prüfsumme von {name} stimmt nicht – bitte erneut bauen.")
        os.replace(part, dest)
        done += size
    mdir = os.path.join(store, "manifests", host, ns, model)
    os.makedirs(mdir, exist_ok=True)
    with open(os.path.join(mdir, tag), "wb") as f:
        f.write(raw)
    log(f"Modell {name} fertig ✔")


def _image_file(image):
    return re.sub(r"[^a-zA-Z0-9._-]+", "_", image) + ".tar"


def _full_ref(image):
    first = image.split("/")[0]
    if "/" not in image:
        image = f"docker.io/library/{image}"
    elif "." not in first and ":" not in first and first != "localhost":
        image = f"docker.io/{image}"
    if ":" not in image.split("/")[-1] and "@" not in image:
        image += ":latest"
    return image


def container_tool():
    for tool in ("skopeo", "docker", "podman"):
        if shutil.which(tool):
            return tool
    return None


def pull_image(image, dest_dir, log, arch="amd64"):
    os.makedirs(dest_dir, exist_ok=True)
    dest = os.path.join(dest_dir, _image_file(image))
    if os.path.exists(dest) and os.path.getsize(dest) > 0:
        log(f"Image {image}: aus Cache")
        return dest
    ref = _full_ref(image)
    tool = container_tool()
    if not tool:
        raise FetchError("Zum Vorladen von Container-Images wird skopeo, docker oder podman benötigt "
                         "(empfohlen: sudo pacman -S skopeo).")
    log(f"Lade Image {ref} mit {tool} …")
    tmp = dest + ".part"
    if os.path.exists(tmp):
        os.remove(tmp)
    if tool == "skopeo":
        cmds = [["skopeo", "copy", "--override-os", "linux", "--override-arch", arch,
                 f"docker://{ref}", f"docker-archive:{tmp}:{ref}"]]
    else:
        cmds = [[tool, "pull", "--platform", f"linux/{arch}", ref], [tool, "save", "-o", tmp, ref]]
    for cmd in cmds:
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            raise FetchError(f"Image {image} konnte nicht geladen werden:\n{(r.stderr or r.stdout).strip()[-600:]}")
    os.replace(tmp, dest)
    log(f"Image {image}: {os.path.getsize(dest) / 1024**2:.0f} MB ✔")
    return dest


def _link_tree(src, dst):
    """Kopiert src nach dst – per Hardlink, wenn möglich (spart Platz und Zeit)."""
    for root, _dirs, files in os.walk(src):
        rel = os.path.relpath(root, src)
        os.makedirs(os.path.join(dst, rel), exist_ok=True)
        for fn in files:
            if fn.endswith(".part"):
                continue
            s, d = os.path.join(root, fn), os.path.join(dst, rel, fn)
            if os.path.exists(d):
                os.remove(d)
            try:
                os.link(s, d)
            except OSError:
                shutil.copy2(s, d)


def prepare_assets(cfg, pth, log, progress=None):
    """Lädt Modelle/Images in den Cache und legt sie ins airootfs des Profils."""
    cache = os.path.join(pth["base"], "cache")
    air = os.path.join(pth["profile"], "airootfs")
    gid = cfg["general"]["id"]
    models = cfg["ai"]["models"] if cfg["ai"]["ollama"] else []
    images = C.container_images(cfg) if cfg["containers"]["preload"] else []
    steps = len(models) + len(images)
    if not steps:
        return
    step = 0

    def sub(frac, text):
        if progress:
            progress((step + frac) / steps, text)

    if models:
        store = os.path.join(cache, "ollama")
        for m in models:
            pull_model(m, store, log, sub)
            step += 1
        _link_tree(store, air + OLLAMA_MODELS_DIR)
    if images:
        oci_arch = C.oci_arch(cfg)
        # getrennte Caches pro Architektur, damit ein Wechsel keine falschen Images mitbringt
        idir = os.path.join(cache, "images" if oci_arch == "amd64" else f"images-{oci_arch}")
        wanted = set()
        for img in images:
            sub(0, f"Container-Image {img}")
            wanted.add(os.path.basename(pull_image(img, idir, log, oci_arch)))
            step += 1
        target = os.path.join(air, "var/lib", gid, "container-images")
        os.makedirs(target, exist_ok=True)
        for fn in wanted:
            d = os.path.join(target, fn)
            if not os.path.exists(d):
                try:
                    os.link(os.path.join(idir, fn), d)
                except OSError:
                    shutil.copy2(os.path.join(idir, fn), d)
    if progress:
        progress(1.0, "Downloads fertig")


def estimate_sizes(cfg, log):
    """Gibt (Modelle in Bytes, Fehler) zurück – für die Größenanzeige."""
    total, errors = 0, []
    for m in cfg["ai"]["models"]:
        try:
            size = model_size(m)
            total += size
            log(m, size)
        except FetchError as e:
            errors.append(str(e))
            log(m, None)
    return total, errors
