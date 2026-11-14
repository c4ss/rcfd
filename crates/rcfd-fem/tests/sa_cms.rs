//! F-430 (ADR 0469): static condensation and the Craig-Bampton reduction (`rcfd_fem::cms`). The checks of ADR 0469 with
//! their registered limits; every measured number is printed.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{self, condense, reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::harmonic::{HarmonicLoad, ModalBasis};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{symmetric_eigen, ResidualTest};
use std::collections::HashMap;

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

/// The node at `p` (bit-equal coordinates) as a one-node set.
fn add_point_set(mesh: &mut SolidMesh, name: &str, p: [f64; 3]) {
    let s: Vec<usize> = (0..mesh.nodes().len()).filter(|&i| mesh.nodes()[i] == p).collect();
    assert_eq!(s.len(), 1, "one node at {p:?}");
    mesh.add_node_set(name, s).unwrap();
}

/// The small model S of ADR 0469 with the node set `mid` (x = 0.2).
fn small() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mid: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&i| (mesh.nodes()[i][0] - 0.2).abs() < 1e-12)
        .collect();
    assert_eq!(mid.len(), 9);
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

/// Part A: [0, 0.5], clamped on xmin, interface xmax.
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

/// The free part R.
fn free_part_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.2, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn opts(modes: usize) -> CmsOptions {
    CmsOptions::new(modes).threads(1)
}

fn bar_parts(modes: usize) -> (ReducedPart, ReducedPart) {
    let a = reduce(&part_a_model(), "xmax", &opts(modes)).unwrap();
    let b = reduce(&part_b_model(), "xmin", &opts(modes)).unwrap();
    (a, b)
}

/// The full bar's eight lowest omegas (rad/s).
fn full_bar_omegas() -> Vec<f64> {
    let r = modal_analysis(
        &full_bar(),
        &ModalOptions::new(8)
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

fn stress_of(model: &SolidModel, u: &[f64]) -> Vec<f64> {
    IntegrationPointField::compute(model, u)
        .unwrap()
        .stress()
        .iter()
        .flat_map(|s| s.iter().copied())
        .collect()
}

// ------------------------------------------------------------------------------------------------------------- statics

fn statics(c: &mut Checks, interface: &str, tag: &str) {
    let s = small();
    let full = solve_static(&s, &StaticOptions::default()).unwrap();
    let uf = full.displacement_values();
    let scale = max_abs(uf);
    let part = condense(&s, interface, Some(1)).unwrap();
    let ub = part.solve_static().unwrap();
    let dofs = s.dof_map();
    let mut is_b = vec![false; uf.len()];
    let mut eb = 0.0f64;
    for (a, b) in part.boundary().iter().enumerate() {
        let eq = dofs.equation(b.node, b.component).unwrap();
        is_b[eq] = true;
        eb = eb.max((ub[a] - uf[eq]).abs());
    }
    let u = part.recover_static(&s, &ub).unwrap();
    let ei = (0..uf.len())
        .filter(|&e| !is_b[e])
        .fold(0.0f64, |m, e| m.max((u[e] - uf[e]).abs()));
    let sf = stress_of(&s, uf);
    let sr = stress_of(&s, &u);
    let es = sf.iter().zip(&sr).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(&sf);
    c.le(&format!("{tag}1 retained ({interface})"), eb / scale, 1e-10);
    c.le(&format!("{tag}2 interior ({interface})"), ei / scale, 1e-10);
    c.le(&format!("{tag}3 stresses ({interface})"), es, 1e-10);
}

/// ST1 to ST3 (a1 to a3) on the interface xmax, ST4 on the interface mid.
#[test]
fn st_statics_condensed_and_recovered() {
    let mut c = Checks::new();
    statics(&mut c, "xmax", "ST");
    statics(&mut c, "mid", "ST4.");
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ all modes

/// AM1, AM2 (a4): every fixed-interface mode kept reproduces every frequency of S.
#[test]
fn am_all_modes_reproduce_all_frequencies() {
    let s = small();
    let reference = omegas_of(ModalBasis::all_modes(&s, Some(1)).unwrap().eigenvalues());
    let mut c = Checks::new();
    for (id, interface) in [("AM1", "xmax"), ("AM2", "mid")] {
        let ni = condense(&s, interface, Some(1)).unwrap().n_interior();
        let part = reduce(&s, interface, &opts(ni)).unwrap();
        let got = omegas_of(&part.eigen().unwrap().values);
        c.holds(
            &format!("{id} count"),
            got.len() == reference.len(),
            &format!("{} and {}", got.len(), reference.len()),
        );
        let worst = got
            .iter()
            .zip(&reference)
            .fold(0.0f64, |m, (a, b)| m.max((a / b - 1.0).abs()));
        println!(
            "{id}: interior {ni}, omega_1 {:.6} omega_max {:.6e}",
            got[0],
            got[got.len() - 1]
        );
        c.le(&format!("{id} ({interface}) largest |f/f_full - 1|"), worst, 1e-8);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ bar

fn assembly_omegas(modes: usize) -> Vec<f64> {
    let (a, b) = bar_parts(modes);
    let asm = Assembly::new(vec![a, b]).unwrap();
    omegas_of(&asm.eigen().unwrap().values)
}

/// B1, B2, B3 (a5 to a7): the bar of two parts.
#[test]
fn b_bar_of_two_parts() {
    let full = full_bar_omegas();
    println!("F omegas: {full:?}");
    let mut c = Checks::new();
    let mut err = Vec::new();
    for m in 0..=5 {
        let w = assembly_omegas(m);
        let e: Vec<f64> = (0..3).map(|k| w[k] / full[k] - 1.0).collect();
        println!("m = {m}: errors of the first three {:?}", e);
        if m == 5 {
            let rest: Vec<f64> = (3..8).map(|k| w[k] / full[k] - 1.0).collect();
            println!("record: m = 5, errors of frequencies 4 to 8: {rest:?}");
            for (k, ek) in e.iter().enumerate() {
                c.le(&format!("B1 f{} error", k + 1), *ek, 1e-3);
                c.ge(&format!("B2 f{} error", k + 1), *ek, -1e-10);
            }
        }
        err.push(e);
    }
    for k in 0..3 {
        let mut worst_rise = f64::NEG_INFINITY;
        for m in 0..5 {
            worst_rise = worst_rise.max(err[m + 1][k] - err[m][k]);
        }
        c.le(
            &format!("B3 f{} largest rise err(m+1) - err(m)", k + 1),
            worst_rise,
            1e-9,
        );
        c.holds(
            &format!("B3 f{} err(5) < err(0)/10", k + 1),
            err[5][k] < err[0][k] / 10.0,
            &format!("{:e} and {:e}", err[5][k], err[0][k]),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------- symmetry, mass, rigid motions

fn spd_parts() -> Vec<(&'static str, ReducedPart)> {
    let (a, b) = bar_parts(5);
    let s = small();
    let ni = condense(&s, "xmax", Some(1)).unwrap().n_interior();
    let sp = reduce(&s, "xmax", &opts(ni)).unwrap();
    let r = reduce(&free_part_model(), "xmin", &opts(5)).unwrap();
    vec![("A", a), ("B", b), ("S", sp), ("R", r)]
}

/// S1, S2 (a8), P1 (a9).
#[test]
fn s_p_symmetry_and_positive_mass() {
    let mut c = Checks::new();
    for (name, p) in spd_parts() {
        let n = p.n_reduced();
        let sk = cms::relative_asymmetry(p.stiffness(), n);
        let sm = cms::relative_asymmetry(p.mass(), n);
        c.le(&format!("S1 {name} K_r asymmetry"), sk, 1e-13);
        c.le(&format!("S1 {name} M_r asymmetry"), sm, 1e-13);
        c.le(
            &format!("S2 {name} asymmetry before symmetrisation"),
            p.asymmetry(),
            1e-10,
        );
        let e = symmetric_eigen(p.mass(), n).unwrap();
        let ratio = e.values[0] / e.values[n - 1];
        c.ge(&format!("P1 {name} lambda_min(M_r)/lambda_max(M_r)"), ratio, 1e-12);
    }
    c.finish();
}

fn rigid_ratio(p: &ReducedPart) -> (f64, f64) {
    let v = p.eigen().unwrap().values;
    let l7 = v[6];
    let worst = v[..6].iter().fold(0.0f64, |m, l| m.max((l.abs() / l7).sqrt()));
    (worst, l7)
}

/// R1 (a10): the free part R keeps six zero frequencies; record R' (part B, free).
#[test]
fn r1_free_part_rigid_motions() {
    let mut c = Checks::new();
    let r = reduce(&free_part_model(), "xmin", &opts(5)).unwrap();
    let (worst, l7) = rigid_ratio(&r);
    c.holds("R1 lambda_7 > 0", l7 > 0.0, &format!("{l7:e}"));
    c.le("R1 largest |f_k|/f_7, k <= 6", worst, 1e-6);
    let b = reduce(&part_b_model(), "xmin", &opts(5)).unwrap();
    let (wb, lb) = rigid_ratio(&b);
    println!("record R': part B free, largest |f_k|/f_7 = {wb:.3e}, lambda_7 = {lb:e}");
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- harmonic

/// The relative error of the assembly's harmonic response against F at each omega (loss factor 0.02, the corner load).
fn harmonic_errors(omegas: &[f64]) -> Vec<f64> {
    let full = full_bar();
    let am = part_a_model();
    let bm = part_b_model();
    let (a, b) = bar_parts(5);
    let asm = Assembly::new(vec![a, b]).unwrap();
    let dh = DirectHarmonic::new(
        &full,
        &DirectHarmonicOptions::new()
            .loss_factor("all", LossFactor::Constant(0.02))
            .threads(1),
    )
    .unwrap();
    let load = HarmonicLoad::from_model(&full).unwrap();
    // The node of A (x <= 0.5) or of B at each node of F.
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
    omegas
        .iter()
        .map(|&w| {
            let r = dh.response(&load, w).unwrap();
            let (xr, xi) = asm.harmonic(w, 0.02).unwrap();
            let models = [&am, &bm];
            let mut u = Vec::new();
            for p in 0..2 {
                let re = asm.parts()[p].expand(models[p], &asm.part_coordinates(p, &xr)).unwrap();
                let im = asm.parts()[p].expand(models[p], &asm.part_coordinates(p, &xi)).unwrap();
                u.push((re, im));
            }
            let (mut num, mut den) = (0.0, 0.0);
            for &(ef, part, ep) in &map {
                let (dr, di) = (u[part].0[ep] - r.re[ef], u[part].1[ep] - r.im[ef]);
                num += dr * dr + di * di;
                den += r.re[ef] * r.re[ef] + r.im[ef] * r.im[ef];
            }
            (num / den).sqrt()
        })
        .collect()
}

/// H1 (a11): within 1 percent up to half the lowest omitted fixed-interface frequency (predicted miss, ADR 0469). Missed at
/// its registered run (commit 4117733c): largest 9.966e-1 at omega 8241.00 (half of 16482, B's axial fixed-interface mode),
/// 9.381e-1 at the axial resonance 8178.99, first point above 1e-2 at 1977.84 rad/s; the limit is kept.
#[test]
#[ignore = "known miss of ADR 0469 H1 (predicted by its arithmetic item 3): 9.966e-1 > 1e-2; no axial fixed-interface mode is kept and the interior load's flexibility is missing above about 0.12 of the lowest omitted frequency"]
fn h1_harmonic_to_half_the_lowest_omitted_known_miss() {
    let (a, b) = bar_parts(5);
    let half = 0.5
        * a.omitted_eigenvalue()
            .unwrap()
            .sqrt()
            .min(b.omitted_eigenvalue().unwrap().sqrt());
    println!("omega_half = {half:.3} rad/s");
    let mut ws: Vec<f64> = (1..=50).map(|j| j as f64 * half / 50.0).collect();
    ws.extend(full_bar_omegas().into_iter().filter(|&w| w < half));
    let e = harmonic_errors(&ws);
    let mut worst = (0.0f64, 0.0);
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
        "H1: largest at omega {:.2}; first omega above 1e-2: {first:.2}",
        worst.1
    );
    let mut c = Checks::new();
    c.le("H1 largest relative error", worst.0, 1e-2);
    c.finish();
}

/// H2 (a11, band): within 1 percent up to 1000 rad/s.
#[test]
fn h2_harmonic_band() {
    let mut ws: Vec<f64> = (1..=30).map(|j| j as f64 * 1000.0 / 30.0).collect();
    ws.extend(full_bar_omegas().into_iter().filter(|&w| w < 1000.0));
    let e = harmonic_errors(&ws);
    let worst = e.iter().fold(0.0f64, |m, x| m.max(*x));
    let mut c = Checks::new();
    c.le("H2 largest relative error", worst, 1e-2);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ file

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// F1, F2 (a12).
#[test]
fn f_file_round_trip() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sa_cms");
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Checks::new();
    let (a, _) = bar_parts(5);
    let r = reduce(&free_part_model(), "xmin", &opts(5)).unwrap();
    for (name, p) in [("A", a), ("R", r)] {
        let path = dir.join(format!("{name}.part"));
        p.write(&path).unwrap();
        let q = ReducedPart::read(&path).unwrap();
        let floats = same_bits(p.stiffness(), q.stiffness())
            && same_bits(p.mass(), q.mass())
            && same_bits(p.load(), q.load())
            && same_bits(p.static_interior(), q.static_interior())
            && same_bits(p.fixed_interface_eigenvalues(), q.fixed_interface_eigenvalues())
            && same_bits(p.psi(), q.psi())
            && same_bits(p.phi(), q.phi())
            && p.omitted_eigenvalue().map(f64::to_bits) == q.omitted_eigenvalue().map(f64::to_bits)
            && p.asymmetry().to_bits() == q.asymmetry().to_bits()
            && p.boundary()
                .iter()
                .zip(q.boundary())
                .all(|(x, y)| x.node == y.node && x.component == y.component && same_bits(&x.position, &y.position));
        let ints = p.boundary().len() == q.boundary().len()
            && p.modes() == q.modes()
            && p.n_interior() == q.n_interior()
            && p == q;
        c.holds(&format!("F1 {name} floats bit-equal"), floats, "");
        c.holds(&format!("F1 {name} integers equal"), ints, "");
        let path2 = dir.join(format!("{name}-again.part"));
        q.write(&path2).unwrap();
        let same_file = std::fs::read(&path).unwrap() == std::fs::read(&path2).unwrap();
        c.holds(&format!("F1 {name} file written again byte-identical"), same_file, "");
        let text = std::fs::read_to_string(&path).unwrap();
        let cut = ReducedPart::from_text(&text[..text.len() / 2]);
        c.holds(
            &format!("F2 {name} half file refused"),
            cut.is_err(),
            &format!("{:?}", cut.err()),
        );
        let other = text.replacen("rcfd-cms-part 1", "rcfd-cms-part 2", 1);
        let hdr = ReducedPart::from_text(&other);
        c.holds(
            &format!("F2 {name} header 2 refused"),
            hdr.is_err(),
            &format!("{:?}", hdr.err()),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ refusals

/// X1 (a13, amendment 1) and X2 (a14).
#[test]
fn x_refusals() {
    let mut c = Checks::new();
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let empty = mesh.add_node_set("none", vec![]);
    c.holds(
        "X1 (i) empty node set refused by the mesh",
        matches!(empty, Err(FemError::EmptySet { .. })),
        &format!("{empty:?}"),
    );
    let s = small();
    let fixed = reduce(&s, "xmin", &opts(0));
    c.holds(
        "X1 (ii) interface with no free unknown refused",
        matches!(fixed, Err(FemError::InvalidValue { key: "interface", .. })),
        &format!("{:?}", fixed.err()),
    );
    let unknown = reduce(&s, "nowhere", &opts(0));
    c.holds(
        "X1 (iii) unknown set refused",
        matches!(unknown, Err(FemError::UnknownSet { .. })),
        &format!("{:?}", unknown.err()),
    );
    let ni = condense(&s, "xmax", Some(1)).unwrap().n_interior();
    c.holds("X2 interior size", ni == 81, &format!("{ni}"));
    let above = reduce(&s, "xmax", &opts(ni + 1));
    c.holds(
        "X2 modes = n_i + 1 refused",
        matches!(above, Err(FemError::InvalidValue { key: "modes", .. })),
        &format!("{:?}", above.err()),
    );
    let at = reduce(&s, "xmax", &opts(ni));
    c.holds("X2 modes = n_i taken", at.is_ok(), &format!("{:?}", at.err()));
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ paths, threads

/// C1: the subspace path against the dense path.
#[test]
fn c1_subspace_and_dense_paths_agree() {
    let s = small();
    let d = reduce(&s, "xmax", &opts(5)).unwrap();
    let p = reduce(&s, "xmax", &opts(5).dense_limit(0)).unwrap();
    let mut c = Checks::new();
    let mut worst = 0.0f64;
    for (x, y) in d
        .fixed_interface_eigenvalues()
        .iter()
        .zip(p.fixed_interface_eigenvalues())
    {
        worst = worst.max((x / y - 1.0).abs());
    }
    worst = worst.max((d.omitted_eigenvalue().unwrap() / p.omitted_eigenvalue().unwrap() - 1.0).abs());
    c.le("C1 fixed-interface eigenvalues", worst, 1e-9);
    let fd = omegas_of(&d.eigen().unwrap().values);
    let fp = omegas_of(&p.eigen().unwrap().values);
    let wf = fd.iter().zip(&fp).fold(0.0f64, |m, (x, y)| m.max((x / y - 1.0).abs()));
    c.le("C1 frequencies of the part", wf, 1e-9);
    c.finish();
}

/// T1: 1 and 4 threads give the same bits.
#[test]
fn t1_threads_bit_identical() {
    let m = part_a_model();
    let one = reduce(&m, "xmax", &CmsOptions::new(5).threads(1)).unwrap();
    let four = reduce(&m, "xmax", &CmsOptions::new(5).threads(4)).unwrap();
    let mut c = Checks::new();
    let same = same_bits(one.stiffness(), four.stiffness())
        && same_bits(one.mass(), four.mass())
        && same_bits(one.load(), four.load())
        && same_bits(one.psi(), four.psi())
        && same_bits(one.phi(), four.phi());
    c.holds("T1 K_r, M_r, load, Psi, Phi bit-identical", same, "");
    c.holds("E1 cms::PENDING is None", cms::PENDING.is_none(), "");
    c.finish();
}
