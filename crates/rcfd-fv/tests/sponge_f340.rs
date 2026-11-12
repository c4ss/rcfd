//! F-340 (ADR 0248): the matched sponge layer with a frequency shift, a corner of two layers and a damping along the
//! layer. Clause 1: set-up D of ADR 0237 (D..s, D..d). Clause 2: a pressure offset with a gradient along the layer (Z1).
//! Clause 3: a pulse leaving through the corner of two layers (C1). Clause 4: one isolated plane pulse at 60 degrees
//! (X60). Clause 5: the checks A0, A30b, A45b, A60b, X1, X45 of ADR 0237 with the shifted form (A..c, X..c). U, S, B: the
//! uniform states, the refusals of the solver and the bit identity with the matched form of ADR 0237. The helpers of
//! the set-ups are copies of those of `sponge_f331.rs`, which is not edited. Limits are the registered ones of ADR 0248.

use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::MsdWall;
use rcfd_fv::sponge::{rate_max_for_reflection, MatchedOptions, Sponge, SpongeForm, SpongeLayer, SpongeMemories};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed is 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude of the waves.
const EPS: f64 = 1e-4 * P0;
/// Width of the Gaussian pulses.
const W: f64 = 0.04;
/// Cell size of the reflection checks.
const DX: f64 = 0.005;
/// The layer of ADR 0237 D7: thickness, exponent, two-way attenuation at normal incidence.
const LAYER: f64 = 0.2;
const EXPONENT: f64 = 2.0;
const R0: f64 = 1e-6;
/// The limit of the acceptance of F-340.
const LIMIT: f64 = 0.01;
/// The registered frequency shift (ADR 0248 E5) [1/s].
const SHIFT: f64 = 0.25;
/// The registered damping along the layer of the configuration K2 (ADR 0248 E5).
const ALONG: f64 = 0.05;
const THOMPSON: CompBc = CompBc::NonReflecting { p: P0, sigma: 0.0 };

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

/// Superposition of exact isentropic simple waves: each part is a pressure perturbation travelling along a unit vector.
fn superpose(parts: &[(f64, [f64; 3])]) -> Prim {
    let mut u = [0.0; 3];
    let mut dp_sum = 0.0;
    for &(dp, d) in parts {
        let a = ((P0 + dp) / P0).powf((GAMMA - 1.0) / (2.0 * GAMMA));
        let v = 2.0 * (a - 1.0) / (GAMMA - 1.0);
        for k in 0..3 {
            u[k] += v * d[k];
        }
        dp_sum += dp;
    }
    Prim {
        rho: ((P0 + dp_sum) / P0).powf(1.0 / GAMMA),
        u,
        p: P0 + dp_sum,
    }
}

fn gauss(xi: f64) -> f64 {
    (-(xi / W).powi(2)).exp()
}

fn solver(nx: usize, ny: usize, lx: f64, ly: f64, init: impl Fn([f64; 3]) -> Prim) -> CompressibleSolver {
    let mesh = Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    CompressibleSolver::new(geom, AIR, CompSettings::default(), init).unwrap()
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// The layer of ADR 0237 D7 with its front at `front` along the axis `axis` (0: normal `+x`, 1: normal `+y`).
fn registered_layer(axis: usize, front: f64) -> SpongeLayer {
    let mut origin = [0.0; 3];
    let mut normal = [0.0; 3];
    origin[axis] = front;
    normal[axis] = 1.0;
    SpongeLayer {
        origin,
        normal,
        thickness: LAYER,
        rate_max: rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap(),
        exponent: EXPONENT,
    }
}

/// The sponge of a run: the matched form of ADR 0237, or the shifted form with one layer.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Old,
    Shifted { shift: f64, along: f64 },
}

/// K1 of ADR 0248: the configuration of the acceptance.
const K1: Kind = Kind::Shifted {
    shift: SHIFT,
    along: 0.0,
};
/// K2 of ADR 0248: K1 with the damping along the layer.
const K2: Kind = Kind::Shifted {
    shift: SHIFT,
    along: ALONG,
};

fn apply(s: &mut CompressibleSolver, kind: Kind, x_front: f64) {
    let layer = registered_layer(0, x_front);
    let sponge = match kind {
        Kind::Old => Some(Sponge::planar(&s.geom, ambient(), &layer, true).unwrap()),
        Kind::Shifted { shift, along } => {
            Some(Sponge::planar_shifted(&s.geom, ambient(), &layer, shift, along).unwrap())
        }
    };
    s.set_sponge(sponge).unwrap();
}

/// The cells of the column with the centre `x`, sorted by `y`.
fn column(s: &CompressibleSolver, x: f64) -> Vec<usize> {
    let cc = s.geom.mesh.cell_centres();
    let mut v: Vec<usize> = (0..cc.len()).filter(|&c| (cc[c][0] - x).abs() < 0.2 * DX).collect();
    v.sort_by(|&a, &b| cc[a][1].total_cmp(&cc[b][1]));
    assert!(!v.is_empty(), "no cell at x = {x}");
    v
}

/// The cell with the centre `(x, y)`.
fn cell_at(s: &CompressibleSolver, x: f64, y: f64) -> usize {
    let cc = s.geom.mesh.cell_centres();
    let found: Vec<usize> = (0..cc.len())
        .filter(|&c| (cc[c][0] - x).abs() < 0.2 * DX && (cc[c][1] - y).abs() < 0.2 * DX)
        .collect();
    assert_eq!(found.len(), 1, "cell at ({x}, {y})");
    found[0]
}

/// The raw numbers of the measure of ADR 0237 per test run `i` and probe `k`: `num[i][k]` the largest `|p_test - p_ref|`
/// over the steps of the window, `late[i][k]` the same over the steps with `t >= t_late`, and `den[k]` the largest
/// `p_ref - p0`.
struct Raw {
    num: Vec<Vec<f64>>,
    late: Vec<Vec<f64>>,
    den: Vec<f64>,
}

impl Raw {
    /// `max_k num / max_k den` per test run (the measure of ADR 0237), strict and late.
    fn ratio(&self) -> Vec<(f64, f64)> {
        let den = self.den.iter().fold(0.0f64, |m, d| m.max(*d));
        let top = |v: &Vec<f64>| v.iter().fold(0.0f64, |m, d| m.max(*d)) / den;
        self.num.iter().zip(&self.late).map(|(n, l)| (top(n), top(l))).collect()
    }

    /// `max_k (num_k / den_k)` per test run: each probe against its own incident peak.
    fn ratio_per_probe(&self) -> Vec<f64> {
        self.num
            .iter()
            .map(|n| n.iter().zip(&self.den).fold(0.0f64, |m, (n, d)| m.max(n / d)))
            .collect()
    }
}

/// The measure of ADR 0237: the runs `tests` and the reference run advance in lockstep with the smallest `max_dt()`
/// until `t >= t_end`; the window is `t_start <= t <= t_window_end`; `probes[i]` are the probe cells of run `i`, the last
/// entry those of the reference (the same number of cells in the same order). Every run steps in its own thread.
fn reflection(
    tests: &mut [CompressibleSolver],
    reference: &mut CompressibleSolver,
    probes: &[Vec<usize>],
    t_start: f64,
    t_window_end: f64,
    t_late: f64,
    t_end: f64,
) -> Raw {
    assert_eq!(probes.len(), tests.len() + 1);
    let pr = probes.last().unwrap();
    assert!(probes.iter().all(|p| p.len() == pr.len()));
    let mut raw = Raw {
        num: vec![vec![0.0; pr.len()]; tests.len()],
        late: vec![vec![0.0; pr.len()]; tests.len()],
        den: vec![0.0; pr.len()],
    };
    let mut dt = tests.iter().map(|s| s.max_dt()).fold(reference.max_dt(), f64::min);
    while reference.time < t_end {
        let next: Vec<f64> = std::thread::scope(|scope| {
            let handles: Vec<_> = tests
                .iter_mut()
                .chain(std::iter::once(&mut *reference))
                .map(|s| {
                    scope.spawn(move || {
                        s.step(dt).unwrap();
                        s.max_dt()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        dt = next.iter().fold(f64::INFINITY, |m, d| m.min(*d));
        let t = reference.time;
        if t < t_start || t > t_window_end {
            continue;
        }
        for (k, &c) in pr.iter().enumerate() {
            let p_ref = pressure(reference, c);
            raw.den[k] = raw.den[k].max(p_ref - P0);
            for (i, s) in tests.iter().enumerate() {
                let d = (pressure(s, probes[i][k]) - p_ref).abs();
                raw.num[i][k] = raw.num[i][k].max(d);
                if t >= t_late {
                    raw.late[i][k] = raw.late[i][k].max(d);
                }
            }
        }
    }
    let den = raw.den.iter().fold(0.0f64, |m, d| m.max(*d));
    println!("  incident peak at the probes / eps = {:.5}", den / EPS);
    raw
}

// ---------------------------------------------------------------------------------------------------------------------
// Clause 5, normal incidence (fast)

/// The tube of A0 of ADR 0237: an isolated pulse at normal incidence; the runs are (sponge, boundary behind the layer).
fn tube(runs: &[(Kind, CompBc)]) -> Vec<f64> {
    let init = |x: [f64; 3]| superpose(&[(EPS * gauss(x[0] - 0.5), [1.0, 0.0, 0.0])]);
    let mut tests: Vec<CompressibleSolver> = runs
        .iter()
        .map(|&(kind, bc)| {
            let mut s = solver(240, 1, 1.2, DX, init);
            s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
            s.set_bc("xmax", bc).unwrap();
            apply(&mut s, kind, 1.0);
            s
        })
        .collect();
    let mut reference = solver(600, 1, 3.0, DX, init);
    reference.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    reference.set_bc("xmax", THOMPSON).unwrap();
    let mut probes: Vec<Vec<usize>> = tests.iter().map(|s| column(s, 0.8975)).collect();
    probes.push(column(&reference, 0.8975));
    reflection(&mut tests, &mut reference, &probes, 0.0, f64::INFINITY, 0.0, 1.6)
        .ratio()
        .iter()
        .map(|r| r.0)
        .collect()
}

/// A0c and A0sc of ADR 0248: the tube of A0 of ADR 0237 with the configuration K1, in front of the Thompson boundary
/// and in front of a rigid wall.
#[test]
fn a0c_normal_incidence_with_the_shift() {
    let r = tube(&[(K1, THOMPSON), (K1, CompBc::Slip)]);
    println!(
        "A0c K1, Thompson: R = {:.3e}; A0sc K1, Slip: R = {:.3e} (limit {LIMIT:e})",
        r[0], r[1]
    );
    assert!(r[0] <= LIMIT, "A0c {}", r[0]);
    assert!(r[1] <= LIMIT, "A0sc {}", r[1]);
}

// ---------------------------------------------------------------------------------------------------------------------
// Clause 1: set-up D of ADR 0237 (heavy)

/// The periodic, zero-mean train of set-up D (ADR 0237) in a duct of height `h`.
fn train(x: [f64; 3], theta: f64, h: f64) -> Prim {
    let (c, s) = (theta.cos(), theta.sin());
    let lam = 2.0 * h * s;
    let mean = W * std::f64::consts::PI.sqrt() / lam;
    let g = |xi: f64| {
        let r = xi - lam * (xi / lam).round();
        (-1..=1).map(|m| gauss(r - m as f64 * lam)).sum::<f64>() - mean
    };
    superpose(&[
        (EPS * g(x[0] * c + x[1] * s), [c, s, 0.0]),
        (EPS * g(x[0] * c - x[1] * s), [c, -s, 0.0]),
    ])
}

/// One row of the table of set-up D of ADR 0237.
struct Duct {
    theta_deg: f64,
    h: f64,
    ny: usize,
    t_s: f64,
    t_end: f64,
    x_i: f64,
    nx: usize,
    nx_ref: usize,
}

const D30: Duct = Duct {
    theta_deg: 30.0,
    h: 0.64,
    ny: 128,
    t_s: 0.84,
    t_end: 1.53,
    x_i: 1.75,
    nx: 390,
    nx_ref: 660,
};
const D45: Duct = Duct {
    theta_deg: 45.0,
    h: 0.455,
    ny: 91,
    t_s: 1.05,
    t_end: 1.75,
    x_i: 1.95,
    nx: 430,
    nx_ref: 740,
};
const D60: Duct = Duct {
    theta_deg: 60.0,
    h: 0.37,
    ny: 74,
    t_s: 1.58,
    t_end: 2.28,
    x_i: 2.5,
    nx: 540,
    nx_ref: 960,
};

/// Set-up D of ADR 0237 for the given sponges (Thompson boundary behind, memories zero): per sponge the measure over
/// the registered window and over its last 0.2.
fn duct(d: &Duct, kinds: &[Kind]) -> Vec<(f64, f64)> {
    let th = d.theta_deg.to_radians();
    let make = |nx: usize| {
        let mut s = solver(nx, d.ny, nx as f64 * DX, d.h, |x| train(x, th, d.h));
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.set_bc("ymax", CompBc::Slip).unwrap();
        s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        s
    };
    assert!((d.nx as f64 * DX - (d.x_i + LAYER)).abs() < 1e-9);
    let mut tests: Vec<CompressibleSolver> = kinds
        .iter()
        .map(|&kind| {
            let mut s = make(d.nx);
            apply(&mut s, kind, d.x_i);
            s
        })
        .collect();
    let mut reference = make(d.nx_ref);
    let x_probe = d.x_i - 0.1025;
    let mut probes: Vec<Vec<usize>> = tests.iter().map(|s| column(s, x_probe)).collect();
    probes.push(column(&reference, x_probe));
    assert_eq!(probes[0].len(), d.ny);
    reflection(
        &mut tests,
        &mut reference,
        &probes,
        d.t_s,
        d.t_end,
        d.t_end - 0.2,
        d.t_end,
    )
    .ratio()
}

/// D..s and D..d of ADR 0248 at one angle: set-up D with K1 and with K2.
fn clause_1(name: &str, d: &Duct) {
    let r = duct(d, &[K1, K2]);
    println!(
        "D{name}s K1, set-up D: R = {:.3e} (last 0.2 of the window {:.3e}); D{name}d K2: R = {:.3e} (last 0.2: {:.3e}); limit {LIMIT:e}",
        r[0].0, r[0].1, r[1].0, r[1].1
    );
    assert!(r[0].0 <= LIMIT, "D{name}s {}", r[0].0);
    assert!(r[1].0 <= LIMIT, "D{name}d {}", r[1].0);
}

/// D30s and D30d of ADR 0248 (clause 1 of F-340).
#[test]
#[ignore = "heavy: three duct runs of 50 000, 50 000 and 85 000 cells; run in the release profile"]
fn d30_setup_d_record_30_degrees() {
    clause_1("30", &D30);
}

/// D45s and D45d of ADR 0248 (clause 1 of F-340).
#[test]
#[ignore = "heavy: three duct runs of 39 000, 39 000 and 67 000 cells; run in the release profile"]
fn d45_setup_d_record_45_degrees() {
    clause_1("45", &D45);
}

/// D60s and D60d of ADR 0248 (clause 1 of F-340).
#[test]
#[ignore = "heavy: three duct runs of 40 000, 40 000 and 71 000 cells; run in the release profile"]
fn d60_setup_d_record_60_degrees() {
    clause_1("60", &D60);
}

// ---------------------------------------------------------------------------------------------------------------------
// Clause 5: set-up D2 of ADR 0237 amendment 1 (heavy)

/// The antiderivative with zero mean of the periodic, zero-mean train profile (ADR 0237 amendment 1): Simpson's rule
/// with 256 intervals for the Gaussian, minus `mean r`.
fn train_antiderivative(r: f64, mean: f64) -> f64 {
    let n = 256;
    let h = r / n as f64;
    let mut sum = gauss(0.0) + gauss(r);
    for i in 1..n {
        sum += if i % 2 == 1 { 4.0 } else { 2.0 } * gauss(i as f64 * h);
    }
    sum * h / 3.0 - mean * r
}

/// DR8 of ADR 0237 (amendment 1) at `t = 0`: the linear state of the train of set-up D at the point `x` and the memory
/// `Q` there, for the unshifted layer with its front at `layer_front` (`None`: no layer).
fn train_in_layer(x: [f64; 3], theta: f64, h: f64, layer_front: Option<f64>) -> (Prim, f64) {
    let (c, s) = (theta.cos(), theta.sin());
    let lam = 2.0 * h * s;
    let mean = W * std::f64::consts::PI.sqrt() / lam;
    let (att, rate) = match layer_front {
        Some(front) if x[0] > front => {
            let rate_max = rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap();
            let d = (x[0] - front).min(LAYER);
            let integral = rate_max * LAYER / (EXPONENT + 1.0) * (d / LAYER).powf(EXPONENT + 1.0);
            ((-c * integral).exp(), rate_max * (d / LAYER).powf(EXPONENT))
        }
        _ => (1.0, 0.0),
    };
    let (mut p, mut q, mut v) = (0.0, 0.0, 0.0);
    for sign in [1.0, -1.0] {
        let xi = x[0] * c + sign * x[1] * s;
        let r = xi - lam * (xi / lam).round();
        let g = (-1..=1).map(|m| gauss(r - m as f64 * lam)).sum::<f64>() - mean;
        let p_fam = EPS * att * g;
        let q_fam = -EPS * att * train_antiderivative(r, mean);
        p += p_fam;
        q += q_fam;
        v += sign * s * (p_fam + rate * q_fam);
    }
    let state = Prim {
        rho: 1.0 + p,
        u: [c * p, v, 0.0],
        p: P0 + p,
    };
    (state, if rate > 0.0 { q } else { 0.0 })
}

/// Set-up D2 of ADR 0237 (amendment 1) with the shifted form: the layer starts in the state and the `Q` of DR8 for
/// the unshifted layer and with the momentum memory `n cos(theta) Q / c` (ADR 0248, clause 5). The measure.
fn duct2(d: &Duct, shift: f64, along: f64) -> f64 {
    let th = d.theta_deg.to_radians();
    let make = |nx: usize, front: Option<f64>| {
        let mut s = solver(nx, d.ny, nx as f64 * DX, d.h, |x| train_in_layer(x, th, d.h, front).0);
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.set_bc("ymax", CompBc::Slip).unwrap();
        s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        s
    };
    assert!((d.nx as f64 * DX - (d.x_i + LAYER)).abs() < 1e-9);
    let mut test = make(d.nx, Some(d.x_i));
    apply(&mut test, Kind::Shifted { shift, along }, d.x_i);
    let memories = {
        let cc = test.geom.mesh.cell_centres();
        let rate = &test.sponge().unwrap().rate;
        let q: Vec<f64> = (0..cc.len())
            .map(|c| {
                if rate[c] > 0.0 {
                    train_in_layer(cc[c], th, d.h, Some(d.x_i)).1
                } else {
                    0.0
                }
            })
            .collect();
        SpongeMemories {
            m: q.iter().map(|q| [th.cos() * q, 0.0, 0.0]).collect(),
            r: vec![0.0; q.len()],
            q,
        }
    };
    test.set_sponge_memories(&memories).unwrap();
    let mut reference = make(d.nx_ref, None);
    let x_probe = d.x_i - 0.1025;
    let probes = vec![column(&test, x_probe), column(&reference, x_probe)];
    assert_eq!(probes[0].len(), d.ny);
    let mut tests = vec![test];
    reflection(&mut tests, &mut reference, &probes, d.t_s, d.t_end, d.t_end, d.t_end).ratio()[0].0
}

/// A30c of ADR 0248.
#[test]
#[ignore = "heavy: two duct runs of 50 000 and 85 000 cells; run in the release profile"]
fn a30c_oblique_train_30_degrees_with_the_shift() {
    let r = duct2(&D30, SHIFT, 0.0);
    println!("A30c K1, set-up D2: R = {r:.3e} (limit {LIMIT:e})");
    assert!(r <= LIMIT, "A30c {r}");
}

/// A45c of ADR 0248.
#[test]
#[ignore = "heavy: two duct runs of 39 000 and 67 000 cells; run in the release profile"]
fn a45c_oblique_train_45_degrees_with_the_shift() {
    let r = duct2(&D45, SHIFT, 0.0);
    println!("A45c K1, set-up D2: R = {r:.3e} (limit {LIMIT:e})");
    assert!(r <= LIMIT, "A45c {r}");
}

/// A60c of ADR 0248.
#[test]
#[ignore = "heavy: two duct runs of 40 000 and 71 000 cells; run in the release profile"]
fn a60c_oblique_train_60_degrees_with_the_shift() {
    let r = duct2(&D60, SHIFT, 0.0);
    println!("A60c K1, set-up D2: R = {r:.3e} (limit {LIMIT:e})");
    assert!(r <= LIMIT, "A60c {r}");
}

/// NCB60 of ADR 0248: the price of the damping along the layer. Set-up D2 at 60 degrees with K2 reflects more than
/// 1 percent (ADR 0248 DR12).
#[test]
#[ignore = "heavy: two duct runs of 40 000 and 71 000 cells; run in the release profile"]
fn ncb60_damping_along_the_layer_reflects_at_60_degrees() {
    let r = duct2(&D60, SHIFT, ALONG);
    println!("NCB60 K2, set-up D2: R = {r:.3e} (>= {LIMIT:e})");
    assert!(r >= LIMIT, "NCB60 {r}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Clauses 4 and 5: isolated pulses in a half space (heavy)

/// The half-space set-up of X1 and X45 of ADR 0237: one isolated plane pulse at `theta_deg` and its mirror image above
/// a rigid wall, a layer with its front at `x_i`; the probe is the wall cell 0.1025 in front of the layer. The measure
/// per sponge.
#[allow(clippy::too_many_arguments)]
fn half_space(
    theta_deg: f64,
    xi0: f64,
    x_i: f64,
    nx: usize,
    nx_ref: usize,
    ny: usize,
    t_end: f64,
    kinds: &[Kind],
) -> Vec<f64> {
    let th = theta_deg.to_radians();
    let (c, s) = (th.cos(), th.sin());
    let init = move |x: [f64; 3]| {
        superpose(&[
            (EPS * gauss(x[0] * c + x[1] * s - xi0), [c, s, 0.0]),
            (EPS * gauss(x[0] * c - x[1] * s - xi0), [c, -s, 0.0]),
        ])
    };
    let make = |nx: usize| {
        let mut sv = solver(nx, ny, nx as f64 * DX, ny as f64 * DX, init);
        sv.set_bc("ymin", CompBc::Slip).unwrap();
        sv.set_bc("ymax", CompBc::Extrapolate).unwrap();
        sv.set_bc("xmin", CompBc::Extrapolate).unwrap();
        sv.set_bc("xmax", THOMPSON).unwrap();
        sv
    };
    assert!((nx as f64 * DX - (x_i + LAYER)).abs() < 1e-9);
    let mut tests: Vec<CompressibleSolver> = kinds
        .iter()
        .map(|&kind| {
            let mut sv = make(nx);
            apply(&mut sv, kind, x_i);
            sv
        })
        .collect();
    let mut reference = make(nx_ref);
    let x_probe = x_i - 0.1025;
    let mut probes: Vec<Vec<usize>> = tests.iter().map(|sv| vec![column(sv, x_probe)[0]]).collect();
    probes.push(vec![column(&reference, x_probe)[0]]);
    reflection(&mut tests, &mut reference, &probes, 0.0, f64::INFINITY, 0.0, t_end)
        .ratio()
        .iter()
        .map(|r| r.0)
        .collect()
}

/// X1c of ADR 0248: X1 of ADR 0237 (one isolated pulse at 30 degrees) with K1.
#[test]
#[ignore = "heavy: two runs of 151 000 and 259 000 cells; run in the release profile"]
fn x1c_isolated_oblique_pulse_30_degrees_with_the_shift() {
    let r = half_space(30.0, 0.545, 1.9, 420, 720, 360, 1.75, &[K1]);
    println!(
        "X1c K1, isolated pulse at 30 degrees: R = {:.3e} (limit {LIMIT:e})",
        r[0]
    );
    assert!(r[0] <= LIMIT, "X1c {}", r[0]);
}

/// X45c of ADR 0248: X45 of ADR 0237 (one isolated pulse at 45 degrees) with K1.
#[test]
#[ignore = "heavy: two runs of 390 000 and 660 000 cells, about an hour; run in the release profile"]
fn x45c_isolated_oblique_pulse_45_degrees_with_the_shift() {
    let r = half_space(45.0, 0.2243, 3.35, 710, 1200, 550, 2.70, &[K1]);
    println!(
        "X45c K1, isolated pulse at 45 degrees: R = {:.3e} (limit {LIMIT:e})",
        r[0]
    );
    assert!(r[0] <= LIMIT, "X45c {}", r[0]);
}

/// X60 and X60o of ADR 0248 (clause 4 of F-340): one isolated plane pulse at 60 degrees, with K1 and with the matched
/// form of ADR 0237 (DR15: the set-up of X45 with tails of 3.5 widths and the reference grid 0.6 longer than the layer
/// front).
#[test]
#[ignore = "heavy: three runs of 705 000, 705 000 and 770 000 cells, more than an hour; run in the release profile"]
fn x60_isolated_oblique_pulse_60_degrees() {
    let r = half_space(60.0, -1.5724, 4.15, 870, 950, 810, 4.04, &[K1, Kind::Old]);
    println!(
        "X60 K1, isolated pulse at 60 degrees: R = {:.3e}; X60o matched form of ADR 0237: R = {:.3e} (limit {LIMIT:e})",
        r[0], r[1]
    );
    assert!(r[0] <= LIMIT, "X60 {}", r[0]);
    assert!(r[1] <= LIMIT, "X60o {}", r[1]);
}

// ---------------------------------------------------------------------------------------------------------------------
// Clause 3: the corner of two layers

/// C1, NCC1 and NCC2 of ADR 0248 (clause 3 of F-340): a quarter of a cylindrical pulse leaves through two layers and
/// their corner; without a sponge and with one layer only the Thompson boundaries reflect.
#[test]
#[ignore = "heavy: four runs of 25 600, 25 600, 25 600 and 62 500 cells; run in the release profile"]
fn c1_pulse_leaving_through_the_corner_of_two_layers() {
    let init = |x: [f64; 3]| {
        let dp = EPS * (-(x[0] * x[0] + x[1] * x[1]) / (W * W)).exp();
        Prim {
            rho: ((P0 + dp) / P0).powf(1.0 / GAMMA),
            u: [0.0; 3],
            p: P0 + dp,
        }
    };
    let make = |n: usize| {
        let mut s = solver(n, n, n as f64 * DX, n as f64 * DX, init);
        s.set_bc("xmin", CompBc::Slip).unwrap();
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        s.set_bc("ymax", THOMPSON).unwrap();
        s
    };
    let mut two = make(160);
    let sponge = Sponge::corner(
        &two.geom,
        ambient(),
        &registered_layer(0, 0.6),
        &registered_layer(1, 0.6),
        SHIFT,
        0.0,
    )
    .unwrap();
    two.set_sponge(Some(sponge)).unwrap();
    let none = make(160);
    let mut one = make(160);
    apply(&mut one, K1, 0.6);
    let mut reference = make(250);
    let at = |s: &CompressibleSolver| {
        vec![
            cell_at(s, 0.4975, 0.4975),
            cell_at(s, 0.4975, 0.3975),
            cell_at(s, 0.3975, 0.4975),
            cell_at(s, 0.4975, 0.0125),
        ]
    };
    let mut tests = vec![two, none, one];
    let mut probes: Vec<Vec<usize>> = tests.iter().map(at).collect();
    probes.push(at(&reference));
    let raw = reflection(&mut tests, &mut reference, &probes, 0.0, f64::INFINITY, 0.0, 1.8);
    let r = raw.ratio_per_probe();
    println!(
        "C1 two layers K1: R = {:.3e} (limit {LIMIT:e}); NCC1 no sponge: R = {:.5} (>= 0.04); NCC2 one layer K1: R = {:.5} (>= 0.02); incident peaks / eps {:?}",
        r[0],
        r[1],
        r[2],
        raw.den.iter().map(|d| d / EPS).collect::<Vec<_>>()
    );
    assert!(r[0] <= LIMIT, "C1 {}", r[0]);
    assert!(r[1] >= 0.04, "NCC1 {}", r[1]);
    assert!(r[2] >= 0.02, "NCC2 {}", r[2]);
}

// ---------------------------------------------------------------------------------------------------------------------
// Clause 2: a pressure offset with a gradient along the layer

/// Z1 and Z1o of ADR 0248 (clause 2 of F-340): a closed box with a layer, started at rest with the pressure offset
/// `delta (1 + 0.5 cos(pi y / H))`; over 100 transit times of the layer the momentum along the layer stays bounded and
/// does not grow, with K1 and with the matched form of ADR 0237.
#[test]
#[ignore = "long: two runs of 6400 cells over 20 000 steps; run in the release profile"]
fn z1_pressure_offset_with_a_gradient_along_the_layer() {
    let (h, delta) = (0.32, EPS);
    let run = |kind: Kind| -> ([f64; 4], f64) {
        let mut s = solver(100, 64, 0.5, h, |x| {
            let dp = delta * (1.0 + 0.5 * (std::f64::consts::PI * x[1] / h).cos());
            Prim {
                rho: ((P0 + dp) / P0).powf(1.0 / GAMMA),
                u: [0.0; 3],
                p: P0 + dp,
            }
        });
        for patch in ["xmin", "xmax", "ymin", "ymax"] {
            s.set_bc(patch, CompBc::Slip).unwrap();
        }
        apply(&mut s, kind, 0.3);
        let layer: Vec<usize> = (0..s.u.len()).filter(|&c| s.sponge().unwrap().rate[c] > 0.0).collect();
        assert_eq!(layer.len(), 40 * 64);
        let mut b = [0.0f64; 4];
        while s.time < 20.0 {
            let dt = s.max_dt();
            s.step(dt).unwrap();
            let k = ((s.time / 5.0) as usize).min(3);
            b[k] = layer.iter().fold(b[k], |m, &c| m.max(s.u[c][2].abs() / delta));
        }
        let mean = (0..s.u.len()).map(|c| pressure(&s, c) - P0).sum::<f64>() / s.u.len() as f64 / delta;
        (b, mean)
    };
    let ((new, new_mean), (old, old_mean)) = std::thread::scope(|scope| {
        let h = scope.spawn(|| run(Kind::Old));
        (run(K1), h.join().unwrap())
    });
    println!(
        "Z1 K1: c |m_t| / delta per quarter {new:.4?}, mean offset left {new_mean:.4}; Z1o matched form of ADR 0237: {old:.4?}, mean offset left {old_mean:.2e}; limits: largest <= 6, last quarter <= 1.05 first quarter"
    );
    for (name, b) in [("Z1", new), ("Z1o", old)] {
        let top = b.iter().fold(0.0f64, |m, x| m.max(*x));
        assert!(top <= 6.0, "{name} {top}");
        assert!(b[3] <= 1.05 * b[0], "{name} grows: {b:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// U, S, B (fast)

/// U3 of ADR 0248: a uniform state equal to the reference stays in two layers and their corner; the memories stay
/// zero.
#[test]
fn u3_uniform_reference_state_does_not_drift_in_a_corner() {
    let reference = Prim {
        rho: 1.2,
        u: [0.0; 3],
        p: 0.9,
    };
    let mut s = solver(40, 20, 1.0, 0.5, |_| reference);
    for patch in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(patch, CompBc::Slip).unwrap();
    }
    let first = SpongeLayer {
        origin: [0.5, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.5,
        rate_max: 50.0,
        exponent: 2.0,
    };
    let second = SpongeLayer {
        origin: [0.0, 0.25, 0.0],
        normal: [0.0, 1.0, 0.0],
        thickness: 0.25,
        ..first
    };
    let sponge = Sponge::corner(&s.geom, reference, &first, &second, SHIFT, 0.0).unwrap();
    let both = sponge
        .rate
        .iter()
        .zip(sponge.second_rates(&s.geom).unwrap())
        .filter(|(a, b)| **a > 0.0 && *b > 0.0)
        .count();
    assert_eq!(both, 200);
    s.set_sponge(Some(sponge)).unwrap();
    for _ in 0..200 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
    }
    let uref = reference.to_cons(AIR);
    let a = reference.sound_speed(AIR);
    let scale = [
        reference.rho,
        reference.rho * a,
        reference.rho * a,
        reference.rho * a,
        uref[4],
    ];
    let drift =
        s.u.iter()
            .flat_map(|c| (0..5).map(move |k| (c[k] - uref[k]).abs() / scale[k]))
            .fold(0.0, f64::max);
    let mem = s.sponge_memories();
    let memory = mem
        .q
        .iter()
        .chain(&mem.r)
        .chain(mem.m.iter().flatten())
        .fold(0.0f64, |m, x| m.max(x.abs()));
    println!(
        "U3 after 200 steps (t = {:.4}): drift {drift:.2e} (limit 1e-12), largest memory {memory:.2e} (zero)",
        s.time
    );
    assert!(drift <= 1e-12, "{drift}");
    assert!(memory == 0.0, "{memory}");
}

/// U4 of ADR 0248: a uniform state away from the reference in the shifted form (rate 2 in every cell, shift 0.5)
/// follows the closed forms of the pressure, the density and the memory.
#[test]
fn u4_uniform_offset_follows_the_closed_form_of_the_shifted_layer() {
    let (sigma, a) = (2.0, 0.5);
    let start = Prim {
        rho: 1.001,
        u: [0.0; 3],
        p: 1.002 * P0,
    };
    let mut s = solver(20, 20, 1.0, 1.0, |_| start);
    let n = s.u.len();
    s.set_sponge(Some(Sponge {
        reference: ambient(),
        rate: vec![sigma; n],
        form: SpongeForm::MatchedShifted(MatchedOptions {
            normal: [1.0, 0.0, 0.0],
            shift: a,
            along: 0.0,
            second: None,
        }),
    }))
    .unwrap();
    while s.time < 1.0 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
    }
    let t = s.time;
    let (z0, rho0, q) = (start.p - P0, start.rho - 1.0, sigma + a);
    let c2 = GAMMA * P0;
    let (es, eq) = ((-sigma * t).exp(), (-q * t).exp());
    let p_want = z0 * (a + sigma * eq) / q;
    let mem_want = z0 * (1.0 - eq) / q;
    let rho_want = rho0 * es + sigma * a * z0 / (c2 * q) * ((1.0 - es) / sigma - (es - eq) / a);
    let (mut worst_p, mut worst_rho, mut worst_q, mut momentum) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mem = s.sponge_memories();
    for c in 0..n {
        worst_p = worst_p.max(((pressure(&s, c) - P0) / p_want - 1.0).abs());
        worst_rho = worst_rho.max(((s.u[c][0] - 1.0) / rho_want - 1.0).abs());
        worst_q = worst_q.max((mem.q[c] / mem_want - 1.0).abs());
        momentum = momentum.max(s.u[c][1].abs()).max(s.u[c][2].abs()).max(s.u[c][3].abs());
    }
    println!(
        "U4 t = {t:.4}, {} steps: pressure {worst_p:.2e}, density {worst_rho:.2e} (limit 1e-5 each), memory {worst_q:.2e} (1e-4), momentum {momentum:.2e} (1e-12); offset left {:.5} of the start",
        s.steps,
        p_want / z0
    );
    assert!(worst_p <= 1e-5, "{worst_p}");
    assert!(worst_rho <= 1e-5, "{worst_rho}");
    assert!(worst_q <= 1e-4, "{worst_q}");
    assert!(momentum <= 1e-12, "{momentum}");
}

/// S3 of ADR 0248 (the solver's part): invalid options are refused and leave the solver unchanged; the shifted form
/// blocks the moving-mesh step, the mesh replacement and a mass-spring-damper wall; the memories are checked; clearing
/// is accepted.
#[test]
fn s3_solver_refusals_of_the_shifted_form() {
    let mut s = solver(8, 2, 1.0, 0.25, |_| ambient());
    let good = Sponge::planar_shifted(&s.geom, ambient(), &registered_layer(0, 0.5), SHIFT, ALONG).unwrap();
    let options = |change: &dyn Fn(&mut MatchedOptions)| {
        let mut bad = good.clone();
        if let SpongeForm::MatchedShifted(o) = &mut bad.form {
            change(o);
        }
        bad
    };
    let mut moving = good.clone();
    moving.reference.u = [0.1, 0.0, 0.0];
    for bad in [
        options(&|o| o.shift = -1.0),
        options(&|o| o.shift = f64::NAN),
        options(&|o| o.along = -0.1),
        options(&|o| o.along = f64::INFINITY),
        options(&|o| o.normal = [0.0; 3]),
        options(&|o| o.second = Some(registered_layer(0, 0.5))),
        moving,
    ] {
        assert!(s.set_sponge(Some(bad)).is_err());
        assert!(s.sponge().is_none() && s.sponge_memory().is_empty());
    }
    let n = s.u.len();
    let zero = SpongeMemories {
        q: vec![0.0; n],
        r: vec![0.0; n],
        m: vec![[0.0; 3]; n],
    };
    assert!(s.set_sponge_memories(&zero).is_err(), "no sponge");
    apply(&mut s, Kind::Old, 0.5);
    assert!(s.set_sponge_memories(&zero).is_err(), "the matched form of ADR 0237");
    assert!(s.sponge_memories().r.is_empty() && s.sponge_memories().q.len() == n);
    s.set_sponge(Some(good.clone())).unwrap();
    assert_eq!(s.sponge(), Some(&good));
    assert_eq!(s.sponge_memories(), zero);
    let mut short = zero.clone();
    short.m.pop();
    let mut nan = zero.clone();
    nan.r[3] = f64::NAN;
    assert!(s.set_sponge_memories(&short).is_err());
    assert!(s.set_sponge_memories(&nan).is_err());
    let mut set = zero.clone();
    set.q[5] = 0.25;
    set.m[6] = [0.5, 0.0, 0.0];
    s.set_sponge_memories(&set).unwrap();
    assert_eq!(s.sponge_memories(), set);
    assert_eq!(s.sponge_memory()[5], 0.25);
    assert!(s.step_ale(s.geom.clone(), 1e-3).is_err(), "step_ale with a sponge");
    assert!(
        s.replace_mesh(s.geom.clone(), s.u.clone()).is_err(),
        "replace_mesh with a sponge"
    );
    let wall = MsdWall {
        p: P0,
        r: 1.0,
        m: 1.0,
        k: 1.0,
    };
    assert!(s.set_msd_wall("xmax", Some(wall)).is_err(), "wall with a sponge");
    assert_eq!(s.steps, 0);
    s.set_sponge(None).unwrap();
    assert!(s.sponge().is_none() && s.sponge_memory().is_empty() && s.sponge_memories() == SpongeMemories::default());
    s.step_ale(s.geom.clone(), 1e-3).unwrap();
}

/// B3 (ii) of ADR 0248: the shifted form without shift, damping and second layer produces the bits of the matched
/// form of ADR 0237, state and memory; with K1 the state differs.
#[test]
fn b3_shifted_form_without_options_is_bit_identical_to_the_matched_form() {
    let th = 45f64.to_radians();
    let run = |kind: Kind| -> (Vec<[u64; 5]>, Vec<u64>) {
        let mut s = solver(60, 20, 0.3, 0.1, |x| train(x, th, 0.1));
        s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        apply(&mut s, kind, 0.1);
        for _ in 0..30 {
            let dt = s.max_dt();
            s.step(dt).unwrap();
        }
        (
            s.u.iter().map(|c| c.map(f64::to_bits)).collect(),
            s.sponge_memory().iter().map(|q| q.to_bits()).collect(),
        )
    };
    let old = run(Kind::Old);
    let plain = run(Kind::Shifted { shift: 0.0, along: 0.0 });
    let k1 = run(K1);
    assert!(old.0 == plain.0, "state");
    assert!(old.1 == plain.1, "memory");
    assert!(old.1.iter().any(|q| *q != 0), "the memory is in use");
    let changed = old.0.iter().zip(&k1.0).filter(|(a, b)| a != b).count();
    println!(
        "B3 {} cells, 30 steps: the shifted form without options is bit-identical to the matched form (state and memory); K1 changes {changed} cells",
        old.0.len()
    );
    assert!(changed > 0, "the control run must differ");
}
