//! Electric potential and Joule heating library checks (F-213, ADR 0035 checks H1 to H7). The limits are the ones
//! registered in `docs/adr/0035-wave21a-thermal-extras.md` before any run.

mod cht_common;

use cht_common::*;
use rcfd_fv::bc::{FixedGradient, Profile};
use rcfd_fv::cht::{conductive_face_flows, patch_heat_flow_out};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::{History, TimeScheme};
use rcfd_fv::electric::{boundary_power, face_conductivity, face_currents, joule_heat, sigma_of_t, solve};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField};
use rcfd_mesh::gen::{box_2d, box_2d_graded, Grading};
use std::sync::Arc;

fn phi_field(geom: &Arc<FvGeometry>, left: f64, right: f64) -> ScalarField {
    let mut p = ScalarField::uniform("phi", geom, 0.0);
    p.set_bc(geom, "xmin", Box::new(FixedValue::scalar(left))).unwrap();
    p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(right))).unwrap();
    p
}

fn bar(n: usize) -> Arc<FvGeometry> {
    geometry(box_2d(n, 1, 1.0, 1.0, false, false).unwrap())
}

fn current_out(geom: &FvGeometry, phi: &ScalarField, sf: &[f64], patch: &str) -> f64 {
    let flows = face_currents(geom, phi, sf, false);
    patch_heat_flow_out(geom, &flows, patch).unwrap()
}

#[test]
fn h1_ohm_bar() {
    let geom = bar(40);
    let mut phi = phi_field(&geom, 10.0, 0.0);
    let sigma = vec![100.0; geom.n_cells()];
    let (sf, _) = solve(&geom, &mut phi, &sigma, &tight(), 5);
    let cc = geom.mesh.cell_centres();
    let err = cc
        .iter()
        .zip(&phi.values)
        .map(|(c, p)| (p - 10.0 * (1.0 - c[0])).abs())
        .fold(0.0, f64::max)
        / 10.0;
    let i_exact = 100.0 * 10.0 * patch_area(&geom, "xmin");
    let i_in = -current_out(&geom, &phi, &sf, "xmin");
    let i_out = current_out(&geom, &phi, &sf, "xmax");
    let e_i = (i_in - i_exact).abs() / i_exact;
    let e_s = (i_in - i_out).abs() / i_exact;
    println!("H1 phi error {err:.3e}, current error {e_i:.3e}, in/out {e_s:.3e} (limits 1e-10)");
    assert!(err <= 1e-10 && e_i <= 1e-10 && e_s <= 1e-10);
}

#[test]
fn h2_two_layer_bar() {
    let geom = bar(40);
    let mut phi = phi_field(&geom, 10.0, 0.0);
    let sigma: Vec<f64> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| if c[0] < 0.5 { 100.0 } else { 10.0 })
        .collect();
    let (sf, _) = solve(&geom, &mut phi, &sigma, &tight(), 5);
    let j = 10.0 / (0.5 / 100.0 + 0.5 / 10.0);
    let exact = |x: f64| {
        if x < 0.5 {
            10.0 - j * x / 100.0
        } else {
            10.0 - j * 0.5 / 100.0 - j * (x - 0.5) / 10.0
        }
    };
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&phi.values)
        .map(|(c, p)| (p - exact(c[0])).abs())
        .fold(0.0, f64::max)
        / 10.0;
    let i_in = -current_out(&geom, &phi, &sf, "xmin");
    let e_i = (i_in - j * patch_area(&geom, "xmin")).abs() / j;
    println!("H2 phi error {err:.3e}, current error {e_i:.3e} (limits 1e-10)");
    assert!(err <= 1e-10 && e_i <= 1e-10);
}

fn checker(geom: &FvGeometry, nx: usize) -> Vec<f64> {
    (0..geom.n_cells())
        .map(|c| if (c % nx + c / nx) % 2 == 0 { 1.0 } else { 10.0 })
        .collect()
}

#[test]
fn h3_power_identity() {
    for (name, mesh) in [
        ("uniform", box_2d(16, 16, 1.0, 1.0, false, false).unwrap()),
        (
            "graded",
            box_2d_graded(
                16,
                16,
                1.0,
                1.0,
                [Grading::Expansion(4.0), Grading::Symmetric(3.0)],
                false,
                false,
            )
            .unwrap(),
        ),
    ] {
        let geom = geometry(mesh);
        let mut phi = phi_field(&geom, 1.0, 0.0);
        let sigma = checker(&geom, 16);
        let (sf, _) = solve(&geom, &mut phi, &sigma, &tight(), 5);
        let q = joule_heat(&geom, &phi, &sf, false);
        let vol = geom.mesh.cell_volumes();
        let total: f64 = q.iter().zip(vol).map(|(q, v)| q * v).sum();
        let i_v = -current_out(&geom, &phi, &sf, "xmin");
        let pw = boundary_power(&geom, &phi, &sf, false);
        let rel = (total - i_v).abs() / i_v;
        println!(
            "H3 {name}: sum QV {total:.9e}, I V {i_v:.9e}, relative {rel:.3e} (limit 1e-10); boundary_power {pw:.9e}"
        );
        assert!(rel <= 1e-10, "H3 {name} {rel}");
    }
}

/// Frozen-flow energy iteration with the Joule source of the current potential, lagged coupling.
struct Coupled {
    geom: Arc<FvGeometry>,
    phi: ScalarField,
    en: rcfd_fv::energy::EnergySolver,
    sigma0: f64,
    a: f64,
    t_ref: f64,
    rho_cp: f64,
    current_density: Option<f64>,
    sf: Vec<f64>,
}

impl Coupled {
    fn new(geom: &Arc<FvGeometry>, k: f64, rho_cp: f64, phi: ScalarField, t: ScalarField, sigma0: f64, a: f64) -> Self {
        let mats = uniform_materials(geom, k, rho_cp);
        let en = cht_solver(geom, t, &mats, ConvScheme::Upwind);
        Coupled {
            geom: geom.clone(),
            phi,
            en,
            sigma0,
            a,
            t_ref: 300.0,
            rho_cp,
            current_density: None,
            sf: vec![],
        }
    }

    fn sigma(&self) -> Vec<f64> {
        self.en
            .t
            .values
            .iter()
            .map(|&t| sigma_of_t(self.sigma0, self.a, self.t_ref, t).unwrap())
            .collect()
    }

    /// Potential for the current temperatures, then the Joule source.
    fn update_source(&mut self) {
        let sigma = self.sigma();
        if let Some(j) = self.current_density {
            let own = self.geom.mesh.owner();
            let prof = Profile::PerFace(
                self.geom
                    .mesh
                    .patch("xmin")
                    .unwrap()
                    .faces()
                    .map(|f| [j / sigma[own[f]], 0.0, 0.0])
                    .collect(),
            );
            self.phi
                .set_bc(&self.geom, "xmin", Box::new(FixedGradient(prof)))
                .unwrap();
        }
        let (sf, _) = solve(&self.geom, &mut self.phi, &sigma, &tight(), 5);
        self.sf = sf;
        let q = joule_heat(&self.geom, &self.phi, &self.sf, false);
        self.en.source = Some(q.iter().map(|x| x / self.rho_cp).collect());
    }

    /// Steady Picard iteration to a temperature change below `tol`.
    fn steady(&mut self, tol: f64, max: usize) -> usize {
        let zero = vec![0.0; self.geom.mesh.n_faces()];
        for i in 1..=max {
            let before = self.en.t.values.clone();
            self.update_source();
            self.en.iterate(&zero);
            let ch = before
                .iter()
                .zip(&self.en.t.values)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            if ch < tol {
                return i;
            }
        }
        max
    }
}

fn dirichlet_t(geom: &Arc<FvGeometry>) -> ScalarField {
    let mut t = ScalarField::uniform("T", geom, 300.0);
    t.set_bc(geom, "xmin", Box::new(FixedValue::scalar(300.0))).unwrap();
    t.set_bc(geom, "xmax", Box::new(FixedValue::scalar(300.0))).unwrap();
    t
}

fn parabola_run(n: usize) -> (f64, f64, f64) {
    let geom = bar(n);
    let mut c = Coupled::new(
        &geom,
        2.0,
        1.0,
        phi_field(&geom, 2.0, 0.0),
        dirichlet_t(&geom),
        100.0,
        0.0,
    );
    c.steady(1e-11, 50);
    // E = 2, Q = sigma E^2 = 400, T - 300 = Q/(2k) x (1 - x), peak 25 K
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&c.en.t.values)
        .map(|(x, t)| (t - (300.0 + 100.0 * x[0] * (1.0 - x[0]))).abs())
        .fold(0.0, f64::max)
        / 25.0;
    let vol = geom.mesh.cell_volumes();
    let q = joule_heat(&geom, &c.phi, &c.sf, false);
    let qv: f64 = q.iter().zip(vol).map(|(q, v)| q * v).sum();
    let flows = conductive_face_flows(&geom, &c.en.t, &vec![2.0; geom.mesh.n_faces()], false);
    let out: f64 = ["xmin", "xmax"]
        .iter()
        .map(|p| patch_heat_flow_out(&geom, &flows, p).unwrap())
        .sum();
    let iv = -current_out(&geom, &c.phi, &c.sf, "xmin") * 2.0;
    let bal = (out - qv).abs() / qv;
    let bal_iv = (out - iv).abs() / iv;
    println!("H4 N={n}: peak-normalised error {err:.4e}, balance vs sum QV {bal:.3e}, vs I V {bal_iv:.3e}");
    (err, bal, bal_iv)
}

#[test]
fn h4_joule_parabola() {
    let r: Vec<_> = [20, 40, 80].iter().map(|&n| parabola_run(n)).collect();
    let o1 = (r[0].0 / r[1].0).log2();
    let o2 = (r[1].0 / r[2].0).log2();
    println!(
        "H4 errors {:.4e} {:.4e} {:.4e}, orders {o1:.3} {o2:.3} (limits: N=20 <= 1e-2, order >= 1.8, balances 1e-9)",
        r[0].0, r[1].0, r[2].0
    );
    assert!(r[0].0 <= 1e-2, "H4 error {}", r[0].0);
    assert!(o1 >= 1.8 && o2 >= 1.8, "H4 orders {o1} {o2}");
    assert!(r.iter().all(|x| x.1 <= 1e-9 && x.2 <= 1e-9), "H4 balances {r:?}");
}

/// theta(x) of the sigma(T) bar and the exact voltage (ADR 0035, closed forms).
fn h5_exact(a: f64, rho0: f64, j: f64, k: f64, l: f64) -> (impl Fn(f64) -> f64, f64, f64) {
    let m = (a * rho0 * j * j / k).sqrt();
    let theta = move |x: f64| ((m * (x - l / 2.0)).cos() / (m * l / 2.0).cos() - 1.0) / a;
    let v = 2.0 * rho0 * j * (m * l / 2.0).tan() / m;
    (theta, v, m)
}

fn h5_run(n: usize) -> (f64, f64) {
    let (a, rho0, j, k, l) = (0.1, 1.0, 4.0, 1.0, 1.0);
    let geom = bar(n);
    let mut phi = ScalarField::uniform("phi", &geom, 0.0);
    phi.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut c = Coupled::new(&geom, k, 1.0, phi, dirichlet_t(&geom), 1.0 / rho0, a);
    c.current_density = Some(j);
    let its = c.steady(1e-11, 200);
    assert!(its < 200, "Picard iteration did not converge");
    let (theta, v, _m) = h5_exact(a, rho0, j, k, l);
    let peak = theta(0.5);
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&c.en.t.values)
        .map(|(x, t)| (t - 300.0 - theta(x[0])).abs())
        .fold(0.0, f64::max)
        / peak;
    let bv = c.phi.boundary_values(&geom);
    let nbx = geom.mesh.patch("xmin").unwrap().faces().count();
    let off = geom.mesh.patch("xmin").unwrap().faces().next().unwrap() - geom.n_internal;
    let v_num = bv[off..off + nbx].iter().sum::<f64>() / nbx as f64;
    let ev = (v_num - v).abs() / v;
    println!("H5 N={n}: Picard sweeps {its}, theta peak {peak:.4}, error/peak {err:.4e}, voltage {v_num:.6} vs {v:.6} (rel {ev:.3e})");
    (err, ev)
}

#[test]
fn h5_sigma_of_t_bar() {
    let r: Vec<_> = [20, 40, 80].iter().map(|&n| h5_run(n)).collect();
    let o1 = (r[0].0 / r[1].0).log2();
    let o2 = (r[1].0 / r[2].0).log2();
    println!("H5 orders {o1:.3} {o2:.3}; limits: N=40 temperature <= 5e-3, voltage <= 5e-3, order >= 1.8");
    assert!(r[1].0 <= 5e-3, "H5 temperature {}", r[1].0);
    assert!(r[1].1 <= 5e-3, "H5 voltage {}", r[1].1);
    assert!(o1 >= 1.8 && o2 >= 1.8, "H5 orders {o1} {o2}");
}

fn transient_run(a: f64, steps: usize, t_end: f64) -> f64 {
    let geom = bar(10);
    let mut t = ScalarField::uniform("T", &geom, 300.0);
    // adiabatic ends: default zero-gradient
    t.set_bc(&geom, "ymin", Box::new(rcfd_fv::ZeroGradient)).unwrap();
    let mut c = Coupled::new(&geom, 1.0, 1.0, phi_field(&geom, 1.0, 0.0), t, 1.0, a);
    c.en.history = History::new(c.en.t.values.clone());
    let zero = vec![0.0; geom.mesh.n_faces()];
    let dt = t_end / steps as f64;
    for _ in 0..steps {
        c.update_source();
        c.en.advance(&zero, dt, TimeScheme::ImplicitEuler);
    }
    let mean = c.en.t.values.iter().sum::<f64>() / c.en.t.values.len() as f64;
    mean - 300.0
}

#[test]
fn h6_transient_joule_heating() {
    // (i) constant sigma: exact increase 2.0 K in 20 steps of 0.1
    let th = transient_run(0.0, 20, 2.0);
    println!("H6(i) rise {th:.12} vs 2 (limit 1e-10)");
    assert!((th - 2.0).abs() <= 1e-10, "H6 i {th}");
    // (ii) a = 0.5: theta + a theta^2/2 = t, t = 2 -> theta = 2 (sqrt 3 - 1)
    let exact = 2.0 * (3.0_f64.sqrt() - 1.0);
    let e: Vec<f64> = [40usize, 80, 160]
        .iter()
        .map(|&n| (transient_run(0.5, n, 2.0) - exact).abs() / exact)
        .collect();
    let (o1, o2) = ((e[0] / e[1]).log2(), (e[1] / e[2]).log2());
    println!(
        "H6(ii) errors {:.3e} {:.3e} {:.3e}, orders {o1:.3} {o2:.3} (limits: 40 steps <= 2e-2, order >= 0.9)",
        e[0], e[1], e[2]
    );
    assert!(e[0] <= 2e-2, "H6 ii {}", e[0]);
    assert!(o1 >= 0.9 && o2 >= 0.9, "H6 orders {o1} {o2}");
}

#[test]
fn h7_insulator_cells() {
    let geom = geometry(box_2d(20, 20, 1.0, 1.0, false, false).unwrap());
    let sigma: Vec<f64> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| if c[1] < 0.5 { 0.0 } else { 100.0 })
        .collect();
    let mut phi = phi_field(&geom, 10.0, 0.0);
    let (sf, _) = solve(&geom, &mut phi, &sigma, &tight(), 5);
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&phi.values)
        .filter(|(c, _)| c[1] > 0.5)
        .map(|(c, p)| (p - 10.0 * (1.0 - c[0])).abs())
        .fold(0.0, f64::max)
        / 10.0;
    let q = joule_heat(&geom, &phi, &sf, false);
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    let mut heat_ins = 0.0_f64;
    for (c, s) in sigma.iter().enumerate() {
        if *s == 0.0 {
            heat_ins = heat_ins.max(q[c].abs());
        }
    }
    let zero_faces = (0..geom.n_internal)
        .filter(|&f| sigma[own[f]] == 0.0 || sigma[nei[f]] == 0.0)
        .all(|f| sf[f] == 0.0);
    let fc = face_conductivity(&geom, &sigma);
    println!("H7 conducting phi error {err:.3e} (limit 1e-10), insulator heat {heat_ins:e} (exactly 0)");
    assert!(err <= 1e-10, "H7 {err}");
    assert_eq!(heat_ins, 0.0);
    assert!(zero_faces && fc == sf);
}
