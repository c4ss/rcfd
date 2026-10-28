//! ADR 0151 (job 48Q): the CLI render path of r4 (`render_cli_w39a.rs`) timed and broken down. Both tests are ignored and run
//! by hand in release builds.
//! * Q5: one `rcfd serve` per `rcfd` binary listed in `RCFD_W48Q_EXES` (`;`-separated; default: this build's), each with
//!   r4's session; then `RCFD_W48Q_ROUNDS` (default 7) rounds of `rcfd call service.info` and `rcfd call render.png`,
//!   alternating between the binaries. The stdout of every `render.png` call must be byte-identical.
//! * Q6: r4's session in an in-process `Service`: the `render.png` handler, the reply's serialisation, `base64`, the reply's
//!   transport over loopback TCP (as `serve_stream` writes it, and as one write) and the client's parse and print.

use rcfd_api::service::{base64, Service, ServiceConfig};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// r4's case: a 1000 x 1000 cavity whose initial speed |sin(6 x) y| varies over the domain.
const CASE: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 1000
ny = 1000
[physics]
nu = 0.01
[initial]
velocity_expression_x = "sin(6 * x / 1 [m]) * y / 1 [s]"
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
"#;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// One `rcfd call` process: its stdout and its wall time.
fn call(exe: &str, addr: &str, method: &str, params: &Value) -> (Vec<u8>, f64) {
    let t = Instant::now();
    let o = Command::new(exe)
        .args(["call", "--addr", addr, method, &params.to_string()])
        .output()
        .unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{method}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    (o.stdout, secs)
}

fn stats(xs: &[f64]) -> (f64, f64, f64) {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    (s[s.len() / 2], s[0], s[s.len() - 1])
}

#[test]
#[ignore = "ADR 0151 Q5: run by hand in release (RCFD_W48Q_EXES, RCFD_W48Q_ROUNDS); a 1M-cell session per server"]
fn q5_cli_render_timing() {
    let exes: Vec<String> = match std::env::var("RCFD_W48Q_EXES") {
        Ok(s) if !s.trim().is_empty() => s.split(';').map(|x| x.trim().to_string()).collect(),
        _ => vec![env!("CARGO_BIN_EXE_rcfd").to_string()],
    };
    let rounds = env_usize("RCFD_W48Q_ROUNDS", 7);
    let mut servers = Vec::new();
    for exe in &exes {
        let mut child = Command::new(exe)
            .args(["serve", "--addr", "127.0.0.1:0"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let srv = Server(child);
        let mut line = String::new();
        BufReader::new(out).read_line(&mut line).unwrap();
        let addr = line.trim().rsplit(' ').next().unwrap().to_string();
        let (s, t_create) = call(exe, &addr, "session.create", &json!({ "toml": CASE }));
        let s: Value = serde_json::from_slice(&s).unwrap();
        let id = s["session"].as_str().unwrap().to_string();
        let (_, t_init) = call(exe, &addr, "run.initialize", &json!({ "session": id }));
        eprintln!("Q5 {exe}: session.create {t_create:.3} s, run.initialize {t_init:.3} s");
        servers.push((exe.clone(), addr, id, srv));
    }
    let params = |id: &str| json!({ "session": id, "field": "speed", "width": 1024 });
    let mut reference: Option<Vec<u8>> = None;
    let mut first = Vec::new();
    for (exe, addr, id, _) in &servers {
        let (out, secs) = call(exe, addr, "render.png", &params(id));
        first.push(secs);
        match &reference {
            None => reference = Some(out),
            Some(r) => assert!(
                *r == out,
                "{exe}: the first render.png stdout differs from the first binary's"
            ),
        }
    }
    let mut info = vec![Vec::new(); servers.len()];
    let mut render = vec![Vec::new(); servers.len()];
    for round in 0..rounds {
        for (k, (exe, addr, id, _)) in servers.iter().enumerate() {
            let (_, ti) = call(exe, addr, "service.info", &json!({}));
            let (out, tr) = call(exe, addr, "render.png", &params(id));
            assert!(
                reference.as_ref() == Some(&out),
                "{exe}, round {round}: the render.png stdout differs from the first call's"
            );
            info[k].push(ti);
            render[k].push(tr);
        }
    }
    let r = reference.unwrap();
    let v: Value = serde_json::from_slice(&r).unwrap();
    eprintln!(
        "Q5 render.png stdout {} bytes, identical in all {} calls; image {} x {}, speed {} .. {}",
        r.len(),
        servers.len() * (rounds + 1),
        v["width"],
        v["height"],
        v["min"],
        v["max"]
    );
    eprintln!("Q5 per binary (s): first render.png; render.png median, min, max; service.info median, min, max ({rounds} rounds)");
    for (k, (exe, ..)) in servers.iter().enumerate() {
        let (rm, rl, rh) = stats(&render[k]);
        let (im, il, ih) = stats(&info[k]);
        eprintln!(
            "Q5 {exe}: first {:.3}; render.png {rm:.3} {rl:.3} {rh:.3}; service.info {im:.3} {il:.3} {ih:.3}",
            first[k]
        );
    }
}

/// Standard base64 decoding (the inverse of `rcfd_api::service::base64`).
fn decode64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for q in s.as_bytes().chunks(4) {
        let n = q.iter().filter(|&&c| c != b'=').count();
        let mut acc = 0u32;
        for (i, &c) in q.iter().enumerate() {
            acc |= u32::from(if c == b'=' { 0 } else { val(c) }) << (18 - 6 * i);
        }
        out.extend_from_slice(&acc.to_be_bytes()[1..n]);
    }
    out
}

/// Seconds from the first write until a loopback reader (a `BufReader` as `Client` uses) has the whole line: once written
/// as `serve_stream` writes it (`writeln!` straight to the `TcpStream`, then `flush`), once as a single `write_all`.
fn transport(envelope: &Value, line: &str) -> (f64, f64) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        let mut r = BufReader::new(s);
        for _ in 0..2 {
            let mut l = String::new();
            r.read_line(&mut l).unwrap();
            tx.send((Instant::now(), l.len())).unwrap();
        }
    });
    let mut w = TcpStream::connect(addr).unwrap();
    let t = Instant::now();
    writeln!(w, "{envelope}").unwrap();
    w.flush().unwrap();
    let (at, n1) = rx.recv().unwrap();
    let unbuffered = at.duration_since(t).as_secs_f64();
    let t = Instant::now();
    w.write_all(line.as_bytes()).unwrap();
    w.flush().unwrap();
    let (at, n2) = rx.recv().unwrap();
    let single = at.duration_since(t).as_secs_f64();
    reader.join().unwrap();
    assert_eq!((n1, n2), (line.len(), line.len()));
    (unbuffered, single)
}

#[test]
#[ignore = "ADR 0151 Q6: run by hand in release; a 1M-cell session in process"]
fn q6_cli_handler_breakdown() {
    let reps = env_usize("RCFD_W48Q_REPS", 5);
    let svc = Service::new(ServiceConfig::default());
    let s = svc.call("session.create", &json!({ "toml": CASE })).unwrap();
    let id = s["session"].as_str().unwrap().to_string();
    let t = Instant::now();
    svc.call("run.initialize", &json!({ "session": id })).unwrap();
    eprintln!("Q6 run.initialize in process {:.3} s", t.elapsed().as_secs_f64());
    let params = json!({ "session": id, "field": "speed", "width": 1024 });
    let names = [
        "handler render.png (field, render, png, base64, reply)",
        "reply line serialised (serve_stream's text)",
        "base64 of the PNG alone",
        "transport, serve_stream's unbuffered writeln!",
        "transport, one write_all",
        "client parse of the line",
        "client result clone and print text",
    ];
    let mut rows = vec![Vec::new(); names.len()];
    let mut sizes = (0, 0);
    for _ in 0..reps {
        let t = Instant::now();
        let r = svc.call("render.png", &params).unwrap();
        rows[0].push(t.elapsed().as_secs_f64());
        let envelope = json!({ "jsonrpc": "2.0", "id": 3, "result": r });
        let t = Instant::now();
        let line = format!("{envelope}\n");
        rows[1].push(t.elapsed().as_secs_f64());
        let b64 = envelope["result"]["data_base64"].as_str().unwrap();
        let png = decode64(b64);
        let t = Instant::now();
        let again = base64(&png);
        rows[2].push(t.elapsed().as_secs_f64());
        assert_eq!(again, b64);
        let (unbuffered, single) = transport(&envelope, &line);
        rows[3].push(unbuffered);
        rows[4].push(single);
        let t = Instant::now();
        let v: Value = serde_json::from_str(&line).unwrap();
        rows[5].push(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let result = v["result"].clone();
        let printed = format!("{result}\n");
        rows[6].push(t.elapsed().as_secs_f64());
        sizes = (png.len(), printed.len());
    }
    eprintln!(
        "Q6 PNG {} bytes, printed reply {} bytes; {reps} repetitions (ms: median, min, max)",
        sizes.0, sizes.1
    );
    for (name, xs) in names.iter().zip(&rows) {
        let (m, lo, hi) = stats(xs);
        eprintln!("Q6 {name:<56} {:>9.2} {:>9.2} {:>9.2}", m * 1e3, lo * 1e3, hi * 1e3);
    }
}
