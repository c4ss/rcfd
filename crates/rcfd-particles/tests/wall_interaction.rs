//! F-254 (ADR 0194): particle wall interaction, erosion and rough-wall rebound, in the library. Registered checks A (the
//! closed forms), B (the erosion acceptance), C (the rough-wall acceptance) and E1 (the record R3 of a tracker run with
//! the wall film of ADR 0192, made on the base code main 54ec840 before any code of F-254). The limits are those of
//! ADR 0194, fixed before this code ran.

use rcfd_core::hash::Fnv64;
use rcfd_core::vec3::{cross, norm, scale, Vec3};
use rcfd_mesh::edit::{transform_mesh, Affine};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::f64::consts::PI;
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// DNV RP O501 eq. 7.2 as printed, `sum_{i=1..8} (-1)^(i+1) A_i (alpha pi/180)^i` with `alpha` in degrees and the
/// constants of Table 7-1 typed here again.
fn f_dnv_deg(alpha_deg: f64) -> f64 {
    let a = [9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170];
    let x = alpha_deg * PI / 180.0;
    (0..8)
        .map(|i| {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            s * a[i] * x.powi(i as i32 + 1)
        })
        .sum()
}

/// A1: the angle function against eq. 7.2.
#[test]
fn a1_dnv_angle_function_against_the_printed_sum() {
    let mut worst = 0.0f64;
    for deg in 0..=90 {
        let d = deg as f64;
        worst = worst.max((dnv_angle_function(d.to_radians()) - f_dnv_deg(d)).abs());
    }
    let mut lowest = f64::INFINITY;
    for k in 0..=900 {
        lowest = lowest.min(dnv_angle_function((k as f64 * 0.1).to_radians()));
    }
    println!(
        "A1: worst difference {worst:e}; F(0) = {}; smallest F on the 0.1-degree grid {lowest:e}; F(36.4) {}, F(90) {}",
        dnv_angle_function(0.0),
        dnv_angle_function(36.4f64.to_radians()),
        dnv_angle_function(PI / 2.0)
    );
    assert!(worst <= 1e-11, "A1");
    assert_eq!(dnv_angle_function(0.0), 0.0);
    assert!(lowest >= 0.0, "A1: F < 0");
}

/// A2: the impact angle on a face of normal unit(1, 2, 3).
#[test]
fn a2_impact_angle_on_a_tilted_face() {
    let n = scale([1.0, 2.0, 3.0], 1.0 / 14f64.sqrt());
    let c = cross(n, [1.0, 0.0, 0.0]);
    let t = scale(c, 1.0 / norm(c));
    let u = 7.3;
    let mut worst = 0.0f64;
    for k in 0..=18 {
        let a = (5.0 * k as f64).to_radians();
        let v = [0, 1, 2].map(|i| u * (a.cos() * t[i] + a.sin() * n[i]));
        worst = worst.max((impact_angle(v, n) - a).abs());
    }
    println!("A2: worst angle error {worst:e} rad");
    assert!(worst <= 1e-13, "A2");
}

/// Geometry P of ADR 0194: the plate `ymin` of `box_mesh([6, 4, 2], [0.06, 0.04, 0.02])`.
fn plate() -> Arc<Mesh> {
    Arc::new(box_mesh([6, 4, 2], [0.06, 0.04, 0.02], [Grading::Uniform; 3]).unwrap())
}

/// A tracker on P: ballistic parcels (no drag, no gravity), the plate reflecting with `e`, every other patch `Escape`.
fn plate_tracker(e: f64) -> Tracker {
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut overrides: Vec<(String, WallAction)> = ["xmin", "xmax", "ymax", "zmin", "zmax"]
        .iter()
        .map(|p| (p.to_string(), WallAction::Escape))
        .collect();
    overrides.push(("ymin".into(), WallAction::Reflect(e)));
    Tracker::new(plate(), opts, &overrides).unwrap()
}

/// A stream of P: a point injector `L = 0.03` m before the target `x_t` on the plate, at `alpha` to the plate, speed `u`.
#[allow(clippy::too_many_arguments)]
fn add_stream(
    tr: &mut Tracker,
    name: &str,
    alpha_deg: f64,
    target: Vec3,
    u: f64,
    d: f64,
    rho: f64,
    rate: f64,
    t_end: f64,
) {
    let a = alpha_deg.to_radians();
    let dir = [a.cos(), -a.sin(), 0.0];
    let l = 0.03;
    tr.add_injector(InjectorSpec {
        name: name.into(),
        kind: InjKind::Point([target[0] - l * dir[0], target[1] - l * dir[1], target[2]]),
        rate,
        t_start: 0.0,
        t_end,
        velocity: VelMode::Given(scale(dir, u)),
        dist: SizeDist::Constant(d),
        density: rho,
        parcel_count: 1.0,
        seed: 7,
    })
    .unwrap();
}

/// Steady carrier at rest.
fn advance_to(tr: &mut Tracker, steps: usize, dt: f64) {
    let u = vec![[0.0; 3]; tr.locator().mesh().n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 1.2,
        mu: 1.8e-5,
    };
    for _ in 0..steps {
        tr.advance(&flow, dt).unwrap();
    }
}

/// The streams of B: (angle [deg], target, rate [1/s]).
const B_STREAMS: [(f64, Vec3, f64); 3] = [
    (15.0, [0.045, 0.0, 0.005], 1000.0),
    (35.0, [0.035, 0.0, 0.015], 2000.0),
    (90.0, [0.015, 0.0, 0.005], 500.0),
];
const B_U: f64 = 20.0;
const B_D: f64 = 2e-4;
const B_RHO: f64 = 2650.0;
const B_K: f64 = 2.0e-9;
const B_N: f64 = 2.6;
const B_RHO_T: f64 = 7800.0;

/// The run of B, with or without erosion on the plate.
fn b_run(erosion: bool) -> Tracker {
    let mut tr = plate_tracker(1.0);
    if erosion {
        tr.set_erosion(ErosionSpec {
            patches: vec!["ymin".into()],
            k: B_K,
            n: B_N,
            density: B_RHO_T,
        })
        .unwrap();
    }
    for (i, (a, x, r)) in B_STREAMS.iter().enumerate() {
        add_stream(&mut tr, &format!("s{}", i + 1), *a, *x, B_U, B_D, B_RHO, *r, 0.5);
    }
    advance_to(&mut tr, 600, 1e-3);
    tr
}

/// The erosion face whose centre is the point `x` of the plate.
fn face_at(er: &Erosion, x: Vec3) -> usize {
    (0..er.len())
        .find(|&k| (0..3).all(|i| (er.centre(k)[i] - x[i]).abs() < 1e-9))
        .expect("a plate face centred at the target")
}

/// B1, B2 (the acceptance): the reported erosion rate of each target face against `G K U^n F(alpha)`.
#[test]
fn b1_b2_erosion_rate_equals_the_impact_flux_expression() {
    let tr = b_run(true);
    let er = tr.erosion().unwrap();
    let t = tr.time;
    let m = B_RHO * PI * B_D.powi(3) / 6.0;
    let area = 1e-4;
    assert_eq!(tr.count(State::Escaped), tr.injected(), "every parcel leaves the box");
    let mut targets = Vec::new();
    let (mut worst, mut worst_mass, mut worst_depth) = (0.0f64, 0.0f64, 0.0f64);
    for (a, x, r) in B_STREAMS {
        let k = face_at(er, x);
        targets.push(k);
        let n_k = (r * 0.5).floor();
        let g = n_k * m / (area * t);
        let expected = g * B_K * B_U.powf(B_N) * f_dnv_deg(a);
        let e = rel(er.rate(k, t), expected);
        println!(
            "B1: {a} deg, face {k}: rate {:e} kg/(m^2 s), closed form {expected:e}, relative {e:e}; hits {} (N {n_k})",
            er.rate(k, t),
            er.hits()[k]
        );
        worst = worst.max(e);
        assert_eq!(er.hits()[k] as f64, n_k, "B2 hits");
        worst_mass = worst_mass.max(rel(er.impact_mass()[k], n_k * m));
        worst_depth = worst_depth.max(rel(er.depth_rate(k, t), er.rate(k, t) / B_RHO_T));
    }
    let others: Vec<usize> = (0..er.len()).filter(|k| !targets.contains(k)).collect();
    println!(
        "B2: impact mass {worst_mass:e}, depth rate {worst_depth:e}; {} other faces, hits {}, eroded {:e}",
        others.len(),
        others.iter().map(|&k| er.hits()[k]).sum::<u64>(),
        others.iter().map(|&k| er.eroded()[k]).sum::<f64>()
    );
    assert!(worst <= 1e-10, "B1");
    assert_eq!(others.len(), 9);
    assert!(
        others.iter().all(|&k| er.hits()[k] == 0 && er.eroded()[k] == 0.0),
        "B2 other faces"
    );
    assert!(worst_mass <= 1e-12 && worst_depth <= 1e-15, "B2");
}

/// B3: erosion is an observation: the parcels of the run without it are the same, bit for bit.
#[test]
fn b3_erosion_does_not_change_the_parcels() {
    let with = b_run(true).fingerprint();
    let without = b_run(false).fingerprint();
    println!(
        "B3: fingerprint words {} without, {} with erosion",
        without.len(),
        with.len()
    );
    assert!(with.len() > without.len());
    assert!(with[..without.len()] == without[..], "B3");
}

/// The standard normal distribution function by composite Simpson quadrature of the density on a table of step 1e-3,
/// completed by one Simpson panel (ADR 0194 C).
struct NormalCdf {
    h: f64,
    table: Vec<f64>,
}

fn density(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

fn simpson(a: f64, b: f64) -> f64 {
    (b - a) / 6.0 * (density(a) + 4.0 * density(0.5 * (a + b)) + density(b))
}

impl NormalCdf {
    fn new() -> NormalCdf {
        let h = 1e-3;
        let mut table = vec![0.0];
        for j in 0..20_000 {
            let a = j as f64 * h;
            table.push(table[j] + simpson(a, a + h));
        }
        NormalCdf { h, table }
    }

    /// `Phi(x)`.
    fn cdf(&self, x: f64) -> f64 {
        let y = x.abs();
        let j = ((y / self.h) as usize).min(self.table.len() - 1);
        let half = self.table[j] + simpson(j as f64 * self.h, y);
        if x >= 0.0 {
            0.5 + half
        } else {
            0.5 - half
        }
    }
}

/// `K_0.01`: the root of `2 sum_{k=1..100} (-1)^(k-1) exp(-2 k^2 x^2) = 0.01` (Kolmogorov distribution).
fn k_001() -> f64 {
    let f = |x: f64| {
        2.0 * (1..=100)
            .map(|k| {
                let s = if k % 2 == 1 { 1.0 } else { -1.0 };
                s * (-2.0 * (k * k) as f64 * x * x).exp()
            })
            .sum::<f64>()
            - 0.01
    };
    let (mut lo, mut hi) = (1.0, 3.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The Kolmogorov distance of a sample from the distribution function `cdf`.
fn ks_distance(sample: &[f64], cdf: impl Fn(f64) -> f64) -> f64 {
    let mut s = sample.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len() as f64;
    let mut d = 0.0f64;
    for (i, &x) in s.iter().enumerate() {
        let f = cdf(x);
        d = d.max(f - i as f64 / n).max((i + 1) as f64 / n - f);
    }
    d
}

const C_U: f64 = 10.0;
const C_STD_DEG: f64 = 8.0;
const C_SEED: u64 = 254;
const C_TARGET: Vec3 = [0.035, 0.0, 0.005];

/// The run of C: one stream at `alpha` on the rough plate (restitution `e`) from 0 to 1 s at `rate`, 1100 steps.
fn c_run(alpha_deg: f64, e: f64, rate: f64) -> Tracker {
    let mut tr = plate_tracker(e);
    tr.set_rough_wall(RoughSpec {
        patches: vec!["ymin".into()],
        angle_std: C_STD_DEG.to_radians(),
        seed: C_SEED,
    })
    .unwrap();
    add_stream(&mut tr, "s", alpha_deg, C_TARGET, C_U, 1e-4, 2500.0, rate, 1.0);
    advance_to(&mut tr, 1100, 1e-3);
    tr
}

/// Rebound angle `atan2(-v . n, v . t)` against the plate (`n = -y`, `t = +x`) of each parcel's final velocity.
fn rebound_angles(tr: &Tracker) -> Vec<f64> {
    tr.particles.iter().map(|p| p.v[1].atan2(p.v[0])).collect()
}

/// C1, C2, C3 (the acceptance): the rebound angles at 10, 40 and 75 degrees against the distribution of W5 (`e = 1`).
#[test]
fn c1_c2_c3_rough_rebound_angles_match_the_prescribed_distribution() {
    let phi = NormalCdf::new();
    let crit = k_001() / 20_000f64.sqrt();
    let s = C_STD_DEG.to_radians();
    let mut fails = Vec::new();
    for a_deg in [10.0, 40.0, 75.0] {
        let tr = c_run(a_deg, 1.0, 20_000.0);
        let n = tr.injected();
        assert_eq!(n, 20_000);
        assert_eq!(tr.count(State::Escaped), n, "C1: every parcel escapes");
        let a: f64 = f64::to_radians(a_deg);
        let (lo, hi) = (phi.cdf(-a / (2.0 * s)), phi.cdf((PI - a) / (2.0 * s)));
        let cdf = |th: f64| ((phi.cdf((th - a) / (2.0 * s)) - lo) / (hi - lo)).clamp(0.0, 1.0);
        let sample = rebound_angles(&tr);
        let d = ks_distance(&sample, cdf);
        let c = tr.rough_wall().unwrap().counts();
        let (mut w_z, mut w_u) = (0.0f64, 0.0f64);
        for p in &tr.particles {
            w_z = w_z.max(p.v[2].abs() / norm(p.v));
            w_u = w_u.max(rel(norm(p.v), C_U));
        }
        println!(
            "C1: {a_deg} deg: n {n}, Kolmogorov distance {d:.6} (critical {crit:.6}); accepted fraction {:.6} (prescribed {:.6}); C2: out of plane {w_z:e}, speed {w_u:e}, counts {c:?}",
            c.collisions as f64 / (c.collisions + c.redraws) as f64,
            hi - lo
        );
        if d >= crit {
            fails.push(a_deg);
        }
        assert!(w_z <= 1e-12 && w_u <= 1e-12, "C2");
        assert_eq!(
            (c.collisions, c.fallbacks, c.normal_incidence, c.grazing),
            (n as u64, 0, 0, 0),
            "C2 counts"
        );
        if a_deg == 10.0 {
            let d0 = ks_distance(&sample, |th: f64| phi.cdf((th - a) / (2.0 * s)));
            println!("C3: 10 deg: distance from the untruncated map {d0:.6} (critical {crit:.6})");
            assert!(d0 > crit, "C3");
        }
    }
    assert!(
        fails.is_empty(),
        "C1: Kolmogorov distance above the critical value at {fails:?} deg"
    );
}

/// `alpha_r(gamma)` of W5 and its inverse by bisection on `(-alpha, pi/2)`.
fn alpha_r(alpha: f64, e: f64, gamma: f64) -> f64 {
    let b = alpha + gamma;
    (e * b.sin()).atan2(b.cos()) + gamma
}

fn gamma_of(alpha: f64, e: f64, theta: f64) -> f64 {
    let (mut lo, mut hi) = (-alpha, PI / 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if alpha_r(alpha, e, mid) < theta {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// C4: with `e = 0.8`, each rebound speed is `U sqrt(cos^2 beta + e^2 sin^2 beta)` at the roughness angle of its angle.
#[test]
fn c4_rough_rebound_with_restitution() {
    let (a_deg, e) = (40.0, 0.8);
    let a: f64 = f64::to_radians(a_deg);
    let tr = c_run(a_deg, e, 2000.0);
    assert_eq!(tr.count(State::Escaped), tr.injected());
    let g_lo = gamma_of(a, e, 0.0);
    let g_hi = if alpha_r(a, e, PI / 2.0) > PI {
        gamma_of(a, e, PI)
    } else {
        PI / 2.0
    };
    let (mut worst, mut outside) = (0.0f64, 0usize);
    for p in &tr.particles {
        let th = p.v[1].atan2(p.v[0]);
        let g = gamma_of(a, e, th);
        if !(g > g_lo && g < g_hi) {
            outside += 1;
        }
        let b = a + g;
        let speed = C_U * (b.cos().powi(2) + e * e * b.sin().powi(2)).sqrt();
        worst = worst.max(rel(norm(p.v), speed));
    }
    println!(
        "C4: {} parcels, gamma in ({:.4}, {:.4}) deg, outside {outside}, worst speed error {worst:e}",
        tr.injected(),
        g_lo.to_degrees(),
        g_hi.to_degrees()
    );
    assert_eq!(outside, 0, "C4");
    assert!(worst <= 1e-10, "C4");
}

/// C5: at normal incidence, the polar angle of the rebound and the azimuth of its tangential part.
#[test]
fn c5_rough_rebound_at_normal_incidence() {
    let phi = NormalCdf::new();
    let crit = k_001() / 20_000f64.sqrt();
    let s = C_STD_DEG.to_radians();
    let tr = c_run(90.0, 1.0, 20_000.0);
    let n = tr.injected();
    assert_eq!(tr.count(State::Escaped), n);
    let psi: Vec<f64> = tr
        .particles
        .iter()
        .map(|p| (p.v[0] * p.v[0] + p.v[2] * p.v[2]).sqrt().atan2(p.v[1]))
        .collect();
    let az: Vec<f64> = tr
        .particles
        .iter()
        .map(|p| p.v[2].atan2(p.v[0]).rem_euclid(2.0 * PI))
        .collect();
    let z = phi.cdf(PI / (4.0 * s)) - phi.cdf(-PI / (4.0 * s));
    let d_psi = ks_distance(&psi, |x| {
        ((phi.cdf(x / (2.0 * s)) - phi.cdf(-x / (2.0 * s))) / z).clamp(0.0, 1.0)
    });
    let d_az = ks_distance(&az, |x| (x / (2.0 * PI)).clamp(0.0, 1.0));
    let c = tr.rough_wall().unwrap().counts();
    println!(
        "C5: n {n}, polar angle distance {d_psi:.6}, azimuth distance {d_az:.6} (critical {crit:.6}); counts {c:?}"
    );
    assert_eq!(c.normal_incidence, n as u64, "C5 counts");
    assert!(d_psi < crit && d_az < crit, "C5");
}

/// ADR 0194 R3: FNV-1a 64 of the R3 scenario's fingerprint on main 54ec840 (Windows; the analytic integrator calls `exp`,
/// whose last bits may differ elsewhere, ADR 0137).
const R3: u64 = 0xfe94_f34a_c9fc_5dbd;

/// The incline of ADR 0192 geometry G: `box_mesh([10, 4, 2], [0.05, 0.02, 0.01])` rotated by 30 degrees about y.
fn incline() -> Arc<Mesh> {
    let m = box_mesh([10, 4, 2], [0.05, 0.02, 0.01], [Grading::Uniform; 3]).unwrap();
    let a = Affine::rotation([0.0, 1.0, 0.0], 30f64.to_radians(), [0.0; 3]).unwrap();
    Arc::new(transform_mesh(&m, &a).unwrap())
}

/// Local (box) coordinates to world.
fn to_world(l: Vec3) -> Vec3 {
    let (s, c) = 30f64.to_radians().sin_cos();
    [c * l[0] + s * l[2], l[1], -s * l[0] + c * l[2]]
}

/// The R3 scenario: the wall film of ADR 0192 on the incline `zmin`, fed by a falling stream, and a second stream thrown
/// sideways at the reflecting walls (restitution 0.7, `xmin` sticks) before it lands on the film; Schiller-Naumann drag,
/// RK2, 300 steps of 0.01 s.
fn r3_hash() -> u64 {
    let mesh = incline();
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Rk2,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.7),
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[("xmin".to_string(), WallAction::Stick)]).unwrap();
    tr.set_film(FilmSpec {
        patches: vec!["zmin".into()],
        density: 1000.0,
        viscosity: 1e-3,
        gravity: [0.0, 0.0, -9.81],
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "fall".into(),
        kind: InjKind::Point(to_world([1.3e-3, 7.5e-3, 2e-3])),
        rate: 730.0,
        t_start: 0.0,
        t_end: 3.0,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(1e-4),
        density: 1000.0,
        parcel_count: 1.0,
        seed: 3,
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "throw".into(),
        kind: InjKind::Point(to_world([2e-2, 1e-2, 6e-3])),
        rate: 300.0,
        t_start: 0.1,
        t_end: 2.0,
        velocity: VelMode::Given(to_world([-1.5, 0.6, 0.3])),
        dist: SizeDist::Uniform {
            d_min: 5e-5,
            d_max: 2e-4,
        },
        density: 1000.0,
        parcel_count: 2.0,
        seed: 4,
    })
    .unwrap();
    let n = mesh.n_cells();
    let u: Vec<Vec3> = mesh
        .cell_centres()
        .iter()
        .map(|c| [0.05 + 2.0 * c[2], 0.0, 0.0])
        .collect();
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 1.2,
        mu: 1.8e-5,
    };
    assert_eq!(u.len(), n);
    for _ in 0..300 {
        tr.advance(&flow, 0.01).unwrap();
    }
    let counts = [
        State::Active,
        State::Deposited,
        State::Escaped,
        State::Stuck,
        State::Film,
    ]
    .map(|s| tr.count(s));
    println!(
        "R3 set-up: injected {}, active/deposited/escaped/stuck/film {counts:?}, film held {:e}, drained {:e}",
        tr.injected(),
        tr.film().unwrap().held(),
        tr.film().unwrap().drained()
    );
    assert!(counts[4] > 0 && counts[1] > 0, "R3 set-up: {counts:?}");
    let mut h = Fnv64::new();
    for b in tr.fingerprint() {
        h.write_u64(b);
    }
    h.finish()
}

/// ADR 0194 R3 (check E1): a tracker with the wall film and no F-254 model gives the fingerprint recorded on the base
/// code. Compared on Windows, printed elsewhere (ADR 0137). `RCFD_F254_RECORD=1` prints the value only.
#[test]
fn e1_film_tracker_matches_the_base_record() {
    let h = r3_hash();
    println!("R3 {h:016x}");
    if std::env::var("RCFD_F254_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R3, "R3 {h:016x} differs from the base record {R3:016x}");
}
