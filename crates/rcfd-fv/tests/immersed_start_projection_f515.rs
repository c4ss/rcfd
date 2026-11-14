//! F-515 (ADR 0425): the immersed start projection in the library and the refusal of ADR 0374 lifted for it.
//!
//! `rcfd_fv::immersed_start::project_start` is ADR 0421 D1 (the solid cells at rest, the fluid-solid faces closed, the flux
//! projected onto the fluid region), with the outlet read from the pressure condition; `PimpleSolver::project_immersed_start`
//! puts the solver at it with the projected face flux and lifts the refusal of a reversed flux face with a single PISO
//! corrector (ADR 0374 D2). Fast checks: P1 (the library against the F-501 test code, copied here as the reference, to the
//! bit), P2 (the flux passed to the solver), E1 (the errors), RF1 to RF4 (the refusal kept by default, lifted, cleared, and the
//! trial of the pre-run estimate). Heavy records (`#[ignore]`, run once in release): H1 (B with one corrector, 200 steps, the
//! F-501 number), L2 (Sb with one corrector, 200 steps). The DFG helpers and the reference projection are copies of those of
//! `immersed_early_state_start_f501.rs` (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::immersed_start::project_start;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_single_corrector_f363.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `build_second_order_bounded`, `theta_min = 0.01` (ADR 0374 B).
    B,
    /// `build_second_order`, `theta_min = 0.25` (ADR 0374 Sb: weights up to 2, no image).
    Sb,
}

/// The forcing; `check` is `flux_weight_check` (`true` from the constructors).
fn forcing(geom: &FvGeometry, bodies: &[ImmersedBody], f: Forcing, check: bool) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, NU);
    let st = |theta_min: f64| {
        let mut s = ImmersedSettings::new(K_DFG);
        s.theta_min = theta_min;
        s
    };
    let mut ib = match f {
        Forcing::B => ImmersedForcing::build_second_order_bounded(geom, bodies, st(1e-2), &gamma),
        Forcing::Sb => ImmersedForcing::build_second_order(geom, bodies, st(0.25), &gamma),
    }
    .unwrap();
    ib.flux_weight_check = check;
    ib
}

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

fn dfg_body() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    )
}

fn dfg_fields(xs: &[f64], ys: &[f64], um: f64) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(xs, ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (geom, u, p)
}

fn two_d_two_start(geom: &FvGeometry, u: &mut VectorField, um: f64) {
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
}

/// The set-up R of ADR 0374 on `dfg_family(level)` with `n_correctors` correctors, the forcing `f` and `flux_weight_check`
/// `check`.
fn r_solver(level: usize, f: Forcing, n_correctors: usize, check: bool) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = dfg_family(level);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let mut st = PimpleSettings::piso(NU);
    st.n_correctors = n_correctors;
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(forcing(&geom, &[dfg_body()], f, check));
    (geom, s)
}

fn max_abs_u(s: &PimpleSolver) -> f64 {
    s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The reference: ADR 0421 D1 as test code, copied from immersed_early_state_start_f501.rs
// ---------------------------------------------------------------------------------------------------------------------------

/// Per cell: solid (the forcing's body holds the cell centre).
fn solid_cells(ib: &ImmersedForcing) -> Vec<bool> {
    ib.body_of_cell.iter().map(|b| b.is_some()).collect()
}

/// The faces of the projection (D1 3): the open internal faces `(face, owner, neighbour, a_f)` and the outlet faces
/// `(face, owner, a_f)`; every other face is closed.
struct OpenFaces {
    internal: Vec<(usize, usize, usize, f64)>,
    outlet: Vec<(usize, usize, f64)>,
}

fn open_faces(geom: &FvGeometry, solid: &[bool]) -> OpenFaces {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let cc = mesh.cell_centres();
    let fc = mesh.face_centres();
    let closed = |f: usize| solid[own[f]] || solid[nei[f]];
    let mut internal = Vec::new();
    for f in 0..geom.n_internal {
        if closed(f) {
            continue;
        }
        let (o, n) = (own[f], nei[f]);
        let d = [cc[n][0] - cc[o][0], cc[n][1] - cc[o][1], cc[n][2] - cc[o][2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        internal.push((f, o, n, mesh.face_area(f) / dist));
    }
    let mut outlet = Vec::new();
    for f in mesh.patch("xmax").unwrap().faces() {
        let o = own[f];
        let d = [fc[f][0] - cc[o][0], fc[f][1] - cc[o][1], fc[f][2] - cc[o][2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        outlet.push((f, o, mesh.face_area(f) / dist));
    }
    OpenFaces { internal, outlet }
}

/// Per cell `sum_f phi_f` out of the cell (empty faces carry zero flux).
fn divergence(geom: &FvGeometry, phi: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut d = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        d[own[f]] += phi[f];
        if f < geom.n_internal {
            d[nei[f]] -= phi[f];
        }
    }
    d
}

/// S1: `max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)`.
fn divergence_ratio(geom: &FvGeometry, phi: &[f64]) -> f64 {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut scale = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        scale[own[f]] += phi[f].abs();
        if f < geom.n_internal {
            scale[nei[f]] += phi[f].abs();
        }
    }
    let d = divergence(geom, phi);
    let dmax = d.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let smax = scale.iter().fold(0.0f64, |a, x| a.max(*x));
    dmax / smax
}

/// D1 3: the projection of a face flux onto the fluid region (closed faces kept), by Jacobi-preconditioned CG on
/// `sum_f a_f (psi_P - psi_N) = -sum_f phi*_f`. Returns the corrected flux, `psi` and the CG iterations.
fn project_flux(geom: &FvGeometry, open: &OpenFaces, phi_star: &[f64]) -> (Vec<f64>, Vec<f64>, usize) {
    let nc = geom.n_cells();
    let mut diag = vec![0.0; nc];
    for &(_, o, n, a) in &open.internal {
        diag[o] += a;
        diag[n] += a;
    }
    for &(_, o, a) in &open.outlet {
        diag[o] += a;
    }
    // Cells without an open face (the solid ones) are decoupled: psi = 0.
    for d in diag.iter_mut() {
        if *d == 0.0 {
            *d = 1.0;
        }
    }
    let apply = |x: &[f64]| -> Vec<f64> {
        let mut y: Vec<f64> = x.iter().zip(&diag).map(|(a, b)| a * b).collect();
        for &(_, o, n, a) in &open.internal {
            y[o] -= a * x[n];
            y[n] -= a * x[o];
        }
        y
    };
    let div = divergence(geom, phi_star);
    let b: Vec<f64> = div.iter().map(|d| -d).collect();
    let nb = norm(&b);
    let mut psi = vec![0.0; nc];
    let mut iters = 0;
    if nb > 0.0 {
        let mut r = b.clone();
        let mut z: Vec<f64> = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
        let mut p = z.clone();
        let mut rz = dot(&r, &z);
        loop {
            if norm(&r) <= 1e-13 * nb {
                break;
            }
            assert!(
                iters < 50000,
                "project_flux: CG did not converge ({:.3e})",
                norm(&r) / nb
            );
            let ap = apply(&p);
            let alpha = rz / dot(&p, &ap);
            for i in 0..nc {
                psi[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }
            z = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
            let rz_new = dot(&r, &z);
            let beta = rz_new / rz;
            rz = rz_new;
            for i in 0..nc {
                p[i] = z[i] + beta * p[i];
            }
            iters += 1;
        }
    }
    let mut phi = phi_star.to_vec();
    for &(f, o, n, a) in &open.internal {
        phi[f] -= a * (psi[n] - psi[o]);
    }
    for &(f, o, a) in &open.outlet {
        phi[f] += a * psi[o];
    }
    (phi, psi, iters)
}

/// The projected start of D1 and what was measured on the way.
struct Start {
    u: Vec<Vec3>,
    phi: Vec<f64>,
    solid: Vec<bool>,
    open: OpenFaces,
    /// `sum abs(phi*)` over the fluid-solid faces before they are closed (the flow into the body of the old start).
    into_solid: f64,
    iters: usize,
}

/// D1 1 to 4 for the solver `s` of set-up R (its start velocity, boundary conditions and forcing).
fn projected_start(geom: &FvGeometry, s: &PimpleSolver) -> Start {
    let solid = solid_cells(s.immersed.as_ref().expect("projected_start: no forcing"));
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut field = s.u.clone();
    for (c, v) in field.values.iter_mut().enumerate() {
        if solid[c] {
            *v = [0.0; 3];
        }
    }
    let bu = field.boundary_values(geom);
    let mut phi_star = face_flux(geom, &field.values, &bu);
    let mut into_solid = 0.0;
    for f in 0..geom.n_internal {
        let (so, sn) = (solid[own[f]], solid[nei[f]]);
        if so != sn {
            into_solid += phi_star[f].abs();
        }
        if so || sn {
            phi_star[f] = 0.0;
        }
    }
    let open = open_faces(geom, &solid);
    let (phi, psi, iters) = project_flux(geom, &open, &phi_star);
    // D1 4: the Gauss gradient of psi.
    let sf = mesh.face_area_vectors();
    let vol = mesh.cell_volumes();
    let mut g = vec![[0.0; 3]; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let o = own[f];
        if f < geom.n_internal {
            let n = nei[f];
            let (psi_o, psi_n) = match (solid[o], solid[n]) {
                (false, false) => {
                    let w = geom.weights[f];
                    let v = w * psi[o] + (1.0 - w) * psi[n];
                    (v, v)
                }
                _ => (psi[o], psi[n]),
            };
            for k in 0..3 {
                g[o][k] += psi_o * sf[f][k];
                g[n][k] -= psi_n * sf[f][k];
            }
        } else {
            let pf = if mesh.patch("xmax").unwrap().faces().contains(&f) {
                0.0
            } else {
                psi[o]
            };
            for k in 0..3 {
                g[o][k] += pf * sf[f][k];
            }
        }
    }
    let u: Vec<Vec3> = (0..geom.n_cells())
        .map(|c| {
            if solid[c] {
                [0.0; 3]
            } else {
                let x = field.values[c];
                [x[0] - g[c][0] / vol[c], x[1] - g[c][1] / vol[c], x[2]]
            }
        })
        .collect();
    Start {
        u,
        phi,
        solid,
        open,
        into_solid,
        iters,
    }
}

/// S2: `u` zero in every solid cell and `phi` zero on every face with a solid cell on either side (exactly).
fn solid_is_closed(geom: &FvGeometry, st: &Start) -> bool {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let cells = (0..geom.n_cells()).all(|c| !st.solid[c] || st.u[c] == [0.0; 3]);
    let faces = (0..geom.n_internal).all(|f| !(st.solid[own[f]] || st.solid[nei[f]]) || st.phi[f] == 0.0);
    cells && faces
}

// ---------------------------------------------------------------------------------------------------------------------------
// P1, P2, E1
// ---------------------------------------------------------------------------------------------------------------------------

fn bits_u(u: &[Vec3]) -> Vec<u64> {
    u.iter().flat_map(|v| v.iter().map(|x| x.to_bits())).collect()
}

fn bits(x: &[f64]) -> Vec<u64> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// The library's start as a `Start` of the reference (for `solid_is_closed`).
fn as_start(geom: &FvGeometry, ib: &ImmersedForcing, u: &[Vec3], phi: &[f64], into_solid: f64, iters: usize) -> Start {
    let solid = solid_cells(ib);
    Start {
        u: u.to_vec(),
        phi: phi.to_vec(),
        open: open_faces(geom, &solid),
        solid,
        into_solid,
        iters,
    }
}

/// ADR 0425 P1: `project_start` against the F-501 reference on M1 and M2 with B, to the bit; S1, S2, the report.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p1_library_against_the_f501_reference() {
    for level in [1, 2] {
        let (geom, s) = r_solver(level, Forcing::B, 1, true);
        let ib = s.immersed.as_ref().unwrap();
        let lib = project_start(&geom, &s.u, &s.p, ib).unwrap();
        let reference = projected_start(&geom, &s);
        println!(
            "P1 M{level}: {:?}; reference CG iterations {}, into the solid {:.6e}, open faces {} + {}",
            lib.report,
            reference.iters,
            reference.into_solid,
            reference.open.internal.len(),
            reference.open.outlet.len()
        );
        assert!(
            bits_u(&lib.u) == bits_u(&reference.u),
            "P1 M{level}: u differs from the reference"
        );
        assert!(
            bits(&lib.phi) == bits(&reference.phi),
            "P1 M{level}: phi differs from the reference"
        );
        let ratio = divergence_ratio(&geom, &lib.phi);
        assert!(
            lib.report.divergence_ratio <= 1e-10 && ratio <= 1e-10,
            "P1 (a) M{level}: {:.3e}, {ratio:.3e}",
            lib.report.divergence_ratio
        );
        let st = as_start(
            &geom,
            ib,
            &lib.u,
            &lib.phi,
            lib.report.into_solid,
            lib.report.cg_iterations,
        );
        assert!(solid_is_closed(&geom, &st), "P1 (b) M{level}: flow in the solid");
        assert!(lib.report.into_solid > 0.0, "P1 (c) M{level}: no flow into the solid");
        let n_solid = ib.solid_mask().iter().filter(|&&b| b).count();
        assert_eq!(lib.report.solid_cells, n_solid, "P1 (c) M{level}");
        println!(
            "P1 M{level}: into the solid {:.6e} (reference {:.6e})",
            st.into_solid, reference.into_solid
        );
    }
}

/// ADR 0425 P2: on M2 with B, `project_immersed_start` puts the solver at the projected start with its face flux.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p2_the_start_passed_to_the_solver() {
    let (geom, mut s) = r_solver(2, Forcing::B, 1, true);
    let expect = project_start(&geom, &s.u, &s.p, s.immersed.as_ref().unwrap()).unwrap();
    let (p0, t0) = (s.p.values.clone(), s.time);
    assert!(!s.start_projected(), "P2: projected before the call");
    let rep = s.project_immersed_start().unwrap();
    assert_eq!(rep, expect.report, "P2: report");
    assert!(bits_u(&s.u.values) == bits_u(&expect.u), "P2: u");
    assert!(bits(&s.phi) == bits(&expect.phi), "P2: phi is not the projected flux");
    assert!(
        bits(&s.p.values) == bits(&p0) && s.time.to_bits() == t0.to_bits(),
        "P2: p or time changed"
    );
    assert!(s.start_projected(), "P2: flag");
    let ratio = divergence_ratio(&geom, &s.phi);
    println!("P2: divergence ratio of the solver's flux {ratio:.3e}");
    assert!(ratio <= 1e-10, "P2 (d): {ratio:.3e}");
}

/// ADR 0425 E1: the errors of the projection.
#[test]
fn e1_errors() {
    // (a) without a forcing
    let (_, mut s) = r_solver(1, Forcing::B, 2, true);
    s.immersed = None;
    let e = s.project_immersed_start();
    println!("E1 (a): {e:?}");
    assert!(e.is_err(), "E1 (a)");
    // (b) after a step
    let (_, mut s) = r_solver(1, Forcing::B, 2, true);
    s.step(1e-3);
    let e = s.project_immersed_start();
    println!("E1 (b): {e:?}");
    assert!(e.is_err() && !s.start_projected(), "E1 (b)");
    // (c) no boundary face of fixed pressure
    let (geom, mut s) = r_solver(1, Forcing::B, 2, true);
    s.p.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let e = s.project_immersed_start();
    println!("E1 (c): {e:?}");
    assert!(e.is_err() && !s.start_projected(), "E1 (c)");
    // (d) a forcing built for another mesh
    let (geom1, s1) = r_solver(1, Forcing::B, 2, true);
    let (geom2, _) = r_solver(2, Forcing::B, 2, true);
    let ib2 = forcing(&geom2, &[dfg_body()], Forcing::B, true);
    let e = project_start(&geom1, &s1.u, &s1.p, &ib2);
    println!("E1 (d): {:?}", e.as_ref().err());
    assert!(e.is_err(), "E1 (d)");
}

// ---------------------------------------------------------------------------------------------------------------------------
// RF1 to RF4: the refusal of ADR 0374 kept, lifted, cleared; the trial
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0425 RF1 to RF4 on M2 with B and one corrector, the check on.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn rf_refusal_kept_lifted_and_cleared() {
    let dt = 1e-3;
    // RF1: the default still refuses
    let (geom, mut s) = r_solver(2, Forcing::B, 1, true);
    s.step(dt);
    let d = s.diverged.clone().expect("RF1: not refused without the projection");
    let msg = d.message(Some(geom.mesh.cell_centres()));
    println!("RF1: {msg}");
    assert_eq!(d.step, 1, "RF1");
    assert_eq!(d.stage, "pressure correctors", "RF1");
    assert_eq!(d.quantity, "immersed flux corrector", "RF1");
    assert!(msg.contains("ADR 0374") && msg.contains("n_correctors"), "RF1: {msg}");
    // RF2: lifted for a projected start
    let (_, mut s) = r_solver(2, Forcing::B, 1, true);
    s.project_immersed_start().unwrap();
    for k in 1..=3 {
        s.step(dt);
        assert!(s.diverged.is_none(), "RF2: stopped at step {k}: {:?}", s.diverged);
    }
    println!("RF2: three steps, max |u| {:.5}", max_abs_u(&s));
    // RF3: reinitialise clears the lift
    let (_, mut s) = r_solver(2, Forcing::B, 1, true);
    s.project_immersed_start().unwrap();
    let (u, p, phi) = (s.u.values.clone(), s.p.values.clone(), s.phi.clone());
    s.reinitialise(u, p, Some(phi), 0.0).unwrap();
    assert!(!s.start_projected(), "RF3: flag kept");
    s.step(dt);
    let d = s.diverged.clone().expect("RF3: not refused after reinitialise");
    assert!(d.step == 1 && d.quantity == "immersed flux corrector", "RF3: {d:?}");
    // RF4: the trial steps of the pre-run estimate
    let (_, s) = r_solver(2, Forcing::B, 1, true);
    let unprojected = s.lag_preflight(dt, 5);
    let (_, mut s) = r_solver(2, Forcing::B, 1, true);
    s.project_immersed_start().unwrap();
    let projected = s.lag_preflight(dt, 5);
    println!("RF4: unprojected {unprojected:?}, projected {projected:?}");
    assert!(unprojected.is_none(), "RF4: the trial of the default is not refused");
    assert!(projected.is_some(), "RF4: the trial of the projected start is refused");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Heavy records H1, L2
// ---------------------------------------------------------------------------------------------------------------------------

/// The outcome of a run from the library's projected start.
struct Run {
    /// Step, quantity and value of a stop.
    stop: Option<(usize, &'static str, f64)>,
    /// `max abs(u)` at the last step taken.
    max_u: f64,
    /// S1 of the start.
    ratio: f64,
    /// S2 of the start.
    closed: bool,
}

/// 200 steps on M2 from the library's projected start with one corrector, `dt = 1e-3`, the check on.
fn run_projected(label: &str, f: Forcing) -> Run {
    let dt = 1e-3;
    let (geom, mut s) = r_solver(2, f, 1, true);
    let rep = s.project_immersed_start().unwrap();
    let st = as_start(
        &geom,
        s.immersed.as_ref().unwrap(),
        &s.u.values,
        &s.phi,
        rep.into_solid,
        rep.cg_iterations,
    );
    let closed = solid_is_closed(&geom, &st);
    println!("{label}: {rep:?}, S2 {closed}");
    let mut stop = None;
    for n in 1..=200 {
        s.step(dt);
        if let Some(dv) = &s.diverged {
            println!("{label}: stop {}", dv.message(Some(geom.mesh.cell_centres())));
            stop = Some((n, dv.quantity, dv.value));
            break;
        }
        if n % 20 == 0 {
            println!("{label}: step {n} max |u| {:.9}", max_abs_u(&s));
        }
    }
    Run {
        stop,
        max_u: max_abs_u(&s),
        ratio: rep.divergence_ratio,
        closed,
    }
}

/// ADR 0425 H1: B with one corrector at dt 1e-3 on M2 from the projected start, check on: 200 steps, max |u| of ADR 0421.
#[test]
#[ignore = "heavy record of ADR 0425 (H1): run once in release"]
fn record_h1_bounded_one_corrector_from_the_projected_start() {
    let t0 = Instant::now();
    let r = run_projected("H1", Forcing::B);
    println!(
        "H1: stop {:?}, max |u| at the end {:.9} (ADR 0421: 2.20802), |diff| {:.3e}, S1 {:.3e}, S2 {}, wall {:.1} s",
        r.stop,
        r.max_u,
        (r.max_u - 2.20802).abs(),
        r.ratio,
        r.closed,
        t0.elapsed().as_secs_f64()
    );
    assert!(r.stop.is_none(), "H1: stopped {:?}", r.stop);
    assert!((r.max_u - 2.20802).abs() <= 2e-5, "H1: max |u| {:.9}", r.max_u);
    assert!(r.ratio <= 1e-10 && r.closed, "H1: S1 {:.3e}, S2 {}", r.ratio, r.closed);
}

/// ADR 0425 L2: Sb (weights up to 2) with one corrector at dt 1e-3 on M2 from the projected start, check on: 200 steps.
#[test]
#[ignore = "heavy record of ADR 0425 (L2): run once in release"]
fn record_l2_unbounded_weights_two_one_corrector_from_the_projected_start() {
    let t0 = Instant::now();
    let r = run_projected("L2", Forcing::Sb);
    println!(
        "L2: stop {:?}, max |u| at the end {:.9}, S1 {:.3e}, S2 {}, wall {:.1} s",
        r.stop,
        r.max_u,
        r.ratio,
        r.closed,
        t0.elapsed().as_secs_f64()
    );
    assert!(r.stop.is_none(), "L2: stopped {:?}", r.stop);
}
