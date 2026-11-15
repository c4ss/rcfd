//! F-596 (ADR 0608): the incompatible-modes Hex8 at finite strain (an enhanced deformation gradient, the internal modes
//! condensed in the Newton loop). The checks SS1 to SS3, PT1 to PT4, PM, PS, T1 to T4, X1, Q1, CB1 to CB3, CE, NT1, RF1,
//! RF2 and the negative controls CL, CN, N3, N4 of the registration (B0 is in `si_eas_default_bits.rs`). Every aggregated
//! error is required finite before it is compared. The helpers are copies of those of `si_fbar.rs`, `si_nonlinear.rs` and
//! `si_hex8x_linear.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::estimate::{recover as patch_recover, PatchOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::hex8x::{element_stiffness, internal_modes};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::{HyperelasticEnergy, MaterialAxes, Matrix6, PointContext, PronyTerm};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nl_incompatible::{enhanced_force_and_tangent, modes_and_strains};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions, StepKind};
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_internal_force_at, hex8_second_piola_kirchhoff, hex8_strains, initial_states,
    internal_force, internal_force_with_field, solve, solve_in_time, tangent, NewtonOptions, NonlinearResult,
    TimeSteppingOptions,
};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
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

// ------------------------------------------------------------------------------------- the patch of ADR 0493 (c5)

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

fn patch_model(material: Material, f: SolidFormulation) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(patch_mesh(0.25, 1001), f));
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

/// The largest mode of the converged patch over `max |F - I| * max element edge` (PM).
fn patch_modes(model: &SolidModel, r: &NonlinearResult, material: &Material, g: &[[f64; 3]; 3]) -> f64 {
    let mut amax = Vec::new();
    for e in 0..model.mesh().n_elements() {
        let ue = element_u(model, r.result.displacement_values(), e);
        let (alpha, _) = modes_and_strains(
            &element_coords(model.mesh(), e),
            material,
            &ue,
            &r.states[e],
            &PointContext::default(),
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

// --------------------------------------------------------------------------------- the one-layer cantilever (SS)

const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;

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
    let mut m = SolidModel::new(mesh);
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
fn ss1_ss2_ss3_n3_small_strain_limit() {
    let mut ch = Checks::new();
    let mat = steel(0.3);
    let d = mat.elasticity_matrix().unwrap();
    // SS1: the element at u = 0 against the F-390 condensed stiffness.
    let x = distorted();
    let mut elements = vec![("T1 element", x.clone())];
    let patch = patch_model(mat.clone(), INC);
    for e in 0..patch.mesh().n_elements() {
        elements.push(("patch element", element_coords(patch.mesh(), e)));
    }
    let mut errs = Vec::new();
    for (_, coords) in &elements {
        let (f, k) =
            hex8_force_and_tangent_at(INC, coords, &mat, &[0.0; 24], &empty(), &PointContext::default()).unwrap();
        let kl = element_stiffness(INC, coords, &d).unwrap();
        let dk: Vec<f64> = k.values().iter().zip(kl.values()).map(|(a, b)| a - b).collect();
        errs.push(max_abs(&dk) / max_abs(kl.values()));
        errs.push(max_abs(&f));
    }
    ch.at_most(
        "SS1 element tangent at u = 0 against element_stiffness(Incompatible), T1 element and 36 patch elements",
        worst(errs),
        1e-13,
    );
    let zero_tangent = |m: &SolidModel| {
        let n = m.dof_map().n_equations();
        tangent(m, &vec![0.0; n], &initial_states(m).unwrap(), Some(4)).unwrap()
    };
    ch.at_most(
        "SS1 patch: nonlinear::tangent(0) against assemble::stiffness",
        csr_rel_diff(&zero_tangent(&patch), &stiffness(&patch).unwrap()),
        1e-13,
    );
    let cant = small_cantilever(1.0);
    ch.at_most(
        "SS1 cantilever: nonlinear::tangent(0) against assemble::stiffness",
        csr_rel_diff(&zero_tangent(&cant), &stiffness(&cant).unwrap()),
        1e-13,
    );
    // N3: the Full patch's tangent at 0 against the Incompatible stiffness.
    let full = patch_model(mat.clone(), FULL);
    ch.at_least(
        "N3 Full patch tangent(0) against the Incompatible stiffness",
        csr_rel_diff(&zero_tangent(&full), &stiffness(&patch).unwrap()),
        1e-3,
    );

    // SS2: the modes at small displacement against the linear modes.
    let mut rng = Rng::new(0xF596_0608 + 2);
    let w: Vec<f64> = (0..24).map(|_| rng.sym(1.0)).collect();
    let mut rel = Vec::new();
    for eps in [1e-6, 1e-4] {
        let u: Vec<f64> = w.iter().map(|v| eps * v).collect();
        let (alpha, _) = modes_and_strains(&x, &mat, &u, &empty(), &PointContext::default()).unwrap();
        let al = internal_modes(&x, &d, &u).unwrap();
        let da: Vec<f64> = alpha.iter().zip(&al).map(|(a, b)| a - b).collect();
        rel.push(max_abs(&da) / max_abs(&al));
    }
    ch.at_most("SS2 eps 1e-6: max |alpha - alpha_lin| / max |alpha_lin|", rel[0], 1e-5);
    ch.at_most("SS2 eps 1e-4: the same", rel[1], 1e-3);
    let ratio = rel[1] / rel[0];
    ch.holds(
        "SS2 ratio of the two in [30, 300]",
        ratio.is_finite() && (30.0..=300.0).contains(&ratio),
        &format!("{ratio:.4}"),
    );

    // SS3: the cantilever at the load scale 1e-3 against the linear solve.
    let s = 1e-3;
    let lin = solve_static(&small_cantilever(1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&small_cantilever(s), &[], &[], &newton(1)).unwrap();
    println!(
        "SS3 residuals {:?}",
        nl.steps.iter().map(|st| st.residuals.clone()).collect::<Vec<_>>()
    );
    let ul = lin.displacement_values();
    let du: Vec<f64> = nl
        .result
        .displacement_values()
        .iter()
        .zip(ul)
        .map(|(a, b)| a / s - b)
        .collect();
    ch.at_most(
        "SS3 cantilever s = 1e-3: max |u_nl / s - u_lin| / max |u_lin|",
        max_abs(&du) / max_abs(ul),
        1e-6,
    );
    ch.finish();
}

// --------------------------------------------------------------------------------- patch test at finite strain (c5)

#[test]
fn pt1_to_pt4_pm_ps_patch_at_finite_strain() {
    let mut ch = Checks::new();
    let cases: Vec<(&str, Material, f64, bool, usize)> = vec![
        ("PT1 nu 0.3 FIELD_A", steel(0.3), 1.0, false, 1),
        ("PT1 nu 0.4999 FIELD_A", steel(0.4999), 1.0, false, 1),
        ("PT2 100 FIELD_A", steel(0.3), 100.0, false, 4),
        ("PT3 R(60 deg) (I + 100 FIELD_A)", steel(0.3), 100.0, true, 8),
        ("PT4 J2 FIELD_A", j2_patch_material(), 1.0, false, 4),
    ];
    for (name, mat, scale, rotated, steps) in cases {
        let g = field_grad(scale, rotated);
        let m = patch_model(mat.clone(), INC);
        let r = solve(&m, &patch_prescribed(m.mesh(), &g), &[], &newton(steps)).unwrap();
        println!(
            "{name}: increments {}, iterations {:?}",
            r.steps.len(),
            r.steps.iter().map(|s| s.residuals.len() - 1).collect::<Vec<_>>()
        );
        if name.starts_with("PT4") {
            let yielded = r
                .states
                .iter()
                .all(|s| s.values().chunks(rcfd_fem::mat_j2::STATE_SIZE).all(|p| p[12] > 0.0));
            ch.holds("PT4 set-up: every point yielded", yielded, "");
        }
        ch.at_most(
            &format!("{name}: interior error"),
            interior_error(&m, r.result.displacement_values(), &g),
            1e-10,
        );
        ch.at_most(
            &format!("PM {name}: max |alpha| / (max |F - I| h)"),
            patch_modes(&m, &r, &mat, &g),
            1e-10,
        );
        if name.starts_with("PT2") {
            let e = voigt_green_lagrange(&g);
            let s_ex = apply(&mat.elasticity_matrix().unwrap(), &e);
            let smax = max_abs(&s_ex);
            let mut errs = Vec::new();
            for el in 0..m.mesh().n_elements() {
                let ue = element_u(&m, r.result.displacement_values(), el);
                let s =
                    hex8_second_piola_kirchhoff(INC, &element_coords(m.mesh(), el), &mat, &ue, &r.states[el]).unwrap();
                for sq in s {
                    for k in 0..6 {
                        errs.push((sq[k] - s_ex[k]).abs() / smax);
                    }
                }
            }
            ch.at_most("PS PT2: S at every point against D E(F)", worst(errs), 1e-10);
        }
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------- consistency (c8, c2, c1)

fn neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(8.0e10, 1.6e11).unwrap(), 7850.0).unwrap()
}

#[test]
fn t1_t2_t3_t4_x1_consistency() {
    let mut ch = Checks::new();
    let x = distorted();
    let mut rng = Rng::new(0xF596_0608 + 1);
    let creep_base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    let j2 = Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap();
    let creep = Material::creep(creep_base, 1e-25, 4.0, -0.5).unwrap();
    let mut cases: Vec<(&str, Material, Vec<f64>, ElementState, PointContext)> = Vec::new();
    // (a) SVK, small.
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    cases.push(("(a) SVK small", steel(0.3), u, empty(), PointContext::default()));
    // (b) SVK, rotated by 90 degrees about z.
    let u: Vec<f64> = x
        .iter()
        .flat_map(|p| [-p[1] - p[0], p[0] - p[1], 0.0])
        .map(|v| v + rng.sym(5e-2))
        .collect();
    cases.push(("(b) SVK rotated", steel(0.3), u, empty(), PointContext::default()));
    // (c) J2 from a committed state.
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
    // (d) creep at a point context.
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
    // (e) neo-Hookean at a large strain.
    let b = [[0.5, 0.2, -0.1], [0.1, -0.3, 0.2], [0.3, 0.1, 0.4]];
    let u: Vec<f64> = x
        .iter()
        .flat_map(|p| (0..3).map(move |i| 0.3 * (b[i][0] * p[0] + b[i][1] * p[1] + b[i][2] * p[2])))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|v| v + rng.sym(5e-2))
        .collect();
    cases.push(("(e) neo-Hookean", neo_hookean(), u, empty(), PointContext::default()));

    for (name, mat, u, st, ctx) in &cases {
        let (alpha, strains) = modes_and_strains(&x, mat, u, st, ctx).unwrap();
        // Set-ups of (c) and (d): every point in the inelastic branch.
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
                &format!("T1 {name} set-up: inelastic points"),
                active == 8,
                &format!("{active} of 8"),
            );
        }
        println!("{name}: max |alpha| {:.4e}", max_abs(&alpha));
        let (_, k) = hex8_force_and_tangent_at(INC, &x, mat, u, st, ctx).unwrap();
        let h = 1e-8;
        let mut diffs = Vec::new();
        for j in 0..24 {
            let mut a = u.clone();
            a[j] += h;
            let mut b = u.clone();
            b[j] -= h;
            let fa = hex8_internal_force_at(INC, &x, mat, &a, st, ctx).unwrap();
            let fb = hex8_internal_force_at(INC, &x, mat, &b, st, ctx).unwrap();
            for i in 0..24 {
                diffs.push((k.values()[i * 24 + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = max_abs(k.values());
        ch.at_most(
            &format!("T1 {name}: max |K - K_fd| / max |K|"),
            worst(diffs) / scale,
            1e-6,
        );
        let asym: Vec<f64> = (0..24 * 24)
            .map(|ij| k.values()[ij] - k.values()[(ij % 24) * 24 + ij / 24])
            .collect();
        ch.at_most(
            &format!("T2 {name}: max |K - K^T| / max |K|"),
            max_abs(&asym) / scale,
            1e-12,
        );
        // T4: the mode residual at the returned modes.
        if name.starts_with("(a)") || name.starts_with("(b)") || name.starts_with("(e)") {
            let (f, _) = enhanced_force_and_tangent(&x, mat, u, &alpha, st, ctx).unwrap();
            ch.at_most(
                &format!("T4 {name}: max |R_a| / max |f_u|"),
                max_abs(&f[24..]) / max_abs(&f[..24]),
                1e-10,
            );
        }
        // X1: the strain against the test's own enhanced deformation gradient.
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
                "X1 (b): strain against the test's enhanced F, relative",
                worst(errs) / emax,
                1e-12,
            );
            // T3: the extended tangent at random modes against central differences.
            let modes: [f64; 9] = std::array::from_fn(|_| rng.sym(1e-2));
            let (_, kx) = enhanced_force_and_tangent(&x, mat, u, &modes, st, ctx).unwrap();
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
                let (fa, _) = enhanced_force_and_tangent(&x, mat, &ua, &ma, st, ctx).unwrap();
                let (fb, _) = enhanced_force_and_tangent(&x, mat, &ub, &mb, st, ctx).unwrap();
                for i in 0..33 {
                    diffs.push((kx.values()[i * 33 + jdx] - (fa[i] - fb[i]) / (2.0 * h)).abs());
                }
            }
            ch.at_most(
                "T3 (b) extended 33 x 33 tangent at random modes: max |K - K_fd| / max |K|",
                worst(diffs) / max_abs(kx.values()),
                1e-6,
            );
        }
    }
    ch.finish();
}

// -------------------------------------------------------------------------------- Newton order on a block (c7)

/// The reading Q(C, F) of ADR 0515 without an active set: (violations, pairs, steps without an informative pair, largest
/// ratio r_{k+1} / r_k^2 over the pairs with r_{k+1} > F).
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

fn block(f: SolidFormulation) -> SolidModel {
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
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), f));
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
fn q1_newton_order_on_a_sheared_block() {
    let mut ch = Checks::new();
    let m = block(INC);
    let r = solve(&m, &[], &[], &newton(4)).unwrap();
    for s in &r.steps {
        println!("Q1 step {} residuals {:?}", s.step, s.residuals);
    }
    let (viol, pairs, dull, largest) = q_reading(&r, 10.0, 1e-13);
    println!("Q1 pairs {pairs}, largest r_(k+1)/r_k^2 above 1e-13 {largest:.4e}");
    ch.holds(
        "Q1 Q(10, 1e-13): no violation",
        viol == 0,
        &format!("{viol} of {pairs}"),
    );
    ch.holds(
        "Q1 an informative pair in every step",
        dull == 0 && !r.steps.is_empty(),
        &format!("{dull} steps without"),
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------- the elastica cantilever (c6)

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

fn hex8_cantilever(n: usize, f: SolidFormulation, alpha: f64) -> SolidModel {
    let mesh = with_formulation(hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [n, 1, 1]).unwrap(), f);
    let mut m = SolidModel::new(mesh);
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

/// The tip deflection and shortening: the mean over the nodes of `xmax` (Hex8) or over the two mid-edge nodes on the
/// neutral axis (Hex20, as `si_nonlinear.rs`).
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

/// The elastica `theta'' = -alpha cos(theta)`, `theta(0) = 0`, `theta'(1) = 0` by shooting on `theta'(0)` (bisection):
/// the tip deflection and shortening over `L` (the copy of `si_nonlinear.rs`).
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
fn cb1_to_cb3_ce_cl_cn_n4_elastica_cantilever() {
    let mut ch = Checks::new();
    let registered = [(1.0, 0.30172077, 0.05643324), (2.0, 0.49345748, 0.16064172)];
    for (alpha, v_reg, d_reg) in registered {
        let (v_ref, d_ref) = elastica(alpha);
        ch.at_most(
            &format!("CE alpha {alpha}: elastica against the registered values"),
            (v_ref - v_reg).abs().max((d_ref - d_reg).abs()),
            1e-6,
        );
        let rel = |v: f64, d: f64| ((v / v_ref - 1.0).abs(), (d / d_ref - 1.0).abs());
        // CB1, CB2: the element on 50 and 25 elements.
        let mut inc50 = (0.0, 0.0);
        for (id, n) in [("CB1", 50), ("CB2", 25)] {
            let m = hex8_cantilever(n, INC, alpha);
            let r = run(&m, alpha);
            if id == "CB1" {
                for s in &r.steps {
                    println!("N4 CB1 alpha {alpha} step {} residuals {:?}", s.step, s.residuals);
                }
            }
            let (v, d) = tip(&m, &r, false);
            if id == "CB1" {
                inc50 = (v, d);
            }
            let (ev, ed) = rel(v, d);
            println!("{id} alpha {alpha}: v {v:.10}, d {d:.10}, increments {}", r.steps.len());
            ch.at_most(&format!("{id} alpha {alpha} {n} x 1 x 1: |v / v_ref - 1|"), ev, 2e-3);
            ch.at_most(&format!("{id} alpha {alpha} {n} x 1 x 1: |d / d_ref - 1|"), ed, 2e-3);
        }
        // CB3: against the Hex20 50 x 1 x 1 run.
        let m = hex20_cantilever(alpha);
        let r = run(&m, alpha);
        let (vh, dh) = tip(&m, &r, true);
        let (evh, edh) = rel(vh, dh);
        println!("CB3 alpha {alpha}: Hex20 v {vh:.10}, d {dh:.10} (against the elastica {evh:.4e}, {edh:.4e})");
        ch.at_most(
            &format!("CB3 alpha {alpha}: |v - v_H20| / v_ref"),
            (inc50.0 - vh).abs() / v_ref,
            2e-3,
        );
        ch.at_most(
            &format!("CB3 alpha {alpha}: |d - d_H20| / d_ref"),
            (inc50.1 - dh).abs() / d_ref,
            2e-3,
        );
        // CL: the plain Hex8 on the same 50 elements.
        let m = hex8_cantilever(50, FULL, alpha);
        let (v, d) = tip(&m, &run(&m, alpha), false);
        let (ev, ed) = rel(v, d);
        println!("CL alpha {alpha}: plain Hex8 50 x 1 x 1 |d / d_ref - 1| {ed:.4e}");
        ch.at_least(
            &format!("CL alpha {alpha}: plain Hex8 50 x 1 x 1 |v / v_ref - 1|"),
            ev,
            0.1,
        );
        // CN: the plain Hex8 on 200 elements.
        let m = hex8_cantilever(200, FULL, alpha);
        let (v, d) = tip(&m, &run(&m, alpha), false);
        let (ev, ed) = rel(v, d);
        ch.at_least(
            &format!("CN alpha {alpha}: plain Hex8 200 x 1 x 1 |v / v_ref - 1|"),
            ev,
            5e-3,
        );
        ch.at_least(
            &format!("CN alpha {alpha}: plain Hex8 200 x 1 x 1 |d / d_ref - 1|"),
            ed,
            5e-3,
        );
        // N4: the plain Hex8 on 400 elements, report only.
        let m = hex8_cantilever(400, FULL, alpha);
        let (v, d) = tip(&m, &run(&m, alpha), false);
        let (ev, ed) = rel(v, d);
        println!("N4 alpha {alpha}: plain Hex8 400 x 1 x 1 |v / v_ref - 1| {ev:.4e}, |d / d_ref - 1| {ed:.4e}");
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------ nl_transient (D4)

#[test]
fn nt1_strain_energy_of_an_incompatible_bar() {
    let mut ch = Checks::new();
    let mat = steel(0.3);
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
    let w = it.strain_energy().unwrap();
    let u = it.displacement().unwrap();
    let d = mat.elasticity_matrix().unwrap();
    let el = element_for(ElementKind::Hex8);
    let mut w_ref = 0.0;
    for e in 0..m.mesh().n_elements() {
        let coords = element_coords(m.mesh(), e);
        let (_, eb) =
            modes_and_strains(&coords, &mat, &element_u(&m, &u, e), &empty(), &PointContext::default()).unwrap();
        for (ip, s) in el
            .integration_points(&coords, el.stiffness_rule())
            .unwrap()
            .iter()
            .zip(&eb)
        {
            let sig = apply(&d, s);
            w_ref += ip.weight * 0.5 * sig.iter().zip(s).map(|(a, b)| a * b).sum::<f64>();
        }
    }
    ch.holds("NT1 set-up: the bar moved", w_ref > 0.0, &format!("{w_ref:e}"));
    ch.at_most(
        "NT1 strain_energy against the enhanced energy, relative",
        (w - w_ref).abs() / w_ref,
        1e-12,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------- refusals (c3, c9)

fn key_of<T>(r: &rcfd_fem::Result<T>) -> Option<(&'static str, String)> {
    match r {
        Ok(_) => None,
        Err(e) => {
            let mut inner = e;
            while let FemError::Element { source, .. } = inner {
                inner = source;
            }
            match inner {
                FemError::InvalidValue { key, reason } => Some((key, reason.clone())),
                other => Some(("", format!("{other:?}"))),
            }
        }
    }
}

#[test]
fn rf1_incompatible_taken_after_f596() {
    let mut ch = Checks::new();
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap(),
        INC,
    ));
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 1e6, 0.0])).unwrap();
    let n = m.dof_map().n_equations();
    let u = vec![0.0; n];
    let states = initial_states(&m).unwrap();
    let x = distorted();
    let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        ("solve", solve(&m, &[], &[], &newton(1)).map(|_| ())),
        (
            "solve_in_time",
            solve_in_time(&m, &[], &[1.0], &TimeSteppingOptions::new(f64::INFINITY)).map(|_| ()),
        ),
        ("internal_force", internal_force(&m, &u, &states).map(|_| ())),
        ("tangent", tangent(&m, &u, &states, None).map(|_| ())),
        (
            "nl_transient::Integrator::new",
            Integrator::new(&m, &[], &NlTransientOptions::new(1e-3)).map(|_| ()),
        ),
        (
            "hex8_force_and_tangent_at",
            hex8_force_and_tangent_at(INC, &x, &steel(0.3), &[0.0; 24], &empty(), &PointContext::default()).map(|_| ()),
        ),
    ];
    for (path, r) in rows {
        ch.holds(
            &format!("RF1 Incompatible {path} taken"),
            r.is_ok(),
            &format!("{:?}", r.err()),
        );
    }
    // The refusals that stay.
    let r = Integrator::new(
        &m,
        &[],
        &NlTransientOptions::new(1e-3).step_kind(StepKind::EnergyMomentum),
    )
    .map(|_| ());
    let k = key_of(&r);
    ch.holds(
        "RF1 Incompatible energy-momentum step refused (key nl_transient)",
        matches!(&k, Some(("nl_transient", _))),
        &format!("{k:?}"),
    );
    let temps = vec![350.0; m.mesh().nodes().len()];
    let r = internal_force_with_field(&m, &u, &states, &PointContext::default(), &temps, 293.0).map(|_| ());
    let k = key_of(&r);
    ch.holds(
        "RF1 Incompatible internal_force_with_field refused (key temperature)",
        matches!(&k, Some(("temperature", _))),
        &format!("{k:?}"),
    );
    let r = hex8_strains(INC, &x, &[0.0; 24]).map(|_| ());
    let k = key_of(&r);
    ch.holds(
        "RF1 hex8_strains(Incompatible) refused (key formulation, naming modes_and_strains)",
        matches!(&k, Some(("formulation", reason)) if reason.contains("modes_and_strains")),
        &format!("{k:?}"),
    );
    ch.finish();
}

// ---------------------- RF2: the copy of `si_hex8x_linear.rs::rf1_rf2_rf3_refusals_after_f588_amended` after F-596 (D6)

const FORMS: [SolidFormulation; 2] = [SolidFormulation::BBar, SolidFormulation::Incompatible];

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

fn model_with(mesh: SolidMesh, material: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m
}

fn rf_cantilever(f: SolidFormulation) -> SolidModel {
    let mesh = with_formulation(hex_box([0.0; 3], [2.0, C_H, C_H], [20, 1, 1]).unwrap(), f);
    let mut m = model_with(mesh, steel(0.3));
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1000.0 / (C_H * C_H)]))
        .unwrap();
    m
}

fn strip_model(f: SolidFormulation, material: Material) -> SolidModel {
    let mesh = with_formulation(hex_box([0.0; 3], [2.0, 0.1, 0.1], [10, 1, 1]).unwrap(), f);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
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

/// The orthotropic material "O" of ADR 0525.
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

#[test]
fn rf1_rf2_rf3_refusals_after_f596() {
    let mut ch = Checks::new();
    // RF1: the formulations are a Hex8 choice.
    let tet4 = tet_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let tet10 = upgrade_to_tet10(&tet4).unwrap().0;
    let hex20 = upgrade_to_hex20(&hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap())
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
        let m = strip_model(f, visco(0.03, 0.01, 1.0e-2));
        let r1 = damped_modes(
            &m,
            &DampedOptions::with_modal(modal()).viscoelastic_reference(100.0, None),
        );
        let r2 = DirectHarmonic::new(&m, &DirectHarmonicOptions::new());
        if f == SolidFormulation::Incompatible {
            let (ok, msg) = is_formulation_refusal(r1, f);
            ch.holds(&format!("RF2 {name} viscoelastic damped_modes refused"), ok, &msg);
            let (ok, msg) = is_formulation_refusal(r2, f);
            ch.holds(
                &format!("RF2 {name} viscoelastic DirectHarmonic::new refused"),
                ok,
                &msg,
            );
        } else {
            ch.holds(
                &format!("RF2 {name} viscoelastic damped_modes taken"),
                r1.is_ok(),
                &format!("{:?}", r1.err()),
            );
            ch.holds(
                &format!("RF2 {name} viscoelastic DirectHarmonic::new taken"),
                r2.is_ok(),
                &format!("{:?}", r2.err()),
            );
        }
    }

    for f in FORMS {
        let name = f.name();
        // RF3: the paths lifted are taken.
        let mut t = rf_cantilever(f);
        let temps = vec![350.0; t.mesh().nodes().len()];
        t.add_load(Load::temperature(temps, 293.0)).unwrap();
        let n = t.dof_map().n_equations();
        let mesh = with_formulation(hex_box([0.0; 3], [1.0, 0.25, 0.25], [4, 1, 1]).unwrap(), f);
        let mut o = model_with(
            mesh,
            material_o(
                [0.0; 3],
                MaterialAxes::Fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            ),
        );
        for c in 0..3 {
            o.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        o.add_load(Load::traction("xmax", [1e5, 0.0, 0.0])).unwrap();
        let c = rf_cantilever(f);
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
                (
                    "nodal_load_vector with a temperature",
                    r.is_ok(),
                    format!("{:?}", r.err()),
                )
            },
            {
                let r = IntegrationPointField::compute(&t, &vec![0.0; n]);
                (
                    "IntegrationPointField with a temperature",
                    r.is_ok(),
                    format!("{:?}", r.err()),
                )
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
                // ADR 0525 amendment 2: a box with interior nodes (the strip has none).
                let b = model_with(
                    with_formulation(hex_box([0.0; 3], [1.0, 0.5, 0.5], [4, 2, 2]).unwrap(), f),
                    steel(0.3),
                );
                let nb = b.dof_map().n_equations();
                let r = patch_recover(&b, &vec![0.0; nb], &PatchOptions::new());
                ("estimate::recover", r.is_ok(), format!("{:?}", r.err()))
            },
        ];
        for (path, ok, msg) in rows {
            ch.holds(&format!("RF3 {name} {path} taken"), ok, &msg);
        }
        let states = initial_states(&c).unwrap();
        for (path, r) in [
            ("nonlinear::internal_force", internal_force(&c, &u, &states).map(|_| ())),
            ("nonlinear::tangent", tangent(&c, &u, &states, None).map(|_| ())),
        ] {
            // ADR 0506 D7: the nonlinear rows of BBar are taken; ADR 0608 D6: those of Incompatible too.
            ch.holds(
                &format!("RF3 {name} {path} taken (ADR 0506, ADR 0608)"),
                r.is_ok(),
                &format!("{r:?}"),
            );
        }
    }
    ch.finish();
}
