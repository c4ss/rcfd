//! F-559 (ADR 0511): residual vectors of load patterns given by the caller and the harmonic acceptance of the bar at
//! resonance, options of the reduction of ADR 0474 (`rcfd_fem::cms`). The checks of ADR 0511 with their registered limits;
//! every measured number is printed. Every aggregated error is required finite before it is compared.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{self, reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent_with;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{symmetric_eigen, ResidualTest};
use std::collections::HashMap;

/// The registered cutoff (ADR 0474, kept by ADR 0511).
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
        let ok = measured.is_finite() && measured <= limit;
        println!(
            "{id}: {measured:.3e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} > {limit:e}"));
        }
    }
    fn ge(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured.is_finite() && measured >= limit;
        println!(
            "{id}: {measured:.3e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} < {limit:e}"));
        }
    }
    fn lt(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured.is_finite() && measured < limit;
        println!(
            "{id}: {measured:.3e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} >= {limit:e}"));
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

/// Asserts that every entry is finite (an `f64::max` over NaN would hide the failure) and returns the slice.
fn finite<'a>(what: &str, v: &'a [f64]) -> &'a [f64] {
    assert!(v.iter().all(|x| x.is_finite()), "{what}: a non-finite entry");
    v
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

/// The small model S of ADR 0469 with the node set `mid`; `loads` selects the traction and the gravity.
fn small_with(traction: bool, gravity: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mid: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&i| (mesh.nodes()[i][0] - 0.2).abs() < 1e-12)
        .collect();
    mesh.add_node_set("mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    if traction {
        m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    }
    if gravity {
        m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    }
    m
}

/// The full bar F with the corner force `force`.
fn full_with(force: [f64; 3]) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m.add_load(Load::nodal_force("corner", force)).unwrap();
    m
}

fn full_bar() -> SolidModel {
    full_with([1.0, 1.0, 1.0])
}

fn part_a_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m
}

/// Part B with the corner force `force`.
fn part_b_with(force: [f64; 3]) -> SolidModel {
    let mut mesh = hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_load(Load::nodal_force("corner", force)).unwrap();
    m
}

fn part_b_model() -> SolidModel {
    part_b_with([1.0, 1.0, 1.0])
}

fn unit(c: usize) -> [f64; 3] {
    let mut f = [0.0; 3];
    f[c] = 1.0;
    f
}

/// The pattern of component `c` of the corner force on B (a vector over B's mesh equations).
fn pattern_b(c: usize) -> Vec<f64> {
    nodal_load_vector(&part_b_with(unit(c)), &[]).unwrap()
}

fn patterns_b() -> Vec<Vec<f64>> {
    (0..3).map(pattern_b).collect()
}

/// The ADR 0474 option.
fn option_0474() -> CmsOptions {
    CmsOptions::new(0).cutoff(CUTOFF).residual_vectors(true).threads(1)
}

/// The ADR 0511 option on B: the ADR 0474 option and the three component patterns.
fn option_b() -> CmsOptions {
    option_0474().load_patterns(patterns_b())
}

fn cutoff_only() -> CmsOptions {
    CmsOptions::new(0).cutoff(CUTOFF).threads(1)
}

/// A and B with the options `oa`, `ob`.
fn assembly_of(oa: &CmsOptions, ob: &CmsOptions) -> Assembly {
    let a = reduce(&part_a_model(), "xmax", oa).unwrap();
    let b = reduce(&part_b_model(), "xmin", ob).unwrap();
    Assembly::new(vec![a, b]).unwrap()
}

fn asm_patterns() -> Assembly {
    assembly_of(&option_0474(), &option_b())
}

fn asm_0474() -> Assembly {
    assembly_of(&option_0474(), &option_0474())
}

fn asm_cutoff() -> Assembly {
    assembly_of(&cutoff_only(), &cutoff_only())
}

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

/// Half the lowest omitted fixed-interface frequency of the parts of `asm`.
fn half_of(asm: &Assembly) -> f64 {
    0.5 * asm
        .parts()
        .iter()
        .map(|p| p.omitted_eigenvalue().unwrap().sqrt())
        .fold(f64::INFINITY, f64::min)
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
                let e = (num / den).sqrt();
                assert!(e.is_finite(), "harmonic error at omega {w}: {e}");
                e
            })
            .collect()
    }

    /// The static error of component `c` of the corner force: the assembly load of the pattern `pattern` of B reduced by
    /// `ReducedPart::reduce_load` against `solve_static` of F with only that component, every equation of F.
    fn component_statics(&self, asm: &Assembly, c: usize, pattern: &[f64]) -> f64 {
        let uf = solve_static(&full_with(unit(c)), &StaticOptions::default()).unwrap();
        let uf = finite("F statics", uf.displacement_values()).to_vec();
        let lb = asm.parts()[1].reduce_load(&self.bm, pattern).unwrap();
        let load = asm.load_from(&[Vec::new(), lb]).unwrap();
        let (x, _) = asm.harmonic_with_load(0.0, 0.0, &load).unwrap();
        finite("assembly statics", &x);
        let u = self.expand(asm, &x);
        let d = self
            .map
            .iter()
            .fold(0.0f64, |m, &(ef, part, ep)| m.max((u[part][ep] - uf[ef]).abs()));
        d / max_abs(&uf)
    }
}

// ------------------------------------------------------------------------------------------------------------- defaults

/// D5 (b7): the option asked but empty; HW1 (b7): `harmonic` is `harmonic_with_load` of the assembly's own load.
#[test]
fn d5_hw1_defaults() {
    let mut c = Checks::new();
    let bm = part_b_model();
    for (name, o) in [
        ("m = 5", CmsOptions::new(5).threads(1)),
        ("the ADR 0474 option", option_0474()),
    ] {
        let plain = reduce(&bm, "xmin", &o).unwrap();
        let empty = reduce(&bm, "xmin", &o.clone().load_patterns(vec![])).unwrap();
        c.holds(
            &format!("D5 {name}: load_patterns(vec![]) equals the part without"),
            empty == plain
                && same_bits(empty.stiffness(), plain.stiffness())
                && same_bits(empty.mass(), plain.mass())
                && empty.to_text() == plain.to_text(),
            "",
        );
    }
    let asm = asm_0474();
    let mut same = true;
    for w in [0.0, 1000.0, 5000.0] {
        let a = asm.harmonic(w, LOSS).unwrap();
        let b = asm.harmonic_with_load(w, LOSS, asm.load()).unwrap();
        same &= same_bits(&a.0, &b.0) && same_bits(&a.1, &b.1);
    }
    c.holds(
        "HW1 harmonic_with_load(own load) equals harmonic in bits at 0, 1000, 5000",
        same,
        "",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- the vectors

/// The interior of a part model without constraints (B): the equation of each interior unknown, in the part's order.
fn interior_equations(model: &SolidModel, p: &ReducedPart) -> Vec<usize> {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut is_b = vec![false; n];
    for b in p.boundary() {
        is_b[dofs.equation(b.node, b.component).unwrap()] = true;
    }
    let interior: Vec<usize> = (0..n).filter(|&e| !is_b[e]).collect();
    assert_eq!(interior.len(), p.n_interior());
    interior
}

/// P1, P2, P3 (b1) and SP3, TH (design).
#[test]
fn p_vectors_of_the_patterns() {
    let mut c = Checks::new();
    let bm = part_b_model();
    let pats = patterns_b();
    let with_option = reduce(&bm, "xmin", &option_b()).unwrap();
    let patterns_only = reduce(&bm, "xmin", &cutoff_only().load_patterns(pats.clone())).unwrap();
    let twice = reduce(
        &bm,
        "xmin",
        &cutoff_only()
            .load_pattern(pats[0].clone())
            .load_pattern(pats[0].clone()),
    )
    .unwrap();
    let zero = reduce(
        &bm,
        "xmin",
        &cutoff_only().load_pattern(vec![0.0; bm.dof_map().n_equations()]),
    )
    .unwrap();
    let plain = reduce(&bm, "xmin", &cutoff_only()).unwrap();
    c.holds(
        "P1 the option and three patterns: three vectors",
        with_option.residual_vectors() == 3,
        &format!("{}", with_option.residual_vectors()),
    );
    c.holds(
        "P1 three patterns alone: three vectors",
        patterns_only.residual_vectors() == 3,
        &format!("{}", patterns_only.residual_vectors()),
    );
    c.holds(
        "P1 one pattern given twice: one vector",
        twice.residual_vectors() == 1,
        &format!("{}", twice.residual_vectors()),
    );
    c.holds(
        "P1 a zero pattern: no vector and the part equal to the part without",
        zero.residual_vectors() == 0 && zero == plain,
        &format!("{}", zero.residual_vectors()),
    );
    // P2: M_qR, M_RR - I.
    for (name, p) in [("option + patterns", &with_option), ("patterns alone", &patterns_only)] {
        let (nb, nm, np, nr) = (p.n_boundary(), p.modes(), p.residual_vectors(), p.n_reduced());
        let o = nb + nm;
        let mut mq = 0.0f64;
        let mut mrr = 0.0f64;
        for j in 0..np {
            for q in 0..nm {
                mq = mq.max(p.mass()[(nb + q) * nr + o + j].abs());
            }
            for k in 0..np {
                let id = if j == k { 1.0 } else { 0.0 };
                mrr = mrr.max((p.mass()[(o + j) * nr + o + k] - id).abs());
            }
        }
        c.le(&format!("P2 {name}: largest |M_qR|"), mq, 1e-10);
        c.le(&format!("P2 {name}: largest |M_RR - I|"), mrr, 1e-10);
        let kq = (0..nm).fold(0.0f64, |m, q| {
            (0..np).fold(m, |m, j| m.max(p.stiffness()[(nb + q) * nr + o + j].abs()))
        });
        println!(
            "record {name}: |K_qR| over the largest K_r entry {:.3e}",
            kq / max_abs(p.stiffness())
        );
    }
    // P3: each vector recomputed from the pattern's static response and the earlier vectors.
    {
        let p = &patterns_only;
        let mass = consistent_with(&bm, Some(1)).unwrap();
        let n = bm.dof_map().n_equations();
        let interior = interior_equations(&bm, p);
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
        let (ni, nm, np) = (p.n_interior(), p.modes(), p.residual_vectors());
        let mut basis: Vec<Vec<f64>> = (0..nm)
            .map(|k| full(&(0..ni).map(|a| p.phi()[a * nm + k]).collect::<Vec<_>>()))
            .collect();
        let mut worst = 0.0f64;
        for j in 0..np {
            let single = reduce(&part_b_with(unit(j)), "xmin", &CmsOptions::new(0).threads(1)).unwrap();
            let mut r = full(single.static_interior());
            for _pass in 0..2 {
                for q in &basis {
                    let a = mdot(q, &r);
                    for (x, y) in r.iter_mut().zip(q) {
                        *x -= a * y;
                    }
                }
            }
            let norm = mdot(&r, &r).sqrt();
            for x in r.iter_mut() {
                *x /= norm;
            }
            let got = full(&(0..ni).map(|a| p.residual()[a * np + j]).collect::<Vec<_>>());
            let d = r.iter().zip(&got).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / max_abs(&got);
            worst = worst.max(d);
            basis.push(r);
        }
        c.le("P3 the vectors against the recomputed ones", worst, 1e-8);
    }
    // SP3, TH on the option with the patterns.
    {
        let p = &with_option;
        let n = p.n_reduced();
        c.le("SP3 K_r asymmetry", cms::relative_asymmetry(p.stiffness(), n), 1e-13);
        c.le("SP3 M_r asymmetry", cms::relative_asymmetry(p.mass(), n), 1e-13);
        c.le("SP3 asymmetry before symmetrisation", p.asymmetry(), 1e-10);
        let e = symmetric_eigen(p.mass(), n).unwrap();
        c.ge(
            "SP3 lambda_min(M_r)/lambda_max(M_r)",
            e.values[0] / e.values[n - 1],
            1e-12,
        );
        let four = reduce(&bm, "xmin", &option_b().threads(4)).unwrap();
        let (l1, l4) = (
            p.reduce_load(&bm, &pats[1]).unwrap(),
            four.reduce_load(&bm, &pats[1]).unwrap(),
        );
        let same = same_bits(p.stiffness(), four.stiffness())
            && same_bits(p.mass(), four.mass())
            && same_bits(p.load(), four.load())
            && same_bits(p.psi(), four.psi())
            && same_bits(p.phi(), four.phi())
            && same_bits(p.residual(), four.residual())
            && same_bits(&l1, &l4);
        c.holds(
            "TH threads 1 and 4: K_r, M_r, load, Psi, Phi, R, reduce_load bit-identical",
            same,
            "",
        );
    }
    c.finish();
}

/// X4 (b1): refusals.
#[test]
fn x4_refusals() {
    let mut c = Checks::new();
    let bm = part_b_model();
    let n = bm.dof_map().n_equations();
    let good = pattern_b(0);
    let mut short = good.clone();
    short.pop();
    let mut long = good.clone();
    long.push(0.0);
    for (name, p) in [("one entry short", short), ("one entry long", long)] {
        let r = reduce(&bm, "xmin", &cutoff_only().load_pattern(p));
        c.holds(
            &format!("X4 {name} refused"),
            matches!(
                r,
                Err(FemError::FieldLength {
                    key: "load pattern",
                    ..
                })
            ),
            &format!("{:?}", r.err()),
        );
    }
    for (name, v) in [("NaN", f64::NAN), ("+inf", f64::INFINITY), ("-inf", f64::NEG_INFINITY)] {
        let mut p = vec![0.0; n];
        p[n / 2] = v;
        let r = reduce(&bm, "xmin", &cutoff_only().load_pattern(p));
        c.holds(
            &format!("X4 {name} refused"),
            matches!(
                r,
                Err(FemError::InvalidValue {
                    key: "load_patterns",
                    ..
                })
            ),
            &format!("{:?}", r.err()),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- statics

/// ST-P, ST-P2, ST-P3, NC-P, HW2 on the bar.
#[test]
fn st_p_statics_of_each_component() {
    let mut c = Checks::new();
    let bar = Bar::new();
    let pats = patterns_b();
    let m5_patterns = CmsOptions::new(5).load_patterns(pats.clone()).threads(1);
    let patterns_only = cutoff_only().load_patterns(pats.clone());
    let cases: [(&str, Assembly); 3] = [
        ("ST-P the option and the patterns", asm_patterns()),
        ("ST-P2 the patterns alone", assembly_of(&cutoff_only(), &patterns_only)),
        (
            "ST-P3 m = 5 and the patterns",
            assembly_of(&CmsOptions::new(5).threads(1), &m5_patterns),
        ),
    ];
    for (name, asm) in &cases {
        for (comp, p) in pats.iter().enumerate() {
            c.le(
                &format!("{name}, component {comp}"),
                bar.component_statics(asm, comp, p),
                1e-10,
            );
        }
    }
    // NC-P: the ADR 0474 option has the vector of the total load only.
    let nc = asm_0474();
    for (comp, p) in pats.iter().enumerate() {
        c.ge(
            &format!("NC-P the ADR 0474 option, component {comp}"),
            bar.component_statics(&nc, comp, p),
            1e-6,
        );
    }
    // HW2: reduce_load of the model's own load vector is load().
    let bm = part_b_model();
    let b = &cases[0].1.parts()[1];
    let own = nodal_load_vector(&bm, &[]).unwrap();
    let rl = b.reduce_load(&bm, &own).unwrap();
    let d = rl.iter().zip(b.load()).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    c.le(
        "HW2 B: reduce_load(own load) against load()",
        d / max_abs(b.load()),
        1e-13,
    );
    let s = small_with(true, true);
    let sp = reduce(&s, "mid", &CmsOptions::new(5).residual_vectors(true).threads(1)).unwrap();
    let own = nodal_load_vector(&s, &[]).unwrap();
    let rl = sp.reduce_load(&s, &own).unwrap();
    let d = rl.iter().zip(sp.load()).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    c.le(
        "HW2 S: reduce_load(own load) against load()",
        d / max_abs(sp.load()),
        1e-13,
    );
    c.finish();
}

/// The static error of the single-part assembly of S, with the part `p` reduced from `s`, under the load of `loaded` (S with
/// only that load) given as the pattern `pattern`.
fn s_statics(p: ReducedPart, s: &SolidModel, loaded: &SolidModel, pattern: &[f64]) -> f64 {
    let uf = solve_static(loaded, &StaticOptions::default()).unwrap();
    let uf = finite("S statics", uf.displacement_values()).to_vec();
    let asm = Assembly::new(vec![p]).unwrap();
    let load = asm
        .load_from(&[asm.parts()[0].reduce_load(s, pattern).unwrap()])
        .unwrap();
    let (x, _) = asm.harmonic_with_load(0.0, 0.0, &load).unwrap();
    finite("S assembly statics", &x);
    let u = asm.parts()[0].expand(s, &x).unwrap();
    u.iter().zip(&uf).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(&uf)
}

/// ST-P4, ST-P5 (b6) and the S part of NC-P.
#[test]
fn st_p4_p5_small_model() {
    let mut c = Checks::new();
    let s = small_with(true, true);
    let traction = small_with(true, false);
    let gravity = small_with(false, true);
    let pt = nodal_load_vector(&traction, &[]).unwrap();
    let pg = nodal_load_vector(&gravity, &[]).unwrap();
    for iface in ["xmax", "mid"] {
        for m in [0usize, 5] {
            let o = CmsOptions::new(m)
                .load_pattern(pg.clone())
                .load_pattern(pt.clone())
                .threads(1);
            for (name, loaded, pat) in [("gravity", &gravity, &pg), ("traction", &traction, &pt)] {
                let p = reduce(&s, iface, &o).unwrap();
                c.le(
                    &format!("ST-P4 S {iface}, m = {m}, {name}"),
                    s_statics(p, &s, loaded, pat),
                    1e-10,
                );
            }
        }
    }
    let o = CmsOptions::new(5)
        .load_pattern(pg.clone())
        .load_pattern(pt.clone())
        .dense_limit(0)
        .threads(1);
    for (name, loaded, pat) in [("gravity", &gravity, &pg), ("traction", &traction, &pt)] {
        let p = reduce(&s, "xmax", &o).unwrap();
        c.le(
            &format!("ST-P5 S xmax, m = 5, subspace path, {name}"),
            s_statics(p, &s, loaded, pat),
            1e-10,
        );
    }
    // NC-P on S: the total load's vector only.
    let o = CmsOptions::new(5).residual_vectors(true).threads(1);
    for (name, loaded, pat) in [("gravity", &gravity, &pg), ("traction", &traction, &pt)] {
        let p = reduce(&s, "mid", &o).unwrap();
        c.ge(
            &format!("NC-P S mid, m = 5, the total load's vector only, {name}"),
            s_statics(p, &s, loaded, pat),
            1e-6,
        );
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- frequencies

/// The ten indices of F below `half`.
fn below(full: &[f64], half: f64) -> Vec<usize> {
    (0..full.len()).filter(|&k| full[k] < half).collect()
}

/// LF (b3), predicted miss: each frequency of the assembly below the bound within 1e-3 of F's. Registered run
/// (cargo test --release -p rcfd-fem --test sa_cms_pattern -- --ignored): see the addendum of ADR 0511.
#[test]
#[ignore = "known miss of ADR 0511 LF (predicted by its arithmetic item 2): the frequency errors of the modes near the bound are 3.5e-3 to 5.0e-3"]
fn lf_each_frequency_below_the_bound_within_1e3_known_miss() {
    let full = full_bar_omegas();
    let asm = asm_patterns();
    let half = half_of(&asm);
    let wr = omegas_of(finite("assembly eigenvalues", &asm.eigen().unwrap().values));
    let idx = below(&full, half);
    let mut worst = 0.0f64;
    for &k in &idx {
        let d = wr[k] / full[k] - 1.0;
        println!("  omega_F {:.1}: delta {d:.3e}", full[k]);
        worst = worst.max(d.abs());
    }
    println!(
        "LF: omega_half {half:.2}, {} frequencies below, largest |delta| {worst:.3e}",
        idx.len()
    );
    let mut c = Checks::new();
    c.le("LF largest |omega_R / omega_F - 1| below the bound", worst, 1e-3);
    c.finish();
}

/// LF (count), LF2 (b3 band), SL1, SL2, SL3, NC-S (b4).
#[test]
fn lf2_sl_frequencies() {
    let mut c = Checks::new();
    let full = full_bar_omegas();
    let with = asm_patterns();
    let half = half_of(&with);
    let w_with = omegas_of(finite("with", &with.eigen().unwrap().values));
    let w_0474 = omegas_of(finite("0474", &asm_0474().eigen().unwrap().values));
    let w_cut = omegas_of(finite("cutoff", &asm_cutoff().eigen().unwrap().values));
    let idx = below(&full, half);
    let count = w_with.iter().filter(|&&w| w < half).count();
    c.holds(
        "LF count: the assembly's frequencies below the bound as many as F's",
        count == idx.len() && idx.len() == 10,
        &format!("{count} and {} (omega_half {half:.2})", idx.len()),
    );
    let mut worst = 0.0f64;
    for &k in idx.iter().filter(|&&k| k < 6 || k == 7) {
        worst = worst.max((w_with[k] / full[k] - 1.0).abs());
    }
    c.le("LF2 frequencies below 9500 rad/s other than the axial one", worst, 1e-3);
    let mut least_cut = f64::NEG_INFINITY;
    let mut least_0474 = f64::NEG_INFINITY;
    for k in 0..8 {
        let (a, b) = (w_with[k] / w_cut[k] - 1.0, w_with[k] / w_0474[k] - 1.0);
        println!(
            "  f{}: with patterns over cutoff only {a:.3e}, over the ADR 0474 option {b:.3e}",
            k + 1
        );
        least_cut = least_cut.max(a);
        least_0474 = least_0474.max(b);
    }
    c.lt(
        "SL1 largest change of the first eight against the cutoff alone",
        least_cut,
        -1e-14,
    );
    c.lt(
        "SL2 largest change of the first eight against the ADR 0474 option",
        least_0474,
        -1e-14,
    );
    let mut lowest = f64::INFINITY;
    for &k in &idx {
        lowest = lowest.min(w_with[k] / full[k] - 1.0);
    }
    c.ge("SL3 smallest omega_with / omega_F - 1 over the ten", lowest, -1e-9);
    // NC-S: the same reduction against itself shows no lowering.
    let again = omegas_of(&asm_cutoff().eigen().unwrap().values);
    let nothing = (0..8).fold(f64::NEG_INFINITY, |m, k| m.max(again[k] / w_cut[k] - 1.0));
    c.holds(
        "NC-S the cutoff-only assembly against itself: no frequency lowered",
        nothing >= -1e-14,
        &format!("largest change {nothing:e}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ harmonic

/// The 100 points `j omega_half / 100` that are farther than `2 eta` from every frequency of F (`abs(omega / omega_k - 1)
/// > 2 eta`).
fn off_resonance_points(half: f64, full: &[f64]) -> Vec<f64> {
    (1..=100)
        .map(|j| j as f64 * half / 100.0)
        .filter(|w| full.iter().all(|k| (w / k - 1.0).abs() > 2.0 * LOSS))
        .collect()
}

/// OFF1 (b2), predicted miss: the off-resonance response to half the lowest omitted fixed-interface frequency within 1 percent.
#[test]
#[ignore = "known miss of ADR 0511 OFF1 (predicted by its arithmetic item 3): the response near the modes 3.5e-3 to 5e-3 high is off by about 0.1"]
fn off1_off_resonance_to_half_the_lowest_omitted_known_miss() {
    let bar = Bar::new();
    let full = full_bar_omegas();
    let asm = asm_patterns();
    let half = half_of(&asm);
    let ws = off_resonance_points(half, &full);
    let e = bar.harmonic_errors(&asm, &ws);
    let mut worst = (0.0f64, 0.0f64);
    for (w, x) in ws.iter().zip(&e) {
        println!("  omega {w:.2}: {x:.3e}");
        if *x > worst.0 {
            worst = (*x, *w);
        }
    }
    let first = ws
        .iter()
        .zip(&e)
        .filter(|(_, x)| **x > 1e-2)
        .map(|(w, _)| *w)
        .fold(f64::INFINITY, f64::min);
    println!(
        "OFF1: {} off-resonance points, omega_half {half:.2}; largest {:.3e} at {:.2}; first above 1e-2 at {first:.2}",
        ws.len(),
        worst.0,
        worst.1
    );
    let mut c = Checks::new();
    c.holds("OFF1 at least 60 points", ws.len() >= 60, &format!("{}", ws.len()));
    c.le("OFF1 largest off-resonance relative error", worst.0, 1e-2);
    c.finish();
}

/// OFF2 (b2, band) and the point count of OFF1.
#[test]
fn off2_band() {
    let bar = Bar::new();
    let full = full_bar_omegas();
    let asm = asm_patterns();
    let half = half_of(&asm);
    let all = off_resonance_points(half, &full);
    let ws: Vec<f64> = all.iter().copied().filter(|&w| w < 4000.0).collect();
    let e = bar.harmonic_errors(&asm, &ws);
    let big = e.iter().fold(0.0f64, |m, x| m.max(*x));
    let mut c = Checks::new();
    c.holds(
        "OFF1 point count: at least 60 off-resonance points",
        all.len() >= 60,
        &format!("{}", all.len()),
    );
    c.holds("OFF2 band has points", ws.len() >= 10, &format!("{}", ws.len()));
    c.le("OFF2 off-resonance points below 4000 rad/s", big, 1e-2);
    // Records: the whole curve of the three reductions, and the first point above 1e-2.
    for (name, a) in [
        ("patterns", &asm),
        ("ADR 0474 option", &asm_0474()),
        ("cutoff only", &asm_cutoff()),
    ] {
        let ew = bar.harmonic_errors(a, &all);
        let first = all
            .iter()
            .zip(&ew)
            .filter(|(_, x)| **x > 1e-2)
            .map(|(w, _)| *w)
            .fold(f64::INFINITY, f64::min);
        println!(
            "record {name}: off-resonance largest {:.3e}, first above 1e-2 at {first:.2}",
            ew.iter().fold(0.0f64, |m, x| m.max(*x))
        );
    }
    c.finish();
}

/// AX1, AX2, AX3 (b5).
#[test]
fn ax_axial_resonance() {
    let bar = Bar::new();
    let full = full_bar_omegas();
    let axial = 0.5 * std::f64::consts::PI * (2.1e11f64 / 7850.0).sqrt();
    let k = (0..full.len())
        .min_by(|&a, &b| (full[a] - axial).abs().partial_cmp(&(full[b] - axial).abs()).unwrap())
        .unwrap();
    println!(
        "axial closed form {axial:.1}; the nearest frequency of F is #{} = {:.1}",
        k + 1,
        full[k]
    );
    let mut c = Checks::new();
    c.holds(
        "AX the nearest frequency of F within 2 percent of the closed form",
        (full[k] / axial - 1.0).abs() <= 0.02,
        &format!("{:.4}", full[k] / axial - 1.0),
    );
    let point = |asm: &Assembly| -> (f64, f64) {
        let wr = omegas_of(&asm.eigen().unwrap().values);
        let delta = wr[k] / full[k] - 1.0;
        let e = bar.harmonic_errors(asm, &[full[k]])[0];
        (e, delta)
    };
    let (e, delta) = point(&asm_patterns());
    println!(
        "AX: error {e:.3e}, delta {delta:.3e}, 2|delta|/eta {:.3e}, 4|delta|/eta {:.3e}",
        2.0 * delta.abs() / LOSS,
        4.0 * delta.abs() / LOSS
    );
    c.le(
        "AX1 error against 4|delta|/eta (error over the bound)",
        e / (4.0 * delta.abs() / LOSS),
        1.0,
    );
    c.le(
        "AX2 record: error against 2|delta|/eta (error over the bound)",
        e / (2.0 * delta.abs() / LOSS),
        1.0,
    );
    let (e0, d0) = point(&asm_cutoff());
    println!("AX3 cutoff alone: error {e0:.3e}, delta {d0:.3e}");
    c.ge("AX3 the cutoff-only axial delta", d0.abs(), 1e-3);
    c.ge("AX3 the cutoff-only axial error", e0, 1e-1);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ file

/// FP (design): part files with the vectors of patterns.
#[test]
fn fp_file() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sa_cms_pattern");
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Checks::new();
    let bm = part_b_model();
    let b = reduce(&bm, "xmin", &option_b()).unwrap();
    let text = b.to_text();
    c.holds(
        "FP header 2",
        text.starts_with(&format!("{}\n", cms::FILE_HEADER_2)),
        &text[..text.find('\n').unwrap()],
    );
    let path = dir.join("B.part");
    b.write(&path).unwrap();
    let q = ReducedPart::read(&path).unwrap();
    let pat = pattern_b(2);
    c.holds(
        "FP read back equal, every float in bits, three vectors",
        b == q && same_bits(b.residual(), q.residual()) && q.residual_vectors() == 3,
        &format!("{}", q.residual_vectors()),
    );
    c.holds(
        "FP reduce_load of the read part bit-equal",
        same_bits(&b.reduce_load(&bm, &pat).unwrap(), &q.reduce_load(&bm, &pat).unwrap()),
        "",
    );
    let a = reduce(&bm, "xmin", &option_0474()).unwrap();
    let pa = dir.join("B-0474.part");
    a.write(&pa).unwrap();
    c.holds(
        "FP a part of ADR 0474 (no patterns) written and read unchanged",
        ReducedPart::read(&pa).unwrap() == a,
        "",
    );
    c.finish();
}
