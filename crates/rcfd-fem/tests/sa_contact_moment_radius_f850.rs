//! F-850 (ADR 0778), check (3): the moment radius of ADR 0570 (D5; HS20m-M and HS10m-M, held there against 0.15) against
//! the limit 0.08. Rows HS20m-M' and HS10m-M' (heavy records, each run once), NC (a uniform pressure on the Hertz disc,
//! which the limit must refuse) and HZ (the Hertz pressure on the same surfaces, the fast test) of ADR 0778. The set-up of
//! the solves is that of `sa_contact_tl_fallback_f639.rs`, copied.

mod f601_common;

use f601_common::*;
use rcfd_fem::builders::upgrade_to_tet10;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, QuadraticContactor, Target};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::face_shape_functions;
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::{SolidMesh, SolidModel};
use std::collections::BTreeMap;

/// The limit of the acceptance of F-850, clause (3).
const LIMIT: f64 = 0.08;
/// The limit of ADR 0570 for the same reading (printed for comparison, not asserted).
const LIMIT_ADR0570: f64 = 0.15;
/// Sample points per direction of a face in the surface integrals of NC and HZ.
const SAMPLES: usize = 48;

fn hs20_mesh() -> SolidMesh {
    upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0
}

fn hs10_mesh() -> SolidMesh {
    upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0
}

/// ADR 0570 D5: `a_M = sqrt(5/2 M_2/M_0)` over the entries `(t, X, Y)` with `t > 0`.
fn moment_radius(forces: &[(f64, f64, f64)]) -> f64 {
    let (mut m0, mut m2) = (0.0, 0.0);
    for &(t, x, y) in forces.iter().filter(|v| v.0 > 0.0) {
        m0 += t;
        m2 += t * (x * x + y * y);
    }
    (2.5 * m2 / m0).sqrt()
}

/// ADR 0551 D1: the weighting functions of a quadratic face from its shape-function values (corners first, then the
/// mid-edge nodes, the edge `c + i` from corner `i` to corner `i + 1`): `Phi_i = N_i + (N_{c+i} + N_{c+i-1})/4`, `Phi_e =
/// N_e/2`.
fn weighting(n: &[f64], corners: usize) -> Vec<f64> {
    let mut phi = vec![0.0; n.len()];
    for e in corners..n.len() {
        phi[e] = 0.5 * n[e];
    }
    for i in 0..corners {
        phi[i] = n[i] + 0.25 * (n[corners + i] + n[corners + (i + corners - 1) % corners]);
    }
    phi
}

/// The nodal forces `t_a = int Phi_a p dA` of the pressure `p(r)` on the surface set "top" of `mesh`, as `(t_a, X_a, Y_a)`
/// per node, and `int p dA`. The integral: `SAMPLES^2` midpoints of a Quad8 face, the `SAMPLES^2` centroids of a uniform
/// subdivision of a Tri6 face.
fn surface_forces(mesh: &SolidMesh, p: impl Fn(f64) -> f64) -> (Vec<(f64, f64, f64)>, f64) {
    let ns = SAMPLES;
    let mut t: BTreeMap<usize, f64> = BTreeMap::new();
    let mut total = 0.0;
    for &face in mesh.surface_set("top").unwrap() {
        let nodes = mesh.face_nodes(face);
        let corners = match nodes.len() {
            8 => 4,
            6 => 3,
            other => panic!("a face of {other} nodes"),
        };
        // The sample points and their reference areas.
        let mut points: Vec<([f64; 2], f64)> = Vec::new();
        if corners == 4 {
            let d = 2.0 / ns as f64;
            for i in 0..ns {
                for j in 0..ns {
                    points.push(([-1.0 + (i as f64 + 0.5) * d, -1.0 + (j as f64 + 0.5) * d], d * d));
                }
            }
        } else {
            let d = 1.0 / ns as f64;
            for i in 0..ns {
                for j in 0..ns - i {
                    points.push(([(i as f64 + 1.0 / 3.0) * d, (j as f64 + 1.0 / 3.0) * d], 0.5 * d * d));
                    if j + 1 < ns - i {
                        points.push(([(i as f64 + 2.0 / 3.0) * d, (j as f64 + 2.0 / 3.0) * d], 0.5 * d * d));
                    }
                }
            }
        }
        for (uv, wr) in points {
            let (n, dn) = face_shape_functions(nodes.len(), uv).unwrap();
            let (mut x, mut xu, mut xv) = ([0.0; 3], [0.0; 3], [0.0; 3]);
            for (a, &v) in nodes.iter().enumerate() {
                for c in 0..3 {
                    let xc = mesh.nodes()[v][c];
                    x[c] += n[a] * xc;
                    xu[c] += dn[a][0] * xc;
                    xv[c] += dn[a][1] * xc;
                }
            }
            let cross = [
                xu[1] * xv[2] - xu[2] * xv[1],
                xu[2] * xv[0] - xu[0] * xv[2],
                xu[0] * xv[1] - xu[1] * xv[0],
            ];
            let da = wr * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
            let pv = p((x[0] * x[0] + x[1] * x[1]).sqrt());
            if pv == 0.0 {
                continue;
            }
            total += pv * da;
            let phi = weighting(&n, corners);
            for (a, &v) in nodes.iter().enumerate() {
                *t.entry(v).or_insert(0.0) += pv * da * phi[a];
            }
        }
    }
    let forces = t
        .into_iter()
        .map(|(v, f)| (f, mesh.nodes()[v][0], mesh.nodes()[v][1]))
        .collect();
    (forces, total)
}

/// The reading of the pressure `p(r)` on the top surface of `mesh` against the disc radius `a = A_H`: `|a_M/a - 1|`.
fn surface_reading(tag: &str, mesh: &SolidMesh, p: impl Fn(f64) -> f64, exact_total: f64) -> f64 {
    let (forces, total) = surface_forces(mesh, p);
    let sum: f64 = forces.iter().map(|v| v.0).sum();
    let a_m = moment_radius(&forces);
    let err = (a_m / A_H - 1.0).abs();
    println!(
        "{tag}: {} nodes with a force, sum t {sum:.6e}, int p dA {total:.6e} (exact {exact_total:.6e}), a_M {a_m:.6e}, \
         |a_M/a - 1| {err:.6e}",
        forces.iter().filter(|v| v.0 > 0.0).count()
    );
    err
}

fn uniform(r: f64) -> f64 {
    if r <= A_H {
        1.0
    } else {
        0.0
    }
}

fn hertz(r: f64) -> f64 {
    if r < A_H {
        (1.0 - r * r / (A_H * A_H)).sqrt()
    } else {
        0.0
    }
}

/// NC: a uniform pressure on the Hertz disc is read with an error above the limit (continuum `sqrt(5/4) - 1 = 0.118`).
#[test]
fn nc_uniform_pressure_on_the_hertz_disc_is_refused() {
    let quarter = std::f64::consts::PI * A_H * A_H / 4.0;
    let mut missed = Vec::new();
    for (tag, mesh) in [("NC HS20", hs20_mesh()), ("NC HS10", hs10_mesh())] {
        let err = surface_reading(tag, &mesh, uniform, quarter);
        let refused = err.is_finite() && err > LIMIT;
        println!(
            "{tag} uniform pressure: {err:.6e} (limit > {LIMIT:e}) {}; under the limit {LIMIT_ADR0570:e} of ADR 0570 it \
             would {}",
            if refused { "met (refused)" } else { "MISSED" },
            if err <= LIMIT_ADR0570 { "pass" } else { "be refused" }
        );
        if !refused {
            missed.push(format!("{tag} = {err:e} is not above {LIMIT:e}"));
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}

/// HZ: the Hertz pressure on the same two surfaces is read within the limit.
#[test]
fn hz_hertz_pressure_on_the_same_surfaces_is_within_the_limit() {
    let quarter = std::f64::consts::PI * A_H * A_H / 6.0;
    let mut c = Checks::new();
    for (tag, mesh) in [("HZ HS20", hs20_mesh()), ("HZ HS10", hs10_mesh())] {
        let err = surface_reading(tag, &mesh, hertz, quarter);
        c.at_most(&format!("{tag} Hertz pressure, moment radius"), err, LIMIT);
    }
    c.finish();
}

/// HS20m-M', HS10m-M': ADR 0551 HS20m and HS10m (the linear body, the mortar contactor) read by D5 of ADR 0570.
fn hertz_solve(c: &mut Checks, tag: &str, mesh: SolidMesh, h: f64) {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let o = ContactOptions::new(1, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30))
        .quadratic_contactor(QuadraticContactor::Mortar);
    let pair = ContactPair::new(
        "top",
        Target::RigidSphere {
            centre: [0.0, 0.0, R_H],
            radius: R_H,
        },
        100.0 / h,
    );
    let t0 = std::time::Instant::now();
    let r = solve(&model, &pres, &[], &[pair], &o);
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
        "{tag}: {} unknowns, {elapsed:?}, {} augmentations, Newton iterations {:?}, contactor nodes {}, active {}",
        model.dof_map().n_equations(),
        s.solves.len() - 1,
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>(),
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count()
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
    let (a_h, _, _) = hertz_theory(f_fe);
    let forces: Vec<(f64, f64, f64)> = r
        .nodes
        .iter()
        .map(|v| {
            let x = point_reference(&model, v);
            (v.force, x[0], x[1])
        })
        .collect();
    let a_m = moment_radius(&forces);
    println!("{tag}: F_fe {f_fe:.6e}, a_M {a_m:.6e}, a_H {a_h:.6e}");
    c.at_most(
        &format!("{tag} contact radius (moments)"),
        (a_m / a_h - 1.0).abs(),
        LIMIT,
    );
}

#[test]
#[ignore = "heavy record of ADR 0778 (run once): HS20m-M', the moment radius of ADR 0570 HS20m-M against 0.08"]
fn hs20m_m_moment_radius_against_0_08() {
    let mut c = Checks::new();
    hertz_solve(&mut c, "HS20m-M'", hs20_mesh(), A_H / 4.0);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0778 (run once): HS10m-M', the moment radius of ADR 0570 HS10m-M against 0.08"]
fn hs10m_m_moment_radius_against_0_08() {
    let mut c = Checks::new();
    hertz_solve(&mut c, "HS10m-M'", hs10_mesh(), A_H / 3.0);
    c.finish();
}
