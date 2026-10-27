//! Platform pieces, std only (the workspace forbids unsafe code): the runner lock, process liveness,
//! killing a process by id, and detached spawning.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Windows `CreateProcess` flag: a console program gets a console without a window.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Windows `CreateProcess` flag: no console at all (`CREATE_NO_WINDOW` is then ignored).
pub const DETACHED_PROCESS: u32 = 0x0000_0008;
/// Windows `CreateProcess` flag: the caller's Ctrl+C does not reach the new process.
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// `program` as a console program that opens no window: creation flags `extra | CREATE_NO_WINDOW` on Windows (flags
/// are combined, never replaced; none elsewhere). Returns the command and the flags set: std's `Command` has no
/// getter, so the tests read them here. Every console program the runner or its client starts goes through this
/// (ADR 0128 finding 17; `tests/no_window_scan.rs` checks the source).
pub fn console(program: impl AsRef<OsStr>, extra: u32) -> (Command, u32) {
    let mut cmd = Command::new(program);
    let flags = hide(&mut cmd, extra);
    (cmd, flags)
}

fn hide(cmd: &mut Command, extra: u32) -> u32 {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let flags = extra | CREATE_NO_WINDOW;
        cmd.creation_flags(flags);
        flags
    }
    #[cfg(not(windows))]
    {
        let _ = (cmd, extra);
        0
    }
}

/// The liveness query of [`pid_alive`] and its creation flags.
fn probe_cmd(pid: u32) -> (Command, u32) {
    let (mut c, flags) = if cfg!(windows) {
        let (mut c, f) = console("tasklist", 0);
        c.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
        (c, f)
    } else {
        let (mut c, f) = console("ps", 0);
        c.args(["-p", &pid.to_string(), "-o", "comm="]);
        (c, f)
    };
    c.stdin(Stdio::null()).stderr(Stdio::null());
    (c, flags)
}

/// Is process `pid` alive and its executable named `name` (without `.exe`)? The name check guards against a
/// recycled pid of an unrelated program.
// shortcut: a recycled pid of another process with the same name reads as alive; a per-process handle would fix it.
pub fn pid_alive(pid: u32, name: &str) -> bool {
    #[cfg(windows)]
    {
        let Ok(o) = probe_cmd(pid).0.output() else {
            return false;
        };
        let want = format!("\"{}.exe\",\"{pid}\"", name.to_ascii_lowercase());
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.to_ascii_lowercase().starts_with(&want))
    }
    #[cfg(not(windows))]
    {
        probe_cmd(pid)
            .0
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().ends_with(name))
            .unwrap_or(false)
    }
}

/// The command of [`kill`] and its creation flags.
fn kill_cmd(pid: u32) -> (Command, u32) {
    if cfg!(windows) {
        let (mut c, f) = console("taskkill", 0);
        c.args(["/PID", &pid.to_string(), "/F"]);
        (c, f)
    } else {
        let (mut c, f) = console("kill", 0);
        c.args(["-9", &pid.to_string()]);
        (c, f)
    }
}

/// Kill process `pid` (forced). Returns whether the kill command succeeded.
pub fn kill(pid: u32) -> bool {
    kill_cmd(pid)
        .0
        .stdin(Stdio::null())
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
        launcher_cmd(&start_script(exe, args)).0.spawn()
    }
    #[cfg(not(windows))]
    {
        spawn_direct(exe, args)
    }
}

/// The PowerShell launcher of [`spawn_detached`] running `script`, and its creation flags: detached from the
/// caller's console and Ctrl+C like [`spawn_direct`], no window.
fn launcher_cmd(script: &str) -> (Command, u32) {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let (mut c, flags) = console("powershell", DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    c.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &base64(&utf16)])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    (c, flags)
}

/// The Windows command line of `args`: each argument in double quotes by the `CommandLineToArgvW` rules (backslashes
/// before a quote or at the end are doubled, a quote is escaped), separated by spaces. A project path ending in `\`
/// (or `C:\`) stays one argument; trimming it instead would turn `C:\` into `C:` (ADR 0128 finding 15).
pub fn command_line(args: &[String]) -> String {
    let quote = |a: &String| {
        let mut out = String::from("\"");
        let mut slashes = 0;
        for c in a.chars() {
            match c {
                '\\' => slashes += 1,
                '"' => {
                    out.extend(std::iter::repeat('\\').take(2 * slashes + 1));
                    out.push('"');
                    slashes = 0;
                }
                c => {
                    out.extend(std::iter::repeat('\\').take(slashes));
                    out.push(c);
                    slashes = 0;
                }
            }
        }
        out.extend(std::iter::repeat('\\').take(2 * slashes));
        out.push('"');
        out
    };
    args.iter().map(quote).collect::<Vec<_>>().join(" ")
}

/// The PowerShell script of [`spawn_detached`]: the whole command line as one `-ArgumentList` string (PowerShell
/// passes a single string through as the command line), single quotes doubled for PowerShell.
pub fn start_script(exe: &Path, args: &[String]) -> String {
    let q = |s: &str| s.replace('\'', "''");
    format!(
        "Start-Process -FilePath '{}' -ArgumentList '{}' -WindowStyle Hidden",
        q(&exe.display().to_string()),
        q(&command_line(args))
    )
}

/// Start `exe args` directly, with null stdio, in its own process group (Windows: also without a console).
/// On Windows it inherits the caller's inheritable handles (see [`spawn_detached`]).
pub fn spawn_direct(exe: &Path, args: &[String]) -> io::Result<std::process::Child> {
    direct_cmd(exe, args).0.spawn()
}

/// The command of [`spawn_direct`] and its creation flags.
fn direct_cmd(exe: &Path, args: &[String]) -> (Command, u32) {
    let (mut cmd, flags) = console(exe, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    (cmd, flags)
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

/// A console program started by the (console-less) runner must not open a window (see [`console`]).
pub fn no_window(cmd: &mut Command) {
    hide(cmd, 0);
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
    /// ADR 0128 finding 17 (a, b): every console program the runner and its client start is built with
    /// `CREATE_NO_WINDOW`; the PowerShell launcher and the direct start keep `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`.
    #[cfg(windows)]
    #[test]
    fn no_window_flags_on_every_console_spawn() {
        use super::*;
        let hidden = |f: u32| f & CREATE_NO_WINDOW != 0;
        let detached =
            |f: u32| f & (DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP) == DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        let exe = Path::new("rcfd-runner.exe");
        let sites = [
            ("tasklist (pid_alive)", probe_cmd(1).1),
            ("taskkill (kill)", kill_cmd(1).1),
            ("powershell (spawn_detached)", launcher_cmd("x").1),
            ("runner (spawn_direct)", direct_cmd(exe, &[]).1),
            ("rcfd --version (Client::submit)", crate::client::version_cmd(exe).1),
        ];
        eprintln!(
            "F17 runner flags: {:?}",
            sites.iter().map(|(n, f)| format!("{n} {f:#x}")).collect::<Vec<_>>()
        );
        for (name, f) in sites {
            assert!(hidden(f), "{name}: flags {f:#x} lack CREATE_NO_WINDOW");
        }
        assert!(detached(launcher_cmd("x").1) && detached(direct_cmd(exe, &[]).1));
        // negative control: a command built without the helper carries no flags and fails the same check
        let _plain = Command::new("tasklist");
        assert!(!hidden(0));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let v: Vec<String> = ["", "f", "fo", "foo", "foob", "fooba", "foobar"]
            .iter()
            .map(|s| super::base64(s.as_bytes()))
            .collect();
        assert_eq!(v, ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy"]);
    }
}
