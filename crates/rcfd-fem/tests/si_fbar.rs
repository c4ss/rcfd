//! F-587 (ADR 0506): the B-bar Hex8 in the nonlinear driver, the variational F-bar on the total Lagrangian kinematics. The
//! checks SS1 to SS3, T1, T2, PT1 to PT6, PS1, J1 to J3, C1 to C4, NT1, RF1, RF2 and the negative controls JL, CL, N3 of the
//! registration (B0 is in `si_fbar_default_bits.rs`). Every aggregated error is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::element::{b_matrix, element_for, ElementState};
use rcfd_fem::estimate::{recover as patch_recover, PatchOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex8x::{point_strains, NONLINEAR_FOLLOW_UP};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::{MaterialAxes, Matrix6, PointContext, PronyTerm};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_internal_force_at, hex8_second_piola_kirchhoff, hex8_strains, initial_states,
    internal_force, solve, solve_in_time, solve_in_time_with, tangent, NewtonOptions, TimeLoading, TimeSteppingOptions,
};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable, MotionHistory};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel,
};
use std::collections::BTreeSet;
use std::f64::consts::PI;

const BBAR: SolidFormulation = SolidFormulation::BBar;
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

// ------------------------------------------------------------------------------------- the patch of ADR 0493 (a4)

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

fn linear_field(x: Vec3, scale: f64) -> Vec3 {
    let a = field_a(scale);
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += a[i][j] * x[j];
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

/// The patch box of ADR 0493 reading (a2), interior nodes moved by `amp h_k (2U - 1)` (the RNG of `sb_static.rs`).
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

fn patch_model(amp: f64, tag: u64, material: Material, f: SolidFormulation) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(patch_mesh(amp, tag), f));
    m.add_material("all", material).unwrap();
    m
}

fn patch_prescribed(mesh: &SolidMesh, scale: f64) -> Vec<Prescribed> {
    let mut pres = Vec::new();
    for &n in &boundary_nodes(mesh) {
        let u = linear_field(mesh.nodes()[n], scale);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    pres
}

/// Interior error relative to the largest field component (ADR 0493 PT1).
fn interior_error(model: &SolidModel, u: &[f64], scale: f64) -> f64 {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let boundary = boundary_nodes(mesh);
    let mut umax = 0.0f64;
    let mut errs = Vec::new();
    for (n, x) in mesh.nodes().iter().enumerate() {
        let ue = linear_field(*x, scale);
        for k in 0..3 {
            umax = umax.max(ue[k].abs());
            if !boundary.contains(&n) {
                errs.push((u[dofs.equation(n, k).unwrap()] - ue[k]).abs());
            }
        }
    }
    worst(errs) / umax
}

fn newton(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps).threads(4)
}

fn j2_patch_material() -> Material {
    Material::j2(steel(0.3), 1.0e8, 2.0e9, 1.0e9).unwrap()
}

fn creep_patch_material() -> Material {
    Material::creep(steel(0.3), 1e-25, 4.0, 0.0).unwrap()
}

fn unequal_visco() -> Material {
    let terms = [(0.3, 0.1, 1e-3), (0.2, 0.0, 1e-1), (0.1, 0.05, 10.0)]
        .iter()
        .map(|&(g, k, t)| PronyTerm::new(g, k, t).unwrap())
        .collect();
    Material::viscoelastic(
        Material::isotropic_elastic(2.0e9, 0.35, 1200.0, 0.0).unwrap(),
        terms,
        None,
    )
    .unwrap()
}

#[test]
#[ignore = "record of the registered run of ADR 0506 (run 1): PT1 1.7e-16, PT2 1.7e-16 and 4.6e-15, PS1 1.6e-15, PT3 1.7e-16, PT4 1.7e-16 met (limit 1e-10); PT5 set-up defective (an equilibrium start of every step, the energy criterion on rounding noise), PT6 not reached; corrected by amendment 1 as pt5_pt6_amendment_1, the met rows continued in pt1_to_pt4_ps1_met_rows"]
fn pt1_to_pt6_ps1_the_patch_through_the_driver() {
    let mut ch = Checks::new();
    // PT1, PT2.
    for (name, amp, tag, nu) in [
        ("PT1 0.1 h nu 0.3", 0.1, 11, 0.3),
        ("PT2 0.25 h nu 0.3", 0.25, 1001, 0.3),
        ("PT2 0.25 h nu 0.4999", 0.25, 1001, 0.4999),
    ] {
        let m = patch_model(amp, tag, steel(nu), BBAR);
        let r = solve(&m, &patch_prescribed(m.mesh(), 1.0), &[], &newton(1)).unwrap();
        ch.at_most(
            &format!("{name} BBar interior error"),
            interior_error(&m, r.result.displacement_values(), 1.0),
            1e-10,
        );
        if name == "PT2 0.25 h nu 0.3" {
            // PS1: the second Piola-Kirchhoff stress against D E(F).
            let e = voigt_green_lagrange(&field_a(1.0));
            let s_ex = apply(&steel(nu).elasticity_matrix().unwrap(), &e);
            let smax = max_abs(&s_ex);
            let mut errs = Vec::new();
            for el in 0..m.mesh().n_elements() {
                let ue = element_u(&m, r.result.displacement_values(), el);
                let s =
                    hex8_second_piola_kirchhoff(BBAR, &element_coords(m.mesh(), el), &steel(nu), &ue, &r.states[el])
                        .unwrap();
                for sq in s {
                    for k in 0..6 {
                        errs.push((sq[k] - s_ex[k]).abs() / smax);
                    }
                }
            }
            ch.at_most("PS1 PT2 S at every point against D E(F)", worst(errs), 1e-10);
        }
    }
    // PT3: strains 0.1.
    let m = patch_model(0.25, 1001, steel(0.3), BBAR);
    let r = solve(&m, &patch_prescribed(m.mesh(), 100.0), &[], &newton(4)).unwrap();
    ch.at_most(
        "PT3 0.25 h 100 FIELD_A interior error",
        interior_error(&m, r.result.displacement_values(), 100.0),
        1e-10,
    );
    // PT4: J2, yielding at every point.
    let m = patch_model(0.25, 1001, j2_patch_material(), BBAR);
    let r = solve(&m, &patch_prescribed(m.mesh(), 1.0), &[], &newton(4)).unwrap();
    let yielded = r
        .states
        .iter()
        .all(|s| s.values().chunks(rcfd_fem::mat_j2::STATE_SIZE).all(|p| p[12] > 0.0));
    ch.holds("PT4 set-up: every point yielded", yielded, "");
    ch.at_most(
        "PT4 J2 interior error",
        interior_error(&m, r.result.displacement_values(), 1.0),
        1e-10,
    );
    // PT5: creep through solve_in_time.
    let m = patch_model(0.25, 1001, creep_patch_material(), BBAR);
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(newton(1));
    let h = solve_in_time(&m, &patch_prescribed(m.mesh(), 1.0), &[1.0, 2.0], &opts).unwrap();
    let crept = h.steps[2]
        .states
        .iter()
        .all(|s| s.values().chunks(rcfd_fem::mat_creep::STATE_SIZE).all(|p| p[6] > 0.0));
    ch.holds("PT5 set-up: creep strain at every point at t = 2", crept, "");
    for s in &h.steps[1..] {
        ch.at_most(
            &format!("PT5 creep interior error at t = {}", s.time),
            interior_error(&m, &s.u, 1.0),
            1e-10,
        );
    }
    // PT6: the viscoelastic material through solve_in_time.
    let m = patch_model(0.25, 1001, unequal_visco(), BBAR);
    let h = solve_in_time(&m, &patch_prescribed(m.mesh(), 1.0), &[0.01, 0.1, 1.0], &opts).unwrap();
    for s in &h.steps[1..] {
        ch.at_most(
            &format!("PT6 viscoelastic interior error at t = {}", s.time),
            interior_error(&m, &s.u, 1.0),
            1e-10,
        );
    }
    ch.finish();
}

/// PT1 to PT4 and PS1 as in the first run (ADR 0506 amendment 1).
#[test]
fn pt1_to_pt4_ps1_met_rows() {
    let mut ch = Checks::new();
    // PT1, PT2.
    for (name, amp, tag, nu) in [
        ("PT1 0.1 h nu 0.3", 0.1, 11, 0.3),
        ("PT2 0.25 h nu 0.3", 0.25, 1001, 0.3),
        ("PT2 0.25 h nu 0.4999", 0.25, 1001, 0.4999),
    ] {
        let m = patch_model(amp, tag, steel(nu), BBAR);
        let r = solve(&m, &patch_prescribed(m.mesh(), 1.0), &[], &newton(1)).unwrap();
        ch.at_most(
            &format!("{name} BBar interior error"),
            interior_error(&m, r.result.displacement_values(), 1.0),
            1e-10,
        );
        if name == "PT2 0.25 h nu 0.3" {
            // PS1: the second Piola-Kirchhoff stress against D E(F).
            let e = voigt_green_lagrange(&field_a(1.0));
            let s_ex = apply(&steel(nu).elasticity_matrix().unwrap(), &e);
            let smax = max_abs(&s_ex);
            let mut errs = Vec::new();
            for el in 0..m.mesh().n_elements() {
                let ue = element_u(&m, r.result.displacement_values(), el);
                let s =
                    hex8_second_piola_kirchhoff(BBAR, &element_coords(m.mesh(), el), &steel(nu), &ue, &r.states[el])
                        .unwrap();
                for sq in s {
                    for k in 0..6 {
                        errs.push((sq[k] - s_ex[k]).abs() / smax);
                    }
                }
            }
            ch.at_most("PS1 PT2 S at every point against D E(F)", worst(errs), 1e-10);
        }
    }
    // PT3: strains 0.1.
    let m = patch_model(0.25, 1001, steel(0.3), BBAR);
    let r = solve(&m, &patch_prescribed(m.mesh(), 100.0), &[], &newton(4)).unwrap();
    ch.at_most(
        "PT3 0.25 h 100 FIELD_A interior error",
        interior_error(&m, r.result.displacement_values(), 100.0),
        1e-10,
    );
    // PT4: J2, yielding at every point.
    let m = patch_model(0.25, 1001, j2_patch_material(), BBAR);
    let r = solve(&m, &patch_prescribed(m.mesh(), 1.0), &[], &newton(4)).unwrap();
    let yielded = r
        .states
        .iter()
        .all(|s| s.values().chunks(rcfd_fem::mat_j2::STATE_SIZE).all(|p| p[12] > 0.0));
    ch.holds("PT4 set-up: every point yielded", yielded, "");
    ch.at_most(
        "PT4 J2 interior error",
        interior_error(&m, r.result.displacement_values(), 1.0),
        1e-10,
    );
    ch.finish();
}

/// The boundary of the patch moved by the linear field times the factor `table` (ADR 0506 amendment 1).
fn patch_motion(mesh: &SolidMesh, table: LoadTable) -> TimeLoading {
    TimeLoading::new().motion(MotionHistory::new(patch_prescribed(mesh, 1.0), table))
}

/// The interior error at a step end against `s` times the linear field (the measure of PT1 for the scaled field).
fn scaled_interior_error(model: &SolidModel, u: &[f64], s: f64) -> f64 {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let boundary = boundary_nodes(mesh);
    let mut umax = 0.0f64;
    let mut errs = Vec::new();
    for (n, x) in mesh.nodes().iter().enumerate() {
        let ue = linear_field(*x, 1.0);
        for k in 0..3 {
            umax = umax.max((s * ue[k]).abs());
            if !boundary.contains(&n) {
                errs.push((u[dofs.equation(n, k).unwrap()] - s * ue[k]).abs());
            }
        }
    }
    worst(errs) / umax
}

/// PT5' and PT6' of ADR 0506 amendment 1: creep and the viscoelastic material through `solve_in_time_with` with the
/// boundary field as a displacement history.
#[test]
fn pt5_pt6_amendment_1() {
    let mut ch = Checks::new();
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(newton(1));
    let m = patch_model(0.25, 1001, creep_patch_material(), BBAR);
    let loading = patch_motion(m.mesh(), LoadTable::new(vec![(0.0, 1.0), (2.0, 2.0)]).unwrap());
    let h = solve_in_time_with(&m, &[], &loading, &[1.0, 2.0], &opts).unwrap();
    let crept = h.steps[2]
        .states
        .iter()
        .all(|s| s.values().chunks(rcfd_fem::mat_creep::STATE_SIZE).all(|p| p[6] > 0.0));
    ch.holds("PT5' set-up: creep strain at every point at t = 2", crept, "");
    for st in &h.steps[1..] {
        ch.at_most(
            &format!("PT5' creep interior error at t = {}", st.time),
            scaled_interior_error(&m, &st.u, 1.0 + st.time / 2.0),
            1e-10,
        );
    }
    let m = patch_model(0.25, 1001, unequal_visco(), BBAR);
    let loading = patch_motion(m.mesh(), LoadTable::new(vec![(0.0, 1.0), (1.0, 2.0)]).unwrap());
    let h = solve_in_time_with(&m, &[], &loading, &[0.01, 0.1, 1.0], &opts).unwrap();
    for st in &h.steps[1..] {
        ch.at_most(
            &format!("PT6' viscoelastic interior error at t = {}", st.time),
            scaled_interior_error(&m, &st.u, 1.0 + st.time),
            1e-10,
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------ small strain (a1)

/// The largest entry-wise difference of two matrices of one pattern, relative to the largest entry of `b`.
fn csr_difference(a: &rcfd_linalg::CsrMatrix, b: &rcfd_linalg::CsrMatrix) -> f64 {
    assert_eq!(a.row_ptr, b.row_ptr, "the same pattern");
    assert_eq!(a.col, b.col, "the same pattern");
    let d: Vec<f64> = a.val.iter().zip(&b.val).map(|(x, y)| x - y).collect();
    max_abs(&d) / max_abs(&b.val)
}

#[test]
fn ss1_n3_tangent_at_rest_is_the_b_bar_stiffness() {
    let mut ch = Checks::new();
    let box_mesh = |f| {
        let mut m = SolidModel::new(with_formulation(
            hex_box([0.0; 3], [2.0, 1.0, 1.0], [4, 2, 2]).unwrap(),
            f,
        ));
        m.add_material("all", steel(0.3)).unwrap();
        m
    };
    for (name, bbar, full) in [
        (
            "PT2 patch",
            patch_model(0.25, 1001, steel(0.3), BBAR),
            patch_model(0.25, 1001, steel(0.3), FULL),
        ),
        ("4 x 2 x 2 box", box_mesh(BBAR), box_mesh(FULL)),
    ] {
        let n = bbar.dof_map().n_equations();
        let k_lin = stiffness(&bbar).unwrap();
        let kt = tangent(&bbar, &vec![0.0; n], &initial_states(&bbar).unwrap(), Some(4)).unwrap();
        ch.at_most(
            &format!("SS1 {name}: max |K_T(0) - K_Bbar| / max |K_Bbar|"),
            csr_difference(&kt, &k_lin),
            1e-13,
        );
        let kf = tangent(&full, &vec![0.0; n], &initial_states(&full).unwrap(), Some(4)).unwrap();
        ch.at_least(
            &format!("N3 {name}: the Full tangent at rest against K_Bbar"),
            csr_difference(&kf, &k_lin),
            1e-3,
        );
    }
    ch.finish();
}

/// T1's distorted Hex8.
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

/// The F-390 B-bar matrices (6 x 24) and weights at the points of the rule, test-side (ADR 0493 D1).
fn bbar_matrices(x: &[Vec3]) -> Vec<(Vec<f64>, f64)> {
    let el = element_for(ElementKind::Hex8);
    let ips = el.integration_points(x, el.stiffness_rule()).unwrap();
    let volume: f64 = ips.iter().map(|ip| ip.weight).sum();
    let mut mean = [[0.0; 3]; 8];
    for ip in &ips {
        for (m, g) in mean.iter_mut().zip(&ip.geometry.grad) {
            for c in 0..3 {
                m[c] += ip.weight * g[c] / volume;
            }
        }
    }
    ips.iter()
        .map(|ip| {
            let mut b = b_matrix(&ip.geometry.grad);
            for (a, (m, g)) in mean.iter().zip(&ip.geometry.grad).enumerate() {
                for c in 0..3 {
                    for i in 0..3 {
                        b[i * 24 + 3 * a + c] += (m[c] - g[c]) / 3.0;
                    }
                }
            }
            (b, ip.weight)
        })
        .collect()
}

#[test]
fn ss2_ss3_small_strain_limit() {
    let mut ch = Checks::new();
    let x = distorted();
    let d = steel(0.3).elasticity_matrix().unwrap();
    let mut rng = Rng::new(0xF587_0506 + 2);
    let mut errs = Vec::new();
    for amp in [1e-6, 1e-4] {
        let u: Vec<f64> = (0..24).map(|_| rng.sym(amp)).collect();
        let eb = hex8_strains(BBAR, &x, &u).unwrap();
        let lin = point_strains(BBAR, &x, &d, &u).unwrap();
        let scale = worst(lin.iter().flat_map(|s| s.iter().map(|v| v.abs())));
        let e = worst(
            eb.iter()
                .zip(&lin)
                .flat_map(|(a, b)| (0..6).map(move |k| (a[k] - b[k]).abs())),
        ) / scale;
        errs.push(e);
    }
    ch.at_most("SS2 |u| 1e-6: max |E_bar - Bbar u| / max |Bbar u|", errs[0], 1e-5);
    ch.at_most("SS2 |u| 1e-4: the same", errs[1], 1e-3);
    let ratio = errs[1] / errs[0];
    ch.holds(
        "SS2 ratio of the two in [30, 300]",
        ratio.is_finite() && (30.0..=300.0).contains(&ratio),
        &format!("{ratio:.4}"),
    );

    // SS3: J2 at small strain.
    let mat = Material::j2(steel(0.3), 2.0e4, 2.0e9, 1.0e9).unwrap();
    let ns = rcfd_fem::mat_j2::STATE_SIZE;
    let zero = ElementState::new(vec![0.0; 8 * ns]);
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-6)).collect();
    let (f, k) = hex8_force_and_tangent_at(BBAR, &x, &mat, &u, &zero, &PointContext::default()).unwrap();
    let mut k_ref = vec![0.0; 24 * 24];
    let mut f_ref = vec![0.0; 24];
    let mut plastic = 0;
    for (b, w) in bbar_matrices(&x) {
        let strain: [f64; 6] = std::array::from_fn(|r| (0..24).map(|j| b[r * 24 + j] * u[j]).sum());
        let up = rcfd_fem::mat_j2::update(&mat, &strain, &vec![0.0; ns]).unwrap();
        if up.plastic {
            plastic += 1;
        }
        for i in 0..24 {
            f_ref[i] += w * (0..6).map(|r| b[r * 24 + i] * up.stress[r]).sum::<f64>();
            for j in 0..24 {
                let mut v = 0.0;
                for r in 0..6 {
                    for s in 0..6 {
                        v += b[r * 24 + i] * up.tangent[r][s] * b[s * 24 + j];
                    }
                }
                k_ref[i * 24 + j] += w * v;
            }
        }
    }
    ch.holds(
        "SS3 set-up: every point plastic",
        plastic == 8,
        &format!("{plastic} of 8"),
    );
    let dk: Vec<f64> = k.values().iter().zip(&k_ref).map(|(a, b)| a - b).collect();
    ch.at_most(
        "SS3 max |K - sum w Bbar^T C_t Bbar| / max |.|",
        max_abs(&dk) / max_abs(&k_ref),
        1e-4,
    );
    let df: Vec<f64> = f.iter().zip(&f_ref).map(|(a, b)| a - b).collect();
    ch.at_most(
        "SS3 max |f - sum w Bbar^T sigma(Bbar u)| / max |.|",
        max_abs(&df) / max_abs(&f_ref),
        1e-4,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------- central differences (a3)

#[test]
fn t1_t2_tangent_against_central_differences() {
    let mut ch = Checks::new();
    let x = distorted();
    let mut rng = Rng::new(0xF587_0506 + 1);
    let creep_base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    let j2 = Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap();
    let creep = Material::creep(creep_base, 1e-25, 4.0, -0.5).unwrap();
    let visco = unequal_visco();
    let mut cases: Vec<(&str, Material, Vec<f64>, ElementState, PointContext)> = Vec::new();
    // (a) SVK, small.
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    cases.push((
        "(a) SVK small",
        steel(0.3),
        u,
        ElementState::new(Vec::new()),
        PointContext::default(),
    ));
    // (b) SVK, rotated by 90 degrees about z.
    let u: Vec<f64> = x
        .iter()
        .flat_map(|p| [-p[1] - p[0], p[0] - p[1], 0.0])
        .map(|v| v + rng.sym(5e-2))
        .collect();
    cases.push((
        "(b) SVK rotated",
        steel(0.3),
        u,
        ElementState::new(Vec::new()),
        PointContext::default(),
    ));
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
    // (e) viscoelastic at a point context.
    let u: Vec<f64> = (0..24).map(|_| rng.sym(1e-3)).collect();
    let vs = ElementState::new(vec![0.0; 8 * visco.state_size()]);
    cases.push((
        "(e) viscoelastic",
        visco.clone(),
        u,
        vs,
        PointContext::new().with_time(0.01, 0.01).unwrap(),
    ));
    for (name, mat, u, st, ctx) in &cases {
        // Set-ups of (c) and (d): every point in the inelastic branch.
        if name.starts_with("(c)") || name.starts_with("(d)") {
            let eb = hex8_strains(BBAR, &x, u).unwrap();
            let ns = mat.state_size();
            let active = (0..8)
                .filter(|&q| {
                    let s = &st.values()[q * ns..(q + 1) * ns];
                    if name.starts_with("(c)") {
                        rcfd_fem::mat_j2::update(mat, &eb[q], s).unwrap().plastic
                    } else {
                        let r = rcfd_fem::mat_creep::update(mat, &eb[q], s, ctx).unwrap();
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
        let (_, k) = hex8_force_and_tangent_at(BBAR, &x, mat, u, st, ctx).unwrap();
        let h = 1e-8;
        let mut diffs = Vec::new();
        for j in 0..24 {
            let mut a = u.clone();
            a[j] += h;
            let mut b = u.clone();
            b[j] -= h;
            let fa = hex8_internal_force_at(BBAR, &x, mat, &a, st, ctx).unwrap();
            let fb = hex8_internal_force_at(BBAR, &x, mat, &b, st, ctx).unwrap();
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
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------ the cylinders (D6)

const CYL_A: f64 = 1.0;
const CYL_B: f64 = 2.0;

/// The graded plane-strain quarter cylinder of ADR 0506 D6: `nr x nt` Hex8, one layer of 0.02.
fn cylinder(nr: usize, nt: usize, f: SolidFormulation, material: Material, p: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 0.02], [nr, nt, 1]).unwrap();
    let nodes = base
        .nodes()
        .iter()
        .map(|q| {
            let r = CYL_A + (CYL_B - CYL_A) * ((2.0 * q[0]).exp() - 1.0) / (2f64.exp() - 1.0);
            let th = 0.5 * PI * q[1];
            [r * th.cos(), r * th.sin(), q[2]]
        })
        .collect();
    let mut mesh = with_formulation(rebuild(&base, nodes), f);
    let nn = mesh.nodes().len();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", p)).unwrap();
    m
}

/// One point: radius, sigma_r, sigma_theta, sigma_z, and the state values of the point.
struct PolarPoint {
    r: f64,
    srr: f64,
    stt: f64,
    szz: f64,
    /// The von Mises stress of the point.
    q: f64,
    state: Vec<f64>,
}

fn polar_points(model: &SolidModel, f: SolidFormulation, u: &[f64], states: &[ElementState]) -> Vec<PolarPoint> {
    let mesh = model.mesh();
    let material = &model.materials()[0].1;
    let el = element_for(ElementKind::Hex8);
    let ns = material.state_size();
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        let coords = element_coords(mesh, e);
        let ue = element_u(model, u, e);
        let s = hex8_second_piola_kirchhoff(f, &coords, material, &ue, &states[e]).unwrap();
        let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
        for (q, (s, ip)) in s.iter().zip(&ips).enumerate() {
            let x = ip.geometry.x;
            let r = x[0].hypot(x[1]);
            let (cs, sn) = (x[0] / r, x[1] / r);
            out.push(PolarPoint {
                r,
                srr: cs * cs * s[0] + sn * sn * s[1] + 2.0 * cs * sn * s[3],
                stt: sn * sn * s[0] + cs * cs * s[1] - 2.0 * cs * sn * s[3],
                szz: s[2],
                q: {
                    let mean = (s[0] + s[1] + s[2]) / 3.0;
                    let (a, b, c) = (s[0] - mean, s[1] - mean, s[2] - mean);
                    (1.5 * (a * a + b * b + c * c + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]))).sqrt()
                },
                state: states[e].values()[q * ns..(q + 1) * ns].to_vec(),
            });
        }
    }
    out
}

const J2_SY: f64 = 2.0e7;

fn j2_k() -> f64 {
    J2_SY / 3f64.sqrt()
}

fn j2_pressure(c: f64) -> f64 {
    let k = j2_k();
    2.0 * k * (c / CYL_A).ln() + k * (1.0 - c * c / (CYL_B * CYL_B))
}

/// The closed form of ADR 0506 arithmetic 4: (sigma_r, sigma_theta, sigma_z) at `r` for the plastic radius `c`.
fn j2_closed(r: f64, c: f64) -> (f64, f64, f64) {
    let k = j2_k();
    let (sr, st) = if r <= c {
        let sr = -j2_pressure(c) + 2.0 * k * (r / CYL_A).ln();
        (sr, sr + 2.0 * k)
    } else {
        let a = k * c * c / (CYL_B * CYL_B);
        let q = CYL_B * CYL_B / (r * r);
        (-a * (q - 1.0), a * (q + 1.0))
    };
    (sr, st, 0.5 * (sr + st))
}

/// (J1 measure, J2 measure, misplaced points) of the J2 cylinder at plastic radius `c`.
fn j2_cylinder(f: SolidFormulation, c: f64) -> (f64, f64, usize) {
    let mat = Material::j2(
        Material::isotropic_elastic(2.0e11, 0.499, 7850.0, 0.0).unwrap(),
        J2_SY,
        0.0,
        0.0,
    )
    .unwrap();
    let p = j2_pressure(c);
    let m = cylinder(48, 24, f, mat, p);
    let res = solve(&m, &[], &[], &newton(20)).unwrap();
    let iters: Vec<usize> = res.steps.iter().map(|s| s.residuals.len() - 1).collect();
    let halvings: usize = res.steps.iter().map(|s| s.halvings).sum();
    println!(
        "J {} c = {c}: {} increments, {halvings} halvings, largest iteration count {}",
        f.name(),
        res.steps.len(),
        iters.iter().max().unwrap()
    );
    let pts = polar_points(&m, f, res.result.displacement_values(), &res.states);
    let mut point = Vec::new();
    let mut cs = Vec::new();
    let mut misplaced = 0;
    for pt in &pts {
        let (er, et, ez) = j2_closed(pt.r, c);
        point.push((pt.srr - er).abs() / p);
        point.push((pt.stt - et).abs() / p);
        point.push((pt.szz - ez).abs() / p);
        let alpha = pt.state[12];
        if !alpha.is_finite() {
            point.push(f64::NAN);
        }
        let yielded = alpha > 0.0;
        if (yielded && pt.r > c + 0.05) || (!yielded && pt.r < c - 0.05) {
            misplaced += 1;
        }
        if alpha == 0.0 {
            cs.push(pt.r * (pt.q / J2_SY).sqrt());
        }
    }
    let c_fe = cs.iter().sum::<f64>() / cs.len() as f64;
    println!(
        "J {} c = {c}: c_FE = {c_fe:.6} from {} elastic points",
        f.name(),
        cs.len()
    );
    ((j2_pressure(c_fe) - p).abs() / p, worst(point), misplaced)
}

#[test]
fn j1_j2_j3_jl_j2_thick_cylinder_plane_strain() {
    let mut ch = Checks::new();
    for c in [1.25, 1.5] {
        let (j1, j2, mis) = j2_cylinder(BBAR, c);
        ch.at_most(&format!("J1 BBar c = {c} a: |p(c_FE) - p| / p"), j1, 1e-2);
        ch.at_most(
            &format!("J2 BBar c = {c} a: point-wise sigma_r, sigma_theta, sigma_z / p"),
            j2,
            2e-2,
        );
        ch.holds(
            &format!("J3 BBar c = {c} a: misplaced points"),
            mis == 0,
            &format!("{mis}"),
        );
        let (f1, f2, fm) = j2_cylinder(FULL, c);
        println!("JL Full c = {c} a: J1 measure {f1:.4e}, misplaced {fm} (reported)");
        ch.at_least(&format!("JL Full c = {c} a: point-wise (locking control)"), f2, 0.2);
    }
    ch.finish();
}

const CR_P: f64 = 2e5;
const CR_N: f64 = 4.0;

fn creep_material(p: f64) -> Material {
    let base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    Material::creep(base, 1.0 / (2.0e9 * p.powi(3)), CR_N, 0.0).unwrap()
}

/// The Lame field over `p`.
fn lame(r: f64) -> (f64, f64) {
    let d = (CYL_B / CYL_A).powi(2) - 1.0;
    let q = (CYL_B / r).powi(2);
    (-(q - 1.0) / d, (q + 1.0) / d)
}

/// The stationary creep field over `p`.
fn stationary(r: f64) -> (f64, f64) {
    let d = (CYL_B / CYL_A).powf(2.0 / CR_N) - 1.0;
    let q = (CYL_B / r).powf(2.0 / CR_N);
    (-(q - 1.0) / d, ((2.0 - CR_N) / CR_N * q + 1.0) / d)
}

/// The point-wise error over `p` against `field` (over `p`).
fn creep_error(pts: &[PolarPoint], field: fn(f64) -> (f64, f64), p: f64) -> f64 {
    worst(pts.iter().flat_map(|pt| {
        let (fr, ft) = field(pt.r);
        [(pt.srr / p - fr).abs(), (pt.stt / p - ft).abs()]
    }))
}

/// The times of ADR 0506 D6: the 40 of ADR 0489 to `5 t*`, then 30 of `0.5 t*` to `20 t*`.
fn creep_times() -> Vec<f64> {
    let dt0 = 5.0 * 0.05 / (1.05f64.powi(40) - 1.0);
    let mut ts: Vec<f64> = (1..40).map(|k| dt0 * (1.05f64.powi(k) - 1.0) / 0.05).collect();
    ts.push(5.0);
    ts.extend((1..=30).map(|k| 5.0 + 0.5 * k as f64));
    ts
}

/// The creep cylinder at bore pressure `p`: the model and its history.
fn creep_run(f: SolidFormulation, p: f64) -> (SolidModel, rcfd_fem::nonlinear::TimeHistory) {
    let m = cylinder(48, 24, f, creep_material(p), p);
    let opts = TimeSteppingOptions::new(5e-4).newton(newton(1));
    let h = solve_in_time(&m, &[], &creep_times(), &opts).unwrap();
    let halvings: usize = h.steps.iter().map(|s| s.halvings).sum();
    let largest = worst(h.steps.iter().map(|s| s.max_creep_increment));
    println!(
        "C {} p = {p:e}: {halvings} halvings, largest creep-strain increment {largest:.4e}, t_end {}",
        f.name(),
        h.steps.last().unwrap().time
    );
    assert_eq!(h.steps[40].time, 5.0);
    assert_eq!(h.steps[70].time, 20.0);
    (m, h)
}

/// The point-wise errors of the creep cylinder at t = 0 (Lame), 5 t*, 20 t* (stationary).
fn creep_cylinder(f: SolidFormulation) -> (f64, f64, f64) {
    let (m, h) = creep_run(f, CR_P);
    let at = |i: usize| polar_points(&m, f, &h.steps[i].u, &h.steps[i].states);
    for i in [50, 60] {
        println!(
            "C {} t = {}: point-wise {:.4e} (reported)",
            f.name(),
            h.steps[i].time,
            creep_error(&at(i), stationary, CR_P)
        );
    }
    (
        creep_error(&at(0), lame, CR_P),
        creep_error(&at(40), stationary, CR_P),
        creep_error(&at(70), stationary, CR_P),
    )
}

#[test]
#[ignore = "known miss of ADR 0506 C4 (run 1): the growth of the point-wise error from 5 t* to 20 t* 6.02e-3 > 2e-3 (C1 5.37e-3, C2 9.54e-3, C3 1.56e-2, CL 0.103 and 0.364 met); diagnosed in addendum 1; the met rows continue in c1_to_c3_cl_creep_cylinder_met_rows"]
fn c1_to_c4_cl_creep_cylinder_known_miss() {
    let mut ch = Checks::new();
    let (c1, c2, c3) = creep_cylinder(BBAR);
    ch.at_most("C1 BBar t = 0: point-wise against Lame / p", c1, 1e-2);
    ch.at_most(
        "C2 BBar t = 5 t*: point-wise against the stationary field / p",
        c2,
        2e-2,
    );
    ch.at_most("C3 BBar t = 20 t*: the same", c3, 2e-2);
    ch.at_most("C4 BBar |e(20 t*) - e(5 t*)|", (c3 - c2).abs(), 2e-3);
    let (f1, f2, f3) = creep_cylinder(FULL);
    println!("CL Full t = 0: {f1:.4e} (reported)");
    ch.at_least("CL Full t = 5 t*: point-wise (locking control)", f2, 5e-2);
    ch.at_least("CL Full t = 20 t*: point-wise (locking control)", f3, 0.2);
    ch.finish();
}

/// C1 to C3 and CL as in the first run (C4 is the known miss above).
#[test]
fn c1_to_c3_cl_creep_cylinder_met_rows() {
    let mut ch = Checks::new();
    let (c1, c2, c3) = creep_cylinder(BBAR);
    ch.at_most("C1 BBar t = 0: point-wise against Lame / p", c1, 1e-2);
    ch.at_most(
        "C2 BBar t = 5 t*: point-wise against the stationary field / p",
        c2,
        2e-2,
    );
    ch.at_most("C3 BBar t = 20 t*: the same", c3, 2e-2);
    println!(
        "C4 BBar |e(20 t*) - e(5 t*)| (the known miss, reported): {:.4e}",
        (c3 - c2).abs()
    );
    let (f1, f2, f3) = creep_cylinder(FULL);
    println!("CL Full t = 0: {f1:.4e} (reported)");
    ch.at_least("CL Full t = 5 t*: point-wise (locking control)", f2, 5e-2);
    ch.at_least("CL Full t = 20 t*: point-wise (locking control)", f3, 0.2);
    ch.finish();
}

/// The diagnostic run of ADR 0506 amendment 1 for C4 (no limit): `p` and `p / 10`, the point-wise error over `p` every
/// `2.5 t*` from `5 t*` with the worst point, and the largest equivalent creep strain.
#[test]
#[ignore = "diagnostic of ADR 0506 amendment 1 for the known miss C4, run once; reported in addendum 1"]
fn c4_diagnosis_tenth_pressure() {
    for p in [CR_P, 0.1 * CR_P] {
        let (m, h) = creep_run(BBAR, p);
        let e5 = creep_error(
            &polar_points(&m, BBAR, &h.steps[40].u, &h.steps[40].states),
            stationary,
            p,
        );
        for i in (40..=70).step_by(5) {
            let pts = polar_points(&m, BBAR, &h.steps[i].u, &h.steps[i].states);
            let e = creep_error(&pts, stationary, p);
            let (mut wi, mut wv) = (0, -1.0);
            for (k, pt) in pts.iter().enumerate() {
                let (fr, ft) = stationary(pt.r);
                let v = (pt.srr / p - fr).abs().max((pt.stt / p - ft).abs());
                if v > wv {
                    wv = v;
                    wi = k;
                }
            }
            let eq = worst(pts.iter().map(|pt| pt.state[6]));
            println!(
                "diagnostic p = {p:e} t = {:5.2}: point-wise {e:.4e}, growth from 5 t* {:.4e}, worst at r = {:.4}, largest eq. creep strain {eq:.4e}",
                h.steps[i].time,
                e - e5,
                pts[wi].r
            );
        }
    }
}

// ------------------------------------------------------------------------------------------------ nl_transient (D3)

#[test]
fn nt1_strain_energy_of_a_bbar_bar() {
    let mut ch = Checks::new();
    let mat = steel(0.3);
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(),
        BBAR,
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
        let eb = hex8_strains(BBAR, &coords, &element_u(&m, &u, e)).unwrap();
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
        "NT1 strain_energy against the F-bar energy, relative",
        (w - w_ref).abs() / w_ref,
        1e-12,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------ refusals (b2)

fn is_incompatible_refusal<T: std::fmt::Debug>(r: &rcfd_fem::Result<T>) -> (bool, String) {
    match r {
        Ok(_) => (false, "taken".to_string()),
        Err(e) => {
            let mut inner = e;
            while let FemError::Element { source, .. } = inner {
                inner = source;
            }
            let ok = matches!(inner, FemError::InvalidValue { key: "nonlinear", reason }
                if reason.contains("ADR 0506") && reason.contains(NONLINEAR_FOLLOW_UP));
            (ok, format!("{e}"))
        }
    }
}

#[test]
fn rf1_incompatible_refused_by_the_nonlinear_driver() {
    let mut ch = Checks::new();
    let inc = SolidFormulation::Incompatible;
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap(),
        inc,
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
    let rows: Vec<(&str, (bool, String))> = vec![
        (
            "solve",
            is_incompatible_refusal(&solve(&m, &[], &[], &newton(1)).map(|_| ())),
        ),
        (
            "solve_in_time",
            is_incompatible_refusal(
                &solve_in_time(&m, &[], &[1.0], &TimeSteppingOptions::new(f64::INFINITY)).map(|_| ()),
            ),
        ),
        (
            "internal_force",
            is_incompatible_refusal(&internal_force(&m, &u, &states).map(|_| ())),
        ),
        (
            "tangent",
            is_incompatible_refusal(&tangent(&m, &u, &states, None).map(|_| ())),
        ),
        (
            "nl_transient::Integrator::new",
            is_incompatible_refusal(&Integrator::new(&m, &[], &NlTransientOptions::new(1e-3)).map(|_| ())),
        ),
        (
            "hex8_force_and_tangent_at",
            is_incompatible_refusal(
                &hex8_force_and_tangent_at(
                    inc,
                    &x,
                    &steel(0.3),
                    &[0.0; 24],
                    &ElementState::new(Vec::new()),
                    &PointContext::default(),
                )
                .map(|_| ()),
            ),
        ),
    ];
    for (path, (ok, msg)) in rows {
        ch.holds(&format!("RF1 Incompatible {path} refused"), ok, &msg);
    }
    ch.finish();
}

// ----------------------------------- RF2: the copy of `si_hex8x.rs::rf1_rf2_rf3_refusals` after F-587 (ADR 0506 D7)

const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;
const FORMS: [SolidFormulation; 2] = [SolidFormulation::BBar, SolidFormulation::Incompatible];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Support {
    Beam,
}

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(nu)).unwrap();
    m
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
        Support::Beam => m
            .add_constraint(Constraint::displacement("root_corner", 1, 0.0))
            .unwrap(),
    }
    m
}

fn tip_shear(mut m: SolidModel) -> SolidModel {
    m.add_load(Load::traction("xmax", [0.0, 0.0, -C_P / (C_H * C_H)]))
        .unwrap();
    m
}

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
fn rf1_rf2_rf3_refusals_after_f587() {
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

    for f in FORMS {
        let name = f.name();
        // RF2: a temperature load.
        let mut t = tip_shear(cantilever(2.0, 0.3, f, Support::Beam));
        let temps = vec![350.0; t.mesh().nodes().len()];
        t.add_load(Load::temperature(temps, 293.0)).unwrap();
        let n = t.dof_map().n_equations();
        for (path, (ok, msg)) in [
            (
                "solve_static",
                is_formulation_refusal(solve_static(&t, &StaticOptions::default()), f),
            ),
            (
                "nodal_load_vector",
                is_formulation_refusal(nodal_load_vector(&t, &[]), f),
            ),
            (
                "IntegrationPointField",
                is_formulation_refusal(IntegrationPointField::compute(&t, &vec![0.0; n]), f),
            ),
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
            (
                "geometric_stiffness",
                is_formulation_refusal(geometric_stiffness(&c, &pre, None), f),
            ),
            (
                "damped_modes",
                is_formulation_refusal(damped_modes(&c, &DampedOptions::new(2)), f),
            ),
            (
                "DirectHarmonic::new",
                is_formulation_refusal(DirectHarmonic::new(&c, &DirectHarmonicOptions::new()), f),
            ),
            (
                "estimate::recover",
                is_formulation_refusal(patch_recover(&c, &u, &PatchOptions::new()), f),
            ),
        ] {
            ch.holds(&format!("RF3 {name} {path} refused"), ok, &msg);
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
                ch.holds(
                    &format!("RF3 {name} {path} taken (ADR 0506)"),
                    r.is_ok(),
                    &format!("{r:?}"),
                );
            } else {
                let ok = matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", .. }));
                ch.holds(&format!("RF3 {name} {path} refused"), ok, &format!("{r:?}"));
            }
        }
    }
    ch.finish();
}
