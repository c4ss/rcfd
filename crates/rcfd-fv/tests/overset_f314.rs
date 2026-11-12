//! F-314 (ADR 0232): diagnosis of the observed-order misses of the overset coupling (T1, T2 at level 2 and T1c of
//! ADR 0219). The set-ups are the Taylor-Green runs of `overset_f134.rs` (ADR 0219 TG_L), here on three levels with the
//! step proportional to `h^2`, with the error split by cell class and, in the fringe cells, into the truncation of the
//! donor interpolation and the carried error of the donors. No library code is changed by these tests.
//!
//! Release only (the flow runs take minutes): `cargo test -p rcfd-fv --release --test overset_f314`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, containment_margin, CellKind, Connectivity, Overset, OversetSpec, Stencil};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
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

// ---- the set-up TG_L of ADR 0219 ---------------------------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn tg_overset_mesh(level: usize) -> Overset {
    overset_mesh(level, 0.6, 0.25)
}

/// The composite of level `level` with a component of half width `half` and the hole `max(|x_ref|, |y_ref|) < hole`.
fn overset_mesh(level: usize, half: f64, hole: f64) -> Overset {
    let bs = uniform(0.0, PI, 40 * level);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let cs = uniform(-half, 2.0 * half, 20 * level);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

/// The motion of the component: as registered by ADR 0219, at half its speed, or held at the start pose.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Motion {
    Full,
    Half,
    Rest,
}

fn pose_of(m: Motion, t: f64) -> Pose {
    match m {
        Motion::Full => tg_pose(t),
        Motion::Half => tg_pose(0.5 * t),
        Motion::Rest => tg_pose(0.0),
    }
}

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
}

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

/// Observed order between two errors on meshes whose spacings have the ratio `ratio` (coarse over fine).
fn order(coarse: f64, fine: f64, ratio: f64) -> f64 {
    (coarse / fine).ln() / ratio.ln()
}

// ---- kinematic measurements (no flow solve) ------------------------------------------------------------------------------

/// Truncation of the stencil of fringe cell `c` for the exact velocity at `t = 0`: `sum_k w_k u_ex(x_k) - u_ex(x_c)`.
fn truncation(centres: &[Vec3], c: usize, st: &Stencil, t: f64) -> Vec3 {
    let mut v = vec3::scale(tg_u(centres[c], t), -1.0);
    for (&k, &w) in st.cells.iter().zip(&st.weights) {
        v = vec3::add(v, vec3::scale(tg_u(centres[k], t), w));
    }
    v
}

/// One pose of one level: the largest truncation over the component fringe and the background fringe cells with the
/// registered stencils, the same with the donor cell alone, and the G1 census (stencils that are not five cells, donors
/// that do not contain the fringe centre, the smallest and the largest weight).
struct PoseStats {
    i_cf: f64,
    i_bf: f64,
    i_cf_donor: f64,
    i_bf_donor: f64,
    not_five: usize,
    not_contained: usize,
    w_min: f64,
    w_max: f64,
}

fn pose_stats(ov: &Overset, pose: &Pose) -> PoseStats {
    let mesh = mesh_at(ov, pose);
    let conn: Connectivity = ov.connectivity(&mesh, pose).unwrap();
    let centres = mesh.cell_centres();
    let vol = mesh.cell_volumes();
    let nb = conn.n_background_cells;
    let mut s = PoseStats {
        i_cf: 0.0,
        i_bf: 0.0,
        i_cf_donor: 0.0,
        i_bf_donor: 0.0,
        not_five: 0,
        not_contained: 0,
        w_min: f64::INFINITY,
        w_max: f64::NEG_INFINITY,
    };
    for (c, st) in &conn.stencils {
        let i = vec3::norm(truncation(centres, *c, st, 0.0));
        let donor = Stencil {
            cells: vec![st.cells[0]],
            weights: vec![1.0],
        };
        let id = vec3::norm(truncation(centres, *c, &donor, 0.0));
        if *c < nb {
            s.i_bf = s.i_bf.max(i);
            s.i_bf_donor = s.i_bf_donor.max(id);
        } else {
            s.i_cf = s.i_cf.max(i);
            s.i_cf_donor = s.i_cf_donor.max(id);
        }
        if st.cells.len() != 5 {
            s.not_five += 1;
        }
        let d = st.cells[0];
        let size = (vol[d] / DEPTH).sqrt();
        if containment_margin(&mesh, d, centres[*c]) < -1e-10 * size {
            s.not_contained += 1;
        }
        for &w in &st.weights {
            s.w_min = s.w_min.min(w);
            s.w_max = s.w_max.max(w);
        }
    }
    s
}

/// The 200 poses of the registered motion, `tg_pose(k * 0.0025)`.
fn poses() -> Vec<Pose> {
    (1..=200).map(|k| tg_pose(k as f64 * 0.0025)).collect()
}

/// Largest second difference of the exact velocity along the paths of the component cells of level 2, `|g(t + dt) - 2 g(t) +
/// g(t - dt)| / dt^2` with `g(t) = u_ex(x_c(t), t)`, over the cells and the steps `t = k dt`, `k = 1..199`, `dt = 0.0025`
/// (twice the local truncation of backward Euler per unit `dt`), and its root mean square.
fn path_second_difference(ov: &Overset, m: Motion) -> (f64, f64) {
    let nb = ov.composite.n_background_cells;
    let refc = &ov.composite.mesh.cell_centres()[nb..];
    let dt = 0.0025;
    let g = |x: Vec3, t: f64| tg_u(pose_of(m, t).apply(x), t);
    let (mut worst, mut sum, mut count) = (0.0f64, 0.0, 0usize);
    for k in 1..200 {
        let t = k as f64 * dt;
        for &x in refc {
            let (a, b, c) = (g(x, t - dt), g(x, t), g(x, t + dt));
            let d = vec3::norm(vec3::add(vec3::sub(a, vec3::scale(b, 2.0)), c)) / (dt * dt);
            worst = worst.max(d);
            sum += d * d;
            count += 1;
        }
    }
    (worst, (sum / count as f64).sqrt())
}

/// ADR 0232 G1, G2, G3 and the numbers of H2 (kinematic, no flow solve).
#[test]
fn g_kinematic_stencils_and_truncation_order() {
    let t0 = Instant::now();
    let ps = poses();
    let levels = [1usize, 2, 4];
    let stats: Vec<Vec<PoseStats>> = levels
        .iter()
        .map(|&l| {
            let ov = tg_overset_mesh(l);
            ps.iter().map(|p| pose_stats(&ov, p)).collect()
        })
        .collect();
    let mut c = Checks::new();
    let sup = |v: &[PoseStats], f: &dyn Fn(&PoseStats) -> f64| v.iter().map(f).fold(0.0, f64::max);
    let mut i_sup = Vec::new();
    let mut i_sup_donor = Vec::new();
    for (l, v) in levels.iter().zip(&stats) {
        let not_five: usize = v.iter().map(|s| s.not_five).sum();
        let not_in: usize = v.iter().map(|s| s.not_contained).sum();
        let w_min = v.iter().map(|s| s.w_min).fold(f64::INFINITY, f64::min);
        let w_max = v.iter().map(|s| s.w_max).fold(f64::NEG_INFINITY, f64::max);
        c.holds(
            &format!("G1 level {l} stencils of five cells"),
            not_five == 0,
            &format!("{not_five} stencils that are not five cells"),
        );
        c.holds(
            &format!("G1 level {l} donors contain the fringe centre"),
            not_in == 0,
            &format!("{not_in} donors that do not"),
        );
        c.at_least(&format!("G1 level {l} smallest weight"), w_min, -0.05);
        c.at_most(&format!("G1 level {l} largest weight"), w_max, 1.0 + 1e-12);
        let (cf, bf) = (sup(v, &|s| s.i_cf), sup(v, &|s| s.i_bf));
        let (cfd, bfd) = (sup(v, &|s| s.i_cf_donor), sup(v, &|s| s.i_bf_donor));
        println!(
            "G level {l}: I_sup component fringe {cf:.4e}, background fringe {bf:.4e}; donor cell only {cfd:.4e}, {bfd:.4e}"
        );
        i_sup.push(cf.max(bf));
        i_sup_donor.push(cfd.max(bfd));
    }
    c.at_least("G2 order of I_sup, levels 1 to 2", order(i_sup[0], i_sup[1], 2.0), 1.8);
    c.at_least("G2 order of I_sup, levels 2 to 4", order(i_sup[1], i_sup[2], 2.0), 1.8);
    c.at_most(
        "G3 (negative control) donor cell only, levels 1 to 2",
        order(i_sup_donor[0], i_sup_donor[1], 2.0),
        1.3,
    );
    c.at_most(
        "G3 (negative control) donor cell only, levels 2 to 4",
        order(i_sup_donor[1], i_sup_donor[2], 2.0),
        1.3,
    );
    // H2: the single-pose order of the largest truncation over the component fringe, levels 1 to 2
    let mut o: Vec<f64> = stats[0]
        .iter()
        .zip(&stats[1])
        .map(|(a, b)| order(a.i_cf, b.i_cf, 2.0))
        .collect();
    let end = o[199];
    let below = o.iter().filter(|&&x| x < 1.7).count();
    o.sort_by(f64::total_cmp);
    println!(
        "H2: single-pose order of max_CF |I|, levels 1 to 2, 200 poses: smallest {:.3}, median {:.3}, largest {:.3}; {} of 200 below 1.7; end pose {:.3} (max_CF |I| {:.4e}, {:.4e})",
        o[0],
        0.5 * (o[99] + o[100]),
        o[199],
        below,
        end,
        stats[0][199].i_cf,
        stats[1][199].i_cf
    );
    let mut o24: Vec<f64> = stats[1]
        .iter()
        .zip(&stats[2])
        .map(|(a, b)| order(a.i_cf, b.i_cf, 2.0))
        .collect();
    let below24 = o24.iter().filter(|&&x| x < 1.7).count();
    o24.sort_by(f64::total_cmp);
    println!(
        "H2 (printed): the same, levels 2 to 4: smallest {:.3}, median {:.3}, largest {:.3}; {} of 200 below 1.7",
        o24[0],
        0.5 * (o24[99] + o24[100]),
        o24[199],
        below24
    );
    // H6: the kinematic ratio rho_BE
    let ov2 = tg_overset_mesh(2);
    let (full, full_rms) = path_second_difference(&ov2, Motion::Full);
    let (half, half_rms) = path_second_difference(&ov2, Motion::Half);
    let (rest, _) = path_second_difference(&ov2, Motion::Rest);
    println!(
        "H6 (kinematic): largest |d2u/dt2| along the component cell paths: full speed {full:.4e}, half speed {half:.4e}, at rest {rest:.4e}; rho_BE {:.4} (root mean square {:.4})",
        full / half,
        full_rms / half_rms
    );
    println!("G: {:.1} s", t0.elapsed().as_secs_f64());
    c.finish();
}

// ---- flow runs -------------------------------------------------------------------------------------------------------------

/// One Taylor-Green run: the solver at `t = 0.5`, the window maximum of F4 (the largest error of a non-hole cell over the
/// steps with `t >= 0.25`), the largest `abs(delta)` and continuity over the steps, the step size and the wall time.
struct Run {
    level: usize,
    dt: f64,
    s: OversetSolver,
    window: f64,
    /// The same over the calculated cells only (amendment 1, H8c).
    window_calc: f64,
    delta: f64,
    cont: f64,
    secs: f64,
}

fn eps(s: &OversetSolver, c: usize, t: f64) -> f64 {
    vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], t))) / (-2.0 * NU_TG * t).exp()
}

/// TG_L of ADR 0219 in `steps` steps to `t = 0.5` with the motion `motion`; `donor_only` replaces every stencil by its donor
/// cell with weight 1 after every `prepare` (N5).
fn tg_run(level: usize, steps: usize, motion: Motion, donor_only: bool) -> Run {
    run_on(
        tg_overset_mesh(level),
        level,
        steps,
        &|t| pose_of(motion, t),
        donor_only,
    )
}

/// A Taylor-Green run on the composite `ov` of level `level` with the pose `pose_at(t)`.
fn run_on(ov: Overset, level: usize, steps: usize, pose_at: &dyn Fn(f64) -> Pose, donor_only: bool) -> Run {
    let t0 = Instant::now();
    let dt = T_END / steps as f64;
    let pose = pose_at(0.0);
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh_at(&ov, &pose))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let (mut window, mut window_calc, mut delta, mut cont) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let mut st = s.prepare(dt, pose_at(t), &[]).unwrap();
        if donor_only {
            for (_, stn) in st.conn.stencils.iter_mut() {
                *stn = Stencil {
                    cells: vec![stn.cells[0]],
                    weights: vec![1.0],
                };
            }
        }
        let info = s.advance(dt, st).unwrap();
        cont = cont.max(info.continuity);
        delta = delta.max(info.delta.abs());
        if t >= 0.25 - 1e-12 {
            for c in 0..s.geom.n_cells() {
                if s.conn.kind[c] != CellKind::Hole {
                    let e = eps(&s, c, t);
                    window = window.max(e);
                    if s.conn.kind[c] == CellKind::Calculated {
                        window_calc = window_calc.max(e);
                    }
                }
            }
        }
    }
    Run {
        level,
        dt,
        s,
        window,
        window_calc,
        delta,
        cont,
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

/// The errors of a run at `t = 0.5`.
struct Stats {
    /// Largest error over the non-hole cells (the error of T of ADR 0219) and its cell.
    e_all: f64,
    arg_all: usize,
    /// Root mean square over the non-hole cells, weighted with the cell volumes (F2).
    rms_all: f64,
    /// Largest error over the calculated cells and its cell (F3); whether that cell has a fringe face neighbour.
    e_calc: f64,
    arg_calc: usize,
    calc_adjacent: bool,
    /// Largest and root-mean-square error per class.
    class_max: [f64; 4],
    class_rms: [f64; 4],
    /// F1: largest `|u_c - sum w_k u_k|` over the fringe cells.
    identity: f64,
    /// Largest truncation and carried donor error over the component fringe and the background fringe cells.
    i_max: [f64; 2],
    d_max: [f64; 2],
    /// Share of the truncation in the error of the cell `arg_all` (when it is a fringe cell), with `|I|` and `|D|` there.
    s_i: Option<(f64, f64, f64)>,
}

fn stats(r: &Run) -> Stats {
    let s = &r.s;
    let n = s.geom.n_cells();
    let nb = s.conn.n_background_cells;
    let centres = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let decay = (-2.0 * NU_TG * T_END).exp();
    let ov = &s.coupling.as_ref().unwrap().overset;
    let mut st = Stats {
        e_all: 0.0,
        arg_all: 0,
        rms_all: 0.0,
        e_calc: 0.0,
        arg_calc: 0,
        calc_adjacent: false,
        class_max: [0.0; 4],
        class_rms: [0.0; 4],
        identity: 0.0,
        i_max: [0.0; 2],
        d_max: [0.0; 2],
        s_i: None,
    };
    let (mut sum, mut vsum) = (0.0, 0.0);
    let (mut csum, mut cvol) = ([0.0; 4], [0.0; 4]);
    for c in 0..n {
        let Some(k) = class_of(s.conn.kind[c], c < nb) else {
            continue;
        };
        let e = eps(s, c, T_END);
        if e > st.e_all {
            st.e_all = e;
            st.arg_all = c;
        }
        if s.conn.kind[c] == CellKind::Calculated && e > st.e_calc {
            st.e_calc = e;
            st.arg_calc = c;
        }
        st.class_max[k] = st.class_max[k].max(e);
        sum += vol[c] * e * e;
        vsum += vol[c];
        csum[k] += vol[c] * e * e;
        cvol[k] += vol[c];
    }
    st.rms_all = (sum / vsum).sqrt();
    for k in 0..4 {
        st.class_rms[k] = (csum[k] / cvol[k]).sqrt();
    }
    st.calc_adjacent = ov
        .neighbours(st.arg_calc)
        .iter()
        .any(|&q| s.conn.kind[q] == CellKind::Fringe);
    for (c, stn) in &s.conn.stencils {
        let mut interp = [0.0; 3];
        for (&k, &w) in stn.cells.iter().zip(&stn.weights) {
            interp = vec3::add(interp, vec3::scale(s.u.values[k], w));
        }
        st.identity = st.identity.max(vec3::norm(vec3::sub(s.u.values[*c], interp)));
        let i = vec3::scale(truncation(centres, *c, stn, T_END), 1.0 / decay);
        let e = vec3::scale(vec3::sub(s.u.values[*c], tg_u(centres[*c], T_END)), 1.0 / decay);
        let d = vec3::sub(e, i);
        let part = usize::from(*c < nb);
        st.i_max[part] = st.i_max[part].max(vec3::norm(i));
        st.d_max[part] = st.d_max[part].max(vec3::norm(d));
        if *c == st.arg_all {
            st.s_i = Some((vec3::dot(e, i) / vec3::dot(e, e), vec3::norm(i), vec3::norm(d)));
        }
    }
    st
}

fn describe(s: &OversetSolver, c: usize) -> String {
    let nb = s.conn.n_background_cells;
    let x = s.geom.mesh.cell_centres()[c];
    let k = class_of(s.conn.kind[c], c < nb).map_or("hole", |k| CLASSES[k]);
    format!("cell {c} ({k}) at ({:.3}, {:.3})", x[0], x[1])
}

fn report(tag: &str, r: &Run, st: &Stats) {
    let h_b = PI / (40.0 * r.level as f64);
    println!(
        "{tag} level {} dt {:.6e} ({:.1} s): e {:.4e} in {}; rms {:.4e}; calculated max {:.4e} in {} (fringe-adjacent: {}); window max {:.4e}; continuity {:.2e}",
        r.level,
        r.dt,
        r.secs,
        st.e_all,
        describe(&r.s, st.arg_all),
        st.rms_all,
        st.e_calc,
        describe(&r.s, st.arg_calc),
        st.calc_adjacent,
        r.window,
        r.cont
    );
    println!(
        "{tag} level {} classes (max / rms): CF {:.4e} / {:.4e}; CC {:.4e} / {:.4e}; BF {:.4e} / {:.4e}; BC {:.4e} / {:.4e}",
        r.level,
        st.class_max[0],
        st.class_rms[0],
        st.class_max[1],
        st.class_rms[1],
        st.class_max[2],
        st.class_rms[2],
        st.class_max[3],
        st.class_rms[3]
    );
    println!(
        "{tag} level {} fringe split: component max |I| {:.4e}, max |D| {:.4e}; background max |I| {:.4e}, max |D| {:.4e}; identity {:.2e}; at the largest error: {}",
        r.level,
        st.i_max[0],
        st.d_max[0],
        st.i_max[1],
        st.d_max[1],
        st.identity,
        match st.s_i {
            Some((s, i, d)) => format!("s_I {s:.3}, |I| {i:.4e}, |D| {d:.4e}"),
            None => "not a fringe cell".to_string(),
        }
    );
    println!(
        "{tag} level {} delta: max |delta| {:.4e}, bound b = dt max|delta| / h_B {:.4e} ({:.3} of e)",
        r.level,
        r.delta,
        r.dt * r.delta / h_b,
        r.dt * r.delta / h_b / st.e_all
    );
}

/// The orders of F2, F3, F4 (and, printed, of the classes and of the T1c measure) between two levels.
fn orders(tag: &str, a: (&Run, &Stats), b: (&Run, &Stats)) -> (f64, f64, f64) {
    let ratio = b.0.level as f64 / a.0.level as f64;
    let o = |x: f64, y: f64| order(x, y, ratio);
    let (f2, f3, f4) = (
        o(a.1.rms_all, b.1.rms_all),
        o(a.1.e_calc, b.1.e_calc),
        o(a.0.window, b.0.window),
    );
    println!(
        "{tag} levels {} to {}: order of e (the T1c measure) {:.4}; rms {:.4}; calculated max {:.4}; window max {:.4}; max |delta| {:.4}",
        a.0.level,
        b.0.level,
        o(a.1.e_all, b.1.e_all),
        f2,
        f3,
        f4,
        o(a.0.delta, b.0.delta)
    );
    println!(
        "{tag} levels {} to {}: class orders max / rms: CF {:.3} / {:.3}; CC {:.3} / {:.3}; BF {:.3} / {:.3}; BC {:.3} / {:.3}; component max |I| {:.3}, max |D| {:.3}",
        a.0.level,
        b.0.level,
        o(a.1.class_max[0], b.1.class_max[0]),
        o(a.1.class_rms[0], b.1.class_rms[0]),
        o(a.1.class_max[1], b.1.class_max[1]),
        o(a.1.class_rms[1], b.1.class_rms[1]),
        o(a.1.class_max[2], b.1.class_max[2]),
        o(a.1.class_rms[2], b.1.class_rms[2]),
        o(a.1.class_max[3], b.1.class_max[3]),
        o(a.1.class_rms[3], b.1.class_rms[3]),
        o(a.1.i_max[0], b.1.i_max[0]),
        o(a.1.d_max[0], b.1.d_max[0])
    );
    (f2, f3, f4)
}

/// The runs of the levels `levels` with the step proportional to `h^2` (200 `L^2` steps), reported.
fn level_runs(tag: &str, levels: &[usize], donor_only: bool) -> Vec<(Run, Stats)> {
    levels
        .iter()
        .map(|&l| {
            let r = tg_run(l, 200 * l * l, Motion::Full, donor_only);
            let st = stats(&r);
            report(tag, &r, &st);
            (r, st)
        })
        .collect()
}

/// F2, F3 and F4 between two levels; `with_f3` false prints F3 without asserting it (levels 2 to 3: the known miss of
/// amendment 1).
fn check_pair(c: &mut Checks, tag: &str, a: &(Run, Stats), b: &(Run, Stats), with_f3: bool) {
    let (f2, f3, f4) = orders(tag, (&a.0, &a.1), (&b.0, &b.1));
    let pair = format!("levels {} to {}", a.0.level, b.0.level);
    c.at_least(&format!("F2 order of the root mean square, {pair}"), f2, 1.7);
    if with_f3 {
        c.at_least(&format!("F3 order of the calculated maximum, {pair}"), f3, 1.7);
    } else {
        println!(
            "F3 order of the calculated maximum, {pair} (printed, known miss of amendment 1): {f3:.4} (limit >= 1.7)"
        );
    }
    c.at_least(&format!("F4 order of the window maximum, {pair}"), f4, 1.7);
}

/// ADR 0232 F1 to F4 on levels 1 and 2 (the runs of T1c of ADR 0219).
#[test]
fn f_split_and_orders_levels_1_and_2() {
    let runs = level_runs("F", &[1, 2], false);
    let mut c = Checks::new();
    for (r, st) in &runs {
        c.at_most(&format!("F1 fringe identity, level {}", r.level), st.identity, 1e-12);
    }
    check_pair(&mut c, "F", &runs[0], &runs[1], true);
    c.finish();
}

/// ADR 0232 F1, F2 and F4 on levels 1, 2 and 3 (level 3: 1800 steps), and F3 at levels 1 to 2. F3 at levels 2 to 3 missed
/// (`f3_calculated_maximum_levels_2_to_3_known_miss`, amendment 1) and is printed.
#[test]
#[ignore = "slow (level 3 of ADR 0232, minutes): run once for the addendum"]
fn f_split_and_orders_three_levels() {
    let runs = level_runs("F", &[1, 2, 3], false);
    let mut c = Checks::new();
    for (r, st) in &runs {
        c.at_most(&format!("F1 fringe identity, level {}", r.level), st.identity, 1e-12);
    }
    check_pair(&mut c, "F", &runs[0], &runs[1], true);
    check_pair(&mut c, "F", &runs[1], &runs[2], false);
    c.finish();
}

/// ADR 0232 F3 at levels 2 to 3: known miss (registered run: order 1.580; the calculated maximum 2.096e-4 and 1.104e-4 lies
/// in a component calculated cell that is not adjacent to the fringe). Diagnosed by amendment 1.
#[test]
#[ignore = "known miss, ADR 0232 amendment 1: F3 order of the calculated maximum, levels 2 to 3, 1.580 (limit 1.7)"]
fn f3_calculated_maximum_levels_2_to_3_known_miss() {
    let runs = level_runs("F", &[2, 3], false);
    let (_, f3, _) = orders("F", (&runs[0].0, &runs[0].1), (&runs[1].0, &runs[1].1));
    let mut c = Checks::new();
    c.at_least("F3 order of the calculated maximum, levels 2 to 3", f3, 1.7);
    c.finish();
}

/// ADR 0232 N5 (negative control): the donor cell alone (first-order interpolation), levels 1 and 2: the parts that held
/// (the root-mean-square part did not: `n5_root_mean_square_control_known_miss`, amendment 1).
#[test]
fn n5_orders_with_the_donor_cell_only() {
    let runs = level_runs("N5", &[1, 2], true);
    let (f2, f3, f4) = orders("N5", (&runs[0].0, &runs[0].1), (&runs[1].0, &runs[1].1));
    println!("N5 order of the root mean square (printed, did not hold as registered): {f2:.4} (limit <= 1.3)");
    let mut c = Checks::new();
    c.at_most("N5 order of the calculated maximum", f3, 1.3);
    c.at_most("N5 order of the window maximum", f4, 1.3);
    c.finish();
}

/// ADR 0232 N5, the root-mean-square part: the control did not hold at its registered limit (registered run: 1.463, which is
/// still below the limit 1.7 of F2; amendment 1).
#[test]
#[ignore = "known miss, ADR 0232 amendment 1: N5 order of the root mean square with the donor cell only, 1.463 (limit <= 1.3)"]
fn n5_root_mean_square_control_known_miss() {
    let runs = level_runs("N5", &[1, 2], true);
    let (f2, ..) = orders("N5", (&runs[0].0, &runs[0].1), (&runs[1].0, &runs[1].1));
    let mut c = Checks::new();
    c.at_most("N5 order of the root mean square", f2, 1.3);
    c.finish();
}

/// Largest difference of the velocities of two runs of one level and one motion at `t = 0.5` over the cells of the classes
/// `classes`, over the decay, and its cell.
fn difference(a: &Run, b: &Run, classes: &[usize]) -> (f64, usize) {
    assert!(a.s.conn.kind == b.s.conn.kind, "the two runs end at different kinds");
    let nb = a.s.conn.n_background_cells;
    let decay = (-2.0 * NU_TG * T_END).exp();
    let mut best = (0.0, 0);
    for c in 0..a.s.geom.n_cells() {
        if class_of(a.s.conn.kind[c], c < nb).is_some_and(|k| classes.contains(&k)) {
            let d = vec3::norm(vec3::sub(a.s.u.values[c], b.s.u.values[c])) / decay;
            if d > best.0 {
                best = (d, c);
            }
        }
    }
    best
}

/// ADR 0232 F5, F6 and the numbers of H6: the time error of the moving cells at level 2.
#[test]
#[ignore = "slow (seven level-2 runs of ADR 0232, minutes): run once for the addendum"]
fn h6_time_error_of_the_moving_cells() {
    let all = [0usize, 1, 2, 3];
    let run = |steps: usize, m: Motion| {
        let r = tg_run(2, steps, m, false);
        let st = stats(&r);
        println!(
            "H6 {m:?} {steps} steps ({:.1} s): e {:.4e} in {}",
            r.secs,
            st.e_all,
            describe(&r.s, st.arg_all)
        );
        r
    };
    let (m200, m400, m800) = (run(200, Motion::Full), run(400, Motion::Full), run(800, Motion::Full));
    let (d200_cc, _) = difference(&m200, &m800, &[1]);
    let (d400_cc, _) = difference(&m400, &m800, &[1]);
    let (d200, cell) = difference(&m200, &m800, &all);
    let (d400, _) = difference(&m400, &m800, &all);
    println!(
        "H6 moving: max |d_200| {d200:.4e} in {} (CC {d200_cc:.4e}), max |d_400| {d400:.4e} (CC {d400_cc:.4e}); time error at dt 0.0025 = (4/3) max |d_200| = {:.4e}",
        describe(&m800.s, cell),
        4.0 / 3.0 * d200
    );
    for (k, name) in CLASSES.iter().enumerate() {
        println!(
            "H6 moving: class {name}: max |d_200| {:.4e}, max |d_400| {:.4e}",
            difference(&m200, &m800, &[k]).0,
            difference(&m400, &m800, &[k]).0
        );
    }
    let (r200, r800) = (run(200, Motion::Rest), run(800, Motion::Rest));
    let (d_rest, cell_rest) = difference(&r200, &r800, &all);
    println!(
        "H6 at rest: max |d_200| {d_rest:.4e} in {}",
        describe(&r800.s, cell_rest)
    );
    let (h200, h800) = (run(200, Motion::Half), run(800, Motion::Half));
    let (d_half, cell_half) = difference(&h200, &h800, &all);
    println!(
        "H6 half speed: max |d_200| {d_half:.4e} in {}; rho = full / half = {:.4}",
        describe(&h800.s, cell_half),
        d200 / d_half
    );
    let mut c = Checks::new();
    let r = d200_cc / d400_cc;
    c.at_least("F5 R = max_CC |d_200| / max_CC |d_400|", r, 2.5);
    c.at_most("F5 R = max_CC |d_200| / max_CC |d_400|", r, 3.5);
    c.at_most("F6 (control) at rest over moving, max |d_200|", d_rest / d200, 0.2);
    c.finish();
}

// ---- amendment 1: the F3 miss at levels 2 to 3 -------------------------------------------------------------------------

/// The spatial and the time part of the error of the calculated cells from two runs of one level and one motion, `coarse`
/// with twice the step of `fine`: the spatial part is the error of the field `2 u_fine - u_coarse` (extrapolated to `dt =
/// 0`), the time part of `fine` is `u_coarse - u_fine` (first order in `dt`). Returns the largest spatial part over the
/// calculated cells with its cell, the volume-weighted root mean square of the spatial part over the non-hole cells, and the
/// largest time part over the calculated cells with its cell.
fn parts(coarse: &Run, fine: &Run) -> ((f64, usize), f64, (f64, usize)) {
    assert!(
        coarse.s.conn.kind == fine.s.conn.kind,
        "the two runs end at different kinds"
    );
    let s = &fine.s;
    let centres = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let decay = (-2.0 * NU_TG * T_END).exp();
    let (mut sp, mut tm) = ((0.0, 0usize), (0.0, 0usize));
    let (mut sum, mut vsum) = (0.0, 0.0);
    for c in 0..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Hole {
            continue;
        }
        let (uf, uc) = (s.u.values[c], coarse.s.u.values[c]);
        let x = vec3::sub(vec3::scale(uf, 2.0), uc);
        let e = vec3::norm(vec3::sub(x, tg_u(centres[c], T_END))) / decay;
        let t = vec3::norm(vec3::sub(uc, uf)) / decay;
        sum += vol[c] * e * e;
        vsum += vol[c];
        if s.conn.kind[c] == CellKind::Calculated {
            if e > sp.0 {
                sp = (e, c);
            }
            if t > tm.0 {
                tm = (t, c);
            }
        }
    }
    (sp, (sum / vsum).sqrt(), tm)
}

/// ADR 0232 amendment 1, H8a to H8c: the F3 miss at levels 2 to 3 (printed numbers; the hypotheses are decided in the ADR).
#[test]
#[ignore = "slow (levels 1 to 3 at two steps each and at rest, ADR 0232 amendment 1, about 20 minutes): run once"]
fn a1_f3_miss_spatial_and_time_parts() {
    let ratio = |a: usize, b: usize| b as f64 / a as f64;
    // H8a: at rest
    let rest: Vec<(Run, Stats)> = [1usize, 2, 3]
        .iter()
        .map(|&l| {
            let r = tg_run(l, 200 * l, Motion::Rest, false);
            let st = stats(&r);
            report("A1 at rest", &r, &st);
            (r, st)
        })
        .collect();
    for w in rest.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let k = ratio(a.0.level, b.0.level);
        println!(
            "A1 H8a at rest, levels {} to {}: order of the calculated maximum {:.4}; of e {:.4}; of the root mean square {:.4}",
            a.0.level,
            b.0.level,
            order(a.1.e_calc, b.1.e_calc, k),
            order(a.1.e_all, b.1.e_all, k),
            order(a.1.rms_all, b.1.rms_all, k)
        );
    }
    drop(rest);
    // H8b, H8c: moving, two steps per level
    let mut rows = Vec::new();
    for l in [1usize, 2, 3] {
        let coarse = tg_run(l, 100 * l * l, Motion::Full, false);
        let fine = tg_run(l, 200 * l * l, Motion::Full, false);
        let st = stats(&fine);
        let (sp, sp_rms, tm) = parts(&coarse, &fine);
        println!(
            "A1 moving level {l} ({:.1} s, {:.1} s): calculated maximum {:.4e} in {}; spatial part {:.4e} in {} (root mean square over the non-hole cells {:.4e}); time part {:.4e} in {}; window maximum over the calculated cells {:.4e}",
            coarse.secs,
            fine.secs,
            st.e_calc,
            describe(&fine.s, st.arg_calc),
            sp.0,
            describe(&fine.s, sp.1),
            sp_rms,
            tm.0,
            describe(&fine.s, tm.1),
            fine.window_calc
        );
        rows.push((l, st.e_calc, sp.0, sp_rms, tm.0, fine.window_calc));
    }
    for w in rows.windows(2) {
        let (a, b) = (w[0], w[1]);
        let k = ratio(a.0, b.0);
        println!(
            "A1 moving, levels {} to {}: order of the calculated maximum {:.4}; H8b spatial part {:.4} (root mean square {:.4}), time part {:.4}; H8c window maximum over the calculated cells {:.4}",
            a.0,
            b.0,
            order(a.1, b.1, k),
            order(a.2, b.2, k),
            order(a.3, b.3, k),
            order(a.4, b.4, k),
            order(a.5, b.5, k)
        );
    }
}

// ---- amendment 1: a single moving mesh without overset (SM) --------------------------------------------------------------

/// SM of amendment 1: the level-2 component mesh alone, moved by `tg_pose(speed t)` with the library's ALE step and
/// `n_outer` outer iterations, the exact velocity on its four sides, `steps` steps to `t = 0.5`. Returns the velocities,
/// the largest error at `t = 0.5`, the largest continuity over the steps and the largest `abs(Q) / A` (below). `corrected`
/// (SMc of amendment 2) makes the boundary velocity discretely free of net flux: with `Q = sum v_f . S_f` and `A = sum
/// abs(S_f)` over the faces of the four sides, every face gets `v_f - (Q / A) n_f`.
fn sm_run(steps: usize, speed: f64, n_outer: usize, corrected: bool) -> (Vec<Vec3>, f64, f64, f64) {
    let dt = T_END / steps as f64;
    let cs = uniform(-0.6, 1.2, 40);
    let mesh0 = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let points_at = |t: f64| -> Vec<Vec3> {
        let pose = tg_pose(speed * t);
        mesh0.points().iter().map(|&q| pose.apply(q)).collect()
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(moved_mesh(&mesh0, points_at(0.0)).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut set = settings();
    set.n_outer = n_outer;
    let mut s = PimpleSolver::new(geom, u, p, set);
    let (mut cont, mut corr) = (0.0f64, 0.0f64);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let st = move_mesh(&s.geom, points_at(t), dt).unwrap();
        if corrected {
            corr = corr.max(sm_conditions(&st.geom, &mut s.u, t));
        } else {
            tg_conditions(&st.geom, &mut s.u, t);
        }
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, Vec::new()).unwrap();
        cont = cont.max(info.continuity);
    }
    let decay = (-2.0 * NU_TG * T_END).exp();
    let e =
        s.u.values
            .iter()
            .zip(s.geom.mesh.cell_centres())
            .map(|(v, &x)| vec3::norm(vec3::sub(*v, tg_u(x, T_END))) / decay)
            .fold(0.0, f64::max);
    (s.u.values.clone(), e, cont, corr)
}

/// The exact velocity at `t` on the four sides of `geom`, less its net flux (SMc of amendment 2). Returns `abs(Q) / A`.
fn sm_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) -> f64 {
    let sides = ["xmin", "xmax", "ymin", "ymax"];
    let sf = geom.mesh.face_area_vectors();
    let fc = geom.mesh.face_centres();
    let (mut q, mut a) = (0.0, 0.0);
    for name in sides {
        for f in geom.mesh.patch(name).unwrap().faces() {
            q += vec3::dot(tg_u(fc[f], t), sf[f]);
            a += vec3::norm(sf[f]);
        }
    }
    for name in sides {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| vec3::sub(tg_u(fc[f], t), vec3::scale(vec3::unit(sf[f]), q / a)))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
    q.abs() / a
}

/// The SM runs of H6a and H6b (one and three outer iterations, full and half speed, 200 and 800 steps each), printed.
/// Returns the largest continuity, the largest error of the run with one outer iteration and 800 steps at full speed, and
/// the largest `abs(Q) / A`.
fn sm_measure(tag: &str, corrected: bool) -> (f64, f64, f64) {
    let decay = (-2.0 * NU_TG * T_END).exp();
    let (mut cont, mut corr) = (0.0f64, 0.0f64);
    let mut d200 = |speed: f64, n_outer: usize| -> (f64, f64) {
        let (a, ea, ca, qa) = sm_run(200, speed, n_outer, corrected);
        let (b, eb, cb, qb) = sm_run(800, speed, n_outer, corrected);
        cont = cont.max(ca).max(cb);
        corr = corr.max(qa).max(qb);
        let d = a
            .iter()
            .zip(&b)
            .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)) / decay)
            .fold(0.0, f64::max);
        println!(
            "{tag} speed {speed} n_outer {n_outer}: e with 200 steps {ea:.4e}, with 800 steps {eb:.4e}; max |d_200| {d:.4e}"
        );
        (d, eb)
    };
    let (full1, e800) = d200(1.0, 1);
    let (half1, _) = d200(0.5, 1);
    let (full3, _) = d200(1.0, 3);
    let (half3, _) = d200(0.5, 3);
    println!(
        "{tag} H6a: max |d_200| {full1:.4e} ({:.3} of the overset value 4.4936e-4); rho_SM {:.4} (overset 2.2924)",
        full1 / 4.4936e-4,
        full1 / half1
    );
    println!(
        "{tag} H6b: q = three outer iterations over one {:.4}; rho with three outer iterations {:.4} (rho_BE 4.783, root mean square 3.700)",
        full3 / full1,
        full3 / half3
    );
    (cont, e800, corr)
}

/// ADR 0232 amendment 1, SM with the exact boundary velocity: F7 missed (registered run: continuity 3.07e-5; the exact
/// velocity on the closed moving boundary has a net flux, which the pin cell takes). Corrected by SMc (amendment 2).
#[test]
#[ignore = "known miss, ADR 0232 amendment 2: F7 continuity of SM with the exact boundary velocity 3.07e-5 (limit 1e-9)"]
fn sm_exact_boundary_velocity_known_miss() {
    let (cont, e800, _) = sm_measure("SM", false);
    let mut c = Checks::new();
    c.at_most("F7 SM continuity, every step of every run", cont, 1e-9);
    c.at_most("F8 SM largest error, one outer iteration, 800 steps", e800, 1e-3);
    c.finish();
}

/// ADR 0232 amendment 2, SMc: H6a and H6b with F7c, F8c and F9c: the time error of a single moving mesh without overset,
/// the boundary velocity free of net flux.
#[test]
fn smc_single_moving_mesh_time_error() {
    let (cont, e800, corr) = sm_measure("SMc", true);
    let mut c = Checks::new();
    c.at_most("F7c SMc continuity, every step of every run", cont, 1e-9);
    c.at_most("F8c SMc largest error, one outer iteration, 800 steps", e800, 1e-3);
    c.at_most("F9c (set-up) SMc correction |Q| / A", corr, 1e-5);
    c.finish();
}

// ---- amendment 3: the component at rest with the fringe phase locked between the levels (RL) -----------------------------

/// Background cell size of level 1.
const H1: f64 = PI / 40.0;

/// RL of amendment 3: a component of half width `7.5 H1` (its cells are 3/4 of the background cells at every level) with
/// its corner on the background node `(9 H1, 9 H1)` and the hole half width `3.1 H1`, so that every fringe centre falls at
/// the same place in its donor cell at every level.
fn rl_mesh(level: usize) -> Overset {
    overset_mesh(level, 7.5 * H1, 3.1 * H1)
}

fn rl_pose() -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: 0.0,
        translation: [16.5 * H1, 16.5 * H1, 0.0],
    }
}

/// Root mean square over the component fringe cells of the truncation of the exact velocity at `t = 0`, over `h_B^2`
/// (kinematic).
fn kappa(ov: &Overset, pose: &Pose, level: usize) -> f64 {
    let mesh = mesh_at(ov, pose);
    let conn = ov.connectivity(&mesh, pose).unwrap();
    let nb = conn.n_background_cells;
    let (mut sum, mut count) = (0.0, 0usize);
    for (c, st) in conn.stencils.iter().filter(|(c, _)| *c >= nb) {
        let i = truncation(mesh.cell_centres(), *c, st, 0.0);
        sum += vec3::dot(i, i);
        count += 1;
    }
    let h = H1 / level as f64;
    (sum / count as f64).sqrt() / (h * h)
}

/// ADR 0232 amendment 3, H9: the order of the solved field at rest with the phase of the fringe locked between the levels
/// (printed numbers; the hypothesis is decided in the ADR).
#[test]
#[ignore = "slow (levels 1 to 3 at rest, ADR 0232 amendment 3, minutes): run once"]
fn a3_rest_with_the_fringe_phase_locked() {
    for l in [1usize, 2, 3] {
        println!(
            "A3 level {l}: kappa = rms_CF |I| / h_B^2: registered rest pose {:.5}, RL {:.5}",
            kappa(&tg_overset_mesh(l), &tg_pose(0.0), l),
            kappa(&rl_mesh(l), &rl_pose(), l)
        );
    }
    let runs: Vec<(Run, Stats)> = [1usize, 2, 3]
        .iter()
        .map(|&l| {
            let r = run_on(rl_mesh(l), l, 200 * l, &|_| rl_pose(), false);
            let st = stats(&r);
            report("A3 RL", &r, &st);
            (r, st)
        })
        .collect();
    for w in runs.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let k = b.0.level as f64 / a.0.level as f64;
        println!(
            "A3 RL, levels {} to {}: order of the calculated maximum {:.4}; of e {:.4}; of the root mean square {:.4}; of the CC root mean square {:.4}",
            a.0.level,
            b.0.level,
            order(a.1.e_calc, b.1.e_calc, k),
            order(a.1.e_all, b.1.e_all, k),
            order(a.1.rms_all, b.1.rms_all, k),
            order(a.1.class_rms[1], b.1.class_rms[1], k)
        );
    }
}

// ---- amendment 4: the first-order term of the calculated component cells at rest ----------------------------------------

/// Ring of every calculated component cell: 1 with a fringe face neighbour, `n + 1` with a face neighbour in ring `n`
/// (0 for every other cell).
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

/// Root mean square (volume-weighted) and largest error at `t = 0.5` over the calculated component cells whose ring
/// satisfies `pick`.
fn ring_error(r: &Run, ring: &[usize], pick: impl Fn(usize) -> bool) -> (f64, f64) {
    let vol = r.s.geom.mesh.cell_volumes();
    let (mut sum, mut vsum, mut worst) = (0.0, 0.0, 0.0f64);
    for c in 0..ring.len() {
        if ring[c] > 0 && pick(ring[c]) {
            let e = eps(&r.s, c, T_END);
            sum += vol[c] * e * e;
            vsum += vol[c];
            worst = worst.max(e);
        }
    }
    ((sum / vsum).sqrt(), worst)
}

/// ADR 0232 amendment 4, H10 and H11: the first-order term of the calculated component cells in RL (printed numbers; the
/// hypotheses are decided in the ADR).
#[test]
#[ignore = "slow (RL at levels 1 to 3 and level 3 with half the step, ADR 0232 amendment 4, minutes): run once"]
fn a4_first_order_term_of_the_component_at_rest() {
    let mut rows = Vec::new();
    for l in [1usize, 2, 3] {
        let r = run_on(rl_mesh(l), l, 200 * l, &|_| rl_pose(), false);
        let ring = rings(&r.s);
        let deepest = ring.iter().copied().max().unwrap_or(0);
        let all = ring_error(&r, &ring, |_| true);
        let near = ring_error(&r, &ring, |k| k <= 2);
        let inner = ring_error(&r, &ring, |k| k >= 4);
        let profile: Vec<String> = (1..=6)
            .map(|k| format!("{:.3e}", ring_error(&r, &ring, |q| q == k).0))
            .collect();
        println!(
            "A4 RL level {l} ({:.1} s, {deepest} rings): CC rms {:.4e} max {:.4e}; rings 1 and 2 rms {:.4e} max {:.4e}; rings from 4 on rms {:.4e} max {:.4e}; rms of rings 1 to 6: {}",
            r.secs,
            all.0,
            all.1,
            near.0,
            near.1,
            inner.0,
            inner.1,
            profile.join(", ")
        );
        rows.push((l, all.0, near.0, inner.0));
    }
    for w in rows.windows(2) {
        let (a, b) = (w[0], w[1]);
        let k = b.0 as f64 / a.0 as f64;
        println!(
            "A4 RL, levels {} to {}: order of the CC rms {:.4}; H11 rings 1 and 2 {:.4}; rings from 4 on {:.4}",
            a.0,
            b.0,
            order(a.1, b.1, k),
            order(a.2, b.2, k),
            order(a.3, b.3, k)
        );
    }
    let fine = run_on(rl_mesh(3), 3, 1200, &|_| rl_pose(), false);
    let ring = rings(&fine.s);
    let half = ring_error(&fine, &ring, |_| true).0;
    println!(
        "A4 H10 RL level 3: CC rms with 1200 steps {half:.4e} ({:.1} s), with 600 steps {:.4e}: ratio {:.4}",
        fine.secs,
        rows[2].1,
        half / rows[2].1
    );
}
