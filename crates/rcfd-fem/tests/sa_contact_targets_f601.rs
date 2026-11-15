//! F-601 (ADR 0540 D2): the rigid sphere and cylinder targets. T1s and T1c the tangents, NT-s its negative control, CL0 to
//! CL5 Hertz's line contact of the rigid cylinder on a plane-strain slab under Flamant's far field, HS and HSH Hertz's point
//! contact of the rigid sphere (ADR 0507 D7 H's model), SX the sphere against ADR 0507's plane with offsets, and R3's target
//! rows. Every limit is the one registered in ADR 0540.

mod f601_common;

use f601_common::*;
use rcfd_core::vec3::Vec3;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, ContactSet, Target};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::{ElementKind, FaceRef, FemError, SolidMesh, SolidModel};
use std::f64::consts::PI;

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

// ------------------------------------------------------------------------------------------------------------ T1s, T1c

/// The tangent check of a node contactor against a rigid target; also the error of `eps A n n^T` alone (NT-s).
fn rigid_tangent(target: Target) -> (f64, bool, f64) {
    let m = two_hex(T1_CONTACTOR);
    let pair = ContactPair::new("contactor", target, 10.0);
    let set = ContactSet::new(&m, std::slice::from_ref(&pair), &ContactOptions::new(1, 0.0)).unwrap();
    set.set_multipliers(0, &[0.5, 0.4, 0.6, 0.3]).unwrap();
    let n = m.dof_map().n_equations();
    let u = t1_state(n);
    let (err, same, _, kfd) = tangent_error(&set, &u);
    let dofs = m.dof_map();
    let mut kg = vec![0.0; n * n];
    for v in set.evaluate(&u).unwrap() {
        let e0 = dofs.equation(v.node, 0).unwrap();
        let eps = pair.penalty * v.area;
        for r in 0..3 {
            for s in 0..3 {
                kg[n * (e0 + r) + e0 + s] += eps * v.normal[r] * v.normal[s];
            }
        }
    }
    let kmax = worst(kfd.iter().map(|v| v.abs()));
    let nt = worst(kg.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kmax;
    (err, same, nt)
}

#[test]
fn t1s_t1c_rigid_sphere_and_cylinder_tangents() {
    let mut c = Checks::new();
    let (err, same, nt) = rigid_tangent(Target::RigidSphere {
        centre: [0.5, 0.45, 0.0],
        radius: 0.97,
    });
    c.holds("T1s active set unchanged", same, &format!("{same}"));
    c.at_most("T1s sphere tangent", err, 1e-6);
    c.at_least("NT-s sphere without its curvature part (negative control)", nt, 1e-2);
    let (err, same, _) = rigid_tangent(Target::RigidCylinder {
        point: [0.5, 0.5, 0.0],
        axis: [1.0, 0.2, 0.0],
        radius: 0.97,
    });
    c.holds("T1c active set unchanged", same, &format!("{same}"));
    c.at_most("T1c cylinder tangent", err, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ CL

const B_CL: f64 = 0.1;
const R_CL: f64 = 1.0;

/// Flamant's stress `(sigma_xx, sigma_xz, sigma_zz)` of the line load `p` into `z <= 0` at `(x, z)` (ADR 0540 arithmetic 4).
fn flamant(p: f64, x: f64, z: f64) -> [f64; 3] {
    let r2 = x * x + z * z;
    let c = 2.0 * p * z / (PI * r2 * r2);
    [c * x * x, c * x * z, c * z * z]
}

#[test]
fn cl0_flamant_field() {
    let mut c = Checks::new();
    let mut eq = Vec::new();
    for &(x, z) in &[
        (0.3f64, -0.2f64),
        (0.05, -0.4),
        (-0.2, -0.5),
        (0.01, -0.03),
        (1.0, -0.7),
        (0.0, -0.4),
    ] {
        let r: f64 = (x * x + z * z).sqrt();
        let s = 1e-4 * r;
        let dx = |k: usize| (flamant(1.0, x + s, z)[k] - flamant(1.0, x - s, z)[k]) / (2.0 * s);
        let dz = |k: usize| (flamant(1.0, x, z + s)[k] - flamant(1.0, x, z - s)[k]) / (2.0 * s);
        let r1 = dx(0) + dz(1);
        let r2 = dx(1) + dz(2);
        eq.push((r1 * r1 + r2 * r2).sqrt() / (1.0 / (r * r)));
    }
    c.at_most("CL0 equilibrium", worst(eq), 1e-5);
    let mut tr = Vec::new();
    for &x in &[0.1f64, -0.3, 0.7, 2.0] {
        let s = flamant(1.0, x, 0.0);
        tr.push(s[2].abs().max(s[1].abs()) / (1.0 / x.abs()));
    }
    c.at_most("CL0 traction-free surface", worst(tr), 1e-12);
    let zeta: f64 = 0.3;
    let (s0, s1) = (1e-6f64.ln(), 300f64.ln());
    let nint = 4000;
    let h = (s1 - s0) / nint as f64;
    let mut sum = 0.0;
    for i in 0..=nint {
        let x = (s0 + h * i as f64).exp();
        let w = if i == 0 || i == nint { 0.5 } else { 1.0 };
        sum += w * 2.0 * flamant(1.0, x, -zeta)[2] * x * h;
    }
    c.at_most("CL0 resultant against -P", (sum + 1.0).abs(), 1e-4);
    c.finish();
}

struct Line {
    model: SolidModel,
    prescribed: Vec<Prescribed>,
    forces: Vec<NodalForceAt>,
    applied: f64,
    t: f64,
    h: f64,
}

/// ADR 0540 D5 CL: the plane-strain slab with Flamant's far field of `P_th`.
fn line_model() -> Line {
    let b = B_CL;
    let h = b / 16.0;
    let t = h;
    let xs = graded(h, 20, 12, 10.0 * b);
    let n = xs.len();
    let ys = [0.0, t];
    let id = |i: usize, j: usize, k: usize| i + n * (j + 2 * k);
    let mut nodes: Vec<Vec3> = Vec::new();
    for k in 0..n {
        for y in ys {
            for &x in &xs {
                nodes.push([x, y, -xs[k]]);
            }
        }
    }
    let mut conn = Vec::new();
    let mut top = Vec::new();
    let mut e = 0;
    for k in 0..n - 1 {
        for i in 0..n - 1 {
            let j = 0;
            conn.extend_from_slice(&[
                id(i, j, k + 1),
                id(i + 1, j, k + 1),
                id(i + 1, j + 1, k + 1),
                id(i, j + 1, k + 1),
                id(i, j, k),
                id(i + 1, j, k),
                id(i + 1, j + 1, k),
                id(i, j + 1, k),
            ]);
            if k == 0 {
                top.push(FaceRef { element: e, face: 1 });
            }
            e += 1;
        }
    }
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    mesh.add_element_set("all", (0..e).collect()).unwrap();
    mesh.add_surface_set("top", top).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let mut prescribed = Vec::new();
    for (v, x) in nodes.iter().enumerate() {
        prescribed.push(Prescribed::new(v, 1, 0.0));
        if x[0] == 0.0 {
            prescribed.push(Prescribed::new(v, 0, 0.0));
        }
    }
    // Flamant's traction on x = W and z = -D, consistent nodal forces by 2 x 2 Gauss.
    let p_th = PI * e_star() * b * b / (4.0 * R_CL);
    let mut f = vec![[0.0; 3]; nodes.len()];
    let g = 1.0 / 3f64.sqrt();
    let w_end = xs[n - 1];
    for seg in 0..n - 1 {
        // x = W: the segment z in [-xs[seg + 1], -xs[seg]]; z = -D: x in [xs[seg], xs[seg + 1]].
        for side in 0..2 {
            let (a0, a1) = (xs[seg], xs[seg + 1]);
            for &sa in &[-g, g] {
                let s = 0.5 * (a0 + a1) + 0.5 * (a1 - a0) * sa;
                let jac = 0.5 * (a1 - a0) * 0.5 * t;
                let (pt, tr) = if side == 0 {
                    let sg = flamant(p_th, w_end, -s);
                    ([w_end, -s], [sg[0], 0.0, sg[1]])
                } else {
                    let sg = flamant(p_th, s, -w_end);
                    ([s, -w_end], [-sg[1], 0.0, -sg[2]])
                };
                let _ = pt;
                // The two nodes of the segment (weights linear in s), each on y = 0 and y = t (weight 1/2 times 2 Gauss
                // points in y of weight 1 each: the y integral is exact for a field constant in y).
                let na = [(1.0 - sa) / 2.0, (1.0 + sa) / 2.0];
                let ends = if side == 0 {
                    [(n - 1, seg), (n - 1, seg + 1)]
                } else {
                    [(seg, n - 1), (seg + 1, n - 1)]
                };
                for (q, &(i, k)) in ends.iter().enumerate() {
                    for j in 0..2 {
                        let v = id(i, j, k);
                        for c in 0..3 {
                            // 2 Gauss points in y, each of weight 1 with N = 1/2 at both nodes on average: sum 1.
                            f[v][c] += na[q] * tr[c] * jac * 2.0 * 0.5;
                        }
                    }
                }
            }
        }
    }
    let applied = (2.0 / t) * f.iter().map(|v| v[2]).sum::<f64>();
    println!("CL: P_th {p_th:.6e}, applied resultant (2/t) sum f_z {applied:.6e}");
    let forces = f
        .iter()
        .enumerate()
        .filter(|(_, v)| v.iter().any(|x| *x != 0.0))
        .map(|(i, v)| NodalForceAt::new(i, *v))
        .collect();
    Line {
        model,
        prescribed,
        forces,
        applied,
        t,
        h,
    }
}

#[test]
fn cl1_to_cl5_hertz_line_contact_of_a_rigid_cylinder() {
    let l = line_model();
    let b = B_CL;
    let pair = ContactPair::new(
        "top",
        Target::RigidCylinder {
            point: [0.0, 0.0, R_CL - b * b / (4.0 * R_CL)],
            axis: [0.0, 1.0, 0.0],
            radius: R_CL,
        },
        100.0 / l.h,
    );
    let r: ContactResult = solve(
        &l.model,
        &l.prescribed,
        &l.forces,
        &[pair],
        &ContactOptions::new(1, 1e-8).newton(newton()),
    )
    .unwrap();
    let s = r.steps.last().unwrap();
    println!(
        "CL run: {} Newton solves, iterations {:?}",
        s.solves.len(),
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>()
    );
    let x = l.model.mesh().nodes();
    let p_fe = (2.0 / l.t) * r.nodes.iter().map(|v| v.force * v.normal[2].abs()).sum::<f64>();
    let pmax = r.nodes.iter().map(|v| v.pressure).fold(0.0, f64::max);
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut p_c, mut count) = (f64::NAN, 0);
    for v in &r.nodes {
        let p = x[v.node];
        if p[0] == 0.0 && p[1] == 0.0 {
            p_c = v.pressure;
        }
        if p[1] == 0.0 && v.force > 0.0 {
            count += 1;
        }
        if v.pressure >= 0.5 * pmax {
            let (x2, y) = (p[0] * p[0], v.pressure * v.pressure);
            sw += 1.0;
            sx += x2;
            sy += y;
            sxx += x2 * x2;
            sxy += x2 * y;
        }
    }
    let c1 = (sw * sxy - sx * sy) / (sw * sxx - sx * sx);
    let c0 = (sy - c1 * sx) / sw;
    let b_fe = (-c0 / c1).sqrt();
    let b_h = (4.0 * p_fe * R_CL / (PI * e_star())).sqrt();
    let p_h = 2.0 * p_fe / (PI * b_h);
    println!(
        "CL: P_fe {p_fe:.6e} (applied {:.6e}), b_fe {b_fe:.6e}, b_H {b_h:.6e}, p_c {p_c:.6e}, p_H {p_h:.6e}, nodes on y = 0 {count}",
        l.applied
    );
    let mut c = Checks::new();
    c.at_most("CL1 half-width", (b_fe / b_h - 1.0).abs(), 5e-2);
    c.at_most("CL2 peak pressure", (p_c / p_h - 1.0).abs(), 5e-2);
    c.at_most("CL3 equilibrium", (p_fe / l.applied - 1.0).abs(), 1e-6);
    c.at_least("CL4 nodes on y = 0 in contact", count as f64, 10.0);
    c.at_most("CL5 penetration", s.solves.last().unwrap().penetration, 1e-8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ HS, SX, HSH

fn sphere() -> Target {
    Target::RigidSphere {
        centre: [0.0, 0.0, R_H],
        radius: R_H,
    }
}

/// ADR 0507 H's model on the Hex8 block (`h = a/per_a`) against `target`; with `offsets` the sphere's profile as initial
/// gaps (ADR 0507's representation).
fn hertz_hex8(per_a: usize, nf: usize, m: usize, target: Target, offsets: bool) -> Hertz {
    let mesh = hertz_block(per_a, nf, m);
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let h = A_H / per_a as f64;
    let mut pair = ContactPair::new("top", target, 100.0 / h);
    if offsets {
        for (v, x) in model.mesh().nodes().iter().enumerate() {
            if x[2] == 0.0 {
                let r2 = x[0] * x[0] + x[1] * x[1];
                pair = pair.initial_gap(v, R_H - (R_H * R_H - r2).sqrt());
            }
        }
    }
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
        "H run (per_a {per_a}, offsets {offsets}): {:?}, {} Newton solves, iterations {:?}",
        t0.elapsed(),
        s.solves.len(),
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>()
    );
    hertz_read(&model, &r)
}

fn hertz_checks(c: &mut Checks, tag: &str, h: &Hertz, limits: [f64; 3]) {
    let (a_h, p_h, appr) = hertz_theory(h.force);
    println!(
        "{tag}: F {:.6e}, a_fe {:.6e}, a_H {a_h:.6e}, p_centre {:.6e}, p_H {p_h:.6e}, axis points {}, points {}",
        h.force, h.a_fe, h.p_centre, h.axis_count, h.points
    );
    c.at_most(&format!("{tag} contact radius"), (h.a_fe / a_h - 1.0).abs(), limits[0]);
    c.at_most(
        &format!("{tag} peak pressure"),
        (h.p_centre / p_h - 1.0).abs(),
        limits[1],
    );
    c.at_most(&format!("{tag} approach"), appr, limits[2]);
}

#[test]
fn hs_sx_hertz_rigid_sphere_fast() {
    let mut c = Checks::new();
    let hs = hertz_hex8(6, 8, 6, sphere(), false);
    hertz_checks(&mut c, "HS", &hs, [0.15, 0.20, 0.10]);
    let plane = hertz_hex8(
        6,
        8,
        6,
        Target::RigidPlane {
            point: [0.0; 3],
            normal: [0.0, 0.0, -1.0],
        },
        true,
    );
    println!("SX plane with offsets: {plane:?}");
    c.at_most("SX force", (hs.force / plane.force - 1.0).abs(), 3e-2);
    c.at_most("SX contact radius", (hs.a_fe / plane.a_fe - 1.0).abs(), 3e-2);
    c.at_most("SX peak", (hs.p_centre / plane.p_centre - 1.0).abs(), 3e-2);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0540 HSH (ADR 0507 H's mesh, about 4 minutes and 0.7 GB in release, run once alone; hs_sx_hertz_rigid_sphere_fast covers the path)"]
fn hsh_hertz_rigid_sphere_heavy() {
    let mut c = Checks::new();
    let h = hertz_hex8(12, 15, 9, sphere(), false);
    hertz_checks(&mut c, "HSH", &h, [5e-2, 5e-2, 3e-2]);
    c.at_least("HSH nodes on y = 0 in contact", h.axis_count as f64, 10.0);
    c.at_most("HSH penetration", h.penetration, 1e-8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ R3

#[test]
fn r3_rigid_target_refusals() {
    let mut c = Checks::new();
    let m = two_hex(T1_CONTACTOR);
    let o = ContactOptions::new(1, 0.0);
    let refused = |t: Target, key: &str| {
        let e = ContactSet::new(&m, &[ContactPair::new("contactor", t, 1.0)], &o).err();
        (
            matches!(&e, Some(FemError::InvalidValue { key: k, .. }) if *k == key),
            format!("{e:?}"),
        )
    };
    for r in [0.0, -1.0, f64::NAN] {
        let (ok, d) = refused(
            Target::RigidSphere {
                centre: [0.0; 3],
                radius: r,
            },
            "rigid sphere",
        );
        c.holds(&format!("R3 sphere radius {r}"), ok, &d);
    }
    let (ok, d) = refused(
        Target::RigidSphere {
            centre: [f64::NAN, 0.0, 0.0],
            radius: 1.0,
        },
        "rigid sphere",
    );
    c.holds("R3 sphere centre NaN", ok, &d);
    let (ok, d) = refused(
        Target::RigidCylinder {
            point: [0.0; 3],
            axis: [0.0; 3],
            radius: 1.0,
        },
        "rigid cylinder",
    );
    c.holds("R3 cylinder axis zero", ok, &d);
    let (ok, d) = refused(
        Target::RigidCylinder {
            point: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            radius: 0.0,
        },
        "rigid cylinder",
    );
    c.holds("R3 cylinder radius zero", ok, &d);
    c.finish();
}
