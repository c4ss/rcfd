//! F-257 (ADR 0213) records L1 and L2: the bits of two library runs that F-257 must not change, recorded on main c398654
//! before any code of F-257 (Windows; printed elsewhere, ADR 0137). L1 is ADR 0033's corrected piston channel G5b (the
//! incompressible PISO step on a smoothed moving mesh, 100 steps); L2 is a scripted sequence of piston positions through
//! `LayerStack::set_piston` and `LayerStack::adjust` (removals and additions), hashed with every event's cell source map
//! and the final mesh.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::layering::{LayerEvent, LayerStack};
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn push(bytes: &mut Vec<u8>, x: f64) {
    bytes.extend_from_slice(&x.to_bits().to_le_bytes());
}

fn check(name: &str, bytes: &[u8], want: &str) {
    let got = format!("{:016x}", rcfd_core::hash::fnv64(bytes));
    println!("{name}: {got}");
    if cfg!(windows) {
        assert_eq!(got, want, "{name}");
    }
}

/// L1: ADR 0033 G5b, the piston channel `box_mesh([20, 2, 2], [1, 0.2, 0.2])` with the `xmin` piston at `x_p = a0 t^2 / 2`
/// (`a0 = 2`), Laplacian smoothing, the outlet `xmax` at `p = 0`, zero-gradient sides, PISO with central convection and
/// tight solvers, `dt = 0.005`, 100 steps. Hash: the bits of every cell velocity, pressure and face flux, the time and the
/// step count.
#[test]
fn l1_g5b_piston_channel_record() {
    let mesh0 = box_mesh([20, 2, 2], [1.0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|pt| match pt.name.as_str() {
            "xmin" => PatchClass::Prescribed,
            "xmax" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let mut st = PimpleSettings::piso(0.01);
    st.convection = ConvScheme::Central;
    st.momentum_solver = TIGHT;
    st.pressure_solver = TIGHT;
    let mut s = PimpleSolver::new(geom, u, p, st);
    let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
    let piston = rcfd_mesh::motion::patch_points(&mesh0, "xmin").unwrap();
    let (a0, dt) = (2.0, 0.005);
    let mut t = 0.0;
    for _ in 0..100 {
        let t1 = t + dt;
        let x_p = 0.5 * a0 * t1 * t1;
        let mut presc = vec![vec3::ZERO; mesh0.n_points()];
        for &q in &piston {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(&g0, &classes, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
        let pts: Vec<Vec3> = mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect();
        let step = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &step, &old, &["xmin".to_string()], dt).unwrap();
        s.step_ale_with_walls(dt, step.geom, step.mesh_flux, walls).unwrap();
        t = t1;
    }
    let mut bytes = Vec::new();
    for v in &s.u.values {
        for x in v {
            push(&mut bytes, *x);
        }
    }
    for x in &s.p.values {
        push(&mut bytes, *x);
    }
    for x in &s.phi {
        push(&mut bytes, *x);
    }
    push(&mut bytes, s.time);
    bytes.extend_from_slice(&(s.history.len() as u64).to_le_bytes());
    println!("L1: t = {}, {} steps", s.time, s.history.len());
    check("L1", &bytes, "affff0bb34a248e2");
}

/// L2: `LayerStack::uniform(6, 1, [2, 1], [0.1, 0.1], 0.4, 1.6)`, the piston moved from 1 down to 0.3 and back up to 1.3
/// in steps of 0.01 with one `adjust` after each move. Hash: every event's kind, plane coordinates and cell sources (cell
/// index and fraction bits), the event count, and the content hash and point bits of the final mesh.
#[test]
fn l2_layer_stack_event_record() {
    let mut stack = LayerStack::uniform(6, 1.0, [2, 1], [0.1, 0.1], 0.4, 1.6).unwrap();
    let mut positions: Vec<f64> = (0..=70).map(|k| 1.0 - 0.01 * k as f64).collect();
    positions.extend((1..=100).map(|k| 0.3 + 0.01 * k as f64));
    let mut bytes = Vec::new();
    let (mut removed, mut added) = (0usize, 0usize);
    for x in positions {
        stack.set_piston(x).unwrap();
        if let Some(ch) = stack.adjust() {
            match ch.event {
                LayerEvent::Removed => {
                    removed += 1;
                    bytes.push(0);
                }
                LayerEvent::Added => {
                    added += 1;
                    bytes.push(1);
                }
            }
            for x in &stack.xs {
                push(&mut bytes, *x);
            }
            for src in &ch.sources {
                for &(c, w) in src {
                    bytes.extend_from_slice(&(c as u64).to_le_bytes());
                    push(&mut bytes, w);
                }
            }
        }
    }
    let m = stack.mesh().unwrap();
    bytes.extend_from_slice(&m.content_hash().to_le_bytes());
    for q in m.points() {
        for x in q {
            push(&mut bytes, *x);
        }
    }
    bytes.extend_from_slice(&((removed + added) as u64).to_le_bytes());
    println!(
        "L2: {removed} removals, {added} additions, {} layers at the end",
        stack.n_layers()
    );
    check("L2", &bytes, "52af54712423ce0a");
}
