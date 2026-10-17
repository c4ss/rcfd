//! Communicator abstraction (ADR 0058, D2, D3): point-to-point send and receive are the only required transport primitives;
//! halo exchange and global reductions are provided methods built on them, so an MPI or socket backend is a drop-in.

use std::fmt;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

/// Message tag; a receive checks that the tag matches the one the sender used.
pub type Tag = u32;

/// Tag of halo-exchange messages.
pub const TAG_HALO: Tag = 1;
/// Tag of reduction messages.
pub const TAG_REDUCE: Tag = 2;
/// Tag of barrier messages.
pub const TAG_BARRIER: Tag = 3;
/// Tag of gather messages.
pub const TAG_GATHER: Tag = 4;

/// A located communication failure: never a hang, always names rank, peer, tag and reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommError {
    /// Rank that observed the failure.
    pub rank: usize,
    /// Peer rank involved.
    pub peer: usize,
    /// Message tag involved.
    pub tag: Tag,
    /// What went wrong.
    pub reason: String,
}

impl fmt::Display for CommError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "communication error on rank {} with peer {} (tag {}): {}",
            self.rank, self.peer, self.tag, self.reason
        )
    }
}

impl std::error::Error for CommError {}

impl From<CommError> for rcfd_core::Error {
    fn from(e: CommError) -> Self {
        rcfd_core::Error::Numerical(e.to_string())
    }
}

/// One rank's view of a group of cooperating ranks.
///
/// Required: [`rank`](Communicator::rank), [`size`](Communicator::size), [`send`](Communicator::send),
/// [`recv`](Communicator::recv). Messages between one ordered pair of ranks are delivered in order. `send` must not block
/// waiting for the matching `recv` (buffered semantics), which the provided collectives rely on.
///
/// Reduction ordering policy (ADR 0058, D3): [`allreduce_sum`](Communicator::allreduce_sum) gathers the P partial values on
/// every rank and adds them sequentially in ascending rank order starting from partial 0, so every rank holds the bit-identical
/// result and the result is reproducible for a fixed number of ranks.
pub trait Communicator: Send + Sync {
    /// This rank, `0..size()`.
    fn rank(&self) -> usize;
    /// Number of ranks.
    fn size(&self) -> usize;
    /// Send `data` to `dest` (never to self).
    fn send(&self, dest: usize, tag: Tag, data: &[f64]) -> Result<(), CommError>;
    /// Receive the next message from `src`; its tag must equal `tag`.
    fn recv(&self, src: usize, tag: Tag) -> Result<Vec<f64>, CommError>;

    /// Every rank obtains the value of every rank, indexed by rank.
    fn allgather_scalar(&self, v: f64) -> Result<Vec<f64>, CommError> {
        let (me, p) = (self.rank(), self.size());
        if p == 1 {
            return Ok(vec![v]);
        }
        for q in (0..p).filter(|&q| q != me) {
            self.send(q, TAG_REDUCE, &[v])?;
        }
        let mut out = vec![0.0; p];
        for (q, o) in out.iter_mut().enumerate() {
            if q == me {
                *o = v;
            } else {
                let m = self.recv(q, TAG_REDUCE)?;
                if m.len() != 1 {
                    return Err(CommError {
                        rank: me,
                        peer: q,
                        tag: TAG_REDUCE,
                        reason: format!("reduction message of length {} (expected 1)", m.len()),
                    });
                }
                *o = m[0];
            }
        }
        Ok(out)
    }

    /// Global sum in the deterministic rank order; the serial case returns `v` itself.
    fn allreduce_sum(&self, v: f64) -> Result<f64, CommError> {
        if self.size() == 1 {
            return Ok(v);
        }
        let all = self.allgather_scalar(v)?;
        let mut s = all[0];
        for x in &all[1..] {
            s += *x;
        }
        Ok(s)
    }

    /// Global maximum (exact, order independent).
    fn allreduce_max(&self, v: f64) -> Result<f64, CommError> {
        if self.size() == 1 {
            return Ok(v);
        }
        let all = self.allgather_scalar(v)?;
        Ok(all.into_iter().fold(f64::NEG_INFINITY, f64::max))
    }

    /// Synchronise all ranks.
    fn barrier(&self) -> Result<(), CommError> {
        let (me, p) = (self.rank(), self.size());
        if p == 1 {
            return Ok(());
        }
        for q in (0..p).filter(|&q| q != me) {
            self.send(q, TAG_BARRIER, &[])?;
        }
        for q in (0..p).filter(|&q| q != me) {
            self.recv(q, TAG_BARRIER)?;
        }
        Ok(())
    }
}

/// Rank 0 of 1: exchange is a no-op and reductions return the local value unchanged (the serial fallback).
#[derive(Debug, Clone, Copy, Default)]
pub struct SerialComm;

impl Communicator for SerialComm {
    fn rank(&self) -> usize {
        0
    }
    fn size(&self) -> usize {
        1
    }
    fn send(&self, dest: usize, tag: Tag, _data: &[f64]) -> Result<(), CommError> {
        Err(CommError {
            rank: 0,
            peer: dest,
            tag,
            reason: "SerialComm has no peers".into(),
        })
    }
    fn recv(&self, src: usize, tag: Tag) -> Result<Vec<f64>, CommError> {
        Err(CommError {
            rank: 0,
            peer: src,
            tag,
            reason: "SerialComm has no peers".into(),
        })
    }
}

struct Msg {
    tag: Tag,
    data: Vec<f64>,
}

/// In-process rank: one unbounded channel per ordered (source, destination) pair.
pub struct ThreadComm {
    rank: usize,
    size: usize,
    tx: Vec<Option<Sender<Msg>>>,
    rx: Vec<Option<Mutex<Receiver<Msg>>>>,
    timeout: Duration,
}

/// Default receive timeout of the in-process transport.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

impl ThreadComm {
    /// Build the communicators of `p` ranks (rank order).
    pub fn group(p: usize, timeout: Duration) -> Vec<ThreadComm> {
        let mut tx: Vec<Vec<Option<Sender<Msg>>>> = (0..p).map(|_| (0..p).map(|_| None).collect()).collect();
        let mut rx: Vec<Vec<Option<Mutex<Receiver<Msg>>>>> = (0..p).map(|_| (0..p).map(|_| None).collect()).collect();
        for s in 0..p {
            for d in 0..p {
                if s != d {
                    let (t, r) = channel();
                    tx[s][d] = Some(t);
                    rx[d][s] = Some(Mutex::new(r));
                }
            }
        }
        tx.into_iter()
            .zip(rx)
            .enumerate()
            .map(|(rank, (tx, rx))| ThreadComm {
                rank,
                size: p,
                tx,
                rx,
                timeout,
            })
            .collect()
    }
}

impl Communicator for ThreadComm {
    fn rank(&self) -> usize {
        self.rank
    }
    fn size(&self) -> usize {
        self.size
    }
    fn send(&self, dest: usize, tag: Tag, data: &[f64]) -> Result<(), CommError> {
        let err = |reason: String| CommError {
            rank: self.rank,
            peer: dest,
            tag,
            reason,
        };
        let Some(Some(t)) = self.tx.get(dest) else {
            return Err(err(format!("no channel to rank {dest} (size {})", self.size)));
        };
        t.send(Msg {
            tag,
            data: data.to_vec(),
        })
        .map_err(|_| err("peer has terminated".into()))
    }
    fn recv(&self, src: usize, tag: Tag) -> Result<Vec<f64>, CommError> {
        let err = |reason: String| CommError {
            rank: self.rank,
            peer: src,
            tag,
            reason,
        };
        let Some(Some(r)) = self.rx.get(src) else {
            return Err(err(format!("no channel from rank {src} (size {})", self.size)));
        };
        let guard = r.lock().map_err(|_| err("receive channel poisoned".into()))?;
        match guard.recv_timeout(self.timeout) {
            Ok(m) if m.tag == tag => Ok(m.data),
            Ok(m) => Err(err(format!("received tag {} while waiting for tag {}", m.tag, tag))),
            Err(RecvTimeoutError::Timeout) => Err(err(format!("no message within {:?}", self.timeout))),
            Err(RecvTimeoutError::Disconnected) => Err(err("peer has terminated".into())),
        }
    }
}

/// Run `f` on `p` in-process ranks (one `std::thread` each, not rayon tasks) and return the results in rank order. A panic on
/// any rank is propagated after all ranks have finished.
pub fn run_ranks<T, F>(p: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(&ThreadComm) -> T + Sync,
{
    run_ranks_with_timeout(p, DEFAULT_TIMEOUT, f)
}

/// [`run_ranks`] with an explicit receive timeout.
pub fn run_ranks_with_timeout<T, F>(p: usize, timeout: Duration, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(&ThreadComm) -> T + Sync,
{
    let comms = ThreadComm::group(p, timeout);
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = comms.into_iter().map(|c| s.spawn(move || f(&c))).collect();
        let mut out = Vec::new();
        let mut panic = None;
        for h in handles {
            match h.join() {
                Ok(v) => out.push(v),
                Err(e) => {
                    panic.get_or_insert(e);
                }
            }
        }
        if let Some(e) = panic {
            std::panic::resume_unwind(e);
        }
        out
    })
}
