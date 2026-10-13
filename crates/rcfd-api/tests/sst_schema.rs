//! k-omega SST and boundary-layer inflation through the case schema (F-043, F-108): parity with
//! the library API, validation diagnostics and the mesh wall-layer keys.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

const EXAMPLE: &str = include_str!("../../../cases/channel/sst_channel.toml");

/// Diagnostics from parsing and, when the tree is valid, from building the simulation.
fn diag_paths(text: &str) -> Vec<(String, String)> {
    let errs = match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => Vec::new(),
        },
    };
    errs.into_iter().map(|d| (d.path, d.message)).collect()
}

#[test]
fn sst_channel_from_the_tree_equals_the_library_run() {
    let n = 150;
    let case = Case::parse_toml(EXAMPLE).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(n);
    // the same set-up through the library
    let nu = 1.0 / 395.0;
    let g = Grading::WallLayers {
        first: 2.0 * nu,
        growth: 1.1,
        layers: 16,
        both: true,
    };
    let mesh = box_2d_graded(4, 60, 1.0, 2.0, [Grading::Uniform, g], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [17.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut model = KOmegaSst::new(geom.clone(), nu, &["ymin", "ymax"], 1.0, 39.5).unwrap();
    for _ in 0..n {
        model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        flow.iterate();
    }
    let ux: Vec<f64> = flow.u.values.iter().map(|v| v[0]).collect();
    let d = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    assert!(d(&sim.field("ux").unwrap(), &ux) < 1e-9, "ux");
    assert!(d(&sim.field("k").unwrap(), &model.k.values) < 1e-9, "k");
    assert!(d(&sim.field("omega").unwrap(), &model.omega.values) < 1e-9, "omega");
    assert!(d(&sim.field("nut").unwrap(), &model.nut) < 1e-9, "nut");
    // wall-cell y+ is the target (first cell height from target_y_plus, u_tau = 1)
    let yp = sim.monitors.last("yplus_lower").unwrap();
    assert!((yp - 1.0).abs() < 0.3, "y+ {yp}");
    let st = sim.status_json();
    assert_eq!(st["turbulence"]["model"], "k_omega_sst");
    assert_eq!(st["turbulence"]["variant"], "menter_2003");
}

#[test]
fn sst_validation_errors() {
    let with = |from: &str, to: &str| EXAMPLE.replace(from, to);
    // k-epsilon-only keys on an SST case
    let t = with("k_initial = 1.0", "k_initial = 1.0\nvariant = \"rng\"");
    assert!(
        diag_paths(&t).iter().any(|e| e.0 == "turbulence.variant"),
        "{:?}",
        diag_paths(&t)
    );
    let t = with("k_initial = 1.0", "k_initial = 1.0\nepsilon_initial = 0.3");
    assert!(diag_paths(&t).iter().any(|e| e.0 == "turbulence.epsilon_initial"));
    // omega_initial on a k-epsilon case
    let t = with("model = \"k_omega_sst\"", "model = \"k_epsilon\"");
    assert!(diag_paths(&t).iter().any(|e| e.0 == "turbulence.omega_initial"));
    // no wall patch
    let t =
        with("[boundaries.ymin]\nvelocity = \"fixed\"\n", "").replace("[boundaries.ymax]\nvelocity = \"fixed\"\n", "");
    assert!(
        diag_paths(&t).iter().any(|e| e.1.contains("wall patch")),
        "{:?}",
        diag_paths(&t)
    );
    // unknown model
    let t = with("model = \"k_omega_sst\"", "model = \"k_omega\"");
    assert!(!diag_paths(&t).is_empty());
}

#[test]
fn mesh_wall_layer_keys() {
    // explicit first height: the lowest cell centre is half of it
    let t = EXAMPLE.replace(
        "target_y_plus = 1.0\nlayer_u_tau = 1.0\n",
        "first_layer_height = 0.004\n",
    );
    let sim = Simulation::new(&Case::parse_toml(&t).unwrap()).unwrap();
    let cc = sim.geom.mesh.cell_centres();
    let ymin = cc.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
    assert!((ymin - 0.002).abs() < 1e-6, "{ymin}");
    // neither a first height nor a target y+
    let t = EXAMPLE.replace(
        "target_y_plus = 1.0
layer_u_tau = 1.0
",
        "",
    );
    assert!(
        diag_paths(&t).iter().any(|e| e.0 == "mesh.wall_layers"),
        "{:?}",
        diag_paths(&t)
    );
    // target y+ without any friction velocity information
    let t = EXAMPLE.replace(
        "layer_u_tau = 1.0
",
        "",
    );
    assert!(
        diag_paths(&t).iter().any(|e| e.0 == "mesh.target_y_plus"),
        "{:?}",
        diag_paths(&t)
    );
    // more layers than cells, and a bad growth ratio
    let t = EXAMPLE.replace("wall_layers = 16", "wall_layers = 40");
    assert!(
        diag_paths(&t).iter().any(|e| e.0 == "mesh.wall_layers"),
        "{:?}",
        diag_paths(&t)
    );
    let t = EXAMPLE.replace("layer_growth = 1.1", "layer_growth = -0.5");
    assert!(!diag_paths(&t).is_empty());
}
