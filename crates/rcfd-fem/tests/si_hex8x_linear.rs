//! F-588 (ADR 0525): the Hex8 formulations (B-bar, incompatible modes) on the remaining linear paths: the temperature load,
//! per-point materials (orthotropic, tabulated), the geometric stiffness, the damped modes, the direct harmonic solve and the
//! patch recovery. Checks TH1 to TH4b, PO1 to PO3, PP1a, PP1b, LIN1, N-LIN, GB1, GB2, N-GB, O-GB, SA1, ES1, ES2, DM1, DM2a to
//! DM2c, DH1, DH2, DH3a to DH3c and RF1 to RF3 of the registration (B1 is in `si_hex8x_linear_bits.rs`). Every aggregated error
//! is required finite before it is compared. The helpers are copies of those of `si_hex8x.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions, OwnFrequency};
use rcfd_fem::element::{b_matrix, element_for};
use rcfd_fem::estimate::{energy_error, recover as patch_recover, PatchOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::hex8x::{
    element_stiffness, element_stiffness_at, element_thermal_load, point_strains, point_strains_at, strain_at,
};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mat_ortho::{thermal_strain, MaterialPoints};
use rcfd_fem::mat_visco::complex_moduli;
use rcfd_fem::material::{MaterialAxes, Matrix6, PointContext, PronyTerm};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::quadrature;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel, SolidResult,
};
use rcfd_linalg::eigen::ResidualTest;
use std::collections::BTreeSet;

const E_MOD: f64 = 2.0e11;
const FORMS: [SolidFormulation; 2] = [SolidFormulation::BBar, SolidFormulation::Incompatible];

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
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

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, 7850.0, 1.2e-5).unwrap()
}

fn dmat(nu: f64) -> Matrix6 {
    steel(nu).elasticity_matrix().unwrap()
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
}

fn rotation() -> [[f64; 3]; 3] {
    // Rz(0.5) Rx(0.3), the rows are the material directions.
    let (cz, sz) = (0.5f64.cos(), 0.5f64.sin());
    let (cx, sx) = (0.3f64.cos(), 0.3f64.sin());
    let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| rz[i][k] * rx[k][j]).sum()))
}

/// The orthotropic material "O" of the registration.
fn material_o(expansion: [f64; 3], axes: MaterialAxes) -> Material {
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.4],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        expansion,
        axes,
    )
    .unwrap()
}

fn d_o(material: &Material) -> Matrix6 {
    material.elasticity_at(&PointContext::default()).unwrap()
}

fn tabulated() -> Material {
    Material::tabulated(
        steel(0.3),
        vec![200.0, 600.0],
        vec![
            ("e".to_string(), vec![2.0e11, 1.8e11]),
            ("nu".to_string(), vec![0.3, 0.3]),
            ("expansion".to_string(), vec![1.2e-5, 1.5e-5]),
        ],
    )
    .unwrap()
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

fn model_with(mesh: SolidMesh, material: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m
}

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

fn element_u(res: &SolidResult, mesh: &SolidMesh, e: usize) -> Vec<f64> {
    mesh.element_nodes(e)
        .iter()
        .flat_map(|&n| res.displacement(n).iter().copied())
        .collect()
}

fn flat6(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

// ------------------------------------------------------------------------------------------------------- the patch

const SEED: u64 = 0xF374_0275;
const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const P_DIV: [usize; 3] = [4, 3, 3];
const FIELD_C: Vec3 = [1e-3, -2e-3, 0.5e-3];
const FIELD_A: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, -0.2e-3],
    [0.5e-3, -0.7e-3, 0.4e-3],
    [0.1e-3, 0.6e-3, 0.9e-3],
];

/// SplitMix64 of `sb_static.rs` (ADR 0275 D8).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn linear_field(x: Vec3) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += FIELD_A[i][j] * x[j];
        }
    }
    u
}

fn field_strain() -> [f64; 6] {
    let a = FIELD_A;
    [
        a[0][0],
        a[1][1],
        a[2][2],
        a[0][1] + a[1][0],
        a[1][2] + a[2][1],
        a[2][0] + a[0][2],
    ]
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

/// The patch box of ADR 0493 reading (a2), interior nodes moved by `amp h_k (2U - 1)`.
fn patch_mesh(amp: f64, tag: u64) -> SolidMesh {
    let base = hex_box(P_ORIGIN, P_LENGTHS, P_DIV).unwrap();
    let mut rng = Rng::new(tag);
    let boundary = boundary_nodes(&base);
    let h = [
        P_LENGTHS[0] / P_DIV[0] as f64,
        P_LENGTHS[1] / P_DIV[1] as f64,
        P_LENGTHS[2] / P_DIV[2] as f64,
    ];
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for k in 0..3 {
                p[k] += amp * h[k] * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    rebuild(&base, nodes)
}

fn patch_prescribed(mesh: &SolidMesh) -> Vec<Prescribed> {
    let mut pres = Vec::new();
    for &n in &boundary_nodes(mesh) {
        let u = linear_field(mesh.nodes()[n]);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    pres
}

fn node_at(mesh: &SolidMesh, p: Vec3) -> usize {
    mesh.nodes()
        .iter()
        .position(|x| (0..3).all(|k| (x[k] - p[k]).abs() < 1e-12))
        .expect("a node at the point")
}

/// The six displacement components of reading "free": `A` the origin corner (three), `B` the corner at `+L_x` (`u_y`, `u_z`),
/// `C` the corner at `+L_y` (`u_z`).
fn free_supports(mesh: &SolidMesh) -> Vec<Prescribed> {
    let a = node_at(mesh, P_ORIGIN);
    let b = node_at(mesh, [P_ORIGIN[0] + P_LENGTHS[0], P_ORIGIN[1], P_ORIGIN[2]]);
    let c = node_at(mesh, [P_ORIGIN[0], P_ORIGIN[1] + P_LENGTHS[1], P_ORIGIN[2]]);
    vec![
        Prescribed::new(a, 0, 0.0),
        Prescribed::new(a, 1, 0.0),
        Prescribed::new(a, 2, 0.0),
        Prescribed::new(b, 1, 0.0),
        Prescribed::new(b, 2, 0.0),
        Prescribed::new(c, 2, 0.0),
    ]
}

fn set_uniform_temperature(m: &mut SolidModel, t: f64) {
    let n = m.mesh().nodes().len();
    m.add_load(Load::temperature(vec![t; n], 293.0)).unwrap();
}

// ------------------------------------------------------------------------------------------ the thermal load (a1, a2)

/// The free expansion of the distorted patch: (max stress / `s0`, max displacement error / (`alpha dT L_max`), or NaN without
/// the displacement reference).
fn free_expansion(f: SolidFormulation, material: Material, t: f64, s0: f64, ref_strain: Option<f64>) -> (f64, f64) {
    let mesh = with_formulation(patch_mesh(0.25, 1001), f);
    let mut m = model_with(mesh, material);
    set_uniform_temperature(&mut m, t);
    let supports = free_supports(m.mesh());
    let res = solve_static_with(&m, &supports, &[], &StaticOptions::default()).unwrap();
    let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
    let serr = worst(field.stress().iter().flat_map(|s| s.iter().map(|v| v.abs()))) / s0;
    let uerr = match ref_strain {
        Some(eps) => {
            let xa = P_ORIGIN;
            let lmax = P_LENGTHS[0];
            worst(m.mesh().nodes().iter().enumerate().flat_map(|(n, x)| {
                let u = res.displacement(n);
                (0..3)
                    .map(move |k| (u[k] - eps * (x[k] - xa[k])).abs() / (eps * lmax))
                    .collect::<Vec<_>>()
            }))
        }
        None => 0.0,
    };
    (serr, uerr)
}

#[test]
fn th1_to_th3_free_thermal_expansion_of_the_distorted_patch() {
    let mut ch = Checks::new();
    let alpha_dt = 1.2e-5 * 60.0;
    // N-TH: the default Full element on the same harness (estimate 1e-13).
    let (s, u) = free_expansion(SolidFormulation::Full, steel(0.3), 353.0, E_MOD * alpha_dt, Some(alpha_dt));
    ch.at_most("N-TH Full nu 0.3 stress / (E alpha dT)", s, 1e-13);
    ch.at_most("N-TH Full nu 0.3 displacement error", u, 1e-13);
    for f in FORMS {
        let name = f.name();
        for nu in [0.3, 0.4999] {
            let (s, u) = free_expansion(f, steel(nu), 353.0, E_MOD * alpha_dt, Some(alpha_dt));
            ch.at_most(&format!("TH1 {name} nu {nu} stress / (E alpha dT)"), s, 1e-10);
            ch.at_most(&format!("TH1 {name} nu {nu} |u - alpha dT (x - x_A)| / (alpha dT L)"), u, 1e-10);
        }
        // TH2: material O, the full thermal strain tensor in the rotated axes.
        let o = material_o([2.0e-6, 3.0e-5, 4.0e-5], MaterialAxes::Fixed(rotation()));
        let (s, _) = free_expansion(f, o, 353.0, 1.4e11 * 4.0e-5 * 60.0, None);
        ch.at_most(&format!("TH2 {name} orthotropic stress / s_0"), s, 1e-10);
        // TH3: the tabulated steel at T = 353.
        let (s, _) = free_expansion(f, tabulated(), 353.0, E_MOD * 1.2e-5 * 60.0, None);
        ch.at_most(&format!("TH3 {name} tabulated stress / (E_0 alpha_0 dT)"), s, 1e-10);
    }
    ch.finish();
}

// -------------------------------------------------------------------------------- the test's own B-bar and mode matrices

/// `B_q` (6 x 24), `G_q` (6 x 9) and `w det J_q` at the points of `gauss_2`, the modes' gradients corrected (Taylor).
fn test_modes(coords: &[Vec3]) -> Vec<(Vec<f64>, Vec<f64>, f64)> {
    let el = element_for(ElementKind::Hex8);
    let c = el.point(coords, [0.0; 3]).unwrap();
    let rule = quadrature::gauss_2();
    rule.points()
        .iter()
        .zip(rule.weights())
        .map(|(&xi, &w)| {
            let p = el.point(coords, xi).unwrap();
            let scale = c.det_j / p.det_j;
            let jm: Vec<f64> = c.jacobian.iter().flatten().copied().collect();
            let mut inv = [[0.0; 3]; 3];
            for m in 0..3 {
                let mut e = vec![0.0; 3];
                e[m] = 1.0;
                let col = dense_solve(jm.clone(), e, 3);
                for r in 0..3 {
                    inv[r][m] = col[r];
                }
            }
            let grads: Vec<[f64; 3]> = (0..3)
                .map(|k| std::array::from_fn(|m| scale * inv[k][m] * (-2.0 * xi[k])))
                .collect();
            (b_matrix(&p.grad), b_matrix(&grads), w * p.det_j)
        })
        .collect()
}

/// The B-bar matrices `Bbar_q` (6 x 24) and the weights `w det J_q`, from the element means of the gradients.
fn test_bbar(coords: &[Vec3]) -> Vec<(Vec<f64>, f64)> {
    let el = element_for(ElementKind::Hex8);
    let rule = quadrature::gauss_2();
    let pts: Vec<_> = rule
        .points()
        .iter()
        .zip(rule.weights())
        .map(|(&xi, &w)| {
            let p = el.point(coords, xi).unwrap();
            let wd = w * p.det_j;
            (p, wd)
        })
        .collect();
    let vol: f64 = pts.iter().map(|(_, w)| w).sum();
    let mut mean = [[0.0; 3]; 8];
    for (p, w) in &pts {
        for a in 0..8 {
            for c in 0..3 {
                mean[a][c] += w * p.grad[a][c] / vol;
            }
        }
    }
    pts.iter()
        .map(|(p, w)| {
            let mut b = b_matrix(&p.grad);
            for a in 0..8 {
                for c in 0..3 {
                    let corr = (mean[a][c] - p.grad[a][c]) / 3.0;
                    for i in 0..3 {
                        b[i * 24 + 3 * a + c] += corr;
                    }
                }
            }
            (b, *w)
        })
        .collect()
}

/// Dense solve of `a x = b` by Gauss elimination with partial pivoting (`a` n x n row-major).
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
            if f != 0.0 {
                for j in c..n {
                    a[r * n + j] -= f * a[c * n + j];
                }
                b[r] -= f * b[c];
            }
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

/// The distorted single element of `est1.py` (ADR 0493 `distorted_element`).
fn distorted_element() -> Vec<Vec3> {
    let offs = [
        [0.0, 0.0, 0.0],
        [0.1, 0.05, -0.05],
        [0.15, -0.1, 0.05],
        [-0.1, 0.1, 0.0],
        [0.05, -0.05, 0.1],
        [0.0, 0.1, 0.05],
        [0.1, 0.1, 0.1],
        [-0.05, 0.0, -0.1],
    ];
    ElementKind::Hex8
        .reference_nodes()
        .iter()
        .zip(offs)
        .map(|(r, o)| std::array::from_fn(|k| 0.5 * r[k] + 0.5 + o[k]))
        .collect()
}

fn rigid_motions(coords: &[Vec3]) -> Vec<Vec<f64>> {
    let mut out = Vec::new();
    for c in 0..3 {
        let mut r = vec![0.0; 24];
        for a in 0..8 {
            r[3 * a + c] = 1.0;
        }
        out.push(r);
    }
    for w in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        let mut r = vec![0.0; 24];
        for (a, x) in coords.iter().enumerate() {
            let v = [
                w[1] * x[2] - w[2] * x[1],
                w[2] * x[0] - w[0] * x[2],
                w[0] * x[1] - w[1] * x[0],
            ];
            r[3 * a..3 * a + 3].copy_from_slice(&v);
        }
        out.push(r);
    }
    out
}

// -------------------------------------------------------------------------------------- non-uniform temperature (TH4)

#[test]
fn th4a_th4b_element_identities_with_a_non_uniform_temperature() {
    let mut ch = Checks::new();
    let nu = 0.3;
    let d = dmat(nu);
    let alpha = 1.2e-5;
    let mut rng = Rng(0xF588 + 7);
    let mesh0 = patch_mesh(0.25, 1001);
    let temps: Vec<f64> = (0..mesh0.nodes().len()).map(|_| 293.0 + 80.0 * rng.uniform()).collect();
    for f in FORMS {
        let name = f.name();
        let mesh = with_formulation(mesh0.clone(), f);
        let mut m = model_with(mesh, steel(nu));
        m.add_load(Load::temperature(temps.clone(), 293.0)).unwrap();
        let pres: Vec<Prescribed> = boundary_nodes(m.mesh())
            .iter()
            .flat_map(|&n| (0..3).map(move |c| Prescribed::new(n, c, 0.0)))
            .collect();
        let res = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
        let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
        let el = element_for(ElementKind::Hex8);
        let mesh = m.mesh();
        let (mut id1, mut id2) = (Vec::new(), Vec::new());
        for e in 0..mesh.n_elements() {
            let coords = element_coords(mesh, e);
            let ue = element_u(&res, mesh, e);
            let nodes = mesh.element_nodes(e);
            let rule = quadrature::gauss_2();
            let eth: Vec<[f64; 6]> = rule
                .points()
                .iter()
                .map(|&xi| {
                    let p = el.point(&coords, xi).unwrap();
                    let dt: f64 = nodes.iter().zip(&p.n).map(|(&n, s)| s * (temps[n] - 293.0)).sum();
                    [alpha * dt, alpha * dt, alpha * dt, 0.0, 0.0, 0.0]
                })
                .collect();
            let fe = element_thermal_load(f, &coords, &[d; 8], &eth).unwrap();
            let ke = element_stiffness(f, &coords, &d).unwrap();
            let ku = ke.matvec(&ue);
            let rhs: Vec<f64> = ku.iter().zip(&fe).map(|(k, f)| k - f).collect();
            let bs: Vec<(Vec<f64>, f64)> = if f == SolidFormulation::BBar {
                test_bbar(&coords)
            } else {
                test_modes(&coords).into_iter().map(|(b, _, w)| (b, w)).collect()
            };
            let mut lhs = vec![0.0; 24];
            for (q, (b, w)) in bs.iter().enumerate() {
                let s = field.stress()[field.points_of(e).start + q];
                for c in 0..24 {
                    for r in 0..6 {
                        lhs[c] += w * b[r * 24 + c] * s[r];
                    }
                }
            }
            let scale = max_abs(&ku).max(max_abs(&fe));
            id1.push(max_abs(&lhs.iter().zip(&rhs).map(|(a, b)| a - b).collect::<Vec<_>>()) / scale);
            if f == SolidFormulation::Incompatible {
                let parts = test_modes(&coords);
                let (mut fa, mut fa_scale) = (vec![0.0; 9], vec![0.0; 9]);
                for (q, (_, g, w)) in parts.iter().enumerate() {
                    let s = field.stress()[field.points_of(e).start + q];
                    for c in 0..9 {
                        for r in 0..6 {
                            fa[c] += w * g[r * 9 + c] * s[r];
                            fa_scale[c] += w * (g[r * 9 + c] * s[r]).abs();
                        }
                    }
                }
                id2.push(max_abs(&fa) / max_abs(&fa_scale));
            }
        }
        ch.at_most(
            &format!("TH4a {name} |sum w B^T sigma - (K_e u_e - f_e)| / max(|K_e u_e|, |f_e|)"),
            worst(id1),
            1e-9,
        );
        if f == SolidFormulation::Incompatible {
            ch.at_most("TH4b Incompatible |sum w G^T sigma| / sum w |G^T sigma|", worst(id2), 1e-9);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------ per-point materials (a2)

fn orthotropic_patch(amp: f64, tag: u64, f: SolidFormulation) -> (SolidModel, SolidResult) {
    let mesh = with_formulation(patch_mesh(amp, tag), f);
    let m = model_with(mesh, material_o([0.0; 3], MaterialAxes::Fixed(rotation())));
    let pres = patch_prescribed(m.mesh());
    let res = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    (m, res)
}

#[test]
fn po1_po2_po3_orthotropic_patch_stress_and_element_algebra() {
    let mut ch = Checks::new();
    for f in FORMS {
        let name = f.name();
        for (amp, tag) in [(0.1, 11), (0.25, 1001)] {
            let (m, res) = orthotropic_patch(amp, tag, f);
            let boundary = boundary_nodes(m.mesh());
            let mut umax = 0.0f64;
            let mut errs = Vec::new();
            for (n, x) in m.mesh().nodes().iter().enumerate() {
                let ue = linear_field(*x);
                let uh = res.displacement(n);
                for k in 0..3 {
                    umax = umax.max(ue[k].abs());
                    if !boundary.contains(&n) {
                        errs.push((uh[k] - ue[k]).abs());
                    }
                }
            }
            ch.at_most(&format!("PO1 {name} {amp} h interior error"), worst(errs) / umax, 1e-10);
            let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
            let o = material_o([0.0; 3], MaterialAxes::Fixed(rotation()));
            let sig = apply(&d_o(&o), &field_strain());
            let smax = max_abs(&sig);
            let serr = worst(
                field
                    .stress()
                    .iter()
                    .flat_map(|s| (0..6).map(move |k| (s[k] - sig[k]).abs() / smax)),
            );
            ch.at_most(&format!("PO2 {name} {amp} h stress against D_O eps"), serr, 1e-10);
        }
        // PO3: the distorted element.
        let coords = distorted_element();
        let o = material_o([0.0; 3], MaterialAxes::Fixed(rotation()));
        let d = d_o(&o);
        let k1 = element_stiffness(f, &coords, &d).unwrap();
        let k2 = element_stiffness_at(f, &coords, &[d; 8]).unwrap();
        let same = k1
            .values()
            .iter()
            .zip(k2.values())
            .all(|(a, b)| a.to_bits() == b.to_bits());
        ch.holds(&format!("PO3 {name} element_stiffness_at([D; 8]) bit-equal element_stiffness"), same, "");
        let ds: Vec<Matrix6> = (0..8)
            .map(|q| std::array::from_fn(|i| std::array::from_fn(|j| d[i][j] * (1.0 + 0.05 * q as f64))))
            .collect();
        let k = element_stiffness_at(f, &coords, &ds).unwrap();
        let kv = k.values();
        let kmax = max_abs(kv);
        let sym = worst((0..24).flat_map(|i| (0..24).map(move |j| (kv[i * 24 + j] - kv[j * 24 + i]).abs())));
        ch.at_most(&format!("PO3 {name} D_q scaled: max |K - K^T| / max |K|"), sym / kmax, 1e-13);
        let rig = worst(
            rigid_motions(&coords)
                .iter()
                .map(|r| max_abs(&k.matvec(r)) / (kmax * max_abs(r))),
        );
        ch.at_most(&format!("PO3 {name} D_q scaled: rigid motions"), rig, 1e-11);
    }
    ch.finish();
}

fn element_mesh(coords: &[Vec3]) -> SolidMesh {
    let mut mesh = SolidMesh::new(coords.to_vec()).unwrap();
    mesh.add_block(ElementKind::Hex8, (0..8).collect()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    mesh
}

fn single_supports() -> Vec<Prescribed> {
    vec![
        Prescribed::new(0, 0, 0.0),
        Prescribed::new(0, 1, 0.0),
        Prescribed::new(0, 2, 0.0),
        Prescribed::new(1, 1, 0.0),
        Prescribed::new(1, 2, 0.0),
        Prescribed::new(3, 2, 0.0),
    ]
}

/// The free dofs of `single_supports` and the solution of `K u = f` reduced to them (zeros at the supports).
fn reduced_solve(k: &[f64], f: &[f64]) -> Vec<f64> {
    let fixed = [0usize, 1, 2, 4, 5, 11];
    let free: Vec<usize> = (0..24).filter(|i| !fixed.contains(i)).collect();
    let n = free.len();
    let a: Vec<f64> = free
        .iter()
        .flat_map(|&i| free.iter().map(move |&j| (i, j)))
        .map(|(i, j)| k[i * 24 + j])
        .collect();
    let b: Vec<f64> = free.iter().map(|&i| f[i]).collect();
    let x = dense_solve(a, b, n);
    let mut u = vec![0.0; 24];
    for (xi, &i) in x.iter().zip(&free) {
        u[i] = *xi;
    }
    u
}

/// The test's `D_q` and (with a temperature) `eps_th,q` of the single element at the points of the rule.
fn point_data(model: &SolidModel, material: &Material, coords: &[Vec3]) -> (Vec<Matrix6>, Option<Vec<[f64; 6]>>) {
    let points = MaterialPoints::new(model).unwrap();
    let el = element_for(ElementKind::Hex8);
    let mut ds = Vec::new();
    let mut eth = points.reference().map(|_| Vec::new());
    for &xi in quadrature::gauss_2().points() {
        let p = el.point(coords, xi).unwrap();
        let (ctx, dt) = points.context(0, material, p.x, &p.n).unwrap();
        ds.push(material.elasticity_at(&ctx).unwrap());
        if let (Some(v), Some(r)) = (eth.as_mut(), points.reference()) {
            v.push(thermal_strain(material, &ctx, r, dt).unwrap());
        }
    }
    (ds, eth)
}

fn pp1_run(ch: &mut Checks, tag: &str, f: SolidFormulation, material: Material, temps: Option<Vec<f64>>, mech: bool) {
    let coords = distorted_element();
    let mut m = model_with(
        with_formulation(element_mesh(&coords), f),
        material.clone(),
    );
    if let Some(t) = &temps {
        m.add_load(Load::temperature(t.clone(), 293.0)).unwrap();
    }
    let forces = if mech {
        vec![
            NodalForceAt::new(6, [1.0e3, -2.0e3, 3.0e3]),
            NodalForceAt::new(7, [-1.0e3, 5.0e2, 8.0e2]),
        ]
    } else {
        Vec::new()
    };
    let res = solve_static_with(&m, &single_supports(), &forces, &StaticOptions::default()).unwrap();
    let (ds, eth) = point_data(&m, &material, &coords);
    let ke = element_stiffness_at(f, &coords, &ds).unwrap();
    let mut load = vec![0.0; 24];
    if let Some(eth) = &eth {
        load = element_thermal_load(f, &coords, &ds, eth).unwrap();
    }
    if mech {
        load[18..21].copy_from_slice(&[1.0e3, -2.0e3, 3.0e3]);
        load[21..24].copy_from_slice(&[-1.0e3, 5.0e2, 8.0e2]);
    }
    let uref = reduced_solve(ke.values(), &load);
    let ue = element_u(&res, m.mesh(), 0);
    let err = max_abs(&ue.iter().zip(&uref).map(|(a, b)| a - b).collect::<Vec<_>>()) / max_abs(&uref);
    ch.at_most(&format!("{tag} {} u against the dense reduction", f.name()), err, 1e-9);
    let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
    let s = point_strains_at(f, &coords, &ds, eth.as_deref(), &ue).unwrap();
    let same = (0..8).all(|q| (0..6).all(|r| field.strain()[q][r].to_bits() == s[q][r].to_bits()));
    ch.holds(&format!("{tag} {} field strains bit for bit", f.name()), same, "");
}

#[test]
fn pp1a_pp1b_one_distorted_element_with_point_materials() {
    let mut ch = Checks::new();
    for f in FORMS {
        let cyl = material_o(
            [0.0; 3],
            MaterialAxes::cylindrical([-0.4, -0.3, 0.0], [0.0, 0.0, 1.0]).unwrap(),
        );
        pp1_run(&mut ch, "PP1a", f, cyl, None, true);
        pp1_run(
            &mut ch,
            "PP1b",
            f,
            tabulated(),
            Some(vec![300.0, 320.0, 350.0, 310.0, 340.0, 380.0, 360.0, 330.0]),
            false,
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------- additivity in D (a7)

fn k_diff(f: SolidFormulation) -> f64 {
    let coords = distorted_element();
    let d1 = dmat(0.3);
    let o = material_o([0.0; 3], MaterialAxes::Fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]));
    let d2 = d_o(&o);
    let sum: Matrix6 = std::array::from_fn(|i| std::array::from_fn(|j| d1[i][j] + d2[i][j]));
    let k12 = element_stiffness(f, &coords, &sum).unwrap();
    let k1 = element_stiffness(f, &coords, &d1).unwrap();
    let k2 = element_stiffness(f, &coords, &d2).unwrap();
    let kmax = max_abs(k12.values());
    worst(
        (0..576).map(|i| (k12.values()[i] - k1.values()[i] - k2.values()[i]).abs()),
    ) / kmax
}

#[test]
fn lin1_n_lin_additivity_of_the_stiffness_in_d() {
    let mut ch = Checks::new();
    ch.at_most("LIN1 BBar max |K(D1 + D2) - K(D1) - K(D2)| / max |K|", k_diff(SolidFormulation::BBar), 1e-13);
    ch.above(
        "N-LIN Incompatible max |K(D1 + D2) - K(D1) - K(D2)| / max |K|",
        k_diff(SolidFormulation::Incompatible),
        1e-3,
    );
    println!("(record) Full additivity {:.3e}", k_diff(SolidFormulation::Full));
    ch.finish();
}

// ---------------------------------------------------------------------------------------- Euler buckling (a3)

/// `lambda P / P_cr` of the clamped-free strip `2 x 0.1 x 0.1` of `20 x 1 x 1` elements.
fn strip_buckling(f: SolidFormulation, nu: f64) -> f64 {
    let (l, h) = (2.0, 0.1);
    let mesh = with_formulation(hex_box([0.0; 3], [l, h, h], [20, 1, 1]).unwrap(), f);
    let mut m = model_with(mesh, Material::isotropic_elastic(E_MOD, nu, 7850.0, 0.0).unwrap());
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let traction = 1.0e6;
    m.add_load(Load::traction("xmax", [-traction, 0.0, 0.0])).unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&m, &r).unwrap();
    let bf = factors(&m, &pre, &BucklingOptions::new(2)).unwrap();
    let p = traction * h * h;
    let i = h.powi(4) / 12.0;
    let pcr = std::f64::consts::PI.powi(2) * E_MOD * i / (4.0 * l * l);
    bf.factors[0] * p / pcr
}

#[test]
fn gb1_gb2_n_gb_o_gb_euler_buckling_of_the_one_layer_strip() {
    let mut ch = Checks::new();
    let inc = SolidFormulation::Incompatible;
    ch.at_most("GB1 Incompatible nu 0 |lambda P / P_cr - 1|", (strip_buckling(inc, 0.0) - 1.0).abs(), 0.01);
    ch.at_most("GB2 Incompatible nu 0.3 |lambda P / P_cr - 1|", (strip_buckling(inc, 0.3) - 1.0).abs(), 0.02);
    for nu in [0.0, 0.3] {
        ch.above(
            &format!("N-GB Full nu {nu} lambda P / P_cr (shear locking control)"),
            strip_buckling(SolidFormulation::Full, nu),
            1.3,
        );
        println!(
            "O-GB BBar nu {nu} lambda P / P_cr {:.6}",
            strip_buckling(SolidFormulation::BBar, nu)
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------- patch recovery (a6)

#[test]
fn sa1_strain_at_against_point_strains() {
    let mut ch = Checks::new();
    let coords = distorted_element();
    let d = dmat(0.3);
    let mut rng = Rng(0xF588 + 1);
    let u: Vec<f64> = (0..24).map(|_| 1e-3 * (2.0 * rng.uniform() - 1.0)).collect();
    for f in FORMS {
        let reference = point_strains(f, &coords, &d, &u).unwrap();
        let smax = max_abs(&flat6(&reference));
        let err = worst(quadrature::gauss_2().points().iter().enumerate().flat_map(|(q, &xi)| {
            let s = strain_at(f, &coords, &d, &u, xi).unwrap();
            (0..6).map(|r| (s[r] - reference[q][r]).abs() / smax).collect::<Vec<_>>()
        }));
        ch.at_most(&format!("SA1 {} strain_at against point_strains", f.name()), err, 1e-12);
    }
    ch.finish();
}

#[test]
fn es1_patch_recovery_of_the_linear_field() {
    let mut ch = Checks::new();
    for f in FORMS {
        let mesh = with_formulation(patch_mesh(0.25, 1001), f);
        let m = model_with(mesh, steel(0.3));
        let pres = patch_prescribed(m.mesh());
        let res = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
        let sig = apply(&dmat(0.3), &field_strain());
        let smax = max_abs(&sig);
        let (rec, ee) = energy_error(&m, res.displacement_values(), &PatchOptions::new()).unwrap();
        let boundary = boundary_nodes(m.mesh());
        let mut errs = Vec::new();
        let mut present = true;
        for (n, s) in rec.nodal().iter().enumerate() {
            if boundary.contains(&n) {
                continue;
            }
            match s {
                Some(s) => errs.extend((0..6).map(|k| (s[k] - sig[k]).abs() / smax)),
                None => present = false,
            }
        }
        ch.holds(&format!("ES1 {} every interior node recovered", f.name()), present, "");
        ch.at_most(&format!("ES1 {} recovered nodal stress against D eps", f.name()), worst(errs), 1e-10);
        ch.at_most(
            &format!("ES1 {} energy_error estimate / solution norm", f.name()),
            ee.estimate() / ee.solution_norm(),
            1e-9,
        );
    }
    ch.finish();
}

/// `integral N_k(z) sigma(z) dz` of the hat function of node `k` of the grid `zs`, by two-point Gauss on each element.
fn hat_integral(zs: &[f64], k: usize, sigma: &dyn Fn(f64) -> f64) -> f64 {
    let mut total = 0.0;
    for e in 0..zs.len() - 1 {
        if e != k && e + 1 != k {
            continue;
        }
        let (a, b) = (zs[e], zs[e + 1]);
        let (mid, half) = (0.5 * (a + b), 0.5 * (b - a));
        for g in [-1.0 / 3f64.sqrt(), 1.0 / 3f64.sqrt()] {
            let z = mid + half * g;
            let n = if k == e { (b - z) / (b - a) } else { (z - a) / (b - a) };
            total += half * n * sigma(z);
        }
    }
    total
}

/// Pure bending of the `8 x 2 x 2` box: the end couple as the consistent nodal forces of the linear stress, supports
/// compatible with the exact field (ADR 0525 amendment 1): `u_x` on the root face, `u_y`, `u_z` at the root centre, `u_y` at the
/// root node `(y, z) = (h/2, h)`.
fn bending_run(f: SolidFormulation) -> (f64, f64, f64) {
    let (l, h, mom): (f64, f64, f64) = (2.0, 0.1, 10.0);
    let i = h.powi(4) / 12.0;
    let mut mesh = with_formulation(hex_box([0.0; 3], [l, h, h], [8, 2, 2]).unwrap(), f);
    let centre = node_at(&mesh, [0.0, 0.5 * h, 0.5 * h]);
    let top = node_at(&mesh, [0.0, 0.5 * h, h]);
    mesh.add_node_set("root_centre", vec![centre]).unwrap();
    mesh.add_node_set("root_top", vec![top]).unwrap();
    let mut m = model_with(mesh, steel(0.3));
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_centre", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_centre", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_top", 1, 0.0)).unwrap();
    let grid = [0.0, 0.5 * h, h];
    let sigma = |z: f64| mom * (z - 0.5 * h) / i;
    let one = |_: f64| 1.0;
    let mesh = m.mesh();
    let forces: Vec<NodalForceAt> = mesh
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| {
            let x = mesh.nodes()[n];
            let ky = grid.iter().position(|g| (g - x[1]).abs() < 1e-12).unwrap();
            let kz = grid.iter().position(|g| (g - x[2]).abs() < 1e-12).unwrap();
            NodalForceAt::new(n, [hat_integral(&grid, ky, &one) * hat_integral(&grid, kz, &sigma), 0.0, 0.0])
        })
        .collect();
    let res = solve_static_with(&m, &[], &forces, &StaticOptions::default()).unwrap();
    let smax = mom * 0.5 * h / i;
    let (rec, ee) = energy_error(&m, res.displacement_values(), &PatchOptions::new()).unwrap();
    let boundary = boundary_nodes(mesh);
    let mut errs = Vec::new();
    for (n, s) in rec.nodal().iter().enumerate() {
        if boundary.contains(&n) {
            continue;
        }
        let x = mesh.nodes()[n];
        let exact = [mom * (x[2] - 0.5 * h) / i, 0.0, 0.0, 0.0, 0.0, 0.0];
        match s {
            Some(s) => errs.extend((0..6).map(|k| (s[k] - exact[k]).abs() / smax)),
            None => errs.push(f64::NAN),
        }
    }
    (worst(errs), ee.estimate() / ee.solution_norm(), ee.solution_norm())
}

#[test]
fn es2_patch_recovery_of_pure_bending() {
    let mut ch = Checks::new();
    let (serr, rel, _) = bending_run(SolidFormulation::Incompatible);
    ch.at_most("ES2 Incompatible recovered nodal stress against M z' / I", serr, 1e-8);
    ch.at_most("ES2 Incompatible energy_error estimate / solution norm", rel, 1e-8);
    let (serr, rel, _) = bending_run(SolidFormulation::Full);
    println!("N-ES Full (observation): recovered stress error {serr:.4e}, estimate / norm {rel:.4e}");
    ch.finish();
}

// ------------------------------------------------------------------------------- damped modes and direct harmonic (a4, a5)

fn strip_model(f: SolidFormulation, two: bool, material: Material) -> SolidModel {
    let mut mesh = with_formulation(hex_box([0.0; 3], [2.0, 0.1, 0.1], [10, 1, 1]).unwrap(), f);
    let mut m;
    if two {
        mesh.add_element_set("a", (0..5).collect()).unwrap();
        mesh.add_element_set("b", (5..10).collect()).unwrap();
        m = SolidModel::new(mesh);
        m.add_material("a", material.clone()).unwrap();
        m.add_material("b", material).unwrap();
    } else {
        m = SolidModel::new(mesh);
        m.add_material("all", material).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e5])).unwrap();
    m
}

fn modal() -> ModalOptions {
    ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn visco(shear: f64, bulk: f64, tau: f64) -> Material {
    Material::viscoelastic(steel(0.3), vec![PronyTerm::new(shear, bulk, tau).unwrap()], None).unwrap()
}

fn g0() -> f64 {
    E_MOD / (2.0 * 1.3)
}

fn k0() -> f64 {
    E_MOD / (3.0 * (1.0 - 0.6))
}

/// `lambda^2` of a damped mode.
fn lambda_sq(re: f64, im: f64) -> (f64, f64) {
    (re * re - im * im, 2.0 * re * im)
}

fn omega0_sq(f: SolidFormulation) -> Vec<f64> {
    modal_analysis(&strip_model(f, false, steel(0.3)), &modal()).unwrap().eigenvalues
}

#[test]
fn dm1_loss_factor_damped_modes() {
    let mut ch = Checks::new();
    for f in FORMS {
        let m = strip_model(f, true, steel(0.3));
        let opts = DampedOptions::with_modal(modal())
            .loss_factor("a", LossFactor::Constant(0.02))
            .loss_factor("b", LossFactor::Constant(0.02));
        let d: DampedModes = damped_modes(&m, &opts).unwrap();
        let w2 = modal_analysis(&m, &modal()).unwrap().eigenvalues;
        ch.holds(&format!("DM1 {} four modes", f.name()), d.modes.len() >= 4, &format!("{}", d.modes.len()));
        let mut eta_err = Vec::new();
        let mut w_err = Vec::new();
        for (mode, w2) in d.modes.iter().zip(&w2).take(4) {
            let (re, im) = lambda_sq(mode.eigenvalue.re, mode.eigenvalue.im);
            eta_err.push((im / re / 0.02 - 1.0).abs());
            w_err.push((-re / w2 - 1.0).abs());
        }
        ch.at_most(&format!("DM1 {} |eta_mode / 0.02 - 1|", f.name()), worst(eta_err), 1e-6);
        ch.at_most(&format!("DM1 {} |(-Re lambda^2) / omega^2 - 1|", f.name()), worst(w_err), 1e-6);
    }
    ch.finish();
}

#[test]
fn dm2a_dm2b_dm2c_viscoelastic_damped_modes_of_bbar() {
    let mut ch = Checks::new();
    let f = SolidFormulation::BBar;
    let w2 = omega0_sq(f);
    let vopts = |o: DampedOptions| o.viscoelastic_reference(100.0, None);
    // DM2a: equal weights, K* = c K_0.
    let eq = visco(0.2, 0.2, 1.0e-2);
    let c = complex_moduli(&eq, 100.0, None).unwrap().shear;
    let c = (c.re / g0(), c.im / g0());
    let d = damped_modes(&strip_model(f, false, eq.clone()), &vopts(DampedOptions::with_modal(modal()))).unwrap();
    let (mut e1, mut e2) = (Vec::new(), Vec::new());
    for (mode, w2) in d.modes.iter().zip(&w2).take(4) {
        let (re, im) = lambda_sq(mode.eigenvalue.re, mode.eigenvalue.im);
        e1.push((im / re / (c.1 / c.0) - 1.0).abs());
        e2.push((-re / (w2 * c.0) - 1.0).abs());
    }
    ch.at_most("DM2a BBar |eta_mode / (Im c / Re c) - 1|", worst(e1), 1e-6);
    ch.at_most("DM2a BBar |(-Re lambda^2) / (omega_0^2 Re c) - 1|", worst(e2), 1e-6);
    // DM2b: the own frequency.
    let o = damped_modes(
        &strip_model(f, false, eq.clone()),
        &vopts(DampedOptions::with_modal(modal())).viscoelastic_own_frequency(OwnFrequency::new(8)),
    )
    .unwrap();
    let mut e3 = Vec::new();
    for ((mode, root), w2) in o.modes.iter().zip(&o.own_frequency).zip(&w2).take(4) {
        let cm = complex_moduli(&eq, root.omega, None).unwrap().shear;
        let (cr, ci) = (cm.re / g0(), cm.im / g0());
        let (lr, li) = lambda_sq(mode.eigenvalue.re, mode.eigenvalue.im);
        // lambda^2 / (-omega_0^2 c) - 1 as a complex quotient.
        let (dr, di) = (-w2 * cr, -w2 * ci);
        let den = dr * dr + di * di;
        let (qr, qi) = ((lr * dr + li * di) / den, (li * dr - lr * di) / den);
        e3.push(((qr - 1.0).powi(2) + qi * qi).sqrt());
    }
    ch.holds("DM2b BBar four own-frequency roots", e3.len() == 4, &format!("{}", e3.len()));
    ch.at_most("DM2b BBar |lambda^2 / (-omega_0^2 c(omega_own)) - 1|", worst(e3), 1e-6);
    // DM2c: unequal weights, the bounds.
    let un = visco(0.03, 0.01, 1.0e-2);
    let cm = complex_moduli(&un, 100.0, None).unwrap();
    let eta_g = cm.shear.im / cm.shear.re;
    let eta_k = cm.bulk.im / cm.bulk.re;
    let (re_g, re_k) = (cm.shear.re / g0(), cm.bulk.re / k0());
    let slack = 1e-9;
    let d = damped_modes(&strip_model(f, false, un), &vopts(DampedOptions::with_modal(modal()))).unwrap();
    let mut inside = true;
    for (mode, w2) in d.modes.iter().zip(&w2).take(4) {
        let (re, im) = lambda_sq(mode.eigenvalue.re, mode.eigenvalue.im);
        let eta = im / re;
        let ratio = -re / w2;
        let ok_eta = eta >= eta_g.min(eta_k) - slack && eta <= eta_g.max(eta_k) + slack;
        let ok_w = ratio >= re_g.min(re_k) - slack && ratio <= re_g.max(re_k) + slack;
        println!("DM2c mode: eta {eta:.6e} in [{:.6e}, {:.6e}], ratio {ratio:.9} in [{:.9}, {:.9}]", eta_g.min(eta_k), eta_g.max(eta_k), re_g.min(re_k), re_g.max(re_k));
        inside &= ok_eta && ok_w;
    }
    ch.holds("DM2c BBar unequal weights within the bounds", inside, "");
    ch.finish();
}

fn response_error(u_h: &rcfd_fem::harmonic::HarmonicResponse, eta: f64, u_s: &[f64]) -> f64 {
    // u_h (1 + i eta) against the real u_s.
    assert_eq!(u_h.re.len(), u_s.len());
    let umax = max_abs(u_s);
    worst(
        (0..u_s.len())
            .map(|i| {
                let re = u_h.re[i] - eta * u_h.im[i] - u_s[i];
                let im = u_h.im[i] + eta * u_h.re[i];
                (re * re + im * im).sqrt() / umax
            }),
    )
}

#[test]
fn dh1_dh2_direct_harmonic_with_loss_factors() {
    let mut ch = Checks::new();
    for f in FORMS {
        let m = strip_model(f, true, steel(0.3));
        let opts = DirectHarmonicOptions::new()
            .loss_factor("a", LossFactor::Constant(0.02))
            .loss_factor("b", LossFactor::Constant(0.02));
        let d = DirectHarmonic::new(&m, &opts).unwrap();
        let load = HarmonicLoad::from_model(&m).unwrap();
        let r = d.response(&load, 0.1).unwrap();
        let s = solve_static(&m, &StaticOptions::default()).unwrap();
        ch.at_most(
            &format!("DH1 {} max |u_h (1 + 0.02 i) - u_s| / max |u_s|", f.name()),
            response_error(&r, 0.02, s.displacement_values()),
            1e-6,
        );
        let st = d.stress(&m, &r).unwrap();
        let field = IntegrationPointField::compute(&m, s.displacement_values()).unwrap();
        let smax = max_abs(&flat6(field.stress()));
        let re = worst(
            st.re
                .iter()
                .zip(field.stress())
                .flat_map(|(a, b)| (0..6).map(move |k| (a[k] - b[k]).abs() / smax)),
        );
        let im = worst(st.im.iter().flat_map(|a| (0..6).map(move |k| a[k].abs() / smax)));
        ch.at_most(&format!("DH2 {} real part of the response stress against the static stress", f.name()), re, 1e-6);
        ch.at_most(&format!("DH2 {} imaginary part of the response stress", f.name()), im, 1e-6);
    }
    ch.finish();
}

/// The `E` and `nu` of the relaxed moduli `G_inf = g G_0`, `K_inf = k K_0`.
fn relaxed(g: f64, k: f64) -> Material {
    let (gi, ki) = (g * g0(), k * k0());
    let e = 9.0 * ki * gi / (3.0 * ki + gi);
    let nu = (3.0 * ki - 2.0 * gi) / (2.0 * (3.0 * ki + gi));
    Material::isotropic_elastic(e, nu, 7850.0, 1.2e-5).unwrap()
}

#[test]
fn dh3a_dh3b_dh3c_viscoelastic_static_limit_of_bbar() {
    let mut ch = Checks::new();
    let f = SolidFormulation::BBar;
    for (tag, shear, bulk, rel) in [("DH3a", 0.2, 0.2, (0.8, 0.8)), ("DH3b", 0.3, 0.1, (0.7, 0.9))] {
        let m = strip_model(f, false, visco(shear, bulk, 1.0));
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new()).unwrap();
        let load = HarmonicLoad::from_model(&m).unwrap();
        let r = d.response(&load, 1.0e-6).unwrap();
        let sm = strip_model(f, false, relaxed(rel.0, rel.1));
        let s = solve_static(&sm, &StaticOptions::default()).unwrap();
        let us = s.displacement_values();
        let umax = max_abs(us);
        let err = worst((0..us.len()).map(|i| ((r.re[i] - us[i]).powi(2) + r.im[i].powi(2)).sqrt() / umax));
        ch.at_most(&format!("{tag} BBar max |u_h - u_s| / max |u_s|"), err, 1e-5);
        if tag == "DH3b" {
            let st = d.stress(&m, &r).unwrap();
            let field = IntegrationPointField::compute(&sm, us).unwrap();
            let smax = max_abs(&flat6(field.stress()));
            let err = worst(
                st.re
                    .iter()
                    .zip(&st.im)
                    .zip(field.stress())
                    .flat_map(|((re, im), s)| (0..6).map(move |k| ((re[k] - s[k]).powi(2) + im[k].powi(2)).sqrt() / smax)),
            );
            ch.at_most("DH3c BBar response stress against the relaxed static stress", err, 1e-5);
        }
    }
    ch.finish();
}

// ---------------------------------------------------------------------------------------------- refusals (RF1 to RF3)

fn is_formulation_refusal<T>(r: rcfd_fem::Result<T>, f: SolidFormulation) -> (bool, String) {
    match r {
        Ok(_) => (false, "taken".to_string()),
        Err(e) => {
            let mut inner = &e;
            while let FemError::Element { source, .. } = inner {
                inner = source;
            }
            let ok = matches!(inner, FemError::NotYetSupported { item: "formulation", variant, package, .. }
                if *variant == f.name() && *package == rcfd_fem::hex8x::FOLLOW_UP);
            (ok, format!("{e:?}"))
        }
    }
}

const C_H: f64 = 0.1;

fn cantilever(f: SolidFormulation) -> SolidModel {
    let mesh = with_formulation(hex_box([0.0; 3], [2.0, C_H, C_H], [20, 1, 1]).unwrap(), f);
    let mut m = model_with(mesh, steel(0.3));
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1000.0 / (C_H * C_H)])).unwrap();
    m
}

#[test]
#[ignore = "record of the registered run 1 of ADR 0525 RF3: the estimate::recover row ran on the strip, which has no interior node (set-up defective, ADR 0525 amendment 2); the other 28 rows met; corrected as rf1_rf2_rf3_refusals_after_f588_amended"]
fn rf1_rf2_rf3_refusals_after_f588() {
    rf_rows(false);
}

#[test]
fn rf1_rf2_rf3_refusals_after_f588_amended() {
    rf_rows(true);
}

fn rf_rows(estimate_on_box: bool) {
    let mut ch = Checks::new();
    // RF1: the formulations are a Hex8 choice.
    let tet4 = tet_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let tet10 = upgrade_to_tet10(&tet4).unwrap().0;
    let hex20 = rcfd_fem::hex20::upgrade_to_hex20(&hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap())
        .unwrap()
        .0;
    for (name, mesh) in [("Tet4", tet4.clone()), ("Tet10", tet10), ("Hex20", hex20)] {
        for f in FORMS {
            let mut m = mesh.clone();
            let r = m.set_block_property(0, BlockProperty::solid(f));
            let ok = matches!(
                r,
                Err(FemError::UnsupportedKind {
                    operation: "a Hex8 formulation",
                    ..
                })
            );
            ch.holds(&format!("RF1 {} on {name} refused", f.name()), ok, &format!("{r:?}"));
        }
    }
    let mut m = tet4;
    let r = m.set_block_property(0, BlockProperty::solid(SolidFormulation::Full));
    ch.holds("RF1 Full on Tet4 taken", r.is_ok(), &format!("{r:?}"));

    // RF2: Incompatible with a viscoelastic material is refused on the damped modes and the direct solve; BBar takes both.
    for f in FORMS {
        let name = f.name();
        let m = strip_model(f, false, visco(0.03, 0.01, 1.0e-2));
        let r1 = damped_modes(&m, &DampedOptions::with_modal(modal()).viscoelastic_reference(100.0, None));
        let r2 = DirectHarmonic::new(&m, &DirectHarmonicOptions::new());
        if f == SolidFormulation::Incompatible {
            let (ok, msg) = is_formulation_refusal(r1, f);
            ch.holds(&format!("RF2 {name} viscoelastic damped_modes refused"), ok, &msg);
            let (ok, msg) = is_formulation_refusal(r2, f);
            ch.holds(&format!("RF2 {name} viscoelastic DirectHarmonic::new refused"), ok, &msg);
        } else {
            ch.holds(&format!("RF2 {name} viscoelastic damped_modes taken"), r1.is_ok(), &format!("{:?}", r1.err()));
            ch.holds(&format!("RF2 {name} viscoelastic DirectHarmonic::new taken"), r2.is_ok(), &format!("{:?}", r2.err()));
        }
    }

    for f in FORMS {
        let name = f.name();
        // RF3: the paths lifted are taken.
        let mut t = cantilever(f);
        let temps = vec![350.0; t.mesh().nodes().len()];
        t.add_load(Load::temperature(temps, 293.0)).unwrap();
        let n = t.dof_map().n_equations();
        let mesh = with_formulation(hex_box([0.0; 3], [1.0, 0.25, 0.25], [4, 1, 1]).unwrap(), f);
        let mut o = model_with(
            mesh,
            material_o([0.0; 3], MaterialAxes::Fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])),
        );
        for c in 0..3 {
            o.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        o.add_load(Load::traction("xmax", [1e5, 0.0, 0.0])).unwrap();
        let c = cantilever(f);
        let nc = c.dof_map().n_equations();
        let pre = Prestress::uniform(&c, [1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
        let u = vec![0.0; nc];
        let rows: Vec<(&str, bool, String)> = vec![
            {
                let r = solve_static(&t, &StaticOptions::default());
                ("solve_static with a temperature", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = nodal_load_vector(&t, &[]);
                ("nodal_load_vector with a temperature", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = IntegrationPointField::compute(&t, &vec![0.0; n]);
                ("IntegrationPointField with a temperature", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = solve_static(&o, &StaticOptions::default());
                ("orthotropic solve_static", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = geometric_stiffness(&c, &pre, None);
                ("geometric_stiffness", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = damped_modes(&c, &DampedOptions::new(2).rayleigh(1.0, 1.0e-6));
                ("damped_modes (Rayleigh)", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = DirectHarmonic::new(&c, &DirectHarmonicOptions::new());
                ("DirectHarmonic::new", r.is_ok(), format!("{:?}", r.err()))
            },
            {
                let r = if estimate_on_box {
                    // ADR 0525 amendment 2: a box with interior nodes (the strip has none).
                    let b = model_with(
                        with_formulation(hex_box([0.0; 3], [1.0, 0.5, 0.5], [4, 2, 2]).unwrap(), f),
                        steel(0.3),
                    );
                    let nb = b.dof_map().n_equations();
                    patch_recover(&b, &vec![0.0; nb], &PatchOptions::new())
                } else {
                    patch_recover(&c, &u, &PatchOptions::new())
                };
                ("estimate::recover", r.is_ok(), format!("{:?}", r.err()))
            },
        ];
        for (path, ok, msg) in rows {
            ch.holds(&format!("RF3 {name} {path} taken"), ok, &msg);
        }
        let states = rcfd_fem::nonlinear::initial_states(&c).unwrap();
        for (path, r) in [
            (
                "nonlinear::internal_force",
                rcfd_fem::nonlinear::internal_force(&c, &u, &states).map(|_| ()),
            ),
            (
                "nonlinear::tangent",
                rcfd_fem::nonlinear::tangent(&c, &u, &states, None).map(|_| ()),
            ),
        ] {
            // ADR 0506 D7: the nonlinear rows of BBar are taken; Incompatible keeps the refusal with the key.
            if f == SolidFormulation::BBar {
                ch.holds(&format!("RF3 {name} {path} taken (ADR 0506)"), r.is_ok(), &format!("{r:?}"));
            } else {
                let ok = matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", .. }));
                ch.holds(&format!("RF3 {name} {path} refused"), ok, &format!("{r:?}"));
            }
        }
    }
    ch.finish();
}
