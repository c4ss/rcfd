//! ADR 0023 (wave 17A): checks F1-F4 (registered residual convergence of ADR 0022 C2, D1, D2, E2 with the corrected set-up:
//! pseudo-time controller and the `scaled` residual normalisation), W1-W2 (settings-tree wiring) and R2 (defaults inert).
//! Limits are those registered in docs/adr/0023-wave17a-convergence.md before the first run; none is changed here.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn build(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"))
}

fn box2d(nx: usize, ny: usize, lx: f64, ly: f64, mesh_extra: &str, patches: &str, solver_extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {nx}
ny = {ny}
lx = {lx}
ly = {ly}
{mesh_extra}
[physics]
nu = 0.1
{patches}
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
{solver_extra}
"#
    )
}

fn duct(xmin: &str, xmax: &str, extra: &str) -> String {
    box2d(
        20,
        4,
        5.0,
        1.0,
        "",
        &format!(
            "[boundaries.xmin]\n{xmin}\n[boundaries.xmax]\n{xmax}\n[boundaries.ymin]\nvelocity = \"symmetry\"\n[boundaries.ymax]\nvelocity = \"symmetry\"\n"
        ),
        &format!("component_diagonal = \"corrected\"\n{extra}"),
    )
    .replace("nu = 0.1", "nu = 1e-3")
}

fn ctl(law: &str) -> String {
    if law == "none_phys" {
        // addendum A6: fixed physical step, no controller
        return "pseudo_mode = \"physical\"
pseudo_dt = 0.2
residual_norm = \"scaled\""
            .to_string();
    }
    if let Some(l) = law.strip_suffix("_phys") {
        // corrected set-up of ADR 0023 addendum A5: physical start step 0.2 as in ADR 0022
        return format!(
            "pseudo_mode = \"physical\"
pseudo_dt = 0.2
pseudo_control = \"{l}\"
residual_norm = \"scaled\""
        );
    }
    format!("pseudo_mode = \"local\"\npseudo_cfl = 1.0\npseudo_control = \"{law}\"\nresidual_norm = \"scaled\"")
}

const P0: &str = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0";

fn run_to_end(text: &str) -> Simulation {
    let mut s = build(text);
    while !s.is_done() {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
    }
    let l = s.last().unwrap();
    println!(
        "  converged {} after {} iterations, res_u {:.3e} res_p {:.3e}",
        s.is_converged(),
        l.index,
        l.res_u,
        l.res_p
    );
    s
}

fn imbalance(s: &Simulation) -> f64 {
    let flows: Vec<f64> = s
        .geom
        .mesh
        .patches()
        .iter()
        .map(|p| p.faces().map(|f| s.flux()[f]).sum::<f64>())
        .collect();
    let top = flows.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    flows.iter().sum::<f64>().abs() / top
}

fn mean_abs_ux(s: &Simulation) -> f64 {
    let ux = s.field("ux").unwrap();
    ux.iter().map(|v| v.abs()).sum::<f64>() / ux.len() as f64
}

const PO: &str =
    "bc_kind = \"pressure_outlet\"\nbc_pressure = 0.5\nbackflow_pressure = \"total\"\nbackflow_direction = \"normal\"";
const TP: &str = "bc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5";

fn f1(law: &str) {
    println!("F1 (C2) law {law}");
    let s = run_to_end(&duct(P0, PO, &ctl(law)));
    assert!(s.is_converged(), "residual targets not met");
    let m = mean_abs_ux(&s);
    println!(
        "  mean |u_x| = {m:.9} (limit |.|-1 <= 1e-5), imbalance {:.2e}",
        imbalance(&s)
    );
    assert!((m - 1.0).abs() <= 1e-5, "{m}");
}

fn f2(law: &str) {
    println!("F2 (D1) law {law}");
    let s = run_to_end(&duct(TP, P0, &ctl(law)));
    assert!(s.is_converged(), "residual targets not met");
    let m = mean_abs_ux(&s);
    println!("  mean |u_x| = {m:.9} (limit |.|-1 <= 1e-5)");
    assert!((m - 1.0).abs() <= 1e-5, "{m}");
}

fn f3(law: &str) {
    println!("F3 (D2) law {law}");
    let c = (30.0_f64).to_radians().cos();
    let sn = (30.0_f64).to_radians().sin();
    let text = box2d(
        20,
        8,
        2.0,
        1.0,
        "periodic_y = true",
        &format!(
            "[boundaries.xmin]\nbc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5\nflow_direction = [{c:.17}, {sn:.17}, 0.0]\n[boundaries.xmax]\n{P0}\n"
        ),
        &format!("component_diagonal = \"corrected\"\n{}", ctl(law)),
    )
    .replace("nu = 0.1", "nu = 1e-3");
    let s = run_to_end(&text);
    assert!(s.is_converged(), "residual targets not met");
    let (ux, uy) = (s.field("ux").unwrap(), s.field("uy").unwrap());
    let (mut e_mag, mut e_ang) = (0.0_f64, 0.0_f64);
    for i in 0..ux.len() {
        e_mag = e_mag.max(((ux[i] * ux[i] + uy[i] * uy[i]).sqrt() - 1.0).abs());
        e_ang = e_ang.max((uy[i].atan2(ux[i]) - 30.0_f64.to_radians()).abs());
    }
    println!("  max ||u|-1| = {e_mag:.2e} (limit 1e-5), max angle error {e_ang:.2e} rad (limit 1e-6)");
    assert!(e_mag <= 1e-5, "{e_mag}");
    assert!(e_ang <= 1e-6, "{e_ang}");
}

const E2_CASE: &str = r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 12
nz = 24
r_in = 0.5
r_out = 1.0
lz = 4.0
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = 2.0
[boundaries.outer]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = 2.0
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 2.3494714599609283
swirl_rate = 2.0
[boundaries.zmax]
bc_kind = "radial_equilibrium_outlet"
radial_reference = "area_average"
bc_pressure = 1.25
[solver]
algorithm = "simplec"
convection = "linear_upwind"
component_diagonal = "corrected"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;

fn f4(norm: &str) {
    println!("F4 (E2) residual_norm {norm} (registered set-up otherwise: no pseudo time)");
    let text = format!("{E2_CASE}residual_norm = \"{norm}\"\n");
    let s = run_to_end(&text);
    assert!(s.is_converged(), "residual targets not met");
    let exact = |r: f64| 1.25 + 2.0 * (r * r - 0.625);
    let range = 1.5;
    let pb = s.boundary_field("p").unwrap();
    let uf = s.velocity().boundary_values(&s.geom);
    let patch = s.geom.mesh.patch("zmax").unwrap();
    let (mut ep, mut eu) = (0.0_f64, 0.0_f64);
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    for f in patch.faces() {
        let x = s.geom.mesh.face_centres()[f];
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let v = uf[f - s.geom.n_internal];
        let w = (-x[1] * v[0] + x[0] * v[1]) / r;
        let p = pb[f - s.geom.n_internal];
        ep = ep.max((p - exact(r)).abs());
        eu = eu.max((w - 2.0 * r).abs());
        rows.push((r, p, w));
    }
    let p = s.field("p").unwrap();
    let mut em = 0.0_f64;
    for (c, x) in s.geom.mesh.cell_centres().iter().enumerate() {
        if x[2] >= 1.5 && x[2] <= 2.5 {
            let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
            em = em.max((p[c] - exact(r)).abs());
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ed = 0.0_f64;
    for i in 0..rows.len() - 1 {
        let g = |q: &(f64, f64, f64)| q.2 * q.2 / q.0;
        let dp = rows[i + 1].1 - rows[i].1;
        let want = 0.5 * (g(&rows[i]) + g(&rows[i + 1])) * (rows[i + 1].0 - rows[i].0);
        ed = ed.max((dp - want).abs());
    }
    let im = imbalance(&s);
    println!(
        "  outlet p error {:.3} % of 1.5 (limit 3), u_theta error {:.3} % of 2 (limit 3), mid-length p {:.3} % (limit 3), balance {:.2e} (limit 1e-8), imbalance {im:.2e} (limit 1e-9)",
        100.0 * ep / range,
        100.0 * eu / 2.0,
        100.0 * em / range,
        ed / range
    );
    assert!(ep <= 0.03 * range && eu <= 0.03 * 2.0 && em <= 0.03 * range && ed / range <= 1e-8 && im <= 1e-9);
}

#[test]
fn f4_e2_residual_targets_with_scaled_normalisation() {
    f4("scaled");
}

/// Reported control: the registered reference normalisation (stalls at 4.4e-12, ADR 0022 addendum, ADR 0023 X2).
#[test]
#[ignore = "KNOWN MISS (ADR 0022 addendum, ADR 0023 X2): the reference-normalised residual stalls at 4.4e-12 (round-off amplified)"]
fn f4_e2_residual_targets_with_reference_normalisation_known_miss() {
    f4("reference");
}

#[test]
fn f1c_c2_physical_step_scaled_norm() {
    f1("none_phys");
}
#[test]
fn f2c_d1_physical_step_scaled_norm() {
    f2("none_phys");
}
#[test]
fn f3c_d2_physical_step_scaled_norm() {
    f3("none_phys");
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A6): the ser2 controller drives the frictionless duct unstable (non-finite fields)"]
fn f1b_c2_physical_start_ser2_known_miss() {
    f1("ser2_phys");
}
#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A6): the ser2 controller drives the frictionless duct unstable (non-finite fields)"]
fn f2b_d1_physical_start_ser2_known_miss() {
    f2("ser2_phys");
}
#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A6): the ser2 controller drives the frictionless duct unstable (non-finite fields)"]
fn f3b_d2_physical_start_ser2_known_miss() {
    f3("ser2_phys");
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A5): local pseudo mode from rest over-shoots like the plain run"]
fn f1_c2_ser2_known_miss() {
    f1("ser2");
}
#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A5): local pseudo mode from rest over-shoots like the plain run"]
fn f2_d1_ser2_known_miss() {
    f2("ser2");
}
#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A5): local pseudo mode from rest over-shoots like the plain run"]
fn f3_d2_ser2_known_miss() {
    f3("ser2");
}

// ---- W: wiring --------------------------------------------------------------------------------------------------

fn cavity_text(extra: &str) -> String {
    box2d(
        16,
        16,
        1.0,
        1.0,
        "",
        "[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n",
        extra,
    )
    .replace("tolerance_u = 1e-12", "tolerance_u = 1e-14")
    .replace("nu = 0.1", "nu = 0.001")
}

#[test]
fn r2_defaults_are_bitwise_inert_on_a_cavity() {
    let a = cavity_text("pseudo_mode = \"local\"\npseudo_cfl = 5.0\nalpha_u = 1.0");
    let b = cavity_text(
        "pseudo_mode = \"local\"\npseudo_cfl = 5.0\nalpha_u = 1.0\npseudo_control = \"off\"\nresidual_norm = \"reference\"\npseudo_scale_max = 10.0",
    );
    let (mut x, mut y) = (build(&a), build(&b));
    for _ in 0..150 {
        x.step();
        y.step();
        let (lx, ly) = (x.last().unwrap(), y.last().unwrap());
        assert_eq!(lx.res_u.to_bits(), ly.res_u.to_bits());
        assert_eq!(lx.res_p.to_bits(), ly.res_p.to_bits());
    }
    for n in ["ux", "uy", "p"] {
        assert_eq!(x.field(n).unwrap(), y.field(n).unwrap(), "{n}");
    }
}

#[test]
fn w1_tree_run_equals_library_run() {
    use rcfd_fv::simple::{PseudoController, ResidualNorm};
    let base = "pseudo_mode = \"local\"\npseudo_cfl = 2.0\nalpha_u = 1.0\nalpha_p = 1.0";
    for (law, norm_key, cap) in [("ser", "backward", 50.0), ("ser2", "scaled", 200.0)] {
        let tree =
            format!("{base}\npseudo_control = \"{law}\"\npseudo_scale_max = {cap}\nresidual_norm = \"{norm_key}\"");
        let mut a = build(&cavity_text(&tree));
        let mut b = build(&cavity_text(base));
        {
            let sv = b.simple_solver_mut().unwrap();
            sv.pseudo_control = Some(if law == "ser2" {
                PseudoController::new_symmetric(cap)
            } else {
                PseudoController::new(cap)
            });
            sv.residual_norm = if norm_key == "scaled" {
                ResidualNorm::Scaled
            } else {
                ResidualNorm::Backward
            };
        }
        for _ in 0..200 {
            a.step();
            b.step();
        }
        let mut worst = 0.0_f64;
        for n in ["ux", "uy", "p"] {
            for (u, v) in a.field(n).unwrap().iter().zip(&b.field(n).unwrap()) {
                worst = worst.max((u - v).abs());
            }
        }
        println!("W1 {law}/{norm_key}: max field difference {worst:.2e} (limit 1e-12)");
        assert!(worst <= 1e-12, "{worst}");
        assert_eq!(a.last().unwrap().res_u.to_bits(), b.last().unwrap().res_u.to_bits());
    }
}

#[test]
fn w2_validation_is_located() {
    let text = cavity_text("pseudo_control = \"ser2\"");
    let c = Case::parse_toml(&text).unwrap();
    let e = match Simulation::new(&c) {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    println!("W2: {e:?}");
    assert!(e.iter().any(|d| d.path == "solver.pseudo_control"));
    for bad in ["pseudo_control = \"fast\"", "residual_norm = \"relative\""] {
        let r = Case::parse_toml(&cavity_text(bad));
        let errs = match r {
            Err(e) => e,
            Ok(c) => c.semantic_errors(),
        };
        println!("W2 {bad}: {errs:?}");
        assert!(!errs.is_empty(), "{bad} accepted");
    }
}
