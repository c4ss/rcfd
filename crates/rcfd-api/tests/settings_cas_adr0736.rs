//! ADR 0736 P2 (F-829 E-829b): compare-and-set for `settings.set` and `settings.remove`. T6: with a matching
//! `base_value` or `base_hash` the call equals the call without one (reply, journal, events); with a stale one it is 1007
//! with the current value and hash, and the journal, the settings and the events are unchanged. A call without a base is
//! the call as before (the rest of the service tests and `baseline_hashes.rs` run unedited).

use rcfd_api::service::{code, Service, ServiceConfig};
use serde_json::{json, Value};
use std::sync::Arc;

const CAVITY: &str = r#"title = "cas cavity"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 5
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

fn svc() -> Arc<Service> {
    Service::new(ServiceConfig::default())
}

fn session(s: &Arc<Service>) -> String {
    s.call("session.create", &json!({"toml": CAVITY})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string()
}

fn journal(s: &Arc<Service>, id: &str) -> Value {
    s.call("journal.get", &json!({"session": id})).unwrap()
}

/// The session's events after `after`, without the session id, and the next cursor.
fn events(s: &Arc<Service>, id: &str, after: u64) -> (Vec<Value>, u64) {
    let r = s
        .call("events.poll", &json!({"after": after, "session": id, "limit": 5000}))
        .unwrap();
    let list = r["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut e = e.clone();
            for k in ["session", "seq", "t", "time", "id"] {
                if let Some(o) = e.as_object_mut() {
                    o.remove(k);
                }
            }
            e
        })
        .collect();
    (list, r["next"].as_u64().unwrap_or(after))
}

/// A matching base, value or hash or both, gives what the call without one gives: the reply, the journal entries and
/// hash, and the events.
#[test]
fn t6_matching_base_equals_no_base() {
    let s = svc();
    let plain = session(&s);
    let (_, p0) = events(&s, &plain, 0);
    let r_plain = s
        .call(
            "settings.set",
            &json!({"session": plain, "path": "physics.nu", "value": 0.02}),
        )
        .unwrap();
    let (ev_plain, _) = events(&s, &plain, p0);
    let j_plain = journal(&s, &plain);
    assert!(!ev_plain.is_empty(), "setting_changed is emitted");
    for (n, base) in ["value", "hash", "both", "value as an integer"].iter().enumerate() {
        let b = session(&s);
        let (_, b0) = events(&s, &b, 0);
        let h = journal(&s, &b)["case_hash"].clone();
        let mut p = json!({"session": b, "path": "physics.nu", "value": 0.02});
        match n {
            0 => p["base_value"] = json!(0.01),
            1 => p["base_hash"] = h,
            2 => {
                p["base_value"] = json!(0.01);
                p["base_hash"] = h;
            }
            _ => {
                // numbers compare as numbers: 8 and 8.0 are one value
                p["path"] = json!("mesh.nx");
                p["value"] = json!(10);
                p["base_value"] = json!(8.0);
            }
        }
        let r = s.call("settings.set", &p).unwrap();
        if n == 3 {
            assert_eq!(r["value"], json!(10), "{base}: {r}");
            continue;
        }
        assert_eq!(r, r_plain, "{base}");
        let (ev, _) = events(&s, &b, b0);
        assert_eq!(ev, ev_plain, "{base}");
        let j = journal(&s, &b);
        assert_eq!(j["entries"], j_plain["entries"], "{base}");
        assert_eq!(j["case_hash"], j_plain["case_hash"], "{base}");
    }
    eprintln!(
        "T6 match: reply {r_plain}, {} events, journal equal for value, hash and both",
        ev_plain.len()
    );
}

/// A stale `base_value` or `base_hash` is 1007 with the current value and hash; nothing is set, journalled or emitted.
#[test]
fn t6_stale_base_is_1007_and_changes_nothing() {
    let s = svc();
    let id = session(&s);
    let h0 = journal(&s, &id)["case_hash"].clone();
    // another client sets the value
    s.call(
        "settings.set",
        &json!({"session": id, "path": "physics.nu", "value": 0.03}),
    )
    .unwrap();
    let before = journal(&s, &id);
    let (_, e0) = events(&s, &id, 0);
    for p in [
        json!({"session": id, "path": "physics.nu", "value": 0.02, "base_value": 0.01}),
        json!({"session": id, "path": "physics.nu", "value": 0.02, "base_hash": h0}),
        json!({"session": id, "path": "physics.nu", "value": 0.02, "base_value": 0.03, "base_hash": h0}),
    ] {
        let e = s.call("settings.set", &p).unwrap_err();
        assert_eq!(e.code, code::CONFLICT, "{p}");
        assert_eq!(e.code, 1007);
        let d = e.data.unwrap();
        assert_eq!(d["path"], "physics.nu");
        assert_eq!(d["current_value"], json!(0.03));
        assert_eq!(d["current_hash"], before["case_hash"]);
        assert_ne!(d["current_hash"], h0);
        assert_eq!(journal(&s, &id), before, "{p}");
        let v = s
            .call("settings.get", &json!({"session": id, "path": "physics.nu"}))
            .unwrap();
        assert_eq!(v["value"], json!(0.03));
        assert_eq!(events(&s, &id, e0).0, Vec::<Value>::new(), "{p}");
        eprintln!("T6 stale: {}", e.message);
    }
    // a base_hash that is not a string: -32602, nothing changed
    let e = s
        .call(
            "settings.set",
            &json!({"session": id, "path": "physics.nu", "value": 0.02, "base_hash": 7}),
        )
        .unwrap_err();
    assert_eq!(e.code, code::INVALID_PARAMS);
    assert_eq!(journal(&s, &id), before);
}

/// `settings.remove` takes the same base: the removed entry's value as last read (an object), then a second remove with
/// that base finds it absent (null) and is 1007.
#[test]
fn t6_remove_with_a_base() {
    let s = svc();
    let id = session(&s);
    let v = s
        .call("settings.get", &json!({"session": id, "path": "reports.lid_drag"}))
        .unwrap()["value"]
        .clone();
    assert!(v.is_object(), "{v}");
    let r = s
        .call(
            "settings.remove",
            &json!({"session": id, "path": "reports.lid_drag", "base_value": v}),
        )
        .unwrap();
    assert_eq!(r, json!({"removed": "reports.lid_drag"}));
    let before = journal(&s, &id);
    let e = s
        .call(
            "settings.remove",
            &json!({"session": id, "path": "reports.lid_drag", "base_value": v}),
        )
        .unwrap_err();
    assert_eq!(e.code, 1007);
    assert_eq!(e.data.unwrap()["current_value"], Value::Null);
    assert_eq!(journal(&s, &id), before);
    // the call without a base answers as before: the collection has no such entry now
    let plain = s
        .call("settings.remove", &json!({"session": id, "path": "reports.lid_drag"}))
        .unwrap_err();
    assert_ne!(plain.code, 1007);
}
