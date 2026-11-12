//! Bulk field data beside the JSON-RPC service (F-447, ADR 0350 amendment 1): protocol `rcfd-data/1`, service protocol 1.2.
//!
//! JSON carries a field of a million cells badly (text numbers or base64). The `data.*` methods read a run's field store
//! (`rcfd_tsstore`, ADR 0124) and hand the values over a second TCP connection as raw little-endian bytes:
//!
//! * `data.info`: `{protocol, port, unix_ns}` (the data port; the service's clock in nanoseconds since the Unix epoch).
//! * `data.open {path}`: sizes, snapshots and fields of the store in the (sandboxed) folder `path`.
//! * `data.field {path, snapshot, field}`: one field of one snapshot as `f32`.
//! * `data.surface {path}`: the boundary faces of the stored mesh cut into triangles (a fan per face): `points` (`f32`,
//!   three per point), `triangles` (`u32`, three corners each) and `cells` (`u32`, the owner cell of each triangle).
//!
//! The last two answer `{ticket, sections: [{name, dtype, count}]}`. The bytes are prepared at the call and wait under the
//! ticket (at most [`MAX_TICKETS`]; the oldest is dropped). A client keeps one connection to `port`, writes the ticket as 8
//! bytes little-endian and reads, per section, the byte length as `u64` little-endian and then the values. A ticket is
//! used once; an unknown ticket is answered with the length `u64::MAX` and the connection is closed.
//!
//! The family is off unless the embedding program calls [`Service::enable_data`]. A service without it answers
//! `service.info` and every other method exactly as protocol 1.1 does, and `data.*` with "unknown method".

use crate::service::{code, Rpc, RpcError, Service};
use rcfd_tsstore::Reader;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Identifier of the bulk data calls, returned by `data.info` and by `service.info` (`data`) when they are on.
pub const DATA_PROTOCOL: &str = "rcfd-data/1";
/// `protocol_version` of a service whose `data.*` methods are on.
pub const DATA_PROTOCOL_VERSION: &str = "1.2";
/// The `data.*` methods, appended to `methods` of `service.info` when they are on.
pub const DATA_METHODS: [&str; 4] = ["data.info", "data.open", "data.field", "data.surface"];
/// Tickets that may wait at once.
pub const MAX_TICKETS: usize = 8;

struct Section {
    name: &'static str,
    dtype: &'static str,
    count: usize,
    bytes: Vec<u8>,
}

struct Inner {
    addr: SocketAddr,
    next: AtomicU64,
    tickets: Mutex<VecDeque<(u64, Vec<Section>)>>,
}

/// The data channel of a service (empty until [`Service::enable_data`]).
#[derive(Default)]
pub(crate) struct Channel(OnceLock<Arc<Inner>>);

fn serve_connection(inner: &Inner, mut stream: TcpStream) -> std::io::Result<()> {
    let mut t = [0u8; 8];
    loop {
        if stream.read_exact(&mut t).is_err() {
            return Ok(());
        }
        let ticket = u64::from_le_bytes(t);
        let found = {
            let mut q = inner.tickets.lock().unwrap_or_else(|e| e.into_inner());
            q.iter().position(|(k, _)| *k == ticket).and_then(|i| q.remove(i))
        };
        let Some((_, sections)) = found else {
            stream.write_all(&u64::MAX.to_le_bytes())?;
            return Ok(());
        };
        for s in &sections {
            stream.write_all(&(s.bytes.len() as u64).to_le_bytes())?;
            stream.write_all(&s.bytes)?;
        }
        stream.flush()?;
    }
}

impl Service {
    /// Switch the `data.*` methods on and listen for data connections on `addr` (port 0 for an ephemeral port). Returns
    /// the bound address; a second call returns the address of the first.
    pub fn enable_data(&self, addr: &str) -> std::io::Result<SocketAddr> {
        if let Some(i) = self.data.0.get() {
            return Ok(i.addr);
        }
        let listener = TcpListener::bind(addr)?;
        let inner = Arc::new(Inner {
            addr: listener.local_addr()?,
            next: AtomicU64::new(1),
            tickets: Mutex::new(VecDeque::new()),
        });
        if self.data.0.set(inner.clone()).is_err() {
            return Ok(self.data.0.get().map_or(inner.addr, |i| i.addr));
        }
        let shared = inner.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_nodelay(true);
                let inner = shared.clone();
                std::thread::spawn(move || {
                    let _ = serve_connection(&inner, stream);
                });
            }
        });
        Ok(inner.addr)
    }
}

/// Add what protocol 1.2 adds to the `service.info` reply; nothing when the data methods are off.
pub(crate) fn extend_info(svc: &Service, info: &mut Value) {
    if svc.data.0.get().is_none() {
        return;
    }
    info["protocol_version"] = json!(DATA_PROTOCOL_VERSION);
    info["data"] = json!(DATA_PROTOCOL);
    if let Some(m) = info["methods"].as_array_mut() {
        m.extend(DATA_METHODS.iter().map(|n| json!(n)));
    }
}

fn store_err(e: rcfd_core::Error) -> RpcError {
    RpcError::params(e.to_string())
}

fn open(svc: &Service, p: &Value) -> Rpc<Reader> {
    let path = p
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::params("missing string parameter 'path'"))?;
    Reader::open(&svc.sandbox_path(path)?).map_err(store_err)
}

fn f32_bytes(v: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 * v.len());
    for x in v {
        out.extend_from_slice(&(*x as f32).to_le_bytes());
    }
    out
}

fn u32_bytes(v: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 * v.len());
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn issue(inner: &Inner, sections: Vec<Section>) -> Value {
    let ticket = inner.next.fetch_add(1, Ordering::SeqCst);
    let list: Vec<Value> = sections
        .iter()
        .map(|s| json!({"name": s.name, "dtype": s.dtype, "count": s.count, "bytes": s.bytes.len()}))
        .collect();
    let mut q = inner.tickets.lock().unwrap_or_else(|e| e.into_inner());
    while q.len() >= MAX_TICKETS {
        q.pop_front();
    }
    q.push_back((ticket, sections));
    json!({"ticket": ticket, "sections": list})
}

fn surface(r: &mut Reader) -> Rpc<Vec<Section>> {
    let mesh = r.mesh().map_err(store_err)?;
    let t = mesh.topology();
    let too_big = || RpcError::new(code::LIMIT, "the mesh has more than 2^32 - 1 points or cells");
    if t.points.len() >= u32::MAX as usize || mesh.n_cells() >= u32::MAX as usize {
        return Err(too_big());
    }
    let mut new_id = vec![u32::MAX; t.points.len()];
    let (mut points, mut triangles, mut cells) = (Vec::<f64>::new(), Vec::<u32>::new(), Vec::<u32>::new());
    for f in t.neighbour.len()..t.owner.len() {
        let corners = &t.face_points[t.face_offsets[f]..t.face_offsets[f + 1]];
        for &c in corners {
            if new_id[c] == u32::MAX {
                new_id[c] = (points.len() / 3) as u32;
                points.extend_from_slice(&t.points[c]);
            }
        }
        for k in 1..corners.len().saturating_sub(1) {
            triangles.extend_from_slice(&[new_id[corners[0]], new_id[corners[k]], new_id[corners[k + 1]]]);
            cells.push(t.owner[f] as u32);
        }
    }
    Ok(vec![
        Section {
            name: "points",
            dtype: "f32",
            count: points.len(),
            bytes: f32_bytes(&points),
        },
        Section {
            name: "triangles",
            dtype: "u32",
            count: triangles.len(),
            bytes: u32_bytes(&triangles),
        },
        Section {
            name: "cells",
            dtype: "u32",
            count: cells.len(),
            bytes: u32_bytes(&cells),
        },
    ])
}

/// Dispatch a `data.*` method.
pub(crate) fn call(svc: &Service, method: &str, p: &Value) -> Rpc {
    let Some(inner) = svc.data.0.get() else {
        return Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{method}'"),
        ));
    };
    match method {
        "data.info" => {
            let ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64);
            Ok(json!({"protocol": DATA_PROTOCOL, "port": inner.addr.port(), "unix_ns": ns}))
        }
        "data.open" => {
            let mut r = open(svc, p)?;
            let snaps: Vec<Value> = r
                .snapshots()
                .iter()
                .map(|s| json!({"step": s.step, "time": s.time}))
                .collect();
            let fields = if snaps.is_empty() {
                Vec::new()
            } else {
                r.fields(0).map_err(store_err)?
            };
            let fields: Vec<Value> = fields
                .iter()
                .map(|f| {
                    json!({"name": f.name, "unit": f.unit, "location": f.location, "n": f.n,
                           "precision": f.precision, "codec": f.codec})
                })
                .collect();
            Ok(json!({
                "protocol": DATA_PROTOCOL, "n_cells": r.n_cells(), "n_points": r.n_points(),
                "mesh_hash": format!("{:016x}", r.mesh_hash()), "snapshots": snaps, "fields": fields,
            }))
        }
        "data.field" => {
            let mut r = open(svc, p)?;
            let i = p
                .get("snapshot")
                .and_then(Value::as_u64)
                .ok_or_else(|| RpcError::params("missing integer parameter 'snapshot'"))? as usize;
            let name = p
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::params("missing string parameter 'field'"))?;
            let v = r.read_field(i, name).map_err(store_err)?;
            Ok(issue(
                inner,
                vec![Section {
                    name: "values",
                    dtype: "f32",
                    count: v.len(),
                    bytes: f32_bytes(&v),
                }],
            ))
        }
        "data.surface" => {
            let mut r = open(svc, p)?;
            Ok(issue(inner, surface(&mut r)?))
        }
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{other}'"),
        )),
    }
}

/// Redeem a ticket on a data connection: the sections' bytes in order (for tests and Rust clients).
pub fn redeem(stream: &mut TcpStream, ticket: u64, sections: usize) -> std::io::Result<Vec<Vec<u8>>> {
    stream.write_all(&ticket.to_le_bytes())?;
    let mut out = Vec::with_capacity(sections);
    for _ in 0..sections {
        let mut l = [0u8; 8];
        stream.read_exact(&mut l)?;
        let len = u64::from_le_bytes(l);
        if len == u64::MAX {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "unknown data ticket"));
        }
        let mut b = vec![0u8; len as usize];
        stream.read_exact(&mut b)?;
        out.push(b);
    }
    Ok(out)
}
