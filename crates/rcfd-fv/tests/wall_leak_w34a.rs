//! Wave 34A (ADR 0072, T1): circular Couette flow between faceted cylinders whose nodes are moved in angle by a non-symmetric
//! pattern, so the wall facets are warped and their normals are not perpendicular to the wall velocity. The exact solution
//! `u_theta = A r + B/r` has zero wall-normal velocity. Each mesh is solved with the wall velocity at the face centres (the 33A
//! treatment) and with its tangential projection ([`rcfd_fv::bc::tangential_projection`]). The limits were registered before
//! this code ran; a miss is moved unchanged into an ignored `*_known_miss` test.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{tangential_projection, FixedValue};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_mesh::mesh::{BoundarySpec, Mesh, PatchKind};
use std::f64::consts::PI;
use std::sync::Arc;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const LZ: f64 = 0.5;
const NU: f64 = 0.1;
const W_I: f64 = 1.0;
const V_I: f64 = 0.5;
const EPS: f64 = 0.25;
const SIGMA: [f64; 3] = [1.0, -0.5, 0.3];
const MESHES: [(usize, usize, usize); 3] = [(24, 6, 4), (48, 12, 8), (96, 24, 16)];

fn tc(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -W_I * RI * RI / d * r + W_I * RI * RI * RO * RO / d / r
}

fn hex_faces(p: [usize; 8]) -> [Vec<usize>; 6] {
    [
        vec![p[0], p[2], p[6], p[4]],
        vec![p[1], p[3], p[7], p[5]],
        vec![p[0], p[1], p[5], p[4]],
        vec![p[2], p[3], p[7], p[6]],
        vec![p[0], p[1], p[3], p[2]],
        vec![p[4], p[5], p[7], p[6]],
    ]
}

/// Faceted annulus of ADR 0072 T1 (z-periodic translational pair `zmin` / `zmax`).
fn faceted_annulus(n_t: usize, n_r: usize, n_z: usize) -> Mesh {
    let pid = |j: usize, i: usize, k: usize| j + (n_r + 1) * ((i % n_t) + n_t * k);
    let mut points = Vec::new();
    for k in 0..=n_z {
        let z = LZ * k as f64 / n_z as f64;
        for i in 0..n_t {
            let th = 2.0 * PI * i as f64 / n_t as f64
                + EPS * (2.0 * PI / n_t as f64) * SIGMA[i % 3] * (2.0 * PI * k as f64 / n_z as f64).sin();
            for j in 0..=n_r {
                let r = RI + (RO - RI) * j as f64 / n_r as f64;
                points.push([r * th.cos(), r * th.sin(), z]);
            }
        }
    }
    let (mut cells, mut inner, mut outer, mut zmin, mut zmax) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..n_z {
        for i in 0..n_t {
            for j in 0..n_r {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = pid(j + (c & 1), i + ((c >> 1) & 1), k + (c >> 2));
                }
                let f = hex_faces(p);
                if j == 0 {
                    inner.push(f[0].clone());
                }
                if j + 1 == n_r {
                    outer.push(f[1].clone());
                }
                if k == 0 {
                    zmin.push(f[4].clone());
                }
                if k + 1 == n_z {
                    zmax.push(f[5].clone());
                }
                cells.push(f.to_vec());
            }
        }
    }
    let spec = |name: &str, kind: PatchKind, faces: Vec<Vec<usize>>| BoundarySpec {
        name: name.into(),
        kind,
        faces,
    };
    let mut mesh = Mesh::from_cell_faces(
        points,
        &cells,
        &[
            spec("inner", PatchKind::Wall, inner),
            spec("outer", PatchKind::Wall, outer),
            spec("zmin", PatchKind::Generic, zmin),
            spec("zmax", PatchKind::Generic, zmax),
        ],
    )
    .unwrap();
    mesh.set_periodic("zmin", "zmax").unwrap();
    mesh.set_periodic("zmax", "zmin").unwrap();
    mesh
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    converged: bool,
    iterations: usize,
    /// solver continuity measure of the last iteration
    continuity: f64,
    /// max |u_theta - exact| / V_i
    e: f64,
    /// max(|u_r|, |u_z|) / V_i
    spurious: f64,
    /// max over inner-wall faces |phi_f| / (|u_w| |S_f|)
    leak: f64,
    /// sum over inner-wall faces |phi_f|
    leak_sum: f64,
    /// largest |u_w . n| / |u_w| of the face-centre wall velocity
    tilt: f64,
}

/// `corrected`: the convergence criterion of ADR 0072 Amendment 2 for the unprojected arm (whose closed-domain pressure equation is
/// incompatible by the net facet flux): `res_u < 1e-10` and `max_du < 1e-12 V_i`. Otherwise (and always for the projected arm) the
/// registered `res_u < 1e-10` and continuity `< 1e-9`.
fn solve(n_t: usize, n_r: usize, n_z: usize, project: bool, corrected: bool) -> Outcome {
    solve_mode(n_t, n_r, n_z, project, corrected, false)
}

/// `impermeable` (ADR 0072 Amendment 3, decoupled arm): the face-centre wall velocity in the momentum condition and zero mass
/// flux on the inner wall ([`SimpleSolver::impermeable`]); converged on the registered criterion.
fn solve_mode(n_t: usize, n_r: usize, n_z: usize, project: bool, corrected: bool, impermeable: bool) -> Outcome {
    let mesh = faceted_annulus(n_t, n_r, n_z);
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (cf, sf) = (geom.mesh.face_centres(), geom.mesh.face_area_vectors());
    let inner: Vec<usize> = geom.mesh.patch("inner").unwrap().faces().collect();
    let uw: Vec<Vec3> = inner.iter().map(|&f| [-W_I * cf[f][1], W_I * cf[f][0], 0.0]).collect();
    let s_in: Vec<Vec3> = inner.iter().map(|&f| sf[f]).collect();
    let (proj, tilt) = tangential_projection(&uw, &s_in);
    let wall = if project { proj } else { uw.clone() };
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "inner", Box::new(FixedValue(Profile::PerFace(wall))))
        .unwrap();
    u.set_bc(&geom, "outer", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    if impermeable {
        let mut mask = vec![false; geom.mesh.n_boundary_faces()];
        for &f in &inner {
            mask[f - geom.n_internal] = true;
        }
        solver.impermeable = Some(mask);
    }
    let (mut converged, mut iterations, mut continuity) = (false, 0, f64::NAN);
    for it in 1..=6000 {
        let info = solver.iterate();
        iterations = it;
        let done = if corrected && !project && !impermeable {
            info.res_u < 1e-10 && info.max_du < 1e-12 * V_I
        } else {
            info.res_u < 1e-10 && info.continuity < 1e-9
        };
        if done {
            converged = true;
            continuity = info.continuity;
            break;
        }
        continuity = info.continuity;
    }
    let cc = geom.mesh.cell_centres();
    let (mut e, mut spurious): (f64, f64) = (0.0, 0.0);
    for (c, v) in solver.u.values.iter().enumerate() {
        let (x, y) = (cc[c][0], cc[c][1]);
        let r = x.hypot(y);
        let ut = (-y * v[0] + x * v[1]) / r;
        let ur = (x * v[0] + y * v[1]) / r;
        e = e.max((ut - tc(r)).abs() / V_I);
        spurious = spurious.max(ur.abs().max(v[2].abs()) / V_I);
    }
    let (mut leak, mut leak_sum): (f64, f64) = (0.0, 0.0);
    for (q, &f) in inner.iter().enumerate() {
        leak = leak.max(solver.phi[f].abs() / (vec3::norm(uw[q]) * vec3::norm(sf[f])));
        leak_sum += solver.phi[f].abs();
    }
    Outcome {
        converged,
        iterations,
        continuity,
        e,
        spurious,
        leak,
        leak_sum,
        tilt,
    }
}

fn orders(e: &[f64]) -> Vec<f64> {
    e.windows(2).map(|w| (w[0] / w[1]).log2()).collect()
}

/// (unprojected, projected) outcomes on the three meshes, printed.
fn t1_runs(corrected: bool) -> Vec<(Outcome, Outcome)> {
    let mut out = Vec::new();
    for (n_t, n_r, n_z) in MESHES {
        let a = solve(n_t, n_r, n_z, false, corrected);
        let b = solve(n_t, n_r, n_z, true, corrected);
        for (tag, o) in [("unprojected", a), ("projected", b)] {
            println!(
                "T1 ({n_t}, {n_r}, {n_z}) {tag}: converged {} in {} iterations (continuity {:.2e}); e {:.4e} V_i; spurious {:.3e} V_i; wall leak max {:.3e}, sum {:.3e}; max |u_w.n|/|u_w| {:.3e}",
                o.converged, o.iterations, o.continuity, o.e, o.spurious, o.leak, o.leak_sum, o.tilt
            );
        }
        out.push((a, b));
    }
    let ea: Vec<f64> = out.iter().map(|o| o.0.e).collect();
    let eb: Vec<f64> = out.iter().map(|o| o.1.e).collect();
    println!(
        "T1 observed orders: unprojected {:?}, projected {:?}",
        orders(&ea),
        orders(&eb)
    );
    out
}

/// T1 as registered: positive control, projected leak <= 1e-13, finest projected error <= 1.5e-2 V_i, orders >= 1.0, and the projected
/// error not above the unprojected one on every mesh; all runs converged with `res_u < 1e-10` and continuity `< 1e-9`.
#[test]
#[ignore = "set-up defect (ADR 0072 Amendment 2): the unprojected arm's net facet flux makes the closed pressure equation incompatible, its continuity cannot fall below 1.6e-7; see t1_corrected_faceted_annulus_couette_projection"]
fn t1_faceted_annulus_couette_projection() {
    t1_gates(&t1_runs(false));
}

/// T1 corrected (ADR 0072 Amendment 2): the unprojected arm converges on res_u and the velocity change; all gates unchanged.
/// KNOWN MISS: the positive control (tilt >= 1e-4) fails on the finest mesh (4.7e-5, the tilt falls as h^2) and the projected error is
/// above the unprojected one on all three meshes (ADR 0072 addendum).
#[test]
#[ignore = "known miss: finest-mesh positive control 4.7e-5 < 1e-4 and projected error above unprojected on every mesh, see ADR 0072 addendum"]
fn t1_corrected_faceted_annulus_couette_projection_known_miss() {
    t1_gates(&t1_runs(true));
}

/// T1d (ADR 0072 Amendment 3): the decoupled impermeable wall on the three meshes against the unprojected arm (corrected criterion).
/// Gates: converged, wall leak <= 1e-13, finest error <= 1.5e-2 V_i, orders >= 1.0, error not above the unprojected arm's on every mesh.
#[test]
#[ignore = "known miss: the decoupled error is above the unprojected one on every mesh (3.6252e-2 vs 3.6000e-2, 1.0177e-2 vs 1.0154e-2, 2.7158e-3 vs 2.7145e-3 V_i), see ADR 0072 addendum"]
fn t1d_decoupled_impermeable_wall_known_miss() {
    let mut ea = Vec::new();
    let mut ed = Vec::new();
    for (n_t, n_r, n_z) in MESHES {
        let a = solve(n_t, n_r, n_z, false, true);
        let d = solve_mode(n_t, n_r, n_z, false, true, true);
        for (tag, o) in [("unprojected", a), ("decoupled", d)] {
            println!(
                "T1d ({n_t}, {n_r}, {n_z}) {tag}: converged {} in {} iterations (continuity {:.2e}); e {:.4e} V_i; spurious {:.3e} V_i; wall leak max {:.3e}",
                o.converged, o.iterations, o.continuity, o.e, o.spurious, o.leak
            );
        }
        assert!(a.converged && d.converged, "not converged");
        assert!(d.leak <= 1e-13, "decoupled wall leak {}", d.leak);
        ea.push(a.e);
        ed.push(d.e);
    }
    println!(
        "T1d observed orders: unprojected {:?}, decoupled {:?}",
        orders(&ea),
        orders(&ed)
    );
    assert!(ed[2] <= 1.5e-2);
    for o in orders(&ed) {
        assert!(o >= 1.0, "order {o}");
    }
    for (a, d) in ea.iter().zip(&ed) {
        assert!(d <= a, "decoupled error {d} above the unprojected {a}");
    }
}

fn t1_gates(runs: &[(Outcome, Outcome)]) {
    for (a, b) in runs {
        assert!(a.converged && b.converged, "not converged");
        assert!(
            a.tilt >= 1e-4,
            "positive control: the facets are not tilted enough ({})",
            a.tilt
        );
        assert!(b.leak <= 1e-13, "projected wall leak {}", b.leak);
    }
    let eb: Vec<f64> = runs.iter().map(|o| o.1.e).collect();
    assert!(eb[2] <= 1.5e-2, "finest projected error {}", eb[2]);
    for o in orders(&eb) {
        assert!(o >= 1.0, "order {o}");
    }
    for (a, b) in runs {
        assert!(
            b.e <= a.e,
            "the projected error {} is above the unprojected {}",
            b.e,
            a.e
        );
    }
}

/// Geometry only: the net face-centre wall flux of the unprojected inner wall (the closed domain has no Dirichlet pressure, so a
/// nonzero net makes the pressure equation incompatible), and the continuity normalisation of the solver.
#[test]
fn t1_net_unprojected_wall_flux_geometry() {
    for (n_t, n_r, n_z) in MESHES {
        let mesh = faceted_annulus(n_t, n_r, n_z);
        let (cf, sf) = (mesh.face_centres(), mesh.face_area_vectors());
        let (mut net, mut abs) = (0.0, 0.0);
        for f in mesh.patch("inner").unwrap().faces() {
            let q = vec3::dot([-W_I * cf[f][1], W_I * cf[f][0], 0.0], sf[f]);
            net += q;
            abs += q.abs();
        }
        let area_scale = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
        println!(
            "T1 geometry ({n_t}, {n_r}, {n_z}): net sum u_w(c_f).S_f {net:.3e}, sum of magnitudes {abs:.3e}; net / max|S_f| = {:.3e} (the solver's continuity measure of an incompatible system)",
            net.abs() / area_scale
        );
    }
}

#[test]
fn tangential_projection_removes_the_normal_part_only() {
    let u = [[1.0, 2.0, 3.0], [0.0, 0.0, 0.0], [5.0, -1.0, 0.5]];
    let s = [[0.0, 0.0, 2.0], [1.0, 1.0, 0.0], [0.3, 0.4, -0.2]];
    let (p, worst) = tangential_projection(&u, &s);
    assert_eq!(p[0], [1.0, 2.0, 0.0]);
    assert_eq!(p[1], [0.0; 3]);
    assert!(vec3::dot(p[2], s[2]).abs() <= 1e-15 * vec3::norm(u[2]) * vec3::norm(s[2]));
    let t = vec3::sub(u[2], p[2]);
    assert!(vec3::norm(vec3::cross(t, s[2])) <= 1e-15 * vec3::norm(t) * vec3::norm(s[2]));
    // the largest removed fraction is that of the first face, 3 / sqrt(14)
    assert!((worst - 3.0 / 14f64.sqrt()).abs() < 1e-15, "{worst}");
}
