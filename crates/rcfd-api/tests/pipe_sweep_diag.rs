//! F-235 through the case tree (ADR 0047 checks S10, S11): the `pipe_sweep` generator and fully developed Poiseuille pipe flow.
//! The limits are the registered ones.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_mesh::gen::Grading;
use rcfd_mesh::sweep::pipe_mesh;

fn pipe_case(extra_mesh: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "pipe_sweep"
nx = 17
ny = 12
nz = 8
r_out = 1.0
lz = 1.0
periodic_z = true
{extra_mesh}
[physics]
nu = 0.1
rho = 1.0
body_force = [0.0, 0.0, 1.0]
[initial]
velocity = [0.0, 0.0, 0.0]
[boundaries.wall]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-10
pressure_rel_tol = 1e-10
pressure_max_iter = 1000
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
{extra}
"#
    )
}

fn solve(nx: usize, ny: usize, extra_mesh: &str, extra: &str) -> (f64, f64) {
    let text = pipe_case(extra_mesh, extra)
        .replace("nx = 17", &format!("nx = {nx}"))
        .replace("ny = 12", &format!("ny = {ny}"))
        .replace("nz = 8", "nz = 2");
    let case = Case::parse_toml(&text).unwrap();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut n = 0;
    loop {
        let before = sim.field("uz").unwrap();
        for _ in 0..200 {
            sim.step();
        }
        n += 200;
        let after = sim.field("uz").unwrap();
        let scale = after.iter().fold(1e-300_f64, |m, v| m.max(v.abs()));
        let ch = after
            .iter()
            .zip(&before)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
            / scale;
        if ch < 1e-10 || n >= 20000 {
            break;
        }
    }
    let uz = sim.field("uz").unwrap();
    let cc = sim.geom.mesh.cell_centres();
    let uc = (0..uz.len())
        .filter(|&c| cc[c][0].abs() < 1e-12 && cc[c][1].abs() < 1e-12)
        .map(|c| uz[c])
        .next()
        .unwrap_or(f64::NAN);
    let vol = sim.geom.mesh.cell_volumes();
    let ub = uz.iter().zip(vol).map(|(u, v)| u * v).sum::<f64>() / vol.iter().sum::<f64>();
    (uc, ub)
}

#[test]
#[ignore]
fn diag_pipe_refinement() {
    for (nx, ny, cf, nc) in [
        (9, 6, 0.5, 2),
        (17, 12, 0.5, 1),
        (17, 12, 0.5, 2),
        (17, 12, 0.5, 3),
        (33, 24, 0.5, 2),
        (17, 12, 0.3, 2),
    ] {
        let (uc, ub) = solve(
            nx,
            ny,
            &format!("core_fraction = {cf}"),
            &format!("non_orthogonal_correctors = {nc}"),
        );
        println!("correctors {nc}");
        let nn = 4.0 * nx as f64;
        let area = 0.5 * nn * (2.0 * std::f64::consts::PI / nn).sin();
        println!("DIAG nx {nx} ny {ny} cf {cf}: N {nn} uc {uc:.5} ({:.3} %) ub {ub:.5} ({:.3} %) polygon area deficit {:.3} %", 100.0 * (uc - 2.5) / 2.5, 100.0 * (ub - 1.25) / 1.25, 100.0 * (1.0 - area / std::f64::consts::PI));
    }
}

/// Diagnostic for the s10b divergence: which settings keep nz = 8 stable with the correctors.
#[test]
#[ignore]
fn diag_nan_onset() {
    for (nz, extra) in [
        (4usize, "non_orthogonal_correctors = 2"),
        (
            8,
            "non_orthogonal_correctors = 2
alpha_u = 0.5",
        ),
        (8, "non_orthogonal_correctors = 1"),
    ] {
        let text = pipe_case("", extra).replace("nz = 8", &format!("nz = {nz}"));
        let case = Case::parse_toml(&text).unwrap();
        let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
        for k in 0..12 {
            for _ in 0..100 {
                sim.step();
            }
            let uz = sim.field("uz").unwrap();
            let m = uz
                .iter()
                .fold(0.0_f64, |m, v| if v.is_nan() { f64::NAN } else { m.max(v.abs()) });
            println!("nz {nz} [{}] it {}: max|uz| = {m:.6e}", extra.len(), (k + 1) * 100);
            if !(m < 50.0) {
                break;
            }
        }
    }
}
