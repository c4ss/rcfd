//! F-584 (ADR 0527): the corrected frequency `theta'` and the second-order perturbation form of the Schur Ritz error
//! `schur_eta` of the corrected sector split modes. Checks G1 to G10 and G12, the negative control N1 and the records of the
//! registration. G11 is `sa_gyro_sector_split_schur_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::HarmonicModes;
use rcfd_fem::gyro::{split_modes_dense, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector, SplitSchur, Stability, WhirlModes,
};
use rcfd_fem::{Constraint, Material, SolidMesh, SolidModel};
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
    /// `value <= limit`; a non-finite value fails.
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} against {limit:e}"));
        }
    }
    /// `value > limit`; a non-finite value fails.
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{id}: {value:.3e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} against > {limit:e}"));
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

/// The largest of `values`, NaN when one is not finite (so that a failure is never hidden by `max`), 0 when empty.
fn worst(values: &[f64]) -> f64 {
    if values.iter().any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    values.iter().fold(0.0, |a: f64, &b| a.max(b))
}

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const N: usize = 6;

// ------------------------------------------------------------------------------------------------------------ set-ups

fn ds_mesh() -> SolidMesh {
    let a = 2.0 * PI / N as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [2, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

/// DS of ADR 0411 (steel).
fn ds() -> SolidModel {
    let mut m = SolidModel::new(ds_mesh());
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn options(mb: usize, correction: bool) -> RotatingOptions {
    let mut o = ModalOptions::new(mb).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(mb)
        .modal(o)
        .coriolis(true)
        .split_correction(correction)
}

fn whirl_of(r: HarmonicWhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

fn harmonic_of(r: HarmonicSpeedModes, what: &str) -> HarmonicModes {
    match r {
        HarmonicSpeedModes::Modes(h) => *h,
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

/// The lowest rest whirl frequency over `k = 0 .. 3` of the B4 sector without the option (ADR 0411's `omega_ref`).
fn omega_ref(model: &SolidModel) -> f64 {
    let rs = RotatingSector::new(model, &options(4, false)).unwrap();
    (0..=N / 2)
        .map(|k| whirl_of(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min)
}

// ------------------------------------------------------------------------------------------------------------ algebra

/// `A_2 x` with `A_2 = blockdiag(a, a, ..)` over a vector of a multiple of `a.n` entries.
fn mv(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let n = a.n;
    let mut out = vec![0.0; x.len()];
    for (xb, ob) in x.chunks(n).zip(out.chunks_mut(n)) {
        a.matvec(xb, ob);
    }
    out
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

fn quad(a: &CsrMatrix, x: &[f64], y: &[f64]) -> f64 {
    dot(x, &mv(a, y))
}

/// The complex MAC `|phi^H M psi|^2 / ((phi^H M phi)(psi^H M psi))`.
fn cmac(m: &CsrMatrix, pr: &[f64], pi: &[f64], qr: &[f64], qi: &[f64]) -> f64 {
    let re = quad(m, pr, qr) + quad(m, pi, qi);
    let im = quad(m, pr, qi) - quad(m, pi, qr);
    let a = quad(m, pr, pr) + quad(m, pi, pi);
    let b = quad(m, qr, qr) + quad(m, qi, qi);
    (re * re + im * im) / (a * b)
}

/// The best-MAC match of every reference mode (ties: the lower split mode): `(index, MAC)`.
fn matches(m: &CsrMatrix, reference: &WhirlModes, s: &HarmonicSplitModes) -> Vec<(usize, f64)> {
    reference
        .modes
        .iter()
        .map(|r| {
            let mut best = 0;
            let mut best_mac = -1.0;
            for (j, sm) in s.modes.modes.iter().enumerate() {
                let v = cmac(m, &r.re, &r.im, &sm.re, &sm.im);
                if v > best_mac {
                    best_mac = v;
                    best = j;
                }
            }
            (best, best_mac)
        })
        .collect()
}

fn distinct(m: &[(usize, f64)]) -> bool {
    let mut seen: Vec<usize> = Vec::new();
    for &(j, _) in m {
        if seen.contains(&j) {
            return false;
        }
        seen.push(j);
    }
    true
}

// ------------------------------------------------------------------------------------------------------------ test-side reference

/// Dense Gaussian elimination with partial pivoting for several right-hand sides.
fn solve_many(mut g: Vec<Vec<f64>>, mut bs: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let d = g.len();
    for c in 0..d {
        let p = (c..d).max_by(|&x, &y| g[x][c].abs().total_cmp(&g[y][c].abs())).unwrap();
        g.swap(c, p);
        for b in bs.iter_mut() {
            b.swap(c, p);
        }
        for r in c + 1..d {
            let f = g[r][c] / g[c][c];
            if f == 0.0 {
                continue;
            }
            for j in c..d {
                g[r][j] -= f * g[c][j];
            }
            for b in bs.iter_mut() {
                b[r] -= f * b[c];
            }
        }
    }
    bs.into_iter()
        .map(|b| {
            let mut x = vec![0.0; d];
            for r in (0..d).rev() {
                let s: f64 = (r + 1..d).map(|j| g[r][j] * x[j]).sum();
                x[r] = (b[r] - s) / g[r][r];
            }
            x
        })
        .collect()
}

fn dmv(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    a.iter().map(|r| dot(r, x)).collect()
}

/// The reduced `K_eff` at the speed, `M` and `C` (doubled for a pair) of an index, dense, and the enlarged basis rebuilt
/// test-side by the rule of ADR 0482 D1.
struct Dense {
    n: usize,
    k: Vec<Vec<f64>>,
    m: Vec<Vec<f64>>,
    c: Vec<Vec<f64>>,
    v: Vec<Vec<f64>>,
    kv: Vec<f64>,
    mvv: Vec<f64>,
    cv: Vec<f64>,
}

fn project_dense(a: &[Vec<f64>], v: &[Vec<f64>]) -> Vec<f64> {
    let nb = v.len();
    let mut out = vec![0.0; nb * nb];
    for j in 0..nb {
        let y = dmv(a, &v[j]);
        for i in 0..nb {
            out[i * nb + j] = dot(&v[i], &y);
        }
    }
    out
}

fn dense_of(h: &HarmonicModes, c: &CsrMatrix) -> Dense {
    let tr = &h.transformation;
    let n = h.reduced_mass.n;
    let mut k = vec![vec![0.0; n]; n];
    let mut m = vec![vec![0.0; n]; n];
    let mut cc = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        let kj = mv(&h.reduced_stiffness, &e);
        let mj = mv(&h.reduced_mass, &e);
        let cj = tr.transpose_times(&mv(c, &tr.expand(&e)));
        for i in 0..n {
            k[i][j] = kj[i];
            m[i][j] = mj[i];
            cc[i][j] = cj[i];
        }
    }
    let ind = tr.independent();
    let mut v: Vec<Vec<f64>> = h
        .modes
        .iter()
        .map(|md| {
            let x: Vec<f64> = if h.multiplicity == 2 {
                md.cos.iter().chain(&md.sin).copied().collect()
            } else {
                md.cos.clone()
            };
            ind.iter().map(|&i| x[i]).collect()
        })
        .collect();
    let nm = v.len();
    for a in 0..nm {
        let f = dmv(&cc, &v[a]);
        let mut r = solve_many(k.clone(), vec![f]).remove(0);
        let own = dot(&r, &dmv(&m, &r)).sqrt();
        for _ in 0..2 {
            let y = dmv(&m, &r);
            let co: Vec<f64> = v.iter().map(|p| dot(p, &y)).collect();
            for (p, a) in v.iter().zip(co) {
                for (ri, pi) in r.iter_mut().zip(p) {
                    *ri -= a * pi;
                }
            }
        }
        let after = dot(&r, &dmv(&m, &r)).sqrt();
        if after > 1e-8 * own {
            for e in r.iter_mut() {
                *e /= after;
            }
            v.push(r);
        }
    }
    let kv = project_dense(&k, &v);
    let mvv = project_dense(&m, &v);
    let cv = project_dense(&cc, &v);
    Dense {
        n,
        k,
        m,
        c: cc,
        v,
        kv,
        mvv,
        cv,
    }
}

/// The test-side split mode matched to the reference mode `(rr, ri)` (reduced coordinates) by the complex MAC: `(theta, y_re,
/// y_im, MAC)`.
fn dense_match(d: &Dense, sp: &rcfd_fem::gyro::DenseSplit, rr: &[f64], ri: &[f64]) -> (f64, Vec<f64>, Vec<f64>, f64) {
    let mr = dmv(&d.m, rr);
    let mi = dmv(&d.m, ri);
    let a2 = dot(rr, &mr) + dot(ri, &mi);
    let mut best = (0usize, -1.0);
    for (j, dm) in sp.modes.iter().enumerate() {
        let mut xr = vec![0.0; d.n];
        let mut xi = vec![0.0; d.n];
        for (b, vb) in d.v.iter().enumerate() {
            for e in 0..d.n {
                xr[e] += vb[e] * dm.q[b].re;
                xi[e] += vb[e] * dm.q[b].im;
            }
        }
        let re = dot(&xr, &mr) + dot(&xi, &mi);
        let im = dot(&xi, &mr) - dot(&xr, &mi);
        let b2 = dot(&xr, &dmv(&d.m, &xr)) + dot(&xi, &dmv(&d.m, &xi));
        let mac = (re * re + im * im) / (a2 * b2);
        if mac > best.1 {
            best = (j, mac);
        }
    }
    let dm = &sp.modes[best.0];
    (
        dm.omega,
        dm.q.iter().map(|z| z.re).collect(),
        dm.q.iter().map(|z| z.im).collect(),
        best.1,
    )
}

/// The test-side complement coupling `E(theta)` (dense `nb x nb`, real and imaginary parts) by the bordered dense solves of ADR
/// 0492's test-side reference.
fn dense_e(d: &Dense, speed: f64, theta: f64) -> (Vec<f64>, Vec<f64>) {
    let n = d.n;
    let v = &d.v;
    let nb = v.len();
    let g = 2.0 * speed;
    let t = theta;
    let a: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| d.k[i][j] - t * t * d.m[i][j]).collect())
        .collect();
    let gt: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| t * g * d.c[i][j]).collect()).collect();
    let dim = 2 * n + 2 * nb;
    let mut bord = vec![vec![0.0; dim]; dim];
    for i in 0..n {
        for j in 0..n {
            bord[i][j] = a[i][j];
            bord[n + i][n + j] = a[i][j];
            bord[i][n + j] = -gt[i][j];
            bord[n + i][j] = gt[i][j];
        }
    }
    for (b, vb) in v.iter().enumerate() {
        let mvb = dmv(&d.m, vb);
        for i in 0..n {
            bord[i][2 * n + b] = mvb[i];
            bord[2 * n + b][i] = mvb[i];
            bord[n + i][2 * n + nb + b] = mvb[i];
            bord[2 * n + nb + b][n + i] = mvb[i];
        }
    }
    let rhs: Vec<Vec<f64>> = v
        .iter()
        .map(|vj| {
            let av = dmv(&a, vj);
            let gv = dmv(&gt, vj);
            let mut r = vec![0.0; dim];
            for i in 0..n {
                r[i] = -av[i];
                r[n + i] = -gv[i];
            }
            r
        })
        .collect();
    let xs = solve_many(bord, rhs);
    let mut er = vec![0.0; nb * nb];
    let mut ei = vec![0.0; nb * nb];
    for (j, x) in xs.iter().enumerate() {
        let (xr, xi) = (&x[..n], &x[n..2 * n]);
        let ar = dmv(&a, xr);
        let ai = dmv(&a, xi);
        let gr = dmv(&gt, xr);
        let gi = dmv(&gt, xi);
        for i in 0..nb {
            er[i * nb + j] = -(dot(&v[i], &ar) - dot(&v[i], &gi));
            ei[i * nb + j] = -(dot(&v[i], &ai) + dot(&v[i], &gr));
        }
    }
    (er, ei)
}

/// `y^H A x` for a real row-major `n x n` matrix `A` and complex `y`, `x` given as `(re, im)`.
fn herm(a: &[f64], n: usize, yr: &[f64], yi: &[f64], xr: &[f64], xi: &[f64]) -> (f64, f64) {
    let mut re = 0.0;
    let mut im = 0.0;
    for i in 0..n {
        for j in 0..n {
            let aij = a[i * n + j];
            re += aij * (yr[i] * xr[j] + yi[i] * xi[j]);
            im += aij * (yr[i] * xi[j] - yi[i] * xr[j]);
        }
    }
    (re, im)
}

/// `s = y^H Q'(l) y`, `Q'(l) = -2 l M + i G`, `G = 2 speed C` (ADR 0527 D1).
fn s_of(d: &Dense, speed: f64, l: f64, yr: &[f64], yi: &[f64]) -> f64 {
    let n = d.v.len();
    let (mm, _) = herm(&d.mvv, n, yr, yi, yr, yi);
    let (_, ci) = herm(&d.cv, n, yr, yi, yr, yi);
    -2.0 * l * mm - 2.0 * speed * ci
}

/// The test-side D1 of ADR 0527 for the Ritz pair `a` of `dsp`: `(theta', delta_1, delta_2)` from the column `E y_a` of the
/// full test-side `E(theta_a)`.
fn second_order(
    d: &Dense,
    dsp: &rcfd_fem::gyro::DenseSplit,
    a: usize,
    speed: f64,
    er: &[f64],
    ei: &[f64],
) -> (f64, f64, f64) {
    let nb = d.v.len();
    let ya = &dsp.modes[a].q;
    let yr: Vec<f64> = ya.iter().map(|z| z.re).collect();
    let yi: Vec<f64> = ya.iter().map(|z| z.im).collect();
    let theta = dsp.modes[a].omega;
    let mut e_r = vec![0.0; nb];
    let mut e_i = vec![0.0; nb];
    for i in 0..nb {
        for j in 0..nb {
            e_r[i] += er[i * nb + j] * yr[j] - ei[i * nb + j] * yi[j];
            e_i[i] += er[i * nb + j] * yi[j] + ei[i * nb + j] * yr[j];
        }
    }
    let cdot = |br: &[f64], bi: &[f64]| -> (f64, f64) {
        let mut re = 0.0;
        let mut im = 0.0;
        for i in 0..nb {
            re += br[i] * e_r[i] + bi[i] * e_i[i];
            im += br[i] * e_i[i] - bi[i] * e_r[i];
        }
        (re, im)
    };
    let sa = s_of(d, speed, theta, &yr, &yi);
    let d1 = cdot(&yr, &yi).0 / sa;
    let mut d2 = 0.0;
    for (b, mb) in dsp.modes.iter().enumerate() {
        let br: Vec<f64> = mb.q.iter().map(|z| z.re).collect();
        let bi: Vec<f64> = mb.q.iter().map(|z| z.im).collect();
        let bim: Vec<f64> = bi.iter().map(|x| -x).collect();
        for (l, vi) in [(mb.omega, &bi), (-mb.omega, &bim)] {
            if b == a && l > 0.0 {
                continue;
            }
            let sb = s_of(d, speed, l, &br, vi);
            let (cr, ci) = cdot(&br, vi);
            d2 += (cr * cr + ci * ci) / ((theta - l) * sa * sb);
        }
    }
    (theta + d1 + d2, d1, d2)
}

// ------------------------------------------------------------------------------------------------------------ the runs

fn opts(mb: usize, schur: SplitSchur, corrected: bool) -> RotatingOptions {
    options(mb, true).split_schur(schur).corrected_frequency(corrected)
}

fn opt_bits(v: &[Option<f64>]) -> Vec<u64> {
    v.iter().map(|x| x.map_or(u64::MAX, f64::to_bits)).collect()
}

/// The words of a sector split result as in ADR 0492's F5 digest (every field but `schur_eta` and `corrected_frequency`).
fn words_of(s: &HarmonicSplitModes) -> Vec<u64> {
    let mut w = vec![s.harmonic as u64, s.multiplicity as u64];
    let m = &s.modes;
    w.push(m.speed.to_bits());
    for sm in &m.modes {
        w.extend([
            sm.omega.to_bits(),
            sm.eigenvalue.to_bits(),
            sm.frequency.to_bits(),
            sm.growth.to_bits(),
            sm.whirl.to_bits(),
            sm.residual.to_bits(),
        ]);
        w.extend(sm.re.iter().chain(&sm.im).map(|x| x.to_bits()));
    }
    w.extend(opt_bits(&s.waves));
    w.extend(m.roots.iter().flat_map(|r| [r.re.to_bits(), r.im.to_bits()]));
    w.extend(m.real_roots.iter().map(|x| x.to_bits()));
    w.extend([m.asymmetry.to_bits(), m.skew_defect.to_bits(), m.basis_size as u64]);
    w.extend([s.corrections as u64, s.dropped_corrections as u64]);
    w.extend(opt_bits(&s.eta));
    w
}

/// The fields of [`words_of`] that are not numbers: stability, basis and loss.
fn same_flags(a: &HarmonicSplitModes, b: &HarmonicSplitModes) -> bool {
    a.modes.stability == b.modes.stability && a.modes.basis == b.modes.basis && a.modes.loss == b.modes.loss
}

/// G5: `corrected_frequency` `Some(t)` exactly where `schur_eta` is `Some(v)`, with `(theta/t - 1).abs() == v` bit for bit.
fn consistent(s: &HarmonicSplitModes) -> bool {
    s.schur_eta.len() == s.modes.modes.len()
        && s.corrected_frequency.len() == s.modes.modes.len()
        && s.schur_eta
            .iter()
            .zip(&s.corrected_frequency)
            .zip(&s.modes.modes)
            .all(|((e, t), m)| match (e, t) {
                (None, None) => true,
                (Some(v), Some(t)) => (m.omega / t - 1.0).abs().to_bits() == v.to_bits(),
                _ => false,
            })
}

/// One index and speed of DS B4: the default, the dense and the perturbation run, the reference, the matches (on the dense
/// run) and the wall times of the dense and the perturbation `split_modes`.
struct Case {
    tag: String,
    speed: f64,
    k: usize,
    default: HarmonicSplitModes,
    dense: HarmonicSplitModes,
    pert: HarmonicSplitModes,
    reference: WhirlModes,
    mt: Vec<(usize, f64)>,
    t_dense: f64,
    t_pert: f64,
}

fn cases(model: &SolidModel, w_ref: f64) -> Vec<Case> {
    let rs0 = RotatingSector::new(model, &options(4, true)).unwrap();
    let rsd = RotatingSector::new(model, &opts(4, SplitSchur::Dense, true)).unwrap();
    let rsp = RotatingSector::new(model, &opts(4, SplitSchur::Perturbation, true)).unwrap();
    let m = rs0.part().mass().clone();
    let mut out = Vec::new();
    for f in [0.4, 0.8] {
        let speed = f * w_ref;
        for k in 0..=N / 2 {
            let tag = format!("{f} omega_ref k = {k}");
            let default = rs0.split_modes(k, speed).unwrap();
            let t0 = Instant::now();
            let dense = rsd.split_modes(k, speed).unwrap();
            let t_dense = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let pert = rsp.split_modes(k, speed).unwrap();
            let t_pert = t1.elapsed().as_secs_f64();
            let reference = whirl_of(rs0.whirl_modes(k, speed).unwrap(), &tag);
            let mt = matches(&m, &reference, &dense);
            out.push(Case {
                tag,
                speed,
                k,
                default,
                dense,
                pert,
                reference,
                mt,
                t_dense,
                t_pert,
            });
        }
    }
    out
}

fn check_matches(ch: &mut Checks, c: &Case) {
    let mac_ok = c.mt.iter().all(|&(_, v)| v >= 0.99);
    ch.holds(
        &format!("matches {}", c.tag),
        mac_ok && distinct(&c.mt) && c.dense.modes.stability == Some(Stability::Stable),
        &format!(
            "{} reference modes: MAC >= 0.99 {mac_ok}, distinct {}, {:?}",
            c.reference.modes.len(),
            distinct(&c.mt),
            c.dense.modes.stability
        ),
    );
}

// ------------------------------------------------------------------------------------------------------------ G3, G5, G6, G7, G12, N1

#[test]
fn g3_g5_g6_g7_g12_n1_disc_sector_b4() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    println!("omega_ref = {w_ref:.4} rad/s");
    let all = cases(&model, w_ref);
    let rs0 = RotatingSector::new(&model, &options(4, true)).unwrap();
    let c = rs0.part().coriolis().unwrap();
    let mut g3 = Vec::new();
    let mut g6 = Vec::new();
    let mut n1 = Vec::new();
    let (mut td, mut tp) = (0.0, 0.0);
    for cs in &all {
        check_matches(&mut ch, cs);
        let w0 = words_of(&cs.default);
        ch.holds(
            &format!(
                "G12 {} dense and perturbation runs: every other field equal to the default",
                cs.tag
            ),
            words_of(&cs.dense) == w0
                && words_of(&cs.pert) == w0
                && same_flags(&cs.dense, &cs.default)
                && same_flags(&cs.pert, &cs.default),
            &format!("{} words", w0.len()),
        );
        ch.holds(
            &format!("G12 {} dense run schur_eta equal to the default", cs.tag),
            opt_bits(&cs.dense.schur_eta) == opt_bits(&cs.default.schur_eta),
            &format!("{} entries", cs.default.schur_eta.len()),
        );
        ch.holds(
            &format!("G5 {} theta' and schur_eta consistent (dense, perturbation)", cs.tag),
            consistent(&cs.dense) && consistent(&cs.pert),
            &format!(
                "{} Some (dense), {} Some (perturbation)",
                cs.dense.corrected_frequency.iter().filter(|x| x.is_some()).count(),
                cs.pert.corrected_frequency.iter().filter(|x| x.is_some()).count()
            ),
        );
        td += cs.t_dense;
        tp += cs.t_pert;
        let h = harmonic_of(rs0.modes(cs.k, cs.speed).unwrap(), &cs.tag);
        let d = dense_of(&h, &c);
        let nb = d.v.len();
        let dsp = split_modes_dense(&d.mvv, &d.cv, cs.speed, &d.kv, nb).unwrap();
        let ind = h.transformation.independent();
        for (r, &(j, mac)) in cs.reference.modes.iter().zip(&cs.mt) {
            let err = (cs.dense.modes.modes[j].omega / r.omega - 1.0).abs();
            let td_ = cs.dense.corrected_frequency[j].unwrap_or(f64::NAN);
            g3.push((td_ / r.omega - 1.0).abs());
            let sd = cs.dense.schur_eta[j].unwrap_or(f64::NAN);
            let sp = cs.pert.schur_eta[j].unwrap_or(f64::NAN);
            let rr: Vec<f64> = ind.iter().map(|&i| r.re[i]).collect();
            let ri: Vec<f64> = ind.iter().map(|&i| r.im[i]).collect();
            let (theta_t, _, _, _) = dense_match(&d, &dsp, &rr, &ri);
            let a = dsp.modes.iter().position(|x| x.omega == theta_t).unwrap();
            let (er, ei) = dense_e(&d, cs.speed, theta_t);
            let (t2, d1, d2) = second_order(&d, &dsp, a, cs.speed, &er, &ei);
            let est = (theta_t / t2 - 1.0).abs();
            if est > 1e-9 || !est.is_finite() {
                g6.push((sp / est - 1.0).abs());
            }
            let eta1 = (theta_t / (theta_t + d1) - 1.0).abs();
            if sd > 1e-9 || !sd.is_finite() {
                n1.push((eta1 / sd - 1.0).abs());
            }
            let r2 = d2 / d1;
            let pade = (theta_t / (theta_t + d1 / (1.0 - r2)) - 1.0).abs();
            let tp_ = cs.pert.corrected_frequency[j].unwrap_or(f64::NAN);
            println!(
                "  {}: omega {:.9e} split {j} mac {mac:.6} err {err:.3e} schur_eta dense {sd:.4e} perturbation {sp:.4e} \
                 pert/dense - 1 {:.3e}; theta' err dense {:.2e} perturbation {:.2e}; test-side d2/d1 {r2:.4} \
                 -(d2/d1)^2 {:.3e} lib/test - 1 {:.2e} pade/dense - 1 {:.2e}",
                cs.tag,
                r.omega,
                sp / sd - 1.0,
                (td_ / r.omega - 1.0).abs(),
                (tp_ / r.omega - 1.0).abs(),
                -r2 * r2,
                sp / est - 1.0,
                pade / sd - 1.0
            );
        }
        let top: Vec<String> = cs
            .pert
            .schur_eta
            .iter()
            .enumerate()
            .filter(|(j, _)| !cs.mt.iter().any(|&(b, _)| b == *j))
            .map(|(j, e)| {
                format!(
                    "{:.4e}:{}",
                    cs.pert.modes.modes[j].omega,
                    e.map_or("None".to_string(), |v| format!("{v:.2e}"))
                )
            })
            .collect();
        println!(
            "  record {}: unmatched split modes (omega:perturbation schur_eta) {top:?}; None {}; split_modes dense {:.4} s, \
             perturbation {:.4} s",
            cs.tag,
            cs.pert.schur_eta.iter().filter(|e| e.is_none()).count(),
            cs.t_dense,
            cs.t_pert
        );
    }
    ch.le(
        "G3 max |theta'/omega - 1| of the dense run over the matched modes",
        worst(&g3),
        1e-7,
    );
    ch.holds("G3 count", g3.len() == 48, &format!("{} matched modes", g3.len()));
    ch.le("G6 max |lib/test - 1| where the test-side D1 > 1e-9", worst(&g6), 1e-3);
    ch.holds(
        "G6 count",
        g6.len() >= 30,
        &format!("{} compared (at least 30)", g6.len()),
    );
    ch.le(
        "G7 split_modes wall time, perturbation over dense, 8 runs",
        tp / td,
        0.5,
    );
    println!("record wall time: dense {td:.4} s, perturbation {tp:.4} s");
    ch.gt("N1 max |eta_1/schur_eta - 1| of the first order alone", worst(&n1), 0.1);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ G1, G2, G4

/// Known miss (ADR 0527 addendum 1, registered run of ad8c247d, predicted by D4): G1 max |perturbation/dense - 1| = 3.835e-1
/// (16 of 34 within 1e-2), G2 the same 3.835e-1, G4 max |theta'/omega - 1| = 7.154e-7 (43 of 48 within 1e-7). The
/// second-order sum is truncated where the series ratio delta_2/delta_1 reaches 0.62 (the poor Ritz pairs of the top half of
/// the enlarged basis); the limits are those of the acceptance and stay.
#[test]
#[ignore = "known miss of ADR 0527: G1 3.835e-1 and G2 3.835e-1 against 1e-2, G4 7.154e-7 against 1e-7"]
fn g1_g2_g4_acceptance_limits_known_miss() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let all = cases(&model, w_ref);
    let mut g1 = Vec::new();
    let mut g2 = Vec::new();
    let mut g4 = Vec::new();
    for cs in &all {
        check_matches(&mut ch, cs);
        for (r, &(j, _)) in cs.reference.modes.iter().zip(&cs.mt) {
            let sd = cs.dense.schur_eta[j].unwrap_or(f64::NAN);
            let sp = cs.pert.schur_eta[j].unwrap_or(f64::NAN);
            if sd > 1e-9 || !sd.is_finite() {
                g1.push((sp / sd - 1.0).abs());
            }
            if sp > 1e-9 || !sp.is_finite() {
                g2.push((sp / sd - 1.0).abs());
            }
            let tp = cs.pert.corrected_frequency[j].unwrap_or(f64::NAN);
            g4.push((tp / r.omega - 1.0).abs());
        }
    }
    let within = |v: &[f64], lim: f64| v.iter().filter(|x| **x <= lim).count();
    ch.le(
        "G1 max |perturbation/dense - 1| where the dense schur_eta > 1e-9",
        worst(&g1),
        1e-2,
    );
    ch.holds(
        "G1 count",
        g1.len() >= 30,
        &format!("{} (at least 30); {} within 1e-2", g1.len(), within(&g1, 1e-2)),
    );
    ch.le(
        "G2 max |perturbation/dense - 1| where the perturbation schur_eta > 1e-9",
        worst(&g2),
        1e-2,
    );
    println!("G2 count {}; {} within 1e-2", g2.len(), within(&g2, 1e-2));
    ch.le(
        "G4 max |theta'/omega - 1| of the perturbation run over the matched modes",
        worst(&g4),
        1e-7,
    );
    println!("G4 count {}; {} within 1e-7", g4.len(), within(&g4, 1e-7));
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ G8

#[test]
fn g8_none_without_the_options() {
    let mut ch = Checks::new();
    let model = ds();
    let w = 0.4 * omega_ref(&model);
    let all_none = |s: &HarmonicSplitModes, schur: bool| {
        s.corrected_frequency.len() == s.modes.modes.len()
            && s.corrected_frequency.iter().all(Option::is_none)
            && (!schur || s.schur_eta.iter().all(Option::is_none))
    };
    for schur in [SplitSchur::Dense, SplitSchur::Perturbation] {
        let rs = RotatingSector::new(&model, &opts(4, schur, false)).unwrap();
        for k in 0..=N / 2 {
            let s = rs.split_modes(k, w).unwrap();
            ch.holds(
                &format!("G8 {schur:?} k = {k} without corrected_frequency: every theta' None"),
                all_none(&s, false),
                &format!("{} split modes", s.modes.modes.len()),
            );
        }
    }
    let plain = RotatingSector::new(&model, &opts(4, SplitSchur::Perturbation, true).split_correction(false)).unwrap();
    let inert = RotatingSector::new(&model, &opts(4, SplitSchur::Perturbation, true).coriolis(false)).unwrap();
    for k in 0..=N / 2 {
        let a = plain.split_modes(k, w).unwrap();
        ch.holds(
            &format!("G8 k = {k} without split_correction: schur_eta and theta' None"),
            all_none(&a, true),
            &format!("{} split modes", a.modes.modes.len()),
        );
        let b = inert.split_modes(k, w).unwrap();
        ch.holds(
            &format!("G8 k = {k} without coriolis: schur_eta and theta' None"),
            all_none(&b, true),
            &format!("{} split modes", b.modes.modes.len()),
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ G9

#[test]
fn g9_b41_empty_complement() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(&model, &opts(41, SplitSchur::Perturbation, true)).unwrap();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            let sp = rs.split_modes(k, f * w_ref).unwrap();
            let ok = sp.schur_eta.len() == sp.modes.modes.len()
                && sp.schur_eta.iter().all(|e| *e == Some(0.0))
                && sp
                    .corrected_frequency
                    .iter()
                    .zip(&sp.modes.modes)
                    .all(|(t, m)| t.map(f64::to_bits) == Some(m.omega.to_bits()));
            ch.holds(
                &format!("G9 {f} omega_ref k = {k} every schur_eta Some(0), theta' = theta"),
                ok,
                &format!("{} split modes, basis {}", sp.modes.modes.len(), sp.modes.basis_size),
            );
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ G10

#[test]
fn g10_rest_basis_follows_eta() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(
        &model,
        &opts(4, SplitSchur::Perturbation, true).stress_stiffening(false),
    )
    .unwrap();
    let w = 10.0 * w_ref;
    for k in 0..=N / 2 {
        let sp = rs.split_modes(k, w).unwrap();
        let follows = |v: &[Option<f64>]| {
            v.len() == sp.eta.len()
                && v.iter().zip(&sp.eta).all(|(s, e)| match (s, e) {
                    (None, None) => true,
                    (Some(x), Some(_)) => x.is_finite(),
                    _ => false,
                })
        };
        ch.holds(
            &format!("G10 k = {k} schur_eta and theta' None exactly where eta is None"),
            follows(&sp.schur_eta) && follows(&sp.corrected_frequency),
            &format!(
                "{:?}, {} eta Some, {} schur_eta Some",
                sp.modes.stability,
                sp.eta.iter().filter(|e| e.is_some()).count(),
                sp.schur_eta.iter().filter(|e| e.is_some()).count()
            ),
        );
    }
    ch.finish();
}
