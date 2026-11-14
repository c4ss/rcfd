//! Shared set-ups of the viscoelastic harmonic tests (ADR 0494 D2): the rods A, B and C of ADR 0491, their models, the
//! closed forms and the discrete chain with every nodal value, and the list of checks. Factored from `sa_harmonic_visco.rs`
//! (which keeps its own copies).
#![allow(dead_code)]

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::material::{PronyTerm, TimeTemperatureShift};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::complex::C64;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed. A
/// value that is not finite is a miss.
pub struct Checks(pub Vec<String>);

impl Checks {
    pub fn new() -> Checks {
        Checks(Vec::new())
    }
    pub fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    pub fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    pub fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    pub fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; infinite if any is not finite (a NaN must not hide behind `f64::max`).
pub fn worst(values: &[f64]) -> f64 {
    let mut w: f64 = 0.0;
    for &v in values {
        if !v.is_finite() {
            return f64::INFINITY;
        }
        w = w.max(v);
    }
    w
}

/// The smallest of the values; a non-finite value gives 0 (a miss of an `at_least`).
pub fn least(values: &[f64]) -> f64 {
    let mut w = f64::INFINITY;
    for &v in values {
        if !v.is_finite() {
            return 0.0;
        }
        w = w.min(v);
    }
    w
}

pub const L: f64 = 1.0;
pub const B: f64 = 0.05;
pub const A: f64 = B * B;
pub const RHO: f64 = 1200.0;
pub const E0: f64 = 2.0e9;
/// The total tip force, N.
pub const F: f64 = 1.0;
/// Rod A: equal weights.
pub const W_A: f64 = 2.0 / 21.0;
/// Rods B and C: unequal weights.
pub const G_B: f64 = 0.15;
pub const K_B: f64 = 0.05;
pub const NU_B: f64 = 0.3;
pub const T_REF: f64 = 373.0;
pub const T_RUN: f64 = 383.0;

pub fn g0(nu: f64) -> f64 {
    E0 / (2.0 * (1.0 + nu))
}

pub fn k0(nu: f64) -> f64 {
    E0 / (3.0 * (1.0 - 2.0 * nu))
}

pub fn base(nu: f64) -> Material {
    Material::isotropic_elastic(E0, nu, RHO, 0.0).unwrap()
}

/// The standard solid on the base `nu`: one term with the weights `g`, `k` and the time `tau`.
pub fn standard(nu: f64, g: f64, k: f64, tau: f64, shift: Option<TimeTemperatureShift>) -> Material {
    Material::viscoelastic(base(nu), vec![PronyTerm::new(g, k, tau).unwrap()], shift).unwrap()
}

pub fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

/// The branch factor `i x/(1 + i x)`, `x = omega tau`.
pub fn branch(w: f64, tau: f64) -> C64 {
    let ix = C64::new(0.0, w * tau);
    ix / (C64::ONE + ix)
}

/// `G*(omega)`, `K*(omega)` of the standard solid.
pub fn star(nu: f64, g: f64, k: f64, tau: f64, w: f64) -> (C64, C64) {
    let r = branch(w, tau);
    let gs = (C64::from_real(1.0 - g) + r.scale(g)).scale(g0(nu));
    let ks = (C64::from_real(1.0 - k) + r.scale(k)).scale(k0(nu));
    (gs, ks)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Axial, the lateral motion held: modulus `K* + 4 G*/3`.
    Axial,
    /// Shear, `u_x = u_z = 0` held, the force along `y`: modulus `G*`.
    Shear,
}

/// A rod case of ADR 0491.
#[derive(Clone, Copy)]
pub struct Rod {
    pub kind: Kind,
    pub nu: f64,
    pub g: f64,
    pub k: f64,
    pub tau: f64,
    pub wc: f64,
}

impl Rod {
    pub fn new(kind: Kind, nu: f64, g: f64, k: f64) -> Rod {
        // the storage modulus at omega tau = 1
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
    pub fn material(&self) -> Material {
        standard(self.nu, self.g, self.k, self.tau, None)
    }
    /// The rod modulus at `w`.
    pub fn modulus(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        match self.kind {
            Kind::Axial => ks + gs.scale(4.0 / 3.0),
            Kind::Shear => gs,
        }
    }
    /// The lateral modulus `K* - 2 G*/3` (the stress `sigma_yy = sigma_zz` per axial strain).
    pub fn lateral(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        ks - gs.scale(2.0 / 3.0)
    }
    /// The rod modulus by the one-part route `E*/E_0` on the base modulus.
    pub fn wrong_modulus(&self, w: f64) -> C64 {
        let (gs, ks) = star(self.nu, self.g, self.k, self.tau, w);
        let young = (gs * ks).scale(9.0) / (ks.scale(3.0) + gs);
        young.scale(self.base_modulus() / E0)
    }
    /// The elastic base modulus of the rod state.
    pub fn base_modulus(&self) -> f64 {
        match self.kind {
            Kind::Axial => k0(self.nu) + 4.0 / 3.0 * g0(self.nu),
            Kind::Shear => g0(self.nu),
        }
    }
    /// The ten frequencies of ADR 0491.
    pub fn omegas(&self) -> Vec<f64> {
        (0..10).map(|k| self.wc * (0.9 + 0.2 * k as f64 / 9.0)).collect()
    }
    pub fn component(&self) -> usize {
        match self.kind {
            Kind::Axial => 0,
            Kind::Shear => 1,
        }
    }
}

pub fn rod_a() -> Rod {
    Rod::new(Kind::Axial, 0.0, W_A, W_A)
}

pub fn rod_b() -> Rod {
    Rod::new(Kind::Axial, NU_B, G_B, K_B)
}

pub fn rod_c() -> Rod {
    Rod::new(Kind::Shear, NU_B, G_B, K_B)
}

pub fn hold(m: &mut SolidModel, kind: Kind) {
    let (free, held) = match kind {
        Kind::Axial => (0, [1, 2]),
        Kind::Shear => (1, [0, 2]),
    };
    for c in held {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("xmin", free, 0.0)).unwrap();
}

pub fn rod_model(n: usize, kind: Kind, material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [L, B, B], [n, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    hold(&mut m, kind);
    m
}

/// The two-material rod: elements `0 .. n/2` the material `left` (set `left`), the rest `right`.
pub fn two_rod(n: usize, kind: Kind, left: Material, right: Material) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [L, B, B], [n, 1, 1]).unwrap();
    mesh.add_element_set("left", (0..n / 2).collect()).unwrap();
    mesh.add_element_set("right", (n / 2..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("left", left).unwrap();
    m.add_material("right", right).unwrap();
    hold(&mut m, kind);
    m
}

pub fn tip_load(model: &SolidModel, component: usize) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, component).unwrap()] = F / 4.0;
    }
    HarmonicLoad::real(f).unwrap()
}

/// The nodal displacement along the rod (component `comp`) at the nodes of the line `y = z = 0`, by position.
pub fn nodal(model: &SolidModel, r: &HarmonicResponse, comp: usize, n: usize) -> Vec<C64> {
    let h = L / n as f64;
    let mut u = vec![C64::ZERO; n + 1];
    let mut seen = vec![false; n + 1];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        if p[1] == 0.0 && p[2] == 0.0 {
            let j = (p[0] / h).round() as usize;
            let eq = model.dof_map().equation(i, comp).unwrap();
            u[j] = C64::new(r.re[eq], r.im[eq]);
            seen[j] = true;
        }
    }
    assert!(seen.iter().all(|&s| s), "every node of the line found");
    u
}

/// The nodal displacements of the 1-D chain of linear elements with consistent mass, the modulus of each element given,
/// clamped at node 0, the force `F` at the last node; complex Thomas algorithm (as ADR 0491, with every node returned).
pub fn chain_nodes(moduli: &[C64], w: f64) -> Vec<C64> {
    let n = moduli.len();
    let h = L / n as f64;
    let mm = RHO * A * h / 6.0;
    let mut diag = vec![C64::ZERO; n + 1];
    let mut off = vec![C64::ZERO; n + 1];
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
    let mut x = vec![C64::ZERO; n + 1];
    x[n] = rp[n - 1] / dp[n - 1];
    for j in (1..n).rev() {
        x[j] = (rp[j - 1] - off[j] * x[j + 1]) / dp[j - 1];
    }
    x
}

/// The strain of each element of the chain, `(u_{e+1} - u_e)/h`.
pub fn element_strains(u: &[C64]) -> Vec<C64> {
    let h = L / (u.len() - 1) as f64;
    (0..u.len() - 1).map(|e| (u[e + 1] - u[e]).scale(1.0 / h)).collect()
}
