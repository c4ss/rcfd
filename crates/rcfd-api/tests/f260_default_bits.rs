//! ADR 0222 V0b (F-260): the method parsing and the written `method` of the design tools for the existing method types,
//! hashed bit for bit. FNV-1a (64 bit, as `f249_default_bits.rs`) over text as bytes. The values below were recorded on main
//! 539956da with the ADR 0222 registration and this file only, before any code of F-260, and must stay bit-identical after it.
//! Not covered (ADR 0222 D4 changes them): the unknown-type and missing-method messages, and any `sigma0` key.

use rcfd_api::design::tools::{call, parse_study};
use rcfd_api::service::{Service, ServiceConfig};
use serde_json::{json, Value};
use std::path::PathBuf;

/// The records, recorded on main 539956da.
const PARSED: u64 = 0x9f0f_f078_2251_2596;
const WRITTEN: u64 = 0x5a06_b394_eb85_c3a8;

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn s(&mut self, t: &str) {
        self.bytes(&(t.len() as u64).to_le_bytes());
        self.bytes(t.as_bytes());
    }
}

/// The lid-driven cavity template of `rcfd-workspace` (6 x 6 cells).
const CAVITY: &str = r#"title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 6
ny = 6
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
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

/// The method objects: valid ones first (13), then refused ones.
fn methods() -> Vec<(bool, Value)> {
    vec![
        (true, json!({"type": "nelder_mead"})),
        (true, json!({"type": "nelder_mead", "budget": 80, "restarts": 1, "seed": 3})),
        (
            true,
            json!({"type": "nelder_mead", "budget": 400, "restarts": 1, "seed": 1, "initial_simplex": "start", "coefficients": "adaptive"}),
        ),
        (true, json!({"type": "nelder_mead", "initial_simplex": "range"})),
        (true, json!({"type": "differential_evolution"})),
        (true, json!({"type": "differential_evolution", "budget": 300, "pop": 12, "seed": 2})),
        (true, json!({"type": "nsga2", "budget": 400, "pop": 20})),
        (true, json!({"type": "nsga2", "budget": 400, "pop": 20, "seed": 5, "crossover": "exchange"})),
        (true, json!({"type": "nsga2", "budget": 400, "pop": 20, "crossover": "sbx"})),
        (true, json!({"type": "factorial", "levels": [3, 2]})),
        (true, json!({"type": "lhs", "n": 8, "seed": 4})),
        (true, json!({"type": "sobol", "n": 8})),
        (true, json!({"type": "nelder_mead", "budget": 50, "extra": "ignored"})),
        (false, json!({"type": "nelder_mead", "initial_simplex": "simplex"})),
        (false, json!({"type": "nelder_mead", "coefficients": 3})),
        (false, json!({"type": "lhs", "n": 8, "initial_simplex": "start"})),
        (false, json!({"type": "nsga2", "budget": 400, "pop": 7})),
        (false, json!({"type": "nsga2", "budget": 10, "pop": 20})),
        (false, json!({"type": "nsga2", "pop": 20})),
        (false, json!({"type": "nsga2", "budget": 400})),
        (false, json!({"type": "nsga2", "budget": 400, "pop": 20, "crossover": "uniform"})),
        (false, json!({"type": "nelder_mead", "crossover": "exchange"})),
        (false, json!({"type": "nelder_mead", "seed": -1})),
        (false, json!({"type": "factorial"})),
        (false, json!({"type": "nelder_mead", "budget": "many"})),
        (false, json!({"type": "differential_evolution", "pop": "x"})),
        (false, json!({"budget": 3})),
    ]
}

fn study(dir: &str, method: &Value) -> Value {
    json!({
        "dir": dir, "base": "cavity.toml", "name": "v0b",
        "parameters": [
            {"name": "x", "lo": 0.0, "hi": 1.0, "targets": [{"path": "physics.nu", "scale": 0.01, "offset": 0.01}]},
            {"name": "y", "lo": 0.0, "hi": 1.0, "init": 0.25, "targets": [{"path": "physics.rho", "scale": 0.1, "offset": 1.0}]}
        ],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": method,
    })
}

#[test]
fn v0b_method_parsing_and_written_method() {
    let root: PathBuf = std::env::temp_dir().join(format!("rcfd_f260_v0b_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("cavity.toml"), CAVITY).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    let (mut parsed, mut written) = (Fnv::new(), Fnv::new());
    for (i, (valid, m)) in methods().into_iter().enumerate() {
        let v = study(&format!("s{i:02}"), &m);
        let text = match parse_study(&v, root.join("cavity.toml"), "fp".into()) {
            Ok((_, meta)) => format!("ok {:?}", meta.method),
            Err(e) => format!("err {} {}", e.code, e.message),
        };
        println!("V0b {i:02} {m}: {text}");
        assert_eq!(text.starts_with("ok"), valid, "{i}: {text}");
        parsed.s(&text);
        if valid {
            call(&svc, "design.create_study", &v).unwrap_or_else(|e| panic!("{i}: {}", e.message));
            let sj: Value =
                serde_json::from_str(&std::fs::read_to_string(root.join(format!("s{i:02}/study.json"))).unwrap()).unwrap();
            let w = serde_json::to_string(&sj["method"]).unwrap();
            println!("V0b {i:02} written: {w}");
            written.s(&w);
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    println!("V0b parsed {:#018x}, written {:#018x}", parsed.0, written.0);
    assert_eq!(parsed.0, PARSED, "V0b: the parsed methods changed");
    assert_eq!(written.0, WRITTEN, "V0b: the written methods changed");
}
