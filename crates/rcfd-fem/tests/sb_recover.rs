//! F-376 (ADR 0277): strain and stress recovery, nodal values, von Mises and principal stresses, strain energy. Checks P1,
//! P1t, P2 to P5, V1 to V5, R1, R2, S1 to S3, W1, W2, W4, X1 to X5, A1, E1 and the negative controls N1 to N3 of the
//! registration (seed 0xF376_0277; the patch test on the meshes of ADR 0275, seed 0xF374_0275).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::{apply, isotropic_elasticity};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::quadrature::gauss_3;
use rcfd_fem::recover::{
    attach, element_nodal_values, nodal_average, principal, principal_values_trigonometric, tensor_norm, von_mises,
    IntegrationPointField,
};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::eigen::symmetric_eigen;
use std::collections::BTreeSet;

const SEED: u64 = 0xF376_0277;
const PATCH_SEED: u64 = 0xF374_0275;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;

/// SplitMix64, a deterministic source only (ADR 0273 D6, ADR 0277 D5).
struct Rng(u64);

impl Rng {
    fn with(seed: u64, tag: u64) -> Rng {
        Rng(seed.wrapping_add(tag))
    }
    fn new(tag: u64) -> Rng {
        Rng::with(SEED, tag)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
    /// A value uniform in [-1, 1] times 10^k, k uniform in [-6, 9].
    fn scaled(&mut self) -> f64 {
        let k = -6.0 + 15.0 * self.uniform();
        self.sym(1.0) * 10f64.powf(k)
    }
    fn tensor(&mut self) -> [f64; 6] {
        let k = -6.0 + 15.0 * self.uniform();
        let s = 10f64.powf(k);
        [0; 6].map(|_| self.sym(1.0) * s)
    }
    fn rotation(&mut self) -> [[f64; 3]; 3] {
        let q = loop {
            let q = [self.sym(1.0), self.sym(1.0), self.sym(1.0), self.sym(1.0)];
            let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
            if n > 0.1 && n <= 1.0 {
                break q.map(|v| v / n);
            }
        };
        let [w, x, y, z] = q;
        [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
            [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
            [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
        ]
    }
}

fn to_mat(s: &[f64; 6]) -> [[f64; 3]; 3] {
    [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]]
}

fn to_voigt(m: &[[f64; 3]; 3]) -> [f64; 6] {
    [
        m[0][0],
        m[1][1],
        m[2][2],
        0.5 * (m[0][1] + m[1][0]),
        0.5 * (m[1][2] + m[2][1]),
        0.5 * (m[2][0] + m[0][2]),
    ]
}

/// `R S R^T` in floating point.
fn rotate(s: &[f64; 6], r: &[[f64; 3]; 3]) -> [f64; 6] {
    let a = to_mat(s);
    let mut t = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut v = 0.0;
            for k in 0..3 {
                for l in 0..3 {
                    v += r[i][k] * a[k][l] * r[j][l];
                }
            }
            t[i][j] = v;
        }
    }
    to_voigt(&t)
}

fn jacobi_descending(s: &[f64; 6]) -> [f64; 3] {
    let a = to_mat(s);
    let flat: Vec<f64> = a.iter().flatten().copied().collect();
    let e = symmetric_eigen(&flat, 3).unwrap();
    [e.values[2], e.values[1], e.values[0]]
}

fn max_diff3(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max)
}

fn orthonormality(d: &[Vec3; 3]) -> f64 {
    let mut worst: f64 = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            let p: f64 = (0..3).map(|k| d[i][k] * d[j][k]).sum();
            let target = if i == j { 1.0 } else { 0.0 };
            worst = worst.max((p - target).abs());
        }
    }
    worst
}

fn residual(s: &[f64; 6], l: f64, d: Vec3) -> f64 {
    let a = to_mat(s);
    (0..3)
        .map(|i| {
            let ad: f64 = (0..3).map(|k| a[i][k] * d[k]).sum();
            (ad - l * d[i]).powi(2)
        })
        .sum::<f64>()
        .sqrt()
}

// ------------------------------------------------------------------------------------------------------- random tensors

#[test]
fn p1_to_p5_v5_random_tensors_and_n3() {
    let mut tensors = Rng::new(1);
    let mut rotations = Rng::new(2);
    let (mut p1, mut p1t, mut p3, mut p4, mut p5, mut v5, mut n3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, f64::INFINITY);
    let mut ordered = 0;
    for _ in 0..1000 {
        let s = tensors.tensor();
        let r = rotations.rotation();
        let n = tensor_norm(&s);
        let pr = principal(&s);
        let jac = jacobi_descending(&s);
        p1 = p1.max(max_diff3(&pr.values, &jac) / n);
        p1t = p1t.max(max_diff3(&principal_values_trigonometric(&s), &jac) / n);
        if pr.values[0] >= pr.values[1] && pr.values[1] >= pr.values[2] {
            ordered += 1;
        }
        p3 = p3.max(orthonormality(&pr.directions));
        let rotated = principal(&rotate(&s, &r));
        p4 = p4.max(max_diff3(&rotated.values, &pr.values) / n);
        for k in 0..3 {
            p5 = p5.max(residual(&s, pr.values[k], pr.directions[k]) / n);
        }
        let [l1, l2, l3] = pr.values;
        let vm_p = (0.5 * ((l1 - l2).powi(2) + (l2 - l3).powi(2) + (l3 - l1).powi(2))).sqrt();
        v5 = v5.max((von_mises(&s) - vm_p).abs() / n);
        let ascending = [jac[2], jac[1], jac[0]];
        n3 = n3.min(max_diff3(&pr.values, &ascending) / n);
    }
    println!("P1 routine vs Jacobi: {p1:.3e} (limit 1e-12)");
    println!("P1t closed trigonometric form vs Jacobi: {p1t:.3e} (limit 1e-12)");
    println!("P2 ordered: {ordered} of 1000");
    println!("P3 orthonormality: {p3:.3e} (limit 1e-12)");
    println!("P4 rotation invariance: {p4:.3e} (limit 1e-12)");
    println!("P5 eigen residual: {p5:.3e} (limit 1e-12)");
    println!("V5 von Mises vs principal values: {v5:.3e} (limit 1e-12)");
    println!("N3 ascending Jacobi vs descending routine (min over tensors): {n3:.3e} (expected > 1e-3)");
    assert!(p1 <= 1e-12, "P1");
    assert!(p1t <= 1e-12, "P1t");
    assert_eq!(ordered, 1000, "P2");
    assert!(p3 <= 1e-12, "P3");
    assert!(p4 <= 1e-12, "P4");
    assert!(p5 <= 1e-12, "P5");
    assert!(v5 <= 1e-12, "V5");
    assert!(n3 > 1e-3, "N3");
}

// ------------------------------------------------------------------------------------------------------------ von Mises

#[test]
fn v1_to_v4_von_mises() {
    let mut rng = Rng::new(3);
    let values: Vec<f64> = (0..100).map(|_| rng.scaled()).collect();
    let (mut v1, mut v2, mut v3, mut v4) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &x in &values {
        for axis in 0..3 {
            let mut s = [0.0; 6];
            s[axis] = x;
            v1 = v1.max((von_mises(&s) - x.abs()).abs() / tensor_norm(&s));
            let mut t = [0.0; 6];
            t[3 + axis] = x;
            v2 = v2.max((von_mises(&t) - 3f64.sqrt() * x.abs()).abs() / tensor_norm(&t));
        }
        let h = [x, x, x, 0.0, 0.0, 0.0];
        v3 = v3.max(von_mises(&h) / tensor_norm(&h));
        let r = rng.rotation();
        let s = rotate(&[x, 0.0, 0.0, 0.0, 0.0, 0.0], &r);
        v4 = v4.max((von_mises(&s) - x.abs()).abs() / tensor_norm(&s));
    }
    println!("V1 uniaxial: {v1:.3e} (limit 1e-14)");
    println!("V2 pure shear: {v2:.3e} (limit 1e-14)");
    println!("V3 hydrostatic: {v3:.3e} (limit 1e-14)");
    println!("V4 rotated uniaxial: {v4:.3e} (limit 1e-13)");
    assert!(v1 <= 1e-14, "V1");
    assert!(v2 <= 1e-14, "V2");
    assert!(v3 <= 1e-14, "V3");
    assert!(v4 <= 1e-13, "V4");
}

// ------------------------------------------------------------------------------------------------------- repeated roots

/// The 54 repeated-root tensors of ADR 0277 D5 with their exact roots, descending.
fn repeated_root_tensors() -> Vec<(&'static str, [f64; 6], [f64; 3])> {
    let mut rng = Rng::new(4);
    let mut out: Vec<(&'static str, [f64; 6], [f64; 3])> = Vec::new();
    let desc = |mut v: [f64; 3]| {
        v.sort_by(|a, b| b.total_cmp(a));
        v
    };
    out.push(("zero", [0.0; 6], [0.0; 3]));
    for p in [3.7e5, -2.0] {
        out.push(("hydrostatic", [p, p, p, 0.0, 0.0, 0.0], [p; 3]));
    }
    for s in [2.5e8, -1.0] {
        out.push(("uniaxial", [s, 0.0, 0.0, 0.0, 0.0, 0.0], desc([s, 0.0, 0.0])));
    }
    for (a, b) in [(1e8, -3e8), (1e8, 2e8)] {
        let d = [a, a, b, 0.0, 0.0, 0.0];
        out.push(("double", d, desc([a, a, b])));
        for _ in 0..10 {
            let r = rng.rotation();
            out.push(("double rotated", rotate(&d, &r), desc([a, a, b])));
        }
    }
    let p = 3.7e5;
    for _ in 0..10 {
        let r = rng.rotation();
        out.push(("hydrostatic rotated", rotate(&[p, p, p, 0.0, 0.0, 0.0], &r), [p; 3]));
    }
    for d in [1e-9, 1e-15] {
        let (a, b) = (1e8, -3e8);
        let t = [a, a * (1.0 + d), b, 0.0, 0.0, 0.0];
        for _ in 0..10 {
            let r = rng.rotation();
            out.push(("near-repeated rotated", rotate(&t, &r), desc([a, a * (1.0 + d), b])));
        }
    }
    out
}

/// Record of the defect of ADR 0277 amendment 1: the registered total 54 was mis-added (the list has 57 tensors); this
/// test stops at the count before any measurement. Replaced by `r1_r2_repeated_roots_57`.
#[test]
#[ignore]
fn r1_r2_repeated_roots() {
    let tensors = repeated_root_tensors();
    assert_eq!(tensors.len(), 54);
    let (mut orth, mut err, mut res, mut r2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut all_finite = true;
    let mut all_ordered = true;
    let mut zero_exact = true;
    let mut worst_kind = "";
    for (name, s, exact) in &tensors {
        let pr = principal(s);
        let n = tensor_norm(s).max(f64::MIN_POSITIVE);
        all_finite &= pr.values.iter().all(|v| v.is_finite()) && pr.directions.iter().flatten().all(|v| v.is_finite());
        all_ordered &= pr.values[0] >= pr.values[1] && pr.values[1] >= pr.values[2];
        orth = orth.max(orthonormality(&pr.directions));
        let e = if tensor_norm(s) == 0.0 {
            zero_exact &= pr.values == [0.0; 3];
            0.0
        } else {
            max_diff3(&pr.values, exact) / n
        };
        if e > err {
            err = e;
            worst_kind = name;
        }
        for k in 0..3 {
            res = res.max(residual(s, pr.values[k], pr.directions[k]) / n);
        }
        let t = principal_values_trigonometric(s);
        if tensor_norm(s) > 0.0 {
            r2 = r2.max(max_diff3(&t, exact) / n);
        }
    }
    println!("R1 finite: {all_finite}, ordered: {all_ordered}, zero tensor exact: {zero_exact}");
    println!("R1 orthonormality: {orth:.3e} (limit 1e-12)");
    println!("R1 values against the exact roots: {err:.3e} (limit 1e-12; worst: {worst_kind})");
    println!("R1 eigen residual (record): {res:.3e}");
    println!("R2 bare closed form against the exact roots (record): {r2:.3e}");
    assert!(all_finite, "R1 finite");
    assert!(all_ordered, "R1 ordered");
    assert!(zero_exact, "R1 zero");
    assert!(orth <= 1e-12, "R1 orthonormality");
    assert!(err <= 1e-12, "R1 values");
}

/// R1 and R2 as registered, with the count of the list corrected to 57 (ADR 0277 amendment 1).
#[test]
fn r1_r2_repeated_roots_57() {
    let tensors = repeated_root_tensors();
    assert_eq!(tensors.len(), 57);
    let (mut orth, mut err, mut res, mut r2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut all_finite = true;
    let mut all_ordered = true;
    let mut zero_exact = true;
    let mut worst_kind = "";
    for (name, s, exact) in &tensors {
        let pr = principal(s);
        let n = tensor_norm(s).max(f64::MIN_POSITIVE);
        all_finite &= pr.values.iter().all(|v| v.is_finite()) && pr.directions.iter().flatten().all(|v| v.is_finite());
        all_ordered &= pr.values[0] >= pr.values[1] && pr.values[1] >= pr.values[2];
        orth = orth.max(orthonormality(&pr.directions));
        let e = if tensor_norm(s) == 0.0 {
            zero_exact &= pr.values == [0.0; 3];
            0.0
        } else {
            max_diff3(&pr.values, exact) / n
        };
        if e > err {
            err = e;
            worst_kind = name;
        }
        for k in 0..3 {
            res = res.max(residual(s, pr.values[k], pr.directions[k]) / n);
        }
        let t = principal_values_trigonometric(s);
        if tensor_norm(s) > 0.0 {
            r2 = r2.max(max_diff3(&t, exact) / n);
        }
    }
    println!("R1 finite: {all_finite}, ordered: {all_ordered}, zero tensor exact: {zero_exact}");
    println!("R1 orthonormality: {orth:.3e} (limit 1e-12)");
    println!("R1 values against the exact roots: {err:.3e} (limit 1e-12; worst: {worst_kind})");
    println!("R1 eigen residual (record): {res:.3e}");
    println!("R2 bare closed form against the exact roots (record): {r2:.3e}");
    assert!(all_finite, "R1 finite");
    assert!(all_ordered, "R1 ordered");
    assert!(zero_exact, "R1 zero");
    assert!(orth <= 1e-12, "R1 orthonormality");
    assert!(err <= 1e-12, "R1 values");
}

// ---------------------------------------------------------------------------------------------------------------- meshes

fn material_e(e: f64) -> Material {
    Material::isotropic_elastic(e, NU, 7850.0, 0.0).unwrap()
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

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

/// A box of the kind (Tet10: the Tet4 box upgraded, straight edges); with `perturb = (seed, tag)` the interior grid nodes
/// move by `0.1 h_k (2U - 1)` per coordinate before the upgrade (ADR 0275 arithmetic 6 and D8).
fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<(u64, u64)>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some((seed, tag)) => {
            let mut rng = Rng::with(seed, tag);
            let boundary = boundary_nodes(&base);
            let h = [0, 1, 2].map(|k| lengths[k] / div[k] as f64);
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material_e(E_MOD)).unwrap();
    m
}

fn node_near(mesh: &SolidMesh, x: Vec3) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (i, p) in mesh.nodes().iter().enumerate() {
        let d = (0..3).map(|k| (p[k] - x[k]).powi(2)).sum::<f64>();
        if d < best.0 {
            best = (d, i);
        }
    }
    assert!(best.0 < 1e-24, "no node at {x:?}");
    best.1
}

fn max_rel6(a: &[f64; 6], b: &[f64; 6], scale: f64) -> f64 {
    (0..6).map(|c| (a[c] - b[c]).abs()).fold(0.0, f64::max) / scale
}

fn max_abs6(a: &[f64; 6]) -> f64 {
    a.iter().fold(0.0f64, |m, v| m.max(v.abs()))
}

// ------------------------------------------------------------------------------------------------------------ patch test

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const P_DIV: [usize; 3] = [4, 3, 3];
const FIELD_C: Vec3 = [1e-3, -2e-3, 0.5e-3];
const FIELD_A: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, -0.2e-3],
    [0.5e-3, -0.7e-3, 0.4e-3],
    [0.1e-3, 0.6e-3, 0.9e-3],
];

fn linear_field(x: Vec3) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += FIELD_A[i][j] * x[j];
        }
    }
    u
}

fn exact_strain() -> [f64; 6] {
    let a = FIELD_A;
    [a[0][0], a[1][1], a[2][2], a[0][1] + a[1][0], a[1][2] + a[2][1], a[2][0] + a[0][2]]
}

/// The patch test of ADR 0275 (its meshes, field and boundary values), solved.
fn patch(kind: ElementKind) -> (SolidModel, SolidResult) {
    let mesh = box_of(kind, P_ORIGIN, P_LENGTHS, P_DIV, Some((PATCH_SEED, 10 + kind_index(kind))));
    let mut pres = Vec::new();
    for &n in &boundary_nodes(&mesh) {
        for (k, &uk) in linear_field(mesh.nodes()[n]).iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    let model = model_of(mesh);
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    (model, res)
}

#[test]
fn s1_to_s3_patch_a1_and_n1() {
    let eps = exact_strain();
    let sig = apply(&isotropic_elasticity(E_MOD, NU), &eps);
    let sig_other = apply(&isotropic_elasticity(E_MOD, 0.25), &eps);
    let (es, ss) = (max_abs6(&eps), max_abs6(&sig));
    for kind in KINDS {
        let (model, res) = patch(kind);
        let field = IntegrationPointField::from_result(&model, &res).unwrap();
        let mut s1 = (0.0f64, 0.0f64);
        let mut n1 = f64::INFINITY;
        for q in 0..field.n_points() {
            s1.0 = s1.0.max(max_rel6(&field.stress()[q], &sig, ss));
            s1.1 = s1.1.max(max_rel6(&field.strain()[q], &eps, es));
            n1 = n1.min(max_rel6(&field.stress()[q], &sig_other, ss));
        }
        let mut s2 = (0.0f64, 0.0f64);
        for (ms, me) in field
            .element_means(field.stress())
            .unwrap()
            .iter()
            .zip(field.element_means(field.strain()).unwrap())
        {
            s2.0 = s2.0.max(max_rel6(ms, &sig, ss));
            s2.1 = s2.1.max(max_rel6(&me, &eps, es));
        }
        let ns = nodal_average(&model, &field, field.stress(), None).unwrap();
        let ne = nodal_average(&model, &field, field.strain(), None).unwrap();
        let mut s3 = (0.0f64, 0.0f64);
        let mut covered = true;
        for (a, b) in ns.iter().zip(&ne) {
            match (a, b) {
                (Some(a), Some(b)) => {
                    s3.0 = s3.0.max(max_rel6(a, &sig, ss));
                    s3.1 = s3.1.max(max_rel6(b, &eps, es));
                }
                _ => covered = false,
            }
        }
        let attached = attach(res.clone(), &field).unwrap();
        let bits = |v: &[[f64; 6]]| v.iter().flatten().map(|x| x.to_bits()).collect::<Vec<_>>();
        let a1 = bits(attached.strain()) == bits(field.strain())
            && bits(attached.stress()) == bits(field.stress())
            && attached.integration_point_offsets() == field.offsets();
        println!(
            "{}: S1 points stress {:.3e} strain {:.3e}; S2 element means {:.3e} / {:.3e}; S3 nodal averages {:.3e} / {:.3e} \
             (all nodes: {covered}); A1 attach bit-equal: {a1}; N1 other D: {n1:.3e} ({} points)",
            kind.name(),
            s1.0,
            s1.1,
            s2.0,
            s2.1,
            s3.0,
            s3.1,
            field.n_points()
        );
        assert!(s1.0 <= 1e-10 && s1.1 <= 1e-10, "S1");
        assert!(s2.0 <= 1e-10 && s2.1 <= 1e-10, "S2");
        assert!(s3.0 <= 1e-10 && s3.1 <= 1e-10 && covered, "S3");
        assert!(a1, "A1");
        assert!(n1 > 1e-3, "N1");
    }
}

// --------------------------------------------------------------------------------------------------------- strain energy

const SIGMA: f64 = 5e6;

/// Uniaxial bar of ADR 0275 arithmetic 10 with end tractions and the 3-2-1 support.
fn uniaxial(kind: ElementKind) -> (SolidModel, SolidResult) {
    let mesh = box_of(kind, [0.0; 3], [2.0, 0.5, 0.4], [6, 2, 2], None);
    let p1 = node_near(&mesh, [0.0, 0.0, 0.0]);
    let p2 = node_near(&mesh, [0.0, 0.0, 0.4]);
    let p3 = node_near(&mesh, [0.0, 0.5, 0.0]);
    let pres = vec![
        Prescribed::new(p1, 0, 0.0),
        Prescribed::new(p1, 1, 0.0),
        Prescribed::new(p1, 2, 0.0),
        Prescribed::new(p2, 0, 0.0),
        Prescribed::new(p2, 1, 0.0),
        Prescribed::new(p3, 0, 0.0),
    ];
    let mut model = model_of(mesh);
    model.add_load(Load::traction("xmax", [SIGMA, 0.0, 0.0])).unwrap();
    model.add_load(Load::traction("xmin", [-SIGMA, 0.0, 0.0])).unwrap();
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    (model, res)
}

/// Cantilever of ADR 0277 arithmetic 14: clamped at x = 0, tip traction on x = 2.
fn cantilever(kind: ElementKind) -> (SolidModel, SolidResult) {
    let mesh = box_of(kind, [0.0; 3], [2.0, 0.2, 0.2], [10, 2, 2], None);
    let mut model = model_of(mesh);
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::traction("xmax", [0.0, 0.0, -1e6])).unwrap();
    let res = solve_static(&model, &StaticOptions::default()).unwrap();
    (model, res)
}

#[test]
fn w1_w2_w4_strain_energy_and_n2() {
    for kind in KINDS {
        for (case, (model, res)) in [("uniaxial", uniaxial(kind)), ("bending", cantilever(kind))] {
            let field = IntegrationPointField::from_result(&model, &res).unwrap();
            let u_energy = field.strain_energy();
            let f = nodal_load_vector(&model, &[]).unwrap();
            let u = res.displacement_values();
            let work: f64 = f.iter().zip(u).map(|(a, b)| a * b).sum();
            let w1 = (u_energy - work / 2.0).abs() / (work / 2.0);
            let n2 = (u_energy - work).abs() / work;
            let k = stiffness(&model).unwrap();
            let mut ku = vec![0.0; u.len()];
            k.matvec(u, &mut ku);
            let utku: f64 = u.iter().zip(&ku).map(|(a, b)| a * b).sum();
            let w4 = (u_energy - utku / 2.0).abs() / (utku / 2.0);
            let elements: f64 = field.element_strain_energy().iter().sum();
            assert_eq!(elements.to_bits(), u_energy.to_bits());
            let mut line = format!(
                "{} {case}: U = {u_energy:.6e} J, W/2 = {:.6e} J; W1 {w1:.3e} (limit 1e-10); W4 {w4:.3e} (limit 1e-10); \
                 N2 {n2:.3e} (expected > 0.4)",
                kind.name(),
                work / 2.0
            );
            if case == "uniaxial" {
                let closed = SIGMA * SIGMA * 0.4 / (2.0 * E_MOD);
                let w2 = (u_energy - closed).abs() / closed;
                line.push_str(&format!("; W2 {w2:.3e} (limit 1e-10)"));
                assert!(w2 <= 1e-10, "W2");
            }
            println!("{line}");
            assert!(w1 <= 1e-10, "W1");
            assert!(w4 <= 1e-10, "W4");
            assert!(n2 > 0.4, "N2");
        }
    }
}

// ------------------------------------------------------------------------------------------ extrapolation and averaging

fn g_field(x: Vec3) -> [f64; 6] {
    let [a, b, c] = x;
    [
        1e3 * a + 1.0,
        1e3 * b + 2.0,
        1e3 * c + 3.0,
        1e3 * (a + b) + 4.0,
        1e3 * (b - c) + 5.0,
        1e3 * (2.0 * c - a) + 6.0,
    ]
}

fn x_mesh(kind: ElementKind) -> SolidModel {
    model_of(box_of(kind, P_ORIGIN, P_LENGTHS, P_DIV, Some((SEED, 10 + kind_index(kind)))))
}

fn zero_field(model: &SolidModel) -> IntegrationPointField {
    IntegrationPointField::compute(model, &vec![0.0; model.dof_map().n_equations()]).unwrap()
}

#[test]
fn x1_x2_x5_extrapolation_and_means() {
    for kind in KINDS {
        let model = x_mesh(kind);
        let mesh = model.mesh();
        let field = zero_field(&model);
        let g = |x: Vec3| {
            if kind == ElementKind::Tet4 {
                g_field([0.0; 3])
            } else {
                g_field(x)
            }
        };
        let values: Vec<[f64; 6]> = field.positions().iter().map(|&x| g(x)).collect();
        let gmax = mesh.nodes().iter().map(|&x| max_abs6(&g(x))).fold(0.0, f64::max);
        let per_element = element_nodal_values(&model, &field, &values).unwrap();
        let mut x1: f64 = 0.0;
        for (e, vals) in per_element.iter().enumerate() {
            for (&node, v) in mesh.element_nodes(e).iter().zip(vals) {
                x1 = x1.max(max_rel6(v, &g(mesh.nodes()[node]), gmax));
            }
        }
        let avg = nodal_average(&model, &field, &values, None).unwrap();
        let mut x2: f64 = 0.0;
        for (node, v) in avg.iter().enumerate() {
            x2 = x2.max(max_rel6(&v.unwrap(), &g(mesh.nodes()[node]), gmax));
        }
        let mut line = format!("{}: X1 extrapolation {x1:.3e} (limit 1e-13); X2 nodal average {x2:.3e} (limit 1e-13)", kind.name());
        if kind == ElementKind::Hex8 {
            let means = field.element_means(&values).unwrap();
            let el = element_for(kind);
            let mut x5: f64 = 0.0;
            for (e, mean) in means.iter().enumerate() {
                let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
                let ips = el.integration_points(&coords, gauss_3()).unwrap();
                let v: f64 = ips.iter().map(|ip| ip.weight).sum();
                let mut c = [0.0; 3];
                for ip in &ips {
                    for k in 0..3 {
                        c[k] += ip.weight * ip.geometry.x[k] / v;
                    }
                }
                x5 = x5.max(max_rel6(mean, &g(c), gmax));
            }
            line.push_str(&format!("; X5 element means {x5:.3e} (limit 1e-13)"));
            assert!(x5 <= 1e-13, "X5");
        }
        println!("{line}");
        assert!(x1 <= 1e-13, "X1");
        assert!(x2 <= 1e-13, "X2");
    }
}

#[test]
fn x3_volume_weighting() {
    let model = x_mesh(ElementKind::Tet4);
    let mesh = model.mesh();
    let field = zero_field(&model);
    let mut values = Vec::with_capacity(field.n_points());
    for e in 0..field.n_elements() {
        for _ in field.points_of(e) {
            values.push([e as f64; 6]);
        }
    }
    let avg = nodal_average(&model, &field, &values, None).unwrap();
    let n = mesh.nodes().len();
    let (mut num, mut den) = (vec![0.0; n], vec![0.0; n]);
    for e in 0..mesh.n_elements() {
        let v = mesh.element_volume(e);
        for &node in mesh.element_nodes(e) {
            num[node] += v * e as f64;
            den[node] += v;
        }
    }
    let expected: Vec<f64> = num.iter().zip(&den).map(|(a, b)| a / b).collect();
    let emax = expected.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let mut x3: f64 = 0.0;
    for (a, &b) in avg.iter().zip(&expected) {
        let a = a.unwrap();
        for c in a {
            x3 = x3.max((c - b).abs() / emax);
        }
    }
    println!("X3 volume weighting: {x3:.3e} (limit 1e-14)");
    assert!(x3 <= 1e-14, "X3");
}

fn two_hexes(e_right: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut mesh = rebuild(&base, base.nodes().to_vec());
    mesh.add_element_set("left", vec![0]).unwrap();
    mesh.add_element_set("right", vec![1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("left", material_e(E_MOD)).unwrap();
    m.add_material("right", material_e(e_right)).unwrap();
    m
}

#[test]
fn x4_material_interface() {
    let model = two_hexes(1e11);
    let field = zero_field(&model);
    let refused = nodal_average(&model, &field, field.stress(), None);
    let msg = match &refused {
        Err(e) => e.to_string(),
        Ok(_) => String::new(),
    };
    println!("X4 refusal: {msg}");
    assert!(matches!(refused, Err(FemError::InvalidValue { .. })), "X4 refused");
    assert!(msg.contains("node 1 ") && msg.contains("'left'") && msg.contains("'right'"), "X4 message");
    let left = nodal_average(&model, &field, field.stress(), Some("left")).unwrap();
    let mesh = model.mesh();
    for (node, v) in left.iter().enumerate() {
        let on_right_face = mesh.nodes()[node][0] == 2.0;
        assert_eq!(v.is_none(), on_right_face, "X4 region node {node}");
    }
    let same = two_hexes(E_MOD);
    let field = zero_field(&same);
    let ok = nodal_average(&same, &field, field.stress(), None).unwrap();
    assert!(ok.iter().all(|v| v.is_some()), "X4 equal materials");
    println!("X4 region 'left': nodes of x = 2 have no value; equal materials on both sets: accepted");
}

#[test]
fn e1_refusals() {
    let model = x_mesh(ElementKind::Hex8);
    let n = model.dof_map().n_equations();
    let r = IntegrationPointField::compute(&model, &vec![0.0; n - 1]);
    assert!(matches!(r, Err(FemError::FieldLength { key: "displacement", .. })), "E1 displacement");
    let field = zero_field(&model);
    let short = vec![[0.0; 6]; field.n_points() - 1];
    assert!(matches!(field.element_means(&short), Err(FemError::FieldLength { key: "values", .. })), "E1 means");
    assert!(
        matches!(nodal_average(&model, &field, &short, None), Err(FemError::FieldLength { key: "values", .. })),
        "E1 average"
    );
    assert!(
        matches!(nodal_average(&model, &field, field.stress(), Some("nowhere")), Err(FemError::UnknownSet { .. })),
        "E1 region"
    );
    println!("E1 refusals: displacement length, values length (means, average), unknown region");
}
