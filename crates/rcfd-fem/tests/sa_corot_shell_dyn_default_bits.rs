//! F-585 (ADR 0522): check N1 (record of D7), the solid paths of the arc length, `solve_from` and the nonlinear transient,
//! and the corotated shell element of F-578, give the bits they gave before F-585: `arclength::trace` of a Hex8 column,
//! `solve_from` of a Hex8 cantilever, the nonlinear transient of a Hex8 cantilever by the default method and by the
//! generalised-alpha method, `shell_corot::element_force_and_tangent` of a warped Shell4 and a Shell3. Uses only API that
//! existed at b6d3fb1c; the expected digests were printed by this file run on the unchanged base b6d3fb1c
//! (`f585-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-585.

use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, solve_from, NewtonOptions};
use rcfd_fem::shell_corot::element_force_and_tangent;
use rcfd_fem::transient::{LoadHistory, LoadTable, Method};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// Digests of b6d3fb1c in the order of the run, from the base run (`f585-scratch/n1-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("Hex8 column arc length", 0x6ac6_c567_7136_db09),
    ("Hex8 cantilever solve_from", 0x1ecf_eacb_7d0d_0969),
    ("Hex8 transient average acceleration", 0x03b0_7ec0_b509_054d),
    ("Hex8 transient generalised alpha", 0x5798_d059_2dbf_e757),
    ("Shell4 corotated element", 0x3f2c_62b5_fd1e_9ac4),
    ("Shell3 corotated element", 0xa6d3_5cc8_077a_ea80),
    ("count", 0x0000_0000_0000_0006),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn digest_bytes(b: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 0.0).unwrap()
}

fn column() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.1, 0.1, 1.0], [1, 1, 4]).unwrap();
    mesh.add_surface_set("top", mesh.surface_set("zmax").unwrap().to_vec())
        .unwrap();
    let mut col = SolidModel::new(mesh);
    col.add_material("all", steel(0.0)).unwrap();
    for c in 0..3 {
        col.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    col
}

fn cantilever() -> SolidModel {
    let mut cant = SolidModel::new(hex_box([0.0; 3], [1.0, 0.05, 0.05], [10, 1, 1]).unwrap());
    cant.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        cant.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    cant.add_load(Load::traction("xmax", [0.0, 4.0e6, 1.0e6])).unwrap();
    cant
}

fn shell_one(kind: ElementKind, nodes: Vec<[f64; 3]>) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, (0..n).collect()).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    m
}

#[test]
fn n1_default_bits_f585() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // The arc-length trace of the Hex8 column under an end force, three points.
    let mut col = column();
    col.add_load(Load::traction("top", [1.0e6, 0.0, -1.0e7])).unwrap();
    let mut count = 0;
    let mut stop = |_: &PathPoint| {
        count += 1;
        count >= 3
    };
    let path = trace(&col, &[], &[], &ArcLengthOptions::new(1e-3), &[], &mut stop).unwrap();
    let mut v = vec![path.load_scale];
    for p in &path.points {
        v.push(p.load_factor);
        v.push(p.length);
        v.push(p.negative_pivots as f64);
        v.extend_from_slice(&p.displacement);
        v.extend_from_slice(&p.residuals);
        v.extend_from_slice(&p.energies);
    }
    got.push(("Hex8 column arc length", digest(&v)));
    // solve_from of the Hex8 cantilever from zero to its load.
    let cant = cantilever();
    let n = cant.dof_map().n_equations();
    let f = nodal_load_vector(&cant, &[]).unwrap();
    let states = initial_states(&cant).unwrap();
    let eq = solve_from(&cant, &[], &f, &vec![0.0; n], &states, &NewtonOptions::new(1), &[]).unwrap();
    let mut v = eq.u.clone();
    v.extend_from_slice(&eq.internal_force);
    v.extend_from_slice(&eq.residuals);
    v.extend_from_slice(&eq.energies);
    got.push(("Hex8 cantilever solve_from", digest(&v)));
    // The nonlinear transient of the cantilever under a step load, 10 steps, by two methods.
    for (name, method) in [
        ("Hex8 transient average acceleration", Method::average_acceleration()),
        ("Hex8 transient generalised alpha", Method::GeneralisedAlpha { rho_inf: 0.8 }),
    ] {
        let table = LoadTable::constant(1.0, 0.0, 1.0).unwrap();
        let loads = [LoadHistory::new(f.clone(), table)];
        let mut it = Integrator::new(&cant, &loads, &NlTransientOptions::new(2.0e-4).method(method)).unwrap();
        it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
        let mut v = Vec::new();
        let mut bytes = Vec::new();
        for _ in 0..10 {
            let r = it.step().unwrap();
            v.push(r.halvings as f64);
            v.push(r.iterations as f64);
            v.push(r.residual);
            v.push(it.kinetic_energy().unwrap());
            v.push(it.strain_energy().unwrap());
            let (p, j) = it.momentum().unwrap();
            v.extend_from_slice(&p);
            v.extend_from_slice(&j);
            bytes.extend_from_slice(&it.state().unwrap().to_bytes());
        }
        v.push(f64::from_bits(digest_bytes(&bytes)));
        got.push((name, digest(&v)));
    }
    // The corotated element of F-578 at a deformed state.
    for (name, kind, nodes) in [
        (
            "Shell4 corotated element",
            ElementKind::Shell4,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.06], [1.1, 0.9, 0.0], [-0.1, 1.0, 0.08]],
        ),
        (
            "Shell3 corotated element",
            ElementKind::Shell3,
            vec![[0.0, 0.0, 0.0], [1.0, 0.1, 0.05], [0.2, 0.9, -0.03]],
        ),
    ] {
        let m = shell_one(kind, nodes);
        let ne = m.dof_map().n_equations();
        let ue: Vec<f64> = (0..ne).map(|i| 0.05 * ((i as f64) * 0.71).sin()).collect();
        let (force, k) = element_force_and_tangent(&m, 0, &ue).unwrap();
        let mut v = force;
        v.extend_from_slice(k.values());
        got.push((name, digest(&v)));
    }
    got.push(("count", got.len() as u64));
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("no expected digests yet: print them on the base"),
        Some(expected) => {
            let mut bad = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                if name != en || d != ed {
                    bad.push(format!("{name}: 0x{d:016x} != 0x{ed:016x}"));
                }
            }
            assert!(got.len() == expected.len() && bad.is_empty(), "N1 changed: {bad:?}");
        }
    }
}
