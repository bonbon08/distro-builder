# KI und Container

Die Seite **Container & KI** im Builder bindet lokale Sprachmodelle und Container-Apps ein.
Beides kann vorab in die ISO gepackt werden und funktioniert dann ohne Internet.

## Ollama

| Einstellung | Wirkung |
|---|---|
| Ollama installieren | Paket `ollama`, Dienst `ollama.service` im Live- und im installierten System |
| Beschleunigung | `cpu` → `ollama`; `cuda` → `+ollama-cuda`; `rocm` → `+ollama-rocm`; `vulkan` → `+ollama-vulkan` |
| Im Netzwerk freigeben | `OLLAMA_HOST=0.0.0.0:11434`, sonst nur `127.0.0.1` |
| KI-Entwicklungsumgebung | `python-pytorch` (bzw. `-cuda`/`-rocm` passend zur Beschleunigung), `jupyterlab`, `python-numpy`, `python-pandas`, `python-scikit-learn`, `python-huggingface-hub`, `python-pip` |

Für CUDA brauchst du zusätzlich unter **Pakete** den NVIDIA-Grafiktreiber. CUDA und ROCm sind
jeweils mehrere GB groß.

### Modelle

Zur Auswahl stehen fertige Modelle (Llama 3.2/3.1, Qwen 3, Gemma 3, Mistral, DeepSeek R1,
Phi-4 mini, nomic-embed-text). Eigene Modelle trägst du so ein:

- aus [ollama.com/library](https://ollama.com/library): `qwen2.5-coder:7b`, `llama3.3:70b`
- aus anderen Namespaces: `benutzer/modell:tag`
- direkt von Hugging Face (GGUF): `hf.co/<benutzer>/<repo>:<quantisierung>`, z. B. `hf.co/bartowski/Llama-3.2-3B-Instruct-GGUF:Q4_K_M`

„Größe prüfen“ fragt bei der Registry die tatsächliche Downloadgröße ab und meldet unbekannte Namen.

**So kommen die Modelle in die ISO**

1. `distro-builder-core::fetch` lädt Manifest und Blobs direkt über die Registry-API (`registry.ollama.ai` bzw. `hf.co`). Auf dem Build-Rechner muss Ollama dafür nicht installiert sein.
2. Jeder Blob wird mit SHA256 geprüft. Abgebrochene Downloads werden fortgesetzt.
3. Gespeichert wird im Ollama-Format unter `~/DistroBuilder/<id>/cache/ollama/{manifests,blobs}`.
4. Per Hardlink (oder Kopie) landen die Dateien in `airootfs/var/lib/ollama/models`.
5. Ein systemd-Drop-in setzt `OLLAMA_MODELS=/var/lib/ollama/models`. Vor dem Start übergibt es Verzeichnisse und Manifeste an den Benutzer `ollama`. Die großen Blobs bleiben `root` gehören und lesbar, damit im Live-System nichts in den RAM kopiert werden muss.

Im Live-System und nach der Installation:

```bash
ollama list
ollama run llama3.2:3b
```

> Modelle machen die ISO groß: Bei 8–10 GB brauchst du einen entsprechend großen USB-Stick.
> Die Abbilder werden im SquashFS zwar mitkomprimiert, sparen dabei aber kaum Platz.

## Container

| Einstellung | Wirkung |
|---|---|
| Docker | `docker`, `docker-compose`, `docker-buildx`, `lazydocker`, Dienst `docker.service`; mit NVIDIA zusätzlich `nvidia-container-toolkit` |
| Podman | `podman`, `podman-compose`, `podman-docker`, `distrobox`, `podman.socket` |
| Images in die ISO packen | Images aus der Liste und aus allen Compose-Stacks werden beim Bauen geladen |

Der Benutzer kommt automatisch in die Gruppe `docker`.

### Vorgeladene Images

- Beim Bauen lädt Distro Builder jedes Image mit **skopeo** (bevorzugt, ohne Daemon und root), sonst mit `docker` oder `podman`, als `docker-archive` nach `cache/images/`.
- In der ISO liegen sie unter `/var/lib/<id>/container-images/*.tar`.
- Auf dem installierten System lädt `<id>-container-import.service` sie beim ersten Start mit `docker load` bzw. `podman load` und löscht die Tar-Dateien danach.
- Im Live-System werden die Images nicht importiert (`ConditionKernelCommandLine=!archisobasedir`), weil Docker auf einem overlayfs-Root nicht zuverlässig läuft.

Kurznamen werden ergänzt: `nginx` → `docker.io/library/nginx:latest`.

### Container-Apps (Compose-Stacks)

Vorlagen:

| Vorlage | Adresse | Hinweis |
|---|---|---|
| Open WebUI | http://localhost:3000 | Chat-Oberfläche für Ollama, `network_mode: host`, nutzt `http://127.0.0.1:11434` |
| Portainer | https://localhost:9443 | Container-Verwaltung |
| SearXNG | http://localhost:8888 | Metasuchmaschine |

Unter „Eigener Stack …“ fügst du eine beliebige `compose.yaml` ein.

Im System:

- `/etc/<id>/compose/<name>/compose.yaml`
- `<id>-compose@<name>.service` (oneshot) führt `docker compose up -d` bzw. `podman-compose up -d` aus. Aktiviert ist er, wenn der Schalter „Autostart“ am Stack an ist.
- Ist eine Web-Adresse angegeben, gibt es einen Menüeintrag, der sie im Browser öffnet.

```bash
systemctl status nebulaos-compose@open-webui
cd /etc/nebulaos/compose/open-webui && sudo docker compose logs -f
```
