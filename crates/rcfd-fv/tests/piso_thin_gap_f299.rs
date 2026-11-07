//! F-299 (ADR 0214): the PISO/PIMPLE instability at a large cell diffusion number across a thin gap.
//!
//! D: the mechanism H-RC (the first corrector over-corrects by about the cross-gap diffusion number, and the Rhie-Chow term
//! that the cell-gradient velocity update leaves for a pressure wave of four rows along the gap is not removed by the second
//! corrector), checked against the replica values of ADR 0214 M3 by twin-run power iteration of the solver's own step (TW).
//! L: the located refusal of the default on a fixed mesh (the ADR 0073 B4 check in Raise mode).
//! F, S, A1c, A4u-c, R1, N1: the opt-in `PimpleSettings::consistent_correctors` (ADR 0214 F0).
//!
//! The falling-block mesh, its constants and the coupled loop with the A checks are copied unchanged from
//! `sixdof_flow_f244.rs` (ADR 0211 set-up FB, amendment 1's helper); the copy is asserted to give ADR 0211's cell counts. The
//! limits were registered in ADR 0214 before this file existed.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, MeshStep};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{LagAction, LagCheck, PimpleSettings, PimpleSolver, PimpleState};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, Grading};
use rcfd_mesh::motion::{cell_swept_sum, patch_points};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::sync::Arc;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

// =================================================================================================================
// The FB set-up of ADR 0211 (copied from sixdof_flow_f244.rs, unchanged)
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
    /// Lubrication drag coefficient `C` (`D = C U`).
    c: f64,
    /// Drag coefficient of a block whose sides did not move (Poiseuille only in the gaps, negative control AN1).
    c_fixed: f64,
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
        v_b,
        tau: (MASS + m_a) / c,
        u_t: (MASS - RHO * v_b) * GRAV / c,
    }
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

// =================================================================================================================
// FX: the fixed-mesh reproduction of ADR 0211 amendment 1 (ADR 0214 set-up FX)
// =================================================================================================================

/// FNV-1a over 64-bit words (ADR 0214 N2: the record of a default-path state).
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

fn state_hash(flow: &PimpleSolver) -> u64 {
    fnv(flow
        .u
        .values
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(flow.p.values.iter().map(|x| x.to_bits()))
        .chain(flow.phi.iter().map(|x| x.to_bits())))
}

/// The FB mesh with its geometry and the gap bookkeeping of the shape descriptors.
struct Fx {
    geom: Arc<FvGeometry>,
    forms: FbForms,
    /// For each gap (left, right): the rows along the block from the bottom, each the list of its cells.
    gap_rows: [Vec<Vec<usize>>; 2],
    gap_cell: Vec<bool>,
}

impl Fx {
    fn new() -> Self {
        let (mesh, counts) = falling_block_mesh();
        assert_eq!(mesh.n_cells(), 12_320, "copy of the FB mesh: cell count of ADR 0211");
        assert_eq!(counts, [56, 343, 24, 287], "copy of the FB mesh: grid of ADR 0211");
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let cc = geom.mesh.cell_centres().to_vec();
        let mut rows: [std::collections::BTreeMap<u64, Vec<usize>>; 2] = Default::default();
        let mut gap_cell = vec![false; cc.len()];
        for (c, x) in cc.iter().enumerate() {
            if x[1] <= YB || x[1] >= YB + HB {
                continue;
            }
            let g = if x[0] < H {
                0
            } else if x[0] > H + WB {
                1
            } else {
                continue;
            };
            gap_cell[c] = true;
            // rows of the structured grid share their centre height; key by the rounded height
            rows[g].entry((x[1] * 1e9).round() as u64).or_default().push(c);
        }
        let gap_rows = rows.map(|m| m.into_values().collect::<Vec<_>>());
        for g in &gap_rows {
            assert_eq!(g.len(), 287, "rows along the block");
            assert!(g.iter().all(|r| r.len() == N_GAP), "cells across a gap");
        }
        Fx {
            geom,
            forms: fb_forms(),
            gap_rows,
            gap_cell,
        }
    }

    fn walls(&self) -> Vec<String> {
        self.geom
            .mesh
            .patches()
            .iter()
            .filter(|p| p.kind != PatchKind::Empty)
            .map(|p| p.name.clone())
            .collect()
    }

    fn solver(&self, settings: PimpleSettings) -> PimpleSolver {
        let mut u = VectorField::uniform("U", &self.geom, vec3::ZERO);
        for name in self.walls() {
            u.set_bc(&self.geom, &name, Box::new(FixedValue::vector(vec3::ZERO)))
                .unwrap();
        }
        let p = ScalarField::uniform("p", &self.geom, 0.0);
        PimpleSolver::new(self.geom.clone(), u, p, settings)
    }

    /// `U(t) = U_t (1 - e^{-t/tau})` (ADR 0211 M3).
    fn speed(&self, t: f64) -> f64 {
        self.forms.u_t * (1.0 - (-t / self.forms.tau).exp())
    }

    /// The block's fixed velocity of the step that ends at `t`.
    fn set_block(&self, flow: &mut PimpleSolver, t: f64) {
        let g = flow.geom.clone();
        flow.u
            .set_bc(&g, "block", Box::new(FixedValue::vector([0.0, -self.speed(t), 0.0])))
            .unwrap();
    }

    /// One FX step (step number `n`, `dt`); returns whether the state is bounded (ADR 0214 FX).
    fn step(&self, flow: &mut PimpleSolver, n: usize, dt: f64) -> (bool, f64, f64) {
        self.set_block(flow, n as f64 * dt);
        let info = flow.step(dt);
        let umax = flow.u.values.iter().fold(0.0f64, |m, v| m.max(vec3::norm(*v)));
        let finite = flow.u.values.iter().flatten().all(|x| x.is_finite());
        let bounded = flow.diverged.is_none() && finite && umax <= 10.0 * self.forms.u_t;
        (bounded, umax, info.continuity)
    }

    /// The `y` force on the block.
    fn force_y(&self, flow: &PimpleSolver) -> f64 {
        let patches = vec!["block".to_string()];
        let (f, _) = patch_force_moment(&flow.geom, &flow.u, &flow.p, RHO, NU, &patches, [0.0; 3]).unwrap();
        f[1]
    }
}

/// Shape descriptors of ADR 0214 of a cell field: `(E, X, c_1, c_2)`.
fn shape(fx: &Fx, q: &[f64]) -> (f64, f64, f64, f64) {
    let vol = fx.geom.mesh.cell_volumes();
    let (mut all, mut gap) = (0.0, 0.0);
    for (c, v) in vol.iter().enumerate() {
        all += v * q[c] * q[c];
        if fx.gap_cell[c] {
            gap += v * q[c] * q[c];
        }
    }
    let (mut spread, mut tot) = (0.0, 0.0);
    let (mut s0, mut s1, mut s2) = (0.0, 0.0, 0.0);
    for rows in &fx.gap_rows {
        let m: Vec<f64> = rows
            .iter()
            .map(|r| r.iter().map(|&c| q[c]).sum::<f64>() / r.len() as f64)
            .collect();
        for (r, mj) in rows.iter().zip(&m) {
            for &c in r {
                spread += (q[c] - mj).powi(2);
                tot += q[c] * q[c];
            }
        }
        for j in 0..m.len() {
            s0 += m[j] * m[j];
            if j + 1 < m.len() {
                s1 += m[j] * m[j + 1];
            }
            if j + 2 < m.len() {
                s2 += m[j] * m[j + 2];
            }
        }
    }
    (gap / all, spread / tot, s1 / s0, s2 / s0)
}

fn shape_holds(d: (f64, f64, f64, f64)) -> bool {
    d.0 >= 0.9 && d.1 <= 0.05 && d.2.abs() <= 0.3 && d.3 <= -0.7
}

/// Result of a twin-run power iteration (ADR 0214 TW).
struct Tw {
    rho: f64,
    sign: f64,
    mode_p: Vec<f64>,
    base_p: Vec<f64>,
    hash: u64,
}

/// Twin-run power iteration of the step map of FX at `dt = tau/spt` with `settings` (ADR 0214 TW).
fn twin(fx: &Fx, settings: PimpleSettings, spt: usize) -> Tw {
    twin_with(fx, settings, spt, None)
}

/// [`twin`] whose iterations (and the base step from `S`) use `tw_correctors` correctors when given, the base run the
/// correctors of `settings` (ADR 0214 amendment 1, D6b and DN1b).
fn twin_with(fx: &Fx, settings: PimpleSettings, spt: usize, tw_correctors: Option<usize>) -> Tw {
    const K: usize = 60;
    let dt = fx.forms.tau / spt as f64;
    let nb = (spt / 2).max(1);
    let mut flow = fx.solver(settings);
    for n in 1..=nb {
        let (ok, _, _) = fx.step(&mut flow, n, dt);
        assert!(ok, "TW base run not bounded at step {n}");
    }
    let s: PimpleState = flow.snapshot();
    if let Some(nc) = tw_correctors {
        flow.settings.n_correctors = nc;
    }
    fx.set_block(&mut flow, (nb + 1) as f64 * dt);
    flow.step(dt);
    assert!(flow.diverged.is_none(), "TW base step");
    let (bu, bp, bphi) = (flow.u.values.clone(), flow.p.values.clone(), flow.phi.clone());
    let vol = fx.geom.mesh.cell_volumes().to_vec();
    let n = vol.len();
    let ut2 = fx.forms.u_t * fx.forms.u_t;
    let pmax = s.p.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    assert!(pmax > 0.0, "TW base pressure is zero");
    let p2 = pmax * pmax;
    let norm = |du: &[Vec3], dp: &[f64]| -> f64 {
        (0..n)
            .map(|c| vol[c] * ((du[c][0].powi(2) + du[c][1].powi(2)) / ut2 + dp[c] * dp[c] / p2))
            .sum::<f64>()
            .sqrt()
    };
    // deterministic start: the xorshift of `lag_amplification_n` (seed 0x2545F4914F6CDD1D), values in [-0.5, 0.5)
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    // in units of the norm: the velocity in units of U_t, the pressure in units of P
    let mut du: Vec<Vec3> = Vec::with_capacity(n);
    let mut dp: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        dp.push(rnd() * pmax);
        du.push([rnd() * fx.forms.u_t, rnd() * fx.forms.u_t, 0.0]);
    }
    let mut dphi = vec![0.0; flow.phi.len()];
    let d0 = norm(&du, &dp);
    du.iter_mut().for_each(|v| *v = vec3::scale(*v, 1.0 / d0));
    dp.iter_mut().for_each(|x| *x /= d0);
    // the perturbation has 1e-7 of the norm of the base state
    let eps = 1e-7 * norm(&s.u, &s.p);
    let (mut logs, mut signs) = (Vec::new(), Vec::new());
    let mut words = Vec::new();
    for _ in 0..K {
        let mut st = s.clone();
        for c in 0..n {
            for k in 0..2 {
                st.u[c][k] += eps * du[c][k];
                st.hist_u[k].old[c] = st.u[c][k];
            }
            st.p[c] += eps * dp[c];
        }
        for (f, d) in dphi.iter().enumerate() {
            st.phi[f] += eps * d;
        }
        st.phi_hist.old = st.phi.clone();
        flow.restore(&st).unwrap();
        flow.step(dt);
        assert!(flow.diverged.is_none(), "TW perturbed step");
        let tu: Vec<Vec3> = (0..n)
            .map(|c| vec3::scale(vec3::sub(flow.u.values[c], bu[c]), 1.0 / eps))
            .collect();
        let tp: Vec<f64> = (0..n).map(|c| (flow.p.values[c] - bp[c]) / eps).collect();
        let tphi: Vec<f64> = flow.phi.iter().zip(&bphi).map(|(a, b)| (a - b) / eps).collect();
        let nt = norm(&tu, &tp);
        let ip: f64 = (0..n)
            .map(|c| vol[c] * ((tu[c][0] * du[c][0] + tu[c][1] * du[c][1]) / ut2 + tp[c] * dp[c] / p2))
            .sum();
        let nd = norm(&du, &dp);
        logs.push((nt / nd).ln());
        signs.push(ip.signum());
        words.push((nt / nd).to_bits());
        du = tu.iter().map(|v| vec3::scale(*v, 1.0 / nt)).collect();
        dp = tp.iter().map(|x| x / nt).collect();
        dphi = tphi.iter().map(|x| x / nt).collect();
    }
    let tail = &logs[K / 2..];
    let rho = (tail.iter().sum::<f64>() / tail.len() as f64).exp();
    let sign = signs[K / 2..].iter().sum::<f64>() / (K - K / 2) as f64;
    words.extend(dp.iter().map(|x| x.to_bits()));
    Tw {
        rho,
        sign,
        mode_p: dp,
        base_p: s.p.clone(),
        hash: fnv(words),
    }
}

/// Height of the cell with the largest entry of a mode.
fn peak_y(fx: &Fx, q: &[f64]) -> f64 {
    let c = (0..q.len()).max_by(|&a, &b| q[a].abs().total_cmp(&q[b].abs())).unwrap();
    fx.geom.mesh.cell_centres()[c][1]
}

fn report(fx: &Fx, id: &str, tw: &Tw, model: Option<f64>) {
    let d = shape(fx, &tw.mode_p);
    let rel = model.map_or(String::new(), |m| {
        format!(", model {m}, |rho| - m relative {:+.4}", (tw.rho - m) / m)
    });
    println!(
        "{id}: |rho| {:.5} sign {:+.3}{rel}; mode E {:.4} X {:.3e} c1 {:+.4} c2 {:+.4}, peak at y {:.4}",
        tw.rho,
        tw.sign,
        d.0,
        d.1,
        d.2,
        d.3,
        peak_y(fx, &tw.mode_p)
    );
    println!("N2 hash {id} {:016x}", tw.hash);
}

fn within(rho: f64, m: f64) -> bool {
    (rho - m).abs() / m <= 0.10
}

fn with_correctors(n: usize) -> PimpleSettings {
    let mut s = fb_settings();
    s.n_correctors = n;
    s
}

// ----- D, the diagnosis -------------------------------------------------------------------------------------------------

/// ADR 0214 D0: FX at `tau/10` with the default is not bounded within 100 steps (postdiction of ADR 0211 amendment 1).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d0_fixed_mesh_reproduction_is_not_bounded_at_tau_10() {
    let fx = Fx::new();
    let dt = fx.forms.tau / 10.0;
    let mut flow = fx.solver(fb_settings());
    let mut first = None;
    let mut umax_at = Vec::new();
    for n in 1..=100 {
        let (ok, umax, _) = fx.step(&mut flow, n, dt);
        umax_at.push(umax);
        if !ok {
            first = Some(n);
            break;
        }
    }
    let stop = flow
        .diverged
        .as_ref()
        .map(|d| d.message(Some(fx.geom.mesh.cell_centres())));
    println!(
        "D0: first step not bounded {first:?} (max|u|/U_t by step: {:?}); guard: {stop:?}",
        umax_at
            .iter()
            .map(|u| (u / fx.forms.u_t * 1e3).round() / 1e3)
            .collect::<Vec<_>>()
    );
    println!("N2 hash D0 {:016x}", state_hash(&flow));
    assert!(first.is_some(), "D0: FX at tau/10 stayed bounded for 100 steps");
}

/// ADR 0214 D0b: FX at `tau/40` with the default is bounded for 400 steps (postdiction of ADR 0211 amendment 1).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d0b_fixed_mesh_reproduction_is_bounded_at_tau_40() {
    let fx = Fx::new();
    let dt = fx.forms.tau / 40.0;
    let mut flow = fx.solver(fb_settings());
    let mut bad = None;
    let mut cont: f64 = 0.0;
    for n in 1..=400 {
        let (ok, _, c) = fx.step(&mut flow, n, dt);
        cont = cont.max(c);
        if !ok && bad.is_none() {
            bad = Some(n);
            break;
        }
    }
    println!(
        "D0b: first step not bounded {bad:?}; continuity {cont:.2e}; y force on the block at 10 tau {:.10e}",
        fx.force_y(&flow)
    );
    println!("N2 hash D0b {:016x}", state_hash(&flow));
    assert!(bad.is_none(), "D0b: FX at tau/40 not bounded at step {bad:?}");
}

/// ADR 0214 D1, D7 (on the D1 mode) and DN2: TW at `tau/10`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d1_d7_dn2_twin_run_at_tau_10() {
    let fx = Fx::new();
    let tw = twin(&fx, fb_settings(), 10);
    report(&fx, "D1", &tw, Some(3.165));
    let d7 = shape(&fx, &tw.mode_p);
    let dn2 = shape(&fx, &tw.base_p);
    println!(
        "DN2: shape of the base pressure: E {:.4} X {:.3e} c1 {:+.4} c2 {:+.4} (must fail: c2 > -0.7)",
        dn2.0, dn2.1, dn2.2, dn2.3
    );
    assert!(within(tw.rho, 3.165) && tw.sign <= -0.5, "D1 {} {}", tw.rho, tw.sign);
    assert!(shape_holds(d7), "D7 (D1 mode) {d7:?}");
    assert!(dn2.3 > -0.7, "DN2 {dn2:?}");
}

/// ADR 0214 D2 and D7 (on the D2 mode): TW at `tau/20`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d2_d7_twin_run_at_tau_20() {
    let fx = Fx::new();
    let tw = twin(&fx, fb_settings(), 20);
    report(&fx, "D2", &tw, Some(1.737));
    let d7 = shape(&fx, &tw.mode_p);
    assert!(within(tw.rho, 1.737) && tw.sign <= -0.5, "D2 {} {}", tw.rho, tw.sign);
    assert!(shape_holds(d7), "D7 (D2 mode) {d7:?}");
}

/// ADR 0214 D3: TW at `tau/5`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d3_twin_run_at_tau_5() {
    let fx = Fx::new();
    let tw = twin(&fx, fb_settings(), 5);
    report(&fx, "D3", &tw, Some(4.992));
    assert!(within(tw.rho, 4.992) && tw.sign <= -0.5, "D3 {} {}", tw.rho, tw.sign);
}

/// ADR 0214 D4: TW at `tau/10` with three correctors; four correctors printed.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d4_twin_run_with_three_and_four_correctors() {
    let fx = Fx::new();
    let tw3 = twin(&fx, with_correctors(3), 10);
    report(&fx, "D4", &tw3, Some(1.660));
    let tw4 = twin(&fx, with_correctors(4), 10);
    report(&fx, "D4 printed (4 correctors)", &tw4, Some(1.035));
    assert!(
        within(tw3.rho, 1.660) && tw3.sign <= -0.5,
        "D4 {} {}",
        tw3.rho,
        tw3.sign
    );
}

/// ADR 0214 D5: TW at `tau/40` is below one; `tau/30` printed.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d5_twin_run_at_tau_40() {
    let fx = Fx::new();
    let tw = twin(&fx, fb_settings(), 40);
    report(&fx, "D5", &tw, Some(0.980));
    let tw30 = twin(&fx, fb_settings(), 30);
    report(&fx, "D5 printed (tau/30)", &tw30, Some(1.144));
    assert!(tw.rho < 1.0, "D5 {}", tw.rho);
}

/// ADR 0214 D6 and DN1 as registered: TW at `tau/40` with one corrector (R5's one-corrector instability). Defective set-up
/// (ADR 0214 amendment 1): the one-corrector base run is not bounded at step 4, so the base state does not exist. Kept as the
/// record; corrected by `d6b_dn1b_one_corrector_from_the_d5_base_state`.
#[test]
#[ignore = "defective set-up: the one-corrector base run is not bounded at step 4 (ADR 0214 amendment 1)"]
fn d6_dn1_twin_run_with_one_corrector_at_tau_40_defective_setup() {
    let fx = Fx::new();
    let tw = twin(&fx, with_correctors(1), 40);
    report(&fx, "D6", &tw, Some(7.315));
    assert!(tw.rho >= 1.0 && tw.sign <= -0.5, "DN1 {} {}", tw.rho, tw.sign);
    assert!(within(tw.rho, 7.315), "D6 {}", tw.rho);
}

/// ADR 0214 amendment 1, D6b and DN1b: the base state of D5 (two correctors, `tau/40`), then the TW iterations with one
/// corrector.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d6b_dn1b_one_corrector_from_the_d5_base_state() {
    let fx = Fx::new();
    let tw = twin_with(&fx, fb_settings(), 40, Some(1));
    report(&fx, "D6b", &tw, Some(7.315));
    assert!(tw.rho >= 1.0 && tw.sign <= -0.5, "DN1b {} {}", tw.rho, tw.sign);
    assert!(within(tw.rho, 7.315), "D6b {}", tw.rho);
}

/// ADR 0214 (printed, no limit): TW at `tau/10` without the transient Rhie-Chow term.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d_printed_twin_run_without_the_rhie_chow_time_term() {
    let fx = Fx::new();
    let mut s = fb_settings();
    s.ddt_correction = false;
    let tw = twin(&fx, s, 10);
    report(&fx, "D printed (no Rhie-Chow time term)", &tw, Some(3.153));
}

// ----- L, the located limit of the default ------------------------------------------------------------------------------

/// ADR 0214 L1: the ADR 0073 B4 check in Raise mode stops FX at `tau/10` at step 1, located in a gap.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn l1_lag_check_refuses_the_default_at_step_one() {
    let fx = Fx::new();
    let dt = fx.forms.tau / 10.0;
    let mut s = fb_settings();
    s.pressure_lag_check = Some(LagCheck {
        every: 0,
        start: 1,
        iterations: 20,
        action: LagAction::Raise,
    });
    let mut flow = fx.solver(s);
    fx.step(&mut flow, 1, dt);
    let d = flow.diverged.clone().expect("L1: the lag check did not stop the run");
    let cc = fx.geom.mesh.cell_centres();
    let x = d.cell.map(|c| cc[c]).unwrap_or([f64::NAN; 3]);
    let in_gap = (x[0] < H || x[0] > H + WB) && x[1] > YB && x[1] < YB + HB;
    println!(
        "L1: step {}, quantity {}, value {:.5}, cell {:?} at ({:.5}, {:.5}); in a gap {in_gap}; message: {}",
        d.step,
        d.quantity,
        d.value,
        d.cell,
        x[0],
        x[1],
        d.message(Some(cc))
    );
    println!(
        "N2 hash L1 {:016x}",
        fnv([d.value.to_bits(), d.cell.unwrap_or(usize::MAX) as u64])
    );
    assert!(d.step == 1 && d.quantity == "lag amplification", "L1 stop {d:?}");
    assert!(d.value >= 1.3, "L1 value {}", d.value);
    assert!(in_gap, "L1 cell not in a gap: {x:?}");
}

// =================================================================================================================
// The coupled FB loop and the A checks of ADR 0211 (copied from sixdof_flow_f244.rs; only the settings are a parameter)
// =================================================================================================================

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

/// The FB mesh with its smoothing data: the unit displacement field of the block (solved once, ADR 0211 FB).
struct Fb {
    mesh0: Mesh,
    g0: Arc<FvGeometry>,
    classes: Vec<PatchClass>,
    block_points: Vec<usize>,
    d_unit: Vec<Vec3>,
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
        let d_unit = Self::smooth(&g0, &classes, &block_points, mesh0.n_points(), 1.0);
        Fb {
            mesh0,
            g0,
            classes,
            block_points,
            d_unit,
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

fn fb_run(fb: &Fb, steps_per_tau: usize, settings: &PimpleSettings) -> FbRun {
    let forms = fb_forms();
    let dt = forms.tau / steps_per_tau as f64;
    let geom = fb.g0.clone();
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in fb.walls() {
        u.set_bc(&geom, &name, Box::new(FixedValue::vector(vec3::ZERO)))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, settings.clone());
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

/// ADR 0211 A1 to A5, A7 to A9 and AN1 on the runs `dt = tau/spt` and `tau/(2 spt)` with `settings`.
fn falling_block_terminal_velocity(spt: usize, settings: &PimpleSettings) {
    let fb = Fb::new();
    let forms = fb_forms();
    let r1 = fb_run(&fb, spt, settings);
    let r2 = fb_run(&fb, 2 * spt, settings);
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
        "A: {} cells; tau/{spt}: U(10 tau)/U_t {a1:.5} (limit |. - 1| <= 0.02), continuity {:.2e}, GCL {:.2e} mean(V), \
         volume {:.2e} V0, min V/V0 {:.4}, travel {:.4} h, pressure iterations {}",
        fb.mesh0.n_cells(),
        r1.cont,
        r1.gcl,
        r1.vol,
        r1.min_ratio,
        r1.travel / H,
        r1.p_iters
    );
    println!(
        "A: tau/{}: U(10 tau)/U_t {:.5}, continuity {:.2e}, GCL {:.2e} mean(V), volume {:.2e} V0, min V/V0 {:.4}, \
         pressure iterations {}",
        2 * spt,
        u2 / forms.u_t,
        r2.cont,
        r2.gcl,
        r2.vol,
        r2.min_ratio,
        r2.p_iters
    );
    println!(
        "A2 dt halving {a2:.2e} (limit 5e-3); A7 |U(10 tau) - U(9 tau)|/U_t {a7:.2e} (limit 1e-3); A8 {a8:.2e} (limit 1e-9); \
         AN1 against U_t' {an1:.4} (limit >= 0.08)"
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

// ----- F, S, A1c, A4u-c, R1, N1: the opt-in consistent correctors -------------------------------------------------------

fn consistent() -> PimpleSettings {
    let mut s = fb_settings();
    s.consistent_correctors = true;
    s
}

/// ADR 0214 F1: TW with the consistent correctors at `tau/5`, `tau/10`, `tau/20` is below one.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f1_twin_run_with_the_consistent_correctors() {
    let fx = Fx::new();
    let mut rhos = Vec::new();
    for (spt, m) in [(5, 0.977), (10, 0.957), (20, 0.972)] {
        let tw = twin(&fx, consistent(), spt);
        report(&fx, &format!("F1 tau/{spt}"), &tw, Some(m));
        rhos.push(tw.rho);
    }
    assert!(rhos.iter().all(|r| *r < 1.0), "F1 {rhos:?}");
}

/// ADR 0214 F2: FX at `tau/10` with the consistent correctors is bounded for 100 steps, its continuity at most 1e-10, and its
/// block force at `10 tau` equals that of the default at `tau/40` (the D0b set-up, computed again here) within 1e-3.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f2_fixed_mesh_reproduction_with_the_consistent_correctors_at_tau_10() {
    let fx = Fx::new();
    // the reference: the D0b set-up (default, tau/40, 400 steps)
    let dt40 = fx.forms.tau / 40.0;
    let mut reference = fx.solver(fb_settings());
    for n in 1..=400 {
        let (ok, _, _) = fx.step(&mut reference, n, dt40);
        assert!(ok, "F2 reference (the D0b set-up) not bounded at step {n}");
    }
    let f_ref = fx.force_y(&reference);
    println!("N2 hash F2 reference (the D0b set-up) {:016x}", state_hash(&reference));
    let dt = fx.forms.tau / 10.0;
    let mut flow = fx.solver(consistent());
    let (mut bad, mut cont, mut umax): (Option<usize>, f64, f64) = (None, 0.0, 0.0);
    let mut p_iters = 0;
    for n in 1..=100 {
        let (ok, um, c) = fx.step(&mut flow, n, dt);
        cont = cont.max(c);
        umax = umax.max(um);
        p_iters += flow.history.last().map_or(0, |i| i.p_iters);
        if !ok {
            bad = Some(n);
            break;
        }
    }
    let f = fx.force_y(&flow);
    let rel = (f - f_ref).abs() / f_ref.abs();
    println!(
        "F2: first step not bounded {bad:?}; max|u|/U_t {:.4}; continuity {cont:.2e} (limit 1e-10); pressure iterations \
         {p_iters}; y force at 10 tau {f:.10e} against the default at tau/40 {f_ref:.10e}: relative {rel:.2e} (limit 1e-3)",
        umax / fx.forms.u_t
    );
    assert!(bad.is_none(), "F2 not bounded at step {bad:?}");
    assert!(cont <= 1e-10, "F2 continuity {cont}");
    assert!(rel <= 1e-3, "F2 force {rel}");
}

/// ADR 0214 F3: the ADR 0073 B4 estimate on FX at `tau/10` with the consistent correctors is below one.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f3_lag_estimate_with_the_consistent_correctors() {
    let fx = Fx::new();
    let dt = fx.forms.tau / 10.0;
    let mut s = consistent();
    s.pressure_lag_check = Some(LagCheck {
        every: 0,
        start: 1,
        iterations: 20,
        action: LagAction::Warn,
    });
    let mut flow = fx.solver(s);
    fx.step(&mut flow, 1, dt);
    let w = flow.lag_estimates.first().copied().expect("F3: no estimate at step 1");
    let x = fx.geom.mesh.cell_centres()[w.cell];
    println!(
        "F3: step {}, rho {:.5} (limit < 1; the default's is L1's), cell {} at ({:.5}, {:.5})",
        w.step, w.rho, w.cell, x[0], x[1]
    );
    assert!(w.rho < 1.0, "F3 {}", w.rho);
}

/// ADR 0214 S0 and S1: the default and the consistent correctors share their steady state (graded channel SC).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s0_s1_steady_state_of_the_consistent_correctors_is_the_default_one() {
    const U0: f64 = 0.1;
    let mesh = box_2d_graded(
        20,
        12,
        0.3,
        0.1,
        [Grading::Uniform, Grading::Symmetric(3.0)],
        false,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([U0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    for name in ["ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut flow = PimpleSolver::new(geom, u, p, fb_settings());
    let dt = 0.002;
    let mut last = 0.0f64;
    for _ in 0..3000 {
        let before = flow.u.values.clone();
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S0 run stopped");
        last = flow
            .u
            .values
            .iter()
            .zip(&before)
            .fold(0.0, |m, (a, b)| m.max(vec3::norm(vec3::sub(*a, *b))));
    }
    let (u_s0, p_s0) = (flow.u.values.clone(), flow.p.values.clone());
    let hi = p_s0.iter().fold(f64::NEG_INFINITY, |m, x| m.max(*x));
    let lo = p_s0.iter().fold(f64::INFINITY, |m, x| m.min(*x));
    let range = hi - lo;
    flow.settings.consistent_correctors = true;
    let (mut du, mut dp): (f64, f64) = (0.0, 0.0);
    for _ in 0..50 {
        flow.step(dt);
        assert!(flow.diverged.is_none(), "S1 run stopped: {:?}", flow.diverged);
        for (a, b) in flow.u.values.iter().zip(&u_s0) {
            du = du.max(vec3::norm(vec3::sub(*a, *b)));
        }
        for (a, b) in flow.p.values.iter().zip(&p_s0) {
            dp = dp.max((a - b).abs());
        }
    }
    println!(
        "S0: last step change {:.2e} U0 (limit 1e-12); S1: max|u - u_S0| {:.2e} U0 (limit 1e-10), max|p - p_S0| {:.2e} of the \
         range {range:.4e} (limit 1e-10)",
        last / U0,
        du / U0,
        dp / range
    );
    assert!(last <= 1e-12 * U0, "S0 {last}");
    assert!(du <= 1e-10 * U0, "S1 velocity {du}");
    assert!(dp <= 1e-10 * range, "S1 pressure {dp}");
}

/// ADR 0214 A1c: the registered `tau/10` set-up of F-244 (ADR 0211 A1 to A5, A7 to A9, AN1 with `tau/10` and `tau/20`) with the
/// consistent correctors. The default run is ADR 0211's ignored known miss, unchanged.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1c_falling_block_reaches_the_lubrication_terminal_velocity_with_consistent_correctors() {
    println!("A1c: the checks of ADR 0211 at tau/10 and tau/20 with PimpleSettings::consistent_correctors");
    falling_block_terminal_velocity(10, &consistent());
}

/// ADR 0214 A4u-c: the A4u replay of ADR 0211 (a uniform flow on the FB mesh moved by the closed-form travel) with the
/// consistent correctors.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a4u_c_falling_block_mesh_keeps_a_uniform_flow_with_consistent_correctors() {
    let fb = Fb::new();
    let forms = fb_forms();
    let u0 = [0.3, -0.2, 0.0];
    let geom = fb.g0.clone();
    let mut u = VectorField::uniform("U", &geom, u0);
    for name in fb.walls() {
        u.set_bc(&geom, &name, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom, u, p, consistent());
    let dt = forms.tau / 10.0;
    let (mut du, mut dp): (f64, f64) = (0.0, 0.0);
    for n in 1..=100 {
        let t = n as f64 * dt;
        let s = -forms.u_t * (t - forms.tau * (1.0 - (-t / forms.tau).exp()));
        let st = move_mesh(&flow.geom, fb.points(s), dt).unwrap();
        flow.step_ale(dt, st.geom, st.mesh_flux).unwrap();
        assert!(flow.diverged.is_none(), "A4u-c stopped: {:?}", flow.diverged);
        for v in &flow.u.values {
            du = du.max(vec3::norm(vec3::sub(*v, u0)));
        }
        for x in &flow.p.values {
            dp = dp.max(x.abs());
        }
    }
    let un = vec3::norm(u0);
    println!(
        "A4u-c: max|u - u0| {:.2e} |u0| (limit 1e-12), max|p| {:.2e} |u0|^2 (limit 1e-12)",
        du / un,
        dp / (un * un)
    );
    assert!(du <= 1e-12 * un, "A4u-c velocity {du}");
    assert!(dp <= 1e-12 * un * un, "A4u-c pressure {dp}");
}

/// A 2-D box solver for the refusals of R1.
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

/// ADR 0214 R1: every refusal of the consistent correctors stops the run at step 1 with the state as it was.
#[test]
fn r1_refusals_of_the_consistent_correctors() {
    let on = || {
        let mut s = PimpleSettings::piso(0.01);
        s.consistent_correctors = true;
        s
    };
    let mut cases: Vec<(&str, PimpleSolver, bool)> = Vec::new();
    let mut s = on();
    s.fractional = Some(FractionalSettings::default());
    cases.push(("fractional step", box_solver(false, s), false));
    let mut s = on();
    s.non_orth_correctors = 1;
    cases.push(("non-orthogonal correctors", box_solver(false, s), false));
    let mut s = on();
    s.component_diagonal = true;
    cases.push(("component diagonal", box_solver(false, s), false));
    let mut f = box_solver(false, on());
    f.body_force = Some(BodyForce::uniform(&f.geom.clone(), [0.0, -9.81, 0.0]));
    cases.push(("body force", f, false));
    let mut f = box_solver(false, on());
    f.face_jump = Some(FaceJump::zeros(f.geom.mesh.n_faces()));
    cases.push(("face jump", f, false));
    cases.push(("periodic faces", box_solver(true, on()), false));
    // a body force supplied by the hook of the outer iteration
    cases.push(("body force", box_solver(false, on()), true));
    for (stage, mut flow, hooked) in cases {
        let before = (flow.u.values.clone(), flow.p.values.clone(), flow.phi.clone());
        if hooked {
            let geom = flow.geom.clone();
            let mut hook = move |_u: &VectorField, _phi: &[f64]| rcfd_fv::pimple::OuterUpdate {
                source_force: Some(BodyForce::uniform(&geom, [0.0, -9.81, 0.0])),
                ..Default::default()
            };
            flow.step_with(0.01, Some(&mut hook));
        } else {
            flow.step(0.01);
        }
        let d = flow
            .diverged
            .clone()
            .unwrap_or_else(|| panic!("R1 {stage}: not refused"));
        let msg = d.message(None);
        println!("R1 {stage}{}: {msg}", if hooked { " (hook)" } else { "" });
        assert_eq!(d.step, 1, "R1 {stage} step");
        assert_eq!(d.stage, stage, "R1 stage");
        assert_eq!(d.quantity, "consistent correctors", "R1 {stage} quantity");
        assert!(msg.contains("ADR 0214"), "R1 {stage} message {msg}");
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        let ubits = |v: &[Vec3]| v.iter().flat_map(|x| x.iter().map(|y| y.to_bits())).collect::<Vec<_>>();
        assert_eq!(ubits(&flow.u.values), ubits(&before.0), "R1 {stage}: velocity changed");
        assert_eq!(bits(&flow.p.values), bits(&before.1), "R1 {stage}: pressure changed");
        assert_eq!(bits(&flow.phi), bits(&before.2), "R1 {stage}: flux changed");
    }
}

/// ADR 0214 N1: the consistent correctors are off by default.
#[test]
fn n1_the_consistent_correctors_are_off_by_default() {
    assert!(!PimpleSettings::piso(0.01).consistent_correctors);
    assert!(!PimpleSettings::pimple(0.01, 3).consistent_correctors);
}
