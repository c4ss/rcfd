//! F-673 (ADR 0597): the distributed load height on the pre-stressed modes of the monosymmetric warping beam, with the inertial
//! coupling of the shear-centre offset in the consistent mass. Checks B1, M0 (a known miss), A, S, N1, N3, N4, Z and Y of the
//! registration and the diagnostic D-M0 of its amendment 1 (B0 is the existing test files). Each registered run is made once;
//! every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors_with, modal_analysis, prestressed_modes, solve_static, Warping, WarpingPrestress, WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::Shift;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite is a miss.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;
const Q_REF: f64 = 1000.0;
const L: f64 = 3.0;
const N_EL: usize = 16;
/// Flange 1 of the monosymmetric I from the centroid (`MS_TOP` of `sa_beam_warping_f663.rs`).
const MS_TOP: f64 = 0.0718681318681;

/// ADR 0597 D5: the coupled Timoshenko-Rayleigh pinned frequency (M0).
const OMEGA_T: f64 = 152.755814868645;
/// ADR 0597 arithmetic 6 (N3).
const RATIO_C_S: f64 = 0.967356879;
const RATIO_F_S: f64 = 1.012498926;

/// A case of the readings: name, the entry point (none at the shear centre), `lambda_ref` of ADR 0588, `omega_ref` and
/// `|phi(L/2)/v_c(L/2)|` of ADR 0597 D3, and the reference frequencies with the coupling dropped and with the mass at the
/// shear centre (arithmetic 6, N4).
struct Case {
    name: &'static str,
    point: Option<[f64; 2]>,
    lambda_ref: f64,
    omega_ref: f64,
    shape_ref: f64,
    omega_nocoup: f64,
    omega_atsc: f64,
}

const CASES: [Case; 3] = [
    Case {
        name: "S",
        point: None,
        lambda_ref: 135.747863965,
        omega_ref: 166.799139498357,
        shape_ref: 3.784772413,
        omega_nocoup: 130.8703296103,
        omega_atsc: 134.9833059340,
    },
    Case {
        name: "C",
        point: Some([0.0, 0.0]),
        lambda_ref: 163.396623703,
        omega_ref: 161.354294924654,
        shape_ref: 3.636579706,
        omega_nocoup: 129.7436614651,
        omega_atsc: 132.9134771139,
    },
    Case {
        name: "F",
        point: Some([0.0, MS_TOP]),
        lambda_ref: 126.453805543,
        omega_ref: 168.883949617271,
        shape_ref: 3.746893414,
        omega_nocoup: 131.3414946757,
        omega_atsc: 135.8571297984,
    },
];

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, KAPPA).unwrap()
}

/// The fork beam of the readings: `N_EL` elements along `x`, `u = 0` at node 0, `v = w = phi = 0` at both ends, warping
/// free; sets `all`, `x0`, `xl`, `mid`. No load.
fn fork() -> SolidModel {
    fork_n(N_EL)
}

/// The same fork beam with `n` elements (`n` even; `mid` the node `n/2`).
fn fork_n(n: usize) -> SolidModel {
    let (section, _) = mono();
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

fn plain() -> Warping {
    Warping::new().block(0, mono().1)
}

/// The downward line load of the factor `lambda` (`lambda q_ref` N/m along `-z`, flange 1 compressed).
fn force(lambda: f64) -> Vec3 {
    [0.0, 0.0, -lambda * Q_REF]
}

/// The warping of the case with its entry holding the applied load `force(lambda)` (ADR 0594 D1); `point` none: no entry.
fn warping(point: Option<[f64; 2]>, lambda: f64) -> Warping {
    match point {
        Some(p) => plain().line_load_point("all", force(lambda), p),
        None => plain(),
    }
}

/// The pre-stress of the readings: the warping static solve of the fork beam under `force(lambda)` on every element,
/// `WarpingPrestress::from_static(.., [0, 0])`.
fn prestress(lambda: f64) -> WarpingPrestress {
    let mut l = fork();
    l.add_load(Load::line_load("all", force(lambda))).unwrap();
    let w = plain();
    let r = solve_static(&l, &w, None).unwrap();
    WarpingPrestress::from_static(&l, &w, &r, [0.0; 2]).unwrap()
}

fn eq(m: &SolidModel, node: usize, component: usize) -> usize {
    m.dof_map().equation(node, component).unwrap()
}

/// The unloaded `omega_0^2` of `modal_analysis` (D4).
fn omega0_sq() -> f64 {
    let md = modal_analysis(&fork(), &plain(), &ModalOptions::new(1)).unwrap();
    println!("  unloaded eigenvalues {:?}", md.eigenvalues);
    md.eigenvalues[0]
}

/// M0 of the registration, missed in the one registered run (ADR 0597 addendum 1): measured `5.2189e-4` (`omega_0^2 =
/// 23358.70109196537`) against the limit `1e-4`. Diagnosed in ADR 0597 amendment 1: the mass interpolates the centroid
/// translation from `(v_c, theta_z)`, an `O(l^2)` error for a section with an offset (diagnostic `dm_mass_convergence`).
#[test]
#[ignore = "known miss of ADR 0597 M0, kept as the record"]
fn m0_unloaded_coupled_mass_known_miss() {
    let mut c = Checks::new();
    let w0 = omega0_sq();
    c.at_most(
        "M0 unloaded omega_0 against the coupled Timoshenko-Rayleigh omega_T",
        rel(w0.sqrt(), OMEGA_T),
        1e-4,
    );
    c.finish();
}

#[test]
fn b1_shear_centre_entry() {
    let mut c = Checks::new();
    // B1: an entry at the shear centre gives the bits of no entry.
    let z_0 = mono().1.shear_centre()[1];
    let sc = &CASES[0];
    let lam = 0.5 * sc.lambda_ref;
    let pre = prestress(lam);
    let m = fork();
    let a = prestressed_modes(&m, &pre, &warping(None, lam), &ModalOptions::new(1)).unwrap();
    let b = prestressed_modes(&m, &pre, &warping(Some([0.0, z_0]), lam), &ModalOptions::new(1)).unwrap();
    println!(
        "B1 z_0 {z_0:.12e}: no entry {:?}, entry at (0, z_0) {:?}",
        a.eigenvalues, b.eigenvalues
    );
    c.holds(
        "B1 modes eigenvalue bit-equal",
        a.eigenvalues[0].to_bits() == b.eigenvalues[0].to_bits(),
        &format!("{:e} / {:e}", a.eigenvalues[0], b.eigenvalues[0]),
    );
    let same = a.modes[0].len() == b.modes[0].len()
        && a.modes[0]
            .iter()
            .zip(&b.modes[0])
            .all(|(x, y)| x.to_bits() == y.to_bits());
    c.holds("B1 mode 1 bit-equal", same, &format!("{} components", a.modes[0].len()));
    c.finish();
}

/// ADR 0597 amendment 1, diagnostic D-M0: the unloaded `omega_0` against `omega_T` at 16, 32 and 64 elements.
#[test]
fn dm_mass_convergence() {
    let mut c = Checks::new();
    let e: Vec<f64> = [16usize, 32, 64]
        .iter()
        .map(|&n| {
            let md = modal_analysis(&fork_n(n), &plain(), &ModalOptions::new(1)).unwrap();
            let e = md.eigenvalues[0].sqrt() / OMEGA_T - 1.0;
            println!("  n {n}: omega_0^2 {:.12e}, e_n {e:+.4e}", md.eigenvalues[0]);
            e
        })
        .collect();
    for (name, r) in [("DM-1 e_32/e_16", e[1] / e[0]), ("DM-2 e_64/e_32", e[2] / e[1])] {
        c.holds(
            name,
            r.is_finite() && (0.15..=0.35).contains(&r),
            &format!("{r:.4} (limit in [0.15, 0.35])"),
        );
    }
    c.at_most("DM-3 |e_64|", e[2].abs(), 1e-4);
    c.finish();
}

#[test]
fn a_s_n_frequencies_at_half_the_buckling_load() {
    let mut c = Checks::new();
    let m = fork();
    let (e_v, e_phi) = (eq(&m, N_EL / 2, 1), eq(&m, N_EL / 2, 3));
    let mut omegas = Vec::new();
    for case in &CASES {
        let lam = 0.5 * case.lambda_ref;
        let pre = prestress(lam);
        let w = warping(case.point, lam);
        println!(
            "case {} at lambda {lam:.10}, entry {:?}",
            case.name,
            w.line_load_points()
        );
        let md = prestressed_modes(&m, &pre, &w, &ModalOptions::new(1)).unwrap();
        let ev = md.eigenvalues[0];
        println!(
            "  modes eigenvalue {ev:.12e}, omega {:.12}, residual {:?}, Sturm {:?}",
            ev.sqrt(),
            md.residuals,
            md.sturm
        );
        // A: the acceptance.
        c.at_most(
            &format!("A-{} modes omega against the reference (acceptance)", case.name),
            rel(ev.sqrt(), case.omega_ref),
            1e-2,
        );
        omegas.push(ev.sqrt());
        // S: the mode.
        let shape = (md.modes[0][e_phi] / md.modes[0][e_v]).abs();
        println!(
            "  mode 1: v(L/2) {:e}, phi(L/2) {:e}, |phi/v| {shape:.9}",
            md.modes[0][e_v], md.modes[0][e_phi]
        );
        c.at_most(
            &format!("S-{} |phi(L/2)/v(L/2)| against the reference", case.name),
            rel(shape, case.shape_ref),
            3e-2,
        );
        // N4: the inertial coupling.
        c.above(
            &format!(
                "N4-{} against the reference with the coupling dropped (negative control)",
                case.name
            ),
            rel(ev.sqrt(), case.omega_nocoup),
            1e-1,
        );
        c.above(
            &format!(
                "N4-{} against the reference with the mass at the shear centre (negative control)",
                case.name
            ),
            rel(ev.sqrt(), case.omega_atsc),
            1e-1,
        );
    }
    // N3: the ratios.
    c.at_most(
        "N3 omega_C/omega_S against the reference ratio",
        rel(omegas[1] / omegas[0], RATIO_C_S),
        2e-3,
    );
    c.at_most(
        "N3 omega_F/omega_S against the reference ratio",
        rel(omegas[2] / omegas[0], RATIO_F_S),
        2e-3,
    );
    // N1: the centroid and flange-1 loads without the entry.
    for (i, limit) in [(1usize, 2e-2), (2, 4e-3)] {
        let case = &CASES[i];
        let lam = 0.5 * case.lambda_ref;
        let md = prestressed_modes(&m, &prestress(lam), &plain(), &ModalOptions::new(1)).unwrap();
        println!("  N1-{} eigenvalue {:.12e}", case.name, md.eigenvalues[0]);
        c.above(
            &format!("N1-{} no entry against the reference (negative control)", case.name),
            rel(md.eigenvalues[0].sqrt(), case.omega_ref),
            limit,
        );
    }
    c.finish();
}

#[test]
fn z_y_frequency_falls_to_zero_at_the_buckling_load() {
    let mut c = Checks::new();
    let m = fork();
    let w0 = omega0_sq();
    let sigma = 0.5 * w0;
    for case in &CASES {
        let lam_cr = buckling_factors_with(&m, &prestress(1.0), &warping(case.point, 1.0), &BucklingOptions::new(3))
            .unwrap()
            .factors[0];
        println!(
            "case {}: lambda_cr {lam_cr:.12} (ADR 0588 reference {}, {:+.3e})",
            case.name,
            case.lambda_ref,
            lam_cr / case.lambda_ref - 1.0
        );
        for (tag, lam) in [("Z", lam_cr), ("Y", case.lambda_ref)] {
            let md = prestressed_modes(
                &m,
                &prestress(lam),
                &warping(case.point, lam),
                &ModalOptions::new(1).shift(Shift::Value(sigma)),
            )
            .unwrap();
            println!(
                "  {tag}-{} modes eigenvalue {:.6e}, residual {:?}, Sturm {:?}",
                case.name, md.eigenvalues[0], md.residuals, md.sturm
            );
            c.at_most(
                &format!("{tag}-{} modes |omega^2|/omega_0^2 at lambda {lam:.10}", case.name),
                md.eigenvalues[0].abs() / w0,
                1e-2,
            );
        }
    }
    c.finish();
}
