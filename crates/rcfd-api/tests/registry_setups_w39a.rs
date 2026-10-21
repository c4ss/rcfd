//! Wave 39A (ADR 0092, F-240): case-tree set-ups of the five benchmark registry cases (`cases/*/case.json`) and of further
//! published validation cases, each put through the same path as `rcfd check` (`runctl::check_only`: validation, simulation
//! construction, mesh build, mesh quality, set-up rules). Registered limits: exit code 0 and zero error-level rule diagnostics on
//! every set-up; warnings and infos are printed, not limited.
//!
//! The registry descriptors `backward_step` and `naca0012` name no case-tree generator, so their meshes are built here (the
//! library step generator with the descriptor's parameters; an O-grid warped onto the NACA 0012 thickness formula of
//! `cases/naca0012/reference/naca4_thickness.txt`), written as an OpenFOAM polyMesh directory and read by the set-up through
//! `mesh.source = "file"`.

use rcfd_api::case::Case;
use rcfd_api::runctl::check_only;
use rcfd_mesh::gen::{backward_step, cylinder_ogrid, Grading, StepSpec};
use rcfd_mesh::Mesh;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w39a_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// cavity: Re = 100 (U = 1, L = 1, nu = 0.01), n = 32 (first entries of the descriptor).
const CAVITY: &str = r#"
title = "registry cavity, Re 100"
[mesh]
generator = "cavity_2d"
nx = 32
ny = 32
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 2000
"#;

/// heated_cavity: Ra = 1e3, Pr = 0.71 in units with L = 1, alpha = 1, dT = 1, beta = 1: nu = Pr = 0.71,
/// g = Ra Pr = 710 (Ra = g beta dT L^3 / (nu alpha)); n = 40.
const HEATED_CAVITY: &str = r#"
title = "registry heated cavity, Ra 1e3"
[mesh]
generator = "box_2d"
nx = 40
ny = 40
lx = 1.0
ly = 1.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
reference_temperature = 0.5
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "tvd_van_leer"
alpha_u = 0.8
max_iterations = 3000
"#;

/// flat_plate: laminar Re_L = 1e5 on L = 2 (U = 1, nu = 2e-5); 100 x 60 cells over 2 x 0.5 with inflation layers on the
/// plate (ymin).
const FLAT_PLATE: &str = r#"
title = "registry flat plate, Re_L 1e5"
[mesh]
generator = "box_2d"
nx = 100
ny = 60
lx = 2.0
ly = 0.5
wall_layers = 20
layer_walls = "lower"
first_layer_height = 1e-4
layer_growth = 1.15
[physics]
nu = 2e-5
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 3000
"#;

/// backward_step: Re_h = 37500 (U = 1, h = 1), standard k-epsilon with wall functions; the mesh is the descriptor's.
const BACKWARD_STEP: &str = r#"
title = "registry backward-facing step, Re_h 37500"
[mesh]
source = "file"
file = "MESH"
[physics]
nu = 2.6666666666666667e-5
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
turbulence_spec = "intensity_length"
turbulence_intensity = 0.05
turbulence_length = 0.8
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
alpha_u = 0.7
alpha_p = 0.3
max_iterations = 3000
[turbulence]
model = "k_epsilon"
variant = "standard"
wall_function = "standard"
"#;

/// naca0012: incompressible k-omega SST at Re_c = 6e6 (Ladson's condition), alpha = 4 degrees, chord 1.
const NACA0012: &str = r#"
title = "registry NACA 0012, Re 6e6, alpha 4 deg"
[mesh]
source = "file"
file = "MESH"
[physics]
nu = 1.6666666666666667e-7
[initial]
velocity = [0.9975640502598242, 0.0697564737441253, 0.0]
[boundaries.airfoil]
velocity = "fixed"
[boundaries.farfield]
velocity = "freestream"
velocity_value = [0.9975640502598242, 0.0697564737441253, 0.0]
pressure = "freestream"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 3000
[turbulence]
model = "k_omega_sst"
"#;

/// Cylinder at Re = 100 (D = 1, U = 1, nu = 0.01), the set-up of the shedding statistics test.
const CYLINDER_RE100: &str = r#"
title = "cylinder Re 100"
[mesh]
generator = "cylinder_ogrid"
nx = 128
ny = 100
r_in = 0.5
r_out = 20.0
grading = 150.0
depth = 0.1
[physics]
nu = 0.01
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.cylinder]
velocity = "fixed"
[boundaries.farfield]
velocity = "freestream"
velocity_value = [1.0, 0.0, 0.0]
pressure = "freestream"
[solver]
algorithm = "piso"
convection = "linear_upwind"
dt = 0.05
end_time = 160.0
"#;

/// The descriptor's step: l_in 10, l_out 40, h_in 8, step 1, depth 0.1, n = [40, 160, 32, 8].
fn step_mesh() -> Mesh {
    backward_step(StepSpec {
        l_in: 10.0,
        l_out: 40.0,
        h_in: 8.0,
        step: 1.0,
        depth: 0.1,
        n: [40, 160, 32, 8],
    })
    .unwrap()
}

/// NACA 0012 O-grid: a cylinder O-grid (128 x 48, radial expansion 200) whose inner ring is moved onto the closed-trailing-edge
/// NACA 4-digit thickness y_t = 5 t (0.2969 sqrt x - 0.1260 x - 0.3516 x^2 + 0.2843 x^3 - 0.1036 x^4), t = 0.12, at
/// x = (1 + cos theta) / 2, and whose outer ring is a circle of radius 20 chords about mid-chord; points in between are
/// blended linearly in the generator's graded radial coordinate.
fn naca0012_mesh() -> Mesh {
    let (r_in, r_out, far) = (1.0, 2.0, 20.0);
    let m = cylinder_ogrid(128, 48, r_in, r_out, Grading::Expansion(200.0), 0.1).unwrap();
    let yt = |x: f64| 0.6 * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1036 * x.powi(4));
    let mut topo = m.topology().clone();
    for p in topo.points.iter_mut() {
        let th = p[1].atan2(p[0]);
        let s = ((p[0].hypot(p[1]) - r_in) / (r_out - r_in)).clamp(0.0, 1.0);
        let xa = 0.5 * (1.0 + th.cos());
        let ya = yt(xa).max(0.0) * th.sin().signum();
        let (xo, yo) = (0.5 + far * th.cos(), far * th.sin());
        *p = [(1.0 - s) * xa + s * xo, (1.0 - s) * ya + s * yo, p[2]];
    }
    for pa in topo.patches.iter_mut() {
        if pa.name == "cylinder" {
            pa.name = "airfoil".into();
        }
    }
    Mesh::from_topology(topo).unwrap()
}

/// (name, case) of every set-up; file meshes are written under `dir`.
fn setups(dir: &Path) -> Vec<(&'static str, Case)> {
    let parse = |t: &str| Case::parse_toml_in(t, dir).unwrap_or_else(|e| panic!("{e:?}"));
    let with_mesh = |t: &str, name: &str, m: Mesh| {
        let d = dir.join(name);
        rcfd_io::openfoam::write_dir(&m, &d).unwrap();
        parse(&t.replace("MESH", &d.display().to_string().replace('\\', "/")))
    };
    let channel = std::fs::read_to_string(repo().join("cases/channel/sst_channel.toml")).unwrap();
    // Re_tau = 178.12 (Moser, Kim and Mansour): nu = 1/178.12, omega_initial = k / (10 nu)
    let re180 = channel
        .replace(
            "nu = 0.002531645569620253   # 1 / 395",
            "nu = 0.005614192679092746   # 1 / 178.12",
        )
        .replace("omega_initial = 39.5   # k / (10 nu)", "omega_initial = 17.812")
        .replace("Re_tau = 395", "Re_tau = 178.12");
    assert!(
        re180.contains("1 / 178.12") && re180.contains("17.812"),
        "sst_channel.toml changed"
    );
    let load = |rel: &str| Case::load(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
    vec![
        ("cavity", parse(CAVITY)),
        ("backward_step", with_mesh(BACKWARD_STEP, "step", step_mesh())),
        ("flat_plate", parse(FLAT_PLATE)),
        ("heated_cavity", parse(HEATED_CAVITY)),
        ("naca0012", with_mesh(NACA0012, "naca", naca0012_mesh())),
        ("cylinder_re100", parse(CYLINDER_RE100)),
        (
            "channel_re180",
            Case::parse_toml_in(&re180, &repo().join("cases/channel")).unwrap(),
        ),
        ("channel_wall_law", load("cases/channel/kepsilon_channel.toml")),
        ("sod", load("cases/sod/sod.toml")),
    ]
}

#[test]
fn v1_registry_and_validation_setups_pass_check_with_no_error_level_rule() {
    let dir = tmp("setups");
    let mut fails = Vec::new();
    for (name, case) in setups(&dir) {
        let (code, ev) = check_only(&case);
        let rules = ev["rules"].as_array().cloned().unwrap_or_default();
        let sev = |s: &str| rules.iter().filter(|r| r["severity"] == s).count();
        let ids: Vec<String> = rules
            .iter()
            .map(|r| {
                format!(
                    "{} {}",
                    r["id"].as_str().unwrap_or(""),
                    r["severity"].as_str().unwrap_or("")
                )
            })
            .collect();
        let q = &ev["mesh"];
        println!(
            "V1 {name}: exit {code}, cells {}, errors {}, warnings {}, infos {} {ids:?}; max non-orth {:.2} deg, max skew {:.3}, max aspect {:.1}; diagnostics {}",
            ev["cells"],
            sev("error"),
            sev("warning"),
            sev("info"),
            q["max_non_orthogonality"].as_f64().unwrap_or(f64::NAN),
            q["max_skewness"].as_f64().unwrap_or(f64::NAN),
            q["max_aspect_ratio"].as_f64().unwrap_or(f64::NAN),
            ev["diagnostics"]
        );
        if code != 0 || sev("error") != 0 {
            fails.push(format!("{name}: exit {code}, {ids:?}, {}", ev["diagnostics"]));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(fails.is_empty(), "{fails:#?}");
}

/// Graetz problem, parallel plates (the validation_graetz case): energy only on a prescribed fully developed flow
/// (`solver.frozen_flow`), parabolic u with mean 1 between plates at y = 0 and 1, inlet temperature 0, walls 1.
const GRAETZ: &str = r#"
title = "Graetz parallel plates, prescribed flow"
[mesh]
generator = "box_2d"
nx = 100
ny = 20
lx = 10.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 0.01
[energy]
enabled = true
[initial]
velocity_expression_x = "6 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])"
temperature = 0.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.ymax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[solver]
algorithm = "simple"
convection = "upwind"
frozen_flow = true
max_iterations = 500
"#;

/// Exit code and error-level rule count of `rcfd check` on `text`, printed.
fn check_text(label: &str, text: &str) -> (u8, usize) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let (code, ev) = check_only(&case);
    let rules = ev["rules"].as_array().cloned().unwrap_or_default();
    let errors = rules.iter().filter(|r| r["severity"] == "error").count();
    println!(
        "{label}: exit {code}, cells {}, errors {errors}, rules {}, diagnostics {}",
        ev["cells"], ev["rules"], ev["diagnostics"]
    );
    (code, errors)
}

/// V2 (ADR 0092 amendment 1): the Graetz set-up, same limits as V1. DEFECTIVE SET-UP (amendment 2): the inlet face keeps the
/// fixed velocity 0, so the prescribed flux is not divergence free and the semantic check refuses the case (exit 3); kept
/// unchanged and ignored, corrected in V2b.
#[test]
#[ignore = "known miss: defective set-up (inlet velocity 0 under a parabolic prescribed flux, ADR 0092 amendment 2); see V2b"]
fn v2_graetz_setup_passes_check_with_no_error_level_rule_known_miss() {
    let (code, errors) = check_text("V2 graetz", GRAETZ);
    assert!(code == 0 && errors == 0, "exit {code}, {errors} errors");
}

/// V2b (ADR 0092 amendment 2): V2 with the parabolic profile also on the inlet boundary; same limits.
#[test]
fn v2b_graetz_setup_with_inlet_profile_passes_check_with_no_error_level_rule() {
    let text = GRAETZ.replace(
        "[boundaries.xmin]\nvelocity = \"fixed\"\n",
        "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_expression_x = \"6 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])\"\n",
    );
    assert_ne!(text, GRAETZ);
    let (code, errors) = check_text("V2b graetz", &text);
    assert!(code == 0 && errors == 0, "exit {code}, {errors} errors");
}
