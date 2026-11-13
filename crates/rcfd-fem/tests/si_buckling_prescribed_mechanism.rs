//! F-496 (ADR 0402): `buckling::factors` on the prescribed-displacement path refuses a `K_FF` whose zero-shift pivot ratio is
//! below `1e-8` (the rule of ADR 0401), naming the unknown of the failing pivot, on ADR 0401's two-block Hex8 hinge held by
//! `Displacement` rows (which passes the rigid-motion test). Checks A1 to A3, C1 to C3 and the negative controls N1, N2 of the
//! registration (A4 and C4 are runs of existing test files); the models of ADR 0390 and ADR 0394 for the bit-for-bit hashes.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt, SkylineOptions};

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name}: {value:e} >= {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name}: {value:e} <= {limit:e}"));
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

// ------------------------------------------------------------------------------------------------- set-ups (D5)

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const H: f64 = 0.1;
const B: f64 = 0.1;
const TOL: f64 = 1e-9;
/// The rule of ADR 0384 (ADR 0283 D3).
const RULE: f64 = 1e-8;
/// The hinge's block edge (m).
const A_HINGE: f64 = 0.1;
/// The reference compression of the hinge (Pa).
const S_REF: f64 = -1e6;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

fn nodes_where(mesh: &SolidMesh, f: impl Fn(Vec3) -> bool) -> Vec<usize> {
    mesh.nodes()
        .iter()
        .enumerate()
        .filter(|(_, &p)| f(p))
        .map(|(i, _)| i)
        .collect()
}

/// ADR 0401's hinge on the prescribed path: Hex8 blocks `[0, a]^3` and `[a, 2a] x [a, 2a] x [0, a]` sharing the edge `x = y
/// = a`; `a0` held by `Displacement` rows in the components `held` (all three for HP, `x` and `y` for HZ); `bfar` (block B's
/// far edge) held in `y` when `held_far` (HD).
fn hinge(held: &[usize], held_far: bool) -> SolidModel {
    let a = A_HINGE;
    let mut nodes: Vec<Vec3> = Vec::new();
    let mut ia = [[[0usize; 2]; 2]; 2];
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..2 {
                ia[i][j][k] = nodes.len();
                nodes.push([a * i as f64, a * j as f64, a * k as f64]);
            }
        }
    }
    let mut ib = [[[0usize; 2]; 2]; 2];
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..2 {
                ib[i][j][k] = if i == 0 && j == 0 {
                    ia[1][1][k]
                } else {
                    nodes.push([a + a * i as f64, a + a * j as f64, a * k as f64]);
                    nodes.len() - 1
                };
            }
        }
    }
    let hex = |n: &[[[usize; 2]; 2]; 2]| {
        vec![
            n[0][0][0], n[1][0][0], n[1][1][0], n[0][1][0], n[0][0][1], n[1][0][1], n[1][1][1], n[0][1][1],
        ]
    };
    let mut conn = hex(&ia);
    conn.extend(hex(&ib));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("a0", vec![ia[0][0][0], ia[0][1][0], ia[0][0][1], ia[0][1][1]])
        .unwrap();
    mesh.add_node_set("bfar", vec![ib[1][1][0], ib[1][1][1]]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for &c in held {
        m.add_constraint(Constraint::displacement("a0", c, 0.0)).unwrap();
    }
    if held_far {
        m.add_constraint(Constraint::displacement("bfar", 1, 0.0)).unwrap();
    }
    m
}

fn hinge_prestress(m: &SolidModel) -> Prestress {
    Prestress::uniform(m, [S_REF, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

/// The hinge rotation: `u = (-(y - a), x - a, 0)` on the nodes of block B off `A`, zero on `A`.
fn hinge_field(p: Vec3, c: usize) -> f64 {
    let on_a = p[0] <= A_HINGE + TOL && p[1] <= A_HINGE + TOL;
    if on_a {
        0.0
    } else {
        [-(p[1] - A_HINGE), p[0] - A_HINGE, 0.0][c]
    }
}

/// ADR 0390's beam B (`si_geometric.rs`): `[40, 2, 1]`, Hex20 or Hex8, pin and roller knife edges, `u_z = 0`.
fn beam_0390(s: f64, hex20: bool) -> SolidModel {
    let l = 2.0;
    let base = hex_box([0.0; 3], [l, H, B], [40, 2, 1]).unwrap();
    let mut mesh = if hex20 {
        upgrade_to_hex20(&base).unwrap().0
    } else {
        base
    };
    knife_edges(&mut mesh, l);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke1", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [s, 0.0, 0.0])).unwrap();
    m.add_load(Load::traction("xmin", [-s, 0.0, 0.0])).unwrap();
    m
}

/// ADR 0390's column C: `[40, 2, 2]` Hex20, clamped at `x = 0`, the compression `-s` at `x = L`.
fn column_0390(s: f64) -> SolidModel {
    let l = 2.0;
    let base = hex_box([0.0; 3], [l, H, B], [40, 2, 2]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    knife_edges(&mut mesh, l);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [-s, 0.0, 0.0])).unwrap();
    m
}

/// ADR 0394's knife-edge beam (`si_knife_edge_closed_form.rs`): Hex20 `[nx, 2, 1]` of length `length`.
fn beam_0394(length: f64, nx: usize, s: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [length, H, B], [nx, 2, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    knife_edges(&mut mesh, length);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke1", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [s, 0.0, 0.0])).unwrap();
    m.add_load(Load::traction("xmin", [-s, 0.0, 0.0])).unwrap();
    m
}

fn knife_edges(mesh: &mut SolidMesh, length: f64) {
    let ke0 = nodes_where(mesh, |p| p[0].abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    let ke1 = nodes_where(mesh, |p| (p[0] - length).abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    let every = (0..mesh.nodes().len()).collect();
    mesh.add_node_set("ke0", ke0).unwrap();
    mesh.add_node_set("ke1", ke1).unwrap();
    mesh.add_node_set("every", every).unwrap();
}

fn static_prestress(model: &SolidModel) -> Prestress {
    let r = solve_static(model, &StaticOptions::default()).expect("static solve");
    Prestress::from_static(model, &r).expect("pre-stress")
}

// ------------------------------------------------------------------------------------- the factor in the test

/// The zero-shift factor of `K_FF` computed here from public functions: the first pivot (elimination order) with ratio
/// below the rule or refused, as (node, component, row of `K_FF`), and the smallest ratio.
struct InTest {
    first: Option<(usize, usize, usize)>,
    min_ratio: f64,
}

fn in_test(m: &SolidModel) -> InTest {
    let fixed = prescribed_equations(m, &[]).unwrap();
    let k = stiffness(m).unwrap();
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed).unwrap();
    let d = kr.matrix.diag();
    let (row, min_ratio) = match SkylineLdlt::factor_with(&kr.matrix, &SkylineOptions::default()) {
        Ok(f) => {
            let mut first = None;
            let mut min = f64::INFINITY;
            for (&p, &o) in f.pivots().iter().zip(f.order()) {
                let r = p / d[o];
                if first.is_none() && (r.is_nan() || r < RULE) {
                    first = Some(o);
                }
                min = min.min(r);
            }
            (first, min)
        }
        Err(SkylineError::Pivot { row, pivot, .. }) => (Some(row), pivot / d[row]),
        Err(e) => panic!("factor: {e}"),
    };
    let first = row.map(|r| {
        let (node, c) = m.dof_map().node_component(kr.free()[r]).expect("free equation");
        (node, c, r)
    });
    InTest { first, min_ratio }
}

/// "Refused as registered" (ADR 0402): `Solver { lowest_buckling_factors }`, the reason starting with the name computed in
/// the test and containing "pivot ratio" (identity), and the named unknown moving in the hinge field (meaning).
fn refused_as_registered(c: &mut Checks, id: &str, m: &SolidModel, r: &rcfd_fem::Result<BucklingResult>) -> InTest {
    let t = in_test(m);
    c.below(&format!("{id} smallest pivot ratio of K_FF"), t.min_ratio, RULE);
    let reason = match r {
        Err(FemError::Solver {
            solver: "lowest_buckling_factors",
            reason,
        }) => Some(reason.clone()),
        _ => None,
    };
    println!("{id}: {:?}", r.as_ref().map(|b| b.factors.clone()));
    let Some((node, comp, _)) = t.first else {
        c.holds(&format!("{id} failing pivot in the test"), false, "none found");
        return t;
    };
    let name = format!("node {node} component {comp}");
    let identity = reason
        .as_deref()
        .is_some_and(|s| s.starts_with(&format!("{name}: ")) && s.contains("pivot ratio"));
    c.holds(
        &format!("{id} refused naming the unknown (identity)"),
        identity,
        &format!("expected {name}; got {reason:?}"),
    );
    let mesh = m.mesh();
    let zmax = mesh
        .nodes()
        .iter()
        .flat_map(|&p| (0..3).map(move |k| hinge_field(p, k).abs()))
        .fold(0.0f64, f64::max);
    let z = hinge_field(mesh.nodes()[node], comp).abs() / zmax;
    c.at_most(
        &format!("{id} named unknown moves in the hinge rotation (1e-3 <= ...)"),
        1e-3,
        z,
    );
    t
}

/// FNV-1a (64 bit) of the bits of the factors and of every mode value.
fn hash(r: &BucklingResult) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(r.factors.len() as u64);
    for f in &r.factors {
        eat(f.to_bits());
    }
    for m in &r.modes {
        eat(m.len() as u64);
        for v in m {
            eat(v.to_bits());
        }
    }
    h
}

// ------------------------------------------------------------------------------------------------------- the tests

#[test]
fn a1_a2_c1_n2_hinge_on_the_prescribed_path() {
    let mut c = Checks::new();
    let hp = hinge(&[0, 1, 2], false);
    // A2: the rigid-motion test passes.
    let fixed = prescribed_equations(&hp, &[]).unwrap();
    let rigid = check_rigid_body_motions(&hp, &fixed);
    c.holds(
        "A2 HP passes the rigid-motion test",
        rigid.is_ok(),
        &format!("{rigid:?}"),
    );
    // C1: no free rigid motion.
    let f = free_rigid_motions(&hp, &[]).unwrap();
    c.holds(
        "C1 HP free rigid motions == 0",
        f.count == 0,
        &format!("count {}, singular values {:?}", f.count, f.singular_values),
    );
    // A1: refused, naming the unknown.
    let r = factors(&hp, &hinge_prestress(&hp), &BucklingOptions::new(2));
    if let Err(e) = &r {
        println!("A1 HP error: {e}");
    }
    let t = refused_as_registered(&mut c, "A1 HP", &hp, &r);
    // N2: the failing row taken as a mesh equation names another unknown.
    match t.first {
        Some((node, comp, row)) => {
            let (n2, c2) = hp.dof_map().node_component(row).expect("equation");
            let wrong = format!("node {n2} component {c2}: ");
            let reported = match &r {
                Err(FemError::Solver { reason, .. }) => reason.clone(),
                _ => String::new(),
            };
            c.holds(
                "N2 HP row as a mesh equation differs",
                (n2, c2) != (node, comp) && !reported.starts_with(&wrong),
                &format!("row {row}: node {n2} component {c2} against node {node} component {comp}"),
            );
        }
        None => c.holds("N2 HP failing pivot in the test", false, "none found"),
    }
    c.finish();
}

#[test]
fn c2_n1_order_of_refusals_and_held_hinge() {
    let mut c = Checks::new();
    let hp = hinge(&[0, 1, 2], false);
    let p = hinge_prestress(&hp);
    // C2: the earlier refusals keep their errors.
    let r = factors(&hp, &p, &BucklingOptions::new(0));
    c.holds(
        "C2 HP factors = 0",
        matches!(r, Err(FemError::InvalidValue { key: "factors", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let other = beam_0390(-1e6, true);
    let po = hinge_prestress(&other);
    let r = factors(&hp, &po, &BucklingOptions::new(2));
    c.holds(
        "C2 HP pre-stress of another mesh",
        matches!(r, Err(FemError::FieldLength { key: "prestress", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let hz = hinge(&[0, 1], false);
    let r = factors(&hz, &hinge_prestress(&hz), &BucklingOptions::new(2));
    c.holds(
        "C2 HZ RigidBodyMotionFree",
        matches!(r, Err(FemError::RigidBodyMotionFree { .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    // N1: the hinge held by a Displacement row on block B.
    let hd = hinge(&[0, 1, 2], true);
    let t = in_test(&hd);
    println!("N1 HD smallest pivot ratio of K_FF: {:e}", t.min_ratio);
    let r = factors(&hd, &hinge_prestress(&hd), &BucklingOptions::new(2));
    println!("N1 HD: {:?}", r.as_ref().map(|b| b.factors.clone()));
    c.holds("N1 HD solved", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    c.finish();
}

#[test]
fn a3_c3_bit_for_bit_hashes() {
    let mut c = Checks::new();
    let b = beam_0390(-1e6, true);
    let pb = static_prestress(&b);
    let ten = beam_0390(1e8, true);
    let h8 = beam_0390(-1e6, false);
    let col = column_0390(1e6);
    let b20 = beam_0394(2.0, 40, -1e6);
    let b40 = beam_0394(4.0, 80, -1e6);
    let runs: Vec<(&str, &SolidModel, Prestress, BucklingOptions)> = vec![
        ("B threads 1", &b, pb.clone(), BucklingOptions::new(2).threads(1)),
        ("B threads 8", &b, pb, BucklingOptions::new(2).threads(8)),
        ("tension", &ten, static_prestress(&ten), BucklingOptions::new(2)),
        ("Hex8", &h8, static_prestress(&h8), BucklingOptions::new(2)),
        ("C", &col, static_prestress(&col), BucklingOptions::new(2)),
        ("B20", &b20, static_prestress(&b20), BucklingOptions::new(2)),
        ("B40", &b40, static_prestress(&b40), BucklingOptions::new(2)),
    ];
    for (id, m, p, o) in runs {
        let t = in_test(m);
        c.above(&format!("C3 {id} smallest pivot ratio of K_FF"), t.min_ratio, RULE);
        match factors(m, &p, &o) {
            Ok(r) => {
                println!("A3 hash {id}: {:016x} (factors {:?})", hash(&r), r.factors);
                c.holds(&format!("C3 {id} solved"), true, "Ok");
            }
            Err(e) => c.holds(&format!("C3 {id} solved"), false, &format!("{e:?}")),
        }
    }
    c.finish();
}
