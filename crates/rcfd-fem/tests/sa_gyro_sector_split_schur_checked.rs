//! F-612 (ADR 0535): the series-ratio indicator `|delta_2 / delta_1|` of the perturbation Schur Ritz error of the sector split
//! modes and the dense fallback of `SplitSchur::PerturbationChecked`. Checks J1 to J6, J8 to J11 and the negative controls NJ1
//! and NJ2 of the registration. J7 is `sa_gyro_sector_split_schur_checked_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::HarmonicModes;
use rcfd_fem::gyro::{split_modes_dense, HarmonicSplitModes, SPLIT_SCHUR_RATIO_LIMIT};
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

fn bits(v: &[Option<f64>]) -> Vec<u64> {
    opt_bits(v)
}

/// One index and speed of DS B4: the default, the dense, the perturbation and the checked run, the reference, the matches (on
/// the dense run) and the wall times of the dense and the checked `split_modes` (back to back).
struct Case {
    tag: String,
    speed: f64,
    k: usize,
    default: HarmonicSplitModes,
    dense: HarmonicSplitModes,
    pert: HarmonicSplitModes,
    checked: HarmonicSplitModes,
    reference: WhirlModes,
    mt: Vec<(usize, f64)>,
    t_dense: f64,
    t_checked: f64,
}

fn cases(model: &SolidModel, w_ref: f64, timing: bool) -> Vec<Case> {
    let rs0 = RotatingSector::new(model, &options(4, true)).unwrap();
    let rsd = RotatingSector::new(model, &opts(4, SplitSchur::Dense, true)).unwrap();
    let rsp = RotatingSector::new(model, &opts(4, SplitSchur::Perturbation, true)).unwrap();
    let rsc = RotatingSector::new(model, &opts(4, SplitSchur::PerturbationChecked, true)).unwrap();
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
            let pert = if timing {
                default.clone()
            } else {
                rsp.split_modes(k, speed).unwrap()
            };
            let t1 = Instant::now();
            let checked = rsc.split_modes(k, speed).unwrap();
            let t_checked = t1.elapsed().as_secs_f64();
            let reference = whirl_of(rs0.whirl_modes(k, speed).unwrap(), &tag);
            let mt = matches(&m, &reference, &dense);
            out.push(Case {
                tag,
                speed,
                k,
                default,
                dense,
                pert,
                checked,
                reference,
                mt,
                t_dense,
                t_checked,
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

fn ratio_ok(s: &HarmonicSplitModes) -> bool {
    s.perturbation_ratio.len() == s.modes.modes.len()
        && s.perturbation_ratio.iter().all(|r| r.is_some_and(|v| v.is_finite()))
}

// ------------------------------------------------------------------------------------------------------------ J1 to J5, J8, NJ1, NJ2

#[test]
fn j1_to_j5_j8_nj1_nj2_disc_sector_b4() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    println!("omega_ref = {w_ref:.4} rad/s");
    let all = cases(&model, w_ref, false);
    let rs0 = RotatingSector::new(&model, &options(4, true)).unwrap();
    let c = rs0.part().coriolis().unwrap();
    let mut j1 = Vec::new();
    let mut j2 = Vec::new();
    let mut j3 = Vec::new();
    let mut nj1 = Vec::new();
    let mut scope = 0usize;
    let mut scope_fallback = 0usize;
    let (mut fb_all, mut n_all) = (0usize, 0usize);
    for cs in &all {
        check_matches(&mut ch, cs);
        let w0 = words_of(&cs.default);
        ch.holds(
            &format!("J8 {} checked run: every other field equal to the default", cs.tag),
            words_of(&cs.checked) == w0 && same_flags(&cs.checked, &cs.default),
            &format!("{} words", w0.len()),
        );
        let n = cs.checked.modes.modes.len();
        // J3: the indicator is reported by the perturbation and checked runs only, equal between them.
        ch.holds(
            &format!("J3 {} ratio None in the default and the dense runs", cs.tag),
            cs.default.perturbation_ratio.iter().all(Option::is_none)
                && cs.dense.perturbation_ratio.iter().all(Option::is_none)
                && cs.default.perturbation_ratio.len() == n
                && cs.dense.perturbation_ratio.len() == n,
            &format!("{n} split modes"),
        );
        ch.holds(
            &format!(
                "J3 {} ratio Some finite and equal in the perturbation and checked runs",
                cs.tag
            ),
            ratio_ok(&cs.pert)
                && ratio_ok(&cs.checked)
                && bits(&cs.pert.perturbation_ratio) == bits(&cs.checked.perturbation_ratio),
            &format!("{n} split modes"),
        );
        // J4: the flags.
        let fl_ok = cs
            .checked
            .schur_fallback
            .iter()
            .zip(&cs.checked.perturbation_ratio)
            .all(|(&f, r)| f == r.is_none_or(|v| v > SPLIT_SCHUR_RATIO_LIMIT))
            && cs.checked.schur_fallback.len() == n;
        let none_fb = [&cs.default, &cs.dense, &cs.pert]
            .iter()
            .all(|s| s.schur_fallback.len() == n && s.schur_fallback.iter().all(|f| !f));
        ch.holds(
            &format!(
                "J4 {} fallback flags follow the ratio; none without the checked variant",
                cs.tag
            ),
            fl_ok && none_fb,
            &format!(
                "{} fallback of {n}",
                cs.checked.schur_fallback.iter().filter(|f| **f).count()
            ),
        );
        // J5: the values.
        let mut j5 = consistent(&cs.checked);
        for j in 0..n {
            let src = if cs.checked.schur_fallback[j] {
                &cs.dense
            } else {
                &cs.pert
            };
            j5 &= cs.checked.schur_eta[j].map(f64::to_bits) == src.schur_eta[j].map(f64::to_bits)
                && cs.checked.corrected_frequency[j].map(f64::to_bits) == src.corrected_frequency[j].map(f64::to_bits);
        }
        ch.holds(
            &format!(
                "J5 {} checked values bit equal to the dense (fallback) or perturbation (others), consistent",
                cs.tag
            ),
            j5,
            &format!("{n} split modes"),
        );
        fb_all += cs.checked.schur_fallback.iter().filter(|f| **f).count();
        n_all += n;
        // The test-side ratio and the scope of J1.
        let h = harmonic_of(rs0.modes(cs.k, cs.speed).unwrap(), &cs.tag);
        let d = dense_of(&h, &c);
        let nb = d.v.len();
        let dsp = split_modes_dense(&d.mvv, &d.cv, cs.speed, &d.kv, nb).unwrap();
        let ind = h.transformation.independent();
        for (r, &(j, mac)) in cs.reference.modes.iter().zip(&cs.mt) {
            let sd = cs.dense.schur_eta[j].unwrap_or(f64::NAN);
            let sp = cs.pert.schur_eta[j].unwrap_or(f64::NAN);
            let sc = cs.checked.schur_eta[j].unwrap_or(f64::NAN);
            let rr: Vec<f64> = ind.iter().map(|&i| r.re[i]).collect();
            let ri: Vec<f64> = ind.iter().map(|&i| r.im[i]).collect();
            let (theta_t, _, _, _) = dense_match(&d, &dsp, &rr, &ri);
            let a = dsp.modes.iter().position(|x| x.omega == theta_t).unwrap();
            let (er, ei) = dense_e(&d, cs.speed, theta_t);
            let (_, d1, d2) = second_order(&d, &dsp, a, cs.speed, &er, &ei);
            let rt = (d2 / d1).abs();
            let rl = cs.checked.perturbation_ratio[j].unwrap_or(f64::NAN);
            j3.push((rl - rt).abs());
            let fell = cs.checked.schur_fallback[j];
            if sd > 1e-9 || !sd.is_finite() {
                j1.push((sc / sd - 1.0).abs());
                nj1.push((sp / sd - 1.0).abs());
                scope += 1;
                scope_fallback += usize::from(fell);
            }
            if sc > 1e-9 || !sc.is_finite() {
                j2.push((sc / sd - 1.0).abs());
            }
            println!(
                "  {}: omega {:.9e} split {j} mac {mac:.6} ratio lib {rl:.4} test {rt:.4} fallback {fell} schur_eta dense {sd:.4e} \
                 perturbation {sp:.4e} checked {sc:.4e} checked/dense - 1 {:.2e}",
                cs.tag,
                r.omega,
                sc / sd - 1.0
            );
        }
        println!(
            "  record {}: fallback {} of {n}; split_modes dense {:.4} s, checked {:.4} s",
            cs.tag,
            cs.checked.schur_fallback.iter().filter(|f| **f).count(),
            cs.t_dense,
            cs.t_checked
        );
    }
    ch.le(
        "J1 max |checked/dense - 1| where the dense schur_eta > 1e-9",
        worst(&j1),
        1e-2,
    );
    ch.holds(
        "J1 count",
        j1.len() >= 30,
        &format!("{} compared (at least 30)", j1.len()),
    );
    ch.le(
        "J2 max |checked/dense - 1| where the checked schur_eta > 1e-9",
        worst(&j2),
        1e-2,
    );
    ch.le(
        "J3 max |library ratio - test-side ratio| over the matched modes",
        worst(&j3),
        1e-4,
    );
    ch.holds("J3 count", j3.len() == 48, &format!("{} matched modes", j3.len()));
    ch.gt(
        "NJ1 max |perturbation/dense - 1| of the unchecked run on the scope of J1",
        worst(&nj1),
        0.1,
    );
    ch.holds(
        "NJ2 fallback count over the scope of J1",
        (10..=30).contains(&scope_fallback),
        &format!("{scope_fallback} of {scope} (expected 10 to 30)"),
    );
    println!("record: fallback {fb_all} of {n_all} split modes over the 8 runs");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ J6

#[test]
fn j6_cost_of_the_checked_run() {
    let model = ds();
    let w_ref = omega_ref(&model);
    let all = cases(&model, w_ref, true);
    let (mut td, mut tc) = (0.0, 0.0);
    let mut fb = 0usize;
    let mut n = 0usize;
    for cs in &all {
        td += cs.t_dense;
        tc += cs.t_checked;
        fb += cs.checked.schur_fallback.iter().filter(|f| **f).count();
        n += cs.checked.modes.modes.len();
        println!(
            "record {}: dense {:.4} s checked {:.4} s fallback {} of {}",
            cs.tag,
            cs.t_dense,
            cs.t_checked,
            cs.checked.schur_fallback.iter().filter(|f| **f).count(),
            cs.checked.modes.modes.len()
        );
    }
    println!("record wall time: dense {td:.4} s, checked {tc:.4} s, fallback {fb} of {n}");
    let mut ch = Checks::new();
    ch.le("J6 split_modes wall time, checked over dense, 8 runs", tc / td, 0.5);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ J9

#[test]
fn j9_none_without_the_options() {
    let mut ch = Checks::new();
    let model = ds();
    let w = 0.4 * omega_ref(&model);
    let inert = |s: &HarmonicSplitModes| {
        let n = s.modes.modes.len();
        s.schur_eta.iter().all(Option::is_none)
            && s.corrected_frequency.iter().all(Option::is_none)
            && s.perturbation_ratio.len() == n
            && s.perturbation_ratio.iter().all(Option::is_none)
            && s.schur_fallback.len() == n
            && s.schur_fallback.iter().all(|f| !f)
    };
    let rs = RotatingSector::new(&model, &opts(4, SplitSchur::PerturbationChecked, false)).unwrap();
    let plain = RotatingSector::new(
        &model,
        &opts(4, SplitSchur::PerturbationChecked, true).split_correction(false),
    )
    .unwrap();
    let no_cor = RotatingSector::new(&model, &opts(4, SplitSchur::PerturbationChecked, true).coriolis(false)).unwrap();
    for k in 0..=N / 2 {
        let s = rs.split_modes(k, w).unwrap();
        ch.holds(
            &format!("J9 k = {k} without corrected_frequency: theta' None, schur_eta Some, ratio defined"),
            s.corrected_frequency.len() == s.modes.modes.len()
                && s.corrected_frequency.iter().all(Option::is_none)
                && s.schur_eta.iter().all(Option::is_some)
                && ratio_ok(&s),
            &format!("{} split modes", s.modes.modes.len()),
        );
        let a = plain.split_modes(k, w).unwrap();
        ch.holds(
            &format!("J9 k = {k} without split_correction"),
            inert(&a),
            "all None, no fallback",
        );
        let b = no_cor.split_modes(k, w).unwrap();
        ch.holds(
            &format!("J9 k = {k} without coriolis"),
            inert(&b),
            "all None, no fallback",
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ J10

#[test]
fn j10_b41_empty_complement() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(&model, &opts(41, SplitSchur::PerturbationChecked, true)).unwrap();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            let sp = rs.split_modes(k, f * w_ref).unwrap();
            let n = sp.modes.modes.len();
            let ok = sp.schur_eta.len() == n
                && sp.schur_eta.iter().all(|e| *e == Some(0.0))
                && sp
                    .corrected_frequency
                    .iter()
                    .zip(&sp.modes.modes)
                    .all(|(t, m)| t.map(f64::to_bits) == Some(m.omega.to_bits()))
                && sp.perturbation_ratio.len() == n
                && sp.perturbation_ratio.iter().all(Option::is_none)
                && sp.schur_fallback.len() == n
                && sp.schur_fallback.iter().all(|x| !x);
            ch.holds(
                &format!("J10 {f} omega_ref k = {k} schur_eta Some(0), theta' = theta, no ratio, no fallback"),
                ok,
                &format!("{n} split modes, basis {}", sp.modes.basis_size),
            );
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ J11

#[test]
fn j11_rest_basis_follows_eta() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(
        &model,
        &opts(4, SplitSchur::PerturbationChecked, true).stress_stiffening(false),
    )
    .unwrap();
    let w = 10.0 * w_ref;
    for k in 0..=N / 2 {
        let sp = rs.split_modes(k, w).unwrap();
        let n = sp.modes.modes.len();
        let follows = |v: &[Option<f64>]| {
            v.len() == sp.eta.len()
                && v.iter().zip(&sp.eta).all(|(s, e)| match (s, e) {
                    (None, None) => true,
                    (Some(x), Some(_)) => x.is_finite(),
                    _ => false,
                })
        };
        ch.holds(
            &format!("J11 k = {k} schur_eta, theta' and ratio None exactly where eta is None, no fallback there"),
            follows(&sp.schur_eta)
                && follows(&sp.corrected_frequency)
                && follows(&sp.perturbation_ratio)
                && sp.schur_fallback.len() == n
                && sp.schur_fallback.iter().zip(&sp.eta).all(|(f, e)| !f || e.is_some()),
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
