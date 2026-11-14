//! F-557 (ADR 0486): the pre-stress of a static result on a model with both solid and structural elements. Checks A1, T1,
//! T2, T2b, M1 to M3, T3, T3u, T3m, H1 to H3 and the negative controls N1 to N4 of the registration. Each registered run is
//! made once; every value is printed. Every joint is `JointCoupling::Consistent`; the solid comparison models are joined by a
//! node-to-surface tie on coincident nodes.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::hex20::{traction_loads, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::rotating::{CampbellTable, RotatingOptions, RotatingPart};
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie::{self, JointCoupling, TieMethod, TieOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel, SolidResult,
};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
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
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap()
}

fn opts(n: usize, tol: f64) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(tol)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// The nodes `n` in `range` of `mesh` with `pred(position)`, as a node set `name`.
fn add_set<F: Fn(Vec3) -> bool>(
    mesh: &mut SolidMesh,
    name: &str,
    range: std::ops::Range<usize>,
    pred: F,
) -> Vec<usize> {
    let v: Vec<usize> = range.filter(|&n| pred(mesh.nodes()[n])).collect();
    assert!(!v.is_empty(), "node set {name} is empty");
    mesh.add_node_set(name, v.clone()).unwrap();
    v
}

/// A surface set `name` of the faces of the surface set `from` whose element index satisfies `pred`.
fn sub_surface<F: Fn(usize) -> bool>(mesh: &mut SolidMesh, name: &str, from: &str, pred: F) {
    let faces: Vec<FaceRef> = mesh
        .surface_set(from)
        .unwrap()
        .iter()
        .copied()
        .filter(|f| pred(f.element))
        .collect();
    mesh.add_surface_set(name, faces).unwrap();
}

fn add_rows(m: &mut SolidModel, rows: Vec<Constraint>) {
    for r in rows {
        m.add_constraint(r).unwrap();
    }
}

fn solve(m: &SolidModel, forces: &[NodalForceAt]) -> SolidResult {
    solve_static_mpc(m, &[], forces, &[], &StaticOptions::default())
        .unwrap()
        .result
}

/// `phi^T M_c phi / phi^T M phi`: the kinetic share of component `c`.
fn share(model: &SolidModel, m: &CsrMatrix, phi: &[f64], c: usize) -> f64 {
    let dofs = model.dof_map();
    let mut y = vec![0.0; m.n];
    m.matvec(phi, &mut y);
    let (mut part, mut all) = (0.0, 0.0);
    for r in 0..m.n {
        let v = phi[r] * y[r];
        all += v;
        if dofs.node_component(r).unwrap().1 == c {
            part += v;
        }
    }
    part / all
}

/// A Shell4 (or Shell3, each cell cut from its first to its third node) grid of `nx x ny` cells on `[x0, x0 + lx] x [y0, y0
/// + ly]` at `z = 0`, thickness `t`; element set `all`.
fn shell_grid(x0: f64, y0: f64, lx: f64, ly: f64, nx: usize, ny: usize, t: f64, tri: bool) -> SolidMesh {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([x0 + lx * i as f64 / nx as f64, y0 + ly * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if tri { 2 * nx * ny } else { nx * ny };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh
}

// ------------------------------------------------------------------------------------------------- exact comparisons

/// Whether the pre-stress of elements `first..first + part.n_elements` of `mixed` equals `part`'s, bit for bit.
fn same_part(mixed: &Prestress, part: &Prestress, first: usize) -> bool {
    let n = part.offsets().len() - 1;
    (0..n).all(|e| {
        let a = mixed.of_element(first + e);
        let b = part.of_element(e);
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits()))
    })
}

fn entries(k: &CsrMatrix, shift: usize) -> BTreeMap<(usize, usize), f64> {
    let mut out = BTreeMap::new();
    for i in 0..k.n {
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            out.insert((i + shift, k.col[p] + shift), k.val[p]);
        }
    }
    out
}

/// M3: every stored entry of `mixed` bit-equal to the entry of the part at the shifted indices (zero across), and every
/// entry of each part found. Returns (mismatches, entries of the parts not found).
fn same_assembly(mixed: &CsrMatrix, a: &CsrMatrix, b: &CsrMatrix) -> (usize, usize) {
    let mut want = entries(a, 0);
    want.extend(entries(b, a.n));
    let mut bad = 0;
    for i in 0..mixed.n {
        for p in mixed.row_ptr[i]..mixed.row_ptr[i + 1] {
            let j = mixed.col[p];
            let v = mixed.val[p];
            match want.remove(&(i, j)) {
                Some(w) if w.to_bits() == v.to_bits() => {}
                Some(_) => bad += 1,
                None if v == 0.0 => {}
                None => bad += 1,
            }
        }
    }
    (bad, want.len())
}

/// M1 to M3 for a mixed model of a solid part `a` (first) and a structural part `b`, with their materials.
fn m1_m3(
    c: &mut Checks,
    tag: &str,
    m: &SolidModel,
    pre: &Prestress,
    u: &SolidResult,
    parts: [(&SolidMesh, Material); 2],
) {
    let [(a, ma), (b, mb)] = parts;
    let mut alone = Vec::new();
    let mut start = 0;
    for (mesh, mat) in [(a, ma), (b, mb)] {
        let mut pm = SolidModel::new(mesh.clone());
        pm.add_material("all", mat).unwrap();
        let n = pm.dof_map().n_equations();
        let slice = u.displacement_values()[start..start + n].to_vec();
        start += n;
        let r = SolidResult::new(pm.dof_map(), slice).unwrap();
        let p = Prestress::from_static(&pm, &r).unwrap();
        alone.push((pm, p));
    }
    assert_eq!(start, u.displacement_values().len());
    c.holds(
        &format!("M1 {tag} solid part"),
        same_part(pre, &alone[0].1, 0),
        "bit-equal to the block alone",
    );
    c.holds(
        &format!("M2 {tag} structural part"),
        same_part(pre, &alone[1].1, a.n_elements()),
        "bit-equal to the structural part alone",
    );
    let k = geometric_stiffness(m, pre, Some(1)).unwrap();
    let ka = geometric_stiffness(&alone[0].0, &alone[0].1, Some(1)).unwrap();
    let kb = geometric_stiffness(&alone[1].0, &alone[1].1, Some(1)).unwrap();
    let (bad, missing) = same_assembly(&k, &ka, &kb);
    c.holds(
        &format!("M3 {tag} assembled K_sigma"),
        bad == 0 && missing == 0,
        &format!("{bad} entries differ, {missing} part entries not found"),
    );
}

// ------------------------------------------------------------------------------------------------------------- A1

#[test]
fn a1_rf1_model_prestress_after_f557() {
    let mut c = Checks::new();
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let mut sh = SolidMesh::new(vec![[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 1.0, 0.0], [3.0, 1.0, 0.0]]).unwrap();
    sh.add_block(ElementKind::Shell4, vec![0, 1, 2, 3]).unwrap();
    sh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    sh.add_element_set("all", vec![0]).unwrap();
    let mesh = tie::assemble_parts(&[("A", &hex), ("B", &sh)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", steel(0.3)).unwrap();
    m.add_material("B.all", steel(0.3)).unwrap();
    let n = m.dof_map().n_equations();
    let u: Vec<f64> = (0..n).map(|i| 1e-4 * (0.37 * i as f64 + 0.2).sin()).collect();
    let r = SolidResult::new(m.dof_map(), u).unwrap();
    let p = Prestress::from_static(&m, &r);
    c.holds(
        "A1 mixed pre-stress taken",
        p.is_ok(),
        &format!("{:?}", p.as_ref().err()),
    );
    let p = p.unwrap();
    c.holds("A1 check_fits", p.check_fits(&m).is_ok(), "fits the model");
    let mut alone = Vec::new();
    let mut start = 0;
    for mesh in [&hex, &sh] {
        let mut pm = SolidModel::new(mesh.clone());
        pm.add_material("all", steel(0.3)).unwrap();
        let k = pm.dof_map().n_equations();
        let slice = r.displacement_values()[start..start + k].to_vec();
        start += k;
        let rr = SolidResult::new(pm.dof_map(), slice).unwrap();
        alone.push(Prestress::from_static(&pm, &rr).unwrap());
    }
    c.holds(
        "A1 Hex8 part",
        same_part(&p, &alone[0], 0),
        "bit-equal to the Hex8 alone",
    );
    c.holds(
        "A1 Shell4 part",
        same_part(&p, &alone[1], 1),
        "bit-equal to the Shell4 alone",
    );
    println!(
        "A1 Hex8 point 0 stress {:?}; Shell4 stress {:?}",
        p.of_element(0)[0],
        p.of_element(1)[0]
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ T1, T2: tension

const C_BLOCK: f64 = 0.0025;
const H_BLOCK: f64 = 0.01;

/// T1: the beam of B3 (ADR 0473) carried at `x = 0` by a Hex20 block through the tension.
#[test]
fn t1_m1_m3_n1_beam_carried_by_a_block() {
    let mut c = Checks::new();
    let (l, b, nb) = (1.0, 0.005, 16usize);
    let section: BeamSection = beam::rectangle_section(b, b, 0.1406 * b.powi(4), KAPPA).unwrap();
    let ei = E * section.i_z();
    let ra = RHO * section.area();
    let t = PI * PI * ei / (l * l);
    let a = upgrade_to_hex20(
        &hex_box(
            [-C_BLOCK, -H_BLOCK / 2.0, -H_BLOCK / 2.0],
            [C_BLOCK, H_BLOCK, H_BLOCK],
            [1, 2, 2],
        )
        .unwrap(),
    )
    .unwrap()
    .0;
    let nodes: Vec<Vec3> = (0..=nb).map(|i| [l * i as f64 / nb as f64, 0.0, 0.0]).collect();
    let mut bm = SolidMesh::new(nodes).unwrap();
    bm.add_block(ElementKind::Beam2, (0..nb).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    bm.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    bm.add_element_set("all", (0..nb).collect()).unwrap();
    let mut mesh = tie::assemble_parts(&[("A", &a), ("B", &bm)]).unwrap();
    let na = a.nodes().len();
    let all = mesh.nodes().len();
    add_set(&mut mesh, "blk", 0..na, |_| true);
    add_set(&mut mesh, "hinge", 0..na, |p| near(p[0], 0.0) && near(p[1], 0.0));
    add_set(&mut mesh, "far", 0..na, |p| near(p[0], -C_BLOCK) && near(p[1], 0.0));
    add_set(&mut mesh, "bfree", na + 1..all, |_| true);
    add_set(&mut mesh, "bl", na + nb..all, |_| true);
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", steel(0.0)).unwrap();
    m.add_material("B.all", steel(0.3)).unwrap();
    m.add_constraint(Constraint::displacement("blk", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("hinge", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("far", 0, 0.0)).unwrap();
    for comp in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("bfree", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("bl", 1, 0.0)).unwrap();
    let rows = tie::beam_to_solid_with(&m, na, "A.xmax", JointCoupling::Consistent).unwrap();
    add_rows(&mut m, rows);
    let u = solve(&m, &[NodalForceAt::new(na + nb, [t, 0.0, 0.0])]);
    let pre = Prestress::from_static(&m, &u).unwrap();
    let res = prestressed_modes(&m, &pre, &opts(4, 1e-12)).unwrap();
    let mass = rcfd_fem::mass::consistent(&m).unwrap();
    let lateral: Vec<f64> = (0..res.modes.len())
        .filter(|&i| share(&m, &mass, &res.modes[i], 1) > 0.5)
        .map(|i| res.eigenvalues[i])
        .collect();
    println!("T1 eigenvalues {:?}, lateral {lateral:?}", res.eigenvalues);
    for n in 1..=2 {
        let k = n as f64 * PI / l;
        let formula = (k.powi(4) * ei + k * k * t) / ra;
        let lam = lateral[n - 1];
        println!(
            "T1 n = {n}: omega^2 = {lam:.10e}, formula {formula:.10e}, signed {:.4e}",
            lam / formula - 1.0
        );
        c.at_most(
            &format!("T1 n = {n} |omega^2/formula - 1|"),
            (lam / formula - 1.0).abs(),
            1e-2,
        );
    }
    let zero = prestressed_modes(&m, &Prestress::uniform(&m, [0.0; 6]).unwrap(), &opts(4, 1e-12)).unwrap();
    let k = PI / l;
    let f1 = (k.powi(4) * ei + k * k * t) / ra;
    c.above(
        "N1 T1 zero pre-stress |omega_1^2/formula - 1|",
        (zero.eigenvalues[0] / f1 - 1.0).abs(),
        5e-2,
    );
    m1_m3(&mut c, "T1", &m, &pre, &u, [(&a, steel(0.0)), (&bm, steel(0.3))]);
    c.finish();
}

/// T2, T2b: the strip of S1 (ADR 0473) carried at `x = 0` by a Hex20 block through the tension.
fn strip_carried(c: &mut Checks, tag: &str, tri: bool, exact: bool) {
    let (l, b, t, nx) = (1.0, 0.05, 0.005, 40usize);
    let ei = E * b * t * t * t / 12.0;
    let ra = RHO * b * t;
    let tension = PI * PI * ei / (l * l);
    let a = upgrade_to_hex20(&hex_box([-C_BLOCK, 0.0, -H_BLOCK / 2.0], [C_BLOCK, b, H_BLOCK], [1, 2, 2]).unwrap())
        .unwrap()
        .0;
    let sm = shell_grid(0.0, 0.0, l, b, nx, 2, t, tri);
    let mut mesh = tie::assemble_parts(&[("A", &a), ("B", &sm)]).unwrap();
    let na = a.nodes().len();
    let all = mesh.nodes().len();
    add_set(&mut mesh, "hinge", 0..na, |p| near(p[0], 0.0) && near(p[2], 0.0));
    add_set(&mut mesh, "far", 0..na, |p| near(p[0], -C_BLOCK) && near(p[2], 0.0));
    add_set(&mut mesh, "fy", 0..na, |p| {
        near(p[0], -C_BLOCK) && near(p[1], 0.0) && near(p[2], 0.0)
    });
    add_set(&mut mesh, "x0", na..all, |p| near(p[0], 0.0));
    let xl = add_set(&mut mesh, "xl", na..all, |p| near(p[0], l));
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", steel(0.0)).unwrap();
    m.add_material("B.all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("hinge", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("far", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("fy", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xl", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xl", 3, 0.0)).unwrap();
    let rows = tie::shell_to_solid_with(&m, "x0", "A.xmax", JointCoupling::Consistent).unwrap();
    add_rows(&mut m, rows);
    let forces: Vec<NodalForceAt> = xl
        .iter()
        .map(|&n| {
            let y = m.mesh().nodes()[n][1];
            let w = if near(y, 0.0) || near(y, b) { 0.25 } else { 0.5 };
            NodalForceAt::new(n, [w * tension, 0.0, 0.0])
        })
        .collect();
    let u = solve(&m, &forces);
    let pre = Prestress::from_static(&m, &u).unwrap();
    let res = prestressed_modes(&m, &pre, &opts(6, 1e-12)).unwrap();
    let mass = rcfd_fem::mass::consistent(&m).unwrap();
    let bending: Vec<f64> = (0..res.modes.len())
        .filter(|&i| share(&m, &mass, &res.modes[i], 2) > 0.5)
        .map(|i| res.eigenvalues[i])
        .collect();
    println!("{tag} eigenvalues {:?}, bending {bending:?}", res.eigenvalues);
    for n in 1..=2 {
        let k = n as f64 * PI / l;
        let formula = (k.powi(4) * ei + k * k * tension) / ra;
        let lam = bending.get(n - 1).copied().unwrap_or(f64::NAN);
        println!(
            "{tag} n = {n}: omega^2 = {lam:.10e}, formula {formula:.10e}, signed {:.4e}",
            lam / formula - 1.0
        );
        let d = (lam / formula - 1.0).abs();
        c.at_most(
            &format!("{tag} n = {n} |omega^2/formula - 1|"),
            if d.is_nan() { f64::INFINITY } else { d },
            1e-2,
        );
    }
    if exact {
        let zero = prestressed_modes(&m, &Prestress::uniform(&m, [0.0; 6]).unwrap(), &opts(6, 1e-12)).unwrap();
        let first = (0..zero.modes.len())
            .find(|&i| share(&m, &mass, &zero.modes[i], 2) > 0.5)
            .map(|i| zero.eigenvalues[i])
            .unwrap_or(f64::NAN);
        let k = PI / l;
        let f1 = (k.powi(4) * ei + k * k * tension) / ra;
        c.above(
            "N1 T2 zero pre-stress |omega_1^2/formula - 1|",
            (first / f1 - 1.0).abs(),
            5e-2,
        );
        m1_m3(c, tag, &m, &pre, &u, [(&a, steel(0.0)), (&sm, steel(0.0))]);
    }
}

#[test]
fn t2_t2b_m1_m3_n1_strip_carried_by_a_block() {
    let mut c = Checks::new();
    strip_carried(&mut c, "T2 Shell4 40 x 2", false, true);
    strip_carried(&mut c, "T2b Shell3 40 x 2 x 2", true, false);
    c.finish();
}

// ------------------------------------------------------------------------------------------- T3: the stiffened plate

const A: f64 = 1.0;
const TP: f64 = 0.02;
const Y1: f64 = 0.46875;
const Y2: f64 = 0.53125;
const ZB: f64 = -0.05;
const SIGMA: f64 = 1.0e6;
const NX: usize = 16;
const NY: usize = 8;

/// The rib: Hex20 16 x 1 x 3 on `[0, a] x [Y1, Y2] x [ZB, t/2]`, with the surface sets of its top layer's side faces.
fn rib() -> SolidMesh {
    let mut r = upgrade_to_hex20(&hex_box([0.0, Y1, ZB], [A, Y2 - Y1, TP / 2.0 - ZB], [NX, 1, 3]).unwrap())
        .unwrap()
        .0;
    let top = |e: usize| e / NX == 2;
    sub_surface(&mut r, "ymin_top", "ymin", top);
    sub_surface(&mut r, "ymax_top", "ymax", top);
    r
}

struct Plate {
    model: SolidModel,
    forces: Vec<NodalForceAt>,
    /// The number of solid elements (the first ones).
    solid_elements: usize,
    /// The rib's bottom node at mid-span on the centre line.
    rib_node: usize,
    /// The plate's nodes (shell nodes, or the Hex20 plate halves).
    plate_nodes: std::ops::Range<usize>,
}

fn rib_end_loads(mesh: &SolidMesh, prefix: &str, out: &mut Vec<NodalForceAt>) {
    out.extend(traction_loads(mesh, &format!("{prefix}.xmin"), [SIGMA, 0.0, 0.0]).unwrap());
    out.extend(traction_loads(mesh, &format!("{prefix}.xmax"), [-SIGMA, 0.0, 0.0]).unwrap());
}

fn find(mesh: &SolidMesh, range: std::ops::Range<usize>, p: Vec3) -> usize {
    range
        .into_iter()
        .find(|&n| {
            let q = mesh.nodes()[n];
            near(q[0], p[0]) && near(q[1], p[1]) && near(q[2], p[2])
        })
        .unwrap_or_else(|| panic!("no node at {p:?}"))
}

fn plate_mixed() -> Plate {
    let r = rib();
    let p = shell_grid(0.0, 0.0, A, Y1, NX, NY, TP, false);
    let q = shell_grid(0.0, Y2, A, 1.0 - Y2, NX, NY, TP, false);
    let mut mesh = tie::assemble_parts(&[("R", &r), ("P", &p), ("Q", &q)]).unwrap();
    let nr = r.nodes().len();
    let np = p.nodes().len();
    let all = mesh.nodes().len();
    add_set(&mut mesh, "pedge", nr..nr + np, |x| near(x[1], Y1));
    add_set(&mut mesh, "qedge", nr + np..all, |x| near(x[1], Y2));
    add_set(&mut mesh, "outer", nr..all, |x| {
        (near(x[0], 0.0) || near(x[0], A) || near(x[1], 0.0) || near(x[1], 1.0)) && !near(x[1], Y1) && !near(x[1], Y2)
    });
    let c0 = find(&mesh, nr..nr + np, [0.0, 0.0, 0.0]);
    let c1 = find(&mesh, nr..nr + np, [A, 0.0, 0.0]);
    mesh.add_node_set("c0", vec![c0]).unwrap();
    mesh.add_node_set("c1", vec![c1]).unwrap();
    let rib_node = find(&mesh, 0..nr, [A / 2.0, 0.5, ZB]);
    let mut forces = Vec::new();
    rib_end_loads(&mesh, "R", &mut forces);
    let h = Y1 / NY as f64;
    let n = SIGMA * TP;
    for n0 in nr..all {
        let x = mesh.nodes()[n0];
        let end = near(x[1], 0.0) || near(x[1], Y1) || near(x[1], Y2) || near(x[1], 1.0);
        let w = if end { 0.5 } else { 1.0 } * n * h;
        if near(x[0], 0.0) {
            forces.push(NodalForceAt::new(n0, [w, 0.0, 0.0]));
        } else if near(x[0], A) {
            forces.push(NodalForceAt::new(n0, [-w, 0.0, 0.0]));
        }
    }
    let solid_elements = r.n_elements();
    let mut m = SolidModel::new(mesh);
    for s in ["R.all", "P.all", "Q.all"] {
        m.add_material(s, steel(0.0)).unwrap();
    }
    for s in ["outer", "R.xmin", "R.xmax"] {
        m.add_constraint(Constraint::displacement(s, 2, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("c0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c1", 1, 0.0)).unwrap();
    let rows = tie::shell_to_solid_with(&m, "pedge", "R.ymin_top", JointCoupling::Consistent).unwrap();
    add_rows(&mut m, rows);
    let rows = tie::shell_to_solid_with(&m, "qedge", "R.ymax_top", JointCoupling::Consistent).unwrap();
    add_rows(&mut m, rows);
    Plate {
        model: m,
        forces,
        solid_elements,
        rib_node,
        plate_nodes: nr..all,
    }
}

fn plate_solid() -> Plate {
    let r = rib();
    let p = upgrade_to_hex20(&hex_box([0.0, 0.0, -TP / 2.0], [A, Y1, TP], [NX, NY, 1]).unwrap())
        .unwrap()
        .0;
    let q = upgrade_to_hex20(&hex_box([0.0, Y2, -TP / 2.0], [A, 1.0 - Y2, TP], [NX, NY, 1]).unwrap())
        .unwrap()
        .0;
    let mut mesh = tie::assemble_parts(&[("R", &r), ("P", &p), ("Q", &q)]).unwrap();
    let nr = r.nodes().len();
    let np = p.nodes().len();
    let all = mesh.nodes().len();
    add_set(&mut mesh, "outer", nr..all, |x| {
        (near(x[0], 0.0) || near(x[0], A) || near(x[1], 0.0) || near(x[1], 1.0)) && !near(x[1], Y1) && !near(x[1], Y2)
    });
    let c0 = find(&mesh, nr..nr + np, [0.0, 0.0, 0.0]);
    let c1 = find(&mesh, nr..nr + np, [A, 0.0, 0.0]);
    mesh.add_node_set("c0", vec![c0]).unwrap();
    mesh.add_node_set("c1", vec![c1]).unwrap();
    let rib_node = find(&mesh, 0..nr, [A / 2.0, 0.5, ZB]);
    let mut forces = Vec::new();
    for prefix in ["R", "P", "Q"] {
        rib_end_loads(&mesh, prefix, &mut forces);
    }
    let mut m = SolidModel::new(mesh);
    for s in ["R.all", "P.all", "Q.all"] {
        m.add_material(s, steel(0.0)).unwrap();
    }
    for s in ["outer", "R.xmin", "R.xmax"] {
        m.add_constraint(Constraint::displacement(s, 2, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("c0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c1", 1, 0.0)).unwrap();
    let o = TieOptions::new(TieMethod::NodeToSurface);
    tie::add_tie(&mut m, "P.ymax", "R.ymin_top", &o).unwrap();
    tie::add_tie(&mut m, "Q.ymin", "R.ymax_top", &o).unwrap();
    let solid_elements = m.mesh().n_elements();
    Plate {
        model: m,
        forces,
        solid_elements,
        rib_node,
        plate_nodes: nr..all,
    }
}

fn uniformity(p: &Prestress) -> f64 {
    p.stress()
        .iter()
        .map(|s| {
            let mut d: f64 = (s[0] / SIGMA + 1.0).abs();
            for v in &s[1..] {
                d = d.max((v / SIGMA).abs());
            }
            d
        })
        .fold(0.0, f64::max)
}

/// (lowest factor, whether the lowest mode is the overall one, the pre-stress).
fn plate_factor(c: &mut Checks, tag: &str, p: &Plate) -> (f64, Prestress) {
    let u = solve(&p.model, &p.forces);
    let pre = Prestress::from_static(&p.model, &u).unwrap();
    c.at_most(&format!("T3u {tag} uniform pre-stress"), uniformity(&pre), 1e-8);
    let r = factors(&p.model, &pre, &BucklingOptions::new(2)).unwrap();
    println!("T3 {tag} factors {:?}", r.factors);
    let dofs = p.model.dof_map();
    let uz = |phi: &[f64], n: usize| phi[dofs.equation(n, 2).unwrap()].abs();
    let phi = &r.modes[0];
    let plate_max = p.plate_nodes.clone().map(|n| uz(phi, n)).fold(0.0, f64::max);
    let rib = uz(phi, p.rib_node);
    c.holds(
        &format!("T3m {tag} overall mode"),
        rib >= 0.5 * plate_max,
        &format!("rib |u_z| {rib:.4e}, plate max {plate_max:.4e}"),
    );
    (r.factors[0], pre)
}

#[test]
fn t3_t3u_t3m_n2_n3_plate_stiffened_by_a_solid_rib() {
    let mut c = Checks::new();
    let mixed = plate_mixed();
    let solid = plate_solid();
    let (lm, pre) = plate_factor(&mut c, "mixed", &mixed);
    let (ls, _) = plate_factor(&mut c, "solid", &solid);
    println!(
        "T3 lambda_mixed {lm:.10e}, lambda_solid {ls:.10e}, sigma_cr {:.6e} Pa, signed {:.4e}",
        lm * SIGMA,
        lm / ls - 1.0
    );
    c.at_most("T3 |lambda_mixed/lambda_solid - 1|", (lm / ls - 1.0).abs(), 3e-2);
    let ns = mixed.solid_elements;
    for (name, keep_solid) in [
        ("N2 rib pre-stress masked", false),
        ("N3 shell pre-stress masked", true),
    ] {
        let masked = pre.masked(|e| (e < ns) == keep_solid);
        let r = factors(&mixed.model, &masked, &BucklingOptions::new(2)).unwrap();
        let l = r.factors.first().copied().unwrap_or(f64::INFINITY);
        println!("{name}: factors {:?}", r.factors);
        c.above(&format!("{name} |lambda/lambda_solid - 1|"), (l / ls - 1.0).abs(), 3e-2);
    }
    c.finish();
}

// --------------------------------------------------------------------------------------- H1 to H3: the rotating hub

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const SPEEDS: [f64; 4] = [0.0, 120.0, 240.0, 360.0];
const TB: f64 = 0.001;
const HUB_H: f64 = 0.01;
const R1: f64 = 0.05;
const SPAN: f64 = 0.15;
const CHORD: f64 = 0.05;

/// The hub: Hex20 2 x 12 x 3 on `[0.02, R1] x [-c/2, c/2] x [-h/2, h/2]`, the middle layer `[-t/2, t/2]`; the surface set
/// `xmax_mid` of its middle-layer faces at `x = R1`.
fn hub() -> SolidMesh {
    let base = hex_box([0.02, -CHORD / 2.0, -1.5], [R1 - 0.02, CHORD, 3.0], [2, 12, 3]).unwrap();
    let map = |p: Vec3| {
        let z = p[2];
        let zz = if z.abs() <= 0.5 {
            z * TB
        } else {
            z.signum() * (TB / 2.0 + (z.abs() - 0.5) * (HUB_H / 2.0 - TB / 2.0))
        };
        [p[0], p[1], zz]
    };
    let mut h = upgrade_to_hex20_mapped(&base, map).unwrap().0;
    sub_surface(&mut h, "xmax_mid", "xmax", |e| e / 24 == 1);
    h
}

fn hub_model(blade: &SolidMesh, shell: bool) -> SolidModel {
    let h = hub();
    let mut mesh = tie::assemble_parts(&[("A", &h), ("B", blade)]).unwrap();
    let nh = h.nodes().len();
    let all = mesh.nodes().len();
    if shell {
        add_set(&mut mesh, "root", nh..all, |p| near(p[0], R1));
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", steel(0.3)).unwrap();
    m.add_material("B.all", steel(0.3)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("A.xmin", comp, 0.0)).unwrap();
    }
    if shell {
        let rows = tie::shell_to_solid_with(&m, "root", "A.xmax_mid", JointCoupling::Consistent).unwrap();
        add_rows(&mut m, rows);
    } else {
        tie::add_tie(
            &mut m,
            "B.xmin",
            "A.xmax_mid",
            &TieOptions::new(TieMethod::NodeToSurface),
        )
        .unwrap();
    }
    m
}

fn table(m: &SolidModel, stiffening: bool) -> CampbellTable {
    let o = RotatingOptions::new(6)
        .modal(opts(6, 1e-10))
        .stress_stiffening(stiffening);
    RotatingPart::new(m, AXIS, ORIGIN, &o)
        .unwrap()
        .campbell(&SPEEDS)
        .unwrap()
}

fn print_table(name: &str, t: &CampbellTable) {
    for (k, tr) in t.tracks.iter().enumerate().take(5) {
        let f: Vec<String> = tr
            .frequencies
            .iter()
            .map(|v| format!("{:.6}", v.unwrap_or(f64::NAN)))
            .collect();
        println!("{name} track {k} Hz: {}", f.join(", "));
    }
}

fn h3(c: &mut Checks, name: &str, t: &CampbellTable) {
    let mut ok = t.losses.is_empty();
    let mut low: f64 = 1.0;
    for tr in t.tracks.iter().take(4) {
        for m in tr.mac.iter().skip(1) {
            let v = m.unwrap_or(0.0);
            low = low.min(v);
            ok &= v > 0.9;
        }
    }
    c.holds(
        &format!("H3 {name}"),
        ok,
        &format!("losses {}, lowest MAC {low:.6}", t.losses.len()),
    );
}

fn freq(t: &CampbellTable, k: usize, j: usize) -> f64 {
    t.tracks[k].frequencies[j].unwrap_or(f64::NAN)
}

#[test]
fn h1_h3_n4_hub_with_a_shell_blade_rotating() {
    let mut c = Checks::new();
    let shell = shell_grid(R1, -CHORD / 2.0, SPAN, CHORD, 36, 12, TB, false);
    let solid = upgrade_to_hex20(&hex_box([R1, -CHORD / 2.0, -TB / 2.0], [SPAN, CHORD, TB], [36, 12, 1]).unwrap())
        .unwrap()
        .0;
    let mm = hub_model(&shell, true);
    let ms = hub_model(&solid, false);
    let tm = table(&mm, true);
    let ts = table(&ms, true);
    print_table("mixed", &tm);
    print_table("solid", &ts);
    let mut worst: f64 = 0.0;
    for k in 0..4 {
        for j in 0..SPEEDS.len() {
            let d = (freq(&tm, k, j) / freq(&ts, k, j) - 1.0).abs();
            let d = if d.is_nan() { f64::INFINITY } else { d };
            println!("H1 track {k} speed {}: {d:.4e}", SPEEDS[j]);
            worst = worst.max(d);
        }
    }
    c.at_most("H1 max |omega_mixed/omega_solid - 1|", worst, 2e-2);
    let inc = |t: &CampbellTable| freq(t, 0, 3).powi(2) - freq(t, 0, 0).powi(2);
    let (im, is) = (inc(&tm), inc(&ts));
    println!("H2 increments (Hz^2) mixed {im:.8e}, solid {is:.8e}");
    let d = (im / is - 1.0).abs();
    c.at_most(
        "H2 stiffening increment of track 1",
        if d.is_nan() { f64::INFINITY } else { d },
        3e-2,
    );
    h3(&mut c, "mixed", &tm);
    h3(&mut c, "solid", &ts);
    let off = table(&mm, false);
    let d = (freq(&off, 0, 3) / freq(&ts, 0, 3) - 1.0).abs();
    c.above("N4 no stress stiffening, track 1 at 360 rad/s", d, 5e-2);
    c.finish();
}
