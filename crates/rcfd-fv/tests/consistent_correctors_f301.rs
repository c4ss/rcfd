//! F-301 (ADR 0223): the consistent PISO correctors of ADR 0214, continued.
//!
//! MA: the ADR 0073 pressure-lag estimate on a moving-mesh step (`PimpleSettings::moving_mesh_lag`, E0), the adaptive count on it
//! and the pre-run estimate of a moving step (`PimpleSolver::lag_preflight_moving`, E1), on the falling block of ADR 0211.
//! S, R2: the consistent correctors on periodic faces (`consistent_periodic`, E2) and with body forces (`consistent_body_force`,
//! E3), each with its steady-state identity check, and the refusals that stay. MSG1: the corrected Raise message (E4). N1.
//!
//! The falling-block mesh, its constants, the twin-run power iteration and the coupled loop are copied from
//! `piso_thin_gap_f299.rs` (ADR 0214, itself ADR 0211's set-up); the copy is asserted to give ADR 0211's cell counts. The limits were
//! registered in ADR 0223 before this file existed.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, MeshStep};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{
    CorrectorControl, LagAction, LagCheck, OuterHook, OuterUpdate, PimpleSettings, PimpleSolver, PimpleState, StepInfo,
};
use rcfd_fv::pressure::pressure_bc;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, Grading};
use rcfd_mesh::motion::patch_points;
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::f64::consts::PI;
use std::sync::Arc;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

fn ubits(v: &[Vec3]) -> Vec<u64> {
    v.iter().flat_map(|x| x.iter().map(|y| y.to_bits())).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_du(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .fold(0.0f64, |m, (x, y)| m.max(vec3::norm(vec3::sub(*x, *y))))
}

fn max_dp(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()))
}

fn range(p: &[f64]) -> f64 {
    let hi = p.iter().fold(f64::NEG_INFINITY, |m, x| m.max(*x));
    let lo = p.iter().fold(f64::INFINITY, |m, x| m.min(*x));
    hi - lo
}

// =================================================================================================================
// The FB set-up of ADR 0211 (copied from piso_thin_gap_f299.rs, unchanged)
// =================================================================================================================

const H: f64 = 0.01; // gap width
const WB: f64 = 0.04; // block width
const HB: f64 = 5.0; // block height
const YB: f64 = 0.1; // fluid below the block (and above it)
const DZ: f64 = 0.01; // depth (one cell, empty front and back)
const RHO: f64 = 1.0;
const NU: f64 = 0.01;
const MASS: f64 = 0.03;
const GRAV: f64 = 27.0;
const N_GAP: usize = 16;

fn width() -> f64 {
    WB + 2.0 * H
}

/// Closed forms of ADR 0211 M1 to M3 for the numbers of FB.
#[derive(Debug, Clone, Copy)]
struct FbForms {
    v_b: f64,
    tau: f64,
    u_t: f64,
}

fn fb_forms() -> FbForms {
    let mu = RHO * NU;
    let b = (WB + H) / H;
    let c = 2.0 * mu * HB * DZ * (3.0 * b * b + 1.0) / H;
    let m_a = 2.0 * RHO * HB * H * DZ * (0.3 * b * b - 0.5 * b + 1.0 / 3.0);
    let v_b = WB * HB * DZ;
    FbForms {
        v_b,
        tau: (MASS + m_a) / c,
        u_t: (MASS - RHO * v_b) * GRAV / c,
    }
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

/// The mesh of FB; returns the mesh and `(columns, rows, cells across the block, rows along the block)`.
fn falling_block_mesh() -> (Mesh, [usize; 4]) {
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
                vec![p[0], p[2], p[6], p[4]], // x-
                vec![p[1], p[3], p[7], p[5]], // x+
                vec![p[0], p[1], p[5], p[4]], // y-
                vec![p[2], p[3], p[7], p[6]], // y+
                vec![p[0], p[1], p[3], p[2]], // z-
                vec![p[4], p[5], p[7], p[6]], // z+
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
    let mesh = Mesh::from_cell_faces(points, &cells, &specs).unwrap();
    (mesh, [nx, ny, span_x.len(), span_y.len()])
}

fn fb_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(20000);
    s.amg_pressure = true;
    s
}

/// The FB mesh with its smoothing data: the unit displacement field of the block (solved once, ADR 0211 FB).
struct Fb {
    mesh0: Mesh,
    g0: Arc<FvGeometry>,
    d_unit: Vec<Vec3>,
    forms: FbForms,
}

impl Fb {
    fn new() -> Self {
        let (mesh0, counts) = falling_block_mesh();
        assert_eq!(mesh0.n_cells(), 12_320, "copy of the FB mesh: cell count of ADR 0211");
        assert_eq!(counts, [56, 343, 24, 287], "copy of the FB mesh: grid of ADR 0211");
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
        Fb {
            mesh0,
            g0,
            d_unit,
            forms: fb_forms(),
        }
    }

    /// Points of the mesh with the block displaced by `s` along y.
    fn points(&self, s: f64) -> Vec<Vec3> {
        self.mesh0
            .points()
            .iter()
            .zip(&self.d_unit)
            .map(|(p, d)| vec3::add(*p, vec3::scale(*d, s)))
            .collect()
    }

    fn walls(&self) -> Vec<String> {
        self.mesh0
            .patches()
            .iter()
            .filter(|p| p.kind != PatchKind::Empty)
            .map(|p| p.name.clone())
            .collect()
    }

    /// The fluid at rest on the unmoved mesh, every wall a fixed velocity zero.
    fn solver(&self, settings: PimpleSettings) -> PimpleSolver {
        let geom = self.g0.clone();
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in self.walls() {
            u.set_bc(&geom, &name, Box::new(FixedValue::vector(vec3::ZERO)))
                .unwrap();
        }
        let p = ScalarField::uniform("p", &geom, 0.0);
        PimpleSolver::new(geom, u, p, settings)
    }

    /// `U(t) = U_t (1 - e^{-t/tau})` (ADR 0211 M3).
    fn speed(&self, t: f64) -> f64 {
        self.forms.u_t * (1.0 - (-t / self.forms.tau).exp())
    }

    /// FX (ADR 0214): the block's fixed velocity of the step that ends at `t`, on the unmoved mesh.
    fn set_block(&self, flow: &mut PimpleSolver, t: f64) {
        let g = flow.geom.clone();
        flow.u
            .set_bc(&g, "block", Box::new(FixedValue::vector([0.0, -self.speed(t), 0.0])))
            .unwrap();
    }
}

/// The FB coupled loop of ADR 0211: the flow, the body and the block's start height.
struct Coupled {
    flow: PimpleSolver,
    body: Body,
    y0: f64,
    patches: Vec<String>,
}

fn coupled_start(fb: &Fb, settings: PimpleSettings) -> Coupled {
    let flow = fb.solver(settings);
    let mut spec = BodySpec::new(MASS, [1.0, 1.0, 1.0]);
    spec.translation_locked = [true, false, true];
    spec.gravity = [0.0, -GRAV, 0.0];
    let mut body = Body::new(spec, BodyState::at([0.5 * width(), YB + 0.5 * HB, 0.5 * DZ])).unwrap();
    let buoyancy = RHO * fb.forms.v_b * GRAV;
    body.set_hook(Box::new(move |_s, _t| ([0.0, buoyancy, 0.0], vec3::ZERO)));
    let y0 = body.state.x[1];
    Coupled {
        flow,
        body,
        y0,
        patches: vec!["block".to_string()],
    }
}

/// The motion data of the next coupled step: the body advanced with the present force (RK4), the moved mesh and its flux, and the
/// follow conditions set on the flow's velocity (the block's wall velocity) with their wall gradients.
fn coupled_motion(fb: &Fb, c: &mut Coupled, dt: f64) -> Result<(MeshStep, Vec<f64>), String> {
    let (f, m) = patch_force_moment(&c.flow.geom, &c.flow.u, &c.flow.p, RHO, NU, &c.patches, c.body.state.x)
        .map_err(|e| e.to_string())?;
    c.body.advance(dt, Integrator::Rk4, (f, m))?;
    let s = c.body.state.x[1] - c.y0;
    let st = move_mesh(&c.flow.geom, fb.points(s), dt)?;
    let old = c.flow.geom.mesh.clone();
    let walls = set_follow_conditions(&mut c.flow.u, &st, &old, &c.patches, dt)?;
    Ok((st, walls))
}

/// One coupled step; `Err` for a failure (a mesh error, a guard trip, a non-finite velocity).
fn coupled_step(fb: &Fb, c: &mut Coupled, dt: f64) -> Result<StepInfo, String> {
    let (st, walls) = coupled_motion(fb, c, dt)?;
    let info = c.flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)?;
    if let Some(d) = &c.flow.diverged {
        return Err(d.message(None));
    }
    if !c.flow.u.values.iter().flatten().all(|x| x.is_finite()) {
        return Err("non-finite velocity".into());
    }
    Ok(info)
}

fn umax(flow: &PimpleSolver) -> f64 {
    flow.u.values.iter().fold(0.0f64, |m, v| m.max(vec3::norm(*v)))
}

fn warn_at(step: usize) -> Option<LagCheck> {
    Some(LagCheck {
        every: 0,
        start: step,
        iterations: 20,
        action: LagAction::Warn,
    })
}

fn consistent() -> PimpleSettings {
    let mut s = fb_settings();
    s.consistent_correctors = true;
    s
}

// =================================================================================================================
// MA: the estimate on a moving mesh
// =================================================================================================================

/// ADR 0223 MA1 and MA1n: FX's step 1 taken as a moving-mesh step that does not move against the same step taken by `step`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma1_ma1n_the_estimate_of_a_moving_step_that_does_not_move() {
    let fb = Fb::new();
    let dt = fb.forms.tau / 10.0;
    let mut s = fb_settings();
    s.pressure_lag_check = warn_at(1);
    s.moving_mesh_lag = true;
    let nf = fb.g0.mesh.n_faces();
    // B: the fixed-mesh step
    let mut b = fb.solver(s.clone());
    fb.set_block(&mut b, dt);
    b.step(dt);
    // A: the same step as a moving-mesh step with the same geometry, no mesh flux and no wall gradients
    let mut a = fb.solver(s.clone());
    fb.set_block(&mut a, dt);
    a.step_ale_with_walls(dt, fb.g0.clone(), vec![0.0; nf], Vec::new())
        .unwrap();
    // A': A without moving_mesh_lag
    let mut an = s.clone();
    an.moving_mesh_lag = false;
    let mut a0 = fb.solver(an);
    fb.set_block(&mut a0, dt);
    a0.step_ale_with_walls(dt, fb.g0.clone(), vec![0.0; nf], Vec::new())
        .unwrap();
    let (wa, wb) = (a.lag_estimates.first().copied(), b.lag_estimates.first().copied());
    let pmax = b.p.values.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let du = max_du(&a.u.values, &b.u.values);
    let dp = max_dp(&a.p.values, &b.p.values);
    println!(
        "MA1: moving step {wa:?}, fixed step {wb:?}; fields: max|du| {:.2e} U_t, max|dp| {:.2e} max|p|; MA1n: estimates {}, fields \
         bit-identical to the moving step's {}",
        du / fb.forms.u_t,
        dp / pmax,
        a0.lag_estimates.len(),
        ubits(&a0.u.values) == ubits(&a.u.values) && bits(&a0.p.values) == bits(&a.p.values) && bits(&a0.phi) == bits(&a.phi)
    );
    let (wa, wb) = (
        wa.expect("MA1: no estimate on the moving step"),
        wb.expect("MA1: no estimate on the fixed step"),
    );
    assert!(wa.step == 1 && wb.step == 1, "MA1 steps");
    assert!(
        (wa.rho - wb.rho).abs() <= 1e-9 * wb.rho,
        "MA1 rho {} {}",
        wa.rho,
        wb.rho
    );
    assert_eq!(wa.cell, wb.cell, "MA1 cell");
    assert!(du <= 1e-12 * fb.forms.u_t && dp <= 1e-12 * pmax, "MA1 fields {du} {dp}");
    assert!(a0.lag_estimates.is_empty(), "MA1n: an estimate without moving_mesh_lag");
    assert_eq!(ubits(&a0.u.values), ubits(&a.u.values), "MA1n velocity");
    assert_eq!(bits(&a0.p.values), bits(&a.p.values), "MA1n pressure");
    assert_eq!(bits(&a0.phi), bits(&a.phi), "MA1n flux");
}

/// Result of TW-ALE (ADR 0223).
struct TwAle {
    rho: f64,
    sign: f64,
    estimate: f64,
    cell: usize,
}

/// TW-ALE: the twin-run power iteration of ADR 0214 on the moving step 6 of the FB coupled run with `settings`, with the motion
/// data of step 6 computed once from the base state `S` after 5 steps; E-ALE on the base step.
fn twin_ale(fb: &Fb, settings: PimpleSettings) -> TwAle {
    const K: usize = 60;
    const NB: usize = 5;
    let dt = fb.forms.tau / 10.0;
    let mut c = coupled_start(fb, settings);
    for n in 1..=NB {
        let r = coupled_step(fb, &mut c, dt);
        let um = umax(&c.flow);
        println!(
            "MA2s: base step {n}: {:?}, max|u|/U_t {:.4}",
            r.as_ref().err(),
            um / fb.forms.u_t
        );
        assert!(
            r.is_ok() && um <= 10.0 * fb.forms.u_t,
            "MA2s: base step {n} not bounded"
        );
    }
    let s: PimpleState = c.flow.snapshot();
    let g_s = c.flow.geom.clone();
    let (st, walls) = coupled_motion(fb, &mut c, dt).expect("TW-ALE: motion of step 6");
    // the base step from S with E-ALE
    c.flow.settings.moving_mesh_lag = true;
    c.flow.settings.pressure_lag_check = warn_at(NB + 1);
    c.flow
        .step_ale_with_walls(dt, st.geom.clone(), st.mesh_flux.clone(), walls.clone())
        .unwrap();
    assert!(c.flow.diverged.is_none(), "TW-ALE base step");
    let w = c
        .flow
        .lag_estimates
        .iter()
        .find(|w| w.step == NB + 1)
        .copied()
        .expect("E-ALE: no estimate on the base step");
    c.flow.settings.pressure_lag_check = None;
    let (bu, bp, bphi) = (c.flow.u.values.clone(), c.flow.p.values.clone(), c.flow.phi.clone());
    let vol = g_s.mesh.cell_volumes().to_vec();
    let n = vol.len();
    let u_t = fb.forms.u_t;
    let ut2 = u_t * u_t;
    let pmax = s.p.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    assert!(pmax > 0.0, "TW-ALE base pressure is zero");
    let p2 = pmax * pmax;
    let norm = |du: &[Vec3], dp: &[f64]| -> f64 {
        (0..n)
            .map(|c| vol[c] * ((du[c][0].powi(2) + du[c][1].powi(2)) / ut2 + dp[c] * dp[c] / p2))
            .sum::<f64>()
            .sqrt()
    };
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let mut du: Vec<Vec3> = Vec::with_capacity(n);
    let mut dp: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        dp.push(rnd() * pmax);
        du.push([rnd() * u_t, rnd() * u_t, 0.0]);
    }
    let mut dphi = vec![0.0; c.flow.phi.len()];
    let d0 = norm(&du, &dp);
    du.iter_mut().for_each(|v| *v = vec3::scale(*v, 1.0 / d0));
    dp.iter_mut().for_each(|x| *x /= d0);
    let eps = 1e-7 * norm(&s.u, &s.p);
    let (mut logs, mut signs) = (Vec::new(), Vec::new());
    for _ in 0..K {
        let mut stk = s.clone();
        for c in 0..n {
            for k in 0..2 {
                stk.u[c][k] += eps * du[c][k];
                stk.hist_u[k].old[c] = stk.u[c][k];
            }
            stk.p[c] += eps * dp[c];
        }
        for (f, d) in dphi.iter().enumerate() {
            stk.phi[f] += eps * d;
        }
        stk.phi_hist.old = stk.phi.clone();
        c.flow.geom = g_s.clone();
        c.flow.restore(&stk).unwrap();
        c.flow
            .step_ale_with_walls(dt, st.geom.clone(), st.mesh_flux.clone(), walls.clone())
            .unwrap();
        assert!(c.flow.diverged.is_none(), "TW-ALE perturbed step");
        let tu: Vec<Vec3> = (0..n)
            .map(|i| vec3::scale(vec3::sub(c.flow.u.values[i], bu[i]), 1.0 / eps))
            .collect();
        let tp: Vec<f64> = (0..n).map(|i| (c.flow.p.values[i] - bp[i]) / eps).collect();
        let tphi: Vec<f64> = c.flow.phi.iter().zip(&bphi).map(|(a, b)| (a - b) / eps).collect();
        let nt = norm(&tu, &tp);
        let ip: f64 = (0..n)
            .map(|i| vol[i] * ((tu[i][0] * du[i][0] + tu[i][1] * du[i][1]) / ut2 + tp[i] * dp[i] / p2))
            .sum();
        let nd = norm(&du, &dp);
        logs.push((nt / nd).ln());
        signs.push(ip.signum());
        du = tu.iter().map(|v| vec3::scale(*v, 1.0 / nt)).collect();
        dp = tp.iter().map(|x| x / nt).collect();
        dphi = tphi.iter().map(|x| x / nt).collect();
    }
    let tail = &logs[K / 2..];
    TwAle {
        rho: (tail.iter().sum::<f64>() / tail.len() as f64).exp(),
        sign: signs[K / 2..].iter().sum::<f64>() / (K - K / 2) as f64,
        estimate: w.rho,
        cell: w.cell,
    }
}

fn report_tw(fb: &Fb, id: &str, t: &TwAle) {
    let x = fb.g0.mesh.cell_centres()[t.cell];
    println!(
        "{id}: TW-ALE |rho| {:.5} sign {:+.3}; E-ALE {:.5} at cell {} ({:.5}, {:.5}); E-ALE against |rho| {:+.4} (ADR 0214 D1 3.15959, \
         L1 3.30207, F1 tau/10 0.91984, F3 0.91697)",
        t.rho,
        t.sign,
        t.estimate,
        t.cell,
        x[0],
        x[1],
        (t.estimate - t.rho) / t.rho
    );
}

/// ADR 0223 MA2s and MA2: the estimate of the moving step 6 of the FB coupled run against the step's own twin-run radius.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma2_the_estimate_of_a_moving_step_predicts_its_growth() {
    let fb = Fb::new();
    let t = twin_ale(&fb, fb_settings());
    report_tw(&fb, "MA2", &t);
    assert!(
        (t.rho - 3.15959).abs() <= 0.10 * 3.15959 && t.sign <= -0.5,
        "MA2 |rho| {} sign {}",
        t.rho,
        t.sign
    );
    assert!(
        (t.estimate - t.rho).abs() <= 0.10 * t.rho,
        "MA2 E-ALE {} |rho| {}",
        t.estimate,
        t.rho
    );
}

/// ADR 0223 MA2c: MA2 with the consistent correctors.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma2c_the_moving_step_with_the_consistent_correctors() {
    let fb = Fb::new();
    let t = twin_ale(&fb, consistent());
    report_tw(&fb, "MA2c", &t);
    assert!(t.rho < 1.0, "MA2c |rho| {}", t.rho);
    assert!(t.estimate < 1.0, "MA2c E-ALE {}", t.estimate);
}

/// The FB coupled run of MA3: 100 steps at `tau/10`, stopped at the first failure. Returns `(failure, history of (t, U), continuity)`.
fn coupled_run(fb: &Fb, c: &mut Coupled) -> (Option<(usize, String)>, Vec<(f64, f64)>, f64, usize) {
    let dt = fb.forms.tau / 10.0;
    let (mut hist, mut cont, mut p_iters) = (Vec::new(), 0.0f64, 0usize);
    for n in 1..=100 {
        match coupled_step(fb, c, dt) {
            Ok(info) => {
                cont = cont.max(info.continuity);
                p_iters += info.p_iters;
                hist.push((n as f64 * dt, -c.body.state.v[1]));
            }
            Err(e) => return (Some((n, e)), hist, cont, p_iters),
        }
    }
    (None, hist, cont, p_iters)
}

/// ADR 0223 MA3: the adaptive count on the moving mesh of the FB coupled run.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma3_the_adaptive_count_on_a_moving_mesh() {
    let fb = Fb::new();
    let mut s = fb_settings();
    s.corrector_control = Some(CorrectorControl::registered());
    s.moving_mesh_lag = true;
    let mut c = coupled_start(&fb, s);
    let (fail, hist, cont, p_iters) = coupled_run(&fb, &mut c);
    let r = &c.flow.correctors;
    for e in &r.estimates {
        println!(
            "MA3 estimate: step {}, count {}, rho {:.4}, cap-bound {}, monitor {}, dt-triggered {}, evaluations {}",
            e.step, e.correctors, e.rho, e.cap_bound, e.triggered, e.dt_triggered, e.evaluations
        );
    }
    let a1 = hist.last().map_or(f64::NAN, |h| h.1 / fb.forms.u_t);
    for t in [1usize, 2, 5] {
        if let Some((time, uu)) = hist.get(10 * t - 1) {
            println!(
                "MA3 approach: t = {:.0} tau: U/U_t {:.5}",
                time / fb.forms.tau,
                uu / fb.forms.u_t
            );
        }
    }
    println!(
        "MA3: failure {fail:?}; steps {}; record: steps {}, mean count {:.3}, max {}, last {}, estimates {}, cap-bound {}; \
         U(10 tau)/U_t {a1:.5} (limit |. - 1| <= 0.02); continuity {cont:.2e} (limit 1e-10); pressure iterations {p_iters} (ADR 0214 \
         A1c: 2434)",
        hist.len(),
        r.steps,
        r.mean(),
        r.max,
        r.last,
        r.estimates.len(),
        r.cap_bound
    );
    let first = r.estimates.first().copied().expect("MA3: no estimate");
    assert!(
        first.step == 1 && !first.cap_bound && first.correctors >= 3 && first.rho <= 0.5,
        "MA3 (i) {first:?}"
    );
    assert!(fail.is_none(), "MA3 (ii) {fail:?}");
    assert!((a1 - 1.0).abs() <= 0.02, "MA3 (iii) A1 {a1}");
    assert!(cont <= 1e-10, "MA3 (iii) A3 {cont}");
}

/// ADR 0223 MA3n: MA3 without `moving_mesh_lag`: no estimate on the moving mesh, and the known miss of F-244.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma3n_without_the_flag_the_adaptive_count_is_not_used_on_a_moving_mesh() {
    let fb = Fb::new();
    let mut s = fb_settings();
    s.corrector_control = Some(CorrectorControl::registered());
    let mut c = coupled_start(&fb, s);
    let (fail, hist, _, _) = coupled_run(&fb, &mut c);
    let r = &c.flow.correctors;
    println!(
        "MA3n: failure {fail:?} after {} good steps; record: steps {}, estimates {}",
        hist.len(),
        r.steps,
        r.estimates.len()
    );
    assert!(r.steps == 0 && r.estimates.is_empty(), "MA3n record");
    assert!(fail.is_some(), "MA3n: the run did not fail within 100 steps");
}

/// ADR 0223 MA4: the pre-run estimate of the first moving step against the step's own estimate.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn ma4_the_pre_run_estimate_of_a_moving_step() {
    let fb = Fb::new();
    let dt = fb.forms.tau / 10.0;
    let mut c = coupled_start(&fb, fb_settings());
    let (st, walls) = coupled_motion(&fb, &mut c, dt).unwrap();
    let before = c.flow.snapshot();
    let pre = c
        .flow
        .lag_preflight_moving(dt, 20, st.geom.clone(), st.mesh_flux.clone(), walls.clone());
    let after = c.flow.snapshot();
    let unchanged = ubits(&before.u) == ubits(&after.u)
        && bits(&before.p) == bits(&after.p)
        && bits(&before.phi) == bits(&after.phi)
        && before.time.to_bits() == after.time.to_bits()
        && before.step_no == after.step_no
        && c.flow.lag_estimates.is_empty();
    c.flow.settings.moving_mesh_lag = true;
    c.flow.settings.pressure_lag_check = warn_at(1);
    c.flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
    let real = c.flow.lag_estimates.first().copied();
    println!(
        "MA4: preflight {pre:?}; the real step's estimate {real:?}; the solver unchanged by the preflight {unchanged}"
    );
    let pre = pre.expect("MA4: no preflight estimate");
    let real = real.expect("MA4: no estimate on the real step");
    assert_eq!(pre.step, 1, "MA4 step");
    assert_eq!(pre.rho.to_bits(), real.rho.to_bits(), "MA4 rho");
    assert_eq!(pre.cell, real.cell, "MA4 cell");
    assert!(unchanged, "MA4: the preflight changed the solver");
    assert!((pre.rho - 3.30207).abs() <= 0.10 * 3.30207, "MA4 rho {}", pre.rho);
}

// =================================================================================================================
// S: the steady-state identities of the extensions (Q5)
// =================================================================================================================

const U0: f64 = 0.1;
const V0: f64 = 0.02;
const LP: f64 = 0.3;

/// SPC and SPX (ADR 0223): the channel periodic in x of `periods` periods of length 0.3, `ymin` at rest, `ymax` with the velocity
/// `(U0, V0 sin(2 pi x / 0.3), 0)` per face.
fn spc_solver(periods: usize, settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d_graded(
        20 * periods,
        12,
        LP * periods as f64,
        0.1,
        [Grading::Uniform, Grading::Symmetric(3.0)],
        true,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let fc = geom.mesh.face_centres();
    let top = geom
        .mesh
        .patches()
        .iter()
        .find(|p| p.name == "ymax")
        .expect("ymax patch");
    let profile: Vec<Vec3> = top
        .faces()
        .map(|f| [U0, V0 * (2.0 * PI * fc[f][0] / LP).sin(), 0.0])
        .collect();
    u.set_bc(&geom, "ymax", Box::new(FixedValue(Profile::PerFace(profile))))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

/// ADR 0223 S0p, SPs, S1p: the consistent correctors on periodic faces keep the default's steady state of SPC.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s0p_sps_s1p_steady_state_on_periodic_faces() {
    let dt = 0.002;
    let mut flow = spc_solver(1, fb_settings());
    for _ in 0..3000 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S0p run stopped");
    }
    let s0 = flow.snapshot();
    let (u_s0, p_s0) = (s0.u.clone(), s0.p.clone());
    let rg = range(&p_s0);
    // SPs: the pressure difference across the periodic pairs
    let pbc = pressure_bc(&flow.geom, &flow.u, &flow.p);
    let own = flow.geom.mesh.owner();
    let ni = flow.geom.n_internal;
    let seam = pbc
        .coupled
        .iter()
        .enumerate()
        .filter_map(|(i, q)| q.map(|q| (p_s0[own[q]] - p_s0[own[ni + i]]).abs()))
        .fold(0.0f64, f64::max);
    let (mut du_a, mut dp_a) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S0p branch A stopped");
        du_a = du_a.max(max_du(&flow.u.values, &u_s0));
        dp_a = dp_a.max(max_dp(&flow.p.values, &p_s0));
    }
    let (u_a, p_a) = (flow.u.values.clone(), flow.p.values.clone());
    flow.restore(&s0).unwrap();
    flow.settings.consistent_correctors = true;
    flow.settings.consistent_periodic = true;
    let (mut du_b, mut dp_b) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S1p branch B stopped: {:?}", flow.diverged);
        du_b = du_b.max(max_du(&flow.u.values, &u_s0));
        dp_b = dp_b.max(max_dp(&flow.p.values, &p_s0));
    }
    println!(
        "S0p: branch A (default) max|u - u_S0| {:.2e} U0 (limit 2e-11), max|p - p_S0| {:.2e} of the range {rg:.4e}; SPs: largest \
         seam pressure difference {:.3e} of the range (limit >= 1e-2); S1p: branch B max|u - u_S0| {:.2e} U0 (limit 1e-10), \
         max|p - p_S0| {:.2e} of the range (limit 1e-10); B against A after 50 steps {:.2e} U0, {:.2e} of the range",
        du_a / U0,
        dp_a / rg,
        seam / rg,
        du_b / U0,
        dp_b / rg,
        max_du(&flow.u.values, &u_a) / U0,
        max_dp(&flow.p.values, &p_a) / rg
    );
    assert!(du_a <= 2e-11 * U0, "S0p {du_a}");
    assert!(seam >= 1e-2 * rg, "SPs {seam}");
    assert!(du_b <= 1e-10 * U0, "S1p velocity {du_b}");
    assert!(dp_b <= 1e-10 * rg, "S1p pressure {dp_b}");
}

fn mean_removed(flow: &PimpleSolver) -> Vec<f64> {
    let vol = flow.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let m = flow.p.values.iter().zip(vol).map(|(p, v)| p * v).sum::<f64>() / tv;
    flow.p.values.iter().map(|p| p - m).collect()
}

/// SPX: 20 steps of box 1 and box 2 with `settings`; the largest matched velocity difference over the steps (m/s) and the largest
/// matched pressure difference relative to box 1's pressure range.
fn spx_arm(settings: &PimpleSettings) -> (f64, f64) {
    let mut b1 = spc_solver(1, settings.clone());
    let mut b2 = spc_solver(2, settings.clone());
    let c1 = b1.geom.mesh.cell_centres().to_vec();
    let c2 = b2.geom.mesh.cell_centres().to_vec();
    let matched: Vec<usize> = c2
        .iter()
        .map(|x| {
            let xm = if x[0] >= LP { x[0] - LP } else { x[0] };
            (0..c1.len())
                .find(|&i| (c1[i][0] - xm).abs() <= 1e-12 && (c1[i][1] - x[1]).abs() <= 1e-12)
                .expect("SPX: no matching cell")
        })
        .collect();
    let (mut du, mut dp) = (0.0f64, 0.0f64);
    for _ in 0..20 {
        b1.step(0.002);
        b2.step(0.002);
        assert!(b1.diverged.is_none() && b2.diverged.is_none(), "SPX stopped");
        let (p1, p2) = (mean_removed(&b1), mean_removed(&b2));
        let rg = range(&p1);
        for (i, &j) in matched.iter().enumerate() {
            du = du.max(vec3::norm(vec3::sub(b2.u.values[i], b1.u.values[j])));
            dp = dp.max((p2[i] - p1[j]).abs() / rg);
        }
    }
    (du, dp)
}

/// ADR 0223 SPx: with the consistent correctors a seam cell behaves as the same cell of the unsplit mesh (one period against two).
#[test]
fn spx_seam_cells_behave_as_interior_cells() {
    let d = spx_arm(&fb_settings());
    let mut s = fb_settings();
    s.consistent_correctors = true;
    s.consistent_periodic = true;
    let c = spx_arm(&s);
    println!(
        "SPx: default max|du| {:.2e} U0, max|dp| {:.2e} of the range; consistent max|du| {:.2e} U0 (limit max(1e-10, 10 x the \
         default's)), max|dp| {:.2e} (limit max(1e-10, 10 x the default's))",
        d.0 / U0,
        d.1,
        c.0 / U0,
        c.1
    );
    assert!(c.0 <= (1e-10 * U0).max(10.0 * d.0), "SPx velocity {}", c.0);
    assert!(c.1 <= 1e-10f64.max(10.0 * d.1), "SPx pressure {}", c.1);
}

const A_SB: f64 = 0.1;

fn sb_accel(x: Vec3) -> Vec3 {
    [A_SB * (PI * x[1] / 0.1).sin(), -A_SB, 0.0]
}

/// The body force of SBC: cell values at the centres, `f(x_f) . S_f` on every non-empty face.
fn sbc_force(geom: &FvGeometry) -> BodyForce {
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    BodyForce {
        cell: cc.iter().map(|&c| sb_accel(c)).collect(),
        face: (0..mesh.n_faces())
            .map(|f| {
                if geom.empty[f] {
                    0.0
                } else {
                    vec3::dot(sb_accel(fc[f]), sf[f])
                }
            })
            .collect(),
    }
}

fn sbc_solver(settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d_graded(
        20,
        12,
        0.3,
        0.1,
        [Grading::Symmetric(3.0), Grading::Symmetric(3.0)],
        false,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
    flow.body_force = Some(sbc_force(&geom));
    flow
}

/// ADR 0223 S0B, SBs, S1B, S1Bh: the consistent correctors with a body force keep the default's steady state of SBC.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s0b_sbs_s1b_s1bh_steady_state_with_a_body_force() {
    let dt = 0.002;
    let mut flow = sbc_solver(fb_settings());
    for _ in 0..3000 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S0B run stopped");
    }
    let s0 = flow.snapshot();
    let (u_s0, p_s0) = (s0.u.clone(), s0.p.clone());
    let rg = range(&p_s0);
    let us = u_s0.iter().fold(0.0f64, |m, v| m.max(vec3::norm(*v)));
    let (mut du_a, mut dp_a) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S0B branch A stopped");
        du_a = du_a.max(max_du(&flow.u.values, &u_s0));
        dp_a = dp_a.max(max_dp(&flow.p.values, &p_s0));
    }
    let (u_a, p_a) = (flow.u.values.clone(), flow.p.values.clone());
    // B: the static body force with the consistent correctors
    flow.restore(&s0).unwrap();
    flow.settings.consistent_correctors = true;
    flow.settings.consistent_body_force = true;
    let (mut du_b, mut dp_b) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S1B branch B stopped: {:?}", flow.diverged);
        du_b = du_b.max(max_du(&flow.u.values, &u_s0));
        dp_b = dp_b.max(max_dp(&flow.p.values, &p_s0));
    }
    let ab = (max_du(&flow.u.values, &u_a) / us, max_dp(&flow.p.values, &p_a) / rg);
    // B': the same force supplied by the hook
    let force = flow.body_force.take().unwrap();
    flow.restore(&s0).unwrap();
    let (mut du_h, mut dp_h) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        let f = force.clone();
        let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
            source_force: Some(f.clone()),
            ..Default::default()
        };
        flow.step_with(dt, Some(&mut hook as &mut OuterHook));
        assert!(flow.diverged.is_none(), "S1Bh branch B' stopped: {:?}", flow.diverged);
        du_h = du_h.max(max_du(&flow.u.values, &u_s0));
        dp_h = dp_h.max(max_dp(&flow.p.values, &p_s0));
    }
    println!(
        "S0B: U_s {us:.4e} m/s (SBs limit >= 1e-3); branch A (default) max|u - u_S0| {:.2e} U_s (limit 2e-11), max|p - p_S0| {:.2e} \
         of the range {rg:.4e}; S1B: branch B max|u - u_S0| {:.2e} U_s (limit 1e-10), max|p - p_S0| {:.2e} of the range (limit \
         1e-10); B against A after 50 steps {:.2e} U_s, {:.2e}; S1Bh: branch B' max|u - u_S0| {:.2e} U_s, max|p - p_S0| {:.2e}",
        du_a / us,
        dp_a / rg,
        du_b / us,
        dp_b / rg,
        ab.0,
        ab.1,
        du_h / us,
        dp_h / rg
    );
    assert!(du_a <= 2e-11 * us, "S0B {du_a}");
    assert!(us >= 1e-3, "SBs {us}");
    assert!(du_b <= 1e-10 * us, "S1B velocity {du_b}");
    assert!(dp_b <= 1e-10 * rg, "S1B pressure {dp_b}");
    assert!(du_h <= 1e-10 * us, "S1Bh velocity {du_h}");
    assert!(dp_h <= 1e-10 * rg, "S1Bh pressure {dp_h}");
}

// =================================================================================================================
// R2: the refusals with the extension flags
// =================================================================================================================

/// ADR 0214's 2-D box solver for the refusals (periodic in x for the periodic one).
fn box_solver(periodic: bool, settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d(8, 6, 1.0, 0.5, periodic, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let walls: &[&str] = if periodic {
        &["ymin", "ymax"]
    } else {
        &["xmin", "xmax", "ymin", "ymax"]
    };
    for name in walls {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

fn with_flags(consistent: bool, periodic: bool, body: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.01);
    s.consistent_correctors = consistent;
    s.consistent_periodic = periodic;
    s.consistent_body_force = body;
    s
}

/// One step of `flow` (with a body force from the hook when `hooked`): `Some(stage)` when the consistent correctors refused it,
/// with the state checked unchanged and the message checked.
fn refusal(label: &str, mut flow: PimpleSolver, hooked: bool) -> Option<&'static str> {
    let before = (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi));
    if hooked {
        let geom = flow.geom.clone();
        let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
            source_force: Some(BodyForce::uniform(&geom, [0.0, -9.81, 0.0])),
            ..Default::default()
        };
        flow.step_with(0.01, Some(&mut hook as &mut OuterHook));
    } else {
        flow.step(0.01);
    }
    let d = flow.diverged.clone()?;
    let msg = d.message(None);
    println!("R2 {label}: {msg}");
    assert_eq!(d.step, 1, "R2 {label} step");
    assert_eq!(d.quantity, "consistent correctors", "R2 {label} quantity");
    assert!(msg.contains("ADR 0214"), "R2 {label} message {msg}");
    assert_eq!(
        (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi)),
        before,
        "R2 {label}: the state changed"
    );
    Some(d.stage)
}

fn body(mut f: PimpleSolver) -> PimpleSolver {
    f.body_force = Some(BodyForce::uniform(&f.geom.clone(), [0.0, -9.81, 0.0]));
    f
}

/// ADR 0223 R2: the refusals that stay and the combinations the extension flags accept.
#[test]
fn r2_refusals_with_the_extension_flags() {
    // (i) the consistent correctors alone: as ADR 0214 R1
    let on = with_flags(true, false, false);
    assert_eq!(
        refusal("(i) periodic", box_solver(true, on.clone()), false),
        Some("periodic faces")
    );
    assert_eq!(
        refusal("(i) body force", body(box_solver(false, on.clone())), false),
        Some("body force")
    );
    assert_eq!(
        refusal("(i) body force (hook)", box_solver(false, on.clone()), true),
        Some("body force")
    );
    // (ii) with consistent_periodic: the periodic box is accepted
    assert_eq!(
        refusal("(ii) periodic", box_solver(true, with_flags(true, true, false)), false),
        None
    );
    // (iii) with consistent_body_force: the static and the hook body force are accepted
    let b = with_flags(true, false, true);
    assert_eq!(
        refusal("(iii) body force", body(box_solver(false, b.clone())), false),
        None
    );
    assert_eq!(refusal("(iii) body force (hook)", box_solver(false, b), true), None);
    // (iv) with both flags the other refusals stay
    let all = with_flags(true, true, true);
    let mut s = all.clone();
    s.fractional = Some(FractionalSettings::default());
    assert_eq!(
        refusal("(iv) fractional", box_solver(false, s), false),
        Some("fractional step")
    );
    let mut s = all.clone();
    s.non_orth_correctors = 1;
    assert_eq!(
        refusal("(iv) non-orthogonal", box_solver(false, s), false),
        Some("non-orthogonal correctors")
    );
    let mut s = all.clone();
    s.component_diagonal = true;
    assert_eq!(
        refusal("(iv) component diagonal", box_solver(false, s), false),
        Some("component diagonal")
    );
    let mut f = box_solver(false, all);
    f.face_jump = Some(FaceJump::zeros(f.geom.mesh.n_faces()));
    assert_eq!(refusal("(iv) face jump", f, false), Some("face jump"));
    // (v) the two flags without the consistent correctors are inert
    let run = |s: PimpleSettings| {
        let mut f = body(box_solver(true, s));
        for _ in 0..3 {
            f.step(0.01);
            assert!(f.diverged.is_none(), "R2 (v) stopped");
        }
        (ubits(&f.u.values), bits(&f.p.values), bits(&f.phi))
    };
    assert_eq!(
        run(with_flags(false, true, true)),
        run(PimpleSettings::piso(0.01)),
        "R2 (v): the flags without the consistent correctors changed the run"
    );
}

// =================================================================================================================
// MSG1, N1
// =================================================================================================================

/// ADR 0223 MSG1: the corrected Raise message on ADR 0214 L1's set-up (FX at `tau/10`, two correctors).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn msg1_the_raise_message_names_the_short_waves_and_the_consistent_correctors() {
    let fb = Fb::new();
    let dt = fb.forms.tau / 10.0;
    let mut s = fb_settings();
    s.pressure_lag_check = Some(LagCheck {
        every: 0,
        start: 1,
        iterations: 20,
        action: LagAction::Raise,
    });
    let mut flow = fb.solver(s);
    fb.set_block(&mut flow, dt);
    flow.step(dt);
    let d = flow.diverged.clone().expect("MSG1: the check did not stop the run");
    let msg = d.message(Some(fb.g0.mesh.cell_centres()));
    println!("MSG1: {msg}");
    for part in [
        "pressure-lag check",
        "n_correctors >= 2",
        "cell",
        "short pressure waves",
        "ADR 0214",
        "solver.consistent_correctors.enabled",
    ] {
        assert!(msg.contains(part), "MSG1: '{part}' missing");
    }
    assert!(!msg.contains("with one pressure corrector"), "MSG1: the old text");
}

/// ADR 0223 N1: the new options are off by default.
#[test]
fn n1_the_new_options_are_off_by_default() {
    for s in [PimpleSettings::piso(0.01), PimpleSettings::pimple(0.01, 3)] {
        assert!(!s.consistent_correctors && !s.consistent_periodic && !s.consistent_body_force && !s.moving_mesh_lag);
    }
}
