//! F-417 (ADR 0477): random vibration by modal superposition. Checks S1 to S9, R1 to R3, M1 to M3, Q1 to Q5 and the negative
//! controls N1 to N4 of the registration; every value is printed.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::fatigue::spectral_rate;
use rcfd_fem::harmonic::{participation, HarmonicLoad, HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::mass::{consistent, rigid_motions};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    spectral_response, von_mises_mean_square, Excitation, ModalChannels, Quantity, RandomRequest, RandomResponse,
    RandomSolver, RandomStress, SpectralOptions, Spectrum,
};
use rcfd_fem::recover::{von_mises, IntegrationPointField};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

const ZETA: f64 = 0.02;
/// `I_k(10) = int_0^10 r^k/D dr` (ADR 0477 arithmetic 5).
const I0: f64 = 39.26957079652609;
const I1: f64 = 38.77258066850795;
const I2: f64 = 39.16923599186199;
const I4: f64 = 49.006230409610886;
const IB: f64 = 39.33224157411308;
const NU0_RATIO: f64 = 0.9987216695502159;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
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
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str, contains: &[&str]) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => {
            let s = format!("{e}");
            (key_of(&e) == Some(key) && contains.iter().all(|c| s.contains(c)), s)
        }
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

// --------------------------------------------------------------------------------------------------- one-mode system

const FN: f64 = 50.0;
const MASS: f64 = 2.0;
const G: f64 = 1e-2;

fn omega_n() -> f64 {
    2.0 * PI * FN
}

fn stiffness() -> f64 {
    MASS * omega_n() * omega_n()
}

/// The one-mode system as channels: force (gain 1/m) or base (gain -1, absolute 1).
fn one_mode(base: bool) -> ModalChannels {
    let w = omega_n();
    let gain = if base { -1.0 } else { 1.0 / MASS };
    let ch = ModalChannels::new(vec![w * w], vec![2.0 * ZETA * w], vec![vec![vec![gain]]], vec![vec![0.0]]).unwrap();
    if base {
        ch.with_absolute(vec![1.0]).unwrap()
    } else {
        ch
    }
}

fn run(ch: &ModalChannels, spectra: &[Spectrum], q: Quantity, refinement: usize) -> rcfd_fem::random::ChannelResponse {
    spectral_response(ch, spectra, None, &SpectralOptions::default().quantity(q).refinement(refinement)).unwrap()
}

#[test]
fn s1_to_s4_one_mode_force_and_base() {
    let mut c = Checks::new();
    let k = stiffness();
    let spec = [Spectrum::flat(0.0, 10.0 * FN, G).unwrap()];
    let q = 1.0 / (2.0 * ZETA);
    let force = one_mode(false);
    let base = one_mode(true);
    let exact_f = G * FN * I0 / (k * k);
    let exact_b = G * FN * IB;
    let mut ef = Vec::new();
    let mut eb = Vec::new();
    for r in [1, 2, 4] {
        let f = run(&force, &spec, Quantity::Displacement, r);
        let b = run(&base, &spec, Quantity::Acceleration, r);
        println!(
            "R = {r}: {} nodes, force m0 {:.15e}, base m0 {:.15e}",
            f.frequencies.len(),
            f.moments[0].m0,
            b.moments[0].m0
        );
        ef.push(rel(f.moments[0].m0, exact_f));
        eb.push(rel(b.moments[0].m0, exact_b));
        if r == 1 {
            let m = f.moments[0];
            c.at_most("S1 force m0 against (pi/2) f_n Q G/k^2", rel(m.m0, PI / 2.0 * FN * q * G / (k * k)), 5e-3);
            c.at_most("S1x force m0 against G f_n I_0(10)/k^2", ef[0], 1e-6);
            let bm = b.moments[0].m0;
            c.at_most("S2 base m0 against (pi/2) f_n Q G", rel(bm, PI / 2.0 * FN * q * G), 5e-3);
            c.at_most("S2x base m0 against G f_n (I_0 + 4 zeta^2 I_2)", eb[0], 1e-6);
            c.above("N1 base m0 against the force form G f_n I_0(10)", rel(bm, G * FN * I0), 1e-3);
            let nu0 = m.zero_crossing_rate().unwrap();
            c.at_most("S4 nu_0/f_n - 1", rel(nu0, FN), 1e-2);
            c.at_most("S4x nu_0 against f_n sqrt(I_2/I_0)", rel(nu0, FN * NU0_RATIO), 1e-6);
            c.at_most("Q3 m1", rel(m.m1, G * FN.powi(2) * I1 / (k * k)), 1e-6);
            c.at_most("Q3 m2", rel(m.m2, G * FN.powi(3) * I2 / (k * k)), 1e-6);
            c.at_most("Q3 m4", rel(m.m4, G * FN.powi(5) * I4 / (k * k)), 1e-6);
            let wn = omega_n();
            let v = run(&force, &spec, Quantity::Velocity, 1).moments[0].m0;
            let a = run(&force, &spec, Quantity::Acceleration, 1).moments[0].m0;
            c.at_most("Q3 velocity m0", rel(v, G * wn.powi(2) * FN * I2 / (k * k)), 1e-6);
            c.at_most("Q3 acceleration m0", rel(a, G * wn.powi(4) * FN * I4 / (k * k)), 1e-6);
            c.holds(
                "Q4 zero_crossing_rate is fatigue::spectral_rate",
                nu0.to_bits() == spectral_rate(m.m0, m.m2).unwrap().1.to_bits(),
                &format!("{nu0:e}"),
            );
        }
    }
    for (name, e) in [("force", &ef), ("base", &eb)] {
        println!("S3 {name} errors e_1, e_2, e_4: {:e}, {:e}, {:e}", e[0], e[1], e[2]);
        c.holds(&format!("S3 {name} e_1 > e_2 > e_4"), e[0] > e[1] && e[1] > e[2], "");
        for (k, o) in [(1, (e[0] / e[1]).log2()), (2, (e[1] / e[2]).log2())] {
            c.holds(&format!("S3 {name} observed order {k} (>= 4)"), o >= 4.0, &format!("{o:.4}"));
        }
    }
    c.finish();
}

#[test]
fn s5_two_well_separated_modes() {
    let mut c = Checks::new();
    let (f1, f2) = (50.0, 150.0);
    let (u1, u2) = (1.0, 3f64.powf(1.5));
    let w = |f: f64| 2.0 * PI * f;
    let spec = [Spectrum::flat(0.0, 1500.0, 1.0).unwrap()];
    let two = ModalChannels::new(
        vec![w(f1).powi(2), w(f2).powi(2)],
        vec![2.0 * ZETA * w(f1), 2.0 * ZETA * w(f2)],
        vec![vec![vec![u1, u2]]],
        vec![vec![0.0]],
    )
    .unwrap();
    let single = |f: f64, u: f64| {
        let ch = ModalChannels::new(vec![w(f).powi(2)], vec![2.0 * ZETA * w(f)], vec![vec![vec![u]]], vec![vec![0.0]]).unwrap();
        run(&ch, &spec, Quantity::Displacement, 1).moments[0].m0
    };
    let m = run(&two, &spec, Quantity::Displacement, 1).moments[0].m0;
    let q = 1.0 / (2.0 * ZETA);
    let closed = |f: f64, u: f64| u * u * PI / 2.0 * f * q / w(f).powi(4);
    let sum = closed(f1, u1) + closed(f2, u2);
    println!("S5 m0 {m:.15e}, closed sum {sum:.15e} (registered 4.031441804149936e-07)");
    c.at_most("S5 m0 against C_1 + C_2", rel(m, sum), 1e-2);
    c.at_most("S5x m0 against the exact 4.035601798182644e-07", rel(m, 4.035601798182644e-07), 1e-6);
    let r = m / (single(f1, u1) + single(f2, u2)) - 1.0;
    c.holds("N2 cross term resolved: ratio - 1 in [5e-4, 2e-3]", (5e-4..=2e-3).contains(&r), &format!("{r:.6e}"));
    c.finish();
}

#[test]
fn s6_band_limited_and_q1_q2_spectra() {
    let mut c = Checks::new();
    let w = 2.0 * PI * FN;
    let ch = ModalChannels::new(vec![w * w], vec![2.0 * ZETA * w], vec![vec![vec![w * w]]], vec![vec![0.0]]).unwrap();
    let lin = Spectrum::new(vec![30.0, 40.0, 55.0, 75.0], vec![0.0, 1.0, 1.0, 0.5]).unwrap();
    let ll = Spectrum::log_log(vec![20.0, 45.0, 60.0, 100.0], vec![1.0, 4.0, 4.0, 0.5]).unwrap();
    let ml = run(&ch, std::slice::from_ref(&lin), Quantity::Displacement, 1).moments[0].m0;
    let mll = run(&ch, std::slice::from_ref(&ll), Quantity::Displacement, 1).moments[0].m0;
    println!("S6 linear m0 {ml:.15e}, log-log m0 {mll:.15e}");
    c.at_most("S6 band-limited linear against 1886.2401793570568", rel(ml, 1886.2401793570568), 1e-6);
    c.at_most("S6b band-limited log-log against 7584.40095699183", rel(mll, 7584.40095699183), 1e-6);
    c.at_most("Q1 mean_square linear against 35", rel(lin.mean_square(), 35.0), 1e-12);
    c.at_most("Q1 mean_square log-log against 180.92544802681783", rel(ll.mean_square(), 180.92544802681783), 1e-12);
    let pass = ModalChannels::new(vec![], vec![], vec![vec![vec![]]], vec![vec![1.0]]).unwrap();
    let pl = run(&pass, std::slice::from_ref(&lin), Quantity::Displacement, 1).moments[0].m0;
    let pll = run(&pass, std::slice::from_ref(&ll), Quantity::Displacement, 1).moments[0].m0;
    c.at_most("Q2 pass-through linear against mean_square", rel(pl, lin.mean_square()), 1e-13);
    c.at_most("Q2 pass-through log-log against mean_square", rel(pll, ll.mean_square()), 1e-5);
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- the bar B

fn bar(nu: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [40, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, nu, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

/// The x equations of the four tip nodes and the tip pattern (0.25 N each).
fn tip(m: &SolidModel) -> (Vec<usize>, Vec<f64>) {
    let dofs = m.dof_map();
    let eqs: Vec<usize> = m.mesh().node_set("xmax").unwrap().iter().map(|&p| dofs.equation(p, 0).unwrap()).collect();
    let mut f = vec![0.0; dofs.n_equations()];
    for &e in &eqs {
        f[e] = 0.25;
    }
    (eqs, f)
}

fn opts(correction: bool) -> HarmonicOptions {
    HarmonicOptions::new(ModalDamping::Ratio(ZETA))
        .static_correction(correction)
        .threads(1)
}

fn uniaxial_ratio(s: &RandomStress) -> f64 {
    (0..s.covariance.len())
        .map(|p| rel(s.rms_von_mises(p), s.rms(p, 0)))
        .fold(0.0, f64::max)
}

#[test]
fn s7_uniaxial_von_mises_and_q4() {
    let mut c = Checks::new();
    for nu in [0.0, 0.3] {
        let m = bar(nu);
        let basis = ModalBasis::all_modes(&m, Some(1)).unwrap();
        let f1 = basis.eigenvalues()[0].sqrt() / (2.0 * PI);
        println!("bar nu = {nu}: {} modes, f_1 = {f1:.6} Hz", basis.len());
        let solver = RandomSolver::new(&m, basis, &opts(true)).unwrap();
        let (eqs, f) = tip(&m);
        let spec = Spectrum::flat(0.0, 10.0 * f1, 1.0).unwrap();
        let req = RandomRequest::new(eqs[..1].to_vec()).stress(true);
        let fr = solver.response(&m, &Excitation::force(f, spec.clone()), &req).unwrap();
        let br = solver
            .response(&m, &Excitation::base([1.0, 0.0, 0.0], spec).unwrap(), &req)
            .unwrap();
        let fs = fr.stress.as_ref().unwrap();
        let bs = br.stress.as_ref().unwrap();
        if nu == 0.0 {
            c.at_most("S7 force max |rms_vm/rms_xx - 1|", uniaxial_ratio(fs), 1e-10);
            c.at_most("S7 base max |rms_vm/rms_xx - 1|", uniaxial_ratio(bs), 1e-10);
            let rate = (0..fs.covariance.len())
                .map(|p| {
                    rel(
                        fs.von_mises_rate(p).unwrap(),
                        (fs.rate_covariance[p][0][0] / fs.covariance[p][0][0]).sqrt(),
                    )
                })
                .fold(0.0, f64::max);
            c.at_most("Q4 von_mises_rate against the xx component's rate", rate, 1e-10);
        } else {
            c.above("N4 nu = 0.3: max |rms_vm/rms_xx - 1|", uniaxial_ratio(fs), 0.1);
        }
    }
    // S7x: the form itself.
    let s = 3.7e6;
    let mut shear = [[0.0; 6]; 6];
    shear[3][3] = s * s;
    let mut bi = [[0.0; 6]; 6];
    for i in 0..2 {
        for j in 0..2 {
            bi[i][j] = s * s;
        }
    }
    c.at_most("S7x pure shear: 3 s^2", rel(von_mises_mean_square(&shear), 3.0 * s * s), 1e-12);
    c.at_most("S7x equal biaxial correlated: s^2", rel(von_mises_mean_square(&bi), s * s), 1e-12);
    for k in [1, 2] {
        let mut u = [[0.0; 6]; 6];
        u[k][k] = s * s;
        c.at_most(&format!("S7x uniaxial component {k}"), rel(von_mises_mean_square(&u), s * s), 1e-12);
    }
    let vs: [[f64; 6]; 5] = [
        [1.0, 2.0, 3.0, 0.5, -0.25, 0.75],
        [-4.0, 1.5, 0.2, 2.0, 1.0, -3.0],
        [10.0, -10.0, 0.0, 0.0, 0.0, 0.0],
        [0.3, 0.3, 0.3, 1.0, 1.0, 1.0],
        [7.0, -2.0, 5.0, -1.0, 0.5, 2.5],
    ];
    for (k, v) in vs.iter().enumerate() {
        let cv: [[f64; 6]; 6] = std::array::from_fn(|i| std::array::from_fn(|j| v[i] * v[j]));
        c.at_most(&format!("S7x C = v v^T, vector {k}"), rel(von_mises_mean_square(&cv), von_mises(v).powi(2)), 1e-12);
    }
    c.finish();
}

#[test]
fn m1_to_m3_bar_model_path() {
    let mut c = Checks::new();
    let m = bar(0.0);
    let res = modal_analysis(
        &m,
        &ModalOptions::new(1)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let basis = ModalBasis::from_result(&res).unwrap();
    let l1 = basis.eigenvalues()[0];
    let f1 = l1.sqrt() / (2.0 * PI);
    let phi = basis.modes()[0].clone();
    let solver = RandomSolver::new(&m, basis.clone(), &opts(false)).unwrap();
    let (eqs, f) = tip(&m);
    let spec = Spectrum::flat(0.0, 10.0 * f1, 1.0).unwrap();
    // M1.
    let p: f64 = phi.iter().zip(&f).map(|(a, b)| a * b).sum();
    let r = solver
        .response(&m, &Excitation::force(f.clone(), spec.clone()), &RandomRequest::new(vec![eqs[0]]).stress(true))
        .unwrap();
    let want = f1 * (phi[eqs[0]] * p).powi(2) * I0 / (l1 * l1);
    println!("M1 f_1 {f1:.6} Hz, m0 {:.15e}, formula {want:.15e}", r.moments[0].m0);
    c.at_most("M1 tip m0 against G f_1 (phi_L p)^2 I_0/lambda^2", rel(r.moments[0].m0, want), 1e-5);
    let zero = IntegrationPointField::compute(&m, &vec![0.0; phi.len()]).unwrap();
    let sp = IntegrationPointField::compute(&m, &phi).unwrap();
    let st = r.stress.as_ref().unwrap();
    let worst = (0..st.covariance.len())
        .map(|k| {
            let sxx = (sp.stress()[k][0] - zero.stress()[k][0]).abs();
            rel(st.rms(k, 0), sxx * p.abs() * (f1 * I0).sqrt() / l1)
        })
        .fold(0.0, f64::max);
    c.at_most("M1 rms_xx at every point against the closed form", worst, 1e-5);
    // M2.
    let gamma = participation(&m, &basis, [1.0, 0.0, 0.0], Some(1)).unwrap().factors[0];
    let a = phi[eqs[0]] * gamma;
    let rb = solver
        .response(
            &m,
            &Excitation::base([1.0, 0.0, 0.0], spec.clone()).unwrap(),
            &RandomRequest::new(vec![eqs[0]]).quantity(Quantity::Acceleration),
        )
        .unwrap();
    let z2 = 4.0 * ZETA * ZETA;
    let form = 10.0 * (1.0 - a).powi(2) + a * (2.0 - a) * I0 + (z2 * a * (2.0 - a) - 2.0 * a * (1.0 - a)) * I2;
    println!("M2 a {a:.12}, m0/(G f_1) {:.15e}, formula {form:.15e}", rb.moments[0].m0 / f1);
    c.at_most("M2 tip absolute acceleration against the formula", rel(rb.moments[0].m0 / f1, form), 1e-5);
    c.at_most("M2a a/(4/pi) - 1", rel(a, 4.0 / PI), 1e-3);
    // M3: every mode, static correction on.
    let all = RandomSolver::new(&m, ModalBasis::all_modes(&m, Some(1)).unwrap(), &opts(true)).unwrap();
    let rbase = all
        .response(&m, &Excitation::base([1.0, 0.0, 0.0], spec.clone()).unwrap(), &RandomRequest::new(eqs.clone()))
        .unwrap();
    let mass = consistent(&m).unwrap();
    let rx = rigid_motions(&m, [0.0; 3])[0].clone();
    let mut mr = vec![0.0; mass.n];
    mass.matvec(&rx, &mut mr);
    let mut load: Vec<f64> = mr.iter().map(|x| -x).collect();
    for (e, _) in prescribed_equations(&m, &[]).unwrap() {
        load[e] = 0.0;
    }
    let rforce = all.response(&m, &Excitation::force(load, spec), &RandomRequest::new(eqs.clone())).unwrap();
    let worst = (0..eqs.len())
        .map(|o| rel(rbase.moments[o].m0, rforce.moments[o].m0))
        .fold(0.0, f64::max);
    c.at_most("M3 base relative displacement against the force path with -M r", worst, 1e-12);
    c.finish();
}

// ---------------------------------------------------------------------------------------------- the cantilever C

fn cantilever() -> (SolidModel, Vec<f64>, Vec<f64>, Vec<usize>) {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let dofs = m.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let (a, b) = (find([1.0, 0.1, 0.08]), find([1.0, 0.0, 0.0]));
    let n = dofs.n_equations();
    let mut p1 = vec![0.0; n];
    let mut p2 = vec![0.0; n];
    p1[dofs.equation(a, 1).unwrap()] = 1.0;
    p2[dofs.equation(b, 1).unwrap()] = 1.0;
    let eqs: Vec<usize> = [a, b]
        .iter()
        .flat_map(|&v| (0..3).map(move |c| (v, c)))
        .map(|(v, c)| dofs.equation(v, c).unwrap())
        .collect();
    (m, p1, p2, eqs)
}

fn disp_metric(a: &RandomResponse, b: &[f64]) -> f64 {
    let scale = b.iter().copied().fold(0.0, f64::max);
    a.moments.iter().zip(b).map(|(x, y)| (x.m0 - y).abs()).fold(0.0, f64::max) / scale
}

fn stress_metric(a: &RandomStress, b: &[[[f64; 6]; 6]]) -> f64 {
    a.covariance
        .iter()
        .zip(b)
        .map(|(x, y)| {
            let scale = (0..6).map(|k| y[k][k]).fold(0.0, f64::max);
            let mut d: f64 = 0.0;
            for i in 0..6 {
                for j in 0..6 {
                    d = d.max((x[i][j] - y[i][j]).abs());
                }
            }
            d / scale
        })
        .fold(0.0, f64::max)
}

fn m0s(r: &RandomResponse) -> Vec<f64> {
    r.moments.iter().map(|m| m.m0).collect()
}

#[test]
fn s8_s9_correlated_patterns_and_q5() {
    let mut c = Checks::new();
    let (m, p1, p2, eqs) = cantilever();
    let res = modal_analysis(
        &m,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let g = Spectrum::flat(5.0, 2000.0, 1.0).unwrap();
    let g4 = g.scaled(4.0).unwrap();
    let one = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
    let req = RandomRequest::new(eqs.clone()).stress(true);
    let add = |x: &[f64], y: &[f64], s: f64| -> Vec<f64> { x.iter().zip(y).map(|(a, b)| a + s * b).collect() };
    for corr in [false, true] {
        let solver = RandomSolver::new(&m, ModalBasis::from_result(&res).unwrap(), &opts(corr)).unwrap();
        let both = solver
            .response(
                &m,
                &Excitation::forces(vec![p1.clone(), p2.clone()], vec![g.clone(), g.clone()], Some(one.clone())).unwrap(),
                &req,
            )
            .unwrap();
        let summed = solver
            .response(&m, &Excitation::force(add(&p1, &p2, 1.0), g.clone()), &req)
            .unwrap();
        let (name, lim) = if corr { ("S8s", 1e-11) } else { ("S8", 1e-12) };
        println!("{name}: {} nodes", both.frequencies.len());
        c.at_most(&format!("{name} displacement, rho = 1 against P_1 + P_2"), disp_metric(&both, &m0s(&summed)), lim);
        c.at_most(
            &format!("{name} stress, rho = 1 against P_1 + P_2"),
            stress_metric(both.stress.as_ref().unwrap(), &summed.stress.as_ref().unwrap().covariance),
            lim,
        );
        if !corr {
            let b4 = solver
                .response(
                    &m,
                    &Excitation::forces(vec![p1.clone(), p2.clone()], vec![g.clone(), g4.clone()], Some(one.clone())).unwrap(),
                    &req,
                )
                .unwrap();
            let s2 = solver
                .response(&m, &Excitation::force(add(&p1, &p2, 2.0), g.clone()), &req)
                .unwrap();
            c.at_most("S8b displacement, G_2 = 4 G against P_1 + 2 P_2", disp_metric(&b4, &m0s(&s2)), 1e-12);
            c.at_most(
                "S8b stress, G_2 = 4 G against P_1 + 2 P_2",
                stress_metric(b4.stress.as_ref().unwrap(), &s2.stress.as_ref().unwrap().covariance),
                1e-12,
            );
        } else {
            let unc = solver
                .response(
                    &m,
                    &Excitation::forces(vec![p1.clone(), p2.clone()], vec![g.clone(), g.clone()], None).unwrap(),
                    &req,
                )
                .unwrap();
            let r1 = solver.response(&m, &Excitation::force(p1.clone(), g.clone()), &req).unwrap();
            let r2 = solver.response(&m, &Excitation::force(p2.clone(), g.clone()), &req).unwrap();
            let sum: Vec<f64> = m0s(&r1).iter().zip(m0s(&r2)).map(|(a, b)| a + b).collect();
            c.at_most("S9 displacement, rho = I against m0(P_1) + m0(P_2)", disp_metric(&unc, &sum), 1e-12);
            let s1 = &r1.stress.as_ref().unwrap().covariance;
            let s2 = &r2.stress.as_ref().unwrap().covariance;
            let ssum: Vec<[[f64; 6]; 6]> = s1
                .iter()
                .zip(s2)
                .map(|(a, b)| std::array::from_fn(|i| std::array::from_fn(|j| a[i][j] + b[i][j])))
                .collect();
            c.at_most("S9 stress, rho = I against C_0(P_1) + C_0(P_2)", stress_metric(unc.stress.as_ref().unwrap(), &ssum), 1e-12);
            // N3: the tip u_y of node a (output 1).
            c.above("N3 rho = I against the summed pattern, tip u_y", rel(unc.moments[1].m0, summed.moments[1].m0), 0.1);
            // Q5: against the F-399 response at the same nodes.
            let omegas: Vec<f64> = r1.frequencies.iter().map(|f| 2.0 * PI * f).collect();
            let sweep = solver.harmonic().sweep(&HarmonicLoad::real(p1.clone()).unwrap(), &omegas).unwrap();
            let mut worst: f64 = 0.0;
            for (o, &e) in eqs.iter().enumerate().take(3) {
                let direct: f64 = sweep
                    .iter()
                    .zip(&r1.weights)
                    .zip(&r1.frequencies)
                    .map(|((y, w), f)| w * g.value(*f) * (y.re[e] * y.re[e] + y.im[e] * y.im[e]))
                    .sum();
                worst = worst.max(rel(r1.moments[o].m0, direct));
            }
            c.at_most("Q5 tip m0 against the F-399 response at the nodes", worst, 1e-11);
        }
    }
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- refusals

#[test]
fn r1_to_r3_refusals() {
    let mut c = Checks::new();
    let (ok, d) = refused_with(Spectrum::new(vec![10.0, 20.0, 30.0], vec![1.0, -1.0, 1.0]), "spectrum", &["entry 1", "-1"]);
    c.holds("R1 negative value", ok, &d);
    for fs in [vec![10.0, 20.0, 20.0], vec![10.0, 30.0, 20.0]] {
        let (ok, d) = refused_with(Spectrum::new(fs.clone(), vec![1.0; 3]), "spectrum", &["entry 2"]);
        c.holds(&format!("R2 frequencies {fs:?}"), ok, &d);
    }
    let cases: Vec<(&str, rcfd_fem::Result<Spectrum>, &[&str])> = vec![
        ("one entry", Spectrum::new(vec![10.0], vec![1.0]), &[]),
        ("lengths 3 and 2", Spectrum::new(vec![10.0, 20.0, 30.0], vec![1.0, 1.0]), &[]),
        ("NaN frequency", Spectrum::new(vec![10.0, f64::NAN], vec![1.0, 1.0]), &["entry 1"]),
        ("negative frequency", Spectrum::new(vec![-1.0, 10.0], vec![1.0, 1.0]), &["entry 0"]),
        ("NaN value", Spectrum::new(vec![10.0, 20.0], vec![1.0, f64::NAN]), &["entry 1"]),
        ("log-log zero value", Spectrum::log_log(vec![10.0, 20.0], vec![1.0, 0.0]), &["entry 1"]),
        ("log-log frequency 0", Spectrum::log_log(vec![0.0, 20.0], vec![1.0, 1.0]), &["entry 0"]),
    ];
    for (name, r, contains) in cases {
        let (ok, d) = refused_with(r, "spectrum", contains);
        c.holds(&format!("R3 spectrum {name}"), ok, &d);
    }
    let g = Spectrum::flat(5.0, 100.0, 1.0).unwrap();
    let pat = vec![vec![1.0; 3], vec![2.0; 3]];
    for (name, rho) in [
        ("not symmetric", vec![vec![1.0, 0.5], vec![0.4, 1.0]]),
        ("diagonal 0.9", vec![vec![0.9, 0.0], vec![0.0, 1.0]]),
        ("rho 1.5", vec![vec![1.0, 1.5], vec![1.5, 1.0]]),
    ] {
        let (ok, d) = refused_with(
            Excitation::forces(pat.clone(), vec![g.clone(), g.clone()], Some(rho)),
            "correlation",
            &[],
        );
        c.holds(&format!("R3 correlation {name}"), ok, &d);
    }
    let (ok, d) = refused_with(Excitation::forces(pat.clone(), vec![g.clone()], None), "random", &[]);
    c.holds("R3 two patterns, one spectrum", ok, &d);
    let w = 2.0 * PI * 50.0;
    let undamped = ModalChannels::new(vec![w * w], vec![0.0], vec![vec![vec![1.0]]], vec![vec![0.0]]).unwrap();
    let (ok, d) = refused_with(
        spectral_response(&undamped, &[Spectrum::flat(0.0, 500.0, 1.0).unwrap()], None, &SpectralOptions::default()),
        "random",
        &["mode 0"],
    );
    c.holds("R3 undamped mode in the band", ok, &d);
    // Base excitation of a free model and of a model with an elastic support.
    let mut free = SolidModel::new(hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap());
    free.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    let fb = ModalBasis::all_modes(&free, Some(1)).unwrap();
    let fs = RandomSolver::new(&free, fb, &opts(false)).unwrap();
    let base = Excitation::base([0.0, 0.0, 1.0], g.clone()).unwrap();
    let (ok, d) = refused_with(fs.response(&free, &base, &RandomRequest::new(vec![0])), "base excitation", &[]);
    c.holds("R3 base excitation of a free body", ok, &d);
    let mut clamped = free.clone();
    for k in 0..3 {
        clamped.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let cb = ModalBasis::all_modes(&clamped, Some(1)).unwrap();
    let mut sprung = clamped.clone();
    sprung.add_support(ElasticSupport::nodal_spring("xmax", 0, 1e6)).unwrap();
    let ss = RandomSolver::new(&sprung, cb, &opts(false)).unwrap();
    let (ok, d) = refused_with(ss.response(&sprung, &base, &RandomRequest::new(vec![0])), "base excitation", &[]);
    c.holds("R3 base excitation with an elastic support", ok, &d);
    c.finish();
}
