//! F-614 (ADR 0530): a non-equilibrium quadratic displacement test for the samples of the patch recovery
//! (`estimate::element_samples`) with an opted Hex8. The samples are private; their public observable is the nodal stress of
//! `estimate::recover` (the constant coefficient of the linear least-squares fit over the elements around an interior node),
//! compared with an independent computation written in this file (own gradients, own B-bar mean, own normal equations).
//! Checks E0 to E6 of the registration. Every aggregated error is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::{b_matrix, element_for};
use rcfd_fem::estimate::{energy_error, recover, PatchOptions};
use rcfd_fem::material::Matrix6;
use rcfd_fem::quadrature;
use rcfd_fem::{BlockProperty, ElementKind, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const DIV: [usize; 3] = [4, 3, 3];
const AMP: f64 = 0.3;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The largest of finite values; NaN if any is not finite (so that the check misses).
fn worst<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = 0.0f64;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    w
}

/// The smallest of finite values; NaN if any is not finite.
fn least<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = f64::INFINITY;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.min(v);
    }
    w
}

fn steel() -> Material {
    Material::isotropic_elastic(E_MOD, NU, 7850.0, 1.2e-5).unwrap()
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

/// The patch box with every interior node moved by `AMP h_k sin(a_k)` (the formula of the registration).
fn mesh_with(f: SolidFormulation) -> SolidMesh {
    let base = hex_box(ORIGIN, LENGTHS, DIV).unwrap();
    let boundary = boundary_nodes(&base);
    let h = [
        LENGTHS[0] / DIV[0] as f64,
        LENGTHS[1] / DIV[1] as f64,
        LENGTHS[2] / DIV[2] as f64,
    ];
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if boundary.contains(&i) {
            continue;
        }
        let [x, y, z] = *p;
        let a = [
            7.1 * x + 3.3 * y + 5.9 * z + 0.5,
            4.3 * x - 6.1 * y + 2.7 * z + 1.9,
            -3.7 * x + 5.3 * y + 6.9 * z + 3.1,
        ];
        for k in 0..3 {
            p[k] += AMP * h[k] * a[k].sin();
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    for b in base.blocks() {
        mesh.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in base.node_set_names() {
        mesh.add_node_set(name, base.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in base.element_set_names() {
        mesh.add_element_set(name, base.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn model_with(f: SolidFormulation) -> SolidModel {
    let mut m = SolidModel::new(mesh_with(f));
    m.add_material("all", steel()).unwrap();
    m
}

/// The non-equilibrium quadratic displacement of the registration.
fn field(x: Vec3) -> Vec3 {
    [
        4e-3 * x[0] * x[0] - 1e-3 * x[1] * x[2],
        -2e-3 * x[1] * x[1] + 1.5e-3 * x[0] * x[2],
        3e-3 * x[2] * x[2] + 2e-3 * x[0] * x[1],
    ]
}

fn displacement(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in model.mesh().nodes().iter().enumerate() {
        let v = field(*x);
        for c in 0..3 {
            u[dofs.equation(n, c).unwrap()] = v[c];
        }
    }
    u
}

fn element_u(mesh: &SolidMesh, e: usize) -> Vec<f64> {
    mesh.element_nodes(e)
        .iter()
        .flat_map(|&n| field(mesh.nodes()[n]))
        .collect()
}

fn mul6(b: &[f64], u: &[f64]) -> [f64; 6] {
    std::array::from_fn(|r| (0..24).map(|c| b[r * 24 + c] * u[c]).sum())
}

/// The centroid and the centroid stress of an element: compatible (`bbar = false`) or with the B-bar dilatation (the
/// element mean of the gradients over the 2 x 2 x 2 rule, weighted by `w det J`), written here from the gradients.
fn centroid_sample(mesh: &SolidMesh, d: &Matrix6, e: usize, bbar: bool) -> (Vec3, [f64; 6]) {
    let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
    let u = element_u(mesh, e);
    let el = element_for(ElementKind::Hex8);
    let c = el.point(&coords, [0.0; 3]).unwrap();
    let mut eps = mul6(&b_matrix(&c.grad), &u);
    if bbar {
        let rule = quadrature::gauss_2();
        let mut vol = 0.0;
        let mut mean = [[0.0; 3]; 8];
        for (&xi, &w) in rule.points().iter().zip(rule.weights()) {
            let p = el.point(&coords, xi).unwrap();
            vol += w * p.det_j;
            for a in 0..8 {
                for k in 0..3 {
                    mean[a][k] += w * p.det_j * p.grad[a][k];
                }
            }
        }
        let mut corr = 0.0;
        for a in 0..8 {
            for k in 0..3 {
                corr += u[3 * a + k] * (mean[a][k] / vol - c.grad[a][k]);
            }
        }
        for e in eps.iter_mut().take(3) {
            *e += corr / 3.0;
        }
    }
    (c.x, apply(d, &eps))
}

/// Gauss elimination with partial pivoting (`a` n x n row-major).
fn dense_solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Vec<f64> {
    for c in 0..n {
        let p = (c..n)
            .max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs()))
            .unwrap();
        if p != c {
            for j in 0..n {
                a.swap(c * n + j, p * n + j);
            }
            b.swap(c, p);
        }
        for r in c + 1..n {
            let f = a[r * n + c] / a[c * n + c];
            for j in c..n {
                a[r * n + j] -= f * a[c * n + j];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let mut s = b[r];
        for j in r + 1..n {
            s -= a[r * n + j] * x[j];
        }
        x[r] = s / a[r * n + r];
    }
    x
}

/// The expected nodal stress at every interior node: the constant coefficient of the linear least-squares fit
/// `[1, dx, dy, dz] c = sigma` over the centroid samples of the elements around the node (own normal equations).
fn expected_nodal(mesh: &SolidMesh, d: &Matrix6, bbar: bool) -> Vec<(usize, [f64; 6])> {
    let boundary = boundary_nodes(mesh);
    let samples: Vec<(Vec3, [f64; 6])> = (0..mesh.n_elements())
        .map(|e| centroid_sample(mesh, d, e, bbar))
        .collect();
    let mut out = Vec::new();
    for n in 0..mesh.nodes().len() {
        if boundary.contains(&n) {
            continue;
        }
        let around: Vec<usize> = (0..mesh.n_elements())
            .filter(|&e| mesh.element_nodes(e).contains(&n))
            .collect();
        assert_eq!(around.len(), 8, "an interior node of the box has 8 elements");
        let o = mesh.nodes()[n];
        let rows: Vec<[f64; 4]> = around
            .iter()
            .map(|&e| {
                let x = samples[e].0;
                [1.0, x[0] - o[0], x[1] - o[1], x[2] - o[2]]
            })
            .collect();
        let mut ata = vec![0.0; 16];
        for r in &rows {
            for i in 0..4 {
                for j in 0..4 {
                    ata[i * 4 + j] += r[i] * r[j];
                }
            }
        }
        let mut c0 = [0.0; 6];
        for (comp, slot) in c0.iter_mut().enumerate() {
            let rhs: Vec<f64> = (0..4)
                .map(|i| rows.iter().zip(&around).map(|(r, &e)| r[i] * samples[e].1[comp]).sum())
                .collect();
            *slot = dense_solve(ata.clone(), rhs, 4)[0];
        }
        out.push((n, c0));
    }
    out
}

fn library_nodal(f: SolidFormulation) -> (Vec<Option<[f64; 6]>>, SolidModel) {
    let model = model_with(f);
    let u = displacement(&model);
    let rec = recover(&model, &u, &PatchOptions::new()).unwrap();
    (rec.nodal().to_vec(), model)
}

/// `max |lib - exp| / smax` over the interior nodes and components; NaN if a node is not recovered or a value is not finite.
fn distance(lib: &[Option<[f64; 6]>], exp: &[(usize, [f64; 6])], smax: f64) -> f64 {
    worst(exp.iter().flat_map(|(n, e)| match lib[*n] {
        Some(s) => (0..6).map(|k| (s[k] - e[k]).abs() / smax).collect::<Vec<_>>(),
        None => vec![f64::NAN],
    }))
}

fn compare(a: &[(usize, [f64; 6])], b: &[(usize, [f64; 6])], smax: f64) -> Vec<f64> {
    a.iter()
        .zip(b)
        .map(|((_, x), (_, y))| worst((0..6).map(|k| (x[k] - y[k]).abs() / smax)))
        .collect()
}

#[test]
fn e0_to_e6_estimate_samples_of_an_opted_hex8_on_a_quadratic_field() {
    let mut ch = Checks::new();
    let d = steel().elasticity_matrix().unwrap();

    // E0: the field is not an equilibrium solution. lap u = grad div u = (8e-3, -4e-3, 6e-3) for this field, so
    // b = -(lambda + 2 mu) (8e-3, -4e-3, 6e-3) (closed form from the Lame constants).
    let lambda = E_MOD * NU / ((1.0 + NU) * (1.0 - 2.0 * NU));
    let mu = E_MOD / (2.0 * (1.0 + NU));
    let g: [f64; 3] = [8e-3, -4e-3, 6e-3];
    let b_norm = (lambda + 2.0 * mu) * (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
    ch.above(
        "E0 |b| / E (the field is not an equilibrium solution)",
        b_norm / E_MOD,
        1e-3,
    );

    let full_mesh = mesh_with(SolidFormulation::Full);
    let comp = expected_nodal(&full_mesh, &d, false);
    let bbar = expected_nodal(&full_mesh, &d, true);
    assert_eq!(comp.len(), 12);
    let smax =
        worst((0..full_mesh.n_elements()).flat_map(|e| centroid_sample(&full_mesh, &d, e, false).1.map(f64::abs)));
    println!("smax = {smax:.4e}");

    // E2, E2b: the margin between the two independent expectations.
    let margin = compare(&bbar, &comp, smax);
    ch.at_least(
        "E2 BBar against compatible expectation, largest over nodes",
        worst(margin.clone()),
        1e-5,
    );
    ch.at_least("E2b the same, smallest over nodes", least(margin), 1e-6);

    // E1, E3: BBar.
    let (lib_bbar, _) = library_nodal(SolidFormulation::BBar);
    ch.at_most(
        "E1 BBar library nodal stress against the B-bar expectation",
        distance(&lib_bbar, &bbar, smax),
        1e-10,
    );
    ch.at_least(
        "E3 BBar library nodal stress against the compatible expectation (catches MU12)",
        distance(&lib_bbar, &comp, smax),
        1e-5,
    );

    // E4, E4b: Incompatible (the strain at the centroid is the compatible one).
    let (lib_inc, _) = library_nodal(SolidFormulation::Incompatible);
    ch.at_most(
        "E4 Incompatible library nodal stress against the compatible expectation",
        distance(&lib_inc, &comp, smax),
        1e-10,
    );
    ch.at_least(
        "E4b Incompatible library nodal stress against the B-bar expectation",
        distance(&lib_inc, &bbar, smax),
        1e-5,
    );

    // E5: the Full control.
    let (lib_full, full_model) = library_nodal(SolidFormulation::Full);
    ch.at_most(
        "E5 Full library nodal stress against the compatible expectation",
        distance(&lib_full, &comp, smax),
        1e-10,
    );

    // E6: the energy estimate runs for the three formulations and differs from Full's for the opted ones.
    let norm_full = {
        let u = displacement(&full_model);
        let (_, ee) = energy_error(&full_model, &u, &PatchOptions::new()).unwrap();
        let norm = ee.solution_norm();
        ch.above("E6 Full solution norm finite and positive", norm, 0.0);
        (ee.estimate(), norm)
    };
    for f in [SolidFormulation::BBar, SolidFormulation::Incompatible] {
        let model = model_with(f);
        let u = displacement(&model);
        let (_, ee) = energy_error(&model, &u, &PatchOptions::new()).unwrap();
        ch.above(
            &format!("E6 {} |estimate - estimate_Full| / solution norm_Full", f.name()),
            (ee.estimate() - norm_full.0).abs() / norm_full.1,
            1e-6,
        );
    }
    ch.finish();
}
