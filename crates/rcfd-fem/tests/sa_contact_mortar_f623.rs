//! F-623 (ADR 0551): the mortar contactor of Hex20 and Tet10 faces. PT10m, PT20m (with EQm and PW), PT20t, PT10t (the total
//! Lagrangian body), the tangents M20w, M10w, M20s, M10s, M20q, the negative controls NT-m and NW, and the refusals R4. Every
//! limit is the one registered in ADR 0551.

mod f601_common;
mod f623_common;

use f601_common::*;
use f623_common::*;
use rcfd_core::vec3::Vec3;
use rcfd_fem::contact::{
    solve, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, QuadraticContactor, SearchMethod, Target,
};
use rcfd_fem::loads::{face_rule, face_shape_functions};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::{ElementKind, FaceRef, FemError, SolidMesh, SolidModel};
use std::collections::BTreeMap;

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

fn mortar(o: ContactOptions) -> ContactOptions {
    o.quadratic_contactor(QuadraticContactor::Mortar)
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

// ------------------------------------------------------------------------------------------------------------ PT, PW

const P: f64 = 1.0e8;

fn patch_pair() -> ContactPair {
    ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5)
}

fn patch_options() -> ContactOptions {
    mortar(ContactOptions::new(2, 1e-13).newton(newton()))
}

fn reaction_error(m: &SolidModel, r: &ContactResult) -> f64 {
    let dofs = m.dof_map();
    let re = r.result.reactions().unwrap();
    let sz: f64 = m
        .mesh()
        .node_set("lower.zmin")
        .unwrap()
        .iter()
        .map(|&n| re[dofs.equation(n, 2).unwrap()])
        .sum();
    (sz - P).abs() / P
}

fn pressure_error(r: &ContactResult) -> f64 {
    worst(r.nodes.iter().map(|n| (n.pressure - P).abs() / P))
}

/// PW: the closed-form areas of D6 (flat faces): Quad8 corner A/12, edge A/6; Tri6 A/6 at every node.
fn closed_form_areas(m: &SolidModel) -> BTreeMap<usize, f64> {
    let mesh = m.mesh();
    let x = mesh.nodes();
    let mut out: BTreeMap<usize, f64> = BTreeMap::new();
    for &f in mesh.surface_set("upper.zmin").unwrap() {
        let nodes = mesh.face_nodes(f);
        let d = |a: usize, b: usize| -> Vec3 {
            let (p, q) = (x[nodes[a]], x[nodes[b]]);
            [q[0] - p[0], q[1] - p[1], q[2] - p[2]]
        };
        let cr = |u: Vec3, v: Vec3| -> f64 {
            let c = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
        };
        let shares: Vec<f64> = if nodes.len() == 8 {
            let a = cr(d(0, 1), d(0, 3));
            (0..8).map(|k| if k < 4 { a / 12.0 } else { a / 6.0 }).collect()
        } else {
            let a = 0.5 * cr(d(0, 1), d(0, 2));
            vec![a / 6.0; 6]
        };
        for (&v, s) in nodes.iter().zip(shares) {
            *out.entry(v).or_insert(0.0) += s;
        }
    }
    out
}

fn patch_linear(c: &mut Checks, tag: &str, kind: Kind, count: usize) {
    let m = patch_model(kind, P);
    let pair = patch_pair();
    let o = patch_options();
    let ra = solve(
        &m,
        &[],
        &[],
        std::slice::from_ref(&pair),
        &o.clone().search_method(SearchMethod::AllFaces),
    )
    .unwrap();
    let r = solve(
        &m,
        &[],
        &[],
        std::slice::from_ref(&pair),
        &o.clone().search_method(SearchMethod::Buckets),
    )
    .unwrap();
    let (wa, wb) = (result_words(&ra), result_words(&r));
    let (equal, finite) = (wa == wb, result_finite(&ra) && result_finite(&r));
    c.holds(
        &format!("EQm {tag}"),
        equal && finite,
        &format!("{} words, equal {equal}, finite {finite}", wa.len()),
    );
    let u = r.result.displacement_values();
    c.at_most(
        &format!("{tag} stress / p"),
        stress_error(&m, u, [0.0, 0.0, -P, 0.0, 0.0, 0.0]) / P,
        1e-8,
    );
    c.at_most(&format!("{tag} node pressure / p"), pressure_error(&r), 1e-8);
    c.at_most(&format!("{tag} reaction / p"), reaction_error(&m, &r), 1e-8);
    c.holds(
        &format!("{tag} contactor nodes"),
        r.nodes.len() == count,
        &format!("{} (registered {count})", r.nodes.len()),
    );
    let s = r.steps.last().unwrap();
    println!(
        "{tag}: {} Newton solves in the last step, active {}",
        s.solves.len(),
        r.nodes.iter().filter(|n| n.active).count()
    );
    // PW.
    let cf = closed_form_areas(&m);
    let same_nodes = cf.keys().copied().collect::<Vec<_>>() == r.nodes.iter().map(|n| n.node).collect::<Vec<_>>();
    c.holds(&format!("PW {tag} node list"), same_nodes, &format!("{same_nodes}"));
    c.at_most(
        &format!("PW {tag} areas (relative)"),
        worst(r.nodes.iter().map(|n| {
            let e = cf.get(&n.node).copied().unwrap_or(f64::NAN);
            (n.area / e - 1.0).abs()
        })),
        1e-14,
    );
}

fn patch_tl(c: &mut Checks, tag: &str, kind: Kind, count: usize) {
    let m = patch_model(kind, P);
    let o = patch_options().kinematics(Kinematics::TotalLagrangian);
    let r = solve(&m, &[], &[], &[patch_pair()], &o).unwrap();
    c.at_most(&format!("{tag} node pressure / p"), pressure_error(&r), 1e-8);
    c.at_most(&format!("{tag} reaction / p"), reaction_error(&m, &r), 1e-8);
    c.holds(
        &format!("{tag} contactor nodes"),
        r.nodes.len() == count,
        &format!("{} (registered {count})", r.nodes.len()),
    );
    let fin = result_finite(&r);
    c.holds(&format!("{tag} finite"), fin, &format!("{fin}"));
    let s = r.steps.last().unwrap();
    println!(
        "{tag}: {} Newton solves in the last step, iterations {:?}",
        s.solves.len(),
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>()
    );
}

#[test]
fn pt10m_pt20m_patch_tests_eqm_pw() {
    let mut c = Checks::new();
    patch_linear(&mut c, "PT10m", Kind::Tet10, 25);
    patch_linear(&mut c, "PT20m", Kind::Hex20, 21);
    c.finish();
}

#[test]
fn pt20t_pt10t_patch_tests_total_lagrangian() {
    let mut c = Checks::new();
    patch_tl(&mut c, "PT20t", Kind::Hex20, 21);
    patch_tl(&mut c, "PT10t", Kind::Tet10, 25);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ tangents

const SPHERE: Target = Target::RigidSphere {
    centre: [0.5, 0.45, 0.0],
    radius: 0.97,
};

/// The tangent check of `m`'s mortar pair against `target` (eps 10, multipliers 0.1 + 0.01 k); also the error of the
/// matrix without the curvature part (NT-m).
fn mortar_tangent(m: &SolidModel, target: Target) -> (f64, bool, usize, f64) {
    let pair = ContactPair::new("contactor", target, 10.0);
    let set = ContactSet::new(m, &[pair], &mortar(ContactOptions::new(1, 0.0))).unwrap();
    let k = set.nodes(0).len();
    let lam: Vec<f64> = (0..k).map(|i| 0.1 + 0.01 * i as f64).collect();
    set.set_multipliers(0, &lam).unwrap();
    let n = m.dof_map().n_equations();
    let u = t1_state(n);
    let (err, same, _, kfd) = tangent_error(&set, &u);
    let mut kc = vec![0.0; n * n];
    for (r, s, v) in set.convex_tangent(&u).unwrap() {
        kc[n * r + s] += v;
    }
    let kmax = worst(kfd.iter().map(|v| v.abs()));
    let nt = worst(kc.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kmax;
    (err, same, k, nt)
}

#[test]
fn m20w_m10w_m20s_m10s_m20q_tangents_and_nt_m() {
    let mut c = Checks::new();
    let surface = || Target::Surface("target".into());
    for (tag, m, target, nodes, control) in [
        ("M20w", hex20_contactor_model(), surface(), 8, false),
        ("M10w", tet10_contactor_model(), surface(), 6, false),
        ("M20s", hex20_contactor_model(), SPHERE, 8, true),
        ("M10s", tet10_contactor_model(), SPHERE, 6, false),
        ("M20q", hex20_on_hex20_model(), surface(), 8, true),
    ] {
        let (err, same, k, nt) = mortar_tangent(&m, target);
        c.holds(
            &format!("{tag}: every node active, the same faces"),
            same && k == nodes,
            &format!("{same}, {k} mortar nodes (expected {nodes})"),
        );
        c.at_most(&format!("{tag} tangent"), err, 1e-6);
        if control {
            c.at_least(
                &format!("NT-m {tag} without the curvature part (negative control)"),
                nt,
                1e-2,
            );
        } else {
            println!("{tag}: without the curvature part {nt:.3e} (recorded)");
        }
    }
    c.finish();
}

#[test]
fn nw_untransformed_corner_weight_is_negative() {
    let mut c = Checks::new();
    let (mut corner, mut total) = (0.0, 0.0);
    for (uv, w) in face_rule(8).unwrap() {
        let (n, _) = face_shape_functions(8, uv).unwrap();
        corner += w * n[0];
        total += w;
    }
    c.less("NW int N_corner / A (alpha = 0)", corner / total, 0.0);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ R4

fn key_of(e: FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn r4_mortar_refusals() {
    let mut c = Checks::new();
    let o = mortar(ContactOptions::new(1, 1e-8));
    // An initial gap on a Hex20 contactor (node 8: the first corner of the contactor block, on its face 0).
    let m = hex20_contactor_model();
    let pair = ContactPair::new(
        "contactor",
        Target::RigidPlane {
            point: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        },
        10.0,
    )
    .initial_gap(8, 0.0);
    let k = ContactSet::new(&m, &[pair], &o).err().map(key_of);
    c.holds(
        "R4 initial gap on a mortar pair",
        k.as_deref() == Some("initial gap"),
        &format!("{k:?}"),
    );
    // A contactor set of a Hex8 face and a Hex20 face.
    let mut nodes = T1_LOWER.to_vec();
    nodes.extend(hex20_contactor_nodes());
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, (0..8).collect()).unwrap();
    mesh.add_block(ElementKind::Hex20, (8..28).collect()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_surface_set(
        "mixed",
        vec![FaceRef { element: 0, face: 1 }, FaceRef { element: 1, face: 0 }],
    )
    .unwrap();
    let mut mm = SolidModel::new(mesh);
    mm.add_material("all", elastic(1.0, 0.3)).unwrap();
    let pair = ContactPair::new(
        "mixed",
        Target::RigidPlane {
            point: [0.0, 0.0, -5.0],
            normal: [0.0, 0.0, 1.0],
        },
        10.0,
    );
    let k = ContactSet::new(&mm, &[pair], &o).err().map(key_of);
    c.holds(
        "R4 a set of a Hex8 and a Hex20 face",
        k.as_deref() == Some("contact surface"),
        &format!("{k:?}"),
    );
    c.finish();
}
