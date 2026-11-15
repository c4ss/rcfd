//! F-623 (ADR 0551): ADR 0540's Hertz problem HS20 and HS10 with the mortar contactor (HS20m, HS10m, FR), and with the total
//! Lagrangian body as heavy records (HS20t, HS10t). Every limit is the one registered in ADR 0551.

mod f601_common;

use f601_common::*;
use rcfd_fem::builders::upgrade_to_tet10;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactSet, Kinematics, QuadraticContactor, Target};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{SolidMesh, SolidModel};

fn sphere_pair(h: f64) -> ContactPair {
    ContactPair::new(
        "top",
        Target::RigidSphere {
            centre: [0.0, 0.0, R_H],
            radius: R_H,
        },
        100.0 / h,
    )
}

fn hertz_mortar(c: &mut Checks, tag: &str, mesh: SolidMesh, h: f64, tl: bool) {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let mut o = ContactOptions::new(1, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30))
        .quadratic_contactor(QuadraticContactor::Mortar);
    if tl {
        o = o
            .kinematics(Kinematics::TotalLagrangian)
            .factor(DirectFactor::SupernodalNestedDissection);
    }
    let t0 = std::time::Instant::now();
    let r = solve(&model, &pres, &[], &[sphere_pair(h)], &o);
    let elapsed = t0.elapsed();
    let r = match r {
        Ok(r) => r,
        Err(e) => {
            c.holds(&format!("{tag} solve"), false, &format!("{e} ({elapsed:?})"));
            return;
        }
    };
    c.holds(&format!("{tag} solve"), true, "succeeds with the gap tolerance 1e-8");
    let s = r.steps.last().unwrap();
    println!(
        "{tag}: {} unknowns, {elapsed:?}, {} Newton solves ({} augmentations), iterations {:?}, contactor nodes {}, active {}",
        model.dof_map().n_equations(),
        s.solves.len(),
        s.solves.len() - 1,
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>(),
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count()
    );
    // F_fe from the z reactions of the prescribed equations.
    let dofs = model.dof_map();
    let re = r.result.reactions().unwrap();
    let mut eqs: Vec<usize> = pres
        .iter()
        .filter(|p| p.component == 2)
        .map(|p| dofs.equation(p.node, 2).unwrap())
        .collect();
    eqs.sort_unstable();
    eqs.dedup();
    let f_fe = 4.0 * eqs.iter().map(|&e| re[e]).sum::<f64>().abs();
    let hz = hertz_read(&model, &r);
    let (a_h, p_h, appr) = hertz_theory(f_fe);
    println!(
        "{tag}: F_fe {f_fe:.6e} (4 sum t |n_z| {:.6e}), a_fe {:.6e}, a_H {a_h:.6e}, p_fit {:.6e}, p_H {p_h:.6e}, \
         penetration {:.3e}, nodes on y = 0 in contact {}",
        hz.force, hz.a_fe, hz.p_fit, hz.penetration, hz.axis_count
    );
    c.at_most(&format!("{tag} contact radius"), (hz.a_fe / a_h - 1.0).abs(), 0.15);
    c.at_most(&format!("{tag} peak pressure"), (hz.p_fit / p_h - 1.0).abs(), 0.20);
    c.at_most(&format!("{tag} approach"), appr, 0.10);
    if !tl {
        c.at_most(&format!("FR {tag}"), (hz.force / f_fe - 1.0).abs(), 1e-3);
    }
    // Recorded: the largest penetration at the load-rule points (the Gauss-point contactor at the mortar state).
    let gp = ContactSet::new(&model, &[sphere_pair(h)], &ContactOptions::new(1, 1e-8)).unwrap();
    let pts = gp.evaluate(r.result.displacement_values()).unwrap();
    let pen = pts.iter().filter_map(|v| v.gap).fold(0.0f64, |a, g| a.max(-g));
    let over = pts.iter().filter_map(|v| v.gap).filter(|g| *g < -1e-8).count();
    println!(
        "{tag}: at the {} load-rule points: largest penetration {pen:.3e}, {over} points below -1e-8 (recorded)",
        pts.len()
    );
}

fn hs20_mesh() -> SolidMesh {
    upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0
}

fn hs10_mesh() -> SolidMesh {
    upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0
}

#[test]
fn hs20m_hertz_on_hex20_mortar() {
    let mut c = Checks::new();
    hertz_mortar(&mut c, "HS20m", hs20_mesh(), A_H / 4.0, false);
    c.finish();
}

#[test]
fn hs10m_hertz_on_tet10_mortar() {
    let mut c = Checks::new();
    hertz_mortar(&mut c, "HS10m", hs10_mesh(), A_H / 3.0, false);
    c.finish();
}

#[test]
#[ignore = "heavy record (ADR 0551 HS20t): the total Lagrangian body, several minutes; run once"]
fn hs20t_hertz_on_hex20_mortar_total_lagrangian() {
    let mut c = Checks::new();
    hertz_mortar(&mut c, "HS20t", hs20_mesh(), A_H / 4.0, true);
    c.finish();
}

#[test]
#[ignore = "heavy record (ADR 0551 HS10t): the total Lagrangian body, several minutes; run once"]
fn hs10t_hertz_on_tet10_mortar_total_lagrangian() {
    let mut c = Checks::new();
    hertz_mortar(&mut c, "HS10t", hs10_mesh(), A_H / 3.0, true);
    c.finish();
}
