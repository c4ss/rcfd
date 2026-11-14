//! F-589 (ADR 0516): the covariances `C_1` and `C_4` of the random stress, the moments of a stress component and of a linear
//! combination, the broadband rate fed from them, the refusal of the von Mises equivalent. Checks A1 to A4, X1, X2, X2b, V1, V1b,
//! V2, L1, B2, N1, N2 of the registration; every value is printed.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    spectral_response, spectral_response_cross, ChannelResponse, Coherence, CrossSpectrum, Excitation, ModalChannels,
    Moments, RandomRequest, RandomResponse, RandomSolver, RandomStress, SpectralOptions, Spectrum,
};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

const ZETA: f64 = 0.02;
const I0: f64 = 39.26957079652609;
const I1: f64 = 38.77258066850795;
const I4: f64 = 49.006230409610886;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    /// `value <= limit`, the value finite.
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    /// `value > limit`, the value finite.
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    /// `value >= limit`, the value finite.
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

/// The error of two numbers: 0 if equal, else `|a - b|/max(|a|, |b|)`.
fn err(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// The error of two 6x6 matrices: `max |A - B| / max |B|` (0 for two zero matrices).
fn mat_err(a: &[[f64; 6]; 6], b: &[[f64; 6]; 6]) -> f64 {
    let mut num: f64 = 0.0;
    let mut den: f64 = 0.0;
    for i in 0..6 {
        for j in 0..6 {
            let d = (a[i][j] - b[i][j]).abs();
            if !d.is_finite() {
                return f64::NAN;
            }
            num = num.max(d);
            den = den.max(b[i][j].abs());
        }
    }
    if num == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// The largest of the values, NaN if any is not finite (the maximum is never taken over a NaN).
fn worst(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut w: f64 = 0.0;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    w
}

fn key_is(e: &FemError, key: &str) -> bool {
    matches!(e, FemError::InvalidValue { key: k, .. } if *k == key)
}

/// `Ok` is a miss; an error of this key whose text contains every word is the expected refusal.
fn refused<T>(r: rcfd_fem::Result<T>, key: &str, words: &[&str]) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => {
            let s = format!("{e}");
            (key_is(&e, key) && words.iter().all(|w| s.contains(w)), s)
        }
    }
}

// ----------------------------------------------------------------------------------------------------- the bar B, one mode

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

struct OneMode {
    model: SolidModel,
    solver: RandomSolver,
    f1: f64,
    /// The output equation (x of the first tip node) and the mode's value there.
    e: usize,
    phi_e: f64,
    /// The tip patterns: 0.25 N on the four tip nodes, and on the first only.
    pattern_all: Vec<f64>,
    pattern_one: Vec<f64>,
    /// The stress of the mode at every point.
    s: Vec<[f64; 6]>,
}

fn one_mode(nu: f64) -> OneMode {
    let m = bar(nu);
    let res = modal_analysis(
        &m,
        &ModalOptions::new(1)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let basis = ModalBasis::from_result(&res).unwrap();
    assert_eq!(basis.len(), 1);
    let f1 = basis.eigenvalues()[0].sqrt() / (2.0 * PI);
    let phi = basis.modes()[0].clone();
    let solver = RandomSolver::new(
        &m,
        basis,
        &HarmonicOptions::new(ModalDamping::Ratio(ZETA))
            .static_correction(false)
            .threads(1),
    )
    .unwrap();
    let dofs = m.dof_map();
    let eqs: Vec<usize> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&p| dofs.equation(p, 0).unwrap())
        .collect();
    let mut pattern_all = vec![0.0; dofs.n_equations()];
    for &e in &eqs {
        pattern_all[e] = 0.25;
    }
    let mut pattern_one = vec![0.0; dofs.n_equations()];
    pattern_one[eqs[0]] = 0.25;
    let zero = IntegrationPointField::compute(&m, &vec![0.0; phi.len()]).unwrap();
    let sp = IntegrationPointField::compute(&m, &phi).unwrap();
    let s: Vec<[f64; 6]> = sp
        .stress()
        .iter()
        .zip(zero.stress())
        .map(|(a, z)| std::array::from_fn(|c| a[c] - z[c]))
        .collect();
    OneMode {
        phi_e: phi[eqs[0]],
        e: eqs[0],
        model: m,
        solver,
        f1,
        pattern_all,
        pattern_one,
        s,
    }
}

impl OneMode {
    fn spec(&self) -> Spectrum {
        Spectrum::flat(0.0, 10.0 * self.f1, 1.0).unwrap()
    }
    fn request(&self) -> RandomRequest {
        RandomRequest::new(vec![self.e]).stress(true)
    }
    fn force(&self) -> RandomResponse {
        self.solver
            .response(
                &self.model,
                &Excitation::force(self.pattern_all.clone(), self.spec()),
                &self.request(),
            )
            .unwrap()
    }
    fn base(&self) -> RandomResponse {
        self.solver
            .response(
                &self.model,
                &Excitation::base([1.0, 0.0, 0.0], self.spec()).unwrap(),
                &self.request(),
            )
            .unwrap()
    }
    /// A response to two tip patterns with the pure delay `tau` (the general complex loop).
    fn delayed(&self, tau: f64) -> RandomResponse {
        let cs = CrossSpectrum::coherent(
            vec![self.spec(), self.spec()],
            Coherence::function(2, move |f, _, _| {
                let phase = -2.0 * PI * f * tau;
                (phase.cos(), phase.sin())
            }),
        )
        .unwrap();
        self.solver
            .response(
                &self.model,
                &Excitation::cross(vec![self.pattern_all.clone(), self.pattern_one.clone()], cs).unwrap(),
                &self.request(),
            )
            .unwrap()
    }
    /// The stress per unit displacement of component `c` at point `p`.
    fn k(&self, p: usize, c: usize) -> f64 {
        self.s[p][c] / self.phi_e
    }
}

fn moment_array(m: &Moments) -> [f64; 4] {
    [m.m0, m.m1, m.m2, m.m4]
}

/// `[C_0, C_1, C_2, C_4]` at point `p`.
fn cov4(s: &RandomStress, p: usize) -> [&[[f64; 6]; 6]; 4] {
    [
        &s.covariance[p],
        &s.first_covariance[p],
        &s.rate_covariance[p],
        &s.fourth_covariance[p],
    ]
}

/// The rank-one prediction `s s^T m_k/phi_e^2` of `C_k` at point `p` for the moments of the channel.
fn rank_one(om: &OneMode, p: usize, m: f64) -> [[f64; 6]; 6] {
    std::array::from_fn(|i| std::array::from_fn(|j| om.s[p][i] * om.s[p][j] * m / (om.phi_e * om.phi_e)))
}

#[test]
fn a1_a2_a3_uniaxial_force_and_base_n1_n2_v1() {
    let mut c = Checks::new();
    let om = one_mode(0.0);
    let np = om.s.len();
    println!("A1: f_1 {:.6} Hz, {np} points, phi_e {:e}", om.f1, om.phi_e);
    for (name, r) in [("force", om.force()), ("base", om.base())] {
        let st = r.stress.as_ref().unwrap();
        assert_eq!(st.first_covariance.len(), np);
        assert_eq!(st.fourth_covariance.len(), np);
        let mc = moment_array(&r.moments[0]);
        for (idx, kname) in ["m0", "m1", "m2", "m4"].iter().enumerate() {
            let w = worst((0..np).map(|p| {
                let k = om.k(p, 0);
                err(cov4(st, p)[idx][0][0], k * k * mc[idx])
            }));
            c.at_most(
                &format!("A1 nu=0 {name}: worst Err of C_{kname}[0][0] against k_p^2 m_k"),
                w,
                1e-10,
            );
        }
        // The same through RandomStress::moments.
        let w = worst((0..np).flat_map(|p| {
            let k = om.k(p, 0);
            let sm = moment_array(&st.moments(p, 0).unwrap());
            (0..4).map(move |i| err(sm[i], k * k * mc[i]))
        }));
        c.at_most(&format!("A1 nu=0 {name}: worst Err of RandomStress::moments"), w, 1e-10);
    }
    // A2: the broadband rate, m = 3 and 5.
    let r = om.force();
    let st = r.stress.as_ref().unwrap();
    for ex in [3.0, 5.0] {
        let ch = r.moments[0].broadband_damage_rate(ex, 1.0).unwrap();
        let w = worst((0..np).map(|p| {
            let want = om.k(p, 0).abs().powf(ex) * ch;
            err(st.broadband_damage_rate(p, 0, ex, 1.0).unwrap(), want)
        }));
        c.at_most(
            &format!("A2 nu=0 force m = {ex}: worst Err of the stress rate against |k_p|^m channel rate"),
            w,
            1e-10,
        );
    }
    // A3: the closed forms of ADR 0477.
    let (c0, c1, c4) = (
        st.covariance[0][0][0],
        st.first_covariance[0][0][0],
        st.fourth_covariance[0][0][0],
    );
    c.at_most("A3 C_1/C_0 against f_1 I_1/I_0", err(c1 / c0, om.f1 * I1 / I0), 1e-6);
    c.at_most(
        "A3 C_4/C_0 against f_1^4 I_4/I_0",
        err(c4 / c0, om.f1.powi(4) * I4 / I0),
        1e-6,
    );
    // N1: the k of the next point.
    let w = (0..np - 1)
        .map(|p| {
            let k = om.k(p + 1, 0);
            err(st.covariance[p][0][0], k * k * r.moments[0].m0)
        })
        .fold(0.0, |a: f64, b: f64| if b.is_finite() { a.max(b) } else { f64::NAN });
    c.above("N1 Err with the k_p of the next point", w, 1e-3);
    // N2: m_1 and m_4 resolve the broadband rate from the narrow-band one.
    let sm = st.moments(0, 0).unwrap();
    let ratio = sm.broadband_damage_rate(3.0, 1.0).unwrap() / sm.narrow_band_damage_rate(3.0, 1.0).unwrap();
    c.at_least(
        "N2 |broadband/narrow-band - 1| of the stress moments",
        (ratio - 1.0).abs(),
        5e-3,
    );
    // V1 and V1b.
    let uni = worst((0..np).map(|p| (st.rms_von_mises(p) / st.rms(p, 0) - 1.0).abs()));
    c.at_most(
        "V1 nu=0: worst |rms_vm/rms_xx - 1| (the equivalent equals |sigma_xx|)",
        uni,
        1e-10,
    );
    let mut all_refused = true;
    let mut accepted = true;
    let mut sample = String::new();
    for p in 0..np {
        let (ok, text) = refused(
            st.von_mises_broadband_damage_rate(p, 3.0, 1.0),
            "broadband",
            &["von Mises", "Gaussian"],
        );
        all_refused &= ok;
        sample = text;
        let rate = st.broadband_damage_rate(p, 0, 3.0, 1.0);
        accepted &= matches!(rate, Ok(v) if v.is_finite() && v > 0.0);
    }
    c.holds("V1 nu=0: von Mises refused at every point", all_refused, &sample);
    c.holds(
        "V1b nu=0: the xx component accepted, finite and positive, at every point",
        accepted,
        "",
    );
    c.finish();
}

#[test]
fn a1b_l1_v1_rank_one_six_components_nu_03() {
    let mut c = Checks::new();
    let om = one_mode(0.3);
    let np = om.s.len();
    let r = om.force();
    let st = r.stress.as_ref().unwrap();
    let mc = moment_array(&r.moments[0]);
    for (idx, kname) in ["0", "1", "2", "4"].iter().enumerate() {
        let w = worst((0..np).map(|p| mat_err(cov4(st, p)[idx], &rank_one(&om, p, mc[idx]))));
        c.at_most(&format!("A1b nu=0.3: worst matrix Err of C_{kname}"), w, 1e-10);
    }
    // L1: a plane normal.
    let n = [2.0 / 3.0, -1.0 / 3.0, 2.0 / 3.0];
    let wv = [
        n[0] * n[0],
        n[1] * n[1],
        n[2] * n[2],
        2.0 * n[0] * n[1],
        2.0 * n[1] * n[2],
        2.0 * n[2] * n[0],
    ];
    let wl = worst((0..np).flat_map(|p| {
        let proj: f64 = (0..6).map(|i| wv[i] * om.s[p][i]).sum();
        let cm = moment_array(&st.combination_moments(p, &wv).unwrap());
        (0..4).map(move |i| err(cm[i], proj * proj * mc[i] / (om.phi_e * om.phi_e)))
    }));
    c.at_most(
        "L1 nu=0.3: worst Err of combination_moments, plane normal (2,-1,2)/3",
        wl,
        1e-10,
    );
    // V1 on the multiaxial state.
    let mut ok_all = true;
    let mut sample = String::new();
    for p in 0..np {
        let (ok, text) = refused(
            st.von_mises_broadband_damage_rate(p, 3.0, 1.0),
            "broadband",
            &["von Mises", "Gaussian"],
        );
        ok_all &= ok;
        sample = text;
    }
    c.holds("V1 nu=0.3: von Mises refused at every point", ok_all, &sample);
    c.finish();
}

#[test]
fn x2_complex_delay_through_the_solver() {
    let mut c = Checks::new();
    let om = one_mode(0.3);
    let np = om.s.len();
    let tau = 1.0 / (4.0 * om.f1);
    let r = om.delayed(tau);
    let st = r.stress.as_ref().unwrap();
    let mc = moment_array(&r.moments[0]);
    for (idx, kname) in ["0", "1", "2", "4"].iter().enumerate() {
        let w = worst((0..np).map(|p| mat_err(cov4(st, p)[idx], &rank_one(&om, p, mc[idx]))));
        c.at_most(
            &format!("X2 pure delay tau = 1/(4 f_1), nu=0.3: worst matrix Err of C_{kname}"),
            w,
            1e-10,
        );
    }
    let r0 = om.delayed(0.0);
    let s0 = r0.stress.as_ref().unwrap();
    let w = worst((0..np).map(|p| (st.first_covariance[p][0][0] / s0.first_covariance[p][0][0] - 1.0).abs()));
    c.above("X2b: max |C_1(tau)/C_1(0) - 1|", w, 1e-3);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the real route against the loop

fn spectra() -> Vec<Spectrum> {
    vec![
        Spectrum::flat(10.0, 900.0, 2.0).unwrap(),
        Spectrum::new(vec![20.0, 80.0, 400.0, 700.0], vec![1.0, 3.0, 3.0, 0.5]).unwrap(),
        Spectrum::log_log(vec![30.0, 120.0, 600.0], vec![0.5, 4.0, 1.0]).unwrap(),
    ]
}

fn channels() -> ModalChannels {
    let freqs = [60.0_f64, 140.0, 310.0];
    let eig: Vec<f64> = freqs.iter().map(|f| (2.0 * PI * f).powi(2)).collect();
    let damp: Vec<f64> = freqs.iter().map(|f| 2.0 * 0.02 * 2.0 * PI * f).collect();
    let gains: Vec<Vec<Vec<f64>>> = (0..3)
        .map(|a| {
            (0..6)
                .map(|o| {
                    (0..3)
                        .map(|r| 1e3 * (0.3 + 0.1 * a as f64 - 0.2 * o as f64 + 0.35 * r as f64 * (a as f64 - 1.0)))
                        .collect()
                })
                .collect()
        })
        .collect();
    let res: Vec<Vec<f64>> = (0..3)
        .map(|a| (0..6).map(|o| 1e-3 * (a as f64 + 0.5 * o as f64)).collect())
        .collect();
    ModalChannels::new(eig, damp, gains, res).unwrap()
}

fn flat_err(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    let mut num: f64 = 0.0;
    let mut den: f64 = 0.0;
    for (x, y) in a.iter().flatten().zip(b.iter().flatten()) {
        let d = (x - y).abs();
        if !d.is_finite() {
            return f64::NAN;
        }
        num = num.max(d);
        den = den.max(y.abs());
    }
    if num == 0.0 {
        0.0
    } else {
        num / den
    }
}

fn group_bits_equal_moments(r: &ChannelResponse) -> bool {
    r.moments.iter().enumerate().all(|(k, m)| {
        r.first_covariance[k][0].to_bits() == m.m1.to_bits() && r.fourth_covariance[k][0].to_bits() == m.m4.to_bits()
    })
}

#[test]
fn x1_b2_real_route_against_the_general_loop() {
    let mut c = Checks::new();
    let ch = channels();
    let rho = vec![vec![1.0, 0.6, -0.3], vec![0.6, 1.0, 0.2], vec![-0.3, 0.2, 1.0]];
    let cs = CrossSpectrum::coherent(spectra(), Coherence::constant(rho.clone()).unwrap()).unwrap();
    for group in [3, 6] {
        let fast = spectral_response_cross(&ch, &cs, &SpectralOptions::default().group(group)).unwrap();
        let slow =
            spectral_response_cross(&ch, &cs, &SpectralOptions::default().group(group).real_route(false)).unwrap();
        let direct = spectral_response(&ch, &spectra(), Some(&rho), &SpectralOptions::default().group(group)).unwrap();
        c.at_most(
            &format!("X1 group {group}: Err of first_covariance, general loop against real route"),
            flat_err(&slow.first_covariance, &fast.first_covariance),
            1e-12,
        );
        c.at_most(
            &format!("X1 group {group}: Err of fourth_covariance, general loop against real route"),
            flat_err(&slow.fourth_covariance, &fast.fourth_covariance),
            1e-12,
        );
        c.at_most(
            &format!("X1 group {group}: Err of fourth_covariance, real route against spectral_response"),
            flat_err(&fast.fourth_covariance, &direct.fourth_covariance),
            1e-12,
        );
        // Both are non-trivial: the first covariance differs from the second by a factor of order f.
        c.above(
            &format!("X1 group {group}: C_1 resolved from C_0 (Err)"),
            flat_err(&fast.first_covariance, &fast.covariance),
            1e-3,
        );
    }
    let real = spectral_response_cross(&ch, &cs, &SpectralOptions::default()).unwrap();
    let general = spectral_response_cross(&ch, &cs, &SpectralOptions::default().real_route(false)).unwrap();
    let direct = spectral_response(&ch, &spectra(), Some(&rho), &SpectralOptions::default()).unwrap();
    c.holds(
        "B2 group 1, real route",
        group_bits_equal_moments(&real),
        "C_1, C_4 bit-equal m1, m4",
    );
    c.holds(
        "B2 group 1, general loop",
        group_bits_equal_moments(&general),
        "C_1, C_4 bit-equal m1, m4",
    );
    c.holds(
        "B2 group 1, spectral_response",
        group_bits_equal_moments(&direct),
        "C_1, C_4 bit-equal m1, m4",
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- the cantilever

#[test]
fn a4_cantilever_covariances_are_positive_semi_definite() {
    let mut c = Checks::new();
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for k in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let dofs = m.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let (a, b) = (find([1.0, 0.1, 0.08]), find([1.0, 0.0, 0.0]));
    let n = dofs.n_equations();
    let mut p1 = vec![0.0; n];
    let mut p2 = vec![0.0; n];
    p1[dofs.equation(a, 1).unwrap()] = 1.0;
    p2[dofs.equation(b, 2).unwrap()] = 1.0;
    p2[dofs.equation(b, 1).unwrap()] = 0.5;
    let res = modal_analysis(
        &m,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let solver = RandomSolver::new(
        &m,
        ModalBasis::from_result(&res).unwrap(),
        &HarmonicOptions::new(ModalDamping::Ratio(ZETA)).threads(1),
    )
    .unwrap();
    let g = Spectrum::flat(5.0, 2000.0, 1.0).unwrap();
    let g2 = Spectrum::new(vec![5.0, 300.0, 2000.0], vec![1.0, 4.0, 0.5]).unwrap();
    let ex = Excitation::forces(vec![p1, p2], vec![g, g2], Some(vec![vec![1.0, 0.7], vec![0.7, 1.0]])).unwrap();
    let eq = dofs.equation(a, 1).unwrap();
    let r = solver
        .response(&m, &ex, &RandomRequest::new(vec![eq]).stress(true))
        .unwrap();
    let st = r.stress.as_ref().unwrap();
    let np = st.covariance.len();
    println!("A4: {np} points");
    for (idx, name) in [(1, "C_1"), (3, "C_4")] {
        let mut ratio_max: f64 = 0.0;
        let mut finite = true;
        let mut diag_min = f64::INFINITY;
        let mut pairs = 0usize;
        for p in 0..np {
            let cm = cov4(st, p)[idx];
            for i in 0..6 {
                diag_min = diag_min.min(cm[i][i]);
                for j in 0..i {
                    let den = cm[i][i] * cm[j][j];
                    if !cm[i][j].is_finite() || !den.is_finite() {
                        finite = false;
                    } else if den > 0.0 {
                        pairs += 1;
                        ratio_max = ratio_max.max(cm[i][j] * cm[i][j] / den);
                    }
                }
            }
        }
        println!("A4 {name}: {pairs} pairs");
        c.at_most(
            &format!("A4 {name}: max C_ij^2/(C_ii C_jj)"),
            if finite { ratio_max } else { f64::NAN },
            1.0 + 1e-9,
        );
        c.at_least(&format!("A4 {name}: smallest diagonal entry"), diag_min, 0.0);
    }
    let mut r1: f64 = 0.0;
    let mut r2: f64 = 0.0;
    let mut finite = true;
    for p in 0..np {
        for comp in 0..6 {
            let mo = st.moments(p, comp).unwrap();
            if mo.m0 > 0.0 && mo.m2 > 0.0 && mo.m4 > 0.0 {
                let (a1, a2) = (mo.m1 * mo.m1 / (mo.m0 * mo.m2), mo.m2 * mo.m2 / (mo.m0 * mo.m4));
                finite &= a1.is_finite() && a2.is_finite();
                r1 = r1.max(a1);
                r2 = r2.max(a2);
            }
        }
    }
    c.at_most(
        "A4 max m_1^2/(m_0 m_2)",
        if finite { r1 } else { f64::NAN },
        1.0 + 1e-12,
    );
    c.at_most(
        "A4 max m_2^2/(m_0 m_4)",
        if finite { r2 } else { f64::NAN },
        1.0 + 1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ refusals

#[test]
fn v2_refusals() {
    let mut c = Checks::new();
    let om = one_mode(0.3);
    let r = om.force();
    let st = r.stress.as_ref().unwrap();
    let np = st.covariance.len();
    let key = "random stress";
    let (ok, t) = refused(st.moments(np, 0), key, &["point"]);
    c.holds("V2 moments, point out of range", ok, &t);
    let (ok, t) = refused(st.moments(0, 6), key, &["component"]);
    c.holds("V2 moments, component 6", ok, &t);
    let (ok, t) = refused(st.broadband_damage_rate(np, 0, 3.0, 1.0), key, &["point"]);
    c.holds("V2 broadband_damage_rate, point out of range", ok, &t);
    let (ok, t) = refused(st.broadband_damage_rate(0, 9, 3.0, 1.0), key, &["component"]);
    c.holds("V2 broadband_damage_rate, component 9", ok, &t);
    let (ok, t) = refused(st.von_mises_broadband_damage_rate(np, 3.0, 1.0), key, &["point"]);
    c.holds("V2 von Mises, point out of range", ok, &t);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let (ok, t) = refused(
            st.combination_moments(0, &[1.0, 0.0, bad, 0.0, 0.0, 0.0]),
            key,
            &["weight 2"],
        );
        c.holds(&format!("V2 combination weight {bad}"), ok, &t);
    }
    let zero = st.combination_moments(0, &[0.0; 6]).unwrap();
    c.holds(
        "V2 all-zero combination has zero moments",
        moment_array(&zero) == [0.0; 4],
        &format!("{zero:?}"),
    );
    let (ok, t) = refused(zero.broadband_damage_rate(3.0, 1.0), "broadband", &["m0"]);
    c.holds("V2 all-zero combination refused by the broadband rate (m0)", ok, &t);
    c.finish();
}
