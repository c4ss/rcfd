//! F-495 (ADR 0401): `buckling::factors` on the transformation path refuses a motion the constraints and supports leave free,
//! by the zero-shift pivot-ratio rule of ADR 0384, naming the unknown of the failing pivot. Checks A1 to A3, C1 to C5, the
//! negative controls N1, N2 and the records O1 of the registration (A4 and O2 are runs of `si_prestress_mpc.rs`), on the
//! knife-edge Hex20 beam of ADR 0398 D5 and a two-block Hex8 hinge.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::mpc;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_buckling_factors, BucklingEigenOptions, ResidualTest};
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

// ------------------------------------------------------------------------------------------------- set-ups (D4)

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const L: f64 = 2.0;
const H: f64 = 0.1;
const B: f64 = 0.1;
const AREA: f64 = H * B;
/// The reference compression of the buckling runs (Pa).
const S_REF: f64 = -1e6;
const TOL: f64 = 1e-9;
/// The rule of ADR 0384 (ADR 0283 D3).
const RULE: f64 = 1e-8;
/// The hinge's block edge (m).
const A_HINGE: f64 = 0.1;

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

/// The Hex20 box of ADR 0398's B with the node sets `ke0`, `ke1` (three nodes each) and `every`.
fn beam_mesh() -> SolidMesh {
    let base = hex_box([0.0; 3], [L, H, B], [40, 2, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let ke0 = nodes_where(&mesh, |p| p[0].abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    let ke1 = nodes_where(&mesh, |p| (p[0] - L).abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    assert_eq!((ke0.len(), ke1.len()), (3, 3), "knife-edge nodes");
    let every = (0..mesh.nodes().len()).collect();
    mesh.add_node_set("ke0", ke0).unwrap();
    mesh.add_node_set("ke1", ke1).unwrap();
    mesh.add_node_set("every", every).unwrap();
    mesh
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    /// ADR 0398's B: Displacement knife edges.
    B,
    /// ADR 0398's BM: the pin as Linear rows, no roller.
    Bm,
    /// B without the roller, a spring on the `ke1` nodes in `z` (prescribed equations: it holds nothing).
    Bz,
    /// B without the roller, an axial spring on the `ke1` nodes (not engaged by the free rotation).
    Bx,
    /// B without the roller (the prescribed path).
    Bf,
    /// The roller replaced by a lateral spring of total stiffness `k`.
    Bs(f64),
}

fn beam(kind: Kind, s: f64) -> SolidModel {
    let mut m = SolidModel::new(beam_mesh());
    m.add_material("all", material()).unwrap();
    let ke0 = m.mesh().node_set("ke0").unwrap().to_vec();
    match kind {
        Kind::Bm => {
            for &a in &ke0 {
                m.add_constraint(Constraint::linear(vec![LinearTerm::new(a, 0, 1.0)], 0.0))
                    .unwrap();
                m.add_constraint(Constraint::linear(vec![LinearTerm::new(a, 1, 2.5)], 0.0))
                    .unwrap();
            }
        }
        _ => {
            m.add_constraint(Constraint::displacement("ke0", 0, 0.0)).unwrap();
            m.add_constraint(Constraint::displacement("ke0", 1, 0.0)).unwrap();
            if let Kind::B = kind {
                m.add_constraint(Constraint::displacement("ke1", 1, 0.0)).unwrap();
            }
        }
    }
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    match kind {
        Kind::Bz => m
            .add_support(ElasticSupport::nodal_spring("ke1", 2, 1e6 / 3.0))
            .unwrap(),
        Kind::Bx => m
            .add_support(ElasticSupport::nodal_spring("ke1", 0, 1e6 / 3.0))
            .unwrap(),
        Kind::Bs(k) => m.add_support(ElasticSupport::nodal_spring("ke1", 1, k / 3.0)).unwrap(),
        _ => {}
    }
    m.add_load(Load::traction("xmax", [s, 0.0, 0.0])).unwrap();
    m.add_load(Load::traction("xmin", [-s, 0.0, 0.0])).unwrap();
    m
}

fn static_prestress(model: &SolidModel) -> Prestress {
    let r = solve_static(model, &StaticOptions::default()).expect("static solve");
    Prestress::from_static(model, &r).expect("pre-stress")
}

/// The free rotation about the pin line, `u = (-(y - h/2), x, 0)`.
fn sway(p: Vec3, c: usize) -> f64 {
    [-(p[1] - H / 2.0), p[0], 0.0][c]
}

/// The hinge (D4): Hex8 blocks `[0, a]^3` and `[a, 2a] x [a, 2a] x [0, a]` sharing the edge `x = y = a`; `a0` held in all
/// three components; `spring` adds `nodal_spring("a0", 0, 1e6)` (prescribed equations), `held` adds `nodal_spring("bfar",
/// 1, 1e6)` on block B's far edge.
fn hinge(spring: bool, held: bool) -> SolidModel {
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
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("a0", c, 0.0)).unwrap();
    }
    if spring {
        m.add_support(ElasticSupport::nodal_spring("a0", 0, 1e6)).unwrap();
    }
    if held {
        m.add_support(ElasticSupport::nodal_spring("bfar", 1, 1e6)).unwrap();
    }
    m
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

// ------------------------------------------------------------------------------------- the factor in the test

/// The zero-shift factor of `T^T K_ext T` computed here from public functions: the first pivot (elimination order) with
/// ratio below the rule or refused, as (node, component, row of the reduced matrix), and the smallest ratio.
struct InTest {
    first: Option<(usize, usize, usize)>,
    min_ratio: f64,
}

fn in_test(m: &SolidModel) -> InTest {
    let tr = mpc::transformation(m, &[]).unwrap();
    let mut k = stiffness(m).unwrap();
    mpc::add_supports(&mut k, m).unwrap();
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k, &zero).unwrap();
    let d = kr.diag();
    let (row, min_ratio) = match SkylineLdlt::factor_with(&kr, &SkylineOptions::default()) {
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
        let (node, c) = m.dof_map().node_component(tr.independent()[r]).expect("mesh unknown");
        (node, c, r)
    });
    InTest { first, min_ratio }
}

/// "Refused as registered" (ADR 0401): `Solver { lowest_buckling_factors }`, the reason starting with the name computed in
/// the test and containing "pivot ratio" (identity), and the named unknown moving in the free field (meaning).
fn refused_as_registered(
    c: &mut Checks,
    id: &str,
    m: &SolidModel,
    r: &rcfd_fem::Result<BucklingResult>,
    field: fn(Vec3, usize) -> f64,
) -> InTest {
    let t = in_test(m);
    c.below(&format!("{id} smallest pivot ratio"), t.min_ratio, RULE);
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
        .flat_map(|&p| (0..3).map(move |k| field(p, k).abs()))
        .fold(0.0f64, f64::max);
    let z = field(mesh.nodes()[node], comp).abs() / zmax;
    c.at_most(
        &format!("{id} named unknown moves in the free motion (1e-3 <= ...)"),
        1e-3,
        z,
    );
    t
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// `|lambda |S_REF| A / p - 1|`.
fn off(lambda: f64, p: f64) -> f64 {
    (lambda * S_REF.abs() * AREA / p - 1.0).abs()
}

// ------------------------------------------------------------------------------------------------------- the tests

#[test]
fn a1_a2_c2_c4_n1_mechanisms_of_the_beam() {
    let mut c = Checks::new();
    let bmodel = beam(Kind::B, S_REF);
    let pb = static_prestress(&bmodel);
    // A1: BM.
    let bm = beam(Kind::Bm, S_REF);
    let r = factors(&bm, &pb, &BucklingOptions::new(2));
    let t = refused_as_registered(&mut c, "A1 BM", &bm, &r, sway);
    // N1: the failing row taken as a mesh equation.
    if let Some((node, comp, row)) = t.first {
        let (n2, c2) = bm.dof_map().node_component(row).expect("equation");
        c.holds(
            "N1 BM row as a mesh equation differs",
            (n2, c2) != (node, comp),
            &format!("row {row}: node {n2} component {c2} against node {node} component {comp}"),
        );
    }
    // C4: the rigid count agrees on BM.
    let f = free_rigid_motions(&bm, &[]).unwrap();
    c.holds(
        "C4 BM free rigid motions == 1",
        f.count == 1,
        &format!("count {}, singular values {:?}", f.count, f.singular_values),
    );
    // A2: BZ, BX; BF on the old path.
    for (id, kind) in [("A2 BZ", Kind::Bz), ("A2 BX", Kind::Bx)] {
        let m = beam(kind, S_REF);
        let r = factors(&m, &pb, &BucklingOptions::new(2));
        refused_as_registered(&mut c, id, &m, &r, sway);
    }
    let bf = beam(Kind::Bf, S_REF);
    let r = factors(&bf, &pb, &BucklingOptions::new(2));
    c.holds(
        "A2 BF still RigidBodyMotionFree",
        matches!(r, Err(FemError::RigidBodyMotionFree { .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    // C2: a spring too soft for the rule (a false refusal).
    let soft = beam(Kind::Bs(1e-4), S_REF);
    let r = factors(&soft, &pb, &BucklingOptions::new(2));
    refused_as_registered(&mut c, "C2 BS(1e-4)", &soft, &r, sway);
    c.finish();
}

#[test]
fn a3_c1_c5_soft_spring_solved() {
    let mut c = Checks::new();
    let bmodel = beam(Kind::B, S_REF);
    let pb = static_prestress(&bmodel);
    let rb = factors(&bmodel, &pb, &BucklingOptions::new(2)).expect("B buckling");
    println!("B: factors {:?}", rb.factors);
    let k = 1e4;
    let m = beam(Kind::Bs(k), S_REF);
    let p = static_prestress(&m);
    let r = factors(&m, &p, &BucklingOptions::new(2));
    let t = in_test(&m);
    c.above("A3 BS(1e4) smallest pivot ratio", t.min_ratio, RULE);
    let r = match r {
        Ok(r) => r,
        Err(e) => {
            c.holds("A3 BS(1e4) solved", false, &format!("{e:?}"));
            c.finish();
            return;
        }
    };
    println!(
        "BS(1e4): factors {:?}, iterations {}, residuals {:?}, Sturm {:?}, unknowns {}",
        r.factors, r.iterations, r.residuals, r.sturm, r.unknowns
    );
    c.holds(
        "A3 BS(1e4) two factors, Sturm",
        r.factors.len() == 2 && r.sturm.counted == 2 && r.sturm.expected == 2,
        &format!(
            "{} factors, counted {}, expected {}",
            r.factors.len(),
            r.sturm.counted,
            r.sturm.expected
        ),
    );
    c.at_most("A3 BS(1e4) |lambda_1 |s| A/(k L) - 1|", off(r.factors[0], k * L), 0.01);
    let bound = k * L / (S_REF.abs() * AREA);
    c.holds(
        "C1 BS(1e4) Rayleigh bound",
        r.factors[0] <= bound * (1.0 + 1e-9),
        &format!("lambda_1 = {:.12} <= {bound:.12}", r.factors[0]),
    );
    c.at_most(
        "C1 BS(1e4) |lambda_2/lambda_1(B) - 1|",
        (r.factors[1] / rb.factors[0] - 1.0).abs(),
        1e-3,
    );
    // C5: bit-equal to the reduction in the test.
    let tr = mpc::transformation(&m, &[]).unwrap();
    let mut kk = stiffness(&m).unwrap();
    mpc::add_supports(&mut kk, &m).unwrap();
    let kg = geometric_stiffness(&m, &p, None).unwrap();
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&kk, &zero).unwrap();
    let (gr, _) = tr.reduce(&kg, &zero).unwrap();
    let opts = BucklingEigenOptions {
        tolerance: 1e-10,
        residual_test: ResidualTest::Backward,
        max_iterations: 200,
        subspace_size: None,
        ..BucklingEigenOptions::default()
    };
    let eig = lowest_buckling_factors(&kr, &gr, 2, &opts).unwrap();
    let mut same = bits(&eig.values) == bits(&r.factors) && eig.vectors.len() == r.modes.len();
    for (q, x) in eig.vectors.iter().zip(&r.modes) {
        same &= bits(&tr.expand(q)[..tr.n_mesh()]) == bits(x);
    }
    c.holds(
        "C5 BS(1e4) bit-equal to the reduction in the test",
        same,
        "factors and modes",
    );
    c.finish();
}

#[test]
fn c3_n2_o1_hinge() {
    let mut c = Checks::new();
    let h = hinge(true, false);
    let p = Prestress::uniform(&h, [S_REF, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let f = free_rigid_motions(&h, &[]).unwrap();
    c.holds(
        "C3 H free rigid motions == 0",
        f.count == 0,
        &format!("count {}, singular values {:?}", f.count, f.singular_values),
    );
    let r = factors(&h, &p, &BucklingOptions::new(2));
    refused_as_registered(&mut c, "C3 H", &h, &r, hinge_field);
    // N2: the hinge held by a spring on block B.
    let hn = hinge(true, true);
    let pn = Prestress::uniform(&hn, [S_REF, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let rn = factors(&hn, &pn, &BucklingOptions::new(2));
    println!("N2 HN: {:?}", rn.as_ref().map(|b| b.factors.clone()));
    c.holds("N2 HN solved", rn.is_ok(), &format!("{:?}", rn.as_ref().err()));
    // O1: the hinge on the prescribed path (recorded).
    let hp = hinge(false, false);
    let pp = Prestress::uniform(&hp, [S_REF, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let rp = factors(&hp, &pp, &BucklingOptions::new(2));
    println!("O1 HP (prescribed path): {:?}", rp.map(|b| b.factors));
    c.finish();
}
