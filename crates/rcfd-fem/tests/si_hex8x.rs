//! F-390 (ADR 0493): Hex8 without locking, the mean-dilatation (B-bar) and the incompatible-mode formulations, opt-in per
//! element block. Checks PT1, PT2, RS1 to RS3, EV1 to EV3, C1 to C4b, T1, T2, X1, D1, D2, RF1 to RF4 and the negative controls
//! N1 to N4 with the observations O1 to O4 of the registration (B1 is in `si_hex8x_bits.rs`). Every aggregated error is
//! required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{assemble_with, stiffness, stiffness_with};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::element::{b_matrix, element_for, ElementMatrix};
use rcfd_fem::estimate::{recover as patch_recover, PatchOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex8x::{element_stiffness, internal_modes, point_strains};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::material::{Matrix6, MaterialAxes};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::quadrature;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel, SolidResult,
};
use rcfd_linalg::eigen::symmetric_eigen;
use std::collections::BTreeSet;
use std::f64::consts::PI;

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
        println!("{name}: {value:.4e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!("{name}: {value:.6} (limit < {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!("{name}: {value:.4e} (limit > {limit:e}) {}", if ok { "met" } else { "MISSED" });
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

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, 7850.0, 1.2e-5).unwrap()
}

fn dmat(nu: f64) -> Matrix6 {
    steel(nu).elasticity_matrix().unwrap()
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
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

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(nu)).unwrap();
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

// ------------------------------------------------------------------------------------------------------- the patch (a2)

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

/// The patch box of reading (a2), interior nodes moved by `amp h_k (2U - 1)`.
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

/// Interior error relative to the largest field component, over the unknowns of `u` (one per node and component).
fn interior_error(mesh: &SolidMesh, u: &dyn Fn(usize) -> Vec3) -> f64 {
    let boundary = boundary_nodes(mesh);
    let mut umax = 0.0f64;
    let mut errs = Vec::new();
    for (n, x) in mesh.nodes().iter().enumerate() {
        let ue = linear_field(*x);
        let uh = u(n);
        for k in 0..3 {
            umax = umax.max(ue[k].abs());
            if !boundary.contains(&n) {
                errs.push((uh[k] - ue[k]).abs());
            }
        }
    }
    worst(errs) / umax
}

/// Solves the patch; returns (model, result, interior error, recovered-stress error).
fn patch_run(amp: f64, tag: u64, nu: f64, f: SolidFormulation) -> (SolidModel, SolidResult, f64, f64) {
    let mesh = with_formulation(patch_mesh(amp, tag), f);
    let model = model_of(mesh, nu);
    let pres = patch_prescribed(model.mesh());
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    let err = interior_error(model.mesh(), &|n| {
        let d = res.displacement(n);
        [d[0], d[1], d[2]]
    });
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let sig = apply(&dmat(nu), &field_strain());
    let smax = worst(sig.iter().map(|s| s.abs()));
    let serr = worst(
        field
            .stress()
            .iter()
            .flat_map(|s| (0..6).map(move |k| (s[k] - sig[k]).abs() / smax)),
    );
    (model, res, err, serr)
}

#[test]
fn pt1_pt2_rs1_patch_tests_and_recovered_stress() {
    let mut ch = Checks::new();
    for f in FORMS {
        let name = f.name();
        let (_, _, err, serr) = patch_run(0.1, 11, 0.3, f);
        ch.at_most(&format!("PT1 {name} 0.1 h interior error"), err, 1e-10);
        ch.at_most(&format!("RS1 {name} 0.1 h stress against D eps"), serr, 1e-10);
        for nu in [0.3, 0.4999] {
            let (_, _, err, serr) = patch_run(0.25, 1001, nu, f);
            ch.at_most(&format!("PT2 {name} 0.25 h nu {nu} interior error"), err, 1e-10);
            ch.at_most(&format!("RS1 {name} 0.25 h nu {nu} stress against D eps"), serr, 1e-10);
        }
    }
    ch.finish();
}

// -------------------------------------------------------- the test's own incompatible element (X1, N1, RS3)

/// Dense solve of `a x = b` by Gauss elimination with partial pivoting (`a` n x n row-major).
fn dense_solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Vec<f64> {
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs())).unwrap();
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

/// `B_q` (6 x 24), `G_q` (6 x 9) and `w det J_q` at the points of `gauss_2`, the modes' gradients corrected (Taylor) or not
/// (Wilson, the Jacobian at the point).
fn test_modes(coords: &[Vec3], corrected: bool) -> Vec<(Vec<f64>, Vec<f64>, f64)> {
    let el = element_for(ElementKind::Hex8);
    let c = el.point(coords, [0.0; 3]).unwrap();
    let rule = quadrature::gauss_2();
    rule.points()
        .iter()
        .zip(rule.weights())
        .map(|(&xi, &w)| {
            let p = el.point(coords, xi).unwrap();
            let (j, scale) = if corrected {
                (c.jacobian, c.det_j / p.det_j)
            } else {
                (p.jacobian, 1.0)
            };
            // J^{-1} by solving J x = e_m.
            let jm: Vec<f64> = j.iter().flatten().copied().collect();
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

fn btdb(b1: &[f64], n1: usize, d: &Matrix6, b2: &[f64], n2: usize, w: f64, out: &mut [f64]) {
    for i in 0..n1 {
        for j in 0..n2 {
            let mut s = 0.0;
            for r in 0..6 {
                for m in 0..6 {
                    s += b1[r * n1 + i] * d[r][m] * b2[m * n2 + j];
                }
            }
            out[i * n2 + j] += w * s;
        }
    }
}

/// The test's incompatible element: the condensed stiffness and `alpha(u)`.
fn test_incompatible(coords: &[Vec3], d: &Matrix6, corrected: bool) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let parts = test_modes(coords, corrected);
    let (mut kuu, mut kua, mut kaa) = (vec![0.0; 576], vec![0.0; 216], vec![0.0; 81]);
    for (b, g, w) in &parts {
        btdb(b, 24, d, b, 24, *w, &mut kuu);
        btdb(b, 24, d, g, 9, *w, &mut kua);
        btdb(g, 9, d, g, 9, *w, &mut kaa);
    }
    let mut k = kuu.clone();
    for j in 0..24 {
        let col: Vec<f64> = (0..9).map(|a| kua[j * 9 + a]).collect();
        let x = dense_solve(kaa.clone(), col, 9);
        for i in 0..24 {
            let s: f64 = (0..9).map(|a| kua[i * 9 + a] * x[a]).sum();
            k[i * 24 + j] -= s;
        }
    }
    (k, kua, kaa)
}

fn test_alpha(kua: &[f64], kaa: &[f64], u: &[f64]) -> Vec<f64> {
    let rhs: Vec<f64> = (0..9).map(|a| -(0..24).map(|i| kua[i * 9 + a] * u[i]).sum::<f64>()).collect();
    dense_solve(kaa.to_vec(), rhs, 9)
}

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

/// The distorted single element of `est1.py`.
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

#[test]
fn n1_uncorrected_modes_fail_the_patch_and_x1_the_element_agrees() {
    let mut ch = Checks::new();
    // N1: Wilson's element without the correction on PT1's patch, through assemble_with, solved densely.
    let model = model_of(patch_mesh(0.1, 11), 0.3);
    let d = dmat(0.3);
    let k = assemble_with(&model, None, |ctx| {
        let (kc, _, _) = test_incompatible(ctx.coords, &d, false);
        let mut m = ElementMatrix::zeros(24);
        m.values_mut().copy_from_slice(&kc);
        Ok(m)
    })
    .unwrap();
    let mesh = model.mesh();
    let n = k.n;
    let boundary = boundary_nodes(mesh);
    let mut u = vec![0.0; n];
    let mut fixed = vec![false; n];
    for &b in &boundary {
        let ub = linear_field(mesh.nodes()[b]);
        for c in 0..3 {
            u[3 * b + c] = ub[c];
            fixed[3 * b + c] = true;
        }
    }
    let free: Vec<usize> = (0..n).filter(|&i| !fixed[i]).collect();
    let mut dense = vec![0.0; n * n];
    for i in 0..n {
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            dense[i * n + k.col[p]] = k.val[p];
        }
    }
    let nf = free.len();
    let mut a = vec![0.0; nf * nf];
    let mut rhs = vec![0.0; nf];
    for (r, &i) in free.iter().enumerate() {
        for (c, &j) in free.iter().enumerate() {
            a[r * nf + c] = dense[i * n + j];
        }
        rhs[r] = -(0..n).filter(|&j| fixed[j]).map(|j| dense[i * n + j] * u[j]).sum::<f64>();
    }
    let x = dense_solve(a, rhs, nf);
    for (r, &i) in free.iter().enumerate() {
        u[i] = x[r];
    }
    let err = interior_error(mesh, &|nd| [u[3 * nd], u[3 * nd + 1], u[3 * nd + 2]]);
    ch.above("N1 uncorrected incompatible element, PT1 interior error", err, 1e-6);

    // X1: the test's corrected element against the library on EV2's element and on PT1's elements.
    let mut xs = Vec::new();
    let mut elements: Vec<Vec<Vec3>> = vec![distorted_element()];
    for e in 0..mesh.n_elements() {
        elements.push(element_coords(mesh, e));
    }
    for coords in &elements {
        let (kt, _, _) = test_incompatible(coords, &d, true);
        let kl = element_stiffness(SolidFormulation::Incompatible, coords, &d).unwrap();
        let scale = max_abs(kl.values());
        xs.push(max_abs(&kt.iter().zip(kl.values()).map(|(a, b)| a - b).collect::<Vec<_>>()) / scale);
    }
    ch.at_most("X1 test element against element_stiffness(Incompatible), relative", worst(xs), 1e-11);
    ch.finish();
}

// -------------------------------------------------------------------------------------------- single elements (a3)

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
            let v = [w[1] * x[2] - w[2] * x[1], w[2] * x[0] - w[0] * x[2], w[0] * x[1] - w[1] * x[0]];
            r[3 * a..3 * a + 3].copy_from_slice(&v);
        }
        out.push(r);
    }
    out
}

fn eigen_checks(ch: &mut Checks, tag: &str, coords: &[Vec3], nu: f64, record_only: bool) {
    let d = dmat(nu);
    for f in [SolidFormulation::Full, SolidFormulation::BBar, SolidFormulation::Incompatible] {
        let k = element_stiffness(f, coords, &d).unwrap();
        let ev = symmetric_eigen(k.values(), 24).unwrap();
        let lmax = ev.values[23];
        let zeros = ev.values.iter().filter(|l| l.abs() <= 1e-10 * lmax).count();
        let r7 = ev.values[6] / lmax;
        let name = f.name();
        if record_only || f == SolidFormulation::Full {
            println!("record {tag} {name} nu {nu}: zeros {zeros}, lambda_7 / lambda_max {r7:.4e}");
            continue;
        }
        ch.holds(
            &format!("EV {tag} {name} zero eigenvalues"),
            zeros == 6,
            &format!("{zeros} (|lambda_6| / lambda_max {:.2e})", ev.values[5].abs() / lmax),
        );
        ch.above(&format!("EV {tag} {name} lambda_7 / lambda_max"), r7, 1e-6);
        let kv = k.values();
        let kmax = max_abs(kv);
        let sym = worst((0..24).flat_map(|i| (0..24).map(move |j| (kv[i * 24 + j] - kv[j * 24 + i]).abs())));
        ch.at_most(&format!("EV3 {tag} {name} max |K - K^T| / max |K|"), sym / kmax, 1e-13);
        let rig = worst(rigid_motions(coords).iter().map(|r| max_abs(&k.matvec(r)) / (kmax * max_abs(r))));
        ch.at_most(&format!("EV3 {tag} {name} rigid motions |K r| / (max |K| max |r|)"), rig, 1e-11);
    }
}

#[test]
fn ev1_ev2_ev3_single_element_zero_modes() {
    let mut ch = Checks::new();
    for ar in [1.0, 10.0, 100.0] {
        let coords: Vec<Vec3> = ElementKind::Hex8
            .reference_nodes()
            .iter()
            .map(|r| [ar * 0.5 * (r[0] + 1.0), 0.5 * (r[1] + 1.0), 0.5 * (r[2] + 1.0)])
            .collect();
        eigen_checks(&mut ch, &format!("EV1 box {ar} x 1 x 1"), &coords, 0.3, false);
        eigen_checks(&mut ch, &format!("O4 box {ar} x 1 x 1"), &coords, 0.4999, true);
    }
    eigen_checks(&mut ch, "EV2 distorted", &distorted_element(), 0.3, false);
    ch.finish();
}

// ------------------------------------------------------------------------------------------ the cantilever (a4, a5)

const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Support {
    /// Reading (a4): `u_x`, `u_z` on the root face, `u_y` at the root node `(0, 0, 0)`.
    Beam,
    /// O1: every component on the root face.
    Clamp,
}

fn cantilever(length: f64, nu: f64, f: SolidFormulation, support: Support) -> SolidModel {
    let mut mesh = with_formulation(hex_box([0.0; 3], [length, C_H, C_H], [20, 1, 1]).unwrap(), f);
    let corner = mesh
        .node_set("xmin")
        .unwrap()
        .iter()
        .copied()
        .find(|&n| mesh.nodes()[n] == [0.0, 0.0, 0.0])
        .unwrap();
    mesh.add_node_set("root_corner", vec![corner]).unwrap();
    let mut m = model_of(mesh, nu);
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
    match support {
        Support::Beam => m.add_constraint(Constraint::displacement("root_corner", 1, 0.0)).unwrap(),
        Support::Clamp => m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap(),
    }
    m
}

fn tip_shear(mut m: SolidModel) -> SolidModel {
    m.add_load(Load::traction("xmax", [0.0, 0.0, -C_P / (C_H * C_H)])).unwrap();
    m
}

fn delta_t(length: f64, nu: f64) -> f64 {
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + nu));
    C_P * length.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (5.0 / 6.0 * g * C_H * C_H * length * length))
}

/// `delta / delta_T` with `delta = sum f u / P` (ADR 0433 D3).
fn ratio(m: &SolidModel, length: f64, nu: f64) -> (SolidResult, f64) {
    let res = solve_static(m, &StaticOptions::default()).unwrap();
    let f = nodal_load_vector(m, &[]).unwrap();
    let delta: f64 = f.iter().zip(res.displacement_values()).map(|(f, u)| f * u).sum::<f64>() / C_P;
    (res, delta / delta_t(length, nu))
}

#[test]
fn c1_to_c4b_cantilever_one_element_through_the_thickness() {
    let mut ch = Checks::new();
    let inc = SolidFormulation::Incompatible;
    let full = SolidFormulation::Full;
    let (_, r) = ratio(&tip_shear(cantilever(2.0, 0.0, inc, Support::Beam)), 2.0, 0.0);
    ch.at_most("C1 Incompatible L/h 20 nu 0 |delta / delta_T - 1|", (r - 1.0).abs(), 0.01);
    let (_, r) = ratio(&tip_shear(cantilever(2.0, 0.3, inc, Support::Beam)), 2.0, 0.3);
    ch.at_most("C2 Incompatible L/h 20 nu 0.3 |delta / delta_T - 1|", (r - 1.0).abs(), 0.03);
    for nu in [0.0, 0.3] {
        let (_, r) = ratio(&tip_shear(cantilever(2.0, nu, full, Support::Beam)), 2.0, nu);
        ch.below(&format!("C3 Full L/h 20 nu {nu} delta / delta_T (locking control)"), r, 0.8);
    }
    let (_, r) = ratio(&tip_shear(cantilever(10.0, 0.3, inc, Support::Beam)), 10.0, 0.3);
    ch.at_most("C4 Incompatible L/h 100 AR 5 nu 0.3 |delta / delta_T - 1|", (r - 1.0).abs(), 0.02);
    let (_, r) = ratio(&tip_shear(cantilever(10.0, 0.3, full, Support::Beam)), 10.0, 0.3);
    ch.below("C4b Full L/h 100 AR 5 nu 0.3 delta / delta_T (control)", r, 0.8);
    // O1, O2: records.
    for nu in [0.0, 0.3] {
        let (_, r) = ratio(&tip_shear(cantilever(2.0, nu, inc, Support::Clamp)), 2.0, nu);
        println!("O1 Incompatible L/h 20 nu {nu} full clamp: delta / delta_T {r:.6}");
        let (_, r) = ratio(&tip_shear(cantilever(2.0, nu, SolidFormulation::BBar, Support::Beam)), 2.0, nu);
        println!("O2 BBar L/h 20 nu {nu}: delta / delta_T {r:.6}");
    }
    let (_, r) = ratio(&tip_shear(cantilever(10.0, 0.3, SolidFormulation::BBar, Support::Beam)), 10.0, 0.3);
    println!("O2 BBar L/h 100 AR 5 nu 0.3: delta / delta_T {r:.6}");
    ch.finish();
}

// ------------------------------------------------------------------------------- recovery of the modes (a7: RS2, RS3)

#[test]
fn rs2_n3_pure_bending_stress_with_the_recovered_modes() {
    let mut ch = Checks::new();
    let (l, h, m_end) = (2.0, C_H, 10.0);
    let i = h.powi(4) / 12.0;
    let el = element_for(ElementKind::Hex8);
    for nu in [0.0, 0.3] {
        let model = cantilever(l, nu, SolidFormulation::Incompatible, Support::Beam);
        let mesh = model.mesh();
        let forces: Vec<NodalForceAt> = mesh
            .node_set("xmax")
            .unwrap()
            .iter()
            .map(|&n| {
                let s = if mesh.nodes()[n][2] > 0.5 * h { 1.0 } else { -1.0 };
                NodalForceAt::new(n, [s * m_end / (2.0 * h), 0.0, 0.0])
            })
            .collect();
        let res = solve_static_with(&model, &[], &forces, &StaticOptions::default()).unwrap();
        let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
        let smax = m_end * 0.5 * h / i;
        let d = dmat(nu);
        let mut errs = Vec::new();
        let mut errs_compatible = Vec::new();
        for e in 0..mesh.n_elements() {
            let coords = element_coords(mesh, e);
            let ue = element_u(&res, mesh, e);
            for (q, p) in field.points_of(e).enumerate() {
                let x = field.positions()[p];
                let mut exact = [0.0; 6];
                exact[0] = m_end * (x[2] - 0.5 * h) / i;
                let s = field.stress()[p];
                errs.push(worst((0..6).map(|k| (s[k] - exact[k]).abs() / smax)));
                let xi = quadrature::gauss_2().points()[q];
                let sc = apply(&d, &el.strain_at(&coords, &ue, xi).unwrap());
                errs_compatible.push(worst((0..6).map(|k| (sc[k] - exact[k]).abs() / smax)));
            }
        }
        ch.at_most(&format!("RS2 nu {nu} recovered stress against M z' / I"), worst(errs), 1e-8);
        let tip = mesh.node_set("xmax").unwrap();
        let w = tip.iter().map(|&n| res.displacement(n)[2]).sum::<f64>() / tip.len() as f64;
        let wt = m_end * l * l / (2.0 * E_MOD * i);
        ch.at_most(&format!("RS2 nu {nu} |-mean tip u_z / (M L^2 / (2 E I)) - 1| (the tip moves down)"), (-w / wt - 1.0).abs(), 1e-8);
        ch.above(&format!("N3 nu {nu} compatible strain only, stress error"), worst(errs_compatible), 0.1);
    }
    ch.finish();
}

#[test]
fn rs3_n4_x1_element_identities_with_the_recovered_stress() {
    let mut ch = Checks::new();
    let nu = 0.3;
    let d = dmat(nu);
    let model = tip_shear(cantilever(2.0, nu, SolidFormulation::Incompatible, Support::Beam));
    let res = solve_static(&model, &StaticOptions::default()).unwrap();
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let mesh = model.mesh();
    let el = element_for(ElementKind::Hex8);
    let (mut id1, mut id2, mut id1c, mut xa) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for e in 0..mesh.n_elements() {
        let coords = element_coords(mesh, e);
        let ue = element_u(&res, mesh, e);
        let ke = element_stiffness(SolidFormulation::Incompatible, &coords, &d).unwrap();
        let ku = ke.matvec(&ue);
        let parts = test_modes(&coords, true);
        let (mut fint, mut fa, mut fa_scale, mut fc) = (vec![0.0; 24], vec![0.0; 9], vec![0.0; 9], vec![0.0; 24]);
        for (q, (b, g, w)) in parts.iter().enumerate() {
            let s = field.stress()[field.points_of(e).start + q];
            let xi = quadrature::gauss_2().points()[q];
            let sc = apply(&d, &el.strain_at(&coords, &ue, xi).unwrap());
            for c in 0..24 {
                for r in 0..6 {
                    fint[c] += w * b[r * 24 + c] * s[r];
                    fc[c] += w * b[r * 24 + c] * sc[r];
                }
            }
            for c in 0..9 {
                for r in 0..6 {
                    fa[c] += w * g[r * 9 + c] * s[r];
                    fa_scale[c] += w * (g[r * 9 + c] * s[r]).abs();
                }
            }
        }
        let kumax = max_abs(&ku);
        id1.push(max_abs(&fint.iter().zip(&ku).map(|(a, b)| a - b).collect::<Vec<_>>()) / kumax);
        id1c.push(max_abs(&fc.iter().zip(&ku).map(|(a, b)| a - b).collect::<Vec<_>>()) / kumax);
        id2.push(max_abs(&fa) / max_abs(&fa_scale));
        // X1: internal_modes against the test's alpha.
        let (_, kua, kaa) = test_incompatible(&coords, &d, true);
        let at = test_alpha(&kua, &kaa, &ue);
        let al = internal_modes(&coords, &d, &ue).unwrap();
        xa.push(max_abs(&at.iter().zip(&al).map(|(a, b)| a - b).collect::<Vec<_>>()) / max_abs(&at));
    }
    ch.at_most("RS3 |sum w B^T sigma - K_e u_e| / max |K_e u_e|", worst(id1), 1e-9);
    ch.at_most("RS3 |sum w G^T sigma| / max sum w |G^T sigma|", worst(id2), 1e-9);
    ch.above("N4 compatible strain only, first identity", worst(id1c), 0.1);
    ch.at_most("X1 internal_modes against the test's alpha, relative", worst(xa), 1e-9);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------- the cylinder (a6)

const A_IN: f64 = 1.0;
const B_OUT: f64 = 2.0;
const PRESS: f64 = 1.0e6;

fn polar(p: Vec3) -> Vec3 {
    let r = A_IN + (B_OUT - A_IN) * ((2.0 * p[0]).exp() - 1.0) / (2f64.exp() - 1.0);
    let th = 0.5 * PI * p[1];
    [r * th.cos(), r * th.sin(), p[2]]
}

fn lame_bore(nu: f64) -> f64 {
    let a = PRESS * A_IN * A_IN / (B_OUT * B_OUT - A_IN * A_IN);
    let b = a * B_OUT * B_OUT;
    (1.0 + nu) / E_MOD * ((1.0 - 2.0 * nu) * a * A_IN + b / A_IN)
}

/// Bore `u_r / u_r^Lame(a)` at every bore node of the k = 2 cylinder.
fn cylinder_ratios(f: SolidFormulation, nu: f64) -> Vec<f64> {
    let base = hex_box([0.0; 3], [1.0, 1.0, 0.1], [8, 16, 1]).unwrap();
    let nodes = base.nodes().iter().map(|&p| polar(p)).collect();
    let mut m = model_of(with_formulation(rebuild(&base, nodes), f), nu);
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", PRESS)).unwrap();
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let mesh = m.mesh();
    mesh.node_set("xmin")
        .unwrap()
        .iter()
        .map(|&n| {
            let (p, u) = (mesh.nodes()[n], res.displacement(n));
            (u[0] * p[0] + u[1] * p[1]) / p[0].hypot(p[1]) / lame_bore(nu)
        })
        .collect()
}

#[test]
fn t1_t2_cylinder_nearly_incompressible() {
    let mut ch = Checks::new();
    let nu = 0.4999;
    let bb = cylinder_ratios(SolidFormulation::BBar, nu);
    ch.at_most("T1 BBar k=2 nu 0.4999 bore max |u_r / u_Lame - 1|", worst(bb.iter().map(|r| (r - 1.0).abs())), 0.01);
    let fu = cylinder_ratios(SolidFormulation::Full, nu);
    ch.below("T2 Full k=2 nu 0.4999 bore max u_r / u_Lame (control)", worst(fu.iter().copied()), 0.9);
    let ic = cylinder_ratios(SolidFormulation::Incompatible, nu);
    println!("O3 Incompatible k=2 nu 0.4999 bore max |u_r / u_Lame - 1| {:.4e}", worst(ic.iter().map(|r| (r - 1.0).abs())));
    ch.finish();
}

// ----------------------------------------------------------------------------------------- dispatch (D1, D2, RF4)

#[test]
fn d1_d2_rf4_dispatch_is_the_element() {
    let mut ch = Checks::new();
    let d = dmat(0.3);
    for f in FORMS {
        let name = f.name();
        // D1: a single-element model.
        let coords = distorted_element();
        let mut mesh = SolidMesh::new(coords.clone()).unwrap();
        mesh.add_block(ElementKind::Hex8, (0..8).collect()).unwrap();
        mesh.add_element_set("all", vec![0]).unwrap();
        let model = model_of(with_formulation(mesh, f), 0.3);
        let k = stiffness(&model).unwrap();
        let ke = element_stiffness(f, &coords, &d).unwrap();
        let mut same = true;
        for i in 0..24 {
            for p in k.row_ptr[i]..k.row_ptr[i + 1] {
                same &= k.val[p].to_bits() == ke.get(i, k.col[p]).to_bits();
            }
        }
        ch.holds(&format!("D1 {name} single-element stiffness bit for bit"), same && k.val.len() == 576, "");
        // D1: threads.
        let c = tip_shear(cantilever(2.0, 0.3, f, Support::Beam));
        let k1 = stiffness_with(&c, Some(1)).unwrap();
        let k8 = stiffness_with(&c, Some(8)).unwrap();
        let bits = k1.val.iter().zip(&k8.val).all(|(a, b)| a.to_bits() == b.to_bits()) && k1.col == k8.col;
        ch.holds(&format!("D1 {name} cantilever 1 and 8 threads bit for bit"), bits, "");
        // D2: the recovered strains are point_strains.
        let res = solve_static(&c, &StaticOptions::default()).unwrap();
        let field = IntegrationPointField::compute(&c, res.displacement_values()).unwrap();
        let mesh = c.mesh();
        let mut same = true;
        for e in 0..mesh.n_elements() {
            let s = point_strains(f, &element_coords(mesh, e), &d, &element_u(&res, mesh, e)).unwrap();
            for (q, p) in field.points_of(e).enumerate() {
                same &= (0..6).all(|r| field.strain()[p][r].to_bits() == s[q][r].to_bits());
            }
        }
        ch.holds(&format!("D2 {name} cantilever field strains bit for bit"), same, "");
        // RF4: the modal analysis takes it.
        let modal = modal_analysis(&c, &ModalOptions::new(2)).unwrap();
        let ok = modal.frequencies.len() == 2 && modal.frequencies.iter().all(|f| f.is_finite() && *f > 0.0);
        ch.holds(&format!("RF4 {name} modal analysis taken"), ok, &format!("{:?} Hz", modal.frequencies));
    }
    // D2 for BBar on PT1.
    let (model, res, _, _) = patch_run(0.1, 11, 0.3, SolidFormulation::BBar);
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let mesh = model.mesh();
    let mut same = true;
    for e in 0..mesh.n_elements() {
        let s = point_strains(SolidFormulation::BBar, &element_coords(mesh, e), &d, &element_u(&res, mesh, e)).unwrap();
        for (q, p) in field.points_of(e).enumerate() {
            same &= (0..6).all(|r| field.strain()[p][r].to_bits() == s[q][r].to_bits());
        }
    }
    ch.holds("D2 BBar PT1 field strains bit for bit", same, "");
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

#[test]
fn rf1_rf2_rf3_refusals() {
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
            let ok = matches!(r, Err(FemError::UnsupportedKind { operation: "a Hex8 formulation", .. }));
            ch.holds(&format!("RF1 {} on {name} refused", f.name()), ok, &format!("{r:?}"));
        }
    }
    let mut m = tet4;
    let r = m.set_block_property(0, BlockProperty::solid(SolidFormulation::Full));
    ch.holds("RF1 Full on Tet4 taken", r.is_ok(), &format!("{r:?}"));

    for f in FORMS {
        let name = f.name();
        // RF2: a temperature load.
        let mut t = tip_shear(cantilever(2.0, 0.3, f, Support::Beam));
        let temps = vec![350.0; t.mesh().nodes().len()];
        t.add_load(Load::temperature(temps, 293.0)).unwrap();
        let n = t.dof_map().n_equations();
        for (path, (ok, msg)) in [
            ("solve_static", is_formulation_refusal(solve_static(&t, &StaticOptions::default()), f)),
            ("nodal_load_vector", is_formulation_refusal(nodal_load_vector(&t, &[]), f)),
            ("IntegrationPointField", is_formulation_refusal(IntegrationPointField::compute(&t, &vec![0.0; n]), f)),
        ] {
            ch.holds(&format!("RF2 {name} temperature {path} refused"), ok, &msg);
        }
        // RF2: a per-point material.
        let mesh = with_formulation(hex_box([0.0; 3], [1.0, 0.25, 0.25], [4, 1, 1]).unwrap(), f);
        let mut o = SolidModel::new(mesh);
        let ortho = Material::orthotropic(
            [1.4e11, 1.0e10, 1.0e10],
            [0.3, 0.3, 0.4],
            [5.0e9, 5.0e9, 3.5e9],
            1600.0,
            [0.0; 3],
            MaterialAxes::Fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        )
        .unwrap();
        o.add_material("all", ortho).unwrap();
        for c in 0..3 {
            o.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        o.add_load(Load::traction("xmax", [1e5, 0.0, 0.0])).unwrap();
        let (ok, msg) = is_formulation_refusal(solve_static(&o, &StaticOptions::default()), f);
        ch.holds(&format!("RF2 {name} orthotropic solve_static refused"), ok, &msg);

        // RF3: the paths of D5.
        let c = tip_shear(cantilever(2.0, 0.3, f, Support::Beam));
        let n = c.dof_map().n_equations();
        let pre = Prestress::uniform(&c, [1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
        let u = vec![0.0; n];
        for (path, (ok, msg)) in [
            ("geometric_stiffness", is_formulation_refusal(geometric_stiffness(&c, &pre, None), f)),
            ("damped_modes", is_formulation_refusal(damped_modes(&c, &DampedOptions::new(2)), f)),
            ("DirectHarmonic::new", is_formulation_refusal(DirectHarmonic::new(&c, &DirectHarmonicOptions::new()), f)),
            ("estimate::recover", is_formulation_refusal(patch_recover(&c, &u, &PatchOptions::new()), f)),
        ] {
            ch.holds(&format!("RF3 {name} {path} refused"), ok, &msg);
        }
        let states = rcfd_fem::nonlinear::initial_states(&c).unwrap();
        for (path, r) in [
            ("nonlinear::internal_force", rcfd_fem::nonlinear::internal_force(&c, &u, &states).map(|_| ())),
            ("nonlinear::tangent", rcfd_fem::nonlinear::tangent(&c, &u, &states, None).map(|_| ())),
        ] {
            let ok = matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", .. }));
            ch.holds(&format!("RF3 {name} {path} refused"), ok, &format!("{r:?}"));
        }
    }
    ch.finish();
}
