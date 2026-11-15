//! F-670 (ADR 0594): the distributed load height on the pre-stressed warping modes and harmonic path. Checks B1, M0, A, H, C,
//! S, N1 to N3, Z, ZH, Y and YH of the registration (B0 is the existing test files). Each registered run is made once; every
//! value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors_with, load_vector, modal_analysis, prestressed_modes, solve_static, Warping, WarpingHarmonic,
    WarpingHarmonicOptions, WarpingPrestress, WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
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
const T: f64 = 1000.0;
const DS_FLANGE: f64 = 0.095;

/// ADR 0594 arithmetic 2: the Timoshenko-Rayleigh pinned lateral frequency (M0).
const OMEGA_T: f64 = 128.729211880906;
/// ADR 0594 arithmetic 7 (N3).
const RATIO_TOP_SC: f64 = 1.040660587;
const RATIO_BOT_SC: f64 = 0.961921633;

/// A case of the readings: name, the entry point (none at the shear centre), `lambda_ref` of ADR 0588, `omega_ref` and
/// `|phi(L/2)/v(L/2)|` of ADR 0594 D3.
struct Case {
    name: &'static str,
    point: Option<[f64; 2]>,
    lambda_ref: f64,
    omega_ref: f64,
    shape_ref: f64,
}

const CASES: [Case; 3] = [
    Case {
        name: "T",
        point: Some([0.0, DS_FLANGE]),
        lambda_ref: 45.1856414209,
        omega_ref: 111.798383170594,
        shape_ref: 4.08927986,
    },
    Case {
        name: "S",
        point: None,
        lambda_ref: 59.0640165887,
        omega_ref: 107.430207818881,
        shape_ref: 3.85897491,
    },
    Case {
        name: "B",
        point: Some([0.0, -DS_FLANGE]),
        lambda_ref: 77.1475881351,
        omega_ref: 103.339440973516,
        shape_ref: 3.46106352,
    },
];

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn dsym() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap()
}

/// The fork beam of the readings: `N_EL` elements along `x`, `u = 0` at node 0, `v = w = phi = 0` at both ends, warping
/// free; sets `all`, `x0`, `xl`, `mid`. No load.
fn fork() -> SolidModel {
    let (section, _) = dsym();
    let nodes: Vec<Vec3> = (0..=N_EL).map(|i| [L * i as f64 / N_EL as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..N_EL).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..N_EL).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![N_EL]).unwrap();
    mesh.add_node_set("mid", vec![N_EL / 2]).unwrap();
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

fn with_torque(mut m: SolidModel) -> SolidModel {
    m.add_load(Load::nodal_moment("mid", [T, 0.0, 0.0])).unwrap();
    m
}

fn plain() -> Warping {
    Warping::new().block(0, dsym().1)
}

/// The downward line load of the factor `lambda` (`lambda q_ref` N/m along `-z`).
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

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::FieldLength { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        other => format!("{other:?}"),
    }
}

/// D4 (ADR 0585 D4): the secant search of the zero of `g(s) = 1/Re X_d(s)`; with `negative_stop` an iterate `s < 0` ends the
/// search with that iterate (ADR 0594 D4). Returns `(s, steps)` or `None` after 20 steps or when an evaluation fails other
/// than by a resonance refusal.
fn resonance(
    x_d: impl Fn(f64) -> Result<f64, FemError>,
    s0: f64,
    s1: f64,
    negative_stop: bool,
) -> Option<(f64, usize)> {
    let g = |s: f64| -> Result<f64, FemError> { Ok(1.0 / x_d(s.sqrt())?) };
    let (mut s0, mut s1) = (s0, s1);
    let mut g0 = match g(s0) {
        Ok(v) => v,
        Err(e) => {
            println!("    first evaluation refused: {e}");
            return None;
        }
    };
    let mut g1 = match g(s1) {
        Ok(v) => v,
        Err(e) if key_of(&e) == "frequency" => return Some((s1, 0)),
        Err(_) => return None,
    };
    for step in 1..=20 {
        let s2 = s1 - g1 * (s1 - s0) / (g1 - g0);
        println!("    secant step {step}: s = {s2:.12e}");
        if negative_stop && s2 < 0.0 {
            println!("    negative iterate: the search ends");
            return Some((s2, step));
        }
        if (s2 - s1).abs() <= 1e-6 * s1.abs() {
            return Some((s2, step));
        }
        s0 = s1;
        g0 = g1;
        s1 = s2;
        g1 = match g(s1) {
            Ok(v) => v,
            Err(e) if key_of(&e) == "frequency" => {
                println!("    refused at step {step}: {e}");
                return Some((s1, step));
            }
            Err(e) => {
                println!("    evaluation failed at step {step}: {e}");
                return None;
            }
        };
    }
    None
}

/// The searched resonance `s` of the midspan twist of the fork beam under the midspan torque, with `pre`, `w` and the
/// resonance pivot `pivot` (`None`: the default), started at `s0`, `s1`.
fn search(pre: &WarpingPrestress, w: &Warping, pivot: Option<f64>, s0: f64, s1: f64, negative_stop: bool) -> f64 {
    let m = with_torque(fork());
    let e = eq(&m, N_EL / 2, 3);
    let load = HarmonicLoad::real(load_vector(&m, w).unwrap()).unwrap();
    let mut opts = WarpingHarmonicOptions::new().prestress(pre.clone());
    if let Some(p) = pivot {
        opts = opts.resonance_pivot(p);
    }
    let h = WarpingHarmonic::new(&m, w, &opts).unwrap();
    let (s, steps) =
        resonance(|om: f64| Ok(h.response(&load, om)?.re[e]), s0, s1, negative_stop).unwrap_or((f64::NAN, 21));
    println!("    resonance s {s:.12e} after {steps} steps");
    s
}

/// The unloaded `omega_0^2` of `modal_analysis` (D5).
fn omega0_sq() -> f64 {
    let md = modal_analysis(&fork(), &plain(), &ModalOptions::new(1)).unwrap();
    println!("  unloaded eigenvalues {:?}", md.eigenvalues);
    md.eigenvalues[0]
}

#[test]
fn m0_b1_mass_and_shear_centre_entry() {
    let mut c = Checks::new();
    // M0: the mass.
    let w0 = omega0_sq();
    c.at_most(
        "M0 unloaded omega_0 against the Timoshenko-Rayleigh omega_T",
        rel(w0.sqrt(), OMEGA_T),
        1e-4,
    );

    // B1: an entry at the shear centre on both paths gives the bits of no entry.
    let sc = &CASES[1];
    let lam = 0.5 * sc.lambda_ref;
    let pre = prestress(lam);
    let w_none = warping(None, lam);
    let w_sc = warping(Some([0.0, 0.0]), lam);
    let m = fork();
    let a = prestressed_modes(&m, &pre, &w_none, &ModalOptions::new(1)).unwrap();
    let b = prestressed_modes(&m, &pre, &w_sc, &ModalOptions::new(1)).unwrap();
    println!(
        "B1 modes: no entry {:?}, entry at (0, 0) {:?}",
        a.eigenvalues, b.eigenvalues
    );
    c.holds(
        "B1 modes eigenvalue bit-equal",
        a.eigenvalues[0].to_bits() == b.eigenvalues[0].to_bits(),
        &format!("{:e} / {:e}", a.eigenvalues[0], b.eigenvalues[0]),
    );
    let mh = with_torque(fork());
    let load = HarmonicLoad::real(load_vector(&mh, &w_none).unwrap()).unwrap();
    let om = (0.5 * sc.omega_ref * sc.omega_ref).sqrt();
    let opts = WarpingHarmonicOptions::new().prestress(pre.clone());
    let ra = WarpingHarmonic::new(&mh, &w_none, &opts)
        .unwrap()
        .response(&load, om)
        .unwrap();
    let rb = WarpingHarmonic::new(&mh, &w_sc, &opts)
        .unwrap()
        .response(&load, om)
        .unwrap();
    let same = ra.re.len() == rb.re.len()
        && ra.re.iter().zip(&rb.re).all(|(x, y)| x.to_bits() == y.to_bits())
        && ra.im.iter().zip(&rb.im).all(|(x, y)| x.to_bits() == y.to_bits());
    c.holds(
        "B1 harmonic response bit-equal",
        same,
        &format!("midspan twist {:e} / {:e}", ra.re[eq(&mh, 8, 3)], rb.re[eq(&mh, 8, 3)]),
    );
    c.finish();
}

#[test]
fn a_h_c_s_n_frequencies_at_half_the_buckling_load() {
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
        // A: the modes.
        let md = prestressed_modes(&m, &pre, &w, &ModalOptions::new(1)).unwrap();
        let ev = md.eigenvalues[0];
        println!(
            "  modes eigenvalue {ev:.12e}, residual {:?}, Sturm {:?}",
            md.residuals, md.sturm
        );
        c.at_most(
            &format!("A-{} modes omega against the reference (acceptance)", case.name),
            rel(ev.sqrt(), case.omega_ref),
            1e-2,
        );
        omegas.push(ev.sqrt());
        // S: the mode.
        let shape = (md.modes[0][e_phi] / md.modes[0][e_v]).abs();
        println!(
            "  mode 1: v(L/2) {:e}, phi(L/2) {:e}",
            md.modes[0][e_v], md.modes[0][e_phi]
        );
        c.at_most(
            &format!("S-{} |phi(L/2)/v(L/2)| against the reference", case.name),
            rel(shape, case.shape_ref),
            2e-2,
        );
        // H: the harmonic resonance, default pivot.
        let s_c = case.omega_ref * case.omega_ref;
        println!("  H-{} search, default pivot", case.name);
        let sr = search(&pre, &w, None, 0.98 * s_c, 1.02 * s_c, false);
        c.at_most(
            &format!("H-{} harmonic resonance against the reference (acceptance)", case.name),
            rel(sr.sqrt(), case.omega_ref),
            1e-2,
        );
        // C: the search without the resonance pivot against the modes.
        println!("  C-{} search, pivot 0", case.name);
        let s0 = search(&pre, &w, Some(0.0), 0.98 * s_c, 1.02 * s_c, false);
        c.at_most(
            &format!(
                "C-{} resonance (pivot 0) against the pre-stressed eigenvalue",
                case.name
            ),
            rel(s0, ev),
            1e-8,
        );
    }
    // N3: the ratios.
    c.at_most(
        "N3 omega_top/omega_sc against the reference ratio",
        rel(omegas[0] / omegas[1], RATIO_TOP_SC),
        2e-3,
    );
    c.at_most(
        "N3 omega_bot/omega_sc against the reference ratio",
        rel(omegas[2] / omegas[1], RATIO_BOT_SC),
        2e-3,
    );
    // N1: the top and bottom loads without the entry.
    for (i, limit) in [(0usize, 2e-2), (2, 5e-2)] {
        let case = &CASES[i];
        let lam = 0.5 * case.lambda_ref;
        let md = prestressed_modes(&m, &prestress(lam), &plain(), &ModalOptions::new(1)).unwrap();
        c.above(
            &format!("N1-{} no entry against the reference (negative control)", case.name),
            rel(md.eigenvalues[0].sqrt(), case.omega_ref),
            limit,
        );
    }
    // N2: the top entry left at q_ref.
    let top = &CASES[0];
    let lam = 0.5 * top.lambda_ref;
    let md = prestressed_modes(&m, &prestress(lam), &warping(top.point, 1.0), &ModalOptions::new(1)).unwrap();
    c.above(
        "N2-T entry at q_ref (not scaled) against the reference (negative control)",
        rel(md.eigenvalues[0].sqrt(), top.omega_ref),
        2e-2,
    );
    c.finish();
}

#[test]
fn z_zh_y_yh_frequency_falls_to_zero_at_the_buckling_load() {
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
            let pre = prestress(lam);
            let w = warping(case.point, lam);
            let md = prestressed_modes(&m, &pre, &w, &ModalOptions::new(1).shift(Shift::Value(sigma))).unwrap();
            println!(
                "  {tag}-{} modes eigenvalue {:.6e}, residual {:?}, Sturm {:?}",
                case.name, md.eigenvalues[0], md.residuals, md.sturm
            );
            c.at_most(
                &format!("{tag}-{} modes |omega^2|/omega_0^2 at lambda {lam:.10}", case.name),
                md.eigenvalues[0].abs() / w0,
                1e-2,
            );
            println!("  {tag}H-{} search near zero, default pivot", case.name);
            let sr = search(&pre, &w, None, 0.02 * w0, 0.01 * w0, true);
            c.at_most(
                &format!("{tag}H-{} harmonic |s_r|/omega_0^2 at lambda {lam:.10}", case.name),
                sr.abs() / w0,
                1e-2,
            );
        }
    }
    c.finish();
}
