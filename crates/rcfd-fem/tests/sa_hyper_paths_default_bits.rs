//! F-620 (ADR 0555): check B0 (D8): the paths that F-620 touches give the bits they gave before F-620: the linear static
//! solve and its recovered stress, the modes, the damped modes and the direct harmonic solve with a loss factor, the linear
//! transient integrator, the pre-stress and the buckling factors, the patch recovery (Full and BBar Hex8, the isotropic
//! material), the nonlinear transient Method step and the energy-momentum step with the neo-Hookean material and
//! `DiscreteGradient`, and the refusal messages of the J2, viscoelastic and creep materials. Uses only API that existed at
//! 89bdd903; the expected digests were printed by this file run on the unchanged base 89bdd903 (`f620-scratch/b0-base.log`)
//! and written into `EXPECTED` before any library line of F-620.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::estimate::{recover, PatchOptions};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::material::{HyperelasticEnergy, PronyTerm};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidModel};

/// Digests of 89bdd903 in the order of the run, from the base run (`f620-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 4]> = Some([
    ("linear paths Full", 0x682f_5829_6f6a_1ab9),
    ("linear paths BBar", 0xe6ff_9692_f7be_0120),
    (
        "nl_transient Method step and energy-momentum DiscreteGradient",
        0x4fa6_f056_fe32_0fdf,
    ),
    ("refusal messages", 0x2b33_2355_8519_b547),
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

fn push_err<T>(out: &mut Vec<u8>, r: &rcfd_fem::Result<T>) {
    match r {
        Ok(_) => out.extend_from_slice(b"ok"),
        Err(e) => out.extend_from_slice(format!("err {e}").as_bytes()),
    }
}

/// The isotropic twin of NHc (ADR 0555 D7): `G = 1e6`, `K = 1e7`.
fn twin() -> Material {
    let (g, k) = (1e6, 1e7);
    let e = 9.0 * k * g / (3.0 * k + g);
    let nu = (3.0 * k - 2.0 * g) / (2.0 * (3.0 * k + g));
    Material::isotropic_elastic(e, nu, 1000.0, 0.0).unwrap()
}

/// Cantilever L of ADR 0555 D7 with a material, a formulation and the tip force per node.
fn cantilever(material: Material, formulation: SolidFormulation, force: [f64; 3]) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", force)).unwrap();
    model
}

fn linear_paths(formulation: SolidFormulation) -> u64 {
    let mut out = Vec::new();
    let model = cantilever(twin(), formulation, [0.0, 0.0, 1.0]);
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    push_f64(&mut out, r.displacement_values());
    let field = IntegrationPointField::from_result(&model, &r).unwrap();
    push_f64(&mut out, &field.stress().concat());
    let rec = recover(&model, r.displacement_values(), &PatchOptions::new().threads(Some(1))).unwrap();
    for s in rec.nodal().iter().flatten() {
        push_f64(&mut out, s);
    }
    let modal = modal_analysis(&model, &ModalOptions::new(6)).unwrap();
    push_f64(&mut out, &modal.frequencies);
    push_f64(&mut out, &modal.modes[0]);
    let mut d = DampedOptions::new(4);
    d.loss_factors.push(("all".to_string(), LossFactor::Constant(0.02)));
    let dm = damped_modes(&model, &d).unwrap();
    for root in &dm.roots {
        push_f64(&mut out, &[root.re, root.im]);
    }
    let dh = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new()
            .loss_factor("all", LossFactor::Constant(0.02))
            .threads(1),
    )
    .unwrap();
    let load = HarmonicLoad::from_model(&model).unwrap();
    let x = dh.response(&load, 50.0).unwrap();
    push_f64(&mut out, &x.re);
    push_f64(&mut out, &x.im);
    let n = model.dof_map().n_equations();
    let history = LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    let mut it = Integrator::new(&model, &[history], &TransientOptions::new(1e-3).threads(1)).unwrap();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it.run(20).unwrap();
    push_f64(&mut out, &it.displacement().unwrap());
    let axial = cantilever(twin(), formulation, [-1.0, 0.0, 0.0]);
    let ra = solve_static(&axial, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&axial, &ra).unwrap();
    let b = rcfd_fem::buckling::factors(&axial, &pre, &rcfd_fem::buckling::BucklingOptions::new(3)).unwrap();
    push_f64(&mut out, &b.factors);
    digest_bytes(out)
}

/// Cube F of ADR 0555 D7 with a material.
fn cube(material: Material) -> (SolidModel, Vec<f64>) {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model.add_material("all", material).unwrap();
    let dofs = model.dof_map();
    let mut v0 = vec![0.0; dofs.n_equations()];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        let r = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let w = [
            -3.0 * r[1] + 0.3 + 5.0 * r[0] + 4.0 * r[1],
            3.0 * r[0] - 0.2 + 5.0 * r[1],
            0.1 + 5.0 * r[2],
        ];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    (model, v0)
}

fn transient_steps() -> u64 {
    let mut out = Vec::new();
    let iso = Material::isotropic_elastic(2.9e5, 0.3, 1000.0, 0.0).unwrap();
    let (model, v0) = cube(iso);
    let n = v0.len();
    let mut it = NlIntegrator::new(
        &model,
        &[],
        &NlTransientOptions::new(0.004).newton(NewtonOptions::new(1)),
    )
    .unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    for _ in 0..2 {
        it.run(10).unwrap();
        out.extend_from_slice(&it.state().unwrap().to_bytes());
        push_f64(&mut out, &[it.strain_energy().unwrap(), it.kinetic_energy().unwrap()]);
    }
    let nh = Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e5, 1e6).unwrap(), 1000.0).unwrap();
    let (model, v0) = cube(nh);
    let o = NlTransientOptions::new(0.004)
        .newton(NewtonOptions::new(1))
        .energy_momentum()
        .algorithmic_stress(AlgorithmicStress::DiscreteGradient);
    let mut it = NlIntegrator::new(&model, &[], &o).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    it.run(10).unwrap();
    out.extend_from_slice(&it.state().unwrap().to_bytes());
    push_f64(&mut out, &[it.strain_energy().unwrap()]);
    digest_bytes(out)
}

fn refusals() -> u64 {
    let mut out = Vec::new();
    let base = twin();
    for m in [
        Material::j2(base.clone(), 2e5, 1e5, 0.0).unwrap(),
        Material::viscoelastic(base.clone(), vec![PronyTerm::new(0.3, 0.1, 1e-3).unwrap()], None).unwrap(),
        Material::creep(base.clone(), 1e-30, 4.0, -0.5).unwrap(),
    ] {
        let model = cantilever(m, SolidFormulation::Full, [0.0, 0.0, 1.0]);
        push_err(&mut out, &solve_static(&model, &StaticOptions::default()));
        push_err(&mut out, &modal_analysis(&model, &ModalOptions::new(3)));
        push_err(
            &mut out,
            &NlIntegrator::new(&model, &[], &NlTransientOptions::new(0.01)),
        );
    }
    digest_bytes(out)
}

#[test]
fn b0_default_paths_bits() {
    let got = [
        ("linear paths Full", linear_paths(SolidFormulation::Full)),
        ("linear paths BBar", linear_paths(SolidFormulation::BBar)),
        (
            "nl_transient Method step and energy-momentum DiscreteGradient",
            transient_steps(),
        ),
        ("refusal messages", refusals()),
    ];
    for (name, d) in &got {
        println!("B0 {name}: 0x{d:016x}");
    }
    match EXPECTED {
        None => panic!("B0 base run: copy the digests above into EXPECTED"),
        Some(want) => {
            for ((n, d), (wn, wd)) in got.iter().zip(want.iter()) {
                assert_eq!(n, wn);
                assert_eq!(d, wd, "B0 {n}: 0x{d:016x} against the base 0x{wd:016x}");
            }
        }
    }
}
