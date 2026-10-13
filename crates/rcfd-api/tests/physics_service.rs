//! The turbulence and compressible example cases through the solver service (settings tree, run
//! control, field slices and reports), end to end on small meshes.

use rcfd_api::service::{Client, Service, ServiceConfig};
use serde_json::{json, Value};

fn example(rel: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn run_to_end(c: &mut Client, toml: &str) -> (String, Value) {
    let id = c.call("session.create", json!({"toml": toml})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    c.call("run.start", json!({"session": id})).unwrap();
    let mut st = json!({});
    for _ in 0..120 {
        st = c.call("run.wait", json!({"session": id, "timeout_ms": 5000})).unwrap();
        if st["state"] != "running" {
            break;
        }
    }
    (id, st)
}

fn slice(c: &mut Client, id: &str, name: &str) -> Vec<f64> {
    let s = c
        .call(
            "field.slice",
            json!({"session": id, "name": name, "max_points": 100000}),
        )
        .unwrap();
    s["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

#[test]
fn compressible_and_turbulence_examples_run_through_the_service() {
    let svc = Service::new(ServiceConfig::default());
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    let mut c = Client::connect(server.addr).unwrap();

    // Sod shock tube: ends at t = 0.2 with the exact post-contact plateau density 0.4263 and
    // the post-shock plateau 0.2656 (Sod 1978); sample the latter
    let (id, st) = run_to_end(&mut c, &example("sod/sod.toml"));
    assert_eq!(st["state"], "finished", "{st}");
    let rho = slice(&mut c, &id, "rho");
    assert_eq!(rho.len(), 100);
    assert!(rho.iter().all(|r| *r > 0.1 && *r < 1.0 + 1e-9));
    println!("Sod rho[60] = {}, rho[78] = {}", rho[60], rho[78]);
    assert!((rho[60] - 0.4263).abs() < 0.02, "plateau density {}", rho[60]);
    assert!((rho[78] - 0.2656).abs() < 0.02, "plateau density {}", rho[78]);
    let mach = slice(&mut c, &id, "mach");
    assert!(mach.iter().cloned().fold(0.0, f64::max) > 0.8);
    let reports = c.call("reports.get", json!({"session": id})).unwrap();
    assert!(reports.to_string().contains("rho_max"), "{reports}");

    // wedge: the oblique shock raises the pressure above the free stream value 1/1.4
    let (id, st) = run_to_end(&mut c, &example("wedge/wedge.toml"));
    assert_eq!(st["state"], "finished", "{st}");
    let p = slice(&mut c, &id, "p");
    let pmax = p.iter().cloned().fold(0.0, f64::max);
    println!("wedge pmax = {pmax}");
    assert!(pmax > 0.7142857142857143 * 1.3, "pmax {pmax}");

    // k-epsilon channel with the inlet helpers: fields k, epsilon, nut and y_plus are served
    let (id, st) = run_to_end(&mut c, &example("channel/kepsilon_inlet.toml"));
    println!("inlet channel state {}", st["state"]);
    assert!(st["state"] != "running", "{st}");
    for f in ["k", "epsilon", "nut"] {
        let v = slice(&mut c, &id, f);
        assert!(v.iter().all(|x| x.is_finite() && *x >= 0.0), "{f}");
        assert!(v.iter().any(|x| *x > 0.0), "{f}");
    }
    let yp = slice(&mut c, &id, "y_plus");
    assert!(yp.iter().any(|x| *x > 0.0));
    let status = c.call("run.status", json!({"session": id})).unwrap();
    assert!(status.to_string().contains("k_epsilon"), "{status}");
}
