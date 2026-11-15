//! F-685 (ADR 0614): the bit record B1 (D6). The Incompatible Hex8 paths of the nonlinear driver (ADR 0608) of a model that
//! does not opt in to the stabilisation give the bits they gave before F-685. Uses only API that existed at a9efc22e; the
//! expected digests were printed by this file run on the unchanged base a9efc22e (`f685-scratch/b1-base.log`) and written
//! into `EXPECTED` before any library code of F-685.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{enhanced_force_and_tangent, modes_and_strains};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{hex8_force_and_tangent_at, initial_states, internal_force, solve, tangent, NewtonOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;

/// Digests of a9efc22e in the order of the run (`f685-scratch/b1-base.log`); written after it.
const EXPECTED: Option<[(&str, u64); 8]> = Some([
    ("c1 patch Incompatible tangent at 0.6", 0xef09_d43e_801b_008b),
    ("c1 patch Full tangent at 0.6", 0x5cb6_8eb8_9486_d9da),
    ("ADR 0608 Q1 block Incompatible solve", 0x13e4_b296_f155_b725),
    (
        "ADR 0608 CB2 cantilever 25 x 1 x 1 alpha 1 solve",
        0x55fb_06a7_1181_49e4,
    ),
    ("T1 (b) modes_and_strains", 0xc84f_95da_f97e_99cc),
    ("T1 (b) enhanced_force_and_tangent", 0x1497_fd0c_b30a_c03f),
    ("T1 (b) hex8_force_and_tangent_at Incompatible", 0xcf17_25df_4877_ce84),
    ("ADR 0608 NT1 bar nl_transient Incompatible", 0xf143_bd47_2aac_31d3),
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

// --------------------------------------------------------------------------------------------- the patch of ADR 0614 c1

const MU: f64 = 1.0e6;

fn neo_hookean() -> Material {
    let nu = 0.3;
    let kappa = 2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa).unwrap(), 1000.0).unwrap()
}

/// The c1 patch: the unit cube on `n^3` Hex8, interior nodes moved by `0.1 h (2U - 1)` (seed `0xF685_0614`).
fn patch(n: usize, f: SolidFormulation) -> SolidModel {
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
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), f));
    m.add_material("all", neo_hookean()).unwrap();
    m
}

/// `S_11` of `diag(t, t, lambda)` for the material.
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

/// The lateral stretch of uniaxial stress: 200 bisections of `S_11` on `[1, 2]`.
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

/// The homogeneous field `u = (F - I) X` at every node.
fn homogeneous(m: &SolidModel, lambda: f64) -> Vec<f64> {
    let t = lateral(&neo_hookean(), lambda);
    let st = [t - 1.0, t - 1.0, lambda - 1.0];
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            u[dofs.equation(n, c).unwrap()] = st[c] * x[c];
        }
    }
    u
}

// ------------------------------------------------------------------------------------- the ADR 0608 set-ups (copies)

fn block() -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [3, 3, 3]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng(0xF596_0608);
    let h = 1.0 / 3.0;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), INC));
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    for (c, v) in [0.3, 0.0, -0.1].into_iter().enumerate() {
        m.add_constraint(Constraint::displacement("zmax", c, v)).unwrap();
    }
    m
}

fn cantilever(n: usize, alpha: f64) -> SolidModel {
    let (l, h, e) = (1.0, 0.02, 2.0e11);
    let mesh = with_formulation(hex_box([0.0; 3], [l, h, h], [n, 1, 1]).unwrap(), INC);
    let mut m = SolidModel::new(mesh);
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
    let lambda = 0.6;
    for (name, f) in [
        ("c1 patch Incompatible tangent at 0.6", INC),
        ("c1 patch Full tangent at 0.6", SolidFormulation::Full),
    ] {
        let m = patch(6, f);
        let u = homogeneous(&m, lambda);
        let states = initial_states(&m).unwrap();
        let k = tangent(&m, &u, &states, Some(4)).unwrap();
        let mut v = k.val.clone();
        v.extend(k.row_ptr.iter().map(|&p| p as f64));
        v.extend(k.col.iter().map(|&p| p as f64));
        v.extend(internal_force(&m, &u, &states).unwrap());
        out.push((name, digest(&v)));
    }
    let m = block();
    let r = solve(&m, &[], &[], &NewtonOptions::new(4).threads(4)).unwrap();
    let mut v = r.result.displacement_values().to_vec();
    for s in &r.steps {
        v.extend_from_slice(&s.residuals);
    }
    out.push(("ADR 0608 Q1 block Incompatible solve", digest(&v)));
    let m = cantilever(25, 1.0);
    let r = solve(&m, &[], &[], &NewtonOptions::new(5).threads(4)).unwrap();
    let mut v = r.result.displacement_values().to_vec();
    for s in &r.steps {
        v.extend_from_slice(&s.residuals);
    }
    out.push(("ADR 0608 CB2 cantilever 25 x 1 x 1 alpha 1 solve", digest(&v)));
    let x = distorted();
    let u = t1b_displacement(&x);
    let mat = steel(0.3);
    let ctx = PointContext::default();
    let (alpha, strains) = modes_and_strains(&x, &mat, &u, &empty(), &ctx).unwrap();
    let mut v = alpha.to_vec();
    for s in &strains {
        v.extend_from_slice(s);
    }
    out.push(("T1 (b) modes_and_strains", digest(&v)));
    let (f, k) = enhanced_force_and_tangent(&x, &mat, &u, &alpha, &empty(), &ctx).unwrap();
    let mut v = f;
    v.extend_from_slice(k.values());
    out.push(("T1 (b) enhanced_force_and_tangent", digest(&v)));
    let (f, k) = hex8_force_and_tangent_at(INC, &x, &mat, &u, &empty(), &ctx).unwrap();
    let mut v = f;
    v.extend_from_slice(k.values());
    out.push(("T1 (b) hex8_force_and_tangent_at Incompatible", digest(&v)));
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(),
        INC,
    ));
    m.add_material("all", mat.clone()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let n = m.dof_map().n_equations();
    let mut pattern = vec![0.0; n];
    for &nd in m.mesh().node_set("xmax").unwrap() {
        pattern[m.dof_map().equation(nd, 1).unwrap()] = 1.0e8;
    }
    let loads = [LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())];
    let mut it = Integrator::new(&m, &loads, &NlTransientOptions::new(1e-5).threads(4)).unwrap();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it.run(3).unwrap();
    let mut v = it.displacement().unwrap();
    v.push(it.strain_energy().unwrap());
    out.push(("ADR 0608 NT1 bar nl_transient Incompatible", digest(&v)));
    out
}

#[test]
fn b1_incompatible_paths_without_the_opt_in_keep_their_bits() {
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
    let expected = EXPECTED.expect("B1: the expected digests are written after the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(*d, *ed, "B1 {name}: digest changed");
    }
}
