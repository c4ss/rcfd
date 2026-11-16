//! F-712 (ADR 0650): the uniform in-plane flow of ADR 0637 UT on the frozen join of `annulus_3d_cylindrical_interface` with
//! `zmin`, `zmax` symmetry. Diagnosis (ADR 0650 D-a, D-b): the averaged component diagonal acts as a uniform drag
//! `k = 4 nu / (3 lz^2)` on the in-plane components of the one-cell-deep pair; the pressure balances it with a linear field,
//! whose Green-Gauss cell gradient is wrong in the wall cells. Checks A1 to A8 and B2; the limits are those registered in
//! ADR 0650 before this code ran. Each test prints every measured number and then asserts its registered checks.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};

const U: Vec3 = [1.0, 0.5, 0.0];
/// `k = 4 nu / (3 lz^2)` with `nu = 0.1`, `lz = 0.1` (ADR 0650 D-a).
const K_DRAG: f64 = 4.0 * 0.1 / (3.0 * 0.1 * 0.1);

/// The frozen join of ADR 0637's TP with UT's flow (`@SLIDING@`, `@PLANES@`, `@SOLVER@` replaced).
const PAIR: &str = r#"[mesh]
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
"#;

/// The conformal annulus of UA (no interface).
const CONFORMAL: &str = r#"[mesh]
generator = "annulus_3d"
nx = 144
ny = 16
nz = 1
r_in = 1.0
r_out = 2.0
lz = 0.1
"#;

const REST: &str = r#"[physics]
nu = 0.1

[initial]
velocity = [1.0, 0.5, 0.0]

[boundaries.inner]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[boundaries.outer]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[boundaries.zmin]
velocity = "@PLANES@"

[boundaries.zmax]
velocity = "@PLANES@"

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
@SOLVER@
"#;

const CORRECTED: &str = "component_diagonal = \"corrected\"";

/// A case text: `mesh` (`PAIR` or `CONFORMAL`), `sliding` (empty: the frozen join), `planes` (`symmetry` or
/// `zero_gradient`), `solver` (extra solver lines).
fn text(mesh: &str, sliding: &str, planes: &str, solver: &str) -> String {
    let t = format!("{}{}", mesh.replace("@SLIDING@", sliding), REST);
    t.replace("@PLANES@", planes).replace("@SOLVER@", solver)
}

fn sim_of(t: &str) -> Simulation {
    let case = Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}\n{t}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The deviations of the current state from the uniform one: `(du, dp, cell of du)`.
fn deviation(sim: &Simulation) -> (f64, f64, usize) {
    let m = &sim.geom.mesh;
    let u = &sim.velocity().values;
    let (mut du, mut at) = (0.0, 0);
    for (c, x) in u.iter().enumerate() {
        let d = vec3::norm(vec3::sub(*x, U)) / vec3::norm(U);
        if d > du {
            du = d;
            at = c;
        }
    }
    let p = sim.field("p").unwrap();
    let v = m.cell_volumes();
    let pbar = p.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
    let dp = p.iter().map(|x| (x - pbar).abs()).fold(0.0, f64::max) / vec3::dot(U, U);
    (du, dp, at)
}

fn last(sim: &Simulation, name: &str) -> f64 {
    *sim.monitors
        .get(name)
        .unwrap_or_else(|| panic!("no monitor {name}"))
        .y
        .last()
        .unwrap()
}

/// Every-step largest `(du, dp, continuity)` over `steps` steps.
fn run(sim: &mut Simulation, steps: usize) -> (f64, f64, f64) {
    let (mut du, mut dp, mut cont) = (0.0f64, 0.0f64, 0.0f64);
    for _ in 0..steps {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let (a, b, _) = deviation(sim);
        du = du.max(a);
        dp = dp.max(b);
        cont = cont.max(last(sim, "continuity"));
    }
    (du, dp, cont)
}

/// The cells that own a face of `inner` or `outer`.
fn wall_cells(sim: &Simulation) -> Vec<usize> {
    let m = &sim.geom.mesh;
    let mut v: Vec<usize> = m
        .patches()
        .iter()
        .filter(|p| p.name == "inner" || p.name == "outer")
        .flat_map(|p| p.faces().map(|f| m.owner()[f]))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Volume-weighted least-squares fit `p = c + b . (x, y)`; returns `b`.
fn pressure_slope(sim: &Simulation) -> [f64; 2] {
    let m = &sim.geom.mesh;
    let p = sim.field("p").unwrap();
    let (xc, v) = (m.cell_centres(), m.cell_volumes());
    // normal equations of the basis (1, x, y)
    let mut a = [[0.0f64; 3]; 3];
    let mut r = [0.0f64; 3];
    for c in 0..p.len() {
        let phi = [1.0, xc[c][0], xc[c][1]];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += v[c] * phi[i] * phi[j];
            }
            r[i] += v[c] * phi[i] * p[c];
        }
    }
    // Gaussian elimination (3 x 3, well conditioned: the annulus is centred)
    for i in 0..3 {
        for j in (i + 1)..3 {
            let f = a[j][i] / a[i][i];
            for l in i..3 {
                a[j][l] -= f * a[i][l];
            }
            r[j] -= f * r[i];
        }
    }
    let mut x = [0.0f64; 3];
    for i in (0..3).rev() {
        let s: f64 = ((i + 1)..3).map(|l| a[i][l] * x[l]).sum();
        x[i] = (r[i] - s) / a[i][i];
    }
    [x[1], x[2]]
}

fn assert_uniform(id: &str, (du, dp, cont): (f64, f64, f64), with_cont: bool) {
    println!("{id}: every step du {du:.3e}, dp {dp:.3e} (1e-10 each); continuity {cont:.3e}");
    assert!(du <= 1e-10 && dp <= 1e-10, "{id}: du {du:.3e}, dp {dp:.3e}");
    if with_cont {
        assert!(cont <= 1e-9, "{id}: continuity {cont:.3e}");
    }
}

/// A1: UFc (the frozen join, symmetry planes, `corrected`), 40 steps.
#[test]
fn a1_frozen_join_symmetry_planes_corrected_diagonal_keeps_the_uniform_flow() {
    let mut sim = sim_of(&text(PAIR, "", "symmetry", CORRECTED));
    assert!(sim.sliding_interface_faces().is_none());
    assert_uniform("A1", run(&mut sim, 40), true);
}

/// A2, A3: UF (the frozen join, symmetry planes, the default diagonal) after one step: the miss of ADR 0637 and the pressure
/// slope of the drag `-k U`.
#[test]
fn a2_a3_frozen_join_default_diagonal_is_a_drag() {
    let mut sim = sim_of(&text(PAIR, "", "symmetry", ""));
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let (du, dp, at) = deviation(&sim);
    let b = pressure_slope(&sim);
    let want = [-K_DRAG * U[0], -K_DRAG * U[1]];
    let rel = (b[0] - want[0]).hypot(b[1] - want[1]) / (K_DRAG * vec3::norm(U));
    let walls = wall_cells(&sim);
    let c = sim.geom.mesh.cell_centres()[at];
    println!(
        "A2: step 1 du {du:.4e} (in [2.35e-2, 2.37e-2]); dp {dp:.3e}; at cell {at} r = {:.4}, wall cell {}",
        c[0].hypot(c[1]),
        walls.binary_search(&at).is_ok()
    );
    println!(
        "A3: pressure slope ({:.4}, {:.4}) against -k U = ({:.4}, {:.4}), relative {rel:.3e} (0.10)",
        b[0], b[1], want[0], want[1]
    );
    assert!((2.35e-2..=2.37e-2).contains(&du), "A2: du {du:.4e}");
    assert!(rel <= 0.10, "A3: relative {rel:.3e}");
}

/// A4: UFz (the frozen join, zero-gradient planes, the default diagonal), 40 steps.
#[test]
fn a4_frozen_join_zero_gradient_planes_keep_the_uniform_flow() {
    let mut sim = sim_of(&text(PAIR, "", "zero_gradient", ""));
    assert_uniform("A4", run(&mut sim, 40), false);
}

/// A5, A6: UA (conformal annulus, symmetry planes, the default diagonal) after one step.
#[test]
fn a5_a6_conformal_annulus_default_diagonal_misses_in_a_wall_cell() {
    let mut sim = sim_of(&text(CONFORMAL, "", "symmetry", ""));
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let (du, dp, at) = deviation(&sim);
    let walls = wall_cells(&sim);
    let wall = walls.binary_search(&at).is_ok();
    let c = sim.geom.mesh.cell_centres()[at];
    let b = pressure_slope(&sim);
    println!(
        "A5: step 1 du {du:.4e} (in [1e-2, 4e-2]); dp {dp:.3e}; pressure slope ({:.4}, {:.4})",
        b[0], b[1]
    );
    println!(
        "A6: largest at cell {at}, r = {:.4}, wall cell {wall}",
        c[0].hypot(c[1])
    );
    assert!((1e-2..=4e-2).contains(&du), "A5: du {du:.4e}");
    assert!(wall, "A6: cell {at} is not a wall cell");
}

/// A7: UAc (conformal annulus, symmetry planes, `corrected`), 40 steps.
#[test]
fn a7_conformal_annulus_corrected_diagonal_keeps_the_uniform_flow() {
    let mut sim = sim_of(&text(CONFORMAL, "", "symmetry", CORRECTED));
    assert_uniform("A7", run(&mut sim, 40), true);
}

/// A8: USc (ADR 0637's UT through `[mesh.sliding]`, `corrected`), 40 steps with five coincidence steps.
#[test]
fn a8_sliding_table_corrected_diagonal_keeps_the_uniform_flow() {
    let mut sim = sim_of(&text(
        PAIR,
        "\n[mesh.sliding]\nangular_velocity = 1.0\n",
        "symmetry",
        CORRECTED,
    ));
    assert!(sim.sliding_interface_faces().is_some());
    assert_uniform("A8", run(&mut sim, 40), true);
    println!("A8: angle {:.4}", last(&sim, "sliding_angle"));
}

const C3: &str = r#"[mesh]
generator = "box_3d"
nx = 4
ny = 4
nz = 4
lx = 1.0
ly = 1.0
lz = 1.0

[physics]
nu = 0.01

[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[solver]
algorithm = "piso"
dt = 0.01
end_time = 1.0
@SOLVER@
"#;

/// B2 as registered: a defective set-up (ADR 0650 amendment 1: `box_3d` needs every patch written; the registered run was
/// refused by `Simulation::new` before any step, so nothing was measured). Kept as the record; B2c below is the corrected
/// set-up.
#[test]
#[ignore]
fn b2_three_d_walls_only_default_against_corrected() {
    b2_on(C3);
}

/// The five walls of C3c (ADR 0650 amendment 1).
const C3_WALLS: &str = r#"
[boundaries.xmin]
velocity = "fixed"

[boundaries.xmax]
velocity = "fixed"

[boundaries.ymin]
velocity = "fixed"

[boundaries.zmin]
velocity = "fixed"

[boundaries.zmax]
velocity = "fixed"

"#;

/// B2c (ADR 0650 amendment 1): C3c, the default against `corrected` after 20 steps.
#[test]
fn b2c_three_d_walls_only_default_against_corrected() {
    let t = C3.replace("[solver]", &format!("{C3_WALLS}[solver]"));
    assert_ne!(t, C3);
    b2_on(&t);
}

/// B2 on the case text `c3` (`@SOLVER@` replaced): the default against `corrected` after 20 steps.
fn b2_on(c3: &str) {
    let mut a = sim_of(&c3.replace("@SOLVER@", ""));
    let mut b = sim_of(&c3.replace("@SOLVER@", CORRECTED));
    for _ in 0..20 {
        a.step();
        b.step();
    }
    assert!(a.failure().is_none() && b.failure().is_none());
    let (ua, ub) = (&a.velocity().values, &b.velocity().values);
    let du = ua
        .iter()
        .zip(ub)
        .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
        .fold(0.0, f64::max);
    let (pa, pb) = (a.field("p").unwrap(), b.field("p").unwrap());
    let range = pa.iter().cloned().fold(f64::MIN, f64::max) - pa.iter().cloned().fold(f64::MAX, f64::min);
    let dp = pa.iter().zip(&pb).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / range;
    let bits = ua
        .iter()
        .zip(ub)
        .all(|(x, y)| (0..3).all(|k| x[k].to_bits() == y[k].to_bits()))
        && pa.iter().zip(&pb).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("B2: du {du:.3e} (of the lid speed), dp {dp:.3e} (of the range {range:.4e}); bit for bit {bits}");
    assert!(du <= 1e-12 && dp <= 1e-12, "B2: du {du:.3e}, dp {dp:.3e}");
}
