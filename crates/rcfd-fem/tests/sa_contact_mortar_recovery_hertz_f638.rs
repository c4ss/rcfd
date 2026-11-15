//! F-638 (ADR 0571): ADR 0551's Hertz problems HS20m and HS10m read from the recovered traction (HS20r, HS10r, with FW and
//! the negative control NC-n). Heavy records (about two minutes each), run once. Every limit is the one registered in ADR
//! 0571.

mod f601_common;

use f601_common::*;
use rcfd_fem::builders::upgrade_to_tet10;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactSet, QuadraticContactor, Target};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::NewtonOptions;
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

/// ADR 0571 D4: the fit `p^2 = c_0 + c_1 r^2` over `(r^2, p)` with `p >= 0.5 max p`, unit weights: `(a_fe, sqrt(c_0),
/// count)`.
fn fit(v: &[(f64, f64)]) -> (f64, f64, usize) {
    let pmax = v.iter().map(|x| x.1).fold(0.0, f64::max);
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(r2, p) in v.iter().filter(|x| x.1 >= 0.5 * pmax) {
        let y = p * p;
        sw += 1.0;
        sx += r2;
        sy += y;
        sxx += r2 * r2;
        sxy += r2 * y;
    }
    let c1 = (sw * sxy - sx * sy) / (sw * sxx - sx * sx);
    let c0 = (sy - c1 * sx) / sw;
    ((-c0 / c1).sqrt(), c0.sqrt(), sw as usize)
}

fn hertz_recovered(c: &mut Checks, tag: &str, mesh: SolidMesh, h: f64) {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let o = ContactOptions::new(1, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30))
        .quadratic_contactor(QuadraticContactor::Mortar);
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
        "{tag}: {} unknowns, {elapsed:?}, {} augmentations, iterations {:?}, contactor nodes {}, active {}, penetration {:.3e}",
        model.dof_map().n_equations(),
        s.solves.len() - 1,
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>(),
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count(),
        s.solves.last().unwrap().penetration
    );
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
    let (a_h, p_h, appr) = hertz_theory(f_fe);
    // The recovered traction (ADR 0571 D1, D3).
    let set = ContactSet::new(&model, &[sphere_pair(h)], &o).unwrap();
    let rec = set.recovered_tractions(&r.nodes).unwrap();
    let mut v: Vec<(f64, f64)> = rec
        .iter()
        .map(|t| {
            (
                t.position[0] * t.position[0] + t.position[1] * t.position[1],
                t.pressure,
            )
        })
        .collect();
    let (a_fe, p_fit, used) = fit(&v);
    let active = rec.iter().filter(|t| t.pressure > 0.0).count();
    let pmin = rec.iter().map(|t| t.pressure).fold(f64::INFINITY, f64::min);
    let pmax = rec.iter().map(|t| t.pressure).fold(f64::NEG_INFINITY, f64::max);
    println!(
        "{tag}: F_fe {f_fe:.6e}, a_fe {a_fe:.6e}, a_H {a_h:.6e}, p_fit {p_fit:.6e}, p_H {p_h:.6e}; {} points, {active} with \
         p > 0, {used} in the fit, p from {pmin:.4e} to {pmax:.4e}",
        rec.len()
    );
    c.at_most(&format!("{tag} contact radius"), (a_fe / a_h - 1.0).abs(), 0.15);
    c.at_most(&format!("{tag} peak pressure"), (p_fit / p_h - 1.0).abs(), 0.20);
    c.at_most(&format!("{tag} approach"), appr, 0.10);
    let wsum: f64 = rec.iter().map(|t| t.weight * t.pressure).sum();
    let tsum: f64 = r.nodes.iter().map(|n| n.force).sum();
    c.at_most(&format!("FW {tag}"), (wsum / tsum - 1.0).abs(), 1e-12);
    // NC-n: ADR 0551's nodal reading of the same solve.
    let hz = hertz_read(&model, &r);
    c.at_least(
        &format!("NC-n {tag} nodal reading radius (negative control)"),
        (hz.a_fe / a_h - 1.0).abs(),
        0.15,
    );
    // Reported per point (recorded): the points with p > 0, by radius.
    v.retain(|x| x.1 > 0.0);
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (r2, p) in &v {
        println!("{tag} point r {:.5} p {p:.5e}", r2.sqrt());
    }
}

#[test]
#[ignore = "heavy record (ADR 0571 HS20r, about two minutes): run once"]
fn hs20r_hertz_on_hex20_recovered_traction() {
    let mut c = Checks::new();
    hertz_recovered(
        &mut c,
        "HS20r",
        upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0,
        A_H / 4.0,
    );
    c.finish();
}

#[test]
#[ignore = "heavy record (ADR 0571 HS10r, about two minutes): run once"]
fn hs10r_hertz_on_tet10_recovered_traction() {
    let mut c = Checks::new();
    let mesh = upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0;
    hertz_recovered(&mut c, "HS10r", mesh, A_H / 3.0);
    c.finish();
}
