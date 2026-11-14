//! F-519 (ADR 0428): the sparse whirl state solver above the loss of stiffness: an isotropic shift-invert Arnoldi process on
//! `(C^2 + sigma)^-1` of the state matrix of ADR 0416 with one indefinite skyline factor of the real form of `Q(i
//! sqrt(sigma))`, against the dense state solver on the point masses, random problems, the cube of ADR 0416, the disc sector
//! of ADR 0420 and its full rotor, and on the ring of ADR 0427 above the dense cap against its dense sector union. Checks A2
//! to R1 and the controls N1 to N3 of the registration, U1 in the form U1b of amendment 1.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::full_rotor;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    gyroscopic_state_eigen_dense, gyroscopic_state_eigen_sparse, HarmonicWhirlSpeedModes, RotatingOptions,
    RotatingPart, RotatingSector, SparseStateOptions, SparseStateWhirl, Stability, StateWhirlMode,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::time::Instant;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{id}: {value:.6e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const W0: f64 = 1000.0;
const WY: f64 = 1.2 * W0;
const H: f64 = 0.1;
const MASS: f64 = 7.85;
const SIGMA_A: f64 = (0.25 * W0) * (0.25 * W0);
const SIGMA_B: f64 = (2.7 * W0) * (2.7 * W0);

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn key_of(e: FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::Solver { solver, .. } => format!("solver:{solver}"),
        other => format!("other:{other:?}"),
    }
}

fn csr(a: &[f64], n: usize) -> CsrMatrix {
    let t = (0..n * n)
        .filter(|&p| a[p] != 0.0)
        .map(|p| (p / n, p % n, a[p]))
        .collect();
    CsrMatrix::from_triplets(n, t)
}

fn cabs(s: [f64; 2]) -> f64 {
    s[0].hypot(s[1])
}

/// `|nu(s)| = 1 / |s^2 + sigma|`.
fn nu_abs(s: [f64; 2], sigma: f64) -> f64 {
    let s2 = [s[0] * s[0] - s[1] * s[1], 2.0 * s[0] * s[1]];
    1.0 / (s2[0] + sigma).hypot(s2[1])
}

/// The global greedy matching of ADR 0420: `(i, j)` pairs of `a[i]` and `b[j]` by increasing distance.
fn matching(a: &[[f64; 2]], b: &[[f64; 2]]) -> Vec<(usize, usize)> {
    let mut d: Vec<(f64, usize, usize)> = Vec::with_capacity(a.len() * b.len());
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            d.push(((x[0] - y[0]).hypot(x[1] - y[1]), i, j));
        }
    }
    d.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)).then(p.2.cmp(&q.2)));
    let (mut ta, mut tb) = (vec![false; a.len()], vec![false; b.len()]);
    let mut out = Vec::new();
    for (_, i, j) in d {
        if !ta[i] && !tb[j] {
            ta[i] = true;
            tb[j] = true;
            out.push((i, j));
        }
    }
    out
}

/// The comparison of the reading: the dense eigenvalues ranked by `|nu|` (ties by index), the first `sparse.len()` the
/// reference; `(worst matched relative deviation, boundary valid, reference set, whether the dense eigenvalue of largest Re
/// s is in the reference set)`.
fn compare(sparse: &[[f64; 2]], dense: &[[f64; 2]], sigma: f64) -> (f64, bool, Vec<[f64; 2]>, bool) {
    let mut order: Vec<usize> = (0..dense.len()).collect();
    order.sort_by(|&i, &j| {
        nu_abs(dense[j], sigma)
            .total_cmp(&nu_abs(dense[i], sigma))
            .then(i.cmp(&j))
    });
    let k = sparse.len();
    if k > dense.len() {
        return (f64::INFINITY, false, Vec::new(), false);
    }
    let valid = k == dense.len() || nu_abs(dense[order[k - 1]], sigma) > (1.0 + 1e-6) * nu_abs(dense[order[k]], sigma);
    let reference: Vec<[f64; 2]> = order[..k].iter().map(|&i| dense[i]).collect();
    let pairs = matching(sparse, &reference);
    let dev = pairs
        .iter()
        .map(|&(i, j)| (sparse[i][0] - reference[j][0]).hypot(sparse[i][1] - reference[j][1]) / cabs(reference[j]))
        .fold(0.0f64, f64::max);
    let imax = (0..dense.len())
        .max_by(|&i, &j| dense[i][0].total_cmp(&dense[j][0]).then(j.cmp(&i)))
        .unwrap();
    let covered = order[..k].contains(&imax);
    (dev, valid, reference, covered)
}

fn same_flag(a: Stability, b: Stability) -> bool {
    a.is_stable() == b.is_stable()
}

fn growth_of(s: Stability) -> Option<f64> {
    match s {
        Stability::Stable => None,
        Stability::Unstable { growth } => Some(growth),
    }
}

/// W1: the returned modes matched to a dense reported mode (within 1e-8 relative) with a relative gap above 1e-3 to every
/// other dense eigenvalue: `(max |kappa_s - kappa_d|, the number compared)`.
fn whirl_dev(sparse: &[StateWhirlMode], dense: &[StateWhirlMode], all: &[[f64; 2]]) -> (f64, usize) {
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for x in sparse {
        let s = [x.growth, x.omega];
        let Some(y) = dense
            .iter()
            .find(|y| (y.growth - s[0]).hypot(y.omega - s[1]) <= 1e-8 * cabs(s))
        else {
            continue;
        };
        let t = [y.growth, y.omega];
        let gap = all
            .iter()
            .filter(|e| (e[0] - t[0]).hypot(e[1] - t[1]) > 1e-8 * cabs(t))
            .map(|e| (e[0] - t[0]).hypot(e[1] - t[1]) / cabs(t))
            .fold(f64::INFINITY, f64::min);
        if gap > 1e-3 {
            worst = worst.max((x.whirl - y.whirl).abs());
            count += 1;
        }
    }
    (worst, count)
}

fn max_residual(modes: &[StateWhirlMode]) -> f64 {
    modes.iter().fold(0.0f64, |a, x| a.max(x.residual))
}

fn print_sparse(tag: &str, r: &SparseStateWhirl) {
    println!(
        "{tag}: unknowns {}, loss {:?}, {:?}, real defect {:.2e}, {} eigenvalues in {} steps, isotropy {:.2e}; modes \
         (growth, omega): {:?}",
        r.unknowns,
        r.loss,
        r.stability,
        r.real_defect,
        r.eigenvalues.len(),
        r.steps,
        r.isotropy,
        r.modes.iter().map(|x| (x.growth, x.omega)).collect::<Vec<_>>()
    );
}

// ------------------------------------------------------------------------------------------------------ PM, RND, R1, N1

/// ADR 0416 D5: `(lambda_+, lambda_-)`.
fn closed(wx: f64, wy: f64, w: f64) -> (f64, f64) {
    let s = wx * wx + wy * wy + 2.0 * w * w;
    let p = (wx * wx - w * w) * (wy * wy - w * w);
    let d = (wx * wx - wy * wy).powi(2) + 8.0 * w * w * (wx * wx + wy * wy);
    let lm = -(s + d.sqrt()) / 2.0;
    (p / lm, lm)
}

fn point_mass(wx: f64, wy: f64, w: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let k = vec![MASS * (wx * wx - w * w), 0.0, 0.0, MASS * (wy * wy - w * w)];
    let m = vec![MASS, 0.0, 0.0, MASS];
    let g = vec![0.0, -2.0 * w * MASS, 2.0 * w * MASS, 0.0];
    (k, m, g)
}

fn lcg(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

fn spd(n: usize, state: &mut u64) -> Vec<f64> {
    let b: Vec<f64> = (0..n * n).map(|_| lcg(state)).collect();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| b[i * n + k] * b[j * n + k]).sum::<f64>();
        }
        a[i * n + i] += n as f64;
    }
    a
}

#[test]
fn p1_u1b_q1_i1_r1_n1_algebraic() {
    let mut ch = Checks::new();
    let (mut q1, mut i1): (f64, f64) = (0.0, 0.0);
    // P1: the isotropic point mass at 1.5 omega_0.
    let w = 1.5 * W0;
    let (k, m, g) = point_mass(W0, W0, w);
    let r = gyroscopic_state_eigen_sparse(&csr(&k, 2), &csr(&m, 2), &csr(&g, 2), &SparseStateOptions::new(SIGMA_A, 2))
        .unwrap();
    println!("PMI: eigenvalues {:?}, {:?}, omegas {:?}", r.eigenvalues, r.stability, r.omegas);
    let p1i = if r.omegas.len() == 2 {
        (r.omegas[0] / (w - W0) - 1.0)
            .abs()
            .max((r.omegas[1] / (w + W0) - 1.0).abs())
    } else {
        f64::INFINITY
    };
    ch.le("P1 PMI Omega -/+ omega_0 relative to each", p1i, 1e-9);
    ch.holds("P1 PMI flag", r.stability == Stability::Stable, "Stable");
    q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
    i1 = i1.max(r.isotropy);
    // P1: the anisotropic point mass at 1.1 omega_0.
    let w = 1.1 * W0;
    let (k, m, g) = point_mass(W0, WY, w);
    let (lp, lm) = closed(W0, WY, w);
    let r = gyroscopic_state_eigen_sparse(&csr(&k, 2), &csr(&m, 2), &csr(&g, 2), &SparseStateOptions::new(SIGMA_A, 2))
        .unwrap();
    println!(
        "PMA: eigenvalues {:?}, {:?}, growth {:?}, omegas {:?} (closed growth {}, frequency {})",
        r.eigenvalues,
        r.stability,
        r.growth,
        r.omegas,
        lp.sqrt(),
        (-lm).sqrt()
    );
    let p1a = if r.omegas.len() == 2 && r.omegas[0] == 0.0 {
        (r.growth[0] / lp.sqrt() - 1.0)
            .abs()
            .max((r.omegas[1] / (-lm).sqrt() - 1.0).abs())
    } else {
        f64::INFINITY
    };
    ch.le("P1 PMA growth and frequency relative to each", p1a, 1e-9);
    ch.holds(
        "P1 PMA flag",
        matches!(r.stability, Stability::Unstable { .. }),
        "Unstable",
    );
    q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
    i1 = i1.max(r.isotropy);

    // U1b: random problems.
    let mut st = 19u64;
    let (mut u1, mut flags_ok, mut exercised, mut all_valid) = (0.0f64, true, 0usize, true);
    for (n, count) in [(6usize, 2usize), (12, 3), (30, 5), (60, 8)] {
        let m = spd(n, &mut st);
        let mut k = spd(n, &mut st);
        for i in 0..n {
            k[i * n + i] -= (n + 2) as f64;
        }
        let mut g = vec![0.0; n * n];
        for i in 0..n {
            for j in i + 1..n {
                let v = 3.0 * lcg(&mut st);
                g[i * n + j] = v;
                g[j * n + i] = -v;
            }
        }
        let d = gyroscopic_state_eigen_dense(&k, &m, &g, n).unwrap();
        let r = gyroscopic_state_eigen_sparse(&csr(&k, n), &csr(&m, n), &csr(&g, n), &SparseStateOptions::new(1.0, count))
            .unwrap();
        let (dev, valid, _, covered) = compare(&r.eigenvalues, &d.eigenvalues, 1.0);
        println!(
            "RND n = {n}: {} eigenvalues, deviation {dev:.3e}, boundary valid {valid}, sparse {:?}, dense {:?}, unstable \
             eigenvalue in the set {covered}, steps {}, isotropy {:.2e}",
            r.eigenvalues.len(),
            r.stability,
            d.stability,
            r.steps,
            r.isotropy
        );
        u1 = u1.max(dev);
        all_valid &= valid;
        let dense_unstable = !d.stability.is_stable();
        if !dense_unstable || covered {
            exercised += 1;
            flags_ok &= same_flag(r.stability, d.stability);
        }
        q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
        i1 = i1.max(r.isotropy);
    }
    ch.holds("U1b boundary", all_valid, "every reference boundary valid");
    ch.le("U1b eigenvalues against the dense state solver", u1, 1e-9);
    println!("U1b flags exercised on {exercised} of 4 problems");
    ch.holds("U1b flags where exercised", flags_ok, "equal");
    ch.le("Q1 point masses and random problems", q1, 1e-8);
    ch.le("I1 point masses and random problems", i1, 1e-10);

    // R1
    let eye = |n: usize| CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect());
    let zero = |n: usize| CsrMatrix::from_triplets(n, Vec::new());
    let (k2, m2, g2) = (eye(2), eye(2), zero(2));
    let refuse = |k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, o: SparseStateOptions| {
        key_of(gyroscopic_state_eigen_sparse(k, m, g, &o).unwrap_err())
    };
    for (what, sigma) in [("0", 0.0), ("-1", -1.0), ("NaN", f64::NAN)] {
        ch.holds(
            &format!("R1 shift {what}"),
            refuse(&k2, &m2, &g2, SparseStateOptions::new(sigma, 1)) == "shift",
            "\"shift\"",
        );
    }
    ch.holds(
        "R1 count 0",
        refuse(&k2, &m2, &g2, SparseStateOptions::new(0.25, 0)) == "modes",
        "\"modes\"",
    );
    ch.holds(
        "R1 count n + 1",
        refuse(&k2, &m2, &g2, SparseStateOptions::new(0.25, 3)) == "modes",
        "\"modes\"",
    );
    ch.holds(
        "R1 wrong size",
        refuse(&eye(3), &m2, &g2, SparseStateOptions::new(0.25, 1)) == "matrix",
        "\"matrix\"",
    );
    let minus = CsrMatrix::from_triplets(2, vec![(0, 0, -1.0), (1, 1, -1.0)]);
    ch.holds(
        "R1 mass",
        refuse(&k2, &minus, &g2, SparseStateOptions::new(0.25, 1)) == "mass",
        "\"mass\"",
    );
    ch.holds(
        "R1 tolerance 0",
        refuse(&k2, &m2, &g2, SparseStateOptions::new(0.25, 1).tolerance(0.0)) == "options",
        "\"options\"",
    );
    ch.holds(
        "R1 zero pivot (K = M = I, sigma = 1)",
        refuse(&k2, &m2, &g2, SparseStateOptions::new(1.0, 1)) == "shift",
        "\"shift\"",
    );

    // N1: one frequency of multiplicity 20.
    let (k20, m20, g20) = (eye(20), eye(20), zero(20));
    let n1 = gyroscopic_state_eigen_sparse(&k20, &m20, &g20, &SparseStateOptions::new(0.25, 2));
    println!("N1 count 2: {:?}", n1.as_ref().map(|r| r.eigenvalues.clone()));
    ch.holds(
        "N1 count 2",
        n1.map_or_else(|e| key_of(e) == "solver:sparse_state_whirl", |_| false),
        "refused (one Ritz value)",
    );
    let r = gyroscopic_state_eigen_sparse(&k20, &m20, &g20, &SparseStateOptions::new(0.25, 1)).unwrap();
    println!("N1 count 1: {:?}", r.eigenvalues);
    let d = if r.eigenvalues.len() == 2 {
        (r.eigenvalues[0][0].hypot(r.eigenvalues[0][1] + 1.0)).max(r.eigenvalues[1][0].hypot(r.eigenvalues[1][1] - 1.0))
    } else {
        f64::INFINITY
    };
    ch.le("N1 count 1: the eigenvalues +/- i", d, 1e-12);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ the cube

fn cube(wx: f64, wy: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let mass = RHO * H * H * H;
    m.add_support(ElasticSupport::nodal_spring("all", 0, mass * wx * wx / 8.0))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, mass * wy * wy / 8.0))
        .unwrap();
    m
}

#[test]
fn e1_f1_l1_w1_q1_i1_n2_n3_cube() {
    let mut ch = Checks::new();
    let (mut e1, mut q1, mut i1, mut w1): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let (mut valid, mut flags, mut expected, mut losses) = (true, true, true, true);
    let mut w1n = 0;
    let cases: [(&str, f64, &[f64], &[bool]); 2] = [
        ("CBI", W0, &[1.2, 1.5, 2.0], &[true, true, true]),
        ("CBA", WY, &[1.05, 1.1, 1.18, 1.3], &[false, false, false, true]),
    ];
    for (tag, wy, speeds, stable) in cases {
        let cb = cube(W0, wy);
        let part = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(12).coriolis(true)).unwrap();
        for (j, &f) in speeds.iter().enumerate() {
            let w = f * W0;
            let d = part.whirl_state(w).unwrap();
            for (sname, sigma) in [("sigma_a", SIGMA_A), ("sigma_b", SIGMA_B)] {
                let r = part.whirl_state_sparse(w, &SparseStateOptions::new(sigma, 6)).unwrap();
                print_sparse(&format!("{tag} {f} omega_0 {sname}"), &r);
                let (dev, ok, _, covered) = compare(&r.eigenvalues, &d.eigenvalues, sigma);
                println!(
                    "  E1 deviation {dev:.3e}, boundary valid {ok}; dense {:?}, unstable eigenvalue in the set {covered}; \
                     growth sparse {:?} dense {:?}",
                    d.stability,
                    growth_of(r.stability),
                    growth_of(d.stability)
                );
                e1 = e1.max(dev);
                valid &= ok;
                flags &= same_flag(r.stability, d.stability);
                expected &= r.stability.is_stable() == stable[j];
                losses &= r.loss == d.loss && r.loss.is_some();
                let (wd, wn) = whirl_dev(&r.modes, &d.modes, &d.eigenvalues);
                w1 = w1.max(wd);
                w1n += wn;
                q1 = q1.max(max_residual(&r.modes));
                i1 = i1.max(r.isotropy);
            }
        }
    }
    ch.holds("E1 boundary", valid, "every reference boundary valid");
    ch.le("E1 cube eigenvalues against the dense state solver", e1, 1e-9);
    ch.holds("F1 flags", flags, "the sparse flag equals the dense flag at every speed and shift");
    ch.holds(
        "F1 expected flags",
        expected,
        "Stable at CBI 1.2, 1.5, 2.0 and CBA 1.3; Unstable at CBA 1.05, 1.1, 1.18",
    );
    ch.holds("L1 cube losses", losses, "equal to the dense whirl_state's and Some");
    println!("W1 cube: {w1n} modes compared");
    ch.le("W1 cube whirl numbers", w1, 1e-6);
    ch.le("Q1 cube residuals", q1, 1e-8);
    ch.le("I1 cube isotropy", i1, 1e-10);

    // N2: the sparse solver without Coriolis against the dense one with it (CBA, 1.1 omega_0, sigma_a).
    let cb = cube(W0, WY);
    let on = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(12).coriolis(true)).unwrap();
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(12).coriolis(false)).unwrap();
    let w = 1.1 * W0;
    let d = on.whirl_state(w).unwrap();
    let r = off.whirl_state_sparse(w, &SparseStateOptions::new(SIGMA_A, 6)).unwrap();
    print_sparse("N2 CBA 1.1 off", &r);
    let (dev, _, _, _) = compare(&r.eigenvalues, &d.eigenvalues, SIGMA_A);
    ch.gt("N2 sparse off against dense on", dev, 1e-2);

    // N3: CBI at 1.5 omega_0 without Coriolis: the pair diverges.
    let cb = cube(W0, W0);
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(12).coriolis(false)).unwrap();
    let w = 1.5 * W0;
    let r = off.whirl_state_sparse(w, &SparseStateOptions::new(SIGMA_A, 6)).unwrap();
    print_sparse("N3 CBI 1.5 off", &r);
    let expected_growth = (w * w - W0 * W0).sqrt();
    match r.stability {
        Stability::Unstable { growth } => {
            ch.le(
                "N3 growth against sqrt(Omega^2 - omega_0^2)",
                (growth / expected_growth - 1.0).abs(),
                1e-8,
            );
            ch.gt("N3 growth", growth, 1e3);
        }
        Stability::Stable => ch.holds("N3 flag", false, "Unstable"),
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ the disc sector

fn sector_mesh(sectors: usize, div: [usize; 3]) -> SolidMesh {
    let alpha = 2.0 * PI / sectors as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], div).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn model_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn sector_model(sectors: usize, div: [usize; 3]) -> SolidModel {
    let mut m = model_on(sector_mesh(sectors, div));
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, sectors))
        .unwrap();
    m
}

fn full_of(sector: &SolidModel) -> SolidModel {
    let rotor = full_rotor(sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    full
}

fn modal(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn options(modes: usize, stress: bool) -> RotatingOptions {
    RotatingOptions::new(modes)
        .modal(modal(modes))
        .coriolis(true)
        .stress_stiffening(stress)
}

fn lost(rs: &RotatingSector, k: usize, w: f64) -> bool {
    rs.whirl_modes(k, w).unwrap().loss().is_some()
}

/// The rule of ADR 0420 D3: `(omega_ref, stress stiffening on, Omega_L, k*, Omega_T)` with the options of ADR 0420
/// (`options(4, .)`, the 4 subspace modes of `ds_modal`).
fn test_speed(sector: &SolidModel, sectors: usize) -> (f64, bool, f64, usize, f64) {
    let rs_both = RotatingSector::new(sector, &options(4, true)).unwrap();
    let w_ref = (0..=sectors / 2)
        .map(|k| match rs_both.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss at rest {l:?}"),
        })
        .fold(f64::INFINITY, f64::min);
    let bracket = |rs: &RotatingSector| -> Option<f64> {
        for f in [2.0, 4.0, 8.0, 16.0] {
            let w = f * w_ref;
            let ks: Vec<usize> = (0..=sectors / 2).filter(|&k| lost(rs, k, w)).collect();
            println!("D: at {f} omega_ref the indices {ks:?} have lost stiffness");
            if !ks.is_empty() {
                return Some(w);
            }
        }
        None
    };
    let (stress, hi) = match bracket(&rs_both) {
        Some(hi) => (true, hi),
        None => {
            println!("D: no loss up to 16 omega_ref with both effects; fallback stress_stiffening(false)");
            let rs_off = RotatingSector::new(sector, &options(4, false)).unwrap();
            let hi = bracket(&rs_off).expect("defective set-up: no loss up to 16 omega_ref with the fallback");
            (false, hi)
        }
    };
    let rs = RotatingSector::new(sector, &options(4, stress)).unwrap();
    let mut w_l = f64::INFINITY;
    let mut k_star = 0;
    for k in 0..=sectors / 2 {
        if !lost(&rs, k, hi) {
            continue;
        }
        let (mut lo, mut up) = (0.8 * w_ref, hi);
        assert!(!lost(&rs, k, lo), "defective set-up: index {k} lost at 0.8 omega_ref");
        for _ in 0..60 {
            let mid = 0.5 * (lo + up);
            if lost(&rs, k, mid) {
                up = mid;
            } else {
                lo = mid;
            }
        }
        if up < w_l {
            w_l = up;
            k_star = k;
        }
    }
    let w_t = 1.25 * w_l;
    println!(
        "D: omega_ref = {w_ref:.9e}, stress stiffening {stress}, Omega_L = {w_l:.12e} rad/s = {:.9} omega_ref, k* = \
         {k_star}, Omega_T = {w_t:.12e}",
        w_l / w_ref
    );
    (w_ref, stress, w_l, k_star, w_t)
}

#[test]
fn e2_f2_e3_w1_q1_i1_r1_disc_sector() {
    let mut ch = Checks::new();
    let sectors = 6;
    let sector = sector_model(sectors, [2, 1, 1]);
    let (w_ref, stress, _, _, w_t) = test_speed(&sector, sectors);
    let rs = RotatingSector::new(&sector, &options(8, stress)).unwrap();
    let shifts = [
        ("sigma_a", (0.5 * w_ref).powi(2)),
        ("sigma_b", (2.0 * w_ref).powi(2)),
    ];
    let (mut e2, mut q1, mut i1, mut w1): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let (mut valid, mut flags, mut rotor, mut some_loss) = (true, true, true, false);
    let mut w1n = 0;
    let dense: Vec<_> = (0..=sectors / 2).map(|k| rs.whirl_state(k, w_t).unwrap()).collect();
    for (sname, sigma) in shifts {
        let mut sflags = Vec::new();
        for (k, d) in dense.iter().enumerate() {
            let r = rs
                .whirl_state_sparse(k, w_t, &SparseStateOptions::new(sigma, 8))
                .unwrap();
            print_sparse(&format!("DS k = {k} {sname}"), &r);
            let (dev, ok, _, covered) = compare(&r.eigenvalues, &d.state.eigenvalues, sigma);
            println!(
                "  E2 deviation {dev:.3e}, boundary valid {ok}; dense {:?} (real defect {:.2e}), unstable eigenvalue in \
                 the set {covered}",
                d.state.stability, d.state.real_defect
            );
            e2 = e2.max(dev);
            valid &= ok;
            flags &= same_flag(r.stability, d.state.stability);
            some_loss |= r.loss.is_some();
            sflags.push(r.stability);
            let (wd, wn) = whirl_dev(&r.modes, &d.state.modes, &d.state.eigenvalues);
            w1 = w1.max(wd);
            w1n += wn;
            q1 = q1.max(max_residual(&r.modes));
            i1 = i1.max(r.isotropy);
        }
        let dflags: Vec<Stability> = dense.iter().map(|d| d.state.stability).collect();
        rotor &= same_flag(Stability::combined(&sflags), Stability::combined(&dflags));
    }
    ch.holds("E2 boundary", valid, "every reference boundary valid");
    ch.le("E2 DS eigenvalues against the dense sector state solver", e2, 1e-9);
    ch.holds("F2 index flags", flags, "the sparse flag equals the dense flag per index");
    ch.holds("F2 rotor flag", rotor, "combined sparse flags equal the combined dense flags");
    ch.holds("L1 DS", some_loss, "an index has lost stiffness at Omega_T");
    println!("W1 DS: {w1n} modes compared");
    ch.le("W1 DS whirl numbers", w1, 1e-6);

    // E3: the full rotor.
    let full = full_of(&sector);
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &options(8, stress)).unwrap();
    let fd = fp.whirl_state(w_t).unwrap();
    let (mut e3, mut v3, mut f3) = (0.0f64, true, true);
    for (sname, sigma) in shifts {
        let r = fp.whirl_state_sparse(w_t, &SparseStateOptions::new(sigma, 8)).unwrap();
        print_sparse(&format!("DS full {sname}"), &r);
        let (dev, ok, _, _) = compare(&r.eigenvalues, &fd.eigenvalues, sigma);
        println!("  E3 deviation {dev:.3e}, boundary valid {ok}; dense {:?}", fd.stability);
        e3 = e3.max(dev);
        v3 &= ok;
        f3 &= same_flag(r.stability, fd.stability);
        q1 = q1.max(max_residual(&r.modes));
        i1 = i1.max(r.isotropy);
    }
    ch.holds("E3 boundary", v3, "every reference boundary valid");
    ch.le("E3 full rotor of DS against the dense state solver", e3, 1e-9);
    ch.holds("E3 flags", f3, "equal");
    ch.le("Q1 DS and its full rotor residuals", q1, 1e-8);
    ch.le("I1 DS and its full rotor isotropy", i1, 1e-10);

    // R1 on the models.
    let o = SparseStateOptions::new(shifts[0].1, 8);
    ch.holds(
        "R1 harmonic 4",
        key_of(rs.whirl_state_sparse(4, w_t, &o).unwrap_err()) == "harmonic",
        "\"harmonic\"",
    );
    ch.holds(
        "R1 speed -1",
        key_of(fp.whirl_state_sparse(-1.0, &o).unwrap_err()) == "speed",
        "\"speed\"",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ the ring above the cap

#[test]
fn a2_m1_m2_q1_i1_medium_ring() {
    let mut ch = Checks::new();
    let sectors = 16;
    let sector = sector_model(sectors, [4, 1, 2]);
    let (w_mp, stress, _, _, w_t) = test_speed(&sector, sectors);
    let rs = RotatingSector::new(&sector, &options(4, stress)).unwrap();
    let full = full_of(&sector);
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &options(4, stress)).unwrap();
    let refused = fp.whirl_state(w_t).map(|_| ()).unwrap_err();
    let sigma = (0.5 * w_mp).powi(2);
    let t = Instant::now();
    let r = fp.whirl_state_sparse(w_t, &SparseStateOptions::new(sigma, 10)).unwrap();
    println!("MP sparse solve {:.2} s", t.elapsed().as_secs_f64());
    print_sparse("MP", &r);
    ch.holds(
        "A2 MP",
        r.unknowns > 1500 && key_of(refused) == "unknowns" && r.eigenvalues.len() >= 20 && r.loss.is_some(),
        &format!(
            "{} unknowns, dense refused with \"unknowns\", {} eigenvalues, loss {:?}",
            r.unknowns,
            r.eigenvalues.len(),
            r.loss
        ),
    );
    let t = Instant::now();
    let mut union = Vec::new();
    let mut dflags = Vec::new();
    for k in 0..=sectors / 2 {
        let h = rs.whirl_state(k, w_t).unwrap();
        println!(
            "MP k = {k}: unknowns {}, loss {:?}, {:?}",
            h.state.unknowns, h.state.loss, h.state.stability
        );
        union.extend_from_slice(&h.state.eigenvalues);
        dflags.push(h.state.stability);
    }
    println!("MP dense sector union {:.2} s, {} eigenvalues", t.elapsed().as_secs_f64(), union.len());
    let (dev, ok, _, covered) = compare(&r.eigenvalues, &union, sigma);
    ch.holds("M1 boundary", ok, "the reference boundary valid");
    ch.le("M1 MP against the dense sector union", dev, 1e-8);
    let combined = Stability::combined(&dflags);
    println!(
        "M2: sparse {:?}, union {combined:?}, unstable eigenvalue in the set {covered}",
        r.stability
    );
    if combined.is_stable() || covered {
        ch.holds("M2 flag", same_flag(r.stability, combined), "equal");
    } else {
        println!("M2: not exercised (the union's eigenvalue of largest Re s is outside the reference set)");
    }
    ch.le("Q1 MP residuals", max_residual(&r.modes), 1e-8);
    ch.le("I1 MP isotropy", r.isotropy, 1e-10);
    ch.finish();
}
