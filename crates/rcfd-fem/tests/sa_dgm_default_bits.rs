//! F-611 (ADR 0544): check B1 (record B0, D7): the energy-momentum step of `nl_transient` gives the bits it gave before F-611
//! on every path F-611 touches outside its new variant: the neo-Hookean bar and cube with `DiscreteGradient` (the step-local
//! energy of ADR 0523), the neo-Hookean cube with `Average`, the isotropic cube with `DiscreteGradient`, the isotropic bar
//! with `Auto` (the F-570 step) and a split run of the neo-Hookean cube (the start's internal force and the strain energy
//! after a restore). State bytes every 10 steps, the energies and momenta at the end. Uses only API that existed at f69eb128;
//! the expected digests were printed by this file run on the unchanged base f69eb128 (`f611-scratch/b0-base.log`) and written
//! into `EXPECTED` before any library line of F-611.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::{Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Digests of f69eb128 in the order of the run, from the base run (`f611-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 6]> = Some([
    ("NH bar discrete gradient", 0x1288_3fba_d403_85f5),
    ("NH cube discrete gradient", 0x8519_8b25_310d_5757),
    ("NH cube average", 0x78b9_1589_65f6_b9ed),
    ("isotropic cube discrete gradient", 0xf878_ad9f_170e_0b80),
    ("isotropic bar Auto", 0x9509_004c_1882_b948),
    ("NH cube split run", 0x7b19_05d1_3ced_dbf3),
]);

/// FNV-1a 64 over bytes.
fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn push_f64(out: &mut Vec<u8>, xs: &[f64]) {
    for x in xs {
        out.extend_from_slice(&x.to_bits().to_le_bytes());
    }
}

const L_BAR: f64 = 1.0;
const B_BAR: f64 = 0.02;
const RHO: f64 = 7850.0;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn slender_mesh() -> rcfd_fem::mesh::SolidMesh {
    hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap()
}

fn soft() -> Material {
    Material::isotropic_elastic(2.0e9, 0.3, RHO, 0.0).unwrap()
}

fn bar_neo_hookean() -> Material {
    let (e, nu) = (2.0e9, 0.3);
    let mu = e / (2.0 * (1.0 + nu));
    let kappa = e / (3.0 * (1.0 - 2.0 * nu));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(mu, kappa).unwrap(), RHO).unwrap()
}

fn newton() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    o
}

/// The end values of a run: the state bytes, the energies and the momenta.
fn tail(it: &NlIntegrator, out: &mut Vec<u8>) {
    out.extend(it.state().unwrap().to_bytes());
    let (p, j) = it.momentum().unwrap();
    push_f64(out, &[it.kinetic_energy().unwrap(), it.strain_energy().unwrap()]);
    push_f64(out, &p);
    push_f64(out, &j);
}

struct Centrifugal {
    m: CsrMatrix,
    offset: Vec<Option<f64>>,
    omega: f64,
}

impl Contribution for Centrifugal {
    fn name(&self) -> &str {
        "centrifugal"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let w2 = self.omega * self.omega;
        let r: Vec<f64> = (0..u.len()).map(|j| self.offset[j].map_or(0.0, |x| x + u[j])).collect();
        let mut mr = vec![0.0; u.len()];
        self.m.matvec(&r, &mut mr);
        for (f, x) in force.iter_mut().zip(&mr) {
            *f -= w2 * x;
        }
        for i in 0..self.m.n {
            for p in self.m.row_ptr[i]..self.m.row_ptr[i + 1] {
                let j = self.m.col[p];
                if self.offset[j].is_some() {
                    tangent.push((i, j, -w2 * self.m.val[p]));
                }
            }
        }
        Ok(())
    }
    fn in_residual_reference(&self) -> bool {
        true
    }
}

/// The free spinning bar of ADR 0479 D5 with the steady start of the St. Venant-Kirchhoff bar (ADR 0523 D7), the model with
/// `material`.
fn spinning_bar(material: Material, options: &NlTransientOptions) -> NlIntegrator {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut svk = SolidModel::new(slender_mesh());
    svk.add_material("all", soft()).unwrap();
    let dofs = svk.dof_map();
    let nodes = svk.mesh().nodes().to_vec();
    let n = dofs.n_equations();
    let mut offset = vec![None; n];
    let mut prescribed = Vec::new();
    for (i, p) in nodes.iter().enumerate() {
        for (comp, off) in [(0, p[0] - centre[0]), (1, p[1] - centre[1])] {
            offset[dofs.equation(i, comp).unwrap()] = Some(off);
        }
        if near(p[0], centre[0]) {
            prescribed.push(Prescribed::new(i, 0, 0.0));
        }
        if near(p[1], 0.0) {
            prescribed.push(Prescribed::new(i, 1, 0.0));
        }
        if near(p[2], 0.0) {
            prescribed.push(Prescribed::new(i, 2, 0.0));
        }
    }
    let spin = Centrifugal {
        m: consistent(&svk).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&svk, &prescribed, &[], &NewtonOptions::new(1), ext).unwrap();
    let u0 = steady.result.displacement_values().to_vec();
    let mut v0 = vec![0.0; n];
    for (i, p) in nodes.iter().enumerate() {
        let rx = p[0] + u0[dofs.equation(i, 0).unwrap()] - centre[0];
        let ry = p[1] + u0[dofs.equation(i, 1).unwrap()] - centre[1];
        let w = [-omega * ry + velocity[0], omega * rx + velocity[1], velocity[2]];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", material).unwrap();
    let mut it = NlIntegrator::new(&model, &[], options).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    it
}

fn cube_model(material: Material) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model.add_material("all", material).unwrap();
    model
}

/// The free cube of ADR 0523 arithmetic 3.
fn cube(material: Material, options: &NlTransientOptions) -> NlIntegrator {
    let model = cube_model(material);
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (omega, alpha, gamma) = (3.0, 5.0, 4.0);
    let velocity = [0.3, -0.2, 0.1];
    let mut v0 = vec![0.0; n];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        let r = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let w = [
            -omega * r[1] + velocity[0] + alpha * r[0] + gamma * r[1],
            omega * r[0] + velocity[1] + alpha * r[1],
            velocity[2] + alpha * r[2],
        ];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    let mut it = NlIntegrator::new(&model, &[], options).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    it
}

fn cube_nh() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e5, 1e6).unwrap(), 1000.0).unwrap()
}

fn cube_elastic() -> Material {
    let (mu, kappa) = (1e5, 1e6);
    let e = 9.0 * kappa * mu / (3.0 * kappa + mu);
    let nu = (3.0 * kappa - 2.0 * mu) / (2.0 * (3.0 * kappa + mu));
    Material::isotropic_elastic(e, nu, 1000.0, 0.0).unwrap()
}

fn bar_options(stress: AlgorithmicStress) -> NlTransientOptions {
    NlTransientOptions::new(0.01)
        .newton(newton())
        .energy_momentum()
        .algorithmic_stress(stress)
}

fn cube_options(stress: AlgorithmicStress) -> NlTransientOptions {
    NlTransientOptions::new(0.004)
        .newton(NewtonOptions::new(1))
        .energy_momentum()
        .algorithmic_stress(stress)
}

fn record(mut it: NlIntegrator, steps: usize) -> u64 {
    let mut out = Vec::new();
    for _ in 0..steps / 10 {
        it.run(10).unwrap();
        out.extend(it.state().unwrap().to_bytes());
    }
    tail(&it, &mut out);
    digest_bytes(out)
}

#[test]
fn b1_paths_of_the_energy_momentum_step_keep_the_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let dg = AlgorithmicStress::DiscreteGradient;
    got.push((
        "NH bar discrete gradient",
        record(spinning_bar(bar_neo_hookean(), &bar_options(dg)), 30),
    ));
    got.push((
        "NH cube discrete gradient",
        record(cube(cube_nh(), &cube_options(dg)), 30),
    ));
    got.push((
        "NH cube average",
        record(cube(cube_nh(), &cube_options(AlgorithmicStress::Average)), 20),
    ));
    got.push((
        "isotropic cube discrete gradient",
        record(cube(cube_elastic(), &cube_options(dg)), 20),
    ));
    got.push((
        "isotropic bar Auto",
        record(spinning_bar(soft(), &bar_options(AlgorithmicStress::Auto)), 20),
    ));

    let mut out = Vec::new();
    let options = cube_options(dg);
    let mut first = cube(cube_nh(), &options);
    first.run(10).unwrap();
    let bytes = first.state().unwrap().to_bytes();
    out.extend_from_slice(&bytes);
    let mut second = NlIntegrator::new(&cube_model(cube_nh()), &[], &options).unwrap();
    second.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
    push_f64(&mut out, &[second.strain_energy().unwrap()]);
    second.run(10).unwrap();
    tail(&second, &mut out);
    got.push(("NH cube split run", digest_bytes(out)));

    for (name, d) in &got {
        println!("B0 {name}: 0x{d:016x}");
    }
    match EXPECTED {
        None => panic!("no expected digests recorded yet (run on the base and write them into EXPECTED)"),
        Some(expected) => {
            assert_eq!(got.len(), expected.len());
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(name, en);
                assert_eq!(*d, *ed, "{name}: digest 0x{d:016x}, base 0x{ed:016x}");
            }
        }
    }
}
