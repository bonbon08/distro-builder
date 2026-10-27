"""Tests für Konfiguration, Profil-Erzeugung und Installer-Vorlagen.

Die Profil-Tests brauchen die releng-Vorlage von archiso. Pfad über
DISTRO_BUILDER_RELENG überschreibbar, sonst /usr/share/archiso/configs/releng.
"""

import json
import os
import py_compile
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from distrobuilder import config as C  # noqa: E402
from distrobuilder import fetch as F  # noqa: E402
from distrobuilder import profile as P  # noqa: E402

RELENG = os.environ.get("DISTRO_BUILDER_RELENG", P.RELENG)
TEMPLATES = P.TEMPLATES


def make_cfg(tmp, **overrides):
    cfg = C.default_config()
    cfg["general"].update(name="Test OS", id="testos", hostname="testos")
    cfg["build"]["project_dir"] = tmp
    for section, values in overrides.items():
        cfg[section].update(values)
    return cfg


class ConfigTests(unittest.TestCase):
    def test_defaults_valid(self):
        errors, _ = C.validate(C.default_config())
        self.assertEqual(errors, [])

    def test_invalid_id_and_packages(self):
        cfg = C.default_config()
        cfg["general"]["id"] = "Mein OS"
        cfg["packages"]["custom"] = "gut b@d$name"
        errors, _ = C.validate(cfg)
        self.assertTrue(any("ID" in e for e in errors))
        self.assertTrue(any("b@d$name" in e for e in errors))

    def test_steam_needs_multilib(self):
        cfg = C.default_config()
        cfg["packages"]["presets"].append("gaming")
        self.assertTrue(any("multilib" in e for e in C.validate(cfg)[0]))

    def test_target_packages_and_services(self):
        cfg = C.default_config()
        cfg["packages"].update(kernel="linux-zen", gpu="nvidia")
        cfg["containers"]["engine"] = "docker"
        cfg["ai"].update(ollama=True, accel="cuda")
        pkgs = C.target_packages(cfg)
        for p in ("linux-zen", "linux-zen-headers", "nvidia-open-dkms", "docker", "nvidia-container-toolkit",
                  "ollama", "ollama-cuda"):
            self.assertIn(p, pkgs)
        self.assertEqual(len(pkgs), len(set(pkgs)))
        svcs = C.target_services(cfg)
        self.assertIn("docker.service", svcs)
        self.assertIn("ollama.service", svcs)
        self.assertIn("sddm.service", svcs)

    def test_live_contains_target(self):
        cfg = C.default_config()
        cfg["packages"]["kernel"] = "linux-lts"
        live = C.live_packages(cfg, ["base", "linux"])
        self.assertTrue(set(C.target_packages(cfg)) <= set(live))
        self.assertIn("squashfs-tools", live)

    def test_container_images_from_stacks(self):
        cfg = C.default_config()
        cfg["containers"].update(engine="docker", images="nginx:alpine\npostgres:17")
        title, _desc, compose = C.STACK_PRESETS["open-webui"]
        cfg["containers"]["stacks"] = [{"name": "open-webui", "title": title, "compose": compose}]
        self.assertEqual(C.container_images(cfg),
                         ["nginx:alpine", "postgres:17", "ghcr.io/open-webui/open-webui:main"])

    def test_save_load_roundtrip(self):
        cfg = C.default_config()
        cfg["general"]["name"] = "Rundreise"
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "p.json")
            C.save(cfg, path)
            self.assertEqual(C.load(path), cfg)


class PacmanConfTests(unittest.TestCase):
    def test_repo_order(self):
        cfg = C.default_config()
        cfg["repos"].update(multilib=True, custom=[
            {"name": "vorne", "server": "https://a/$arch", "siglevel": "Optional TrustAll", "first": True},
            {"name": "hinten", "server": "https://b/$arch", "siglevel": "Never", "first": False}])
        text = P.pacman_conf(cfg)
        order = [text.index(f"[{r}]") for r in ("vorne", "core", "extra", "multilib", "hinten")]
        self.assertEqual(order, sorted(order))
        self.assertNotIn("[core-testing]", text)


class FetchTests(unittest.TestCase):
    def test_parse_model(self):
        self.assertEqual(F.parse_model("llama3.2:3b"), ("registry.ollama.ai", "library", "llama3.2", "3b"))
        self.assertEqual(F.parse_model("phi4-mini"), ("registry.ollama.ai", "library", "phi4-mini", "latest"))
        self.assertEqual(F.parse_model("hf.co/user/Repo-GGUF:Q4_K_M"), ("hf.co", "user", "Repo-GGUF", "Q4_K_M"))

    def test_full_ref(self):
        self.assertEqual(F._full_ref("nginx"), "docker.io/library/nginx:latest")
        self.assertEqual(F._full_ref("portainer/portainer-ce"), "docker.io/portainer/portainer-ce:latest")
        self.assertEqual(F._full_ref("ghcr.io/a/b:main"), "ghcr.io/a/b:main")


class TemplateTests(unittest.TestCase):
    def test_python_templates_compile(self):
        with tempfile.TemporaryDirectory() as tmp:
            for fn in ("installer.py", "backend.py"):
                py_compile.compile(os.path.join(TEMPLATES, fn), doraise=True, cfile=os.path.join(tmp, fn + "c"))

    def test_shell_templates_syntax(self):
        for fn in ("apply-branding.sh", "set-wallpaper.sh", "live-setup.sh"):
            r = subprocess.run(["bash", "-n", os.path.join(TEMPLATES, fn)], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)


@unittest.skipUnless(os.path.isdir(RELENG), "archiso releng-Vorlage nicht vorhanden")
class ProfileTests(unittest.TestCase):
    def generate(self, tmp, **overrides):
        cfg = make_cfg(tmp, **overrides)
        return cfg, P.generate(cfg, log=lambda *_: None, releng=RELENG)

    def check_bash(self, path):
        r = subprocess.run(["bash", "-n", path], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_all_desktops(self):
        for desktop in C.DESKTOPS:
            with self.subTest(desktop=desktop), tempfile.TemporaryDirectory() as tmp:
                _cfg, pth = self.generate(tmp, packages={"desktop": desktop})
                air = os.path.join(pth["profile"], "airootfs")
                self.check_bash(os.path.join(pth["profile"], "profiledef.sh"))
                self.check_bash(pth["script"])
                with open(os.path.join(pth["profile"], "profiledef.sh")) as f:
                    self.assertIn('iso_name="testos"', f.read())
                self.assertTrue(os.path.isfile(os.path.join(air, "usr/share/testos/os-release")))
                has_installer = desktop != "none"
                self.assertEqual(os.path.isfile(os.path.join(air, "etc/distro-installer.json")), has_installer)
                if has_installer:
                    self.assertTrue(os.path.islink(os.path.join(air, "etc/systemd/system/display-manager.service")))

    def test_installer_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            _cfg, pth = self.generate(tmp, installer={"encrypt": True, "bootloader": "systemd-boot"})
            with open(os.path.join(pth["profile"], "airootfs/etc/distro-installer.json")) as f:
                conf = json.load(f)
            self.assertIn("/etc/mkinitcpio.conf.d/archiso.conf", conf["live_files"])
            self.assertIn("/usr/local/bin/live-setup", conf["live_files"])
            for keep in ("/etc/passwd", "/etc/shadow", "/etc/resolv.conf", "/etc/pacman.conf"):
                self.assertNotIn(keep, conf["live_files"])
            self.assertTrue(conf["defaults"]["encrypt"])
            self.assertEqual(conf["defaults"]["bootloader"], "systemd-boot")

    def test_ai_and_containers(self):
        with tempfile.TemporaryDirectory() as tmp:
            title, _d, compose = C.STACK_PRESETS["portainer"]
            _cfg, pth = self.generate(
                tmp, ai={"ollama": True, "models": ["llama3.2:3b"], "expose": True},
                containers={"engine": "docker", "stacks": [{"name": "portainer", "title": title,
                                                            "compose": compose, "autostart": True}]})
            air = os.path.join(pth["profile"], "airootfs")
            with open(os.path.join(air, "etc/systemd/system/ollama.service.d/10-testos.conf")) as f:
                dropin = f.read()
            self.assertIn("OLLAMA_MODELS=/var/lib/ollama/models", dropin)
            self.assertIn("OLLAMA_HOST=0.0.0.0", dropin)
            self.assertTrue(os.path.isfile(os.path.join(air, "etc/testos/compose/portainer/compose.yaml")))
            self.check_bash(os.path.join(air, "usr/local/bin/testos-container-import"))
            with open(os.path.join(air, "etc/distro-installer.json")) as f:
                conf = json.load(f)
            self.assertIn("testos-compose@portainer.service", conf["services"])
            self.assertIn("/var/lib/ollama/models", conf["data_paths"])


if __name__ == "__main__":
    unittest.main()
