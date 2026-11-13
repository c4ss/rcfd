//! F-490 (ADR 0393): the transient integrator on the transformation path (elastic supports, rigid sets) and with prescribed
//! motion histories. Checks B1 to B4, R1, R2, X1 to X7, D1, Q1, Q2 and the negative controls N1 to N3 of the registration.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions, TransientState};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

/// The digest of the three ADR 0389 runs of D1, recorded from the unchanged library (ADR 0393 amendment 1).
const REF_DIGEST: u64 = 0xabb6_faeb_ad69_662b;

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// FNV-1a 64.
struct Digest(u64);

impl Digest {
    fn new() -> Digest {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn floats(&mut self, v: &[f64]) {
        for x in v {
            self.bytes(&x.to_bits().to_le_bytes());
        }
    }
}

// ------------------------------------------------------------------------------------------------- bar B (ADR 0389)

fn bar_b() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

struct Bar {
    model: SolidModel,
    m: CsrMatrix,
    phi: Vec<f64>,
    omega: f64,
    period: f64,
}

fn bar() -> Bar {
    let model = bar_b();
    let modal = modal_analysis(&model, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let k = stiffness(&model).unwrap();
    let m = consistent(&model).unwrap();
    let omega = (dot(&phi, &mv(&k, &phi)) / dot(&phi, &mv(&m, &phi))).sqrt();
    Bar {
        period: 2.0 * PI / omega,
        model,
        m,
        phi,
        omega,
    }
}

fn damping_of(omega: f64) -> Rayleigh {
    Rayleigh::from_ratio(0.02, omega, omega).unwrap()
}

/// ADR 0389's A10 set-up: from rest, `M phi_1` with the ramp, `rho_inf = 0.8`, Rayleigh 0.02, `dt = T_1/40`.
fn a10_options(b: &Bar, threads: usize) -> (Vec<LoadHistory>, TransientOptions) {
    let table = LoadTable::new(vec![(0.0, 0.0), (b.period, 1.0), (10.0 * b.period, 1.0)]).unwrap();
    let o = TransientOptions::new(b.period / 40.0)
        .method(Method::GeneralisedAlpha { rho_inf: 0.8 })
        .damping(damping_of(b.omega))
        .threads(threads);
    (vec![LoadHistory::new(mv(&b.m, &b.phi), table)], o)
}

fn digest_run(d: &mut Digest, it: &mut Integrator, steps: u64) {
    d.bytes(&it.state().unwrap().to_bytes());
    for _ in 0..steps {
        it.step().unwrap();
        d.bytes(&it.state().unwrap().to_bytes());
    }
    d.floats(&it.displacement().unwrap());
    d.floats(&it.velocity().unwrap());
    d.floats(&it.acceleration().unwrap());
    d.floats(&[it.kinetic_energy().unwrap(), it.strain_energy().unwrap()]);
}

fn model_q(omega_2: f64) -> (CsrMatrix, CsrMatrix) {
    let (s, c) = 0.3f64.sin_cos();
    let (l1, l2) = (1.0, omega_2 * omega_2);
    let k = CsrMatrix::from_triplets(
        2,
        vec![
            (0, 0, c * c * l1 + s * s * l2),
            (0, 1, c * s * (l1 - l2)),
            (1, 0, c * s * (l1 - l2)),
            (1, 1, s * s * l1 + c * c * l2),
        ],
    );
    let m = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, 1.0)]);
    (k, m)
}

/// The digest of D1 through the API of ADR 0389 only.
fn adr_0389_digest(b: &Bar) -> u64 {
    let mut d = Digest::new();
    let zero = vec![0.0; b.phi.len()];
    // (i) free vibration, Newmark, Omega = 0.2.
    let mut it = Integrator::new(&b.model, &[], &TransientOptions::new(0.2 / b.omega)).unwrap();
    it.start(0.0, &b.phi, &zero).unwrap();
    digest_run(&mut d, &mut it, 200);
    // (ii) the A10 set-up.
    let (loads, o) = a10_options(b, 1);
    let mut it = Integrator::new(&b.model, &loads, &o).unwrap();
    it.start(0.0, &zero, &zero).unwrap();
    digest_run(&mut d, &mut it, 200);
    // (iii) model Q.
    let (k, m) = model_q(1e4);
    let o = TransientOptions::new(0.1).method(Method::GeneralisedAlpha { rho_inf: 0.5 });
    let mut it = Integrator::from_matrices(k, m, &[], &o).unwrap();
    it.start(0.0, &[1.0, 0.5], &[0.0, 0.0]).unwrap();
    digest_run(&mut d, &mut it, 50);
    d.0
}

#[test]
fn d1_models_without_the_features_keep_the_bits_of_adr_0389() {
    let b = bar();
    let digest = adr_0389_digest(&b);
    println!("D1 digest {digest:#018x} (reference {REF_DIGEST:#018x})");
    assert_eq!(digest, REF_DIGEST, "D1: the digest of the ADR 0389 runs changed");
    let _ = TransientState::from_bytes;
}
