//! F-408 (ADR 0440): the scaffold of the advanced tier. Checks D1 to D8, M1 to M4, P1, P2, K1, R1 to R4, H1 to H3, X1 and
//! the negative controls N2, N3 of the registration (amendment 1 for N3 and H3). B1 to B8 and N1 are in
//! `sa_scaffold_bits.rs`, Z1 to Z3 and N4 in `rcfd-linalg/tests/sa_complex.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementMatrix};
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::material::{
    isotropic_elasticity, HyperelasticEnergy, MaterialAxes, PointContext, PronyTerm, TimeTemperatureShift,
};
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::nonlinear::{
    add_triplets, initial_states, internal_force, solve, solve_with, tangent, Contribution, DriverExtensions, LoadStep,
    NewtonOptions, NonlinearResult, StateLayout, StepControl,
};
use rcfd_fem::shell::{
    self, element_stiffness, element_stiffness_section, homogeneous_section, section_of, SectionStiffness,
    ShearStiffness, ShellOptions,
};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, Ply, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::cell::Cell;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------------- models

/// The cantilever C of ADR 0291 with a material.
fn cantilever_with(mat: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m
}

fn p_exact() -> f64 {
    E_MOD * 1.2 * (1.2 * 1.2 - 1.0) / 2.0
}

/// The bar of `si_nonlinear.rs` (Hex8 `[1, 0.5, 0.5]`, `[2, 1, 1]`, nu = 0, three symmetry planes, the traction of the
/// stretch 1.2 for E = 2e11) with Young's modulus `e`.
fn stretch_bar(e: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(e, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [p_exact(), 0.0, 0.0])).unwrap();
    m
}

/// The elastica cantilever of `si_nonlinear.rs` at alpha = 1.
fn elastica() -> SolidModel {
    let (l, h) = (1.0, 0.02);
    let mesh = rcfd_fem::hex20::upgrade_to_hex20(&hex_box([0.0; 3], [l, h, h], [50, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.0)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let i = h * h.powi(3) / 12.0;
    let p = E_MOD * i / (l * l);
    m.add_load(Load::traction("xmax", [0.0, p / (h * h), 0.0])).unwrap();
    m
}

/// The simply supported plate of `si_shell.rs` (16 x 16, unit square): nodes, quadrilaterals, prescribed list.
fn plate_grid() -> (Vec<Vec3>, Vec<[usize; 4]>, Vec<Prescribed>) {
    let n = 16;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut quads = Vec::new();
    for j in 0..n {
        for i in 0..n {
            quads.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            pres.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            pres.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            pres.push(Prescribed::new(k, 4, 0.0));
        }
    }
    (nodes, quads, pres)
}

fn shell_model(kind: ElementKind, property: BlockProperty) -> (SolidModel, Vec<Prescribed>) {
    let (nodes, quads, pres) = plate_grid();
    let conn: Vec<usize> = match kind {
        ElementKind::Shell4 => quads.iter().flatten().copied().collect(),
        ElementKind::Shell3 => quads
            .iter()
            .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
            .collect(),
        _ => unreachable!(),
    };
    let nn = nodes.len();
    let ne = conn.len() / kind.nodes_per_element();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    (m, pres)
}

// ------------------------------------------------------------------------------------------------- D1 to D8

fn same_result(a: &NonlinearResult, b: &NonlinearResult) -> (bool, String) {
    let u = bits(a.result.displacement_values()) == bits(b.result.displacement_values());
    let r = bits(a.result.reactions().unwrap()) == bits(b.result.reactions().unwrap());
    let steps = a.steps.len() == b.steps.len()
        && a.steps.iter().zip(&b.steps).all(|(x, y)| {
            x.step == y.step
                && x.halvings == y.halvings
                && x.load_factor.to_bits() == y.load_factor.to_bits()
                && bits(&x.residuals) == bits(&y.residuals)
                && bits(&x.energies) == bits(&y.energies)
        });
    (
        u && r && steps,
        format!(
            "u {u}, reactions {r}, increments {steps} ({} increments)",
            a.steps.len()
        ),
    )
}

#[test]
fn d1_to_d4_driver_without_extensions_and_step_control() {
    let mut c = Checks::new();
    // D1: the default extensions are `solve`, for E and H.
    let em = elastica();
    let e0 = solve(&em, &[], &[], &NewtonOptions::new(5)).unwrap();
    let e1 = solve_with(&em, &[], &[], &NewtonOptions::new(5), DriverExtensions::default()).unwrap();
    let (ok, d) = same_result(&e0, &e1);
    c.holds("D1 E: solve_with(default) = solve", ok, &d);
    let h = stretch_bar(E_MOD);
    let ho = NewtonOptions::new(1).max_iterations(4);
    let h0 = solve(&h, &[], &[], &ho).unwrap();
    let h1 = solve_with(&h, &[], &[], &ho, DriverExtensions::default()).unwrap();
    let (ok, d) = same_result(&h0, &h1);
    c.holds("D1 H: solve_with(default) = solve", ok, &d);

    // D2: an identity hook.
    struct Identity {
        converged: usize,
        failed: usize,
    }
    impl StepControl for Identity {
        fn name(&self) -> &str {
            "identity"
        }
        fn load_factor(&mut self, _c: f64, proposed: f64, _s: usize, _h: usize) -> rcfd_fem::Result<f64> {
            Ok(proposed)
        }
        fn converged(&mut self, _i: &LoadStep) {
            self.converged += 1;
        }
        fn failed(&mut self, _c: f64, _a: f64) {
            self.failed += 1;
        }
    }
    let mut id = Identity {
        converged: 0,
        failed: 0,
    };
    let h2 = solve_with(
        &h,
        &[],
        &[],
        &ho,
        DriverExtensions {
            contributions: Vec::new(),
            step_control: Some(&mut id),
        },
    )
    .unwrap();
    let (ok, d) = same_result(&h0, &h2);
    c.holds("D2 identity hook = solve", ok, &d);
    let mut expected_failed = 0;
    for (k, s) in h0.steps.iter().enumerate() {
        if h0.steps.get(k + 1).is_none_or(|n| n.step != s.step) {
            expected_failed += s.halvings;
        }
    }
    c.holds(
        "D2 callbacks",
        id.converged == h0.steps.len() && id.failed == expected_failed,
        &format!(
            "converged {} of {}, failed {} of {}",
            id.converged,
            h0.steps.len(),
            id.failed,
            expected_failed
        ),
    );

    // D3: a hook capping each attempt at committed + 0.5, one user step, against two load steps.
    struct Cap;
    impl StepControl for Cap {
        fn name(&self) -> &str {
            "cap"
        }
        fn load_factor(&mut self, committed: f64, proposed: f64, _s: usize, _h: usize) -> rcfd_fem::Result<f64> {
            Ok(if proposed - committed > 0.5 {
                committed + 0.5
            } else {
                proposed
            })
        }
    }
    let bar = stretch_bar(E_MOD);
    let two = solve(&bar, &[], &[], &NewtonOptions::new(2)).unwrap();
    let mut cap = Cap;
    let capped = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: Vec::new(),
            step_control: Some(&mut cap),
        },
    )
    .unwrap();
    let same = bits(capped.result.displacement_values()) == bits(two.result.displacement_values())
        && capped.steps.len() == two.steps.len()
        && capped.steps.iter().zip(&two.steps).all(|(x, y)| {
            x.load_factor.to_bits() == y.load_factor.to_bits()
                && bits(&x.residuals) == bits(&y.residuals)
                && bits(&x.energies) == bits(&y.energies)
        });
    c.holds(
        "D3 capped hook = two load steps",
        same,
        &format!(
            "load factors {:?} and {:?}",
            capped.steps.iter().map(|s| s.load_factor).collect::<Vec<_>>(),
            two.steps.iter().map(|s| s.load_factor).collect::<Vec<_>>()
        ),
    );

    // D4: values outside (committed, target].
    struct Fixed(f64, bool);
    impl StepControl for Fixed {
        fn name(&self) -> &str {
            "fixed"
        }
        fn load_factor(&mut self, committed: f64, _p: f64, _s: usize, _h: usize) -> rcfd_fem::Result<f64> {
            Ok(if self.1 { committed } else { self.0 })
        }
    }
    for (name, mut hook) in [("committed", Fixed(0.0, true)), ("target + 0.25", Fixed(1.25, false))] {
        let r = solve_with(
            &bar,
            &[],
            &[],
            &NewtonOptions::new(1),
            DriverExtensions {
                contributions: Vec::new(),
                step_control: Some(&mut hook),
            },
        );
        let ok = matches!(&r, Err(FemError::InvalidValue { key, .. }) if *key == "step control");
        c.holds(&format!("D4 hook returning {name}"), ok, &format!("{:?}", r.err()));
    }
    c.finish();
}

/// A contribution that adds the model's own internal force and tangent (D5).
struct SelfCopy<'a> {
    model: &'a SolidModel,
}

impl Contribution for SelfCopy<'_> {
    fn name(&self) -> &str {
        "copy of the internal force"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent_triplets: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let states = initial_states(self.model)?;
        let f = internal_force(self.model, u, &states)?;
        force.copy_from_slice(&f);
        let k = tangent(self.model, u, &states, None)?;
        for r in 0..k.row_ptr.len() - 1 {
            for q in k.row_ptr[r]..k.row_ptr[r + 1] {
                tangent_triplets.push((r, k.col[q], k.val[q]));
            }
        }
        Ok(())
    }
}

#[test]
#[ignore = "record of the registered run of ADR 0440 (commit 144c2d4a): D5, D7, D8, N2 met; D6 set-up defective (equations 3 and 35 are on the pattern, the off-pattern path not exercised; measure 0); corrected by amendment 2 as d6b_off_pattern_contribution"]
fn d5_to_d8_contributions() {
    let mut c = Checks::new();
    // D5: the copy contribution doubles E.
    let bar = stretch_bar(E_MOD);
    let copy = SelfCopy { model: &bar };
    let doubled = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: vec![&copy],
            step_control: None,
        },
    )
    .unwrap();
    let stiff = solve(&stretch_bar(2.0 * E_MOD), &[], &[], &NewtonOptions::new(1)).unwrap();
    let same = bits(doubled.result.displacement_values()) == bits(stiff.result.displacement_values())
        && bits(doubled.result.reactions().unwrap()) == bits(stiff.result.reactions().unwrap())
        && doubled.steps.len() == stiff.steps.len()
        && doubled
            .steps
            .iter()
            .zip(&stiff.steps)
            .all(|(x, y)| bits(&x.residuals) == bits(&y.residuals) && bits(&x.energies) == bits(&y.energies));
    c.holds(
        "D5 copy contribution = E doubled",
        same,
        &format!(
            "iterations {:?} and {:?}",
            doubled.steps.iter().map(|s| s.residuals.len() - 1).collect::<Vec<_>>(),
            stiff.steps.iter().map(|s| s.residuals.len() - 1).collect::<Vec<_>>()
        ),
    );
    // N2: against E = next_up(4e11) the displacement differs.
    let next = f64::from_bits((2.0 * E_MOD).to_bits() + 1);
    let off = solve(&stretch_bar(next), &[], &[], &NewtonOptions::new(1)).unwrap();
    c.holds(
        "N2 E = next_up(4e11) differs",
        bits(doubled.result.displacement_values()) != bits(off.result.displacement_values()),
        "some entry differs",
    );

    // D6: zero triplets off the pattern.
    struct OffPattern {
        a: usize,
        b: usize,
    }
    impl Contribution for OffPattern {
        fn name(&self) -> &str {
            "zeros off the pattern"
        }
        fn add(&self, _u: &[f64], _l: f64, _f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
            t.push((self.a, self.b, 0.0));
            t.push((self.b, self.a, 0.0));
            Ok(())
        }
    }
    let reference = solve(&bar, &[], &[], &NewtonOptions::new(1)).unwrap();
    let k0 = stiffness(&bar).unwrap();
    // The first and the last free equation: the constrained components are x at xmin, y at ymin, z at zmin.
    let dofs = bar.dof_map();
    let fixed: Vec<bool> = {
        let mut f = vec![false; dofs.n_equations()];
        for (set, comp) in [("xmin", 0), ("ymin", 1), ("zmin", 2)] {
            for &n in bar.mesh().node_set(set).unwrap() {
                f[dofs.equation(n, comp).unwrap()] = true;
            }
        }
        f
    };
    let first = (0..fixed.len()).find(|&i| !fixed[i]).unwrap();
    let last = (0..fixed.len()).rev().find(|&i| !fixed[i]).unwrap();
    let on_pattern = k0.col[k0.row_ptr[first]..k0.row_ptr[first + 1]].contains(&last);
    let opc = OffPattern { a: first, b: last };
    let r6 = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: vec![&opc],
            step_control: None,
        },
    )
    .unwrap();
    let (a, b) = (r6.result.displacement_values(), reference.result.displacement_values());
    let dev =
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    println!("D6: equations {first} and {last}, on the pattern before: {on_pattern}");
    c.at_most("D6 off-pattern zeros: max |u - u_ref| / max |u_ref|", dev, 1e-10);
    c.holds("D6 the pair is off the pattern", !on_pattern, "set-up");

    // D7: an error of a contribution names it.
    struct Failing;
    impl Contribution for Failing {
        fn name(&self) -> &str {
            "always fails"
        }
        fn add(&self, _u: &[f64], _l: f64, _f: &mut [f64], _t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
            Err(FemError::InvalidValue {
                key: "test",
                reason: "refused on purpose".to_string(),
            })
        }
    }
    let failing = Failing;
    let r7 = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: vec![&failing],
            step_control: None,
        },
    );
    let ok = matches!(&r7, Err(FemError::Solver { reason, .. }) if reason.contains("always fails"));
    c.holds("D7 the error names the contribution", ok, &format!("{:?}", r7.err()));

    // D8: add_triplets on a 3 x 3 example.
    let mut m = CsrMatrix::from_triplets(3, vec![(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 4.0), (2, 2, 5.0)]);
    add_triplets(&mut m, &[(0, 1, 0.5)]);
    let in_place = m.val == vec![1.0, 2.5, 3.0, 4.0, 5.0] && m.col == vec![0, 1, 0, 1, 2];
    add_triplets(&mut m, &[(1, 1, 1.0), (0, 2, 7.0)]);
    let mut dense = [[0.0; 3]; 3];
    for r in 0..3 {
        for q in m.row_ptr[r]..m.row_ptr[r + 1] {
            dense[r][m.col[q]] += m.val[q];
        }
    }
    let expect = [[1.0, 2.5, 7.0], [3.0, 5.0, 0.0], [0.0, 0.0, 5.0]];
    c.holds(
        "D8 add_triplets",
        in_place && dense == expect,
        &format!("in place {in_place}, dense {dense:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- M1 to M4

fn iso() -> Material {
    steel(0.3)
}

fn rot(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, co) = angle.sin_cos();
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]][i][j];
            r[i][j] = (if i == j { co } else { 0.0 }) + s * kk + (1.0 - co) * k[i] * k[j];
        }
    }
    r
}

fn ortho() -> Material {
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [-1e-6, 3e-5, 3e-5],
        MaterialAxes::fixed(rot([0.0, 0.0, 1.0], 0.5)).unwrap(),
    )
    .unwrap()
}

/// The six pending variants with their registered name and package.
fn pending_variants() -> Vec<(Material, &'static str, &'static str)> {
    let wlf = TimeTemperatureShift::wlf(300.0, 17.4, 51.6).unwrap();
    let terms = vec![
        PronyTerm::new(0.3, 0.1, 1e-3).unwrap(),
        PronyTerm::new(0.2, 0.0, 1e-1).unwrap(),
        PronyTerm::new(0.1, 0.05, 10.0).unwrap(),
    ];
    vec![
        (ortho(), "Orthotropic", "ADR 0312 (F-409)"),
        (
            Material::tabulated(iso(), vec![293.0, 373.0], vec![("e".to_string(), vec![2.0e11, 1.9e11])]).unwrap(),
            "Tabulated",
            "ADR 0312 (F-409)",
        ),
        (
            Material::viscoelastic(iso(), terms, Some(wlf)).unwrap(),
            "Viscoelastic",
            "ADR 0313 (F-410)",
        ),
        (
            Material::creep(iso(), 1e-30, 4.0, -0.5).unwrap(),
            "Creep",
            "ADR 0314 (F-411)",
        ),
        (Material::j2(iso(), 2.5e8, 1e9, 5e8).unwrap(), "J2", "ADR 0325 (F-422)"),
        (
            Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e6, 1e9).unwrap(), 1100.0).unwrap(),
            "Hyperelastic",
            "ADR 0328 (F-425)",
        ),
    ]
}

fn not_yet(
    r: &rcfd_fem::Result<impl std::fmt::Debug>,
    item: &str,
    variant: &str,
    location: &str,
    package: &str,
) -> bool {
    match r {
        Err(
            e @ FemError::NotYetSupported {
                item: i,
                variant: v,
                location: l,
                package: p,
            },
        ) => {
            let msg = e.to_string();
            *i == item
                && *v == variant
                && l == location
                && *p == package
                && msg.contains(variant)
                && msg.contains(location)
        }
        _ => false,
    }
}

#[test]
#[ignore = "record of ADR 0440: ADR 0452 (F-409) landed Orthotropic and Tabulated (pending None, the constitutive call of ADR 0452 D3), retiring their M1 and M4 rows; continued in m1_to_m4_after_f409 (ADR 0291 pattern, maintainer standing decision)"]
fn m1_to_m4_material_variants_and_state_layout() {
    let mut c = Checks::new();
    // M1.
    for (m, name, package) in pending_variants() {
        c.holds(
            &format!("M1 {name}"),
            m.name() == name && m.pending() == Some(package),
            &format!("{} {:?}", m.name(), m.pending()),
        );
    }
    c.holds(
        "M1 IsotropicElastic",
        iso().name() == "IsotropicElastic" && iso().pending().is_none(),
        "",
    );
    // M2: refused data.
    let r3 = rot([1.0, 2.0, 3.0], 0.3);
    let mut scaled = r3;
    scaled[0][0] *= 1.001;
    let mut left = r3;
    left[2] = [-left[2][0], -left[2][1], -left[2][2]];
    let fixed = || MaterialAxes::fixed(r3).unwrap();
    let refused: Vec<(&str, bool)> = vec![
        (
            "orthotropic E_2 = 0",
            Material::orthotropic([1e11, 0.0, 1e10], [0.3; 3], [5e9; 3], 1600.0, [0.0; 3], fixed()).is_err(),
        ),
        (
            "orthotropic nu NaN",
            Material::orthotropic(
                [1e11, 1e10, 1e10],
                [0.3, f64::NAN, 0.3],
                [5e9; 3],
                1600.0,
                [0.0; 3],
                fixed(),
            )
            .is_err(),
        ),
        ("axes scaled", MaterialAxes::fixed(scaled).is_err()),
        ("axes left-handed", MaterialAxes::fixed(left).is_err()),
        (
            "cylindrical zero axis",
            MaterialAxes::cylindrical([0.0; 3], [0.0; 3]).is_err(),
        ),
        ("per-element empty", MaterialAxes::per_element(Vec::new()).is_err()),
        (
            "tabulated one temperature",
            Material::tabulated(iso(), vec![293.0], vec![("e".into(), vec![2e11])]).is_err(),
        ),
        (
            "tabulated decreasing",
            Material::tabulated(iso(), vec![373.0, 293.0], vec![("e".into(), vec![2e11, 1.9e11])]).is_err(),
        ),
        (
            "tabulated short column",
            Material::tabulated(iso(), vec![293.0, 373.0], vec![("e".into(), vec![2e11])]).is_err(),
        ),
        (
            "tabulated J2 base",
            Material::tabulated(
                Material::j2(iso(), 2.5e8, 0.0, 0.0).unwrap(),
                vec![293.0, 373.0],
                vec![("e".into(), vec![2e11, 1.9e11])],
            )
            .is_err(),
        ),
        (
            "Prony weights sum 1.5",
            Material::viscoelastic(
                iso(),
                vec![
                    PronyTerm::new(0.8, 0.0, 1.0).unwrap(),
                    PronyTerm::new(0.7, 0.0, 2.0).unwrap(),
                ],
                None,
            )
            .is_err(),
        ),
        ("Prony tau 0", PronyTerm::new(0.1, 0.1, 0.0).is_err()),
        ("creep n = 0", Material::creep(iso(), 1e-30, 0.0, 0.0).is_err()),
        ("creep m = -1", Material::creep(iso(), 1e-30, 4.0, -1.0).is_err()),
        ("J2 orthotropic base", Material::j2(ortho(), 2.5e8, 0.0, 0.0).is_err()),
        ("J2 yield 0", Material::j2(iso(), 0.0, 0.0, 0.0).is_err()),
        ("J2 kinematic -1", Material::j2(iso(), 2.5e8, 0.0, -1.0).is_err()),
        (
            "neo-Hookean kappa 0",
            HyperelasticEnergy::neo_hookean(1e6, 0.0).is_err(),
        ),
        (
            "Mooney-Rivlin c10 = c01 = 0",
            HyperelasticEnergy::mooney_rivlin(0.0, 0.0, 1e9).is_err(),
        ),
        ("ply thickness 0", Ply::new(iso(), 0.0, 0.0).is_err()),
        ("empty layup", BlockProperty::layup(Vec::new()).is_err()),
    ];
    for (name, ok) in refused {
        c.holds(&format!("M2 {name} refused"), ok, "");
    }
    // M3: state sizes and the layout.
    let sizes: Vec<(String, usize, usize)> = std::iter::once(("IsotropicElastic".to_string(), iso().state_size(), 0))
        .chain(pending_variants().into_iter().map(|(m, name, _)| {
            let expect = match name {
                "Viscoelastic" => 18,
                "Creep" => 7,
                "J2" => 13,
                _ => 0,
            };
            (name.to_string(), m.state_size(), expect)
        }))
        .collect();
    for (name, got, expect) in sizes {
        c.holds(
            &format!("M3 state size {name}"),
            got == expect,
            &format!("{got} (registered {expect})"),
        );
    }
    let cm = cantilever_with(iso());
    let pm = {
        let mesh = upgrade_to_tet10(&tet_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [2, 2, 2]).unwrap())
            .unwrap()
            .0;
        let mut m = SolidModel::new(mesh);
        m.add_material("all", iso()).unwrap();
        m
    };
    for (name, model, points) in [("C", &cm, 8), ("P-Tet10", &pm, 4)] {
        let layout = StateLayout::of(model).unwrap();
        let n = model.mesh().n_elements();
        let ok = (0..n).all(|e| layout.points(e) == points && layout.values_per_point(e) == 0)
            && layout.total() == 0
            && layout.empty_states() == initial_states(model).unwrap();
        c.holds(&format!("M3 layout {name}"), ok, &format!("{n} elements"));
    }
    // M4: the constitutive call of each pending variant.
    for (m, name, package) in pending_variants() {
        let r1 = m.elasticity_matrix();
        let state = vec![0.0; m.state_size()];
        let r2 = m.stress(&[1e-4; 6], &PointContext::default(), &state);
        c.holds(
            &format!("M4 {name}"),
            not_yet(&r1, "material", name, "the constitutive call", package)
                && not_yet(&r2, "material", name, "the constitutive call", package),
            &format!("{:?} / {:?}", r1.err(), r2.err()),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- P1, P2

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
}

#[test]
fn p1_p2_point_context() {
    let mut c = Checks::new();
    let d = PointContext::default();
    c.holds(
        "P1 default",
        d.temperature.is_none() && d.time.is_none() && d.time_increment.is_none() && d.axes.is_none() && d.is_default(),
        &format!("{d:?}"),
    );
    let ctx = PointContext::new()
        .with_temperature(350.0)
        .unwrap()
        .with_time(1.0, 0.1)
        .unwrap()
        .with_axes(rot([1.0, 2.0, 3.0], 0.3))
        .unwrap();
    let mat = Material::isotropic_elastic(E_MOD, 0.3, RHO, 0.0).unwrap();
    let mut g = Lcg(0xF408_0440);
    let mut all = true;
    for _ in 0..64 {
        let mut eps = [0.0; 6];
        for v in &mut eps {
            *v = (g.next() - 0.5) * 1e-3;
        }
        let (s0, t0) = mat.stress(&eps, &d, &[]).unwrap();
        let (s1, t1) = mat.stress(&eps, &ctx, &[]).unwrap();
        let flat = |t: &[[f64; 6]; 6]| t.iter().flatten().map(|x| x.to_bits()).collect::<Vec<_>>();
        all &= bits(&s0) == bits(&s1) && flat(&t0) == flat(&t1);
        all &= flat(&t0) == flat(&isotropic_elasticity(E_MOD, 0.3));
    }
    c.holds("P1 full context = default, isotropic", all, "64 strains");
    let r3 = rot([1.0, 2.0, 3.0], 0.3);
    let mut scaled = r3;
    for row in &mut scaled {
        for v in row {
            *v *= 1.001;
        }
    }
    let mut negated = r3;
    negated[1] = [-negated[1][0], -negated[1][1], -negated[1][2]];
    let cases = [
        (
            "temperature NaN",
            PointContext::new().with_temperature(f64::NAN).is_err(),
        ),
        ("increment -1", PointContext::new().with_time(1.0, -1.0).is_err()),
        (
            "time infinite",
            PointContext::new().with_time(f64::INFINITY, 0.1).is_err(),
        ),
        ("axes scaled by 1.001", PointContext::new().with_axes(scaled).is_err()),
        (
            "axes with a row negated",
            PointContext::new().with_axes(negated).is_err(),
        ),
    ];
    for (name, ok) in cases {
        c.holds(&format!("P2 {name} refused"), ok, "");
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- K1

#[test]
#[ignore = "record of ADR 0440 before ADR 0454 (F-419) landed Shell3, retiring its pending row; continued in k1_shell3_kind_after_f419 (ADR 0291 pattern, maintainer standing decision)"]
fn k1_shell3_kind() {
    let mut c = Checks::new();
    let k = ElementKind::Shell3;
    c.holds(
        "K1 data",
        k.name() == "Shell3"
            && k.nodes_per_element() == 3
            && k.corners() == 3
            && !k.is_solid()
            && k.pending() == Some("ADR 0322 (F-419)")
            && k.edges() == [[0, 1], [1, 2], [2, 0]],
        &format!(
            "{} {} {} {:?} {:?}",
            k.name(),
            k.nodes_per_element(),
            k.corners(),
            k.pending(),
            k.edges()
        ),
    );
    let coords = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let r = element_for(k).stiffness(&coords, &isotropic_elasticity(E_MOD, 0.3));
    let ok = matches!(&r, Err(e) if e.to_string().contains("Shell3"));
    c.holds("K1 placeholder refuses naming Shell3", ok, &format!("{:?}", r.err()));
    let mut mesh = SolidMesh::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]).unwrap();
    let built = mesh.add_block(k, vec![0, 1, 2, 0, 2, 3]).is_ok();
    let shell = mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).is_ok();
    let layup = mesh
        .set_block_property(
            0,
            BlockProperty::layup(vec![Ply::new(iso(), 0.01, 0.0).unwrap()]).unwrap(),
        )
        .is_ok();
    let solid = mesh
        .set_block_property(0, BlockProperty::solid(rcfd_fem::SolidFormulation::Full))
        .is_err();
    c.holds(
        "K1 mesh",
        built && shell && layup && solid,
        &format!("built {built}, Shell {shell}, Layup {layup}, Solid refused {solid}"),
    );
    c.finish();
}

/// K1 after ADR 0454 (F-419): Shell3 has landed (no pending package); its placeholder still refuses naming Shell3.
#[test]
fn k1_shell3_kind_after_f419() {
    let mut c = Checks::new();
    let k = ElementKind::Shell3;
    c.holds(
        "K1 data",
        k.name() == "Shell3"
            && k.nodes_per_element() == 3
            && k.corners() == 3
            && !k.is_solid()
            && k.pending().is_none()
            && k.edges() == [[0, 1], [1, 2], [2, 0]],
        &format!(
            "{} {} {} {:?} {:?}",
            k.name(),
            k.nodes_per_element(),
            k.corners(),
            k.pending(),
            k.edges()
        ),
    );
    let coords = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let r = element_for(k).stiffness(&coords, &isotropic_elasticity(E_MOD, 0.3));
    let ok = matches!(&r, Err(e) if e.to_string().contains("Shell3"));
    c.holds("K1 placeholder refuses naming Shell3", ok, &format!("{:?}", r.err()));
    let mut mesh = SolidMesh::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]).unwrap();
    let built = mesh.add_block(k, vec![0, 1, 2, 0, 2, 3]).is_ok();
    let shell = mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).is_ok();
    let layup = mesh
        .set_block_property(
            0,
            BlockProperty::layup(vec![Ply::new(iso(), 0.01, 0.0).unwrap()]).unwrap(),
        )
        .is_ok();
    let solid = mesh
        .set_block_property(0, BlockProperty::solid(rcfd_fem::SolidFormulation::Full))
        .is_err();
    c.holds(
        "K1 mesh",
        built && shell && layup && solid,
        &format!("built {built}, Shell {shell}, Layup {layup}, Solid refused {solid}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1 to R4, N3

/// The entry points of R1 (E1 to E10); `shells` selects the rows of R2 and R3 (E1 to E6, E9, E10, E2 direct).
fn entries(
    model: &SolidModel,
    pres: &[Prescribed],
    prestress: Option<&Prestress>,
    shells: bool,
) -> Vec<(&'static str, rcfd_fem::Result<()>)> {
    let e2 = if shells {
        StaticOptions::default().threads(8)
    } else {
        StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::BlockJacobi,
            tolerance: 1e-8,
            max_iterations: 20000,
        })
        .threads(8)
    };
    let mut out: Vec<(&'static str, rcfd_fem::Result<()>)> = vec![
        (
            "E1 solve_static",
            if pres.is_empty() {
                solve_static(model, &StaticOptions::default()).map(|_| ())
            } else {
                solve_static_with(model, pres, &[], &StaticOptions::default()).map(|_| ())
            },
        ),
        (
            "E2 solve_static_with",
            solve_static_with(model, pres, &[], &e2).map(|_| ()),
        ),
        (
            "E3 modal_analysis",
            modal_analysis_with(model, pres, &ModalOptions::new(2)).map(|_| ()),
        ),
        ("E4 stiffness", stiffness(model).map(|_| ())),
        (
            "E5 nonlinear::solve",
            solve(model, pres, &[], &NewtonOptions::new(1)).map(|_| ()),
        ),
        (
            "E6 Integrator::new",
            Integrator::new(model, &[], &TransientOptions::new(1e-4)).map(|_| ()),
        ),
    ];
    if let Some(p) = prestress {
        out.push((
            "E7 buckling::factors",
            factors(model, p, &BucklingOptions::new(1)).map(|_| ()),
        ));
        out.push((
            "E8 prestressed_modes",
            prestressed_modes(model, p, &ModalOptions::new(2)).map(|_| ()),
        ));
    }
    out.push((
        "E9 solve_static_mpc",
        solve_static_mpc(model, pres, &[], &[], &StaticOptions::default()).map(|_| ()),
    ));
    out.push((
        "E10 modal_analysis_mpc",
        modal_analysis_mpc(model, pres, &ModalOptions::new(2)).map(|_| ()),
    ));
    out
}

#[test]
#[ignore = "record of the registered run of ADR 0440 (commit 144c2d4a): N3 and R1 (60 of 60) met; stopped before R2 to R4 (the Shell3 model was given a shell pressure, refused by add_load); corrected by amendment 2 as r2b_to_r4b_refusals"]
fn r1_to_r4_n3_refusals() {
    let mut c = Checks::new();
    // N3 for C, and its prestress for E7, E8.
    let twin = cantilever_with(iso());
    let prestress = Prestress::uniform(&twin, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    for (entry, r) in entries(&twin, &[], Some(&prestress), false) {
        c.holds(&format!("N3 C {entry}"), r.is_ok(), &format!("{:?}", r.err()));
    }
    // R1.
    for (m, name, package) in pending_variants() {
        let model = cantilever_with(m);
        let mut n = 0;
        for (entry, r) in entries(&model, &[], Some(&prestress), false) {
            n += 1;
            c.holds(
                &format!("R1 {name} {entry}"),
                not_yet(&r, "material", name, "material 0, element set 'all'", package),
                &format!("{:?}", r.err()),
            );
        }
        assert_eq!(n, 10);
    }
    // N3 for the Shell4 twin (amendment 1).
    let (plate, pres) = shell_model(ElementKind::Shell4, BlockProperty::shell(0.01).unwrap());
    for (entry, r) in entries(&plate, &pres, None, true) {
        let takes_shells = !(entry.starts_with("E5") || entry.starts_with("E6") || entry.starts_with("E2"));
        if takes_shells {
            c.holds(&format!("N3 plate {entry}"), r.is_ok(), &format!("{:?}", r.err()));
        } else if !entry.starts_with("E2") {
            let ok = matches!(&r, Err(e) if { let s = e.to_string(); s.contains("Shell4") && !s.contains("Shell3") && !s.contains("Layup") });
            c.holds(
                &format!("N3 plate {entry} refuses Shell4"),
                ok,
                &format!("{:?}", r.err()),
            );
        } else {
            println!("N3 plate {entry}: {:?} (recorded)", r.as_ref().err());
        }
    }
    // R2.
    let (tri, tpres) = shell_model(ElementKind::Shell3, BlockProperty::shell(0.01).unwrap());
    for (entry, r) in entries(&tri, &tpres, None, true) {
        c.holds(
            &format!("R2 Shell3 {entry}"),
            not_yet(&r, "element kind", "Shell3", "block 0", "ADR 0322 (F-419)"),
            &format!("{:?}", r.err()),
        );
    }
    // R3.
    let plies = vec![
        Ply::new(iso(), 0.005, 0.0).unwrap(),
        Ply::new(iso(), 0.005, PI / 2.0).unwrap(),
    ];
    let layup = BlockProperty::layup(plies).unwrap();
    let (lam, lpres) = shell_model(ElementKind::Shell4, layup.clone());
    for (entry, r) in entries(&lam, &lpres, None, true) {
        c.holds(
            &format!("R3 Layup {entry}"),
            not_yet(&r, "shell property", "Layup", "block 0", "ADR 0331 (F-428)"),
            &format!("{:?}", r.err()),
        );
    }
    // R4.
    let r4a = section_of(&layup, &iso());
    let plies = match &layup {
        BlockProperty::Layup { plies, .. } => plies.clone(),
        _ => unreachable!(),
    };
    let r4b = rcfd_fem::laminate::section_stiffness(&plies);
    c.holds(
        "R4 section_of(Layup), laminate::section_stiffness",
        not_yet(
            &r4a,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ) && not_yet(
            &r4b,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ),
        &format!("{:?} / {:?}", r4a.err(), r4b.err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- H1 to H3

fn kbits(k: &ElementMatrix) -> Vec<u64> {
    bits(k.values())
}

#[test]
fn h1_to_h3_section_hook() {
    let mut c = Checks::new();
    let mat = iso();
    let t = 0.01;
    let hs = homogeneous_section(&mat, t);
    let via = section_of(&BlockProperty::shell(t).unwrap(), &mat).unwrap();
    c.holds("H1 section_of(Shell) = homogeneous_section", via == hs, "");
    let flat = [
        [0.0, 0.0, 0.0],
        [0.0625, 0.0, 0.0],
        [0.0625, 0.0625, 0.0],
        [0.0, 0.0625, 0.0],
    ];
    let warped = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.01], [1.0, 1.0, 0.0], [0.0, 1.0, 0.01]];
    for (name, coords) in [("plate element 0", flat), ("warped facet", warped)] {
        let a = element_stiffness(&coords, t, &mat).unwrap();
        let b = element_stiffness_section(&coords, &hs, &ShellOptions::default()).unwrap();
        c.holds(&format!("H1 {name}"), kbits(&a) == kbits(&b), "bit-equal");
    }
    // H2: the coupling path on the unit square.
    let square = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    let bmat = [[1.0, 2.0, 3.0], [2.0, 5.0, 4.0], [3.0, 4.0, 6.0]].map(|r| r.map(|v| v * 1e5));
    let coupled = SectionStiffness::new(hs.membrane, Some(bmat), hs.bending, hs.shear, hs.drilling);
    let k = element_stiffness_section(&square, &coupled, &ShellOptions::default()).unwrap();
    let kv = k.values();
    let quad = |x: &[f64], y: &[f64]| -> f64 {
        let mut s = 0.0;
        for i in 0..24 {
            for j in 0..24 {
                s += x[i] * kv[i * 24 + j] * y[j];
            }
        }
        s
    };
    let (eps, kappa) = (1e-3, 2e-3);
    let mut ua = vec![0.0; 24];
    let mut ua2 = vec![0.0; 24];
    let mut ub = vec![0.0; 24];
    for (i, x) in square.iter().enumerate() {
        ua[6 * i] = eps * x[0];
        ua2[6 * i + 1] = eps * x[1];
        ub[6 * i + 4] = kappa * x[0];
    }
    for (name, u, expect) in [
        ("B_00", &ua, eps * kappa * bmat[0][0]),
        ("B_10", &ua2, eps * kappa * bmat[1][0]),
    ] {
        let got = quad(u, &ub);
        c.at_most(
            &format!("H2 u_A^T K u_B against area eps kappa {name}"),
            (got / expect - 1.0).abs(),
            1e-12,
        );
    }
    // H3: the matrix shear path with H = ds I.
    let ds = match hs.shear {
        ShearStiffness::Isotropic(v) => v,
        _ => unreachable!(),
    };
    let m = SectionStiffness::new(
        hs.membrane,
        None,
        hs.bending,
        ShearStiffness::Matrix([[ds, 0.0], [0.0, ds]]),
        hs.drilling,
    );
    let k1 = element_stiffness_section(&warped, &m, &ShellOptions::default()).unwrap();
    let k2 = element_stiffness_section(&warped, &hs, &ShellOptions::default()).unwrap();
    let max2 = k2.values().iter().map(|v| v.abs()).fold(0.0, f64::max);
    let diff = k1
        .values()
        .iter()
        .zip(k2.values())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    c.at_most("H3 matrix shear = isotropic shear", diff / max2, 1e-14);
    let _ = shell::DRILLING_FACTOR;
    c.finish();
}

// ------------------------------------------------------------------------------------------------- X1

#[test]
#[ignore = "record of ADR 0440: ADR 0448 (F-412) landed harmonic_direct and cskyline and ADR 0447 (F-431) landed sparse_chol (PENDING None), retiring their rows; continued in x1_modules_and_pending_after_landed (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "ADR 0312 (F-409)"),
        ("mat_table", f::mat_table::PENDING, "ADR 0312 (F-409)"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "ADR 0315 (F-412)"),
        ("fatigue", f::fatigue::PENDING, "ADR 0316 (F-413)"),
        ("damped", f::damped::PENDING, "ADR 0317 (F-414)"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "ADR 0322 (F-419)"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "ADR 0329 (F-426)"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "ADR 0315 (F-412)"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "ADR 0317 (F-414)"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "ADR 0334 (F-431)"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        c.holds(&format!("X1 {name}"), *got == Some(*expect), &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// X1 after ADR 0448 (F-412) and ADR 0447 (F-431): harmonic_direct, cskyline and sparse_chol have landed.
#[test]
#[ignore = "record of ADR 0448 and ADR 0447: ADR 0450 (F-414) landed damped and eigen_general and ADR 0449 (F-413) landed fatigue (PENDING None), retiring their rows; continued in x1_modules_and_pending_after_f413_f414 (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending_after_landed() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "ADR 0312 (F-409)"),
        ("mat_table", f::mat_table::PENDING, "ADR 0312 (F-409)"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "ADR 0316 (F-413)"),
        ("damped", f::damped::PENDING, "ADR 0317 (F-414)"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "ADR 0322 (F-419)"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "ADR 0329 (F-426)"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "ADR 0317 (F-414)"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// X1 after ADR 0450 (F-414) and ADR 0449 (F-413): damped, eigen_general and fatigue have landed as well.
#[test]
#[ignore = "record of ADR 0450 and ADR 0449: ADR 0452 (F-409) landed mat_ortho and mat_table (PENDING None), retiring their rows; continued in x1_modules_and_pending_after_f409 (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending_after_f413_f414() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "ADR 0312 (F-409)"),
        ("mat_table", f::mat_table::PENDING, "ADR 0312 (F-409)"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "landed"),
        ("damped", f::damped::PENDING, "landed"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "ADR 0322 (F-419)"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "ADR 0329 (F-426)"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "landed"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447, ADR 0450, ADR 0449: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// X1 after ADR 0452 (F-409) and ADR 0454 (F-419): mat_ortho, mat_table and shell3 have landed as well.
#[test]
#[ignore = "record of ADR 0454: ADR 0456 (F-426) landed tie (PENDING None), retiring its row; continued in x1_modules_and_pending_after_f419_f426 (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending_after_f419() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "landed"),
        ("mat_table", f::mat_table::PENDING, "landed"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "landed"),
        ("damped", f::damped::PENDING, "landed"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "landed"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "ADR 0329 (F-426)"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "landed"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447, ADR 0450, ADR 0449, ADR 0452, ADR 0454: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

#[test]
fn extra_contribution_called_once_per_iterate() {
    // The two traits are usable as trait objects (compile-time check of the extension points).
    let cell = Cell::new(0);
    struct Count<'a>(&'a Cell<usize>);
    impl Contribution for Count<'_> {
        fn name(&self) -> &str {
            "count"
        }
        fn add(&self, _u: &[f64], _l: f64, _f: &mut [f64], _t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }
    let count = Count(&cell);
    let list: Vec<&dyn Contribution> = vec![&count];
    let bar = stretch_bar(E_MOD);
    let r = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: list,
            step_control: None,
        },
    )
    .unwrap();
    let iterations: usize = r.steps.iter().map(|s| s.residuals.len()).sum();
    // Called once per evaluated iterate and once for the reactions.
    assert_eq!(cell.get(), iterations + 1, "calls of the contribution");
}

// ------------------------------------------------------------------------------------------------- amendment 2

/// A contribution adding zeros at `(a, b)` and `(b, a)`.
struct ZerosAt {
    a: usize,
    b: usize,
}

impl Contribution for ZerosAt {
    fn name(&self) -> &str {
        "zeros off the pattern"
    }
    fn add(&self, _u: &[f64], _l: f64, _f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        t.push((self.a, self.b, 0.0));
        t.push((self.b, self.a, 0.0));
        Ok(())
    }
}

/// D6b (amendment 2): the smallest free equation `i` and the smallest free `j` not in row `i` of the pattern.
#[test]
#[ignore = "record of the run of ADR 0440 D6b (commit 0907226f): set-up stopped, the smallest free equation has a full pattern row (no j); corrected by amendment 3 as d6c_off_pattern_contribution"]
fn d6b_off_pattern_contribution() {
    let mut c = Checks::new();
    let bar = stretch_bar(E_MOD);
    let reference = solve(&bar, &[], &[], &NewtonOptions::new(1)).unwrap();
    let k0 = stiffness(&bar).unwrap();
    let dofs = bar.dof_map();
    let mut fixed = vec![false; dofs.n_equations()];
    for (set, comp) in [("xmin", 0), ("ymin", 1), ("zmin", 2)] {
        for &n in bar.mesh().node_set(set).unwrap() {
            fixed[dofs.equation(n, comp).unwrap()] = true;
        }
    }
    let i = (0..fixed.len()).find(|&q| !fixed[q]).unwrap();
    let row = &k0.col[k0.row_ptr[i]..k0.row_ptr[i + 1]];
    let j = (0..fixed.len()).find(|&q| !fixed[q] && !row.contains(&q)).unwrap();
    let off = !row.contains(&j);
    c.holds(
        "D6b the pair is off the pattern",
        off,
        &format!("equations {i} and {j}"),
    );
    let z = ZerosAt { a: i, b: j };
    let r = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: vec![&z],
            step_control: None,
        },
    )
    .unwrap();
    let (a, b) = (r.result.displacement_values(), reference.result.displacement_values());
    let dev =
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    println!("D6b: bit-equal {}", bits(a) == bits(b));
    c.at_most("D6b off-pattern zeros: max |u - u_ref| / max |u_ref|", dev, 1e-10);
    c.finish();
}

/// The Shell3 model of R2 without a load (amendment 2).
fn shell3_model_unloaded() -> (SolidModel, Vec<Prescribed>) {
    let (nodes, quads, pres) = plate_grid();
    let conn: Vec<usize> = quads
        .iter()
        .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
        .collect();
    let nn = nodes.len();
    let ne = conn.len() / 3;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    (m, pres)
}

/// R2b to R4b (amendment 2).
#[test]
#[ignore = "record of ADR 0440 amendment 2 before ADR 0454 (F-419) landed Shell3, retiring the R2b rows; continued in r2b_to_r4b_refusals_after_f419 (ADR 0291 pattern, maintainer standing decision)"]
fn r2b_to_r4b_refusals() {
    let mut c = Checks::new();
    let (tri, tpres) = shell3_model_unloaded();
    for (entry, r) in entries(&tri, &tpres, None, true) {
        c.holds(
            &format!("R2b Shell3 {entry}"),
            not_yet(&r, "element kind", "Shell3", "block 0", "ADR 0322 (F-419)"),
            &format!("{:?}", r.err()),
        );
    }
    let plies = vec![
        Ply::new(iso(), 0.005, 0.0).unwrap(),
        Ply::new(iso(), 0.005, PI / 2.0).unwrap(),
    ];
    let layup = BlockProperty::layup(plies.clone()).unwrap();
    let (lam, lpres) = shell_model(ElementKind::Shell4, layup.clone());
    for (entry, r) in entries(&lam, &lpres, None, true) {
        c.holds(
            &format!("R3b Layup {entry}"),
            not_yet(&r, "shell property", "Layup", "block 0", "ADR 0331 (F-428)"),
            &format!("{:?}", r.err()),
        );
    }
    let r4a = section_of(&layup, &iso());
    let r4b = rcfd_fem::laminate::section_stiffness(&plies);
    c.holds(
        "R4b section_of(Layup), laminate::section_stiffness",
        not_yet(
            &r4a,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ) && not_yet(
            &r4b,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ),
        &format!("{:?} / {:?}", r4a.err(), r4b.err()),
    );
    c.finish();
}

/// R2b to R4b after ADR 0454 (F-419): the R2b rows state the paths that take Shell3 (R2c); R3b and R4b unchanged.
#[test]
fn r2b_to_r4b_refusals_after_f419() {
    let mut c = Checks::new();
    let (tri, tpres) = shell3_model_unloaded();
    // ADR 0454 D3: Shell3 is taken where Shell4 is; E5 and E6 refuse it naming its follow-up.
    for (entry, r) in entries(&tri, &tpres, None, true) {
        let refused = entry.starts_with("E5") || entry.starts_with("E6");
        let ok = if refused {
            not_yet(&r, "element kind", "Shell3", "block 0", rcfd_fem::shell3::FOLLOW_UP)
        } else {
            !matches!(&r, Err(FemError::NotYetSupported { .. }))
        };
        c.holds(&format!("R2c Shell3 {entry}"), ok, &format!("{:?}", r.err()));
    }
    let plies = vec![
        Ply::new(iso(), 0.005, 0.0).unwrap(),
        Ply::new(iso(), 0.005, PI / 2.0).unwrap(),
    ];
    let layup = BlockProperty::layup(plies.clone()).unwrap();
    let (lam, lpres) = shell_model(ElementKind::Shell4, layup.clone());
    for (entry, r) in entries(&lam, &lpres, None, true) {
        c.holds(
            &format!("R3b Layup {entry}"),
            not_yet(&r, "shell property", "Layup", "block 0", "ADR 0331 (F-428)"),
            &format!("{:?}", r.err()),
        );
    }
    let r4a = section_of(&layup, &iso());
    let r4b = rcfd_fem::laminate::section_stiffness(&plies);
    c.holds(
        "R4b section_of(Layup), laminate::section_stiffness",
        not_yet(
            &r4a,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ) && not_yet(
            &r4b,
            "shell property",
            "Layup",
            "the section stiffness",
            "ADR 0331 (F-428)",
        ),
        &format!("{:?} / {:?}", r4a.err(), r4b.err()),
    );
    c.finish();
}

/// D6c (amendment 3): `i` the smallest free equation whose pattern row does not hold every free equation, `j` the smallest
/// free equation not in that row.
#[test]
fn d6c_off_pattern_contribution() {
    let mut c = Checks::new();
    let bar = stretch_bar(E_MOD);
    let reference = solve(&bar, &[], &[], &NewtonOptions::new(1)).unwrap();
    let k0 = stiffness(&bar).unwrap();
    let dofs = bar.dof_map();
    let mut fixed = vec![false; dofs.n_equations()];
    for (set, comp) in [("xmin", 0), ("ymin", 1), ("zmin", 2)] {
        for &n in bar.mesh().node_set(set).unwrap() {
            fixed[dofs.equation(n, comp).unwrap()] = true;
        }
    }
    let row = |i: usize| &k0.col[k0.row_ptr[i]..k0.row_ptr[i + 1]];
    let free: Vec<usize> = (0..fixed.len()).filter(|&q| !fixed[q]).collect();
    let i = *free
        .iter()
        .find(|&&q| free.iter().any(|f| !row(q).contains(f)))
        .unwrap();
    let j = *free.iter().find(|&&q| !row(i).contains(&q)).unwrap();
    c.holds(
        "D6c the pair is off the pattern",
        !row(i).contains(&j),
        &format!("equations {i} and {j}"),
    );
    let z = ZerosAt { a: i, b: j };
    let r = solve_with(
        &bar,
        &[],
        &[],
        &NewtonOptions::new(1),
        DriverExtensions {
            contributions: vec![&z],
            step_control: None,
        },
    )
    .unwrap();
    let (a, b) = (r.result.displacement_values(), reference.result.displacement_values());
    let dev =
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    println!("D6c: bit-equal {}", bits(a) == bits(b));
    c.at_most("D6c off-pattern zeros: max |u - u_ref| / max |u_ref|", dev, 1e-10);
    c.finish();
}

// X1 after ADR 0452 (F-409): mat_ortho and mat_table have landed as well.
#[test]
#[ignore = "record of ADR 0452: ADR 0454 (F-419) landed shell3 and ADR 0456 (F-426) landed tie (PENDING None), retiring their rows; continued in x1_modules_and_pending_after_f419 (on main) and now x1_modules_and_pending_after_f419_f426 (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending_after_f409() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "landed"),
        ("mat_table", f::mat_table::PENDING, "landed"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "landed"),
        ("damped", f::damped::PENDING, "landed"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "ADR 0322 (F-419)"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "ADR 0329 (F-426)"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "landed"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447, ADR 0450, ADR 0449, ADR 0452: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// X1 after ADR 0456 (F-426): tie has landed as well.
#[test]
#[ignore = "record of ADR 0456: ADR 0469 (F-430) landed cms (PENDING None), retiring its row; continued in x1_modules_and_pending_after_f430 (ADR 0291 pattern, maintainer standing decision)"]
fn x1_modules_and_pending_after_f419_f426() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "landed"),
        ("mat_table", f::mat_table::PENDING, "landed"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "landed"),
        ("damped", f::damped::PENDING, "landed"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "landed"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "landed"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "ADR 0333 (F-430)"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "landed"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447, ADR 0450, ADR 0449, ADR 0452, ADR 0456: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// X1 after ADR 0469 (F-430): cms has landed as well.
#[test]
fn x1_modules_and_pending_after_f430() {
    use rcfd_fem as f;
    let fem: [(&str, Option<&str>, &str); 24] = [
        ("mat_ortho", f::mat_ortho::PENDING, "landed"),
        ("mat_table", f::mat_table::PENDING, "landed"),
        ("mat_visco", f::mat_visco::PENDING, "ADR 0313 (F-410)"),
        ("mat_creep", f::mat_creep::PENDING, "ADR 0314 (F-411)"),
        ("harmonic_direct", f::harmonic_direct::PENDING, "landed"),
        ("fatigue", f::fatigue::PENDING, "landed"),
        ("damped", f::damped::PENDING, "landed"),
        ("gyro", f::gyro::PENDING, "ADR 0318 (F-415)"),
        ("rotor", f::rotor::PENDING, "ADR 0319 (F-416)"),
        ("random", f::random::PENDING, "ADR 0320 (F-417)"),
        ("shell3", f::shell3::PENDING, "landed"),
        ("geometric_struct", f::geometric_struct::PENDING, "ADR 0323 (F-420)"),
        ("mat_j2", f::mat_j2::PENDING, "ADR 0325 (F-422)"),
        ("nl_transient", f::nl_transient::PENDING, "ADR 0326 (F-423)"),
        ("follower", f::follower::PENDING, "ADR 0327 (F-424)"),
        ("arclength", f::arclength::PENDING, "ADR 0327 (F-424)"),
        ("mat_hyper", f::mat_hyper::PENDING, "ADR 0328 (F-425)"),
        ("nonlinear_md", f::nonlinear_md::PENDING, "ADR 0328 (F-425)"),
        ("tie", f::tie::PENDING, "landed"),
        ("contact", f::contact::PENDING, "ADR 0330 (F-427)"),
        ("laminate", f::laminate::PENDING, "ADR 0331 (F-428)"),
        ("fracture", f::fracture::PENDING, "ADR 0332 (F-429)"),
        ("cms", f::cms::PENDING, "landed"),
        ("estimate", f::estimate::PENDING, "ADR 0335 (F-432)"),
    ];
    let linalg: [(&str, Option<&str>, &str); 3] = [
        ("cskyline", rcfd_linalg::cskyline::PENDING, "landed"),
        ("eigen_general", rcfd_linalg::eigen_general::PENDING, "landed"),
        ("sparse_chol", rcfd_linalg::sparse_chol::PENDING, "landed"),
    ];
    let mut c = Checks::new();
    for (name, got, expect) in fem.iter().chain(&linalg) {
        // ADR 0448, ADR 0447, ADR 0450, ADR 0449, ADR 0452, ADR 0456, ADR 0469: a landed module has PENDING None
        let want = if *expect == "landed" { None } else { Some(*expect) };
        c.holds(&format!("X1 {name}"), *got == want, &format!("{got:?}"));
    }
    let _ = rcfd_linalg::complex::C64::ONE;
    c.finish();
}

// M1 to M4 after ADR 0452 (F-409): Orthotropic and Tabulated have landed (ADR 0452 D9).
#[test]
fn m1_to_m4_after_f409() {
    let mut c = Checks::new();
    // M1.
    for (m, name, package) in pending_variants() {
        // ADR 0452: Orthotropic and Tabulated have landed.
        let want = if landed_f409(name) { None } else { Some(package) };
        c.holds(
            &format!("M1 {name}"),
            m.name() == name && m.pending() == want,
            &format!("{} {:?}", m.name(), m.pending()),
        );
    }
    c.holds(
        "M1 IsotropicElastic",
        iso().name() == "IsotropicElastic" && iso().pending().is_none(),
        "",
    );
    // M2: refused data.
    let r3 = rot([1.0, 2.0, 3.0], 0.3);
    let mut scaled = r3;
    scaled[0][0] *= 1.001;
    let mut left = r3;
    left[2] = [-left[2][0], -left[2][1], -left[2][2]];
    let fixed = || MaterialAxes::fixed(r3).unwrap();
    let refused: Vec<(&str, bool)> = vec![
        (
            "orthotropic E_2 = 0",
            Material::orthotropic([1e11, 0.0, 1e10], [0.3; 3], [5e9; 3], 1600.0, [0.0; 3], fixed()).is_err(),
        ),
        (
            "orthotropic nu NaN",
            Material::orthotropic(
                [1e11, 1e10, 1e10],
                [0.3, f64::NAN, 0.3],
                [5e9; 3],
                1600.0,
                [0.0; 3],
                fixed(),
            )
            .is_err(),
        ),
        ("axes scaled", MaterialAxes::fixed(scaled).is_err()),
        ("axes left-handed", MaterialAxes::fixed(left).is_err()),
        (
            "cylindrical zero axis",
            MaterialAxes::cylindrical([0.0; 3], [0.0; 3]).is_err(),
        ),
        ("per-element empty", MaterialAxes::per_element(Vec::new()).is_err()),
        (
            "tabulated one temperature",
            Material::tabulated(iso(), vec![293.0], vec![("e".into(), vec![2e11])]).is_err(),
        ),
        (
            "tabulated decreasing",
            Material::tabulated(iso(), vec![373.0, 293.0], vec![("e".into(), vec![2e11, 1.9e11])]).is_err(),
        ),
        (
            "tabulated short column",
            Material::tabulated(iso(), vec![293.0, 373.0], vec![("e".into(), vec![2e11])]).is_err(),
        ),
        (
            "tabulated J2 base",
            Material::tabulated(
                Material::j2(iso(), 2.5e8, 0.0, 0.0).unwrap(),
                vec![293.0, 373.0],
                vec![("e".into(), vec![2e11, 1.9e11])],
            )
            .is_err(),
        ),
        (
            "Prony weights sum 1.5",
            Material::viscoelastic(
                iso(),
                vec![
                    PronyTerm::new(0.8, 0.0, 1.0).unwrap(),
                    PronyTerm::new(0.7, 0.0, 2.0).unwrap(),
                ],
                None,
            )
            .is_err(),
        ),
        ("Prony tau 0", PronyTerm::new(0.1, 0.1, 0.0).is_err()),
        ("creep n = 0", Material::creep(iso(), 1e-30, 0.0, 0.0).is_err()),
        ("creep m = -1", Material::creep(iso(), 1e-30, 4.0, -1.0).is_err()),
        ("J2 orthotropic base", Material::j2(ortho(), 2.5e8, 0.0, 0.0).is_err()),
        ("J2 yield 0", Material::j2(iso(), 0.0, 0.0, 0.0).is_err()),
        ("J2 kinematic -1", Material::j2(iso(), 2.5e8, 0.0, -1.0).is_err()),
        (
            "neo-Hookean kappa 0",
            HyperelasticEnergy::neo_hookean(1e6, 0.0).is_err(),
        ),
        (
            "Mooney-Rivlin c10 = c01 = 0",
            HyperelasticEnergy::mooney_rivlin(0.0, 0.0, 1e9).is_err(),
        ),
        ("ply thickness 0", Ply::new(iso(), 0.0, 0.0).is_err()),
        ("empty layup", BlockProperty::layup(Vec::new()).is_err()),
    ];
    for (name, ok) in refused {
        c.holds(&format!("M2 {name} refused"), ok, "");
    }
    // M3: state sizes and the layout.
    let sizes: Vec<(String, usize, usize)> = std::iter::once(("IsotropicElastic".to_string(), iso().state_size(), 0))
        .chain(pending_variants().into_iter().map(|(m, name, _)| {
            let expect = match name {
                "Viscoelastic" => 18,
                "Creep" => 7,
                "J2" => 13,
                _ => 0,
            };
            (name.to_string(), m.state_size(), expect)
        }))
        .collect();
    for (name, got, expect) in sizes {
        c.holds(
            &format!("M3 state size {name}"),
            got == expect,
            &format!("{got} (registered {expect})"),
        );
    }
    let cm = cantilever_with(iso());
    let pm = {
        let mesh = upgrade_to_tet10(&tet_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [2, 2, 2]).unwrap())
            .unwrap()
            .0;
        let mut m = SolidModel::new(mesh);
        m.add_material("all", iso()).unwrap();
        m
    };
    for (name, model, points) in [("C", &cm, 8), ("P-Tet10", &pm, 4)] {
        let layout = StateLayout::of(model).unwrap();
        let n = model.mesh().n_elements();
        let ok = (0..n).all(|e| layout.points(e) == points && layout.values_per_point(e) == 0)
            && layout.total() == 0
            && layout.empty_states() == initial_states(model).unwrap();
        c.holds(&format!("M3 layout {name}"), ok, &format!("{n} elements"));
    }
    // M4: the constitutive call of each pending variant.
    for (m, name, package) in pending_variants().into_iter().filter(|(_, n, _)| !landed_f409(n)) {
        let r1 = m.elasticity_matrix();
        let state = vec![0.0; m.state_size()];
        let r2 = m.stress(&[1e-4; 6], &PointContext::default(), &state);
        c.holds(
            &format!("M4 {name}"),
            not_yet(&r1, "material", name, "the constitutive call", package)
                && not_yet(&r2, "material", name, "the constitutive call", package),
            &format!("{:?} / {:?}", r1.err(), r2.err()),
        );
    }
    // M4 for the variants of ADR 0452 (D9 of its registration).
    let eps = [1e-4; 6];
    let o = ortho();
    let r = rot([0.0, 0.0, 1.0], 0.5);
    let d = rcfd_fem::mat_ortho::rotate_elasticity(
        &rcfd_fem::mat_ortho::material_elasticity([1.4e11, 1.0e10, 1.0e10], [0.3, 0.3, 0.45], [5.0e9, 5.0e9, 3.5e9]),
        &r,
    );
    let em = o.elasticity_matrix();
    let st = o.stress(&eps, &PointContext::default(), &[]);
    let want = rcfd_fem::material::apply(&d, &eps);
    c.holds(
        "M4 Orthotropic (Fixed axes)",
        em.as_ref().is_ok_and(|m| bits(&m.concat()) == bits(&d.concat()))
            && st.as_ref().is_ok_and(|(s, _)| bits(s) == bits(&want)),
        &format!("{:?} / {:?}", em.as_ref().err(), st.as_ref().err()),
    );
    let t = Material::tabulated(iso(), vec![293.0, 373.0], vec![("e".to_string(), vec![2.0e11, 1.9e11])]).unwrap();
    let em = t.elasticity_matrix();
    let st0 = t.stress(&eps, &PointContext::default(), &[]);
    let st1 = t.stress(&eps, &PointContext::default().with_temperature(300.0).unwrap(), &[]);
    let e300 = 2.0e11 + (1.9e11 - 2.0e11) * ((300.0 - 293.0) / (373.0 - 293.0));
    let d300 = isotropic_elasticity(e300, iso().poisson_ratio());
    c.holds(
        "M4 Tabulated",
        not_yet(
            &em,
            "material",
            "Tabulated",
            "the element-constant elasticity matrix",
            rcfd_fem::mat_ortho::FOLLOW_UP,
        ) && matches!(&st0, Err(FemError::InvalidValue { key: "temperature", .. }))
            && st1
                .as_ref()
                .is_ok_and(|(_, m)| bits(&m.concat()) == bits(&d300.concat())),
        &format!(
            "{:?} / {:?} / {:?}",
            em.as_ref().err(),
            st0.as_ref().err(),
            st1.as_ref().err()
        ),
    );
    c.finish();
}

/// The variants landed by ADR 0452 (F-409).
fn landed_f409(name: &str) -> bool {
    name == "Orthotropic" || name == "Tabulated"
}
