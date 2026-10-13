//! End-to-end test of the solver service over TCP (F-085).

use rcfd_api::service::{code, Client, Service, ServiceConfig};
use serde_json::json;

const CASE: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
"#;

#[test]
fn start_run_receive_residual_events_fetch_slice_and_stop() {
    let svc = Service::new(ServiceConfig::default());
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    let mut c = Client::connect(server.addr).unwrap();
    let info = c.call("service.info", json!({})).unwrap();
    assert_eq!(info["protocol"], "rcfd-service/1");
    let id = c.call("session.create", json!({"toml": CASE})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    c.call("run.start", json!({"session": id})).unwrap();

    // long-poll until at least 5 iteration events arrived
    let mut after = 0;
    let mut iters = Vec::new();
    while iters.len() < 5 {
        let r = c
            .call(
                "events.poll",
                json!({"after": after, "timeout_ms": 5000, "session": id}),
            )
            .unwrap();
        after = r["next"].as_u64().unwrap();
        for e in r["events"].as_array().unwrap() {
            if e["kind"] == "iteration" {
                iters.push(e["data"]["res_u"].as_f64().unwrap());
            }
        }
    }
    assert!(iters.iter().all(|r| r.is_finite() && *r > 0.0));

    let slice = c
        .call("field.slice", json!({"session": id, "name": "ux", "max_points": 100}))
        .unwrap();
    let n = slice["values"].as_array().unwrap().len();
    assert!(n > 0 && n <= 100, "{n}");
    assert_eq!(slice["x"].as_array().unwrap().len(), n);

    let st = c.call("run.stop", json!({"session": id})).unwrap();
    assert_eq!(st["state"], "stopped");
    // a second client sees the same session
    let mut c2 = Client::connect(server.addr).unwrap();
    let list = c2.call("session.list", json!({})).unwrap();
    assert_eq!(list["sessions"][0]["state"], "stopped");
    assert_eq!(
        c2.call("run.status", json!({"session": "nope"})).unwrap_err().code,
        code::NO_SESSION
    );
    c.call("session.close", json!({"session": id})).unwrap();
    svc.shutdown();
    server.stop();
}

#[test]
fn malformed_input_gets_json_rpc_errors() {
    use std::io::{BufRead, BufReader, Write};
    let svc = Service::new(ServiceConfig::default());
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    let mut s = std::net::TcpStream::connect(server.addr).unwrap();
    writeln!(s, "{{not json").unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["error"]["code"], code::PARSE);
    server.stop();
}
