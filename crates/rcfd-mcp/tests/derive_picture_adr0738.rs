//! ADR 0738 (F-809) P4: the project tool `derive_picture` on a stored run (the cavity of the service's run path, 2D,
//! with `ux`, `uy` and `p`).
//!
//! - every kind writes `runs/<id>/derived/<kind>-<hash>.glb` that passes `validate_glb`, says `length_unit: "m"`, and
//!   is the file `rcfd_workspace::derive::derive_dir` makes of the store (the library of `rcfd post derive`, G16);
//! - the same arguments reuse the file (`reused`), other arguments make another;
//! - the errors: 1202 no such field or snapshot, 1003 a level outside the range (with the range), 1004 over the limits,
//!   -32602 a bad argument; the server's `--read-only` policy still answers it (class `read`).

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use rcfd_post::picture::{read_primitives, Attribute};
use serde_json::{json, Value};

#[test]
fn p4_derive_picture_writes_the_engines_glb() {
    let dir = project("adr0738-p4");
    let (_svc, _sess, store) = service_store(&dir.parent().unwrap().join("svc"), 12, 30);
    let id = run_with_store(&dir, &store);
    let s = server(&dir, Policy::read_only());
    let mesh = rcfd_tsstore::Reader::open(&store).unwrap().mesh().unwrap();
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let at = |k: usize, f: f64| lo[k] + f * (hi[k] - lo[k]);
    let kinds = [
        json!({"kind": "iso", "field": "p", "level": 0.0}),
        json!({"kind": "iso", "field": "speed", "level": 0.3, "colour_by": "p"}),
        json!({"kind": "streamlines", "seeds": {"rake": {"p0": [at(0, 0.2), at(1, 0.5), at(2, 0.5)], "p1": [at(0, 0.8), at(1, 0.5), at(2, 0.5)], "n": 7}}, "direction": "both"}),
        json!({"kind": "slice", "field": "ux", "axis": "x", "at": at(0, 0.5), "vectors": true}),
        json!({"kind": "cut_surface", "field": "p", "vectors": true}),
        json!({"kind": "cut_surface", "field": "p", "axis": "y", "at": at(1, 0.4), "keep": "above", "vectors": true}),
    ];
    let mut lines = Vec::new();
    for k in &kinds {
        let mut a = k.clone();
        a["run"] = json!(id);
        // p ranges about 0: take the middle of its range for the iso level
        if k["kind"] == "iso" && k["field"] == "p" {
            let fs = ok(&call(&s, "field_summary", json!({"run": id, "field": "p"})));
            a["level"] = json!(0.5 * (fs["min"].as_f64().unwrap() + fs["max"].as_f64().unwrap()));
        }
        let r = ok(&call(&s, "derive_picture", a.clone()));
        assert_eq!(r["length_unit"], "m", "{r}");
        assert_eq!(r["reused"], false);
        let rel = r["path"].as_str().unwrap();
        assert!(
            rel.starts_with(&format!("runs/{id}/derived/{}-", k["kind"].as_str().unwrap())) && rel.ends_with(".glb")
        );
        let bytes = std::fs::read(dir.join(rel)).unwrap();
        assert_eq!(r["bytes"], bytes.len());
        let (doc, prims) = read_primitives(&bytes).unwrap();
        assert!(!prims.is_empty(), "{}: a picture", k["kind"]);
        assert_eq!(doc["asset"]["extras"]["kind"], k["kind"]);
        for p in &prims {
            assert!(p.attributes.contains_key("_CELL"));
            let Attribute::F32(pos, 3) = &p.attributes["POSITION"] else {
                panic!("POSITION")
            };
            assert!(pos.iter().all(|x| x.is_finite()));
        }
        // the CLI's library makes the same file of the store
        let mut b = a.clone();
        b.as_object_mut().unwrap().remove("run");
        let d = rcfd_workspace::derive::derive_dir(&dir.join(format!("runs/{id}/fields")), &b).unwrap();
        assert!(d.glb == bytes, "{}: derive_dir differs from derive_picture", k["kind"]);
        // reused
        let again = ok(&call(&s, "derive_picture", a.clone()));
        assert_eq!(
            (again["reused"].clone(), again["path"].clone()),
            (json!(true), json!(rel))
        );
        lines.push(format!("{} {} bytes", k["kind"], bytes.len()));
        if k["kind"] == "cut_surface" && k.get("axis").is_some() {
            let cut = &prims[0];
            let Attribute::F32(pos, 3) = &cut.attributes["POSITION"] else {
                panic!()
            };
            let y = k["at"].as_f64().unwrap() as f32;
            assert!(pos.chunks_exact(3).all(|p| p[1] >= y), "kept above the plane");
            assert_eq!(cut.mode, 4);
            assert!(prims.iter().any(|p| p.mode == 1), "the arrows");
        }
    }
    // the errors
    // a refusal by the argument check is a JSON-RPC error, one of the tool a result with isError
    let e = |a: Value| {
        let r = rpc_call(&s, "derive_picture", a);
        r["error"]["code"].as_i64().unwrap_or_else(|| err_code(&r["result"]))
    };
    let base = |k: Value| {
        let mut k = k;
        k["run"] = json!(id);
        k
    };
    assert_eq!(e(base(json!({"kind": "iso", "field": "nope", "level": 0.0}))), 1202);
    assert_eq!(
        e(base(
            json!({"kind": "iso", "field": "p", "level": 0.0, "snapshot": 999})
        )),
        1202
    );
    let r = call(
        &s,
        "derive_picture",
        base(json!({"kind": "iso", "field": "p", "level": 1e9})),
    );
    assert_eq!(err_code(&r), 1003);
    assert!(r["structuredContent"]["error"]["data"]["range"].is_array(), "{r}");
    assert_eq!(
        e(base(
            json!({"kind": "streamlines", "seeds": {"rake": {"p0": [0.0, 0.0, 0.0], "p1": [0.1, 0.1, 0.0], "n": 1001}}})
        )),
        1004
    );
    assert_eq!(
        e(base(
            json!({"kind": "streamlines", "seeds": {"points": [[at(0, 0.5), at(1, 0.5), at(2, 0.5)]]}, "max_steps": 10001})
        )),
        -32602,
        "the schema's maximum"
    );
    assert_eq!(e(base(json!({"kind": "slice", "field": "p", "axis": "x"}))), -32602);
    assert_eq!(e(base(json!({"kind": "cut_surface", "axis": "q", "at": 0.0}))), -32602);
    assert_eq!(e(base(json!({"kind": "volume"}))), -32602);
    assert_eq!(e(json!({"run": "r-20260101T000000-0000", "kind": "iso"})), 1202);
    eprintln!(
        "P4: {}; errors 1202, 1003 (with the range), 1004, -32602",
        lines.join(", ")
    );
}

/// The limit of 1000 seeds x 10 000 steps is the workspace's too (the CLI has no schema in front of it).
#[test]
fn q7_the_limits_are_the_libraries() {
    let dir = project("adr0738-q7");
    let (_svc, _sess, store) = service_store(&dir.parent().unwrap().join("svc"), 8, 10);
    let a = json!({"kind": "streamlines", "seeds": {"points": [[0.05, 0.05, 0.005]]}, "max_steps": 10001});
    let e = rcfd_workspace::derive::derive_dir(&store, &a).unwrap_err();
    assert_eq!(e.code, 1004);
    let pts: Vec<Value> = (0..1001).map(|i| json!([0.0001 * i as f64, 0.05, 0.005])).collect();
    let e = rcfd_workspace::derive::derive_dir(&store, &json!({"kind": "streamlines", "seeds": {"points": pts}}))
        .unwrap_err();
    assert_eq!(e.code, 1004);
    assert_eq!(e.data.as_ref().unwrap()["limit"], 1000);
    assert_eq!(rcfd_workspace::derive::MAX_TRIANGLES, 2_000_000);
}
