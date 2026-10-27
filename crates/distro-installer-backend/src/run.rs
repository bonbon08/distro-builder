//! Thin layer around `std::process::Command` mirroring the Python backend's
//! `run`/`capture`/`chroot`/`chroot_out`/`write`/`append` helpers, including
//! merging stderr into stdout (so progress lines and error output interleave
//! in the same order the GUI sees them) via a shared OS pipe.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::out;

pub struct Runner {
    pub target: PathBuf,
    pub opened_luks: Vec<String>,
}

impl Runner {
    pub fn new(target: PathBuf) -> Self {
        Self { target, opened_luks: Vec::new() }
    }

    fn target_str(&self) -> &str {
        self.target.to_str().expect("target path must be valid UTF-8")
    }

    pub fn run(&self, cmd: &[&str], check: bool, stdin: Option<&[u8]>, quiet: bool) -> Result<i32> {
        if !quiet {
            out(&format!("$ {}", cmd.join(" ")));
        }
        let (mut child, reader) = spawn_merged(cmd, stdin)?;
        let mut br = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            if br.read_line(&mut line)? == 0 {
                break;
            }
            out(line.trim_end_matches('\n'));
        }
        let status = child.wait()?;
        let rc = status.code().unwrap_or(-1);
        if check && rc != 0 {
            bail!("Befehl fehlgeschlagen ({rc}): {}", cmd.join(" "));
        }
        Ok(rc)
    }

    pub fn capture(&self, cmd: &[&str], check: bool) -> Result<String> {
        let output = Command::new(cmd[0])
            .args(&cmd[1..])
            .output()
            .with_context(|| format!("Konnte {} nicht ausführen", cmd[0]))?;
        if check && !output.status.success() {
            bail!("Befehl fehlgeschlagen: {}", cmd.join(" "));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn chroot(&self, args: &[&str], check: bool, stdin: Option<&[u8]>) -> Result<i32> {
        let mut full = vec!["arch-chroot", self.target_str()];
        full.extend_from_slice(args);
        self.run(&full, check, stdin, false)
    }

    pub fn chroot_out(&self, args: &[&str]) -> String {
        let mut full = vec!["arch-chroot", self.target_str()];
        full.extend_from_slice(args);
        self.capture(&full, false).unwrap_or_default()
    }

    pub fn write(&self, rel: &str, content: &str, mode: Option<u32>) -> Result<()> {
        let full = self.target.join(rel.trim_start_matches('/'));
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        if let Some(m) = mode {
            std::fs::set_permissions(&full, std::fs::Permissions::from_mode(m))?;
        }
        Ok(())
    }

    pub fn append(&self, rel: &str, content: &str) -> Result<()> {
        let full = self.target.join(rel.trim_start_matches('/'));
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&full)?;
        f.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Recursive unmount of the target plus closing any LUKS mappings opened
    /// this run, in reverse order. Best-effort: never fails.
    pub fn cleanup(&mut self) {
        let _ = self.run(&["umount", "-R", self.target_str()], false, None, true);
        for name in self.opened_luks.iter().rev() {
            let _ = Command::new("cryptsetup").args(["close", name]).output();
        }
        self.opened_luks.clear();
    }
}

/// Spawns `cmd`, merging its stdout+stderr into one OS pipe (matching
/// Python's `stderr=subprocess.STDOUT`) so error output and progress lines
/// stay interleaved in the order the process produced them. Optionally
/// writes `stdin` to the child's stdin before returning.
pub fn spawn_merged(cmd: &[&str], stdin: Option<&[u8]>) -> Result<(Child, os_pipe::PipeReader)> {
    let (reader, writer) = os_pipe::pipe()?;
    let writer2 = writer.try_clone()?;
    let mut c = Command::new(cmd[0]);
    c.args(&cmd[1..]);
    c.stdout(Stdio::from(writer));
    c.stderr(Stdio::from(writer2));
    c.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = c.spawn().with_context(|| format!("Konnte {} nicht starten", cmd[0]))?;
    if let Some(data) = stdin {
        if let Some(mut si) = child.stdin.take() {
            si.write_all(data)?;
        }
    }
    Ok((child, reader))
}
