//! F-303 (ADR 0637): the sliding rotor-stator interface in the case tree (`[mesh.sliding]` on
//! `mesh.generator = "annulus_3d_cylindrical_interface"`). Checks K0, L0, D3 (the keys and the defaults), L1, L2 (the tree
//! loop is ADR 0216's library loop), N0, NA1 to NA3, M1, X0 (the non-axisymmetric flow through the turning rotor, which
//! sees the rotor mesh flux), U1, U2 (uniform flow), C3 (set-up) and E1, E2 (refusals). The limits are those registered in
//! ADR 0637 before this code ran; each test prints every measured number and then asserts the registered checks it
//! carries.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::pimple::PimpleSolver;
use rcfd_fv::sliding::{interface_balance, rotating_wall_conditions, sliding_step, SlidingStep};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{annulus_3d, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use std::sync::Arc;

/// `dt` = the rotor pitch over 8 (ADR 0637 arithmetic).
const DT: f64 = 0.00545415391248228;
/// Steps of NA: 6 rotor pitches.
const STEPS: usize = 48;
const NX: usize = 144;
const NY: usize = 8;
const N_ROTOR: usize = NX * NY;
const U0: f64 = 1.0;

/// TP and TS of ADR 0637 with the set-up's own tables (`{SLIDING}`, `{NU}`, `{INITIAL}`, `{INNER}`, `{OUTER}` replaced).
const BASE: &str = r#"[mesh]
generator = "annulus_3d_cylindrical_interface"
nx = 144
ny = 8
nz = 1
nx_right = 120
ny_right = 8
nz_right = 1
r_in = 1.0
r_interface = 1.5
r_out = 2.0
lz = 0.1
@SLIDING@
[physics]
nu = @NU@

[initial]
@INITIAL@

[boundaries.inner]
@INNER@

[boundaries.outer]
@OUTER@

[boundaries.zmin]
velocity = "symmetry"

[boundaries.zmax]
velocity = "symmetry"

[solver]
algorithm = "piso"
convection = "central"
non_orthogonal_correctors = 6
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
momentum_max_iter = 5000
pressure_max_iter = 5000
dt = 0.00545415391248228
end_time = 1.0
"#;

const STRAIN: &str = "velocity_expression_x = \"0.5 [1/s] * x\"\nvelocity_expression_y = \"-0.5 [1/s] * y\"";
const WALL: &str = "velocity = \"fixed\"";
/// NAc of ADR 0637 amendment 1: the field of the stream function `psi = (q - 1)(4 - q) x y / 24`, `q = x^2 + y^2`.
const PSI: &str = "velocity_expression_x = \"0.041666666666666664 [m/s] * (x / 1 [m]) * (2 * (y / 1 [m])^2 * (5 - 2 * ((x / 1 [m])^2 + (y / 1 [m])^2)) + ((x / 1 [m])^2 + (y / 1 [m])^2 - 1) * (4 - ((x / 1 [m])^2 + (y / 1 [m])^2)))\"\nvelocity_expression_y = \"-0.041666666666666664 [m/s] * (y / 1 [m]) * (2 * (x / 1 [m])^2 * (5 - 2 * ((x / 1 [m])^2 + (y / 1 [m])^2)) + ((x / 1 [m])^2 + (y / 1 [m])^2 - 1) * (4 - ((x / 1 [m])^2 + (y / 1 [m])^2)))\"";

fn text(sliding: &str, nu: &str, initial: &str, inner: &str, outer: &str) -> String {
    BASE.replace("@SLIDING@", sliding)
        .replace("@NU@", nu)
        .replace("@INITIAL@", initial)
        .replace("@INNER@", inner)
        .replace("@OUTER@", outer)
}

/// NA (with `walls` the value of `rotor_walls`) and NA0 (no table).
fn na(walls: &str) -> String {
    na_with(walls, STRAIN)
}

/// NA with the initial field `initial` (`STRAIN`: NA; `PSI`: NAc of amendment 1).
fn na_with(walls: &str, initial: &str) -> String {
    text(
        &format!("\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"{walls}\"\n"),
        "0.01",
        initial,
        WALL,
        WALL,
    )
}

fn na0_with(initial: &str) -> String {
    text("", "0.01", initial, WALL, WALL)
}

fn na0() -> String {
    na0_with(STRAIN)
}

/// UT, or UTc of amendment 1 (`fixed_planes`: zmin and zmax fixed at U in place of symmetry).
fn ut(fixed_planes: bool) -> String {
    let fixed = "velocity = \"fixed\"\nvelocity_value = [1.0, 0.5, 0.0]";
    let t = text(
        "\n[mesh.sliding]\nangular_velocity = 1.0\n",
        "0.1",
        "velocity = [1.0, 0.5, 0.0]",
        fixed,
        fixed,
    );
    if fixed_planes {
        t.replace("velocity = \"symmetry\"", fixed)
    } else {
        t
    }
}

/// WT.
fn wt() -> String {
    text(
        "\n[mesh.sliding]\nangular_velocity = 1.0\nrotor_walls = \"rotating\"\n",
        "0.01",
        "velocity = [0.0, 0.0, 0.0]",
        WALL,
        WALL,
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

/// Per-step records of a tree run.
#[derive(Debug, Default)]
struct TreeStats {
    net: f64,
    gcl: f64,
    cont: f64,
    non_orth: f64,
    changed: usize,
    u_dev: f64,
    p_dev: f64,
}

fn pairs(sim: &Simulation) -> Vec<(usize, usize)> {
    let m = &sim.geom.mesh;
    let mut v: Vec<(usize, usize)> = sim
        .sliding_interface_faces()
        .unwrap()
        .iter()
        .map(|&f| (m.owner()[f], m.neighbour()[f]))
        .collect();
    v.sort_unstable();
    v
}

fn last(sim: &Simulation, name: &str) -> f64 {
    *sim.monitors
        .get(name)
        .unwrap_or_else(|| panic!("no monitor {name}"))
        .y
        .last()
        .unwrap()
}

/// Run a tree simulation `steps` steps one at a time; `uniform` (velocity) gives the U1 measures.
fn run_tree(sim: &mut Simulation, steps: usize, uniform: Option<Vec3>) -> TreeStats {
    let mut st = TreeStats::default();
    let mut prev = pairs(sim);
    for _ in 0..steps {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let m = &sim.geom.mesh;
        let faces = sim.sliding_interface_faces().unwrap();
        let area: f64 = faces.iter().map(|&f| m.face_area(f)).sum();
        let scale = uniform.map_or(U0, vec3::norm);
        st.net = st.net.max(last(sim, "sliding_interface_flux").abs() / (scale * area));
        st.gcl = st.gcl.max(last(sim, "sliding_gcl"));
        st.cont = st.cont.max(last(sim, "continuity"));
        st.non_orth = st.non_orth.max(last(sim, "sliding_non_orthogonality"));
        let now = pairs(sim);
        if now != prev {
            st.changed += 1;
        }
        prev = now;
        if let Some(uu) = uniform {
            let u = &sim.velocity().values;
            let du = u.iter().map(|x| vec3::norm(vec3::sub(*x, uu))).fold(0.0, f64::max) / vec3::norm(uu);
            let p = sim.field("p").unwrap();
            let v = m.cell_volumes();
            let pbar = p.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
            let dp = p.iter().map(|x| (x - pbar).abs()).fold(0.0, f64::max) / vec3::dot(uu, uu);
            st.u_dev = st.u_dev.max(du);
            st.p_dev = st.p_dev.max(dp);
        }
    }
    st
}

/// The pair TP through the library (read 1 of ADR 0637: the generator's blocks with grading 1).
fn library_pair() -> SlidingInterface {
    let a = annulus_3d(NX, NY, 1, 1.0, 1.5, 0.1, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(120, 8, 1, 1.5, 2.0, 0.1, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap()
}

fn zoned(mut m: SlidingMesh) -> SlidingMesh {
    let n = m.n_rotor_cells();
    m.mesh.add_cell_zone("rotor", (0..n).collect()).unwrap();
    m
}

/// NAN's alteration: the rotor faces' mesh flux off the interface set to zero and the interface closure solved again (ADR
/// 0216 MU4 as a control).
fn drop_rotor_flux(st: &mut SlidingStep, cur_new: &SlidingMesh, dt: f64) {
    let m = &st.geom.mesh;
    let (n, nf, ni) = (m.n_cells(), m.n_faces(), m.n_internal_faces());
    for f in 0..nf {
        if cur_new.is_rotor_face(f) {
            st.mesh_flux[f] = 0.0;
        }
    }
    let v1 = m.cell_volumes();
    let mut t: Vec<f64> = (0..n).map(|c| (v1[c] - st.vol_old[c]) / dt).collect();
    for f in 0..nf {
        if !cur_new.is_interface(f) {
            t[m.owner()[f]] -= st.mesh_flux[f];
            if f < ni {
                t[m.neighbour()[f]] += st.mesh_flux[f];
            }
        }
    }
    let sol = interface_balance(m, &st.interface_faces, &t).unwrap();
    for (k, &f) in st.interface_faces.iter().enumerate() {
        st.mesh_flux[f] = sol.values[k];
    }
}

/// ADR 0216's loop SL on the solver of a tree simulation that the tree never steps (NAL, WL, NAN).
fn library_loop(s: &mut PimpleSolver, omega: f64, wall_omega: f64, steps: usize, drop: bool) {
    let sc = library_pair();
    let mut cur = zoned(sc.mesh_at(0.0).unwrap());
    for _ in 0..steps {
        let theta = omega * (s.time + DT);
        let new = zoned(sc.mesh_at(theta).unwrap());
        let mut st = sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, DT).unwrap();
        if drop {
            drop_rotor_flux(&mut st, &new, DT);
        }
        let g = s.geom.clone();
        let dpdn = rotating_wall_conditions(&mut s.u, &st, &g, &["inner".to_string()], wall_omega, DT).unwrap();
        s.step_sliding(DT, st, dpdn).unwrap();
        cur = new;
    }
}

fn max_diff(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
        .fold(0.0, f64::max)
}

/// The cell of NA0 at the place of cell `c` of the final NA mesh (N1: rotor cells shifted by 6 pitches).
fn mapped(c: usize) -> usize {
    if c < N_ROTOR {
        let (ir, it) = (c % NY, c / NY);
        ir + NY * ((it + 6) % NX)
    } else {
        c
    }
}

/// `(E all, E rotor, E stator, rms)` of a final sliding state against NA0.
fn e_against(u: &[Vec3], u0: &[Vec3], vol: &[f64]) -> (f64, f64, f64, f64) {
    let (mut all, mut rot, mut sta, mut s2, mut vs) = (0.0f64, 0.0f64, 0.0f64, 0.0, 0.0);
    for c in 0..u.len() {
        let d = vec3::norm(vec3::sub(u[c], u0[mapped(c)])) / U0;
        all = all.max(d);
        if c < N_ROTOR {
            rot = rot.max(d);
        } else {
            sta = sta.max(d);
        }
        s2 += vol[c] * d * d;
        vs += vol[c];
    }
    (all, rot, sta, (s2 / vs).sqrt())
}

/// NA0 run 48 steps.
fn na0_state(initial: &str) -> (Vec<Vec3>, Arc<FvGeometry>, Vec<f64>) {
    let mut s0 = sim_of(&na0_with(initial));
    s0.run(STEPS);
    assert!(s0.failure().is_none(), "{:?}", s0.failure());
    (s0.velocity().values.clone(), s0.geom.clone(), s0.flux().to_vec())
}

// -------------------------------------------------------------------------------------------------------------------
// K0, L0, D3: the keys and the defaults
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn k0_l0_the_optional_table() {
    let mut fails = Fails::new();
    let c0 = case(&na0());
    let json0 = c0.tree.to_json();
    let toml0 = c0.to_toml();
    check(
        &mut fails,
        "K0",
        json0.pointer("/mesh/sliding").is_none() && !toml0.contains("sliding"),
        format!(
            "no table: /mesh/sliding absent {}, to_toml without 'sliding' {}",
            json0.pointer("/mesh/sliding").is_none(),
            !toml0.contains("sliding")
        ),
    );
    let c1 = case(&na("rest"));
    let walls = c1.tree.get("mesh.sliding.rotor_walls").unwrap();
    let empty = case(&text("\n[mesh.sliding]\n", "0.01", STRAIN, WALL, WALL));
    let (w0, d0) = (
        empty.tree.get("mesh.sliding.angular_velocity").unwrap(),
        empty.tree.get("mesh.sliding.rotor_walls").unwrap(),
    );
    let typo = Case::parse_toml(&text(
        "\n[mesh.sliding]\nangular_velocty = 1.0\n",
        "0.01",
        STRAIN,
        WALL,
        WALL,
    ));
    let typo_msg = match &typo {
        Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        Ok(_) => "parsed".into(),
    };
    check(
        &mut fails,
        "K0",
        walls == "rest" && w0 == 0.0 && d0 == "rotating" && typo.is_err() && typo_msg.contains("sliding"),
        format!("NA rotor_walls {walls}; empty table angular_velocity {w0}, rotor_walls {d0}; unknown key: {typo_msg}"),
    );
    // L0: the sliding simulation's mesh at angle 0 against the frozen generator's
    let s1 = sim_of(&na("rest"));
    let (h1, h0) = (s1.geom.mesh.content_hash(), build_mesh(&c0).unwrap().content_hash());
    check(
        &mut fails,
        "L0",
        h1 == h0,
        format!("sliding mesh at angle 0 {h1:#018x}, generator {h0:#018x}"),
    );
    // E2: no restart files
    check(
        &mut fails,
        "E2",
        !s1.supports_restart(),
        format!("supports_restart {}", s1.supports_restart()),
    );
    require(&fails, &["K0", "L0", "E2"]);
}

#[test]
fn d3_the_frozen_path_is_deterministic() {
    let (a, _, pa) = na0_state(STRAIN);
    let (b, _, pb) = na0_state(STRAIN);
    let same = a
        .iter()
        .flatten()
        .zip(b.iter().flatten())
        .all(|(x, y)| x.to_bits() == y.to_bits())
        && pa.iter().zip(&pb).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("D3: NA0 twice, equal bit for bit {same}");
    assert!(same);
}

// -------------------------------------------------------------------------------------------------------------------
// N, M1, X0, C3, L1: the non-axisymmetric flow through the turning rotor
// -------------------------------------------------------------------------------------------------------------------

/// The checks N0, NA1 to NA3, L1, M1, X0, C3 on NA (`STRAIN`) or NAc (`PSI`).
fn na_checks(initial: &str) -> Fails {
    let mut fails = Fails::new();
    let (u0, g0, _) = na0_state(initial);
    // NA through the tree
    let mut sim = sim_of(&na_with("rest", initial));
    let ts = run_tree(&mut sim, STEPS, None);
    let angle = last(&sim, "sliding_angle");
    // N0: the cell map
    let (cc, cc0) = (sim.geom.mesh.cell_centres(), g0.mesh.cell_centres());
    let n0 = (0..cc.len())
        .map(|c| vec3::norm(vec3::sub(cc[c], cc0[mapped(c)])))
        .fold(0.0, f64::max);
    check(
        &mut fails,
        "N0",
        n0 <= 1e-9,
        format!("final angle {angle:.15} rad; largest centre distance under the map {n0:.3e} (1e-9)"),
    );
    let u = sim.velocity().values.clone();
    let vol = sim.geom.mesh.cell_volumes().to_vec();
    let (e, er, es, rms) = e_against(&u, &u0, &vol);
    let umax0 = u0.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    let umax = u.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    println!("NA: E rotor {er:.3e}, stator {es:.3e}, rms {rms:.3e}; largest |u|/U0 NA0 {umax0:.4}, NA {umax:.4}");
    check(&mut fails, "NA1", e <= 1e-2, format!("E = {e:.3e} (1e-2)"));
    // L1: the library loop on a second tree simulation's solver
    let mut lib = sim_of(&na_with("rest", initial));
    library_loop(lib.pimple_solver_mut().unwrap(), 1.0, 0.0, STEPS, false);
    let sl = lib.pimple_solver().unwrap();
    let smax = sl
        .geom
        .mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max);
    let l1u = max_diff(&u, &sl.u.values) / U0;
    let l1p = sim
        .flux()
        .iter()
        .zip(&sl.phi)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
        / (U0 * smax);
    check(
        &mut fails,
        "L1",
        l1u <= 1e-12 && l1p <= 1e-12 && sim.flux().len() == sl.phi.len(),
        format!("tree against library loop: velocity {l1u:.3e}, flux {l1p:.3e} (1e-12 each)"),
    );
    // NAN: the rotor mesh flux dropped
    let mut neg = sim_of(&na_with("rest", initial));
    library_loop(neg.pimple_solver_mut().unwrap(), 1.0, 0.0, STEPS, true);
    let un = neg.pimple_solver().unwrap().u.values.clone();
    let (en, enr, ens, rmsn) = e_against(&un, &u0, &vol);
    println!("NAN: E rotor {enr:.3e}, stator {ens:.3e}, rms {rmsn:.3e}");
    check(
        &mut fails,
        "NA2",
        en >= 5e-2,
        format!("E with the rotor mesh flux dropped {en:.3e} (>= 5e-2)"),
    );
    check(
        &mut fails,
        "NA3",
        en >= 5.0 * e,
        format!("E_NAN / E_NA = {:.2} (>= 5)", en / e.max(1e-300)),
    );
    check(
        &mut fails,
        "M1",
        ts.net <= 1e-10 && ts.gcl <= 1e-12 && ts.cont <= 1e-9,
        format!(
            "every step: net interface flux {:.3e} (1e-10), GCL {:.3e} (1e-12), continuity {:.3e} (1e-9)",
            ts.net, ts.gcl, ts.cont
        ),
    );
    check(
        &mut fails,
        "X0",
        ts.non_orth <= 70.0,
        format!("worst interface non-orthogonality {:.2} deg (70)", ts.non_orth),
    );
    check(
        &mut fails,
        "C3",
        ts.changed >= 1 && angle >= 2.0 * std::f64::consts::PI / 120.0,
        format!(
            "{} of {STEPS} steps changed the interface pairs; angle {angle:.4} against the stator pitch 0.0524",
            ts.changed
        ),
    );
    fails
}

const NA_IDS: [&str; 8] = ["N0", "NA1", "L1", "NA2", "NA3", "M1", "X0", "C3"];

/// NA as registered: KNOWN MISS of NA2 and NA3 (ADR 0637 amendment 1: the strain field is a gradient field, which the
/// projection removes; NA0's largest speed at the end is 0.0427 U0). Registered run: NA2 `6.20e-3` (limit `>= 5e-2`), NA3
/// `0.90` (limit `>= 5`); N0, NA1 (`6.86e-3`), L1, M1, X0, C3 met. Run with `--ignored`; it fails on NA2 and NA3.
#[test]
#[ignore]
fn na_known_miss_strain_field_removed_by_the_projection() {
    let fails = na_checks(STRAIN);
    require(&fails, &NA_IDS);
}

/// NAc of ADR 0637 amendment 1 (the stream-function field, which the projection keeps): N0c, NA1c, NA2c, NA3c, L1c, M1c,
/// X0c, C3c.
#[test]
fn nac_non_axisymmetric_flow_sees_the_rotor_mesh_flux() {
    let fails = na_checks(PSI);
    require(&fails, &NA_IDS);
}

// -------------------------------------------------------------------------------------------------------------------
// U: uniform flow through the tree
// -------------------------------------------------------------------------------------------------------------------

/// The checks U1, U2, X0 on UT (`fixed_planes` false) or UTc (true).
fn ut_checks(fixed_planes: bool) -> Fails {
    let mut fails = Fails::new();
    let uu = [1.0, 0.5, 0.0];
    let mut sim = sim_of(&ut(fixed_planes));
    let ts = run_tree(&mut sim, 40, Some(uu));
    check(
        &mut fails,
        "U1",
        ts.u_dev <= 1e-10 && ts.p_dev <= 1e-10,
        format!(
            "every step velocity {:.3e}, pressure {:.3e} (1e-10 each)",
            ts.u_dev, ts.p_dev
        ),
    );
    check(
        &mut fails,
        "U2",
        ts.gcl <= 1e-12 && ts.cont <= 1e-9,
        format!(
            "every step GCL {:.3e} (1e-12), continuity {:.3e} (1e-9); net interface flux {:.3e}",
            ts.gcl, ts.cont, ts.net
        ),
    );
    check(
        &mut fails,
        "X0",
        ts.non_orth <= 70.0,
        format!(
            "UT worst interface non-orthogonality {:.2} deg; angle {:.4}",
            ts.non_orth,
            last(&sim, "sliding_angle")
        ),
    );
    fails
}

/// UT as registered: KNOWN MISS of U1 (ADR 0637 amendment 1: the tree's symmetry planes do not keep the uniform in-plane
/// flow on this mesh, with or without the table). Registered run: velocity `7.24e-2`, pressure `2.35e1` (limit `1e-10`
/// each); U2, X0 met. Run with `--ignored`; it fails on U1.
#[test]
#[ignore]
fn ut_known_miss_symmetry_planes() {
    let fails = ut_checks(false);
    require(&fails, &["U1", "U2", "X0"]);
}

/// UTc of ADR 0637 amendment 1 (zmin, zmax fixed at U): U1c, U2c, X0c.
#[test]
fn utc_uniform_flow_through_the_tree() {
    let fails = ut_checks(true);
    require(&fails, &["U1", "U2", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// L2: the rotating rotor wall
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn l2_rotating_rotor_wall_tree_equals_library() {
    let mut fails = Fails::new();
    let mut sim = sim_of(&wt());
    let ts = run_tree(&mut sim, 20, None);
    let mut lib = sim_of(&wt());
    library_loop(lib.pimple_solver_mut().unwrap(), 1.0, 1.0, 20, false);
    let sl = lib.pimple_solver().unwrap();
    let smax = sl
        .geom
        .mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max);
    let du = max_diff(&sim.velocity().values, &sl.u.values) / U0;
    let dp = sim
        .flux()
        .iter()
        .zip(&sl.phi)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
        / (U0 * smax);
    let umax = sim.velocity().values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    check(
        &mut fails,
        "L2",
        du <= 1e-12 && dp <= 1e-12,
        format!("WT against WL: velocity {du:.3e}, flux {dp:.3e} (1e-12 each); largest |u| {umax:.4} (the wall drives the flow)"),
    );
    check(
        &mut fails,
        "X0",
        ts.non_orth <= 70.0,
        format!("WT worst interface non-orthogonality {:.2} deg", ts.non_orth),
    );
    require(&fails, &["L2", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E1: refusals
// -------------------------------------------------------------------------------------------------------------------

#[test]
fn e1_refusals_are_located() {
    let base = na("rest");
    let cases: Vec<(String, &str)> = vec![
        (
            base.replace("algorithm = \"piso\"", "algorithm = \"simplec\""),
            "solver.algorithm",
        ),
        (
            base.replace("end_time = 1.0", "end_time = 1.0\ntime_scheme = \"bdf2\""),
            "solver.time_scheme",
        ),
        (format!("{base}\n[turbulence]\nmodel = \"k_epsilon\"\n"), "turbulence"),
        (
            format!("{base}\n[dynamic_mesh]\nenabled = true\nmoving_patches = [\"inner\"]\n"),
            "dynamic_mesh.enabled",
        ),
        (
            base.replace(
                "[boundaries.inner]\nvelocity = \"fixed\"",
                "[boundaries.inner]\nvelocity = \"moving_wall\"",
            ),
            "boundaries.inner.velocity",
        ),
        (
            base.replace(
                "[boundaries.outer]\nvelocity = \"fixed\"",
                "[boundaries.outer]\nvelocity = \"fixed\"\nvelocity_expression_x = \"0.1 [m/s] * t / 1 [s]\"",
            ),
            "boundaries",
        ),
        (
            base.replace(
                "generator = \"annulus_3d_cylindrical_interface\"",
                "generator = \"annulus_3d\"",
            ),
            "mesh.generator",
        ),
        (
            format!("{base}\n[mesh.transform]\nrotate_z_deg = 10.0\n"),
            "mesh.transform",
        ),
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
        println!("E1 {path}: {} -> {msg}", if ok { "met" } else { "MISSED" });
        if !ok {
            bad.push(path.to_string());
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
