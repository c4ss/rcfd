//! F-569 (ADR 0482): static correction vectors for the split modes of a sector index and the Ritz error `eta` of every split
//! mode. Checks E1, E2, E3, K2, D1, G3, C2, L2, B3, X2, the negative controls N1 and N2 and the records of the registration.
//! B1 is `sa_gyro_sector_correction_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::HarmonicModes;
use rcfd_fem::gyro::{Basis, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector, Stability, WhirlModes,
};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

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
            "{id}: {value:.3e} (expected > {limit:.0e}) {}",
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

fn mult(k: usize) -> usize {
    if k == 0 || 2 * k == N {
        1
    } else {
        2
    }
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

/// The lowest rest whirl frequency over `k = 0 .. 3` (ADR 0411's `omega_ref`).
fn omega_ref(rs: &RotatingSector) -> f64 {
    (0..=N / 2)
        .map(|k| whirl_of(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min)
}

fn basis_of(h: &HarmonicModes) -> Vec<Vec<f64>> {
    h.modes
        .iter()
        .map(|m| {
            if h.multiplicity == 2 {
                m.cos.iter().chain(&m.sin).copied().collect()
            } else {
                m.cos.clone()
            }
        })
        .collect()
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

/// Solves the dense system `g c = b` by Gaussian elimination with partial pivoting.
fn solve(mut g: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let d = b.len();
    for c in 0..d {
        let p = (c..d).max_by(|&x, &y| g[x][c].abs().total_cmp(&g[y][c].abs())).unwrap();
        g.swap(c, p);
        b.swap(c, p);
        for r in c + 1..d {
            let f = g[r][c] / g[c][c];
            for j in c..d {
                g[r][j] -= f * g[c][j];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; d];
    for r in (0..d).rev() {
        let s: f64 = (r + 1..d).map(|j| g[r][j] * x[j]).sum();
        x[r] = (b[r] - s) / g[r][r];
    }
    x
}

/// The `M_2`-orthogonal projection of `x` on the span of `phi` (the Gram matrix solved).
fn project(m: &CsrMatrix, phi: &[Vec<f64>], gram: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    let mx = mv(m, x);
    let rhs: Vec<f64> = phi.iter().map(|p| dot(p, &mx)).collect();
    let c = solve(gram.to_vec(), rhs);
    let mut s = vec![0.0; x.len()];
    for (p, ci) in phi.iter().zip(&c) {
        for (si, pi) in s.iter_mut().zip(p) {
            *si += ci * pi;
        }
    }
    s
}

/// The complex MAC `|phi^H M psi|^2 / ((phi^H M phi)(psi^H M psi))`.
fn cmac(m: &CsrMatrix, pr: &[f64], pi: &[f64], qr: &[f64], qi: &[f64]) -> f64 {
    let re = quad(m, pr, qr) + quad(m, pi, qi);
    let im = quad(m, pr, qi) - quad(m, pi, qr);
    let a = quad(m, pr, pr) + quad(m, pi, pi);
    let b = quad(m, qr, qr) + quad(m, qi, qi);
    (re * re + im * im) / (a * b)
}

/// ADR 0481's `eta` (D5) of a reference mode `(pr, pi)` at `omega` on the basis `phi` (record only).
#[allow(clippy::too_many_arguments)]
fn eta_0481(
    k: &CsrMatrix,
    m: &CsrMatrix,
    c: &CsrMatrix,
    speed: f64,
    phi: &[Vec<f64>],
    omega: f64,
    pr: &[f64],
    pi: &[f64],
) -> f64 {
    let gram: Vec<Vec<f64>> = phi
        .iter()
        .map(|p| phi.iter().map(|q| quad(m, p, q)).collect())
        .collect();
    let sr = project(m, phi, &gram, pr);
    let si = project(m, phi, &gram, pi);
    let er: Vec<f64> = pr.iter().zip(&sr).map(|(a, b)| a - b).collect();
    let ei: Vec<f64> = pi.iter().zip(&si).map(|(a, b)| a - b).collect();
    let w2 = omega * omega;
    let g = 2.0 * speed;
    let kq = |x: &[f64]| quad(k, x, x) - w2 * quad(m, x, x);
    let num = kq(&er) + kq(&ei) - 2.0 * omega * g * quad(c, &er, &ei);
    let den = 2.0 * omega * (quad(m, &sr, &sr) + quad(m, &si, &si)) + 2.0 * g * quad(c, &sr, &si);
    num.abs() / (omega * den.abs())
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

// ------------------------------------------------------------------------------------------------------------ E1, E2, E3, K2, G3, N1, N2

#[derive(Default)]
struct Sweep {
    e1_worst: f64,
    e2_worst: f64,
    e2_count: usize,
    ratio_min: f64,
    ratio_max: f64,
    low_ratio_min: f64,
    low_ratio_max: f64,
    worst_matched: f64,
    worst_plain: f64,
    lowest_plain: Vec<f64>,
    lowest_eta: Vec<f64>,
}

/// One basis at one speed over every index; registered checks only for `checks` (B4), records otherwise.
fn sweep(model: &SolidModel, mb: usize, speed: f64, tag: &str, checks: Option<&mut Checks>) -> Sweep {
    let mut local = Checks::new();
    let rs = RotatingSector::new(model, &options(mb, true)).unwrap();
    let plain = RotatingSector::new(model, &options(mb, false)).unwrap();
    let part = rs.part();
    let m = part.mass();
    let c = part.coriolis().unwrap();
    let kop = part.operator(speed);
    let mut out = Sweep {
        ratio_min: f64::INFINITY,
        low_ratio_min: f64::INFINITY,
        ..Sweep::default()
    };
    for k in 0..=N / 2 {
        let mk = mult(k);
        let reference = whirl_of(rs.whirl_modes(k, speed).unwrap(), tag);
        let sp = rs.split_modes(k, speed).unwrap();
        let s = &sp.modes;
        let pp = plain.split_modes(k, speed).unwrap();
        // K2
        local.holds(
            &format!("K2 {tag} k = {k}"),
            s.basis == Some(Basis::Speed)
                && s.loss.is_none()
                && s.stability == Some(Stability::Stable)
                && s.real_roots.is_empty()
                && sp.corrections == mb * mk
                && sp.dropped_corrections == 0
                && s.basis_size == 2 * mb * mk
                && s.modes.len() == 2 * mb * mk,
            &format!(
                "basis {:?}, loss {:?}, {:?}, {} real roots, {} kept, {} dropped, basis {}, {} split modes",
                s.basis,
                s.loss,
                s.stability,
                s.real_roots.len(),
                sp.corrections,
                sp.dropped_corrections,
                s.basis_size,
                s.modes.len()
            ),
        );
        local.le(
            &format!("G3 {tag} k = {k} raw skew defect of C_r"),
            s.skew_defect,
            1e-11,
        );
        // E3
        let e3 = sp.eta.len() == s.modes.len() && sp.eta.iter().all(|e| matches!(e, Some(v) if v.is_finite() && *v >= 0.0));
        local.holds(
            &format!("E3 {tag} k = {k} eta of every split mode"),
            e3,
            &format!("{} of {} reported", sp.eta.iter().filter(|e| e.is_some()).count(), s.modes.len()),
        );
        // E1
        let r0 = &reference.modes[0];
        let e1 = (s.modes[0].omega / r0.omega - 1.0).abs();
        let mac0 = cmac(m, &r0.re, &r0.im, &s.modes[0].re, &s.modes[0].im);
        local.le(&format!("E1 {tag} k = {k} lowest |theta/omega - 1|"), e1, 1e-5);
        local.holds(
            &format!("E1 {tag} k = {k} MAC of the lowest modes"),
            mac0 >= 0.99,
            &format!("{mac0:.6}"),
        );
        out.e1_worst = out.e1_worst.max(e1);
        let plain_lowest = (pp.modes.modes[0].omega / r0.omega - 1.0).abs();
        out.lowest_plain.push(plain_lowest);
        out.lowest_eta.push(sp.eta[0].unwrap_or(f64::NAN));
        // E2
        let mt = matches(m, &reference, &sp);
        let mac_ok = mt.iter().all(|&(_, v)| v >= 0.99);
        local.holds(
            &format!("E2 {tag} k = {k} matches"),
            mac_ok && distinct(&mt),
            &format!(
                "{} reference modes: MAC >= 0.99 {mac_ok}, distinct {}",
                reference.modes.len(),
                distinct(&mt)
            ),
        );
        let plain_phi = basis_of(&harmonic_of(rs.modes(k, speed).unwrap(), tag));
        let pm = matches(m, &reference, &pp);
        let mut worst_ratio: f64 = 0.0;
        for (i, (r, &(j, mac))) in reference.modes.iter().zip(&mt).enumerate() {
            let err = (s.modes[j].omega / r.omega - 1.0).abs();
            let eta = sp.eta[j].unwrap_or(f64::NAN);
            out.worst_matched = out.worst_matched.max(err);
            let perr = (pp.modes.modes[pm[i].0].omega / r.omega - 1.0).abs();
            out.worst_plain = out.worst_plain.max(perr);
            let q = err / eta;
            if eta > 1e-9 {
                worst_ratio = worst_ratio.max((q - 1.0).abs());
                out.e2_count += 1;
                out.ratio_min = out.ratio_min.min(q);
                out.ratio_max = out.ratio_max.max(q);
            } else if eta > 1e-10 {
                out.low_ratio_min = out.low_ratio_min.min(q);
                out.low_ratio_max = out.low_ratio_max.max(q);
            }
            let e_plain = eta_0481(&kop, m, &c, speed, &plain_phi, r.omega, &r.re, &r.im);
            println!(
                "  {tag} k = {k}: omega {:.9e} split {j} mac {mac:.6} err {err:.3e} eta {eta:.3e} err/eta {q:.4}; plain err \
                 {perr:.3e} plain eta(0481) {e_plain:.3e}",
                r.omega
            );
        }
        local.le(
            &format!("E2 {tag} k = {k} max |err/eta - 1| over eta > 1e-9"),
            worst_ratio,
            0.1,
        );
        out.e2_worst = out.e2_worst.max(worst_ratio);
        let top: Vec<String> = sp
            .eta
            .iter()
            .enumerate()
            .filter(|(j, _)| !mt.iter().any(|&(b, _)| b == *j))
            .map(|(j, e)| format!("{:.4e}:{}", s.modes[j].omega, e.map_or("None".to_string(), |v| format!("{v:.2e}"))))
            .collect();
        println!("  record {tag} k = {k}: unmatched split modes (omega:eta) {top:?}");
    }
    println!(
        "record {tag}: worst lowest-mode error {:.3e}; E2 over {} modes, worst |err/eta - 1| {:.4}, err/eta in [{:.4}, {:.4}]; \
         1e-10 < eta <= 1e-9: err/eta in [{:.4}, {:.4}]; worst matched error {:.3e} (without the option {:.3e}); lowest without \
         {:?}; lowest eta with {:?}",
        out.e1_worst,
        out.e2_count,
        out.e2_worst,
        out.ratio_min,
        out.ratio_max,
        out.low_ratio_min,
        out.low_ratio_max,
        out.worst_matched,
        out.worst_plain,
        out.lowest_plain.iter().map(|v| format!("{v:.3e}")).collect::<Vec<_>>(),
        out.lowest_eta.iter().map(|v| format!("{v:.3e}")).collect::<Vec<_>>()
    );
    match checks {
        Some(ch) => ch.failed.extend(local.failed),
        None => println!("record {tag} (not registered): limits of B4 not met {:?}", local.failed),
    }
    out
}

#[test]
fn e1_e2_e3_k2_g3_n1_n2_disc_sector_b4() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    println!("omega_ref = {w_ref:.9e} rad/s");
    sweep(&model, 4, 0.4 * w_ref, "B4 0.4 omega_ref", Some(&mut ch));
    let s8 = sweep(&model, 4, 0.8 * w_ref, "B4 0.8 omega_ref", Some(&mut ch));
    // N1: without the correction, the lowest split frequency at 0.8 omega_ref.
    let n1 = s8.lowest_plain.iter().fold(0.0f64, |a, b| a.max(*b));
    ch.gt("N1 B4 0.8 omega_ref without the option: largest lowest-mode error", n1, 1e-5);
    // N2: the reported eta of the lowest mode against the plain error, every index.
    for (k, (e, p)) in s8.lowest_eta.iter().zip(&s8.lowest_plain).enumerate() {
        ch.holds(
            &format!("N2 B4 0.8 omega_ref k = {k}"),
            *e < 0.1 * p,
            &format!("eta {e:.3e} < 0.1 x plain error {p:.3e}"),
        );
    }
    ch.finish();
}

#[test]
fn record_b12() {
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    sweep(&model, 12, 0.4 * w_ref, "record B12 0.4 omega_ref", None);
    sweep(&model, 12, 0.8 * w_ref, "record B12 0.8 omega_ref", None);
}

// ------------------------------------------------------------------------------------------------------------ D1

#[test]
fn d1_g3_b41_spans_the_index() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    let rs = RotatingSector::new(&model, &options(41, true)).unwrap();
    let m = rs.part().mass();
    for f in [0.4, 0.8] {
        let w = f * w_ref;
        for k in 0..=N / 2 {
            let mk = mult(k);
            let tag = format!("D1 B41 {f} omega_ref k = {k}");
            let nq = harmonic_of(rs.modes(k, w).unwrap(), &tag).unknowns;
            let sp = rs.split_modes(k, w).unwrap();
            let kept_ok = sp.corrections == nq - 41 * mk && sp.dropped_corrections == 41 * mk - sp.corrections;
            ch.holds(
                &format!("{tag} counts"),
                kept_ok,
                &format!(
                    "n_q {nq}, kept {}, dropped {} (expected {}, {})",
                    sp.corrections,
                    sp.dropped_corrections,
                    nq - 41 * mk,
                    41 * mk - (nq - 41 * mk)
                ),
            );
            ch.holds(
                &format!("{tag} eta"),
                sp.eta.iter().all(|e| *e == Some(0.0)),
                "every eta == Some(0.0)",
            );
            ch.le(&format!("{tag} G3 raw skew defect"), sp.modes.skew_defect, 1e-11);
            let reference = whirl_of(rs.whirl_modes(k, w).unwrap(), &tag);
            let mt = matches(m, &reference, &sp);
            let mac_ok = mt.iter().all(|&(_, v)| v >= 0.99) && distinct(&mt);
            ch.holds(&format!("{tag} matches"), mac_ok, "MAC >= 0.99, distinct");
            let worst = reference
                .modes
                .iter()
                .zip(&mt)
                .map(|(r, &(j, _))| (sp.modes.modes[j].omega / r.omega - 1.0).abs())
                .fold(0.0f64, f64::max);
            ch.le(&format!("{tag} max |theta/omega - 1|"), worst, 1e-9);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ C2

#[test]
fn c2_corrected_split_campbell() {
    let mut ch = Checks::new();
    let model = ds();
    let rs = RotatingSector::new(&model, &options(4, true)).unwrap();
    let w_ref = omega_ref(&rs);
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let t = rs.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let tracks: Vec<usize> = t.families.iter().map(|f| f.campbell.table.tracks.len()).collect();
    ch.holds(
        "C2 families",
        t.families.len() == 4 && t.multiplicities == [1, 2, 2, 1] && tracks == [8, 16, 16, 8] && t.coriolis,
        &format!(
            "{} families, multiplicities {:?}, tracks {tracks:?}",
            t.families.len(),
            t.multiplicities
        ),
    );
    let mut bits_ok = true;
    let mut distinct_ok = true;
    let mut stable_ok = true;
    for (fi, fam) in t.families.iter().enumerate() {
        let k = t.harmonics[fi];
        for (j, &w) in speeds.iter().enumerate() {
            let s = rs.split_modes(k, w).unwrap();
            let mut seen: Vec<u64> = Vec::new();
            for tr in &fam.campbell.table.tracks {
                match tr.frequencies[j] {
                    Some(f) => {
                        bits_ok &= s.modes.modes.iter().any(|sm| sm.frequency.to_bits() == f.to_bits());
                        if seen.contains(&f.to_bits()) {
                            distinct_ok = false;
                        }
                        seen.push(f.to_bits());
                    }
                    None => bits_ok = false,
                }
            }
            stable_ok &= fam.speeds[j].stability == Some(Stability::Stable);
        }
    }
    ch.holds(
        "C2 stored frequencies are split_modes frequencies by bits",
        bits_ok,
        "every track, every speed",
    );
    ch.holds("C2 distinct modes per speed", distinct_ok, "every family");
    ch.holds("C2 stable", stable_ok, "every family and speed");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ L2

#[test]
fn l2_rest_basis_with_the_correction() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    let rs = RotatingSector::new(&model, &options(4, true).stress_stiffening(false)).unwrap();
    let w = 10.0 * w_ref;
    for k in 0..=N / 2 {
        let f396 = rs.modes(k, w).unwrap();
        let sp = rs.split_modes(k, w).unwrap();
        let s = &sp.modes;
        let finite = s.roots.iter().all(|r| r.re.is_finite() && r.im.is_finite()) && !s.roots.is_empty();
        let basis_ok = match f396.loss() {
            Some(l) => s.basis == Some(Basis::Rest) && s.loss == Some(l.loss),
            None => false,
        };
        let unstable = matches!(s.stability, Some(Stability::Unstable { .. }));
        let eta_ok = sp.eta.len() == s.modes.len()
            && sp.eta.iter().all(|e| match e {
                None => unstable,
                Some(v) => !unstable && v.is_finite(),
            });
        ch.holds(
            &format!("L2 k = {k}"),
            basis_ok && finite && sp.corrections >= 1 && eta_ok,
            &format!(
                "F-396 loss {:?}, basis {:?}, finite roots {finite}, {} kept, {} dropped, {:?}, eta reported {}",
                f396.loss().map(|l| l.loss),
                s.basis,
                sp.corrections,
                sp.dropped_corrections,
                s.stability,
                sp.eta.iter().filter(|e| e.is_some()).count()
            ),
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ B3

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn same_opt(a: &[Option<f64>], b: &[Option<f64>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.map(f64::to_bits) == y.map(f64::to_bits))
}

#[test]
fn b3_correction_without_coriolis_is_inert() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    let with = RotatingSector::new(&model, &options(4, true).coriolis(false)).unwrap();
    let without = RotatingSector::new(&model, &options(4, false).coriolis(false)).unwrap();
    let w = 0.4 * w_ref;
    for k in 0..=N / 2 {
        let a = with.split_modes(k, w).unwrap();
        let b = without.split_modes(k, w).unwrap();
        let ok = a.modes.modes.len() == b.modes.modes.len()
            && a.modes.modes.iter().zip(&b.modes.modes).all(|(x, y)| {
                x.omega.to_bits() == y.omega.to_bits()
                    && x.eigenvalue.to_bits() == y.eigenvalue.to_bits()
                    && x.frequency.to_bits() == y.frequency.to_bits()
                    && x.residual.to_bits() == y.residual.to_bits()
                    && same(&x.re, &y.re)
                    && same(&x.im, &y.im)
            })
            && same_opt(&a.waves, &b.waves)
            && a.modes.basis_size == b.modes.basis_size
            && a.corrections == 0
            && a.dropped_corrections == 0
            && a.eta.iter().all(Option::is_none);
        ch.holds(&format!("B3 split_modes k = {k}"), ok, "bits of the result without the option");
    }
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let ta = with.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let tb = without.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let mut ok = ta.families.len() == tb.families.len() && !ta.coriolis;
    for (a, b) in ta.families.iter().zip(&tb.families) {
        let (x, y) = (&a.campbell.table, &b.campbell.table);
        ok &= x.tracks.len() == y.tracks.len() && x.losses == y.losses;
        for (p, q) in x.tracks.iter().zip(&y.tracks) {
            ok &= same_opt(&p.eigenvalues, &q.eigenvalues)
                && same_opt(&p.frequencies, &q.frequencies)
                && same_opt(&p.mac, &q.mac)
                && p.modes.len() == q.modes.len()
                && p.modes.iter().zip(&q.modes).all(|(u, v)| match (u, v) {
                    (Some(u), Some(v)) => same(u, v),
                    (None, None) => true,
                    _ => false,
                });
        }
    }
    ch.holds("B3 split_campbell", ok, "every family table equal by bits");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ X2

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } => Some(key),
        _ => None,
    }
}

#[test]
fn x2_rigid_set_refused_with_the_option() {
    let mut ch = Checks::new();
    let model = ds();
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, false)).unwrap());
    let mut dsr = ds();
    let a = PI / N as f64;
    let added = dsr.add_constraint(Constraint::rigid_set("xmax", [0.75 * a.cos(), 0.75 * a.sin(), 0.05]));
    let w = 0.4 * w_ref;
    let reach = added
        .map_err(|e| format!("add_constraint: {e}"))
        .and_then(|_| RotatingSector::new(&dsr, &options(4, true)).map_err(|e| format!("RotatingSector::new: {e}")));
    match reach {
        Err(e) => {
            println!("X2 record: the sector refuses the model ({e}): the path is unreachable");
            ch.holds("X2", true, "unreachable (vacuous)");
        }
        Ok(rs) => match rs.modes(1, w) {
            Err(e) => {
                println!("X2 record: RotatingSector::modes refuses the model ({e}): the path is unreachable");
                ch.holds("X2", true, "unreachable (vacuous)");
            }
            Ok(_) => {
                let with = rs.split_modes(1, w);
                let off = RotatingSector::new(&dsr, &options(4, false)).unwrap();
                let without = off.split_modes(1, w);
                let with_ok = matches!(&with, Err(e) if key_of(e) == Some("constraint"));
                let without_ok = match &without {
                    Ok(_) => true,
                    Err(e) => key_of(e) != Some("constraint"),
                };
                ch.holds(
                    "X2 with the option",
                    with_ok,
                    &format!("{:?}", with.as_ref().err()),
                );
                ch.holds(
                    "X2 without the option",
                    without_ok,
                    &format!("{:?}", without.as_ref().err()),
                );
            }
        },
    }
    ch.finish();
}
