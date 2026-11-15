//! F-425 (ADR 0541): check B0 (D9): the paths that F-425 touches give the bits they gave before F-425: the constitutive call
//! and the element-constant matrix of every other material variant, the nonlinear static solve of a Hex8 box (Full and BBar),
//! the linear static solve, the refusal messages of the linear static solve for the materials with history, the nonlinear
//! transient Method step and the energy-momentum step with the neo-Hookean material of ADR 0523. Uses only API that existed at
//! 8fcf5b40; the expected digests were printed by this file run on the unchanged base 8fcf5b40 (`f425-scratch/b0-base.log`)
//! and written into `EXPECTED` before any library line of F-425.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::material::{HyperelasticEnergy, MaterialAxes, PointContext, PronyTerm};
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidModel};

/// Digests of 8fcf5b40 in the order of the run, from the base run (`f425-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("materials stress and elasticity matrix", 0xe9ca_b374_fcf0_e485),
    ("nonlinear box Full", 0x3c1b_1f16_0332_5213),
    ("nonlinear box BBar", 0xd79e_02ea_9b8f_3341),
    ("linear static and refusals", 0xcd23_6138_4998_ab9e),
    ("nl_transient Method step", 0xc82f_d50c_e72c_f9f1),
    ("nl_transient energy-momentum neo-Hookean", 0xd3bd_3291_5583_3c5d),
    ("materials again (determinism)", 0xe9ca_b374_fcf0_e485),
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

fn push_result<T: std::fmt::Debug>(out: &mut Vec<u8>, r: &rcfd_fem::Result<T>) {
    match r {
        Ok(v) => out.extend_from_slice(format!("ok {v:?}").as_bytes()),
        Err(e) => out.extend_from_slice(format!("err {e}").as_bytes()),
    }
}

fn iso() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn variants() -> Vec<(Material, usize, PointContext)> {
    let fixed = MaterialAxes::fixed([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    let hot = PointContext::default().with_temperature(330.0).unwrap();
    let step = PointContext::default().with_time(2.0, 0.5).unwrap();
    vec![
        (iso(), 0, PointContext::default()),
        (
            Material::orthotropic(
                [1.4e11, 1.0e10, 1.0e10],
                [0.3, 0.3, 0.45],
                [5.0e9, 5.0e9, 3.5e9],
                1600.0,
                [0.0; 3],
                fixed,
            )
            .unwrap(),
            0,
            PointContext::default(),
        ),
        (
            Material::tabulated(iso(), vec![293.0, 373.0], vec![("e".to_string(), vec![2.0e11, 1.9e11])]).unwrap(),
            0,
            hot,
        ),
        (
            Material::j2(iso(), 2.5e8, 1e9, 5e8).unwrap(),
            13,
            PointContext::default(),
        ),
        (
            Material::viscoelastic(iso(), vec![PronyTerm::new(0.3, 0.1, 1e-1).unwrap()], None).unwrap(),
            0,
            step.clone(),
        ),
        (Material::creep(iso(), 1e-30, 4.0, -0.5).unwrap(), 7, step),
    ]
}

fn materials_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    let strain = [1.1e-3, -4.0e-4, 2.0e-4, 3.0e-4, -1.5e-4, 5.0e-5];
    for (m, _, ctx) in variants() {
        let state = vec![0.0; m.state_size()];
        out.extend_from_slice(m.name().as_bytes());
        out.extend_from_slice(&(m.state_size() as u64).to_le_bytes());
        match m.stress(&strain, &ctx, &state) {
            Ok((s, d)) => {
                push_f64(&mut out, &s);
                push_f64(&mut out, &d.concat());
            }
            Err(e) => out.extend_from_slice(e.to_string().as_bytes()),
        }
        match m.elasticity_matrix() {
            Ok(d) => push_f64(&mut out, &d.concat()),
            Err(e) => out.extend_from_slice(e.to_string().as_bytes()),
        }
    }
    out
}

/// A 3 x 2 x 2 Hex8 box held at `xmin`, the `xmax` nodes stretched and turned about `x` by prescribed values.
fn nonlinear_box_bytes(formulation: SolidFormulation) -> Vec<u8> {
    let mut mesh = hex_box([0.0; 3], [3.0, 1.0, 1.0], [3, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let nodes = mesh.nodes().to_vec();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.0e9, 0.45, 1000.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let (ca, sa) = (0.4f64.cos(), 0.4f64.sin());
    let mut prescribed = Vec::new();
    for (i, p) in nodes.iter().enumerate() {
        if (p[0] - 3.0).abs() < 1e-12 {
            let (y, z) = (p[1] - 0.5, p[2] - 0.5);
            let target = [3.3, 0.5 + ca * y - sa * z, 0.5 + sa * y + ca * z];
            for c in 0..3 {
                prescribed.push(Prescribed::new(i, c, target[c] - p[c]));
            }
        }
    }
    let r = solve(&model, &prescribed, &[], &NewtonOptions::new(4).threads(2)).unwrap();
    let mut out = Vec::new();
    push_f64(&mut out, r.result.displacement_values());
    push_f64(&mut out, r.result.reactions().unwrap_or(&[]));
    for s in &r.steps {
        push_f64(&mut out, &s.residuals);
        push_f64(&mut out, &s.energies);
    }
    out
}

fn cantilever(material: Material) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap());
    model.add_material("all", material).unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [0.0, -1.0e3, 0.0])).unwrap();
    model
}

fn linear_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    let r = solve_static(&cantilever(iso()), &StaticOptions::default()).unwrap();
    push_f64(&mut out, r.displacement_values());
    for (m, _, _) in variants().into_iter().skip(3) {
        push_result(
            &mut out,
            &solve_static(&cantilever(m), &StaticOptions::default()).map(|_| ()),
        );
    }
    out
}

fn bar(material: Material) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [5, 1, 1]).unwrap());
    model.add_material("all", material).unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [0.0, -2.0e3, 1.0e3])).unwrap();
    model
}

fn transient_bytes(model: &SolidModel, options: &NlTransientOptions) -> Vec<u8> {
    let loads = [LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let mut it = NlIntegrator::new(model, &loads, options).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    it.run(5).unwrap();
    let mut out = it.state().unwrap().to_bytes();
    push_f64(&mut out, &[it.kinetic_energy().unwrap(), it.strain_energy().unwrap()]);
    out
}

fn method_bytes() -> Vec<u8> {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    let model = bar(Material::isotropic_elastic(2.0e9, 0.3, 1000.0, 0.0).unwrap());
    transient_bytes(&model, &NlTransientOptions::new(1e-3).newton(o))
}

fn emc_neo_hookean_bytes() -> Vec<u8> {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    let nh = Material::hyperelastic(HyperelasticEnergy::neo_hookean(7.7e8, 1.67e9).unwrap(), 1000.0).unwrap();
    let model = bar(nh);
    let options = NlTransientOptions::new(1e-3)
        .newton(o)
        .energy_momentum()
        .algorithmic_stress(AlgorithmicStress::DiscreteGradient);
    transient_bytes(&model, &options)
}

#[test]
fn b0_default_bits_of_the_paths_f425_touches() {
    let runs: Vec<(&str, Vec<u8>)> = vec![
        ("materials stress and elasticity matrix", materials_bytes()),
        ("nonlinear box Full", nonlinear_box_bytes(SolidFormulation::Full)),
        ("nonlinear box BBar", nonlinear_box_bytes(SolidFormulation::BBar)),
        ("linear static and refusals", linear_bytes()),
        ("nl_transient Method step", method_bytes()),
        ("nl_transient energy-momentum neo-Hookean", emc_neo_hookean_bytes()),
        ("materials again (determinism)", materials_bytes()),
    ];
    let got: Vec<(&str, u64)> = runs
        .iter()
        .map(|(n, b)| (*n, digest_bytes(b.iter().copied())))
        .collect();
    for (n, d) in &got {
        println!(
            "    (\"{n}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xFFFF,
            (d >> 16) & 0xFFFF,
            d & 0xFFFF
        );
    }
    match EXPECTED {
        None => panic!("EXPECTED not recorded: run on the base and write the digests printed above"),
        Some(expected) => {
            let mut missed = Vec::new();
            for ((n, d), (en, ed)) in got.iter().zip(expected.iter()) {
                let ok = n == en && d == ed;
                println!("B0 {n}: {}", if ok { "bit-equal" } else { "DIFFERS" });
                if !ok {
                    missed.push(*n);
                }
            }
            assert!(missed.is_empty(), "B0 differs: {missed:?}");
        }
    }
}
