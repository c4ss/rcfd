// F-703 (ADR 0630 D5): the copies R-* of the ADR 0608 checks (the R-* part of `si_eas_hg.rs` of ADR 0614 and its helpers,
// unchanged but for the rule) with the stabilisation by the rule `STAB` that the including test file defines. The measures,
// set-ups, seeds and limits are those of ADR 0608; R-NT1's own coefficient is the test's closed form of the rule.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::hex8x::{element_stiffness, internal_modes};
use rcfd_fem::material::{HyperelasticEnergy, Matrix6, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, enhanced_force_and_tangent_with, modes_and_strains_with,
    stabilisation_coefficient_with,
};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, solve, tangent, NewtonOptions, NonlinearResult};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;

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
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

const E_MOD: f64 = 2.0e11;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, 7850.0, 1.2e-5).unwrap()
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

/// A model on `mesh` with the stabilisation opted in by the rule `STAB` (ADR 0630 D3).
fn opted(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.set_incompatible_stabilisation(STAB);
    m
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

fn voigt_green_lagrange(a: &[[f64; 3]; 3]) -> [f64; 6] {
    let e = |i: usize, j: usize| a[i][j] + a[j][i] + a[0][i] * a[0][j] + a[1][i] * a[1][j] + a[2][i] * a[2][j];
    [0.5 * e(0, 0), 0.5 * e(1, 1), 0.5 * e(2, 2), e(0, 1), e(1, 2), e(2, 0)]
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
}

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn inverse3(j: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
        + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0]);
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

fn newton(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps).threads(4)
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
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
        .map(|p| [p[0] + rng.sym(0.1), p[1] + rng.sym(0.1), p[2] + rng.sym(0.1)])
        .collect()
}

/// The test's own coefficient of the rule `STAB` at a centre gradient (ADR 0614 arithmetic 4 (iii), ADR 0630 arithmetic 3):
/// `s = D0 voigt((C - C^{-1})/4)`; `s:s / G0` (RestStress), `dev s : dev s / G0` (Deviatoric), `s:s / (G0 + |s|)`
/// (LinearStress).
fn own_coefficient(d0: &Matrix6, f0: &[[f64; 3]; 3]) -> f64 {
    let ft: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| f0[j][i]));
    let c = mat3_mul(&ft, f0);
    let ci = inverse3(&c);
    let e = |i: usize, j: usize| 0.25 * (c[i][j] - ci[i][j]);
    let v = [e(0, 0), e(1, 1), e(2, 2), 2.0 * e(0, 1), 2.0 * e(1, 2), 2.0 * e(2, 0)];
    let mut s = apply(d0, &v);
    let g0 = (d0[3][3] + d0[4][4] + d0[5][5]) / 3.0;
    if STAB == IncompatibleStabilisation::Deviatoric {
        let m = (s[0] + s[1] + s[2]) / 3.0;
        for v in s.iter_mut().take(3) {
            *v -= m;
        }
    }
    let ss = s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]);
    if STAB == IncompatibleStabilisation::LinearStress {
        ss / (g0 + ss.sqrt())
    } else {
        ss / g0
    }
}

// ===================================================================== R-*: the ADR 0608 checks with the stabilisation

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

fn field_a(scale: f64) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| scale * FIELD_A[i][j]))
}

/// The rotation by `angle` about the unit axis `n` (Rodrigues).
fn rotation(n: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let (c, s) = (angle.cos(), angle.sin());
    let k = [[0.0, -n[2], n[1]], [n[2], 0.0, -n[0]], [-n[1], n[0], 0.0]];
    let k2 = mat3_mul(&k, &k);
    std::array::from_fn(|i| {
        std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } + s * k[i][j] + (1.0 - c) * k2[i][j])
    })
}

/// `F - I` of the patch fields: `scale FIELD_A`, or (rotated) `R (I + scale FIELD_A) - I`.
fn field_grad(scale: f64, rotated: bool) -> [[f64; 3]; 3] {
    let a = field_a(scale);
    if !rotated {
        return a;
    }
    let s3 = 1.0 / 3f64.sqrt();
    let r = rotation([s3, s3, s3], std::f64::consts::PI / 3.0);
    let ia: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| a[i][j] + if i == j { 1.0 } else { 0.0 }));
    let f = mat3_mul(&r, &ia);
    std::array::from_fn(|i| std::array::from_fn(|j| f[i][j] - if i == j { 1.0 } else { 0.0 }))
}

fn linear_field(x: Vec3, g: &[[f64; 3]; 3]) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += g[i][j] * x[j];
        }
    }
    u
}

/// The patch box of ADR 0493 reading (a2), interior nodes moved by `amp h_k (2U - 1)`.
fn patch_mesh(amp: f64, tag: u64) -> SolidMesh {
    let base = hex_box(P_ORIGIN, P_LENGTHS, P_DIV).unwrap();
    let mut rng = Rng::new(SEED.wrapping_add(tag));
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

/// The ADR 0608 patch model, opted in.
fn patch_model(material: Material) -> SolidModel {
    let mut m = opted(with_formulation(patch_mesh(0.25, 1001), INC));
    m.add_material("all", material).unwrap();
    m
}

fn patch_prescribed(mesh: &SolidMesh, g: &[[f64; 3]; 3]) -> Vec<Prescribed> {
    let mut pres = Vec::new();
    for &n in &boundary_nodes(mesh) {
        let u = linear_field(mesh.nodes()[n], g);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    pres
}

/// Interior error relative to the largest field component (ADR 0493 PT1).
fn interior_error(model: &SolidModel, u: &[f64], g: &[[f64; 3]; 3]) -> f64 {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let boundary = boundary_nodes(mesh);
    let mut umax = 0.0f64;
    let mut errs = Vec::new();
    for (n, x) in mesh.nodes().iter().enumerate() {
        let ue = linear_field(*x, g);
        for k in 0..3 {
            umax = umax.max(ue[k].abs());
            if !boundary.contains(&n) {
                errs.push((u[dofs.equation(n, k).unwrap()] - ue[k]).abs());
            }
        }
    }
    worst(errs) / umax
}

fn j2_patch_material() -> Material {
    Material::j2(steel(0.3), 1.0e8, 2.0e9, 1.0e9).unwrap()
}

/// The largest stabilised mode of the converged patch over `max |F - I| * max element edge` (PM).
fn patch_modes(model: &SolidModel, r: &NonlinearResult, material: &Material, g: &[[f64; 3]; 3]) -> f64 {
    let mut amax = Vec::new();
    for e in 0..model.mesh().n_elements() {
        let ue = element_u(model, r.result.displacement_values(), e);
        let (alpha, _) = modes_and_strains_with(
            &element_coords(model.mesh(), e),
            material,
            &ue,
            &r.states[e],
            &PointContext::default(),
            STAB,
        )
        .unwrap();
        amax.extend(alpha.iter().map(|a| a.abs()));
    }
    let gmax = worst(g.iter().flat_map(|row| row.iter().map(|v| v.abs())));
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let edge = worst((0..model.mesh().n_elements()).flat_map(|e| {
        let x = element_coords(model.mesh(), e);
        EDGES
            .iter()
            .map(move |&(a, b)| ((0..3).map(|k| (x[a][k] - x[b][k]).powi(2)).sum::<f64>()).sqrt())
    }));
    worst(amax) / (gmax * edge)
}

const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;

/// The one-layer cantilever of ADR 0608 SS, opted in.
fn small_cantilever(scale: f64) -> SolidModel {
    let mut mesh = with_formulation(hex_box([0.0; 3], [2.0, C_H, C_H], [20, 1, 1]).unwrap(), INC);
    let corner = mesh
        .node_set("xmin")
        .unwrap()
        .iter()
        .copied()
        .find(|&n| mesh.nodes()[n] == [0.0, 0.0, 0.0])
        .unwrap();
    mesh.add_node_set("root_corner", vec![corner]).unwrap();
    let mut m = opted(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_corner", 1, 0.0))
        .unwrap();
    m.add_load(Load::traction("xmax", [0.0, 0.0, -scale * C_P / (C_H * C_H)]))
        .unwrap();
    m
}

fn csr_rel_diff(a: &rcfd_linalg::CsrMatrix, b: &rcfd_linalg::CsrMatrix) -> f64 {
    if a.row_ptr != b.row_ptr || a.col != b.col {
        return f64::NAN;
    }
    let d: Vec<f64> = a.val.iter().zip(&b.val).map(|(x, y)| x - y).collect();
    max_abs(&d) / max_abs(&b.val)
}

#[test]
fn r_ss1_ss2_ss3_small_strain_limit_stabilised() {
    let mut ch = Checks::new();
    let mat = steel(0.3);
    let d = mat.elasticity_matrix().unwrap();
    let ctx = PointContext::default();
    let x = distorted();
    let mut elements = vec![x.clone()];
    let patch = patch_model(mat.clone());
    for e in 0..patch.mesh().n_elements() {
        elements.push(element_coords(patch.mesh(), e));
    }
    let mut errs = Vec::new();
    for coords in &elements {
        let (f, k, _) = condensed_force_and_tangent(coords, &mat, &[0.0; 24], &empty(), &ctx, STAB).unwrap();
        let kl = element_stiffness(INC, coords, &d).unwrap();
        let dk: Vec<f64> = k.values().iter().zip(kl.values()).map(|(a, b)| a - b).collect();
        errs.push(max_abs(&dk) / max_abs(kl.values()));
        errs.push(max_abs(&f));
    }
    ch.at_most(
        "R-SS1 stabilised element tangent at u = 0 against element_stiffness(Incompatible), T1 element and 36 patch elements",
        worst(errs),
        1e-13,
    );
    let zero_tangent = |m: &SolidModel| {
        let n = m.dof_map().n_equations();
        tangent(m, &vec![0.0; n], &initial_states(m).unwrap(), Some(4)).unwrap()
    };
    ch.at_most(
        "R-SS1 opted-in patch: nonlinear::tangent(0) against assemble::stiffness",
        csr_rel_diff(&zero_tangent(&patch), &stiffness(&patch).unwrap()),
        1e-13,
    );
    let cant = small_cantilever(1.0);
    ch.at_most(
        "R-SS1 opted-in cantilever: nonlinear::tangent(0) against assemble::stiffness",
        csr_rel_diff(&zero_tangent(&cant), &stiffness(&cant).unwrap()),
        1e-13,
    );

    let mut rng = Rng::new(0xF596_0608 + 2);
    let w: Vec<f64> = (0..24).map(|_| rng.sym(1.0)).collect();
    let mut rel = Vec::new();
    for eps in [1e-6, 1e-4] {
        let u: Vec<f64> = w.iter().map(|v| eps * v).collect();
        let (alpha, _) = modes_and_strains_with(&x, &mat, &u, &empty(), &ctx, STAB).unwrap();
        let al = internal_modes(&x, &d, &u).unwrap();
        let da: Vec<f64> = alpha.iter().zip(&al).map(|(a, b)| a - b).collect();
        rel.push(max_abs(&da) / max_abs(&al));
    }
    ch.at_most(
        "R-SS2 eps 1e-6: max |alpha - alpha_lin| / max |alpha_lin|",
        rel[0],
        1e-5,
    );
    ch.at_most("R-SS2 eps 1e-4: the same", rel[1], 1e-3);
    let ratio = rel[1] / rel[0];
    ch.holds(
        "R-SS2 ratio of the two in [30, 300]",
        ratio.is_finite() && (30.0..=300.0).contains(&ratio),
        &format!("{ratio:.4}"),
    );

    let s = 1e-3;
    let lin = solve_static(&small_cantilever(1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&small_cantilever(s), &[], &[], &newton(1)).unwrap();
    let ul = lin.displacement_values();
    let du: Vec<f64> = nl
        .result
        .displacement_values()
        .iter()
        .zip(ul)
        .map(|(a, b)| a / s - b)
        .collect();
    ch.at_most(
        "R-SS3 opted-in cantilever s = 1e-3: max |u_nl / s - u_lin| / max |u_lin|",
        max_abs(&du) / max_abs(ul),
        1e-6,
    );
    ch.finish();
}

#[test]
fn r_pt1_to_pt4_pm_ps_patch_at_finite_strain_stabilised() {
    let mut ch = Checks::new();
    let cases: Vec<(&str, Material, f64, bool, usize)> = vec![
        ("R-PT1 nu 0.3 FIELD_A", steel(0.3), 1.0, false, 1),
        ("R-PT1 nu 0.4999 FIELD_A", steel(0.4999), 1.0, false, 1),
        ("R-PT2 100 FIELD_A", steel(0.3), 100.0, false, 4),
        ("R-PT3 R(60 deg) (I + 100 FIELD_A)", steel(0.3), 100.0, true, 8),
        ("R-PT4 J2 FIELD_A", j2_patch_material(), 1.0, false, 4),
    ];
    for (name, mat, scale, rotated, steps) in cases {
        let g = field_grad(scale, rotated);
        let m = patch_model(mat.clone());
        let r = solve(&m, &patch_prescribed(m.mesh(), &g), &[], &newton(steps)).unwrap();
        if name.starts_with("R-PT4") {
            let yielded = r
                .states
                .iter()
                .all(|s| s.values().chunks(rcfd_fem::mat_j2::STATE_SIZE).all(|p| p[12] > 0.0));
            ch.holds("R-PT4 set-up: every point yielded", yielded, "");
        }
        ch.at_most(
            &format!("{name}: interior error"),
            interior_error(&m, r.result.displacement_values(), &g),
            1e-10,
        );
        ch.at_most(
            &format!("R-PM {name}: max |alpha| / (max |F - I| h)"),
            patch_modes(&m, &r, &mat, &g),
            1e-10,
        );
        if name.starts_with("R-PT2") {
            let e = voigt_green_lagrange(&g);
            let s_ex = apply(&mat.elasticity_matrix().unwrap(), &e);
            let smax = max_abs(&s_ex);
            let mut errs = Vec::new();
            for el in 0..m.mesh().n_elements() {
                let ue = element_u(&m, r.result.displacement_values(), el);
                let (_, _, s) = condensed_force_and_tangent(
                    &element_coords(m.mesh(), el),
                    &mat,
                    &ue,
                    &r.states[el],
                    &PointContext::default(),
                    STAB,
                )
                .unwrap();
                for sq in s {
                    for k in 0..6 {
                        errs.push((sq[k] - s_ex[k]).abs() / smax);
                    }
                }
            }
            ch.at_most("R-PS PT2: S at every point against D E(F)", worst(errs), 1e-10);
        }
    }
    ch.finish();
}

fn t1_neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(8.0e10, 1.6e11).unwrap(), 7850.0).unwrap()
}

#[test]
fn r_t1_t2_t3_t4_x1_consistency_stabilised() {
    let mut ch = Checks::new();
    let x = distorted();
    let mut rng = Rng::new(0xF596_0608 + 1);
    let creep_base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    let j2 = Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap();
    let creep = Material::creep(creep_base, 1e-25, 4.0, -0.5).unwrap();
    let mut cases: Vec<(&str, Material, Vec<f64>, ElementState, PointContext)> = Vec::new();
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    cases.push(("(a) SVK small", steel(0.3), u, empty(), PointContext::default()));
    let u: Vec<f64> = x
        .iter()
        .flat_map(|p| [-p[1] - p[0], p[0] - p[1], 0.0])
        .map(|v| v + rng.sym(5e-2))
        .collect();
    cases.push(("(b) SVK rotated", steel(0.3), u, empty(), PointContext::default()));
    let mut st = Vec::new();
    for _ in 0..8 {
        let ep: Vec<f64> = (0..6).map(|_| rng.sym(2e-4)).collect();
        let mut beta: Vec<f64> = (0..6).map(|_| rng.sym(1e7)).collect();
        let mean = (beta[0] + beta[1] + beta[2]) / 3.0;
        for b in beta.iter_mut().take(3) {
            *b -= mean;
        }
        st.extend(ep);
        st.extend(beta);
        st.push(3e-4);
    }
    let u: Vec<f64> = (0..24).map(|_| rng.sym(3e-3)).collect();
    cases.push((
        "(c) J2 plastic",
        j2.clone(),
        u,
        ElementState::new(st),
        PointContext::default(),
    ));
    let mut st = Vec::new();
    for _ in 0..8 {
        for _ in 0..6 {
            st.push(rng.sym(2e-4));
        }
        st.push(3e-4);
    }
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    cases.push((
        "(d) creep",
        creep.clone(),
        u,
        ElementState::new(st),
        PointContext::new().with_time(2.0, 0.5).unwrap(),
    ));
    let b = [[0.5, 0.2, -0.1], [0.1, -0.3, 0.2], [0.3, 0.1, 0.4]];
    let u: Vec<f64> = x
        .iter()
        .flat_map(|p| (0..3).map(move |i| 0.3 * (b[i][0] * p[0] + b[i][1] * p[1] + b[i][2] * p[2])))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|v| v + rng.sym(5e-2))
        .collect();
    cases.push(("(e) neo-Hookean", t1_neo_hookean(), u, empty(), PointContext::default()));

    for (name, mat, u, st, ctx) in &cases {
        let (alpha, strains) = modes_and_strains_with(&x, mat, u, st, ctx, STAB).unwrap();
        if name.starts_with("(c)") || name.starts_with("(d)") {
            let ns = mat.state_size();
            let active = (0..8)
                .filter(|&q| {
                    let s = &st.values()[q * ns..(q + 1) * ns];
                    if name.starts_with("(c)") {
                        rcfd_fem::mat_j2::update(mat, &strains[q], s).unwrap().plastic
                    } else {
                        let r = rcfd_fem::mat_creep::update(mat, &strains[q], s, ctx).unwrap();
                        r.creeping && r.state[6] - s[6] > 1e-8
                    }
                })
                .count();
            ch.holds(
                &format!("R-T1 {name} set-up: inelastic points"),
                active == 8,
                &format!("{active} of 8"),
            );
        }
        let c = stabilisation_coefficient_with(&x, mat, u, ctx, STAB).unwrap();
        println!("{name}: max |alpha| {:.4e}, c {c:.4e}", max_abs(&alpha));
        let (_, k, _) = condensed_force_and_tangent(&x, mat, u, st, ctx, STAB).unwrap();
        let h = 1e-8;
        let mut diffs = Vec::new();
        for j in 0..24 {
            let mut a = u.clone();
            a[j] += h;
            let mut bm = u.clone();
            bm[j] -= h;
            let fa = condensed_force_and_tangent(&x, mat, &a, st, ctx, STAB).unwrap().0;
            let fb = condensed_force_and_tangent(&x, mat, &bm, st, ctx, STAB).unwrap().0;
            for i in 0..24 {
                diffs.push((k.values()[i * 24 + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = max_abs(k.values());
        ch.at_most(
            &format!("R-T1 {name}: max |K - K_fd| / max |K|"),
            worst(diffs) / scale,
            1e-6,
        );
        let asym: Vec<f64> = (0..24 * 24)
            .map(|ij| k.values()[ij] - k.values()[(ij % 24) * 24 + ij / 24])
            .collect();
        ch.at_most(
            &format!("R-T2 {name}: max |K - K^T| / max |K|"),
            max_abs(&asym) / scale,
            1e-12,
        );
        if name.starts_with("(a)") || name.starts_with("(b)") || name.starts_with("(e)") {
            let (f, _) = enhanced_force_and_tangent_with(&x, mat, u, &alpha, st, ctx, STAB).unwrap();
            ch.at_most(
                &format!("R-T4 {name}: max |R_a| / max |f_u|"),
                max_abs(&f[24..]) / max_abs(&f[..24]),
                1e-10,
            );
        }
        if name.starts_with("(b)") {
            let el = element_for(ElementKind::Hex8);
            let centre = el.point(&x, [0.0; 3]).unwrap();
            let j0inv = inverse3(&centre.jacobian);
            let mut errs = Vec::new();
            let mut emax = 0.0f64;
            for (q, ip) in el
                .integration_points(&x, el.stiffness_rule())
                .unwrap()
                .iter()
                .enumerate()
            {
                let g = &ip.geometry;
                let mut hm = [[0.0; 3]; 3];
                for (a, ga) in g.grad.iter().enumerate() {
                    for i in 0..3 {
                        for jj in 0..3 {
                            hm[i][jj] += u[3 * a + i] * ga[jj];
                        }
                    }
                }
                for kk in 0..3 {
                    let gk: [f64; 3] =
                        std::array::from_fn(|m| centre.det_j / g.det_j * j0inv[kk][m] * (-2.0 * g.xi[kk]));
                    for c in 0..3 {
                        for jj in 0..3 {
                            hm[c][jj] += alpha[3 * kk + c] * gk[jj];
                        }
                    }
                }
                let e = voigt_green_lagrange(&hm);
                for r in 0..6 {
                    emax = emax.max(e[r].abs());
                    errs.push((e[r] - strains[q][r]).abs());
                }
            }
            ch.at_most(
                "R-X1 (b): stabilised strain against the test's enhanced F, relative",
                worst(errs) / emax,
                1e-12,
            );
            let modes: [f64; 9] = std::array::from_fn(|_| rng.sym(1e-2));
            let (_, kx) = enhanced_force_and_tangent_with(&x, mat, u, &modes, st, ctx, STAB).unwrap();
            let mut diffs = Vec::new();
            for jdx in 0..33 {
                let (mut ua, mut ub, mut ma, mut mb) = (u.clone(), u.clone(), modes, modes);
                if jdx < 24 {
                    ua[jdx] += h;
                    ub[jdx] -= h;
                } else {
                    ma[jdx - 24] += h;
                    mb[jdx - 24] -= h;
                }
                let (fa, _) = enhanced_force_and_tangent_with(&x, mat, &ua, &ma, st, ctx, STAB).unwrap();
                let (fb, _) = enhanced_force_and_tangent_with(&x, mat, &ub, &mb, st, ctx, STAB).unwrap();
                for i in 0..33 {
                    diffs.push((kx.values()[i * 33 + jdx] - (fa[i] - fb[i]) / (2.0 * h)).abs());
                }
            }
            ch.at_most(
                "R-T3 (b) stabilised extended 33 x 33 tangent at random modes: max |K - K_fd| / max |K|",
                worst(diffs) / max_abs(kx.values()),
                1e-6,
            );
        }
    }
    ch.finish();
}

/// The reading Q(C, F) of ADR 0515 without an active set.
fn q_reading(r: &NonlinearResult, c: f64, floor: f64) -> (usize, usize, usize, f64) {
    let (mut viol, mut pairs, mut dull, mut largest) = (0, 0, 0, 0.0f64);
    for s in &r.steps {
        let rs = &s.residuals;
        let mut informative = false;
        for k in 1..rs.len().saturating_sub(1) {
            pairs += 1;
            if rs[k + 1] > floor {
                informative = true;
                largest = largest.max(rs[k + 1] / (rs[k] * rs[k]));
            }
            if rs[k + 1] > (c * rs[k] * rs[k]).max(floor) {
                viol += 1;
            }
        }
        if !informative {
            dull += 1;
        }
    }
    (viol, pairs, dull, largest)
}

/// The sheared block of ADR 0608 Q1, opted in.
fn block() -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [3, 3, 3]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng::new(0xF596_0608);
    let h = 1.0 / 3.0;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = opted(with_formulation(rebuild(&base, nodes), INC));
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    for (c, v) in [0.3, 0.0, -0.1].into_iter().enumerate() {
        m.add_constraint(Constraint::displacement("zmax", c, v)).unwrap();
    }
    m
}

#[test]
fn r_q1_newton_order_on_a_sheared_block_stabilised() {
    let mut ch = Checks::new();
    let r = solve(&block(), &[], &[], &newton(4)).unwrap();
    for s in &r.steps {
        println!("R-Q1 step {} residuals {:?}", s.step, s.residuals);
    }
    let (viol, pairs, dull, largest) = q_reading(&r, 10.0, 1e-13);
    println!("R-Q1 pairs {pairs}, largest r_(k+1)/r_k^2 above 1e-13 {largest:.4e}");
    ch.holds(
        "R-Q1 Q(10, 1e-13): no violation",
        viol == 0,
        &format!("{viol} of {pairs}"),
    );
    ch.holds(
        "R-Q1 an informative pair in every step",
        dull == 0 && !r.steps.is_empty(),
        &format!("{dull} steps without"),
    );
    ch.finish();
}

const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.02;
const RHO: f64 = 7850.0;

fn cant_material() -> Material {
    Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap()
}

fn tip_load(alpha: f64) -> Load {
    let i = CANT_H * CANT_H.powi(3) / 12.0;
    let p = alpha * E_MOD * i / (CANT_L * CANT_L);
    Load::traction("xmax", [0.0, p / (CANT_H * CANT_H), 0.0])
}

/// The Hex8 elastica cantilever of ADR 0608 CB, opted in.
fn hex8_cantilever(n: usize, alpha: f64) -> SolidModel {
    let mesh = with_formulation(hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [n, 1, 1]).unwrap(), INC);
    let mut m = opted(mesh);
    m.add_material("all", cant_material()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(tip_load(alpha)).unwrap();
    m
}

/// The cantilever of `si_nonlinear.rs` (Hex20, 50 x 1 x 1).
fn hex20_cantilever(alpha: f64) -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [50, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", cant_material()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(tip_load(alpha)).unwrap();
    m
}

fn tip(m: &SolidModel, r: &NonlinearResult, hex20: bool) -> (f64, f64) {
    let tol = 1e-9;
    let nodes: Vec<usize> = if hex20 {
        (0..m.mesh().nodes().len())
            .filter(|&n| {
                let p = m.mesh().nodes()[n];
                (p[0] - CANT_L).abs() < tol && (p[1] - CANT_H / 2.0).abs() < tol
            })
            .collect()
    } else {
        m.mesh().node_set("xmax").unwrap().to_vec()
    };
    let k = nodes.len() as f64;
    let v = nodes.iter().map(|&n| r.result.displacement(n)[1]).sum::<f64>() / k;
    let d = -nodes.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / k;
    (v, d)
}

fn rk4(alpha: f64, c0: f64, steps: usize) -> [f64; 4] {
    let h = 1.0 / steps as f64;
    let f = |s: [f64; 4]| [s[1], -alpha * s[0].cos(), s[0].cos(), s[0].sin()];
    let mut s = [0.0, c0, 0.0, 0.0];
    let add = |s: [f64; 4], k: [f64; 4], a: f64| [s[0] + a * k[0], s[1] + a * k[1], s[2] + a * k[2], s[3] + a * k[3]];
    for _ in 0..steps {
        let k1 = f(s);
        let k2 = f(add(s, k1, h / 2.0));
        let k3 = f(add(s, k2, h / 2.0));
        let k4 = f(add(s, k3, h));
        for i in 0..4 {
            s[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
    }
    s
}

fn elastica(alpha: f64) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0, alpha);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if rk4(alpha, mid, 4000)[1] > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let s = rk4(alpha, lo, 4000);
    (s[3], 1.0 - s[2])
}

fn run(m: &SolidModel, alpha: f64) -> NonlinearResult {
    solve(m, &[], &[], &newton((5.0 * alpha) as usize)).unwrap()
}

#[test]
fn r_cb1_to_cb3_ce_elastica_cantilever_stabilised() {
    let mut ch = Checks::new();
    let registered = [(1.0, 0.30172077, 0.05643324), (2.0, 0.49345748, 0.16064172)];
    for (alpha, v_reg, d_reg) in registered {
        let (v_ref, d_ref) = elastica(alpha);
        ch.at_most(
            &format!("R-CE alpha {alpha}: elastica against the registered values"),
            (v_ref - v_reg).abs().max((d_ref - d_reg).abs()),
            1e-6,
        );
        let rel = |v: f64, d: f64| ((v / v_ref - 1.0).abs(), (d / d_ref - 1.0).abs());
        let mut inc50 = (0.0, 0.0);
        for (id, n) in [("R-CB1", 50), ("R-CB2", 25)] {
            let m = hex8_cantilever(n, alpha);
            let r = run(&m, alpha);
            let (v, d) = tip(&m, &r, false);
            if id == "R-CB1" {
                inc50 = (v, d);
            }
            let (ev, ed) = rel(v, d);
            println!("{id} alpha {alpha}: v {v:.10}, d {d:.10}, increments {}", r.steps.len());
            ch.at_most(&format!("{id} alpha {alpha} {n} x 1 x 1: |v / v_ref - 1|"), ev, 2e-3);
            ch.at_most(&format!("{id} alpha {alpha} {n} x 1 x 1: |d / d_ref - 1|"), ed, 2e-3);
        }
        let m = hex20_cantilever(alpha);
        let r = run(&m, alpha);
        let (vh, dh) = tip(&m, &r, true);
        ch.at_most(
            &format!("R-CB3 alpha {alpha}: |v - v_H20| / v_ref"),
            (inc50.0 - vh).abs() / v_ref,
            2e-3,
        );
        ch.at_most(
            &format!("R-CB3 alpha {alpha}: |d - d_H20| / d_ref"),
            (inc50.1 - dh).abs() / d_ref,
            2e-3,
        );
    }
    ch.finish();
}

#[test]
fn r_nt1_strain_energy_of_a_stabilised_bar() {
    let mut ch = Checks::new();
    let mat = steel(0.3);
    let mut m = opted(with_formulation(
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
    let w = it.strain_energy().unwrap();
    let u = it.displacement().unwrap();
    let d = mat.elasticity_matrix().unwrap();
    let el = element_for(ElementKind::Hex8);
    let (mut w_ref, mut w_pen) = (0.0, 0.0);
    for e in 0..m.mesh().n_elements() {
        let coords = element_coords(m.mesh(), e);
        let ue = element_u(&m, &u, e);
        let (alpha, eb) = modes_and_strains_with(&coords, &mat, &ue, &empty(), &PointContext::default(), STAB).unwrap();
        let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
        for (ip, s) in ips.iter().zip(&eb) {
            let sig = apply(&d, s);
            w_ref += ip.weight * 0.5 * sig.iter().zip(s).map(|(a, b)| a * b).sum::<f64>();
        }
        // The test's own Pi_s: c from the centre gradient, P from the mode gradients.
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
        let c = own_coefficient(&d, &f0);
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
    println!("R-NT1 elastic part {w_ref:.10e}, stabilisation {w_pen:.4e}");
    let total = w_ref + w_pen;
    ch.holds(
        "R-NT1 set-up: the bar moved and the stabilisation acts",
        w_ref > 0.0 && w_pen > 0.0,
        &format!("{w_ref:e}, {w_pen:e}"),
    );
    ch.at_most(
        "R-NT1 strain_energy against the enhanced and stabilisation energy, relative",
        (w - total).abs() / total,
        1e-12,
    );
    ch.finish();
}
