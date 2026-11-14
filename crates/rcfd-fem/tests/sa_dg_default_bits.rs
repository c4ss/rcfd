//! F-607 (ADR 0523): check B1 (record B0, D9): the energy-momentum step of `nl_transient` (F-570, ADR 0514) gives the bits it
//! gave before F-607 on every path that F-607 touches: the spinning bar (state bytes every 10 steps of 50), the pendulum with
//! Rayleigh damping, a split run of the bar through `to_bytes`, `from_bytes` and `restore`, and the energies and momenta at the
//! end of each. Uses only API that existed at 615ad811; the expected digests were printed by this file run on the unchanged
//! base 615ad811 (`f607-scratch/b0-base.log`) and written into `EXPECTED` before any library line of F-607.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable, Rayleigh};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Digests of 615ad811 in the order of the run, from the base run (`f607-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("spinning bar energy-momentum", 0x754a_a415_a2c1_ce63),
    ("pendulum energy-momentum damped", 0x3afb_1c58_37e3_3b4f),
    ("spinning bar energy-momentum split run", 0xa37f_3ede_e46d_8847),
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

fn newton() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    o
}

fn pendulum_model() -> SolidModel {
    let mut mesh = slender_mesh();
    let nodes = mesh.nodes().to_vec();
    let pin: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    let mid: Vec<usize> = pin.iter().copied().filter(|&i| near(nodes[i][1], 0.0)).collect();
    mesh.add_node_set("pin", pin).unwrap();
    mesh.add_node_set("pin_mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", soft()).unwrap();
    m.add_constraint(Constraint::displacement("pin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin_mid", 1, 0.0)).unwrap();
    m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    m
}

fn pendulum(model: &SolidModel, options: NlTransientOptions) -> NlIntegrator {
    let loads = [LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let mut it = NlIntegrator::new(model, &loads, &options).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    it
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

/// The free spinning bar of ADR 0479 D5 started in its steady spin.
fn spinning_bar(options: NlTransientOptions) -> NlIntegrator {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", soft()).unwrap();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes().to_vec();
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
        m: consistent(&model).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&model, &prescribed, &[], &NewtonOptions::new(1), ext).unwrap();
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
    let mut it = NlIntegrator::new(&model, &[], &options).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    it
}

#[test]
fn b1_energy_momentum_step_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let mut out = Vec::new();
    let mut it = spinning_bar(NlTransientOptions::new(0.01).newton(newton()).energy_momentum());
    for _ in 0..5 {
        it.run(10).unwrap();
        out.extend(it.state().unwrap().to_bytes());
    }
    tail(&it, &mut out);
    got.push(("spinning bar energy-momentum", digest_bytes(out)));

    let model = pendulum_model();
    let mut out = Vec::new();
    let options = NlTransientOptions::new(0.005)
        .newton(newton())
        .energy_momentum()
        .damping(Rayleigh {
            mass: 0.5,
            stiffness: 1e-5,
        });
    let mut it = pendulum(&model, options);
    it.run(30).unwrap();
    tail(&it, &mut out);
    got.push(("pendulum energy-momentum damped", digest_bytes(out)));

    let mut out = Vec::new();
    let options = NlTransientOptions::new(0.01).newton(newton()).energy_momentum();
    let mut first = spinning_bar(options.clone());
    first.run(15).unwrap();
    let bytes = first.state().unwrap().to_bytes();
    out.extend_from_slice(&bytes);
    let mut second = spinning_bar(options);
    second.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
    second.run(15).unwrap();
    tail(&second, &mut out);
    got.push(("spinning bar energy-momentum split run", digest_bytes(out)));

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
