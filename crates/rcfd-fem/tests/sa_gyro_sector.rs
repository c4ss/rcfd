//! F-555 (ADR 0481): the split Campbell table of a cyclic sector on the modal subspace: the gyroscopic problem of ADR 0472 on
//! the harmonic modes of each index with the doubled Coriolis matrix of ADR 0411 D5. Checks S1, S2, V1, K1, G3, C1, L1, B1,
//! A1, X1, the negative controls N1 and N2 and the records of the registration. B2 is `sa_gyro_sector_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::HarmonicModes;
use rcfd_fem::gyro::{Basis, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::material::{Axes, MaterialAxes};
use rcfd_fem::modal::{frequency, ModalOptions};
use rcfd_fem::rotating::{
    HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector, Stability, WhirlModes,
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
const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
/// The rule of D5.
const ETA_IN: f64 = 1e-7;
const RESIDUAL_IN: f64 = 1e-9;

// ------------------------------------------------------------------------------------------------------------ set-ups

fn iso() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        axes,
    )
    .unwrap()
}

/// `ORTHO_C` of ADR 0462 D8.
fn ortho_c() -> Material {
    ortho(MaterialAxes::cylindrical(ORIGIN, AXIS).unwrap())
}

fn ds_mesh() -> SolidMesh {
    let a = 2.0 * PI / N as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [2, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

/// DS of ADR 0411 with the material `mat`.
fn ds(mat: Material) -> SolidModel {
    let mut m = SolidModel::new(ds_mesh());
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn options(mb: usize, coriolis: bool) -> RotatingOptions {
    let mut o = ModalOptions::new(mb).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(mb).modal(o).coriolis(coriolis)
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

/// The indicator `eta` of D5 of a reference mode `(pr, pi)` at `omega` on the basis `phi`, the speed `speed`.
#[allow(clippy::too_many_arguments)]
fn eta(
    k: &CsrMatrix,
    m: &CsrMatrix,
    c: &CsrMatrix,
    speed: f64,
    phi: &[Vec<f64>],
    gram: &[Vec<f64>],
    omega: f64,
    pr: &[f64],
    pi: &[f64],
) -> f64 {
    let sr = project(m, phi, gram, pr);
    let si = project(m, phi, gram, pi);
    let er: Vec<f64> = pr.iter().zip(&sr).map(|(a, b)| a - b).collect();
    let ei: Vec<f64> = pi.iter().zip(&si).map(|(a, b)| a - b).collect();
    let w2 = omega * omega;
    let g = 2.0 * speed;
    let kq = |x: &[f64]| quad(k, x, x) - w2 * quad(m, x, x);
    let num = kq(&er) + kq(&ei) - 2.0 * omega * g * quad(c, &er, &ei);
    let den = 2.0 * omega * (quad(m, &sr, &sr) + quad(m, &si, &si)) + 2.0 * g * quad(c, &sr, &si);
    num.abs() / (omega * den.abs())
}

// ------------------------------------------------------------------------------------------------------------ S1, S2

#[derive(Default)]
struct Run {
    reference: usize,
    inside: usize,
    worst_inside: f64,
    worst_excluded: f64,
    excluded: usize,
    ratio_min: f64,
    ratio_max: f64,
    kappa_dev: f64,
    wave_dev: f64,
}

/// One index at one speed: K1, G3 and the comparison of D5 (S1 or S2).
#[allow(clippy::too_many_arguments)]
fn compare(
    rs: &RotatingSector,
    mb: usize,
    k: usize,
    speed: f64,
    tag: &str,
    c: &CsrMatrix,
    ch: &mut Checks,
    run: &mut Run,
) {
    let mk = mult(k);
    let reference = whirl_of(rs.whirl_modes(k, speed).unwrap(), tag);
    let sp: HarmonicSplitModes = rs.split_modes(k, speed).unwrap();
    let s = &sp.modes;
    let h = harmonic_of(rs.modes(k, speed).unwrap(), tag);
    let phi = basis_of(&h);
    let part = rs.part();
    let m = part.mass();
    let kop = part.operator(speed);
    // K1
    ch.holds(
        &format!("K1 {tag} k = {k}"),
        s.basis == Some(Basis::Speed)
            && s.loss.is_none()
            && s.stability == Some(Stability::Stable)
            && s.modes.len() == mb * mk
            && s.real_roots.is_empty(),
        &format!(
            "basis {:?}, loss {:?}, {:?}, {} split modes ({} expected), {} real roots",
            s.basis,
            s.loss,
            s.stability,
            s.modes.len(),
            mb * mk,
            s.real_roots.len()
        ),
    );
    ch.le(&format!("G3 {tag} k = {k} raw skew defect of C_r"), s.skew_defect, 1e-11);
    println!("record {tag} k = {k}: asymmetry of K_r, M_r {:.2e}", s.asymmetry);
    let gram: Vec<Vec<f64>> = phi
        .iter()
        .map(|p| phi.iter().map(|q| quad(m, p, q)).collect())
        .collect();
    let mut used = vec![false; s.modes.len()];
    let mut distinct = true;
    let mut mac_ok = true;
    let mut worst: f64 = 0.0;
    let mut inside = 0;
    for r in &reference.modes {
        let e = eta(&kop, m, c, speed, &phi, &gram, r.omega, &r.re, &r.im);
        let mut best = 0;
        let mut best_mac = -1.0;
        for (j, sm) in s.modes.iter().enumerate() {
            let v = cmac(m, &r.re, &r.im, &sm.re, &sm.im);
            if v > best_mac {
                best_mac = v;
                best = j;
            }
        }
        let theta = s.modes[best].omega;
        let err = (theta / r.omega - 1.0).abs();
        run.reference += 1;
        if (1e-10..=1e-3).contains(&e) {
            let q = err / e;
            run.ratio_min = run.ratio_min.min(q);
            run.ratio_max = run.ratio_max.max(q);
        }
        if e <= ETA_IN && r.residual <= RESIDUAL_IN {
            inside += 1;
            mac_ok &= best_mac >= 0.99;
            if used[best] {
                distinct = false;
            }
            used[best] = true;
            worst = worst.max(err);
            run.kappa_dev = run.kappa_dev.max((s.modes[best].whirl - r.whirl).abs());
            if let (Some(a), Some(b)) = (sp.waves[best], r.wave) {
                run.wave_dev = run.wave_dev.max((a - b).abs());
            }
        } else {
            run.excluded += 1;
            run.worst_excluded = run.worst_excluded.max(err);
        }
        println!(
            "  {tag} k = {k}: omega {:.9e} eta {e:.2e} residual {:.1e} mac {best_mac:.6} err {err:.2e}{}",
            r.omega,
            r.residual,
            if e <= ETA_IN { " inside" } else { "" }
        );
    }
    run.inside += inside;
    run.worst_inside = run.worst_inside.max(worst);
    ch.holds(
        &format!("S {tag} k = {k} matches"),
        mac_ok && distinct,
        &format!("{inside} inside of {}: MAC >= 0.99 {mac_ok}, distinct {distinct}", reference.modes.len()),
    );
    ch.le(&format!("S {tag} k = {k} max |theta/omega - 1| over the inside modes"), worst, 1e-6);
    if mb >= 41 {
        ch.holds(
            &format!("V1 {tag} k = {k}"),
            inside >= 1,
            &format!("{inside} reference modes inside the basis"),
        );
    }
}

fn sweep(model: &SolidModel, mb: usize, w_ref: f64, label: &str, ch: &mut Checks) -> Vec<(f64, Run)> {
    let rs = RotatingSector::new(model, &options(mb, true)).unwrap();
    let c = rs.part().coriolis().unwrap();
    let mut out = Vec::new();
    for factor in [0.4, 0.8] {
        let mut run = Run {
            ratio_min: f64::INFINITY,
            ..Run::default()
        };
        for k in 0..=N / 2 {
            compare(&rs, mb, k, factor * w_ref, &format!("{label} {factor} omega_ref"), &c, ch, &mut run);
        }
        println!(
            "record {label} {factor} omega_ref: {} reference modes, {} inside (worst {:.2e}), {} excluded (worst {:.2e}); \
             err/eta over 1e-10 <= eta <= 1e-3 in [{:.3}, {:.3}]; inside |kappa - kappa_ref| <= {:.2e}, |tau - tau_ref| <= {:.2e}",
            run.reference,
            run.inside,
            run.worst_inside,
            run.excluded,
            run.worst_excluded,
            run.ratio_min,
            run.ratio_max,
            run.kappa_dev,
            run.wave_dev
        );
        out.push((factor, run));
    }
    out
}

#[test]
fn s1_k1_g3_v1_n1_n2_disc_sector() {
    let mut ch = Checks::new();
    let model = ds(iso());
    let b4 = RotatingSector::new(&model, &options(4, true)).unwrap();
    let w_ref = omega_ref(&b4);
    println!("omega_ref = {w_ref:.9e} rad/s");
    let r4 = sweep(&model, 4, w_ref, "S1 B4", &mut ch);
    sweep(&model, 12, w_ref, "S1 B12", &mut ch);
    sweep(&model, 41, w_ref, "S1 B41", &mut ch);
    // N2: B4 at 0.8 omega_ref.
    let (_, run) = &r4[1];
    ch.holds(
        "N2 B4 0.8 omega_ref excluded modes",
        run.excluded >= 1,
        &format!("{} excluded", run.excluded),
    );
    ch.gt("N2 B4 0.8 omega_ref worst excluded |theta/omega - 1|", run.worst_excluded, 1e-6);
    // N1: option off against the reference with the option, index 1 at 0.8 omega_ref, B41.
    let on = RotatingSector::new(&model, &options(41, true)).unwrap();
    let off = RotatingSector::new(&model, &options(41, false)).unwrap();
    let w = 0.8 * w_ref;
    let mut fr: Vec<f64> = whirl_of(on.whirl_modes(1, w).unwrap(), "N1").frequencies_all;
    fr.sort_by(f64::total_cmp);
    let mut fo: Vec<f64> = off.split_modes(1, w).unwrap().modes.modes.iter().map(|m| m.omega).collect();
    fo.sort_by(f64::total_cmp);
    let n1 = fr
        .iter()
        .zip(&fo)
        .take(8)
        .map(|(a, b)| (b / a - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.gt("N1 option off against the reference, k = 1, 0.8 omega_ref, lowest 8", n1, 1e-3);
    // Record: zero speed with the option against F-396 at rest (B4).
    let mut z: f64 = 0.0;
    for k in 0..=N / 2 {
        let s = b4.split_modes(k, 0.0).unwrap();
        let h = harmonic_of(b4.modes(k, 0.0).unwrap(), "rest");
        for (sm, l) in s.modes.modes.iter().zip(&h.eigenvalues) {
            z = z.max((sm.omega / l.sqrt() - 1.0).abs());
        }
    }
    println!("record: zero speed with the option against F-396 at rest (B4): {z:.2e}");
    ch.finish();
}

#[test]
fn s2_k1_g3_v1_orthotropic_sector() {
    let mut ch = Checks::new();
    let model = ds(ortho_c());
    let rs = RotatingSector::new(&model, &options(41, true)).unwrap();
    let w_ref = omega_ref(&rs);
    println!("omega_ref,o = {w_ref:.9e} rad/s");
    sweep(&model, 41, w_ref, "S2 DSO B41", &mut ch);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ C1

#[test]
fn c1_sector_split_campbell() {
    let mut ch = Checks::new();
    let model = ds(iso());
    let rs = RotatingSector::new(&model, &options(4, true)).unwrap();
    let w_ref = omega_ref(&rs);
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let t = rs.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let tracks: Vec<usize> = t.families.iter().map(|f| f.campbell.table.tracks.len()).collect();
    ch.holds(
        "C1 families",
        t.families.len() == 4 && t.multiplicities == [1, 2, 2, 1] && tracks == [4, 8, 8, 4] && t.coriolis,
        &format!("{} families, multiplicities {:?}, tracks {tracks:?}", t.families.len(), t.multiplicities),
    );
    let mut bits_ok = true;
    let mut distinct_ok = true;
    let mut stable_ok = true;
    let mut mac_min = f64::INFINITY;
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
                if let Some(m) = tr.mac[j] {
                    mac_min = mac_min.min(m);
                }
            }
            stable_ok &= fam.speeds[j].stability == Some(Stability::Stable);
        }
    }
    ch.holds("C1 stored frequencies are split_modes frequencies by bits", bits_ok, "every track, every speed");
    ch.holds("C1 distinct modes per speed", distinct_ok, "every family");
    ch.holds("C1 stable", stable_ok, "every family and speed");
    println!("record C1 smallest stored MAC {mac_min:.6}; unstable {:?}", t.unstable());
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ L1

#[test]
fn l1_loss_of_stiffness_takes_the_rest_basis() {
    let mut ch = Checks::new();
    let model = ds(iso());
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, true)).unwrap());
    let rs = RotatingSector::new(&model, &options(4, true).stress_stiffening(false)).unwrap();
    let w = 10.0 * w_ref;
    let mut losses = 0;
    for k in 0..=N / 2 {
        let f396 = rs.modes(k, w).unwrap();
        let s = rs.split_modes(k, w).unwrap().modes;
        let finite = s.roots.iter().all(|r| r.re.is_finite() && r.im.is_finite()) && !s.roots.is_empty();
        let ok = match f396.loss() {
            Some(l) => {
                losses += 1;
                s.basis == Some(Basis::Rest) && s.loss == Some(l.loss)
            }
            None => s.basis == Some(Basis::Speed) && s.loss.is_none(),
        };
        ch.holds(
            &format!("L1 k = {k}"),
            ok && finite,
            &format!("F-396 loss {:?}, basis {:?}, finite roots {finite}, {:?}", f396.loss(), s.basis, s.stability),
        );
    }
    ch.holds("L1 at least one index loses stiffness", losses >= 1, &format!("{losses} indices"));
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ B1

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn same_opt(a: &[Option<f64>], b: &[Option<f64>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.map(f64::to_bits) == y.map(f64::to_bits))
}

#[test]
fn b1_option_off_is_f396() {
    let mut ch = Checks::new();
    let model = ds(iso());
    let w_ref = omega_ref(&RotatingSector::new(&model, &options(4, true)).unwrap());
    let rs = RotatingSector::new(&model, &options(4, false)).unwrap();
    let w = 0.4 * w_ref;
    for k in 0..=N / 2 {
        let s = rs.split_modes(k, w).unwrap();
        let h = harmonic_of(rs.modes(k, w).unwrap(), "B1");
        let shapes = basis_of(&h);
        let ok = s.modes.modes.len() == h.eigenvalues.len()
            && s.modes.modes.iter().enumerate().all(|(i, sm)| {
                sm.eigenvalue.to_bits() == h.eigenvalues[i].to_bits()
                    && sm.frequency.to_bits() == frequency(h.eigenvalues[i]).to_bits()
                    && sm.residual.to_bits() == h.residuals[i].to_bits()
                    && same(&sm.re, &shapes[i])
                    && sm.im.iter().all(|x| *x == 0.0)
            })
            && s.modes.stability.is_none()
            && s.modes.basis.is_none();
        ch.holds(&format!("B1 split_modes off k = {k}"), ok, "bits of modes(k, 0.4 omega_ref)");
    }
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let t = rs.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let c = rs.campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let mut ok = !t.coriolis && t.families.len() == c.families.len();
    for (a, b) in t.families.iter().zip(&c.families) {
        let (ta, tb) = (&a.campbell.table, &b.table);
        ok &= ta.losses == tb.losses && ta.tracks.len() == tb.tracks.len() && !a.coriolis;
        for (x, y) in ta.tracks.iter().zip(&tb.tracks) {
            ok &= same_opt(&x.eigenvalues, &y.eigenvalues) && same_opt(&x.frequencies, &y.frequencies) && same_opt(&x.mac, &y.mac);
            ok &= x.modes.len() == y.modes.len()
                && x.modes.iter().zip(&y.modes).all(|(p, q)| match (p, q) {
                    (Some(p), Some(q)) => same(p, q),
                    (None, None) => true,
                    _ => false,
                });
        }
    }
    ch.holds("B1 split_campbell off", ok, "every family table equal by bits to campbell");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ A1, X1

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

fn material_refused(r: Result<RotatingSector, FemError>) -> bool {
    matches!(r, Err(FemError::NotYetSupported { item: "material", .. }))
}

#[test]
fn a1_x1_axes_and_refusals() {
    let mut ch = Checks::new();
    let dsf = ds(ortho(MaterialAxes::fixed(I3).unwrap()));
    let dsl = ds(ortho(MaterialAxes::cylindrical([0.1, 0.0, 0.0], AXIS).unwrap()));
    ch.holds(
        "A1 DSF (Fixed(I))",
        material_refused(RotatingSector::new(&dsf, &options(4, true))),
        "RotatingSector::new refused with NotYetSupported material",
    );
    ch.holds(
        "A1 DSL (cylindrical about an offset line)",
        material_refused(RotatingSector::new(&dsl, &options(4, true))),
        "RotatingSector::new refused with NotYetSupported material",
    );
    let model = ds(iso());
    let part = RotatingPart::new(&model, AXIS, ORIGIN, &options(4, true)).unwrap();
    ch.holds(
        "A1 RotatingPart::split_modes of DS",
        key_of(part.split_modes(1.0).unwrap_err()) == "cyclic",
        "refused with \"cyclic\"",
    );
    let rs = RotatingSector::new(&model, &options(4, true)).unwrap();
    ch.holds(
        "X1 harmonic 4",
        key_of(rs.split_modes(4, 1.0).unwrap_err()) == "harmonic",
        "refused with \"harmonic\"",
    );
    ch.holds(
        "X1 speed -1",
        key_of(rs.split_modes(1, -1.0).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "X1 speed NaN",
        key_of(rs.split_modes(1, f64::NAN).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "X1 harmonics [1, 0]",
        key_of(rs.split_campbell(&[1, 0], &[0.0, 1.0]).unwrap_err()) == "harmonics",
        "refused with \"harmonics\"",
    );
    ch.holds(
        "X1 speeds decreasing",
        key_of(rs.split_campbell(&[0, 1], &[0.0, 2.0, 1.0]).unwrap_err()) == "speeds",
        "refused with \"speeds\"",
    );
    ch.finish();
}
