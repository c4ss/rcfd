//! F-520 (ADR 0442): the refinement of the mode vectors of the sparse whirl state solver of ADR 0428 by inverse iteration
//! on `Q(s)` at each mode's own eigenvalue: the cube of ADR 0416 at both registered shifts with count 6 (Q1R, E1R, F1R, B1,
//! K1, W1R and the controls N1R, N2R), the cube without Coriolis with its exact double eigenvalues (K3, B1), and the point
//! masses and random problems of ADR 0428 (Q2, B1). The set-ups and helpers are copied from `si_rotating_whirl_state_sparse.rs`
//! (ADR 0428), which is not edited.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::rotating::{
    gyroscopic_state_eigen_sparse, RotatingOptions, RotatingPart, SparseStateEigen, SparseStateOptions,
    SparseStateWhirl, Stability, StateWhirlMode,
};
use rcfd_fem::{Constraint, ElasticSupport, Material, SolidModel};
use rcfd_linalg::CsrMatrix;

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

fn bits(e: &[[f64; 2]]) -> Vec<[u64; 2]> {
    e.iter().map(|s| [s[0].to_bits(), s[1].to_bits()]).collect()
}

fn flag_bits(s: Stability) -> Option<u64> {
    growth_of(s).map(f64::to_bits)
}

/// B1 on a part: everything but the modes bitwise equal between `refine(true)` and `refine(false)`.
fn same_run(a: &SparseStateWhirl, b: &SparseStateWhirl) -> bool {
    bits(&a.eigenvalues) == bits(&b.eigenvalues)
        && a.stability.is_stable() == b.stability.is_stable()
        && flag_bits(a.stability) == flag_bits(b.stability)
        && a.real_defect.to_bits() == b.real_defect.to_bits()
        && a.steps == b.steps
        && a.isotropy.to_bits() == b.isotropy.to_bits()
        && a.loss == b.loss
        && a.unknowns == b.unknowns
        && a.modes.len() == b.modes.len()
}

fn pairs(e: &SparseStateEigen) -> Vec<[f64; 2]> {
    e.growth.iter().zip(&e.omegas).map(|(g, w)| [*g, *w]).collect()
}

/// B1 on a matrix problem.
fn same_eigen(a: &SparseStateEigen, b: &SparseStateEigen) -> bool {
    bits(&a.eigenvalues) == bits(&b.eigenvalues)
        && a.stability.is_stable() == b.stability.is_stable()
        && flag_bits(a.stability) == flag_bits(b.stability)
        && a.real_defect.to_bits() == b.real_defect.to_bits()
        && a.steps == b.steps
        && a.isotropy.to_bits() == b.isotropy.to_bits()
        && bits(&pairs(a)) == bits(&pairs(b))
}

/// `|x^H y| / (||x|| ||y||)` of two modes on the model's equations.
fn cosine(x: &StateWhirlMode, y: &StateWhirlMode) -> f64 {
    let (mut re, mut im, mut nx, mut ny) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..x.re.len() {
        re += x.re[i] * y.re[i] + x.im[i] * y.im[i];
        im += x.re[i] * y.im[i] - x.im[i] * y.re[i];
        nx += x.re[i] * x.re[i] + x.im[i] * x.im[i];
        ny += y.re[i] * y.re[i] + y.im[i] * y.im[i];
    }
    f64::hypot(re, im) / f64::sqrt(nx * ny)
}

/// K1: `(max 1 - |cos| over the compared modes, the number compared, the number of far modes compared)`; the dense reported
/// mode of the matched eigenvalue (within 1e-8 relative) with a relative gap above 1e-6 to every other dense eigenvalue.
fn own_vector(sparse: &[StateWhirlMode], dense: &[StateWhirlMode], all: &[[f64; 2]]) -> (f64, usize, usize) {
    let (mut worst, mut count, mut far): (f64, usize, usize) = (0.0, 0, 0);
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
        if gap > 1e-6 {
            worst = worst.max(1.0 - cosine(x, y));
            count += 1;
            if cabs(s) > 5e4 {
                far += 1;
            }
        }
    }
    (worst, count, far)
}

/// The clusters of the returned modes by the radius 1e-9 of ADR 0442 D3 (transitive): the index lists with two or more.
fn clusters_of(modes: &[StateWhirlMode]) -> Vec<Vec<usize>> {
    let len = modes.len();
    let mut label: Vec<usize> = (0..len).collect();
    loop {
        let mut changed = false;
        for a in 0..len {
            for b in a + 1..len {
                let (sa, sb) = ([modes[a].growth, modes[a].omega], [modes[b].growth, modes[b].omega]);
                if (sa[0] - sb[0]).hypot(sa[1] - sb[1]) <= 1e-9 * cabs(sa).max(cabs(sb)) && label[a] != label[b] {
                    let l = label[a].min(label[b]);
                    label[a] = l;
                    label[b] = l;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = Vec::new();
    for l in 0..len {
        let c: Vec<usize> = (0..len).filter(|&i| label[i] == l).collect();
        if c.len() >= 2 {
            out.push(c);
        }
    }
    out
}

#[test]
fn q1r_e1r_f1r_b1_k1_w1r_n1r_n2r_cube() {
    let mut ch = Checks::new();
    let (mut q1r, mut e1r, mut w1r, mut k1, mut n1r): (f64, f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut valid, mut flags, mut expected, mut b1, mut far_ok) = (true, true, true, true, true);
    let (mut w1n, mut k1n) = (0, 0);
    let mut n2r_worst: f64 = f64::INFINITY;
    let mut n2r_pairs = 0;
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
                let r = part
                    .whirl_state_sparse(w, &SparseStateOptions::new(sigma, 6).refine(true))
                    .unwrap();
                let r0 = part
                    .whirl_state_sparse(w, &SparseStateOptions::new(sigma, 6).refine(false))
                    .unwrap();
                print_sparse(&format!("{tag} {f} omega_0 {sname}"), &r);
                println!(
                    "  residuals refined {:?}",
                    r.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
                );
                println!(
                    "  residuals unrefined {:?}",
                    r0.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
                );
                let (dev, ok, _, _) = compare(&r.eigenvalues, &d.eigenvalues, sigma);
                println!(
                    "  E1R deviation {dev:.3e}, boundary valid {ok}; dense {:?}",
                    d.stability
                );
                e1r = e1r.max(dev);
                valid &= ok;
                flags &= same_flag(r.stability, d.stability);
                expected &= r.stability.is_stable() == stable[j];
                b1 &= same_run(&r, &r0);
                q1r = q1r.max(max_residual(&r.modes));
                n1r = n1r.max(max_residual(&r0.modes));
                let (wd, wn) = whirl_dev(&r.modes, &d.modes, &d.eigenvalues);
                w1r = w1r.max(wd);
                w1n += wn;
                let (kd, kn, kf) = own_vector(&r.modes, &d.modes, &d.eigenvalues);
                println!("  K1 1 - |cos| {kd:.3e} over {kn} modes, {kf} far");
                k1 = k1.max(kd);
                k1n += kn;
                far_ok &= kf >= 3;
                if tag == "CBI" && f == 1.2 && sname == "sigma_a" {
                    // N2R: each far refined mode against the dense vectors of the other two far modes.
                    let far: Vec<&StateWhirlMode> = r.modes.iter().filter(|x| x.omega > 5e4).collect();
                    for x in &far {
                        for y in &d.modes {
                            let neighbour = y.omega > 5e4
                                && (y.omega - x.omega).abs() > 1e-8 * x.omega
                                && far.iter().any(|z| (z.omega - y.omega).abs() <= 1e-8 * y.omega);
                            if neighbour {
                                let c = 1.0 - cosine(x, y);
                                println!("  N2R far {} against dense {}: 1 - |cos| {c:.3e}", x.omega, y.omega);
                                n2r_worst = n2r_worst.min(c);
                                n2r_pairs += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    ch.le("Q1R cube residuals, refined", q1r, 1e-8);
    ch.holds("E1R boundary", valid, "every reference boundary valid");
    ch.le("E1R cube eigenvalues against the dense state solver", e1r, 1e-9);
    ch.holds(
        "F1R flags",
        flags,
        "the sparse flag equals the dense flag at every speed and shift",
    );
    ch.holds(
        "F1R expected flags",
        expected,
        "Stable at CBI 1.2, 1.5, 2.0 and CBA 1.3; Unstable at CBA 1.05, 1.1, 1.18",
    );
    ch.holds(
        "B1 cube",
        b1,
        "refine(true) bitwise equal to refine(false) but for the modes",
    );
    println!("K1 cube: {k1n} modes compared");
    ch.le("K1 cube 1 - |cos| against the dense vector", k1, 1e-8);
    ch.holds("K1 far modes", far_ok, "at least 3 far modes compared in every solve");
    println!("W1R cube: {w1n} modes compared");
    ch.le("W1R cube whirl numbers, refined", w1r, 1e-6);
    ch.gt("N1R cube residuals, unrefined", n1r, 1e-8);
    println!("N2R: {n2r_pairs} pairs");
    ch.holds("N2R pairs", n2r_pairs == 6, "6 pairs compared");
    ch.gt("N2R 1 - |cos| against the neighbours' dense vectors", n2r_worst, 1e-3);
    ch.finish();
}

#[test]
fn k3_b1_cluster_cube_without_coriolis() {
    let mut ch = Checks::new();
    let cb = cube(W0, W0);
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(12).coriolis(false)).unwrap();
    let w = 1.5 * W0;
    let (mut res, mut cos, mut b1, mut nclusters): (f64, f64, bool, usize) = (0.0, 0.0, true, 0);
    for (sname, sigma) in [("sigma_a", SIGMA_A), ("sigma_b", SIGMA_B)] {
        let r = off
            .whirl_state_sparse(w, &SparseStateOptions::new(sigma, 6).refine(true))
            .unwrap();
        let r0 = off
            .whirl_state_sparse(w, &SparseStateOptions::new(sigma, 6).refine(false))
            .unwrap();
        print_sparse(&format!("N3C CBI 1.5 off {sname}"), &r);
        println!(
            "  residuals refined {:?}",
            r.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
        );
        b1 &= same_run(&r, &r0);
        res = res.max(max_residual(&r.modes));
        for c in clusters_of(&r.modes) {
            nclusters += 1;
            for (a, &i) in c.iter().enumerate() {
                for &j in &c[a + 1..] {
                    let x = cosine(&r.modes[i], &r.modes[j]);
                    let x0 = cosine(&r0.modes[i], &r0.modes[j]);
                    println!(
                        "  cluster at {:?}: |cos| refined {x:.6e}, unrefined {x0:.6e} (information)",
                        (r.modes[i].growth, r.modes[i].omega)
                    );
                    cos = cos.max(x);
                }
            }
        }
    }
    ch.le("K3 residuals", res, 1e-8);
    println!("K3: {nclusters} clusters");
    ch.holds(
        "K3 clusters",
        nclusters >= 1,
        "at least one cluster over the two shifts",
    );
    ch.le("K3 |cos| within a cluster", cos, 1.0 - 1e-3);
    ch.holds(
        "B1 N3C",
        b1,
        "refine(true) bitwise equal to refine(false) but for the modes",
    );
    ch.finish();
}

#[test]
fn q2_b1_algebraic() {
    let mut ch = Checks::new();
    let (mut q2, mut b1): (f64, bool) = (0.0, true);
    let mut run = |tag: &str, k: &[f64], m: &[f64], g: &[f64], n: usize, sigma: f64, count: usize| {
        let (kc, mc, gc) = (csr(k, n), csr(m, n), csr(g, n));
        let on = SparseStateOptions::new(sigma, count).refine(true);
        let off = SparseStateOptions::new(sigma, count).refine(false);
        let r = gyroscopic_state_eigen_sparse(&kc, &mc, &gc, &on).unwrap();
        let r0 = gyroscopic_state_eigen_sparse(&kc, &mc, &gc, &off).unwrap();
        println!(
            "{tag}: modes {:?}, residuals refined {:?}, unrefined {:?}",
            pairs(&r),
            r.residuals,
            r0.residuals
        );
        q2 = r.residuals.iter().fold(q2, |a, &b| a.max(b));
        b1 &= same_eigen(&r, &r0);
    };
    let (k, m, g) = point_mass(W0, W0, 1.5 * W0);
    run("PMI", &k, &m, &g, 2, SIGMA_A, 2);
    let (k, m, g) = point_mass(W0, WY, 1.1 * W0);
    run("PMA", &k, &m, &g, 2, SIGMA_A, 2);
    let mut st = 19u64;
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
        run(&format!("RND n = {n}"), &k, &m, &g, n, 1.0, count);
    }
    ch.le("Q2 PM and RND residuals, refined", q2, 1e-8);
    ch.holds(
        "B1 PM and RND",
        b1,
        "refine(true) bitwise equal to refine(false) but for the modes",
    );
    ch.finish();
}
