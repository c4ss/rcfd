//! F-711 (ADR 0646): periodic (sector) sliding in the case tree (`[mesh.sliding]` on `mesh.generator =
//! "annulus_3d_sector_cylindrical_interface"`). Checks K1, K2, K0s, L0s, K3 (the generator and the keys), C3 (the run passes
//! two pitches with the re-imaging steps), L1s, L2s (the tree loop is ADR 0634's library loop), F0 to F4, FN1, FN2, X0, X1,
//! M1 (the sector against the full-annulus tree run of ADR 0637), U1s, U2s (uniform axial flow) and E1s, E2s (refusals).
//! The limits are those registered in ADR 0646 before this code ran; each test prints every measured number and then
//! asserts the registered checks it carries.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::pimple::PimpleSolver;
use rcfd_fv::sliding_periodic::{periodic_rotor_wall_conditions, periodic_sliding_step};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{annulus_3d_sector, Grading};
use rcfd_mesh::non_conformal::interface_non_orthogonality;
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::PI;
use std::time::Instant;

/// The step of ADR 0646 (no node coincidence in 440 steps, arith.py).
const DT: f64 = 0.005;
/// Steps of FS, FW, US: 2.2 rad, 2.10 pitches; re-imaging at 210 and 419.
const STEPS: usize = 440;
const REIMAGING: [usize; 2] = [210, 419];
const U0: f64 = 1.0;
const N_ROTOR_SECTOR: usize = 24 * 8;

/// SP / TP and TS of ADR 0646 (`@GEN@` the generator lines, `@SLIDING@`, `@INITIAL@`, `@PLANES@` replaced).
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
dt = 0.005
end_time = 10.0
"#;

const SECTOR: &str =
    "generator = \"annulus_3d_sector_cylindrical_interface\"\nnx = 24\nnx_right = 20\nsector_angle = 60.0";
const ANNULUS: &str = "generator = \"annulus_3d_cylindrical_interface\"\nnx = 144\nnx_right = 120";
const WALL: &str = "velocity = \"fixed\"";
const SYM: &str = "velocity = \"symmetry\"";
const AXIAL: &str = "velocity = \"fixed\"\nvelocity_value = [0.0, 0.0, 1.0]";

/// FS's initial field: the stream function `psi = C f(r) sin 6 theta`, `f = (r^2 - 1)(4 - r^2)`, `C = 1/12` m/s.
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

/// The same field in closed form (the set-up check of the initial cell values).
fn fs_exact(x: Vec3) -> Vec3 {
    let c = 1.0 / 12.0;
    let r = x[0].hypot(x[1]);
    let t = x[1].atan2(x[0]);
    let f = (r * r - 1.0) * (4.0 - r * r);
    let fp = 2.0 * r * (5.0 - 2.0 * r * r);
    let ur = 6.0 * c * f * (6.0 * t).cos() / r;
    let ut = -c * fp * (6.0 * t).sin();
    [ur * t.cos() - ut * t.sin(), ur * t.sin() + ut * t.cos(), 0.0]
}

struct Setup<'a> {
    generator: &'a str,
    sliding: &'a str,
    initial: String,
    inner: &'a str,
    outer: &'a str,
    planes: &'a str,
}

fn text(s: &Setup) -> String {
    BASE.replace("@GEN@", s.generator)
        .replace("@SLIDING@", s.sliding)
        .replace("@INITIAL@", &s.initial)
        .replace("@INNER@", s.inner)
        .replace("@OUTER@", s.outer)
        .replace("@PLANES@", s.planes)
}

const REST: &str = "\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"rest\"\n";
const ROTATING: &str = "\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"rotating\"\n";

/// FS (sector) or FS-A (annulus).
fn fs(generator: &str) -> String {
    text(&Setup {
        generator,
        sliding: REST,
        initial: fs_initial(),
        inner: WALL,
        outer: WALL,
        planes: SYM,
    })
}

/// FW (sector) or FW-A (annulus).
fn fw(generator: &str) -> String {
    text(&Setup {
        generator,
        sliding: ROTATING,
        initial: "velocity = [0.0, 0.0, 0.0]".into(),
        inner: WALL,
        outer: WALL,
        planes: SYM,
    })
}

/// US (with the table) or FZ (`sliding` empty: the frozen sector generator).
fn us(sliding: &str) -> String {
    text(&Setup {
        generator: SECTOR,
        sliding,
        initial: "velocity = [0.0, 0.0, 1.0]".into(),
        inner: AXIAL,
        outer: AXIAL,
        planes: AXIAL,
    })
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

/// A measure for a running maximum: NaN counts as infinite.
fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
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
        format!("{:.3e} (re-imaging steps {:.3e})", self.all, self.reim)
    }
}

/// Per-step records of a tree run of the sector.
#[derive(Debug, Default)]
struct SectorStats {
    net: Max,
    gcl: Max,
    cont: Max,
    non_orth: Max,
    pieces_bad: usize,
    no_wrap: usize,
    reimaged: Vec<usize>,
    u_dev: Max,
    p_dev: Max,
}

/// One tree step of a sector run and its records; `uniform` gives the U1s measures; `step` is the step number.
fn sector_step(sim: &mut Simulation, st: &mut SectorStats, step: usize, uniform: Option<Vec3>) {
    let k0 = sim.monitors.get("sliding_image").map_or(0.0, |m| *m.y.last().unwrap());
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let k1 = last(sim, "sliding_image");
    let reim = k1 > k0;
    if reim {
        st.reimaged.push(step);
    }
    let m = &sim.geom.mesh;
    let faces = sim.sliding_interface_faces().unwrap();
    let wraps = sim.sliding_wrap_pairs().unwrap();
    if faces.len() + wraps.len() != 44 {
        st.pieces_bad += 1;
    }
    if wraps.is_empty() {
        st.no_wrap += 1;
    }
    let area: f64 = faces
        .iter()
        .map(|&f| m.face_area(f))
        .chain(wraps.iter().map(|&(a, _)| m.face_area(a)))
        .sum();
    let scale = uniform.map_or(U0, vec3::norm);
    st.net
        .put(last(sim, "sliding_interface_flux").abs() / (scale * area), reim);
    st.gcl.put(last(sim, "sliding_gcl"), reim);
    st.cont.put(last(sim, "continuity"), reim);
    st.non_orth.put(last(sim, "sliding_non_orthogonality"), reim);
    if let Some(uu) = uniform {
        let u = &sim.velocity().values;
        let du = u.iter().map(|x| vec3::norm(vec3::sub(*x, uu))).fold(0.0, f64::max) / vec3::norm(uu);
        let p = sim.field("p").unwrap();
        let v = m.cell_volumes();
        let pbar = p.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
        let dp = p.iter().map(|x| (x - pbar).abs()).fold(0.0, f64::max) / vec3::dot(uu, uu);
        st.u_dev.put(du, reim);
        st.p_dev.put(dp, reim);
    }
}

/// One tree step of the full-annulus run: (interface faces, non-orthogonality).
fn annulus_step(sim: &mut Simulation) -> (usize, f64) {
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    (
        sim.sliding_interface_faces().unwrap().len(),
        last(sim, "sliding_non_orthogonality"),
    )
}

/// The library pair of SP (ADR 0646 FSL: the generator's blocks, grading 1, the tree's angle `60 deg` in radians).
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

/// ADR 0634's turned loop on the solver of a tree simulation that the tree never steps (FSL, FWL; with `drop_reimage`
/// the controls FSN, FWN: `st.reimage = None` after the wall conditions).
struct LibLoop {
    sc: PeriodicSlidingInterface,
    cur: PeriodicSlidingMesh,
    wall_omega: f64,
    drop_reimage: bool,
}

impl LibLoop {
    fn new(wall_omega: f64, drop_reimage: bool) -> Self {
        let sc = library_pair();
        let cur = zoned(sc.mesh_turned(0.0).unwrap());
        LibLoop {
            sc,
            cur,
            wall_omega,
            drop_reimage,
        }
    }

    fn step(&mut self, s: &mut PimpleSolver) {
        let turned = 1.0 * (s.time + DT);
        let new = zoned(self.sc.mesh_turned(turned).unwrap());
        let mut st = periodic_sliding_step(&s.geom, &self.cur, &new, &s.phi, &s.u.values, DT).unwrap();
        let g = s.geom.clone();
        let dpdn = periodic_rotor_wall_conditions(&mut s.u, &st, &g, &new, &["inner".to_string()], self.wall_omega, DT)
            .unwrap();
        if self.drop_reimage {
            st.reimage = None;
        }
        s.step_sliding_periodic(DT, st, dpdn).unwrap();
        self.cur = new;
        assert!(
            s.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity"
        );
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

/// The checks of one comparison (FS or FW): the sector and annulus tree runs, the library loop and the control in lockstep.
#[allow(clippy::too_many_arguments)]
fn comparison(
    fails: &mut Fails,
    name: &str,
    sector_text: &str,
    annulus_text: &str,
    wall_omega: f64,
    scale: f64,
    ids: [&'static str; 5],
) {
    let [id_f, id_rms, id_n, id_l, id_m] = ids;
    let t0 = Instant::now();
    let mut sa = sim_of(annulus_text);
    let mut ss = sim_of(sector_text);
    let mut lib_sim = sim_of(sector_text);
    let mut neg_sim = sim_of(sector_text);
    let (mut lib, mut neg) = (LibLoop::new(wall_omega, false), LibLoop::new(wall_omega, true));
    if name == "FS" {
        let g = &ss.geom;
        let dev = g
            .mesh
            .cell_centres()
            .iter()
            .zip(&ss.velocity().values)
            .map(|(x, u)| vec3::norm(vec3::sub(*u, fs_exact(*x))))
            .fold(0.0, f64::max);
        let umax = ss.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
        println!("{name} (set-up, printed): initial cell velocities against the closed form {dev:.2e}; largest |u| {umax:.4}");
    }
    let mut st = SectorStats::default();
    let (mut f0_dist, mut f0_one) = (0.0f64, true);
    let (mut f1, mut fneg, mut x_annulus_bad, mut nonorth_a) = (Max::default(), Max::default(), 0usize, 0.0f64);
    let (mut f1_first, mut f1_later) = (0.0f64, 0.0f64);
    let mut rms_last = 0.0;
    for n in 1..=STEPS {
        let (fa, na) = annulus_step(&mut sa);
        if fa != 264 {
            x_annulus_bad += 1;
        }
        nonorth_a = nonorth_a.max(na);
        sector_step(&mut ss, &mut st, n, None);
        lib.step(lib_sim.pimple_solver_mut().unwrap());
        neg.step(neg_sim.pimple_solver_mut().unwrap());
        let reim = REIMAGING.contains(&n);
        let (map, d, one) = matching(&ss.geom, &sa.geom);
        f0_dist = f0_dist.max(d);
        f0_one &= one;
        let vol = ss.geom.mesh.cell_volumes();
        let ua = &sa.velocity().values;
        let (e, rms) = difference(&ss.velocity().values, ua, &map, vol, scale);
        f1.put(e, reim);
        if n < REIMAGING[0] {
            f1_first = f1_first.max(e);
        } else {
            f1_later = f1_later.max(e);
        }
        rms_last = rms;
        let (en, _) = difference(&neg_sim.pimple_solver().unwrap().u.values, ua, &map, vol, scale);
        fneg.put(en, reim);
    }
    let secs = t0.elapsed().as_secs_f64();
    let angle = last(&ss, "sliding_angle");
    let image = last(&ss, "sliding_image");
    let umax_s = ss.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    let umax_a = sa.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    println!(
        "{name}: {STEPS} steps in {secs:.1} s; final angle {angle:.6} rad, image {image}; largest |u| sector {umax_s:.4}, annulus {umax_a:.4}; difference before the first re-imaging {f1_first:.3e}, from it {f1_later:.3e}"
    );
    check(
        fails,
        "F0",
        f0_dist <= 1e-9 && f0_one,
        format!("{name}: every step largest match distance {f0_dist:.3e} (1e-9), one-to-one {f0_one}"),
    );
    check(
        fails,
        id_f,
        f1.within(2e-2),
        format!(
            "{name}: every step max |u_sector - u_annulus| / scale {} (2e-2)",
            f1.show()
        ),
    );
    check(
        fails,
        id_rms,
        rms_last <= 1e-2,
        format!("{name}: last step rms {rms_last:.3e} (1e-2)"),
    );
    check(
        fails,
        id_n,
        fneg.all >= 2e-2 && fneg.all >= 5.0 * f1.all,
        format!(
            "{name} control (rotor vectors not re-imaged): {} (>= 2e-2 and >= 5 x {:.3e}; ratio {:.1})",
            fneg.show(),
            f1.all,
            fneg.all / f1.all.max(1e-300)
        ),
    );
    // L: the tree against the library loop
    let sl = lib_sim.pimple_solver().unwrap();
    let smax = sl
        .geom
        .mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max);
    let lu = max_diff(&ss.velocity().values, &sl.u.values) / U0;
    let lp = ss
        .flux()
        .iter()
        .zip(&sl.phi)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
        / (U0 * smax);
    check(
        fails,
        id_l,
        lu <= 1e-12 && lp <= 1e-12 && ss.flux().len() == sl.phi.len(),
        format!(
            "{name}: tree against the library loop after {STEPS} steps: velocity {lu:.3e}, flux {lp:.3e} (1e-12 each)"
        ),
    );
    if id_m == "M1" {
        check(
            fails,
            "M1",
            st.net.within(1e-10) && st.gcl.within(1e-12) && st.cont.within(1e-9),
            format!(
                "{name}: every step net interface flux {} (1e-10), GCL {} (1e-12), continuity {} (1e-9)",
                st.net.show(),
                st.gcl.show(),
                st.cont.show()
            ),
        );
    } else {
        println!(
            "{name} (printed): net interface flux {}, GCL {}, continuity {}",
            st.net.show(),
            st.gcl.show(),
            st.cont.show()
        );
    }
    check(
        fails,
        "X0",
        st.non_orth.all <= 70.0 && nonorth_a <= 70.0,
        format!(
            "{name}: worst interface non-orthogonality sector {:.2} deg, annulus {nonorth_a:.2} deg (70)",
            st.non_orth.all
        ),
    );
    check(
        fails,
        "X1",
        st.pieces_bad == 0 && st.no_wrap == 0 && x_annulus_bad == 0,
        format!(
            "{name}: steps with sector pieces != 44: {}, without a wrap pair: {}; annulus steps with interface faces != 264: {x_annulus_bad}",
            st.pieces_bad, st.no_wrap
        ),
    );
    check(
        fails,
        "C3",
        st.reimaged == REIMAGING && angle >= 2.0 * PI / 3.0 && image == 2.0,
        format!(
            "{name}: re-imaging steps {:?} (exactly {REIMAGING:?}); final angle {angle:.6} (>= 2 pi/3 = {:.6})",
            st.reimaged,
            2.0 * PI / 3.0
        ),
    );
}

// -------------------------------------------------------------------------------------------------------------------
// F: the sector against the full-annulus tree run (c2), with L1s, M1, X0, X1, C3
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn fs_sector_against_the_full_annulus_tree_run() {
    let mut fails = Fails::new();
    comparison(
        &mut fails,
        "FS",
        &fs(SECTOR),
        &fs(ANNULUS),
        0.0,
        U0,
        ["F1", "F2", "FN1", "L1s", "M1"],
    );
    require(&fails, &["F0", "F1", "F2", "FN1", "L1s", "M1", "X0", "X1", "C3"]);
}

#[test]
fn fw_turning_hub_sector_against_the_full_annulus_tree_run() {
    let mut fails = Fails::new();
    // the scale Omega r_in = 1
    comparison(
        &mut fails,
        "FW",
        &fw(SECTOR),
        &fw(ANNULUS),
        1.0,
        1.0,
        ["F3", "F4", "FN2", "L2s", "-"],
    );
    require(&fails, &["F0", "F3", "F4", "FN2", "L2s", "X0", "X1", "C3"]);
}

// -------------------------------------------------------------------------------------------------------------------
// U: uniform axial flow through the re-imaging steps
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn us_uniform_axial_flow_through_the_reimaging_steps() {
    let mut fails = Fails::new();
    let uu = [0.0, 0.0, 1.0];
    let mut sim = sim_of(&us(REST.replace("rotor_walls = \"rest\"\n", "").as_str()));
    let mut st = SectorStats::default();
    for n in 1..=STEPS {
        sector_step(&mut sim, &mut st, n, Some(uu));
    }
    check(
        &mut fails,
        "U1s",
        st.u_dev.within(1e-10) && st.p_dev.within(1e-10),
        format!(
            "every step velocity {}, pressure {} (1e-10 each)",
            st.u_dev.show(),
            st.p_dev.show()
        ),
    );
    check(
        &mut fails,
        "U2s",
        st.gcl.within(1e-12) && st.cont.within(1e-9) && st.net.within(1e-10),
        format!(
            "every step GCL {} (1e-12), continuity {} (1e-9), net interface flux {} (1e-10)",
            st.gcl.show(),
            st.cont.show(),
            st.net.show()
        ),
    );
    check(
        &mut fails,
        "X0",
        st.non_orth.all <= 70.0,
        format!("US worst interface non-orthogonality {:.2} deg (70)", st.non_orth.all),
    );
    println!(
        "US (printed): re-imaging steps {:?}; pieces != 44 on {} steps; final angle {:.6}",
        st.reimaged,
        st.pieces_bad,
        last(&sim, "sliding_angle")
    );
    require(&fails, &["U1s", "U2s", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// K: the generator and the keys
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn k_the_sector_generator_and_the_optional_table() {
    let mut fails = Fails::new();
    // K1: the frozen sector join of FZ
    let fz = us("");
    let m: Mesh = build_mesh(&case(&fz)).unwrap_or_else(|e| panic!("{e:?}"));
    let size = |n: &str| m.patch(n).map(|p| p.size);
    let zone_ok = m
        .cell_zones()
        .iter()
        .any(|z| z.name == "rotor" && z.ids == (0..N_ROTOR_SECTOR).collect::<Vec<_>>());
    let cyc = ["theta_min", "theta_max"].iter().all(|n| {
        m.patch(n).is_some_and(|p| {
            p.kind == PatchKind::Cyclic && p.rotation.is_some_and(|r| (r.angle.abs() - PI / 3.0).abs() <= 1e-12)
        })
    });
    let (own, nei) = (m.owner(), m.neighbour());
    let iface: Vec<usize> = (0..m.n_internal_faces())
        .filter(|&f| (own[f] < N_ROTOR_SECTOR) != (nei[f] < N_ROTOR_SECTOR))
        .collect();
    let q = interface_non_orthogonality(&m, &iface, 70.0).unwrap();
    let sizes = [
        size("inner"),
        size("outer"),
        size("zmin"),
        size("zmax"),
        size("theta_min"),
        size("theta_max"),
        size("sliding_wrap_a"),
        size("sliding_wrap_b"),
    ];
    let want = [24, 20, 352, 352, 16, 16, 0, 0].map(Some);
    check(
        &mut fails,
        "K1",
        m.n_cells() == 352 && zone_ok && cyc && sizes == want && iface.len() == 40 && !q.exceeds,
        format!(
            "{} cells, zone rotor 0..192 {zone_ok}, cyclic pair at 60 deg {cyc}; patches inner, outer, zmin, zmax, theta_min, theta_max, wrap a, wrap b {sizes:?}; direct interface faces {} (40); worst non-orthogonality {:.2} deg (70)",
            m.n_cells(),
            iface.len(),
            q.worst_deg
        ),
    );
    // K2: the frozen sector runs
    let mut s = sim_of(&fz);
    let (mut du, mut dp) = (0.0f64, 0.0f64);
    for _ in 0..5 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        let u = &s.velocity().values;
        du = du.max(
            u.iter()
                .map(|x| vec3::norm(vec3::sub(*x, [0.0, 0.0, 1.0])))
                .fold(0.0, f64::max),
        );
        let p = s.field("p").unwrap();
        let v = s.geom.mesh.cell_volumes();
        let pbar = p.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
        dp = dp.max(p.iter().map(|x| (x - pbar).abs()).fold(0.0, f64::max));
    }
    check(
        &mut fails,
        "K2",
        du <= 1e-10 && dp <= 1e-10,
        format!("FZ 5 steps: velocity {du:.3e}, pressure {dp:.3e} (1e-10 each)"),
    );
    // L0s: the sliding simulation's mesh at angle 0 against the frozen generator's
    let s1 = sim_of(&us(REST));
    let (h1, h0) = (s1.geom.mesh.content_hash(), m.content_hash());
    check(
        &mut fails,
        "L0s",
        h1 == h0,
        format!("sector sliding mesh at angle 0 {h1:#018x}, generator {h0:#018x}"),
    );
    check(
        &mut fails,
        "E2s",
        !s1.supports_restart(),
        format!("supports_restart {}", s1.supports_restart()),
    );
    // K0s: the case without the table
    let c0 = case(&fz);
    let (json0, toml0) = (c0.tree.to_json(), c0.to_toml());
    check(
        &mut fails,
        "K0s",
        json0.pointer("/mesh/sliding").is_none() && !toml0.contains("sliding"),
        format!(
            "no table: /mesh/sliding absent {}, to_toml without 'sliding' {}",
            json0.pointer("/mesh/sliding").is_none(),
            !toml0.contains("sliding")
        ),
    );
    // K3: the refusals of the other generators are unchanged
    let a = fs(ANNULUS)
        .replace(REST, "\n")
        .replace("nx_right = 120", "nx_right = 120\nsector_angle = 40.0");
    let b = "[mesh]\ngenerator = \"annulus_3d_sector\"\nnx = 8\nny = 4\nnz = 3\nr_in = 0.5\nr_out = 1.0\nlz = 0.3\nsector_angle = 40.0\nr_interface = 0.7\n";
    let la = build_mesh(&case(&a))
        .err()
        .map(|d| d.iter().map(|x| x.path.clone()).collect::<Vec<_>>());
    let lb = build_mesh(&case(b))
        .err()
        .map(|d| d.iter().map(|x| x.path.clone()).collect::<Vec<_>>());
    check(
        &mut fails,
        "K3",
        la.as_ref().is_some_and(|p| p.iter().any(|x| x == "mesh.sector_angle"))
            && lb.as_ref().is_some_and(|p| p.iter().any(|x| x == "mesh.r_interface")),
        format!("sector_angle on annulus_3d_cylindrical_interface: {la:?}; r_interface on annulus_3d_sector: {lb:?}"),
    );
    require(&fails, &["K1", "K2", "L0s", "E2s", "K0s", "K3"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E1s: refusals
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn e1s_refusals_are_located() {
    let base = fs(SECTOR);
    let cases: Vec<(String, &str)> = vec![
        (
            base.replace("angular_velocity = 1.0", "angular_velocity = -1.0"),
            "mesh.sliding.angular_velocity",
        ),
        (
            base.replace("sector_angle = 60.0", "sector_angle = 200.0"),
            "mesh.sector_angle",
        ),
        (base.replace("sector_angle = 60.0", ""), "mesh.sector_angle"),
        (
            base.replace(
                "[boundaries.inner]\nvelocity = \"fixed\"",
                "[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
            ),
            "boundaries.inner.velocity_value",
        ),
        (
            base.replace(
                "generator = \"annulus_3d_sector_cylindrical_interface\"",
                "generator = \"annulus_3d_sector\"",
            ),
            "mesh.generator",
        ),
        (format!("{base}\n[turbulence]\nmodel = \"k_epsilon\"\n"), "turbulence"),
    ];
    let mut bad = Vec::new();
    for (t, path) in &cases {
        assert_ne!(t, &base, "set-up of {path}");
        let msg = match Case::parse_toml(t) {
            Err(d) => format!(
                "parse: {}",
                d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            Ok(c) => {
                let sem = c.semantic_errors();
                if !sem.is_empty() {
                    format!(
                        "semantic: {}",
                        sem.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
                    )
                } else {
                    match Simulation::new(&c) {
                        Err(d) => format!(
                            "new: {}",
                            d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
                        ),
                        Ok(_) => "no error".into(),
                    }
                }
            }
        };
        let ok = msg != "no error" && (msg.contains(path) || msg.contains("mesh.sliding"));
        println!("E1s {path}: {} -> {msg}", if ok { "met" } else { "MISSED" });
        if !ok {
            bad.push(path.to_string());
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
