//! F-575 (ADR 0492): the Ritz error `schur_eta` of the Schur-complemented projected problem for the corrected sector split
//! modes. Checks F1, F2, F3, F4, F6, F7, F8, the negative controls N1 and N2 and the records of the registration. F5 is
//! `sa_gyro_sector_schur_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::HarmonicModes;
use rcfd_fem::gyro::{split_modes_dense, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector, Stability, WhirlModes,
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

/// The test-side condensed estimate `|theta/theta' - 1|` of the dense split mode `(theta, y)` (ADR 0492 D2 with the complement
/// solves as one bordered dense system). `gyro`: whether the gyroscopic term enters the complement operator and coupling (false
/// for the control N2).
fn dense_estimate(d: &Dense, speed: f64, theta: f64, yre: &[f64], yim: &[f64], gyro: bool) -> f64 {
    let n = d.n;
    let v = &d.v;
    let nb = v.len();
    let g = 2.0 * speed;
    let t = theta;
    let a: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| d.k[i][j] - t * t * d.m[i][j]).collect())
        .collect();
    let gt: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if gyro { t * g * d.c[i][j] } else { 0.0 }).collect())
        .collect();
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
    let mut k2 = d.kv.clone();
    let mut c2 = d.cv.clone();
    for i in 0..nb {
        for j in 0..nb {
            k2[i * nb + j] -= 0.5 * (er[i * nb + j] + er[j * nb + i]);
            if speed > 0.0 {
                c2[i * nb + j] -= 0.5 * (ei[i * nb + j] - ei[j * nb + i]) / (g * t);
            }
        }
    }
    let ds = match split_modes_dense(&d.mvv, &c2, speed, &k2, nb) {
        Ok(x) => x,
        Err(_) => return f64::NAN,
    };
    let mut best = (f64::NAN, -1.0);
    for dm in &ds.modes {
        let mut re = 0.0;
        let mut im = 0.0;
        for i in 0..nb {
            for j in 0..nb {
                let mij = d.mvv[i * nb + j];
                re += mij * (yre[i] * dm.q[j].re + yim[i] * dm.q[j].im);
                im += mij * (yre[i] * dm.q[j].im - yim[i] * dm.q[j].re);
            }
        }
        let ov = re * re + im * im;
        if ov > best.1 {
            best = (dm.omega, ov);
        }
    }
    (theta / best.0 - 1.0).abs()
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

// ------------------------------------------------------------------------------------------------------------ F1 .. F4, N1, N2

#[test]
fn f1_f2_f3_f4_f6_n1_n2_disc_sector_b4() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    println!("omega_ref = {w_ref:.4} rad/s");
    let rs = RotatingSector::new(&model, &options(4, true)).unwrap();
    let part = rs.part();
    let m = part.mass();
    let c = part.coriolis().unwrap();
    let mut f1 = Vec::new();
    let mut f2 = Vec::new();
    let mut f4 = Vec::new();
    let mut n1 = Vec::new();
    let mut n2 = Vec::new();
    let mut low = Vec::new();
    let mut f1_count = 0;
    for f in [0.4, 0.8] {
        let speed = f * w_ref;
        for k in 0..=N / 2 {
            let tag = format!("{f} omega_ref k = {k}");
            let t0 = Instant::now();
            let sp = rs.split_modes(k, speed).unwrap();
            let t_with = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let _ = rs.split_campbell(&[k], &[speed]).unwrap();
            let t_without = t1.elapsed().as_secs_f64();
            let s = &sp.modes;
            ch.holds(
                &format!("F6 {tag} lengths"),
                sp.schur_eta.len() == s.modes.len() && sp.eta.len() == s.modes.len(),
                &format!("{} schur_eta, {} split modes", sp.schur_eta.len(), s.modes.len()),
            );
            ch.holds(
                &format!("set-up {tag} stable"),
                s.stability == Some(Stability::Stable),
                &format!("{:?}", s.stability),
            );
            let reference = whirl_of(rs.whirl_modes(k, speed).unwrap(), &tag);
            let mt = matches(m, &reference, &sp);
            let mac_ok = mt.iter().all(|&(_, v)| v >= 0.99);
            ch.holds(
                &format!("F1 {tag} matches"),
                mac_ok && distinct(&mt),
                &format!(
                    "{} reference modes: MAC >= 0.99 {mac_ok}, distinct {}",
                    reference.modes.len(),
                    distinct(&mt)
                ),
            );
            // The test-side reference.
            let h = harmonic_of(rs.modes(k, speed).unwrap(), &tag);
            let d = dense_of(&h, &c);
            let dsp = split_modes_dense(&d.mvv, &d.cv, speed, &d.kv, d.v.len()).unwrap();
            let ind = h.transformation.independent();
            let mut f3_ok = true;
            for (r, &(j, mac)) in reference.modes.iter().zip(&mt) {
                let err = (s.modes[j].omega / r.omega - 1.0).abs();
                let se = sp.schur_eta[j];
                let eta = sp.eta[j];
                if !matches!(se, Some(v) if v.is_finite() && v >= 0.0) {
                    f3_ok = false;
                }
                let sv = se.unwrap_or(f64::NAN);
                let q = err / sv;
                if sv > 1e-9 {
                    f1.push((q - 1.0).abs());
                    f1_count += 1;
                } else if sv.is_finite() {
                    low.push(q);
                }
                if err > 1e-9 {
                    f2.push((q - 1.0).abs());
                }
                if let Some(e) = eta {
                    if e > 1e-9 {
                        n1.push((err / e - 1.0).abs());
                    }
                }
                let rr: Vec<f64> = ind.iter().map(|&i| r.re[i]).collect();
                let ri: Vec<f64> = ind.iter().map(|&i| r.im[i]).collect();
                let (theta_t, yre, yim, mac_t) = dense_match(&d, &dsp, &rr, &ri);
                let est = dense_estimate(&d, speed, theta_t, &yre, &yim, true);
                let est_ng = dense_estimate(&d, speed, theta_t, &yre, &yim, false);
                if est > 1e-9 || !est.is_finite() {
                    f4.push((sv / est - 1.0).abs());
                }
                if sv > 1e-9 {
                    let err_t = (theta_t / r.omega - 1.0).abs();
                    n2.push((err_t / est_ng - 1.0).abs());
                }
                println!(
                    "  {tag}: omega {:.9e} split {j} mac {mac:.6} err {err:.3e} schur_eta {sv:.4e} err/schur {q:.5} eta {:.3e} \
                     err/eta {:.4}; test-side theta rel {:.1e} mac {mac_t:.6} est {est:.4e} lib/test {:.6}",
                    r.omega,
                    eta.unwrap_or(f64::NAN),
                    err / eta.unwrap_or(f64::NAN),
                    (theta_t / s.modes[j].omega - 1.0).abs(),
                    sv / est
                );
            }
            ch.holds(
                &format!("F3 {tag} schur_eta of every matched mode Some, finite, >= 0"),
                f3_ok,
                &format!("{} matched", mt.len()),
            );
            let top: Vec<String> = sp
                .schur_eta
                .iter()
                .enumerate()
                .filter(|(j, _)| !mt.iter().any(|&(b, _)| b == *j))
                .map(|(j, e)| {
                    format!(
                        "{:.4e}:{}",
                        s.modes[j].omega,
                        e.map_or("None".to_string(), |v| format!("{v:.2e}"))
                    )
                })
                .collect();
            let nones = sp.schur_eta.iter().filter(|e| e.is_none()).count();
            println!(
                "  record {tag}: unmatched split modes (omega:schur_eta) {top:?}; None {nones}; split_modes {t_with:.4} s, \
                 split_campbell at the speed (no schur_eta) {t_without:.4} s"
            );
        }
    }
    ch.le("F1 max |err/schur_eta - 1| where schur_eta > 1e-9", worst(&f1), 0.1);
    ch.holds(
        "F1 count of matches with schur_eta > 1e-9",
        f1_count >= 30,
        &format!("{f1_count} (at least 30)"),
    );
    ch.le("F2 max |err/schur_eta - 1| where err > 1e-9", worst(&f2), 0.1);
    ch.holds(
        "F2 count",
        !f2.is_empty(),
        &format!("{} matches with err > 1e-9", f2.len()),
    );
    ch.le(
        "F4 max |lib/test - 1| where the test-side estimate > 1e-9",
        worst(&f4),
        1e-2,
    );
    ch.holds("F4 count", !f4.is_empty(), &format!("{} compared", f4.len()));
    ch.gt(
        "N1 max |err/eta - 1| of ADR 0482's eta where eta > 1e-9",
        worst(&n1),
        0.1,
    );
    ch.gt(
        "N2 max |err/est - 1| of the test-side estimate without the gyroscopic coupling",
        worst(&n2),
        0.1,
    );
    if !low.is_empty() {
        let lo = low.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b));
        let hi = low.iter().fold(f64::NEG_INFINITY, |a: f64, &b| a.max(b));
        println!(
            "record err/schur_eta where schur_eta <= 1e-9: [{lo:.5}, {hi:.5}] over {} modes",
            low.len()
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ F6

#[test]
fn f6_none_without_the_option() {
    let mut ch = Checks::new();
    let model = ds();
    let w = 0.4 * omega_ref(&model);
    let plain = RotatingSector::new(&model, &options(4, false)).unwrap();
    let inert = RotatingSector::new(&model, &options(4, true).coriolis(false)).unwrap();
    for k in 0..=N / 2 {
        let a = plain.split_modes(k, w).unwrap();
        ch.holds(
            &format!("F6 k = {k} without split_correction"),
            a.schur_eta.len() == a.modes.modes.len() && a.schur_eta.iter().all(Option::is_none),
            &format!(
                "{} entries, {} Some",
                a.schur_eta.len(),
                a.schur_eta.iter().filter(|e| e.is_some()).count()
            ),
        );
        let b = inert.split_modes(k, w).unwrap();
        ch.holds(
            &format!("F6 k = {k} split_correction without coriolis"),
            b.schur_eta.len() == b.modes.modes.len() && b.schur_eta.iter().all(Option::is_none),
            &format!(
                "{} entries, {} Some",
                b.schur_eta.len(),
                b.schur_eta.iter().filter(|e| e.is_some()).count()
            ),
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ F7

#[test]
fn f7_b41_empty_complement() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(&model, &options(41, true)).unwrap();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            let sp = rs.split_modes(k, f * w_ref).unwrap();
            let ok = sp.schur_eta.len() == sp.modes.modes.len() && sp.schur_eta.iter().all(|e| *e == Some(0.0));
            ch.holds(
                &format!("F7 {f} omega_ref k = {k} every schur_eta Some(0)"),
                ok,
                &format!(
                    "{} split modes, basis {}, {} Some(0)",
                    sp.modes.modes.len(),
                    sp.modes.basis_size,
                    sp.schur_eta.iter().filter(|e| **e == Some(0.0)).count()
                ),
            );
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ F8

#[test]
fn f8_rest_basis_follows_eta() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&model);
    let rs = RotatingSector::new(&model, &options(4, true).stress_stiffening(false)).unwrap();
    let w = 10.0 * w_ref;
    for k in 0..=N / 2 {
        let sp = rs.split_modes(k, w).unwrap();
        let ok = sp.schur_eta.len() == sp.eta.len()
            && sp.schur_eta.iter().zip(&sp.eta).all(|(s, e)| match (s, e) {
                (None, None) => true,
                (Some(v), Some(_)) => v.is_finite(),
                _ => false,
            });
        ch.holds(
            &format!("F8 k = {k} schur_eta None exactly where eta is None"),
            ok,
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
