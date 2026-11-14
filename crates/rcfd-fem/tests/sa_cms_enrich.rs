//! F-603 (ADR 0532): interface enrichment of the Craig-Bampton reduction of ADR 0474 by Krylov blocks of `(K_ii, M_ii)`
//! started from the constraint modes (`rcfd_fem::cms`, `CmsOptions::interface_enrichment`). The checks of ADR 0532 with their
//! registered limits (and Amendment 1); every measured number is printed. Every aggregated error is required finite before it
//! is compared.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{self, reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent_with;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{symmetric_eigen, ResidualTest};
use rcfd_linalg::CsrMatrix;
use std::collections::HashMap;

/// The registered cutoff (ADR 0474, kept by ADRs 0511 and 0532).
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

/// E1: the ADR 0474 option and one block (the residual flexibility of the constraint modes).
fn e1() -> CmsOptions {
    option_0474().interface_enrichment(1)
}

/// E2: the ADR 0474 option and two Krylov blocks.
fn e2() -> CmsOptions {
    option_0474().interface_enrichment(2)
}

/// E2c (a record): the cutoff alone and two blocks, no vector of the load.
fn e2c() -> CmsOptions {
    cutoff_only().interface_enrichment(2)
}

fn same_part(a: &ReducedPart, b: &ReducedPart) -> bool {
    a == b && same_bits(a.stiffness(), b.stiffness()) && same_bits(a.mass(), b.mass()) && a.to_text() == b.to_text()
}

// ------------------------------------------------------------------------------------------------------------- defaults

/// D7 (c6): the option asked with zero blocks.
#[test]
fn d7_zero_blocks() {
    let mut c = Checks::new();
    let bm = part_b_model();
    for (name, o) in [
        ("m = 5", CmsOptions::new(5).threads(1)),
        ("the ADR 0474 option", option_0474()),
        ("the ADR 0511 option", option_b()),
    ] {
        let plain = reduce(&bm, "xmin", &o).unwrap();
        let zero = reduce(&bm, "xmin", &o.clone().interface_enrichment(0)).unwrap();
        c.holds(
            &format!("D7 {name}: interface_enrichment(0) equals the part without"),
            same_part(&zero, &plain),
            "",
        );
    }
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

/// B's interior blocks recomputed in the test: dense `K_ii` (its Cholesky factor), `M_ii`, `M_ib`.
struct Interior {
    ni: usize,
    nb: usize,
    l: Vec<f64>,
    mii: Vec<f64>,
    mib: Vec<f64>,
}

impl Interior {
    fn of(model: &SolidModel, p: &ReducedPart) -> Interior {
        let k = stiffness_with(model, Some(1)).unwrap();
        let m = consistent_with(model, Some(1)).unwrap();
        let ii = interior_equations(model, p);
        let dofs = model.dof_map();
        let bb: Vec<usize> = p
            .boundary()
            .iter()
            .map(|b| dofs.equation(b.node, b.component).unwrap())
            .collect();
        let dense = |a: &CsrMatrix, rows: &[usize], cols: &[usize]| -> Vec<f64> {
            let mut at = vec![usize::MAX; a.n.max(a.ncols)];
            for (j, &c) in cols.iter().enumerate() {
                at[c] = j;
            }
            let mut d = vec![0.0; rows.len() * cols.len()];
            for (ri, &r) in rows.iter().enumerate() {
                for q in a.row_ptr[r]..a.row_ptr[r + 1] {
                    if at[a.col[q]] != usize::MAX {
                        d[ri * cols.len() + at[a.col[q]]] += a.val[q];
                    }
                }
            }
            d
        };
        let (ni, nb) = (ii.len(), bb.len());
        let kii = dense(&k, &ii, &ii);
        let mut l = vec![0.0; ni * ni];
        for j in 0..ni {
            let mut s = kii[j * ni + j];
            for q in 0..j {
                s -= l[j * ni + q] * l[j * ni + q];
            }
            assert!(s > 0.0, "K_ii positive definite");
            let d = s.sqrt();
            l[j * ni + j] = d;
            for i in j + 1..ni {
                let mut v = kii[i * ni + j];
                for q in 0..j {
                    v -= l[i * ni + q] * l[j * ni + q];
                }
                l[i * ni + j] = v / d;
            }
        }
        Interior {
            ni,
            nb,
            l,
            mii: dense(&m, &ii, &ii),
            mib: dense(&m, &ii, &bb),
        }
    }
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let (n, l) = (self.ni, &self.l);
        let mut y = b.to_vec();
        for i in 0..n {
            let mut s = y[i];
            for q in 0..i {
                s -= l[i * n + q] * y[q];
            }
            y[i] = s / l[i * n + i];
        }
        for i in (0..n).rev() {
            let mut s = y[i];
            for q in i + 1..n {
                s -= l[q * n + i] * y[q];
            }
            y[i] = s / l[i * n + i];
        }
        y
    }
    fn m(&self, x: &[f64]) -> Vec<f64> {
        let n = self.ni;
        (0..n)
            .map(|r| (0..n).map(|c| self.mii[r * n + c] * x[c]).sum())
            .collect()
    }
    fn mdot(&self, x: &[f64], y: &[f64]) -> f64 {
        x.iter().zip(&self.m(y)).map(|(a, b)| a * b).sum()
    }
    /// The block-1 start vectors `K_ii^{-1} (M_ib e_j + M_ii Psi e_j)` with the part's `Psi`.
    fn block1(&self, p: &ReducedPart) -> Vec<Vec<f64>> {
        let (ni, nb) = (self.ni, self.nb);
        (0..nb)
            .map(|j| {
                let pj: Vec<f64> = (0..ni).map(|r| p.psi()[r * nb + j]).collect();
                let mp = self.m(&pj);
                let g: Vec<f64> = (0..ni).map(|r| self.mib[r * nb + j] + mp[r]).collect();
                self.solve(&g)
            })
            .collect()
    }
    /// The largest relative M-norm of the part of each `x` outside the span of the part's `[Phi, R]` (two projection
    /// passes on the part's M-orthonormal columns).
    fn outside(&self, p: &ReducedPart, xs: &[Vec<f64>]) -> f64 {
        let ni = self.ni;
        let (nm, np) = (p.modes(), p.residual_vectors());
        let mut basis: Vec<Vec<f64>> = (0..nm)
            .map(|k| (0..ni).map(|r| p.phi()[r * nm + k]).collect())
            .collect();
        basis.extend((0..np).map(|k| (0..ni).map(|r| p.residual()[r * np + k]).collect::<Vec<f64>>()));
        let mut worst = 0.0f64;
        for x in xs {
            let own = self.mdot(x, x).sqrt();
            let mut r = x.clone();
            for _pass in 0..2 {
                let mr = self.m(&r);
                let cs: Vec<f64> = basis
                    .iter()
                    .map(|q| q.iter().zip(&mr).map(|(a, b)| a * b).sum())
                    .collect();
                for (q, a) in basis.iter().zip(cs) {
                    for (v, w) in r.iter_mut().zip(q) {
                        *v -= a * w;
                    }
                }
            }
            let e = self.mdot(&r, &r).sqrt() / own;
            assert!(e.is_finite(), "outside: a non-finite ratio");
            worst = worst.max(e);
        }
        worst
    }
}

/// E-P1, E-P2, E-P3, NC-P3 (c1), E-SP, E-TH, E-FP (design).
#[test]
fn e_vectors_of_the_enrichment() {
    let mut c = Checks::new();
    let am = part_a_model();
    let bm = part_b_model();
    let a1 = reduce(&am, "xmax", &e1()).unwrap();
    let b1 = reduce(&bm, "xmin", &e1()).unwrap();
    let a2 = reduce(&am, "xmax", &e2()).unwrap();
    let b2 = reduce(&bm, "xmin", &e2()).unwrap();
    let b0e = reduce(&bm, "xmin", &cutoff_only().interface_enrichment(1)).unwrap();
    let a0e = reduce(&am, "xmax", &cutoff_only().interface_enrichment(1)).unwrap();
    for (name, p, want) in [
        ("E1 A", &a1, 27),
        ("E1 B", &b1, 28),
        ("E2 A", &a2, 54),
        ("E2 B", &b2, 55),
        ("cutoff alone, one block, B", &b0e, 27),
        ("cutoff alone, one block, A", &a0e, 27),
    ] {
        c.holds(
            &format!("E-P1 {name}: {want} vectors"),
            p.residual_vectors() == want,
            &format!("{}", p.residual_vectors()),
        );
    }
    // E-P2: M_qR and M_RR - I.
    for (name, p) in [("E1 A", &a1), ("E1 B", &b1), ("E2 A", &a2), ("E2 B", &b2)] {
        let (nb, nm, np, nr) = (p.n_boundary(), p.modes(), p.residual_vectors(), p.n_reduced());
        let o = nb + nm;
        let mut mq = 0.0f64;
        let mut mrr = 0.0f64;
        for j in 0..np {
            for q in 0..nm {
                mq = mq.max(finite("M_qR", &[p.mass()[(nb + q) * nr + o + j]])[0].abs());
            }
            for k in 0..np {
                let id = if j == k { 1.0 } else { 0.0 };
                mrr = mrr.max((finite("M_RR", &[p.mass()[(o + j) * nr + o + k]])[0] - id).abs());
            }
        }
        c.le(&format!("E-P2 {name}: largest |M_qR|"), mq, 1e-10);
        c.le(&format!("E-P2 {name}: largest |M_RR - I|"), mrr, 1e-10);
        let kq = (0..nm).fold(0.0f64, |m, q| {
            (0..np).fold(m, |m, j| m.max(p.stiffness()[(nb + q) * nr + o + j].abs()))
        });
        println!(
            "record {name}: |K_qR| over the largest K_r entry {:.3e}",
            kq / max_abs(finite("K_r", p.stiffness()))
        );
    }
    // E-P3, NC-P3: the start vectors recomputed in the test against the spans.
    {
        let it = Interior::of(&bm, &b1);
        let starts = it.block1(&b1);
        c.le(
            "E-P3 B E1: block-1 start vectors outside the span of [Phi, R]",
            it.outside(&b1, &starts),
            1e-8,
        );
        let ni = it.ni;
        let np = b2.residual_vectors();
        let block1: Vec<Vec<f64>> = (1..28)
            .map(|k| (0..ni).map(|r| b2.residual()[r * np + k]).collect())
            .collect();
        let starts2: Vec<Vec<f64>> = block1.iter().map(|r| it.solve(&it.m(r))).collect();
        c.le(
            "E-P3 B E2: block-2 start vectors outside the span of [Phi, R]",
            it.outside(&b2, &starts2),
            1e-8,
        );
        c.le(
            "E-P3 B E2: block-1 start vectors outside the span of [Phi, R]",
            it.outside(&b2, &it.block1(&b2)),
            1e-8,
        );
        let c1 = reduce(&bm, "xmin", &option_0474()).unwrap();
        c.ge(
            "NC-P3 B C1: block-1 start vectors outside the span of [Phi, R] without the enrichment",
            it.outside(&c1, &it.block1(&c1)),
            1e-6,
        );
    }
    // E-SP on B, E1 and E2.
    for (name, p) in [("E1 B", &b1), ("E2 B", &b2)] {
        let n = p.n_reduced();
        c.le(
            &format!("E-SP {name}: K_r asymmetry"),
            cms::relative_asymmetry(finite("K_r", p.stiffness()), n),
            1e-13,
        );
        c.le(
            &format!("E-SP {name}: M_r asymmetry"),
            cms::relative_asymmetry(finite("M_r", p.mass()), n),
            1e-13,
        );
        c.le(
            &format!("E-SP {name}: asymmetry before symmetrisation"),
            p.asymmetry(),
            1e-10,
        );
        let e = symmetric_eigen(p.mass(), n).unwrap();
        finite("M_r eigenvalues", &e.values);
        c.ge(
            &format!("E-SP {name}: lambda_min(M_r)/lambda_max(M_r)"),
            e.values[0] / e.values[n - 1],
            1e-12,
        );
    }
    // E-TH: threads 1 and 4.
    {
        let four = reduce(&bm, "xmin", &e2().threads(4)).unwrap();
        let same = same_bits(b2.stiffness(), four.stiffness())
            && same_bits(b2.mass(), four.mass())
            && same_bits(b2.load(), four.load())
            && same_bits(b2.psi(), four.psi())
            && same_bits(b2.phi(), four.phi())
            && same_bits(b2.residual(), four.residual());
        c.holds(
            "E-TH B E2, threads 1 and 4: K_r, M_r, load, Psi, Phi, R bit-identical",
            same,
            "",
        );
    }
    // E-FP: the file.
    {
        let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sa_cms_enrich");
        std::fs::create_dir_all(&dir).unwrap();
        let text = b2.to_text();
        let path = dir.join("B-E2.part");
        b2.write(&path).unwrap();
        let q = ReducedPart::read(&path).unwrap();
        c.holds(
            "E-FP B E2 written and read: equal, every float in bits, header 2, 55 vectors",
            text.starts_with(&format!("{}\n", cms::FILE_HEADER_2))
                && same_part(&b2, &q)
                && same_bits(b2.residual(), q.residual())
                && q.residual_vectors() == 55,
            &format!("{}", q.residual_vectors()),
        );
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- frequencies

/// The indices of F below `half`.
fn below(full: &[f64], half: f64) -> Vec<usize> {
    (0..full.len()).filter(|&k| full[k] < half).collect()
}

/// The largest `abs(omega_R / omega_F - 1)` over `idx` (printed per index).
fn largest_error(name: &str, wr: &[f64], full: &[f64], idx: &[usize]) -> f64 {
    let mut worst = 0.0f64;
    for &k in idx {
        let d = wr[k] / full[k] - 1.0;
        assert!(d.is_finite(), "{name}: a non-finite frequency error");
        println!("  {name}: omega_F {:.1}: delta {d:.3e}", full[k]);
        worst = worst.max(d.abs());
    }
    worst
}

/// LF, NC-LF (c2), SL1, SL2, SL3, NC-S (c4).
#[test]
fn lf_sl_frequencies() {
    let mut c = Checks::new();
    let full = full_bar_omegas();
    finite("F frequencies", &full);
    let w_cut = omegas_of(finite("C0", &asm_cutoff().eigen().unwrap().values));
    let w_c1 = omegas_of(finite("C1", &asm_0474().eigen().unwrap().values));
    let half = half_of(&asm_cutoff());
    assert!(half.is_finite());
    let idx = below(&full, half);
    println!("omega_half {half:.2}, {} frequencies of F below", idx.len());
    for (name, o) in [("E1", e1()), ("E2", e2())] {
        let asm = assembly_of(&o, &o);
        let h = half_of(&asm);
        let wr = omegas_of(finite(name, &asm.eigen().unwrap().values));
        let count = wr.iter().filter(|&&w| w < h).count();
        c.holds(
            &format!("LF {name} count: the assembly's frequencies below the bound as many as F's (ten)"),
            count == idx.len() && idx.len() == 10 && h == half,
            &format!("{count} and {} (omega_half {h:.2})", idx.len()),
        );
        c.le(
            &format!("LF {name} largest |omega_R / omega_F - 1| below the bound"),
            largest_error(name, &wr, &full, &idx),
            1e-3,
        );
        let mut least_cut = f64::NEG_INFINITY;
        let mut least_c1 = f64::NEG_INFINITY;
        let mut lowest = f64::INFINITY;
        for &k in &idx {
            let (a, b, f) = (wr[k] / w_cut[k] - 1.0, wr[k] / w_c1[k] - 1.0, wr[k] / full[k] - 1.0);
            assert!(a.is_finite() && b.is_finite() && f.is_finite());
            println!(
                "  {name} f{}: over the cutoff alone {a:.3e}, over the ADR 0474 option {b:.3e}",
                k + 1
            );
            least_cut = least_cut.max(a);
            least_c1 = least_c1.max(b);
            lowest = lowest.min(f);
        }
        c.lt(
            &format!("SL1 {name}: largest change of the ten against the cutoff alone"),
            least_cut,
            -1e-14,
        );
        c.lt(
            &format!("SL2 {name}: largest change of the ten against the ADR 0474 option"),
            least_c1,
            -1e-14,
        );
        c.ge(
            &format!("SL3 {name}: smallest omega_E / omega_F - 1 over the ten"),
            lowest,
            -1e-9,
        );
    }
    c.ge(
        "NC-LF C1 largest |omega_R / omega_F - 1| below the bound",
        largest_error("C1", &w_c1, &full, &idx),
        1e-3,
    );
    let again = omegas_of(&asm_cutoff().eigen().unwrap().values);
    let nothing = idx
        .iter()
        .fold(f64::NEG_INFINITY, |m, &k| m.max(again[k] / w_cut[k] - 1.0));
    c.holds(
        "NC-S the cutoff-only assembly against itself: no frequency lowered",
        nothing.is_finite() && nothing >= -1e-14,
        &format!("largest change {nothing:e}"),
    );
    // Records: E2c, and E2 against E1.
    let w2c = omegas_of(&assembly_of(&e2c(), &e2c()).eigen().unwrap().values);
    println!(
        "record E2c largest frequency error below the bound {:.3e}",
        largest_error("E2c", &w2c, &full, &idx)
    );
    let w1 = omegas_of(&assembly_of(&e1(), &e1()).eigen().unwrap().values);
    let w2 = omegas_of(&assembly_of(&e2(), &e2()).eigen().unwrap().values);
    for &k in &idx {
        println!("  record E2 over E1 f{}: {:.3e}", k + 1, w2[k] / w1[k] - 1.0);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ harmonic

/// The 100 points `j omega_half / 100` that are farther than `2 eta` from every frequency of F.
fn off_resonance_points(half: f64, full: &[f64]) -> Vec<f64> {
    (1..=100)
        .map(|j| j as f64 * half / 100.0)
        .filter(|w| full.iter().all(|k| (w / k - 1.0).abs() > 2.0 * LOSS))
        .collect()
}

/// The largest error and where, and the first point above 1e-2.
fn worst_of(ws: &[f64], e: &[f64]) -> (f64, f64, f64) {
    let mut worst = (0.0f64, 0.0f64);
    for (w, x) in ws.iter().zip(e) {
        assert!(x.is_finite());
        if *x > worst.0 {
            worst = (*x, *w);
        }
    }
    let first = ws
        .iter()
        .zip(e)
        .filter(|(_, x)| **x > 1e-2)
        .map(|(w, _)| *w)
        .fold(f64::INFINITY, f64::min);
    (worst.0, worst.1, first)
}

/// OFF1, NC-OFF (c3) and the records of the response.
#[test]
fn off1_off_resonance_to_half_the_lowest_omitted() {
    let mut c = Checks::new();
    let bar = Bar::new();
    let full = full_bar_omegas();
    let half = half_of(&asm_cutoff());
    let ws = off_resonance_points(half, &full);
    c.holds("OFF1 at least 60 points", ws.len() >= 60, &format!("{}", ws.len()));
    let all: Vec<f64> = (1..=100).map(|j| j as f64 * half / 100.0).collect();
    for (name, o) in [("E1", e1()), ("E2", e2())] {
        let asm = assembly_of(&o, &o);
        let e = bar.harmonic_errors(&asm, &ws);
        for (w, x) in ws.iter().zip(&e) {
            println!("  {name} omega {w:.2}: {x:.3e}");
        }
        let (big, at, first) = worst_of(&ws, &e);
        println!(
            "{name}: {} points, largest {big:.3e} at {at:.2}, first above 1e-2 at {first:.2}",
            ws.len()
        );
        c.le(&format!("OFF1 {name} largest off-resonance relative error"), big, 1e-2);
        let ea = bar.harmonic_errors(&asm, &all);
        let (b, a, _) = worst_of(&all, &ea);
        println!("record {name}: all 100 points largest {b:.3e} at {a:.2}");
    }
    let e = bar.harmonic_errors(&asm_0474(), &ws);
    let (big, at, first) = worst_of(&ws, &e);
    println!("C1: largest {big:.3e} at {at:.2}, first above 1e-2 at {first:.2}");
    c.ge("NC-OFF C1 largest off-resonance relative error", big, 1e-2);
    let e = bar.harmonic_errors(&assembly_of(&e2c(), &e2c()), &ws);
    let (big, at, first) = worst_of(&ws, &e);
    println!("record E2c: largest {big:.3e} at {at:.2}, first above 1e-2 at {first:.2}");
    // Record: the axial resonance of F (nearest to the closed form).
    let axial = 0.5 * std::f64::consts::PI * (2.1e11f64 / 7850.0).sqrt();
    let k = (0..full.len())
        .min_by(|&a, &b| (full[a] - axial).abs().partial_cmp(&(full[b] - axial).abs()).unwrap())
        .unwrap();
    for (name, o) in [("E1", e1()), ("E2", e2())] {
        let asm = assembly_of(&o, &o);
        let wr = omegas_of(&asm.eigen().unwrap().values);
        let delta = wr[k] / full[k] - 1.0;
        let ea = bar.harmonic_errors(&asm, &[full[k]])[0];
        println!(
            "record {name} axial resonance {:.1}: error {ea:.3e}, delta {delta:.3e}, 2|delta|/eta {:.3e}",
            full[k],
            2.0 * delta.abs() / LOSS
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- statics

/// The static error of the bar's assembly under its own load (the corner force) against `solve_static` of F.
fn bar_statics(bar: &Bar, asm: &Assembly) -> f64 {
    let uf = solve_static(&bar.full, &StaticOptions::default()).unwrap();
    let uf = finite("F statics", uf.displacement_values()).to_vec();
    let (x, _) = asm.harmonic(0.0, 0.0).unwrap();
    finite("assembly statics", &x);
    let u = bar.expand(asm, &x);
    let d = bar
        .map
        .iter()
        .fold(0.0f64, |m, &(ef, part, ep)| m.max((u[part][ep] - uf[ef]).abs()));
    d / max_abs(&uf)
}

/// ST-E3 / ST-E4 as registered (the static recovery) and ST-E3b / ST-E4b (the single-part assembly through the vectors).
fn s_statics_both(s: &SolidModel, iface: &str, o: &CmsOptions) -> (f64, f64) {
    let uf = solve_static(s, &StaticOptions::default()).unwrap();
    let uf = finite("S statics", uf.displacement_values()).to_vec();
    let p = reduce(s, iface, o).unwrap();
    let ub = p.solve_static().unwrap();
    let ur = p.recover_static(s, finite("S u_b", &ub)).unwrap();
    let ea = ur.iter().zip(&uf).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(&uf);
    let asm = Assembly::new(vec![p]).unwrap();
    let (x, _) = asm.harmonic(0.0, 0.0).unwrap();
    finite("S assembly statics", &x);
    let u = asm.parts()[0].expand(s, &x).unwrap();
    let eb = u.iter().zip(&uf).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(&uf);
    println!("  {iface}: {} vectors", asm.parts()[0].residual_vectors());
    (ea, eb)
}

/// ST-E, ST-E2, ST-E3, ST-E3b, ST-E4, ST-E4b, NC-ST (c5).
#[test]
fn st_statics_of_the_corner_load() {
    let mut c = Checks::new();
    let bar = Bar::new();
    for (name, o) in [("E1", e1()), ("E2", e2())] {
        c.le(
            &format!("ST-E {name}: statics of the corner load"),
            bar_statics(&bar, &assembly_of(&o, &o)),
            1e-10,
        );
    }
    let pats = patterns_b();
    let asm = assembly_of(&e2(), &e2().load_patterns(pats.clone()));
    for (comp, p) in pats.iter().enumerate() {
        c.le(
            &format!("ST-E2 E2 and the ADR 0511 patterns, component {comp}"),
            bar.component_statics(&asm, comp, p),
            1e-10,
        );
    }
    c.ge(
        "NC-ST E2c (no load vector): statics of the corner load",
        bar_statics(&bar, &assembly_of(&e2c(), &e2c())),
        1e-6,
    );
    let s = small_with(true, true);
    let one = CmsOptions::new(5)
        .residual_vectors(true)
        .interface_enrichment(1)
        .threads(1);
    for iface in ["mid", "xmax"] {
        let (a, b) = s_statics_both(&s, iface, &one);
        c.le(&format!("ST-E3 S {iface}, m = 5, one block: static recovery"), a, 1e-10);
        c.le(
            &format!("ST-E3b S {iface}, m = 5, one block: through the vectors"),
            b,
            1e-10,
        );
    }
    let guyan = CmsOptions::new(0)
        .residual_vectors(true)
        .interface_enrichment(1)
        .threads(1);
    let (a, b) = s_statics_both(&s, "mid", &guyan);
    c.le("ST-E3 S mid, m = 0, one block: static recovery", a, 1e-10);
    c.le("ST-E3b S mid, m = 0, one block: through the vectors", b, 1e-10);
    let sub = CmsOptions::new(5)
        .residual_vectors(true)
        .interface_enrichment(2)
        .dense_limit(0)
        .threads(1);
    let (a, b) = s_statics_both(&s, "xmax", &sub);
    c.le(
        "ST-E4 S xmax, m = 5, two blocks, subspace path: static recovery",
        a,
        1e-10,
    );
    c.le(
        "ST-E4b S xmax, m = 5, two blocks, subspace path: through the vectors",
        b,
        1e-10,
    );
    c.finish();
}
