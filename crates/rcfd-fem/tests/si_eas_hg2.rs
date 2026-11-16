//! F-693 (ADR 0628): the stabilised incompatible-modes Hex8 of ADR 0614 beyond its set-up. The strain energy of the
//! stabilised element at a large strain against the test's own energies (E0, E1, E3: the missed mutation MU7 of ADR 0614);
//! the ADR 0614 sweep for confined compression at nu = 0.3, 0.45, 0.49 and for uniaxial stress at nu = 0.45, 0.49 (the heavy
//! records G, K0, O1, S1, Z, R1 and the fast companion HF2), with the closed-form acoustic bound of ADR 0628 arithmetic 2.
//! Every aggregated value is required finite before it is compared. The helpers are copies of those of `si_eas_hg.rs`
//! (existing tests are not edited), generalised to a Poisson ratio and a load path.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, Matrix6, PointContext};
use rcfd_fem::nl_incompatible::{enhanced_force_and_tangent_with, modes_and_strains_with};
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

/// The test's own coefficient of ADR 0614 arithmetic 4 (iii) at a centre gradient: `c = s:s / G0`, `s = D0 voigt((C -
/// C^{-1})/4)`.
fn own_coefficient(d0: &Matrix6, f0: &[[f64; 3]; 3]) -> f64 {
    let ft: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| f0[j][i]));
    let c = mat3_mul(&ft, f0);
    let ci = inverse3(&c);
    let e = |i: usize, j: usize| 0.25 * (c[i][j] - ci[i][j]);
    let v = [e(0, 0), e(1, 1), e(2, 2), 2.0 * e(0, 1), 2.0 * e(1, 2), 2.0 * e(2, 0)];
    let s = apply(d0, &v);
    let g0 = (d0[3][3] + d0[4][4] + d0[5][5]) / 3.0;
    (s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5])) / g0
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
fn patch(n: usize, nu: f64, f: SolidFormulation, stabilised: bool) -> SolidModel {
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

/// A registered case of ADR 0628 c2, c3.
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    nu: f64,
    confined: bool,
    /// The probe's zero of the plain element on the 6^3 patch (ADR 0628 arithmetic 1).
    probe_zero: f64,
}

const C30: Case = Case {
    name: "C30",
    nu: 0.3,
    confined: true,
    probe_zero: 0.64179,
};
const C45: Case = Case {
    name: "C45",
    nu: 0.45,
    confined: true,
    probe_zero: 0.80937,
};
const C49: Case = Case {
    name: "C49",
    nu: 0.49,
    confined: true,
    probe_zero: 0.95075,
};
const U45: Case = Case {
    name: "U45",
    nu: 0.45,
    confined: false,
    probe_zero: 0.56444,
};
const U49: Case = Case {
    name: "U49",
    nu: 0.49,
    confined: false,
    probe_zero: 0.51033,
};

/// The lateral stretch of the case at `lambda`: 1 (confined) or the uniaxial-stress `t`.
fn tau(case: Case, lambda: f64) -> f64 {
    if case.confined {
        1.0
    } else {
        lateral(case.nu, lambda)
    }
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

/// The lowest eigenvalue of `(K_ff, L_ff)` over `mu` at `F = diag(t, t, lambda)`.
fn lowest(m: &SolidModel, free: &[usize], lap: &[f64], t: f64, lambda: f64) -> f64 {
    let u = homogeneous(m, t, lambda);
    let k = tangent(m, &u, &initial_states(m).unwrap(), Some(4)).unwrap();
    let kd = dense_free(&k, free);
    let e = generalized_eigen_dense(&kd, lap, free.len()).unwrap();
    e.values[0] / MU
}

fn vi(i: usize, j: usize) -> usize {
    match (i.min(j), i.max(j)) {
        (0, 0) => 0,
        (1, 1) => 1,
        (2, 2) => 2,
        (0, 1) => 3,
        (1, 2) => 4,
        _ => 5,
    }
}

/// `q / mu` of ADR 0614 c3 (a copy of its `q_min` for the Poisson ratio `nu`): the smallest eigenvalue of the acoustic
/// tensor of `A = delta S + F F C` from `Material::stress` over a 1-degree grid of the sphere, `F = diag(t, t, lambda)`.
fn q_grid(nu: f64, t: f64, lambda: f64) -> f64 {
    let (s, d) = stress(nu, t, lambda);
    let f = [t, t, lambda];
    let st = |j: usize, l: usize| s[vi(j, l)];
    let a = |i: usize, jj: usize, k: usize, l: usize| {
        (if i == k { st(jj, l) } else { 0.0 }) + f[i] * f[k] * d[vi(i, jj)][vi(k, l)]
    };
    let mut q = f64::INFINITY;
    for th in 0..=180 {
        for ph in 0..360 {
            let (th, ph) = ((th as f64).to_radians(), (ph as f64).to_radians());
            let n = [th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()];
            let mut qm = [0.0; 9];
            for i in 0..3 {
                for k in 0..3 {
                    let mut v = 0.0;
                    for jj in 0..3 {
                        for l in 0..3 {
                            v += a(i, jj, k, l) * n[jj] * n[l];
                        }
                    }
                    qm[3 * i + k] = v;
                }
            }
            q = q.min(symmetric_eigen(&qm, 3).unwrap().values[0]);
        }
    }
    q / MU
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

// ============================================================================ G, K0, O1, S1, Z, R1: the heavy records

fn sweep(case: Case) {
    let mut ch = Checks::new();
    let name = case.name;
    let n = 6;
    let full = patch(n, case.nu, FULL, false);
    let inc = patch(n, case.nu, INC, false);
    let stab = patch(n, case.nu, INC, true);
    let free = interior(&full);
    println!(
        "{name} set-up: nu {}, confined {}, {} free unknowns",
        case.nu,
        case.confined,
        free.len()
    );
    let lap = laplacian(&full, &free);
    let d0 = material(case.nu).linear_elasticity_matrix().unwrap();
    let (mut g0, mut g1, mut g2, mut g3, mut k0, mut o1a, mut o1b, mut s1) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut prev: Option<(f64, f64)> = None;
    let mut bracket: Option<(f64, f64)> = None;
    let mut rest: Option<(f64, f64)> = None;
    println!("R1 {name} lambda t q_cf/mu q_grid/mu m_Full m_Inc m_Stab m_Stab/m_Full m_Stab/q c/mu kaa");
    for j in 0..=50 {
        let lambda = 1.0 - 0.01 * j as f64;
        let t = tau(case, lambda);
        if !case.confined {
            let (s, _) = stress(case.nu, t, lambda);
            g0.push(if j > 0 {
                s[0].abs() / s[2].abs()
            } else {
                s[0].abs() / MU
            });
        }
        let qc = q_closed(case.nu, t, lambda);
        let qg = q_grid(case.nu, t, lambda);
        let mf = lowest(&full, &free, &lap, t, lambda);
        let mi = lowest(&inc, &free, &lap, t, lambda);
        let ms = lowest(&stab, &free, &lap, t, lambda);
        let kaa = kaa_min(&inc, case.nu, &homogeneous(&inc, t, lambda), n);
        let c = own_coefficient(&d0, &[[t, 0.0, 0.0], [0.0, t, 0.0], [0.0, 0.0, lambda]]) / MU;
        println!(
            "R1 {name} {lambda:.2} {t:.8} {qc:.6} {qg:.6} {mf:.6} {mi:+.6} {ms:.6} {:.6} {:.6} {c:.4} {kaa:.3}",
            ms / mf,
            ms / qc
        );
        g1.push(qc);
        g2.push((qg - qc) / qc);
        g3.push(mf - qc);
        k0.push(kaa);
        if kaa > 0.0 {
            o1a.push(mi - ms);
            o1b.push(ms - mf);
        }
        s1.push(ms);
        if j == 0 {
            rest = Some((mf, mi));
        }
        if bracket.is_none() {
            if let Some((lp, mp)) = prev {
                if mp > 0.0 && mi <= 0.0 {
                    bracket = Some((lp, lambda));
                }
            }
        }
        prev = Some((lambda, mi));
    }
    if !case.confined {
        ch.at_most(
            &format!("G0 {name} uniaxial stress: |S_11| / |S_33| (|S_11| / mu at 1)"),
            worst(g0),
            1e-12,
        );
    }
    ch.at_least(&format!("G1 {name} q_cf / mu over [0.5, 1]"), least(g1), 0.9);
    let (g2lo, g2hi) = (least(g2.iter().copied()), worst(g2.iter().copied()));
    ch.at_least(&format!("G2 {name} (q_grid - q_cf) / q_cf, smallest"), g2lo, -1e-10);
    ch.at_most(&format!("G2 {name} (q_grid - q_cf) / q_cf, largest"), g2hi, 1e-4);
    ch.at_least(&format!("G3 {name} m_Full - q_cf / mu over [0.5, 1]"), least(g3), -1e-9);
    let k0min = least(k0);
    println!(
        "K0 {name} smallest eigenvalue of K_aa over mu h^3: {k0min:.4} ({} of 51 stretches with K_aa > 0)",
        o1a.len()
    );
    ch.holds(
        &format!("K0 {name} K_aa positive definite at every stretch"),
        o1a.len() == 51,
        &format!("{} of 51", o1a.len()),
    );
    ch.at_most(&format!("O1 {name} m_Inc - m_Stab (where K0)"), worst(o1a), 1e-9);
    ch.at_most(&format!("O1 {name} m_Stab - m_Full (where K0)"), worst(o1b), 1e-9);
    ch.at_least(&format!("S1 {name} m_Stab over [0.5, 1]"), least(s1), 0.5);
    if let Some((mf, mi)) = rest {
        println!("R1 {name} locking excess m_Full / m_Inc at rest: {:.6}", mf / mi);
    }
    match bracket {
        None => ch.holds(&format!("Z {name} a zero of m_Inc in [0.5, 1]"), false, "none found"),
        Some((hi0, lo0)) => {
            let (mut hi, mut lo) = (hi0, lo0);
            for _ in 0..20 {
                let mid = 0.5 * (hi + lo);
                if lowest(&inc, &free, &lap, tau(case, mid), mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let ls = 0.5 * (hi + lo);
            println!(
                "Z {name} lambda_s = {ls:.9} (bracket {hi:.9} .. {lo:.9}; probe {})",
                case.probe_zero
            );
            ch.at_most(
                &format!("Z {name} |lambda_s - probe|"),
                (ls - case.probe_zero).abs(),
                0.01,
            );
            let t = tau(case, ls);
            ch.at_least(
                &format!("Z5 {name} q_cf / mu at lambda_s"),
                q_closed(case.nu, t, ls),
                0.9,
            );
            ch.at_least(
                &format!("Z5 {name} m_Full at lambda_s"),
                lowest(&full, &free, &lap, t, ls),
                0.9,
            );
        }
    }
    ch.finish();
}

#[test]
#[ignore = "heavy record of ADR 0628 (C30: 51 stretches x 3 elements plus 20 bisections of dense 375 x 375 pencils); run once with --ignored; the fast companion is hf2_fast_companion_on_a_4_cube_patch"]
fn g_sweep_c30_heavy() {
    sweep(C30);
}

#[test]
#[ignore = "heavy record of ADR 0628 (C45); run once with --ignored; the fast companion is hf2_fast_companion_on_a_4_cube_patch"]
fn g_sweep_c45_heavy() {
    sweep(C45);
}

#[test]
#[ignore = "heavy record of ADR 0628 (C49); run once with --ignored; the fast companion is hf2_fast_companion_on_a_4_cube_patch"]
fn g_sweep_c49_heavy() {
    sweep(C49);
}

#[test]
#[ignore = "heavy record of ADR 0628 (U45); run once with --ignored; the fast companion is hf2_fast_companion_on_a_4_cube_patch"]
fn g_sweep_u45_heavy() {
    sweep(U45);
}

#[test]
#[ignore = "heavy record of ADR 0628 (U49); run once with --ignored; the fast companion is hf2_fast_companion_on_a_4_cube_patch"]
fn g_sweep_u49_heavy() {
    sweep(U49);
}

// ============================================================================ HF2, H3b: the fast companion

#[test]
fn hf2_fast_companion_on_a_4_cube_patch() {
    let mut ch = Checks::new();
    let n = 4;
    for case in [C30, C45, C49, U45, U49] {
        let name = case.name;
        let full = patch(n, case.nu, FULL, false);
        let inc = patch(n, case.nu, INC, false);
        let stab = patch(n, case.nu, INC, true);
        let free = interior(&full);
        let lap = laplacian(&full, &free);
        for lambda in [1.0, 0.8, 0.5] {
            let t = tau(case, lambda);
            let qc = q_closed(case.nu, t, lambda);
            let qg = q_grid(case.nu, t, lambda);
            let mf = lowest(&full, &free, &lap, t, lambda);
            let mi = lowest(&inc, &free, &lap, t, lambda);
            let ms = lowest(&stab, &free, &lap, t, lambda);
            let kaa = kaa_min(&inc, case.nu, &homogeneous(&inc, t, lambda), n);
            println!(
                "HF2 {name} {lambda:.2}: q_cf/mu {qc:.6}, q_grid/mu {qg:.6}, m_Full {mf:.6}, m_Inc {mi:+.6}, m_Stab {ms:.6}, kaa {kaa:.3}"
            );
            let tag = format!("HF2 {name} {lambda}");
            ch.at_least(&format!("{tag}: G2 (q_grid - q_cf) / q_cf"), (qg - qc) / qc, -1e-10);
            ch.at_most(&format!("{tag}: G2 (q_grid - q_cf) / q_cf"), (qg - qc) / qc, 1e-4);
            ch.at_least(&format!("{tag}: G3 m_Full - q_cf / mu"), mf - qc, -1e-9);
            ch.holds(
                &format!("{tag}: K0 K_aa positive definite"),
                kaa > 0.0,
                &format!("{kaa:.4}"),
            );
            if kaa > 0.0 {
                ch.at_most(&format!("{tag}: O1 m_Inc - m_Stab"), mi - ms, 1e-9);
                ch.at_most(&format!("{tag}: O1 m_Stab - m_Full"), ms - mf, 1e-9);
            }
            ch.at_least(&format!("{tag}: m_Stab"), ms, 0.5);
            if (name == "C49" && lambda == 0.8) || (name == "C45" && lambda == 0.5) {
                ch.at_most(&format!("{tag}: N2 m_Inc"), mi, -1.0);
            }
            if name == "C30" && lambda == 0.5 {
                ch.at_most(&format!("{tag}: N2 m_Inc"), mi, -0.1);
            }
        }
    }
    ch.finish();
}

#[test]
fn h3b_equilibrium_near_incompressibility() {
    let mut ch = Checks::new();
    let n = 6;
    for case in [C49, U49] {
        let m = patch(n, case.nu, INC, true);
        let free = interior(&m);
        let lambda = 0.5;
        ch.at_most(
            &format!("H3b {} Stab lambda 0.5: max |f_free| / max |f|", case.name),
            equilibrium(&m, &free, tau(case, lambda), lambda),
            1e-10,
        );
    }
    ch.finish();
}

// ============================================================================ E0, E1, E3: the strain energy at a large strain

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
/// stabilised)`; `Pi_s` by the test's own coefficient and mode gradients (as ADR 0614 R-NT1), zero when not stabilised.
fn own_energies(m: &SolidModel, nu: f64, u: &[f64], stabilised: bool) -> (f64, f64) {
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
        if !stabilised {
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
        let c = own_coefficient(&d0, &f0);
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

#[test]
fn e_strain_energy_at_a_large_strain() {
    let mut ch = Checks::new();
    let nu = 0.3;
    let n = 4;
    let opts = NlTransientOptions::new(1e-5).threads(4);
    // E0: the plain model.
    let plain = patch(n, nu, INC, false);
    let u = e_field(&plain);
    let mut it = Integrator::new(&plain, &[], &opts).unwrap();
    let w_plain = integrator_energy(&mut it, &u);
    let (we0, _) = own_energies(&plain, nu, &u, false);
    println!(
        "E0 plain: strain_energy {w_plain:.12e}, own {we0:.12e} (over mu: {:.8})",
        we0 / MU
    );
    ch.at_most(
        "E0 plain strain_energy against the own elastic energy, relative",
        (w_plain - we0).abs() / we0,
        1e-12,
    );
    // E1s, E1: the stabilised model.
    let stab = patch(n, nu, INC, true);
    let mut its = Integrator::new(&stab, &[], &opts).unwrap();
    let w = integrator_energy(&mut its, &u);
    let (we, wp) = own_energies(&stab, nu, &u, true);
    let total = we + wp;
    println!(
        "E1 stabilised: strain_energy {w:.12e}, own elastic {we:.12e}, own Pi_s {wp:.6e} (over mu: {:.8}, {:.6e})",
        we / MU,
        wp / MU
    );
    ch.at_least("E1s set-up: own Pi_s over the total", wp / total, 1e-4);
    ch.at_most(
        "E1 stabilised strain_energy against the own elastic and stabilisation energy, relative",
        (w - total).abs() / total,
        1e-12,
    );
    // E3: the strain energy is the potential of the stabilised internal force.
    let mut rng = Rng::new(0xF693_0628);
    let d: Vec<f64> = (0..u.len()).map(|_| 1e-2 * (2.0 * rng.uniform() - 1.0)).collect();
    let h = 1e-4;
    let up: Vec<f64> = u.iter().zip(&d).map(|(a, b)| a + h * b).collect();
    let um: Vec<f64> = u.iter().zip(&d).map(|(a, b)| a - h * b).collect();
    let fd = (integrator_energy(&mut its, &up) - integrator_energy(&mut its, &um)) / (2.0 * h);
    let f = internal_force(&stab, &u, &initial_states(&stab).unwrap()).unwrap();
    let fdot: f64 = f.iter().zip(&d).map(|(a, b)| a * b).sum();
    println!("E3 central difference {fd:.12e}, f . d {fdot:.12e}");
    ch.at_most(
        "E3 d strain_energy against internal_force . d, relative",
        (fd - fdot).abs() / fdot.abs(),
        1e-6,
    );
    ch.finish();
}
