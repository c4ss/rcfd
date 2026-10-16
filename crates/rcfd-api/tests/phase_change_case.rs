//! Enthalpy-method melting through the settings tree (F-212, ADR 0035 checks T1 to T3). The limits are the ones registered
//! in the ADR before this file was written.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_fv::solidification::{PhaseChange, PhaseLaw};
use rcfd_fv::{FixedValue, ScalarField};

const EPS: f64 = 0.002;
/// Melting temperature (K): the tree needs absolute temperatures, the Stefan problem is invariant under the shift.
const TM: f64 = 300.0;

fn build(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => vec![],
        },
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

/// Melting bar of length 6, `n` cells, hot end at +1, everything else at the solidus; `k = rho = cp = 1`, `L = 2`.
fn text(n: usize, extra_phase: &str, extra_solver: &str, extra: &str) -> String {
    let dt = 0.5 * 6.0 / n as f64;
    format!(
        r#"
title = "stefan"
[mesh]
generator = "box_2d"
nx = {n}
ny = 1
lx = 6.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = {ts}
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = {hot}
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = {ts}
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "upwind"
frozen_flow = true
dt = {dt}
end_time = 1.0
{extra_solver}
[phase_change]
enabled = true
{extra_phase}
{extra}
"#,
        ts = TM - EPS,
        hot = TM + 1.0
    )
}

const PHASE: &str = "solidus = 299.998\nliquidus = 300.002\nlatent_heat = 2.0";

fn erf(x: f64) -> f64 {
    let mut sum = 0.0;
    let mut term = x;
    for n in 0..80 {
        sum += term / (2 * n + 1) as f64;
        term *= -x * x / (n + 1) as f64;
    }
    2.0 / std::f64::consts::PI.sqrt() * sum
}

/// Root of `st e^{-l^2} / (sqrt(pi) erf l) = l` (one phase, Stefan number `st`).
fn lambda(st: f64) -> f64 {
    let g = |l: f64| st * (-l * l).exp() / (std::f64::consts::PI.sqrt() * erf(l)) - l;
    let (mut a, mut b) = (1e-3, 1.5);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if g(m) > 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

fn xmin_area(s: &Simulation) -> f64 {
    let sf = s.geom.mesh.face_area_vectors();
    s.geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| rcfd_core::vec3::norm(sf[f]))
        .sum()
}

#[test]
fn t1_t2_stefan_through_the_tree_equals_the_library() {
    let n = 240;
    let mut s = build(&text(n, PHASE, "", ""));
    s.run(1000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.field("T").unwrap();
    let f = s.field("liquid_fraction").unwrap();
    // T2: the field is the clamp of T between the solidus and the liquidus, within [0, 1]
    let mut worst: f64 = 0.0;
    for (tc, fc) in t.iter().zip(&f) {
        assert!((0.0..=1.0).contains(fc));
        worst = worst.max((fc - ((tc - 299.998) / (300.002 - 299.998)).clamp(0.0, 1.0)).abs());
    }
    println!("T2: liquid_fraction vs clamp(T): max difference {worst:.3e} (limit 1e-14)");
    assert!(worst <= 1e-14);
    // T1: position against the Neumann solution
    let vol = s.geom.mesh.cell_volumes();
    let pos = f.iter().enumerate().map(|(c, x)| vol[c] * x).sum::<f64>() / xmin_area(&s);
    let exact = 2.0 * lambda(0.5) * 1.0_f64.sqrt();
    let rel = (pos - exact).abs() / exact;
    println!("T1: tree position {pos:.6} vs Neumann {exact:.6} (rel {rel:.4e}, limit 3e-2)");
    assert!(rel <= 0.03);
    // the library run of the same set-up
    let geom = s.geom.clone();
    let mut tl = ScalarField::uniform("T", &geom, TM - EPS);
    tl.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(TM + 1.0)))
        .unwrap();
    tl.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(TM - EPS)))
        .unwrap();
    let law = PhaseLaw {
        solidus: TM - EPS,
        liquidus: TM + EPS,
        latent_k: 2.0,
    };
    let mut pc = PhaseChange::new(law, vec![1.0; geom.mesh.n_faces()], &tl.values);
    let dt = 0.5 * 6.0 / n as f64;
    for _ in 0..80 {
        pc.advance(&geom, &mut tl, dt, None);
    }
    let diff = t.iter().zip(&tl.values).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    let peak = tl.values.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    println!(
        "T1: tree vs library max difference {:.3e} relative (limit 1e-10)",
        diff / peak
    );
    assert!(diff / peak <= 1e-10);
}

#[test]
fn t2_the_field_needs_the_phase_change() {
    let mut text = text(20, PHASE, "", "");
    text = text.replace("[phase_change]\nenabled = true", "[phase_change]\nenabled = false");
    let text = text.replace("solidus = 299.998\nliquidus = 300.002\nlatent_heat = 2.0", "");
    let s = build(&text);
    assert!(s.field("liquid_fraction").is_err());
}

#[test]
fn t3_located_errors() {
    let cases: Vec<(String, &str)> = vec![
        (text(20, "liquidus = 300.002\nlatent_heat = 2.0", "", ""), "phase_change.solidus"),
        (text(20, "solidus = 299.998\nlatent_heat = 2.0", "", ""), "phase_change.liquidus"),
        (text(20, "solidus = 299.998\nliquidus = 300.002", "", ""), "phase_change.latent_heat"),
        (
            text(20, "solidus = 300.002\nliquidus = 300.002\nlatent_heat = 2.0", "", ""),
            "phase_change.liquidus",
        ),
        (
            text(20, "solidus = 300.002\nliquidus = 299.998\nlatent_heat = 2.0", "", ""),
            "phase_change.liquidus",
        ),
        (text(20, PHASE, "time_scheme = \"bdf2\"", ""), "solver.time_scheme"),
        (
            text(20, PHASE, "", "").replace("algorithm = \"piso\"", "algorithm = \"simple\""),
            "solver.algorithm",
        ),
        (
            text(20, PHASE, "", "").replace("frozen_flow = true", "frozen_flow = false"),
            "phase_change.enabled",
        ),
        (
            text(20, "solidus = 299.998\nliquidus = 300.002\nlatent_heat = 2.0\nmomentum_constant = 1e5", "", ""),
            "phase_change.momentum_constant",
        ),
        (
            text(
                20,
                PHASE,
                "",
                "[solids.s]\nzone = \"box\"\nbox_min = [4.0, -1.0, -1.0]\nbox_max = [5.0, 2.0, 2.0]\nconductivity = 1.0\ndensity = 1.0\nspecific_heat = 1.0",
            ),
            "phase_change.enabled",
        ),
        (
            text(
                20,
                PHASE,
                "",
                "[electric]\nenabled = true\nconductivity = 1.0\n[zone_sources.z]\nkind = \"source\"",
            ),
            "phase_change.enabled",
        ),
        (
            text(
                20,
                PHASE,
                "",
                "[zone_sources.z]\nzone = \"all\"\ntarget = \"temperature\"\nkind = \"fixed_value\"\nvalue = 0.0",
            ),
            "zone_sources.z.kind",
        ),
        (
            text(20, PHASE, "", "")
                .replace("[initial]
", "[initial]
velocity = [1.0, 0.0, 0.0]
")
                .replace("[boundaries.xmin]
velocity = \"fixed\"", "[boundaries.xmin]
velocity = \"fixed\"
velocity_value = [1.0, 0.0, 0.0]")
                .replace("[boundaries.xmax]
velocity = \"fixed\"", "[boundaries.xmax]
velocity = \"zero_gradient\""),
            "phase_change.enabled",
        ),
    ];
    for (i, (t, path)) in cases.iter().enumerate() {
        let e = errors(t);
        assert!(has(&e, path), "case {i}: expected {path}, got {e:?}");
    }
    assert!(errors(&text(20, PHASE, "", "")).is_empty());
}
