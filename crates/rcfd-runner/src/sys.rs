//! Platform pieces, std only (the workspace forbids unsafe code): the runner lock, process liveness,
//! killing a process by id, and detached spawning.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
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
        // the state too: a zombie keeps its name (ADR 0143)
        c.args(["-p", &pid.to_string(), "-o", "stat=", "-o", "comm="]);
        (c, f)
    };
    c.stdin(Stdio::null()).stderr(Stdio::null());
    (c, flags)
}

/// Is process `pid` alive and its executable named `name` (without `.exe`)? The name check guards against a
/// recycled pid of an unrelated program. The processes this process started and that have exited are reaped first
/// ([`reap_started`]). Unix: a zombie or a dead process is not alive ([`ps_alive`], ADR 0143).
// shortcut: a recycled pid of another process with the same name reads as alive; a per-process handle would fix it.
pub fn pid_alive(pid: u32, name: &str) -> bool {
    reap_started();
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
            .map(|o| o.status.success() && ps_alive(&String::from_utf8_lossy(&o.stdout), name))
            .unwrap_or(false)
    }
}

/// The verdict of [`pid_alive`] on Unix from the output of `ps -p <pid> -o stat= -o comm=`: alive when the first
/// non-blank line holds a state that does not start with `Z` (zombie) or `X` (dead) and a name that ends with `name`.
/// Plain text in, so that it is tested on every platform (ADR 0143 Z0).
pub fn ps_alive(out: &str, name: &str) -> bool {
    let Some(line) = out.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return false;
    };
    let Some((stat, comm)) = line.split_once(char::is_whitespace) else {
        return false;
    };
    !stat.starts_with(['Z', 'X']) && comm.trim().ends_with(name)
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

/// The command of [`spawn_detached`] and its creation flags: `exe args` itself, with null stdio and nothing in
/// between. Windows: `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` (and `CREATE_NO_WINDOW`, see [`console`]): no
/// console, and the caller's Ctrl+C does not reach it. Unix: its own process group (`process_group(0)`); a new session
/// would need `setsid`, which std offers only unstable or through unsafe code.
pub fn detached_command(exe: &Path, args: &[String]) -> (Command, u32) {
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

/// Start `exe args` detached from the caller ([`detached_command`]), so that the caller's exit or Ctrl+C does not end
/// it: the only start route (ADR 0143). Its stdout and stderr go to `stdout` and `stderr`, its stdin is the null device
/// (ADR 0146). Keep the child until it exits, or hand it to [`keep_started`], so that it is reaped.
///
/// Windows: std's spawn passes the new process every inheritable handle of the caller (std's switch for it is
/// unstable, and the workspace forbids unsafe code). A caller whose stdout is a pipe that another process reads to the
/// end (a shell capturing `rcfd-runner start ...`) therefore keeps that reader waiting until the new process exits,
/// whatever `stdout` and `stderr` are (ADR 0146 I1-I3).
pub fn spawn_detached(exe: &Path, args: &[String], stdout: Stdio, stderr: Stdio) -> io::Result<Child> {
    let (mut cmd, _) = detached_command(exe, args);
    cmd.stdout(stdout).stderr(stderr).spawn()
}

/// The processes this process started with [`spawn_detached`] and handed over to be reaped, until they exit (ADR 0143).
static STARTED: Mutex<Vec<Child>> = Mutex::new(Vec::new());

/// Keep `child` until it exits; [`reap_started`] collects it then.
pub fn keep_started(child: Child) {
    STARTED.lock().unwrap_or_else(|e| e.into_inner()).push(child);
}

/// Reap the kept processes that have exited (`try_wait`, never blocking) and drop them; returns how many. Unix: a
/// dead runner that this process started does not stay its zombie; Windows: its process handle is released.
/// [`lock_held`] and [`pid_alive`] call it first.
pub fn reap_started() -> usize {
    let mut kept = STARTED.lock().unwrap_or_else(|e| e.into_inner());
    let before = kept.len();
    kept.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    before - kept.len()
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

/// Open `path` for reading with every sharing mode (read, write, delete): the open fails with error 32 exactly while
/// another handle holds the file without sharing, and two such opens do not exclude each other (ADR 0229 D1).
#[cfg(windows)]
fn open_shared(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    OpenOptions::new().read(true).share_mode(7).open(path)
}

/// Is a live runner holding the lock at `path`? The processes this process started and that have exited are reaped
/// first ([`reap_started`]). Windows: the probe shares the file, so that two probes at the same moment do not read each
/// other as a runner (ADR 0229 D1).
pub fn lock_held(path: &Path) -> bool {
    reap_started();
    #[cfg(windows)]
    {
        if !path.exists() {
            return false;
        }
        match open_shared(path) {
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

/// The liveness file of a solver about to be launched (ADR 0229 D3): `path` opened (created with its folder) without
/// sharing. Handed to the solver as its standard input and kept by nobody else, it stays open, unshared, exactly as
/// long as that process lives, whatever its pid. Windows only: elsewhere an `Unsupported` error (no file is made).
pub fn alive_file(path: &Path) -> io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no liveness file on this platform",
        ))
    }
}

/// Does a process hold the liveness file at `path` ([`alive_file`])? `Some(true)`: held, its solver lives;
/// `Some(false)`: the file is there and nobody holds it, the solver is gone; `None`: no such file (a run launched
/// without one), or the file cannot be asked (the caller falls back to [`pid_alive`]).
pub fn held(path: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        if !path.exists() {
            return None;
        }
        match open_shared(path) {
            Ok(_) => Some(false),
            Err(e) if e.raw_os_error() == Some(32) => Some(true),
            Err(_) => None,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    /// ADR 0229 P1, P2: a probe in flight (a shared read handle) does not read as a runner; an unshared holder does.
    /// Control: the probe before D1 (an unshared open) fails with error 32 against the probe in flight.
    #[cfg(windows)]
    #[test]
    fn p1_p2_two_lock_probes_do_not_read_each_other_as_a_runner() {
        use super::*;
        use std::os::windows::fs::OpenOptionsExt;
        let dir = std::env::temp_dir().join(format!("rcfd_sys_p1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock = dir.join("runner.lock");
        std::fs::write(&lock, b"").unwrap();
        let in_flight = open_shared(&lock).unwrap();
        let p1 = lock_held(&lock);
        let before = OpenOptions::new().read(true).share_mode(0).open(&lock);
        let control = before.as_ref().err().and_then(|e| e.raw_os_error());
        drop(before);
        drop(in_flight);
        let runner = acquire_lock(&lock).unwrap().expect("the lock is free");
        let p2 = lock_held(&lock);
        drop(runner);
        let free = lock_held(&lock);
        eprintln!("P1: held with a probe in flight {p1} (control: the unshared probe's error {control:?}); P2: held under a runner {p2}, after it {free}");
        assert!(!p1, "P1: a probe in flight read as a runner");
        assert_eq!(control, Some(32), "control: the unshared probe did not collide");
        assert!(p2, "P2: a held lock read as free");
        assert!(!free);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Helper of I1, not a check: sleeps while `RCFD_I1_SLEEP_MS` is set (the test starts this test binary with it).
    #[test]
    #[ignore = "helper process of I1"]
    fn i1_helper_sleeps() {
        if let Some(ms) = std::env::var("RCFD_I1_SLEEP_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }

    /// ADR 0229 I1: the liveness file is held while the process that got it as its standard input lives, and free
    /// within 5 s of its exit; a path without a file answers `None`.
    #[cfg(windows)]
    #[test]
    fn i1_the_liveness_file_is_held_exactly_while_its_process_lives() {
        use super::*;
        use std::time::Instant;
        let dir = std::env::temp_dir().join(format!("rcfd_sys_i1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("alive").join("run-x");
        let none = held(&path);
        let file = alive_file(&path).unwrap();
        let (mut cmd, _) = console(std::env::current_exe().unwrap(), 0);
        cmd.args(["--exact", "sys::tests::i1_helper_sleeps", "--ignored", "--nocapture"])
            .env("RCFD_I1_SLEEP_MS", "1000")
            .stdin(Stdio::from(file))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().unwrap();
        drop(cmd); // as the runner does: no handle of the file stays here
        let while_alive = held(&path);
        let t0 = Instant::now();
        let status = child.wait().unwrap();
        let lived = t0.elapsed();
        let mut after = held(&path);
        while after != Some(false) && t0.elapsed() < lived + Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
            after = held(&path);
        }
        eprintln!(
            "I1: no file {none:?}; while the child lived {while_alive:?}; the child lived {lived:?} (exit {status}); after it {after:?}, {:?} after its exit",
            t0.elapsed() - lived
        );
        assert_eq!(none, None);
        assert_eq!(while_alive, Some(true), "the file was not held while its process lived");
        assert!(
            status.success() && lived >= Duration::from_millis(900),
            "set-up: the helper did not sleep"
        );
        assert_eq!(after, Some(false), "the file stayed held after its process ended");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ADR 0128 finding 17 (a, b): every console program the runner and its client start is built with
    /// `CREATE_NO_WINDOW`; the runner start keeps `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`. ADR 0143: the PowerShell
    /// launcher went with its route.
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
            ("runner (spawn_detached)", detached_command(exe, &[]).1),
            ("rcfd --version (Client::submit)", crate::client::version_cmd(exe).1),
        ];
        eprintln!(
            "F17 runner flags: {:?}",
            sites.iter().map(|(n, f)| format!("{n} {f:#x}")).collect::<Vec<_>>()
        );
        for (name, f) in sites {
            assert!(hidden(f), "{name}: flags {f:#x} lack CREATE_NO_WINDOW");
        }
        assert!(detached(detached_command(exe, &[]).1));
        // negative control: a command built without the helper carries no flags and fails the same check
        let _plain = Command::new("tasklist");
        assert!(!hidden(0));
    }

    /// ADR 0143 N4 (b): the start command is the runner executable itself, with nothing in between; on Windows it is
    /// detached and opens no window.
    #[test]
    fn n4_the_start_command_is_the_runner_itself() {
        use super::*;
        let exe = Path::new(if cfg!(windows) {
            r"C:\x y\rcfd-runner.exe"
        } else {
            "/x y/rcfd-runner"
        });
        let args = ["serve".to_string(), "--project".into(), "p".into()];
        let (cmd, flags) = detached_command(exe, &args);
        let got: Vec<&OsStr> = cmd.get_args().collect();
        eprintln!(
            "N4 (b): program {:?}, arguments {got:?}, flags {flags:#x}",
            cmd.get_program()
        );
        assert_eq!(cmd.get_program(), exe.as_os_str());
        assert_eq!(got, ["serve", "--project", "p"]);
        if cfg!(windows) {
            let want = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
            assert_eq!(flags & want, want, "{flags:#x}");
        } else {
            assert_eq!(flags, 0);
        }
    }

    /// ADR 0143 Z0: `ps_alive` on simulated `ps -o stat= -o comm=` output; zombie and dead states are not alive.
    /// Negative control: the pre-change rule (the trimmed output ends with the name) reads the zombie as alive.
    #[test]
    fn z0_ps_zombie_and_dead_states_are_not_alive() {
        use super::ps_alive;
        let alive = [
            ("S    rcfd-runner", "rcfd-runner"),
            ("Ssl  rcfd-runner", "rcfd-runner"),
            ("R+   rcfd", "rcfd"),
            ("D    rcfd", "rcfd"),
            ("T    rcfd", "rcfd"),
            ("I    rcfd-runner", "rcfd-runner"),
            ("  S rcfd-runner\n", "rcfd-runner"),
        ];
        let dead = [
            ("Z    rcfd-runner", "rcfd-runner"),
            ("Z+   rcfd-runner", "rcfd-runner"),
            ("X    rcfd-runner", "rcfd-runner"),
            ("Z    rcfd-runner <defunct>", "rcfd-runner"),
            ("S    other", "rcfd-runner"),
            ("S", "rcfd-runner"),
            ("", "rcfd-runner"),
            ("\n", "rcfd-runner"),
        ];
        let wrong: Vec<_> = alive
            .iter()
            .filter(|(o, n)| !ps_alive(o, n))
            .chain(dead.iter().filter(|(o, n)| ps_alive(o, n)))
            .collect();
        eprintln!(
            "Z0: {} outputs read alive, {} dead; wrong verdicts {wrong:?}",
            alive.len(),
            dead.len()
        );
        assert!(wrong.is_empty(), "{wrong:?}");
        let old = |out: &str, name: &str| out.trim().ends_with(name);
        assert!(
            old("Z    rcfd-runner", "rcfd-runner"),
            "negative control: the pre-change rule does not read the zombie as alive"
        );
    }
}
