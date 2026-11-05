//! F-256 (ADR 0190): case-tree wiring of the gradient options of the energy and scalar diffusion term
//! (`solver.gradient_face_correction`, `gradient_secondary_limiter`, `gradient_limiter_k`, `gradient_boundary`).
//!
//! P: the skewed `wedge_2d` mesh W reaches 20 degrees of non-orthogonality. G1: located errors (R1 to R4). G2: the tree run
//! equals the library run (`rcfd_fv` only) with the same options to 1e-12. G3: every option set moves the tree result away
//! from the default run by more than 1e-9, so G2 cannot pass unwired. G4 (i): the keys written at their defaults change neither
//! the settings hash nor a bit of the result. The option sets are one table (`sets`), so options added later (F-246) add a row.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::{BoundaryGradient, GradientOptions};
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::gradient::Limiter;
use rcfd_fv::scalar::ScalarTransport;
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, ZeroGradient};
use std::sync::Arc;

/// G2 limit (the acceptance's 1e-12).
const TREE_LIBRARY: f64 = 1e-12;
/// G3 limit: the smallest change of the field (values in [0, 1]) an option set must make against the default run.
const TEETH: f64 = 1e-9;
/// Iterations of the steady cases E1 and S1; steps of E2 (dt 1/16 to end time 1, exact in binary).
const ITERS: usize = 60;
const STEPS: usize = 16;
const DT: f64 = 0.0625;

/// One option set: name, the `[solver]` lines of the case, the library options.
struct Set {
    name: &'static str,
    keys: &'static str,
    opts: GradientOptions,
}

/// ADR 0190 option sets O1 to O6 and C2.
fn sets() -> Vec<Set> {
    let d = GradientOptions::default();
    let venk = |k| Some(Limiter::Venkatakrishnan(k));
    vec![
        Set {
            name: "O1",
            keys: "gradient_face_correction = true\n",
            opts: GradientOptions {
                face_correction: true,
                ..d
            },
        },
        Set {
            name: "O2",
            keys: "gradient_boundary = \"corrected\"\n",
            opts: GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        },
        Set {
            name: "O3",
            keys: "gradient_boundary = \"dropped\"\n",
            opts: GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        },
        Set {
            name: "O4",
            keys: "gradient_secondary_limiter = \"barth_jespersen\"\n",
            opts: GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        },
        Set {
            name: "O5",
            keys: "gradient_secondary_limiter = \"venkatakrishnan\"\n",
            opts: GradientOptions {
                secondary_limiter: venk(5.0),
                ..d
            },
        },
        Set {
            name: "O6",
            keys: "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
            opts: GradientOptions {
                secondary_limiter: venk(0.5),
                ..d
            },
        },
        c2(),
    ]
}

/// ADR 0016's option set C2: face correction, corrected boundary gradient, Venkatakrishnan with K = 5.
fn c2() -> Set {
    Set {
        name: "C2",
        keys: "gradient_face_correction = true\ngradient_boundary = \"corrected\"\ngradient_secondary_limiter = \"venkatakrishnan\"\n",
        opts: GradientOptions {
            face_correction: true,
            secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
            boundary: BoundaryGradient::Corrected,
        },
    }
}

/// The four keys written at their defaults (G4 (i)).
const DEFAULT_KEYS: &str = "gradient_face_correction = false\ngradient_secondary_limiter = \"none\"\ngradient_limiter_k = 5.0\ngradient_boundary = \"owner\"\n";

/// E1 (`steady`) or E2 (transient): energy on the prescribed zero flux of mesh W; `convection` and the extra `[solver]` lines.
fn energy_text(steady: bool, convection: &str, solver: &str) -> String {
    let run = if steady {
        format!("algorithm = \"simplec\"\nmax_iterations = {ITERS}\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n")
    } else {
        format!("algorithm = \"pimple\"\ndt = {DT}\nend_time = 1.0\ntime_scheme = \"euler\"\n")
    };
    format!(
        r#"
title = "f256 energy"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
[energy]
enabled = true
relax = 0.9
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
convection = "{convection}"
frozen_flow = true
{run}{solver}"#
    )
}

/// S1: a steady lid cavity on mesh W with the poisson scalar `s` (`with_scalar`), and the extra `[solver]` lines.
fn scalar_text(with_scalar: bool, solver: &str) -> String {
    let scalar = if with_scalar {
        "[scalars.s]\nequation = \"poisson\"\ndiffusivity = 0.1\nrelax = 0.9\ninitial = 0.5\n[scalars.s.boundaries.xmin]\ntype = \"fixed\"\nvalue = 1.0\n[scalars.s.boundaries.ymin]\ntype = \"fixed\"\nvalue = 0.0\n"
    } else {
        ""
    };
    format!(
        r#"
title = "f256 scalar"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
nu = 0.1
[boundaries.xmin]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
{scalar}[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = {ITERS}
tolerance_u = 1e-14
tolerance_p = 1e-14
{solver}"#
    )
}

/// A phase-change case on mesh W (prescribed zero flux, piso, `linear_upwind`) with extra `[solver]` lines.
fn phase_text(solver: &str) -> String {
    format!(
        r#"
title = "f256 phase change"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "linear_upwind"
frozen_flow = true
dt = {DT}
end_time = 1.0
{solver}[phase_change]
enabled = true
solidus = 0.49
liquidus = 0.51
latent_heat = 2.0
"#
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("case does not parse: {d:?}"))
}

/// Run a case through the tree to its end; returns the simulation and the field.
fn tree(text: &str, field: &str) -> (Simulation, Vec<f64>) {
    let mut s = Simulation::new(&case(text)).unwrap_or_else(|d| panic!("not built: {d:?}"));
    let n = s.run(10 * ITERS);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let want = if text.contains("pimple") { STEPS } else { ITERS };
    assert_eq!(n, want, "iterations or steps of the tree run");
    let f = s.field(field).unwrap();
    (s, f)
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

/// The field data of ADR 0190 on mesh W: value 1 on xmin and 0 on ymin, zero gradient on xmax and ymax.
fn field_w(geom: &FvGeometry, name: &str, zero_gradient_explicit: bool) -> ScalarField {
    let mut f = ScalarField::uniform(name, geom, 0.5);
    f.set_bc(geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    f.set_bc(geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
    if zero_gradient_explicit {
        for p in ["xmax", "ymax"] {
            f.set_bc(geom, p, Box::new(ZeroGradient)).unwrap();
        }
    }
    f
}

/// Library run of E1 (`steady`) or E2: an `EnergySolver` set up as the tree sets it, the options on its field.
fn energy_library(geom: &Arc<FvGeometry>, steady: bool, opts: Option<GradientOptions>) -> Vec<f64> {
    let mut en = EnergySolver::new(geom.clone(), field_w(geom, "T", true), 0.1 / (1.0 * 1.0));
    en.convection = ConvScheme::LinearUpwind;
    en.relax = 0.9;
    en.gradient = opts;
    let zero = vec![0.0; geom.mesh.n_faces()];
    if steady {
        for _ in 0..ITERS {
            en.iterate(&zero);
        }
    } else {
        en.solver.tol_rel = 1e-10;
        en.solver.tol_abs = 1e-300;
        en.solver.max_iter = 2000;
        for _ in 0..STEPS {
            en.advance(&zero, DT, TimeScheme::ImplicitEuler);
        }
    }
    en.t.values
}

/// Library run of S1: a `ScalarTransport` set up as the tree sets it, swept on a zero flux.
fn scalar_library(geom: &Arc<FvGeometry>, opts: Option<GradientOptions>) -> Vec<f64> {
    let mut t = ScalarTransport::new(geom.clone(), field_w(geom, "s", false), 0.1);
    t.convection = ConvScheme::LinearUpwind;
    t.relax = 0.9;
    t.gradient = opts;
    let zero = vec![0.0; geom.mesh.n_faces()];
    for _ in 0..ITERS {
        t.iterate(&zero);
    }
    t.field.values
}

/// P: the precondition on mesh W.
fn precondition(s: &Simulation) {
    let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
    println!(
        "P: mesh W max non-orthogonality {:.3} degrees (at least 20)",
        q.max_non_orthogonality
    );
    assert!(q.max_non_orthogonality >= 20.0, "P set-up: {}", q.max_non_orthogonality);
}

/// G2 and G3 for one case: every set's tree field equals its library run to 1e-12 and differs from the default by > 1e-9.
fn g2_g3(
    label: &str,
    text: impl Fn(&str) -> String,
    field: &str,
    sets: &[Set],
    library: impl Fn(&Arc<FvGeometry>, Option<GradientOptions>) -> Vec<f64>,
) -> Vec<(String, Vec<f64>)> {
    let (s0, default) = tree(&text(""), field);
    precondition(&s0);
    let lib0 = library(&s0.geom, None);
    let d0 = max_diff(&default, &lib0);
    println!("G2 {label} control: default tree vs library without options {d0:.3e} (limit {TREE_LIBRARY:e})");
    assert!(d0 <= TREE_LIBRARY, "G2 {label} control {d0}");
    let mut out = Vec::new();
    for set in sets {
        let (s, f) = tree(&text(set.keys), field);
        let lib = library(&s.geom, Some(set.opts));
        let (d, teeth) = (max_diff(&f, &lib), max_diff(&f, &default));
        println!(
            "G2 {label} {}: tree vs library {d:.3e} (limit {TREE_LIBRARY:e}); G3 change against the default {teeth:.3e} (limit > {TEETH:e})",
            set.name
        );
        assert!(d <= TREE_LIBRARY, "G2 {label} {}: {d}", set.name);
        assert!(teeth > TEETH, "G3 {label} {}: {teeth}", set.name);
        out.push((set.name.to_string(), f));
    }
    out
}

#[test]
fn g2_g3_e1_steady_energy_all_sets() {
    let runs = g2_g3(
        "E1",
        |s| energy_text(true, "linear_upwind", s),
        "T",
        &sets(),
        |g, o| energy_library(g, true, o),
    );
    let get = |n: &str| &runs.iter().find(|(m, _)| m == n).unwrap().1;
    let d = max_diff(get("O5"), get("O6"));
    println!("G3 E1: O5 (K 5) vs O6 (K 0.5) {d:.3e} (limit > 1e-12)");
    assert!(d > 1e-12, "G3 K: {d}");
}

#[test]
fn g2_g3_e2_transient_energy_c2() {
    g2_g3(
        "E2",
        |s| energy_text(false, "linear_upwind", s),
        "T",
        &[c2()],
        |g, o| energy_library(g, false, o),
    );
}

#[test]
fn g2_g3_s1_poisson_scalar_c2() {
    g2_g3("S1", |s| scalar_text(true, s), "s", &[c2()], scalar_library);
}

/// G4 (i): the keys written at their defaults leave the settings hash and every bit of the result unchanged.
#[test]
fn g4_i_keys_at_their_defaults_change_nothing() {
    let cases: [(&str, Box<dyn Fn(&str) -> String>, &str); 3] = [
        ("E1", Box::new(|s: &str| energy_text(true, "linear_upwind", s)), "T"),
        ("E2", Box::new(|s: &str| energy_text(false, "linear_upwind", s)), "T"),
        ("S1", Box::new(|s: &str| scalar_text(true, s)), "s"),
    ];
    for (label, text, field) in cases {
        let (a, b) = (text(""), text(DEFAULT_KEYS));
        let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
        assert_eq!(ha, hb, "G4 (i) {label}: settings hash");
        let (_, fa) = tree(&a, field);
        let (_, fb) = tree(&b, field);
        let differ = fa.iter().zip(&fb).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        println!(
            "G4 (i) {label}: settings hash {ha:016x} both; {differ} of {} cells differ in a bit (limit 0)",
            fa.len()
        );
        assert_eq!(differ, 0, "G4 (i) {label}");
    }
}

/// The path of the first diagnostic of a refused case (parse or build).
fn refused_at(text: &str) -> String {
    match Case::parse_toml(text) {
        Err(d) => d.first().map(|x| x.path.clone()).unwrap_or_default(),
        Ok(c) => match Simulation::new(&c) {
            Err(d) => {
                println!("  {}: {}", d[0].path, d[0].message);
                d[0].path.clone()
            }
            Ok(_) => "accepted".into(),
        },
    }
}

#[test]
fn g1_located_errors() {
    let e1 = |conv: &str, keys: &str| energy_text(true, conv, keys);
    let checks: Vec<(&str, String, &str)> = vec![
        // R1: values the schema refuses
        (
            "R1 limiter name",
            e1("linear_upwind", "gradient_secondary_limiter = \"minmod\"\n"),
            "solver.gradient_secondary_limiter",
        ),
        (
            "R1 boundary name",
            e1("linear_upwind", "gradient_boundary = \"wall\"\n"),
            "solver.gradient_boundary",
        ),
        (
            "R1 negative K",
            e1("linear_upwind", "gradient_limiter_k = -1.0\n"),
            "solver.gradient_limiter_k",
        ),
        (
            "R1 not a boolean",
            e1("linear_upwind", "gradient_face_correction = \"yes\"\n"),
            "solver.gradient_face_correction",
        ),
        // R2: no cell gradient, so no secondary flux
        (
            "R2 upwind",
            e1("upwind", "gradient_face_correction = true\n"),
            "solver.gradient_face_correction",
        ),
        (
            "R2 central",
            e1("central", "gradient_boundary = \"dropped\"\n"),
            "solver.gradient_boundary",
        ),
        (
            "R2 blended",
            e1("blended", "gradient_secondary_limiter = \"barth_jespersen\"\n"),
            "solver.gradient_secondary_limiter",
        ),
        // R3: no equation takes the options
        (
            "R3 no energy, no scalar",
            scalar_text(false, "gradient_face_correction = true\n"),
            "solver.gradient_face_correction",
        ),
        (
            "R3 phase change",
            phase_text("gradient_boundary = \"corrected\"\n"),
            "solver.gradient_boundary",
        ),
        // R4: K without its limiter
        (
            "R4 K alone",
            e1("linear_upwind", "gradient_limiter_k = 1.0\n"),
            "solver.gradient_limiter_k",
        ),
        // controls: accepted
        (
            "control E1 O1",
            e1("linear_upwind", "gradient_face_correction = true\n"),
            "accepted",
        ),
        (
            "control S1 O1",
            scalar_text(true, "gradient_face_correction = true\n"),
            "accepted",
        ),
        (
            "control E1 O6",
            e1(
                "linear_upwind",
                "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
            ),
            "accepted",
        ),
        ("control phase change without keys", phase_text(""), "accepted"),
    ];
    for (what, text, want) in checks {
        let got = refused_at(&text);
        println!("G1 {what}: {got} (want {want})");
        assert_eq!(got, want, "G1 {what}");
    }
}
