//! F-582 (ADR 0495): the viscoelastic material in the damped modes at a reference frequency given in the options. Checks SU
//! (set-up), DA to DC (first root against the discrete chain with the complex modulus at the reference frequency), LA to LCx
//! (loss factor), DE (3-D against the constant loss factor route), DF (shift), C0 (no terms), P1 (threads), RV1 (residual
//! vectors), NC1 to NC3 (negative controls) and RF (refusals) of the registration. The rod set-up of ADR 0491 is copied from
//! `sa_harmonic_visco.rs` (that file is edited by another item and is not touched).

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::mat_visco::{self, complex_moduli, shift_factor};
use rcfd_fem::material::{MaterialAxes, PronyTerm, TimeTemperatureShift};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
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
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
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

fn not_yet<T>(r: rcfd_fem::Result<T>, variant: &str, location: &str, package: &str) -> (bool, String) {
    match &r {
        Err(
            e @ FemError::NotYetSupported {
                item,
                variant: v,
                location: l,
                package: p,
            },
        ) => (
            *item == "material" && *v == variant && l.contains(location) && *p == package,
            format!("{e}"),
        ),
        Err(e) => (false, format!("{e}")),
        Ok(_) => (false, "accepted".to_string()),
    }
}

// ------------------------------------------------------------------------------------------------- the rods

const L: f64 = 1.0;
const B: f64 = 0.05;
const RHO: f64 = 1200.0;
const E0: f64 = 2.0e9;
const N: usize = 40;
/// Rod A: equal weights.
const W_A: f64 = 2.0 / 21.0;
/// Rods B and C: unequal weights.
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

/// The standard solid on the base `nu`: one term with the weights `g`, `k` and the time `tau`.
fn standard(nu: f64, g: f64, k: f64, tau: f64, shift: Option<TimeTemperatureShift>) -> Material {
    Material::viscoelastic(base(nu), vec![PronyTerm::new(g, k, tau).unwrap()], shift).unwrap()
}

/// The branch factor `i x/(1 + i x)`, `x = omega tau`.
fn branch(w: f64, tau: f64) -> C64 {
    let ix = C64::new(0.0, w * tau);
    ix / (C64::ONE + ix)
}

/// `G*(omega)`, `K*(omega)` of the standard solid, from the formulas (ADR 0491 arithmetic 3).
fn star(nu: f64, g: f64, k: f64, tau: f64, w: f64) -> (C64, C64) {
    let r = branch(w, tau);
    let gs = (C64::from_real(1.0 - g) + r.scale(g)).scale(g0(nu));
    let ks = (C64::from_real(1.0 - k) + r.scale(k)).scale(k0(nu));
    (gs, ks)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Axial, the lateral motion held: modulus `K* + 4 G*/3`.
    Axial,
    /// Shear, `u_x = u_z = 0` held, the force along `y`: modulus `G*`.
    Shear,
}

/// A rod case of the registration.
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
        // the storage modulus at omega tau = 1 (ADR 0491 arithmetic 3)
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
    /// The rod's modulus at `w`.
    fn modulus(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        match self.kind {
            Kind::Axial => ks + gs.scale(4.0 / 3.0),
            Kind::Shear => gs,
        }
    }
    /// The rod's modulus by the one-part route `E*/E_0` on the base modulus (the negative control NC3).
    fn wrong_modulus(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        let young = (gs * ks).scale(9.0) / (ks.scale(3.0) + gs);
        let m0 = match self.kind {
            Kind::Axial => k0(self.nu) + 4.0 / 3.0 * g0(self.nu),
            Kind::Shear => g0(self.nu),
        };
        young.scale(m0 / E0)
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

fn csqrt(z: C64) -> C64 {
    let r = z.abs();
    let re = ((r + z.re) / 2.0).sqrt();
    let im = ((r - z.re) / 2.0).sqrt().copysign(z.im);
    C64::new(re, im)
}

/// The smallest eigenvalue `mu_1` of the clamped-free chain of `n` linear elements with consistent mass, per unit modulus
/// and unit density (`K x = mu M x`, `K = (1/h) tridiag(-1, 2, -1)` with the last diagonal 1, `M = (h/6) tridiag(1, 4, 1)`
/// with the last diagonal 2; the area cancels): inverse iteration with the tridiagonal solve (ADR 0495 arithmetic 3).
fn chain_mu1(n: usize) -> f64 {
    let h = L / n as f64;
    let kd: Vec<f64> = (0..n).map(|i| if i + 1 == n { 1.0 / h } else { 2.0 / h }).collect();
    let ko = -1.0 / h;
    let md: Vec<f64> = (0..n)
        .map(|i| if i + 1 == n { 2.0 * h / 6.0 } else { 4.0 * h / 6.0 })
        .collect();
    let mo = h / 6.0;
    let matvec_m = |x: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let mut s = md[i] * x[i];
                if i > 0 {
                    s += mo * x[i - 1];
                }
                if i + 1 < n {
                    s += mo * x[i + 1];
                }
                s
            })
            .collect()
    };
    let matvec_k = |x: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let mut s = kd[i] * x[i];
                if i > 0 {
                    s += ko * x[i - 1];
                }
                if i + 1 < n {
                    s += ko * x[i + 1];
                }
                s
            })
            .collect()
    };
    let solve_k = |b: &[f64]| -> Vec<f64> {
        // Thomas algorithm
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
        let y = solve_k(&matvec_m(&x));
        let nrm = y.iter().map(|v| v * v).sum::<f64>().sqrt();
        x = y.iter().map(|v| v / nrm).collect();
    }
    let kx = matvec_k(&x);
    let mx = matvec_m(&x);
    let num: f64 = x.iter().zip(&kx).map(|(a, b)| a * b).sum();
    let den: f64 = x.iter().zip(&mx).map(|(a, b)| a * b).sum();
    num / den
}

/// `lambda_ref = i sqrt(Mod* mu_1/rho)` (ADR 0495 (a3)).
fn lambda_ref(modulus: C64, mu1: f64) -> C64 {
    let w = csqrt(modulus.scale(mu1 / RHO));
    C64::new(-w.im, w.re)
}

fn rel(a: C64, b: C64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The loss factor `2 |Re lambda|/Im lambda`.
fn loss_factor_of(l: C64) -> f64 {
    2.0 * l.re.abs() / l.im
}

fn first_root(model: &SolidModel, options: &DampedOptions) -> C64 {
    damped_modes(model, options).unwrap().modes[0].eigenvalue
}

fn bits(d: &DampedModes) -> Vec<u64> {
    let mut v = Vec::new();
    for z in &d.roots {
        v.push(z.re.to_bits());
        v.push(z.im.to_bits());
    }
    for m in &d.modes {
        for z in &m.shape {
            v.push(z.re.to_bits());
            v.push(z.im.to_bits());
        }
    }
    v
}

fn cantilever(material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
    m.add_material("all", material).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

#[test]
fn f582_checks() {
    let mut c = Checks::new();
    let mu1 = chain_mu1(N);

    // SU: the test's chain eigenvalue against the library's undamped first eigenvalue of the elastic rod
    let elastic = rod_model(N, Kind::Axial, base(0.0));
    let d = damped_modes(&elastic, &DampedOptions::new(6)).unwrap();
    let w1 = d.omegas[0];
    c.at_most(
        "SU mu_1 against the library",
        ((w1 * w1 * RHO / E0 - mu1) / mu1).abs(),
        1e-9,
    );

    // DA, DAm, LA, LAx and the same for rods B and C
    for (name, rod) in [("A", rod_a()), ("B", rod_b()), ("C", rod_c())] {
        let model = rod_model(N, rod.kind, rod.material());
        let modulus = rod.modulus(rod.wc);
        let lref = lambda_ref(modulus, mu1);
        let eta = modulus.im / modulus.re;
        let sizes: &[usize] = if name == "A" { &[6, 2, 39] } else { &[6] };
        for &m in sizes {
            let o = DampedOptions::new(m).viscoelastic_reference(rod.wc, None);
            let root = first_root(&model, &o);
            let tag = if m == 6 { "D".to_string() } else { format!("Dm{m}_") };
            c.at_most(
                &format!("{tag}{name} first root against the chain (basis {m})"),
                rel(root, lref),
                1e-8,
            );
            if m == 6 {
                let lf = loss_factor_of(root);
                c.at_most(
                    &format!("L{name} loss factor against E''/E'"),
                    ((lf - eta) / eta).abs(),
                    1e-2,
                );
                let exact = 2.0 * ((eta.atan()) / 2.0).tan();
                c.at_most(
                    &format!("L{name}x loss factor against 2 tan(atan(eta)/2)"),
                    ((lf - exact) / exact).abs(),
                    1e-8,
                );
                println!("  rod {name}: eta {eta:.6e}, loss factor {lf:.6e}, root {root:?}");
            }
        }
    }

    // DE: the 3-D cantilever against the elastic model with the constant loss factor
    {
        let e_base = 2e11;
        let base3 = Material::isotropic_elastic(e_base, 0.3, 7850.0, 0.0).unwrap();
        let plain = cantilever(base3.clone());
        let w_first = damped_modes(&plain, &DampedOptions::new(6)).unwrap().omegas[0];
        let wref = 0.8 * w_first;
        let visco = Material::viscoelastic(base3, vec![PronyTerm::new(0.2, 0.2, 1.0 / wref).unwrap()], None).unwrap();
        let cm = complex_moduli(&visco, wref, None).unwrap();
        let eta = cm.young.im / cm.young.re;
        let elastic = cantilever(Material::isotropic_elastic(cm.young.re, 0.3, 7850.0, 0.0).unwrap());
        let tip = plain.mesh().node_set("xmax").unwrap().to_vec();
        let opts = |o: DampedOptions| o.rayleigh(5.0, 2e-6).dashpot(Dashpot::ground(tip[0], 1, 30.0));
        let dv = damped_modes(
            &cantilever(visco),
            &opts(DampedOptions::new(6)).viscoelastic_reference(wref, None),
        )
        .unwrap();
        let de = damped_modes(
            &elastic,
            &opts(DampedOptions::new(6)).loss_factor("all", LossFactor::Constant(eta)),
        )
        .unwrap();
        let k = dv.modes.len().min(de.modes.len()).min(6);
        c.holds(
            "DE six roots exist",
            k == 6,
            &format!("{} and {} reported", dv.modes.len(), de.modes.len()),
        );
        let errs: Vec<f64> = (0..k)
            .map(|i| rel(dv.modes[i].eigenvalue, de.modes[i].eigenvalue))
            .collect();
        c.at_most("DE 3-D against the constant loss factor", worst(&errs), 1e-9);
    }

    // DF: the shift
    {
        let rod = rod_b();
        let t = 383.0;
        let wlf = TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap();
        let a_t = shift_factor(&wlf, t).unwrap();
        let shifted = rod_model(N, rod.kind, standard(rod.nu, rod.g, rod.k, rod.tau, Some(wlf)));
        let unshifted = rod_model(N, rod.kind, standard(rod.nu, rod.g, rod.k, rod.tau * a_t, None));
        let r1 = first_root(&shifted, &DampedOptions::new(6).viscoelastic_reference(rod.wc, Some(t)));
        let r2 = first_root(&unshifted, &DampedOptions::new(6).viscoelastic_reference(rod.wc, None));
        c.at_most("DF shifted against tau a_T", rel(r1, r2), 1e-12);
    }

    // C0: a material without terms
    {
        let none = Material::viscoelastic(base(0.0), Vec::new(), None).unwrap();
        let r1 = first_root(
            &rod_model(N, Kind::Axial, none),
            &DampedOptions::new(6).viscoelastic_reference(1.0, None),
        );
        let r2 = first_root(&elastic, &DampedOptions::new(6));
        c.at_most("C0 no terms against the base", rel(r1, r2), 1e-12);
    }

    // P1: threads
    {
        let rod = rod_b();
        let model = rod_model(N, rod.kind, rod.material());
        let o = DampedOptions::new(6).viscoelastic_reference(rod.wc, None);
        let d1 = damped_modes(&model, &o.clone().threads(1)).unwrap();
        let d4 = damped_modes(&model, &o.threads(4)).unwrap();
        c.holds(
            "P1 threads 1 and 4 give the same bits",
            bits(&d1) == bits(&d4),
            "roots and shapes",
        );
    }

    // RV1: residual vectors
    {
        let rod = rod_b();
        let model = rod_model(N, rod.kind, rod.material());
        let tip = model.mesh().node_set("xmax").unwrap()[0];
        let o = DampedOptions::new(4)
            .viscoelastic_reference(rod.wc, None)
            .dashpot(Dashpot::ground(tip, 0, 1.0e3))
            .residual_vectors(true);
        let d = damped_modes(&model, &o);
        let ok = match &d {
            Ok(d) => !d.modes.is_empty() && d.roots.iter().all(|z| z.re.is_finite() && z.im.is_finite()),
            Err(_) => false,
        };
        c.holds(
            "RV1 residual vectors run, finite roots",
            ok,
            &format!("{:?}", d.as_ref().map(|d| d.basis)),
        );
    }

    // NC1: a wrong reference frequency
    {
        let rod = rod_a();
        let model = rod_model(N, rod.kind, rod.material());
        let lref = lambda_ref(rod.modulus(rod.wc), mu1);
        let r = first_root(
            &model,
            &DampedOptions::new(6).viscoelastic_reference(0.5 * rod.wc, None),
        );
        c.at_least("NC1 reference at 0.5 omega_c", rel(r, lref), 5e-3);
    }

    // NC2: the elastic rod without loss
    {
        let rod = rod_a();
        let modulus = rod.modulus(rod.wc);
        let eta = modulus.im / modulus.re;
        let e = rod_model(
            N,
            rod.kind,
            Material::isotropic_elastic(modulus.re, 0.0, RHO, 0.0).unwrap(),
        );
        let r = first_root(&e, &DampedOptions::new(6));
        c.at_least(
            "NC2 loss factor of the lossless rod",
            ((loss_factor_of(r) - eta) / eta).abs(),
            0.5,
        );
    }

    // NC3: the wrong split
    {
        let rod = rod_b();
        let right = lambda_ref(rod.modulus(rod.wc), mu1);
        let wrong = lambda_ref(rod.wrong_modulus(rod.wc), mu1);
        c.at_least("NC3 one-part modulus on rod B", rel(wrong, right), 5e-3);
    }

    // RF: refusals
    {
        let rod = rod_b();
        let model = rod_model(N, rod.kind, rod.material());
        let opt = DampedOptions::new(4);
        let (ok, msg) = not_yet(
            damped_modes(&model, &opt),
            "Viscoelastic",
            "material 0",
            mat_visco::FOLLOW_UP,
        );
        c.holds("RF (i) without the option", ok, &msg);
        let (ok, msg) = refused_with(
            damped_modes(&elastic, &DampedOptions::new(4).viscoelastic_reference(100.0, None)),
            "reference frequency",
        );
        c.holds("RF (ii) option on a model without the material", ok, &msg);
        for w in [-1.0, f64::NAN, f64::INFINITY] {
            let (ok, msg) = refused_with(
                damped_modes(&model, &DampedOptions::new(4).viscoelastic_reference(w, None)),
                "reference frequency",
            );
            c.holds(&format!("RF (iii) omega {w}"), ok, &msg);
        }
        let wlf = TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap();
        let shifted = rod_model(N, rod.kind, standard(rod.nu, rod.g, rod.k, rod.tau, Some(wlf)));
        let (ok, msg) = refused_with(
            damped_modes(&shifted, &DampedOptions::new(4).viscoelastic_reference(rod.wc, None)),
            "temperature",
        );
        c.holds("RF (iv) shifted without a temperature", ok, &msg);
        let (ok, msg) = refused_with(
            damped_modes(
                &model,
                &DampedOptions::new(4)
                    .viscoelastic_reference(rod.wc, None)
                    .loss_factor("all", LossFactor::Constant(0.02)),
            ),
            "loss factor",
        );
        c.holds("RF (v) loss factor on the viscoelastic set", ok, &msg);
        let ortho = Material::orthotropic(
            [1.4e11, 1.0e10, 1.0e10],
            [0.3, 0.3, 0.45],
            [5.0e9, 5.0e9, 3.5e9],
            1600.0,
            [0.0; 3],
            MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
        )
        .unwrap();
        let vo = Material::viscoelastic(ortho, vec![PronyTerm::new(0.2, 0.2, 1.0).unwrap()], None).unwrap();
        let (ok, msg) = not_yet(
            damped_modes(
                &rod_model(4, Kind::Axial, vo),
                &DampedOptions::new(2).viscoelastic_reference(10.0, None),
            ),
            "Viscoelastic",
            "an orthotropic base",
            mat_visco::FOLLOW_UP,
        );
        c.holds("RF (vi) orthotropic base", ok, &msg);
        let j2 = Material::j2(base(0.3), 2.0e7, 2.0e8, 1.0e8).unwrap();
        let mut mesh = hex_box([0.0; 3], [L, B, B], [4, 1, 1]).unwrap();
        mesh.add_element_set("left", (0..2).collect()).unwrap();
        mesh.add_element_set("right", (2..4).collect()).unwrap();
        let mut mixed = SolidModel::new(mesh);
        mixed.add_material("left", j2).unwrap();
        mixed.add_material("right", rod.material()).unwrap();
        mixed.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        mixed.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
        mixed.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
        let (ok, msg) = not_yet(
            damped_modes(&mixed, &DampedOptions::new(2).viscoelastic_reference(rod.wc, None)),
            "J2",
            "element set 'left'",
            rcfd_fem::mat_j2::FOLLOW_UP,
        );
        c.holds("RF (vii) J2 with a viscoelastic material", ok, &msg);
    }
    c.finish();
}
