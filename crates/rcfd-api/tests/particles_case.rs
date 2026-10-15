//! Particle tracking through the settings tree (F-056, ADR 0028 check PT14). The limits are those registered in the ADR
//! before this file was written; none was changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::particles;
use rcfd_api::sim::Simulation;
use rcfd_particles::State;

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => c.semantic_errors(),
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

/// Frozen Couette flow u = (20 y, 0, 0) on a 12 x 10 box_2d [0,4] x [0,1]; Stokes particle with tau_p = 0.05 s released at
/// rest at (0.5, 0.37).
fn couette_text(extra: &str) -> String {
    format!(
        r#"
title = "particle couette"
[mesh]
generator = "box_2d"
nx = 12
ny = 10
lx = 4.0
ly = 1.0
[physics]
nu = 1e-3
rho = 1.0
[energy]
enabled = true
[initial]
temperature = 300.0
velocity_expression_x = "20 [1/s] * y"
[boundaries.xmin]
velocity = "fixed"
velocity_value = [10.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "fixed"
velocity_value = [10.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [20.0, 0.0, 0.0]
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "stokes"
integration = "analytic"
interpolation = "gradient"
gravity = false
end_time = 0.4
dt = 0.01
[particles.injection.p]
kind = "point"
position = [0.5, 0.37, 0.05]
rate = 1000.0
end_time = 0.001
diameter = 9.486832980505138e-4
{extra}
"#
    )
}

#[test]
fn pt14_frozen_couette_matches_the_closed_form() {
    let c = case(&couette_text(""));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let sim = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = particles::track_frozen(&c, &sim).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(tr.particles.len(), 1);
    let p = &tr.particles[0];
    // injected at t = 0.001 (rate 1000/s, floor), tracked to 0.4
    let t = 0.4 - 0.001;
    let tp: f64 = 0.05;
    let gamma_y = 20.0 * 0.37;
    let vx = gamma_y * (1.0 - (-t / tp).exp());
    let x = 0.5 + gamma_y * (t - tp * (1.0 - (-t / tp).exp()));
    let ev = (p.v[0] - vx).abs() / vx;
    let ex = (p.x[0] - x).abs() / (x - 0.5);
    println!("PT14 frozen Couette: rel err v {ev:e}, x {ex:e}, y = {}", p.x[1]);
    assert!(ev <= 1e-6 && ex <= 1e-6);
    assert_eq!(p.state, State::Active);
}

#[test]
fn pt14_output_file_is_written() {
    let dir = std::env::temp_dir().join("rcfd_particles_case_out");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("p.vtp");
    let _ = std::fs::remove_file(&f);
    let extra = format!("output_file = {:?}\n", f.to_string_lossy());
    // `extra` must sit in the [particles.injection.p] table's parent, so put it in its own header
    let text = couette_text("").replace("[particles.injection.p]", &format!("{extra}[particles.injection.p]"));
    let c = case(&text);
    let sim = Simulation::new(&c).unwrap();
    particles::track_frozen(&c, &sim).unwrap();
    let s = std::fs::read_to_string(&f).unwrap();
    assert!(s.contains("NumberOfPoints=\"1\""));
}

#[test]
fn pt14_transient_hook_reaches_terminal_velocity_in_still_fluid() {
    let text = r#"
title = "settling"
[mesh]
generator = "box_2d"
nx = 6
ny = 12
lx = 1.0
ly = 2.0
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [0.0, -9.81, 0.0]
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.01
end_time = 2.0
[particles]
enabled = true
density = 2500.0
drag = "stokes"
integration = "analytic"
interpolation = "cell_centre"
end_time = 2.0
[particles.injection.p]
kind = "point"
position = [0.5, 1.9, 0.05]
rate = 1000.0
end_time = 0.001
diameter = 1e-4
"#;
    let c = case(text);
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut sim = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = particles::track_transient(&c, &mut sim).unwrap_or_else(|e| panic!("{e:?}"));
    let p = &tr.particles[0];
    let (d, rho_p, rho_f, mu, g) = (1e-4f64, 2500.0f64, 1.2f64, 1.8e-5f64, 9.81f64);
    let vt = d * d * (rho_p - rho_f) * g / (18.0 * mu);
    let rel = (p.v[1] + vt).abs() / vt;
    println!(
        "PT14 transient settling: v_y = {}, terminal {vt}, rel err {rel:e}, state {:?}",
        p.v[1], p.state
    );
    assert_eq!(p.state, State::Active);
    assert!(rel <= 1e-6);
}

#[test]
fn pt14_located_validation_errors() {
    let base = couette_text("");
    let cases: Vec<(String, &str)> = vec![
        (base.replace("density = 1000.0", "density = -1.0"), "particles.density"),
        (base.replace("end_time = 0.4", "end_time = -1.0"), "particles.end_time"),
        (
            base.replace("rate = 1000.0", "rate = -1.0"),
            "particles.injection.p.rate",
        ),
        (
            base.replace("position = [0.5, 0.37, 0.05]", "position = [0.5, 0.37]"),
            "particles.injection.p.position",
        ),
        (
            base.replace("kind = \"point\"", "kind = \"surface\""),
            "particles.injection.p.patch",
        ),
        (
            base.replace("diameter = 9.486832980505138e-4", "diameter = -1.0"),
            "particles.injection.p.diameter",
        ),
        (
            base.replace(
                "diameter = 9.486832980505138e-4",
                "distribution = \"rosin_rammler\"\nrr_d_bar = 1e-4",
            ),
            "particles.injection.p.rr_n",
        ),
        (
            base.clone(),
            "", // control: no error expected
        ),
    ];
    for (text, path) in cases {
        let errs = errors(&text);
        if path.is_empty() {
            assert!(errs.is_empty(), "{errs:?}");
        } else {
            assert!(has(&errs, path), "expected an error at {path}, got {errs:?}");
        }
    }
    // schema level
    assert!(Case::parse_toml(&base.replace("drag = \"stokes\"", "drag = \"bogus\"")).is_err());
    assert!(Case::parse_toml(&base.replace("[particles]\n", "[particles]\nrestitution = 1.5\n")).is_err());
}

#[test]
fn pt14_mesh_dependent_errors_are_located() {
    // unknown wall patch
    let c = case(&couette_text("[particles.walls.nowhere]\ninteraction = \"stick\"\n"));
    let sim = Simulation::new(&c).unwrap();
    let e = particles::build_tracker(&c, &sim).err().expect("error");
    assert!(has(&e, "particles.walls.nowhere"), "{e:?}");
    // injection point outside the mesh
    let c = case(&couette_text("").replace("position = [0.5, 0.37, 0.05]", "position = [9.0, 0.37, 0.05]"));
    let sim = Simulation::new(&c).unwrap();
    let e = particles::build_tracker(&c, &sim).err().expect("error");
    assert!(has(&e, "particles.injection.p.position"), "{e:?}");
    // surface injection on an unknown patch
    let c = case(
        &couette_text("")
            .replace("kind = \"point\"", "kind = \"surface\"\npatch = \"nope\"")
            .replace("position = [0.5, 0.37, 0.05]\n", ""),
    );
    let sim = Simulation::new(&c).unwrap();
    let e = particles::build_tracker(&c, &sim).err().expect("error");
    assert!(has(&e, "particles.injection.p.patch"), "{e:?}");
}

#[test]
fn pt14_disabled_section_changes_nothing() {
    let c = Case::parse_toml("title = \"x\"\n").unwrap();
    assert!(c.semantic_errors().is_empty());
    assert_eq!(c.tree.get("particles.enabled").unwrap(), serde_json::json!(false));
}
