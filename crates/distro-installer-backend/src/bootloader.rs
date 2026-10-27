//! Initramfs and bootloader installation (systemd-boot or GRUB). Mirrors
//! `bootloader`/`kernel_cmdline` from the Python `backend.py`.

use anyhow::Result;
use distro_builder_core::installer_conf::{InstallerConf, Selection};

use crate::disks::DiskInfo;
use crate::progress;
use crate::run::Runner;

fn kernel_cmdline(info: &DiskInfo) -> Vec<String> {
    let mut args: Vec<String> = info
        .luks_root
        .iter()
        .map(|(uuid, name)| format!("rd.luks.name={uuid}={name}"))
        .collect();
    if !info.luks_root.is_empty() {
        args.push("rd.luks.options=discard".to_string());
    }
    args
}

pub fn bootloader(
    rt: &Runner,
    conf: &InstallerConf,
    sel: &Selection,
    info: &DiskInfo,
    uefi: bool,
) -> Result<()> {
    progress(0.88, "Initramfs wird erstellt");
    let mut hooks: Vec<&str> = vec![
        "base",
        "systemd",
        "autodetect",
        "microcode",
        "modconf",
        "kms",
        "keyboard",
        "sd-vconsole",
        "block",
    ];
    if !info.luks_root.is_empty() || !info.crypttab.is_empty() {
        hooks.push("sd-encrypt");
    }
    hooks.extend(["filesystems", "fsck"]);
    rt.write(
        &format!("/etc/mkinitcpio.conf.d/20-{}.conf", conf.id),
        &format!("HOOKS=({})\n", hooks.join(" ")),
        None,
    )?;
    rt.chroot(&["mkinitcpio", "-P"], true, None)?;

    progress(0.93, "Bootloader wird installiert");
    let mut extra = kernel_cmdline(info);
    extra.extend(conf.kernel_params.clone());

    if sel.bootloader == "systemd-boot" && uefi {
        rt.chroot(&["bootctl", "--esp-path=/boot", "install"], true, None)?;
        let root = if let Some((_, name)) = info.luks_root.first() {
            format!("root=/dev/mapper/{name}")
        } else {
            format!("root=UUID={}", info.root_uuid)
        };
        let mut opts = vec![root, "rw".to_string()];
        if info.fs == "btrfs" {
            opts.push("rootflags=subvol=@".to_string());
        }
        opts.extend(extra.clone());
        opts.push("quiet".to_string());

        let mut kernels: Vec<String> =
            glob::glob(&format!("{}/boot/vmlinuz-*", rt.target.display()))?
                .flatten()
                .filter_map(|p| {
                    p.file_name()
                        .and_then(|f| f.to_str())
                        .map(|s| s.trim_start_matches("vmlinuz-").to_string())
                })
                .collect();
        kernels.sort();
        for k in &kernels {
            for (suffix, initrd) in [
                ("", format!("initramfs-{k}.img")),
                ("-fallback", format!("initramfs-{k}-fallback.img")),
            ] {
                if !rt.target.join("boot").join(&initrd).exists() {
                    continue;
                }
                let title = format!(
                    "{}{}{}",
                    conf.name,
                    if kernels.len() > 1 || k != "linux" {
                        format!(" ({k})")
                    } else {
                        String::new()
                    },
                    if !suffix.is_empty() {
                        " – Notfall"
                    } else {
                        ""
                    }
                );
                rt.write(
                    &format!("/boot/loader/entries/{}-{k}{suffix}.conf", conf.id),
                    &format!(
                        "title   {title}\nlinux   /vmlinuz-{k}\ninitrd  /{initrd}\noptions {}\n",
                        opts.join(" ")
                    ),
                    None,
                )?;
            }
        }
        let default = match kernels.first() {
            Some(first) => format!("{}-{first}.conf", conf.id),
            None => "@saved".to_string(),
        };
        rt.write(
            "/boot/loader/loader.conf",
            &format!("default {default}\ntimeout 3\nconsole-mode max\neditor no\n"),
            None,
        )?;
        let _ = rt.chroot(
            &["systemctl", "enable", "systemd-boot-update.service"],
            false,
            None,
        );
    } else {
        let grub_default = rt.target.join("etc/default/grub");
        if grub_default.exists() {
            let text = std::fs::read_to_string(&grub_default)?;
            let re = regex::Regex::new(r"(?m)^GRUB_CMDLINE_LINUX=.*$").unwrap();
            let replacement = format!("GRUB_CMDLINE_LINUX=\"{}\"", extra.join(" "));
            let new_text = re
                .replace(&text, |_: &regex::Captures| replacement.clone())
                .into_owned();
            std::fs::write(&grub_default, new_text)?;
        }
        if uefi {
            rt.chroot(
                &[
                    "grub-install",
                    "--target=x86_64-efi",
                    "--efi-directory=/boot",
                    &format!("--bootloader-id={}", conf.name),
                ],
                true,
                None,
            )?;
            let _ = rt.chroot(
                &[
                    "grub-install",
                    "--target=x86_64-efi",
                    "--efi-directory=/boot",
                    "--removable",
                ],
                false,
                None,
            );
        } else {
            rt.chroot(&["grub-install", "--target=i386-pc", &sel.disk], true, None)?;
        }
        rt.chroot(&["grub-mkconfig", "-o", "/boot/grub/grub.cfg"], true, None)?;
    }
    Ok(())
}
