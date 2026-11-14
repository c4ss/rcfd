//! F-422 (ADR 0475): check N1 (record B0, D7), the nonlinear static path, the element functions of the total Lagrangian
//! driver, the isotropic constitutive call and the static solve give the bits they gave before F-422 for models without the
//! J2 material: an elastic stretched bar on Hex8 and Hex20, an elastic Hex20 cantilever in two load steps, `solve_with` with
//! a zero contribution that refuses one call (the halving path), the assembled force and tangent of a distorted Tet10 model,
//! the element force and tangent of a distorted Hex8, `Material::stress`, `solve_static`, and the refusal of a pending
//! variant. Uses only API that existed at 4abff5fe; the expected digests were printed by this file run on the unchanged base
//! 4abff5fe (`f422-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-422.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::material::PointContext;
use rcfd_fem::material::PronyTerm;
use rcfd_fem::nonlinear::{
    element_internal_force, element_tangent, initial_states, internal_force, solve, solve_with, tangent, Contribution,
    DriverExtensions, NewtonOptions, NonlinearResult,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidModel};
use std::cell::Cell;

/// Digests of 4abff5fe in the order of the run, from the base run (`f422-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 11]> = Some([
    ("bar Hex8", 0x3068_3d9e_fc11_14e6),
    ("bar Hex20", 0x525a_5745_f0ba_76f9),
    ("cantilever Hex20", 0x8b36_a7da_6105_2e68),
    ("solve_with halving", 0xeb70_2f70_cab2_3dcf),
    ("Tet10 internal_force", 0xc211_1141_be1d_06f3),
    ("Tet10 tangent", 0x26fc_5b2d_b9e4_a16b),
    ("Hex8 element_internal_force", 0x3d65_b45a_1305_b2d0),
    ("Hex8 element_tangent", 0x6466_e959_3d88_928b),
    ("Material::stress isotropic", 0x26fa_ce97_5aa6_7f80),
    ("solve_static cantilever", 0xe157_f0b0_37ae_1575),
    ("Viscoelastic refusal", 0xb63a_b71d_2b5a_b342),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// Every value of a nonlinear result, in a fixed order: displacement, reactions, then per increment the load factor, the
/// halvings, the residuals and the energies, then the state values.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

/// A bar `[1, 0.2, 0.2]` of `[3, 1, 1]` cells, three symmetry planes, stretched by `u_x = 2e-3` at `xmax`.
fn bar(kind: ElementKind) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    if kind == ElementKind::Hex20 {
        mesh = upgrade_to_hex20(&mesh).unwrap().0;
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmax", 0, 2e-3)).unwrap();
    m
}

/// A Hex20 cantilever `[1, 0.05, 0.05]` of `[10, 1, 1]` cells, clamped at `xmin`, a traction along `y` at `xmax`.
fn cantilever() -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.05, 0.05], [10, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 2.0e6, 0.0])).unwrap();
    m
}

/// A zero contribution that refuses its call number `fail_at` (1-based), if any.
struct FailOnce {
    calls: Cell<usize>,
    fail_at: Option<usize>,
}

impl Contribution for FailOnce {
    fn name(&self) -> &str {
        "fail once"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if Some(n) == self.fail_at {
            return Err(FemError::InvalidValue {
                key: "fail once",
                reason: "registered refusal".to_string(),
            });
        }
        Ok(())
    }
}

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

#[test]
fn b0_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    for (name, kind) in [("bar Hex8", ElementKind::Hex8), ("bar Hex20", ElementKind::Hex20)] {
        let m = bar(kind);
        let r = solve(&m, &[], &[], &NewtonOptions::new(2)).unwrap();
        got.push((name, digest(&flat(&r))));
    }
    let c = cantilever();
    let r = solve(&c, &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("cantilever Hex20", digest(&flat(&r))));
    // The halving path: the first attempt refused at its third call.
    let m = bar(ElementKind::Hex8);
    let fail = FailOnce {
        calls: Cell::new(0),
        fail_at: Some(3),
    };
    let ext = DriverExtensions {
        contributions: vec![&fail],
        step_control: None,
    };
    let r = solve_with(&m, &[], &[], &NewtonOptions::new(1), ext).unwrap();
    println!(
        "B0 halvings {:?}",
        r.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
    );
    got.push(("solve_with halving", digest(&flat(&r))));
    // Assembled force and tangent of a distorted Tet10 model.
    let mut mesh = upgrade_to_tet10(&tet_box([0.0; 3], [1.0, 0.5, 0.4], [2, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut seed = 0xF422_0475_u64;
    let nodes: Vec<Vec3> = mesh
        .nodes()
        .iter()
        .map(|p| {
            let mut q = *p;
            for v in q.iter_mut() {
                *v += 0.02 * (2.0 * uniform(&mut seed) - 1.0);
            }
            q
        })
        .collect();
    mesh = {
        let mut m2 = rcfd_fem::SolidMesh::new(nodes).unwrap();
        for b in mesh.blocks() {
            m2.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
        }
        let ne = m2.n_elements();
        m2.add_element_set("all", (0..ne).collect()).unwrap();
        m2
    };
    let mut t = SolidModel::new(mesh);
    t.add_material("all", steel()).unwrap();
    let n = t.dof_map().n_equations();
    let u: Vec<f64> = (0..n).map(|_| 1e-3 * (2.0 * uniform(&mut seed) - 1.0)).collect();
    let states = initial_states(&t).unwrap();
    got.push((
        "Tet10 internal_force",
        digest(&internal_force(&t, &u, &states).unwrap()),
    ));
    got.push(("Tet10 tangent", digest(&tangent(&t, &u, &states, Some(3)).unwrap().val)));
    // One distorted Hex8 element.
    let x: Vec<Vec3> = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ]
    .iter()
    .map(|p| {
        [
            p[0] + 0.1 * uniform(&mut seed),
            p[1] + 0.1 * uniform(&mut seed),
            p[2] + 0.1 * uniform(&mut seed),
        ]
    })
    .collect();
    let ue: Vec<f64> = (0..24).map(|_| 2e-3 * (2.0 * uniform(&mut seed) - 1.0)).collect();
    let el = element_for(ElementKind::Hex8);
    let st = ElementState::empty();
    got.push((
        "Hex8 element_internal_force",
        digest(&element_internal_force(el, &x, &steel(), &ue, &st).unwrap()),
    ));
    got.push((
        "Hex8 element_tangent",
        digest(element_tangent(el, &x, &steel(), &ue, &st).unwrap().values()),
    ));
    // The isotropic constitutive call.
    let (s, d) = steel()
        .stress(&[1e-3, -2e-4, 3e-4, 5e-4, -1e-4, 2e-4], &PointContext::default(), &[])
        .unwrap();
    let mut v = s.to_vec();
    v.extend(d.iter().flatten());
    got.push(("Material::stress isotropic", digest(&v)));
    // The static solve.
    let r = solve_static(&c, &StaticOptions::default()).unwrap();
    got.push(("solve_static cantilever", digest(r.displacement_values())));
    // The refusal of a pending variant.
    let visco = Material::viscoelastic(steel(), vec![PronyTerm::new(0.3, 0.1, 1e-3).unwrap()], None).unwrap();
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut vm2 = SolidModel::new(mesh);
    vm2.add_material("all", visco).unwrap();
    vm2.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    vm2.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    vm2.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    vm2.add_constraint(Constraint::displacement("xmax", 0, 2e-3)).unwrap();
    let msg = format!(
        "{:?} / {:?}",
        solve(&vm2, &[], &[], &NewtonOptions::new(1)).err(),
        solve_static(&vm2, &StaticOptions::default()).err()
    );
    println!("B0 refusal: {msg}");
    got.push(("Viscoelastic refusal", digest_bytes(msg.bytes())));
    for (name, d) in &got {
        println!("B0 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B0 {name}: {d:#018x} against {e:#018x}");
    }
}
