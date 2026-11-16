//! F-730 (ADR 0654): sector sliding in the case tree at node coincidences. ADR 0646's FS set-up with ADR 0637's step
//! `pitch / 8` (a node coincidence every rotor pitch) through the coupled periodic step of F-721 (`[mesh.sliding.coincidences]
//! coupled = true`), the sector against the full-annulus tree run; ADR 0646's runs unchanged with the table where no
//! coincidence occurs; the default at coincidences unchanged. Checks A1, L1c, X0, X1, C3, M1, N1, F0 to F2, B1 (the
//! lockstep), R1 (the F-711 runs), T1, L1b, K0c (the fast run) and E1c. The limits are those registered in ADR 0654 before
//! this code ran; each test prints every measured number and then asserts the registered checks it carries.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::pimple::PimpleSolver;
use rcfd_fv::sliding_periodic::{
    periodic_rotor_wall_conditions, periodic_sliding_step, periodic_sliding_step_coupled, PeriodicSlidingStep,
};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{annulus_3d_sector, Grading};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use std::f64::consts::PI;
use std::time::Instant;

/// ADR 0637's step: the rotor pitch `pi / 72` over 8 (`pi / 576` in binary64).
const DT: f64 = 0.00545415391248228;
const DT_TEXT: &str = "0.00545415391248228";
/// FC: 400 steps, 2.08 pitches; coincidences on `n % 8 == 0`; re-imaging at 192 (offset 0) and 384 (arith.py).
const STEPS: usize = 400;
const REIMAGING: [usize; 2] = [192, 384];
/// The fast run FT: coincidences at 8 and 16.
const FAST_STEPS: usize = 16;
/// ADR 0646's runs (no coincidence): 440 steps of 0.005.
const R_STEPS: usize = 440;
const U0: f64 = 1.0;

/// ADR 0646's SP / TP and TS (`@GEN@`, `@SLIDING@`, `@INITIAL@`, `@INNER@`, `@OUTER@`, `@PLANES@`, `@DT@` replaced).
const BASE: &str = r#"[mesh]
@GEN@
ny = 8
nz = 1
ny_right = 8
nz_right = 1
r_in = 1.0
r_interface = 1.5
r_out = 2.0
lz = 0.1
@SLIDING@
[physics]
nu = 0.01

[initial]
@INITIAL@

[boundaries.inner]
@INNER@

[boundaries.outer]
@OUTER@

[boundaries.zmin]
@PLANES@

[boundaries.zmax]
@PLANES@

[solver]
algorithm = "piso"
convection = "central"
non_orthogonal_correctors = 6
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
momentum_max_iter = 5000
pressure_max_iter = 5000
dt = @DT@
end_time = 10.0
"#;

const SECTOR: &str =
    "generator = \"annulus_3d_sector_cylindrical_interface\"\nnx = 24\nnx_right = 20\nsector_angle = 60.0";
const ANNULUS: &str = "generator = \"annulus_3d_cylindrical_interface\"\nnx = 144\nnx_right = 120";
const WALL: &str = "velocity = \"fixed\"";
const SYM: &str = "velocity = \"symmetry\"";
const AXIAL: &str = "velocity = \"fixed\"\nvelocity_value = [0.0, 0.0, 1.0]";

const REST: &str = "\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"rest\"\n";
const ROTATING: &str = "\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"rotating\"\n";
const PLAIN: &str = "\n[mesh.sliding]\nangular_velocity = 1.0\n";
/// The opt-in table of ADR 0654 S1.
const COUPLED: &str = "\n[mesh.sliding.coincidences]\ncoupled = true\n";

/// FS's initial field (ADR 0646): the stream function `psi = C f(r) sin 6 theta`, `f = (r^2 - 1)(4 - r^2)`, `C = 1/12` m/s.
fn fs_initial() -> String {
    let r = "sqrt((x / 1 [m])^2 + (y / 1 [m])^2)";
    let t = "atan2(y / 1 [m], x / 1 [m])";
    let ux = "0.08333333333333333 [m/s] * (6 * (R^2 - 1) * (4 - R^2) / R * cos(6 * T) * cos(T) + 2 * R * (5 - 2 * R^2) * sin(6 * T) * sin(T))";
    let uy = "0.08333333333333333 [m/s] * (6 * (R^2 - 1) * (4 - R^2) / R * cos(6 * T) * sin(T) - 2 * R * (5 - 2 * R^2) * sin(6 * T) * cos(T))";
    let sub = |e: &str| e.replace('R', r).replace('T', t);
    format!(
        "velocity_expression_x = \"{}\"\nvelocity_expression_y = \"{}\"",
        sub(ux),
        sub(uy)
    )
}

#[allow(clippy::too_many_arguments)]
fn text(gen: &str, sliding: &str, initial: &str, inner: &str, outer: &str, planes: &str, dt: &str) -> String {
    BASE.replace("@GEN@", gen)
        .replace("@SLIDING@", sliding)
        .replace("@INITIAL@", initial)
        .replace("@INNER@", inner)
        .replace("@OUTER@", outer)
        .replace("@PLANES@", planes)
        .replace("@DT@", dt)
}

/// FS of ADR 0646 on `gen` with the step `dt`, the coincidence table appended to `[mesh.sliding]` when `coupled`.
fn fs(gen: &str, dt: &str, coupled: bool) -> String {
    let sliding = format!("{REST}{}", if coupled { COUPLED } else { "" });
    text(gen, &sliding, &fs_initial(), WALL, WALL, SYM, dt)
}

/// FW of ADR 0646 (sector, `dt = 0.005`).
fn fw(coupled: bool) -> String {
    let sliding = format!("{ROTATING}{}", if coupled { COUPLED } else { "" });
    text(SECTOR, &sliding, "velocity = [0.0, 0.0, 0.0]", WALL, WALL, SYM, "0.005")
}

/// US of ADR 0646 (sector, `dt = 0.005`).
fn us(coupled: bool) -> String {
    let sliding = format!("{PLAIN}{}", if coupled { COUPLED } else { "" });
    text(
        SECTOR,
        &sliding,
        "velocity = [0.0, 0.0, 1.0]",
        AXIAL,
        AXIAL,
        AXIAL,
        "0.005",
    )
}

fn case(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}\n{t}"))
}

fn sim_of(t: &str) -> Simulation {
    Simulation::new(&case(t)).unwrap_or_else(|e| panic!("{e:?}"))
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

fn last(sim: &Simulation, name: &str) -> f64 {
    *sim.monitors
        .get(name)
        .unwrap_or_else(|| panic!("no monitor {name}"))
        .y
        .last()
        .unwrap()
}

/// NaN counts as infinite in a running maximum.
fn fin(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

fn is_coincidence(n: usize) -> bool {
    n % 8 == 0
}

/// The largest value over all steps, over the coincidence steps and over the re-imaging steps.
#[derive(Debug, Default, Clone, Copy)]
struct Max {
    all: f64,
    coinc: f64,
    reim: f64,
}

impl Max {
    fn put(&mut self, x: f64, n: usize) {
        let x = fin(x);
        self.all = self.all.max(x);
        if is_coincidence(n) {
            self.coinc = self.coinc.max(x);
        }
        if REIMAGING.contains(&n) {
            self.reim = self.reim.max(x);
        }
    }
    fn within(&self, limit: f64) -> bool {
        self.all <= limit && self.coinc <= limit && self.reim <= limit
    }
    fn show(&self) -> String {
        format!(
            "{:.3e} (coincidence steps {:.3e}, re-imaging steps {:.3e})",
            self.all, self.coinc, self.reim
        )
    }
}

/// The library pair of SP (ADR 0646 FSL).
fn library_pair() -> PeriodicSlidingInterface {
    let angle = 60f64.to_radians();
    let a = annulus_3d_sector(24, 8, 1, 1.0, 1.5, 0.1, angle, Grading::Uniform).unwrap();
    let b = annulus_3d_sector(20, 8, 1, 1.5, 2.0, 0.1, angle, Grading::Uniform).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], angle, 1e-8).unwrap()
}

fn zoned(mut m: PeriodicSlidingMesh) -> PeriodicSlidingMesh {
    let n = m.n_rotor_cells();
    m.mesh.add_cell_zone("rotor", (0..n).collect()).unwrap();
    m
}

fn bits_eq(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// B1: the step data bit for bit.
fn step_data_equal(a: &PeriodicSlidingStep, b: &PeriodicSlidingStep) -> bool {
    bits_eq(&a.mesh_flux, &b.mesh_flux)
        && bits_eq(&a.phi_old, &b.phi_old)
        && bits_eq(&a.old_defect, &b.old_defect)
        && bits_eq(&a.vol_old, &b.vol_old)
        && a.gcl_residual.to_bits() == b.gcl_residual.to_bits()
        && a.flux_residual.to_bits() == b.flux_residual.to_bits()
        && a.images == b.images
        && a.reimage == b.reimage
}

/// What one library step reports.
struct LibStep {
    flux_residual: f64,
    /// The largest cell speed before the step (N1's scale).
    umax_before: f64,
    /// B1 (coupled loop only): the coupled step data equal the base step data from the same inputs.
    equal_to_base: Option<bool>,
}

/// ADR 0646's turned loop on the solver of a tree simulation that the tree never steps: LC (`coupled`, also building the
/// base step data from the same inputs every step) or LB (the base).
struct LibLoop {
    sc: PeriodicSlidingInterface,
    cur: PeriodicSlidingMesh,
    coupled: bool,
}

impl LibLoop {
    fn new(coupled: bool) -> Self {
        let sc = library_pair();
        let cur = zoned(sc.mesh_turned(0.0).unwrap());
        LibLoop { sc, cur, coupled }
    }

    fn step(&mut self, s: &mut PimpleSolver) -> LibStep {
        let umax_before = s.u.values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
        let turned = 1.0 * (s.time + DT);
        let new = zoned(self.sc.mesh_turned(turned).unwrap());
        let base = periodic_sliding_step(&s.geom, &self.cur, &new, &s.phi, &s.u.values, DT).unwrap();
        let (st, equal_to_base) = if self.coupled {
            let c = periodic_sliding_step_coupled(&s.geom, &self.cur, &new, &s.phi, &s.u.values, DT).unwrap();
            let eq = step_data_equal(&c, &base);
            (c, Some(eq))
        } else {
            (base, None)
        };
        let flux_residual = st.flux_residual;
        let g = s.geom.clone();
        let dpdn = periodic_rotor_wall_conditions(&mut s.u, &st, &g, &new, &["inner".to_string()], 0.0, DT).unwrap();
        s.step_sliding_periodic(DT, st, dpdn).unwrap();
        self.cur = new;
        assert!(
            s.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity"
        );
        LibStep {
            flux_residual,
            umax_before,
            equal_to_base,
        }
    }
}

/// F0: every sector cell's nearest annulus cell (largest distance, one-to-one).
fn matching(sector: &FvGeometry, annulus: &FvGeometry) -> (Vec<usize>, f64, bool) {
    let (cs, ca) = (sector.mesh.cell_centres(), annulus.mesh.cell_centres());
    let mut dist: f64 = 0.0;
    let map: Vec<usize> = cs
        .iter()
        .map(|x| {
            let (g, d) = ca
                .iter()
                .enumerate()
                .map(|(g, y)| (g, vec3::norm(vec3::sub(*x, *y))))
                .fold((usize::MAX, f64::INFINITY), |b, c| if c.1 < b.1 { c } else { b });
            dist = dist.max(d);
            g
        })
        .collect();
    let mut seen = vec![false; ca.len()];
    let one_to_one = map.iter().all(|&g| !std::mem::replace(&mut seen[g], true));
    (map, dist, one_to_one)
}

/// The largest difference and the volume-weighted rms of the sector velocity against the matched annulus velocity.
fn difference(us: &[Vec3], ua: &[Vec3], map: &[usize], vol: &[f64], scale: f64) -> (f64, f64) {
    let (mut mx, mut s2, mut vs) = (0.0f64, 0.0, 0.0);
    for c in 0..us.len() {
        let d = vec3::norm(vec3::sub(us[c], ua[map[c]])) / scale;
        mx = mx.max(d);
        s2 += vol[c] * d * d;
        vs += vol[c];
    }
    (mx, (s2 / vs).sqrt())
}

fn max_diff(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
        .fold(0.0, f64::max)
}

/// L1c / L1b: the tree's velocity and flux against a library loop's (velocity over `U0`, flux over `U0 max abs(S_f)`).
fn tree_vs_lib(tree: &Simulation, lib: &Simulation) -> (f64, f64, bool) {
    let sl = lib.pimple_solver().unwrap();
    let smax = sl
        .geom
        .mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max);
    let lu = fin(max_diff(&tree.velocity().values, &sl.u.values) / U0);
    let lp = fin(tree
        .flux()
        .iter()
        .zip(&sl.phi)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
        / (U0 * smax));
    (lu, lp, tree.flux().len() == sl.phi.len())
}

fn step_ok(sim: &mut Simulation) {
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
}

// -------------------------------------------------------------------------------------------------------------------
// The lockstep FC, FC-A, LC, LB (c1, c2, c3): A1, L1c, X0, X1, C3, M1, N1, F0, F1, F2, B1
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn fc_coupled_sector_at_pitch_over_8_against_the_full_annulus_tree_run() {
    let mut fails = Fails::new();
    let t0 = Instant::now();
    let mut sa = sim_of(&fs(ANNULUS, DT_TEXT, false));
    let mut sc = sim_of(&fs(SECTOR, DT_TEXT, true));
    let mut lc_sim = sim_of(&fs(SECTOR, DT_TEXT, true));
    let mut lb_sim = sim_of(&fs(SECTOR, DT_TEXT, false));
    let (mut lc, mut lb) = (LibLoop::new(true), LibLoop::new(false));
    let (mut a1, mut gcl, mut cont, mut net, mut nonorth_s) = (
        Max::default(),
        Max::default(),
        Max::default(),
        Max::default(),
        Max::default(),
    );
    let mut a1_step1 = f64::NAN;
    let (mut nonorth_a, mut x1_bad, mut reimaged) = (0.0f64, Vec::<usize>::new(), Vec::<usize>::new());
    let (mut n1_min, mut n1_min_abs, mut n1_count) = (f64::INFINITY, f64::INFINITY, 0usize);
    let mut lc_res = Max::default();
    let (mut b1_equal_off, mut b1_differ_on, mut b1_bad) = (0usize, 0usize, Vec::<usize>::new());
    let (mut f0_dist, mut f0_one) = (0.0f64, true);
    let (mut f1, mut f1_base, mut f1_cb) = (Max::default(), Max::default(), Max::default());
    let mut f1_before_first = 0.0f64;
    let mut rms_last = 0.0;
    for n in 1..=STEPS {
        let k0 = sc.monitors.get("sliding_image").map_or(0.0, |m| *m.y.last().unwrap());
        step_ok(&mut sa);
        step_ok(&mut sc);
        let lcs = lc.step(lc_sim.pimple_solver_mut().unwrap());
        let lbs = lb.step(lb_sim.pimple_solver_mut().unwrap());
        // the tree's records
        if last(&sc, "sliding_image") > k0 {
            reimaged.push(n);
        }
        let r = last(&sc, "sliding_flux_residual");
        if n == 1 {
            a1_step1 = r;
        } else {
            a1.put(r, n);
        }
        gcl.put(last(&sc, "sliding_gcl"), n);
        cont.put(last(&sc, "continuity"), n);
        nonorth_s.put(last(&sc, "sliding_non_orthogonality"), n);
        nonorth_a = nonorth_a.max(fin(last(&sa, "sliding_non_orthogonality")));
        let m = &sc.geom.mesh;
        let faces = sc.sliding_interface_faces().unwrap();
        let wraps = sc.sliding_wrap_pairs().unwrap();
        let area: f64 = faces
            .iter()
            .map(|&f| m.face_area(f))
            .chain(wraps.iter().map(|&(a, _)| m.face_area(a)))
            .sum();
        net.put(last(&sc, "sliding_interface_flux").abs() / (U0 * area), n);
        let want_s = if is_coincidence(n) { 40 } else { 44 };
        let want_a = if is_coincidence(n) { 240 } else { 264 };
        let no_wrap_ok = wraps.is_empty() == REIMAGING.contains(&n);
        if faces.len() + wraps.len() != want_s || !no_wrap_ok || sa.sliding_interface_faces().unwrap().len() != want_a {
            x1_bad.push(n);
        }
        // the library loops
        if n > 1 {
            lc_res.put(lcs.flux_residual, n);
        }
        if is_coincidence(n) {
            let ratio = lbs.flux_residual / lbs.umax_before;
            n1_min = n1_min.min(if ratio.is_nan() { 0.0 } else { ratio });
            n1_min_abs = n1_min_abs.min(lbs.flux_residual);
            n1_count += 1;
        }
        match (lcs.equal_to_base, is_coincidence(n)) {
            (Some(true), false) => b1_equal_off += 1,
            (Some(false), true) => b1_differ_on += 1,
            _ => b1_bad.push(n),
        }
        // F: the sector against the annulus
        let (map, d, one) = matching(&sc.geom, &sa.geom);
        f0_dist = f0_dist.max(d);
        f0_one &= one;
        let vol = sc.geom.mesh.cell_volumes();
        let ua = &sa.velocity().values;
        let (e, rms) = difference(&sc.velocity().values, ua, &map, vol, U0);
        f1.put(e, n);
        if n < 8 {
            f1_before_first = f1_before_first.max(e);
        }
        rms_last = rms;
        let ub = &lb_sim.pimple_solver().unwrap().u.values;
        f1_base.put(difference(ub, ua, &map, vol, U0).0, n);
        f1_cb.put(fin(max_diff(&sc.velocity().values, ub) / U0), n);
    }
    let secs = t0.elapsed().as_secs_f64();
    let angle = last(&sc, "sliding_angle");
    let image = last(&sc, "sliding_image");
    let umax_s = sc.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    let umax_a = sa.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    println!(
        "FC: {STEPS} steps in {secs:.1} s; final angle {angle:.6} rad, image {image}; largest |u| sector {umax_s:.4}, annulus {umax_a:.4}"
    );
    println!(
        "printed: step 1 sliding_flux_residual {a1_step1:.3e}; LC flux residual from step 2 {}",
        lc_res.show()
    );
    println!(
        "printed: F1 before the first coincidence step {f1_before_first:.3e}; base LB against the annulus {}; FC against LB {}",
        f1_base.show(),
        f1_cb.show()
    );
    check(
        &mut fails,
        "A1",
        a1.within(1e-12),
        format!("FC sliding_flux_residual, steps 2 to {STEPS}: {} (1e-12)", a1.show()),
    );
    let (lu, lp, same) = tree_vs_lib(&sc, &lc_sim);
    check(
        &mut fails,
        "L1c",
        lu <= 1e-12 && lp <= 1e-12 && same,
        format!("FC tree against LC after {STEPS} steps: velocity {lu:.3e}, flux {lp:.3e} (1e-12 each)"),
    );
    check(
        &mut fails,
        "X1",
        x1_bad.is_empty(),
        format!(
            "steps with sector pieces != 40 / 44, a wrap pair present or absent wrongly, or annulus faces != 240 / 264: {:?}",
            &x1_bad[..x1_bad.len().min(20)]
        ),
    );
    check(
        &mut fails,
        "C3",
        reimaged == REIMAGING && angle >= 2.0 * PI / 3.0 && image == 2.0,
        format!("re-imaging steps {reimaged:?} (exactly {REIMAGING:?}); final angle {angle:.6} (>= 2 pi/3), image {image} (2)"),
    );
    check(
        &mut fails,
        "X0",
        nonorth_s.all <= 70.0 && nonorth_a <= 70.0,
        format!(
            "worst interface non-orthogonality sector {:.2} deg, annulus {nonorth_a:.2} deg (70)",
            nonorth_s.all
        ),
    );
    check(
        &mut fails,
        "M1",
        net.within(1e-10) && gcl.within(1e-12) && cont.within(1e-9),
        format!(
            "FC every step net interface flux {} (1e-10), GCL {} (1e-12), continuity {} (1e-9)",
            net.show(),
            gcl.show(),
            cont.show()
        ),
    );
    check(
        &mut fails,
        "N1",
        n1_count == 50 && n1_min >= 1e-5,
        format!(
            "LB (base) flux_residual / max |u| on the {n1_count} coincidence steps, smallest {n1_min:.3e} (>= 1e-5); smallest absolute {n1_min_abs:.3e}"
        ),
    );
    check(
        &mut fails,
        "B1",
        b1_bad.is_empty() && b1_equal_off == 350 && b1_differ_on == 50,
        format!(
            "LC coupled step data equal to the base on {b1_equal_off} of 350 steps without coincidence, differing on {b1_differ_on} of 50 coincidence steps; other steps {:?}",
            &b1_bad[..b1_bad.len().min(20)]
        ),
    );
    check(
        &mut fails,
        "F0",
        f0_dist <= 1e-9 && f0_one,
        format!("every step largest match distance {f0_dist:.3e} (1e-9), one-to-one {f0_one}"),
    );
    check(
        &mut fails,
        "F1",
        f1.within(2e-2),
        format!("every step max |u_FC - u_FC-A| / U0 {} (2e-2)", f1.show()),
    );
    check(
        &mut fails,
        "F2",
        rms_last <= 1e-2,
        format!("last step rms {rms_last:.3e} (1e-2)"),
    );
    require(
        &fails,
        &["A1", "L1c", "X1", "C3", "X0", "M1", "N1", "B1", "F0", "F1", "F2"],
    );
}

// -------------------------------------------------------------------------------------------------------------------
// R1: ADR 0646's runs (no coincidence) unchanged with the table (c3)
// -------------------------------------------------------------------------------------------------------------------

/// The tree runs of `plain` and `coupled` in lockstep over `R_STEPS`: the steps after which they differ and the monitor
/// set check.
fn r1_pair(name: &str, plain: &str, coupled: &str) -> (Vec<usize>, bool) {
    let (mut a, mut b) = (sim_of(plain), sim_of(coupled));
    let mut differ = Vec::new();
    for n in 1..=R_STEPS {
        step_ok(&mut a);
        step_ok(&mut b);
        let ua: Vec<f64> = a.velocity().values.iter().flatten().copied().collect();
        let ub: Vec<f64> = b.velocity().values.iter().flatten().copied().collect();
        let mons = a.monitors.names().into_iter().all(|m| {
            b.monitors.get(m).is_some_and(|s| {
                s.y.last().map(|x| x.to_bits()) == a.monitors.get(m).unwrap().y.last().map(|x| x.to_bits())
            })
        });
        if !(bits_eq(&ua, &ub)
            && bits_eq(&a.field("p").unwrap(), &b.field("p").unwrap())
            && bits_eq(a.flux(), b.flux())
            && mons)
        {
            differ.push(n);
        }
    }
    let monitor_ok =
        a.monitors.get("sliding_flux_residual").is_none() && b.monitors.get("sliding_flux_residual").is_some();
    println!(
        "R1 {name}: {R_STEPS} steps, steps that differ {:?}; sliding_flux_residual largest (with the table) {:.3e}",
        &differ[..differ.len().min(20)],
        b.monitors
            .get("sliding_flux_residual")
            .map_or(f64::NAN, |s| s.y.iter().skip(1).fold(0.0f64, |m, x| m.max(*x)))
    );
    (differ, monitor_ok)
}

#[test]
fn r1_the_f711_runs_are_unchanged_with_the_table() {
    let t0 = Instant::now();
    let mut fails = Fails::new();
    for (name, plain, coupled) in [
        ("FS", fs(SECTOR, "0.005", false), fs(SECTOR, "0.005", true)),
        ("FW", fw(false), fw(true)),
        ("US", us(false), us(true)),
    ] {
        let (differ, monitor_ok) = r1_pair(name, &plain, &coupled);
        check(
            &mut fails,
            "R1",
            differ.is_empty() && monitor_ok,
            format!(
                "{name}: bit-equal on every step: {}; sliding_flux_residual only with the table: {monitor_ok}",
                differ.is_empty()
            ),
        );
    }
    println!("R1: {:.1} s", t0.elapsed().as_secs_f64());
    require(&fails, &["R1"]);
}

// -------------------------------------------------------------------------------------------------------------------
// FT: the fast run (T1, L1b, K0c)
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn ft_coupled_and_default_sector_steps_at_two_coincidences() {
    let mut fails = Fails::new();
    let (tc, tb) = (fs(SECTOR, DT_TEXT, true), fs(SECTOR, DT_TEXT, false));
    // K0c
    let (cc, cb) = (case(&tc), case(&tb));
    let data_c = cc.tree.to_json();
    let data_b = cb.tree.to_json();
    let k0 = data_b.pointer("/mesh/sliding/coincidences").is_none()
        && !cb.to_toml().contains("coincidences")
        && data_b.pointer("/mesh/sliding").is_some()
        && data_c.pointer("/mesh/sliding/coincidences/coupled") == Some(&serde_json::Value::Bool(true));
    check(
        &mut fails,
        "K0c",
        k0,
        format!(
            "FB data and text without coincidences, FC data with coupled = true: {k0} (FB sliding {:?})",
            data_b.pointer("/mesh/sliding")
        ),
    );
    let (mut fc, mut fb) = (sim_of(&tc), sim_of(&tb));
    let (mut lc_sim, mut lb_sim) = (sim_of(&tc), sim_of(&tb));
    let (mut lc, mut lb) = (LibLoop::new(true), LibLoop::new(false));
    let (mut res, mut n1) = (0.0f64, f64::INFINITY);
    for n in 1..=FAST_STEPS {
        step_ok(&mut fc);
        step_ok(&mut fb);
        lc.step(lc_sim.pimple_solver_mut().unwrap());
        let lbs = lb.step(lb_sim.pimple_solver_mut().unwrap());
        if n > 1 {
            res = res.max(fin(last(&fc, "sliding_flux_residual")));
        }
        if is_coincidence(n) {
            let ratio = lbs.flux_residual / lbs.umax_before;
            n1 = n1.min(if ratio.is_nan() { 0.0 } else { ratio });
        }
    }
    let (lu, lp, same) = tree_vs_lib(&fc, &lc_sim);
    check(
        &mut fails,
        "T1",
        lu <= 1e-12 && lp <= 1e-12 && same && res <= 1e-12 && n1 >= 1e-5,
        format!(
            "FC against LC after {FAST_STEPS} steps velocity {lu:.3e}, flux {lp:.3e} (1e-12); FC sliding_flux_residual steps 2 to {FAST_STEPS} {res:.3e} (1e-12); LB at steps 8, 16 flux_residual / max |u| {n1:.3e} (>= 1e-5)"
        ),
    );
    let (bu, bp, bsame) = tree_vs_lib(&fb, &lb_sim);
    let no_mon = fb.monitors.get("sliding_flux_residual").is_none();
    check(
        &mut fails,
        "L1b",
        bu == 0.0 && bp == 0.0 && bsame && no_mon,
        format!("FB (default) against LB after {FAST_STEPS} steps velocity {bu:.3e}, flux {bp:.3e} (0); no sliding_flux_residual: {no_mon}"),
    );
    require(&fails, &["K0c", "T1", "L1b"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E1c: the table on the full annulus
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn e1c_the_coincidence_table_is_refused_on_the_full_annulus() {
    let t = fs(ANNULUS, DT_TEXT, true);
    let c = case(&t);
    let path = "mesh.sliding.coincidences";
    let stat: Vec<String> = rcfd_api::dynmesh::check(&c).iter().map(ToString::to_string).collect();
    let new = match Simulation::new(&c) {
        Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        Ok(_) => "no error".into(),
    };
    let ok = stat.iter().any(|m| m.contains(path)) && new != "no error" && new.contains(path);
    println!(
        "E1c: static {stat:?}; new {new} -> {}",
        if ok { "met" } else { "MISSED" }
    );
    assert!(ok);
}
