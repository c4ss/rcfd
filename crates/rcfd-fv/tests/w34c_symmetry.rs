//! Wave 34C (ADR 0074 part A): the `Symmetry` boundary, term by term, and exact solutions with a symmetry plane.
//!
//! A1 to A3: the momentum coefficients, the scalar condition and the pressure-correction flux of a `Symmetry` face. A4 to A10: laminar
//! exact solutions (half-channel Poiseuille, mirror equivalence on a graded mesh, the shear-free plug, plane Couette with symmetry side planes,
//! the transient kernel) with the component-consistent diagonal (`component_diag_correction`, ADR 0010) and, as registered expected misses, with
//! the averaged default. Derivation of the discrete half-channel solution (ADR 0074): `u_i = f (h y_i - y_i^2/2)/nu + f dy^2/(8 nu)` and a discrete
//! wall shear `nu u_1/(dy/2) = f h`, both exact for a consistent momentum equation on a uniform mesh.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::pressure_bc;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_mesh, Grading};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::sync::Arc;

const NU: f64 = 0.01;
const F: f64 = 1.0;

/// 2-D mesh (one cell in z, `frontAndBack` empty) from explicit node coordinates; patches xmin, xmax (cyclic pair when `periodic_x`),
/// ymin, ymax.
fn mesh_2d(xs: &[f64], ys: &[f64], periodic_x: bool) -> Mesh {
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let zs = [0.0, 0.1];
    let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut points: Vec<Vec3> = Vec::new();
    for z in zs {
        for &y in ys {
            for &x in xs {
                points.push([x, y, z]);
            }
        }
    }
    let names = ["xmin", "xmax", "ymin", "ymax", "frontAndBack"];
    let mut specs: Vec<BoundarySpec> = names
        .iter()
        .map(|n| BoundarySpec {
            name: n.to_string(),
            kind: if *n == "frontAndBack" {
                PatchKind::Empty
            } else {
                PatchKind::Wall
            },
            faces: vec![],
        })
        .collect();
    let mut cells = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let p: Vec<usize> = (0..8).map(|c| pid(i + (c & 1), j + ((c >> 1) & 1), c >> 2)).collect();
            let faces = vec![
                vec![p[0], p[2], p[6], p[4]],
                vec![p[1], p[3], p[7], p[5]],
                vec![p[0], p[1], p[5], p[4]],
                vec![p[2], p[3], p[7], p[6]],
                vec![p[0], p[1], p[3], p[2]],
                vec![p[4], p[5], p[7], p[6]],
            ];
            let free = [i == 0, i + 1 == nx, j == 0, j + 1 == ny, true, true];
            for (d, f) in faces.iter().enumerate() {
                if free[d] {
                    specs[d.min(4)].faces.push(f.clone());
                }
            }
            cells.push(faces);
        }
    }
    let mut m = Mesh::from_cell_faces(points, &cells, &specs).unwrap();
    if periodic_x {
        m.set_periodic("xmin", "xmax").unwrap();
        m.set_periodic("xmax", "xmin").unwrap();
    }
    m
}

fn geom_of(m: Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

struct Channel {
    geom: Arc<FvGeometry>,
    solver: SimpleSolver,
    iterations: usize,
    du: f64,
}

/// Body-forced periodic channel on the node rows `ys`: `Symmetry` top when `half`, walls at both ends otherwise.
fn channel(
    ys: &[f64],
    half: bool,
    corrected: bool,
    h_from_solution: bool,
    pair_mean: bool,
    record_phi: bool,
) -> (Channel, f64) {
    let xs: Vec<f64> = (0..=4).map(|i| 0.25 * i as f64).collect();
    let geom = geom_of(mesh_2d(&xs, ys, true));
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    if half {
        u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    } else {
        u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.h_from_solution = h_from_solution;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    solver.body_force = Some(BodyForce::uniform(&geom, [F, 0.0, 0.0]));
    solver.component_diag_correction = corrected;
    solver.coupled_body_pair_mean = pair_mean;
    let sym_faces: Vec<usize> = if half {
        geom.mesh.patch("ymax").unwrap().faces().collect()
    } else {
        vec![]
    };
    let mut worst_phi: f64 = 0.0;
    let (mut iterations, mut du) = (0, f64::INFINITY);
    for it in 0..500_000 {
        let info = solver.iterate();
        iterations = it + 1;
        du = info.max_du;
        if record_phi {
            let pbc = pressure_bc(&geom, &solver.u, &solver.p);
            for &f in &sym_faces {
                assert!(pbc.fixed_u[f - geom.n_internal], "Symmetry face {f} not marked fixed_u");
            }
            let scale = solver.phi.iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1e-300);
            for &f in &sym_faces {
                worst_phi = worst_phi.max(solver.phi[f].abs() / scale);
            }
        }
        if it > 10 && du < 1e-12 {
            break;
        }
    }
    (
        Channel {
            geom,
            solver,
            iterations,
            du,
        },
        worst_phi,
    )
}

fn uniform_rows(n: usize, h: f64) -> Vec<f64> {
    (0..=n).map(|j| h * j as f64 / n as f64).collect()
}

/// (y, u_x) of the cells of the column with the smallest x, sorted by y.
fn column(c: &Channel) -> Vec<(f64, f64)> {
    let cc = c.geom.mesh.cell_centres();
    let mut col: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&i| cc[i][0] < 0.25)
        .map(|i| (cc[i][1], c.solver.u.values[i][0]))
        .collect();
    col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    col
}

/// A4 metrics: (max profile error against the derived discrete solution / u_max, tau_w/(f h) - 1).
fn poiseuille_errors(c: &Channel, n: usize) -> (f64, f64) {
    let (h, dy) = (1.0, 1.0 / n as f64);
    let umax = F * h * h / (2.0 * NU);
    let shift = F * dy * dy / (8.0 * NU);
    let col = column(c);
    let err = col
        .iter()
        .map(|&(y, u)| (u - (F * (h * y - 0.5 * y * y) / NU + shift)).abs() / umax)
        .fold(0.0, f64::max);
    let wall: Vec<usize> = c.geom.mesh.patch("ymin").unwrap().faces().collect();
    let own = c.geom.mesh.owner();
    let tau = wall
        .iter()
        .map(|&f| NU * c.solver.u.values[own[f]][0] / c.geom.dn[f - c.geom.n_internal])
        .sum::<f64>()
        / wall.len() as f64;
    (err, tau / (F * h) - 1.0)
}

/// A1: the assembled momentum matrix of each component next to a `Symmetry` face against the plain conditions.
#[test]
fn a1_symmetry_momentum_coefficients_match_zero_gradient_and_zero_value() {
    let geom = geom_of(box_2d(5, 6, 1.0, 1.0, false, false).unwrap());
    let init = |c: Vec3| [1.0 + c[1] * c[1] + 0.3 * c[0], 0.2 * c[0] - 0.1 * c[1], 0.0];
    let mut sym = VectorField::from_fn("U", &geom, init);
    sym.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let mut zg = VectorField::from_fn("U", &geom, init);
    zg.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut fv = VectorField::from_fn("U", &geom, init);
    fv.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let gamma: Vec<f64> = (0..geom.mesh.n_faces()).map(|f| 0.01 + 1e-3 * f as f64).collect();
    let bu = sym.boundary_values(&geom);
    let phi = rcfd_fv::convection::face_flux(&geom, &sym.values, &bu);
    let mat = |fld: &VectorField, k: usize| {
        let cells: Vec<f64> = fld.values.iter().map(|v| v[k]).collect();
        let bc = fld.boundary_coeffs(&geom, k);
        let mut m = convection(&geom, &phi, &cells, &bc, ConvScheme::Upwind, None);
        m.add(&laplacian(&geom, &gamma, &bc, None));
        m
    };
    for (k, other) in [(0, &zg), (1, &fv)] {
        let (a, b) = (mat(&sym, k), mat(other, k));
        assert!(a.a.diag == b.a.diag, "component {k}: diagonal differs");
        assert!(
            a.a.upper == b.a.upper && a.a.lower == b.a.lower,
            "component {k}: off-diagonal differs"
        );
        assert!(a.b == b.b, "component {k}: right-hand side differs");
    }
    // the symmetry face carries no convective flux
    for f in geom.mesh.patch("ymax").unwrap().faces() {
        assert!(phi[f].abs() < 1e-15, "flux through the symmetry face {}", phi[f]);
    }
}

/// A2: on a scalar the `Symmetry` condition is the zero-gradient condition (the k and omega equations of the SST).
#[test]
fn a2_symmetry_on_a_scalar_is_zero_gradient() {
    let geom = geom_of(box_2d(5, 6, 1.0, 1.0, false, false).unwrap());
    let init = |c: Vec3| 1.0 + c[1] * c[1] + 0.3 * c[0];
    let mut a = ScalarField::from_fn("k", &geom, init);
    a.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let mut b = ScalarField::from_fn("k", &geom, init);
    b.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let (ca, cb) = (a.boundary_coeffs(&geom), b.boundary_coeffs(&geom));
    let gamma = uniform_gamma(&geom, 0.02);
    let (ma, mb) = (laplacian(&geom, &gamma, &ca, None), laplacian(&geom, &gamma, &cb, None));
    assert!(ma.a.diag == mb.a.diag && ma.b == mb.b);
    assert!(a.boundary_values(&geom) == b.boundary_values(&geom));
}

/// A3: the Symmetry faces are fixed-flux faces of the pressure equation and carry zero flux in every iteration.
#[test]
fn a3_symmetry_faces_carry_no_flux_in_every_iteration() {
    let (c, worst) = channel(&uniform_rows(40, 1.0), true, true, true, false, true);
    println!(
        "A3: {} iterations, worst |phi| on the symmetry faces / max |phi| = {worst:e}",
        c.iterations
    );
    assert!(worst <= 1e-15, "A3: {worst:e}");
}

fn a4_case(corrected: bool, hfs: bool) -> (f64, f64, usize, f64) {
    let (c, _) = channel(&uniform_rows(40, 1.0), true, corrected, hfs, false, false);
    let (e, t) = poiseuille_errors(&c, 40);
    (e, t, c.iterations, c.du)
}

/// A4: half-channel Poiseuille with the corrected diagonal reproduces the derived discrete solution and the exact wall shear
/// (`h_from_solution` on, the `SimpleSettings` default).
#[test]
fn a4_half_channel_poiseuille_corrected() {
    let (e, t, n, du) = a4_case(true, true);
    println!(
        "A4 corrected, h_from_solution true: profile error {e:e}, tau_w/(f h) - 1 = {t:e} ({n} iterations, du {du:e})"
    );
    assert!(du < 1e-12, "A4: not converged");
    assert!(e <= 1e-8 && t.abs() <= 1e-8, "A4: {e:e} {t:e}");
}

/// A4b: the same with `h_from_solution` off. The corrected iteration converges slowly in this mode (du falls by about e per 10000
/// iterations; ADR 0074 addendum), so the run needs about 200000 iterations.
#[test]
#[ignore = "heavy: about 4 minutes in release (slow convergence with h_from_solution off)"]
fn a4b_half_channel_poiseuille_corrected_h_from_solution_off() {
    let (e, t, n, du) = a4_case(true, false);
    println!("A4b corrected, h_from_solution false: profile error {e:e}, tau_w/(f h) - 1 = {t:e} ({n} iterations, du {du:e})");
    assert!(du < 1e-12, "A4b: not converged");
    assert!(e <= 1e-8 && t.abs() <= 1e-8, "A4b: {e:e} {t:e}");
}

/// A4m: the same with the averaged default (registered expected miss). KNOWN MISS (ADR 0074 addendum): h_from_solution off: profile error
/// 0.978 of u_max, tau_w/(f h) - 1 = -0.495, u_max 12.754, top cell u 1.114; on: 0.920, -0.442, u_max 15.570, top cell 4.021 (exact u_max 50):
/// the Symmetry top acts like a second no-slip wall.
#[test]
#[ignore = "known miss (registered expectation): averaged diagonal, see ADR 0074 addendum"]
fn a4m_half_channel_poiseuille_averaged_known_miss() {
    let mut fails = Vec::new();
    for hfs in [false, true] {
        let (e, t, n, du) = a4_case(false, hfs);
        let c = channel(&uniform_rows(40, 1.0), true, false, hfs, false, false).0;
        let col = column(&c);
        println!(
            "A4m averaged, h_from_solution {hfs}: profile error {e:e}, tau_w/(f h) - 1 = {t:e}, u_max {:.4}, top cell u {:.4} ({n} iterations, du {du:e})",
            col.iter().map(|q| q.1).fold(0.0, f64::max),
            col.last().unwrap().1
        );
        if e > 1e-8 || t.abs() > 1e-8 {
            fails.push(format!("h_from_solution {hfs}: {e:e} {t:e}"));
        }
    }
    assert!(fails.is_empty(), "A4m: {fails:?}");
}

/// Residual of the x-momentum equation of every cell (diffusion, convection, body force, cell pressure gradient), relative to f V.
fn own_residual(c: &Channel) -> (f64, f64) {
    let g = &c.geom;
    let s = &c.solver;
    let cells: Vec<f64> = s.u.values.iter().map(|v| v[0]).collect();
    let bc = s.u.boundary_coeffs(g, 0);
    let mut m = convection(g, &s.phi, &cells, &bc, s.settings.convection, None);
    m.add(&laplacian(g, &uniform_gamma(g, NU), &bc, None));
    let r = m.residual(&cells);
    let gp = green_gauss(g, &s.p.values, &s.p.boundary_values(g), 0);
    let vol = g.mesh.cell_volumes();
    let cc = g.mesh.cell_centres();
    let (mut worst, mut top) = (0.0_f64, 0.0_f64);
    let ymax = cc.iter().map(|c| c[1]).fold(0.0, f64::max);
    for i in 0..r.len() {
        let res = (r[i] + vol[i] * (F - gp[i][0])) / (F * vol[i]);
        worst = worst.max(res.abs());
        if (cc[i][1] - ymax).abs() < 1e-12 {
            top = top.max(res.abs());
        }
    }
    (worst, top)
}

/// A5: the converged corrected state satisfies the cell x-momentum balance; the averaged state is reported.
#[test]
fn a5_own_component_momentum_residual() {
    let (c, _) = channel(&uniform_rows(40, 1.0), true, true, true, false, false);
    let (w, t) = own_residual(&c);
    let (ca, _) = channel(&uniform_rows(40, 1.0), true, false, true, false, false);
    let (wa, ta) = own_residual(&ca);
    println!("A5: corrected worst {w:e} (top row {t:e}); averaged worst {wa:e} (top row {ta:e})");
    assert!(w <= 1e-8, "A5 corrected: {w:e}");
}

/// A6: on a graded mesh the half channel equals the lower half of the mirrored full channel.
#[test]
fn a6_mirror_equivalence_graded() {
    let ys = axis_coords(
        40,
        1.0,
        Grading::WallLayers {
            first: 0.005,
            growth: 1.1,
            layers: 20,
            both: false,
        },
    )
    .unwrap();
    let mut full = ys.clone();
    for &y in ys.iter().rev().skip(1) {
        full.push(2.0 - y);
    }
    let (h, _) = channel(&ys, true, true, true, false, false);
    let (f, _) = channel(&full, false, true, true, false, false);
    let (ch, cf) = (column(&h), column(&f));
    let umax = ch.iter().map(|q| q.1).fold(0.0, f64::max);
    let mut worst: f64 = 0.0;
    for (a, b) in ch.iter().zip(&cf) {
        assert!((a.0 - b.0).abs() < 1e-12);
        worst = worst.max((a.1 - b.1).abs() / umax);
    }
    println!(
        "A6: half {} / full {} iterations, max |u_half - u_full|/u_max = {worst:e}, u_max {umax:.6}",
        h.iterations, f.iterations
    );
    assert!(worst <= 1e-8, "A6: {worst:e}");
}

/// Shear-free plug: bottom wall moving at 1, `Symmetry` top, periodic x, from rest. Returns max |u - 1| and iterations.
fn plug(corrected: bool) -> (f64, usize) {
    let geom = geom_of(box_2d(4, 20, 1.0, 1.0, true, false).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSolver::new(geom.clone(), u, p, SimpleSettings::new(NU, Coupling::Simplec));
    s.component_diag_correction = corrected;
    let mut n = 0;
    for it in 0..60000 {
        n = it + 1;
        if s.iterate().max_du < 1e-14 && it > 10 {
            break;
        }
    }
    let e = s.u.values.iter().map(|v| (v[0] - 1.0).abs()).fold(0.0, f64::max);
    (e, n)
}

/// A7: the plug with the corrected diagonal is exact.
#[test]
fn a7_shear_free_plug_corrected() {
    let (e, n) = plug(true);
    println!("A7 corrected: max |u - U_w|/U_w = {e:e} ({n} iterations)");
    assert!(e <= 1e-8, "A7: {e:e}");
}

/// A7m: the plug with the averaged default (registered expected miss). KNOWN MISS (ADR 0074 addendum): max |u - U_w|/U_w = 0.813.
#[test]
#[ignore = "known miss (registered expectation): averaged diagonal, see ADR 0074 addendum"]
fn a7m_shear_free_plug_averaged_known_miss() {
    let (e, n) = plug(false);
    println!("A7m averaged: max |u - U_w|/U_w = {e:e} ({n} iterations)");
    assert!(e <= 1e-8, "A7m: {e:e}");
}

/// Plane Couette in a 3-D box with `Symmetry` side planes (z), periodic x: (max |u_x - y|, max(|u_y|, |u_z|)).
fn couette_3d(corrected: bool) -> (f64, f64, usize) {
    let mut m = box_mesh([4, 16, 4], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    let geom = geom_of(m);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "zmin", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "zmax", Box::new(Symmetry)).unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSolver::new(geom.clone(), u, p, SimpleSettings::new(NU, Coupling::Simplec));
    s.component_diag_correction = corrected;
    let mut n = 0;
    for it in 0..60000 {
        n = it + 1;
        if s.iterate().max_du < 1e-14 && it > 10 {
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let (mut ex, mut et): (f64, f64) = (0.0, 0.0);
    for (c, v) in cc.iter().zip(&s.u.values) {
        ex = ex.max((v[0] - c[1]).abs());
        et = et.max(v[1].abs()).max(v[2].abs());
    }
    (ex, et, n)
}

/// A8: plane Couette with symmetry side planes is exact with the corrected diagonal; the averaged value is printed (no limit).
#[test]
fn a8_couette_with_symmetry_side_planes() {
    let (ex, et, n) = couette_3d(true);
    let (ax, at, na) = couette_3d(false);
    println!("A8 corrected: |u_x - U y/h| {ex:e}, |u_y|,|u_z| {et:e} ({n} it); averaged: {ax:e}, {at:e} ({na} it)");
    assert!(ex <= 1e-8 && et <= 1e-10, "A8: {ex:e} {et:e}");
}

/// Transient plug started at the exact solution, 40 PISO steps of 0.05: max |u - 1|.
fn plug_piso(corrected: bool) -> f64 {
    let geom = geom_of(box_2d(4, 20, 1.0, 1.0, true, false).unwrap());
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSettings::piso(NU);
    s.component_diagonal = corrected;
    let mut sol = PimpleSolver::new(geom, u, p, s);
    for _ in 0..40 {
        sol.step(0.05);
    }
    sol.u.values.iter().map(|v| (v[0] - 1.0).abs()).fold(0.0, f64::max)
}

/// A9: the transient kernel keeps the exact plug with its component-consistent split.
#[test]
fn a9_transient_plug_stays_exact() {
    let (c, a) = (plug_piso(true), plug_piso(false));
    println!("A9 PISO: corrected max |u - U_w| {c:e}, averaged {a:e}");
    assert!(c <= 1e-10, "A9: {c:e}");
}

/// A10: `coupled_body_pair_mean` (ADR 0071) is unrelated to the half-channel result (registered prediction: identical to 1e-12).
/// KNOWN MISS (ADR 0074 addendum): with the averaged diagonal the converged half channel is NOT x-invariant (a spurious secondary flow,
/// max |u_y| 0.026, u_x varying by 0.73 along the 4 periodic cells), so the owner-value body-force term of the periodic faces leaves a continuity
/// defect of 4.0e-5 that the pair mean removes (2e-14); u_max 15.5739 against 15.5756 (1.2e-4 relative). With the corrected diagonal the state is
/// x-invariant to 1e-12.
#[test]
#[ignore = "known miss of the registered prediction, see doc comment"]
fn a10_coupled_body_pair_mean_is_unrelated_known_miss() {
    let (a, _) = channel(&uniform_rows(40, 1.0), true, false, true, false, false);
    let (b, _) = channel(&uniform_rows(40, 1.0), true, false, true, true, false);
    let umax = a.solver.u.values.iter().map(|v| v[0]).fold(0.0, f64::max);
    let d = a
        .solver
        .u
        .values
        .iter()
        .zip(&b.solver.u.values)
        .map(|(x, y)| (x[0] - y[0]).abs())
        .fold(0.0, f64::max)
        / umax;
    println!(
        "A10: max relative difference {d:e} ({} / {} iterations)",
        a.iterations, b.iterations
    );
    assert!(d <= 1e-12, "A10: {d:e}");
}
