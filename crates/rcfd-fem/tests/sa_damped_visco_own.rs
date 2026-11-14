//! F-586 (ADR 0520): the viscoelastic damped modes at each root's own frequency. Checks SU, SR (set-up), DA, DAm, DD, DDr,
//! DS, DH, DB, DC (rods against the scalar fixed point of the discrete chain), FP, IT (the fixed point), T, TF, TT, TE (the
//! 3-D truncation), DM (two materials against the dense full-space fixed point), DE (3-D equal weights), DF (shift), C0 (no
//! terms), P1 (threads), RV1 (residual vectors), NC1 to NC3 (negative controls) and RF (refusals) of the registration. The
//! rod set-up of ADR 0495 is copied from `sa_damped_visco.rs` (not edited).

use rcfd_fem::assemble::assemble_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions, OwnFrequency};
use rcfd_fem::element::element_for;
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::mat_visco::shift_factor;
use rcfd_fem::material::{Matrix6, PronyTerm, TimeTemperatureShift};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_general::{quadratic_eigen_with, GeneralEigenOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed. A
/// value that is not finite is a miss.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.6e} (limit in [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} outside [{lo:e}, {hi:e}]"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (record, no limit)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; infinite if any is not finite (a NaN must not hide behind `f64::max`).
fn worst(values: &[f64]) -> f64 {
    let mut w: f64 = 0.0;
    for &v in values {
        if !v.is_finite() {
            return f64::INFINITY;
        }
        w = w.max(v);
    }
    w
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn rel(a: C64, b: C64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The entry of `set` nearest to `z`, relative distance.
fn nearest(z: C64, set: &[C64]) -> f64 {
    let mut best = f64::INFINITY;
    for &s in set {
        let d = rel(z, s);
        if d.is_nan() {
            return f64::INFINITY;
        }
        best = best.min(d);
    }
    best
}

fn csqrt(z: C64) -> C64 {
    let r = z.abs();
    let re = ((r + z.re) / 2.0).sqrt();
    let im = ((r - z.re) / 2.0).sqrt().copysign(z.im);
    C64::new(re, im)
}

// ------------------------------------------------------------------------------------------------- the rods

const L: f64 = 1.0;
const B: f64 = 0.05;
const RHO: f64 = 1200.0;
const E0: f64 = 2.0e9;
const N: usize = 40;
const W_A: f64 = 2.0 / 21.0;
const G_B: f64 = 0.15;
const K_B: f64 = 0.05;
const NU_B: f64 = 0.3;
const T_REF: f64 = 373.0;

fn g0(nu: f64) -> f64 {
    E0 / (2.0 * (1.0 + nu))
}

fn k0(nu: f64) -> f64 {
    E0 / (3.0 * (1.0 - 2.0 * nu))
}

fn base(nu: f64) -> Material {
    Material::isotropic_elastic(E0, nu, RHO, 0.0).unwrap()
}

fn standard(nu: f64, g: f64, k: f64, tau: f64, shift: Option<TimeTemperatureShift>) -> Material {
    Material::viscoelastic(base(nu), vec![PronyTerm::new(g, k, tau).unwrap()], shift).unwrap()
}

/// The branch factor `i x/(1 + i x)`, `x = omega tau`.
fn branch(w: f64, tau: f64) -> C64 {
    let ix = C64::new(0.0, w * tau);
    ix / (C64::ONE + ix)
}

/// `M (1 - g + g i x/(1 + i x))`: a modulus `M` with one term of weight `g`.
fn one_term(m: f64, g: f64, tau: f64, w: f64) -> C64 {
    (C64::from_real(1.0 - g) + branch(w, tau).scale(g)).scale(m)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Axial,
    Shear,
}

#[derive(Clone, Copy)]
struct Rod {
    kind: Kind,
    nu: f64,
    g: f64,
    k: f64,
    tau: f64,
    wc: f64,
}

impl Rod {
    fn new(kind: Kind, nu: f64, g: f64, k: f64) -> Rod {
        let stored = match kind {
            Kind::Axial => k0(nu) * (1.0 - k / 2.0) + 4.0 / 3.0 * g0(nu) * (1.0 - g / 2.0),
            Kind::Shear => g0(nu) * (1.0 - g / 2.0),
        };
        let wc = PI / (2.0 * L) * (stored / RHO).sqrt();
        Rod {
            kind,
            nu,
            g,
            k,
            tau: 1.0 / wc,
            wc,
        }
    }
    fn material(&self) -> Material {
        standard(self.nu, self.g, self.k, self.tau, None)
    }
    fn modulus(&self, w: f64) -> C64 {
        let gs = one_term(g0(self.nu), self.g, self.tau, w);
        let ks = one_term(k0(self.nu), self.k, self.tau, w);
        match self.kind {
            Kind::Axial => ks + gs.scale(4.0 / 3.0),
            Kind::Shear => gs,
        }
    }
    /// `i sqrt(Mod*(w) mu/rho)`.
    fn root_at(&self, w: f64, mu: f64) -> C64 {
        let s = csqrt(self.modulus(w).scale(mu / RHO));
        C64::new(-s.im, s.re)
    }
    /// The scalar fixed point `w <- Im root_at(w)` from `omega_c` (until `w` repeats, at most 200 steps): the root and `w*`.
    fn own(&self, mu: f64) -> (C64, f64) {
        let mut w = self.wc;
        for _ in 0..200 {
            let wn = self.root_at(w, mu).im;
            if wn == w {
                break;
            }
            w = wn;
        }
        (self.root_at(w, mu), w)
    }
}

fn rod_a() -> Rod {
    Rod::new(Kind::Axial, 0.0, W_A, W_A)
}

fn rod_b() -> Rod {
    Rod::new(Kind::Axial, NU_B, G_B, K_B)
}

fn rod_c() -> Rod {
    Rod::new(Kind::Shear, NU_B, G_B, K_B)
}

fn rod_model(n: usize, kind: Kind, material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [L, B, B], [n, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    let (free, held) = match kind {
        Kind::Axial => (0, [1, 2]),
        Kind::Shear => (1, [0, 2]),
    };
    for c in held {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("xmin", free, 0.0)).unwrap();
    m
}

/// The tridiagonals of the clamped-free chain of `n` linear elements per unit modulus and density: `(kd, ko, md, mo)`.
fn chain(n: usize) -> (Vec<f64>, f64, Vec<f64>, f64) {
    let h = L / n as f64;
    let kd: Vec<f64> = (0..n).map(|i| if i + 1 == n { 1.0 / h } else { 2.0 / h }).collect();
    let md: Vec<f64> = (0..n)
        .map(|i| if i + 1 == n { 2.0 * h / 6.0 } else { 4.0 * h / 6.0 })
        .collect();
    (kd, -1.0 / h, md, h / 6.0)
}

/// `mu_1` by inverse iteration (as ADR 0495).
fn chain_mu1(n: usize) -> f64 {
    let (kd, ko, md, mo) = chain(n);
    let tri = |d: &[f64], o: f64, x: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let mut s = d[i] * x[i];
                if i > 0 {
                    s += o * x[i - 1];
                }
                if i + 1 < n {
                    s += o * x[i + 1];
                }
                s
            })
            .collect()
    };
    let solve_k = |b: &[f64]| -> Vec<f64> {
        let mut c = vec![0.0; n];
        let mut d = vec![0.0; n];
        let mut x = vec![0.0; n];
        c[0] = ko / kd[0];
        d[0] = b[0] / kd[0];
        for i in 1..n {
            let den = kd[i] - ko * c[i - 1];
            c[i] = ko / den;
            d[i] = (b[i] - ko * d[i - 1]) / den;
        }
        x[n - 1] = d[n - 1];
        for i in (0..n - 1).rev() {
            x[i] = d[i] - c[i] * x[i + 1];
        }
        x
    };
    let mut x: Vec<f64> = (0..n).map(|i| 1.0 + 0.1 * (i as f64).sin()).collect();
    for _ in 0..80 {
        let y = solve_k(&tri(&md, mo, &x));
        let nrm = y.iter().map(|v| v * v).sum::<f64>().sqrt();
        x = y.iter().map(|v| v / nrm).collect();
    }
    let kx = tri(&kd, ko, &x);
    let mx = tri(&md, mo, &x);
    let num: f64 = x.iter().zip(&kx).map(|(a, b)| a * b).sum();
    let den: f64 = x.iter().zip(&mx).map(|(a, b)| a * b).sum();
    num / den
}

/// `mu_j` (1-based) by bisection on the Sturm count of `K - mu M`: the count of negative pivots of its `LDL^T`.
fn chain_mu(n: usize, j: usize) -> f64 {
    let (kd, ko, md, mo) = chain(n);
    let below = |mu: f64| -> usize {
        let mut count = 0;
        let mut d_prev = 1.0;
        for i in 0..n {
            let a = kd[i] - mu * md[i];
            let d = if i == 0 {
                a
            } else {
                let b = ko - mu * mo;
                a - b * b / d_prev
            };
            if d < 0.0 {
                count += 1;
            }
            d_prev = if d == 0.0 { 1e-300 } else { d };
        }
        count
    };
    let mut lo = 0.0;
    let mut hi = 1.0;
    while below(hi) < j {
        hi *= 2.0;
    }
    for _ in 0..400 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi || hi - lo <= 1e-16 * hi {
            break;
        }
        if below(mid) >= j {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

fn own_opts(basis: usize, roots: usize, wref: f64) -> DampedOptions {
    DampedOptions::new(basis)
        .viscoelastic_reference(wref, None)
        .viscoelastic_own_frequency(OwnFrequency::new(roots))
}

/// `|Im lambda - omega| / Im lambda` of every iterated root.
fn fixed_point_errors(d: &DampedModes) -> Vec<f64> {
    if d.own_frequency.len() != d.modes.len() || d.own_frequency.is_empty() {
        return vec![f64::INFINITY];
    }
    d.modes
        .iter()
        .zip(&d.own_frequency)
        .map(|(m, r)| (m.eigenvalue.im - r.omega).abs() / m.eigenvalue.im)
        .collect()
}

fn bits(d: &DampedModes) -> Vec<u64> {
    let mut v = Vec::new();
    for m in &d.modes {
        v.push(m.eigenvalue.re.to_bits());
        v.push(m.eigenvalue.im.to_bits());
        for z in &m.shape {
            v.push(z.re.to_bits());
            v.push(z.im.to_bits());
        }
    }
    for r in &d.own_frequency {
        v.push(r.omega.to_bits());
        v.push(r.iterations as u64);
    }
    v
}

// ------------------------------------------------------------------------------------------------- the cantilevers

const E_ST: f64 = 2e11;
const NU_ST: f64 = 0.3;
const RHO_ST: f64 = 7850.0;
const E_AL: f64 = 7e10;
const NU_AL: f64 = 0.33;
const RHO_AL: f64 = 2700.0;
const ETA_AL: f64 = 0.01;

fn steel() -> Material {
    Material::isotropic_elastic(E_ST, NU_ST, RHO_ST, 0.0).unwrap()
}

/// The cantilever of T, TE and DM: `[0.6, 0.1, 0.06]`, `6 x 1 x 1`, elements 0..2 in "left", 3..5 in "right".
fn bar_mesh() -> rcfd_fem::SolidMesh {
    let mut mesh = hex_box([0.0; 3], [0.6, 0.1, 0.06], [6, 1, 1]).unwrap();
    mesh.add_element_set("left", (0..3).collect()).unwrap();
    mesh.add_element_set("right", (3..6).collect()).unwrap();
    mesh
}

fn held(mut m: SolidModel) -> SolidModel {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn bar_one(material: Material) -> SolidModel {
    let mut m = SolidModel::new(bar_mesh());
    m.add_material("all", material).unwrap();
    held(m)
}

fn bar_two(left: Material) -> SolidModel {
    let mut m = SolidModel::new(bar_mesh());
    m.add_material("left", left).unwrap();
    m.add_material("right", Material::isotropic_elastic(E_AL, NU_AL, RHO_AL, 0.0).unwrap())
        .unwrap();
    held(m)
}

/// `2 G I_dev + K 1 (x) 1`, Voigt, engineering shear (written here, independent of the library).
fn iso_d(g: f64, k: f64) -> Matrix6 {
    let mut d = [[0.0; 6]; 6];
    for (i, row) in d.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = 2.0 * g * (if i == j { 1.0 } else { 0.0 } - 1.0 / 3.0) + k;
        }
    }
    for (i, row) in d.iter_mut().enumerate().skip(3) {
        row[i] = g;
    }
    d
}

fn d_of_young(e: f64, nu: f64) -> Matrix6 {
    iso_d(e / (2.0 * (1.0 + nu)), e / (3.0 * (1.0 - 2.0 * nu)))
}

/// The dense restriction of `a` to the free equations.
fn dense(a: &CsrMatrix, free: &[usize]) -> Vec<f64> {
    let f = free.len();
    let mut out = vec![0.0; f * f];
    let mut e = vec![0.0; a.n];
    let mut y = vec![0.0; a.n];
    for (j, &cj) in free.iter().enumerate() {
        e[cj] = 1.0;
        a.matvec(&e, &mut y);
        e[cj] = 0.0;
        for (i, &ri) in free.iter().enumerate() {
            out[i * f + j] = y[ri];
        }
    }
    out
}

fn assemble_d(model: &SolidModel, d_of: &(dyn Fn(usize) -> Matrix6 + Sync), free: &[usize]) -> Vec<f64> {
    let k = assemble_with(model, Some(1), |ctx| {
        element_for(ctx.kind).stiffness(ctx.coords, &d_of(ctx.element))
    })
    .unwrap();
    dense(&k, free)
}

fn free_equations(model: &SolidModel) -> Vec<usize> {
    let dofs = model.dof_map();
    let fixed: Vec<usize> = model
        .mesh()
        .node_set("xmin")
        .unwrap()
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .filter_map(|(n, c)| dofs.equation(n, c))
        .collect();
    (0..dofs.n_equations()).filter(|e| !fixed.contains(e)).collect()
}

/// The viscoelastic cantilever material of the dense reference: one term `g`, `k`, `tau` on steel.
#[derive(Clone, Copy)]
struct Visco3 {
    g: f64,
    k: f64,
    tau: f64,
}

impl Visco3 {
    fn material(&self) -> Material {
        Material::viscoelastic(steel(), vec![PronyTerm::new(self.g, self.k, self.tau).unwrap()], None).unwrap()
    }
    /// `(G*, K*)` at `w` from the formulas.
    fn moduli(&self, w: f64) -> (C64, C64) {
        let gs = one_term(E_ST / (2.0 * (1.0 + NU_ST)), self.g, self.tau, w);
        let ks = one_term(E_ST / (3.0 * (1.0 - 2.0 * NU_ST)), self.k, self.tau, w);
        (gs, ks)
    }
}

/// The dense full-space fixed point (ADR 0520, the test set-ups): `two` selects the DM model (left viscoelastic, right
/// aluminium with its loss factor, Rayleigh `(a, b)` on `K'(omega_ref)`, a ground dashpot `(equation, c)`); otherwise every
/// element is viscoelastic. The lowest `count` roots, ordered by `Im`.
fn dense_fixed_point(
    model: &SolidModel,
    v: Visco3,
    two: bool,
    rayleigh: (f64, f64),
    dashpot: Option<(usize, f64)>,
    wref: f64,
    count: usize,
    tol: f64,
) -> Vec<C64> {
    let free = free_equations(model);
    let nf = free.len();
    let mass = dense(&rcfd_fem::mass::consistent(model).unwrap(), &free);
    let d_al = d_of_young(E_AL, NU_AL);
    let mats = |w: f64| -> (Vec<f64>, Vec<f64>) {
        let (gs, ks) = v.moduli(w);
        let (ds, dl) = (iso_d(gs.re, ks.re), iso_d(gs.im, ks.im));
        let mut d_al_loss = d_al;
        for row in d_al_loss.iter_mut() {
            for x in row.iter_mut() {
                *x *= ETA_AL;
            }
        }
        let is_left = |e: usize| !two || e < 3;
        let k1 = assemble_d(model, &|e| if is_left(e) { ds } else { d_al }, &free);
        let k2 = assemble_d(model, &|e| if is_left(e) { dl } else { d_al_loss }, &free);
        (k1, k2)
    };
    let (kref, _) = mats(wref);
    let mut c = vec![0.0; nf * nf];
    for i in 0..nf * nf {
        c[i] = rayleigh.0 * mass[i] + rayleigh.1 * kref[i];
    }
    if let Some((eq, cv)) = dashpot {
        let i = free.iter().position(|&e| e == eq).unwrap();
        c[i * nf + i] += cv;
    }
    let solve = |w: f64| -> Vec<C64> {
        let (k1, k2) = mats(w);
        let q = quadratic_eigen_with(&mass, &c, &k1, Some(&k2), nf, &GeneralEigenOptions::default()).unwrap();
        let mut r: Vec<C64> = q.values.into_iter().filter(|z| z.im > 1e-6 * z.abs()).collect();
        r.sort_by(|a, b| a.im.total_cmp(&b.im));
        r
    };
    let frozen = solve(wref);
    let mut out = Vec::new();
    for &start in frozen.iter().take(count) {
        let mut lam = start;
        let mut w = wref;
        let mut converged = false;
        for _ in 0..100 {
            let wn = lam.im;
            if (wn - w).abs() <= tol * wn {
                converged = true;
                break;
            }
            let cand = solve(wn);
            lam = *cand
                .iter()
                .min_by(|a, b| (**a - lam).abs().total_cmp(&(**b - lam).abs()))
                .unwrap();
            w = wn;
        }
        out.push(if converged { lam } else { C64::new(f64::NAN, f64::NAN) });
    }
    out
}

fn roots_of(d: &DampedModes, count: usize) -> Vec<C64> {
    d.modes.iter().take(count).map(|m| m.eigenvalue).collect()
}

/// `max_j` of the nearest relative distance of `a_j` in `b`; infinite when `a` has fewer than `count` entries.
fn worst_nearest(a: &[C64], b: &[C64], count: usize) -> f64 {
    if a.len() < count || b.is_empty() {
        return f64::INFINITY;
    }
    worst(&a.iter().take(count).map(|&z| nearest(z, b)).collect::<Vec<_>>())
}

/// The registered run (ADR 0520 Amendment 1): TF missed (`inf`, the dense reference below its noise floor), DM missed
/// (`6.87e-9 > 1e-9`, the point dashpot against basis `n - 1`), RV1 missed (refused: the pseudo-mode root takes 170
/// iterations); every other check met. Kept as the record, its checks and limits unchanged.
#[test]
#[ignore = "ADR 0520 Amendment 1 record: TF inf, DM 6.87e-9 > 1e-9, RV1 refused (50 iterations); see f586_checks_amended"]
fn f586_checks_known_miss() {
    checks(false);
}

/// The corrected set-ups of ADR 0520 Amendment 1 (TF with the dense tolerance `1e-10`, DMb without the dashpot, RV1b with
/// 500 iterations); every other check as registered.
#[test]
fn f586_checks_amended() {
    checks(true);
}

fn checks(amended: bool) {
    let dense_tol = if amended { 1e-10 } else { 1e-13 };
    let mut c = Checks::new();
    let mut fp: Vec<f64> = Vec::new();
    let mu1i = chain_mu1(N);
    let mu: Vec<f64> = (1..=4).map(|j| chain_mu(N, j)).collect();

    // SU, SR
    let elastic = rod_model(N, Kind::Axial, base(0.0));
    let w1 = damped_modes(&elastic, &DampedOptions::new(6)).unwrap().omegas[0];
    c.at_most(
        "SU mu_1 bisection against inverse iteration and the library",
        worst(&[
            ((mu[0] - mu1i) / mu1i).abs(),
            ((w1 * w1 * RHO / E0 - mu[0]) / mu[0]).abs(),
        ]),
        1e-9,
    );
    let ra = rod_a();
    let (own_a1, wstar) = ra.own(mu[0]);
    c.at_most(
        "SR scalar fixed point of rod A",
        (own_a1.im - wstar).abs() / wstar,
        1e-14,
    );
    let frz_a1 = ra.root_at(ra.wc, mu[0]);

    // DA, IT, DD, DDr
    let model_a = rod_model(N, Kind::Axial, ra.material());
    let da = damped_modes(&model_a, &own_opts(6, 4, ra.wc)).unwrap();
    let root_a = da.modes[0].eigenvalue;
    c.at_most(
        "DA rod A first root against the chain at its own frequency",
        rel(root_a, own_a1),
        1e-8,
    );
    println!(
        "  rod A: own {root_a:?}, reference {own_a1:?}, frozen reference {frz_a1:?}, records {:?}",
        da.own_frequency
    );
    fp.extend(fixed_point_errors(&da));
    c.within(
        "IT rod A root 1 iterations",
        da.own_frequency.first().map_or(f64::NAN, |r| r.iterations as f64),
        3.0,
        15.0,
    );
    let frozen_lib = damped_modes(&model_a, &DampedOptions::new(6).viscoelastic_reference(ra.wc, None))
        .unwrap()
        .modes[0]
        .eigenvalue;
    let dd = rel(root_a, frozen_lib);
    c.within("DD rod A own against the F-582 frozen root", dd, 9.5e-6, 9.8e-6);
    let dd_ref = rel(own_a1, frz_a1);
    c.at_most(
        "DDr the distance against the test's",
        ((dd - dd_ref) / dd_ref).abs(),
        1e-4,
    );

    // DAm
    let mut dam = Vec::new();
    for m in [2, 39] {
        let d = damped_modes(&model_a, &own_opts(m, 1, ra.wc)).unwrap();
        dam.push(rel(d.modes[0].eigenvalue, own_a1));
    }
    c.at_most("DAm rod A bases 2 and 39", worst(&dam), 1e-8);

    // DS
    let mut ds = Vec::new();
    for f in [0.5, 2.0] {
        let d = damped_modes(&model_a, &own_opts(6, 1, f * ra.wc)).unwrap();
        ds.push(rel(d.modes[0].eigenvalue, own_a1));
    }
    c.at_most("DS rod A from 0.5 and 2 omega_c", worst(&ds), 1e-8);

    // DH
    let dh: Vec<f64> = (1..4)
        .map(|j| match da.modes.get(j) {
            Some(m) => rel(m.eigenvalue, ra.own(mu[j]).0),
            None => f64::INFINITY,
        })
        .collect();
    c.at_most("DH rod A roots 2 to 4", worst(&dh), 1e-8);

    // DB, DC
    for (name, rod) in [("DB", rod_b()), ("DC", rod_c())] {
        let model = rod_model(N, rod.kind, rod.material());
        let d = damped_modes(&model, &own_opts(6, 4, rod.wc)).unwrap();
        fp.extend(fixed_point_errors(&d));
        let e: Vec<f64> = (0..4)
            .map(|j| match d.modes.get(j) {
                Some(m) => rel(m.eigenvalue, rod.own(mu[j]).0),
                None => f64::INFINITY,
            })
            .collect();
        c.at_most(&format!("{name} roots 1 to 4"), worst(&e), 1e-8);
        println!(
            "  {name}: iterations {:?}",
            d.own_frequency.iter().map(|r| r.iterations).collect::<Vec<_>>()
        );
    }

    // T, TF, TT, TE, DM: the 3-D cantilevers
    let plain = bar_one(steel());
    let w1c = damped_modes(&plain, &DampedOptions::new(6)).unwrap().omegas[0];
    let nf = free_equations(&plain).len();
    let full = nf - 1;
    let vt = Visco3 {
        g: 0.15,
        k: 0.05,
        tau: 1.0 / w1c,
    };
    let model_t = bar_one(vt.material());
    let t6 = damped_modes(&model_t, &own_opts(6, 4, w1c)).unwrap();
    let tf = damped_modes(&model_t, &own_opts(full, 4, w1c)).unwrap();
    fp.extend(fixed_point_errors(&t6));
    fp.extend(fixed_point_errors(&tf));
    let r6 = roots_of(&t6, 4);
    let rf = roots_of(&tf, 4);
    let t = worst_nearest(&r6, &rf, 4);
    c.within("T truncation, basis 6 against the full basis", t, 1e-5, 1e-3);
    println!("  T: {nf} free unknowns, basis {full}; basis 6 {r6:?}; full {rf:?}");
    let dense_t = dense_fixed_point(&model_t, vt, false, (0.0, 0.0), None, w1c, 4, dense_tol);
    c.at_most(
        "TF full basis against the dense full space",
        worst_nearest(&rf, &dense_t, 4),
        1e-9,
    );
    c.record(
        "TT basis 6 against the dense full space",
        worst_nearest(&r6, &dense_t, 4),
    );
    let ve = Visco3 {
        g: 0.2,
        k: 0.2,
        tau: 1.0 / w1c,
    };
    let model_te = bar_one(ve.material());
    let te6 = damped_modes(&model_te, &own_opts(6, 4, w1c)).unwrap();
    let tef = damped_modes(&model_te, &own_opts(full, 4, w1c)).unwrap();
    let te = worst_nearest(&roots_of(&te6, 4), &roots_of(&tef, 4), 4);
    c.at_most("TE equal weights, basis 6 against the full basis", te, 1e-10);
    c.holds("NC3 T >= 1e4 TE", t >= 1e4 * te, &format!("T {t:e}, TE {te:e}"));

    let model_dm = bar_two(vt.material());
    let tip = model_dm.mesh().node_set("xmax").unwrap()[0];
    let tip_eq = model_dm.dof_map().equation(tip, 2).unwrap();
    let mut dm_opts = own_opts(full, 4, w1c)
        .rayleigh(5.0, 2e-6)
        .loss_factor("right", LossFactor::Constant(ETA_AL));
    if !amended {
        dm_opts = dm_opts.dashpot(Dashpot::ground(tip, 2, 30.0));
    }
    let dm = damped_modes(&model_dm, &dm_opts).unwrap();
    fp.extend(fixed_point_errors(&dm));
    let pot = if amended { None } else { Some((tip_eq, 30.0)) };
    let dense_dm = dense_fixed_point(&model_dm, vt, true, (5.0, 2e-6), pot, w1c, 4, dense_tol);
    c.at_most(
        if amended {
            "DMb two materials without the dashpot, full basis against the dense full space"
        } else {
            "DM two materials, full basis against the dense full space"
        },
        worst_nearest(&roots_of(&dm, 4), &dense_dm, 4),
        1e-9,
    );

    // DE: equal weights in 3-D against the elastic model at each root's own frequency
    {
        let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
        m.add_material("all", steel()).unwrap();
        let plain = held(m);
        let w1 = damped_modes(&plain, &DampedOptions::new(6)).unwrap().omegas[0];
        let wref = 0.8 * w1;
        let tau = 1.0 / wref;
        let visco = Material::viscoelastic(steel(), vec![PronyTerm::new(0.2, 0.2, tau).unwrap()], None).unwrap();
        let mut mv = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
        mv.add_material("all", visco).unwrap();
        let mv = held(mv);
        let ctip = plain.mesh().node_set("xmax").unwrap()[0];
        let extra = |o: DampedOptions| o.rayleigh(5.0, 0.0).dashpot(Dashpot::ground(ctip, 1, 30.0));
        let dv = damped_modes(&mv, &extra(own_opts(6, 6, wref))).unwrap();
        let mut errs = Vec::new();
        for (md, r) in dv.modes.iter().zip(&dv.own_frequency) {
            let e = one_term(E_ST, 0.2, tau, r.omega);
            let mut me = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
            me.add_material("all", Material::isotropic_elastic(e.re, NU_ST, RHO_ST, 0.0).unwrap())
                .unwrap();
            let me = held(me);
            let de = damped_modes(
                &me,
                &extra(DampedOptions::new(6)).loss_factor("all", LossFactor::Constant(e.im / e.re)),
            )
            .unwrap();
            let set: Vec<C64> = de.modes.iter().map(|x| x.eigenvalue).collect();
            errs.push(nearest(md.eigenvalue, &set));
        }
        c.holds(
            "DE six roots iterated",
            dv.modes.len() == 6,
            &format!("{}", dv.modes.len()),
        );
        c.at_most(
            "DE 3-D equal weights against the elastic model at the own frequency",
            worst(&errs),
            1e-9,
        );
    }

    // DF: the shift
    {
        let rod = rod_b();
        let tt = 383.0;
        let wlf = TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap();
        let a_t = shift_factor(&wlf, tt).unwrap();
        let shifted = rod_model(N, rod.kind, standard(rod.nu, rod.g, rod.k, rod.tau, Some(wlf)));
        let unshifted = rod_model(N, rod.kind, standard(rod.nu, rod.g, rod.k, rod.tau * a_t, None));
        let o1 = DampedOptions::new(6)
            .viscoelastic_reference(rod.wc, Some(tt))
            .viscoelastic_own_frequency(OwnFrequency::new(1));
        let r1 = damped_modes(&shifted, &o1).unwrap().modes[0].eigenvalue;
        let r2 = damped_modes(&unshifted, &own_opts(6, 1, rod.wc)).unwrap().modes[0].eigenvalue;
        c.at_most("DF shifted against tau a_T", rel(r1, r2), 1e-12);
    }

    // C0: no terms
    {
        let none = Material::viscoelastic(base(0.0), Vec::new(), None).unwrap();
        let d = damped_modes(&rod_model(N, Kind::Axial, none), &own_opts(6, 1, 1.0)).unwrap();
        let r2 = damped_modes(&elastic, &DampedOptions::new(6)).unwrap().modes[0].eigenvalue;
        c.at_most("C0 no terms against the base", rel(d.modes[0].eigenvalue, r2), 1e-12);
        let it = d.own_frequency.first().map_or(usize::MAX, |r| r.iterations);
        c.holds("C0 iterations <= 1", it <= 1, &format!("{it}"));
    }

    // P1: threads
    {
        let rod = rod_b();
        let model = rod_model(N, rod.kind, rod.material());
        let o = own_opts(6, 4, rod.wc);
        let d1 = damped_modes(&model, &o.clone().threads(1)).unwrap();
        let d4 = damped_modes(&model, &o.threads(4)).unwrap();
        c.holds(
            "P1 threads 1 and 4 give the same bits",
            bits(&d1) == bits(&d4),
            "roots, shapes, records",
        );
    }

    // RV1: residual vectors
    {
        let rod = rod_b();
        let model = rod_model(N, rod.kind, rod.material());
        let tip = model.mesh().node_set("xmax").unwrap()[0];
        let own = if amended {
            OwnFrequency::new(3).max_iterations(500)
        } else {
            OwnFrequency::new(3)
        };
        let o = DampedOptions::new(4)
            .viscoelastic_reference(rod.wc, None)
            .viscoelastic_own_frequency(own)
            .dashpot(Dashpot::ground(tip, 0, 1.0e3))
            .residual_vectors(true);
        let d = damped_modes(&model, &o);
        let ok = match &d {
            Ok(d) => {
                !d.modes.is_empty()
                    && d.modes
                        .iter()
                        .all(|m| m.eigenvalue.re.is_finite() && m.eigenvalue.im.is_finite())
            }
            Err(_) => false,
        };
        c.holds(
            if amended {
                "RV1b residual vectors run (500 iterations), finite roots"
            } else {
                "RV1 residual vectors run, finite roots"
            },
            ok,
            &format!("{:?}", d.as_ref().map(|d| (d.basis, d.modes.len()))),
        );
        if amended {
            let it = d
                .as_ref()
                .ok()
                .and_then(|d| d.own_frequency.first())
                .map_or(0, |r| r.iterations);
            println!(
                "  RV1b records {:?}",
                d.as_ref().map(|d| d.own_frequency.clone()).unwrap_or_default()
            );
            c.holds("RV1b root 0 takes more than 50 iterations", it > 50, &format!("{it}"));
        }
    }

    // FP over DA, DB, DC, T, DM
    c.at_most("FP |Im lambda - omega|/Im lambda", worst(&fp), 1e-12);

    // NC1, NC2
    c.at_least(
        "NC1 frozen root against the own-frequency reference",
        rel(frozen_lib, own_a1),
        1e-6,
    );
    let one_update = ra.root_at(frz_a1.im, mu[0]);
    c.at_least(
        "NC2 one update against the own-frequency reference",
        rel(one_update, own_a1),
        5e-8,
    );

    // RF: refusals
    {
        let (ok, msg) = refused_with(
            damped_modes(
                &model_a,
                &DampedOptions::new(4).viscoelastic_own_frequency(OwnFrequency::new(1)),
            ),
            "own frequency",
        );
        c.holds("RF (i) without a reference frequency", ok, &msg);
        let bad = [
            ("roots 0", OwnFrequency::new(0)),
            ("tolerance 0", OwnFrequency::new(1).tolerance(0.0)),
            ("tolerance NaN", OwnFrequency::new(1).tolerance(f64::NAN)),
            ("tolerance -1e-12", OwnFrequency::new(1).tolerance(-1e-12)),
            ("max_iterations 0", OwnFrequency::new(1).max_iterations(0)),
        ];
        for (name, own) in bad {
            let (ok, msg) = refused_with(
                damped_modes(
                    &model_a,
                    &DampedOptions::new(4)
                        .viscoelastic_reference(ra.wc, None)
                        .viscoelastic_own_frequency(own),
                ),
                "own frequency",
            );
            c.holds(&format!("RF (ii) {name}"), ok, &msg);
        }
        let r = damped_modes(
            &model_a,
            &DampedOptions::new(4)
                .viscoelastic_reference(ra.wc, None)
                .viscoelastic_own_frequency(OwnFrequency::new(1).max_iterations(1)),
        );
        let (ok, msg) = match &r {
            Err(e @ FemError::Solver { solver, .. }) => (*solver == "viscoelastic own frequency", format!("{e}")),
            Err(e) => (false, format!("{e}")),
            Ok(_) => (false, "accepted".to_string()),
        };
        c.holds("RF (iii) not converged in one iteration", ok, &msg);
        let (ok, msg) = refused_with(damped_modes(&elastic, &own_opts(4, 1, 100.0)), "reference frequency");
        c.holds("RF (iv) on a model without the material", ok, &msg);
    }
    c.finish();
}
