//! ADR 0069 (wave 33B): stability of PISO/PIMPLE on a wall-bounded impulsive start (lid-driven cavity).
//!
//! Registered set-up (fixed before any run): Re in {100, 1000}, linear upwind, lid speed 1, fluid at rest at t = 0, t_end = 10, meshes M-U (uniform
//! 32 x 32) and M-G (32 x 32, `Grading::Symmetric(20)` in x and y), `dt = Co_t h_min / U` with `h_min` the smallest cell width (twice the smallest
//! centre-to-wall distance), algorithms PISO (n_outer 1, 2 correctors), PIMPLE-2 and PIMPLE-4 (1 corrector, alpha_u 0.8, alpha_p 0.5), Euler and BDF2.
//! Required stable set: PISO Co_t in {0.25, 0.5, 1}; PIMPLE Co_t in {0.5, 1, 2, 4}. Report-only scan: PISO {2, 4, 8}, PIMPLE {8, 16}.
//! Stable = no guard trip and finite, max|u| <= 1.5, max|p| <= 50, continuity <= 1e-3 at every step, and t_end reached.
//!
//! Heavy tests are ignored (cost above the 5 minute guideline): `cargo test --release -p rcfd-fv --test w33b_stability -- --ignored --nocapture`.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, cavity_2d, Grading};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mesh {
    Uniform,
    Graded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Alg {
    Piso,
    Pimple2,
    Pimple4,
}

const T_END: f64 = 10.0;

type Tweak<'a> = &'a dyn Fn(&mut PimpleSettings);

fn setup(
    mesh: Mesh,
    nu: f64,
    alg: Alg,
    scheme: TimeScheme,
    tweak: &dyn Fn(&mut PimpleSettings),
) -> (PimpleSolver, f64) {
    let n = 32;
    let (m, lid) = match mesh {
        Mesh::Uniform => (cavity_2d(n, 1.0).unwrap(), "lid"),
        Mesh::Graded => (
            box_2d_graded(n, n, 1.0, 1.0, [Grading::Symmetric(20.0); 2], false, false).unwrap(),
            "ymax",
        ),
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let h_min = 2.0 * geom.dn.iter().copied().fold(f64::INFINITY, f64::min);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    match mesh {
        Mesh::Uniform => {
            u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
                .unwrap();
            u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
                .unwrap();
        }
        Mesh::Graded => {
            u.set_bc(&geom, lid, Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
                .unwrap();
            for w in ["xmin", "xmax", "ymin"] {
                u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            }
        }
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = match alg {
        Alg::Piso => PimpleSettings::piso(nu),
        Alg::Pimple2 => PimpleSettings::pimple(nu, 2),
        Alg::Pimple4 => PimpleSettings::pimple(nu, 4),
    };
    st.convection = ConvScheme::LinearUpwind;
    st.time_scheme = scheme;
    tweak(&mut st);
    (PimpleSolver::new(geom, u, p, st), h_min)
}

#[derive(Debug)]
struct Outcome {
    stable: bool,
    why: String,
    steps: usize,
    max_u: f64,
    max_p: f64,
    max_co: f64,
    max_cont: f64,
    t_reached: f64,
}

fn run_case(mesh: Mesh, re: f64, alg: Alg, scheme: TimeScheme, co_t: f64) -> Outcome {
    run_case_with(mesh, re, alg, scheme, co_t, &|_| {})
}

fn run_case_with(
    mesh: Mesh,
    re: f64,
    alg: Alg,
    scheme: TimeScheme,
    co_t: f64,
    tweak: &dyn Fn(&mut PimpleSettings),
) -> Outcome {
    let (mut s, h_min) = setup(mesh, 1.0 / re, alg, scheme, tweak);
    let dt = co_t * h_min;
    let mut o = Outcome {
        stable: true,
        why: String::new(),
        steps: 0,
        max_u: 0.0,
        max_p: 0.0,
        max_co: 0.0,
        max_cont: 0.0,
        t_reached: 0.0,
    };
    fn fail(o: &mut Outcome, why: String) {
        if o.stable {
            o.stable = false;
            o.why = why;
        }
    }
    while s.time < T_END - 1e-12 {
        let d = dt.min(T_END - s.time);
        let info = s.step(d);
        o.steps += 1;
        o.t_reached = s.time;
        if s.diverged.is_some() {
            let k = o.steps;
            fail(&mut o, format!("guard trip at step {k}"));
            break;
        }
        let mu =
            s.u.values
                .iter()
                .map(|v| (v[0] * v[0] + v[1] * v[1]).sqrt())
                .fold(0.0f64, |a, b| if b.is_nan() { f64::INFINITY } else { a.max(b) });
        let mp =
            s.p.values
                .iter()
                .map(|x| x.abs())
                .fold(0.0f64, |a, b| if b.is_nan() { f64::INFINITY } else { a.max(b) });
        o.max_u = o.max_u.max(mu);
        o.max_p = o.max_p.max(mp);
        o.max_co = o.max_co.max(info.courant);
        o.max_cont = o.max_cont.max(info.continuity);
        if !(mu.is_finite() && mp.is_finite()) {
            let k = o.steps;
            fail(&mut o, format!("non-finite at step {k}"));
            break;
        }
        if mu > 1.5 {
            fail(&mut o, format!("max|u| {mu:.3} > 1.5 at t = {:.3}", s.time));
            if mu > 1e3 {
                break;
            }
        }
        if mp > 50.0 {
            fail(&mut o, format!("max|p| {mp:.3} > 50 at t = {:.3}", s.time));
            if mp > 1e6 {
                break;
            }
        }
        if info.continuity > 1e-3 {
            fail(
                &mut o,
                format!("continuity {:.2e} > 1e-3 at t = {:.3}", info.continuity, s.time),
            );
        }
    }
    if o.stable && s.time < T_END - 1e-9 {
        fail(&mut o, "t_end not reached".into());
    }
    o
}

fn line(id: &str, mesh: Mesh, re: f64, alg: Alg, scheme: TimeScheme, co_t: f64, o: &Outcome) {
    println!(
        "RESULT {id} {mesh:?} Re={re} {alg:?} {scheme:?} Co_t={co_t}: {} (achieved Co {:.3}, steps {}, t {:.2}, max|u| {:.3}, max|p| {:.3}, cont {:.1e}){}",
        if o.stable { "STABLE" } else { "UNSTABLE" },
        o.max_co,
        o.steps,
        o.t_reached,
        o.max_u,
        o.max_p,
        o.max_cont,
        if o.why.is_empty() { String::new() } else { format!(" [{}]", o.why) }
    );
}

fn required_co(alg: Alg) -> &'static [f64] {
    match alg {
        Alg::Piso => &[0.25, 0.5, 1.0],
        _ => &[0.5, 1.0, 2.0, 4.0],
    }
}

fn scan_co(alg: Alg) -> &'static [f64] {
    match alg {
        Alg::Piso => &[2.0, 4.0, 8.0],
        _ => &[8.0, 16.0],
    }
}

fn run_set(
    id: &str,
    mesh: Mesh,
    alg: Alg,
    res: &[f64],
    schemes: &[TimeScheme],
    cos: &dyn Fn(Alg) -> &'static [f64],
) -> Vec<String> {
    let mut fails = Vec::new();
    for &re in res {
        for &scheme in schemes {
            for &co in cos(alg) {
                let o = run_case(mesh, re, alg, scheme, co);
                line(id, mesh, re, alg, scheme, co, &o);
                if !o.stable {
                    fails.push(format!("{mesh:?} Re={re} {alg:?} {scheme:?} Co_t={co}: {}", o.why));
                }
            }
        }
    }
    fails
}

const BOTH: [TimeScheme; 2] = [TimeScheme::ImplicitEuler, TimeScheme::Bdf2];

/// Light subset in the default suite: uniform mesh, Re 100, both time schemes, the largest required Co_t of PISO and PIMPLE-4.
#[test]
fn s1_smoke_uniform_re100() {
    let mut fails = Vec::new();
    for (alg, co) in [(Alg::Piso, 1.0), (Alg::Pimple4, 4.0)] {
        for scheme in BOTH {
            let o = run_case(Mesh::Uniform, 100.0, alg, scheme, co);
            line("S1smoke", Mesh::Uniform, 100.0, alg, scheme, co, &o);
            if !o.stable {
                fails.push(format!("{alg:?} {scheme:?} Co_t={co}: {}", o.why));
            }
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
}

/// KNOWN MISS (registered limit unchanged, ADR 0069 addendum): PIMPLE-2 (one corrector) at Co_t = 4, Re = 100 is unstable.
#[test]
#[ignore = "known miss of ADR 0069: PIMPLE-2 with one corrector at Co_t = 4, Re = 100"]
fn s1_pimple2_co4_known_miss() {
    let mut fails = Vec::new();
    for scheme in BOTH {
        let o = run_case(Mesh::Uniform, 100.0, Alg::Pimple2, scheme, 4.0);
        line("S1pimple2", Mesh::Uniform, 100.0, Alg::Pimple2, scheme, 4.0, &o);
        if !o.stable {
            fails.push(format!("{scheme:?}: {}", o.why));
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
}

/// ADR 0069 X8 remedy: PIMPLE-2 with two pressure correctors is stable at Co_t = 4, Re = 100 (uniform and graded mesh, both schemes).
#[test]
fn s1_pimple2_two_correctors_co4() {
    let mut fails = Vec::new();
    for mesh in [Mesh::Uniform, Mesh::Graded] {
        for scheme in BOTH {
            let o = run_case_with(mesh, 100.0, Alg::Pimple2, scheme, 4.0, &|st| st.n_correctors = 2);
            line("S1pimple2x2", mesh, 100.0, Alg::Pimple2, scheme, 4.0, &o);
            if !o.stable {
                fails.push(format!("{mesh:?} {scheme:?}: {}", o.why));
            }
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
}

macro_rules! required_test {
    ($name:ident, $mesh:expr, $alg:expr) => {
        #[test]
        #[ignore = "heavy (ADR 0069 registered required set)"]
        fn $name() {
            let fails = run_set(
                stringify!($name),
                $mesh,
                $alg,
                &[100.0, 1000.0],
                &BOTH,
                &required_co,
            );
            assert!(fails.is_empty(), "{fails:#?}");
        }
    };
}

required_test!(s2_required_piso_uniform, Mesh::Uniform, Alg::Piso);
required_test!(s2_required_piso_graded, Mesh::Graded, Alg::Piso);
required_test!(s2_required_pimple2_uniform_known_miss, Mesh::Uniform, Alg::Pimple2);
required_test!(s2_required_pimple2_graded_known_miss, Mesh::Graded, Alg::Pimple2);
required_test!(s2_required_pimple4_uniform, Mesh::Uniform, Alg::Pimple4);
required_test!(s2_required_pimple4_graded, Mesh::Graded, Alg::Pimple4);

/// Report-only scan above the required range (never fails).
#[test]
#[ignore = "heavy report-only scan (ADR 0069)"]
fn s3_scan_above_required() {
    for mesh in [Mesh::Uniform, Mesh::Graded] {
        for alg in [Alg::Piso, Alg::Pimple2, Alg::Pimple4] {
            let _ = run_set("S3scan", mesh, alg, &[100.0, 1000.0], &BOTH, &scan_co);
        }
    }
}

/// X8 (ADR 0069 Amendment 2): PIMPLE-2 at Co_t = 4, Re = 100 with (a) two pressure correctors, (b) three outer iterations. Prints only.
#[test]
#[ignore = "diagnostic X8 of ADR 0069 (prints only)"]
fn x8_pimple2_variants() {
    let variants: [(&str, Tweak); 2] = [
        ("a n_correctors=2", &|st: &mut PimpleSettings| st.n_correctors = 2),
        ("b n_outer=3", &|st: &mut PimpleSettings| st.n_outer = 3),
    ];
    for (name, tweak) in variants {
        for mesh in [Mesh::Uniform, Mesh::Graded] {
            for scheme in BOTH {
                let o = run_case_with(mesh, 100.0, Alg::Pimple2, scheme, 4.0, tweak);
                line(&format!("X8 {name}"), mesh, 100.0, Alg::Pimple2, scheme, 4.0, &o);
            }
        }
    }
}
