//! F-491 (ADR 0400): static correction of the harmonic response for free bodies (inertia relief), on the transformation
//! path and with a pre-stressed basis. Checks A1 to A5, C1 to C9 and the negative controls N1 to N5 of the registration
//! (A6, the bits of ADR 0391, is the scratch probe B1).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicOptions, HarmonicResponse, HarmonicSolver, ModalBasis};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{add_supports, solve_static_mpc, transformation};
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const B: f64 = 0.04;
const KAPPA: f64 = 0.25;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
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

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn steel() -> Material {
    Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap()
}

fn c_wave() -> f64 {
    (E_MOD / RHO).sqrt()
}

fn options10() -> ModalOptions {
    ModalOptions::new(10)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

/// The dense block of `a` on the index list `idx` (row-major).
fn dense_of(a: &CsrMatrix, idx: &[usize]) -> Vec<f64> {
    let n = idx.len();
    let mut pos = vec![usize::MAX; a.n.max(a.ncols)];
    for (i, &e) in idx.iter().enumerate() {
        pos[e] = i;
    }
    let mut d = vec![0.0; n * n];
    for (i, &e) in idx.iter().enumerate() {
        for p in a.row_ptr[e]..a.row_ptr[e + 1] {
            let j = pos[a.col[p]];
            if j != usize::MAX {
                d[i * n + j] += a.val[p];
            }
        }
    }
    d
}

/// Gaussian elimination with partial pivoting, `a` row-major `n x n`.
fn dense_solve(mut a: Vec<f64>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for k in 0..n {
        let p = (k..n)
            .max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))
            .unwrap();
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        let d = a[k * n + k];
        for i in k + 1..n {
            let f = a[i * n + k] / d;
            if f != 0.0 {
                for j in k..n {
                    a[i * n + j] -= f * a[k * n + j];
                }
                b[i] -= f * b[k];
            }
        }
    }
    let mut x = vec![0.0; n];
    for k in (0..n).rev() {
        let s: f64 = (k + 1..n).map(|j| a[k * n + j] * x[j]).sum();
        x[k] = (b[k] - s) / a[k * n + k];
    }
    x
}

/// `max_j |X_j - y_j| / max_j |y_j|` of a complex response against a real reference.
fn rel_over_equations(x: &HarmonicResponse, y: &[f64]) -> f64 {
    let num = (0..y.len()).fold(0.0f64, |m, j| m.max((x.re[j] - y[j]).hypot(x.im[j])));
    let den = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    num / den
}

/// `max_j |X_j - Y_j| / max_j |Y_j|` of two complex responses.
fn rel_between(x: &HarmonicResponse, y: &HarmonicResponse) -> f64 {
    let n = y.re.len();
    let num = (0..n).fold(0.0f64, |m, j| m.max((x.re[j] - y.re[j]).hypot(x.im[j] - y.im[j])));
    let den = (0..n).fold(0.0f64, |m, j| m.max(y.amplitude(j)));
    num / den
}

fn bits(r: &HarmonicResponse) -> Vec<u64> {
    r.re.iter().chain(&r.im).map(|v| v.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------------- the bars

/// FF: the free-free bar of 40 Hex8 (transverse components fixed everywhere, the axis free).
fn ff_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, B, B], [40, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

/// KS: FF on an end spring `k = kappa E A / L` at `xmin` (k/4 on each of the four nodes).
fn ks_model() -> SolidModel {
    let mut m = ff_model();
    let k = KAPPA * E_MOD * B * B / 1.0;
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, k / 4.0)).unwrap();
    m
}

/// B40 of ADR 0391 (the axis fixed at `xmin`).
fn b40_model() -> SolidModel {
    let mut m = ff_model();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

/// 0.25 along `component` on each node of `set`.
fn end_load(model: &SolidModel, set: &str, component: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set(set).unwrap() {
        f[dofs.equation(a, component).unwrap()] = 0.25;
    }
    f
}

fn equation(model: &SolidModel, set: &str, component: usize) -> usize {
    let node = model.mesh().node_set(set).unwrap()[0];
    model.dof_map().equation(node, component).unwrap()
}

fn forces_of(model: &SolidModel, f: &[f64]) -> Vec<NodalForceAt> {
    let dofs = model.dof_map();
    (0..model.mesh().nodes().len())
        .map(|a| {
            let c = |k: usize| dofs.equation(a, k).map(|e| f[e]).unwrap_or(0.0);
            NodalForceAt::new(a, [c(0), c(1), c(2)])
        })
        .collect()
}

fn free_equations(model: &SolidModel) -> Vec<usize> {
    let n = model.dof_map().n_equations();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let mut is_fixed = vec![false; n];
    for &(e, _) in &fixed {
        is_fixed[e] = true;
    }
    (0..n).filter(|&e| !is_fixed[e]).collect()
}

/// The direct dense solve of `(K + K_sigma - omega^2 M) x = f` on the free equations of a model with prescribed
/// displacements only.
fn direct_plain(model: &SolidModel, prestress: Option<&Prestress>, f: &[f64], omega: f64) -> Vec<f64> {
    let free = free_equations(model);
    let k = dense_of(&stiffness(model).unwrap(), &free);
    let m = dense_of(&consistent(model).unwrap(), &free);
    let kg = prestress.map(|p| dense_of(&geometric_stiffness(model, p, None).unwrap(), &free));
    let w2 = omega * omega;
    let a: Vec<f64> = (0..k.len())
        .map(|i| k[i] + kg.as_ref().map_or(0.0, |g| g[i]) - w2 * m[i])
        .collect();
    let rhs: Vec<f64> = free.iter().map(|&e| f[e]).collect();
    let x = dense_solve(a, rhs);
    let mut full = vec![0.0; f.len()];
    for (&e, &v) in free.iter().zip(&x) {
        full[e] = v;
    }
    full
}

/// The direct dense solve on the transformation: `(K_r + K_sigma,r - omega^2 M_r) q = T^T f_ext`, `x = (T q)_mesh`.
fn direct_transformation(model: &SolidModel, prestress: Option<&Prestress>, f: &[f64], omega: f64) -> Vec<f64> {
    let tr = transformation(model, &[]).unwrap();
    let mut k = stiffness(model).unwrap();
    add_supports(&mut k, model).unwrap();
    let m = consistent(model).unwrap();
    let zero = vec![0.0; tr.n_unknowns()];
    let all: Vec<usize> = (0..tr.independent().len()).collect();
    let kr = dense_of(&tr.reduce(&k, &zero).unwrap().0, &all);
    let mr = dense_of(&tr.reduce(&m, &zero).unwrap().0, &all);
    let kg = prestress.map(|p| {
        let g = geometric_stiffness(model, p, None).unwrap();
        dense_of(&tr.reduce(&g, &zero).unwrap().0, &all)
    });
    let w2 = omega * omega;
    let a: Vec<f64> = (0..kr.len())
        .map(|i| kr[i] + kg.as_ref().map_or(0.0, |g| g[i]) - w2 * mr[i])
        .collect();
    let mut f_ext = f.to_vec();
    f_ext.resize(tr.n_unknowns(), 0.0);
    let q = dense_solve(a, tr.transpose_times(&f_ext));
    let mut x = tr.expand(&q);
    x.truncate(f.len());
    x
}

// ------------------------------------------------------------------------------------- FF: A1, A2, C1-C3, N1, N5

#[test]
fn a1_a2_c1_c2_c3_free_free_bar_inertia_relief() {
    let mut ch = Checks::new();
    let ff = ff_model();
    let modal = modal_analysis(&ff, &options10()).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    println!("FF: eigenvalues {:?}", basis.eigenvalues());
    let omega1 = basis.eigenvalues()[1].sqrt();
    println!("FF: first elastic omega_1 {omega1:.10e} rad/s");
    let load = HarmonicLoad::real(end_load(&ff, "xmax", 0)).unwrap();
    let with = HarmonicSolver::new(&ff, basis.clone(), &HarmonicOptions::default()).unwrap();
    let without =
        HarmonicSolver::new(&ff, basis.clone(), &HarmonicOptions::default().static_correction(false)).unwrap();
    let tip = equation(&ff, "xmax", 0);
    let far = equation(&ff, "xmin", 0);
    let ea = E_MOD * B * B;
    let m = consistent(&ff).unwrap();
    let n = ff.dof_map().n_equations();
    let fixed = prescribed_equations(&ff, &[]).unwrap();
    let mut r = vec![0.0; n];
    for &a in &(0..ff.mesh().nodes().len()).collect::<Vec<_>>() {
        r[ff.dof_map().equation(a, 0).unwrap()] = 1.0;
    }
    for &(e, _) in &fixed {
        r[e] = 0.0;
    }
    for s in [0.5, 1.5] {
        let omega = s * omega1;
        let k = omega / c_wave();
        let scale = 1.0 / (ea * k);
        let u_tip = -(k * 1.0).cos() / (k * 1.0).sin() * scale;
        let u_far = -scale / (k * 1.0).sin();
        let xw = with.response(&load, omega).unwrap();
        let xn = without.response(&load, omega).unwrap();
        println!(
            "FF s = {s}: closed tip {u_tip:.6e} far {u_far:.6e}; with tip {:.6e} far {:.6e}; without tip {:.6e} far {:.6e}",
            xw.re[tip], xw.re[far], xn.re[tip], xn.re[far]
        );
        let a1 = (xw.re[tip] - u_tip).hypot(xw.im[tip]) / scale;
        ch.at_most(&format!("A1 s = {s}: |X_tip - u_cf(L)| / (F/(EAk)), inertia relief"), a1, 0.005);
        let an = (xn.re[tip] - u_tip).hypot(xn.im[tip]) / scale;
        if s == 0.5 {
            ch.above("A2 s = 0.5: the same without the correction", an, 0.01);
        } else {
            ch.above("N1 s = 1.5: the same without the correction", an, 0.005);
        }
        let c1 = (xw.re[far] - u_far).hypot(xw.im[far]) / u_far.abs();
        ch.at_most(&format!("C1 s = {s}: far end |X_0 / u_cf(0) - 1|"), c1, 0.005);
        if s == 0.5 {
            let n5 = (xn.re[far] - u_far).hypot(xn.im[far]) / u_far.abs();
            println!("N5 s = 0.5: far end without the correction {n5:.6e} (recorded, no limit)");
        }
        let direct = direct_plain(&ff, None, load.re(), omega);
        let c2 = rel_over_equations(&xw, &direct);
        ch.at_most(&format!("C2 s = {s}: against the direct dense solve, every equation"), c2, 2e-3);
        if s == 0.5 {
            let res: Vec<f64> = xw.re.iter().zip(&xn.re).map(|(a, b)| a - b).collect();
            let mres = mv(&m, &res);
            let mr = mv(&m, &r);
            let c3 = dot(&r, &mres).abs() / (dot(&r, &mr).sqrt() * dot(&res, &mres).sqrt());
            ch.at_most("C3 residual M-orthogonal to the rigid motion", c3, 1e-10);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------ KS: A3, C4, C8, N2

#[test]
fn a3_c4_c8_spring_bar_on_the_transformation_path() {
    let mut ch = Checks::new();
    let ks = ks_model();
    let modal = modal_analysis(&ks, &options10()).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    println!("KS: eigenvalues {:?}", basis.eigenvalues());
    let omega1 = basis.eigenvalues()[0].sqrt();
    println!("KS: omega_1 {omega1:.10e} rad/s");
    let f = end_load(&ks, "xmax", 0);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let with = HarmonicSolver::new(&ks, basis.clone(), &HarmonicOptions::default()).unwrap();
    let without =
        HarmonicSolver::new(&ks, basis.clone(), &HarmonicOptions::default().static_correction(false)).unwrap();
    for s in [0.5, 1.5] {
        let omega = s * omega1;
        let direct = direct_transformation(&ks, None, &f, omega);
        let a3 = rel_over_equations(&with.response(&load, omega).unwrap(), &direct);
        ch.at_most(&format!("A3 s = {s}: against the direct dense solve, every equation"), a3, 1e-3);
        if s == 0.5 {
            let n2 = rel_over_equations(&without.response(&load, omega).unwrap(), &direct);
            ch.above("N2 s = 0.5: the same without the correction", n2, 1e-3);
        }
    }
    let st = solve_static_mpc(&ks, &[], &forces_of(&ks, &f), &[], &StaticOptions::default()).unwrap();
    let c4 = rel_over_equations(&with.response(&load, 0.0).unwrap(), st.result.displacement_values());
    ch.at_most("C4 omega = 0 against solve_static_mpc", c4, 1e-10);
    let omegas = [0.5 * omega1, 1.5 * omega1];
    let sw = with.sweep(&load, &omegas).unwrap();
    let same = omegas
        .iter()
        .zip(&sw)
        .all(|(&w, r)| bits(r) == bits(&with.response(&load, w).unwrap()));
    ch.holds("C8 sweep against response", same, if same { "bit-equal" } else { "differs" });
    ch.finish();
}

// ------------------------------------------------------------------------------------------------ KSP: A4, N3

#[test]
fn a4_prestressed_spring_bar_on_the_transformation_path() {
    let mut ch = Checks::new();
    let ks = ks_model();
    let p = Prestress::uniform(&ks, [0.05 * E_MOD, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let modal = prestressed_modes(&ks, &p, &options10()).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    println!("KSP: eigenvalues {:?}", basis.eigenvalues());
    let omega1 = basis.eigenvalues()[0].sqrt();
    let f = end_load(&ks, "xmax", 0);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let with = HarmonicSolver::new(&ks, basis.clone(), &HarmonicOptions::default().prestress(p.clone())).unwrap();
    let elastic = HarmonicSolver::new(&ks, basis.clone(), &HarmonicOptions::default()).unwrap();
    for s in [0.0, 0.5] {
        let omega = s * omega1;
        let direct = direct_transformation(&ks, Some(&p), &f, omega);
        let a4 = rel_over_equations(&with.response(&load, omega).unwrap(), &direct);
        ch.at_most(&format!("A4 s = {s}: pre-stressed, against the direct dense solve"), a4, 1e-3);
        if s == 0.0 {
            let n3 = rel_over_equations(&elastic.response(&load, omega).unwrap(), &direct);
            ch.above("N3 s = 0: the correction with the elastic K", n3, 1e-3);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------- CP: A5, C5, N4

#[test]
fn a5_c5_prestressed_cantilever_plain_path() {
    let mut ch = Checks::new();
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut cp = SolidModel::new(mesh);
    cp.add_material("all", steel()).unwrap();
    for c in 0..3 {
        cp.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let p = Prestress::uniform(&cp, [5e7, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let modal = prestressed_modes(&cp, &p, &options10()).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    println!("CP: eigenvalues {:?}", basis.eigenvalues());
    let omega1 = basis.eigenvalues()[0].sqrt();
    let f = end_load(&cp, "xmax", 1);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let with = HarmonicSolver::new(&cp, basis.clone(), &HarmonicOptions::default().prestress(p.clone())).unwrap();
    let elastic = HarmonicSolver::new(&cp, basis.clone(), &HarmonicOptions::default()).unwrap();
    for s in [0.0, 0.5] {
        let omega = s * omega1;
        let direct = direct_plain(&cp, Some(&p), &f, omega);
        let x = with.response(&load, omega).unwrap();
        let a5 = rel_over_equations(&x, &direct);
        ch.at_most(&format!("A5 s = {s}: pre-stressed, against the direct dense solve"), a5, 1e-3);
        if s == 0.0 {
            ch.at_most("C5 omega = 0 against the dense (K + K_sigma)^-1 F", a5, 1e-8);
            let n4 = rel_over_equations(&elastic.response(&load, omega).unwrap(), &direct);
            ch.above("N4 omega = 0: the correction with the elastic K", n4, 1e-3);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------- SF: C6; C7

#[test]
fn c6_small_free_block_complete_basis() {
    let mut ch = Checks::new();
    let mut sf = SolidModel::new(hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 1, 1]).unwrap());
    sf.add_material("all", steel()).unwrap();
    let basis = ModalBasis::all_modes(&sf, None).unwrap();
    let mut eig = basis.eigenvalues().to_vec();
    eig.sort_by(f64::total_cmp);
    println!("SF: lowest eigenvalues {:?}", &eig[..8]);
    let omega = 0.5 * eig[6].sqrt();
    let n = sf.dof_map().n_equations();
    let mut f = vec![0.0; n];
    let s14 = 14f64.sqrt();
    for c in 0..3 {
        f[sf.dof_map().equation(0, c).unwrap()] = (c + 1) as f64 / s14;
    }
    let load = HarmonicLoad::real(f).unwrap();
    let with = HarmonicSolver::new(&sf, basis.clone(), &HarmonicOptions::default()).unwrap();
    let without = HarmonicSolver::new(&sf, basis, &HarmonicOptions::default().static_correction(false)).unwrap();
    let c6 = rel_between(
        &with.response(&load, omega).unwrap(),
        &without.response(&load, omega).unwrap(),
    );
    ch.at_most("C6 complete basis: with against without the correction", c6, 1e-8);
    ch.finish();
}

#[test]
fn c7_zero_prestress_bit_identical() {
    let mut ch = Checks::new();
    let b40 = b40_model();
    let modal = modal_analysis(&b40, &options10()).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    let omega = 0.5 * basis.eigenvalues()[0].sqrt();
    let load = HarmonicLoad::real(end_load(&b40, "xmax", 0)).unwrap();
    let zero = Prestress::uniform(&b40, [0.0; 6]).unwrap();
    let a = HarmonicSolver::new(&b40, basis.clone(), &HarmonicOptions::default()).unwrap();
    let b = HarmonicSolver::new(&b40, basis, &HarmonicOptions::default().prestress(zero)).unwrap();
    let same = bits(&a.response(&load, omega).unwrap()) == bits(&b.response(&load, omega).unwrap());
    ch.holds("C7 zero pre-stress against none", same, if same { "bit-equal" } else { "differs" });
    ch.finish();
}

// --------------------------------------------------------------------------------------------------------- C9

#[test]
fn c9_refusals() {
    let mut ch = Checks::new();
    let key = "static correction";
    let ff = ff_model();
    let ff_modal = modal_analysis(&ff, &options10()).unwrap();
    let ff_basis = ModalBasis::from_result(&ff_modal).unwrap();
    let elastic_only = ModalBasis::new(
        ff_basis.eigenvalues()[1..].to_vec(),
        ff_basis.modes()[1..].to_vec(),
        ff_basis.dofs().clone(),
    )
    .unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&ff, elastic_only, &HarmonicOptions::default()),
        key,
    );
    ch.holds("C9 FF without its rigid mode", ok, &d);
    let p = Prestress::uniform(&ff, [1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&ff, ff_basis.clone(), &HarmonicOptions::default().prestress(p)),
        key,
    );
    ch.holds("C9 FF with a pre-stress", ok, &d);
    let tip_nodes = ff.mesh().node_set("xmax").unwrap().to_vec();
    let mut tied = ff.clone();
    tied.add_constraint(Constraint::linear(
        vec![LinearTerm::new(tip_nodes[0], 0, 1.0), LinearTerm::new(tip_nodes[1], 0, -1.0)],
        0.0,
    ))
    .unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&tied, ff_basis.clone(), &HarmonicOptions::default()),
        key,
    );
    ch.holds("C9 FF with a Linear tie (transformation path, free axis)", ok, &d);
    let ks = ks_model();
    let ks_modal = modal_analysis(&ks, &options10()).unwrap();
    let ks_basis = ModalBasis::from_result(&ks_modal).unwrap();
    let mut eig = ks_basis.eigenvalues().to_vec();
    eig[0] = -1.0;
    let negative = ModalBasis::new(eig, ks_basis.modes().to_vec(), ks_basis.dofs().clone()).unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&ks, negative, &HarmonicOptions::default()),
        key,
    );
    ch.holds("C9 KS with an eigenvalue -1", ok, &d);
    let mut valued = ks.clone();
    valued
        .add_constraint(Constraint::linear(
            vec![LinearTerm::new(tip_nodes[0], 0, 1.0), LinearTerm::new(tip_nodes[1], 0, -1.0)],
            1e-3,
        ))
        .unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&valued, ks_basis, &HarmonicOptions::default()),
        key,
    );
    ch.holds("C9 KS with a non-zero Linear value", ok, &d);
    ch.finish();
}
