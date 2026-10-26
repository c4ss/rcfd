//! Test double of the `claude` executable (ADR 0126). It replays recorded streams and is used only when a test
//! passes its path as `Claude::program`; the user's PATH is never changed.
//!
//! It reads `.fake-claude.json` in its working directory (the project folder):
//! `{"stream": path, "agents": path, "agents_fail": bool, "hang": bool, "pidfile": path, "delay_ms": n}` and appends
//! `{"args": [...], "stdin": "..."}` to `.fake-claude-calls.jsonl` there. `agents --json` prints the `agents` file (or
//! fails); otherwise it reads stdin to the end and prints the `stream` file. With `hang` it starts a sleeping
//! grandchild, writes both pids to `pidfile`, prints the first stream line and sleeps forever.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::Duration;

fn sleep_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--fake-sleep") {
        sleep_forever();
    }
    let cfg: Value = std::fs::read_to_string(".fake-claude.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    let get = |k: &str| cfg.get(k).and_then(Value::as_str).map(String::from);
    let flag = |k: &str| cfg.get(k).and_then(Value::as_bool).unwrap_or(false);
    let mut stdin = String::new();
    if args.first().map(String::as_str) != Some("agents") {
        let _ = std::io::stdin().read_to_string(&mut stdin);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(".fake-claude-calls.jsonl")
    {
        let _ = writeln!(f, "{}", json!({"args": args, "stdin": stdin}));
    }
    let out = std::io::stdout();
    let mut out = out.lock();
    if args.first().map(String::as_str) == Some("agents") {
        if flag("agents_fail") {
            eprintln!("fake-claude: agents listing failed");
            std::process::exit(3);
        }
        let _ = out.write_all(
            get("agents")
                .and_then(|p| std::fs::read(p).ok())
                .unwrap_or_else(|| b"[]".to_vec())
                .as_slice(),
        );
        return;
    }
    let stream = get("stream")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    if flag("hang") {
        let me = std::env::current_exe().expect("current exe");
        let grandchild = std::process::Command::new(me)
            .arg("--fake-sleep")
            .spawn()
            .expect("spawn grandchild");
        if let Some(p) = get("pidfile") {
            let _ = std::fs::write(
                p,
                json!({"child": std::process::id(), "grandchild": grandchild.id()}).to_string(),
            );
        }
        let _ = writeln!(out, "{}", stream.lines().next().unwrap_or(""));
        let _ = out.flush();
        sleep_forever();
    }
    let delay = cfg.get("delay_ms").and_then(Value::as_u64).unwrap_or(0);
    for line in stream.lines() {
        let _ = writeln!(out, "{line}");
        if delay > 0 {
            let _ = out.flush();
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    eprintln!("fake-claude: replayed");
}
