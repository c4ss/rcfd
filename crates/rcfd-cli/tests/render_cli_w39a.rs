//! Wave 39A (ADR 0092, F-073): the roadmap criterion "CLI renders a contour image in under 2 s for a 1M-cell mesh" through the
//! command line. The CLI's render path is `rcfd serve` plus `rcfd call render.png`; the timed interval is the wall time of the
//! one `rcfd call render.png` process (process start, connection, render, PNG encode, base64 and JSON transfer, print), with the
//! session created and initialised beforehand (mesh generation and solver set-up excluded, as in the library test r2).
//! Release builds only, as r2.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn call(addr: &str, method: &str, params: &serde_json::Value) -> (serde_json::Value, f64) {
    let t = Instant::now();
    let o = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["call", "--addr", addr, method, &params.to_string()])
        .output()
        .unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{method}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    (serde_json::from_slice(&o.stdout).unwrap(), secs)
}

/// 1000 x 1000 cavity whose initial speed |sin(6 x) y| varies over the domain (the r2 field).
const CASE: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 1000
ny = 1000
[physics]
nu = 0.01
[initial]
velocity_expression_x = "sin(6 * x / 1 [m]) * y / 1 [s]"
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
"#;

#[test]
#[cfg_attr(debug_assertions, ignore = "timing criterion is for release builds")]
fn r4_cli_renders_a_one_million_cell_contour_image_in_under_two_seconds() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["serve", "--addr", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let _srv = Server(child);
    let mut line = String::new();
    BufReader::new(out).read_line(&mut line).unwrap();
    let addr = line.trim().rsplit(' ').next().unwrap().to_string();
    let (s, t_create) = call(&addr, "session.create", &serde_json::json!({ "toml": CASE }));
    let id = s["session"].as_str().unwrap().to_string();
    let (_, t_init) = call(&addr, "run.initialize", &serde_json::json!({ "session": id }));
    let (img, t_render) = call(
        &addr,
        "render.png",
        &serde_json::json!({ "session": id, "field": "speed", "width": 1024 }),
    );
    let png = img["data_base64"].as_str().unwrap();
    eprintln!(
        "r4: session.create {t_create:.3} s, run.initialize {t_init:.3} s (not limited); rcfd call render.png {t_render:.3} s, image {} x {}, speed {} .. {}, base64 {} bytes",
        img["width"], img["height"], img["min"], img["max"], png.len()
    );
    assert_eq!(img["mime"], "image/png");
    assert!(png.starts_with("iVBORw0KGgo"));
    assert_eq!(img["width"], 1024);
    assert!(
        img["max"].as_f64().unwrap() > img["min"].as_f64().unwrap() + 0.5,
        "the field has no variation"
    );
    assert!(t_render < 2.0, "rcfd call render.png took {t_render:.3} s");
}
