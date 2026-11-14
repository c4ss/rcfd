//! F-532, ADR 0464: static residual vectors for the damped modes (checks B1, B1n, B2, B2n, B2b, D2, O1, S1, S2, H1, T1, N1).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AREA: f64 = 1e-4;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value >= lo && value <= hi;
        println!(
            "{name}: {value:e} (band [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn modal(m: usize) -> ModalOptions {
    ModalOptions::new(m)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn cantilever(held: bool) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap());
    m.add_material("all", steel(0.3)).unwrap();
    if held {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    m
}

/// The bar of ADR 0450 B1, B2: `[0, 1] x [0, 0.01]^2`, Hex8 100 x 1 x 1, `nu = 0`, y and z held, x held at `xmin`.
fn bar() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.01, 0.01], [100, 1, 1]).unwrap());
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn impedance() -> f64 {
    AREA * (E_MOD * RHO).sqrt()
}

fn c0() -> f64 {
    (E_MOD / RHO).sqrt()
}

fn tips(model: &SolidModel) -> Vec<usize> {
    model.mesh().node_set("xmax").unwrap().to_vec()
}

/// The node of the bar at `x = 0.5` with the `y`, `z` of node `a`.
fn mid_node(model: &SolidModel, a: usize) -> usize {
    let nodes = model.mesh().nodes();
    let p = nodes[a];
    nodes
        .iter()
        .position(|q| (q[0] - 0.5).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-12 && (q[2] - p[2]).abs() < 1e-12)
        .unwrap()
}

/// The bar with the tip dashpot `c` (`c/4` on each `xmax` node along x) on `modes` modes.
fn tip(c: f64, modes: usize, residual: bool, threads: usize) -> DampedModes {
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(modes))
        .residual_vectors(residual)
        .threads(threads);
    for a in tips(&model) {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

/// The relative distance of the reported root nearest `exact`.
fn nearest(d: &DampedModes, exact: C64) -> f64 {
    d.modes
        .iter()
        .filter(|m| m.oscillating)
        .map(|m| (m.eigenvalue - exact).abs() / exact.abs())
        .fold(f64::INFINITY, f64::min)
}

/// The relative error of the non-oscillating root of smallest modulus against `exact`.
fn real_root(d: &DampedModes, exact: f64) -> f64 {
    d.modes
        .iter()
        .filter(|m| !m.oscillating)
        .min_by(|p, q| p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()))
        .map_or(f64::INFINITY, |m| (m.eigenvalue - C64::from_real(exact)).abs() / exact.abs())
}

fn print_modes(d: &DampedModes, count: usize) {
    for m in d.modes.iter().take(count) {
        println!(
            "  lambda {} zeta {:.6e} osc {} residual {:e}",
            m.eigenvalue, m.damping_ratio, m.oscillating, m.residual
        );
    }
    println!(
        "  reported {} (osc {}, real {}), basis {}, residual vectors {}, notes {:?}, defects {}",
        d.modes.len(),
        d.oscillating,
        d.non_oscillating,
        d.basis,
        d.residual_vectors.len(),
        d.notes,
        d.defects.len()
    );
}

/// Every field of a result as bits (the list of `sa_damped_residual_default_bits.rs`, plus the residual vectors).
fn bits(d: &DampedModes) -> Vec<u64> {
    let mut v: Vec<f64> = Vec::new();
    v.push(d.modes.len() as f64);
    for m in &d.modes {
        v.extend([
            m.eigenvalue.re,
            m.eigenvalue.im,
            m.decay_rate,
            m.damped_omega,
            m.frequency,
            m.damping_ratio,
            f64::from(u8::from(m.oscillating)),
            m.residual,
        ]);
        for s in &m.shape {
            v.extend([s.re, s.im]);
        }
    }
    v.push(d.roots.len() as f64);
    for r in &d.roots {
        v.extend([r.re, r.im]);
    }
    v.extend(d.omegas.iter().copied());
    v.extend([d.basis as f64, d.oscillating as f64, d.non_oscillating as f64]);
    v.push(d.defects.len() as f64);
    for g in &d.defects {
        v.extend(g.indices.iter().map(|&i| i as f64));
        v.extend([g.mean.re, g.mean.im, g.spread]);
    }
    for n in &d.notes {
        v.push(n.len() as f64);
        v.extend(n.bytes().map(f64::from));
    }
    v.push(d.residual_vectors.len() as f64);
    v.extend(d.residual_vectors.iter().flatten().copied());
    v.into_iter().map(f64::to_bits).collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn apply(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// `f^T B (B^T K B)^-1 B^T f` by Gaussian elimination with partial pivoting.
fn compliance(k: &CsrMatrix, basis: &[Vec<f64>], f: &[f64]) -> f64 {
    let n = basis.len();
    let kb: Vec<Vec<f64>> = basis.iter().map(|b| apply(k, b)).collect();
    let mut a = vec![vec![0.0; n + 1]; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = dot(&basis[i], &kb[j]);
        }
        a[i][n] = dot(&basis[i], f);
    }
    let rhs: Vec<f64> = (0..n).map(|i| a[i][n]).collect();
    for col in 0..n {
        let p = (col..n).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs())).unwrap();
        a.swap(col, p);
        for r in col + 1..n {
            let q = a[r][col] / a[col][col];
            for c in col..=n {
                a[r][c] -= q * a[col][c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|j| a[i][j] * x[j]).sum();
        x[i] = (a[i][n] - s) / a[i][i];
    }
    dot(&rhs, &x)
}

/// `x^H M x` of a complex shape.
fn mass_norm(m: &CsrMatrix, x: &[C64]) -> f64 {
    let re: Vec<f64> = x.iter().map(|v| v.re).collect();
    let im: Vec<f64> = x.iter().map(|v| v.im).collect();
    dot(&re, &apply(m, &re)) + dot(&im, &apply(m, &im))
}

#[test]
fn b1_b2_bar_near_the_impedance_on_20_modes() {
    let mut c = Checks::new();
    let (z, s) = (impedance(), c0());
    println!("Z = {z} N s/m, c0 = {s} m/s");
    // B1, B1n: c = 0.9 Z
    let a9 = (0.9f64).atanh();
    let with = tip(0.9 * z, 20, true, 4);
    let without = tip(0.9 * z, 20, false, 4);
    print_modes(&with, 6);
    print_modes(&without, 6);
    let (mut ew, mut eo) = (Vec::new(), Vec::new());
    for j in 1..=3 {
        let exact = C64::new(-a9, (2 * j - 1) as f64 * PI / 2.0).scale(s);
        let (a, b) = (nearest(&with, exact), nearest(&without, exact));
        println!("  j {j}: exact {exact}, with {a:e}, without {b:e}");
        ew.push(a);
        eo.push(b);
    }
    c.at_most(
        "B1 c = 0.9 Z, 20 modes + residual vectors, first three roots, relative",
        ew.iter().copied().fold(0.0, f64::max),
        2e-2,
    );
    c.holds(
        "B1n without the option: each of the three errors > 2e-2",
        eo.iter().all(|&e| e > 2e-2),
        &format!("{eo:?}"),
    );
    c.within(
        "B1n without the option: the largest of the three",
        eo.iter().copied().fold(0.0, f64::max),
        2.5e-2,
        5.5e-2,
    );
    // B2, B2n, B2b: c = 2 Z
    let a2 = (0.5f64).atanh();
    let with2 = tip(2.0 * z, 20, true, 4);
    let without2 = tip(2.0 * z, 20, false, 4);
    print_modes(&with2, 6);
    print_modes(&without2, 6);
    let e2 = real_root(&with2, -a2 * s);
    let e2n = real_root(&without2, -a2 * s);
    c.at_most("B2 c = 2 Z, 20 modes + residual vectors, the real root, relative", e2, 1e-2);
    c.within("B2n without the option, the real root", e2n, 8e-3, 2e-2);
    println!("  B2n above 1e-2: {}", e2n > 1e-2);
    let mut eb = Vec::new();
    for j in 1..=3 {
        let exact = C64::new(-a2, j as f64 * PI).scale(s);
        eb.push(nearest(&with2, exact));
    }
    c.at_most(
        "B2b c = 2 Z with the option: oscillating roots j = 1, 2, 3",
        eb.iter().copied().fold(0.0, f64::max),
        2e-2,
    );
    let flagged = with2
        .modes
        .iter()
        .filter(|m| !m.oscillating)
        .all(|m| m.eigenvalue.im == 0.0 && (m.damping_ratio - 1.0).abs() <= 1e-12);
    let says = with2.notes.iter().any(|n| n.contains("non-oscillating"));
    c.holds(
        "B2b real roots reported non-oscillating, ratio 1, said in notes",
        with2.non_oscillating >= 1 && flagged && says,
        &format!("{} real; {eb:?}", with2.non_oscillating),
    );
    c.finish();
}

#[test]
fn o1_s1_h1_t1_the_residual_vectors() {
    let mut c = Checks::new();
    let z = impedance();
    let model = bar();
    let d = tip(0.9 * z, 20, true, 4);
    let modes = modal_analysis(&model, &modal(20)).unwrap().modes;
    let mass = consistent(&model).unwrap();
    let k = stiffness(&model).unwrap();
    let r = &d.residual_vectors;
    c.holds(
        "O1 4 residual vectors kept, basis 24",
        r.len() == 4 && d.basis == 24,
        &format!("{} vectors, basis {}", r.len(), d.basis),
    );
    let mr: Vec<Vec<f64>> = r.iter().map(|v| apply(&mass, v)).collect();
    let mut ortho: f64 = 0.0;
    for p in &modes {
        for q in &mr {
            ortho = ortho.max(dot(p, q).abs());
        }
    }
    let mut norm: f64 = 0.0;
    for (i, a) in r.iter().enumerate() {
        for (j, q) in mr.iter().enumerate() {
            norm = norm.max((dot(a, q) - if i == j { 1.0 } else { 0.0 }).abs());
        }
    }
    c.at_most("O1 max |phi_k^T M r_i|", ortho, 1e-10);
    c.at_most("O1 max |r_i^T M r_j - delta_ij|", norm, 1e-10);
    // S1
    let dofs = model.dof_map();
    let mut f = vec![0.0; k.n];
    for a in tips(&model) {
        f[dofs.equation(a, 0).unwrap()] += 0.25;
    }
    let exact = 1.0 / (E_MOD * AREA);
    let mut basis = modes.clone();
    basis.extend(r.iter().cloned());
    let with = compliance(&k, &basis, &f);
    let alone = compliance(&k, &modes, &f);
    c.at_most(
        "S1 tip compliance on the basis against L/(E A), relative",
        (with - exact).abs() / exact,
        1e-8,
    );
    let deficit = (exact - alone) / exact;
    c.holds(
        "S1 control: the 20 modes alone fall short by >= 5e-3",
        deficit >= 5e-3,
        &format!("{deficit:e}"),
    );
    // H1
    let mn = d
        .modes
        .iter()
        .map(|m| (mass_norm(&mass, &m.shape) - 1.0).abs())
        .fold(0.0, f64::max);
    c.at_most("H1 shapes |x^H M x - 1|", mn, 1e-10);
    c.holds(
        "H1 a note names the residual vectors",
        d.notes.iter().any(|n| n.contains("residual vectors")),
        &format!("{:?}", d.notes),
    );
    // T1
    let d1 = tip(0.9 * z, 20, true, 1);
    let same = bits(&d1) == bits(&d);
    c.holds(
        "T1 threads 1 and 4 bit-identical with the option",
        same,
        &format!("{} words", bits(&d).len()),
    );
    c.finish();
}

#[test]
fn s2_dashpots_between_two_nodes() {
    let mut c = Checks::new();
    let z = impedance();
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(20)).residual_vectors(true);
    let dofs = model.dof_map();
    let k = stiffness(&model).unwrap();
    let mut f = vec![0.0; k.n];
    let mut x_mid = 0.0;
    for a in tips(&model) {
        let b = mid_node(&model, a);
        x_mid = model.mesh().nodes()[b][0];
        o = o.dashpot(Dashpot::between(a, b, 0, 0.9 * z / 4.0));
        f[dofs.equation(a, 0).unwrap()] += 0.25;
        f[dofs.equation(b, 0).unwrap()] -= 0.25;
    }
    let d = damped_modes(&model, &o).unwrap();
    print_modes(&d, 4);
    let modes = modal_analysis(&model, &modal(20)).unwrap().modes;
    let mut basis = modes.clone();
    basis.extend(d.residual_vectors.iter().cloned());
    let exact = (1.0 - x_mid) / (E_MOD * AREA);
    let with = compliance(&k, &basis, &f);
    let alone = compliance(&k, &modes, &f);
    c.holds(
        "S2 4 residual vectors kept",
        d.residual_vectors.len() == 4,
        &format!("{}", d.residual_vectors.len()),
    );
    c.at_most(
        "S2 compliance of the pair load on the basis against (L - x_mid)/(E A), relative",
        (with - exact).abs() / exact,
        1e-8,
    );
    println!("  S2 modes alone: deficit {:e} (recorded)", (exact - alone) / exact);
    c.finish();
}

#[test]
fn d2_option_without_dashpots_is_the_default() {
    let mut c = Checks::new();
    let model = cantilever(true);
    let u = modal_analysis(&model, &modal(12)).unwrap();
    let w: Vec<f64> = u.eigenvalues.iter().map(|l| l.sqrt()).collect();
    let ray = DampedOptions::with_modal(modal(12))
        .rayleigh(0.2 * w[0], 0.5 / w[11])
        .threads(4);
    let lf = DampedOptions::with_modal(modal(12)).loss_factor("all", LossFactor::Constant(0.04));
    for (what, o) in [("Rayleigh", ray), ("loss factor", lf)] {
        let off = damped_modes(&model, &o).unwrap();
        let on = damped_modes(&model, &o.clone().residual_vectors(true)).unwrap();
        c.holds(
            &format!("D2 {what}: option on without dashpots bit-identical to off"),
            bits(&on) == bits(&off) && on.residual_vectors.is_empty(),
            &format!("{} words", bits(&off).len()),
        );
    }
    c.finish();
}

#[test]
fn n1_refusal_and_drop() {
    let mut c = Checks::new();
    let free = cantilever(false);
    let node = free.mesh().node_set("xmax").unwrap()[0];
    let r = damped_modes(
        &free,
        &DampedOptions::new(4)
            .dashpot(Dashpot::ground(node, 0, 10.0))
            .residual_vectors(true),
    );
    let msg = match &r {
        Err(e) => e.to_string(),
        Ok(_) => "Ok".to_string(),
    };
    c.holds(
        "N1 free rigid-body motion refused with the option",
        matches!(r, Err(rcfd_fem::FemError::InvalidValue { .. })) && msg.contains("rigid"),
        &msg,
    );
    let model = bar();
    let a = tips(&model)[0];
    let z = impedance();
    let o = DampedOptions::with_modal(modal(20))
        .dashpot(Dashpot::ground(a, 0, 0.45 * z))
        .dashpot(Dashpot::ground(a, 0, 0.45 * z))
        .residual_vectors(true);
    let d = damped_modes(&model, &o).unwrap();
    c.holds(
        "N1 a repeated dashpot: the second vector dropped, said, 1 kept",
        d.residual_vectors.len() == 1 && d.basis == 21 && d.notes.iter().any(|n| n.contains("dropped")),
        &format!("{} kept, notes {:?}", d.residual_vectors.len(), d.notes),
    );
    c.finish();
}
