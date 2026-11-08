//! F-301 (ADR 0223) N2: fingerprints of default-path runs, written with the API of main 46a5f029 only. They were printed on that
//! library before any library change of F-301 and again on the final tree; ADR 0223 registers that the printed lines are equal.
//! Each test prints `N2 <id> <hash>` (FNV-1a over the bits of the state and of the records named in ADR 0223 N2) and asserts only
//! that its run completed as the base library did. The falling-block mesh and its constants are copied from
//! `piso_thin_gap_f299.rs` (ADR 0214, itself a copy of ADR 0211's set-up).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{CorrectorControl, LagAction, LagCheck, PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, cavity_2d};
use rcfd_mesh::motion::patch_points;
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::f64::consts::PI;
use std::sync::Arc;

fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn state_words(flow: &PimpleSolver) -> Vec<u64> {
    flow.u
        .values
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(flow.p.values.iter().map(|x| x.to_bits()))
        .chain(flow.phi.iter().map(|x| x.to_bits()))
        .collect()
}

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

// ---- the FB set-up (copied from piso_thin_gap_f299.rs, unchanged) ------------------------------------------------------

const H: f64 = 0.01;
const WB: f64 = 0.04;
const HB: f64 = 5.0;
const YB: f64 = 0.1;
const DZ: f64 = 0.01;
const RHO: f64 = 1.0;
const NU: f64 = 0.01;
const MASS: f64 = 0.03;
const GRAV: f64 = 27.0;
const N_GAP: usize = 16;

fn width() -> f64 {
    WB + 2.0 * H
}

/// `(tau, U_t, V_b)` of ADR 0211 M1 to M3.
fn fb_forms() -> (f64, f64, f64) {
    let mu = RHO * NU;
    let b = (WB + H) / H;
    let c = 2.0 * mu * HB * DZ * (3.0 * b * b + 1.0) / H;
    let m_a = 2.0 * RHO * HB * H * DZ * (0.3 * b * b - 0.5 * b + 1.0 / 3.0);
    let v_b = WB * HB * DZ;
    ((MASS + m_a) / c, (MASS - RHO * v_b) * GRAV / c, v_b)
}

fn ramp(first: f64, growth: f64, cap: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut s = first;
    while s < cap {
        out.push(s);
        s *= growth;
    }
    out
}

fn one_sided(len: f64, first: f64, growth: f64, cap: f64) -> Vec<f64> {
    let mut sizes = ramp(first, growth, cap);
    let rest = len - sizes.iter().sum::<f64>();
    assert!(rest > 0.0, "ramp longer than the segment");
    let n = (rest / cap).ceil() as usize;
    let len = sizes.len();
    sizes.resize(len + n, rest / n as f64);
    sizes
}

fn two_sided(len: f64, first: f64, growth: f64, cap: f64) -> Vec<f64> {
    let r = ramp(first, growth, cap);
    let mid = len - 2.0 * r.iter().sum::<f64>();
    assert!(mid > 0.0, "ramps longer than the segment");
    let n = (mid / cap).ceil() as usize;
    let mut sizes = r.clone();
    let len = sizes.len();
    sizes.resize(len + n, mid / n as f64);
    sizes.extend(r.iter().rev());
    sizes
}

fn push_segment(coords: &mut Vec<f64>, sizes: &[f64], end: f64) {
    for (k, s) in sizes.iter().enumerate() {
        let next = if k + 1 == sizes.len() {
            end
        } else {
            coords.last().unwrap() + s
        };
        coords.push(next);
    }
}

fn falling_block_mesh() -> Mesh {
    let w = width();
    let mut xs = vec![0.0];
    push_segment(&mut xs, &[H / N_GAP as f64; N_GAP], H);
    let span_x = two_sided(WB, H / 16.0, 1.2, H / 4.0);
    push_segment(&mut xs, &span_x, H + WB);
    push_segment(&mut xs, &[H / N_GAP as f64; N_GAP], w);
    let mut ys = vec![0.0];
    let mut below = one_sided(YB, H / 16.0, 1.2, H / 2.0);
    below.reverse();
    push_segment(&mut ys, &below, YB);
    let span_y = two_sided(HB, H / 16.0, 1.15, 2.0 * H);
    push_segment(&mut ys, &span_y, YB + HB);
    let above = one_sided(YB, H / 16.0, 1.2, H / 2.0);
    push_segment(&mut ys, &above, YB + HB + YB);
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let (i0, i1) = (N_GAP, N_GAP + span_x.len());
    let (j0, j1) = (below.len(), below.len() + span_y.len());
    let solid = |i: usize, j: usize| i >= i0 && i < i1 && j >= j0 && j < j1;
    let gid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut map = vec![usize::MAX; (nx + 1) * (ny + 1) * 2];
    let mut points: Vec<Vec3> = Vec::new();
    let names = ["block", "left", "right", "bottom", "top", "frontAndBack"];
    let mut specs: Vec<BoundarySpec> = names
        .iter()
        .map(|n| BoundarySpec {
            name: (*n).to_string(),
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
            if solid(i, j) {
                continue;
            }
            let mut p = [0usize; 8];
            for (c, pc) in p.iter_mut().enumerate() {
                let (a, b, k) = (i + (c & 1), j + ((c >> 1) & 1), c >> 2);
                let g = gid(a, b, k);
                if map[g] == usize::MAX {
                    map[g] = points.len();
                    points.push([xs[a], ys[b], if k == 0 { 0.0 } else { DZ }]);
                }
                *pc = map[g];
            }
            let faces = vec![
                vec![p[0], p[2], p[6], p[4]],
                vec![p[1], p[3], p[7], p[5]],
                vec![p[0], p[1], p[5], p[4]],
                vec![p[2], p[3], p[7], p[6]],
                vec![p[0], p[1], p[3], p[2]],
                vec![p[4], p[5], p[7], p[6]],
            ];
            let patch: [Option<&str>; 6] = [
                if i == 0 {
                    Some("left")
                } else if solid(i - 1, j) {
                    Some("block")
                } else {
                    None
                },
                if i + 1 == nx {
                    Some("right")
                } else if solid(i + 1, j) {
                    Some("block")
                } else {
                    None
                },
                if j == 0 {
                    Some("bottom")
                } else if solid(i, j - 1) {
                    Some("block")
                } else {
                    None
                },
                if j + 1 == ny {
                    Some("top")
                } else if solid(i, j + 1) {
                    Some("block")
                } else {
                    None
                },
                Some("frontAndBack"),
                Some("frontAndBack"),
            ];
            for (d, f) in faces.iter().enumerate() {
                if let Some(name) = patch[d] {
                    specs.iter_mut().find(|s| s.name == name).unwrap().faces.push(f.clone());
                }
            }
            cells.push(faces);
        }
    }
    Mesh::from_cell_faces(points, &cells, &specs).unwrap()
}

fn fb_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(20000);
    s.amg_pressure = true;
    s
}

fn walls(mesh: &Mesh) -> Vec<String> {
    mesh.patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty)
        .map(|p| p.name.clone())
        .collect()
}

fn fb_solver(geom: &Arc<FvGeometry>, settings: PimpleSettings) -> PimpleSolver {
    let mut u = VectorField::uniform("U", geom, vec3::ZERO);
    for name in walls(&geom.mesh) {
        u.set_bc(geom, &name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", geom, 0.0);
    PimpleSolver::new(geom.clone(), u, p, settings)
}

/// N2 (a): FX (the fixed-mesh reproduction of ADR 0214) with the default at `tau/40`, 20 steps.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n2a_fixed_mesh_falling_block_default() {
    let mesh = falling_block_mesh();
    assert_eq!(mesh.n_cells(), 12_320, "copy of the FB mesh");
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (tau, u_t, _) = fb_forms();
    let dt = tau / 40.0;
    let mut flow = fb_solver(&geom, fb_settings());
    for n in 1..=20 {
        let speed = u_t * (1.0 - (-(n as f64) * dt / tau).exp());
        let g = flow.geom.clone();
        flow.u
            .set_bc(&g, "block", Box::new(FixedValue::vector([0.0, -speed, 0.0])))
            .unwrap();
        flow.step(dt);
        assert!(flow.diverged.is_none(), "N2 (a) stopped at step {n}");
    }
    println!("N2 a {:016x}", fnv(state_words(&flow)));
}

/// N2 (b): the FB coupled run of ADR 0211 with the default at `tau/10`, 5 steps, with the pressure-lag check (`Warn`, step 1) and
/// the adaptive count set: on a moving mesh neither acts (the state, the number of estimates and of corrector-record steps).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n2b_coupled_falling_block_default_with_the_check_and_the_adaptive_count() {
    let mesh0 = falling_block_mesh();
    let g0 = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|pt| match pt.name.as_str() {
            "block" => PatchClass::Prescribed,
            "bottom" | "top" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let block_points = patch_points(&mesh0, "block").unwrap();
    let mut presc = vec![vec3::ZERO; mesh0.n_points()];
    for &q in &block_points {
        presc[q] = [0.0, 1.0, 0.0];
    }
    let d_unit = smooth_displacement(&g0, &classes, &presc, SmoothingMethod::Laplacian, &tight(5000))
        .unwrap()
        .displacement;
    let (tau, _, v_b) = fb_forms();
    let dt = tau / 10.0;
    let mut s = fb_settings();
    s.pressure_lag_check = Some(LagCheck {
        every: 0,
        start: 1,
        iterations: 20,
        action: LagAction::Warn,
    });
    s.corrector_control = Some(CorrectorControl::registered());
    let mut flow = fb_solver(&g0, s);
    let mut spec = BodySpec::new(MASS, [1.0, 1.0, 1.0]);
    spec.translation_locked = [true, false, true];
    spec.gravity = [0.0, -GRAV, 0.0];
    let mut body = Body::new(spec, BodyState::at([0.5 * width(), YB + 0.5 * HB, 0.5 * DZ])).unwrap();
    let buoyancy = RHO * v_b * GRAV;
    body.set_hook(Box::new(move |_s, _t| ([0.0, buoyancy, 0.0], vec3::ZERO)));
    let y0 = body.state.x[1];
    let patches = vec!["block".to_string()];
    for n in 1..=5 {
        let (f, m) = patch_force_moment(&flow.geom, &flow.u, &flow.p, RHO, NU, &patches, body.state.x).unwrap();
        body.advance(dt, Integrator::Rk4, (f, m)).unwrap();
        let disp = body.state.x[1] - y0;
        let pts: Vec<Vec3> = mesh0
            .points()
            .iter()
            .zip(&d_unit)
            .map(|(p, d)| vec3::add(*p, vec3::scale(*d, disp)))
            .collect();
        let st = move_mesh(&flow.geom, pts, dt).unwrap();
        let old = flow.geom.mesh.clone();
        let w = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt).unwrap();
        flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, w).unwrap();
        assert!(flow.diverged.is_none(), "N2 (b) stopped at step {n}");
    }
    let mut words = state_words(&flow);
    words.push(flow.lag_estimates.len() as u64);
    words.push(flow.correctors.steps);
    words.push(flow.correctors.estimates.len() as u64);
    println!(
        "N2 b {:016x} (estimates {}, corrector-record steps {})",
        fnv(words),
        flow.lag_estimates.len(),
        flow.correctors.steps
    );
}

/// N2 (c): Taylor-Green PISO BDF2, N = 16, dt 0.05, 20 steps.
#[test]
fn n2c_taylor_green_piso_bdf2() {
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(16, 16, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut st = PimpleSettings::piso(0.1);
    st.convection = ConvScheme::Central;
    st.time_scheme = TimeScheme::Bdf2;
    let mut flow = PimpleSolver::new(geom, u, p, st);
    for _ in 0..20 {
        flow.step(0.05);
        assert!(flow.diverged.is_none());
    }
    println!("N2 c {:016x}", fnv(state_words(&flow)));
}

/// N2 (d): ADR 0073's B2 periodic Stokes box, PIMPLE-2, D = 4, Euler, 8 steps, the check every 6 steps (the state and each
/// estimate).
#[test]
fn n2d_periodic_stokes_box_with_the_check() {
    let n = 32;
    let h = 1.0 / n as f64;
    let dt = 0.01;
    let nu = 4.0 / (4.0 / (h * h) * dt);
    let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::from_fn("p", &geom, |c| 1e-6 * (2.0 * PI * c[0]).cos() * (2.0 * PI * c[1]).cos());
    let mut st = PimpleSettings::pimple(nu, 2);
    st.convection = ConvScheme::LinearUpwind;
    st.momentum_solver = SolveControl {
        tol_abs: 1e-30,
        tol_rel: 1e-10,
        max_iter: 5000,
    };
    st.pressure_solver = st.momentum_solver;
    st.pressure_lag_check = Some(LagCheck::new(6, LagAction::Warn));
    let mut flow = PimpleSolver::new(geom, u, p, st);
    for _ in 0..8 {
        flow.step(dt);
        assert!(flow.diverged.is_none());
    }
    let mut words = state_words(&flow);
    for w in &flow.lag_estimates {
        words.extend([w.step as u64, w.rho.to_bits(), w.cell as u64]);
    }
    println!("N2 d {:016x} (estimates {})", fnv(words), flow.lag_estimates.len());
}

/// N2 (e): the uniform cavity, Re 100, PIMPLE-2, Euler, Co_t 4, with the check in `Raise`: the stop (step, quantity, value bits,
/// cell) and the state; not the message text.
#[test]
fn n2e_cavity_raise_stop() {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(32, 1.0).unwrap())).unwrap());
    let h_min = 2.0 * geom.dn.iter().copied().fold(f64::INFINITY, f64::min);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::pimple(0.01, 2);
    st.convection = ConvScheme::LinearUpwind;
    st.pressure_lag_check = Some(LagCheck::new(20, LagAction::Raise));
    let mut flow = PimpleSolver::new(geom, u, p, st);
    flow.step(4.0 * h_min);
    let d = flow.diverged.clone().expect("N2 (e): the check did not stop the run");
    let mut words = state_words(&flow);
    words.extend([
        d.step as u64,
        d.quantity.len() as u64,
        d.value.to_bits(),
        d.cell.unwrap_or(usize::MAX) as u64,
    ]);
    println!("N2 e {:016x} (stop at step {}, {} {:.6})", fnv(words), d.step, d.quantity, d.value);
}

/// N2 (f): a channel periodic in x driven by a body force, PISO, 5 steps.
#[test]
fn n2f_periodic_box_with_a_body_force() {
    let mesh = box_2d(8, 6, 1.0, 0.5, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    for name in ["ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom.clone(), u, p, PimpleSettings::piso(0.01));
    flow.body_force = Some(BodyForce::uniform(&geom, [0.5, -1.0, 0.0]));
    for _ in 0..5 {
        flow.step(0.01);
        assert!(flow.diverged.is_none());
    }
    println!("N2 f {:016x}", fnv(state_words(&flow)));
}
