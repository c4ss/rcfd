//! F-703 (ADR 0630): the coefficients of the stabilisation of the incompatible-modes Hex8 without the bulk modulus (rule D,
//! `Deviatoric`) and linear in the rest stress (rule L, `LinearStress`, diagnostic), against the coefficient of ADR 0614
//! (`RestStress`) and the F-bar Hex8 of ADR 0506 on the same patch (the locking-free reference): the heavy records G4 (S1D,
//! S1L, S1R, K0, O2, F1, F2, LK, R2) of the six cases, the fast companion HF3, the coefficients S2b, S3b, S3c, S3d, the
//! equilibrium H3c and the strain energy E1, E3 of the rules. The copies R-* of the ADR 0608 checks are in
//! `si_eas_hg3_rd.rs` and `si_eas_hg3_rl.rs`, the bit record B3 in `si_eas_hg3_bits.rs`. Every aggregated value is required
//! finite before it is compared. The helpers are copies of those of `si_eas_hg2.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, Matrix6, PointContext};
use rcfd_fem::nl_incompatible::{
    enhanced_force_and_tangent_with, modes_and_strains_with, stabilisation_coefficient, stabilisation_coefficient_with,
    IncompatibleStabilisation,
};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, internal_force, tangent};
use rcfd_fem::{BlockProperty, ElementKind, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{generalized_eigen_dense, symmetric_eigen};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;
const FULL: SolidFormulation = SolidFormulation::Full;

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

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

/// SplitMix64 from a seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
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

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

/// The element unknowns of `u` (one per equation; every node of a solid has three).
fn element_u(model: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    model
        .mesh()
        .element_nodes(e)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect()
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
}

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn det3(j: &[[f64; 3]; 3]) -> f64 {
    j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
        + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0])
}

fn inverse3(j: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = det3(j);
    let mut inv = [[0.0; 3]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            *v = (j[r1][c1] * j[r2][c2] - j[r1][c2] * j[r2][c1]) / det;
        }
    }
    inv
}

fn empty() -> ElementState {
    ElementState::new(Vec::new())
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

// ============================================================================ the material, the patch and the states

const MU: f64 = 1.0e6;
const HG_SEED: u64 = 0xF685_0614;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

fn material(nu: f64) -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(nu)).unwrap(), 1000.0).unwrap()
}

/// The ADR 0614 c1 patch: the unit cube on `n^3` Hex8, interior nodes moved by `0.1 h (2U - 1)` (seed `0xF685_0614`).
fn patch(n: usize, nu: f64, f: SolidFormulation, stabilised: IncompatibleStabilisation) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng::new(HG_SEED);
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
    m.set_incompatible_stabilisation(stabilised);
    m.add_material("all", material(nu)).unwrap();
    m
}

fn diag_strain(t: f64, lambda: f64) -> [f64; 6] {
    [
        0.5 * (t * t - 1.0),
        0.5 * (t * t - 1.0),
        0.5 * (lambda * lambda - 1.0),
        0.0,
        0.0,
        0.0,
    ]
}

/// `S` (Voigt, tensor shear) and `D` of the material at `diag(t, t, lambda)`.
fn stress(nu: f64, t: f64, lambda: f64) -> ([f64; 6], Matrix6) {
    material(nu)
        .stress(&diag_strain(t, lambda), &PointContext::default(), &[])
        .unwrap()
}

/// The lateral stretch of uniaxial stress: 200 bisections of `S_11` on `[1, 2]`.
fn lateral(nu: f64, lambda: f64) -> f64 {
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if stress(nu, mid, lambda).0[0] < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The homogeneous field `u = (F - I) X` at every node, `F = diag(t, t, lambda)`.
fn homogeneous(m: &SolidModel, t: f64, lambda: f64) -> Vec<f64> {
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

/// The equations of the interior nodes (node order, three each).
fn interior(m: &SolidModel) -> Vec<usize> {
    let boundary = boundary_nodes(m.mesh());
    let dofs = m.dof_map();
    let mut eqs = Vec::new();
    for node in 0..m.mesh().nodes().len() {
        if !boundary.contains(&node) {
            for c in 0..3 {
                eqs.push(dofs.equation(node, c).unwrap());
            }
        }
    }
    eqs
}

/// The free block (dense, row-major) of a CSR matrix.
fn dense_free(k: &rcfd_linalg::CsrMatrix, free: &[usize]) -> Vec<f64> {
    let nf = free.len();
    let mut pos = vec![usize::MAX; k.n];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let mut out = vec![0.0; nf * nf];
    for (r, &e) in free.iter().enumerate() {
        for p in k.row_ptr[e]..k.row_ptr[e + 1] {
            let c = pos[k.col[p]];
            if c != usize::MAX {
                out[r * nf + c] = k.val[p];
            }
        }
    }
    out
}

/// The vector Laplacian of the Full Hex8 (2 x 2 x 2 rule) on the free block.
fn laplacian(m: &SolidModel, free: &[usize]) -> Vec<f64> {
    let nf = free.len();
    let mut pos = vec![usize::MAX; m.dof_map().n_equations()];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let el = element_for(ElementKind::Hex8);
    let dofs = m.dof_map();
    let mut out = vec![0.0; nf * nf];
    for e in 0..m.mesh().n_elements() {
        let nodes = m.mesh().element_nodes(e);
        let coords = element_coords(m.mesh(), e);
        for ip in el.integration_points(&coords, el.stiffness_rule()).unwrap() {
            let g = &ip.geometry.grad;
            for (a, &na) in nodes.iter().enumerate() {
                for (b, &nb) in nodes.iter().enumerate() {
                    let v = ip.weight * (g[a][0] * g[b][0] + g[a][1] * g[b][1] + g[a][2] * g[b][2]);
                    for c in 0..3 {
                        let (r, s) = (pos[dofs.equation(na, c).unwrap()], pos[dofs.equation(nb, c).unwrap()]);
                        if r != usize::MAX && s != usize::MAX {
                            out[r * nf + s] += v;
                        }
                    }
                }
            }
        }
    }
    out
}

/// `q / mu` by the closed form of ADR 0628 arithmetic 2: `min_th J^{-2/3} (1 + mu_-(th))` on the 9001-point grid of
/// `[0, pi/2]`, for the neo-Hookean energy at `F = diag(t, t, lambda)`.
fn q_closed(nu: f64, t: f64, lambda: f64) -> f64 {
    let k = kappa_of(nu) / MU;
    let j = t * t * lambda;
    let i1 = 2.0 * t * t + lambda * lambda;
    let alpha = 5.0 / 9.0 * i1 + k * j.powf(8.0 / 3.0);
    let gamma = 2.0 / 3.0;
    let mut q = f64::INFINITY;
    for i in 0..=9000 {
        let th = i as f64 / 9000.0 * std::f64::consts::FRAC_PI_2;
        let (s2, c2) = (th.sin().powi(2), th.cos().powi(2));
        let m2 = s2 / (t * t) + c2 / (lambda * lambda);
        let b2 = t * t * s2 + lambda * lambda * c2;
        let tr = alpha * m2 - 2.0 * gamma;
        let disc = (0.25 * tr * tr + gamma * gamma * (m2 * b2 - 1.0)).max(0.0);
        let mum = 0.5 * tr - disc.sqrt();
        q = q.min(j.powf(-2.0 / 3.0) * (1.0 + mum));
    }
    q
}

/// K0: the smallest eigenvalue of the modes' block `K_aa` of the plain extended element (modes zero) over the elements,
/// over `mu h^3` (`h = 1/n`).
fn kaa_min(m: &SolidModel, nu: f64, u: &[f64], n: usize) -> f64 {
    let mat = material(nu);
    let mut lo = f64::INFINITY;
    for e in 0..m.mesh().n_elements() {
        let coords = element_coords(m.mesh(), e);
        let ue = element_u(m, u, e);
        let (_, k) =
            enhanced_force_and_tangent_with(&coords, &mat, &ue, &[0.0; 9], &empty(), &PointContext::default(), false)
                .unwrap();
        let block: Vec<f64> = (0..81).map(|p| k.get(24 + p / 9, 24 + p % 9)).collect();
        lo = lo.min(symmetric_eigen(&block, 9).unwrap().values[0]);
    }
    lo / (MU / (n * n * n) as f64)
}

/// The interior residual of the homogeneous state: `max |f_free| / max |f|`.
fn equilibrium(m: &SolidModel, free: &[usize], t: f64, lambda: f64) -> f64 {
    let u = homogeneous(m, t, lambda);
    let f = internal_force(m, &u, &initial_states(m).unwrap()).unwrap();
    worst(free.iter().map(|&e| f[e].abs())) / max_abs(&f)
}

// ============================================================================ the E state (copies of ADR 0628)

const E_BETA: f64 = 0.2;
const E_LAMBDA: f64 = 0.6;

/// The E field of ADR 0628 c1: `u = (F - I) X + beta (x z, y z, 0)`, `F = diag(1, 1, 0.6)`.
fn e_field(m: &SolidModel) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (nd, x) in m.mesh().nodes().iter().enumerate() {
        let v = [E_BETA * x[0] * x[2], E_BETA * x[1] * x[2], (E_LAMBDA - 1.0) * x[2]];
        for c in 0..3 {
            u[dofs.equation(nd, c).unwrap()] = v[c];
        }
    }
    u
}

/// The test's own neo-Hookean energy per unit reference volume at a Green strain (Voigt, engineering shear).
fn own_w(nu: f64, e: &[f64; 6]) -> f64 {
    let c = [
        [1.0 + 2.0 * e[0], e[3], e[5]],
        [e[3], 1.0 + 2.0 * e[1], e[4]],
        [e[5], e[4], 1.0 + 2.0 * e[2]],
    ];
    let j = det3(&c).sqrt();
    let i1 = c[0][0] + c[1][1] + c[2][2];
    0.5 * MU * (j.powf(-2.0 / 3.0) * i1 - 3.0) + 0.5 * kappa_of(nu) * (j - 1.0) * (j - 1.0)
}

/// The test's own energies of a model at `u`: `(sum_q w_q own_w(E_q), Pi_s)` at the modes of `modes_and_strains_with(..,
/// rule)`; `Pi_s` by the test's own coefficient of the rule and mode gradients (as ADR 0614 R-NT1), zero for `Off`.
fn own_energies(m: &SolidModel, nu: f64, u: &[f64], stabilised: IncompatibleStabilisation) -> (f64, f64) {
    let mat = material(nu);
    let d0 = mat.linear_elasticity_matrix().unwrap();
    let el = element_for(ElementKind::Hex8);
    let (mut w_el, mut w_pen) = (0.0, 0.0);
    for e in 0..m.mesh().n_elements() {
        let coords = element_coords(m.mesh(), e);
        let ue = element_u(m, u, e);
        let (alpha, eb) =
            modes_and_strains_with(&coords, &mat, &ue, &empty(), &PointContext::default(), stabilised).unwrap();
        let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
        for (ip, s) in ips.iter().zip(&eb) {
            w_el += ip.weight * own_w(nu, s);
        }
        if stabilised == OFF {
            continue;
        }
        let centre = el.point(&coords, [0.0; 3]).unwrap();
        let mut f0 = [[0.0; 3]; 3];
        for (a, ga) in centre.grad.iter().enumerate() {
            for i in 0..3 {
                for jj in 0..3 {
                    f0[i][jj] += ue[3 * a + i] * ga[jj];
                }
            }
        }
        for (i, row) in f0.iter_mut().enumerate() {
            row[i] += 1.0;
        }
        let c = own_coefficient(stabilised, &d0, &f0);
        let j0inv = inverse3(&centre.jacobian);
        let mut p = 0.0;
        for ip in &ips {
            let g = &ip.geometry;
            let mut fe = [[0.0; 3]; 3];
            for kk in 0..3 {
                let gk: [f64; 3] = std::array::from_fn(|mm| centre.det_j / g.det_j * j0inv[kk][mm] * (-2.0 * g.xi[kk]));
                for cc in 0..3 {
                    for jj in 0..3 {
                        fe[cc][jj] += alpha[3 * kk + cc] * gk[jj];
                    }
                }
            }
            p += ip.weight * fe.iter().flatten().map(|v| v * v).sum::<f64>();
        }
        w_pen += 0.5 * c * p;
    }
    (w_el, w_pen)
}

/// `Integrator::strain_energy` of a model started at `u` (no step taken, nothing prescribed).
fn integrator_energy(it: &mut Integrator, u: &[f64]) -> f64 {
    it.start(0.0, u, &vec![0.0; u.len()]).unwrap();
    it.strain_energy().unwrap()
}

// ============================================================================ the six cases and the rules

const RULES: [(&str, IncompatibleStabilisation); 3] = [
    ("R", IncompatibleStabilisation::RestStress),
    ("D", IncompatibleStabilisation::Deviatoric),
    ("L", IncompatibleStabilisation::LinearStress),
];
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const DEV: IncompatibleStabilisation = IncompatibleStabilisation::Deviatoric;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;
const REST: IncompatibleStabilisation = IncompatibleStabilisation::RestStress;
const FBAR: SolidFormulation = SolidFormulation::BBar;

/// A registered case of ADR 0630 c2 (ADR 0614's U30 and the five of ADR 0628).
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    nu: f64,
    confined: bool,
}

const U30: Case = Case {
    name: "U30",
    nu: 0.3,
    confined: false,
};
const C30: Case = Case {
    name: "C30",
    nu: 0.3,
    confined: true,
};
const C45: Case = Case {
    name: "C45",
    nu: 0.45,
    confined: true,
};
const C49: Case = Case {
    name: "C49",
    nu: 0.49,
    confined: true,
};
const U45: Case = Case {
    name: "U45",
    nu: 0.45,
    confined: false,
};
const U49: Case = Case {
    name: "U49",
    nu: 0.49,
    confined: false,
};
const CASES: [Case; 6] = [U30, C30, C45, C49, U45, U49];

/// The lateral stretch of the case at `lambda`: 1 (confined) or the uniaxial-stress `t`.
fn tau(case: Case, lambda: f64) -> f64 {
    if case.confined {
        1.0
    } else {
        lateral(case.nu, lambda)
    }
}

/// The ten lowest eigenvalues of `(K_ff, L_ff)` over `mu` at `F = diag(t, t, lambda)` (ascending).
fn lowest10(m: &SolidModel, free: &[usize], lap: &[f64], t: f64, lambda: f64) -> Vec<f64> {
    let u = homogeneous(m, t, lambda);
    let k = tangent(m, &u, &initial_states(m).unwrap(), Some(4)).unwrap();
    let kd = dense_free(&k, free);
    let e = generalized_eigen_dense(&kd, lap, free.len()).unwrap();
    e.values[..10].iter().map(|v| v / MU).collect()
}

/// The test's own rest stress `s = D0 voigt((C - C^{-1})/4)` (tensor shear) of a centre gradient.
fn own_stress(d0: &Matrix6, f0: &[[f64; 3]; 3]) -> [f64; 6] {
    let ft: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| f0[j][i]));
    let c = mat3_mul(&ft, f0);
    let ci = inverse3(&c);
    let e = |i: usize, j: usize| 0.25 * (c[i][j] - ci[i][j]);
    let v = [e(0, 0), e(1, 1), e(2, 2), 2.0 * e(0, 1), 2.0 * e(1, 2), 2.0 * e(2, 0)];
    apply(d0, &v)
}

/// The test's own coefficient of a rule (ADR 0614 arithmetic 4 (iii), ADR 0630 arithmetic 3), written from the formulas.
fn own_coefficient(rule: IncompatibleStabilisation, d0: &Matrix6, f0: &[[f64; 3]; 3]) -> f64 {
    let mut s = own_stress(d0, f0);
    let g0 = (d0[3][3] + d0[4][4] + d0[5][5]) / 3.0;
    if rule == DEV {
        let p = (s[0] + s[1] + s[2]) / 3.0;
        s[0] -= p;
        s[1] -= p;
        s[2] -= p;
    }
    let ss = s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]);
    match rule {
        IncompatibleStabilisation::Off => 0.0,
        IncompatibleStabilisation::LinearStress => ss / (g0 + ss.sqrt()),
        _ => ss / g0,
    }
}

fn diag(t: f64, lambda: f64) -> [[f64; 3]; 3] {
    [[t, 0.0, 0.0], [0.0, t, 0.0], [0.0, 0.0, lambda]]
}

// ============================================================================ G4: the heavy records

/// One case of ADR 0630 D6 on the 6^3 patch over the 51 grid stretches.
fn sweep(case: Case) {
    let mut ch = Checks::new();
    let name = case.name;
    let n = 6;
    let full = patch(n, case.nu, FULL, OFF);
    let fbar = patch(n, case.nu, FBAR, OFF);
    let inc = patch(n, case.nu, INC, OFF);
    let rest = patch(n, case.nu, INC, REST);
    let dev = patch(n, case.nu, INC, DEV);
    let lin = patch(n, case.nu, INC, LIN);
    let free = interior(&full);
    println!(
        "{name} set-up: nu {}, confined {}, {} free unknowns",
        case.nu,
        case.confined,
        free.len()
    );
    let lap = laplacian(&full, &free);
    let d0 = material(case.nu).linear_elasticity_matrix().unwrap();
    let (mut s1d, mut s1l, mut s1r, mut k0, mut f1, mut f2) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut o2: [Vec<f64>; 5] = Default::default();
    println!(
        "R2 {name} lambda t q/mu m_Full m_Fbar m_Inc m_R m_D m_L c_R/mu c_D/mu c_L/mu kaa | LK Lambda1 R D L Full Inc | Lambda10 R D L Full Inc"
    );
    for j in 0..=50 {
        let lambda = 1.0 - 0.01 * j as f64;
        let t = tau(case, lambda);
        let qc = q_closed(case.nu, t, lambda);
        let mf = lowest10(&full, &free, &lap, t, lambda);
        let mb = lowest10(&fbar, &free, &lap, t, lambda);
        let mi = lowest10(&inc, &free, &lap, t, lambda);
        let mr = lowest10(&rest, &free, &lap, t, lambda);
        let md = lowest10(&dev, &free, &lap, t, lambda);
        let ml = lowest10(&lin, &free, &lap, t, lambda);
        let kaa = kaa_min(&inc, case.nu, &homogeneous(&inc, t, lambda), n);
        let c: Vec<f64> = RULES
            .iter()
            .map(|(_, r)| own_coefficient(*r, &d0, &diag(t, lambda)) / MU)
            .collect();
        let sum = |v: &[f64]| v.iter().sum::<f64>();
        let l1 = |v: &[f64]| v[0] / mb[0] - 1.0;
        let l10 = |v: &[f64]| sum(v) / sum(&mb) - 1.0;
        println!(
            "R2 {name} {lambda:.2} {t:.8} {qc:.6} {:.6} {:.6} {:+.6} {:.6} {:+.6} {:.6} {:.4} {:.4} {:.4} {kaa:.3} | LK {:+.4} {:+.4} {:+.4} {:+.4} {:+.4} | {:+.4} {:+.4} {:+.4} {:+.4} {:+.4}",
            mf[0], mb[0], mi[0], mr[0], md[0], ml[0], c[0], c[1], c[2],
            l1(&mr), l1(&md), l1(&ml), l1(&mf), l1(&mi),
            l10(&mr), l10(&md), l10(&ml), l10(&mf), l10(&mi)
        );
        s1d.push(md[0]);
        s1l.push(ml[0]);
        s1r.push(mr[0]);
        k0.push(kaa);
        f1.push(mb[0]);
        f2.push((mb[0] / qc - 1.0).abs());
        if kaa > 0.0 {
            for i in 0..10 {
                o2[0].push(mi[i] - md[i]);
                o2[1].push(md[i] - mr[i]);
                o2[2].push(mr[i] - mf[i]);
                o2[3].push(mi[i] - ml[i]);
                o2[4].push(ml[i] - mr[i]);
            }
        }
    }
    let k0n = k0.iter().filter(|&&k| k > 0.0).count();
    println!(
        "K0 {name} smallest eigenvalue of K_aa over mu h^3: {:.4}",
        least(k0.iter().copied())
    );
    ch.holds(
        &format!("K0 {name} K_aa positive definite at every stretch"),
        k0n == 51,
        &format!("{k0n} of 51"),
    );
    for (o, label) in o2.iter().zip([
        "m_Inc,i - m_D,i",
        "m_D,i - m_R,i",
        "m_R,i - m_Full,i",
        "m_Inc,i - m_L,i",
        "m_L,i - m_R,i",
    ]) {
        ch.at_most(
            &format!("O2 {name} {label} (ten lowest, where K0)"),
            worst(o.iter().copied()),
            1e-9,
        );
    }
    ch.at_least(&format!("F1 {name} m_Fbar over [0.5, 1]"), least(f1), 0.5);
    ch.at_most(
        &format!("F2 {name} |m_Fbar / (q/mu) - 1| over [0.5, 1]"),
        worst(f2),
        0.15,
    );
    ch.at_least(&format!("S1R {name} m_R over [0.5, 1]"), least(s1r), 0.5);
    ch.at_least(&format!("S1L {name} m_L over [0.5, 1] (diagnostic)"), least(s1l), 0.5);
    let (dmin, dat) = s1d.iter().enumerate().fold(
        (f64::INFINITY, 0),
        |(m, a), (j, &v)| if v < m { (v, j) } else { (m, a) },
    );
    println!(
        "S1D {name} smallest m_D {dmin:.6} at lambda {:.2}",
        1.0 - 0.01 * dat as f64
    );
    ch.at_least(&format!("S1D {name} m_D over [0.5, 1]"), least(s1d), 0.5);
    ch.finish();
}

#[test]
#[ignore = "heavy record of ADR 0630 (U30: 51 stretches x 6 elements of dense 375 x 375 pencils); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_u30_heavy() {
    sweep(U30);
}

#[test]
#[ignore = "heavy record of ADR 0630 (C30); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_c30_heavy() {
    sweep(C30);
}

#[test]
#[ignore = "heavy record of ADR 0630 (C45); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_c45_heavy() {
    sweep(C45);
}

#[test]
#[ignore = "heavy record of ADR 0630 (C49); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_c49_heavy() {
    sweep(C49);
}

#[test]
#[ignore = "heavy record of ADR 0630 (U45); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_u45_heavy() {
    sweep(U45);
}

#[test]
#[ignore = "heavy record of ADR 0630 (U49); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch"]
fn g4_sweep_u49_heavy() {
    sweep(U49);
}

// ============================================================================ HF3: the fast companion

#[test]
fn hf3_fast_companion_on_a_4_cube_patch() {
    let mut ch = Checks::new();
    let n = 4;
    for case in CASES {
        let name = case.name;
        let full = patch(n, case.nu, FULL, OFF);
        let fbar = patch(n, case.nu, FBAR, OFF);
        let inc = patch(n, case.nu, INC, OFF);
        let rest = patch(n, case.nu, INC, REST);
        let dev = patch(n, case.nu, INC, DEV);
        let lin = patch(n, case.nu, INC, LIN);
        let free = interior(&full);
        let lap = laplacian(&full, &free);
        for lambda in [1.0, 0.8, 0.5] {
            let t = tau(case, lambda);
            let m = |x: &SolidModel| lowest10(x, &free, &lap, t, lambda)[0];
            let (mf, mb, mi, mr, md, ml) = (m(&full), m(&fbar), m(&inc), m(&rest), m(&dev), m(&lin));
            let kaa = kaa_min(&inc, case.nu, &homogeneous(&inc, t, lambda), n);
            println!(
                "HF3 {name} {lambda:.2}: m_Full {mf:.6}, m_Fbar {mb:.6}, m_Inc {mi:+.6}, m_R {mr:.6}, m_D {md:+.6}, m_L {ml:.6}, kaa {kaa:.3}"
            );
            let tag = format!("HF3 {name} {lambda}");
            ch.holds(
                &format!("{tag}: K0 K_aa positive definite"),
                kaa > 0.0,
                &format!("{kaa:.4}"),
            );
            if kaa > 0.0 {
                ch.at_most(&format!("{tag}: O2 m_Inc - m_D"), mi - md, 1e-9);
                ch.at_most(&format!("{tag}: O2 m_D - m_R"), md - mr, 1e-9);
                ch.at_most(&format!("{tag}: O2 m_R - m_Full"), mr - mf, 1e-9);
                ch.at_most(&format!("{tag}: O2 m_Inc - m_L"), mi - ml, 1e-9);
                ch.at_most(&format!("{tag}: O2 m_L - m_R"), ml - mr, 1e-9);
            }
            ch.at_least(&format!("{tag}: m_L"), ml, 0.5);
            ch.at_least(&format!("{tag}: m_Fbar"), mb, 0.5);
            if !matches!(name, "C45" | "C49") {
                ch.at_least(&format!("{tag}: m_D"), md, 0.5);
            }
            if (name == "C49" && lambda == 0.8) || (name == "C45" && lambda == 0.5) {
                ch.at_most(&format!("{tag}: N1 m_D"), md, -1.0);
            }
        }
    }
    ch.finish();
}

// ============================================================================ S2b, S3b, S3c, S3d: the coefficients

#[test]
fn s2b_s3_coefficients_of_the_rules() {
    let mut ch = Checks::new();
    let ctx = PointContext::default();
    // S2b: at rest the stabilised tangents are the plain one (c = 0, dc = 0).
    let n = 6;
    let plain = patch(n, 0.49, INC, OFF);
    let z = vec![0.0; plain.dof_map().n_equations()];
    let k0 = tangent(&plain, &z, &initial_states(&plain).unwrap(), Some(4)).unwrap();
    for (label, rule) in [("D", DEV), ("L", LIN)] {
        let m = patch(n, 0.49, INC, rule);
        let k = tangent(&m, &z, &initial_states(&m).unwrap(), Some(4)).unwrap();
        let d: Vec<f64> = k.val.iter().zip(&k0.val).map(|(a, b)| a - b).collect();
        let same = k.row_ptr == k0.row_ptr && k.col == k0.col;
        ch.at_most(
            &format!("S2b {label} tangent at u = 0 against the plain one, nu 0.49"),
            if same { max_abs(&d) / max_abs(&k0.val) } else { f64::NAN },
            1e-14,
        );
    }
    // S3b: the library's coefficients against the test's own closed forms.
    for (case, lambda) in [(C49, 0.6), (U30, 0.6)] {
        let m = patch(n, case.nu, INC, OFF);
        let t = tau(case, lambda);
        let u = homogeneous(&m, t, lambda);
        let ue = element_u(&m, &u, 0);
        let mat = material(case.nu);
        let d0 = mat.linear_elasticity_matrix().unwrap();
        let coords = element_coords(m.mesh(), 0);
        let mut cs = Vec::new();
        for (label, rule) in RULES {
            let c = stabilisation_coefficient_with(&coords, &mat, &ue, &ctx, rule).unwrap();
            let own = own_coefficient(rule, &d0, &diag(t, lambda));
            println!(
                "S3b {} {lambda} {label}: c / mu {:.8} (own {:.8})",
                case.name,
                c / MU,
                own / MU
            );
            if label != "R" {
                ch.at_most(
                    &format!(
                        "S3b {} {lambda} {label}: coefficient against the own closed form, relative",
                        case.name
                    ),
                    (c - own).abs() / own,
                    1e-12,
                );
            }
            cs.push(c);
        }
        if case.name == "C49" {
            ch.holds(
                "S3c C49 0.6: c_D and c_L below c_R",
                cs[1] <= cs[0] && cs[2] <= cs[0],
                &format!("{:e}, {:e}, {:e}", cs[0], cs[1], cs[2]),
            );
        }
    }
    let x = distorted();
    let u = t1b_displacement(&x);
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap();
    let f0 = centre_f(&x, &u);
    let d0 = steel.linear_elasticity_matrix().unwrap();
    for (label, rule) in [("D", DEV), ("L", LIN)] {
        let c = stabilisation_coefficient_with(&x, &steel, &u, &ctx, rule).unwrap();
        let own = own_coefficient(rule, &d0, &f0);
        ch.at_most(
            &format!("S3b T1 (b) {label}: coefficient against the own closed form, relative"),
            (c - own).abs() / own,
            1e-12,
        );
    }
    // S3c: the ADR 0614 coefficient in its bits; Off is zero.
    let cr = stabilisation_coefficient_with(&x, &steel, &u, &ctx, REST).unwrap();
    let c14 = stabilisation_coefficient(&x, &steel, &u, &ctx).unwrap();
    ch.holds(
        "S3c T1 (b): RestStress equal in its bits to stabilisation_coefficient",
        cr.to_bits() == c14.to_bits(),
        &format!("{cr:e}, {c14:e}"),
    );
    let c0 = stabilisation_coefficient_with(&x, &steel, &u, &ctx, OFF).unwrap();
    ch.holds("S3c T1 (b): Off gives 0", c0 == 0.0, &format!("{c0:e}"));
    // S3d: the bulk modulus does not enter rule D (N3: it does enter rule R).
    let (ma, mb) = (material(0.3), material(0.49));
    let cda = stabilisation_coefficient_with(&x, &ma, &u, &ctx, DEV).unwrap();
    let cdb = stabilisation_coefficient_with(&x, &mb, &u, &ctx, DEV).unwrap();
    let cra = stabilisation_coefficient_with(&x, &ma, &u, &ctx, REST).unwrap();
    let crb = stabilisation_coefficient_with(&x, &mb, &u, &ctx, REST).unwrap();
    println!(
        "S3d c_D {cda:e}, {cdb:e}; N3 c_R {cra:e}, {crb:e} (ratio {:.4})",
        crb / cra
    );
    ch.at_most(
        "S3d c_D at kappa/mu 2.17 and 49.7, same mu and F0, relative",
        (cda - cdb).abs() / cda,
        1e-12,
    );
    ch.finish();
}

/// The distorted unit Hex8 of `si_fbar.rs` (seed `0xF587_0506`).
fn distorted() -> Vec<Vec3> {
    let mut rng = Rng::new(0xF587_0506);
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
        .map(|p| {
            let mut q = *p;
            for v in q.iter_mut() {
                *v += 0.1 * (2.0 * rng.uniform() - 1.0);
            }
            q
        })
        .collect()
}

/// The displacement of ADR 0608 T1 (b) (the RNG drawn as there: (a) first).
fn t1b_displacement(x: &[Vec3]) -> Vec<f64> {
    let mut rng = Rng::new(0xF596_0608 + 1);
    let _a: Vec<f64> = (0..24).map(|_| 1e-3 * (2.0 * rng.uniform() - 1.0)).collect();
    x.iter()
        .flat_map(|p| [-p[1] - p[0], p[0] - p[1], 0.0])
        .map(|v| v + 5e-2 * (2.0 * rng.uniform() - 1.0))
        .collect()
}

/// The compatible gradient at the element centre.
fn centre_f(coords: &[Vec3], ue: &[f64]) -> [[f64; 3]; 3] {
    let el = element_for(ElementKind::Hex8);
    let centre = el.point(coords, [0.0; 3]).unwrap();
    let mut f0 = [[0.0; 3]; 3];
    for (a, ga) in centre.grad.iter().enumerate() {
        for i in 0..3 {
            for jj in 0..3 {
                f0[i][jj] += ue[3 * a + i] * ga[jj];
            }
        }
    }
    for (i, row) in f0.iter_mut().enumerate() {
        row[i] += 1.0;
    }
    f0
}

// ============================================================================ H3c: equilibrium

#[test]
fn h3c_equilibrium_of_the_rules() {
    let mut ch = Checks::new();
    let n = 6;
    for case in [C49, U30] {
        for (label, rule) in [("D", DEV), ("L", LIN)] {
            let m = patch(n, case.nu, INC, rule);
            let free = interior(&m);
            let lambda = 0.5;
            ch.at_most(
                &format!("H3c {} {label} lambda 0.5: max |f_free| / max |f|", case.name),
                equilibrium(&m, &free, tau(case, lambda), lambda),
                1e-10,
            );
        }
    }
    ch.finish();
}

// ============================================================================ E1, E3: the strain energy of the rules

#[test]
fn e_strain_energy_of_the_rules_at_a_large_strain() {
    let mut ch = Checks::new();
    let nu = 0.3;
    let n = 4;
    let opts = NlTransientOptions::new(1e-5).threads(4);
    for (label, rule) in [("D", DEV), ("L", LIN)] {
        let m = patch(n, nu, INC, rule);
        let u = e_field(&m);
        let mut it = Integrator::new(&m, &[], &opts).unwrap();
        let w = integrator_energy(&mut it, &u);
        let (we, wp) = own_energies(&m, nu, &u, rule);
        let total = we + wp;
        println!(
            "E1 {label}: strain_energy {w:.12e}, own elastic {we:.12e}, own Pi_s {wp:.6e} (over mu: {:.8}, {:.6e})",
            we / MU,
            wp / MU
        );
        ch.at_least(&format!("E1s{label} set-up: own Pi_s over the total"), wp / total, 1e-4);
        ch.at_most(
            &format!("E1{label} strain_energy against the own elastic and stabilisation energy, relative"),
            (w - total).abs() / total,
            1e-12,
        );
        let mut rng = Rng::new(0xF693_0628);
        let d: Vec<f64> = (0..u.len()).map(|_| 1e-2 * (2.0 * rng.uniform() - 1.0)).collect();
        let h = 1e-4;
        let up: Vec<f64> = u.iter().zip(&d).map(|(a, b)| a + h * b).collect();
        let um: Vec<f64> = u.iter().zip(&d).map(|(a, b)| a - h * b).collect();
        let fd = (integrator_energy(&mut it, &up) - integrator_energy(&mut it, &um)) / (2.0 * h);
        let f = internal_force(&m, &u, &initial_states(&m).unwrap()).unwrap();
        let fdot: f64 = f.iter().zip(&d).map(|(a, b)| a * b).sum();
        println!("E3 {label}: central difference {fd:.12e}, f . d {fdot:.12e}");
        ch.at_most(
            &format!("E3{label} d strain_energy against internal_force . d, relative"),
            (fd - fdot).abs() / fdot.abs(),
            1e-6,
        );
    }
    ch.finish();
}
