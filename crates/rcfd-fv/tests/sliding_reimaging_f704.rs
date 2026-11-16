//! F-704 (ADR 0634): sector and planar sliding beyond one period, the rotor re-imaged by one period whenever its offset
//! reaches it (`PeriodicSlidingInterface::mesh_turned`, the re-imaging step of `periodic_sliding_step`, the re-imaging of
//! the rotor state in `PimpleSolver::step_sliding_periodic`). Checks P (the planar slider over three periods), S (the rotor
//! sector over three pitches), F (the sector against the full-annulus run of ADR 0216), their negative controls, E
//! (refusals) and D3 (the path without re-imaging bit for bit). The limits are those registered in ADR 0634 before this
//! code ran; each test prints every measured number and then asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, StepInfo};
use rcfd_fv::sliding::sliding_step;
use rcfd_fv::sliding_periodic::{periodic_sliding_step, PeriodicSlidingStep, Reimage};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, annulus_sector, cylinder_ogrid, rectilinear_2d, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// The planar step and run length of ADR 0634 (PP).
const PP_DT: f64 = 0.00493;
const PP_STEPS: usize = 620;
/// The sector steps and run lengths of ADR 0634 (SS, S3).
const SS_DT: f64 = 0.005;
const SS_STEPS: usize = 640;
const S3_DT: f64 = 0.01;
const S3_STEPS: usize = 320;

/// PS6 of ADR 0629.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 6;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn uniform(n: usize, a: f64, b: f64) -> Vec<f64> {
    (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
}

fn periodic_x(mut m: Mesh) -> Mesh {
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    m
}

fn pair_pp() -> PeriodicSlidingInterface {
    let a = periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap());
    let b = periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap());
    PeriodicSlidingInterface::planar(a, "ymin", b, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap()
}

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

fn pair_s3() -> PeriodicSlidingInterface {
    let a = annulus_3d_sector(12, 4, 2, 1.0, 1.5, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    let b = annulus_3d_sector(10, 4, 3, 1.5, 2.0, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// ADR 0216's pair P (the full annulus).
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

fn per_face(g: &FvGeometry, patch: &str, f: impl Fn(Vec3) -> Vec3) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| f(cf[k])).collect(),
    )))
}

/// A measure for a running maximum: NaN counts as infinite.
fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

/// GCL residual of a step's data: `max_c abs(V1 - V0 - dt sum sigma m) / mean(V1)`.
fn gcl_of(geom: &FvGeometry, mesh_flux: &[f64], vol_old: &[f64], dt: f64) -> f64 {
    let m = &geom.mesh;
    let ni = m.n_internal_faces();
    let mut s = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        s[m.owner()[f]] += mesh_flux[f] * dt;
        if f < ni {
            s[m.neighbour()[f]] -= mesh_flux[f] * dt;
        }
    }
    let v1 = m.cell_volumes();
    let mean = v1.iter().sum::<f64>() / v1.len() as f64;
    (0..v1.len()).fold(0.0f64, |a, c| a.max((v1[c] - vol_old[c] - s[c]).abs())) / mean
}

/// Continuity of the remapped old flux: `max_c abs(sum sigma phi_old) / max abs(S)`.
fn remap_of(geom: &FvGeometry, phi_old: &[f64]) -> f64 {
    let m = &geom.mesh;
    let ni = m.n_internal_faces();
    let mut d = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        d[m.owner()[f]] += phi_old[f];
        if f < ni {
            d[m.neighbour()[f]] -= phi_old[f];
        }
    }
    let smax = m.face_area_vectors().iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
    d.iter().fold(0.0f64, |a, x| a.max(x.abs())) / smax
}

/// The largest value over all steps and over the re-imaging steps alone.
#[derive(Debug, Default, Clone, Copy)]
struct Max {
    all: f64,
    reim: f64,
}

impl Max {
    fn put(&mut self, x: f64, reim: bool) {
        let x = finite_or_inf(x);
        self.all = self.all.max(x);
        if reim {
            self.reim = self.reim.max(x);
        }
    }
    fn within(&self, limit: f64) -> bool {
        self.all <= limit && self.reim <= limit
    }
    fn show(&self) -> String {
        format!("{:.2e} (re-imaging steps {:.2e})", self.all, self.reim)
    }
}

#[derive(Debug, Default, Clone)]
struct Stats {
    cont: Max,
    gcl: Max,
    remap_late: Max,
    steps: usize,
    reimaged: Vec<usize>,
}

/// What a step's hooks may do: set the boundary data on the new mesh; alter the step data (with whether the step
/// re-images).
type BcHook<'b> = &'b mut dyn FnMut(&mut PimpleSolver, &PeriodicSlidingStep);
type AlterHook<'b> = &'b mut dyn FnMut(&mut PeriodicSlidingStep, bool);

fn no_bc() -> impl FnMut(&mut PimpleSolver, &PeriodicSlidingStep) {
    |_: &mut PimpleSolver, _: &PeriodicSlidingStep| {}
}

fn no_alter() -> impl FnMut(&mut PeriodicSlidingStep, bool) {
    |_: &mut PeriodicSlidingStep, _: bool| {}
}

/// ADR 0629's loop with turned offsets (ADR 0634): the rotor at `speed . n . dt` after step `n`, any number of periods.
struct Turning<'a> {
    sc: &'a PeriodicSlidingInterface,
    cur: PeriodicSlidingMesh,
    s: PimpleSolver,
    speed: f64,
    dt: f64,
    /// PN2 / SN2: build every re-imaging step as a step inside one image (ADR 0634 R4).
    no_reimage: bool,
    stats: Stats,
}

impl<'a> Turning<'a> {
    fn new(
        sc: &'a PeriodicSlidingInterface,
        make: impl FnOnce(Arc<FvGeometry>) -> PimpleSolver,
        speed: f64,
        dt: f64,
    ) -> Self {
        let cur = sc.mesh_turned(0.0).unwrap();
        let s = make(geom_of(&cur.mesh));
        Turning {
            sc,
            cur,
            s,
            speed,
            dt,
            no_reimage: false,
            stats: Stats::default(),
        }
    }

    /// One step; `Ok(true)` when it re-imaged. An error (a refusal or a non-finite velocity) ends the run.
    fn step(&mut self, bc: BcHook, alter: AlterHook) -> Result<(StepInfo, bool), String> {
        let turned = self.speed * (self.stats.steps + 1) as f64 * self.dt;
        let new = self.sc.mesh_turned(turned).map_err(|e| e.to_string())?;
        let reim = new.image != self.cur.image;
        let fake;
        let old = if self.no_reimage && reim {
            let mut o = self.cur.clone();
            o.image = new.image;
            fake = o;
            &fake
        } else {
            &self.cur
        };
        let mut st = periodic_sliding_step(&self.s.geom, old, &new, &self.s.phi, &self.s.u.values, self.dt)?;
        alter(&mut st, reim);
        let first = self.stats.steps == 0;
        self.stats
            .gcl
            .put(gcl_of(&st.geom, &st.mesh_flux, &st.vol_old, self.dt), reim);
        if !first {
            self.stats.remap_late.put(remap_of(&st.geom, &st.phi_old), reim);
        }
        bc(&mut self.s, &st);
        let info = self.s.step_sliding_periodic(self.dt, st, Vec::new())?;
        self.stats.steps += 1;
        if reim {
            self.stats.reimaged.push(self.stats.steps);
        }
        self.cur = new;
        if !self.s.u.values.iter().flatten().all(|x| x.is_finite()) {
            return Err(format!("non-finite velocity at step {}", self.stats.steps));
        }
        self.stats.cont.put(info.continuity, reim);
        Ok((info, reim))
    }

    /// Net flux out of the rotor through the interface (direct faces and rotor copies), the direct faces alone, and the
    /// summed area.
    fn interface_flux(&self) -> (f64, f64, f64) {
        let m = &self.s.geom.mesh;
        let phi = &self.s.phi;
        let direct: f64 = self.cur.report.join.interface_faces.iter().map(|&f| phi[f]).sum();
        let wrap: f64 = self.cur.report.wrap_pairs.iter().map(|&(a, _)| phi[a]).sum();
        let area: f64 = self
            .cur
            .report
            .join
            .interface_faces
            .iter()
            .map(|&f| m.face_area(f))
            .chain(self.cur.report.wrap_pairs.iter().map(|&(a, _)| m.face_area(a)))
            .sum();
        (direct + wrap, direct, area)
    }

    fn pair_sum(&self) -> f64 {
        let pairs = self.s.geom.periodic_pairs().unwrap();
        pairs.iter().map(|p| (self.s.phi[p.f] + self.s.phi[p.g]).abs()).sum()
    }

    fn patch_flux(&self, name: &str) -> f64 {
        let m = &self.s.geom.mesh;
        m.patch(name).unwrap().faces().map(|f| self.s.phi[f]).sum()
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

fn require(fails: &Fails, ids: &[&str]) {
    let bad: Vec<_> = fails.iter().filter(|x| ids.contains(&x.0)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

// -------------------------------------------------------------------------------------------------------------------
// P. The planar slider over three periods
// -------------------------------------------------------------------------------------------------------------------

fn pf_solver(g: Arc<FvGeometry>) -> PimpleSolver {
    let inlet = |x: Vec3| [0.0, 0.5 * (1.0 + 0.5 * (2.0 * PI * x[0]).sin()), 0.0];
    let mut u = VectorField::uniform("U", &g, [0.0, 0.5, 0.0]);
    u.set_bc(&g, "ymin", per_face(&g, "ymin", inlet)).unwrap();
    u.set_bc(&g, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(g, u, p, settings(0.1))
}

/// P1 to P7 (PF) and PN1 of ADR 0634.
#[test]
fn pf_planar_through_flow_over_three_periods_conserves_mass() {
    let sc = pair_pp();
    let mut run = Turning::new(&sc, pf_solver, 1.0, PP_DT);
    let (mut p1, mut p2, mut p6) = (Max::default(), Max::default(), Max::default());
    let mut pn1: f64 = 0.0;
    for _ in 0..PP_STEPS {
        let (_, reim) = run.step(&mut no_bc(), &mut no_alter()).unwrap();
        let q_in = -run.patch_flux("ymin");
        let q_out = run.patch_flux("ymax");
        let (q_i, direct, _) = run.interface_flux();
        p1.put((q_i + q_in).abs() / q_in, reim);
        p2.put((q_out - q_in).abs() / q_in, reim);
        p6.put(run.pair_sum() / q_in, reim);
        pn1 = pn1.max((direct + q_in).abs() / q_in);
    }
    let st = &run.stats;
    let turned = run.cur.turned();
    println!(
        "PF: printed, no limit: Q_in {:.6e}, wrap pairs at the end {}, final reduced offset {:.6}, image {}",
        -run.patch_flux("ymin"),
        run.cur.report.wrap_pairs.len(),
        run.cur.offset,
        run.cur.image
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "P1",
        p1.within(1e-10),
        format!("|Q_I + Q_in|/Q_in {} (1e-10)", p1.show()),
    );
    check(
        &mut fails,
        "P2",
        p2.within(1e-10),
        format!("|Q_out - Q_in|/Q_in {} (1e-10)", p2.show()),
    );
    check(
        &mut fails,
        "P3",
        st.cont.within(1e-9),
        format!("continuity {} (1e-9)", st.cont.show()),
    );
    check(
        &mut fails,
        "P4",
        st.gcl.within(1e-12),
        format!("GCL {} mean(V) (1e-12)", st.gcl.show()),
    );
    check(
        &mut fails,
        "P5",
        st.remap_late.within(1e-9),
        format!("remapped old flux from the second step {} (1e-9)", st.remap_late.show()),
    );
    check(
        &mut fails,
        "P6",
        p6.within(1e-12),
        format!("periodic pair sums {} Q_in (1e-12)", p6.show()),
    );
    check(
        &mut fails,
        "P7",
        st.reimaged == [203, 406, 609] && turned >= 3.0,
        format!(
            "PF: re-imaging steps {:?} ([203, 406, 609]), turned offset {turned:.4} (>= 3)",
            st.reimaged
        ),
    );
    check(
        &mut fails,
        "PN1",
        pn1 >= 0.1,
        format!("direct faces alone, largest {pn1:.3e} Q_in off (>= 0.1)"),
    );
    require(&fails, &["P1", "P2", "P3", "P4", "P5", "P6", "P7", "PN1"]);
}

/// A uniform-flow run: the largest velocity and pressure measures, the interface flux measure, the stats, the turned
/// offset, and the error that ended the run early (if any).
struct UniformRun {
    eu: Max,
    ep: Max,
    eq: Max,
    stats: Stats,
    turned: f64,
    ended: Option<String>,
}

fn uniform_run(
    sc: &PeriodicSlidingInterface,
    u0: Vec3,
    dt: f64,
    steps: usize,
    inflow: Option<&str>,
    no_reimage: bool,
) -> UniformRun {
    let names: Vec<String> = sc
        .mesh_at(0.0)
        .unwrap()
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty && p.kind != PatchKind::Cyclic)
        .map(|p| p.name.clone())
        .collect();
    let mut run = Turning::new(
        sc,
        |g| {
            let mut u = VectorField::uniform("U", &g, u0);
            for n in &names {
                u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
            }
            let p = ScalarField::uniform("p", &g, 0.0);
            PimpleSolver::new(g, u, p, settings(0.1))
        },
        1.0,
        dt,
    );
    run.no_reimage = no_reimage;
    let un = vec3::norm(u0);
    let (mut eu, mut ep, mut eq) = (Max::default(), Max::default(), Max::default());
    let mut ended = None;
    for _ in 0..steps {
        let reim = match run.step(&mut no_bc(), &mut no_alter()) {
            Ok((_, r)) => r,
            Err(e) => {
                ended = Some(e);
                break;
            }
        };
        eu.put(
            run.s
                .u
                .values
                .iter()
                .fold(0.0f64, |a, v| a.max(vec3::norm(vec3::sub(*v, u0))))
                / un,
            reim,
        );
        let p0 = run.s.p.values[0];
        ep.put(
            run.s.p.values.iter().fold(0.0f64, |a, p| a.max((p - p0).abs())) / (un * un),
            reim,
        );
        let (q_i, _, area) = run.interface_flux();
        let q = match inflow {
            Some(name) => {
                let q_in = -run.patch_flux(name);
                (q_i + q_in).abs() / q_in
            }
            None => q_i.abs() / (un * area),
        };
        eq.put(q, reim);
    }
    UniformRun {
        eu,
        ep,
        eq,
        turned: run.cur.turned(),
        stats: run.stats,
        ended,
    }
}

/// P7 (PU), P8 and P9 of ADR 0634.
#[test]
fn pu_uniform_flow_over_three_periods_of_the_planar_slide() {
    let r = uniform_run(&pair_pp(), [1.0, 0.5, 0.0], PP_DT, PP_STEPS, Some("ymin"), false);
    assert!(r.ended.is_none(), "{:?}", r.ended);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "P8",
        r.eu.within(1e-10) && r.ep.within(1e-10),
        format!(
            "PU: velocity {} |U|, pressure {} |U|^2 (1e-10 each)",
            r.eu.show(),
            r.ep.show()
        ),
    );
    check(
        &mut fails,
        "P9",
        r.eq.within(1e-10) && r.stats.gcl.within(1e-12) && r.stats.cont.within(1e-9),
        format!(
            "PU: |Q_I + Q_in|/Q_in {} (1e-10), GCL {} (1e-12), continuity {} (1e-9)",
            r.eq.show(),
            r.stats.gcl.show(),
            r.stats.cont.show()
        ),
    );
    check(
        &mut fails,
        "P7",
        r.stats.reimaged == [203, 406, 609] && r.turned >= 3.0,
        format!(
            "PU: re-imaging steps {:?} ([203, 406, 609]), turned offset {:.4} (>= 3)",
            r.stats.reimaged, r.turned
        ),
    );
    require(&fails, &["P7", "P8", "P9"]);
}

/// PN2 of ADR 0634 (negative control of P8: no re-imaging).
#[test]
fn pn2_uniform_flow_negative_control_no_re_imaging() {
    let r = uniform_run(&pair_pp(), [1.0, 0.5, 0.0], PP_DT, PP_STEPS, Some("ymin"), true);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "PN2",
        r.eu.all >= 1e-6 || r.ended.is_some(),
        format!(
            "no re-imaging: velocity {} |U| (>= 1e-6), steps run {}, ended early: {:?}",
            r.eu.show(),
            r.stats.steps,
            r.ended
        ),
    );
    require(&fails, &["PN2"]);
}

// -------------------------------------------------------------------------------------------------------------------
// S. The rotor sector over three pitches
// -------------------------------------------------------------------------------------------------------------------

const U_IN: f64 = 1.0;
const RI: f64 = 1.0;

fn source_flow(c: Vec3) -> Vec3 {
    vec3::scale(xhat(c), U_IN * RI / c[0].hypot(c[1]))
}

fn sf_solver(g: Arc<FvGeometry>, inlet: &str, outlet: &str) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", &g, source_flow);
    u.set_bc(&g, inlet, per_face(&g, inlet, |x| vec3::scale(xhat(x), U_IN)))
        .unwrap();
    u.set_bc(&g, outlet, Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, outlet, Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(g, u, p, settings(0.1))
}

fn sf_bc() -> impl FnMut(&mut PimpleSolver, &PeriodicSlidingStep) {
    |s: &mut PimpleSolver, st: &PeriodicSlidingStep| {
        s.u.set_bc(
            &st.geom,
            "inner",
            per_face(&st.geom, "inner", |x| vec3::scale(xhat(x), U_IN)),
        )
        .unwrap();
    }
}

/// SN1 / FN: the rotor vectors not re-imaged in the solver.
fn drop_reimage() -> impl FnMut(&mut PeriodicSlidingStep, bool) {
    |st: &mut PeriodicSlidingStep, _: bool| st.reimage = None
}

fn potential_error(s: &PimpleSolver) -> f64 {
    let cc = s.geom.mesh.cell_centres();
    s.u.values
        .iter()
        .zip(cc)
        .map(|(v, c)| vec3::norm(vec3::sub(*v, source_flow(*c))))
        .fold(0.0, f64::max)
        / U_IN
}

/// S1 to S8, S7 (SF) of ADR 0634.
#[test]
fn sf_radial_source_flow_over_three_pitches_of_the_sector() {
    let sc = pair_ss();
    let mut run = Turning::new(&sc, |g| sf_solver(g, "inner", "outer"), 1.0, SS_DT);
    let (mut s1, mut s2, mut s6, mut s8) = (Max::default(), Max::default(), Max::default(), Max::default());
    for _ in 0..SS_STEPS {
        let (_, reim) = run.step(&mut sf_bc(), &mut no_alter()).unwrap();
        let q_in = -run.patch_flux("inner");
        let q_out = run.patch_flux("outer");
        let (q_i, _, _) = run.interface_flux();
        s1.put((q_i - q_in).abs() / q_in, reim);
        s2.put((q_out - q_in).abs() / q_in, reim);
        s6.put(run.pair_sum() / q_in, reim);
        s8.put(potential_error(&run.s), reim);
    }
    let st = &run.stats;
    let turned = run.cur.turned();
    println!(
        "SF: printed, no limit: error against the potential flow at the end {:.3e} U_in, wrap pairs at the end {}",
        potential_error(&run.s),
        run.cur.report.wrap_pairs.len()
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "S1",
        s1.within(1e-10),
        format!("|Q_I - Q_in|/Q_in {} (1e-10)", s1.show()),
    );
    check(
        &mut fails,
        "S2",
        s2.within(1e-10),
        format!("|Q_out - Q_in|/Q_in {} (1e-10)", s2.show()),
    );
    check(
        &mut fails,
        "S3",
        st.cont.within(1e-9),
        format!("continuity {} (1e-9)", st.cont.show()),
    );
    check(
        &mut fails,
        "S4",
        st.gcl.within(1e-12),
        format!("GCL {} mean(V) (1e-12)", st.gcl.show()),
    );
    check(
        &mut fails,
        "S5",
        st.remap_late.within(1e-9),
        format!("remapped old flux from the second step {} (1e-9)", st.remap_late.show()),
    );
    check(
        &mut fails,
        "S6",
        s6.within(1e-12),
        format!("periodic pair sums {} Q_in (1e-12)", s6.show()),
    );
    check(
        &mut fails,
        "S7",
        st.reimaged == [210, 419, 629] && turned >= PI,
        format!(
            "SF: re-imaging steps {:?} ([210, 419, 629]), turned {turned:.4} rad (>= pi)",
            st.reimaged
        ),
    );
    check(
        &mut fails,
        "S8",
        s8.within(2e-2),
        format!("max |u - U_in r_i/r x_hat| {} U_in (2e-2)", s8.show()),
    );
    require(&fails, &["S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8"]);
}

/// SN1 of ADR 0634 (negative control of S8: the rotor vectors not re-imaged in the solver).
#[test]
fn sn1_source_flow_negative_control_vectors_not_re_imaged() {
    let sc = pair_ss();
    let mut run = Turning::new(&sc, |g| sf_solver(g, "inner", "outer"), 1.0, SS_DT);
    let mut s8 = Max::default();
    let mut ended = None;
    for _ in 0..SS_STEPS {
        match run.step(&mut sf_bc(), &mut drop_reimage()) {
            Ok((_, reim)) => s8.put(potential_error(&run.s), reim),
            Err(e) => {
                ended = Some(e);
                break;
            }
        }
    }
    let mut fails = Fails::new();
    check(
        &mut fails,
        "SN1",
        s8.all >= 0.1,
        format!(
            "vectors not re-imaged: max |u - U_in r_i/r x_hat| {} U_in (>= 0.1); ended early: {ended:?}",
            s8.show()
        ),
    );
    require(&fails, &["SN1"]);
}

/// S7 (SU), S9 and S10 of ADR 0634.
#[test]
fn su_uniform_axial_flow_over_three_pitches_of_the_3d_sector() {
    let r = uniform_run(&pair_s3(), [0.0, 0.0, 1.0], S3_DT, S3_STEPS, None, false);
    assert!(r.ended.is_none(), "{:?}", r.ended);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "S9",
        r.eu.within(1e-10) && r.ep.within(1e-10),
        format!(
            "SU: velocity {} |U|, pressure {} |U|^2 (1e-10 each)",
            r.eu.show(),
            r.ep.show()
        ),
    );
    check(
        &mut fails,
        "S10",
        r.eq.within(1e-10) && r.stats.gcl.within(1e-12) && r.stats.cont.within(1e-9),
        format!(
            "SU: |Q_I|/(|U| A_I) {} (1e-10), GCL {} (1e-12), continuity {} (1e-9)",
            r.eq.show(),
            r.stats.gcl.show(),
            r.stats.cont.show()
        ),
    );
    check(
        &mut fails,
        "S7",
        r.stats.reimaged == [105, 210, 315] && r.turned >= PI,
        format!(
            "SU: re-imaging steps {:?} ([105, 210, 315]), turned {:.4} rad (>= pi)",
            r.stats.reimaged, r.turned
        ),
    );
    require(&fails, &["S7", "S9", "S10"]);
}

/// SN2 of ADR 0634 (negative control of S9: no re-imaging).
#[test]
fn sn2_uniform_axial_flow_negative_control_no_re_imaging() {
    let r = uniform_run(&pair_s3(), [0.0, 0.0, 1.0], S3_DT, S3_STEPS, None, true);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "SN2",
        r.eu.all >= 1e-6 || r.ended.is_some(),
        format!(
            "no re-imaging: velocity {} |U| (>= 1e-6), steps run {}, ended early: {:?}",
            r.eu.show(),
            r.stats.steps,
            r.ended
        ),
    );
    require(&fails, &["SN2"]);
}

// -------------------------------------------------------------------------------------------------------------------
// F. The sector against the full annulus of ADR 0216
// -------------------------------------------------------------------------------------------------------------------

/// ADR 0216's loop SL on the full annulus.
struct Annulus {
    sc: SlidingInterface,
    cur: SlidingMesh,
    s: PimpleSolver,
    steps: usize,
}

impl Annulus {
    fn new() -> Self {
        let sc = pair_p();
        let cur = sc.mesh_at(0.0).unwrap();
        let s = sf_solver(geom_of(&cur.mesh), "cylinder", "farfield");
        Annulus { sc, cur, s, steps: 0 }
    }

    fn step(&mut self) {
        let theta = (self.steps + 1) as f64 * SS_DT;
        let new = self.sc.mesh_at(theta).unwrap();
        let st = sliding_step(&self.s.geom, &self.cur, &new, &self.s.phi, &self.s.u.values, SS_DT).unwrap();
        self.s
            .u
            .set_bc(
                &st.geom,
                "cylinder",
                per_face(&st.geom, "cylinder", |x| vec3::scale(xhat(x), U_IN)),
            )
            .unwrap();
        self.s.step_sliding(SS_DT, st, Vec::new()).unwrap();
        self.steps += 1;
        self.cur = new;
    }
}

/// For every cell of the sector, the annulus cell whose centre is nearest, and the largest such distance; whether the
/// match is one-to-one.
fn match_cells(sector: &Mesh, annulus: &Mesh) -> (Vec<usize>, f64, bool) {
    let ca = annulus.cell_centres();
    let mut worst: f64 = 0.0;
    let idx: Vec<usize> = sector
        .cell_centres()
        .iter()
        .map(|c| {
            let (mut best, mut d) = (0, f64::INFINITY);
            for (j, a) in ca.iter().enumerate() {
                let e = vec3::norm(vec3::sub(*c, *a));
                if e < d {
                    d = e;
                    best = j;
                }
            }
            worst = worst.max(d);
            best
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let one_to_one = idx.iter().all(|j| seen.insert(*j));
    (idx, worst, one_to_one)
}

/// The largest velocity difference of a sector solver against the annulus over the matched cells, and the
/// volume-weighted rms.
fn difference(sector: &PimpleSolver, annulus: &PimpleSolver, idx: &[usize]) -> (f64, f64) {
    let vol = sector.geom.mesh.cell_volumes();
    let (mut mx, mut s2, mut sv): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for (c, &j) in idx.iter().enumerate() {
        let d = vec3::norm(vec3::sub(sector.u.values[c], annulus.u.values[j])) / U_IN;
        mx = mx.max(finite_or_inf(d));
        s2 += vol[c] * d * d;
        sv += vol[c];
    }
    (mx, (s2 / sv).sqrt())
}

/// F0 to F3 and FN of ADR 0634.
#[test]
fn f_sector_source_flow_against_the_full_annulus_run() {
    let t0 = Instant::now();
    let sc = pair_ss();
    let mut ann = Annulus::new();
    let mut sec = Turning::new(&sc, |g| sf_solver(g, "inner", "outer"), 1.0, SS_DT);
    let mut neg = Turning::new(&sc, |g| sf_solver(g, "inner", "outer"), 1.0, SS_DT);
    let (mut f0_dist, mut f0_one): (f64, bool) = (0.0, true);
    let (mut f1, mut f1_first, mut f1_later, mut fneg): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let mut neg_ended = None;
    let mut last = (Vec::new(), 0.0);
    for n in 1..=SS_STEPS {
        ann.step();
        sec.step(&mut sf_bc(), &mut no_alter()).unwrap();
        if neg_ended.is_none() {
            if let Err(e) = neg.step(&mut sf_bc(), &mut drop_reimage()) {
                neg_ended = Some(e);
            }
        }
        let (idx, dist, one) = match_cells(&sec.s.geom.mesh, &ann.s.geom.mesh);
        f0_dist = f0_dist.max(dist);
        f0_one &= one;
        let (d, rms) = difference(&sec.s, &ann.s, &idx);
        f1 = f1.max(d);
        if n < 210 {
            f1_first = f1_first.max(d);
        } else {
            f1_later = f1_later.max(d);
        }
        if neg_ended.is_none() {
            let (idx_n, _, _) = match_cells(&neg.s.geom.mesh, &ann.s.geom.mesh);
            fneg = fneg.max(difference(&neg.s, &ann.s, &idx_n).0);
        }
        last = (idx, rms);
    }
    let (idx, f2) = last;
    // F3: the rotor's inner ring
    let nr = sc.n_rotor_cells();
    let cc = sec.s.geom.mesh.cell_centres();
    let (vs, va) = (sec.s.geom.mesh.cell_volumes(), ann.s.geom.mesh.cell_volumes());
    let (mut ps, mut ws, mut pa, mut wa) = (0.0, 0.0, 0.0, 0.0);
    for c in 0..nr {
        if cc[c][0].hypot(cc[c][1]) < 1.0625 {
            ps += vs[c] * sec.s.p.values[c];
            ws += vs[c];
            pa += va[idx[c]] * ann.s.p.values[idx[c]];
            wa += va[idx[c]];
        }
    }
    let dp_pot = U_IN * U_IN * (1.0 - RI * RI / 4.0) / 2.0;
    let f3 = (ps / ws - pa / wa).abs() / dp_pot;
    println!(
        "F: printed, no limit: F1 over steps 1 to 209 {f1_first:.3e}, over 210 to 640 {f1_later:.3e}; against the potential \
         flow at the end: annulus {:.3e}, sector {:.3e} U_in; inner-ring mean pressure sector {:.6e}, annulus {:.6e}; \
         sector re-imaging steps {:?}; elapsed {:.1} s",
        potential_error(&ann.s),
        potential_error(&sec.s),
        ps / ws,
        pa / wa,
        sec.stats.reimaged,
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "F0",
        f0_dist <= 1e-9 && f0_one,
        format!("nearest annulus centre {f0_dist:.2e} (1e-9), one-to-one {f0_one}"),
    );
    check(
        &mut fails,
        "F1",
        f1 <= 2e-2,
        format!("max |u_sector - u_annulus| over the steps {f1:.3e} U_in (2e-2)"),
    );
    check(
        &mut fails,
        "F2",
        f2 <= 1e-2,
        format!("rms at the last step {f2:.3e} U_in (1e-2)"),
    );
    check(
        &mut fails,
        "F3",
        f3 <= 5e-2,
        format!("inner-ring mean pressure difference {f3:.3e} dp_pot (5e-2)"),
    );
    check(
        &mut fails,
        "FN",
        fneg >= 0.1,
        format!("vectors not re-imaged: F1 measure {fneg:.3e} U_in (>= 0.1); ended early: {neg_ended:?}"),
    );
    require(&fails, &["F0", "F1", "F2", "F3", "FN"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E. Refusals; D3. The path without re-imaging
// -------------------------------------------------------------------------------------------------------------------

fn state_bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    v.extend(s.p.values.iter().map(|x| x.to_bits()));
    v.extend(s.phi.iter().map(|x| x.to_bits()));
    v.push(s.time.to_bits());
    v
}

/// E1 and E2 of ADR 0634.
#[test]
fn e_refusals_of_the_re_imaging_step() {
    let sc = pair_ss();
    let (t0, t1) = (FRAC_PI_3 - 0.0025, FRAC_PI_3 + 0.0025);
    let (m0, m1) = (sc.mesh_turned(t0).unwrap(), sc.mesh_turned(t1).unwrap());
    let g = geom_of(&m0.mesh);
    let u = VectorField::from_fn("U", &g, source_flow);
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSolver::new(g, u, p, settings(0.1));
    let mut fails = Fails::new();
    let mut two = m1.clone();
    two.image = 2;
    let r1 = periodic_sliding_step(&s.geom, &m0, &two, &s.phi, &s.u.values, 0.005);
    let mut back = m0.clone();
    back.image = 1;
    let mut m1b = m1.clone();
    m1b.image = 0;
    let r2 = periodic_sliding_step(&s.geom, &back, &m1b, &s.phi, &s.u.values, 0.005);
    println!("E1: 0 -> 2: {:?}; 1 -> 0: {:?}", r1.as_ref().err(), r2.as_ref().err());
    let e1 = r1.as_ref().err().is_some_and(|e| e.contains("image"))
        && r2.as_ref().err().is_some_and(|e| e.contains("image"));
    check(
        &mut fails,
        "E1",
        e1,
        "image changes of 2 and of -1 refused, naming image".into(),
    );
    let mut st = periodic_sliding_step(&s.geom, &m0, &m1, &s.phi, &s.u.values, 0.005).unwrap();
    assert_eq!(st.images, 1);
    let rot = st.reimage.expect("a re-imaging step of a sector").rotation;
    st.reimage = Some(Reimage {
        n_rotor_cells: s.geom.n_cells() + 1,
        rotation: rot,
    });
    let before = state_bits(&s);
    let r3 = s.step_sliding_periodic(0.005, st, Vec::new());
    println!("E2: {:?}", r3.as_ref().err());
    check(
        &mut fails,
        "E2",
        r3.is_err() && state_bits(&s) == before,
        "too many rotor cells refused, the solver unchanged".into(),
    );
    require(&fails, &["E1", "E2"]);
}

/// D3 of ADR 0634: PF for 20 steps through `mesh_turned` and through `mesh_at` (ADR 0629's loop), bit for bit.
#[test]
fn d3_turned_offsets_within_one_period_are_bit_identical() {
    let sc = pair_pp();
    let mut run = Turning::new(&sc, pf_solver, 1.0, PP_DT);
    for _ in 0..20 {
        run.step(&mut no_bc(), &mut no_alter()).unwrap();
    }
    let mut cur = sc.mesh_at(0.0).unwrap();
    let mut s = pf_solver(geom_of(&cur.mesh));
    for n in 1..=20 {
        let new = sc.mesh_at(1.0 * n as f64 * PP_DT).unwrap();
        let st = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, PP_DT).unwrap();
        s.step_sliding_periodic(PP_DT, st, Vec::new()).unwrap();
        cur = new;
    }
    let same = state_bits(&run.s) == state_bits(&s);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "D3",
        same,
        format!("u, p, phi and time after 20 steps equal bit for bit: {same}"),
    );
    require(&fails, &["D3"]);
}
