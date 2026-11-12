//! ADR 0350 amendment 1 (F-447): the opt-in `data.*` methods (`rcfd-data/1`, service protocol 1.2).
//!
//! D1: a service that does not switch them on answers as protocol 1.1 does. D2: a service that does reports 1.2. D3: a
//! field over the data connection equals `Reader::read_field` cast to `f32`, bit for bit, for raw and deflated stores in
//! both precisions (negative control: the next snapshot differs). D4: the surface of a box. D5: tickets. D6: the sandbox.

use rcfd_api::data_service::{redeem, DATA_METHODS, DATA_PROTOCOL, MAX_TICKETS};
use rcfd_api::service::{code, Client, Service, ServiceConfig, METHODS, PROTOCOL};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_tsstore::{Codec, Location, Precision, Reader, WriteOptions, Writer};
use serde_json::{json, Value};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const N: [usize; 3] = [6, 5, 4];
const L: [f64; 3] = [1.0, 2.0, 3.0];
const SNAPS: usize = 3;

fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir()
        .join("rcfd-api-tests")
        .join(format!("f447-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

fn value(c: usize, k: usize, f: usize) -> f64 {
    (0.37 * c as f64 + k as f64 + 0.5 * f as f64).sin() * 1.0e3 + 0.1 * k as f64
}

/// A store of the 6 x 5 x 4 box in `dir` with a block of 50 values (so 120 cells take three blocks, the last one short).
fn write_store(dir: &Path, codec: Codec, precision: Precision) {
    let mesh = box_mesh(N, L, [Grading::Uniform; 3]).unwrap();
    let opts = WriteOptions {
        block_values: 50,
        codec,
    };
    let mut w = Writer::create(dir, &mesh, opts).unwrap();
    for k in 0..SNAPS {
        let mut s = w.snapshot(k as u64 + 1, 0.1 * k as f64).unwrap();
        for (f, name) in ["p", "ux"].iter().enumerate() {
            let v: Vec<f64> = (0..mesh.n_cells()).map(|c| value(c, k, f)).collect();
            s.field(name, "unknown", Location::Cell, &v, precision).unwrap();
        }
        s.commit().unwrap();
    }
    w.finish().unwrap();
}

fn service(root: &Path) -> (Arc<Service>, TcpStream) {
    let svc = Service::new(ServiceConfig {
        root: Some(root.to_path_buf()),
        ..Default::default()
    });
    let addr = svc.enable_data("127.0.0.1:0").unwrap();
    assert_eq!(
        svc.enable_data("127.0.0.1:0").unwrap(),
        addr,
        "a second call keeps the first listener"
    );
    let stream = TcpStream::connect(addr).unwrap();
    stream.set_nodelay(true).unwrap();
    (svc, stream)
}

fn reference(dir: &Path, k: usize, name: &str) -> Vec<u8> {
    let mut r = Reader::open(dir).unwrap();
    r.read_field(k, name)
        .unwrap()
        .iter()
        .flat_map(|x| (*x as f32).to_le_bytes())
        .collect()
}

fn fetch(svc: &Arc<Service>, stream: &mut TcpStream, method: &str, p: Value) -> (Value, Vec<Vec<u8>>) {
    let r = svc.call(method, &p).unwrap();
    let n = r["sections"].as_array().unwrap().len();
    let bytes = redeem(stream, r["ticket"].as_u64().unwrap(), n).unwrap();
    (r, bytes)
}

/// D1: without `enable_data` the reply of `service.info` has the members and values of protocol 1.1 and `data.*` is unknown.
#[test]
fn d1_a_default_service_is_protocol_1_1() {
    let svc = Service::new(ServiceConfig::default());
    let info = svc.call("service.info", &json!({})).unwrap();
    assert_eq!(info["protocol"], PROTOCOL);
    assert_eq!(info["protocol_version"], "1.1");
    assert!(info.get("data").is_none());
    let keys: Vec<&str> = info.as_object().unwrap().keys().map(String::as_str).collect();
    let mut expected = vec![
        "cosim",
        "limits",
        "methods",
        "name",
        "protocol",
        "protocol_version",
        "sandbox_root",
        "version",
    ];
    expected.sort_unstable();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, expected);
    assert_eq!(info["methods"], json!(METHODS));
    assert!(METHODS.iter().all(|m| !m.starts_with("data.")));
    for m in DATA_METHODS {
        let e = svc.call(m, &json!({"path": "."})).unwrap_err();
        assert_eq!(e.code, code::METHOD_NOT_FOUND, "{m}");
        assert_eq!(e.message, format!("unknown method '{m}'"));
    }
}

/// D2: with `enable_data` the identifier stays `rcfd-service/1`, the version is 1.2 and the methods follow the existing ones.
#[test]
fn d2_an_enabled_service_reports_protocol_1_2() {
    let root = temp("d2");
    let (svc, _stream) = service(&root);
    let info = svc.call("service.info", &json!({})).unwrap();
    assert_eq!(info["protocol"], PROTOCOL);
    assert_eq!(info["protocol"], "rcfd-service/1");
    assert_eq!(info["protocol_version"], "1.2");
    assert_eq!(info["data"], DATA_PROTOCOL);
    let methods: Vec<&str> = info["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    assert_eq!(&methods[..METHODS.len()], METHODS);
    assert_eq!(&methods[METHODS.len()..], &DATA_METHODS);
    let d = svc.call("data.info", &json!({})).unwrap();
    assert_eq!(d["protocol"], DATA_PROTOCOL);
    assert!(d["port"].as_u64().unwrap() > 0 && d["unix_ns"].as_u64().unwrap() > 1_700_000_000_000_000_000);
    assert_eq!(
        svc.call("data.nothing", &json!({})).unwrap_err().code,
        code::METHOD_NOT_FOUND
    );
}

/// D3: every field of every snapshot equals the reader's values as `f32` bit for bit, over the data connection and with the
/// JSON calls over TCP as well; the next snapshot's bytes differ (negative control).
#[test]
fn d3_fields_cross_bit_for_bit() {
    let root = temp("d3");
    let (svc, mut stream) = service(&root);
    let tcp = svc.serve_tcp("127.0.0.1:0").unwrap();
    let mut client = Client::connect(tcp.addr).unwrap();
    let mut compared = 0;
    for (i, (codec, precision)) in [
        (Codec::Raw, Precision::F64),
        (Codec::Raw, Precision::F32),
        (Codec::ShuffleDeflate, Precision::F64),
        (Codec::ShuffleDeflate, Precision::F32),
    ]
    .into_iter()
    .enumerate()
    {
        let rel = format!("store{i}");
        let dir = root.join(&rel);
        write_store(&dir, codec, precision);
        let open = svc.call("data.open", &json!({"path": rel})).unwrap();
        assert_eq!(open["n_cells"], 120);
        assert_eq!(open["snapshots"].as_array().unwrap().len(), SNAPS);
        assert_eq!(open["snapshots"][2]["step"], 3);
        assert_eq!(open["fields"][1]["name"], "ux");
        for k in 0..SNAPS {
            for name in ["p", "ux"] {
                let want = reference(&dir, k, name);
                let (r, bytes) = fetch(
                    &svc,
                    &mut stream,
                    "data.field",
                    json!({"path": rel, "snapshot": k, "field": name}),
                );
                assert_eq!(
                    r["sections"][0],
                    json!({"name": "values", "dtype": "f32", "count": 120, "bytes": 480})
                );
                assert_eq!(bytes[0], want, "{codec:?} {precision:?} snapshot {k} field {name}");
                let r = client
                    .call("data.field", json!({"path": rel, "snapshot": k, "field": name}))
                    .unwrap();
                let over_tcp = redeem(&mut stream, r["ticket"].as_u64().unwrap(), 1).unwrap();
                assert_eq!(over_tcp[0], want);
                if k + 1 < SNAPS {
                    assert_ne!(
                        bytes[0],
                        reference(&dir, k + 1, name),
                        "control: the next snapshot differs"
                    );
                }
                compared += 1;
            }
        }
        let e = svc
            .call("data.field", &json!({"path": rel, "snapshot": SNAPS, "field": "p"}))
            .unwrap_err();
        assert_eq!(e.code, code::INVALID_PARAMS);
        let e = svc
            .call("data.field", &json!({"path": rel, "snapshot": 0, "field": "T"}))
            .unwrap_err();
        assert_eq!(e.code, code::INVALID_PARAMS);
    }
    assert_eq!(compared, 4 * SNAPS * 2);
    tcp.stop();
}

/// D4: the surface of the 6 x 5 x 4 box: two triangles per boundary quadrilateral, the corner points once each, owner cells
/// in range, and the triangles' area equal to the box's surface.
#[test]
fn d4_surface_of_the_box() {
    let root = temp("d4");
    let (svc, mut stream) = service(&root);
    write_store(&root.join("s"), Codec::Raw, Precision::F32);
    let (r, bytes) = fetch(&svc, &mut stream, "data.surface", json!({"path": "s"}));
    let quads = 2 * (N[0] * N[1] + N[1] * N[2] + N[0] * N[2]);
    let inner = (N[0] - 1) * (N[1] - 1) * (N[2] - 1);
    let n_points = (N[0] + 1) * (N[1] + 1) * (N[2] + 1) - inner;
    assert_eq!(
        r["sections"][0],
        json!({"name": "points", "dtype": "f32", "count": 3 * n_points, "bytes": 12 * n_points})
    );
    assert_eq!(r["sections"][1]["count"], 6 * quads);
    assert_eq!(r["sections"][2]["count"], 2 * quads);
    let f32s = |b: &[u8]| -> Vec<f32> {
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let u32s = |b: &[u8]| -> Vec<u32> {
        b.chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let (points, tris, cells) = (f32s(&bytes[0]), u32s(&bytes[1]), u32s(&bytes[2]));
    assert_eq!(
        (points.len(), tris.len(), cells.len()),
        (3 * n_points, 6 * quads, 2 * quads)
    );
    assert!(cells.iter().all(|&c| (c as usize) < 120));
    assert!(tris.iter().all(|&p| (p as usize) < n_points));
    let at = |p: u32| -> [f64; 3] {
        let i = 3 * p as usize;
        [f64::from(points[i]), f64::from(points[i + 1]), f64::from(points[i + 2])]
    };
    let mut area = 0.0;
    for t in tris.chunks_exact(3) {
        let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    }
    let exact = 2.0 * (L[0] * L[1] + L[1] * L[2] + L[0] * L[2]);
    eprintln!("D4: {quads} quadrilaterals, {n_points} points, area {area} (exact {exact})");
    assert!((area - exact).abs() <= 1e-5 * exact, "area {area} vs {exact}");
}

/// D5: a ticket is used once; an unknown ticket ends the connection; only the newest `MAX_TICKETS` wait.
#[test]
fn d5_tickets() {
    let root = temp("d5");
    let (svc, mut stream) = service(&root);
    write_store(&root.join("s"), Codec::Raw, Precision::F32);
    let ask = || {
        svc.call("data.field", &json!({"path": "s", "snapshot": 0, "field": "p"}))
            .unwrap()["ticket"]
            .as_u64()
            .unwrap()
    };
    let t = ask();
    assert_eq!(redeem(&mut stream, t, 1).unwrap()[0].len(), 480);
    let e = redeem(&mut stream, t, 1).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound, "a used ticket is unknown");
    assert!(redeem(&mut stream, t, 1).is_err(), "the connection was closed");
    let addr = svc.enable_data("127.0.0.1:0").unwrap();
    let tickets: Vec<u64> = (0..=MAX_TICKETS).map(|_| ask()).collect();
    let mut s2 = TcpStream::connect(addr).unwrap();
    for t in &tickets[1..] {
        assert_eq!(redeem(&mut s2, *t, 1).unwrap()[0].len(), 480);
    }
    assert!(redeem(&mut s2, tickets[0], 1).is_err(), "the oldest ticket was dropped");
}

/// D6: a path outside the sandbox root is refused with 1004; a folder without a store with -32602.
#[test]
fn d6_sandbox() {
    let root = temp("d6");
    let (svc, _stream) = service(&root);
    for m in ["data.open", "data.field", "data.surface"] {
        let e = svc
            .call(m, &json!({"path": "../elsewhere", "snapshot": 0, "field": "p"}))
            .unwrap_err();
        assert_eq!(e.code, code::LIMIT, "{m}");
        let e = svc
            .call(m, &json!({"path": "none", "snapshot": 0, "field": "p"}))
            .unwrap_err();
        assert_eq!(e.code, code::INVALID_PARAMS, "{m}");
        let e = svc.call(m, &json!({})).unwrap_err();
        assert_eq!(e.code, code::INVALID_PARAMS, "{m}");
    }
}
