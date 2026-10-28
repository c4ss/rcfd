//! The capturing-caller harness of ADR 0146 (C1, I1-I4). A process starts with stdout and stderr piped, and one reader thread
//! per pipe reads to EOF. The harness records the instants of its exit and of each EOF. "Returned" is what
//! `wait_with_output` waits for: the exit and both EOFs. "Held at T": at time T after the exit, an EOF is still pending. No
//! bare `wait_with_output` is used, so a held pipe never blocks the test.

#![allow(dead_code)]

use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

pub struct Capture {
    child: Child,
    /// The instant just before the spawn.
    pub t0: Instant,
    rx: Receiver<(usize, Instant, Vec<u8>)>,
    pub exit: Option<(Instant, ExitStatus)>,
    /// stdout (0) and stderr (1): the instant of the EOF and everything read.
    pub eof: [Option<(Instant, Vec<u8>)>; 2],
}

impl Capture {
    /// Start `cmd` with stdout and stderr piped (its stdin as the caller set it).
    pub fn start(mut cmd: Command) -> Capture {
        let t0 = Instant::now();
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the captured process starts");
        let (tx, rx) = channel();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        for (i, mut r) in pipes.into_iter().enumerate() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut b = Vec::new();
                let _ = r.read_to_end(&mut b);
                let _ = tx.send((i, Instant::now(), b));
            });
        }
        Capture {
            child,
            t0,
            rx,
            exit: None,
            eof: [None, None],
        }
    }

    fn tick(&mut self) {
        if self.exit.is_none() {
            if let Ok(Some(s)) = self.child.try_wait() {
                self.exit = Some((Instant::now(), s));
            }
        }
        while let Ok((i, t, b)) = self.rx.try_recv() {
            self.eof[i] = Some((t, b));
        }
    }

    /// Exited and both pipes at EOF.
    pub fn returned(&self) -> bool {
        self.exit.is_some() && self.eof.iter().all(Option::is_some)
    }

    /// Poll every 10 ms until the process has returned or `deadline` has passed; whether it returned.
    pub fn until(&mut self, deadline: Instant) -> bool {
        loop {
            self.tick();
            if self.returned() || Instant::now() >= deadline {
                return self.returned();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Poll until the process has exited; it is killed when it has not exited `limit` after the start. The exit instant.
    pub fn wait_exit(&mut self, limit: Duration) -> Instant {
        let mut killed = false;
        loop {
            self.tick();
            if let Some((t, _)) = self.exit {
                return t;
            }
            if !killed && self.t0.elapsed() > limit {
                let _ = self.child.kill();
                killed = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Held at `after` past the exit: poll until then and report whether an EOF is still pending.
    pub fn held_after_exit(&mut self, after: Duration) -> bool {
        let exit = self.wait_exit(Duration::from_secs(60));
        !self.until(exit + after)
    }

    pub fn code(&self) -> Option<i32> {
        self.exit.as_ref().and_then(|(_, s)| s.code())
    }

    fn secs(&self, t: Instant) -> f64 {
        t.duration_since(self.t0).as_secs_f64()
    }

    /// Seconds from the start to the exit.
    pub fn exit_s(&self) -> Option<f64> {
        self.exit.as_ref().map(|(t, _)| self.secs(*t))
    }

    /// Seconds from the start to the EOF of stdout (0) or stderr (1).
    pub fn eof_s(&self, i: usize) -> Option<f64> {
        self.eof[i].as_ref().map(|(t, _)| self.secs(*t))
    }

    /// Seconds from the start to the return (the last of the exit and both EOFs).
    pub fn returned_s(&self) -> Option<f64> {
        if !self.returned() {
            return None;
        }
        [self.exit_s(), self.eof_s(0), self.eof_s(1)]
            .into_iter()
            .flatten()
            .reduce(f64::max)
    }

    /// Seconds from `t` to the EOF of pipe `i` (none while pending).
    pub fn eof_after(&self, i: usize, t: Instant) -> Option<f64> {
        self.eof[i].as_ref().map(|(e, _)| e.duration_since(t).as_secs_f64())
    }

    /// What pipe `i` carried (empty while pending).
    pub fn text(&self, i: usize) -> String {
        self.eof[i]
            .as_ref()
            .map(|(_, b)| String::from_utf8_lossy(b).to_string())
            .unwrap_or_default()
    }

    /// The instants in seconds after the start, for the test's report line.
    pub fn describe(&self) -> String {
        format!(
            "exit {:?} (code {:?}), stdout EOF {:?}, stderr EOF {:?}",
            self.exit_s(),
            self.code(),
            self.eof_s(0),
            self.eof_s(1)
        )
    }
}
