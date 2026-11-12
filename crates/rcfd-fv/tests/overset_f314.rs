//! F-314 (ADR 0232): diagnosis of the observed-order misses of the overset coupling (T1, T2 at level 2 and T1c of
//! ADR 0219). The set-ups are the Taylor-Green runs of `overset_f134.rs` (ADR 0219 TG_L), here on three levels with the
//! step proportional to `h^2`, with the error split by cell class and, in the fringe cells, into the truncation of the
//! donor interpolation and the carried error of the donors. No library code is changed by these tests.
//!
//! Release only (the flow runs take minutes): `cargo test -p rcfd-fv --release --test overset_f314`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
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
    let bs = uniform(0.0, PI, 40 * level);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let cs = uniform(-0.6, 1.2, 20 * level);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
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
    let t0 = Instant::now();
    let dt = T_END / steps as f64;
    let ov = tg_overset_mesh(level);
    let pose = pose_of(motion, 0.0);
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
    let (mut window, mut delta, mut cont) = (0.0f64, 0.0f64, 0.0f64);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let mut st = s.prepare(dt, pose_of(motion, t), &[]).unwrap();
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
                    window = window.max(eps(&s, c, t));
                }
            }
        }
    }
    Run {
        level,
        dt,
        s,
        window,
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

fn check_pair(c: &mut Checks, tag: &str, a: &(Run, Stats), b: &(Run, Stats)) {
    let (f2, f3, f4) = orders(tag, (&a.0, &a.1), (&b.0, &b.1));
    let pair = format!("levels {} to {}", a.0.level, b.0.level);
    c.at_least(&format!("F2 order of the root mean square, {pair}"), f2, 1.7);
    c.at_least(&format!("F3 order of the calculated maximum, {pair}"), f3, 1.7);
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
    check_pair(&mut c, "F", &runs[0], &runs[1]);
    c.finish();
}

/// ADR 0232 F1 to F4 on levels 1, 2 and 3 (level 3: 1800 steps).
#[test]
#[ignore = "slow (level 3 of ADR 0232, minutes): run once for the addendum"]
fn f_split_and_orders_three_levels() {
    let runs = level_runs("F", &[1, 2, 3], false);
    let mut c = Checks::new();
    for (r, st) in &runs {
        c.at_most(&format!("F1 fringe identity, level {}", r.level), st.identity, 1e-12);
    }
    check_pair(&mut c, "F", &runs[0], &runs[1]);
    check_pair(&mut c, "F", &runs[1], &runs[2]);
    c.finish();
}

/// ADR 0232 N5 (negative control): the donor cell alone (first-order interpolation), levels 1 and 2.
#[test]
fn n5_orders_with_the_donor_cell_only() {
    let runs = level_runs("N5", &[1, 2], true);
    let (f2, f3, f4) = orders("N5", (&runs[0].0, &runs[0].1), (&runs[1].0, &runs[1].1));
    let mut c = Checks::new();
    c.at_most("N5 order of the root mean square", f2, 1.3);
    c.at_most("N5 order of the calculated maximum", f3, 1.3);
    c.at_most("N5 order of the window maximum", f4, 1.3);
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
