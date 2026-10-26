//! Platform pieces, std only (the workspace forbids unsafe code): the runner lock, process liveness,
//! killing a process by id, and detached spawning.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Is process `pid` alive and its executable named `name` (without `.exe`)? The name check guards against a
/// recycled pid of an unrelated program.
// shortcut: a recycled pid of another process with the same name reads as alive; a per-process handle would fix it.
pub fn pid_alive(pid: u32, name: &str) -> bool {
    #[cfg(windows)]
    {
        let Ok(o) = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        else {
            return false;
        };
        let want = format!("\"{}.exe\",\"{pid}\"", name.to_ascii_lowercase());
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.to_ascii_lowercase().starts_with(&want))
    }
    #[cfg(not(windows))]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().ends_with(name))
            .unwrap_or(false)
    }
}

/// Kill process `pid` (forced). Returns whether the kill command succeeded.
pub fn kill(pid: u32) -> bool {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/F"]);
        c
    } else {
        let mut c = Command::new("kill");
        c.args(["-9", &pid.to_string()]);
        c
    };
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Start `exe args` detached from the caller, with no stdio, so the caller's exit, Ctrl+C or captured
/// output does not tie it. Returns the launcher process (Windows: a short-lived PowerShell; Unix: `exe`).
///
/// Windows: a std spawn passes the new process every inheritable handle of the caller (for example the pipe
/// a shell reads the caller's output from, so `$id = rcfd-runner start ...` would wait for the runner to
/// exit). `Start-Process` without redirection uses ShellExecute, which passes no handles; the script goes
/// base64-encoded (`-EncodedCommand`) so paths need no quoting rules.
pub fn spawn_detached(exe: &Path, args: &[String]) -> io::Result<std::process::Child> {
    #[cfg(windows)]
    {
        let q = |s: &str| s.replace('\'', "''");
        let list: Vec<String> = args.iter().map(|a| format!("'\"{}\"'", q(a))).collect();
        let script = format!(
            "Start-Process -FilePath '{}' -ArgumentList {} -WindowStyle Hidden",
            q(&exe.display().to_string()),
            list.join(",")
        );
        let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &base64(&utf16)])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
    #[cfg(not(windows))]
    {
        spawn_direct(exe, args)
    }
}

/// Start `exe args` directly, with null stdio, in its own process group (Windows: also without a console).
/// On Windows it inherits the caller's inheritable handles (see [`spawn_detached`]).
pub fn spawn_direct(exe: &Path, args: &[String]) -> io::Result<std::process::Child> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            s.push(if k <= c.len() {
                T[(n >> (18 - 6 * k) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    s
}

/// A console program started by the (console-less) runner must not open a window.
pub fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

/// The runner lock `<project>/.rcfd/runner.lock`, held for the runner's lifetime.
#[derive(Debug)]
pub struct RunnerLock {
    _file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

#[cfg(unix)]
impl Drop for RunnerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Take the runner lock; `Ok(None)` when another live runner holds it.
///
/// Windows: the file is opened without sharing, so a second open fails while the holder lives and the OS
/// releases it when the holder dies. Unix: `create_new` with the pid inside; a lock whose pid is not a live
/// `rcfd-runner` is taken over under a second `create_new` guard (`runner.lock.takeover`).
pub fn acquire_lock(path: &Path) -> io::Result<Option<RunnerLock>> {
    // retried briefly: a client probing the lock holds it for microseconds
    for _ in 0..10 {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(path)
            {
                Ok(f) => {
                    return Ok(Some(RunnerLock {
                        _file: f,
                        path: path.to_path_buf(),
                    }))
                }
                Err(e) if e.raw_os_error() == Some(32) => {}
                Err(e) => return Err(e),
            }
        }
        #[cfg(not(windows))]
        {
            use std::io::Write;
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut f) => {
                    write!(f, "{}", std::process::id())?;
                    return Ok(Some(RunnerLock {
                        _file: f,
                        path: path.to_path_buf(),
                    }));
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => unix_takeover(path),
                Err(e) => return Err(e),
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(None)
}

#[cfg(not(windows))]
fn unix_takeover(path: &Path) {
    let read_pid = || {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
    };
    let age = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .unwrap_or_default()
    };
    let pid = read_pid();
    let stale = match pid {
        Some(p) => !pid_alive(p, "rcfd-runner"),
        // an empty lock is a starter between create and write; stale only when old
        None => age(path) > Duration::from_secs(5),
    };
    if !stale {
        return;
    }
    let guard = path.with_extension("lock.takeover");
    if OpenOptions::new().write(true).create_new(true).open(&guard).is_ok() {
        if read_pid() == pid {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_file(&guard);
    } else if age(&guard) > Duration::from_secs(30) {
        let _ = std::fs::remove_file(&guard);
    }
}

/// Is a live runner holding the lock at `path`?
pub fn lock_held(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        if !path.exists() {
            return false;
        }
        match OpenOptions::new().read(true).share_mode(0).open(path) {
            Ok(_) => false,
            Err(e) => e.raw_os_error() == Some(32),
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .is_some_and(|p| pid_alive(p, "rcfd-runner"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648_vectors() {
        let v: Vec<String> = ["", "f", "fo", "foo", "foob", "fooba", "foobar"]
            .iter()
            .map(|s| super::base64(s.as_bytes()))
            .collect();
        assert_eq!(v, ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy"]);
    }
}
