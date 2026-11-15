//! F-638 (ADR 0571): the traction of the mortar contactor recovered at the load-rule points. PT20r, PT10r (and PT20rt,
//! PT10rt on the total Lagrangian body), RP, PV, FW, NR. Every limit is the one registered in ADR 0571.

mod f601_common;
mod f623_common;

use f601_common::*;
use f623_common::*;
use rcfd_core::vec3::Vec3;
use rcfd_fem::contact::{
    solve, ContactNode, ContactOptions, ContactPair, ContactSet, Kinematics, QuadraticContactor, RecoveredTraction,
    Target,
};
use rcfd_fem::loads::{face_rule, face_shape_functions};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::{FemError, SolidModel};

const P: f64 = 1.0e8;

const SPHERE: Target = Target::RigidSphere {
    centre: [0.5, 0.45, 0.0],
    radius: 0.97,
};

fn mortar(o: ContactOptions) -> ContactOptions {
    o.quadratic_contactor(QuadraticContactor::Mortar)
}

fn patch_pair() -> ContactPair {
    ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5)
}

fn patch_options() -> ContactOptions {
    mortar(ContactOptions::new(2, 1e-13).newton(NewtonOptions::new(1).max_iterations(30)))
}

/// ADR 0551 D1 coded again: `Phi` from the face's shape-function values `n` (corners first, then the mid-edge nodes).
fn phi_again(n: &[f64]) -> Vec<f64> {
    let c = if n.len() == 8 { 4 } else { 3 };
    (0..n.len())
        .map(|a| {
            if a < c {
                n[a] + 0.25 * (n[c + a] + n[c + (a + c - 1) % c])
            } else {
                0.5 * n[a]
            }
        })
        .collect()
}

fn cross_norm(u: Vec3, v: Vec3) -> f64 {
    let c = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// RP and PV: the recovered points against the test's own evaluation over the faces of `set` (pair 0): the order (bits of
/// face and local), the largest position difference, the largest relative weight difference, the largest pressure
/// difference over `max |p|`, and the count.
fn rp_pv(m: &SolidModel, set: &str, nodes: &[ContactNode], rec: &[RecoveredTraction]) -> (bool, f64, f64, f64, usize) {
    let mesh = m.mesh();
    let x = mesh.nodes();
    let lambda = |v: usize| -> f64 {
        nodes
            .iter()
            .find(|n| n.pair == 0 && n.node == v)
            .map_or(f64::NAN, |n| n.force / n.area)
    };
    let mut k = 0;
    let (mut order, mut dx, mut dw, mut dp) = (true, 0.0f64, 0.0f64, 0.0f64);
    let pmax = worst(rec.iter().map(|r| r.pressure.abs()));
    for &f in mesh.surface_set(set).unwrap() {
        let fn_ = mesh.face_nodes(f);
        for (uv, w) in face_rule(fn_.len()).unwrap() {
            let Some(r) = rec.get(k) else {
                order = false;
                break;
            };
            k += 1;
            order &= r.pair == 0
                && r.face == f
                && r.local[0].to_bits() == uv[0].to_bits()
                && r.local[1].to_bits() == uv[1].to_bits();
            let (n, d) = face_shape_functions(fn_.len(), uv).unwrap();
            let (mut a1, mut a2, mut xp) = ([0.0; 3], [0.0; 3], [0.0; 3]);
            for (j, &v) in fn_.iter().enumerate() {
                for i in 0..3 {
                    a1[i] += d[j][0] * x[v][i];
                    a2[i] += d[j][1] * x[v][i];
                    xp[i] += n[j] * x[v][i];
                }
            }
            let wg = w * cross_norm(a1, a2);
            let phi = phi_again(&n);
            let p: f64 = fn_.iter().zip(&phi).map(|(&v, ph)| lambda(v) * ph).sum();
            dx = dx.max(worst((0..3).map(|i| (r.position[i] - xp[i]).abs())));
            dw = dw.max(worst([(r.weight / wg - 1.0).abs()]));
            dp = dp.max(worst([(r.pressure - p).abs() / pmax]));
        }
    }
    order &= k == rec.len();
    (order, dx, dw, dp, rec.len())
}

/// FW: `sum_g W_g p_g` against `sum_a t_a` (pair 0), relative.
fn fw(nodes: &[ContactNode], rec: &[RecoveredTraction]) -> f64 {
    let a: f64 = rec.iter().map(|r| r.weight * r.pressure).sum();
    let t: f64 = nodes.iter().filter(|n| n.pair == 0).map(|n| n.force).sum();
    (a / t - 1.0).abs()
}

fn patch(c: &mut Checks, tag: &str, kind: Kind, count: usize, tl: bool) {
    let m = patch_model(kind, P);
    let mut o = patch_options();
    if tl {
        o = o.kinematics(Kinematics::TotalLagrangian);
    }
    let r = solve(&m, &[], &[], &[patch_pair()], &o).unwrap();
    let set = ContactSet::new(&m, &[patch_pair()], &o).unwrap();
    let rec = set.recovered_tractions(&r.nodes).unwrap();
    c.at_most(
        &format!("{tag} recovered traction / p"),
        worst(rec.iter().map(|v| (v.pressure / P - 1.0).abs())),
        1e-8,
    );
    c.holds(
        &format!("{tag} points"),
        rec.len() == count,
        &format!("{} (registered {count})", rec.len()),
    );
    if !tl {
        let (order, dx, dw, _, k) = rp_pv(&m, "upper.zmin", &r.nodes, &rec);
        c.holds(&format!("RP {tag} order"), order, &format!("{order}, {k} points"));
        c.at_most(&format!("RP {tag} position"), dx, 1e-13);
        c.at_most(&format!("RP {tag} weight (relative)"), dw, 1e-13);
        c.at_most(&format!("FW {tag}"), fw(&r.nodes, &rec), 1e-12);
    }
}

#[test]
fn pt20r_pt10r_recovered_traction_patch_tests_rp_fw() {
    let mut c = Checks::new();
    patch(&mut c, "PT20r", Kind::Hex20, 36, false);
    patch(&mut c, "PT10r", Kind::Tet10, 72, false);
    c.finish();
}

#[test]
fn pt20rt_pt10rt_recovered_traction_total_lagrangian() {
    let mut c = Checks::new();
    patch(&mut c, "PT20rt", Kind::Hex20, 36, true);
    patch(&mut c, "PT10rt", Kind::Tet10, 72, true);
    c.finish();
}

#[test]
fn pv_rp_fw_on_the_sphere_models() {
    let mut c = Checks::new();
    for (tag, m) in [("M20s", hex20_contactor_model()), ("M10s", tet10_contactor_model())] {
        let pair = ContactPair::new("contactor", SPHERE, 10.0);
        let set = ContactSet::new(&m, &[pair], &mortar(ContactOptions::new(1, 0.0))).unwrap();
        let k = set.nodes(0).len();
        let lam: Vec<f64> = (0..k).map(|i| 0.1 + 0.01 * i as f64).collect();
        set.set_multipliers(0, &lam).unwrap();
        let u = t1_state(m.dof_map().n_equations());
        let nodes = set.evaluate(&u).unwrap();
        let rec = set.recovered_tractions(&nodes).unwrap();
        let active = nodes.iter().filter(|n| n.active).count();
        let (order, dx, dw, dp, np) = rp_pv(&m, "contactor", &nodes, &rec);
        println!(
            "{tag}: {np} points, {active} of {k} nodes active, p from {:.4e} to {:.4e}",
            rec.iter().map(|r| r.pressure).fold(f64::INFINITY, f64::min),
            worst(rec.iter().map(|r| r.pressure))
        );
        c.holds(
            &format!("RP {tag} order"),
            order && np == 9,
            &format!("{order}, {np} points"),
        );
        c.at_most(&format!("RP {tag} position"), dx, 1e-13);
        c.at_most(&format!("RP {tag} weight (relative)"), dw, 1e-13);
        c.at_most(&format!("PV {tag}"), dp, 1e-13);
        c.at_most(&format!("FW {tag}"), fw(&nodes, &rec), 1e-12);
    }
    c.finish();
}

#[test]
fn nr_not_mortar_and_missing_node() {
    let mut c = Checks::new();
    let m = patch_model(Kind::Hex20, P);
    let n = m.dof_map().n_equations();
    let o = ContactOptions::new(2, 1e-13);
    let gp = ContactSet::new(&m, &[patch_pair()], &o).unwrap();
    let rec = gp.recovered_tractions(&gp.evaluate(&vec![0.0; n]).unwrap()).unwrap();
    c.holds(
        "NR Gauss-point set gives no entries",
        rec.is_empty(),
        &format!("{}", rec.len()),
    );
    let ms = ContactSet::new(&m, &[patch_pair()], &patch_options()).unwrap();
    let mut nodes = ms.evaluate(&vec![0.0; n]).unwrap();
    nodes.remove(3);
    let key = match ms.recovered_tractions(&nodes) {
        Err(FemError::InvalidValue { key, .. }) => Some(key.to_string()),
        Err(e) => Some(format!("{e:?}")),
        Ok(_) => None,
    };
    c.holds(
        "NR a missing mortar node is refused",
        key.as_deref() == Some("contact nodes"),
        &format!("{key:?}"),
    );
    c.finish();
}
