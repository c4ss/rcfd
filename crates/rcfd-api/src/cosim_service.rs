//! The co-simulation calls of `rcfd_fv::cosim` over the solver service (F-360, ADR 0345): protocol `rcfd-cosim/1`, part of
//! service protocol 1.1. See `docs/cosimulation-protocol.md` (the calls and the wire format) and
//! `docs/service-protocol.md`.
//!
//! Two halves:
//!
//! * **hosting.** The embedding program hands a participant to a [`Service`] ([`Service::host_fluid`],
//!   [`Service::host_structure`]). The participant is built and called on a thread of its own, one call at a time in the
//!   order the requests arrive, so it need not be `Send` (the flow participant `FlowWall` is not). The service then
//!   answers `cosim.list`, `cosim.open`, `cosim.describe`, `cosim.time`, `cosim.checkpoint`, `cosim.restore`,
//!   `cosim.forces` and `cosim.advance` for it, over TCP, stdio or [`Service::call`]. Nothing is hosted by default.
//! * **proxies.** [`RemoteStructure`] and [`RemoteFluid`] implement the participant traits by sending every call as one
//!   request to a service that hosts the participant, so `rcfd_fv::cosim::coupled_step` drives a participant in another
//!   process unchanged.
//!
//! Real numbers travel as JSON numbers and cross bit for bit (the workspace builds `serde_json` with `float_roundtrip`).
//!
//! Every call but `cosim.list` and `cosim.open` carries the **lease** that `cosim.open` returned. A later `cosim.open`
//! of the same participant supersedes it, so a request left over from a dropped connection cannot be applied after the
//! coupler has restored the participant over a new one. A proxy whose connection is lost fails every call at once except
//! `restore`, which reconnects once (a new lease) and restores the participant to its checkpoint.

use crate::service::{code, Rpc, RpcError, Service};
use rcfd_core::vec3::Vec3;
use rcfd_fv::cosim::{Fluid, Participant, Structure};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// Identifier of the co-simulation calls, returned by `cosim.list` and `cosim.open` and by `service.info` (`cosim`).
pub const COSIM_PROTOCOL: &str = "rcfd-cosim/1";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- hosting ---------------------------------------------------------------------------------------------------------

/// A hosted participant.
enum Hosted {
    Fluid(Box<dyn Fluid>),
    Structure(Box<dyn Structure>),
}

impl Hosted {
    fn kind(&self) -> &'static str {
        match self {
            Hosted::Fluid(_) => "fluid",
            Hosted::Structure(_) => "structure",
        }
    }

    fn name(&self) -> String {
        match self {
            Hosted::Fluid(f) => f.name().to_string(),
            Hosted::Structure(s) => s.name().to_string(),
        }
    }

    /// The participant's time; an error when it is not finite (JSON cannot carry it).
    fn time(&self) -> Rpc<f64> {
        let t = match self {
            Hosted::Fluid(f) => f.time(),
            Hosted::Structure(s) => s.time(),
        };
        if t.is_finite() {
            Ok(t)
        } else {
            Err(RpcError::new(
                code::PARTICIPANT,
                format!("the participant reports the time {t}"),
            ))
        }
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        match self {
            Hosted::Fluid(f) => f.checkpoint(),
            Hosted::Structure(s) => s.checkpoint(),
        }
    }

    fn restore(&mut self) -> Result<(), String> {
        match self {
            Hosted::Fluid(f) => f.restore(),
            Hosted::Structure(s) => s.restore(),
        }
    }
}

struct Job {
    method: String,
    params: Value,
    reply: Sender<Rpc>,
}

struct Entry {
    kind: &'static str,
    name: String,
    jobs: Sender<Job>,
}

/// The participants a service hosts, by the name they were hosted under.
#[derive(Default)]
pub(crate) struct Hosts {
    map: Mutex<BTreeMap<String, Entry>>,
}

fn participant_error(message: String) -> RpcError {
    RpcError::new(code::PARTICIPANT, message)
}

/// Vectors as a JSON array of `[x, y, z]`; a non-finite value is the participant's error.
fn vectors_json(v: &[Vec3], what: &str) -> Rpc {
    if !v.iter().flatten().all(|x| x.is_finite()) {
        return Err(participant_error(format!(
            "the participant returned a non-finite value among the {what}"
        )));
    }
    Ok(Value::Array(v.iter().map(|a| json!([a[0], a[1], a[2]])).collect()))
}

/// The member `key` of `v` as vectors: an array of arrays of three numbers.
fn vectors_from(v: &Value, key: &str) -> Result<Vec<Vec3>, String> {
    let rows = v
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("'{key}' must be an array of [x, y, z]"))?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let c: Vec<f64> = row
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            if c.len() == 3 && row.as_array().is_some_and(|a| a.len() == 3) {
                Ok([c[0], c[1], c[2]])
            } else {
                Err(format!("'{key}[{i}]' must be three numbers"))
            }
        })
        .collect()
}

/// One call, applied on the participant's thread.
fn serve(hosted: &mut Hosted, lease: &mut u64, id: &str, method: &str, p: &Value) -> Rpc {
    if method == "cosim.open" {
        let time = hosted.time()?;
        *lease += 1;
        return Ok(json!({
            "protocol": COSIM_PROTOCOL,
            "lease": *lease,
            "kind": hosted.kind(),
            "name": hosted.name(),
            "time": time,
        }));
    }
    let given = p
        .get("lease")
        .and_then(Value::as_u64)
        .ok_or_else(|| RpcError::params("missing integer parameter 'lease' (cosim.open returns it)"))?;
    if given != *lease {
        return Err(RpcError::new(
            code::BAD_STATE,
            format!(
                "the lease {given} of participant '{id}' is not the current one (a later cosim.open superseded it, or none was issued)"
            ),
        ));
    }
    match method {
        "cosim.describe" => match hosted {
            Hosted::Structure(s) => Ok(json!({
                "kind": "structure",
                "name": s.name(),
                "nodes": vectors_json(&s.node_coordinates(), "node coordinates")?,
            })),
            Hosted::Fluid(f) => Ok(json!({
                "kind": "fluid",
                "name": f.name(),
                "points": vectors_json(&f.point_coordinates(), "point coordinates")?,
                "faces": f.face_points(),
            })),
        },
        "cosim.time" => Ok(json!({"time": hosted.time()?})),
        "cosim.checkpoint" => {
            hosted.checkpoint().map_err(participant_error)?;
            Ok(json!({"time": hosted.time()?}))
        }
        "cosim.restore" => {
            hosted.restore().map_err(participant_error)?;
            Ok(json!({"time": hosted.time()?}))
        }
        "cosim.forces" => match hosted {
            Hosted::Fluid(f) => {
                let forces = f.forces().map_err(participant_error)?;
                Ok(json!({"forces": vectors_json(&forces, "face forces")?, "time": hosted.time()?}))
            }
            Hosted::Structure(_) => Err(RpcError::new(
                code::BAD_STATE,
                format!("cosim.forces is a call of a fluid participant; '{id}' is a structure"),
            )),
        },
        "cosim.advance" => {
            let dt = p
                .get("dt")
                .and_then(Value::as_f64)
                .ok_or_else(|| RpcError::params("missing number parameter 'dt'"))?;
            match hosted {
                Hosted::Structure(s) => {
                    let forces = vectors_from(p, "forces").map_err(RpcError::params)?;
                    let q = s.advance(dt, &forces).map_err(participant_error)?;
                    Ok(json!({"displacements": vectors_json(&q, "node displacements")?, "time": hosted.time()?}))
                }
                Hosted::Fluid(f) => {
                    let d = vectors_from(p, "displacements").map_err(RpcError::params)?;
                    let forces = f.advance(dt, &d).map_err(participant_error)?;
                    Ok(json!({"forces": vectors_json(&forces, "face forces")?, "time": hosted.time()?}))
                }
            }
        }
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{other}'"),
        )),
    }
}

/// The `cosim.*` methods of [`Service::call`].
pub(crate) fn call(svc: &Service, method: &str, p: &Value) -> Rpc {
    match method {
        "cosim.list" => {
            let map = lock(&svc.cosim.map);
            let list: Vec<Value> = map
                .iter()
                .map(|(id, e)| json!({"participant": id, "kind": e.kind, "name": e.name}))
                .collect();
            Ok(json!({"protocol": COSIM_PROTOCOL, "participants": list}))
        }
        "cosim.open" | "cosim.describe" | "cosim.time" | "cosim.checkpoint" | "cosim.restore" | "cosim.forces"
        | "cosim.advance" => {
            let id = p
                .get("participant")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::params("missing string parameter 'participant'"))?;
            let jobs = lock(&svc.cosim.map)
                .get(id)
                .map(|e| e.jobs.clone())
                .ok_or_else(|| RpcError::new(code::NO_PARTICIPANT, format!("no participant '{id}'")))?;
            let stopped = || RpcError::new(code::INTERNAL, format!("the thread of participant '{id}' has stopped"));
            let (reply, answer) = channel();
            jobs.send(Job {
                method: method.to_string(),
                params: p.clone(),
                reply,
            })
            .map_err(|_| stopped())?;
            answer.recv().map_err(|_| stopped())?
        }
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{other}'"),
        )),
    }
}

impl Service {
    fn host(
        &self,
        participant: &str,
        make: impl FnOnce() -> Result<Hosted, String> + Send + 'static,
    ) -> Result<(), String> {
        if participant.is_empty() {
            return Err("co-simulation: a hosted participant needs a name".into());
        }
        if lock(&self.cosim.map).contains_key(participant) {
            return Err(format!(
                "co-simulation: a participant '{participant}' is already hosted"
            ));
        }
        let (jobs, inbox) = channel::<Job>();
        let (ready, built) = channel();
        let id = participant.to_string();
        std::thread::Builder::new()
            .name(format!("cosim-{participant}"))
            .spawn(move || {
                let mut hosted = match make() {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                let _ = ready.send(Ok((hosted.kind(), hosted.name())));
                let mut lease = 0u64;
                for job in inbox {
                    let answer = serve(&mut hosted, &mut lease, &id, &job.method, &job.params);
                    let _ = job.reply.send(answer);
                }
            })
            .map_err(|e| format!("co-simulation: no thread for participant '{participant}': {e}"))?;
        let (kind, name) = built.recv().map_err(|_| {
            format!("co-simulation: participant '{participant}' could not be built (its thread stopped)")
        })??;
        let mut map = lock(&self.cosim.map);
        if map.contains_key(participant) {
            return Err(format!(
                "co-simulation: a participant '{participant}' is already hosted"
            ));
        }
        map.insert(participant.to_string(), Entry { kind, name, jobs });
        Ok(())
    }

    /// Host a fluid participant under the name `participant` (F-360): `make` builds it on the participant's own thread,
    /// where every `cosim.*` call for it is applied, one at a time. Returns when it is built. Fails for an empty or a
    /// taken name and when `make` fails.
    pub fn host_fluid(
        &self,
        participant: &str,
        make: impl FnOnce() -> Result<Box<dyn Fluid>, String> + Send + 'static,
    ) -> Result<(), String> {
        self.host(participant, move || make().map(Hosted::Fluid))
    }

    /// Host a structural participant under the name `participant`; as [`Service::host_fluid`].
    pub fn host_structure(
        &self,
        participant: &str,
        make: impl FnOnce() -> Result<Box<dyn Structure>, String> + Send + 'static,
    ) -> Result<(), String> {
        self.host(participant, move || make().map(Hosted::Structure))
    }

    /// Stop hosting `participant`: its thread ends after the call it is in and the participant is dropped. Returns
    /// whether it was hosted.
    pub fn unhost(&self, participant: &str) -> bool {
        lock(&self.cosim.map).remove(participant).is_some()
    }
}

// ---- proxies ---------------------------------------------------------------------------------------------------------

/// Time-outs of a proxy's connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteSettings {
    /// Longest wait for a connection (also for the reconnection of `restore`).
    pub connect_timeout: Duration,
    /// Longest wait for the reply of one call; a call that outlasts it counts as a lost connection.
    pub call_timeout: Duration,
}

impl Default for RemoteSettings {
    /// 5 s to connect, 120 s for a call.
    fn default() -> Self {
        RemoteSettings {
            connect_timeout: Duration::from_secs(5),
            call_timeout: Duration::from_secs(120),
        }
    }
}

/// The connection of a proxy to one hosted participant.
struct Link {
    address: SocketAddr,
    participant: String,
    settings: RemoteSettings,
    stream: Option<(BufReader<TcpStream>, TcpStream)>,
    next: u64,
    lease: u64,
    time: f64,
    reconnects: usize,
}

impl Link {
    fn new(address: SocketAddr, participant: &str, settings: RemoteSettings) -> Self {
        Link {
            address,
            participant: participant.to_string(),
            settings,
            stream: None,
            next: 1,
            lease: 0,
            time: f64::NAN,
            reconnects: 0,
        }
    }

    /// Connect and take a new lease (`cosim.open`). Returns the reply of `cosim.open`.
    fn open(&mut self) -> Result<Value, String> {
        self.stream = None;
        let refused = |e: std::io::Error| format!("could not connect to {}: {e}", self.address);
        let stream = TcpStream::connect_timeout(&self.address, self.settings.connect_timeout).map_err(refused)?;
        stream
            .set_read_timeout(Some(self.settings.call_timeout))
            .map_err(refused)?;
        stream
            .set_write_timeout(Some(self.settings.call_timeout))
            .map_err(refused)?;
        stream.set_nodelay(true).map_err(refused)?;
        let reader = BufReader::new(stream.try_clone().map_err(refused)?);
        self.stream = Some((reader, stream));
        let reply = self.request("cosim.open", json!({"participant": self.participant}))?;
        if reply["protocol"] != COSIM_PROTOCOL {
            self.stream = None;
            return Err(format!(
                "the service at {} speaks {} for co-simulation, {COSIM_PROTOCOL} expected",
                self.address, reply["protocol"]
            ));
        }
        self.lease = reply["lease"]
            .as_u64()
            .ok_or_else(|| format!("cosim.open of '{}' returned no lease", self.participant))?;
        self.take_time(&reply);
        Ok(reply)
    }

    fn take_time(&mut self, reply: &Value) {
        if let Some(t) = reply.get("time").and_then(Value::as_f64) {
            self.time = t;
        }
    }

    /// One request line and one reply line. A failure of the connection closes it.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let address = self.address;
        let Some((reader, writer)) = self.stream.as_mut() else {
            return Err(format!(
                "not connected to {address}: the connection was lost earlier (restore or reconnect opens a new one)"
            ));
        };
        let id = self.next;
        self.next += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let mut exchange = || -> Result<Value, String> {
            writeln!(writer, "{request}").map_err(|e| e.to_string())?;
            writer.flush().map_err(|e| e.to_string())?;
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("the service closed the connection".into());
            }
            let reply: Value = serde_json::from_str(&line).map_err(|e| format!("the reply is not JSON ({e})"))?;
            if reply.get("id").and_then(Value::as_u64) != Some(id) {
                return Err("the reply does not answer the request".into());
            }
            Ok(reply)
        };
        let reply = match exchange() {
            Ok(reply) => reply,
            Err(e) => {
                self.stream = None;
                return Err(format!("the connection to {address} was lost during {method}: {e}"));
            }
        };
        match reply.get("error") {
            None => Ok(reply["result"].clone()),
            Some(e) => {
                let message = e["message"].as_str().unwrap_or("error without a message");
                match e["code"].as_i64() {
                    Some(code::PARTICIPANT) => Err(message.to_string()),
                    c => Err(format!("{message} (service error {}, {method})", c.unwrap_or(0))),
                }
            }
        }
    }

    /// A call of the participant: `extra` (an object) plus the participant and the lease.
    fn call(&mut self, method: &str, mut extra: Value) -> Result<Value, String> {
        extra["participant"] = json!(self.participant);
        extra["lease"] = json!(self.lease);
        let reply = self.request(method, extra)?;
        self.take_time(&reply);
        Ok(reply)
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.open()?;
        self.reconnects += 1;
        Ok(())
    }

    /// `cosim.restore`; on a lost connection (known, or found by this call) reconnect once and restore.
    fn restore(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            match self.call("cosim.restore", json!({})) {
                Ok(_) => return Ok(()),
                Err(e) if self.stream.is_some() => return Err(e),
                Err(_) => {}
            }
        }
        self.reconnect()?;
        self.call("cosim.restore", json!({})).map(|_| ())
    }
}

fn vectors_value(v: &[Vec3]) -> Value {
    Value::Array(v.iter().map(|a| json!([a[0], a[1], a[2]])).collect())
}

macro_rules! remote_common {
    ($proxy:ident) => {
        impl $proxy {
            /// Ask the participant for its time (`cosim.time`). [`Participant::time`] answers from the last reply.
            pub fn query_time(&self) -> Result<f64, String> {
                let reply = lock(&self.link).call("cosim.time", json!({}))?;
                reply["time"]
                    .as_f64()
                    .ok_or_else(|| "cosim.time returned no time".to_string())
            }

            /// Open a new connection and take a new lease, without restoring. For a caller whose coupled step failed
            /// before its iteration, where the coupler sends no `restore`.
            pub fn reconnect(&self) -> Result<(), String> {
                lock(&self.link).reconnect()
            }

            /// How many times the proxy reconnected.
            pub fn reconnects(&self) -> usize {
                lock(&self.link).reconnects
            }

            /// Whether the connection is in use (not lost).
            pub fn is_connected(&self) -> bool {
                lock(&self.link).stream.is_some()
            }

            /// The current lease.
            pub fn lease(&self) -> u64 {
                lock(&self.link).lease
            }
        }

        impl Participant for $proxy {
            fn name(&self) -> &str {
                &self.name
            }

            fn time(&self) -> f64 {
                lock(&self.link).time
            }

            fn checkpoint(&mut self) -> Result<(), String> {
                lock(&self.link).call("cosim.checkpoint", json!({})).map(|_| ())
            }

            fn restore(&mut self) -> Result<(), String> {
                lock(&self.link).restore()
            }
        }
    };
}

/// A structural participant hosted by a service elsewhere, driven over TCP (F-360). `name`, `time` and
/// `node_coordinates` answer from what the connection and the last reply gave.
pub struct RemoteStructure {
    link: Mutex<Link>,
    name: String,
    nodes: Vec<Vec3>,
}

impl std::fmt::Debug for RemoteStructure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteStructure")
            .field("name", &self.name)
            .field("nodes", &self.nodes.len())
            .finish_non_exhaustive()
    }
}

impl RemoteStructure {
    /// Connect to the service at `address` and open its structural participant `participant`.
    pub fn connect(address: SocketAddr, participant: &str, settings: RemoteSettings) -> Result<Self, String> {
        let mut link = Link::new(address, participant, settings);
        link.open()?;
        let d = link.call("cosim.describe", json!({}))?;
        if d["kind"] != "structure" {
            return Err(format!(
                "participant '{participant}' at {address} is not a structure (kind {})",
                d["kind"]
            ));
        }
        Ok(RemoteStructure {
            name: d["name"].as_str().unwrap_or(participant).to_string(),
            nodes: vectors_from(&d, "nodes")?,
            link: Mutex::new(link),
        })
    }
}

remote_common!(RemoteStructure);

impl Structure for RemoteStructure {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.nodes.clone()
    }

    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let reply = lock(&self.link).call("cosim.advance", json!({"dt": dt, "forces": vectors_value(forces)}))?;
        vectors_from(&reply, "displacements")
    }
}

/// A fluid participant hosted by a service elsewhere, driven over TCP (F-360). `name`, `time`, `point_coordinates` and
/// `face_points` answer from what the connection and the last reply gave.
pub struct RemoteFluid {
    link: Mutex<Link>,
    name: String,
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
}

impl std::fmt::Debug for RemoteFluid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteFluid")
            .field("name", &self.name)
            .field("points", &self.points.len())
            .field("faces", &self.faces.len())
            .finish_non_exhaustive()
    }
}

impl RemoteFluid {
    /// Connect to the service at `address` and open its fluid participant `participant`.
    pub fn connect(address: SocketAddr, participant: &str, settings: RemoteSettings) -> Result<Self, String> {
        let mut link = Link::new(address, participant, settings);
        link.open()?;
        let d = link.call("cosim.describe", json!({}))?;
        if d["kind"] != "fluid" {
            return Err(format!(
                "participant '{participant}' at {address} is not a fluid (kind {})",
                d["kind"]
            ));
        }
        let faces = d["faces"]
            .as_array()
            .ok_or("cosim.describe returned no faces")?
            .iter()
            .map(|f| {
                f.as_array()
                    .and_then(|a| a.iter().map(|i| i.as_u64().map(|i| i as usize)).collect())
                    .ok_or_else(|| "cosim.describe returned a face that is not a list of point indices".to_string())
            })
            .collect::<Result<Vec<Vec<usize>>, String>>()?;
        Ok(RemoteFluid {
            name: d["name"].as_str().unwrap_or(participant).to_string(),
            points: vectors_from(&d, "points")?,
            faces,
            link: Mutex::new(link),
        })
    }
}

remote_common!(RemoteFluid);

impl Fluid for RemoteFluid {
    fn point_coordinates(&self) -> Vec<Vec3> {
        self.points.clone()
    }

    fn face_points(&self) -> Vec<Vec<usize>> {
        self.faces.clone()
    }

    fn forces(&self) -> Result<Vec<Vec3>, String> {
        let reply = lock(&self.link).call("cosim.forces", json!({}))?;
        vectors_from(&reply, "forces")
    }

    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let reply = lock(&self.link).call(
            "cosim.advance",
            json!({"dt": dt, "displacements": vectors_value(displacements)}),
        )?;
        vectors_from(&reply, "forces")
    }
}
