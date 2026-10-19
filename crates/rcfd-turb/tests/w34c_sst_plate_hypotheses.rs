//! Wave 34C (ADR 0074 part B): hypotheses for the low observed grid-convergence order of the SST flat plate (ADR 0070 P2f: 0.662).
//!
//! Every run is the runner of `validation_w33c_sst_plate.rs` (U = 1, nu = 2e-7, plate x in [0, 2], height 1, `Symmetry` top, inlet k = 2.25e-7,
//! omega = 125, SIMPLEC, linear-upwind momentum, `Automatic`, du < 1e-9 and nu_t change < 1e-6) with ONE factor changed:
//! H1 corrected component diagonal, H2 linear-upwind k and omega, H3a wall omega with the wall-cell height instead of the centre distance,
//! H3b wall-cell omega fixed in every wall cell, H4 a symmetry run-up upstream of the leading edge, H6 the NASA TMR nested grids
//! (`validation-data/reference/w34c_tmr_flatplate_545x385_axes.csv`). Stage 1 runs the coarse and medium meshes; stage 2 the finest, only for
//! hypotheses that survive stage 1 (ADR 0074 decision rules). H5 (mesh family) is a computation without a run.
//!
//! Heavy: `cargo test --release -p rcfd-turb --test w34c_sst_plate_hypotheses -- --ignored --nocapture <name>`.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_turb::sst::wall_omega;
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

const NU: f64 = 2.0e-7;
const X_REF: f64 = 0.970084;
const TMR_UPLUS: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_uplus_yplus_x0p97008.csv");
const TMR_AXES: &str = include_str!("../../../validation-data/reference/w34c_tmr_flatplate_545x385_axes.csv");
/// Mean of the three incompressible TMR codes (FUN3D, SC/Tetra, GHOST) at x = 0.97008 on the 545 x 385 grid (ADR 0070).
const TMR_INC_MEAN: f64 = 2.71647e-3;
/// ADR 0070 P2f baseline Cf(x_ref) of the three meshes (deterministic, reproduced by ADR 0063 and 0070).
const BASE: [f64; 3] = [2.57698e-3, 2.65138e-3, 2.69840e-3];

#[derive(Clone, Copy, Debug)]
struct PlateMesh {
    nx: usize,
    ny: usize,
    layers: usize,
    growth: f64,
    y_plus: f64,
}

const MESHES: [PlateMesh; 3] = [
    PlateMesh {
        nx: 70,
        ny: 55,
        layers: 35,
        growth: 1.21,
        y_plus: 1.0,
    },
    PlateMesh {
        nx: 140,
        ny: 110,
        layers: 70,
        growth: 1.10,
        y_plus: 0.5,
    },
    PlateMesh {
        nx: 280,
        ny: 220,
        layers: 140,
        growth: 1.0488,
        y_plus: 0.25,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Change {
    /// H1: `component_diag_correction` on.
    CorrectedDiagonal,
    /// H2: linear-upwind convection of k and omega.
    LinearUpwindTurb,
    /// H3a: wall-face omega `60 nu/(beta1 (2 dn)^2)`.
    WallOmegaCellHeight,
    /// H3b: wall-cell omega fixed in every wall cell (`omega_fix_y_plus = 0`).
    WallCellOmegaFixed,
    /// H4 / H6: no solver change (the change is in the mesh).
    None,
}

/// Node coordinates of the plate mesh of the ADR 0070 sequence.
fn plate_axes(m: PlateMesh) -> (Vec<f64>, Vec<f64>) {
    let ut_end = flat_plate_u_tau(1.0, NU, 2.0);
    let first = first_cell_height_from_y_plus(m.y_plus, ut_end, NU);
    let g_y = Grading::WallLayers {
        first,
        growth: m.growth,
        layers: m.layers,
        both: false,
    };
    (
        axis_coords(m.nx, 2.0, Grading::Expansion(8.0)).unwrap(),
        axis_coords(m.ny, 1.0, g_y).unwrap(),
    )
}

/// 2-D mesh from node coordinates; the bottom faces with x-centre < 0 form the patch `run_up`, the others `ymin`.
fn mesh_from_axes(xs: &[f64], ys: &[f64]) -> Mesh {
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut points: Vec<Vec3> = Vec::new();
    for z in [0.0, 2.0 / nx as f64] {
        for &y in ys {
            for &x in xs {
                points.push([x, y, z]);
            }
        }
    }
    let names = ["xmin", "xmax", "ymin", "ymax", "frontAndBack", "run_up"];
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
                    let slot = if d == 2 && 0.5 * (xs[i] + xs[i + 1]) < 0.0 {
                        5
                    } else {
                        d.min(4)
                    };
                    specs[slot].faces.push(f.clone());
                }
            }
            cells.push(faces);
        }
    }
    specs.retain(|s| !s.faces.is_empty());
    Mesh::from_cell_faces(points, &cells, &specs).unwrap()
}

/// H4 run-up: `n_up` cells over [-1/3, 0], geometric, the cell next to x = 0 equal to the first plate cell.
fn with_run_up(xs: &[f64], n_up: usize) -> Vec<f64> {
    let (len, d0) = (1.0 / 3.0, xs[1] - xs[0]);
    let total = |r: f64| (0..n_up).map(|i| d0 * r.powi(i as i32)).sum::<f64>();
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if total(mid) < len {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let r = 0.5 * (lo + hi);
    let mut up = vec![0.0];
    let mut x = 0.0;
    for i in 0..n_up {
        x -= d0 * r.powi(i as i32);
        up.push(x);
    }
    *up.last_mut().unwrap() = -len;
    up.reverse();
    up.extend_from_slice(&xs[1..]);
    up
}

/// TMR grid of `level` (0 = 545 x 385, 1 = 273 x 193, 2 = 137 x 97, 3 = 69 x 49): every 2^level-th node of the vendored axes.
fn tmr_axes(level: u32) -> (Vec<f64>, Vec<f64>) {
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for l in TMR_AXES.lines().skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        let v: f64 = c[2].parse().unwrap();
        if c[0] == "x" {
            xs.push(v);
        } else {
            ys.push(v);
        }
    }
    let step = 1usize << level;
    let pick = |a: &[f64]| a.iter().step_by(step).copied().collect::<Vec<f64>>();
    let (xs, ys) = (pick(&xs), pick(&ys));
    (xs, ys)
}

#[derive(Clone, Debug)]
struct Column {
    x: f64,
    cf: f64,
    u_tau: f64,
    u: Vec<(f64, f64)>,
}

struct Run {
    cols: Vec<Column>,
    iterations: usize,
    du: f64,
    change: f64,
    cells: usize,
}

impl Run {
    fn converged(&self) -> bool {
        self.change < 1e-6 && self.du < 1e-9
    }
    fn cf_at(&self, x: f64) -> f64 {
        let i = self.cols.partition_point(|c| c.x < x).clamp(1, self.cols.len() - 1);
        let (a, b) = (&self.cols[i - 1], &self.cols[i]);
        let t = (x - a.x) / (b.x - a.x);
        a.cf * (1.0 - t) + b.cf * t
    }
    fn nearest(&self, x: f64) -> &Column {
        self.cols
            .iter()
            .min_by(|a, b| (a.x - x).abs().partial_cmp(&(b.x - x).abs()).unwrap())
            .unwrap()
    }
}

/// The ADR 0070 runner on the mesh given by its node coordinates, with one change.
fn run(xs: &[f64], ys: &[f64], change: Change, max_it: usize) -> Run {
    let mesh = mesh_from_axes(xs, ys);
    let has_run_up = mesh.patch("run_up").is_some();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    if has_run_up {
        u.set_bc(&geom, "run_up", Box::new(Symmetry)).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.alpha_u = 0.8;
    st.convection = ConvScheme::LinearUpwind;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    flow.component_diag_correction = change == Change::CorrectedDiagonal;
    let (kin, omega_in) = (2.25e-7, 125.0);
    let mut turb = KOmegaSst::new(geom.clone(), NU, &["ymin"], kin, omega_in).unwrap();
    turb.relax_turb = 0.6;
    turb.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
    turb.omega
        .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(omega_in)))
        .unwrap();
    match change {
        Change::LinearUpwindTurb => turb.convection = ConvScheme::LinearUpwind,
        Change::WallCellOmegaFixed => turb.omega_fix_y_plus = 0.0,
        Change::WallOmegaCellHeight => {
            let vals: Vec<[f64; 3]> = geom
                .mesh
                .patch("ymin")
                .unwrap()
                .faces()
                .map(|f| [wall_omega(NU, 2.0 * geom.dn[f - geom.n_internal]), 0.0, 0.0])
                .collect();
            turb.omega
                .set_bc(&geom, "ymin", Box::new(FixedValue(Profile::PerFace(vals))))
                .unwrap();
        }
        _ => {}
    }
    let (mut iterations, mut chg, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..max_it {
        chg = turb.update(&flow.u, &flow.phi);
        flow.nu_face = Some(turb.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it % 2000 == 0 {
            println!("  [{} cells] it {it} du {du:.2e} dnut {chg:.2e}", geom.n_cells());
        }
        if !info.res_u.is_finite() {
            break;
        }
        if it > 200 && du < 1e-9 && chg < 1e-6 {
            break;
        }
    }
    let wall_faces: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let mesh = &geom.mesh;
    let cc = mesh.cell_centres();
    let mut cols = Vec::new();
    for (&(f, nutw), &face) in turb.wall_nut.iter().zip(&wall_faces) {
        assert_eq!(f, face);
        let c0 = mesh.owner()[f];
        let x = cc[c0][0];
        let yw = geom.dn[f - geom.n_internal];
        let tau = (NU + nutw) * flow.u.values[c0][0] / yw;
        let mut col: Vec<(f64, f64)> = (0..cc.len())
            .filter(|&c| (cc[c][0] - x).abs() < 1e-9)
            .map(|c| (cc[c][1], flow.u.values[c][0]))
            .collect();
        col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let ue = col
            .iter()
            .min_by(|a, b| (a.0 - 0.25).abs().partial_cmp(&(b.0 - 0.25).abs()).unwrap())
            .unwrap()
            .1;
        cols.push(Column {
            x,
            cf: 2.0 * tau / (ue * ue),
            u_tau: tau.abs().sqrt(),
            u: col,
        });
    }
    cols.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    Run {
        cols,
        iterations,
        du,
        change: chg,
        cells: geom.n_cells(),
    }
}

fn table2(src: &str) -> Vec<(f64, f64)> {
    src.lines()
        .skip(1)
        .map(|l| {
            let c: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
            (c[0], c[1])
        })
        .collect()
}

fn interp_ln(tab: &[(f64, f64)], x: f64) -> f64 {
    let i = tab.partition_point(|r| r.0 < x).clamp(1, tab.len() - 1);
    let (a, b) = (tab[i - 1], tab[i]);
    let t = (x.ln() - a.0.ln()) / (b.0.ln() - a.0.ln());
    a.1 * (1.0 - t) + b.1 * t
}

/// Report of one run: Cf(x_ref) against the incompressible TMR mean and the u+ offsets; returns (Cf, max |u+ offset| over y+ 30..300).
fn report(name: &str, r: &Run) -> (f64, f64) {
    let cf = r.cf_at(X_REF);
    println!(
        "[{name}] {} cells: {} iterations, du {:.2e}, dnut {:.2e}, converged {}; Cf(x_ref) {cf:.6e} ({:+.3}% vs incompressible TMR mean)",
        r.cells,
        r.iterations,
        r.du,
        r.change,
        r.converged(),
        100.0 * (cf / TMR_INC_MEAN - 1.0)
    );
    let tmr = table2(TMR_UPLUS);
    let c = r.nearest(X_REF);
    let prof: Vec<(f64, f64)> = c.u.iter().map(|&(y, u)| (y * c.u_tau / NU, u / c.u_tau)).collect();
    let mut worst: f64 = 0.0;
    let mut line = format!("[{name}] u+ offset at column x {:.4}:", c.x);
    for yp in [5.0, 10.0, 15.0, 30.0, 50.0, 100.0, 200.0, 300.0] {
        let d = 100.0 * (interp_ln(&prof, yp) / interp_ln(&tmr, yp) - 1.0);
        line += &format!(" y+ {yp}: {d:+.2}%");
        if (30.0..=300.0).contains(&yp) {
            worst = worst.max(d.abs());
        }
    }
    println!("{line}");
    (cf, worst)
}

/// Stage 1: coarse and medium with the change; prints the screen quantity |Cf' - Cf_base|/Cf_base (threshold 0.05 percent).
fn stage1(name: &str, change: Change) {
    for (i, m) in MESHES.iter().take(2).enumerate() {
        let (xs, ys) = plate_axes(*m);
        let r = run(&xs, &ys, change, 14000);
        let (cf, _) = report(&format!("{name} mesh {i}"), &r);
        println!(
            "[{name} mesh {i}] screen: Cf change {:+.4}% against the ADR 0070 value {:.6e} (threshold 0.05%)",
            100.0 * (cf / BASE[i] - 1.0),
            BASE[i]
        );
    }
}

/// Order, GCI and Richardson value of a three-mesh sequence (coarse first), ADR 0070 P2f definitions.
fn order(cf: [f64; 3]) -> (f64, f64, f64, f64) {
    let (e21, e32) = (cf[1] - cf[0], cf[2] - cf[1]);
    let p = (e21 / e32).ln() / 2.0f64.ln();
    let gci = 1.25 * (e32 / cf[2]).abs() / (2.0f64.powf(p) - 1.0);
    let rich = cf[2] + e32 / (2.0f64.powf(p) - 1.0);
    (p, gci, rich, (e32 / cf[2]).abs())
}

/// Control: the coarse mesh of the copied runner reproduces the ADR 0070 value 2.57698e-3 (the mesh builder of this file must give the same
/// mesh as `box_2d_graded`).
#[test]
#[ignore = "heavy: control run of the coarse mesh (about 15 s)"]
fn h0_coarse_mesh_reproduces_adr0070() {
    let (xs, ys) = plate_axes(MESHES[0]);
    let r = run(&xs, &ys, Change::None, 14000);
    let (cf, _) = report("H0 control", &r);
    assert!(
        (cf / BASE[0] - 1.0).abs() < 2e-6,
        "control {cf:e} against {:e}",
        BASE[0]
    );
}

/// Measured (ADR 0074 addendum): Cf change -0.0001 percent (coarse) and +0.0000 percent (medium) against ADR 0070: falsified by the screen.
#[test]
#[ignore = "heavy: stage 1 of H1 (coarse + medium, about 2 minutes)"]
fn h1_stage1_corrected_diagonal() {
    stage1("H1", Change::CorrectedDiagonal);
}

#[test]
/// Measured (ADR 0074 addendum): coarse NOT converged (14000 iterations, du 6.7e-2, nu_t change 5e4, Cf -88 percent); medium converged,
/// Cf 2.648934e-3 (-0.09 percent against ADR 0070). No finest run (registered rule for a non-converged screen).
#[ignore = "heavy: stage 1 of H2 (coarse + medium)"]
fn h2_stage1_linear_upwind_k_omega() {
    stage1("H2", Change::LinearUpwindTurb);
}

#[test]
#[ignore = "heavy: stage 1 of H3a (coarse + medium)"]
fn h3a_stage1_wall_omega_cell_height() {
    stage1("H3a", Change::WallOmegaCellHeight);
}

#[test]
#[ignore = "heavy: stage 1 of H3b (coarse + medium)"]
fn h3b_stage1_wall_cell_omega_fixed() {
    stage1("H3b", Change::WallCellOmegaFixed);
}

#[test]
/// KNOWN MISS of the set-up (ADR 0074 amendment 1): neither mesh converges in 14000 iterations (du 1.1e-5, 1.6e-5; Cf 2.247e-3, 2.340e-3)
/// because the `Symmetry` run-up with wall-resolved cells acts as a no-slip wall with the averaged diagonal (part A); H4c is the corrected set-up.
#[ignore = "known miss: defective set-up (averaged diagonal on the run-up symmetry plane), see doc comment"]
fn h4_stage1_run_up_known_miss() {
    h4_with(Change::None, "H4");
}

/// H4c (ADR 0074 amendment 1): H4 with the corrected component diagonal (the run-up `Symmetry` bottom acts as a wall with the averaged one).
#[test]
#[ignore = "heavy: stage 1 of H4c (coarse + medium)"]
fn h4c_stage1_run_up_corrected_diagonal() {
    h4_with(Change::CorrectedDiagonal, "H4c");
}

fn h4_with(change: Change, name: &str) {
    for (i, m) in MESHES.iter().take(2).enumerate() {
        let (xs, ys) = plate_axes(*m);
        let xs = with_run_up(&xs, m.nx / 5);
        let r = run(&xs, &ys, change, 14000);
        let (cf, _) = report(&format!("{name} mesh {i}"), &r);
        println!(
            "[{name} mesh {i}] screen: Cf change {:+.4}% against the ADR 0070 value {:.6e} (threshold 0.05%)",
            100.0 * (cf / BASE[i] - 1.0),
            BASE[i]
        );
    }
}

/// H5: the local refinement ratios of the ADR 0070 mesh family (no run).
#[test]
#[ignore = "diagnostic without a limit (ADR 0074 H5)"]
fn h5_mesh_family_refinement_ratios() {
    let axes: Vec<(Vec<f64>, Vec<f64>)> = MESHES.iter().map(|m| plate_axes(*m)).collect();
    let size_at = |a: &[f64], x: f64| {
        let i = a.partition_point(|v| *v < x).clamp(1, a.len() - 1);
        a[i] - a[i - 1]
    };
    for (name, idx) in [("y", 1usize), ("x", 0)] {
        let probes: &[f64] = if idx == 1 {
            &[0.0, 1e-4, 1e-3, 0.005, 0.01, 0.02, 0.03, 0.1, 0.5, 0.99]
        } else {
            &[0.0, 0.01, 0.1, 0.5, 0.97, 1.9]
        };
        for &x in probes {
            let s: Vec<f64> = axes
                .iter()
                .map(|a| size_at(if idx == 1 { &a.1 } else { &a.0 }, x.max(1e-12)))
                .collect();
            println!(
                "H5 {name} = {x}: cell sizes {:.4e} {:.4e} {:.4e}, ratios coarse/medium {:.4}, medium/fine {:.4}",
                s[0],
                s[1],
                s[2],
                s[0] / s[1],
                s[1] / s[2]
            );
        }
    }
    for (m, a) in MESHES.iter().zip(&axes) {
        let stack = a.1[m.layers];
        println!(
            "H5 mesh {m:?}: layer stack height {stack:.6e}, first cell {:.4e}",
            a.1[1]
        );
    }
}

/// H6: rcfd on the NASA TMR nested grids 69 x 49, 137 x 97, 273 x 193 nodes (3264, 13056, 52224 cells), run in parallel threads.
#[test]
/// KNOWN MISS of the set-up (ADR 0074 amendment 1): stopped after about 28000 iterations on the two coarser grids with du still 8e-6 (it
/// would have ended unconverged at the 30000 cap); same cause as H4. H6c is the corrected set-up.
#[ignore = "known miss: defective set-up (averaged diagonal on the run-up symmetry plane), see doc comment"]
fn h6_tmr_nested_grids_known_miss() {
    let _ = h6_with(Change::None, "H6");
}

/// H6c (ADR 0074 amendment 1): H6 with the corrected component diagonal.
#[test]
/// Measured (ADR 0074 addendum, one run, 41 minutes with other runs in parallel): Cf(x_ref) 2.626407e-3, 2.667342e-3, 2.686801e-3 on 3264, 13056,
/// 52224 cells (1331, 4474, 13868 iterations), -0.871, -0.823, -0.823 percent against the incompressible-code mean of the same grid; observed order
/// 1.073, GCI_fine 0.820 percent, |Cf_f - Cf_m|/Cf_f 0.724 percent, Richardson 2.704436e-3 (-0.443 percent against 2.71647e-3); u+ offset at
/// y+ 30..300 at most 3.26, 1.51, 0.66 percent. The assertions (the registered H6 rule and the ADR 0070 P2a/P2f limits applied to this NEW set-up)
/// were added after that run; the run was not repeated.
#[ignore = "heavy: H6c, three TMR grids (about 40 minutes)"]
fn h6c_tmr_nested_grids_corrected_diagonal() {
    let (converged, cf, p, gci, d) = h6_with(Change::CorrectedDiagonal, "H6c");
    assert!(converged, "H6c: not converged");
    assert!(
        (cf[2] / 2.70909e-3 - 1.0).abs() <= 0.01,
        "H6 rule: finest grid {:e}",
        cf[2]
    );
    assert!((cf[2] / TMR_INC_MEAN - 1.0).abs() <= 0.02, "P2a: {:e}", cf[2]);
    assert!(
        (0.8..=2.5).contains(&p) && gci <= 0.03 && d <= 0.015,
        "P2f: order {p}, GCI {gci}, difference {d}"
    );
}

fn h6_with(change: Change, name: &str) -> (bool, [f64; 3], f64, f64, f64) {
    let levels = [3u32, 2, 1];
    let inc_means = [2.64948e-3, 2.68947e-3, 2.70909e-3];
    let runs: Vec<Run> = std::thread::scope(|s| {
        let hs: Vec<_> = levels
            .iter()
            .map(|&l| {
                s.spawn(move || {
                    let (xs, ys) = tmr_axes(l);
                    run(&xs, &ys, change, 30000)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut cf = [0.0; 3];
    let mut offs = [0.0; 3];
    for (i, r) in runs.iter().enumerate() {
        let (c, o) = report(&format!("{name} TMR grid {} cells", r.cells), r);
        cf[i] = c;
        offs[i] = o;
        println!(
            "[{name}] {} cells: rcfd {c:.6e} against the incompressible-code mean on the same grid {:.6e}: {:+.3}%",
            r.cells,
            inc_means[i],
            100.0 * (c / inc_means[i] - 1.0)
        );
    }
    let (p, gci, rich, d) = order(cf);
    println!(
        "[{name}] observed order {p:.3}, GCI_fine {:.3}%, |Cf_f - Cf_m|/Cf_f {:.3}%, Richardson {rich:.6e} ({:+.3}% vs {TMR_INC_MEAN:.5e}); u+ offsets (max, y+ 30..300) {offs:?}",
        100.0 * gci,
        100.0 * d,
        100.0 * (rich / TMR_INC_MEAN - 1.0)
    );
    let consistent =
        runs.iter().all(|r| r.converged()) && (cf[2] / inc_means[2] - 1.0).abs() <= 0.01 && (0.8..=2.5).contains(&p);
    println!("[{name}] consistent with the TMR codes (registered rule): {consistent}");
    (runs.iter().all(|r| r.converged()), cf, p, gci, d)
}

/// Stage 2 for a stage-1 survivor: the finest mesh with the change, combined with the stage-1 coarse and medium values (measured once, ADR 0074
/// addendum) into the order, GCI and Richardson value, all against the incompressible TMR mean.
fn stage2(name: &str, change: Change, coarse_medium: [f64; 2]) {
    let (xs, ys) = plate_axes(MESHES[2]);
    stage2_on(name, change, coarse_medium, &xs, &ys);
}

fn stage2_on(name: &str, change: Change, coarse_medium: [f64; 2], xs: &[f64], ys: &[f64]) {
    let (xs, ys) = (xs.to_vec(), ys.to_vec());
    let r = run(&xs, &ys, change, 14000);
    let (cf, off) = report(&format!("{name} mesh 2"), &r);
    let seq = [coarse_medium[0], coarse_medium[1], cf];
    let (p, gci, rich, d) = order(seq);
    println!(
        "[{name}] sequence {seq:?}: observed order {p:.3}, GCI_fine {:.3}%, |Cf_f - Cf_m|/Cf_f {:.3}%, Richardson {rich:.6e} ({:+.3}% vs {TMR_INC_MEAN:.5e}); finest {:+.3}%; max |u+ offset| y+ 30..300 {off:.2}%",
        100.0 * gci,
        100.0 * d,
        100.0 * (rich / TMR_INC_MEAN - 1.0),
        100.0 * (cf / TMR_INC_MEAN - 1.0)
    );
    let confirmed = r.converged()
        && (0.8..=2.5).contains(&p)
        && d <= 0.015
        && (cf / TMR_INC_MEAN - 1.0).abs() <= 0.02
        && (rich / TMR_INC_MEAN - 1.0).abs() <= 0.02
        && off < 1.0;
    println!("[{name}] confirmed as the cause under the registered rule: {confirmed}");
}

/// H3a stage 2. Stage 1 (coarse, medium): 2.711722e-3, 2.716642e-3 (+5.23 and +2.46 percent against ADR 0070).
/// Measured (ADR 0074 addendum): finest 2.730924e-3 (+0.53 percent against the incompressible TMR mean), u+ offset 0.77 percent; the sequence is
/// not monotonically converging (differences +4.9e-6 then +1.43e-5): not confirmed. All three meshes lie within 0.6 percent of the TMR mean.
#[test]
#[ignore = "heavy: H3a finest mesh (about 30 minutes)"]
fn h3a_stage2_wall_omega_cell_height_finest() {
    stage2("H3a", Change::WallOmegaCellHeight, [2.711722e-3, 2.716642e-3]);
}

/// H3b stage 2. Stage 1 (coarse, medium): 2.555507e-3, 2.640225e-3 (-0.83 and -0.42 percent against ADR 0070).
/// Measured (ADR 0074 addendum): finest 2.692630e-3, observed order 0.693: falsified.
#[test]
#[ignore = "heavy: H3b finest mesh (about 30 minutes)"]
fn h3b_stage2_wall_cell_omega_fixed_finest() {
    stage2("H3b", Change::WallCellOmegaFixed, [2.555507e-3, 2.640225e-3]);
}

/// H4c stage 2 (run-up with the corrected diagonal). Stage 1 (coarse, medium): 2.535471e-3, 2.604683e-3 (-1.61 and -1.76 percent against ADR 0070).
/// Measured (ADR 0074 addendum): finest 2.651336e-3 (-2.40 percent against the TMR mean), observed order 0.569: falsified.
#[test]
#[ignore = "heavy: H4c finest mesh (about 30 minutes)"]
fn h4c_stage2_run_up_corrected_diagonal_finest() {
    let m = MESHES[2];
    let (xs, ys) = plate_axes(m);
    let xs = with_run_up(&xs, m.nx / 5);
    stage2_on("H4c", Change::CorrectedDiagonal, [2.535471e-3, 2.604683e-3], &xs, &ys);
}
