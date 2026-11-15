//! F-601 (ADR 0540 D3): Hex20, Tet4 and Tet10 faces. PT4, PT10, PT20 the patch tests (with EQ1 on them and on Q20), the
//! tangents T1q, T1t3, T1t6, T1g20, T1g10 and the negative controls NT-q and NG, the repeated benchmark HS20 and HS10 (ADR
//! 0507's Hertz problem with the rigid sphere on quadratic meshes), R3's face rows and R1c (the copy of ADR 0507's R1 and R2,
//! ADR 0540 D9). Every limit is the one registered in ADR 0540.

mod f601_common;

use f601_common::*;
use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::contact::{
    solve, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, SearchMethod, Target,
};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::{face_rule, face_shape_functions};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, ElementKind, FaceRef, FemError, Load, SolidMesh, SolidModel};

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

/// Solves with both search methods (EQ1); the Buckets result.
fn both(c: &mut Checks, name: &str, m: &SolidModel, pair: &ContactPair, o: &ContactOptions) -> ContactResult {
    let ra = solve(
        m,
        &[],
        &[],
        std::slice::from_ref(pair),
        &o.clone().search_method(SearchMethod::AllFaces),
    )
    .unwrap();
    let rb = solve(
        m,
        &[],
        &[],
        std::slice::from_ref(pair),
        &o.clone().search_method(SearchMethod::Buckets),
    )
    .unwrap();
    let (wa, wb) = (result_words(&ra), result_words(&rb));
    let equal = wa == wb;
    let finite = result_finite(&ra) && result_finite(&rb);
    c.holds(
        &format!("EQ1 {name}"),
        equal && finite,
        &format!("{} words, equal {equal}, finite {finite}", wa.len()),
    );
    rb
}

/// Every stress component of every integration point against `reference`, the largest difference.
fn stress_error(model: &SolidModel, u: &[f64], reference: [f64; 6]) -> f64 {
    let f = IntegrationPointField::compute(model, u).unwrap();
    worst(
        f.stress()
            .iter()
            .flat_map(|p| (0..6).map(move |i| (p[i] - reference[i]).abs())),
    )
}

// ------------------------------------------------------------------------------------------------------------ PT

#[derive(Clone, Copy, Debug)]
enum Kind {
    Tet4,
    Tet10,
    Hex20,
}

fn patch_mesh(kind: Kind, origin: Vec3) -> SolidMesh {
    match kind {
        Kind::Tet4 => tet_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap(),
        Kind::Tet10 => {
            upgrade_to_tet10(&tet_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap())
                .unwrap()
                .0
        }
        Kind::Hex20 => {
            upgrade_to_hex20(&hex_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap())
                .unwrap()
                .0
        }
    }
}

/// ADR 0540 D5 PT: ADR 0507 B1's blocks of `kind`, pressed by `p` on the upper top.
fn patch_model(kind: Kind, p: f64) -> SolidModel {
    let lower = patch_mesh(kind, [0.0; 3]);
    let upper = patch_mesh(kind, [0.0, 0.0, 1.0]);
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(E_BL, 0.3)).unwrap();
    m.add_material("upper.all", elastic(E_BL, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("lower.zmin", 2, 0.0))
        .unwrap();
    for s in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{s}.xmin"), 0, 0.0))
            .unwrap();
        m.add_constraint(Constraint::displacement(&format!("{s}.ymin"), 1, 0.0))
            .unwrap();
    }
    m.add_load(Load::pressure("upper.zmax", p)).unwrap();
    m
}

fn patch(c: &mut Checks, tag: &str, kind: Kind, points: usize) {
    let p = 1.0e8;
    let m = patch_model(kind, p);
    let pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5);
    let r = both(c, tag, &m, &pair, &ContactOptions::new(2, 1e-13).newton(newton()));
    let u = r.result.displacement_values();
    c.at_most(
        &format!("{tag} stress / p"),
        stress_error(&m, u, [0.0, 0.0, -p, 0.0, 0.0, 0.0]) / p,
        1e-8,
    );
    c.at_most(
        &format!("{tag} point pressure / p"),
        worst(r.nodes.iter().map(|n| (n.pressure - p).abs() / p)),
        1e-8,
    );
    let dofs = m.dof_map();
    let re = r.result.reactions().unwrap();
    let sz: f64 = m
        .mesh()
        .node_set("lower.zmin")
        .unwrap()
        .iter()
        .map(|&n| re[dofs.equation(n, 2).unwrap()])
        .sum();
    c.at_most(&format!("{tag} reaction / p"), (sz - p).abs() / p, 1e-8);
    c.holds(
        &format!("{tag} contactor points"),
        r.nodes.len() == points,
        &format!("{} (registered {points})", r.nodes.len()),
    );
}

#[test]
fn pt4_pt10_pt20_patch_tests() {
    let mut c = Checks::new();
    patch(&mut c, "PT4", Kind::Tet4, 9);
    patch(&mut c, "PT10", Kind::Tet10, 72);
    patch(&mut c, "PT20", Kind::Hex20, 36);
    c.finish();
}

#[test]
fn eq1_q20_punch_on_a_hex20_plate() {
    let mut c = Checks::new();
    let plate = upgrade_to_hex20(&hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap())
        .unwrap()
        .0;
    let pn = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &pn)]).unwrap();
    let m = punch_constraints(mesh, 0.01);
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let r = both(&mut c, "Q20", &m, &pair, &ContactOptions::new(2, 1e-8).newton(newton()));
    println!(
        "Q20: {} contactor nodes, {} active",
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count()
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ T1x

/// Hex20 nodes of a hexahedron with the given corners: the corners, then the mid-edge nodes in the order of `ElementKind::Hex20`.
fn hex20_nodes(corners: &[Vec3; 8]) -> Vec<Vec3> {
    const EDGES: [[usize; 2]; 12] = [
        [0, 1],
        [1, 2],
        [2, 3],
        [3, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let mut v = corners.to_vec();
    for [a, b] in EDGES {
        v.push([
            0.5 * (corners[a][0] + corners[b][0]),
            0.5 * (corners[a][1] + corners[b][1]),
            0.5 * (corners[a][2] + corners[b][2]),
        ]);
    }
    v
}

/// Tet10 nodes: the corners, then the mid-edge nodes of (0,1), (1,2), (0,2), (0,3), (1,3), (2,3).
fn tet10_nodes(corners: &[Vec3; 4]) -> Vec<Vec3> {
    let mut v = corners.to_vec();
    for [a, b] in [[0, 1], [1, 2], [0, 2], [0, 3], [1, 3], [2, 3]] {
        v.push([
            0.5 * (corners[a][0] + corners[b][0]),
            0.5 * (corners[a][1] + corners[b][1]),
            0.5 * (corners[a][2] + corners[b][2]),
        ]);
    }
    v
}

const T1_LOWER: [Vec3; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.05],
    [1.0, 0.0, 0.96],
    [1.0, 1.0, 1.06],
    [0.0, 1.0, 0.97],
];

fn t1_upper() -> [Vec3; 8] {
    let c = T1_CONTACTOR;
    let mut v = [[0.0; 3]; 8];
    for a in 0..4 {
        v[a] = c[a];
        v[a + 4] = [c[a][0], c[a][1], c[a][2] + 1.0];
    }
    v
}

/// A mesh of a target block (first) and a contactor block (second); surface sets "target" and "contactor".
fn two_blocks(target: (ElementKind, Vec<Vec3>, usize), contactor: (ElementKind, Vec<Vec3>, usize)) -> SolidModel {
    let nt = target.1.len();
    let mut nodes = target.1.clone();
    nodes.extend_from_slice(&contactor.1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(target.0, (0..nt).collect()).unwrap();
    mesh.add_block(contactor.0, (nt..nt + contactor.1.len()).collect())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_surface_set(
        "target",
        vec![FaceRef {
            element: 0,
            face: target.2,
        }],
    )
    .unwrap();
    mesh.add_surface_set(
        "contactor",
        vec![FaceRef {
            element: 1,
            face: contactor.2,
        }],
    )
    .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    m
}

fn hex20_target_model() -> SolidModel {
    let mut t = hex20_nodes(&T1_LOWER);
    for (k, dz) in [(12, 0.03), (13, -0.02), (14, 0.025), (15, -0.015)] {
        t[k][2] += dz;
    }
    two_blocks((ElementKind::Hex20, t, 1), (ElementKind::Hex8, t1_upper().to_vec(), 0))
}

const TET_TARGET: [Vec3; 4] = [[-0.2, -0.2, 1.0], [-0.2, 1.8, 1.02], [1.8, -0.2, 0.97], [0.5, 0.5, 0.0]];

fn tet_target_model(quadratic: bool) -> SolidModel {
    if quadratic {
        let mut t = tet10_nodes(&TET_TARGET);
        for (k, dz) in [(4, 0.02), (5, -0.015), (6, 0.01)] {
            t[k][2] += dz;
        }
        two_blocks((ElementKind::Tet10, t, 0), (ElementKind::Hex8, t1_upper().to_vec(), 0))
    } else {
        two_blocks(
            (ElementKind::Tet4, TET_TARGET.to_vec(), 0),
            (ElementKind::Hex8, t1_upper().to_vec(), 0),
        )
    }
}

fn hex20_contactor_model() -> SolidModel {
    let mut cn = hex20_nodes(&t1_upper());
    for (k, dz) in [(8, 0.01), (9, -0.01), (10, 0.005), (11, -0.005)] {
        cn[k][2] -= dz;
    }
    two_blocks((ElementKind::Hex8, T1_LOWER.to_vec(), 1), (ElementKind::Hex20, cn, 0))
}

fn tet10_contactor_model() -> SolidModel {
    let mut cn = tet10_nodes(&[[0.2, 0.2, 0.99], [0.8, 0.25, 0.98], [0.3, 0.8, 1.0], [0.4, 0.4, 1.8]]);
    for (k, dz) in [(4, 0.01), (5, -0.01), (6, 0.005)] {
        cn[k][2] += dz;
    }
    two_blocks((ElementKind::Hex8, T1_LOWER.to_vec(), 1), (ElementKind::Tet10, cn, 0))
}

/// The tangent check of `m`'s pair (eps 10) with the multipliers `lam`.
fn tangent(m: &SolidModel, lam: &[f64]) -> (f64, bool, ContactSet, Vec<f64>) {
    let pair = ContactPair::new("contactor", Target::Surface("target".into()), 10.0);
    let set = ContactSet::new(m, std::slice::from_ref(&pair), &ContactOptions::new(1, 0.0)).unwrap();
    set.set_multipliers(0, lam).unwrap();
    let u = t1_state(m.dof_map().n_equations());
    let (err, same, _, kfd) = tangent_error(&set, &u);
    (err, same, set, kfd)
}

#[test]
fn t1q_t1t3_t1t6_t1g20_t1g10_tangents() {
    let mut c = Checks::new();
    let node_lam = [0.5, 0.4, 0.6, 0.3];
    let point_lam: Vec<f64> = (0..9).map(|k| 0.1 + 0.01 * k as f64).collect();
    // T1q and NT-q.
    let m = hex20_target_model();
    let (err, same, set, kfd) = tangent(&m, &node_lam);
    c.holds("T1q active set unchanged", same, &format!("{same}"));
    c.at_most("T1q Hex20 target", err, 1e-6);
    let n = m.dof_map().n_equations();
    let dofs = m.dof_map();
    let u = t1_state(n);
    let mut kg = vec![0.0; n * n];
    for v in set.evaluate(&u).unwrap() {
        let mut g: Vec<(usize, f64)> = Vec::new();
        let e0 = dofs.equation(v.node, 0).unwrap();
        for i in 0..3 {
            g.push((e0 + i, v.normal[i]));
        }
        let face = v.face.unwrap();
        let fnodes = m.mesh().face_nodes(face);
        let (nn, _) = face_shape_functions(fnodes.len(), v.local).unwrap();
        for (a, &fnode) in fnodes.iter().enumerate() {
            let ea = dofs.equation(fnode, 0).unwrap();
            for i in 0..3 {
                g.push((ea + i, -nn[a] * v.normal[i]));
            }
        }
        let eps = 10.0 * v.area;
        for &(r, gr) in &g {
            for &(s, gs) in &g {
                kg[n * r + s] += eps * gr * gs;
            }
        }
    }
    let kmax = worst(kfd.iter().map(|v| v.abs()));
    let nt = worst(kg.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kmax;
    c.at_least("NT-q without the curvature part (negative control)", nt, 1e-2);
    for (tag, m, lam) in [
        ("T1t3 Tet4 target", tet_target_model(false), node_lam.to_vec()),
        ("T1t6 Tet10 target", tet_target_model(true), node_lam.to_vec()),
        ("T1g20 Hex20 contactor", hex20_contactor_model(), point_lam.clone()),
        ("T1g10 Tet10 contactor", tet10_contactor_model(), point_lam.clone()),
    ] {
        let (err, same, set, _) = tangent(&m, &lam);
        let pts = set.nodes(0).len();
        c.holds(
            &format!("{tag}: active set unchanged"),
            same,
            &format!("{same}, {pts} contactor points"),
        );
        c.at_most(tag, err, 1e-6);
    }
    c.finish();
}

#[test]
fn ng_hex20_corner_share_is_negative() {
    let mut c = Checks::new();
    let rule = face_rule(8).unwrap();
    let mut share = [0.0; 8];
    let mut total = 0.0;
    for (uv, w) in &rule {
        let (n, _) = face_shape_functions(8, *uv).unwrap();
        for a in 0..8 {
            share[a] += w * n[a];
        }
        total += w;
    }
    println!("NG shares of a flat Hex20 face: {:?}", share.map(|s| s / total));
    c.holds(
        "NG corner share negative",
        (0..4).all(|a| share[a] / total < 0.0),
        &format!("corner {:.6e}", share[0] / total),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ HS20, HS10

fn hertz_quadratic(mesh: SolidMesh, h: f64) -> Hertz {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let pair = ContactPair::new(
        "top",
        Target::RigidSphere {
            centre: [0.0, 0.0, R_H],
            radius: R_H,
        },
        100.0 / h,
    );
    let t0 = std::time::Instant::now();
    let r = solve(
        &model,
        &pres,
        &[],
        &[pair],
        &ContactOptions::new(1, 1e-8).newton(newton()),
    )
    .unwrap();
    let s = r.steps.last().unwrap();
    println!(
        "{} unknowns: {:?}, {} Newton solves, iterations {:?}",
        model.dof_map().n_equations(),
        t0.elapsed(),
        s.solves.len(),
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>()
    );
    hertz_read(&model, &r)
}

fn quadratic_checks(c: &mut Checks, tag: &str, h: &Hertz) {
    let (a_h, p_h, appr) = hertz_theory(h.force);
    println!(
        "{tag}: F {:.6e}, a_fe {:.6e}, a_H {a_h:.6e}, p_fit {:.6e}, p_H {p_h:.6e}, points {}, penetration {:.3e}",
        h.force, h.a_fe, h.p_fit, h.points, h.penetration
    );
    c.at_most(&format!("{tag} contact radius"), (h.a_fe / a_h - 1.0).abs(), 0.15);
    c.at_most(&format!("{tag} peak pressure"), (h.p_fit / p_h - 1.0).abs(), 0.20);
    c.at_most(&format!("{tag} approach"), appr, 0.10);
}

/// Known miss (ADR 0540 Addendum 1): the registered run of 2026-11-15 failed in the solve, "load step 1 of 1, augmentation 20:
/// the gap tolerance 1e-8 not met after 20 augmentations": 76 contactor points over the tolerance, max |gap| 4.95e-7, min
/// 1.03e-8. The Gauss-point contactor (9 points per face) over-constrains the faces' nodes against the curved rigid target.
/// The limits are those registered; none was changed.
#[test]
#[ignore = "known miss (ADR 0540 Addendum 1): augmentation 20, 76 points over the gap tolerance 1e-8, max |gap| 4.95e-7"]
fn hs20_hertz_on_hex20_known_miss() {
    let mut c = Checks::new();
    let mesh = upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0;
    let h = hertz_quadratic(mesh, A_H / 4.0);
    quadratic_checks(&mut c, "HS20", &h);
    c.finish();
}

/// Known miss (ADR 0540 Addendum 1): the registered run of 2026-11-15 failed in the solve, "load step 1 of 1, augmentation 20:
/// the gap tolerance 1e-8 not met after 20 augmentations": 82 contactor points over the tolerance, max |gap| 7.63e-6, min
/// 2.48e-8. The same over-constraint as HS20. The limits are those registered; none was changed.
#[test]
#[ignore = "known miss (ADR 0540 Addendum 1): augmentation 20, 82 points over the gap tolerance 1e-8, max |gap| 7.63e-6"]
fn hs10_hertz_on_tet10_known_miss() {
    let mut c = Checks::new();
    let mesh = upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0;
    let h = hertz_quadratic(mesh, A_H / 3.0);
    quadratic_checks(&mut c, "HS10", &h);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ R3, R1c

#[test]
fn r3_face_refusals() {
    let mut c = Checks::new();
    let base = hex20_contactor_model();
    let mut mesh = base.mesh().clone();
    mesh.add_surface_set(
        "mixed",
        vec![FaceRef { element: 0, face: 1 }, FaceRef { element: 1, face: 0 }],
    )
    .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    let plane = Target::RigidPlane {
        point: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
    };
    let o = ContactOptions::new(1, 0.0);
    let e = ContactSet::new(&m, &[ContactPair::new("mixed", plane.clone(), 1.0)], &o).err();
    c.holds(
        "R3 mixed node and point faces",
        matches!(
            &e,
            Some(FemError::InvalidValue {
                key: "contact surface",
                ..
            })
        ),
        &format!("{e:?}"),
    );
    let e = ContactSet::new(&m, &[ContactPair::new("contactor", plane, 1.0).initial_gap(8, 0.0)], &o).err();
    c.holds(
        "R3 initial gap on a Hex20 contactor",
        matches!(&e, Some(FemError::InvalidValue { key: "initial gap", .. })),
        &format!("{e:?}"),
    );
    c.finish();
}

// The copy of ADR 0507's r1_r2_refusals_a2 (tests/sa_contact.rs) with the Tet4 row replaced (ADR 0540 D9).

const E_BAR: f64 = 2.0e11;
const A_BAR: f64 = 0.01;
const GAP: f64 = 1e-2;
const F_MAX: f64 = 2.0 * E_BAR * A_BAR * GAP;

fn bar(force: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(E_BAR, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    if force != 0.0 {
        m.add_load(Load::traction("xmax", [force / A_BAR, 0.0, 0.0])).unwrap();
    }
    m
}

fn stop() -> ContactPair {
    ContactPair::new(
        "xmax",
        Target::RigidPlane {
            point: [1.0 + GAP, 0.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        },
        100.0 * E_BAR / 0.25,
    )
}

#[test]
fn r1c_r2_refusals_f601() {
    let mut c = Checks::new();
    let m = bar(F_MAX);
    let o = ContactOptions::new(1, 1e-11);
    let e = solve(&m, &[], &[], &[ContactPair::new("nope", stop().target, 1.0)], &o).unwrap_err();
    c.holds(
        "R1 unknown contactor",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    let e = solve(
        &m,
        &[],
        &[],
        &[ContactPair::new("xmax", Target::Surface("nope".into()), 1.0)],
        &o,
    )
    .unwrap_err();
    c.holds(
        "R1 unknown target",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    for pen in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let e = solve(&m, &[], &[], &[ContactPair::new("xmax", stop().target, pen)], &o).unwrap_err();
        c.holds(
            &format!("R1 penalty {pen}"),
            matches!(e, FemError::InvalidValue { key: "penalty", .. }),
            &e.to_string(),
        );
    }
    for tol in [-1.0, f64::NAN] {
        let e = solve(&m, &[], &[], &[stop()], &ContactOptions::new(1, tol)).unwrap_err();
        c.holds(
            &format!("R1 tolerance {tol}"),
            matches!(
                e,
                FemError::InvalidValue {
                    key: "penetration_tolerance",
                    ..
                }
            ),
            &e.to_string(),
        );
    }
    // R1c (ADR 0540 D9): the Tet4 contactor face is accepted.
    let mut t = SolidModel::new(tet_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap());
    t.add_material("all", elastic(1.0, 0.3)).unwrap();
    let set = ContactSet::new(&t, &[ContactPair::new("xmax", stop().target, 1.0)], &o);
    let count = set.as_ref().map(|s| s.nodes(0).len()).unwrap_or(0);
    c.holds(
        "R1c Tet4 face accepted",
        set.is_ok() && count == 4,
        &format!(
            "{:?} contactor nodes",
            set.as_ref().map(|s| s.nodes(0).len()).map_err(|e| e.to_string())
        ),
    );
    let mut m2 = bar(0.0);
    m2.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let e = solve(
        &m2,
        &[],
        &[],
        &[stop()],
        &o.clone().kinematics(Kinematics::TotalLagrangian),
    )
    .unwrap_err();
    c.holds(
        "R2 total Lagrangian with a non-zero model constraint",
        matches!(&e, FemError::InvalidValue { key: "constraint", reason } if reason.contains("'xmax'")),
        &e.to_string(),
    );
    c.finish();
}
