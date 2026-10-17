//! ADR 0050 Part 1: the SIMPLEC non-orthogonal flux compensation (`solver.simplec_non_orthogonal_compensation`) and the
//! divergence of the non-orthogonal pressure corrector on the swept pipe (wave 26B S10b). Limits R1-R7 are the registered ones.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn pipe_case(nz: usize, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "pipe_sweep"
nx = 17
ny = 12
nz = {nz}
r_out = 1.0
lz = 1.0
periodic_z = true
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

struct Pipe {
    uc: f64,
    ub: f64,
    spread: f64,
    trans: f64,
    iterations: usize,
    change: f64,
    finite: bool,
}

/// Iterate to convergence (change of uz per 200 iterations below 1e-9, at most 20000 iterations) and measure.
fn solve(nz: usize, extra: &str) -> Pipe {
    let case = Case::parse_toml(&pipe_case(nz, extra)).unwrap();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let (mut n, mut change) = (0, f64::INFINITY);
    let mut finite = true;
    while n < 20000 {
        let before = sim.field("uz").unwrap();
        for _ in 0..200 {
            sim.step();
        }
        n += 200;
        let after = sim.field("uz").unwrap();
        finite = after.iter().all(|v| v.is_finite());
        if !finite {
            break;
        }
        let scale = after.iter().fold(1e-300_f64, |m, v| m.max(v.abs()));
        change = after
            .iter()
            .zip(&before)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
            / scale;
        if change < 1e-9 {
            break;
        }
    }
    let (uz, ux, uy) = (
        sim.field("uz").unwrap(),
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
    );
    let cc = sim.geom.mesh.cell_centres();
    let axis: Vec<usize> = (0..uz.len())
        .filter(|&c| cc[c][0].abs() < 1e-12 && cc[c][1].abs() < 1e-12)
        .collect();
    assert_eq!(axis.len(), nz);
    let uc = uz[axis[0]];
    let spread = axis.iter().fold(0.0_f64, |m, &c| m.max((uz[c] - uc).abs())) / uc.abs().max(1e-300);
    let vol = sim.geom.mesh.cell_volumes();
    let ub = uz.iter().zip(vol).map(|(u, v)| u * v).sum::<f64>() / vol.iter().sum::<f64>();
    let trans = ux.iter().chain(uy.iter()).fold(0.0_f64, |m, v| m.max(v.abs()));
    Pipe {
        uc,
        ub,
        spread,
        trans,
        iterations: n,
        change,
        finite,
    }
}

fn check_registered_limits(p: &Pipe, label: &str) {
    let (uc_exact, ub_exact) = (2.5, 1.25);
    let e_c = (p.uc - uc_exact).abs() / uc_exact;
    let e_b = (p.ub - ub_exact).abs() / ub_exact;
    println!(
        "{label}: {} iterations, change {:.2e}, u_c {:.6} (error {:.4} %), U_b {:.6} (error {:.4} %), spread {:.2e}, transverse {:.2e}",
        p.iterations,
        p.change,
        p.uc,
        100.0 * e_c,
        p.ub,
        100.0 * e_b,
        p.spread,
        p.trans
    );
    assert!(p.finite, "{label}: not finite (diverged)");
    assert!(p.change < 1e-9, "{label}: not converged ({})", p.change);
    assert!(e_c <= 0.01, "{label}: centreline error {e_c}");
    assert!(e_b <= 0.015, "{label}: bulk error {e_b}");
    assert!(p.spread <= 1e-6, "{label}: axial spread {}", p.spread);
    assert!(p.trans <= 1e-6 * p.uc, "{label}: transverse velocity {}", p.trans);
}

const COMP: &str = "non_orthogonal_correctors = 2\nsimplec_non_orthogonal_compensation = true";

/// R2: nz = 4 (the cheap variant of R1), registered S10 limits; R6: z independence against the nz = 2 value.
#[test]
fn r2_r6_pipe_nz4_with_compensation_meets_the_s10_limits() {
    let p = solve(4, COMP);
    check_registered_limits(&p, "R2 nz=4");
    let q = solve(2, COMP);
    let rel = (p.uc - q.uc).abs() / q.uc;
    println!(
        "R6: u_c(nz=4) {:.8} u_c(nz=2) {:.8} relative difference {rel:.2e} (limit 1e-6)",
        p.uc, q.uc
    );
    assert!(rel <= 1e-6, "{rel}");
}

/// R1: the registered S10b mesh and settings (nz = 8, two correctors) with the switch on, S10 limits unchanged.
#[test]
fn r1_s10d_poiseuille_pipe_nz8_with_compensation() {
    let p = solve(8, COMP);
    check_registered_limits(&p, "R1/S10d nz=8");
}

fn state_bits(extra: &str, iterations: usize) -> Vec<u64> {
    let text = pipe_case(4, extra)
        .replace("nx = 17", "nx = 9")
        .replace("ny = 12", "ny = 6");
    let case = Case::parse_toml(&text).unwrap();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..iterations {
        sim.step();
    }
    ["ux", "uy", "uz", "p"]
        .iter()
        .flat_map(|f| sim.field(f).unwrap())
        .map(f64::to_bits)
        .collect()
}

/// R3 + R4: default off is bit-identical to an explicit false, and the switch is inert without correctors.
#[test]
fn r3_r4_switch_is_inert_by_default_and_without_correctors() {
    let base = state_bits("non_orthogonal_correctors = 2", 60);
    let off = state_bits(
        "non_orthogonal_correctors = 2\nsimplec_non_orthogonal_compensation = false",
        60,
    );
    assert_eq!(base, off, "R3: explicit false must equal the default");
    let on = state_bits(
        "non_orthogonal_correctors = 2\nsimplec_non_orthogonal_compensation = true",
        60,
    );
    assert_ne!(base, on, "the switch must act with correctors on a non-orthogonal mesh");
    let z0 = state_bits("non_orthogonal_correctors = 0", 60);
    let z1 = state_bits(
        "non_orthogonal_correctors = 0\nsimplec_non_orthogonal_compensation = true",
        60,
    );
    assert_eq!(z0, z1, "R4: no correctors -> bit-identical");
}

/// R5: on an orthogonal mesh the switch changes nothing beyond round-off.
#[test]
fn r5_orthogonal_mesh_unaffected() {
    let run = |extra: &str| {
        let text = format!(
            "[mesh]\ngenerator = \"cavity_2d\"\nnx = 16\nny = 16\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simplec\"\nconvection = \"linear_upwind\"\nnon_orthogonal_correctors = 2\nmax_iterations = 400\n{extra}\n"
        );
        let case = Case::parse_toml(&text).unwrap();
        let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
        for _ in 0..400 {
            sim.step();
        }
        (sim.field("ux").unwrap(), sim.field("uy").unwrap())
    };
    let (ax, ay) = run("");
    let (bx, by) = run("simplec_non_orthogonal_compensation = true");
    let scale = ax.iter().chain(&ay).fold(0.0_f64, |m, v| m.max(v.abs()));
    let diff = ax
        .iter()
        .zip(&bx)
        .chain(ay.iter().zip(&by))
        .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
    println!(
        "R5: max velocity difference {:.3e} (relative {:.3e}, limit 1e-9)",
        diff,
        diff / scale
    );
    assert!(diff / scale <= 1e-9, "{}", diff / scale);
}

/// R7 diagnostic (no limit): without the switch, the nz = 4 run for several alpha_u (measured: 0.3-0.8 converge, 0.9 diverges).
#[test]
#[ignore = "diagnostic of ADR 0050 item 5; takes several minutes"]
fn r7_alpha_u_sweep_without_the_switch() {
    for a in [0.3, 0.5, 0.7, 0.8, 0.9] {
        let case = Case::parse_toml(&pipe_case(4, &format!("non_orthogonal_correctors = 2\nalpha_u = {a}"))).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let mut verdict = "no divergence in 3000 iterations";
        for _ in 0..30 {
            for _ in 0..100 {
                sim.step();
            }
            let uz = sim.field("uz").unwrap();
            if uz.iter().any(|v| !v.is_finite()) || uz.iter().any(|v| v.abs() > 50.0) {
                verdict = "diverged";
                break;
            }
        }
        println!("R7 alpha_u {a}: {verdict}");
    }
}
