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


class ArchTests(unittest.TestCase):
    def test_arm_packages(self):
        cfg = C.default_config()
        cfg["build"]["arch"] = "rpi"
        pkgs = C.target_packages(cfg)
        self.assertIn("linux-rpi", pkgs)
        self.assertIn("raspberrypi-bootloader", pkgs)
        for p in ("linux", "intel-ucode", "amd-ucode", "grub"):
            self.assertNotIn(p, pkgs)
        cfg["build"]["arch"] = "aarch64"
        pkgs = C.target_packages(cfg)
        self.assertIn("linux-aarch64", pkgs)
        self.assertIn("grub", pkgs)
        self.assertEqual(C.oci_arch(cfg), "arm64")
        self.assertFalse(C.installer_active(cfg))

    def test_kernel_choice(self):
        cfg = C.default_config()
        cfg["build"]["arch"] = "rpi"
        cfg["packages"]["kernel"] = "linux-rpi-16k"
        self.assertEqual(C.kernel(cfg), "linux-rpi-16k")
        cfg["build"]["arch"] = "x86_64"
        self.assertEqual(C.kernel(cfg), "linux")

    def test_arm_validation(self):
        cfg = C.default_config()
        cfg["build"]["arch"] = "aarch64"
        cfg["packages"].update(kernel="linux-aarch64")
        self.assertEqual(C.validate(cfg)[0], [])
        cfg["packages"]["gpu"] = "nvidia"
        cfg["repos"]["multilib"] = True
        cfg["ai"].update(ollama=True, accel="cuda")
        cfg["image"]["password"] = ""
        errors = C.validate(cfg)[0]
        for word in ("Grafiktreiber", "multilib", "CUDA", "Startpasswort"):
            self.assertTrue(any(word in e for e in errors), word)
        cfg["build"]["arch"] = "sparc"
        self.assertTrue(any("Architektur" in e for e in C.validate(cfg)[0]))


class AppsTests(unittest.TestCase):
    def test_flatpak(self):
        cfg = C.default_config()
        cfg["flatpak"].update(enabled=True, apps=["org.mozilla.firefox", "kde:org.kde.kate"])
        self.assertEqual(C.flatpak_apps(cfg), [("flathub", "org.mozilla.firefox"), ("kde", "org.kde.kate")])
        self.assertTrue(any("kde" in e for e in C.validate(cfg)[0]))
        cfg["flatpak"]["remotes"] = [{"name": "kde", "url": "https://distribute.kde.org/kdeapps.flatpakrepo"}]
        self.assertEqual(C.validate(cfg)[0], [])
        self.assertIn("flatpak", C.target_packages(cfg))
        self.assertIn("discover", C.target_packages(cfg))
        cfg["flatpak"]["apps"].append("kein app")
        self.assertTrue(any("kein app" in e for e in C.validate(cfg)[0]))

    def test_flatpak_x86_only_on_arm(self):
        cfg = C.default_config()
        cfg["build"]["arch"] = "rpi"
        cfg["packages"]["kernel"] = "linux-rpi"
        cfg["flatpak"].update(enabled=True, apps=["com.valvesoftware.Steam"])
        self.assertTrue(any("x86_64" in e for e in C.validate(cfg)[0]))

    def test_distrobox(self):
        cfg = C.default_config()
        cfg["distrobox"].update(enabled=True, boxes=[{"name": "ubuntu", "image": "docker.io/library/ubuntu:24.04",
                                                      "packages": "git", "export": ""}])
        self.assertEqual(C.validate(cfg)[0], [])
        pkgs = C.target_packages(cfg)
        self.assertIn("distrobox", pkgs)
        self.assertIn("podman", pkgs)
        self.assertIn("podman.socket", C.target_services(cfg))
        ini = P.distrobox_ini(cfg)
        self.assertIn("[ubuntu]", ini)
        self.assertIn('additional_packages="git"', ini)
        cfg["distrobox"]["boxes"].append(dict(cfg["distrobox"]["boxes"][0]))
        self.assertTrue(any("eindeutig" in e for e in C.validate(cfg)[0]))


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

    def test_arm_repos(self):
        cfg = C.default_config()
        cfg["build"]["arch"] = "rpi"
        cfg["repos"].update(multilib=True, core_testing=True)
        text = P.pacman_conf(cfg)
        self.assertIn("Architecture = aarch64", text)
        self.assertIn("[alarm]", text)
        self.assertNotIn("[multilib]", text)
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
        for fn in ("apply-branding.sh", "set-wallpaper.sh", "live-setup.sh", "image-setup.sh"):
            r = subprocess.run(["bash", "-n", os.path.join(TEMPLATES, fn)], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)


class ImageProfileTests(unittest.TestCase):
    """ARM-Images brauchen keine releng-Vorlage."""

    def check_bash(self, path):
        r = subprocess.run(["bash", "-n", path], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_platforms(self):
        for plat in ("aarch64", "rpi"):
            with self.subTest(plat=plat), tempfile.TemporaryDirectory() as tmp:
                cfg = make_cfg(tmp, build={"arch": plat}, image={"autologin": True},
                               flatpak={"enabled": True, "apps": ["org.mozilla.firefox"]},
                               distrobox={"enabled": True, "boxes": [{"name": "debian", "image": "debian:12"}]})
                cfg["packages"]["kernel"] = C.kernel(cfg)
                pth = P.generate(cfg, log=lambda *_: None)
                prof = pth["profile"]
                air = os.path.join(prof, "airootfs")
                self.check_bash(pth["script"])
                self.check_bash(os.path.join(prof, "setup.sh"))
                with open(pth["script"]) as f:
                    script = f.read()
                self.assertIn(C.ALARM_TARBALLS[plat], script)
                self.assertIn("label: dos" if plat == "rpi" else "label: gpt", script)
                self.assertIn("--arch=aarch64", script)
                with open(os.path.join(prof, "setup.sh")) as f:
                    setup = f.read()
                self.assertIn('"testos-growfs.service"', setup)
                self.assertIn('"testos-flatpak-setup.service"', setup)
                self.assertNotIn("@@VARS@@", setup)
                with open(os.path.join(prof, "packages.txt")) as f:
                    self.assertIn(C.kernel(cfg), f.read().split())
                for rel in ("etc/sddm.conf.d/autologin.conf", "etc/hostname", "etc/testos/distrobox.ini",
                            "etc/systemd/user/default.target.wants/testos-distrobox.service"):
                    self.assertTrue(os.path.lexists(os.path.join(air, rel)), rel)
                for exe in ("testos-growfs", "testos-flatpak-setup", "testos-apply-branding"):
                    self.check_bash(os.path.join(air, "usr/local/bin", exe))
                    self.assertTrue(os.access(os.path.join(air, "usr/local/bin", exe), os.X_OK))
                self.assertFalse(os.path.exists(os.path.join(air, "etc/distro-installer.json")))


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

    def test_flatpak_and_distrobox(self):
        with tempfile.TemporaryDirectory() as tmp:
            _cfg, pth = self.generate(
                tmp, flatpak={"enabled": True, "apps": ["org.gimp.GIMP"]},
                distrobox={"enabled": True, "boxes": [{"name": "fedora",
                                                       "image": "registry.fedoraproject.org/fedora-toolbox:42"}]})
            air = os.path.join(pth["profile"], "airootfs")
            with open(os.path.join(air, "etc/distro-installer.json")) as f:
                conf = json.load(f)
            self.assertIn("testos-flatpak-setup.service", conf["services"])
            self.assertIn("/var/lib/testos/flatpak.tar", conf["data_paths"])
            self.assertIn("distrobox", conf["packages"])
            with open(pth["script"]) as f:
                self.assertIn("var/lib/testos/flatpak.tar", f.read())
            self.assertTrue(os.path.isdir(os.path.join(air, "var/lib/testos")))
            # auch im Overlay für die Online-Installation
            overlay = os.path.join(air, "usr/share/testos/overlay")
            for rel in ("etc/testos/distrobox.ini", "etc/systemd/system/testos-flatpak-setup.service",
                        "etc/systemd/user/default.target.wants/testos-distrobox.service"):
                self.assertTrue(os.path.lexists(os.path.join(overlay, rel)), rel)


if __name__ == "__main__":
    unittest.main()
