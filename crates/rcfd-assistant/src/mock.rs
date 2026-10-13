//! Scripted provider for tests and offline demos.

use crate::types::*;
use std::collections::VecDeque;
use std::sync::Mutex;

type Dynamic = Box<dyn Fn(&ChatRequest) -> ChatResponse + Send + Sync>;

enum Reply {
    Fixed(ChatResponse),
    Dynamic(Dynamic),
}

/// A provider that answers from a queue of canned or computed replies and
/// records every request it receives.
pub struct MockProvider {
    script: Mutex<VecDeque<Reply>>,
    /// Requests received, oldest first.
    pub requests: Mutex<Vec<ChatRequest>>,
}

impl MockProvider {
    /// Provider that replays `replies` in order.
    pub fn new(replies: Vec<ChatResponse>) -> Self {
        MockProvider {
            script: Mutex::new(replies.into_iter().map(Reply::Fixed).collect()),
            requests: Mutex::new(vec![]),
        }
    }

    /// Provider with no replies yet.
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    /// Queue a fixed reply.
    pub fn push(&self, r: ChatResponse) {
        self.script.lock().unwrap().push_back(Reply::Fixed(r));
    }

    /// Queue a reply computed from the request (e.g. to summarise earlier tool results).
    pub fn push_fn(&self, f: impl Fn(&ChatRequest) -> ChatResponse + Send + Sync + 'static) {
        self.script.lock().unwrap().push_back(Reply::Dynamic(Box::new(f)));
    }

    /// Number of requests received.
    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Provider for MockProvider {
    fn name(&self) -> String {
        "mock".into()
    }

    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.requests.lock().unwrap().push(req.clone());
        match self.script.lock().unwrap().pop_front() {
            Some(Reply::Fixed(r)) => Ok(r),
            Some(Reply::Dynamic(f)) => Ok(f(req)),
            None => Err(ProviderError::Config(
                "mock provider has no more scripted replies".into(),
            )),
        }
    }
}
