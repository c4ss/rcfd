//! Loopback-only mock HTTP server for tests (ADR 0025 D4). `std::net` only; one thread per connection.
//! It speaks just enough HTTP/1.1 (request line, headers, Content-Length body) to stand in for an
//! OpenAI-compatible server. It proves the client wire behaviour against this mock, not against any real server.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A request as received.
#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Req {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// Everything except the credential headers (Authorization; x-api-key, which the Anthropic protocol uses), as one string (for leak scans).
    pub fn everything_but_auth(&self) -> String {
        let h: Vec<String> = self
            .headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("authorization") && !k.eq_ignore_ascii_case("x-api-key"))
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        format!("{} {}\n{}\n{}", self.method, self.path, h.join("\n"), self.body)
    }
}

/// What to answer.
#[derive(Debug, Clone)]
pub enum Reply {
    /// Fixed-length JSON body.
    Json(u16, String),
    /// 200 with `text/event-stream`, sent as HTTP chunks (one per entry) and terminated properly.
    Sse(Vec<String>),
    /// Like `Sse` but the connection is closed without the terminating chunk.
    SseCut(Vec<String>),
    /// Accept, read the request, then say nothing for this long.
    Hang(Duration),
    /// Promise 1000 bytes, send 10, close.
    ShortBody,
}

pub struct Mock {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<Req>>>,
    /// Connections accepted (including ones that never sent a request).
    pub accepts: Arc<AtomicUsize>,
    script: Arc<Mutex<VecDeque<Reply>>>,
    stop: Arc<AtomicBool>,
}

impl Mock {
    pub fn start() -> Mock {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = l.local_addr().unwrap().port();
        let m = Mock {
            port,
            requests: Arc::default(),
            accepts: Arc::default(),
            script: Arc::default(),
            stop: Arc::default(),
        };
        let (reqs, acc, script, stop) = (m.requests.clone(), m.accepts.clone(), m.script.clone(), m.stop.clone());
        std::thread::spawn(move || {
            for conn in l.incoming() {
                if stop.load(SeqCst) {
                    break;
                }
                let Ok(s) = conn else { continue };
                acc.fetch_add(1, SeqCst);
                let (reqs, script) = (reqs.clone(), script.clone());
                std::thread::spawn(move || handle(s, &reqs, &script));
            }
        });
        m
    }

    pub fn push(&self, r: Reply) {
        self.script.lock().unwrap().push_back(r);
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn requests(&self) -> Vec<Req> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn handle(mut s: TcpStream, reqs: &Mutex<Vec<Req>>, script: &Mutex<VecDeque<Reply>>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buf = vec![];
    let mut tmp = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match s.read(&mut tmp) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (method, path) = (
        first.next().unwrap_or_default().to_string(),
        first.next().unwrap_or_default().to_string(),
    );
    let headers: Vec<(String, String)> = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            l.split_once(':')
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        })
        .collect();
    let len: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + len {
        match s.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    let body = String::from_utf8_lossy(&buf[head_end..(head_end + len).min(buf.len())]).to_string();
    reqs.lock().unwrap().push(Req {
        method,
        path,
        headers,
        body,
    });
    let reply = script
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| Reply::Json(500, r#"{"error":"mock script empty"}"#.into()));
    let reply_kind = usize::from(matches!(reply, Reply::SseCut(_)));
    match reply {
        Reply::Json(code, b) => {
            let _ = write!(
                s,
                "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                b.len()
            );
        }
        Reply::Sse(chunks) | Reply::SseCut(chunks) => {
            let cut = reply_kind == 1;
            let _ = s
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n");
            for c in &chunks {
                let _ = s.write_all(format!("{:x}\r\n{c}\r\n", c.len()).as_bytes());
                let _ = s.flush();
            }
            if !cut {
                let _ = s.write_all(b"0\r\n\r\n");
            }
        }
        Reply::Hang(d) => std::thread::sleep(d),
        Reply::ShortBody => {
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n0123456789"
            );
        }
    }
    let _ = s.flush();
}
