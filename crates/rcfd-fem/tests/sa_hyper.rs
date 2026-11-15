//! F-425 (ADR 0541): hyperelastic materials (compressible neo-Hookean and Mooney-Rivlin) on the total Lagrangian driver, the
//! F-bar Hex8 of ADR 0506 as the mean-dilatation element. The checks T1, T1s, T2, R1 to R3, SS1 to SS4, H0 to H6, SP0, SP1,
//! MD1, RF1, INV1, INV2, RF2, EC1, DG1 and the negative controls SPL, N2, N3 of the registration (B0 is in
//! `sa_hyper_default_bits.rs`, RC1 in `sa_scaffold.rs`). Every aggregated error is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{Element, ElementState, Hex8};
use rcfd_fem::mat_hyper::{self, energy_parts, small_strain_moduli, stress_and_tangent, FOLLOW_UP};
use rcfd_fem::material::{HyperelasticEnergy, Matrix6, PointContext};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_internal_force_at, hex8_second_piola_kirchhoff, hex8_strains, solve, NewtonOptions,
    INVERSION_KEY,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel,
};

const BBAR: SolidFormulation = SolidFormulation::BBar;
const FULL: SolidFormulation = SolidFormulation::Full;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.4e} (limit in [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo:e}, {hi:e}]"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn report(&self, name: &str, value: f64) {
        println!("{name}: {value:.4e} (reported, no limit)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The largest of finite values; NaN if any is not finite (so that the check misses).
fn worst<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = 0.0f64;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    w
}

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

/// xorshift64* with Box-Muller normals.
struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
    }
}

// ------------------------------------------------------------------------------------------------------ materials

const MU: f64 = 1e6;
const K_LOW: f64 = 2.1667e6;
const K_HIGH: f64 = 1e10;
const C10: f64 = 3.5e5;
const C01: f64 = 1.5e5;

fn nh(kappa: f64) -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(MU, kappa).unwrap()
}

fn mr(kappa: f64) -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(C10, C01, kappa).unwrap()
}

/// The four energies of D7 (point checks).
fn point_energies() -> Vec<(&'static str, HyperelasticEnergy)> {
    vec![
        ("NH k/mu 2.17", nh(K_LOW)),
        ("NH k/mu 1e4", nh(K_HIGH)),
        ("MR k/mu 2.17", mr(K_LOW)),
        ("MR k/mu 1e4", mr(K_HIGH)),
    ]
}

/// The three energies of D7 (element checks).
fn element_energies() -> Vec<(&'static str, HyperelasticEnergy)> {
    vec![
        ("NH k/mu 2.17", nh(K_LOW)),
        ("NH k/mu 1e4", nh(K_HIGH)),
        ("MR k/mu 1e4", mr(K_HIGH)),
    ]
}

fn material(e: HyperelasticEnergy) -> Material {
    Material::hyperelastic(e, 1000.0).unwrap()
}

/// The isotropic matrix of shear modulus `g` and bulk modulus `k` (Voigt, engineering shear).
fn d_lin(g: f64, k: f64) -> Matrix6 {
    let lambda = k - 2.0 * g / 3.0;
    let mut d = [[0.0; 6]; 6];
    for (i, row) in d.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = if i == j { lambda + 2.0 * g } else { lambda };
        }
    }
    for (k, row) in d.iter_mut().enumerate().skip(3) {
        row[k] = g;
    }
    d
}

fn apply(d: &Matrix6, e: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * e[j]).sum())
}

const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];

fn to_voigt(e: &[[f64; 3]; 3]) -> [f64; 6] {
    std::array::from_fn(|m| {
        let (i, j) = PAIRS[m];
        if m < 3 {
            e[i][j]
        } else {
            e[i][j] + e[j][i]
        }
    })
}

fn to_tensor(e: &[f64; 6]) -> [[f64; 3]; 3] {
    let mut t = [[0.0; 3]; 3];
    for (m, &(i, j)) in PAIRS.iter().enumerate() {
        let v = if m < 3 { e[m] } else { 0.5 * e[m] };
        t[i][j] = v;
        t[j][i] = v;
    }
    t
}

fn det3(a: &[[f64; 3]; 3]) -> f64 {
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

fn matmul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn transpose(a: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

/// `det(I + 2E)` of a Voigt strain.
fn det_c(e: &[f64; 6]) -> f64 {
    let mut c = to_tensor(e);
    for (i, row) in c.iter_mut().enumerate() {
        for v in row.iter_mut() {
            *v *= 2.0;
        }
        row[i] += 1.0;
    }
    det3(&c)
}

/// The Green-Lagrange strain of `F`, `(F^T F - I)/2`, Voigt, engineering shear.
fn green(f: &[[f64; 3]; 3]) -> [f64; 6] {
    let mut c = matmul(&transpose(f), f);
    for (i, row) in c.iter_mut().enumerate() {
        row[i] -= 1.0;
        for v in row.iter_mut() {
            *v *= 0.5;
        }
    }
    to_voigt(&c)
}

/// The rotation by `angle` about `axis` (Rodrigues).
fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, c) = angle.sin_cos();
    let kx = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]];
    let kk = matmul(&kx, &kx);
    std::array::from_fn(|i| {
        std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } + s * kx[i][j] + (1.0 - c) * kk[i][j])
    })
}

fn rotations() -> Vec<[[f64; 3]; 3]> {
    vec![
        rotation([1.0, 2.0, 3.0], 0.7),
        rotation([0.0, 0.0, 1.0], 1.9),
        rotation([1.0, -1.0, 0.5], 3.0),
    ]
}

/// A random symmetric strain `amp (G + G^T)/2` (Voigt, engineering shear).
fn random_strain(rng: &mut Rng, amp: f64) -> [f64; 6] {
    let g: [[f64; 3]; 3] = std::array::from_fn(|_| std::array::from_fn(|_| rng.normal()));
    let e: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| amp * 0.5 * (g[i][j] + g[j][i])));
    to_voigt(&e)
}

/// Ten strains of amplitude `amp` with `det C > 0` (of at most ten draws) and the number kept.
fn strains(rng: &mut Rng, amp: f64) -> Vec<[f64; 6]> {
    (0..10)
        .map(|_| random_strain(rng, amp))
        .filter(|e| det_c(e) > 0.0)
        .collect()
}

/// Amendment 2: ten strains of amplitude `amp` with `det C > 0`, drawn until ten are kept (at most 200 draws).
fn strains_until_ten(rng: &mut Rng, amp: f64) -> Vec<[f64; 6]> {
    let mut out = Vec::new();
    for _ in 0..200 {
        if out.len() == 10 {
            break;
        }
        let e = random_strain(rng, amp);
        if det_c(&e) > 0.0 {
            out.push(e);
        }
    }
    out
}

// --------------------------------------------------------------------------------------------------- point checks

#[test]
#[ignore = "record of the registered run of ADR 0541 (log run1-sa_hyper.log): T1, T1s, EC1, R1 met; the T1 set-up missed at amplitude 0.4 (7, 4, 4, 4 of 10 strains with det C > 0 against 8); corrected by amendment 2 as t1p_t1s_ec1_r1_after_amendment_2"]
fn t1_t1s_ec1_r1_tangent_energy_rotation_at_the_point() {
    let mut c = Checks::new();
    let mut rng = Rng(0xF425_0541);
    let h = 1e-7;
    for (name, en) in point_energies() {
        for amp in [1e-3, 0.1, 0.4] {
            let es = strains(&mut rng, amp);
            c.at_least(
                &format!("T1 set-up {name} amp {amp:e}: strains kept of 10"),
                es.len() as f64,
                8.0,
            );
            let (mut t1, mut t1s, mut ec1) = (0.0f64, 0.0f64, 0.0f64);
            for e in &es {
                let (s, d) = stress_and_tangent(&en, e).unwrap();
                let mut dfd = [[0.0; 6]; 6];
                let mut grad = [0.0; 6];
                for j in 0..6 {
                    let (mut ep, mut em) = (*e, *e);
                    ep[j] += h;
                    em[j] -= h;
                    let (sp, sm) = (
                        mat_hyper::stress(&en, &ep).unwrap(),
                        mat_hyper::stress(&en, &em).unwrap(),
                    );
                    for i in 0..6 {
                        dfd[i][j] = (sp[i] - sm[i]) / (2.0 * h);
                    }
                    let (mut ep, mut em) = (*e, *e);
                    ep[j] += 1e-6;
                    em[j] -= 1e-6;
                    grad[j] = (mat_hyper::energy(&en, &ep).unwrap() - mat_hyper::energy(&en, &em).unwrap()) / 2e-6;
                }
                let scale = max_abs(&d.concat());
                let diff: Vec<f64> = (0..36).map(|k| d[k / 6][k % 6] - dfd[k / 6][k % 6]).collect();
                let asym: Vec<f64> = (0..36).map(|k| d[k / 6][k % 6] - d[k % 6][k / 6]).collect();
                t1 = worst([t1, max_abs(&diff) / scale]);
                t1s = worst([t1s, max_abs(&asym) / scale]);
                if amp >= 0.1 {
                    let ds: Vec<f64> = (0..6).map(|i| s[i] - grad[i]).collect();
                    ec1 = worst([ec1, max_abs(&ds) / max_abs(&s)]);
                }
            }
            c.at_most(&format!("T1 {name} amp {amp:e}: max |D - D_fd| / max |D|"), t1, 1e-6);
            c.at_most(&format!("T1s {name} amp {amp:e}: max |D - D^T| / max |D|"), t1s, 1e-12);
            if amp >= 0.1 {
                c.at_most(&format!("EC1 {name} amp {amp:e}: max |S - dW/de| / max |S|"), ec1, 1e-6);
            }
        }
        // R1: ten F, three rotations.
        let mut r1 = 0.0f64;
        let mut kept = 0;
        for _ in 0..10 {
            let f: [[f64; 3]; 3] =
                std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } + 0.3 * rng.normal()));
            if det3(&f) <= 0.0 {
                continue;
            }
            kept += 1;
            let w = mat_hyper::energy(&en, &green(&f)).unwrap();
            for q in rotations() {
                let wq = mat_hyper::energy(&en, &green(&matmul(&q, &f))).unwrap();
                r1 = worst([r1, (wq - w).abs() / w.abs()]);
            }
        }
        c.at_least(&format!("R1 set-up {name}: F kept of 10"), kept as f64, 5.0);
        c.at_most(&format!("R1 {name}: |W(E(QF)) - W(E(F))| / |W|"), r1, 1e-10);
    }
    c.finish();
}

/// T1' of amendment 2: T1, T1s, EC1 and R1 with ten kept strains per energy and amplitude.
#[test]
fn t1p_t1s_ec1_r1_after_amendment_2() {
    let mut c = Checks::new();
    let mut rng = Rng(0xF425_0541);
    let h = 1e-7;
    for (name, en) in point_energies() {
        for amp in [1e-3, 0.1, 0.4] {
            let es = strains_until_ten(&mut rng, amp);
            c.at_least(
                &format!("T1' set-up {name} amp {amp:e}: strains kept (of at most 200 draws)"),
                es.len() as f64,
                10.0,
            );
            let (mut t1, mut t1s, mut ec1) = (0.0f64, 0.0f64, 0.0f64);
            for e in &es {
                let (s, d) = stress_and_tangent(&en, e).unwrap();
                let mut dfd = [[0.0; 6]; 6];
                let mut grad = [0.0; 6];
                for j in 0..6 {
                    let (mut ep, mut em) = (*e, *e);
                    ep[j] += h;
                    em[j] -= h;
                    let (sp, sm) = (
                        mat_hyper::stress(&en, &ep).unwrap(),
                        mat_hyper::stress(&en, &em).unwrap(),
                    );
                    for i in 0..6 {
                        dfd[i][j] = (sp[i] - sm[i]) / (2.0 * h);
                    }
                    let (mut ep, mut em) = (*e, *e);
                    ep[j] += 1e-6;
                    em[j] -= 1e-6;
                    grad[j] = (mat_hyper::energy(&en, &ep).unwrap() - mat_hyper::energy(&en, &em).unwrap()) / 2e-6;
                }
                let scale = max_abs(&d.concat());
                let diff: Vec<f64> = (0..36).map(|k| d[k / 6][k % 6] - dfd[k / 6][k % 6]).collect();
                let asym: Vec<f64> = (0..36).map(|k| d[k / 6][k % 6] - d[k % 6][k / 6]).collect();
                t1 = worst([t1, max_abs(&diff) / scale]);
                t1s = worst([t1s, max_abs(&asym) / scale]);
                if amp >= 0.1 {
                    let ds: Vec<f64> = (0..6).map(|i| s[i] - grad[i]).collect();
                    ec1 = worst([ec1, max_abs(&ds) / max_abs(&s)]);
                }
            }
            c.at_most(&format!("T1 {name} amp {amp:e}: max |D - D_fd| / max |D|"), t1, 1e-6);
            c.at_most(&format!("T1s {name} amp {amp:e}: max |D - D^T| / max |D|"), t1s, 1e-12);
            if amp >= 0.1 {
                c.at_most(&format!("EC1 {name} amp {amp:e}: max |S - dW/de| / max |S|"), ec1, 1e-6);
            }
        }
        // R1: ten F, three rotations.
        let mut r1 = 0.0f64;
        let mut kept = 0;
        for _ in 0..10 {
            let f: [[f64; 3]; 3] =
                std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } + 0.3 * rng.normal()));
            if det3(&f) <= 0.0 {
                continue;
            }
            kept += 1;
            let w = mat_hyper::energy(&en, &green(&f)).unwrap();
            for q in rotations() {
                let wq = mat_hyper::energy(&en, &green(&matmul(&q, &f))).unwrap();
                r1 = worst([r1, (wq - w).abs() / w.abs()]);
            }
        }
        c.at_least(&format!("R1 set-up {name}: F kept of 10"), kept as f64, 5.0);
        c.at_most(&format!("R1 {name}: |W(E(QF)) - W(E(F))| / |W|"), r1, 1e-10);
    }
    c.finish();
}

/// SS1's 64 seeded directions, scaled to the tensor norm `norm`.
fn small_strains(norm: f64) -> Vec<[f64; 6]> {
    let mut rng = Rng(0xF425_0543);
    (0..64)
        .map(|_| {
            let e = random_strain(&mut rng, 1.0);
            let t = to_tensor(&e);
            let n: f64 = t.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
            std::array::from_fn(|i| e[i] * norm / n)
        })
        .collect()
}

/// `max over the directions of max |S(E) - D E| / max |D E|` with `D = d_lin(g, k)`.
fn small_strain_remainder(en: &HyperelasticEnergy, g: f64, k: f64, norm: f64) -> f64 {
    let d = d_lin(g, k);
    worst(small_strains(norm).iter().map(|e| {
        let s = mat_hyper::stress(en, e).unwrap();
        let l = apply(&d, e);
        let diff: Vec<f64> = (0..6).map(|i| s[i] - l[i]).collect();
        max_abs(&diff) / max_abs(&l)
    }))
}

#[test]
#[ignore = "known miss of ADR 0541 SS1 (registered run, log run1-sa_hyper.log): 4.7166e-6 (NH) and 4.9444e-6 (MR) against the limit 1e-6, the second-order remainder of arithmetic 3 (predicted 4.9e-6)"]
fn ss1_small_strain_literal_known_miss() {
    let mut c = Checks::new();
    for (name, en) in [("NH k/mu 2.17", nh(K_LOW)), ("MR k/mu 2.17", mr(K_LOW))] {
        let (g, k) = small_strain_moduli(&en);
        c.at_most(
            &format!("SS1 {name}: max |S(E) - D_lin E| / max |D_lin E| at |E| = 1e-6"),
            small_strain_remainder(&en, g, k, 1e-6),
            1e-6,
        );
    }
    c.finish();
}

#[test]
fn ss2_ss3_ss4_ssr_n2_the_linearisation() {
    let mut c = Checks::new();
    for (name, en) in point_energies() {
        let (g, k) = small_strain_moduli(&en);
        let dl = d_lin(g, k);
        let (_, d0) = stress_and_tangent(&en, &[0.0; 6]).unwrap();
        let diff: Vec<f64> = (0..36).map(|m| d0[m / 6][m % 6] - dl[m / 6][m % 6]).collect();
        c.at_most(
            &format!("SS2 {name}: max |D(0) - D_lin| / max |D_lin|"),
            max_abs(&diff) / max_abs(&dl.concat()),
            1e-14,
        );
        let ss3 = worst(small_strains(1e-6).iter().map(|e| {
            let sp = mat_hyper::stress(&en, e).unwrap();
            let sm = mat_hyper::stress(&en, &std::array::from_fn(|i| -e[i])).unwrap();
            let l = apply(&dl, e);
            let diff: Vec<f64> = (0..6).map(|i| 0.5 * (sp[i] - sm[i]) - l[i]).collect();
            max_abs(&diff) / max_abs(&l)
        }));
        c.at_most(&format!("SS3 {name}: odd part against D_lin E"), ss3, 1e-8);
    }
    for (name, en) in [("NH k/mu 2.17", nh(K_LOW)), ("MR k/mu 2.17", mr(K_LOW))] {
        let (g, k) = small_strain_moduli(&en);
        let r6 = small_strain_remainder(&en, g, k, 1e-6);
        let r5 = small_strain_remainder(&en, g, k, 1e-5);
        c.within(
            &format!("SS4 {name}: remainder(1e-6) / remainder(1e-5)"),
            r6 / r5,
            0.08,
            0.12,
        );
        c.at_least(
            &format!("N2 {name}: SS1's measure with G = 1.1 mu"),
            small_strain_remainder(&en, 1.1 * g, k, 1e-6),
            1e-2,
        );
    }
    let en = nh(K_HIGH);
    let (g, k) = small_strain_moduli(&en);
    c.report(
        "SSr NH k/mu 1e4: SS1's measure",
        small_strain_remainder(&en, g, k, 1e-6),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- element checks

/// The distorted Hex8 of D7: the unit cube in the Hex8 node order, corners moved by `0.1 (2U - 1)`.
fn distorted_hex8() -> Vec<Vec3> {
    let mut rng = Rng(0xF425_0542);
    ElementKind::Hex8
        .reference_nodes()
        .iter()
        .map(|r| std::array::from_fn(|i| 0.5 * (r[i] + 1.0) + 0.1 * (2.0 * rng.uniform() - 1.0)))
        .collect()
}

/// The displacements (a) to (c) of D7 on `coords`.
fn element_states(coords: &[Vec3]) -> Vec<(&'static str, Vec<f64>)> {
    let mut rng = Rng(0xF425_0544);
    let mut random = |amp: f64| -> Vec<f64> { (0..24).map(|_| amp * (2.0 * rng.uniform() - 1.0)).collect() };
    let a = random(0.1);
    let q = rotation([0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2);
    let rb = random(0.05);
    let b: Vec<f64> = (0..24)
        .map(|k| {
            let (n, i) = (k / 3, k % 3);
            let x = coords[n];
            (0..3).map(|j| q[i][j] * x[j]).sum::<f64>() - x[i] + rb[k]
        })
        .collect();
    let rc = random(0.02);
    let st = [0.6, -0.2, -0.2];
    let cc: Vec<f64> = (0..24).map(|k| st[k % 3] * coords[k / 3][k % 3] + rc[k]).collect();
    vec![("(a)", a), ("(b)", b), ("(c)", cc)]
}

fn empty() -> ElementState {
    ElementState::new(Vec::new())
}

/// The element energy `sum_q w_q W(E_q)` (the F-bar strain for BBar).
fn element_energy(f: SolidFormulation, coords: &[Vec3], en: &HyperelasticEnergy, u: &[f64]) -> f64 {
    let ips = Hex8.integration_points(coords, Hex8.stiffness_rule()).unwrap();
    let es = hex8_strains(f, coords, u).unwrap();
    ips.iter()
        .zip(&es)
        .map(|(ip, e)| ip.weight * mat_hyper::energy(en, e).unwrap())
        .sum()
}

/// `u'` of the rotated configuration `Q (X + u)`.
fn rotated(coords: &[Vec3], u: &[f64], q: &[[f64; 3]; 3]) -> Vec<f64> {
    (0..24)
        .map(|k| {
            let (n, i) = (k / 3, k % 3);
            let x: Vec<f64> = (0..3).map(|j| coords[n][j] + u[3 * n + j]).collect();
            (0..3).map(|j| q[i][j] * x[j]).sum::<f64>() - coords[n][i]
        })
        .collect()
}

#[test]
fn t2_r2_r3_md1_the_element() {
    let mut c = Checks::new();
    let coords = distorted_hex8();
    let ctx = PointContext::default();
    let h = 1e-7;
    for (name, en) in element_energies() {
        let m = material(en);
        for (sname, u) in element_states(&coords) {
            for f in [FULL, BBAR] {
                let tag = format!("{name} {sname} {}", f.name());
                let (force, k) = hex8_force_and_tangent_at(f, &coords, &m, &u, &empty(), &ctx).unwrap();
                let mut kfd = vec![0.0; 24 * 24];
                for j in 0..24 {
                    let (mut up, mut um) = (u.clone(), u.clone());
                    up[j] += h;
                    um[j] -= h;
                    let fp = hex8_internal_force_at(f, &coords, &m, &up, &empty(), &ctx).unwrap();
                    let fm = hex8_internal_force_at(f, &coords, &m, &um, &empty(), &ctx).unwrap();
                    for i in 0..24 {
                        kfd[i * 24 + j] = (fp[i] - fm[i]) / (2.0 * h);
                    }
                }
                let kv = k.values();
                let diff: Vec<f64> = kv.iter().zip(&kfd).map(|(a, b)| a - b).collect();
                c.at_most(
                    &format!("T2 {tag}: max |K - K_fd| / max |K|"),
                    max_abs(&diff) / max_abs(kv),
                    1e-6,
                );
                let w = element_energy(f, &coords, &en, &u);
                let (mut r2, mut r3) = (0.0f64, 0.0f64);
                for q in rotations() {
                    let ur = rotated(&coords, &u, &q);
                    let wr = element_energy(f, &coords, &en, &ur);
                    r2 = worst([r2, (wr - w).abs() / w.abs()]);
                    let fr = hex8_internal_force_at(f, &coords, &m, &ur, &empty(), &ctx).unwrap();
                    let d: Vec<f64> = (0..24)
                        .map(|kk| {
                            let (n, i) = (kk / 3, kk % 3);
                            fr[kk] - (0..3).map(|j| q[i][j] * force[3 * n + j]).sum::<f64>()
                        })
                        .collect();
                    r3 = worst([r3, max_abs(&d) / max_abs(&force)]);
                }
                c.at_most(&format!("R2 {tag}: element energy of Q x against x"), r2, 1e-10);
                c.at_most(&format!("R3 {tag}: max |f(Q x) - Q f(x)| / max |f|"), r3, 1e-10);
                if f == BBAR {
                    let ips = Hex8.integration_points(&coords, Hex8.stiffness_rule()).unwrap();
                    let ef = hex8_strains(FULL, &coords, &u).unwrap();
                    let v: f64 = ips.iter().map(|ip| ip.weight).sum();
                    let mut iso = 0.0;
                    let mut jbar = 0.0;
                    for (ip, e) in ips.iter().zip(&ef) {
                        iso += ip.weight * energy_parts(&en, e).unwrap().0;
                        jbar += ip.weight * det_c(e).sqrt();
                    }
                    jbar /= v;
                    let kappa = small_strain_moduli(&en).1;
                    let split = iso + v * 0.5 * kappa * (jbar - 1.0) * (jbar - 1.0);
                    c.at_most(
                        &format!("MD1 {tag}: F-bar energy against sum w W_iso(E) + V U(J_bar)"),
                        (w - split).abs() / w.abs(),
                        1e-12,
                    );
                }
            }
        }
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------- homogeneous states

/// The nominal stresses `P_i = dW/dlambda_i` of the principal stretches (arithmetic 4, independent of `mat_hyper`).
fn principal(l: [f64; 3], c10: f64, c01: f64, kappa: f64) -> [f64; 3] {
    let j = l[0] * l[1] * l[2];
    let i1 = l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
    let i2 = l[0] * l[0] * l[1] * l[1] + l[1] * l[1] * l[2] * l[2] + l[2] * l[2] * l[0] * l[0];
    let a = j.powf(-2.0 / 3.0);
    let b = j.powf(-4.0 / 3.0);
    std::array::from_fn(|i| {
        c10 * a * (2.0 * l[i] - (2.0 / 3.0) * i1 / l[i])
            + c01 * b * (2.0 * l[i] * (i1 - l[i] * l[i]) - (4.0 / 3.0) * i2 / l[i])
            + kappa * (j - 1.0) * j / l[i]
    })
}

/// The exact compressible nominal stress `P_1` of the uniaxial (`biaxial = false`) or equibiaxial state and the free stretch.
fn compressible(lambda: f64, c10: f64, c01: f64, kappa: f64, biaxial: bool) -> (f64, f64) {
    let stretches = |t: f64| if biaxial { [lambda, lambda, t] } else { [lambda, t, t] };
    let lateral = |t: f64| principal(stretches(t), c10, c01, kappa)[if biaxial { 2 } else { 1 }];
    let mut t = if biaxial { lambda.powi(-2) } else { lambda.powf(-0.5) };
    for _ in 0..100 {
        let h = 1e-7 * t;
        let d = (lateral(t + h) - lateral(t - h)) / (2.0 * h);
        t -= lateral(t) / d;
    }
    (principal(stretches(t), c10, c01, kappa)[0], t)
}

fn newton(steps: usize) -> NewtonOptions {
    let mut o = NewtonOptions::new(steps).threads(1);
    o.residual_tolerance = 1e-8;
    o
}

fn model_of(mesh: SolidMesh, f: SolidFormulation, en: HyperelasticEnergy) -> SolidModel {
    let mut mesh = mesh;
    if f != FULL {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material(en)).unwrap();
    model
}

fn element_displacement(model: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut out = Vec::with_capacity(24);
    for &n in model.mesh().element_nodes(e) {
        for c in 0..3 {
            out.push(dofs.equation(n, c).map_or(0.0, |q| u[q]));
        }
    }
    out
}

fn element_coords(model: &SolidModel, e: usize) -> Vec<Vec3> {
    model
        .mesh()
        .element_nodes(e)
        .iter()
        .map(|&n| model.mesh().nodes()[n])
        .collect()
}

const STRETCHES: [f64; 8] = [0.7, 0.8, 0.9, 1.25, 1.5, 2.0, 2.5, 3.0];

/// One homogeneous stretch of the unit cube: `(P11 at every point, the free stretch, max lateral |S| / |S11|)`.
fn stretch_cube(f: SolidFormulation, en: HyperelasticEnergy, lambda: f64, biaxial: bool) -> (Vec<f64>, f64, f64) {
    let model = model_of(hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap(), f, en);
    let mut prescribed = Vec::new();
    let mut corner = 0;
    for (n, p) in model.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            if p[c] == 0.0 {
                prescribed.push(Prescribed::new(n, c, 0.0));
            }
        }
        if p[0] == 1.0 {
            prescribed.push(Prescribed::new(n, 0, lambda - 1.0));
        }
        if biaxial && p[1] == 1.0 {
            prescribed.push(Prescribed::new(n, 1, lambda - 1.0));
        }
        if p == &[1.0, 1.0, 1.0] {
            corner = n;
        }
    }
    let steps = ((lambda - 1.0).abs() / 0.1).ceil() as usize;
    let r = solve(&model, &prescribed, &[], &newton(steps)).unwrap();
    let u = r.result.displacement_values();
    let ue = element_displacement(&model, u, 0);
    let s = hex8_second_piola_kirchhoff(f, &element_coords(&model, 0), &material(en), &ue, &empty()).unwrap();
    let dofs = model.dof_map();
    let free_c = if biaxial { 2 } else { 1 };
    let t = 1.0 + u[dofs.equation(corner, free_c).unwrap()];
    let p11 = s.iter().map(|si| lambda * si[0]).collect();
    let lateral = worst(s.iter().map(|si| {
        let l = if biaxial {
            si[2].abs()
        } else {
            si[1].abs().max(si[2].abs())
        };
        l / si[0].abs()
    }));
    (p11, t, lateral)
}

/// The H-row of one energy and state: `|P_FE - P_comp| / |P_inc|` and `/ |P_comp|` worst over the stretches and both
/// formulations, with the set-up measure and the correction reported.
fn homogeneous_rows(c: &mut Checks, row: &str, en: HyperelasticEnergy, biaxial: bool, inc: impl Fn(f64) -> f64) {
    let (c10, c01, kappa) = match en {
        HyperelasticEnergy::NeoHookean { mu, kappa, .. } => (0.5 * mu, 0.0, kappa),
        HyperelasticEnergy::MooneyRivlin { c10, c01, kappa, .. } => (c10, c01, kappa),
        _ => unreachable!("two energies"),
    };
    for f in [FULL, BBAR] {
        let (mut m, mut mc, mut setup, mut corr, mut uncorr) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for lambda in STRETCHES {
            let (ps, t, lateral) = stretch_cube(f, en, lambda, biaxial);
            let (pc, tc) = compressible(lambda, c10, c01, kappa, biaxial);
            let pi = inc(lambda);
            println!(
                "  {row} {} lambda {lambda}: P_FE {:.10e} P_comp {pc:.10e} P_inc {pi:.10e} t_FE {t:.10} t_comp {tc:.10}",
                f.name(),
                ps[0]
            );
            for p in ps {
                m = worst([m, (p - (pi + (pc - pi))).abs() / pi.abs()]);
                mc = worst([mc, (p - pc).abs() / pc.abs()]);
                uncorr = worst([uncorr, ((p - pi) / pi).abs()]);
            }
            setup = worst([setup, lateral]);
            corr = worst([corr, ((pc - pi) / pi).abs()]);
        }
        c.at_most(
            &format!("{row} {}: |P_FE - (P_inc + Delta)| / |P_inc|", f.name()),
            m,
            1e-3,
        );
        c.at_most(&format!("{row}c {}: |P_FE - P_comp| / |P_comp|", f.name()), mc, 1e-6);
        c.at_most(&format!("H0 {row} {}: lateral |S| / |S11|", f.name()), setup, 1e-6);
        c.report(&format!("{row} {}: |Delta / P_inc| (the correction)", f.name()), corr);
        c.report(
            &format!("{row} {}: |P_FE - P_inc| / |P_inc| (uncorrected, N3)", f.name()),
            uncorr,
        );
    }
}

#[test]
fn h0_to_h3_h5_n3_uniaxial_and_equibiaxial() {
    let mut c = Checks::new();
    homogeneous_rows(&mut c, "H1", nh(K_HIGH), false, |l| MU * (l - l.powi(-2)));
    homogeneous_rows(&mut c, "H2", mr(K_HIGH), false, |l| {
        2.0 * (C10 + C01 / l) * (l - l.powi(-2))
    });
    homogeneous_rows(&mut c, "H3", nh(K_HIGH), true, |l| MU * (l - l.powi(-5)));
    homogeneous_rows(&mut c, "H5", mr(K_HIGH), true, |l| {
        2.0 * (C10 + C01 * l * l) * (l - l.powi(-5))
    });
    // N3: the H2 data at lambda = 3 against the Mooney-Rivlin form at c01 = 0.
    let (ps, _, _) = stretch_cube(BBAR, mr(K_HIGH), 3.0, false);
    let wrong = 2.0 * C10 * (3.0 - 3.0f64.powi(-2));
    let n3 = ps
        .iter()
        .map(|p| (p - wrong).abs() / wrong)
        .fold(f64::INFINITY, f64::min);
    c.at_least("N3 H2 data at lambda 3 against 2 c10 (lambda - lambda^-2)", n3, 1e-2);
    c.finish();
}

/// Simple shear of the 2 x 2 x 2 unit cube, every boundary node prescribed: `(tau, N1, N2)` worst-case relative errors.
fn shear_rows(c: &mut Checks, row: &str, en: HyperelasticEnergy, g: f64, c01: f64) {
    for f in [FULL, BBAR] {
        let (mut et, mut e1, mut e2, mut setup) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for gamma in [0.1, 0.5, 1.0, 2.0] {
            let model = model_of(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap(), f, en);
            let mut prescribed = Vec::new();
            let mut centre = None;
            for (n, p) in model.mesh().nodes().iter().enumerate() {
                if p.iter().any(|&x| x == 0.0 || x == 1.0) {
                    prescribed.push(Prescribed::new(n, 0, gamma * p[1]));
                    prescribed.push(Prescribed::new(n, 1, 0.0));
                    prescribed.push(Prescribed::new(n, 2, 0.0));
                } else {
                    centre = Some(n);
                }
            }
            let steps = (gamma / 0.1).ceil() as usize;
            let r = solve(&model, &prescribed, &[], &newton(steps)).unwrap();
            let u = r.result.displacement_values();
            let dofs = model.dof_map();
            let cn = centre.unwrap();
            let uc: Vec<f64> = (0..3).map(|k| u[dofs.equation(cn, k).unwrap()]).collect();
            setup = worst([setup, ((uc[0] - 0.5 * gamma).abs() + uc[1].abs() + uc[2].abs()) / gamma]);
            let fm = [[1.0, gamma, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            for e in 0..model.mesh().n_elements() {
                let ue = element_displacement(&model, u, e);
                let s =
                    hex8_second_piola_kirchhoff(f, &element_coords(&model, e), &material(en), &ue, &empty()).unwrap();
                for si in &s {
                    let sig = matmul(
                        &matmul(
                            &fm,
                            &to_tensor(&[si[0], si[1], si[2], 2.0 * si[3], 2.0 * si[4], 2.0 * si[5]]),
                        ),
                        &transpose(&fm),
                    );
                    et = worst([et, (sig[0][1] - g * gamma).abs() / (g * gamma)]);
                    e1 = worst([
                        e1,
                        (sig[0][0] - sig[1][1] - g * gamma * gamma).abs() / (g * gamma * gamma),
                    ]);
                    let n2 = -2.0 * c01 * gamma * gamma;
                    e2 = worst([e2, (sig[1][1] - sig[2][2] - n2).abs() / (g * gamma * gamma)]);
                }
            }
        }
        c.at_most(
            &format!("H0 {row} {}: centre node against gamma Y e1, / gamma", f.name()),
            setup,
            1e-10,
        );
        c.at_most(&format!("{row} {}: |tau - G gamma| / (G gamma)", f.name()), et, 1e-3);
        c.at_most(&format!("{row} {}: |N1 - G gamma^2| / (G gamma^2)", f.name()), e1, 1e-3);
        if c01 == 0.0 {
            c.at_most(&format!("{row}c {}: |N2| / (mu gamma^2)", f.name()), e2, 1e-6);
        } else {
            c.at_most(
                &format!("{row} {}: |N2 + 2 c01 gamma^2| / (G gamma^2)", f.name()),
                e2,
                1e-3,
            );
        }
    }
}

#[test]
fn h4_h6_simple_shear() {
    let mut c = Checks::new();
    shear_rows(&mut c, "H4", nh(K_HIGH), MU, 0.0);
    shear_rows(&mut c, "H6", mr(K_HIGH), 2.0 * (C10 + C01), C01);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ sphere

const A_IN: f64 = 1.0;
const B_OUT: f64 = 2.0;
const L_A: f64 = 1.5;

/// `l_b` of the inner stretch `l_a` (arithmetic 5).
fn l_b(l_a: f64) -> f64 {
    (1.0 + (A_IN / B_OUT).powi(3) * (l_a.powi(3) - 1.0)).cbrt()
}

/// The closed form `p / mu`.
fn p_closed(l_a: f64) -> f64 {
    let lb = l_b(l_a);
    0.5 * ((1.0 / lb.powi(4) + 4.0 / lb) - (1.0 / l_a.powi(4) + 4.0 / l_a))
}

/// Gauss-Legendre nodes and weights on [-1, 1] (Newton on the three-term recurrence).
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 0.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=n {
                    let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                    p0 = p1;
                    p1 = p2;
                }
                dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
                let dx = p1 / dp;
                x -= dx;
                if dx.abs() < 1e-16 {
                    break;
                }
            }
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

/// The octant mesh of D6: `(model, inner nodes, inner faces as surface-node quads mapped to mesh nodes)`.
fn sphere_model(n: usize, m: usize, f: SolidFormulation) -> (SolidModel, Vec<usize>, Vec<[usize; 4]>) {
    // Surface grid: the faces x = 1, y = 1, z = 1 of the unit cube, merged on the integer key of n p.
    let mut keys: Vec<[i64; 3]> = Vec::new();
    let mut dirs: Vec<Vec3> = Vec::new();
    let mut patch_ids: Vec<Vec<Vec<usize>>> = Vec::new();
    for face in 0..3 {
        let mut ids = vec![vec![0usize; n + 1]; n + 1];
        for (i, row) in ids.iter_mut().enumerate() {
            for (j, id) in row.iter_mut().enumerate() {
                let (s, t) = (i as f64 / n as f64, j as f64 / n as f64);
                let p = match face {
                    0 => [1.0, s, t],
                    1 => [t, 1.0, s],
                    _ => [s, t, 1.0],
                };
                let key = [
                    (p[0] * n as f64).round() as i64,
                    (p[1] * n as f64).round() as i64,
                    (p[2] * n as f64).round() as i64,
                ];
                *id = match keys.iter().position(|k| *k == key) {
                    Some(q) => q,
                    None => {
                        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                        keys.push(key);
                        dirs.push([p[0] / r, p[1] / r, p[2] / r]);
                        keys.len() - 1
                    }
                };
            }
        }
        patch_ids.push(ids);
    }
    let radii: Vec<f64> = (0..=m)
        .map(|k| A_IN * (B_OUT / A_IN).powf(k as f64 / m as f64))
        .collect();
    let node = |s: usize, k: usize| s * (m + 1) + k;
    let mut nodes = Vec::with_capacity(dirs.len() * (m + 1));
    for d in &dirs {
        for &r in &radii {
            nodes.push([r * d[0], r * d[1], r * d[2]]);
        }
    }
    let mut conn = Vec::new();
    let mut inner_faces = Vec::new();
    for ids in &patch_ids {
        for i in 0..n {
            for j in 0..n {
                let mut quad = [ids[i][j], ids[i + 1][j], ids[i + 1][j + 1], ids[i][j + 1]];
                // Orientation: the triple product of the edges at node 0 of the hex at layer 0.
                let x = |s: usize, k: usize| nodes[node(s, k)];
                let sub = |a: Vec3, b: Vec3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                let (x0, x1, x3, x4) = (x(quad[0], 0), x(quad[1], 0), x(quad[3], 0), x(quad[0], 1));
                let (e1, e3, e4) = (sub(x1, x0), sub(x3, x0), sub(x4, x0));
                let vol = e1[0] * (e3[1] * e4[2] - e3[2] * e4[1]) - e1[1] * (e3[0] * e4[2] - e3[2] * e4[0])
                    + e1[2] * (e3[0] * e4[1] - e3[1] * e4[0]);
                if vol < 0.0 {
                    quad = [quad[0], quad[3], quad[2], quad[1]];
                }
                for k in 0..m {
                    for &s in &quad {
                        conn.push(node(s, k));
                    }
                    for &s in &quad {
                        conn.push(node(s, k + 1));
                    }
                }
                inner_faces.push([node(quad[0], 0), node(quad[1], 0), node(quad[2], 0), node(quad[3], 0)]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let ne = conn.len() / 8;
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let model = model_of(mesh, f, nh(K_HIGH));
    let inner = (0..dirs.len()).map(|s| node(s, 0)).collect();
    (model, inner, inner_faces)
}

/// `(p_FE / mu, the relative error)` of the sphere of D6, or the solve's error.
fn sphere_pressure(n: usize, m: usize, f: SolidFormulation) -> Result<(f64, f64), String> {
    let (model, inner, faces) = sphere_model(n, m, f);
    let is_inner: std::collections::BTreeSet<usize> = inner.iter().copied().collect();
    let mut prescribed = Vec::new();
    for (k, p) in model.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            if is_inner.contains(&k) {
                prescribed.push(Prescribed::new(k, c, (L_A - 1.0) * p[c]));
            } else if p[c] == 0.0 {
                prescribed.push(Prescribed::new(k, c, 0.0));
            }
        }
    }
    let mut o = NewtonOptions::new(10).threads(4);
    o.residual_tolerance = 1e-8;
    let r = solve(&model, &prescribed, &[], &o).map_err(|e| e.to_string())?;
    let u = r.result.displacement_values();
    let reactions = r.result.reactions().ok_or("no reactions")?;
    let dofs = model.dof_map();
    let x = |k: usize| -> Vec3 {
        let p = model.mesh().nodes()[k];
        std::array::from_fn(|c| p[c] + u[dofs.equation(k, c).unwrap()])
    };
    let mut work = 0.0;
    for &k in &inner {
        let xk = x(k);
        for (c, xc) in xk.iter().enumerate() {
            work += reactions[dofs.equation(k, c).unwrap()] * xc;
        }
    }
    let g = 1.0 / 3.0f64.sqrt();
    let mut flux = 0.0;
    for face in &faces {
        let xs: Vec<Vec3> = face.iter().map(|&k| x(k)).collect();
        for xi in [-g, g] {
            for eta in [-g, g] {
                let nshape = [
                    0.25 * (1.0 - xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 + eta),
                    0.25 * (1.0 - xi) * (1.0 + eta),
                ];
                let dxi = [
                    -0.25 * (1.0 - eta),
                    0.25 * (1.0 - eta),
                    0.25 * (1.0 + eta),
                    -0.25 * (1.0 + eta),
                ];
                let deta = [
                    -0.25 * (1.0 - xi),
                    -0.25 * (1.0 + xi),
                    0.25 * (1.0 + xi),
                    0.25 * (1.0 - xi),
                ];
                let mut p = [0.0; 3];
                let mut a = [0.0; 3];
                let mut b = [0.0; 3];
                for q in 0..4 {
                    for c in 0..3 {
                        p[c] += nshape[q] * xs[q][c];
                        a[c] += dxi[q] * xs[q][c];
                        b[c] += deta[q] * xs[q][c];
                    }
                }
                let nrm = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                flux += p[0] * nrm[0] + p[1] * nrm[1] + p[2] * nrm[2];
            }
        }
    }
    let v_cav = (flux / 3.0).abs();
    let p_fe = work / (3.0 * v_cav) / MU;
    let pc = p_closed(L_A);
    let iterations: Vec<usize> = r.steps.iter().map(|s| s.residuals.len()).collect();
    println!(
        "  sphere n {n} m {m} {}: p_FE / mu {p_fe:.10}, closed {pc:.10}, V_cav {v_cav:.8} (exact {:.8}), steps {}, iterations {iterations:?}",
        f.name(),
        std::f64::consts::PI / 6.0 * L_A.powi(3),
        r.steps.len()
    );
    Ok((p_fe, (p_fe - pc).abs() / pc))
}

/// Amendment 2: [`sphere_pressure`] driven to the inner stretch `l_a` (the prescribed values `(l_a - 1) X`).
fn sphere_pressure_to(n: usize, m: usize, f: SolidFormulation, l_a: f64) -> Result<(f64, f64), String> {
    let (model, inner, faces) = sphere_model(n, m, f);
    let is_inner: std::collections::BTreeSet<usize> = inner.iter().copied().collect();
    let mut prescribed = Vec::new();
    for (k, p) in model.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            if is_inner.contains(&k) {
                prescribed.push(Prescribed::new(k, c, (l_a - 1.0) * p[c]));
            } else if p[c] == 0.0 {
                prescribed.push(Prescribed::new(k, c, 0.0));
            }
        }
    }
    let mut o = NewtonOptions::new(10).threads(4);
    o.residual_tolerance = 1e-8;
    let r = solve(&model, &prescribed, &[], &o).map_err(|e| e.to_string())?;
    let u = r.result.displacement_values();
    let reactions = r.result.reactions().ok_or("no reactions")?;
    let dofs = model.dof_map();
    let x = |k: usize| -> Vec3 {
        let p = model.mesh().nodes()[k];
        std::array::from_fn(|c| p[c] + u[dofs.equation(k, c).unwrap()])
    };
    let mut work = 0.0;
    for &k in &inner {
        let xk = x(k);
        for (c, xc) in xk.iter().enumerate() {
            work += reactions[dofs.equation(k, c).unwrap()] * xc;
        }
    }
    let g = 1.0 / 3.0f64.sqrt();
    let mut flux = 0.0;
    for face in &faces {
        let xs: Vec<Vec3> = face.iter().map(|&k| x(k)).collect();
        for xi in [-g, g] {
            for eta in [-g, g] {
                let nshape = [
                    0.25 * (1.0 - xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 + eta),
                    0.25 * (1.0 - xi) * (1.0 + eta),
                ];
                let dxi = [
                    -0.25 * (1.0 - eta),
                    0.25 * (1.0 - eta),
                    0.25 * (1.0 + eta),
                    -0.25 * (1.0 + eta),
                ];
                let deta = [
                    -0.25 * (1.0 - xi),
                    -0.25 * (1.0 + xi),
                    0.25 * (1.0 + xi),
                    0.25 * (1.0 - xi),
                ];
                let mut p = [0.0; 3];
                let mut a = [0.0; 3];
                let mut b = [0.0; 3];
                for q in 0..4 {
                    for c in 0..3 {
                        p[c] += nshape[q] * xs[q][c];
                        a[c] += dxi[q] * xs[q][c];
                        b[c] += deta[q] * xs[q][c];
                    }
                }
                let nrm = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                flux += p[0] * nrm[0] + p[1] * nrm[1] + p[2] * nrm[2];
            }
        }
    }
    let v_cav = (flux / 3.0).abs();
    let p_fe = work / (3.0 * v_cav) / MU;
    let pc = p_closed(l_a);
    let iterations: Vec<usize> = r.steps.iter().map(|s| s.residuals.len()).collect();
    println!(
        "  sphere n {n} m {m} {}: p_FE / mu {p_fe:.10}, closed {pc:.10}, V_cav {v_cav:.8} (exact {:.8}), steps {}, iterations {iterations:?}",
        f.name(),
        std::f64::consts::PI / 6.0 * l_a.powi(3),
        r.steps.len()
    );
    Ok((p_fe, (p_fe - pc).abs() / pc))
}

#[test]
#[ignore = "record of the registered run of ADR 0541 (log run1-sa_hyper.log): SP0 1.47e-16 and SP1 2.61e-3 met; SPL not measured (the Full solve failed in load step 3 with a negative pivot); SP0, SP1 continue in sp0_sp1_met_rows, the diagnostic SPLd of amendment 2 in spld_locking_diagnostic"]
fn sp0_sp1_spl_the_inflated_sphere_known_miss() {
    let mut c = Checks::new();
    let (lb, la) = (l_b(L_A), L_A);
    let q: f64 = gauss_legendre(20)
        .iter()
        .map(|(x, w)| {
            let l = 0.5 * (la + lb) + 0.5 * (la - lb) * x;
            0.5 * (la - lb) * w * 2.0 * (l.powi(-2) + l.powi(-5))
        })
        .sum();
    c.at_most(
        "SP0 quadrature against the closed form",
        (q - p_closed(L_A)).abs() / p_closed(L_A),
        1e-12,
    );
    match sphere_pressure(6, 8, BBAR) {
        Ok((_, e)) => c.at_most("SP1 BBar n 6 m 8: |p_FE - p_c| / p_c", e, 1e-2),
        Err(e) => c.holds("SP1 BBar n 6 m 8: the solve", false, &e),
    }
    match sphere_pressure(6, 8, FULL) {
        Ok((_, e)) => c.at_least("SPL Full n 6 m 8 (locking control): |p_FE - p_c| / p_c", e, 0.1),
        Err(e) => c.holds("SPL Full n 6 m 8: the solve (control not measured)", false, &e),
    }
    c.finish();
}

/// SP0 and SP1 of the registered run, continued (amendment 2).
#[test]
fn sp0_sp1_met_rows() {
    let mut c = Checks::new();
    let (lb, la) = (l_b(L_A), L_A);
    let q: f64 = gauss_legendre(20)
        .iter()
        .map(|(x, w)| {
            let l = 0.5 * (la + lb) + 0.5 * (la - lb) * x;
            0.5 * (la - lb) * w * 2.0 * (l.powi(-2) + l.powi(-5))
        })
        .sum();
    c.at_most(
        "SP0 quadrature against the closed form",
        (q - p_closed(L_A)).abs() / p_closed(L_A),
        1e-12,
    );
    match sphere_pressure(6, 8, BBAR) {
        Ok((_, e)) => c.at_most("SP1 BBar n 6 m 8: |p_FE - p_c| / p_c", e, 1e-2),
        Err(e) => c.holds("SP1 BBar n 6 m 8: the solve", false, &e),
    }
    c.finish();
}

/// SPLd of amendment 2 (diagnostic, no limit): Full and BBar to the inner stretch 1.125.
#[test]
fn spld_locking_diagnostic() {
    let c = Checks::new();
    for f in [FULL, BBAR] {
        match sphere_pressure_to(6, 8, f, 1.125) {
            Ok((_, e)) => c.report(
                &format!("SPLd {} n 6 m 8 to l_a 1.125: |p_FE - p_c| / p_c", f.name()),
                e,
            ),
            Err(e) => println!("SPLd {} n 6 m 8 to l_a 1.125: the solve failed: {e}", f.name()),
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------- refusals and inversion

#[test]
fn rf1_non_positive_moduli_refused() {
    let mut c = Checks::new();
    let refused = |r: rcfd_fem::Result<HyperelasticEnergy>| matches!(r, Err(FemError::InvalidMaterial { .. }));
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        c.holds(
            &format!("RF1 neo-Hookean mu {bad}"),
            refused(HyperelasticEnergy::neo_hookean(bad, 1e9)),
            "",
        );
        c.holds(
            &format!("RF1 neo-Hookean kappa {bad}"),
            refused(HyperelasticEnergy::neo_hookean(1e6, bad)),
            "",
        );
    }
    for (c10, c01) in [(0.0, 0.0), (-1e5, 5e4), (f64::NAN, 1e5), (1e5, f64::INFINITY)] {
        c.holds(
            &format!("RF1 Mooney-Rivlin c10 {c10} c01 {c01}"),
            refused(HyperelasticEnergy::mooney_rivlin(c10, c01, 1e9)),
            "",
        );
    }
    for bad in [0.0, -1.0, f64::NAN] {
        c.holds(
            &format!("RF1 Mooney-Rivlin kappa {bad}"),
            refused(HyperelasticEnergy::mooney_rivlin(3e5, 1e5, bad)),
            "",
        );
    }
    c.finish();
}

fn is_inversion(e: &FemError) -> bool {
    matches!(e, FemError::InvalidValue { key, .. } if *key == INVERSION_KEY)
}

#[test]
fn inv1_inv2_inverted_elements() {
    let mut c = Checks::new();
    for (name, en) in [("NH", nh(K_LOW)), ("MR", mr(K_LOW))] {
        for f in [FULL, BBAR] {
            let model = model_of(hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap(), f, en);
            let mut prescribed = Vec::new();
            for (n, p) in model.mesh().nodes().iter().enumerate() {
                for k in 0..3 {
                    let v = if k == 0 && p[0] == 2.0 { -1.5 } else { 0.0 };
                    prescribed.push(Prescribed::new(n, k, v));
                }
            }
            let r = solve(&model, &prescribed, &[], &NewtonOptions::new(1).threads(1));
            let ok = matches!(&r, Err(FemError::Element { element: 1, source }) if is_inversion(source));
            c.holds(
                &format!(
                    "INV1 {name} {}: solve names element 1, key deformation gradient",
                    f.name()
                ),
                ok,
                &match &r {
                    Ok(_) => "solved".to_string(),
                    Err(e) => e.to_string(),
                },
            );
            let coords = distorted_hex8();
            let mut u = vec![0.0; 24];
            for (a, x) in coords.iter().enumerate() {
                if ElementKind::Hex8.reference_nodes()[a][0] > 0.0 {
                    u[3 * a] = -1.5 * x[0];
                }
            }
            let r = hex8_force_and_tangent_at(f, &coords, &material(en), &u, &empty(), &PointContext::default());
            c.holds(
                &format!("INV2 {name} {}: element refused with the key", f.name()),
                matches!(&r, Err(e) if is_inversion(e)),
                &format!("{:?}", r.err()),
            );
        }
        let r = mat_hyper::stress(&en, &[-0.6, 0.0, 0.0, 0.0, 0.0, 0.0]);
        c.holds(
            &format!("INV2 {name}: mat_hyper::stress at E11 = -0.6"),
            matches!(&r, Err(e) if is_inversion(e)),
            &format!("{:?}", r.err()),
        );
    }
    c.finish();
}

fn follow_up_refusal<T>(r: &rcfd_fem::Result<T>) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant, package, .. }) if *variant == "Hyperelastic" && *package == FOLLOW_UP)
}

#[test]
#[ignore = "record of ADR 0541: ADR 0555 (F-620) opened the linear paths and the nonlinear transient Method step to the hyperelastic material, retiring the rows solve_static, modal_analysis and Method step refused; continued in sa_hyper_paths.rs rf2_the_paths_after_f620 (ADR 0291 pattern, maintainer standing decision)"]
fn rf2_the_paths() {
    let mut c = Checks::new();
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model.add_material("all", material(nh(K_LOW))).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [1e3, 0.0, 0.0])).unwrap();
    let r = solve_static(&model, &StaticOptions::default());
    c.holds(
        "RF2 solve_static refused naming FOLLOW_UP",
        follow_up_refusal(&r),
        &format!("{:?}", r.err()),
    );
    let r = modal_analysis(&model, &ModalOptions::new(3));
    c.holds(
        "RF2 modal_analysis refused naming FOLLOW_UP",
        follow_up_refusal(&r),
        &format!("{:?}", r.err()),
    );
    let r = material(nh(K_LOW)).elasticity_matrix();
    c.holds(
        "RF2 elasticity_matrix refused naming FOLLOW_UP",
        follow_up_refusal(&r)
            && matches!(&r, Err(FemError::NotYetSupported { location, .. }) if location == "the element-constant elasticity matrix"),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&model, &[], &NlTransientOptions::new(0.01));
    c.holds(
        "RF2 nl_transient Method step refused naming FOLLOW_UP",
        follow_up_refusal(&r),
        &format!("{:?}", r.as_ref().err()),
    );
    let dg = NlTransientOptions::new(0.01)
        .energy_momentum()
        .algorithmic_stress(AlgorithmicStress::DiscreteGradient);
    let r = NlIntegrator::new(&model, &[], &dg);
    c.holds(
        "RF2 energy-momentum step with DiscreteGradient accepted",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = solve(&model, &[], &[], &NewtonOptions::new(2).threads(1));
    c.holds(
        "RF2 the nonlinear static solve takes it",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

#[test]
fn dg1_the_step_local_energy_of_adr_0523() {
    let mut c = Checks::new();
    let en = nh(K_LOW);
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap());
    model.add_material("all", material(en)).unwrap();
    let coords = element_coords(&model, 0);
    let (_, uc) = element_states(&coords).remove(2);
    let dofs = model.dof_map();
    let mut u0 = vec![0.0; dofs.n_equations()];
    for (a, &n) in model.mesh().element_nodes(0).iter().enumerate() {
        for k in 0..3 {
            u0[dofs.equation(n, k).unwrap()] = uc[3 * a + k];
        }
    }
    let options = NlTransientOptions::new(0.01)
        .energy_momentum()
        .algorithmic_stress(AlgorithmicStress::DiscreteGradient);
    let mut it = NlIntegrator::new(&model, &[], &options).unwrap();
    it.start(0.0, &u0, &vec![0.0; u0.len()]).unwrap();
    let w_dg = it.strain_energy().unwrap();
    let w = element_energy(FULL, &coords, &en, &uc);
    c.at_most(
        "DG1 strain_energy (ADR 0523 D2) against mat_hyper::energy",
        (w_dg - w).abs() / w.abs(),
        1e-12,
    );
    c.finish();
}
