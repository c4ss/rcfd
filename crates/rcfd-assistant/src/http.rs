//! Small blocking HTTP layer behind a trait so adapters can be tested without a network.

use crate::types::ProviderError;
use std::io::BufRead;
use std::sync::Mutex;
use std::time::Duration;

/// A POST (or GET when `body` is `None`) request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Full URL.
    pub url: String,
    /// Header name/value pairs.
    pub headers: Vec<(String, String)>,
    /// JSON body for POST.
    pub body: Option<String>,
}

impl HttpRequest {
    /// Value of the header called `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Status and body of a response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status.
    pub status: u16,
    /// Body text (empty for a successful streamed response).
    pub body: String,
}

/// HTTP transport.
pub trait Transport: Send + Sync {
    /// Send and read the whole response.
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, ProviderError>;

    /// Send and deliver a successful response line by line. For a non-2xx status
    /// the lines are not delivered and the body is returned in full.
    fn send_lines(&self, req: &HttpRequest, on_line: &mut dyn FnMut(&str)) -> Result<HttpResponse, ProviderError>;
}

/// Blocking transport over `ureq`.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// Transport with the given overall timeout (0 = 5 minutes).
    pub fn new(timeout_secs: u64) -> Self {
        let t = if timeout_secs == 0 { 300 } else { timeout_secs };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(t)))
            .build()
            .into();
        UreqTransport { agent }
    }

    fn call(&self, req: &HttpRequest) -> Result<ureq::http::Response<ureq::Body>, ProviderError> {
        let res = match &req.body {
            Some(body) => {
                let mut b = self.agent.post(&req.url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.send(body.as_bytes())
            }
            None => {
                let mut b = self.agent.get(&req.url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.call()
            }
        };
        // ureq error text does not include request headers, so it cannot carry the key.
        res.map_err(|e| ProviderError::Transport(e.to_string()))
    }
}

impl Transport for UreqTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, ProviderError> {
        let mut resp = self.call(req)?;
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        Ok(HttpResponse { status, body })
    }

    fn send_lines(&self, req: &HttpRequest, on_line: &mut dyn FnMut(&str)) -> Result<HttpResponse, ProviderError> {
        let mut resp = self.call(req)?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let body = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| ProviderError::Transport(e.to_string()))?;
            return Ok(HttpResponse { status, body });
        }
        let reader = std::io::BufReader::new(resp.body_mut().as_reader());
        for line in reader.lines() {
            let line = line.map_err(|e| ProviderError::Transport(e.to_string()))?;
            on_line(&line);
        }
        Ok(HttpResponse {
            status,
            body: String::new(),
        })
    }
}

/// Test transport: records requests and answers from a queue of canned responses.
#[derive(Default)]
pub struct FakeTransport {
    /// Requests seen so far.
    pub requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<std::collections::VecDeque<HttpResponse>>,
}

impl FakeTransport {
    /// Empty transport.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a response.
    pub fn push(&self, status: u16, body: impl Into<String>) {
        self.responses.lock().unwrap().push_back(HttpResponse {
            status,
            body: body.into(),
        });
    }

    /// The most recent request.
    pub fn last_request(&self) -> Option<HttpRequest> {
        self.requests.lock().unwrap().last().cloned()
    }

    fn next(&self, req: &HttpRequest) -> Result<HttpResponse, ProviderError> {
        self.requests.lock().unwrap().push(req.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| ProviderError::Transport("no canned response queued".into()))
    }
}

impl Transport for FakeTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, ProviderError> {
        self.next(req)
    }

    fn send_lines(&self, req: &HttpRequest, on_line: &mut dyn FnMut(&str)) -> Result<HttpResponse, ProviderError> {
        let r = self.next(req)?;
        if (200..300).contains(&r.status) {
            for l in r.body.lines() {
                on_line(l);
            }
            return Ok(HttpResponse {
                status: r.status,
                body: String::new(),
            });
        }
        Ok(r)
    }
}

/// Trim a body for error messages.
pub(crate) fn trim_body(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 400 {
        let t: String = s.chars().take(400).collect();
        format!("{t}...")
    } else {
        s.to_string()
    }
}
