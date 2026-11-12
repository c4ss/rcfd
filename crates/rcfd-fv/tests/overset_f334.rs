//! F-334 (ADR 0246): the first-order error term of the calculated component cells of the overset coupling. The set-up is
//! RL of ADR 0232 amendment 3 (Taylor-Green, the component at rest, the phase of its fringe locked between the levels),
//! here on levels 1 to 4 and with the switches that decide the hypotheses of ADR 0246: the two parts at different levels,
//! stencils that are exact for biquadratic fields, the amplitude, the reference cell, the step, the reference of the norm,
//! the conditions of the component's outer boundary, coincident meshes. No library code is changed by these tests.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f334`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, pose_inverse_apply, CellKind, Connectivity, Overset, OversetSpec, Stencil};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// Background cell size of level 1.
const H1: f64 = PI / 40.0;
/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU_TG);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    // ADR 0246 He: one scheme on every face, no limiter
    assert!(s.convection == ConvScheme::Central && s.linear_upwind_limiter.is_none());
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
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
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- set-ups ---------------------------------------------------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

/// The composite of the background of level `lb` and a component of half width `half` with `n_side` cells per side and the
/// hole `max(|x_ref|, |y_ref|) < hole`.
fn composite(lb: usize, n_side: usize, half: f64, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n_side);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&background(lb), &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

/// RL of ADR 0232 amendment 3 with the background at level `lb` and the component at level `lc`.
fn rl_mesh(lb: usize, lc: usize) -> Overset {
    composite(lb, 20 * lc, 7.5 * H1, 3.1 * H1)
}

fn translation(d: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: 0.0,
        translation: [d, d, 0.0],
    }
}

fn rl_pose() -> Pose {
    translation(16.5 * H1)
}

/// CM of ADR 0246: a component whose cells coincide with background cells.
fn cm_mesh(level: usize) -> Overset {
    composite(level, 16 * level, 8.0 * H1, 3.1 * H1)
}

fn cm_pose() -> Pose {
    translation(16.0 * H1)
}

fn tg_u(x: Vec3, t: f64, amp: f64) -> Vec3 {
    let e = amp * (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64, amp: f64) -> f64 {
    -amp * amp * ((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
}

fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64, amp: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t, amp))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// Observed order between two errors on meshes whose spacings have the ratio `ratio` (coarse over fine).
fn order(coarse: f64, fine: f64, ratio: f64) -> f64 {
    (coarse / fine).ln() / ratio.ln()
}

/// The first-order coefficient of two errors at a level and at twice that level under `a / L^2 + b / L`, in units of the
/// coarser level.
fn b_hat(coarse: f64, fine: f64) -> f64 {
    4.0 * fine - coarse
}

// ---- the nine-cell stencils that are exact for biquadratic fields (ADR 0246 Hq) -------------------------------------------

/// The quadratic Lagrange polynomials through the points -1, 0, 1 at `s`.
fn lagrange3(s: f64) -> [f64; 3] {
    [0.5 * s * (s - 1.0), 1.0 - s * s, 0.5 * s * (s + 1.0)]
}

/// One uniform Cartesian part as a grid of cells `(i, j)` in its reference frame.
struct Grid {
    x0: f64,
    y0: f64,
    h: f64,
    nx: usize,
    cells: Vec<usize>,
}

impl Grid {
    fn new(refc: &[Vec3], vol: &[f64], cells: std::ops::Range<usize>) -> Self {
        let h = (vol[cells.start] / DEPTH).sqrt();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for c in cells.clone() {
            x0 = x0.min(refc[c][0]);
            y0 = y0.min(refc[c][1]);
            x1 = x1.max(refc[c][0]);
            y1 = y1.max(refc[c][1]);
        }
        let nx = ((x1 - x0) / h).round() as usize + 1;
        let ny = ((y1 - y0) / h).round() as usize + 1;
        let mut g = Grid {
            x0: x0 - 0.5 * h,
            y0: y0 - 0.5 * h,
            h,
            nx,
            cells: vec![usize::MAX; nx * ny],
        };
        for c in cells {
            let (i, j) = g.locate(refc[c]);
            g.cells[i + nx * j] = c;
        }
        assert!(g.cells.iter().all(|&c| c != usize::MAX), "the part is not a full grid");
        g
    }
    fn locate(&self, x: Vec3) -> (usize, usize) {
        (
            ((x[0] - self.x0) / self.h).floor() as usize,
            ((x[1] - self.y0) / self.h).floor() as usize,
        )
    }
    fn at(&self, i: usize, j: usize) -> usize {
        self.cells[i + self.nx * j]
    }
}

/// The two parts of a composite as grids, with the reference cell centres.
struct Grids {
    nb: usize,
    refc: Vec<Vec3>,
    bg: Grid,
    comp: Grid,
}

impl Grids {
    fn new(ov: &Overset) -> Self {
        let m = &ov.composite.mesh;
        let nb = ov.composite.n_background_cells;
        let refc = m.cell_centres().to_vec();
        let bg = Grid::new(&refc, m.cell_volumes(), 0..nb);
        let comp = Grid::new(&refc, m.cell_volumes(), nb..m.n_cells());
        Grids { nb, refc, bg, comp }
    }

    /// The nine-cell stencil of the fringe cell `c` with the centre `x` (at the pose `pose`) and the donor cell `donor`.
    fn quadratic(&self, pose: &Pose, c: usize, x: Vec3, donor: usize) -> Stencil {
        let (grid, point) = if c < self.nb {
            (&self.comp, pose_inverse_apply(pose, x))
        } else {
            (&self.bg, x)
        };
        let (i0, j0) = grid.locate(self.refc[donor]);
        let xi = (point[0] - self.refc[donor][0]) / grid.h;
        let eta = (point[1] - self.refc[donor][1]) / grid.h;
        assert!(
            xi.abs() <= 0.5 + 1e-6 && eta.abs() <= 0.5 + 1e-6,
            "the donor does not contain the point"
        );
        let (lx, ly) = (lagrange3(xi), lagrange3(eta));
        let mut cells = vec![donor];
        let mut weights = vec![lx[1] * ly[1]];
        for n in 0..3 {
            for m in 0..3 {
                if m == 1 && n == 1 {
                    continue;
                }
                cells.push(grid.at(i0 + m - 1, j0 + n - 1));
                weights.push(lx[m] * ly[n]);
            }
        }
        Stencil { cells, weights }
    }
}

/// Which stencils a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Stencils {
    /// Those of the library (ADR 0219 M4).
    Linear,
    /// The donor cell with the weight 1 (ND).
    Donor,
    /// The nine-cell stencils on the component fringe (`cf`) and on the background fringe (`bf`).
    Quadratic { cf: bool, bf: bool },
}

/// The stencils of `conn` replaced as `mode` says. Returns the number of stencil cells that are not calculated.
fn replace_stencils(conn: &mut Connectivity, mode: Stencils, grids: &Grids, pose: &Pose, centres: &[Vec3]) -> usize {
    let mut bad = 0;
    let kind = conn.kind.clone();
    for (c, stn) in conn.stencils.iter_mut() {
        match mode {
            Stencils::Linear => {}
            Stencils::Donor => {
                *stn = Stencil {
                    cells: vec![stn.cells[0]],
                    weights: vec![1.0],
                }
            }
            Stencils::Quadratic { cf, bf } => {
                if (*c < grids.nb && bf) || (*c >= grids.nb && cf) {
                    *stn = grids.quadratic(pose, *c, centres[*c], stn.cells[0]);
                }
            }
        }
        bad += stn.cells.iter().filter(|&&k| kind[k] != CellKind::Calculated).count();
    }
    bad
}

// ---- flow runs -------------------------------------------------------------------------------------------------------------

/// One variant of a run.
#[derive(Clone, Copy)]
struct Cfg {
    steps: usize,
    /// Amplitude of the Taylor-Green solution (Hn).
    amp: f64,
    stencils: Stencils,
    /// The reference cell of the pressure is the last background cell instead of cell 0 (Ha, a3).
    pin_last: bool,
    /// The velocity of the component's outer patches is a fixed value zero instead of zero gradient (Hc).
    fixed_outer: bool,
    /// The transient Rhie-Chow term of the library (`PimpleSettings::ddt_correction`; amendment 2 switches it off).
    ddt_correction: bool,
    /// End time (amendment 2 runs to `t = 1`); `steps` steps reach it.
    end: f64,
}

impl Cfg {
    fn new(steps: usize) -> Self {
        Cfg {
            steps,
            amp: 1.0,
            stencils: Stencils::Linear,
            pin_last: false,
            fixed_outer: false,
            ddt_correction: true,
            end: T_END,
        }
    }
}

struct Run {
    s: OversetSolver,
    amp: f64,
    /// Largest continuity over the steps (C1).
    cont: f64,
    delta_max: f64,
    delta_mean: f64,
    /// Largest error of a calculated cell of the solver as constructed (C2).
    t0_error: f64,
    /// Stencil cells that were not calculated, over the steps.
    bad_stencil_cells: usize,
    /// The velocity at the times `SNAP_TIMES` (amendment 1).
    snaps: Vec<Vec<Vec3>>,
    secs: f64,
}

/// Times at which a run keeps its velocity field (amendment 1); every one is a whole number of steps at every level.
const SNAP_TIMES: [f64; 5] = [0.0025, 0.025, 0.125, 0.25, 0.5];
/// Further times of a run that goes on to `t = 1` (amendment 2).
const LATE_TIMES: [f64; 2] = [0.75, 1.0];

fn eps(s: &OversetSolver, c: usize, t: f64, amp: f64) -> f64 {
    vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], t, amp))) / (amp * (-2.0 * NU_TG * t).exp())
}

/// A Taylor-Green run on the composite `ov` held at `pose`, to `t = 0.5`.
fn run(ov: Overset, pose: Pose, cfg: &Cfg) -> Run {
    let t0 = Instant::now();
    let dt = cfg.end / cfg.steps as f64;
    let grids = Grids::new(&ov);
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh_at(&ov, &pose))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0, cfg.amp));
    tg_conditions(&geom, &mut u, 0.0, cfg.amp);
    if cfg.fixed_outer {
        for name in ["c_xmin", "c_xmax", "c_ymin", "c_ymax"] {
            u.set_bc(&geom, name, Box::new(FixedValue(Profile::Uniform([0.0; 3]))))
                .unwrap();
        }
    }
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0, cfg.amp));
    let mut st = settings();
    st.ddt_correction = cfg.ddt_correction;
    if cfg.pin_last {
        st.p_ref = (grids.nb - 1, 0.0);
    }
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        st,
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let t0_error = (0..s.geom.n_cells())
        .filter(|&c| s.conn.kind[c] == CellKind::Calculated)
        .map(|c| eps(&s, c, 0.0, cfg.amp))
        .fold(0.0, f64::max);
    let (mut cont, mut delta_max, mut delta_sum, mut bad) = (0.0f64, 0.0f64, 0.0f64, 0usize);
    let mut snaps = Vec::new();
    for k in 1..=cfg.steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t, cfg.amp);
        let mut step = s.prepare(dt, pose, &[]).unwrap();
        let centres = step.geom.mesh.cell_centres().to_vec();
        bad += replace_stencils(&mut step.conn, cfg.stencils, &grids, &pose, &centres);
        let info = s.advance(dt, step).unwrap();
        cont = cont.max(info.continuity);
        delta_max = delta_max.max(info.delta.abs());
        delta_sum += info.delta.abs();
        if SNAP_TIMES
            .iter()
            .chain(&LATE_TIMES)
            .any(|ts| (ts - t).abs() < 0.25 * dt)
        {
            snaps.push(s.u.values.clone());
        }
    }
    Run {
        s,
        amp: cfg.amp,
        cont,
        delta_max,
        delta_mean: delta_sum / cfg.steps as f64,
        t0_error,
        bad_stencil_cells: bad,
        snaps,
        secs: t0.elapsed().as_secs_f64(),
    }
}

/// Cell classes: component fringe, component calculated, background fringe, background calculated.
const CLASSES: [&str; 4] = ["CF", "CC", "BF", "BC"];

fn class_of(kind: CellKind, background: bool) -> Option<usize> {
    match (kind, background) {
        (CellKind::Hole, _) => None,
        (CellKind::Fringe, false) => Some(0),
        (CellKind::Calculated, false) => Some(1),
        (CellKind::Fringe, true) => Some(2),
        (CellKind::Calculated, true) => Some(3),
    }
}

/// Root mean square (weighted with the cell volumes) and largest value of `eps_c` over the calculated component cells at
/// `t = 0.5`; with `cell_mean` the reference is the exact mean over the cell, the centre value times `(sin(h/2)/(h/2))^2`.
fn cc_error(r: &Run, cell_mean: bool) -> (f64, f64) {
    let s = &r.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let scale = r.amp * (-2.0 * NU_TG * T_END).exp();
    let (mut sum, mut vsum, mut worst) = (0.0, 0.0, 0.0f64);
    for c in nb..s.geom.n_cells() {
        if s.conn.kind[c] != CellKind::Calculated {
            continue;
        }
        let mut ex = tg_u(centres[c], T_END, r.amp);
        if cell_mean {
            let h = (vol[c] / DEPTH).sqrt();
            let f = ((0.5 * h).sin() / (0.5 * h)).powi(2);
            ex = vec3::scale(ex, f);
        }
        let e = vec3::norm(vec3::sub(s.u.values[c], ex)) / scale;
        sum += vol[c] * e * e;
        vsum += vol[c];
        worst = worst.max(e);
    }
    ((sum / vsum).sqrt(), worst)
}

/// Root mean square over the calculated component cells of the difference of the velocities of two runs on the same
/// composite, over the decay factor (Ht), and the largest difference over the non-hole cells (Hc).
fn difference(a: &Run, b: &Run) -> (f64, f64) {
    let s = &a.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let scale = a.amp * (-2.0 * NU_TG * T_END).exp();
    let (mut sum, mut vsum, mut worst) = (0.0, 0.0, 0.0f64);
    for c in 0..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Hole {
            continue;
        }
        let d = vec3::norm(vec3::sub(s.u.values[c], b.s.u.values[c])) / scale;
        worst = worst.max(d);
        if c >= nb && s.conn.kind[c] == CellKind::Calculated {
            sum += vol[c] * d * d;
            vsum += vol[c];
        }
    }
    ((sum / vsum).sqrt(), worst)
}

/// Mean and root mean square of a sample.
#[derive(Default, Clone, Copy)]
struct Acc {
    n: f64,
    sum: f64,
    sq: f64,
}

impl Acc {
    fn add(&mut self, v: f64) {
        self.n += 1.0;
        self.sum += v;
        self.sq += v * v;
    }
    fn mean(&self) -> f64 {
        self.sum / self.n.max(1.0)
    }
    fn rms(&self) -> f64 {
        (self.sq / self.n.max(1.0)).sqrt()
    }
}

/// The printed numbers of a run at `t = 0.5` (ADR 0246, "printed, no limit") and `D_CF`, `D_BF`, `D_CC` of Hb.
struct Detail {
    class_max: [f64; 4],
    class_rms: [f64; 4],
    /// Mean error vector over CC and the share of the squared CC error that is a multiple of the exact velocity.
    mean_error: Vec3,
    amplitude_share: f64,
    /// Pressure error per class with the mean over BC removed (root mean square), and the class mean of CC.
    p_rms: [f64; 4],
    p_mean_cc: f64,
    /// Pressure jump at the fringe faces of the component and of the background: mean and root mean square.
    jump: [Acc; 2],
    /// Mean normal velocity of the face-interpolated cell velocity out of the calculated component cells and out of the
    /// calculated background cells through `Gamma_B`.
    q: [f64; 2],
    /// `D_CF`, `D_BF`, `D_CC`.
    d: [f64; 3],
}

fn detail(r: &Run) -> Detail {
    let s = &r.s;
    let g = &s.geom;
    let n = g.n_cells();
    let nb = s.conn.n_background_cells;
    let vol = g.mesh.cell_volumes();
    let centres = g.mesh.cell_centres();
    let scale = r.amp * (-2.0 * NU_TG * T_END).exp();
    let kind = &s.conn.kind;
    let (mut csum, mut cvol) = ([0.0; 4], [0.0; 4]);
    let mut class_max = [0.0f64; 4];
    let (mut mean_error, mut eu, mut uu, mut ee) = ([0.0; 3], 0.0, 0.0, 0.0);
    let p_err: Vec<f64> = (0..n).map(|c| s.p.values[c] - tg_p(centres[c], T_END, r.amp)).collect();
    let (mut psum, mut pvol) = ([0.0; 4], [0.0; 4]);
    for c in 0..n {
        let Some(k) = class_of(kind[c], c < nb) else {
            continue;
        };
        let ex = tg_u(centres[c], T_END, r.amp);
        let ev = vec3::scale(vec3::sub(s.u.values[c], ex), 1.0 / scale);
        let e = vec3::norm(ev);
        class_max[k] = class_max[k].max(e);
        csum[k] += vol[c] * e * e;
        cvol[k] += vol[c];
        psum[k] += vol[c] * p_err[c];
        pvol[k] += vol[c];
        if k == 1 {
            mean_error = vec3::add(mean_error, vec3::scale(ev, vol[c]));
            let exs = vec3::scale(ex, 1.0 / scale);
            eu += vol[c] * vec3::dot(ev, exs);
            uu += vol[c] * vec3::dot(exs, exs);
            ee += vol[c] * vec3::dot(ev, ev);
        }
    }
    let level = psum[3] / pvol[3];
    let mut p_sq = [0.0; 4];
    for c in 0..n {
        if let Some(k) = class_of(kind[c], c < nb) {
            p_sq[k] += vol[c] * (p_err[c] - level).powi(2);
        }
    }
    let sf = g.mesh.face_area_vectors();
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    let mut jump = [Acc::default(); 2];
    let mut d = [Acc::default(); 3];
    let (mut flux, mut area) = ([0.0; 2], [0.0; 2]);
    for f in 0..g.n_internal {
        let (o, m) = (own[f], nei[f]);
        let a = vec3::norm(sf[f]);
        if a == 0.0 {
            continue;
        }
        let w = g.weights[f];
        let uf = vec3::add(vec3::scale(s.u.values[o], w), vec3::scale(s.u.values[m], 1.0 - w));
        let through = vec3::dot(uf, sf[f]);
        let defect = (s.phi[f] - through) / a / scale;
        let part = usize::from(o < nb);
        match (kind[o], kind[m]) {
            (CellKind::Calculated, CellKind::Calculated) => {
                if part == 0 {
                    d[2].add(defect);
                }
            }
            (CellKind::Calculated, CellKind::Fringe) | (CellKind::Fringe, CellKind::Calculated) => {
                let (calc, fringe, sign) = if kind[o] == CellKind::Calculated {
                    (o, m, 1.0)
                } else {
                    (m, o, -1.0)
                };
                d[part].add(defect);
                flux[part] += sign * through / scale;
                area[part] += a;
                jump[part].add(p_err[fringe] - p_err[calc]);
            }
            _ => {}
        }
    }
    Detail {
        class_max,
        class_rms: [0, 1, 2, 3].map(|k| (csum[k] / cvol[k]).sqrt()),
        mean_error: vec3::scale(mean_error, 1.0 / cvol[1]),
        amplitude_share: eu * eu / (uu * ee),
        p_rms: [0, 1, 2, 3].map(|k| (p_sq[k] / pvol[k]).sqrt()),
        p_mean_cc: psum[1] / pvol[1] - level,
        jump,
        q: [flux[0] / area[0], flux[1] / area[1]],
        d: [d[0].rms(), d[1].rms(), d[2].rms()],
    }
}

fn report(tag: &str, r: &Run) -> Detail {
    let (e, worst) = cc_error(r, false);
    let dt = detail(r);
    println!(
        "{tag}: {} cells, {:.1} s; continuity {:.2e}; abs(delta) largest {:.3e}, mean {:.3e}; CC rms {e:.4e}, largest {worst:.4e}",
        r.s.geom.n_cells(),
        r.secs,
        r.cont,
        r.delta_max,
        r.delta_mean
    );
    for (k, name) in CLASSES.iter().enumerate() {
        println!(
            "{tag}:   {name}: error largest {:.4e}, rms {:.4e}; pressure error (level removed) rms {:.4e}",
            dt.class_max[k], dt.class_rms[k], dt.p_rms[k]
        );
    }
    println!(
        "{tag}:   CC mean error vector ({:.3e}, {:.3e}); share of the exact velocity in the CC error {:.4}; pressure class mean CC - BC {:.4e}",
        dt.mean_error[0], dt.mean_error[1], dt.amplitude_share, dt.p_mean_cc
    );
    println!(
        "{tag}:   pressure jump at the fringe faces: component mean {:.4e} rms {:.4e}; background mean {:.4e} rms {:.4e}",
        dt.jump[0].mean(),
        dt.jump[0].rms(),
        dt.jump[1].mean(),
        dt.jump[1].rms()
    );
    println!(
        "{tag}:   mean normal velocity of (u)_f out of the calculated component cells {:.4e}, out of the calculated background cells through Gamma_B {:.4e}; D_CF {:.4e}, D_BF {:.4e}, D_CC {:.4e}",
        dt.q[0], dt.q[1], dt.d[0], dt.d[1], dt.d[2]
    );
    dt
}

/// RL at `(lb, lc)` with `200 max(lb, lc)` steps and the changes `f` of the variant.
fn rl(lb: usize, lc: usize, f: impl Fn(&mut Cfg)) -> Run {
    let mut cfg = Cfg::new(200 * lb.max(lc));
    f(&mut cfg);
    run(rl_mesh(lb, lc), rl_pose(), &cfg)
}

/// The background alone with the library step (TGB of ADR 0219), `steps` steps to `t = 0.5`.
fn background_alone(level: usize, steps: usize) -> (Arc<FvGeometry>, Vec<Vec3>) {
    let dt = T_END / steps as f64;
    let geom = Arc::new(FvGeometry::new(Arc::new(background(level))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0, 1.0));
    tg_conditions(&geom, &mut u, 0.0, 1.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0, 1.0));
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings());
    for k in 1..=steps {
        tg_conditions(&geom, &mut s.u, k as f64 * dt, 1.0);
        s.step(dt);
    }
    (geom, s.u.values.clone())
}

// ---- kinematic checks ------------------------------------------------------------------------------------------------------

fn truncation(centres: &[Vec3], c: usize, st: &Stencil, f: &dyn Fn(Vec3) -> Vec3) -> Vec3 {
    let mut v = vec3::scale(f(centres[c]), -1.0);
    for (&k, &w) in st.cells.iter().zip(&st.weights) {
        v = vec3::add(v, vec3::scale(f(centres[k]), w));
    }
    v
}

/// ADR 0246 K1 and K2 (no flow solve).
#[test]
fn k_phase_lock_and_biquadratic_stencils() {
    let mut c = Checks::new();
    let pose = rl_pose();
    let tg = |x: Vec3| tg_u(x, 0.0, 1.0);
    let poly = |x: Vec3| {
        [
            (1.0 + x[0] + 2.0 * x[1]) * (1.0 - x[0] + x[1]) + x[0] * x[0] * x[1] * x[1],
            0.0,
            0.0,
        ]
    };
    let mut kappa = Vec::new();
    let mut sup = Vec::new();
    for l in [1usize, 2, 3, 4] {
        let ov = rl_mesh(l, l);
        let mesh = mesh_at(&ov, &pose);
        let conn = ov.connectivity(&mesh, &pose).unwrap();
        let nb = conn.n_background_cells;
        let centres = mesh.cell_centres();
        let mut acc = Acc::default();
        for (cell, st) in conn.stencils.iter().filter(|(cell, _)| *cell >= nb) {
            acc.add(vec3::norm(truncation(centres, *cell, st, &tg)));
        }
        let h = H1 / l as f64;
        kappa.push(acc.rms() / (h * h));
        println!("K1 level {l}: kappa = rms_CF |I| / h_B^2 = {:.5}", acc.rms() / (h * h));
        if l == 3 {
            continue;
        }
        let grids = Grids::new(&ov);
        let mut q = conn.clone();
        let bad = replace_stencils(
            &mut q,
            Stencils::Quadratic { cf: true, bf: true },
            &grids,
            &pose,
            centres,
        );
        let nine = q.stencils.iter().all(|(_, st)| st.cells.len() == 9);
        c.holds(
            &format!("K2 level {l}: nine calculated cells in every stencil"),
            bad == 0 && nine,
            &format!("{bad} stencil cells that are not calculated"),
        );
        let (mut p_err, mut t_sup, mut w_min) = (0.0f64, 0.0f64, f64::INFINITY);
        for (cell, st) in &q.stencils {
            p_err = p_err.max(vec3::norm(truncation(centres, *cell, st, &poly)));
            t_sup = t_sup.max(vec3::norm(truncation(centres, *cell, st, &tg)));
            w_min = st.weights.iter().copied().fold(w_min, f64::min);
        }
        c.at_most(
            &format!("K2 level {l}: the biquadratic field at the fringe centres"),
            p_err,
            1e-11,
        );
        println!("K2 level {l}: largest truncation of the exact velocity {t_sup:.4e}; smallest weight {w_min:.4}");
        sup.push(t_sup);
    }
    for (l, k) in [2usize, 3, 4].iter().zip(&kappa[1..]) {
        c.at_most(
            &format!("K1 kappa of level {l} against level 1"),
            (k / kappa[0] - 1.0).abs(),
            0.02,
        );
    }
    c.at_least(
        "K2 order of the truncation, levels 1 to 2",
        order(sup[0], sup[1], 2.0),
        2.7,
    );
    c.at_least(
        "K2 order of the truncation, levels 2 to 4",
        order(sup[1], sup[2], 2.0),
        2.7,
    );
    c.finish();
}

// ---- the control CM ---------------------------------------------------------------------------------------------------------

/// ADR 0246 CM: with coincident meshes the overset solution is the solution of the background alone.
#[test]
fn cm_coincident_meshes_give_the_background_alone() {
    let mut c = Checks::new();
    for l in [1usize, 2] {
        let ov = cm_mesh(l);
        let grids = Grids::new(&ov);
        let r = run(ov, cm_pose(), &Cfg::new(200 * l));
        let (_, alone) = background_alone(l, 200 * l);
        let s = &r.s;
        let centres = s.geom.mesh.cell_centres();
        let (mut worst, mut worst_weight) = (0.0f64, 0.0f64);
        for cell in 0..s.geom.n_cells() {
            if s.conn.kind[cell] != CellKind::Calculated {
                continue;
            }
            let (i, j) = grids.bg.locate(centres[cell]);
            let twin = grids.bg.at(i, j);
            assert!(
                vec3::norm(vec3::sub(centres[twin], centres[cell])) < 1e-9,
                "the cells do not coincide"
            );
            worst = worst.max(vec3::norm(vec3::sub(s.u.values[cell], alone[twin])));
        }
        for (_, st) in &s.conn.stencils {
            worst_weight = worst_weight.max((st.weights[0] - 1.0).abs());
        }
        println!(
            "CM level {l}: {:.1} s; largest abs(1 - donor weight) {worst_weight:.3e}; abs(delta) largest {:.3e}; CC rms {:.4e}",
            r.secs,
            r.delta_max,
            cc_error(&r, false).0
        );
        c.at_most(&format!("C1 CM level {l} continuity"), r.cont, 1e-9);
        c.at_most(
            &format!("CM level {l}: difference to the background alone"),
            worst,
            1e-6,
        );
    }
    c.finish();
}

// ---- the acceptance T and the printed numbers ------------------------------------------------------------------------------

/// ADR 0246 T (the acceptance of F-334), C1, and the numbers of Hb. Run once (952 s): C1 met at every level; T missed, the
/// orders of the root mean square of the calculated component cells are 1.277, 1.166 and 1.626 (`1.5486e-4`, `6.3902e-5`,
/// `3.9830e-5`, `2.4949e-5` at levels 1 to 4) against the limit 1.7. Kept with the limit intact (ADR 0246 amendment 1).
#[test]
#[ignore = "known miss, ADR 0246 amendment 1: T orders of the CC root mean square of RL 1.277, 1.166, 1.626 (limit 1.7); slow"]
fn t_rl_levels_1_to_4_known_miss() {
    let mut c = Checks::new();
    let mut rows: Vec<(usize, f64, Detail)> = Vec::new();
    for l in [1usize, 2, 3, 4] {
        let r = rl(l, l, |_| {});
        let dt = report(&format!("T RL level {l}"), &r);
        c.at_most(&format!("C1 RL level {l} continuity"), r.cont, 1e-9);
        rows.push((l, cc_error(&r, false).0, dt));
    }
    println!(
        "T: e_4 = {:.4e} against the prediction 2.86e-5 of the fit of ADR 0232",
        rows[3].1
    );
    println!(
        "T: b of the levels 1 and 2: {:.4e}; of the levels 2 and 4: {:.4e}",
        b_hat(rows[0].1, rows[1].1),
        2.0 * b_hat(rows[1].1, rows[3].1)
    );
    for w in rows.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let k = b.0 as f64 / a.0 as f64;
        println!(
            "Hb levels {} to {}: order of D_CF {:.3}, of D_BF {:.3}, of D_CC {:.3}; of the CC pressure error {:.3}; of the component pressure jump (rms) {:.3}",
            a.0,
            b.0,
            order(a.2.d[0], b.2.d[0], k),
            order(a.2.d[1], b.2.d[1], k),
            order(a.2.d[2], b.2.d[2], k),
            order(a.2.p_rms[1], b.2.p_rms[1], k),
            order(a.2.jump[0].rms(), b.2.jump[0].rms(), k)
        );
    }
    for w in rows.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        c.at_least(
            &format!("T order of the CC rms, levels {} to {}", a.0, b.0),
            order(a.1, b.1, b.0 as f64 / a.0 as f64),
            1.7,
        );
    }
    c.finish();
}

// ---- the switches at levels 1 and 2 ----------------------------------------------------------------------------------------

/// `b^` of a variant: its runs at levels 1 and 2, reported.
fn pair(tag: &str, f: impl Fn(&mut Cfg) + Copy, c: &mut Checks) -> (f64, [Run; 2]) {
    let runs = [1usize, 2].map(|l| {
        let r = rl(l, l, f);
        report(&format!("{tag} level {l}"), &r);
        c.at_most(&format!("C1 {tag} level {l} continuity"), r.cont, 1e-9);
        r
    });
    let b = b_hat(cc_error(&runs[0], false).0, cc_error(&runs[1], false).0);
    println!("{tag}: b^ = {b:.4e}");
    (b, runs)
}

/// ADR 0246: the switches of Hd, Ht, Hs, Hc, Hq, Hn, Ha (a3), the control ND, C1 and C2 (the hypotheses are decided in the
/// ADR by the printed numbers).
#[test]
#[ignore = "slow (RL at levels 1 and 2 in eight variants, ADR 0246, minutes): run once for the addendum"]
fn s_switches_levels_1_and_2() {
    let mut c = Checks::new();
    let (b0, base) = pair("S RL", |_| {}, &mut c);
    for (l, r) in [1usize, 2].iter().zip(&base) {
        c.at_most(
            &format!("C2 level {l}: error of the calculated cells at t = 0"),
            r.t0_error,
            1e-14,
        );
    }
    // Hd: the exact cell mean as the reference
    let b_mean = b_hat(cc_error(&base[0], true).0, cc_error(&base[1], true).0);
    println!("Hd: b^ with the cell mean {b_mean:.4e}; rho_mean {:.4}", b_mean / b0);
    // Ht: twice the steps
    for (l, r) in [1usize, 2].iter().zip(&base) {
        let fine = rl(*l, *l, |cfg| cfg.steps *= 2);
        c.at_most(&format!("C1 Ht level {l} continuity"), fine.cont, 1e-9);
        let tau = difference(r, &fine).0;
        println!(
            "Ht level {l}: tau {tau:.4e}; 2 tau {:.4e} against b0 / L {:.4e}: ratio {:.4}; CC rms with twice the steps {:.4e}",
            2.0 * tau,
            b0 / *l as f64,
            2.0 * tau * *l as f64 / b0,
            cc_error(&fine, false).0
        );
    }
    // Hs: the background alone over the calculated region of the component
    let mut es = Vec::new();
    for l in [1usize, 2] {
        let (geom, u) = background_alone(l, 200 * l);
        let reach = 7.5 * H1 - 0.75 * H1 / l as f64;
        let scale = (-2.0 * NU_TG * T_END).exp();
        let mut acc = Acc::default();
        for (cell, x) in geom.mesh.cell_centres().iter().enumerate() {
            if (x[0] - 16.5 * H1).abs().max((x[1] - 16.5 * H1).abs()) < reach {
                acc.add(vec3::norm(vec3::sub(u[cell], tg_u(*x, T_END, 1.0))) / scale);
            }
        }
        println!(
            "Hs level {l}: the background alone over the calculated region of the component: rms {:.4e} ({} cells)",
            acc.rms(),
            acc.n
        );
        es.push(acc.rms());
    }
    let bs = b_hat(es[0], es[1]);
    println!("Hs: b^_s {bs:.4e}; over b0 {:.4}", bs / b0);
    // Hc: the conditions of the component's outer patches
    let fixed = rl(1, 1, |cfg| cfg.fixed_outer = true);
    println!(
        "Hc level 1: largest difference of u with a fixed value zero on the outer patches {:.3e}",
        difference(&base[0], &fixed).1 * (-2.0 * NU_TG * T_END).exp()
    );
    // Ha (a3): the reference cell
    let (b_pin, _) = pair("S pin", |cfg| cfg.pin_last = true, &mut c);
    println!("Ha (a3): rho_pin {:.4}", b_pin / b0);
    // Hn: the amplitude
    let (b_amp, _) = pair("S amplitude 1/2", |cfg| cfg.amp = 0.5, &mut c);
    println!("Hn: rho_A {:.4}", b_amp / b0);
    // Hq: the nine-cell stencils
    for (tag, cf, bf) in [("Qc", true, false), ("Qb", false, true), ("Qcb", true, true)] {
        let (b, runs) = pair(
            &format!("S {tag}"),
            |cfg| cfg.stencils = Stencils::Quadratic { cf, bf },
            &mut c,
        );
        let bad: usize = runs.iter().map(|r| r.bad_stencil_cells).sum();
        c.holds(
            &format!("{tag}: every stencil cell calculated"),
            bad == 0,
            &format!("{bad} that are not"),
        );
        println!("Hq: rho_{tag} {:.4}", b / b0);
    }
    // ND: the donor cell only
    let (b_donor, _) = pair("S donor", |cfg| cfg.stencils = Stencils::Donor, &mut c);
    c.at_least("ND (negative control) rho_donor", b_donor / b0, 3.0);
    c.finish();
}

// ---- Hf: the two parts at different levels ---------------------------------------------------------------------------------

/// ADR 0246 Hf: the families B2 and C2 (the hypothesis is decided in the ADR by the printed numbers).
#[test]
#[ignore = "slow (RL with the parts at different levels up to level 4, ADR 0246, minutes): run once for the addendum"]
fn f_families_of_different_levels() {
    let mut c = Checks::new();
    let mut e = |lb: usize, lc: usize| {
        let r = rl(lb, lc, |_| {});
        report(&format!("F RL({lb},{lc})"), &r);
        c.at_most(&format!("C1 RL({lb},{lc}) continuity"), r.cont, 1e-9);
        cc_error(&r, false).0
    };
    let b0 = b_hat(e(1, 1), e(2, 2));
    let b_b2 = b_hat(e(2, 1), e(4, 2));
    let b_c2 = b_hat(e(1, 2), e(2, 4));
    let (bb, bc) = (2.0 * (b0 - b_b2), 2.0 * (b0 - b_c2));
    println!(
        "Hf: b0 {b0:.4e}; b^_B2 {b_b2:.4e}; b^_C2 {b_c2:.4e}; b_B {bb:.4e}; b_C {bc:.4e}; s_C {:.4}; k {:.4}",
        bc / (bb + bc),
        (b_b2 + b_c2) / (1.5 * b0)
    );
    c.finish();
}

// ---- amendment 1: the response to the truncation of the linear interpolation -------------------------------------------------

/// Ring of every calculated component cell: 1 with a fringe face neighbour, `n + 1` with a face neighbour in ring `n`
/// (0 for every other cell); as `overset_f314.rs`.
fn rings(s: &OversetSolver) -> Vec<usize> {
    let n = s.geom.n_cells();
    let nb = s.conn.n_background_cells;
    let ov = &s.coupling.as_ref().unwrap().overset;
    let calc = |c: usize| c >= nb && s.conn.kind[c] == CellKind::Calculated;
    let mut ring = vec![0usize; n];
    let mut front: Vec<usize> = (nb..n)
        .filter(|&c| calc(c) && ov.neighbours(c).iter().any(|&q| s.conn.kind[q] == CellKind::Fringe))
        .collect();
    let mut k = 1;
    while !front.is_empty() {
        for &c in &front {
            ring[c] = k;
        }
        let mut next = Vec::new();
        for &c in &front {
            for &q in ov.neighbours(c) {
                if calc(q) && ring[q] == 0 && !next.contains(&q) {
                    next.push(q);
                }
            }
        }
        k += 1;
        front = next;
    }
    ring
}

/// The response `R = u_base - u_variant` of the calculated component cells (amendment 1): its root mean square at every
/// snapshot time (over the decay factor of that time), and at `t = 0.5` its mean vector, its root mean square over the
/// rings 1 and 2 and over the rings from 4 on, and its cosine with the error of the variant. Returns the root mean square
/// at `t = 0.5`.
fn response(tag: &str, base: &Run, var: &Run) -> f64 {
    let s = &base.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let ring = rings(s);
    assert_eq!(
        base.snaps.len(),
        SNAP_TIMES.len(),
        "a snapshot time is not a whole number of steps"
    );
    assert_eq!(
        var.snaps.len(),
        SNAP_TIMES.len(),
        "a snapshot time is not a whole number of steps"
    );
    let mut hist = Vec::new();
    for (k, t) in SNAP_TIMES.iter().enumerate() {
        let scale = (-2.0 * NU_TG * t).exp();
        let (mut sum, mut vsum) = (0.0, 0.0);
        for c in nb..s.geom.n_cells() {
            if ring[c] > 0 {
                let d = vec3::sub(base.snaps[k][c], var.snaps[k][c]);
                sum += vol[c] * vec3::dot(d, d);
                vsum += vol[c];
            }
        }
        hist.push((sum / vsum).sqrt() / scale);
    }
    let scale = (-2.0 * NU_TG * T_END).exp();
    let (mut mean, mut vsum) = ([0.0; 3], 0.0);
    let (mut near, mut inner) = (Acc::default(), Acc::default());
    let (mut re, mut rr, mut ee) = (0.0, 0.0, 0.0);
    for c in nb..s.geom.n_cells() {
        if ring[c] == 0 {
            continue;
        }
        let r = vec3::scale(vec3::sub(s.u.values[c], var.s.u.values[c]), 1.0 / scale);
        let e = vec3::scale(vec3::sub(var.s.u.values[c], tg_u(centres[c], T_END, 1.0)), 1.0 / scale);
        mean = vec3::add(mean, vec3::scale(r, vol[c]));
        vsum += vol[c];
        re += vol[c] * vec3::dot(r, e);
        rr += vol[c] * vec3::dot(r, r);
        ee += vol[c] * vec3::dot(e, e);
        if ring[c] <= 2 {
            near.add(vec3::norm(r));
        } else if ring[c] >= 4 {
            inner.add(vec3::norm(r));
        }
    }
    println!(
        "{tag}: rms of R at t = 0.0025, 0.025, 0.125, 0.25, 0.5: {}; mean vector ({:.3e}, {:.3e}); rings 1 and 2 {:.4e}, rings from 4 on {:.4e}; cosine with the error of the variant {:.4}",
        hist.iter().map(|h| format!("{h:.4e}")).collect::<Vec<_>>().join(", "),
        mean[0] / vsum,
        mean[1] / vsum,
        near.rms(),
        inner.rms(),
        re / (rr * ee).sqrt()
    );
    hist[4]
}

/// Amendment 1 at one level: RL with the library stencils and with the nine-cell stencils of `variants`; the errors with
/// the centre and with the cell mean as the reference, and the responses. Returns the root mean square of the error of
/// the last variant and of its response.
fn a1_level(l: usize, variants: &[(&str, bool, bool)], c: &mut Checks) -> (f64, f64) {
    let base = rl(l, l, |_| {});
    report(&format!("A1 RL level {l}"), &base);
    println!(
        "A1 level {l}: base CC rms {:.4e}, with the cell mean {:.4e}",
        cc_error(&base, false).0,
        cc_error(&base, true).0
    );
    let mut last = (0.0, 0.0);
    for &(tag, cf, bf) in variants {
        let r = rl(l, l, |cfg| cfg.stencils = Stencils::Quadratic { cf, bf });
        report(&format!("A1 {tag} level {l}"), &r);
        c.at_most(&format!("C1 A1 {tag} level {l} continuity"), r.cont, 1e-9);
        c.holds(
            &format!("A1 {tag} level {l}: every stencil cell calculated"),
            r.bad_stencil_cells == 0,
            &format!("{} that are not", r.bad_stencil_cells),
        );
        let e = cc_error(&r, false).0;
        println!(
            "A1 level {l}: {tag} CC rms {e:.4e}, with the cell mean {:.4e}",
            cc_error(&r, true).0
        );
        last = (e, response(&format!("A1 level {l}: R of {tag}"), &base, &r));
    }
    println!("A1 level {l}: RESULT e_Q {:.6e} R {:.6e}", last.0, last.1);
    last
}

const A1_ALL: [(&str, bool, bool); 3] = [("Qc", true, false), ("Qb", false, true), ("Qcb", true, true)];

/// ADR 0246 amendment 1, levels 1 and 2: TQ and H12 on the first pair of levels.
#[test]
#[ignore = "slow (RL at levels 1 and 2 with three stencil variants, ADR 0246 amendment 1, minutes): run once"]
fn a1_response_levels_1_and_2() {
    let mut c = Checks::new();
    let a = a1_level(1, &A1_ALL, &mut c);
    let b = a1_level(2, &A1_ALL, &mut c);
    println!("A1 levels 1 to 2: order of R {:.4}", order(a.1, b.1, 2.0));
    c.at_least(
        "TQ order of the CC rms with the nine-cell stencils, levels 1 to 2",
        order(a.0, b.0, 2.0),
        1.7,
    );
    c.finish();
}

/// ADR 0246 amendment 1, level 3 (the orders against the levels 2 and 4 are formed in the ADR from the printed numbers).
#[test]
#[ignore = "slow (RL at level 3 with three stencil variants, ADR 0246 amendment 1, about fifteen minutes): run once"]
fn a1_response_level_3() {
    let mut c = Checks::new();
    a1_level(3, &A1_ALL, &mut c);
    c.finish();
}

/// ADR 0246 amendment 1, level 4 (the nine-cell stencils on both fringes only).
#[test]
#[ignore = "slow (RL at level 4 twice, ADR 0246 amendment 1, about half an hour): run once"]
fn a1_response_level_4() {
    let mut c = Checks::new();
    a1_level(4, &A1_ALL[2..], &mut c);
    c.finish();
}

// ---- amendment 2: the first-order part of the response ------------------------------------------------------------------------

/// Root mean square over the calculated component cells of the difference of snapshot `k` of two runs (over the decay
/// factor of the time `t`), and of the error of the first run at that time.
fn snapshot_response(a: &Run, b: &Run, k: usize, t: f64) -> (f64, f64) {
    let s = &a.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let scale = (-2.0 * NU_TG * t).exp();
    let (mut sum, mut err, mut vsum) = (0.0, 0.0, 0.0);
    for c in nb..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Calculated {
            let d = vec3::sub(a.snaps[k][c], b.snaps[k][c]);
            let e = vec3::sub(a.snaps[k][c], tg_u(centres[c], t, 1.0));
            sum += vol[c] * vec3::dot(d, d);
            err += vol[c] * vec3::dot(e, e);
            vsum += vol[c];
        }
    }
    ((sum / vsum).sqrt() / scale, (err / vsum).sqrt() / scale)
}

/// ADR 0246 amendment 2, H13: RL at levels 1 to 3 without the transient Rhie-Chow term, with the library stencils and
/// with the nine-cell stencils on the background fringe and on both fringes (printed numbers; decided in the ADR).
#[test]
#[ignore = "slow (RL at levels 1 to 3 in three variants, ADR 0246 amendment 2, about twenty minutes): run once"]
fn a2_without_the_transient_rhie_chow_term() {
    let mut c = Checks::new();
    let mut rows = Vec::new();
    for l in [1usize, 2, 3] {
        let base = rl(l, l, |cfg| cfg.ddt_correction = false);
        report(&format!("A2 N RL level {l}"), &base);
        c.at_most(&format!("C1 A2 N level {l} continuity"), base.cont, 1e-9);
        let mut r = [0.0; 2];
        let mut e = [0.0; 2];
        for (k, (tag, cf, bf)) in [("Qb", false, true), ("Qcb", true, true)].into_iter().enumerate() {
            let v = rl(l, l, |cfg| {
                cfg.ddt_correction = false;
                cfg.stencils = Stencils::Quadratic { cf, bf };
            });
            c.at_most(&format!("C1 A2 N {tag} level {l} continuity"), v.cont, 1e-9);
            e[k] = cc_error(&v, false).0;
            r[k] = response(&format!("A2 N level {l}: R of {tag}"), &base, &v);
        }
        println!(
            "A2 N level {l}: RESULT e_base {:.6e} e_Qb {:.6e} e_Qcb {:.6e} R_b {:.6e} R_cb {:.6e}",
            cc_error(&base, false).0,
            e[0],
            e[1],
            r[0],
            r[1]
        );
        rows.push((cc_error(&base, false).0, e[1], r[0], r[1]));
    }
    for (a, b, k) in [(0usize, 1usize, 2.0f64), (1, 2, 1.5)] {
        println!(
            "A2 N levels {} to {}: order of e_base {:.4}, of e_Qcb {:.4}, of R_b {:.4}, of R_cb {:.4}",
            a + 1,
            b + 1,
            order(rows[a].0, rows[b].0, k),
            order(rows[a].1, rows[b].1, k),
            order(rows[a].2, rows[b].2, k),
            order(rows[a].3, rows[b].3, k)
        );
    }
    println!(
        "A2 N: beta^ = 9 R_3 - 4 R_2: of R_b {:.4e}, of R_cb {:.4e}",
        9.0 * rows[2].2 - 4.0 * rows[1].2,
        9.0 * rows[2].3 - 4.0 * rows[1].3
    );
    c.finish();
}

/// ADR 0246 amendment 2: RL at levels 1 and 2 run on to `t = 1` (printed numbers: whether the response keeps growing).
#[test]
#[ignore = "slow (RL at levels 1 and 2 to t = 1 in three variants, ADR 0246 amendment 2, minutes): run once"]
fn a2_run_on_to_t_1() {
    let mut c = Checks::new();
    for l in [1usize, 2] {
        let long = |cf: bool, bf: bool| {
            rl(l, l, |cfg| {
                cfg.end = 1.0;
                cfg.steps *= 2;
                if cf || bf {
                    cfg.stencils = Stencils::Quadratic { cf, bf };
                }
            })
        };
        let base = long(false, false);
        let qb = long(false, true);
        let qcb = long(true, true);
        for r in [&base, &qb, &qcb] {
            c.at_most(&format!("C1 A2 long level {l} continuity"), r.cont, 1e-9);
        }
        for (k, t) in SNAP_TIMES.iter().chain(&LATE_TIMES).enumerate() {
            let (rb, eb) = snapshot_response(&base, &qb, k, *t);
            let (rcb, _) = snapshot_response(&base, &qcb, k, *t);
            let (_, eq) = snapshot_response(&qcb, &base, k, *t);
            println!("A2 long level {l} t = {t}: e_base {eb:.4e}, e_Qcb {eq:.4e}, R_b {rb:.4e}, R_cb {rcb:.4e}");
        }
    }
    c.finish();
}

// ---- the gate-run check -----------------------------------------------------------------------------------------------------

/// ADR 0246 C1 and C2 on RL at level 1 (the run of T and of the switches at that level, which is deterministic; it is the
/// check that the mutation MU1 must fail).
#[test]
fn c_rl_level_1_continuity_and_start() {
    let mut c = Checks::new();
    let r = rl(1, 1, |_| {});
    c.at_most("C1 RL level 1 continuity", r.cont, 1e-9);
    c.at_most("C2 level 1: error of the calculated cells at t = 0", r.t0_error, 1e-14);
    println!("RL level 1: CC rms {:.4e} (ADR 0246: 1.5486e-4)", cc_error(&r, false).0);
    c.finish();
}
