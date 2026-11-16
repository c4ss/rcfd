//! F-703 (ADR 0630): the bit record B3 (D7). The Incompatible Hex8 of a model opted in with
//! `set_incompatible_stabilisation(true)` (the ADR 0614 coefficient) and the element functions called with `true` give the
//! bits they gave before F-703. Uses only API that existed at 3f9ce17a; the expected digests were printed by this file run on
//! the unchanged base 3f9ce17a (`f703-scratch/b3-base.log`) and written into `EXPECTED` before any library code of F-703.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, enhanced_force_and_tangent_with, modes_and_strains_with, stabilisation_coefficient,
    stabilisation_energy,
};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, internal_force, solve, tangent, NewtonOptions};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;

/// Digests of 3f9ce17a in the order of the run (`f703-scratch/b3-base.log`); written after it.
const EXPECTED: Option<[(&str, u64); 10]> = Some([
    (
        "opted 6^3 patch nu 0.3 U30 0.6 tangent and force",
        0x8dfb_6def_f559_f6c5,
    ),
    (
        "opted 4^3 patch nu 0.49 C49 0.5 tangent and force",
        0xd81a_4d0c_aab5_435b,
    ),
    ("T1 (b) condensed_force_and_tangent stabilised", 0x3d07_1521_bbb3_31b2),
    ("T1 (b) modes_and_strains_with stabilised", 0x6fe1_b059_13cd_97cd),
    (
        "T1 (b) enhanced_force_and_tangent_with stabilised",
        0x7ac4_f307_4416_7a39,
    ),
    ("T1 (b) stabilisation_coefficient", 0xc082_e47f_2eac_b157),
    ("T1 (b) stabilisation_energy", 0x03ae_14af_f4e5_8d85),
    ("opted CB2 cantilever 25 x 1 x 1 alpha 1 solve", 0xf8f3_0439_1236_9867),
    ("opted 4^3 patch E state strain_energy", 0xaebe_4e6e_424f_4544),
    ("opted 4^3 patch E state internal_force", 0x5579_aa4f_2c6e_bb58),
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

/// SplitMix64 from a seed.
struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 1.2e-5).unwrap()
}

fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

fn empty() -> ElementState {
    ElementState::new(Vec::new())
}

// --------------------------------------------------------------------------------------------------------- the set-ups

const MU: f64 = 1.0e6;

fn neo_hookean(nu: f64) -> Material {
    let kappa = 2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa).unwrap(), 1000.0).unwrap()
}

/// The ADR 0614 c1 patch on `n^3` Hex8 (seed `0xF685_0614`), Incompatible, opted in.
fn patch(n: usize, nu: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng(0xF685_0614);
    let h = 1.0 / n as f64;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), INC));
    m.set_incompatible_stabilisation(true);
    m.add_material("all", neo_hookean(nu)).unwrap();
    m
}

fn s11(mat: &Material, t: f64, lambda: f64) -> f64 {
    let e = [
        0.5 * (t * t - 1.0),
        0.5 * (t * t - 1.0),
        0.5 * (lambda * lambda - 1.0),
        0.0,
        0.0,
        0.0,
    ];
    mat.stress(&e, &PointContext::default(), &[]).unwrap().0[0]
}

fn lateral(mat: &Material, lambda: f64) -> f64 {
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if s11(mat, mid, lambda) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// `u = (F - I) X + beta (x z, y z, 0)` with `F = diag(t, t, lambda)`.
fn field(m: &SolidModel, t: f64, lambda: f64, beta: f64) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        let v = [
            (t - 1.0) * x[0] + beta * x[0] * x[2],
            (t - 1.0) * x[1] + beta * x[1] * x[2],
            (lambda - 1.0) * x[2],
        ];
        for c in 0..3 {
            u[dofs.equation(n, c).unwrap()] = v[c];
        }
    }
    u
}

fn cantilever(n: usize, alpha: f64) -> SolidModel {
    let (l, h, e) = (1.0, 0.02, 2.0e11);
    let mesh = with_formulation(hex_box([0.0; 3], [l, h, h], [n, 1, 1]).unwrap(), INC);
    let mut m = SolidModel::new(mesh);
    m.set_incompatible_stabilisation(true);
    m.add_material("all", Material::isotropic_elastic(e, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let p = alpha * e * (h * h.powi(3) / 12.0) / (l * l);
    m.add_load(Load::traction("xmax", [0.0, p / (h * h), 0.0])).unwrap();
    m
}

/// The distorted unit Hex8 of `si_fbar.rs` (seed `0xF587_0506`).
fn distorted() -> Vec<Vec3> {
    let mut rng = Rng(0xF587_0506);
    let unit: [Vec3; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    unit.iter()
        .map(|p| [p[0] + rng.sym(0.1), p[1] + rng.sym(0.1), p[2] + rng.sym(0.1)])
        .collect()
}

/// The displacement of ADR 0608 T1 (b) (the RNG drawn as there: (a) first).
fn t1b_displacement(x: &[Vec3]) -> Vec<f64> {
    let mut rng = Rng(0xF596_0608 + 1);
    let _a: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    x.iter()
        .flat_map(|p| [-p[1] - p[0], p[0] - p[1], 0.0])
        .map(|v| v + rng.sym(5e-2))
        .collect()
}

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let t = lateral(&neo_hookean(0.3), 0.6);
    for (name, n, nu, t, lambda) in [
        ("opted 6^3 patch nu 0.3 U30 0.6 tangent and force", 6, 0.3, t, 0.6),
        ("opted 4^3 patch nu 0.49 C49 0.5 tangent and force", 4, 0.49, 1.0, 0.5),
    ] {
        let m = patch(n, nu);
        let u = field(&m, t, lambda, 0.0);
        let states = initial_states(&m).unwrap();
        let k = tangent(&m, &u, &states, Some(4)).unwrap();
        let mut v = k.val.clone();
        v.extend(k.row_ptr.iter().map(|&p| p as f64));
        v.extend(k.col.iter().map(|&p| p as f64));
        v.extend(internal_force(&m, &u, &states).unwrap());
        out.push((name, digest(&v)));
    }
    let x = distorted();
    let u = t1b_displacement(&x);
    let mat = steel(0.3);
    let ctx = PointContext::default();
    let st = empty();
    let (f, k, s) = condensed_force_and_tangent(&x, &mat, &u, &st, &ctx, true).unwrap();
    let mut v = f;
    v.extend_from_slice(k.values());
    for sq in &s {
        v.extend_from_slice(sq);
    }
    out.push(("T1 (b) condensed_force_and_tangent stabilised", digest(&v)));
    let (alpha, strains) = modes_and_strains_with(&x, &mat, &u, &st, &ctx, true).unwrap();
    let mut v = alpha.to_vec();
    for s in &strains {
        v.extend_from_slice(s);
    }
    out.push(("T1 (b) modes_and_strains_with stabilised", digest(&v)));
    let (f, k) = enhanced_force_and_tangent_with(&x, &mat, &u, &alpha, &st, &ctx, true).unwrap();
    let mut v = f;
    v.extend_from_slice(k.values());
    out.push(("T1 (b) enhanced_force_and_tangent_with stabilised", digest(&v)));
    let c = stabilisation_coefficient(&x, &mat, &u, &ctx).unwrap();
    out.push(("T1 (b) stabilisation_coefficient", digest(&[c])));
    let e = stabilisation_energy(&x, &mat, &u, &alpha, &ctx).unwrap();
    out.push(("T1 (b) stabilisation_energy", digest(&[e])));
    let m = cantilever(25, 1.0);
    let r = solve(&m, &[], &[], &NewtonOptions::new(5).threads(4)).unwrap();
    let mut v = r.result.displacement_values().to_vec();
    for s in &r.steps {
        v.extend_from_slice(&s.residuals);
    }
    out.push(("opted CB2 cantilever 25 x 1 x 1 alpha 1 solve", digest(&v)));
    let m = patch(4, 0.3);
    let u = field(&m, 1.0, 0.6, 0.2);
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(1e-5).threads(4)).unwrap();
    it.start(0.0, &u, &vec![0.0; u.len()]).unwrap();
    let w = it.strain_energy().unwrap();
    out.push(("opted 4^3 patch E state strain_energy", digest(&[w])));
    let f = internal_force(&m, &u, &initial_states(&m).unwrap()).unwrap();
    out.push(("opted 4^3 patch E state internal_force", digest(&f)));
    out
}

#[test]
fn b3_the_adr_0614_coefficient_keeps_its_bits() {
    let got = digests();
    for (name, d) in &got {
        println!(
            "(\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    let expected = EXPECTED.expect("B3: the expected digests are written after the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(*d, *ed, "B3 {name}: digest changed");
    }
}
