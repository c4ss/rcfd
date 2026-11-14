//! F-552 (ADR 0474): the residual vector of a part's interior load and the kept modes chosen by a cutoff, options of the
//! reduction of ADR 0469 (`rcfd_fem::cms`). The checks of ADR 0474 with their registered limits; every measured number is
//! printed.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{self, condense, reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mass::consistent_with;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{symmetric_eigen, ResidualTest};
use std::collections::HashMap;

/// The registered cutoff (ADR 0474): four times the top of ADR 0469's H1 range.
const CUTOFF: f64 = 32964.0;
const LOSS: f64 = 0.02;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured <= limit;
        println!(
            "{id}: {measured:.3e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} > {limit:e}"));
        }
    }
    fn ge(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured >= limit;
        println!(
            "{id}: {measured:.3e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, detail: &str) {
        println!("{id}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{id}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "{:?}", self.failed);
    }
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn add_point_set(mesh: &mut SolidMesh, name: &str, p: [f64; 3]) {
    let s: Vec<usize> = (0..mesh.nodes().len()).filter(|&i| mesh.nodes()[i] == p).collect();
    assert_eq!(s.len(), 1, "one node at {p:?}");
    mesh.add_node_set(name, s).unwrap();
}

/// The small model S of ADR 0469 with the node set `mid`.
fn small() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mid: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&i| (mesh.nodes()[i][0] - 0.2).abs() < 1e-12)
        .collect();
    mesh.add_node_set("mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// The full bar F with the corner load.
fn full_bar() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m.add_load(Load::nodal_force("corner", [1.0, 1.0, 1.0])).unwrap();
    m
}

/// Part A: [0, 0.5], clamped on xmin, interface xmax, no load.
fn part_a_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m
}

/// Part B: [0.5, 1], free, interface xmin, the corner load.
fn part_b_model() -> SolidModel {
    let mut mesh = hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_load(Load::nodal_force("corner", [1.0, 1.0, 1.0])).unwrap();
    m
}

/// "The option" of ADR 0474: the registered cutoff and the residual vectors.
fn option() -> CmsOptions {
    CmsOptions::new(0).cutoff(CUTOFF).residual_vectors(true).threads(1)
}

/// The cutoff alone.
fn cutoff_only() -> CmsOptions {
    CmsOptions::new(0).cutoff(CUTOFF).threads(1)
}

/// Five modes with the residual vectors.
fn m5_vectors() -> CmsOptions {
    CmsOptions::new(5).residual_vectors(true).threads(1)
}

fn m5() -> CmsOptions {
    CmsOptions::new(5).threads(1)
}

fn bar_parts(o: &CmsOptions) -> (ReducedPart, ReducedPart) {
    let a = reduce(&part_a_model(), "xmax", o).unwrap();
    let b = reduce(&part_b_model(), "xmin", o).unwrap();
    (a, b)
}

/// The full bar's twenty lowest omegas (rad/s).
fn full_bar_omegas() -> Vec<f64> {
    let r = modal_analysis(
        &full_bar(),
        &ModalOptions::new(20)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    r.eigenvalues.iter().map(|l| l.sqrt()).collect()
}

fn omegas_of(values: &[f64]) -> Vec<f64> {
    values.iter().map(|l| l.signum() * l.abs().sqrt()).collect()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |s, x| s.max(x.abs()))
}

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The bar: F, the part models and, for every equation of F, the part and its equation there.
struct Bar {
    full: SolidModel,
    am: SolidModel,
    bm: SolidModel,
    map: Vec<(usize, usize, usize)>,
}

impl Bar {
    fn new() -> Bar {
        let full = full_bar();
        let am = part_a_model();
        let bm = part_b_model();
        let index = |mesh: &SolidMesh| -> HashMap<[u64; 3], usize> {
            mesh.nodes()
                .iter()
                .enumerate()
                .map(|(i, p)| ([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()], i))
                .collect()
        };
        let (ia, ib) = (index(am.mesh()), index(bm.mesh()));
        let (fd, ad, bd) = (full.dof_map(), am.dof_map(), bm.dof_map());
        let mut map = Vec::new();
        for (i, p) in full.mesh().nodes().iter().enumerate() {
            let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            let (part, node) = if p[0] <= 0.5 { (0, ia[&key]) } else { (1, ib[&key]) };
            for comp in 0..3 {
                let eq_p = if part == 0 {
                    ad.equation(node, comp)
                } else {
                    bd.equation(node, comp)
                }
                .unwrap();
                map.push((fd.equation(i, comp).unwrap(), part, eq_p));
            }
        }
        Bar { full, am, bm, map }
    }

    /// The mesh displacements of both parts from an assembly vector.
    fn expand(&self, asm: &Assembly, x: &[f64]) -> [Vec<f64>; 2] {
        [
            asm.parts()[0].expand(&self.am, &asm.part_coordinates(0, x)).unwrap(),
            asm.parts()[1].expand(&self.bm, &asm.part_coordinates(1, x)).unwrap(),
        ]
    }

    /// The relative error of the assembly's harmonic response against F at each omega (loss factor 0.02, the corner load).
    fn harmonic_errors(&self, asm: &Assembly, omegas: &[f64]) -> Vec<f64> {
        let dh = DirectHarmonic::new(
            &self.full,
            &DirectHarmonicOptions::new()
                .loss_factor("all", LossFactor::Constant(LOSS))
                .threads(1),
        )
        .unwrap();
        let load = HarmonicLoad::from_model(&self.full).unwrap();
        omegas
            .iter()
            .map(|&w| {
                let r = dh.response(&load, w).unwrap();
                let (xr, xi) = asm.harmonic(w, LOSS).unwrap();
                let (ur, ui) = (self.expand(asm, &xr), self.expand(asm, &xi));
                let (mut num, mut den) = (0.0, 0.0);
                for &(ef, part, ep) in &self.map {
                    let (dr, di) = (ur[part][ep] - r.re[ef], ui[part][ep] - r.im[ef]);
                    num += dr * dr + di * di;
                    den += r.re[ef] * r.re[ef] + r.im[ef] * r.im[ef];
                }
                (num / den).sqrt()
            })
            .collect()
    }

    /// The static solution of the assembly (`harmonic(0, 0)`) against `solve_static` of F, every equation of F.
    fn statics_error(&self, asm: &Assembly) -> f64 {
        let uf = solve_static(&self.full, &StaticOptions::default()).unwrap();
        let uf = uf.displacement_values();
        let (x, _) = asm.harmonic(0.0, 0.0).unwrap();
        let u = self.expand(asm, &x);
        let d = self
            .map
            .iter()
            .fold(0.0f64, |m, &(ef, part, ep)| m.max((u[part][ep] - uf[ef]).abs()));
        d / max_abs(uf)
    }
}

fn assembly(o: &CmsOptions) -> Assembly {
    let (a, b) = bar_parts(o);
    Assembly::new(vec![a, b]).unwrap()
}

/// The largest error on `omegas`, where, and the first omega above 1e-2.
fn worst(omegas: &[f64], e: &[f64]) -> (f64, f64, f64) {
    let mut w = (0.0f64, 0.0);
    for (x, v) in omegas.iter().zip(e) {
        if *v > w.0 {
            w = (*v, *x);
        }
    }
    let first = omegas
        .iter()
        .zip(e)
        .filter(|(_, v)| **v > 1e-2)
        .map(|(x, _)| *x)
        .fold(f64::INFINITY, f64::min);
    (w.0, w.1, first)
}

/// `n` points `j top / n` and the frequencies of F below `top`.
fn grid(top: f64, n: usize, full: &[f64]) -> Vec<f64> {
    let mut ws: Vec<f64> = (1..=n).map(|j| j as f64 * top / n as f64).collect();
    ws.extend(full.iter().copied().filter(|&w| w < top));
    ws
}

// ------------------------------------------------------------------------------------------------------------- defaults

/// D3 (a5): the option asked but inert.
#[test]
fn d3_option_inert() {
    let mut c = Checks::new();
    let am = part_a_model();
    let plain = reduce(&am, "xmax", &m5()).unwrap();
    let asked = reduce(&am, "xmax", &m5_vectors()).unwrap();
    c.holds(
        "D3 A with residual_vectors(true) (no interior load) equals A without",
        asked == plain && same_bits(asked.stiffness(), plain.stiffness()) && same_bits(asked.mass(), plain.mass()),
        &format!("{} vectors", asked.residual_vectors()),
    );
    c.holds("D3 A writes the same bytes", asked.to_text() == plain.to_text(), "");
    let mut explicit = CmsOptions::new(5).threads(1);
    explicit.residual_vectors = false;
    explicit.cutoff = None;
    let e = reduce(&am, "xmax", &explicit).unwrap();
    c.holds(
        "D3 modes 5 with the options set off equals CmsOptions::new(5)",
        e == plain && e.to_text() == plain.to_text(),
        "",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- the vector

/// O1, O2 (a1), SP2 (a1), T2 (design): the residual vector of B.
#[test]
fn o_sp2_t2_the_residual_vector() {
    let mut c = Checks::new();
    let bm = part_b_model();
    let mass = consistent_with(&bm, Some(1)).unwrap();
    let dofs = bm.dof_map();
    for (name, o) in [("option", option()), ("m = 5 with the vectors", m5_vectors())] {
        let p = reduce(&bm, "xmin", &o).unwrap();
        let (nb, nm, np, nr) = (p.n_boundary(), p.modes(), p.residual_vectors(), p.n_reduced());
        c.holds(&format!("O1 {name}: one vector kept"), np == 1, &format!("{np}"));
        if np != 1 {
            continue;
        }
        let o_r = nb + nm;
        let mq = (0..nm).fold(0.0f64, |m, q| m.max(p.mass()[(nb + q) * nr + o_r].abs()));
        c.le(&format!("O1 {name}: largest |M_qR|"), mq, 1e-10);
        c.le(
            &format!("O1 {name}: |M_RR - 1|"),
            (p.mass()[o_r * nr + o_r] - 1.0).abs(),
            1e-10,
        );
        let kq = (0..nm).fold(0.0f64, |m, q| m.max(p.stiffness()[(nb + q) * nr + o_r].abs()));
        println!(
            "record {name}: |K_qR| over the largest K_r entry {:.3e}; K_RR {:.6e}",
            kq / max_abs(p.stiffness()),
            p.stiffness()[o_r * nr + o_r]
        );
        // O2: R against s - Phi Phi^T M_ii s normalised, recomputed here (B has no constraint: q is the equation).
        let n = dofs.n_equations();
        let mut is_b = vec![false; n];
        for b in p.boundary() {
            is_b[dofs.equation(b.node, b.component).unwrap()] = true;
        }
        let interior: Vec<usize> = (0..n).filter(|&e| !is_b[e]).collect();
        assert_eq!(interior.len(), p.n_interior());
        let full = |v: &[f64]| -> Vec<f64> {
            let mut x = vec![0.0; n];
            for (a, &e) in interior.iter().enumerate() {
                x[e] = v[a];
            }
            x
        };
        let mdot = |x: &[f64], y: &[f64]| -> f64 {
            let mut my = vec![0.0; n];
            mass.matvec(y, &mut my);
            x.iter().zip(&my).map(|(a, b)| a * b).sum()
        };
        let mut r = full(p.static_interior());
        let ni = p.n_interior();
        for k in 0..nm {
            let phik: Vec<f64> = (0..ni).map(|a| p.phi()[a * nm + k]).collect();
            let fk = full(&phik);
            let a = mdot(&fk, &r);
            for (x, y) in r.iter_mut().zip(&fk) {
                *x -= a * y;
            }
        }
        let norm = mdot(&r, &r).sqrt();
        let got = full(&(0..ni).map(|a| p.residual()[a]).collect::<Vec<_>>());
        let d = r.iter().zip(&got).fold(0.0f64, |m, (x, y)| m.max((x / norm - y).abs()));
        c.le(
            &format!("O2 {name}: R against the recomputed vector"),
            d / max_abs(&got),
            1e-8,
        );
        if name == "option" {
            let n = p.n_reduced();
            c.le("SP2 K_r asymmetry", cms::relative_asymmetry(p.stiffness(), n), 1e-13);
            c.le("SP2 M_r asymmetry", cms::relative_asymmetry(p.mass(), n), 1e-13);
            c.le("SP2 asymmetry before symmetrisation", p.asymmetry(), 1e-10);
            let e = symmetric_eigen(p.mass(), n).unwrap();
            c.ge(
                "SP2 lambda_min(M_r)/lambda_max(M_r)",
                e.values[0] / e.values[n - 1],
                1e-12,
            );
            let four = reduce(&bm, "xmin", &option().threads(4)).unwrap();
            let same = same_bits(p.stiffness(), four.stiffness())
                && same_bits(p.mass(), four.mass())
                && same_bits(p.load(), four.load())
                && same_bits(p.psi(), four.psi())
                && same_bits(p.phi(), four.phi())
                && same_bits(p.residual(), four.residual());
            c.holds(
                "T2 threads 1 and 4: K_r, M_r, load, Psi, Phi, R bit-identical",
                same,
                "",
            );
        }
    }
    c.finish();
}

/// RV3 (a1, drop): every mode kept, the vector is in their span and dropped.
#[test]
fn rv3_vector_dropped_when_every_mode_is_kept() {
    let mut c = Checks::new();
    let s = small();
    let ni = condense(&s, "xmax", Some(1)).unwrap().n_interior();
    let plain = reduce(&s, "xmax", &CmsOptions::new(ni).threads(1)).unwrap();
    let asked = reduce(&s, "xmax", &CmsOptions::new(ni).residual_vectors(true).threads(1)).unwrap();
    c.holds(
        "RV3 vector dropped",
        asked.residual_vectors() == 0,
        &format!("n_i {ni}, {} vectors", asked.residual_vectors()),
    );
    c.holds("RV3 part equal to the part without the option", asked == plain, "");
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- the cutoff

/// CO1, C2, X3 (a2).
#[test]
fn co1_c2_x3_cutoff() {
    let mut c = Checks::new();
    let c2 = CUTOFF * CUTOFF;
    for (name, model, iface) in [("A", part_a_model(), "xmax"), ("B", part_b_model(), "xmin")] {
        let ni = condense(&model, iface, Some(1)).unwrap().n_interior();
        let all = reduce(&model, iface, &CmsOptions::new(ni).threads(1)).unwrap();
        let all = all.fixed_interface_eigenvalues();
        let count = all.iter().filter(|&&l| l < c2).count();
        let p = reduce(&model, iface, &option()).unwrap();
        let m = p.modes();
        println!("CO1 {name}: {m} modes kept, {count} below the cutoff of {ni}");
        c.holds(&format!("CO1 {name} count"), m == count, &format!("{m} and {count}"));
        c.holds(
            &format!("CO1 {name} kept eigenvalues equal in bits"),
            same_bits(p.fixed_interface_eigenvalues(), &all[..m.min(all.len())]),
            "",
        );
        c.holds(
            &format!("CO1 {name} omitted equal in bits"),
            p.omitted_eigenvalue().map(f64::to_bits) == all.get(m).map(|v| v.to_bits()),
            &format!("{:?}", p.omitted_eigenvalue().map(f64::sqrt)),
        );
        let last_below = m == 0 || p.fixed_interface_eigenvalues()[m - 1] < c2;
        let omitted_not_below = p.omitted_eigenvalue().is_none_or(|l| l >= c2);
        c.holds(
            &format!("CO1 {name} last kept below, omitted not below the cutoff"),
            last_below && omitted_not_below,
            "",
        );
    }
    let s = small();
    for w in [3.0e4, 6.0e4] {
        let o = CmsOptions::new(0).cutoff(w).threads(1);
        let d = reduce(&s, "xmax", &o).unwrap();
        let p = reduce(&s, "xmax", &o.clone().dense_limit(0)).unwrap();
        println!("C2 cutoff {w:e}: dense {} modes, subspace {}", d.modes(), p.modes());
        c.holds(&format!("C2 cutoff {w:e} count"), d.modes() == p.modes(), "");
        if d.modes() == p.modes() {
            let mut worst = 0.0f64;
            for (x, y) in d
                .fixed_interface_eigenvalues()
                .iter()
                .zip(p.fixed_interface_eigenvalues())
            {
                worst = worst.max((x / y - 1.0).abs());
            }
            match (d.omitted_eigenvalue(), p.omitted_eigenvalue()) {
                (Some(x), Some(y)) => worst = worst.max((x / y - 1.0).abs()),
                (None, None) => {}
                _ => worst = f64::INFINITY,
            }
            c.le(&format!("C2 cutoff {w:e} eigenvalues"), worst, 1e-9);
        }
    }
    for w in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let r = reduce(&s, "xmax", &CmsOptions::new(0).cutoff(w).threads(1));
        c.holds(
            &format!("X3 cutoff {w} refused"),
            matches!(r, Err(FemError::InvalidValue { key: "cutoff", .. })),
            &format!("{:?}", r.err()),
        );
    }
    let both = reduce(&s, "xmax", &CmsOptions::new(5).cutoff(3.0e4).threads(1));
    c.holds(
        "X3 a mode count with a cutoff refused",
        matches!(both, Err(FemError::InvalidValue { key: "cutoff", .. })),
        &format!("{:?}", both.err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- statics

/// ST5, ST6, ST7 (a4) and the negative control NC1.
#[test]
fn st_statics_of_an_interior_load() {
    let mut c = Checks::new();
    let bar = Bar::new();
    c.le("ST5 bar, the option", bar.statics_error(&assembly(&option())), 1e-10);
    c.le(
        "ST6 bar, m = 5 with the vectors",
        bar.statics_error(&assembly(&m5_vectors())),
        1e-10,
    );
    c.ge(
        "NC1 bar, the cutoff alone",
        bar.statics_error(&assembly(&cutoff_only())),
        1e-6,
    );
    let s = small();
    let uf = solve_static(&s, &StaticOptions::default()).unwrap();
    let uf = uf.displacement_values().to_vec();
    let one = |o: &CmsOptions, iface: &str| -> f64 {
        let p = reduce(&s, iface, o).unwrap();
        let asm = Assembly::new(vec![p]).unwrap();
        let (x, _) = asm.harmonic(0.0, 0.0).unwrap();
        let u = asm.parts()[0].expand(&s, &x).unwrap();
        u.iter().zip(&uf).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(&uf)
    };
    for iface in ["xmax", "mid"] {
        for m in [0, 5] {
            let e = one(&CmsOptions::new(m).residual_vectors(true).threads(1), iface);
            c.le(&format!("ST7 S {iface}, m = {m}, with the vectors"), e, 1e-10);
        }
    }
    c.ge("NC1 S mid, m = 5, without the vectors", one(&m5(), "mid"), 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ harmonic

fn h1_run(id: &str, o: &CmsOptions) -> f64 {
    let bar = Bar::new();
    let asm = assembly(o);
    let half = 0.5
        * asm.parts()[0]
            .omitted_eigenvalue()
            .unwrap()
            .sqrt()
            .min(asm.parts()[1].omitted_eigenvalue().unwrap().sqrt());
    println!(
        "{id}: omega_half = {half:.3} rad/s; modes A {}, B {}",
        asm.parts()[0].modes(),
        asm.parts()[1].modes()
    );
    let full = full_bar_omegas();
    let ws = grid(half, 50, &full);
    let e = bar.harmonic_errors(&asm, &ws);
    for (w, x) in ws.iter().zip(&e) {
        println!("  omega {w:.2}: {x:.3e}");
    }
    let (big, at, first) = worst(&ws, &e);
    println!("{id}: largest at omega {at:.2}; first omega above 1e-2: {first:.2}");
    big
}

/// H1R (a3): the H1 set-up with the option (predicted miss, ADR 0474). Missed at its registered run (commit 7204c96d):
/// largest 4.937e-1 at omega 15106.88 (omega_half 16785.42), 4.389e-1 and 2.795e-1 at the resonances 15026.69 and 14830.31,
/// 3.535e-1 at the axial resonance 8178.99, first point above 1e-2 at the resonance 2921.42 (1.773e-2); the limit is kept.
#[test]
#[ignore = "known miss of ADR 0474 H1R (predicted by its arithmetic item 2): 4.937e-1 > 1e-2; at a resonance the error is 2 delta / eta and the assembly's frequency errors delta near omega_half are 4e-3 to 5e-3"]
fn h1r_harmonic_to_half_the_lowest_omitted_with_the_option_known_miss() {
    let big = h1_run("H1R", &option());
    let mut c = Checks::new();
    c.le("H1R largest relative error", big, 1e-2);
    c.finish();
}

/// H1R5 (a3): the H1 set-up of ADR 0469 (five modes) with the vectors (predicted miss, ADR 0474). Missed at its registered
/// run (commit 7204c96d): largest 1.038 at omega 8241.00 (omega_half 8241.002), 9.353e-1 at the axial resonance 8178.99,
/// first point above 1e-2 at 2801.94 rad/s (1977.84 without the vectors, ADR 0469); the limit is kept.
#[test]
#[ignore = "known miss of ADR 0474 H1R5 (predicted by its arithmetic item 3): 1.038 > 1e-2; no axial fixed-interface mode is kept (axial frequency 8.07e-2 high) and the response error at a resonance is 2 delta / eta"]
fn h1r5_harmonic_to_half_the_lowest_omitted_five_modes_with_the_vectors_known_miss() {
    let big = h1_run("H1R5", &m5_vectors());
    let mut c = Checks::new();
    c.le("H1R5 largest relative error", big, 1e-2);
    c.finish();
}

/// HB1, HB2 (a3, band) and HR (a1, a3).
#[test]
fn hb_hr_harmonic_bands() {
    let mut c = Checks::new();
    let bar = Bar::new();
    let full = full_bar_omegas();
    let w2000 = grid(2000.0, 50, &full);
    let w1000 = grid(1000.0, 30, &full);
    let largest = |o: &CmsOptions, ws: &[f64]| -> f64 {
        let e = bar.harmonic_errors(&assembly(o), ws);
        worst(ws, &e).0
    };
    c.le("HB1 the option, to 2000 rad/s", largest(&option(), &w2000), 1e-2);
    c.le(
        "HB2 m = 5 with the vectors, to 2000 rad/s",
        largest(&m5_vectors(), &w2000),
        1e-2,
    );
    println!(
        "record: m = 5 without the vectors, to 2000 rad/s: {:.3e}",
        largest(&m5(), &w2000)
    );
    let (with, without) = (largest(&option(), &w1000), largest(&cutoff_only(), &w1000));
    println!("HR cutoff: to 1000 rad/s with {with:.3e}, without {without:.3e}");
    c.le("HR cutoff, with over without", with / without, 0.5);
    let (with, without) = (largest(&m5_vectors(), &w1000), largest(&m5(), &w1000));
    println!("HR m = 5: to 1000 rad/s with {with:.3e}, without {without:.3e}");
    c.le("HR m = 5, with over without", with / without, 0.5);
    let w2800 = grid(2800.0, 50, &full);
    println!(
        "record: to 2800 rad/s, the option {:.3e}, the cutoff alone {:.3e}",
        largest(&option(), &w2800),
        largest(&cutoff_only(), &w2800)
    );
    c.finish();
}

/// RB (a1): the Ritz bounds of the first eight frequencies.
#[test]
fn rb_ritz_bounds() {
    let mut c = Checks::new();
    let full = full_bar_omegas();
    let with = omegas_of(&assembly(&option()).eigen().unwrap().values);
    let without = omegas_of(&assembly(&cutoff_only()).eigen().unwrap().values);
    let mut below = f64::INFINITY;
    let mut above = f64::NEG_INFINITY;
    for k in 0..8 {
        println!(
            "  f{}: with {:.3e}, without {:.3e} (relative to F)",
            k + 1,
            with[k] / full[k] - 1.0,
            without[k] / full[k] - 1.0
        );
        below = below.min(with[k] / full[k] - 1.0);
        above = above.max(with[k] / without[k] - 1.0);
    }
    c.ge("RB smallest f_with / f_full - 1", below, -1e-9);
    c.le("RB largest f_with / f_without - 1", above, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ file

/// F3, F4 (format).
#[test]
fn f3_f4_file() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sa_cms_residual");
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Checks::new();
    let b = reduce(&part_b_model(), "xmin", &option()).unwrap();
    let text = b.to_text();
    c.holds(
        "F3 header 2",
        text.starts_with(&format!("{}\n", cms::FILE_HEADER_2)),
        &text[..text.find('\n').unwrap()],
    );
    let path = dir.join("B.part");
    b.write(&path).unwrap();
    let q = ReducedPart::read(&path).unwrap();
    let floats = same_bits(b.stiffness(), q.stiffness())
        && same_bits(b.mass(), q.mass())
        && same_bits(b.load(), q.load())
        && same_bits(b.static_interior(), q.static_interior())
        && same_bits(b.fixed_interface_eigenvalues(), q.fixed_interface_eigenvalues())
        && same_bits(b.psi(), q.psi())
        && same_bits(b.phi(), q.phi())
        && same_bits(b.residual(), q.residual())
        && b.omitted_eigenvalue().map(f64::to_bits) == q.omitted_eigenvalue().map(f64::to_bits)
        && b.asymmetry().to_bits() == q.asymmetry().to_bits();
    c.holds("F3 read back: every float in bits", floats, "");
    c.holds(
        "F3 read back: equal",
        b == q && q.residual_vectors() == 1,
        &format!("{}", q.residual_vectors()),
    );
    let path2 = dir.join("B-again.part");
    q.write(&path2).unwrap();
    c.holds(
        "F3 written again byte-identical",
        std::fs::read(&path).unwrap() == std::fs::read(&path2).unwrap(),
        "",
    );
    let cut = ReducedPart::from_text(&text[..text.len() / 2]);
    c.holds("F3 half file refused", cut.is_err(), &format!("{:?}", cut.err()));
    let one = text.replacen(cms::FILE_HEADER_2, cms::FILE_HEADER, 1);
    let hdr = ReducedPart::from_text(&one);
    c.holds(
        "F3 header 2 replaced by header 1 refused",
        hdr.is_err(),
        &format!("{:?}", hdr.err()),
    );
    let a = reduce(&part_a_model(), "xmax", &m5()).unwrap();
    c.holds(
        "F4 A (m = 5, no option): header 1",
        a.to_text().starts_with(&format!("{}\n", cms::FILE_HEADER)),
        "",
    );
    c.finish();
}
