#!/usr/bin/env python3
"""@@NAME@@ Installer – Backend (läuft als root).

Aufruf: backend.py <auswahl.json>
Ausgabe: Logzeilen auf stdout, Fortschritt als '##PROGRESS <0..1> <Text>'.

Ablauf: Partitionieren (mehrere Festplatten, optional LUKS2) -> System kopieren
(offline aus dem SquashFS der ISO oder online per pacstrap) -> Live-Reste entfernen
-> konfigurieren -> Bootloader.
"""

import glob
import json
import os
import re
import secrets
import shutil
import subprocess
import sys

CONF = "/etc/distro-installer.json"
TARGET = "/mnt/@@ID@@-target"
SFS_CANDIDATES = ["/run/archiso/copytoram/airootfs.sfs", "/run/archiso/bootmnt/arch/x86_64/airootfs.sfs"]

CONSOLE_KEYMAPS = {"de": "de-latin1", "at": "de-latin1", "ch": "de_CH-latin1", "gb": "uk",
                   "fr": "fr-latin1", "se": "sv-latin1", "no": "no-latin1", "dk": "dk-latin1",
                   "pt": "pt-latin1", "tr": "trq", "be": "be-latin1", "br": "br-abnt2"}

opened_luks = []


def out(msg):
    print(msg, flush=True)


def progress(frac, text):
    out(f"##PROGRESS {frac:.3f} {text}")


def run(cmd, check=True, stdin=None, quiet=False):
    if not quiet:
        out("$ " + " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         stdin=subprocess.PIPE if stdin is not None else subprocess.DEVNULL, text=True)
    if stdin is not None:
        p.stdin.write(stdin)
        p.stdin.close()
    for line in p.stdout:
        out(line.rstrip("\n"))
    rc = p.wait()
    if check and rc != 0:
        raise RuntimeError(f"Befehl fehlgeschlagen ({rc}): {' '.join(cmd)}")
    return rc


def capture(cmd, check=True):
    return subprocess.run(cmd, capture_output=True, text=True, check=check).stdout.strip()


def chroot(*cmd, check=True, stdin=None):
    return run(["arch-chroot", TARGET, *cmd], check=check, stdin=stdin)


def chroot_out(*cmd):
    return capture(["arch-chroot", TARGET, *cmd], check=False)


def write(path, content, mode=None):
    full = TARGET + path
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(content)
    if mode is not None:
        os.chmod(full, mode)


def append(path, content):
    full = TARGET + path
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "a", encoding="utf-8") as f:
        f.write(content)


def partitions(disk):
    r = capture(["lsblk", "-lnpo", "NAME,TYPE", disk])
    return [l.split()[0] for l in r.splitlines() if l.split()[1:] == ["part"]]


def uuid(dev):
    return capture(["blkid", "-s", "UUID", "-o", "value", dev])


def console_keymap(layout):
    km = CONSOLE_KEYMAPS.get(layout, layout)
    if glob.glob(f"/usr/share/kbd/keymaps/**/{km}.map.gz", recursive=True):
        return km
    return "us"


# ---------------------------------------------------------------- Festplatten
def wipe(disk):
    for part in partitions(disk):
        run(["umount", "-q", part], check=False, quiet=True)
        run(["swapoff", part], check=False, quiet=True)
    run(["wipefs", "-af", disk])
    run(["sgdisk", "--zap-all", disk])


def settle(disk, expected):
    run(["partprobe", disk], check=False)
    run(["udevadm", "settle"], check=False)
    parts = partitions(disk)
    if len(parts) < expected:
        raise RuntimeError(f"Partitionen auf {disk} wurden nicht erkannt.")
    return parts


def luks_format(part, name, passphrase=None, keyfile=None):
    """Erstellt und öffnet einen LUKS2-Container, gibt /dev/mapper/<name> zurück."""
    out(f"Verschlüssele {part} (LUKS2) …")
    if keyfile:
        run(["cryptsetup", "luksFormat", "--type", "luks2", "--batch-mode", "--key-file", keyfile, part])
        if passphrase:
            run(["cryptsetup", "luksAddKey", "--key-file", keyfile, "--batch-mode", part, "-"],
                stdin=passphrase)
        run(["cryptsetup", "open", "--key-file", keyfile, part, name])
    else:
        run(["cryptsetup", "luksFormat", "--type", "luks2", "--batch-mode", "--key-file", "-", part],
            stdin=passphrase)
        run(["cryptsetup", "open", "--key-file", "-", part, name], stdin=passphrase)
    opened_luks.append(name)
    return f"/dev/mapper/{name}"


def mkfs(fs, devices, label, raid=False):
    if fs == "btrfs":
        cmd = ["mkfs.btrfs", "-f", "-L", label[:16]]
        if raid:
            cmd += ["-d", "raid1", "-m", "raid1"]
        run(cmd + devices)
    else:
        run(["mkfs.ext4", "-F", "-L", label[:16], devices[0]])


def prepare_disks(sel, conf, uefi):
    """Partitioniert alle Festplatten und hängt alles unter TARGET ein.
    Gibt Infos für Bootloader/crypttab zurück."""
    gid, fs = conf["id"], sel["filesystem"]
    encrypt, pw = sel.get("encrypt"), sel.get("luks_password") or ""
    disk = sel["disk"]
    extras = sel.get("extra_disks", [])
    info = {"luks_root": [], "crypttab": [], "fs": fs, "swap": None}

    # Systemplatte
    wipe(disk)
    root_type = "8309" if encrypt else "8304"
    if uefi:
        run(["sgdisk", "-n1:0:+1G", "-t1:ef00", "-c1:EFI", "-n2:0:0", f"-t2:{root_type}", f"-c2:{gid}", disk])
        boot_part, root_part = settle(disk, 2)[:2]
    else:
        run(["sgdisk", "-n1:0:+1M", "-t1:ef02", "-c1:BIOS", "-n2:0:+1G", "-t2:8300", "-c2:boot",
             "-n3:0:0", f"-t3:{root_type}", f"-c3:{gid}", disk])
        _bios, boot_part, root_part = settle(disk, 3)[:3]

    # RAID1-Spiegel (nur btrfs)
    raid_parts = []
    for i, ex in enumerate(e for e in extras if e["role"] == "raid"):
        wipe(ex["path"])
        run(["sgdisk", "-n1:0:0", f"-t1:{root_type}", f"-c1:{gid}-mirror{i}", ex["path"]])
        raid_parts.append(settle(ex["path"], 1)[0])

    progress(0.05, "Dateisysteme werden erstellt")
    root_devs = []
    for i, part in enumerate([root_part] + raid_parts):
        if encrypt:
            name = "cryptroot" if i == 0 else f"cryptroot{i + 1}"
            root_devs.append(luks_format(part, name, passphrase=pw))
            info["luks_root"].append((uuid(part), name))
        else:
            root_devs.append(part)
    mkfs(fs, root_devs, gid, raid=len(root_devs) > 1)
    info["root_dev"] = root_devs[0]
    info["root_uuid"] = uuid(root_devs[0])

    separate_home = any(e["role"] == "home" for e in extras)
    os.makedirs(TARGET, exist_ok=True)
    if fs == "btrfs":
        if len(root_devs) > 1:
            run(["btrfs", "device", "scan"], check=False)
        run(["mount", root_devs[0], TARGET])
        subvols = ["@", "@log", "@pkg", "@snapshots"] + ([] if separate_home else ["@home"])
        if sel.get("swap_gb") and len(root_devs) == 1:
            subvols.append("@swap")
        for sub in subvols:
            run(["btrfs", "subvolume", "create", f"{TARGET}/{sub}"])
        run(["umount", TARGET])
        opts = "compress=zstd,noatime"
        run(["mount", "-o", f"subvol=@,{opts}", root_devs[0], TARGET])
        mounts = [("@log", "var/log"), ("@pkg", "var/cache/pacman/pkg"), ("@snapshots", ".snapshots")]
        if not separate_home:
            mounts.append(("@home", "home"))
        if "@swap" in subvols:
            mounts.append(("@swap", "swap"))
        for sub, mnt in mounts:
            os.makedirs(f"{TARGET}/{mnt}", exist_ok=True)
            run(["mount", "-o", f"subvol={sub},{'noatime' if sub == '@swap' else opts}", root_devs[0],
                 f"{TARGET}/{mnt}"])
    else:
        run(["mount", root_devs[0], TARGET])

    # Bootpartition
    os.makedirs(f"{TARGET}/boot", exist_ok=True)
    if uefi:
        run(["mkfs.fat", "-F32", "-n", "EFI", boot_part])
        run(["mount", "-o", "fmask=0077,dmask=0077", boot_part, f"{TARGET}/boot"])
    else:
        run(["mkfs.ext4", "-F", "-L", "boot", boot_part])
        run(["mount", boot_part, f"{TARGET}/boot"])

    # /home und Datenlaufwerke
    data_n = 0
    for ex in extras:
        if ex["role"] not in ("home", "data"):
            continue
        wipe(ex["path"])
        run(["sgdisk", "-n1:0:0", f"-t1:{'8309' if encrypt else ('8302' if ex['role'] == 'home' else '8300')}",
             ex["path"]])
        part = settle(ex["path"], 1)[0]
        if ex["role"] == "home":
            mnt, name, label = "/home", "crypthome", "home"
        else:
            data_n += 1
            suffix = "" if data_n == 1 else str(data_n)
            mnt, name, label = f"/data{suffix}", f"cryptdata{suffix}", f"data{suffix}"
        dev = part
        if encrypt:
            # Schlüsseldatei liegt auf der (verschlüsselten) Systempartition -> nur ein Passwort beim Start
            keydir = f"{TARGET}/etc/cryptsetup-keys.d"
            os.makedirs(keydir, mode=0o700, exist_ok=True)
            keyfile = f"{keydir}/{name}.key"
            with open(keyfile, "wb") as f:
                f.write(secrets.token_bytes(512))
            os.chmod(keyfile, 0o400)
            dev = luks_format(part, name, passphrase=pw, keyfile=keyfile)
            info["crypttab"].append(f"{name} UUID={uuid(part)} /etc/cryptsetup-keys.d/{name}.key luks,discard")
        mkfs("btrfs" if fs == "btrfs" else "ext4", [dev], label)
        os.makedirs(TARGET + mnt, exist_ok=True)
        run(["mount", "-o", "compress=zstd,noatime" if fs == "btrfs" else "defaults", dev, TARGET + mnt])
    return info


def make_swap(sel, info):
    gb = int(sel.get("swap_gb") or 0)
    if gb <= 0:
        return
    if info["fs"] == "btrfs":
        if len(info["luks_root"]) > 1 or not os.path.isdir(f"{TARGET}/swap"):
            out("Hinweis: Swapfile auf btrfs-RAID wird nicht unterstützt – übersprungen.")
            return
        path = "/swap/swapfile"
        run(["btrfs", "filesystem", "mkswapfile", "--size", f"{gb}g", "--uuid", "clear", TARGET + path])
    else:
        path = "/swapfile"
        run(["mkswap", "-U", "clear", "--size", f"{gb}G", "--file", TARGET + path])
    append("/etc/fstab", f"\n# Swapfile\n{path} none swap defaults 0 0\n")
    info["swap"] = path


# ---------------------------------------------------------------- System kopieren
def find_sfs():
    for p in SFS_CANDIDATES:
        if os.path.exists(p):
            return p
    found = glob.glob("/run/archiso/bootmnt/*/x86_64/airootfs.sfs")
    return found[0] if found else None


def install_offline(conf, sel):
    sfs = find_sfs()
    if not sfs:
        raise RuntimeError("Das System-Abbild der ISO (airootfs.sfs) wurde nicht gefunden. "
                           "Bitte die Online-Installation wählen.")
    progress(0.10, "System wird von der ISO kopiert")
    cmd = ["unsquashfs", "-f", "-d", TARGET, "-percentage", sfs]
    out("$ " + " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    last = -1
    for line in p.stdout:
        line = line.strip()
        if line.isdigit():
            pct = int(line)
            if pct != last:
                last = pct
                progress(0.10 + 0.50 * pct / 100, f"System wird von der ISO kopiert ({pct} %)")
        elif line:
            out(line)
    if p.wait() != 0:
        raise RuntimeError("Kopieren des System-Abbilds ist fehlgeschlagen.")

    progress(0.61, "Live-System-Reste werden entfernt")
    for rel in conf.get("live_files", []):
        path = os.path.join(TARGET, rel.lstrip("/"))
        if os.path.isdir(path) and not os.path.islink(path):
            shutil.rmtree(path, ignore_errors=True)
        elif os.path.lexists(path):
            os.remove(path)
    for rel in sorted(conf.get("live_dirs", []), key=len, reverse=True):
        try:
            os.rmdir(os.path.join(TARGET, rel.lstrip("/")))
        except OSError:
            pass

    progress(0.63, "Paketschlüssel werden eingerichtet")
    shutil.rmtree(f"{TARGET}/etc/pacman.d/gnupg", ignore_errors=True)
    chroot("pacman-key", "--init")
    chroot("pacman-key", "--populate")
    for repo_key in conf.get("extra_keys", []):
        chroot("pacman-key", "--lsign-key", repo_key, check=False)
    chroot("gpgconf", "--homedir", "/etc/pacman.d/gnupg", "--kill", "all", check=False)

    progress(0.66, "Nur-Live-Pakete werden entfernt")
    installed = set(chroot_out("pacman", "-Qq").split())
    keep = set()
    for name in target_packages(conf, sel):
        if name in installed:
            keep.add(name)
        else:
            keep.update(x for x in chroot_out("pacman", "-Qgq", name).split() if x in installed)
    run(["arch-chroot", TARGET, "pacman", "-D", "--asdeps", *sorted(installed)], quiet=True, check=False)
    run(["arch-chroot", TARGET, "pacman", "-D", "--asexplicit", *sorted(keep)], quiet=True, check=False)
    for _ in range(5):
        orphans = chroot_out("pacman", "-Qtdq").split()
        if not orphans:
            break
        out(f"Entferne {len(orphans)} Pakete, die nur im Live-System gebraucht werden")
        chroot("pacman", "-Rns", "--noconfirm", *orphans, check=False)

    # mkarchiso leert /boot – Kernel aus /usr/lib/modules wiederherstellen
    progress(0.70, "Kernel wird eingerichtet")
    for pkgbase_file in glob.glob(f"{TARGET}/usr/lib/modules/*/pkgbase"):
        with open(pkgbase_file, encoding="utf-8") as f:
            pkgbase = f.read().strip()
        src = os.path.join(os.path.dirname(pkgbase_file), "vmlinuz")
        if os.path.exists(src):
            shutil.copyfile(src, f"{TARGET}/boot/vmlinuz-{pkgbase}")
            out(f"Kernel: vmlinuz-{pkgbase}")
    # Die Sync-Datenbanken entfernt mkarchiso ebenfalls; beim ersten 'pacman -Syu' werden sie neu geladen.


def target_packages(conf, sel):
    pkgs = list(conf["packages"])
    if sel["filesystem"] == "btrfs":
        pkgs.append("btrfs-progs")
    if sel.get("bootloader") == "systemd-boot":
        pkgs = [p for p in pkgs if p != "grub"]
    return pkgs


def install_online(conf, sel, uefi):
    progress(0.10, "Pakete werden heruntergeladen und installiert (das dauert)")
    run(["systemctl", "start", "pacman-init.service"], check=False)
    pkgs = target_packages(conf, sel)
    if not uefi:
        pkgs = [p for p in pkgs if p != "efibootmgr"]
    pacstrap(pkgs)
    # Branding und vorab geladene Daten (KI-Modelle, Container) von der ISO übernehmen
    for src in [f"/usr/share/{conf['id']}/overlay"] + conf.get("data_paths", []):
        if not os.path.exists(src):
            continue
        out(f"Übernehme {src}")
        if src.endswith("/overlay"):
            shutil.copytree(src, TARGET, symlinks=True, dirs_exist_ok=True)
        else:
            run(["cp", "-a", "--parents", src, TARGET])
    for exe in glob.glob(f"{TARGET}/usr/local/bin/*"):
        os.chmod(exe, 0o755)


def pacstrap(pkgs):
    """pacstrap mit Fortschrittsanzeige zwischen 10 % und 70 %."""
    cmd = ["pacstrap", "-C", "/etc/pacman.conf", TARGET, *pkgs]
    out("$ " + " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                         env={**os.environ, "LC_ALL": "C"})
    for line in p.stdout:
        line = line.rstrip("\n")
        out(line)
        m = re.search(r"\((\s*\d+)/(\d+)\) (installing|upgrading) (\S+)", line)
        if m:
            done, total = int(m[1]), int(m[2])
            progress(0.25 + 0.45 * done / max(total, 1), f"Installiere {m[4]} ({done}/{total})")
        elif "downloading" in line.lower():
            progress(0.15, "Pakete werden heruntergeladen")
    if p.wait() != 0:
        raise RuntimeError("pacstrap ist fehlgeschlagen – besteht eine Internetverbindung?")


# ---------------------------------------------------------------- Konfiguration
def configure(conf, sel, info, uefi):
    gid = conf["id"]
    progress(0.74, "System wird konfiguriert")
    fstab = capture(["genfstab", "-U", TARGET])
    write("/etc/fstab", "# Erzeugt vom @@NAME@@ Installer\n" + fstab + "\n")
    make_swap(sel, info)
    if info["crypttab"]:
        write("/etc/crypttab", "# <name> <device> <keyfile> <options>\n" + "\n".join(info["crypttab"]) + "\n",
              0o600)
    shutil.copyfile("/etc/pacman.conf", f"{TARGET}/etc/pacman.conf")
    if os.path.exists("/etc/pacman.d/mirrorlist"):
        shutil.copyfile("/etc/pacman.d/mirrorlist", f"{TARGET}/etc/pacman.d/mirrorlist")
    with open(f"{TARGET}/etc/machine-id", "w") as f:
        f.write("")
    chroot("systemd-machine-id-setup", check=False)

    locale, tz, kb = sel["locale"], sel["timezone"], sel["keyboard"]
    gen_path = f"{TARGET}/etc/locale.gen"
    gen = open(gen_path, encoding="utf-8").read() if os.path.exists(gen_path) else ""
    for loc in {locale, "en_US.UTF-8"}:
        pat = re.compile(rf"^#\s*({re.escape(loc)} UTF-8)", re.M)
        gen = pat.sub(r"\1", gen) if pat.search(gen) else gen + f"\n{loc} UTF-8\n"
    with open(gen_path, "w", encoding="utf-8") as f:
        f.write(gen)
    chroot("locale-gen")
    write("/etc/locale.conf", f"LANG={locale}\n")
    write("/etc/vconsole.conf", f"KEYMAP={console_keymap(kb)}\n")
    write("/etc/X11/xorg.conf.d/00-keyboard.conf",
          f'Section "InputClass"\n    Identifier "system-keyboard"\n    MatchIsKeyboard "on"\n'
          f'    Option "XkbLayout" "{kb}"\nEndSection\n')
    chroot("ln", "-sf", f"/usr/share/zoneinfo/{tz}", "/etc/localtime")
    chroot("hwclock", "--systohc", check=False)
    host = sel["hostname"]
    write("/etc/hostname", host + "\n")
    write("/etc/hosts", f"127.0.0.1 localhost\n::1 localhost\n127.0.1.1 {host}.localdomain {host}\n")

    # Benutzer
    progress(0.80, "Benutzer wird angelegt")
    if conf.get("fastfetch_on_terminal") and os.path.exists(f"{TARGET}/etc/skel/.bashrc"):
        skel = open(f"{TARGET}/etc/skel/.bashrc", encoding="utf-8").read()
        if "fastfetch" not in skel:
            append("/etc/skel/.bashrc",
                   "\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n")
    user = sel["username"]
    groups = [g for g in ("wheel", "audio", "video", "storage", "network", "lp", "docker", "render", "ollama")
              if subprocess.run(["chroot", TARGET, "getent", "group", g], capture_output=True).returncode == 0]
    chroot("useradd", "-m", "-G", ",".join(groups), "-s", "/bin/bash", "-c", sel.get("fullname", ""), user)
    chroot("chpasswd", stdin=f"{user}:{sel['password']}\n")
    if os.path.exists(f"{TARGET}/usr/bin/bash"):
        chroot("usermod", "-s", "/bin/bash", "root", check=False)
    if sel.get("root_same"):
        chroot("chpasswd", stdin=f"root:{sel['password']}\n")
    else:
        chroot("passwd", "-l", "root")
    write("/etc/sudoers.d/10-wheel", "%wheel ALL=(ALL:ALL) ALL\n", 0o440)
    if sel.get("autologin"):
        autologin(conf, user)

    # Dienste & Branding
    progress(0.84, "Dienste werden aktiviert")
    services = list(conf["services"])
    if info["fs"] == "btrfs":
        services.append("btrfs-scrub@-.timer")
    for svc in services:
        chroot("systemctl", "enable", svc, check=False)
    if os.path.exists(f"{TARGET}/usr/local/bin/{gid}-apply-branding"):
        chroot(f"/usr/local/bin/{gid}-apply-branding")


def kernel_cmdline(info):
    args = []
    for luks_uuid, name in info["luks_root"]:
        args.append(f"rd.luks.name={luks_uuid}={name}")
    if info["luks_root"]:
        args.append("rd.luks.options=discard")
    return args


def bootloader(conf, sel, info, uefi):
    progress(0.88, "Initramfs wird erstellt")
    hooks = ["base", "systemd", "autodetect", "microcode", "modconf", "kms", "keyboard", "sd-vconsole", "block"]
    if info["luks_root"] or info["crypttab"]:
        hooks.append("sd-encrypt")
    hooks += ["filesystems", "fsck"]
    write(f"/etc/mkinitcpio.conf.d/20-{conf['id']}.conf", f"HOOKS=({' '.join(hooks)})\n")
    chroot("mkinitcpio", "-P")

    progress(0.93, "Bootloader wird installiert")
    extra = kernel_cmdline(info)
    if conf.get("kernel_params"):
        extra += conf["kernel_params"]
    if sel.get("bootloader") == "systemd-boot" and uefi:
        chroot("bootctl", "--esp-path=/boot", "install")
        root = f"root=/dev/mapper/{info['luks_root'][0][1]}" if info["luks_root"] else f"root=UUID={info['root_uuid']}"
        opts = [root, "rw"] + (["rootflags=subvol=@"] if info["fs"] == "btrfs" else []) + extra + ["quiet"]
        kernels = sorted(os.path.basename(p)[len("vmlinuz-"):] for p in glob.glob(f"{TARGET}/boot/vmlinuz-*"))
        for k in kernels:
            for suffix, initrd in (("", f"initramfs-{k}.img"), ("-fallback", f"initramfs-{k}-fallback.img")):
                if not os.path.exists(f"{TARGET}/boot/{initrd}"):
                    continue
                title = conf["name"] + (f" ({k})" if len(kernels) > 1 or k != "linux" else "") + \
                    (" – Notfall" if suffix else "")
                write(f"/boot/loader/entries/{conf['id']}-{k}{suffix}.conf",
                      f"title   {title}\nlinux   /vmlinuz-{k}\ninitrd  /{initrd}\noptions {' '.join(opts)}\n")
        default = f"{conf['id']}-{kernels[0]}.conf" if kernels else "@saved"
        write("/boot/loader/loader.conf", f"default {default}\ntimeout 3\nconsole-mode max\neditor no\n")
        chroot("systemctl", "enable", "systemd-boot-update.service", check=False)
    else:
        grub_default = f"{TARGET}/etc/default/grub"
        if os.path.exists(grub_default):
            text = open(grub_default, encoding="utf-8").read()
            text = re.sub(r'^GRUB_CMDLINE_LINUX=.*$', f'GRUB_CMDLINE_LINUX="{" ".join(extra)}"', text, flags=re.M)
            with open(grub_default, "w", encoding="utf-8") as f:
                f.write(text)
        if uefi:
            chroot("grub-install", "--target=x86_64-efi", "--efi-directory=/boot",
                   f"--bootloader-id={conf['name']}")
            chroot("grub-install", "--target=x86_64-efi", "--efi-directory=/boot", "--removable", check=False)
        else:
            chroot("grub-install", "--target=i386-pc", sel["disk"])
        chroot("grub-mkconfig", "-o", "/boot/grub/grub.cfg")


def autologin(conf, user):
    desktop = conf.get("desktop")
    if desktop in ("kde", "lxqt"):
        session = "plasma" if desktop == "kde" else "lxqt"
        write("/etc/sddm.conf.d/autologin.conf", f"[Autologin]\nUser={user}\nSession={session}\n")
    elif desktop == "gnome":
        write("/etc/gdm/custom.conf", f"[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin={user}\n")
    elif desktop in ("xfce", "cinnamon"):
        chroot("groupadd", "-rf", "autologin")
        chroot("gpasswd", "-a", user, "autologin")
        write("/etc/lightdm/lightdm.conf.d/50-autologin.conf",
              f"[Seat:*]\nautologin-user={user}\nautologin-session={desktop}\n")


def cleanup():
    run(["umount", "-R", TARGET], check=False, quiet=True)
    for name in reversed(opened_luks):
        run(["cryptsetup", "close", name], check=False, quiet=True)
    opened_luks.clear()


def main():
    with open(CONF, encoding="utf-8") as f:
        conf = json.load(f)
    with open(sys.argv[1], encoding="utf-8") as f:
        sel = json.load(f)
    uefi = os.path.isdir("/sys/firmware/efi")
    if sel.get("bootloader") == "systemd-boot" and not uefi:
        sel["bootloader"] = "grub"
    mode = "von der ISO (offline)" if sel.get("source", "offline") == "offline" else "aus dem Internet"
    out(f"Installiere {conf['name']} {mode} auf {sel['disk']} ({sel['filesystem']}, "
        f"{'UEFI' if uefi else 'BIOS'}, {'verschlüsselt' if sel.get('encrypt') else 'unverschlüsselt'})")
    for ex in sel.get("extra_disks", []):
        out(f"  zusätzlich: {ex['path']} als {ex['role']}")

    progress(0.01, "Festplatten werden vorbereitet")
    cleanup()
    info = prepare_disks(sel, conf, uefi)
    if sel.get("source", "offline") == "offline":
        install_offline(conf, sel)
    else:
        install_online(conf, sel, uefi)
    configure(conf, sel, info, uefi)
    bootloader(conf, sel, info, uefi)

    progress(0.98, "Abschluss")
    run(["sync"])
    cleanup()
    progress(1.0, "Fertig")
    out("##DONE##")


if __name__ == "__main__":
    if os.geteuid() != 0:
        out("##ERROR## Das Backend muss als root laufen.")
        sys.exit(1)
    try:
        main()
    except Exception as e:  # noqa: BLE001
        out(f"##ERROR## {e}")
        cleanup()
        sys.exit(1)
