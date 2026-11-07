//! F-244 (ADR 0211): flow-coupled six-DOF validation with the loose coupling of ADR 0033 D7.
//!
//! (a) A block falling along a closed 2-D channel, the displaced fluid returning through the two side gaps (a falling-piston
//! viscometer), against the terminal velocity of the lubrication closed form `U_t = (M - rho V_b) g / C`,
//! `C = 2 mu H_b dz (3 B^2 + 1)/h`, `B = (w_b + h)/h`.
//! (b) The C2 piston of ADR 0033 on a spring, free decay against `omega = sqrt(k/(M + rho A L0))`.
//!
//! The limits, set-ups and the bound of the neglected terms were registered in ADR 0211 before this file existed.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, MeshStep};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{cell_swept_sum, moved_mesh, patch_points};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

/// Discrete GCL residual of one mesh step: `max_c |sum_f sigma_f W_f - (V^{n+1} - V^n)|` (ADR 0033 G1).
fn gcl_residual(old: &Mesh, st: &MeshStep, dt: f64) -> f64 {
    let w: Vec<f64> = st.mesh_flux.iter().map(|m| m * dt).collect();
    cell_swept_sum(old, &w)
        .iter()
        .zip(old.cell_volumes().iter().zip(st.geom.mesh.cell_volumes()))
        .map(|(s, (v0, v1))| (s - (v1 - v0)).abs())
        .fold(0.0, f64::max)
}

fn no_cyclic(mesh: &Mesh) -> bool {
    mesh.patches().iter().all(|p| p.kind != PatchKind::Cyclic)
}

/// Nodes and weights of the `n`-point Gauss-Legendre rule on `[-1, 1]` (Newton on the Legendre recurrence).
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 1.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=n {
                    let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                    p0 = p1;
                    p1 = p2;
                }
                dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
                let dx = p1 / dp;
                x -= dx;
                if dx.abs() < 1e-16 {
                    break;
                }
            }
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

// =================================================================================================================
// (a) Falling block (set-up FB of ADR 0211)
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

/// Closed forms of ADR 0211 M1 to M3 and the end-layer quantities of M4 for the numbers of FB.
#[derive(Debug, Clone, Copy)]
struct FbForms {
    /// Lubrication drag coefficient `C` (`D = C U`).
    c: f64,
    /// Drag coefficient of a block whose sides did not move (Poiseuille only in the gaps, negative control AN1).
    c_fixed: f64,
    /// Quasi-steady added mass of the gaps.
    m_a: f64,
    /// Block volume.
    v_b: f64,
    /// Time constant `(M + m_a)/C`.
    tau: f64,
    /// Terminal velocity `(M - rho V_b) g / C`.
    u_t: f64,
}

fn fb_forms() -> FbForms {
    let mu = RHO * NU;
    let b = (WB + H) / H;
    let c = 2.0 * mu * HB * DZ * (3.0 * b * b + 1.0) / H;
    let c_fixed = 6.0 * mu * WB * (WB + H) * HB * DZ / H.powi(3);
    let m_a = 2.0 * RHO * HB * H * DZ * (0.3 * b * b - 0.5 * b + 1.0 / 3.0);
    let v_b = WB * HB * DZ;
    FbForms {
        c,
        c_fixed,
        m_a,
        v_b,
        tau: (MASS + m_a) / c,
        u_t: (MASS - RHO * v_b) * GRAV / c,
    }
}

/// The trial field of ADR 0211 M4 in units `h = U = 1`: `f(x)` on `[0, W/2]` and its two derivatives.
struct Trial {
    a: f64,
    b: f64,
    half: f64,
}

impl Trial {
    fn new(bb: f64) -> Self {
        Trial {
            a: -(3.0 * bb - 1.0) / 2.0,
            b: bb,
            half: 1.0 + (bb - 1.0) / 2.0,
        }
    }
    fn f(&self, x: f64) -> [f64; 3] {
        if x <= 1.0 {
            [
                self.a * x * x + self.b * x.powi(3),
                2.0 * self.a * x + 3.0 * self.b * x * x,
                2.0 * self.a + 6.0 * self.b * x,
            ]
        } else {
            [x - self.half, 1.0, 0.0]
        }
    }
}

/// `J(delta)` of ADR 0211 M4 from the closed-form integrals, units `h = U = 1`.
fn trial_j_closed(bb: f64, delta: f64) -> (f64, [f64; 3]) {
    let (a, b, wb) = (-(3.0 * bb - 1.0) / 2.0, bb, bb - 1.0);
    let f2 = 1.0 + 3.0 * bb * bb;
    let f1 = (3.0 * bb - 1.0).powi(2) / 3.0 - 3.0 * bb * (3.0 * bb - 1.0) / 2.0 + 9.0 * bb * bb / 5.0 + wb / 2.0;
    let f0 = a * a / 5.0 + a * b / 3.0 + b * b / 7.0 + (wb / 2.0).powi(3) / 3.0;
    (
        13.0 / 35.0 * delta * f2 + 12.0 / 5.0 * f1 / delta + 12.0 * f0 / delta.powi(3),
        [f0, f1, f2],
    )
}

/// `J(delta)` by Gauss-Legendre quadrature of the dissipation `4 psi_xs^2 + (psi_ss - psi_xx)^2` of the 2-D trial field
/// `psi = f(x) g(s/delta)`, `g = 1 - 3 sigma^2 + 2 sigma^3`, over one end layer of one half (40 points per piece).
fn trial_j_quadrature(bb: f64, delta: f64) -> f64 {
    let t = Trial::new(bb);
    let gl = gauss_legendre(40);
    let map = |lo: f64, hi: f64| -> Vec<(f64, f64)> {
        gl.iter()
            .map(|(x, w)| (0.5 * (hi - lo) * x + 0.5 * (hi + lo), 0.5 * (hi - lo) * w))
            .collect()
    };
    let mut xs = map(0.0, 1.0);
    xs.extend(map(1.0, t.half));
    let ss = map(0.0, delta);
    let mut sum = 0.0;
    for &(x, wx) in &xs {
        let [f, f1, f2] = t.f(x);
        for &(s, ws) in &ss {
            let q = s / delta;
            let g = 1.0 - 3.0 * q * q + 2.0 * q.powi(3);
            let g1 = (-6.0 * q + 6.0 * q * q) / delta;
            let g2 = (-6.0 + 12.0 * q) / (delta * delta);
            let (pxx, pss, pxs) = (f2 * g, f * g2, f1 * g1);
            sum += wx * ws * (4.0 * pxs * pxs + (pss - pxx).powi(2));
        }
    }
    sum
}

/// The registered bound of the neglected terms (ADR 0211 M4): `(eps_1, eps_2, eps_3)`.
fn fb_bound() -> (f64, f64, f64) {
    let f = fb_forms();
    let bb = (WB + H) / H;
    let (j, _) = trial_j_closed(bb, 1.64);
    let lub = 3.0 * bb * bb + 1.0;
    let eps1 = 2.0 * j / ((HB / H) * lub);
    // peak speed of the gap profile relative to the block, 3 B xi (1 - xi) - xi + 1 at xi = (3B - 1)/(6B)
    let xi = (3.0 * bb - 1.0) / (6.0 * bb);
    let vm = 3.0 * bb * xi * (1.0 - xi) - xi + 1.0;
    let re = f.u_t * H / NU;
    let eps2 = 4.0 * vm.powi(4) * (width() / (PI * H)).powi(2) * re * re / (2.0 * (HB / H) * lub);
    let eps3 = (-8.0f64).exp() + 1e-5;
    (eps1, eps2, eps3)
}

/// Cell sizes growing from `first` by `growth` while below `cap`, starting at the fine end.
fn ramp(first: f64, growth: f64, cap: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut s = first;
    while s < cap {
        out.push(s);
        s *= growth;
    }
    out
}

/// One-sided distribution of `len`: the ramp, then equal cells of at most `cap`. Sizes from the fine end.
fn one_sided(len: f64, first: f64, growth: f64, cap: f64) -> Vec<f64> {
    let mut sizes = ramp(first, growth, cap);
    let rest = len - sizes.iter().sum::<f64>();
    assert!(rest > 0.0, "ramp longer than the segment");
    let n = (rest / cap).ceil() as usize;
    let len = sizes.len();
    sizes.resize(len + n, rest / n as f64);
    sizes
}

/// Two-sided distribution of `len`: the ramp at both ends, equal cells of at most `cap` in the middle.
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

/// Append the coordinates of a segment `[start, end]` with the given sizes (the first coordinate is already present).
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

/// The mesh of FB: a masked structured grid (one cell in z) with the block removed. Returns the mesh and
/// `(columns, rows, cells across the block, rows along the block)`.
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
    // compact point numbering: only the corners of fluid cells
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
    classes: Vec<PatchClass>,
    block_points: Vec<usize>,
    d_unit: Vec<Vec3>,
    counts: [usize; 4],
}

impl Fb {
    fn new() -> Self {
        let (mesh0, counts) = falling_block_mesh();
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
        let d_unit = Self::smooth(&g0, &classes, &block_points, mesh0.n_points(), 1.0);
        Fb {
            mesh0,
            g0,
            classes,
            block_points,
            d_unit,
            counts,
        }
    }

    fn smooth(g0: &FvGeometry, classes: &[PatchClass], block: &[usize], n: usize, s: f64) -> Vec<Vec3> {
        let mut presc = vec![vec3::ZERO; n];
        for &q in block {
            presc[q] = [0.0, s, 0.0];
        }
        smooth_displacement(g0, classes, &presc, SmoothingMethod::Laplacian, &tight(5000))
            .unwrap()
            .displacement
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
}

/// Result of one coupled FB run.
#[derive(Debug, Clone)]
struct FbRun {
    /// `(t, U)` after every step, `U = -v_y`.
    history: Vec<(f64, f64)>,
    cont: f64,
    gcl: f64,
    vol: f64,
    min_ratio: f64,
    travel: f64,
    p_iters: usize,
}

fn fb_run(fb: &Fb, steps_per_tau: usize) -> FbRun {
    let forms = fb_forms();
    let dt = forms.tau / steps_per_tau as f64;
    let geom = fb.g0.clone();
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in fb.walls() {
        u.set_bc(&geom, &name, Box::new(FixedValue::vector(vec3::ZERO)))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, fb_settings());
    let mut spec = BodySpec::new(MASS, [1.0, 1.0, 1.0]);
    spec.translation_locked = [true, false, true];
    spec.gravity = [0.0, -GRAV, 0.0];
    let mut body = Body::new(spec, BodyState::at([0.5 * width(), YB + 0.5 * HB, 0.5 * DZ])).unwrap();
    let buoyancy = RHO * forms.v_b * GRAV;
    body.set_hook(Box::new(move |_s, _t| ([0.0, buoyancy, 0.0], vec3::ZERO)));
    let y0 = body.state.x[1];
    let v0 = fb.mesh0.total_volume();
    let mean_v = v0 / fb.mesh0.n_cells() as f64;
    let vol_init = fb.mesh0.cell_volumes().to_vec();
    let patches = vec!["block".to_string()];
    let mut run = FbRun {
        history: Vec::new(),
        cont: 0.0,
        gcl: 0.0,
        vol: 0.0,
        min_ratio: f64::INFINITY,
        travel: 0.0,
        p_iters: 0,
    };
    for n in 1..=10 * steps_per_tau {
        let (f, m) = patch_force_moment(&flow.geom, &flow.u, &flow.p, RHO, NU, &patches, body.state.x).unwrap();
        body.advance(dt, Integrator::Rk4, (f, m)).unwrap();
        let s = body.state.x[1] - y0;
        let st = move_mesh(&flow.geom, fb.points(s), dt).unwrap();
        let old = flow.geom.mesh.clone();
        run.gcl = run.gcl.max(gcl_residual(&old, &st, dt) / mean_v);
        run.vol = run.vol.max((st.geom.mesh.total_volume() - v0).abs() / v0);
        for (a, b) in st.geom.mesh.cell_volumes().iter().zip(&vol_init) {
            run.min_ratio = run.min_ratio.min(a / b);
        }
        let walls = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt).unwrap();
        let info = flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        assert!(
            flow.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity at step {n}"
        );
        run.cont = run.cont.max(info.continuity);
        run.p_iters += info.p_iters;
        run.history.push((n as f64 * dt, -body.state.v[1]));
        run.travel = -s;
    }
    run
}

/// ADR 0211 A0 and A6: the bound of the neglected terms and the added-mass ratio, from the closed forms (no flow).
#[test]
fn a0_falling_block_bound_of_the_neglected_terms() {
    let f = fb_forms();
    let bb = (WB + H) / H;
    let (j, [f0, f1, f2]) = trial_j_closed(bb, 1.64);
    let jq = trial_j_quadrature(bb, 1.64);
    let rel = (jq - j).abs() / j;
    let (e1, e2, e3) = fb_bound();
    let total = e1 + e2 + e3;
    // A6: kinetic energy of the four end layers of the trial field over that of the gaps (ADR 0211 M2)
    let delta = 1.64;
    let layers = 4.0 * (f1 * delta * 13.0 / 35.0 + f0 * 6.0 / 5.0 / delta);
    let gaps = 2.0 * (HB / H) * (0.3 * bb * bb - 0.5 * bb + 1.0 / 3.0);
    let ratio = f.m_a * (1.0 + layers / gaps) / MASS;
    println!(
        "A0: F0 {f0:.4} F1 {f1:.4} F2 {f2:.1}; J(1.64 h) closed {j:.6}, quadrature {jq:.6} (relative {rel:.1e}, limit 1e-10); \
         eps_1 {:.4} %, eps_2 {:.4} %, eps_3 {:.4} %, total {:.4} % (limit 0.5 %)",
        100.0 * e1,
        100.0 * e2,
        100.0 * e3,
        100.0 * total
    );
    println!(
        "A6: C {:.4}, m_a {:.6e}, m_a/M {:.4}, with the end layers {ratio:.4} (limit 0.2); tau {:.6e}, U_t {:.6e}, Re_U {:.4}, \
         displaced mass / m_a {:.3}, U_t' (sides at rest) / U_t {:.4}",
        f.c,
        f.m_a,
        f.m_a / MASS,
        f.tau,
        f.u_t,
        f.u_t * H / NU,
        RHO * f.v_b / f.m_a,
        f.c / f.c_fixed
    );
    assert!(rel <= 1e-10, "A0 quadrature {rel}");
    assert!(total <= 5e-3, "A0 total {total}");
    assert!(ratio <= 0.2, "A6 {ratio}");
}

/// Set-up smoke (ADR 0211 disclosure): the FB mesh builds, two coupled steps stay finite. Prints only counts.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn fb_setup_smoke() {
    let fb = Fb::new();
    let [nx, ny, bx, by] = fb.counts;
    println!(
        "FB smoke: {} cells, {} points, grid {nx} x {ny}, block {bx} x {by}, patches {:?}",
        fb.mesh0.n_cells(),
        fb.mesh0.n_points(),
        fb.mesh0
            .patches()
            .iter()
            .map(|p| (p.name.clone(), p.faces().len()))
            .collect::<Vec<_>>()
    );
    let forms = fb_forms();
    let dt = forms.tau / 10.0;
    let geom = fb.g0.clone();
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in fb.walls() {
        u.set_bc(&geom, &name, Box::new(FixedValue::vector(vec3::ZERO)))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, fb_settings());
    let patches = vec!["block".to_string()];
    for n in 1..=2 {
        let s = -1e-5 * n as f64;
        let st = move_mesh(&flow.geom, fb.points(s), dt).unwrap();
        let old = flow.geom.mesh.clone();
        let walls = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt).unwrap();
        let info = flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        println!("FB smoke step {n}: pressure iterations {}", info.p_iters);
        assert!(flow.u.values.iter().flatten().all(|x| x.is_finite()));
    }
}

/// ADR 0211 A1 to A5, A7 to A9 and AN1 on the runs `dt = tau/spt` and `tau/(2 spt)`.
fn falling_block_terminal_velocity(spt: usize) {
    let fb = Fb::new();
    let forms = fb_forms();
    let r1 = fb_run(&fb, spt);
    let r2 = fb_run(&fb, 2 * spt);
    let u1 = r1.history.last().unwrap().1;
    let u2 = r2.history.last().unwrap().1;
    let u9 = r1.history[9 * spt - 1].1;
    let a1 = u1 / forms.u_t;
    let a2 = (u2 - u1).abs() / u1.abs();
    let a7 = (u1 - u9).abs() / forms.u_t;
    let u_fixed = forms.u_t * forms.c / forms.c_fixed;
    let an1 = (u1 - u_fixed).abs() / u_fixed;
    // A8: the scaled unit field against a direct smoothing at the final displacement of the first run
    let s_end = -r1.travel;
    let direct = Fb::smooth(&fb.g0, &fb.classes, &fb.block_points, fb.mesh0.n_points(), s_end);
    let a8 = fb
        .d_unit
        .iter()
        .zip(&direct)
        .map(|(d, x)| vec3::norm(vec3::sub(vec3::scale(*d, s_end), *x)))
        .fold(0.0, f64::max)
        / s_end.abs();
    for t in [1usize, 2, 5] {
        let (time, uu) = r1.history[spt * t - 1];
        println!(
            "A approach: t = {:.0} tau: U/U_t {:.5}, closed form 1 - e^-t/tau {:.5}",
            time / forms.tau,
            uu / forms.u_t,
            1.0 - (-time / forms.tau).exp()
        );
    }
    println!(
        "A: {} cells; tau/{spt}: U(10 tau)/U_t {a1:.5} (limit |. - 1| <= 0.02), continuity {:.2e}, GCL {:.2e} mean(V),          volume {:.2e} V0, min V/V0 {:.4}, travel {:.4} h, pressure iterations {}",
        fb.mesh0.n_cells(),
        r1.cont,
        r1.gcl,
        r1.vol,
        r1.min_ratio,
        r1.travel / H,
        r1.p_iters
    );
    println!(
        "A: tau/{}: U(10 tau)/U_t {:.5}, continuity {:.2e}, GCL {:.2e} mean(V), volume {:.2e} V0, min V/V0 {:.4},          pressure iterations {}",
        2 * spt,
        u2 / forms.u_t,
        r2.cont,
        r2.gcl,
        r2.vol,
        r2.min_ratio,
        r2.p_iters
    );
    println!(
        "A2 dt halving {a2:.2e} (limit 5e-3); A7 |U(10 tau) - U(9 tau)|/U_t {a7:.2e} (limit 1e-3); A8 {a8:.2e} (limit 1e-9);          AN1 against U_t' {an1:.4} (limit >= 0.08)"
    );
    assert!((a1 - 1.0).abs() <= 0.02, "A1 {a1}");
    assert!(a2 <= 5e-3, "A2 {a2}");
    for r in [&r1, &r2] {
        assert!(r.cont <= 1e-10, "A3 {}", r.cont);
        assert!(r.gcl <= 1e-12, "A4 GCL {}", r.gcl);
        assert!(r.vol <= 1e-12, "A4 volume {}", r.vol);
        assert!(r.min_ratio >= 0.5, "A5 {}", r.min_ratio);
    }
    assert!(a7 <= 1e-3, "A7 {a7}");
    assert!(a8 <= 1e-9, "A8 {a8}");
    assert!(r1.travel >= 0.25 * H, "A9 travel {}", r1.travel);
    assert!(no_cyclic(&fb.mesh0), "A9 cyclic patch");
    assert!(an1 >= 0.08, "AN1 {an1}");
}

/// ADR 0211 A1 as registered (`dt = tau/10` and `tau/20`). The baseline PISO step is unstable on this mesh above a step
/// between `tau/40` and `tau/20` (reproduced on a fixed mesh with the block at rest, ADR 0211 amendment 1): the registered run
/// stops at step 31 with a non-positive cell volume. Kept as the record of the registered set-up.
#[test]
#[ignore = "baseline PISO unstable on this mesh at dt = tau/10 and tau/20 (fixed-mesh reproduction; ADR 0211 amendment 1)"]
fn a1_falling_block_reaches_the_lubrication_terminal_velocity_known_miss() {
    falling_block_terminal_velocity(10);
}

/// ADR 0211 amendment 1: A1 to A5, A7 to A9 and AN1 with `dt = tau/40` and `tau/80`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1b_falling_block_reaches_the_lubrication_terminal_velocity() {
    falling_block_terminal_velocity(40);
}

/// ADR 0211 A4u: a uniform flow stays uniform on the FB mesh moved by the closed-form travel.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a4u_falling_block_mesh_keeps_a_uniform_flow() {
    let fb = Fb::new();
    let forms = fb_forms();
    let u0 = [0.3, -0.2, 0.0];
    let geom = fb.g0.clone();
    let mut u = VectorField::uniform("U", &geom, u0);
    for name in fb.walls() {
        u.set_bc(&geom, &name, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, fb_settings());
    let dt = forms.tau / 10.0;
    let (mut du, mut dp): (f64, f64) = (0.0, 0.0);
    for n in 1..=100 {
        let t = n as f64 * dt;
        let s = -forms.u_t * (t - forms.tau * (1.0 - (-t / forms.tau).exp()));
        let st = move_mesh(&flow.geom, fb.points(s), dt).unwrap();
        flow.step_ale(dt, st.geom, st.mesh_flux).unwrap();
        for v in &flow.u.values {
            du = du.max(vec3::norm(vec3::sub(*v, u0)));
        }
        for x in &flow.p.values {
            dp = dp.max(x.abs());
        }
    }
    let un = vec3::norm(u0);
    println!(
        "A4u: max|u - u0| {:.2e} |u0| (limit 1e-12), max|p| {:.2e} |u0|^2 (limit 1e-12)",
        du / un,
        dp / (un * un)
    );
    assert!(du <= 1e-12 * un, "A4u velocity {du}");
    assert!(dp <= 1e-12 * un * un, "A4u pressure {dp}");
}

// =================================================================================================================
// (b) Spring-mounted piston (set-up SP of ADR 0211)
// =================================================================================================================

const SP_M: f64 = 0.2;
const SP_A: f64 = 0.04;
const SP_L0: f64 = 1.0;
const SP_X: f64 = 0.01;
const SP_NU: f64 = 0.01;

fn sp_k() -> f64 {
    TAU * TAU * (SP_M + RHO * SP_A * SP_L0)
}

fn sp_omega() -> f64 {
    (sp_k() / (SP_M + RHO * SP_A * SP_L0)).sqrt()
}

fn sp_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(SP_NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(5000);
    s
}

/// The C2 channel of ADR 0033 with its smoothing classes.
struct Sp {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    piston: Vec<usize>,
}

impl Sp {
    fn new() -> Self {
        let mesh0 = box_mesh([20, 2, 2], [SP_L0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
        let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
        let classes = mesh0
            .patches()
            .iter()
            .map(|pt| match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        let piston = patch_points(&mesh0, "xmin").unwrap();
        Sp {
            mesh0,
            g0,
            classes,
            piston,
        }
    }

    /// Points with the piston at `x_p` (Laplacian smoothing from the initial mesh, as C2).
    fn points(&self, x_p: f64) -> Vec<Vec3> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for &q in &self.piston {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .unwrap();
        self.mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect()
    }

    /// Geometry with the piston at `x_p`.
    fn geom_at(&self, x_p: f64) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&self.mesh0, self.points(x_p)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }
}

/// Result of one coupled SP run.
#[derive(Debug, Clone)]
struct SpRun {
    t: Vec<f64>,
    x: Vec<f64>,
    cont: f64,
    gcl: f64,
    p_iters: usize,
}

fn sp_run(sp: &Sp, steps_per_period: usize, fluid_on: bool) -> SpRun {
    let dt = 1.0 / steps_per_period as f64;
    let geom = sp.geom_at(SP_X);
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut flow = PimpleSolver::new(geom, u, p, sp_settings());
    let mut spec = BodySpec::new(SP_M, [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [sp_k(), 0.0, 0.0];
    let mut body = Body::new(spec, BodyState::at([SP_X, 0.0, 0.0])).unwrap();
    let mean_v = sp.mesh0.total_volume() / sp.mesh0.n_cells() as f64;
    let patches = vec!["xmin".to_string()];
    let mut run = SpRun {
        t: vec![0.0],
        x: vec![SP_X],
        cont: 0.0,
        gcl: 0.0,
        p_iters: 0,
    };
    let steps = (6.5 * steps_per_period as f64).round() as usize;
    for n in 1..=steps {
        let (f, m) = patch_force_moment(&flow.geom, &flow.u, &flow.p, RHO, SP_NU, &patches, body.state.x).unwrap();
        let fluid = if fluid_on { (f, m) } else { (vec3::ZERO, vec3::ZERO) };
        body.advance(dt, Integrator::Rk4, fluid).unwrap();
        let x_p = body.state.x[0];
        let st = move_mesh(&flow.geom, sp.points(x_p), dt).unwrap();
        let old = flow.geom.mesh.clone();
        run.gcl = run.gcl.max(gcl_residual(&old, &st, dt) / mean_v);
        let walls = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt).unwrap();
        let info = flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        assert!(
            flow.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity at step {n}"
        );
        run.cont = run.cont.max(info.continuity);
        run.p_iters += info.p_iters;
        run.t.push(n as f64 * dt);
        run.x.push(x_p);
    }
    run
}

/// Upward zero crossings by linear interpolation (ADR 0033 R2).
fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

/// `omega_m = 2 pi (N - 1)/(t_N - t_1)` over all upward crossings, and their number.
fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

/// Turning points refined by the parabola through three samples; the start (at rest) is the first maximum.
fn turning_points(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut maxima = vec![x[0]];
    let mut minima = Vec::new();
    for i in 1..x.len() - 1 {
        let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
        let curv = a - 2.0 * b + c;
        if b > a && b >= c && curv != 0.0 {
            maxima.push(b - (c - a).powi(2) / (8.0 * curv));
        } else if b < a && b <= c && curv != 0.0 {
            minima.push(b - (c - a).powi(2) / (8.0 * curv));
        }
    }
    (maxima, minima)
}

/// Largest ratio of successive maxima and of the magnitudes of successive minima.
fn worst_growth(x: &[f64]) -> (f64, Vec<f64>, Vec<f64>) {
    let (mx, mn) = turning_points(x);
    let mut worst: f64 = 0.0;
    for w in mx.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in mn.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    (worst, mx, mn)
}

/// ADR 0211 B0: the period of the model ODE `(M + rho A (L0 - x)) x'' = -k x` against `2 pi/omega`, and the parameters.
#[test]
fn b0_spring_column_model_period_and_parameters() {
    let (k, m0) = (sp_k(), SP_M + RHO * SP_A * SP_L0);
    let acc = |x: f64| -k * x / (m0 - RHO * SP_A * x);
    let dt = 1.0 / 2000.0;
    let (mut x, mut v) = (SP_X, 0.0);
    let (mut ts, mut xs) = (vec![0.0], vec![x]);
    for n in 1..=13000 {
        let (k1x, k1v) = (v, acc(x));
        let (k2x, k2v) = (v + 0.5 * dt * k1v, acc(x + 0.5 * dt * k1x));
        let (k3x, k3v) = (v + 0.5 * dt * k2v, acc(x + 0.5 * dt * k2x));
        let (k4x, k4v) = (v + dt * k3v, acc(x + dt * k3x));
        x += dt / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        v += dt / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        ts.push(n as f64 * dt);
        xs.push(x);
    }
    let (om, n) = measured_omega(&ts, &xs);
    let rel = (TAU / om - TAU / sp_omega()).abs() / (TAU / sp_omega());
    let ratio = RHO * SP_A * SP_L0 / SP_M;
    let shift = ((SP_M + RHO * SP_A * SP_L0) / SP_M).sqrt() - 1.0;
    println!(
        "B0: model period / (2 pi/omega) - 1 = {rel:.2e} over {n} crossings (limit 1e-5); rho A L0/M {ratio:.4} (limit 0.2); \
         fluid-free frequency shift {shift:.4} (limit >= 0.08); k {k:.6}"
    );
    assert!(rel <= 1e-5, "B0 period {rel}");
    assert!(ratio <= 0.2 + 1e-15, "B0 mass ratio {ratio}");
    assert!(shift >= 0.08, "B0 shift {shift}");
}

/// ADR 0211 B1 to B3, B5, B6: the spring-mounted piston oscillates at the added-mass frequency.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn b1_spring_mounted_piston_oscillates_at_the_added_mass_frequency() {
    let sp = Sp::new();
    let r1 = sp_run(&sp, 100, true);
    let r2 = sp_run(&sp, 200, true);
    let (om1, n1) = measured_omega(&r1.t, &r1.x);
    let (om2, n2) = measured_omega(&r2.t, &r2.x);
    let b1 = om1 / sp_omega();
    let b2 = (om2 - om1).abs() / om1;
    let (g1, mx1, mn1) = worst_growth(&r1.x);
    let (g2, mx2, mn2) = worst_growth(&r2.x);
    let amp = r1.x.iter().chain(&r2.x).fold(0.0f64, |a, b| a.max(b.abs()));
    println!(
        "B: T/100: omega_m/omega {b1:.6} (limit |. - 1| <= 0.02) over {n1} crossings, continuity {:.2e}, GCL {:.2e} mean(V), \
         pressure iterations {}",
        r1.cont, r1.gcl, r1.p_iters
    );
    println!(
        "B: T/200: omega_m/omega {:.6} over {n2} crossings, continuity {:.2e}, GCL {:.2e} mean(V), pressure iterations {}",
        om2 / sp_omega(),
        r2.cont,
        r2.gcl,
        r2.p_iters
    );
    println!("B2 dt halving {b2:.2e} (limit 5e-3); B5 largest turning-point ratio T/100 {g1:.5}, T/200 {g2:.5} (limit 1 + 1e-3)");
    println!("B5 T/100 maxima {mx1:.5?} minima {mn1:.5?}");
    println!("B5 T/200 maxima {mx2:.5?} minima {mn2:.5?}");
    println!("B6: max |x| {amp:.6e} (limit 0.01 L0 (1 + 1e-9)), crossings {n1} and {n2} (limit >= 6)");
    assert!((b1 - 1.0).abs() <= 0.02, "B1 {b1}");
    assert!(b2 <= 5e-3, "B2 {b2}");
    for r in [&r1, &r2] {
        assert!(r.cont <= 1e-10, "B3 {}", r.cont);
        assert!(r.gcl <= 1e-12, "B4 GCL {}", r.gcl);
    }
    assert!(g1 <= 1.0 + 1e-3 && g2 <= 1.0 + 1e-3, "B5 {g1} {g2}");
    assert!(n1 >= 6 && n2 >= 6, "B6 crossings {n1} {n2}");
    assert!(amp <= 0.01 * SP_L0 * (1.0 + 1e-9), "B6 amplitude {amp}");
    assert!(no_cyclic(&sp.mesh0), "B6 cyclic patch");
}

/// ADR 0211 BN1 (negative control): with the fluid force off the frequency misses `omega` by at least 8 percent.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn bn1_spring_piston_without_fluid_forces_misses_the_frequency() {
    let sp = Sp::new();
    let r = sp_run(&sp, 100, false);
    let (om, n) = measured_omega(&r.t, &r.x);
    let miss = (om / sp_omega() - 1.0).abs();
    println!(
        "BN1: fluid force off: omega_m/omega {:.6} over {n} crossings, |. - 1| {miss:.4} (limit >= 0.08; sqrt(k/M)/omega = {:.6})",
        om / sp_omega(),
        ((SP_M + RHO * SP_A * SP_L0) / SP_M).sqrt()
    );
    assert!(miss >= 0.08, "BN1 {miss}");
}

/// ADR 0211 B4u: a uniform flow stays uniform on the SP mesh moved by `x_p = X cos(2 pi t)`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn b4u_spring_piston_mesh_keeps_a_uniform_flow() {
    let sp = Sp::new();
    let u0 = [0.3, -0.2, 0.1];
    let geom = sp.geom_at(SP_X);
    let mut u = VectorField::uniform("U", &geom, u0);
    for pt in sp.mesh0.patches() {
        u.set_bc(&geom, &pt.name, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, sp_settings());
    let dt = 0.01;
    let (mut du, mut dp): (f64, f64) = (0.0, 0.0);
    for n in 1..=100 {
        let x_p = SP_X * (TAU * n as f64 * dt).cos();
        let st = move_mesh(&flow.geom, sp.points(x_p), dt).unwrap();
        flow.step_ale(dt, st.geom, st.mesh_flux).unwrap();
        for v in &flow.u.values {
            du = du.max(vec3::norm(vec3::sub(*v, u0)));
        }
        for x in &flow.p.values {
            dp = dp.max(x.abs());
        }
    }
    let un = vec3::norm(u0);
    println!(
        "B4u: max|u - u0| {:.2e} |u0| (limit 1e-12), max|p| {:.2e} |u0|^2 (limit 1e-12)",
        du / un,
        dp / (un * un)
    );
    assert!(du <= 1e-12 * un, "B4u velocity {du}");
    assert!(dp <= 1e-12 * un * un, "B4u pressure {dp}");
}
