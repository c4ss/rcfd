//! ADR 0396 (F-468): the flow checks of the pose-continuous donor interpolation `DonorInterpolation::Continuous` on TG_2
//! and TG_1 of ADR 0359 (BDF2, three outer iterations, `rhie_chow_step_consistent`, the component moving by `tg_pose`).
//! The helpers of the set-up are copies of those of `overset_motion_f442.rs`. The kinematic checks are in
//! `crates/rcfd-mesh/tests/overset_f468.rs`.
//!
//! The slow test is ignored: `cargo test --release -p rcfd-fv --test overset_f468 -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Connectivity, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// Half the side of the component square.
const HALF: f64 = 0.6;

/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit`.
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    /// `value >= limit`.
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

/// The exact velocity at `t` on the four sides.
fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

fn decay() -> f64 {
    (-2.0 * NU_TG * T_END).exp()
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

/// The overset of TG_`level` (with the hole) and the donor interpolation `mode`.
fn tg_overset(level: usize, mode: DonorInterpolation) -> Overset {
    let cs = uniform(-HALF, 2.0 * HALF, 20 * level);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&tg_background(level), &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
    };
    let mut ov = Overset::new(composite, spec).unwrap();
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
    }
    ov
}

/// The number of fringe cells, present at both kinds, whose first stencil cell changed.
fn first_cell_changes(a: &Connectivity, b: &Connectivity) -> usize {
    let mut first = vec![usize::MAX; a.kind.len()];
    for (c, s) in &a.stencils {
        first[*c] = s.cells[0];
    }
    b.stencils
        .iter()
        .filter(|(c, s)| first[*c] != usize::MAX && first[*c] != s.cells[0])
        .count()
}

struct Run {
    s: OversetSolver,
    e: f64,
    continuity: f64,
    fallbacks: usize,
    sizes: [usize; 2],
}

/// TG_`level` with BDF2, three outer iterations and C, `steps` steps of `dt`, the donor interpolation `mode`.
fn run(level: usize, steps: usize, dt: f64, mode: DonorInterpolation) -> Run {
    let t0 = std::time::Instant::now();
    let ov = tg_overset(level, mode);
    let pose = tg_pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let coupling = Some(OversetCoupling { overset: ov, pose });
    let mut set = settings(NU_TG);
    set.rhie_chow_step_consistent = true;
    let mut s = OversetSolver::new(geom, u, p, set, MassMode::Conservative, coupling).unwrap();
    assert_eq!(s.conn.interpolation, mode);
    s.set_time_scheme(TimeScheme::Bdf2).unwrap();
    s.set_outer_iterations(3, 1.0, 1.0).unwrap();
    let (mut continuity, mut fallbacks, mut switches) = (0.0f64, s.conn.n_fallback(), 0);
    let mut sizes = [usize::MAX, 0];
    let mut last: Option<Connectivity> = None;
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let st = s.prepare(dt, tg_pose(t), &[]).unwrap();
        for (_, stn) in &st.conn.stencils {
            sizes = [sizes[0].min(stn.cells.len()), sizes[1].max(stn.cells.len())];
        }
        if let Some(l) = &last {
            switches += first_cell_changes(l, &st.conn);
        }
        last = Some(st.conn.clone());
        let info = s.advance(dt, st).unwrap();
        continuity = continuity.max(info.continuity);
        fallbacks = fallbacks.max(info.n_fallback);
    }
    let e = (0..s.geom.n_cells())
        .filter(|&c| s.conn.kind[c] != CellKind::Hole)
        .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], steps as f64 * dt))))
        .fold(0.0, f64::max)
        / (-2.0 * NU_TG * steps as f64 * dt).exp();
    println!(
        "TG_{level} {mode:?}, {steps} steps of {dt} ({:.1} s): e {e:.4e}, continuity {continuity:.2e}, fallbacks {fallbacks}, stencils of {} to {} cells, first-cell changes {switches}",
        t0.elapsed().as_secs_f64(),
        sizes[0],
        sizes[1]
    );
    Run {
        s,
        e,
        continuity,
        fallbacks,
        sizes,
    }
}

/// The largest difference over the decay over the non-hole cells that `pick` selects.
fn difference_where(a: &OversetSolver, b: &OversetSolver, pick: impl Fn(usize) -> bool) -> f64 {
    assert!(a.conn.kind == b.conn.kind, "the two runs end at different kinds");
    (0..a.geom.n_cells())
        .filter(|&c| a.conn.kind[c] != CellKind::Hole && pick(c))
        .map(|c| vec3::norm(vec3::sub(a.u.values[c], b.u.values[c])) / decay())
        .fold(0.0, f64::max)
}

/// ADR 0396 A8, the fast companion of A7: TG_1 with `Continuous`, 20 steps of 0.005.
#[test]
fn a8_tg1_continuous_runs() {
    let r = run(1, 20, 0.005, DonorInterpolation::Continuous);
    let l = run(1, 20, 0.005, DonorInterpolation::Linear);
    println!(
        "A8: e after 20 steps {:.4e} with Continuous, {:.4e} with Linear (printed); stencils of {} to {} cells",
        r.e, l.e, r.sizes[0], r.sizes[1]
    );
    let mut c = Checks::new();
    c.at_most("A8 continuity, every step", r.continuity, 1e-9);
    c.at_most("A8 fallbacks, every step", r.fallbacks as f64, 0.0);
    c.finish();
}

/// ADR 0396 A7 (acceptance): TG_2 with `Continuous` in 200, 400 and 800 steps.
#[test]
#[ignore = "slow (three level-2 overset runs, about 7 minutes): run once for the addendum"]
fn a7_tg2_continuous_remainder() {
    let m = DonorInterpolation::Continuous;
    let (a, b, r) = (run(2, 200, T_END / 200.0, m), run(2, 400, T_END / 400.0, m), run(2, 800, T_END / 800.0, m));
    let nb = a.s.conn.n_background_cells;
    let k200 = difference_where(&a.s, &r.s, |c| c >= nb);
    let k400 = difference_where(&b.s, &r.s, |c| c >= nb);
    let g200 = difference_where(&a.s, &r.s, |c| c < nb);
    let g400 = difference_where(&b.s, &r.s, |c| c < nb);
    let d200 = difference_where(&a.s, &r.s, |_| true);
    let d400 = difference_where(&b.s, &r.s, |_| true);
    println!(
        "A7 printed: background cells max |d_200| {g200:.4e}, ratio {:.4}; all non-hole cells max |d_200| {d200:.4e}, D2 {:.4}",
        g200 / g400,
        d200 / d400
    );
    let mut c = Checks::new();
    c.at_most("A7 component cells max |d_200|", k200, 2e-6);
    c.at_least("A7 component cells ratio d_200 / d_400", k200 / k400, 4.0);
    c.at_most("A7 e(TG_2) in 800 steps", r.e, 3.186e-4);
    c.at_most("A7 continuity, every step", a.continuity.max(b.continuity).max(r.continuity), 1e-9);
    c.at_most("A7 fallbacks, every step", a.fallbacks.max(b.fallbacks).max(r.fallbacks) as f64, 0.0);
    c.finish();
}
