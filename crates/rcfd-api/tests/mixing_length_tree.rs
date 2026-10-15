//! ADR 0026 ML3 to ML6: the mixing-length closure through the settings tree on the periodic channel at Re_tau = 395
//! (half height 1, nu = 1/395, body acceleration 1, so u_tau = 1 exactly by the momentum balance).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_turb::{MixingLengthModel, MixingProfile, ModelContext, TurbulenceModel};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

const NU: f64 = 1.0 / 395.0;
const NY: usize = 80;
const LAYERS: usize = 30;
const GROWTH: f64 = 1.1;

/// First cell height 1/395 (the centre sits at y+ = 0.5), 30 layers of growth 1.1, 80 cells in y.
const CH: &str = r#"
title = "channel Re_tau 395 with the mixing length"
[mesh]
generator = "box_2d"
nx = 4
ny = 80
lx = 1.0
ly = 2.0
periodic_x = true
wall_layers = 30
first_layer_height = 0.002531645569620253
layer_growth = 1.1
[physics]
nu = 0.002531645569620253
body_force = [1.0, 0.0, 0.0]
[initial]
velocity = [15.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
alpha_u = 0.9
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
[turbulence]
model = "mixing_length"
length_scale = 1.0
wall_damping = "van_driest"
"#;

fn case(edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(CH).unwrap();
    for (p, v) in edits {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    c
}

struct Profile {
    /// Row centres of the lower half (distance from the wall).
    y: Vec<f64>,
    /// Row heights.
    dy: Vec<f64>,
    /// Row velocities.
    u: Vec<f64>,
    converged: bool,
    its: usize,
    max_du: f64,
}

fn rows(s: &Simulation) -> Profile {
    let faces = axis_coords(
        NY,
        2.0,
        Grading::WallLayers {
            first: NU,
            growth: GROWTH,
            layers: LAYERS,
            both: true,
        },
    )
    .unwrap();
    let ux = s.field("ux").unwrap();
    let d = s.field("wall_distance").unwrap();
    let mut y = Vec::new();
    let mut dy = Vec::new();
    let mut u = Vec::new();
    for k in 0..NY / 2 {
        let yc = 0.5 * (faces[k] + faces[k + 1]);
        let (mut sum, mut n) = (0.0, 0);
        for (c, &dc) in d.iter().enumerate() {
            if (dc - yc).abs() <= 1e-9 * yc {
                sum += ux[c];
                n += 1;
            }
        }
        assert_eq!(n, 8, "row {k}: expected 4 cells in each half");
        y.push(yc);
        dy.push(faces[k + 1] - faces[k]);
        u.push(sum / n as f64);
    }
    Profile {
        y,
        dy,
        u,
        converged: false,
        its: 0,
        max_du: 0.0,
    }
}

/// Iterate in chunks of 50 until the largest velocity change per iteration over a chunk is at most 1e-10 (or 8000 iterations).
fn converge(c: &Case) -> (Simulation, Profile) {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    let mut prev = s.field("ux").unwrap();
    let (mut its, mut max_du, mut ok) = (0, f64::INFINITY, false);
    while its < 8000 {
        for _ in 0..50 {
            s.step();
        }
        its += 50;
        let now = s.field("ux").unwrap();
        max_du = prev.iter().zip(&now).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs())) / 50.0;
        prev = now;
        if max_du <= 1e-10 {
            ok = true;
            break;
        }
    }
    let mut p = rows(&s);
    p.converged = ok;
    p.its = its;
    p.max_du = max_du;
    (s, p)
}

fn run_cached(
    slot: &'static OnceLock<Mutex<Profile>>,
    edits: &[(&str, Value)],
) -> std::sync::MutexGuard<'static, Profile> {
    slot.get_or_init(|| Mutex::new(converge(&case(edits)).1))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

static LINEAR: OnceLock<Mutex<Profile>> = OnceLock::new();
static NIK: OnceLock<Mutex<Profile>> = OnceLock::new();

/// Mixing length of the linear profile with van Driest damping at distance `d`, wall shear velocity 1.
fn length_linear(d: f64) -> f64 {
    let f = 1.0 - (-(d / NU) / 26.0).exp();
    (0.41 * d * f).min(0.09)
}

/// Velocity gradient of the 1D reduction at distance `d`: l^2 U'^2 + nu U' = 1 - d/h.
fn gradient(d: f64, l: f64) -> f64 {
    let t = (1.0 - d).max(0.0);
    2.0 * t / (NU + (NU * NU + 4.0 * l * l * t).sqrt())
}

fn simpson(f: &dyn Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let h = (b - a) / n as f64;
    let mut s = f(a) + f(b);
    for i in 1..n {
        s += f(a + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    s * h / 3.0
}

/// U+ at y+ by linear interpolation of the row values.
fn u_plus_at(p: &Profile, y_plus: f64) -> f64 {
    let y = y_plus * NU;
    let k = p.y.iter().position(|&v| v >= y).expect("target above the rows");
    let (y0, y1, u0, u1) = (p.y[k - 1], p.y[k], p.u[k - 1], p.u[k]);
    u0 + (u1 - u0) * (y - y0) / (y1 - y0)
}

fn bulk(p: &Profile) -> f64 {
    p.u.iter().zip(&p.dy).map(|(u, d)| u * d).sum::<f64>() / p.dy.iter().sum::<f64>()
}

fn dns_checks(label: &str, p: &Profile) -> Vec<(String, f64, f64)> {
    let mut v = Vec::new();
    for (yp, dns) in [
        (30.0, 13.493),
        (50.0, 14.856),
        (100.0, 16.534),
        (150.0, 17.646),
        (300.0, 19.745),
    ] {
        v.push((format!("U+(y+={yp})"), u_plus_at(p, yp), dns));
    }
    v.push(("centreline".to_string(), *p.u.last().unwrap(), 20.133));
    v.push(("bulk".to_string(), bulk(p), 17.545));
    for (yp, dns) in [(5.0, 4.82), (10.0, 8.44), (15.0, 10.657), (20.0, 12.007)] {
        println!("{label} info only: U+(y+={yp}) = {:.4} (DNS {dns})", u_plus_at(p, yp));
    }
    v
}

fn report(label: &str, p: &Profile) -> f64 {
    println!(
        "{label}: converged {} after {} iterations, max du per iteration {:.2e}",
        p.converged, p.its, p.max_du
    );
    let mut worst = 0.0_f64;
    for (name, got, dns) in dns_checks(label, p) {
        let e = (got / dns - 1.0).abs();
        worst = worst.max(e);
        println!(
            "{label} {name}: {got:.4} vs DNS {dns} ({:+.2}%)",
            100.0 * (got / dns - 1.0)
        );
    }
    worst
}

#[test]
fn ml3_matches_the_one_dimensional_reduction() {
    let p = run_cached(&LINEAR, &[]);
    assert!(
        p.converged,
        "ML3 NOT MET: no convergence in {} iterations (max du {:.2e})",
        p.its, p.max_du
    );
    let mut worst = 0.0_f64;
    let mut n = 0;
    for (&y, &u) in p.y.iter().zip(&p.u) {
        let reference = simpson(&|t| gradient(t, length_linear(t)), 0.0, y, 20000);
        if reference > 0.5 {
            let e = (u / reference - 1.0).abs();
            worst = worst.max(e);
            n += 1;
        }
    }
    println!("ML3: max relative error against the 1D reduction {worst:.3e} over {n} rows (limit 0.02)");
    assert!(worst <= 0.02);
}

#[test]
fn ml4_linear_profile_against_dns() {
    let p = run_cached(&LINEAR, &[]);
    assert!(
        p.converged,
        "ML4 NOT MET: no convergence in {} iterations (max du {:.2e})",
        p.its, p.max_du
    );
    let w = report("ML4", &p);
    println!("ML4 worst relative deviation {w:.4} (limit 0.10)");
    assert!(w <= 0.10);
}

#[test]
fn ml5_nikuradse_profile_against_dns() {
    let p = run_cached(&NIK, &[("turbulence.mixing_profile", json!("nikuradse"))]);
    assert!(
        p.converged,
        "ML5 NOT MET: no convergence in {} iterations (max du {:.2e})",
        p.its, p.max_du
    );
    let w = report("ML5", &p);
    println!("ML5 worst relative deviation {w:.4} (limit 0.10)");
    assert!(w <= 0.10);
}

// ------------------------------------------------------------------ ML6: the tree run equals the library run

fn register_library_twin(key: &'static str, kappa: f64, l_max: f64, profile: MixingProfile, damping: bool) {
    rcfd_turb::register_model(
        key,
        "mixing length built from the library (test twin)",
        Arc::new(move |ctx: &ModelContext| {
            let mut c = ctx.clone();
            c.length_scale = 1.0;
            c.mixing_profile = profile;
            c.wall_damping = damping;
            c.time_accurate = false;
            let mut m = MixingLengthModel::new(&c)?;
            m.set_coefficient("kappa", kappa).unwrap();
            m.set_coefficient("l_max_ratio", l_max).unwrap();
            Ok(Box::new(m) as Box<dyn TurbulenceModel>)
        }),
    );
}

/// Name, tree edits, kappa, l_max_ratio, profile, damping of one ML6 variant.
type Variant<'a> = (&'a str, Vec<(&'a str, Value)>, f64, f64, MixingProfile, bool);

#[test]
fn ml6_tree_equals_library_and_options_have_teeth() {
    let variants: [Variant; 5] = [
        ("default", vec![], 0.41, 0.09, MixingProfile::Linear, true),
        (
            "nikuradse",
            vec![("turbulence.mixing_profile", json!("nikuradse"))],
            0.41,
            0.09,
            MixingProfile::Nikuradse,
            true,
        ),
        (
            "kappa",
            vec![("turbulence.coefficients.mixing_length.kappa", json!(0.38))],
            0.38,
            0.09,
            MixingProfile::Linear,
            true,
        ),
        (
            "lmax",
            vec![("turbulence.coefficients.mixing_length.l_max_ratio", json!(0.07))],
            0.41,
            0.07,
            MixingProfile::Linear,
            true,
        ),
        (
            "undamped",
            vec![("turbulence.wall_damping", json!("none"))],
            0.41,
            0.09,
            MixingProfile::Linear,
            false,
        ),
    ];
    let run = |c: &Case| -> Vec<f64> {
        let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
        for _ in 0..60 {
            s.step();
        }
        s.field("ux").unwrap()
    };
    let base = run(&case(&[]));
    for (name, edits, kappa, l_max, profile, damping) in variants {
        let tree = run(&case(&edits));
        let key: &'static str = Box::leak(format!("ml_twin_{name}").into_boxed_str());
        register_library_twin(key, kappa, l_max, profile, damping);
        let mut c = case(&[]);
        c.set("turbulence.model", json!("plugin")).unwrap();
        c.set("turbulence.length_scale", json!(0.0)).unwrap();
        c.set("turbulence.wall_damping", json!("none")).unwrap();
        c.set("turbulence.plugin", json!(key)).unwrap();
        let lib = run(&c);
        let d = tree.iter().zip(&lib).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        let teeth = tree.iter().zip(&base).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        println!("ML6 {name}: tree against library {d:.3e} (limit 1e-12), change against the default {teeth:.3e} (must exceed 1e-6)");
        assert!(d <= 1e-12, "{name}");
        if name != "default" {
            assert!(teeth > 1e-6, "{name}");
        }
    }
}
