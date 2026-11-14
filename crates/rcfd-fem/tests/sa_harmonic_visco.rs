//! F-577 (ADR 0491): the viscoelastic material in the direct harmonic solve, by its complex shear and bulk moduli at each
//! frequency; the damped modes refuse it. Checks HA to HD (rods against the closed form and the discrete chain), E1 (the
//! table route), DM1 (damped modes), Z1 (zero frequency), T1 (shift), C0 (no terms), P1 (threads), NC1, NC2 (negative
//! controls) and RF (refusals) of the registration.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mat_visco::{self, modulus_table, shift_factor};
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

/// The smallest of the values; NaN-safe as `worst` (a non-finite value gives 0, a miss of an `at_least`).
fn least(values: &[f64]) -> f64 {
    let mut w = f64::INFINITY;
    for &v in values {
        if !v.is_finite() {
            return 0.0;
        }
        w = w.min(v);
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
const A: f64 = B * B;
const RHO: f64 = 1200.0;
const E0: f64 = 2.0e9;
/// The total tip force, N.
const F: f64 = 1.0;
/// The registered discretisation limit at 40 elements (ADR 0491 arithmetic 4).
const DISC: f64 = 4e-3;
/// The chain checks (arithmetic 6).
const CHAIN: f64 = 1e-9;
/// Rod A: equal weights.
const W_A: f64 = 2.0 / 21.0;
/// Rods B and C: unequal weights.
const G_B: f64 = 0.15;
const K_B: f64 = 0.05;
const NU_B: f64 = 0.3;

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

/// `G*(omega)`, `K*(omega)` of the standard solid, from the formulas (arithmetic 3).
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
        // the storage modulus at omega tau = 1 (arithmetic 3)
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
    /// The rod's modulus by the one-part route `E*/E_0` on the base modulus (the negative control NC1).
    fn wrong_modulus(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        let young = (gs * ks).scale(9.0) / (ks.scale(3.0) + gs);
        let m0 = match self.kind {
            Kind::Axial => k0(self.nu) + 4.0 / 3.0 * g0(self.nu),
            Kind::Shear => g0(self.nu),
        };
        young.scale(m0 / E0)
    }
    /// The ten frequencies of the registration.
    fn omegas(&self) -> Vec<f64> {
        (0..10).map(|k| self.wc * (0.9 + 0.2 * k as f64 / 9.0)).collect()
    }
    fn component(&self) -> usize {
        match self.kind {
            Kind::Axial => 0,
            Kind::Shear => 1,
        }
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

fn hold(m: &mut SolidModel, kind: Kind) {
    let (free, held) = match kind {
        Kind::Axial => (0, [1, 2]),
        Kind::Shear => (1, [0, 2]),
    };
    for c in held {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("xmin", free, 0.0)).unwrap();
}

fn rod_model(n: usize, kind: Kind, material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [L, B, B], [n, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    hold(&mut m, kind);
    m
}

/// The two-material rod of HD: elements `0 .. n/2` the material `left` (set `left`), the rest `right`.
fn two_rod(n: usize, kind: Kind, left: Material, right: Material) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [L, B, B], [n, 1, 1]).unwrap();
    mesh.add_element_set("left", (0..n / 2).collect()).unwrap();
    mesh.add_element_set("right", (n / 2..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("left", left).unwrap();
    m.add_material("right", right).unwrap();
    hold(&mut m, kind);
    m
}

fn tip_load(model: &SolidModel, component: usize) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, component).unwrap()] = F / 4.0;
    }
    HarmonicLoad::real(f).unwrap()
}

fn tip(model: &SolidModel, r: &HarmonicResponse, component: usize) -> C64 {
    let node = model.mesh().node_set("xmax").unwrap()[0];
    let j = model.dof_map().equation(node, component).unwrap();
    C64::new(r.re[j], r.im[j])
}

/// `max_j |x_j - y_j| / max_j |y_j|` over the complex response vectors.
fn vector_rel(x: &HarmonicResponse, y: &HarmonicResponse) -> f64 {
    let mut num: f64 = 0.0;
    let mut den: f64 = 0.0;
    for j in 0..y.re.len() {
        let d = C64::new(x.re[j] - y.re[j], x.im[j] - y.im[j]).abs();
        if !d.is_finite() {
            return f64::INFINITY;
        }
        num = num.max(d);
        den = den.max(C64::new(y.re[j], y.im[j]).abs());
    }
    num / den
}

fn rel(a: C64, b: C64) -> f64 {
    (a - b).abs() / b.abs()
}

// ------------------------------------------------------------------------------------------------- references

fn csin(z: C64) -> C64 {
    C64::new(z.re.sin() * z.im.cosh(), z.re.cos() * z.im.sinh())
}

fn ccos(z: C64) -> C64 {
    C64::new(z.re.cos() * z.im.cosh(), -z.re.sin() * z.im.sinh())
}

fn csqrt(z: C64) -> C64 {
    let r = z.abs();
    let re = ((r + z.re) / 2.0).sqrt();
    let im = ((r - z.re) / 2.0).sqrt().copysign(z.im);
    C64::new(re, im)
}

/// The closed form of the continuous rod (arithmetic 2): `u(L) = F tan(kL)/(A Mod* k)`, `k = w sqrt(rho/Mod*)`.
fn closed(modulus: C64, w: f64) -> C64 {
    let k = csqrt(C64::from_real(RHO) / modulus).scale(w);
    let kl = k.scale(L);
    C64::from_real(F) * csin(kl) / (ccos(kl) * modulus.scale(A) * k)
}

/// The tip of the 1-D chain of linear elements with consistent mass (arithmetic 2, 6), the modulus of each element given,
/// clamped at node 0, the force `F` at the last node; complex Thomas algorithm.
fn chain(moduli: &[C64], w: f64) -> C64 {
    let n = moduli.len();
    let h = L / n as f64;
    let mm = RHO * A * h / 6.0;
    // nodes 1..=n are the unknowns; element e joins nodes e and e + 1
    let mut diag = vec![C64::ZERO; n + 1];
    let mut off = vec![C64::ZERO; n + 1]; // off[i] couples nodes i and i + 1
    for (e, &m) in moduli.iter().enumerate() {
        let kk = m.scale(A / h);
        let d = kk - C64::from_real(w * w * 2.0 * mm);
        diag[e] += d;
        diag[e + 1] += d;
        off[e] = -kk - C64::from_real(w * w * mm);
    }
    let mut dp: Vec<C64> = diag[1..].to_vec();
    let mut rp = vec![C64::ZERO; n];
    rp[n - 1] = C64::from_real(F);
    for j in 1..n {
        let m = off[j] / dp[j - 1];
        dp[j] -= m * off[j];
        let prev = rp[j - 1];
        rp[j] -= m * prev;
    }
    let mut x = vec![C64::ZERO; n];
    x[n - 1] = rp[n - 1] / dp[n - 1];
    for j in (0..n - 1).rev() {
        x[j] = (rp[j] - off[j + 1] * x[j + 1]) / dp[j];
    }
    x[n - 1]
}

struct RodRun {
    closed: Vec<f64>,
    chain: Vec<f64>,
    wrong: Vec<f64>,
}

/// The viscoelastic route on rod `rod` at `n` elements and its ten frequencies: the errors against the closed form, the
/// chain and the chain with the wrong modulus.
fn run_rod(rod: &Rod, n: usize) -> RodRun {
    let model = rod_model(n, rod.kind, rod.material());
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let c = rod.component();
    let load = tip_load(&model, c);
    let mut out = RodRun {
        closed: Vec::new(),
        chain: Vec::new(),
        wrong: Vec::new(),
    };
    for w in rod.omegas() {
        let x = tip(&model, &solver.response(&load, w).unwrap(), c);
        let m = rod.modulus(w);
        out.closed.push(rel(x, closed(m, w)));
        out.chain.push(rel(x, chain(&vec![m; n], w)));
        out.wrong.push(rel(x, chain(&vec![rod.wrong_modulus(w); n], w)));
    }
    out
}

#[test]
fn ha_hb_hc_hd_nc1_rods_against_the_closed_form_and_the_chain() {
    let mut c = Checks::new();
    for (name, rod) in [("A", rod_a()), ("B", rod_b()), ("C", rod_c())] {
        println!(
            "rod {name}: omega_c = {:.10e} rad/s, tau = {:.6e} s, loss factor at omega_c {:.6}",
            rod.wc,
            rod.tau,
            rod.modulus(rod.wc).im / rod.modulus(rod.wc).re
        );
        let r40 = run_rod(&rod, 40);
        let r80 = run_rod(&rod, 80);
        println!(
            "  e_40 per frequency: {}",
            r40.closed
                .iter()
                .map(|e| format!("{e:.3e}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let (e40, e80) = (worst(&r40.closed), worst(&r80.closed));
        c.at_most(&format!("H{name} closed form at 40 elements, max of 10"), e40, DISC);
        println!("  e_80 = {e80:.6e}");
        c.at_least(
            &format!("H{name}o observed order log2(e_40/e_80)"),
            (e40 / e80).log2(),
            1.9,
        );
        c.at_most(
            &format!("H{name}d chain at 40 and 80 elements"),
            worst(&[worst(&r40.chain), worst(&r80.chain)]),
            CHAIN,
        );
        if name != "A" {
            c.at_least(
                &format!("NC1 rod {name}: one-part route, min of 10"),
                least(&r40.wrong),
                1e-2,
            );
        }
    }

    // HD: two materials, the left half the elastic base of B, the right half the material of B
    let rod = rod_b();
    let n = 40;
    let model = two_rod(n, Kind::Axial, base(NU_B), rod.material());
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let load = tip_load(&model, 0);
    let m0 = C64::from_real(k0(NU_B) + 4.0 / 3.0 * g0(NU_B));
    let mut e = Vec::new();
    for w in rod.omegas() {
        let x = tip(&model, &solver.response(&load, w).unwrap(), 0);
        let mut moduli = vec![m0; n / 2];
        moduli.extend(vec![rod.modulus(w); n - n / 2]);
        e.push(rel(x, chain(&moduli, w)));
    }
    c.at_most("HD two materials against the chain", worst(&e), CHAIN);
    c.finish();
}

#[test]
fn e1_equal_weights_against_the_table_route() {
    let mut c = Checks::new();
    let rod = rod_a();
    let material = rod.material();
    let omegas = rod.omegas();
    let freqs: Vec<f64> = omegas.iter().map(|w| w / (2.0 * PI)).collect();
    let table = modulus_table(&material, &freqs, None).unwrap();
    let visco = rod_model(40, Kind::Axial, material);
    let elastic = rod_model(40, Kind::Axial, base(0.0));
    let sv = DirectHarmonic::new(&visco, &DirectHarmonicOptions::new()).unwrap();
    let st = DirectHarmonic::new(
        &elastic,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(table)),
    )
    .unwrap();
    let load = tip_load(&visco, 0);
    let mut e = Vec::new();
    for &w in &omegas {
        let xv = sv.response(&load, w).unwrap();
        let xt = st.response(&load, w).unwrap();
        e.push(vector_rel(&xv, &xt));
    }
    println!(
        "  per frequency: {}",
        e.iter().map(|x| format!("{x:.3e}")).collect::<Vec<_>>().join(" ")
    );
    c.at_most("E1 viscoelastic route against the table route", worst(&e), 1e-12);
    c.finish();
}

const T_REF: f64 = 373.0;
const T_RUN: f64 = 383.0;

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

#[test]
fn z1_t1_c0_p1_zero_frequency_shift_no_terms_threads() {
    let mut c = Checks::new();
    let rod = rod_b();
    let n = 40;

    // Z1: omega = 0, the long-term modulus
    let model = rod_model(n, Kind::Axial, rod.material());
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let load = tip_load(&model, 0);
    let m_inf = k0(NU_B) * (1.0 - K_B) + 4.0 / 3.0 * g0(NU_B) * (1.0 - G_B);
    let u = F * L / (A * m_inf);
    let x = tip(&model, &solver.response(&load, 0.0).unwrap(), 0);
    c.at_most("Z1 zero frequency, F L/(A M_inf)", rel(x, C64::from_real(u)), 1e-11);

    // T1: the shifted material at T against the unshifted one with tau a_T
    let a_t = shift_factor(&wlf(), T_RUN).unwrap();
    println!("  a_T = {a_t:.6e}");
    let shifted = rod_model(n, Kind::Axial, standard(NU_B, G_B, K_B, rod.tau, Some(wlf())));
    let scaled = rod_model(n, Kind::Axial, standard(NU_B, G_B, K_B, rod.tau * a_t, None));
    let ss = DirectHarmonic::new(&shifted, &DirectHarmonicOptions::new().temperature(T_RUN)).unwrap();
    let su = DirectHarmonic::new(&scaled, &DirectHarmonicOptions::new()).unwrap();
    let mut e = Vec::new();
    for w in rod.omegas() {
        e.push(vector_rel(
            &ss.response(&load, w).unwrap(),
            &su.response(&load, w).unwrap(),
        ));
    }
    c.at_most("T1 shifted against tau a_T", worst(&e), 1e-12);

    // C0: no terms against the base, away from resonance, no damping
    let none = rod_model(n, Kind::Axial, Material::viscoelastic(base(0.0), vec![], None).unwrap());
    let plain = rod_model(n, Kind::Axial, base(0.0));
    let sn = DirectHarmonic::new(&none, &DirectHarmonicOptions::new()).unwrap();
    let sp = DirectHarmonic::new(&plain, &DirectHarmonicOptions::new()).unwrap();
    let we = PI / 2.0 * (E0 / RHO).sqrt();
    let mut e = Vec::new();
    for w in [0.5 * we, 1.5 * we] {
        e.push(vector_rel(
            &sn.response(&load, w).unwrap(),
            &sp.response(&load, w).unwrap(),
        ));
    }
    c.at_most("C0 no terms against the base", worst(&e), 1e-12);

    // P1: threads 1 and 4
    let s1 = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().threads(1)).unwrap();
    let s4 = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().threads(4)).unwrap();
    let (r1, r4) = (s1.response(&load, rod.wc).unwrap(), s4.response(&load, rod.wc).unwrap());
    let same = r1.re.iter().zip(&r4.re).all(|(a, b)| a.to_bits() == b.to_bits())
        && r1.im.iter().zip(&r4.im).all(|(a, b)| a.to_bits() == b.to_bits());
    c.holds("P1 threads 1 and 4", same, "bits equal");
    c.finish();
}

#[test]
fn dm1_nc2_rf_refusals() {
    let mut c = Checks::new();
    let rod = rod_a();

    // DM1: the damped modes refuse the material
    let model = rod_model(40, Kind::Axial, rod.material());
    let (ok, msg) = not_yet(
        damped_modes(&model, &DampedOptions::new(4)),
        "Viscoelastic",
        "element set 'all'",
        mat_visco::FOLLOW_UP,
    );
    c.holds("DM1 damped modes refuse the material", ok, &msg);

    // NC2: the table of unequal weights
    let (ok, msg) = refused_with(modulus_table(&rod_b().material(), &[1.0, 2.0], None), "terms");
    c.holds("NC2 table of unequal weights refused", ok, &msg);

    // RF (i): a loss factor on a viscoelastic set
    let (ok, msg) = refused_with(
        DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
        ),
        "loss factor",
    );
    c.holds("RF (i) loss factor on a viscoelastic set", ok, &msg);

    // RF (ii), (iii): the shifted material without a temperature, with a NaN one
    let shifted = rod_model(40, Kind::Axial, standard(NU_B, G_B, K_B, 1e-3, Some(wlf())));
    let (ok, msg) = refused_with(
        DirectHarmonic::new(&shifted, &DirectHarmonicOptions::new()),
        "temperature",
    );
    c.holds("RF (ii) shifted material without a temperature", ok, &msg);
    let (ok, msg) = refused_with(
        DirectHarmonic::new(&shifted, &DirectHarmonicOptions::new().temperature(f64::NAN)),
        "temperature",
    );
    c.holds("RF (iii) NaN temperature", ok, &msg);

    // RF (iv): an orthotropic base
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
        DirectHarmonic::new(&rod_model(4, Kind::Axial, vo), &DirectHarmonicOptions::new()),
        "Viscoelastic",
        "an orthotropic base",
        mat_visco::FOLLOW_UP,
    );
    c.holds("RF (iv) orthotropic base", ok, &msg);

    // RF (v): a viscoelastic and a J2 material
    let j2 = Material::j2(base(0.3), 2.0e7, 2.0e8, 1.0e8).unwrap();
    let mixed = two_rod(4, Kind::Axial, j2, rod.material());
    let (ok, msg) = not_yet(
        DirectHarmonic::new(&mixed, &DirectHarmonicOptions::new()),
        "J2",
        "element set 'left'",
        rcfd_fem::mat_j2::FOLLOW_UP,
    );
    c.holds("RF (v) J2 with a viscoelastic material", ok, &msg);

    // RF (vi): the harmonic stress
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let r = solver.response(&tip_load(&model, 0), rod.wc).unwrap();
    let (ok, msg) = not_yet(
        solver.stress(&model, &r),
        "Viscoelastic",
        "the harmonic stress",
        mat_visco::FOLLOW_UP,
    );
    c.holds("RF (vi) harmonic stress", ok, &msg);
    c.finish();
}
