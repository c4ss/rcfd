//! ADR 0739 Q9: `rcfd serve` refuses a non-loopback `--addr` unless `--allow-unauthenticated-remote` is given; a
//! loopback address behaves as before.
//!
//! No test here listens on a non-loopback interface: the flag is shown to pass the check with an address of the TEST-NET
//! block (192.0.2.1, RFC 5737), which no machine has, so the bind itself fails with exit 1 instead of the refusal's 2.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn rcfd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
}

#[test]
fn a_non_loopback_addr_is_refused_with_exit_2_before_anything_listens() {
    for addr in ["0.0.0.0:0", "192.0.2.1:0", "[::]:0"] {
        let o = rcfd().args(["serve", "--addr", addr]).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{addr}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("not a loopback address"), "{addr}: {err}");
        assert!(err.contains("--allow-unauthenticated-remote"), "{addr}: {err}");
        assert!(err.contains("SSH"), "{addr}: {err}");
        assert!(o.stdout.is_empty(), "{addr}: nothing listened");
    }
}

#[test]
fn the_flag_lifts_the_refusal() {
    let o = rcfd()
        .args(["serve", "--addr", "192.0.2.1:0", "--allow-unauthenticated-remote"])
        .output()
        .unwrap();
    // past the check: the bind is tried and fails as any bind failure does
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("cannot listen on 192.0.2.1:0"), "{err}");
    assert!(!err.contains("not a loopback address"), "{err}");
}

fn listens(addr: &str) -> String {
    let mut child = rcfd()
        .args(["serve", "--addr", addr])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _ = child.kill();
    let _ = child.wait();
    line
}

#[test]
fn a_loopback_addr_listens_as_before() {
    let line = listens("127.0.0.1:0");
    assert!(line.starts_with("rcfd service listening on 127.0.0.1:"), "{line}");
    let line = listens("localhost:0");
    assert!(line.starts_with("rcfd service listening on "), "{line}");
}

#[test]
fn help_names_the_flag() {
    let o = rcfd().arg("--help").output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("[--allow-unauthenticated-remote]"), "{out}");
    assert!(out.contains("--addr must be a loopback address"), "{out}");
}
