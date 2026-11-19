//! F-666 (ADR 0746): hyperelastic materials on Beam2, Shell4 and Shell3 elements on the linear paths by default, that is
//! without `SolidModel::set_hyperelastic_structures`. Checks A-RF1, A-S, A-M, A-N1, A-CY, A-W, A-RO, A-CM, A-MP, X, E,
//! K-0744, K-0555, K-0584, N-T and C of the registration. The retired records restated here: `rf1_to_rf4_refusals` of
//! `sa_hyper_paths.rs` (ADR 0555), `n1_to_n4_settings_and_refusals` of `sa_hyper_structures.rs` (ADR 0584),
//! `n1_refused_without_the_setting` of `sa_hyper_structures_f666.rs` (ADR 0593) and
//! `ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path` of `sa_cyclic_beams_default_adr0744.rs` (ADR 0744). The builders,
//! the measures and the path rows are copies of those files; "by default" means the setter is never called on the model.
//! Every value is printed; a non-finite value, an error or a loss of stiffness is a miss.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::cms::{reduce, CmsOptions};
use rcfd_fem::cyclic::{modal_analysis_harmonic, solve_static_harmonic, HarmonicLoad, HarmonicModes};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_hyper::{self, LINEAR_FOLLOW_UP};
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc, ReferenceLoad};
use rcfd_fem::rotating::{HarmonicSpeedModes, RotatingOptions, RotatingPart, RotatingSector, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed; a non-finite value is a miss; the test fails at the end if
/// any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn miss(&mut self, name: &str, detail: &str) {
        println!("{name}: {detail} MISSED");
        self.0.push(format!("{name}: {detail}"));
    }
    /// `at_most` for an acceptance row, `above` for a control.
    fn row(&mut self, name: &str, value: f64, limit: f64, control: bool) {
        if control {
            self.above(name, value, limit);
        } else {
            self.at_most(name, value, limit);
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ----------------------------------------------------------------------------------------------- materials (ADR 0593 D2)

const RHO: f64 = 1000.0;
const LIMIT: f64 = 1e-10;
const CONTROL: f64 = 1e-8;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn nhc_perturbed() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6 * (1.0 + 1e-6), 1e7).unwrap()
}
fn hyper() -> Material {
    Material::hyperelastic(nhc(), RHO).unwrap()
}
/// The isotropic twin of an energy: `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)`, `s = 3K + G` (ADR 0584 D2).
fn twin_of(e: &HyperelasticEnergy) -> Material {
    let (g, k) = mat_hyper::small_strain_moduli(e);
    let s = 3.0 * k + g;
    Material::isotropic_elastic(9.0 * k * g / s, (3.0 * k - 2.0 * g) / (2.0 * s), RHO, 0.0).unwrap()
}
fn twin() -> Material {
    twin_of(&nhc())
}
fn twin_perturbed() -> Material {
    twin_of(&nhc_perturbed())
}

/// The material of a model and what is done with the setting of ADR 0584.
#[derive(Clone, Copy, PartialEq)]
enum Which {
    /// The hyperelastic material by default: the setter is never called (ADR 0746 D1).
    Hyper,
    /// The hyperelastic material with `set_hyperelastic_structures(true)` (E).
    HyperSet,
    /// The hyperelastic material with `set_hyperelastic_structures(true)`, then `(false)` (E).
    HyperSetUnset,
    /// The isotropic twin.
    Twin,
    /// The twin of the perturbed energy (C).
    Perturbed,
}

fn material(w: Which) -> Material {
    match w {
        Which::Hyper | Which::HyperSet | Which::HyperSetUnset => hyper(),
        Which::Twin => twin(),
        Which::Perturbed => twin_perturbed(),
    }
}

/// Calls the setter only for the two variants of E; for every other model it is never called.
fn setting(m: &mut SolidModel, w: Which) {
    match w {
        Which::HyperSet => m.set_hyperelastic_structures(true),
        Which::HyperSetUnset => {
            m.set_hyperelastic_structures(true);
            m.set_hyperelastic_structures(false);
        }
        Which::Hyper | Which::Twin | Which::Perturbed => {}
    }
}

fn modal_opts(n: usize) -> ModalOptions {
    ModalOptions::new(n).residual_test(ResidualTest::Backward)
}

// ----------------------------------------------------------------------------------------------- set-ups (ADR 0593 D3)

/// Beam B of ADR 0584 D3 with the node sets `x0`, `xl`, `tip` (8, 9, 10), `mid` (5); clamped at `x0`.
fn beam_b(w: Which, loads: bool) -> SolidModel {
    let n = 10;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let section = beam::rectangle_section(0.05, 0.03, 2.6e-7, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("tip", vec![8, 9, 10]).unwrap();
    mesh.add_node_set("mid", vec![5]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(w)).unwrap();
    setting(&mut m, w);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::nodal_force("xl", [0.1, 0.2, 0.3])).unwrap();
        m.add_load(Load::nodal_moment("xl", [0.01, 0.02, 0.03])).unwrap();
        m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    }
    m
}

const NX: usize = 10;
const NY: usize = 6;

fn pidx(i: usize, j: usize) -> usize {
    j * (NX + 1) + i
}

/// Plate P4 (`tri = false`) or P3 of ADR 0584 D3 with the node sets `xl` (x = 1), `c` (the corner).
fn plate(w: Which, tri: bool, loads: bool) -> SolidModel {
    let (lx, ly) = (1.0, 0.6);
    let mut nodes = Vec::new();
    for j in 0..=NY {
        for i in 0..=NX {
            nodes.push([lx * i as f64 / NX as f64, ly * j as f64 / NY as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..NY {
        for i in 0..NX {
            let (a, b, cc, d) = (pidx(i, j), pidx(i + 1, j), pidx(i + 1, j + 1), pidx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, cc, a, cc, d]);
            } else {
                conn.extend_from_slice(&[a, b, cc, d]);
            }
        }
    }
    let ne = if tri { 2 * NX * NY } else { NX * NY };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("x0", (0..=NY).map(|j| pidx(0, j)).collect()).unwrap();
    mesh.add_node_set("xl", (0..=NY).map(|j| pidx(NX, j)).collect())
        .unwrap();
    mesh.add_node_set("c", vec![pidx(NX, NY)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(w)).unwrap();
    setting(&mut m, w);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::nodal_force("c", [0.1, 0.2, 0.3])).unwrap();
        m.add_load(Load::nodal_moment("c", [0.01, 0.02, 0.005])).unwrap();
        m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    }
    m
}

/// The mpc constraints of ADR 0593 D3 on Beam B: the rigid set `tip` (constraint 6) and `u_y = u_z` at node 5.
fn beam_mpc(w: Which) -> SolidModel {
    let mut m = beam_b(w, true);
    m.add_constraint(Constraint::rigid_set("tip", [1.05, 0.0, 0.0]))
        .unwrap();
    m.add_constraint(Constraint::linear(
        vec![LinearTerm::new(5, 1, 1.0), LinearTerm::new(5, 2, -1.0)],
        0.0,
    ))
    .unwrap();
    m
}

/// The mpc constraints of ADR 0593 D3 on the plate: the rigid set `xl` (constraint 6) and `u_z(p1) = u_z(p0)`.
fn plate_mpc(w: Which, tri: bool) -> SolidModel {
    let mut m = plate(w, tri, true);
    m.add_constraint(Constraint::rigid_set("xl", [1.05, 0.3, 0.0])).unwrap();
    m.add_constraint(Constraint::linear(
        vec![
            LinearTerm::new(pidx(5, NY), 2, 1.0),
            LinearTerm::new(pidx(5, 0), 2, -1.0),
        ],
        0.0,
    ))
    .unwrap();
    m
}

// The frustum sector of `sa_cyclic_shells.rs` (ADR 0556 D3).

const N: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn point(i: usize, s: f64) -> Vec3 {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    let r = 1.0 - 0.4 * s;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn sector_node(i: usize, a: usize) -> usize {
    a * (NC + 1) + i
}

fn loaded() -> Vec<(usize, usize)> {
    (1..=NA).flat_map(|a| (0..NC).map(move |i| (i, a))).collect()
}

/// Sector Q4 (`kind` Shell4) or Q3 (Shell3) with the cyclic constraint and the shells option; `loads` adds the static loads
/// of ADR 0593 D3.
fn sector(w: Which, kind: ElementKind, loads: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..=NC {
            nodes.push(point(i, a as f64 / NA as f64));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            let q = [
                sector_node(i, a),
                sector_node(i + 1, a),
                sector_node(i + 1, a + 1),
                sector_node(i, a + 1),
            ];
            match kind {
                ElementKind::Shell4 => conn.extend(q),
                _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..=NC).map(|i| sector_node(i, 0)).collect())
        .unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| sector_node(0, a)).collect())
        .unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| sector_node(NC, a)).collect())
        .unwrap();
    for (i, a) in loaded() {
        mesh.add_node_set(&format!("p{}", sector_node(i, a)), vec![sector_node(i, a)])
            .unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(w)).unwrap();
    setting(&mut m, w);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    if loads {
        m.add_load(Load::centrifugal(axis(), ORIGIN, 10.0)).unwrap();
        m.add_load(Load::shell_pressure("all", 2e2)).unwrap();
        for (i, a) in loaded() {
            let q = sector_node(i, a) as f64;
            let f = [
                10.0 * (1.3 * q + 0.2).sin(),
                10.0 * (0.7 * q + 0.5).cos(),
                10.0 * (2.1 * q + 1.1).sin(),
            ];
            let mo = [(1.9 * q + 0.3).cos(), (0.4 * q + 0.9).sin(), (1.1 * q + 1.7).cos()];
            let set = format!("p{}", sector_node(i, a));
            m.add_load(Load::nodal_force(&set, f)).unwrap();
            m.add_load(Load::nodal_moment(&set, mo)).unwrap();
        }
    }
    m
}

// ----------------------------------------------------------------------------------------------- measures (ADR 0593 D4)

/// `max_i |a_i - b_i| / max_i |b_i|`, NaN if any value is not finite or the lengths differ.
fn rel(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() || a.iter().chain(b).any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    let d = a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    let s = b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    d / s
}

/// `max_k |a_k / b_k - 1|`, NaN if any value is not finite or the lengths differ.
fn ratio(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() || a.iter().chain(b).any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    a.iter().zip(b).map(|(x, y)| (x / y - 1.0).abs()).fold(0.0, f64::max)
}

/// `max_k |phi_h,k - s_k phi_t,k|_inf / |phi_t,k|_inf`, `s_k = sign(phi_h,k . phi_t,k)`.
fn shapes(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::NAN;
    }
    let mut worst: f64 = 0.0;
    for (x, y) in a.iter().zip(b) {
        let dot: f64 = x.iter().zip(y).map(|(p, q)| p * q).sum();
        let s = if dot < 0.0 { -1.0 } else { 1.0 };
        let neg: Vec<f64> = y.iter().map(|q| s * q).collect();
        let e = rel(x, &neg);
        if !e.is_finite() {
            return f64::NAN;
        }
        worst = worst.max(e);
    }
    worst
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && !a.is_empty() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn modal_print(name: &str, a: &ModalResult, b: &ModalResult) {
    println!(
        "{name} iterations {} / {}, residuals max {:.3e} / {:.3e}, lambda {:?}",
        a.iterations,
        b.iterations,
        a.residuals.iter().copied().fold(0.0, f64::max),
        b.residuals.iter().copied().fold(0.0, f64::max),
        a.eigenvalues
    );
}

fn modal_cmp(c: &mut Checks, name: &str, a: &ModalResult, b: &ModalResult) {
    modal_print(name, a, b);
    c.at_most(
        &format!("{name} e_lambda"),
        ratio(&a.eigenvalues, &b.eigenvalues),
        LIMIT,
    );
    c.at_most(&format!("{name} e_phi"), shapes(&a.modes, &b.modes), LIMIT);
}

/// The sector modes: eigenvalues of every index, the shapes (cos then sin concatenated) of the single indices 0 and N/2.
fn harmonic_cmp(c: &mut Checks, name: &str, k: usize, a: &HarmonicModes, b: &HarmonicModes) {
    println!(
        "{name} k = {k}: iterations {} / {}, residuals max {:.3e} / {:.3e}, lambda {:?}",
        a.iterations,
        b.iterations,
        a.residuals.iter().copied().fold(0.0, f64::max),
        b.residuals.iter().copied().fold(0.0, f64::max),
        a.eigenvalues
    );
    c.at_most(
        &format!("{name} k = {k} e_lambda"),
        ratio(&a.eigenvalues, &b.eigenvalues),
        LIMIT,
    );
    if k == 0 || 2 * k == N {
        let flat = |h: &HarmonicModes| -> Vec<Vec<f64>> {
            h.modes
                .iter()
                .map(|m| m.cos.iter().chain(&m.sin).copied().collect())
                .collect()
        };
        c.at_most(&format!("{name} k = {k} e_phi"), shapes(&flat(a), &flat(b)), LIMIT);
    }
}

fn linear_refusal<T>(r: &rcfd_fem::Result<T>, what: &str) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant, package, location, .. })
        if *variant == "Hyperelastic" && *package == LINEAR_FOLLOW_UP && location.contains(what))
}

// ----------------------------------------------------------------------------------------------- the rows

/// `solve_static`: `e_u` of the hyperelastic model against the other (ADR 0584 D3, the rows S).
fn static_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, control: bool) {
    match (
        solve_static(h, &StaticOptions::default()),
        solve_static(t, &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => c.row(
            &format!("{name} e_u"),
            rel(a.displacement_values(), b.displacement_values()),
            if control { CONTROL } else { LIMIT },
            control,
        ),
        (a, b) => c.miss(name, &format!("solve_static: {:?} / {:?}", a.err(), b.err())),
    }
}

/// `modal_analysis`: `e_lambda`, `e_phi` of the hyperelastic model against the twin (ADR 0584 D3, the rows M).
fn modal_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel) {
    match (modal_analysis(h, &modal_opts(6)), modal_analysis(t, &modal_opts(6))) {
        (Ok(a), Ok(b)) => modal_cmp(c, name, &a, &b),
        (a, b) => c.miss(name, &format!("modal_analysis: {:?} / {:?}", a.err(), b.err())),
    }
}

/// `solve_static_mpc` on the loaded sector: `e_u`.
fn cyclic_static(c: &mut Checks, name: &str, kind: ElementKind, other: Which) {
    let h = sector(Which::Hyper, kind, true);
    let t = sector(other, kind, true);
    match (
        solve_static_mpc(&h, &[], &[], &[], &StaticOptions::default()),
        solve_static_mpc(&t, &[], &[], &[], &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => c.row(
            &format!("{name} e_u"),
            rel(a.result.displacement_values(), b.result.displacement_values()),
            if other == Which::Perturbed { CONTROL } else { LIMIT },
            other == Which::Perturbed,
        ),
        (a, b) => c.miss(name, &format!("solve_static_mpc: {:?} / {:?}", a.err(), b.err())),
    }
}

/// The harmonic load of ADR 0593 D3 (`k = 1`).
fn harmonic_load(m: &SolidModel) -> HarmonicLoad {
    let n = nodal_load_vector(m, &[]).unwrap().len();
    let cos = (0..n).map(|e| (1.3 * e as f64 + 0.2).sin()).collect();
    let sin = (0..n).map(|e| (0.7 * e as f64 + 0.5).cos()).collect();
    HarmonicLoad::new(1, cos, sin)
}

fn cyclic_harmonic(c: &mut Checks, name: &str, kind: ElementKind) {
    let h = sector(Which::Hyper, kind, false);
    let t = sector(Which::Twin, kind, false);
    match (
        solve_static_harmonic(&h, &[], &harmonic_load(&h), &StaticOptions::default()),
        solve_static_harmonic(&t, &[], &harmonic_load(&t), &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => {
            let x: Vec<f64> = a.cos.iter().chain(&a.sin).copied().collect();
            let y: Vec<f64> = b.cos.iter().chain(&b.sin).copied().collect();
            c.at_most(&format!("{name} e_u"), rel(&x, &y), LIMIT);
        }
        (a, b) => c.miss(name, &format!("solve_static_harmonic: {:?} / {:?}", a.err(), b.err())),
    }
}

fn cyclic_modes(c: &mut Checks, name: &str, kind: ElementKind) {
    let h = sector(Which::Hyper, kind, false);
    let t = sector(Which::Twin, kind, false);
    for k in 0..=N / 2 {
        match (
            modal_analysis_harmonic(&h, &[], k, &modal_opts(3)),
            modal_analysis_harmonic(&t, &[], k, &modal_opts(3)),
        ) {
            (Ok(a), Ok(b)) => harmonic_cmp(c, name, k, &a, &b),
            (a, b) => c.miss(
                &format!("{name} k = {k}"),
                &format!("modal_analysis_harmonic: {:?} / {:?}", a.err(), b.err()),
            ),
        }
    }
}

fn rest_lambda(part: &RotatingPart) -> Option<f64> {
    match part.modes(0.0) {
        Ok(SpeedModes::Modes(r)) => r.eigenvalues.first().copied(),
        _ => None,
    }
}

/// `RotatingPart::modes(Omega)` of the model `h` against `t` (the twin, or the perturbed twin for a control), `Omega =
/// 0.2 sqrt(lambda_1)` of `t` at rest (ADR 0593 D3). Returns the two results when both have modes.
fn rotating_row(
    c: &mut Checks,
    name: &str,
    h: &SolidModel,
    t: &SolidModel,
    control: bool,
) -> Option<(ModalResult, ModalResult)> {
    let o = RotatingOptions::new(6).modal(modal_opts(6));
    let axis = [0.0, 0.0, 1.0];
    let (ph, pt) = match (
        RotatingPart::new(h, axis, [0.0; 3], &o),
        RotatingPart::new(t, axis, [0.0; 3], &o),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            c.miss(name, &format!("RotatingPart::new: {:?} / {:?}", a.err(), b.err()));
            return None;
        }
    };
    let Some(l1) = rest_lambda(&pt) else {
        c.miss(name, "no modes at rest");
        return None;
    };
    let w = 0.2 * l1.sqrt();
    println!("{name}: lambda_1 at rest {l1:.10e}, Omega = {w:.10e} rad/s");
    match (ph.modes(w), pt.modes(w)) {
        (Ok(SpeedModes::Modes(a)), Ok(SpeedModes::Modes(b))) => {
            if control {
                modal_print(name, &a, &b);
                c.above(
                    &format!("{name} e_lambda"),
                    ratio(&a.eigenvalues, &b.eigenvalues),
                    CONTROL,
                );
            } else {
                modal_cmp(c, name, &a, &b);
            }
            Some((a, b))
        }
        (a, b) => {
            c.miss(name, &format!("modes(Omega): {a:?} / {b:?}"));
            None
        }
    }
}

/// `cms::reduce` on `"xl"`: the reduced matrices, load, fixed-interface eigenvalues, reduced eigenvalues and the recovered
/// static field. With `control`, only the stiffness against `CONTROL`.
fn cms_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, control: bool) {
    let o = CmsOptions::new(4);
    let (a, b) = match (reduce(h, "xl", &o), reduce(t, "xl", &o)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => return c.miss(name, &format!("reduce: {:?} / {:?}", a.err(), b.err())),
    };
    println!(
        "{name}: boundary {}, interior {}, modes {}, reduced {}, fixed-interface eigenvalues {:?}",
        a.n_boundary(),
        a.n_interior(),
        a.modes(),
        a.n_reduced(),
        a.fixed_interface_eigenvalues()
    );
    if control {
        c.above(&format!("{name} e_K"), rel(a.stiffness(), b.stiffness()), CONTROL);
        return;
    }
    c.at_most(&format!("{name} e_K"), rel(a.stiffness(), b.stiffness()), LIMIT);
    c.at_most(&format!("{name} e_M"), rel(a.mass(), b.mass()), LIMIT);
    c.at_most(&format!("{name} e_f"), rel(a.load(), b.load()), LIMIT);
    c.at_most(
        &format!("{name} e_lambda fixed-interface"),
        ratio(a.fixed_interface_eigenvalues(), b.fixed_interface_eigenvalues()),
        LIMIT,
    );
    match (a.eigen(), b.eigen()) {
        (Ok(x), Ok(y)) => c.at_most(&format!("{name} e_lambda reduced"), ratio(&x.values, &y.values), LIMIT),
        (x, y) => c.miss(name, &format!("eigen: {:?} / {:?}", x.err(), y.err())),
    }
    let recovered =
        |p: &rcfd_fem::cms::ReducedPart, m: &SolidModel| p.solve_static().and_then(|ub| p.recover_static(m, &ub));
    match (recovered(&a, h), recovered(&b, t)) {
        (Ok(x), Ok(y)) => c.at_most(&format!("{name} e_u recovered"), rel(&x, &y), LIMIT),
        (x, y) => c.miss(name, &format!("recover_static: {:?} / {:?}", x.err(), y.err())),
    }
}

/// The mesh displacements and the reference motions of an mpc solution, concatenated.
fn mpc_flat(s: &rcfd_fem::mpc::MpcSolution) -> Vec<f64> {
    let mut v = s.result.displacement_values().to_vec();
    for r in &s.references {
        v.extend(r.translation);
        v.extend(r.rotation);
    }
    v
}

fn reference_load() -> [ReferenceLoad; 1] {
    [ReferenceLoad::new(6, [0.1, -0.2, 0.05], [0.01, 0.0, 0.02])]
}

/// `solve_static_mpc` (with the reference load) and `modal_analysis_mpc`. With `control`, only the static `e_u` against
/// `CONTROL`.
fn mpc_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, control: bool) {
    let rl = reference_load();
    match (
        solve_static_mpc(h, &[], &[], &rl, &StaticOptions::default()),
        solve_static_mpc(t, &[], &[], &rl, &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => c.row(
            &format!("{name} static e_u"),
            rel(&mpc_flat(&a), &mpc_flat(&b)),
            if control { CONTROL } else { LIMIT },
            control,
        ),
        (a, b) => c.miss(name, &format!("solve_static_mpc: {:?} / {:?}", a.err(), b.err())),
    }
    if control {
        return;
    }
    match (
        modal_analysis_mpc(h, &[], &modal_opts(6)),
        modal_analysis_mpc(t, &[], &modal_opts(6)),
    ) {
        (Ok(a), Ok(b)) => modal_cmp(c, &format!("{name} modes"), &a.result, &b.result),
        (a, b) => c.miss(name, &format!("modal_analysis_mpc: {:?} / {:?}", a.err(), b.err())),
    }
}

// ----------------------------------------------------------------------------------------------- T1 restated: A-RF1, K-0555

/// The 2 x 2 Shell4 plate of RF1 (ADR 0555) as `rf1_to_rf4_refusals` builds it; `supported` adds the set-up (b) of ADR
/// 0746: the column `x = 0` (nodes 0, 3, 6) held in six components, the loads of P4 at the corner node 8.
fn rf1_plate(material: Material, supported: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=2 {
        for i in 0..=2 {
            nodes.push([i as f64 * 0.5, j as f64 * 0.5, 0.0]);
        }
    }
    let idx = |i: usize, j: usize| j * 3 + i;
    let mut conn = Vec::new();
    for j in 0..2 {
        for i in 0..2 {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    if supported {
        mesh.add_node_set("x0", vec![0, 3, 6]).unwrap();
        mesh.add_node_set("c", vec![8]).unwrap();
    }
    let mut plate = SolidModel::new(mesh);
    plate.add_material("all", material).unwrap();
    if supported {
        for c in 0..6 {
            plate.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
        plate.add_load(Load::nodal_force("c", [0.1, 0.2, 0.3])).unwrap();
        plate.add_load(Load::nodal_moment("c", [0.01, 0.02, 0.005])).unwrap();
        plate.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    }
    plate
}

/// `rf1_to_rf4_refusals` of `sa_hyper_paths.rs` (ADR 0555) restated by ADR 0746: RF1 as acceptance, RF2 to RF4 as they are.
#[test]
fn rf1_to_rf4_after_adr0746() {
    let mut c = Checks::new();
    // A-RF1 (a): the plate of RF1 as built there.
    let r = solve_static(&rf1_plate(hyper(), false), &StaticOptions::default());
    let for_the_material = matches!(&r, Err(FemError::NotYetSupported { variant, .. }) if *variant == "Hyperelastic");
    c.holds(
        "A-RF1 (a) the Shell4 model of RF1 not refused for the material",
        !for_the_material,
        &format!("{:?}", r.as_ref().err()),
    );
    // A-RF1 (b): supported and loaded, against the twin.
    static_row(
        &mut c,
        "A-RF1 (b)",
        &rf1_plate(hyper(), true),
        &rf1_plate(twin(), true),
        false,
    );
    adr0555::rf2_rf3_rf4(&mut c);
    c.finish();
}

/// The set-ups and the rows RF2, RF3, RF4 of `sa_hyper_paths.rs` (ADR 0555 D7), copied.
mod adr0555 {
    use super::{linear_refusal, Checks};
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::builders::hex_box;
    use rcfd_fem::mat_hyper;
    use rcfd_fem::material::{HyperelasticEnergy, MaterialAxes, PointContext};
    use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
    use rcfd_fem::nonlinear::NewtonOptions;
    use rcfd_fem::solve::{solve_static, StaticOptions};
    use rcfd_fem::thermal::TemperatureFieldHistory;
    use rcfd_fem::{BlockProperty, Constraint, FemError, Load, Material, SolidFormulation, SolidModel};

    const RHO: f64 = 1000.0;

    fn nhc() -> HyperelasticEnergy {
        HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
    }
    fn nhk() -> HyperelasticEnergy {
        HyperelasticEnergy::neo_hookean(1e5, 1e6).unwrap()
    }
    fn hyper(e: HyperelasticEnergy) -> Material {
        Material::hyperelastic(e, RHO).unwrap()
    }

    fn key_of(e: &FemError) -> Option<&'static str> {
        match e {
            FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
            _ => None,
        }
    }

    fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str, text: &str) -> (bool, String) {
        match r {
            Ok(_) => (false, "accepted".to_string()),
            Err(e) => {
                let s = e.to_string();
                (key_of(&e) == Some(key) && s.contains(text), s)
            }
        }
    }

    /// Cantilever L.
    fn cantilever(material: Material, formulation: SolidFormulation, force: Vec3) -> SolidModel {
        let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
        if formulation != SolidFormulation::Full {
            mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
        }
        let mut model = SolidModel::new(mesh);
        model.add_material("all", material).unwrap();
        for k in 0..3 {
            model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
        }
        model.add_load(Load::nodal_force("xmax", force)).unwrap();
        model
    }

    /// Cube F (free), the `v_0` of ADR 0523 arithmetic 3.
    fn cube_f(material: Material, formulation: SolidFormulation) -> (SolidModel, Vec<f64>) {
        let mut mesh = hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
        if formulation != SolidFormulation::Full {
            mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
        }
        let mut model = SolidModel::new(mesh);
        model.add_material("all", material).unwrap();
        let dofs = model.dof_map();
        let (omega, alpha, gamma) = (3.0, 5.0, 4.0);
        let velocity = [0.3, -0.2, 0.1];
        let mut v0 = vec![0.0; dofs.n_equations()];
        for (i, p) in model.mesh().nodes().iter().enumerate() {
            let r = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
            let w = [
                -omega * r[1] + velocity[0] + alpha * r[0] + gamma * r[1],
                omega * r[0] + velocity[1] + alpha * r[1],
                velocity[2] + alpha * r[2],
            ];
            for (comp, wc) in w.iter().enumerate() {
                v0[dofs.equation(i, comp).unwrap()] = *wc;
            }
        }
        (model, v0)
    }

    fn method(dt: f64) -> NlTransientOptions {
        NlTransientOptions::new(dt).newton(NewtonOptions::new(1))
    }

    /// The rows RF2, RF3 and RF4 of `rf1_to_rf4_refusals`, as they are there.
    pub fn rf2_rf3_rf4(c: &mut Checks) {
        // RF2.
        let mut model = cantilever(hyper(nhc()), SolidFormulation::Full, [0.0, 0.0, 1.0]);
        let nn = model.mesh().nodes().len();
        model.add_load(Load::temperature(vec![303.0; nn], 293.0)).unwrap();
        let r = solve_static(&model, &StaticOptions::default());
        c.holds(
            "RF2 a temperature load refused naming LINEAR_FOLLOW_UP",
            linear_refusal(&r, "a temperature load"),
            &format!("{:?}", r.err()),
        );
        // RF3.
        let m = hyper(nhc());
        let em = m.elasticity_matrix();
        c.holds(
            "RF3 elasticity_matrix still refused naming mat_hyper::FOLLOW_UP",
            matches!(&em, Err(FemError::NotYetSupported { package, location, .. })
                if *package == mat_hyper::FOLLOW_UP && location == "the element-constant elasticity matrix"),
            &format!("{:?}", em.err()),
        );
        let eps = [1e-3, -4e-4, 2e-4, 6e-4, -3e-4, 5e-4];
        let st = m.stress(&eps, &PointContext::default(), &[]).unwrap();
        let want = mat_hyper::stress_and_tangent(&nhc(), &eps).unwrap();
        let bits = |x: &[f64]| x.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        c.holds(
            "RF3 Material::stress equal in bits to stress_and_tangent",
            bits(&st.0) == bits(&want.0) && bits(&st.1.concat()) == bits(&want.1.concat()),
            "",
        );
        let iso = Material::isotropic_elastic(2e11, 0.3, 7850.0, 1.2e-5).unwrap();
        let others = [
            ("isotropic", iso.clone()),
            (
                "orthotropic",
                Material::orthotropic(
                    [1.4e11, 1.0e10, 1.0e10],
                    [0.3, 0.3, 0.45],
                    [5.0e9, 5.0e9, 3.5e9],
                    1600.0,
                    [0.0; 3],
                    MaterialAxes::fixed([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
                )
                .unwrap(),
            ),
            ("J2", Material::j2(iso, 2.5e8, 1e9, 5e8).unwrap()),
        ];
        for (name, m) in others {
            let (a, b) = (m.linear_elasticity_matrix(), m.elasticity_matrix());
            let same = match (&a, &b) {
                (Ok(x), Ok(y)) => bits(&x.concat()) == bits(&y.concat()),
                (Err(x), Err(y)) => x.to_string() == y.to_string(),
                _ => false,
            };
            c.holds(
                &format!("RF3 {name} linear_elasticity_matrix as elasticity_matrix"),
                same,
                "",
            );
        }
        // RF4.
        let (cube, _) = cube_f(hyper(nhk()), SolidFormulation::Full);
        let nn = cube.mesh().nodes().len();
        let field = TemperatureFieldHistory::constant(293.0, vec![303.0; nn], 0.0, 1.0).unwrap();
        let (ok, d) = refused_with(
            NlIntegrator::new(&cube, &[], &method(0.004).temperature_field(field)),
            "expansion",
            "",
        );
        c.holds(
            "RF4 a temperature field on the Method step refused (key expansion)",
            ok,
            &d,
        );
    }
}

// ----------------------------------------------------------------------------------------------- T2 restated: A-S, A-M, K-0584

/// `n1_to_n4_settings_and_refusals` of `sa_hyper_structures.rs` (ADR 0584) restated by ADR 0746: N1 as acceptance (the
/// static solve and the modes of Beam B, P4 and P3 by default against the twin), N2 to N4 as they are.
#[test]
fn n1_to_n4_after_adr0746() {
    let mut c = Checks::new();
    // A-S, A-M: N1 as acceptance.
    static_row(
        &mut c,
        "A-S Beam2",
        &beam_b(Which::Hyper, true),
        &beam_b(Which::Twin, true),
        false,
    );
    modal_row(
        &mut c,
        "A-M Beam2",
        &beam_b(Which::Hyper, false),
        &beam_b(Which::Twin, false),
    );
    for (name, tri) in [("Shell4", false), ("Shell3", true)] {
        static_row(
            &mut c,
            &format!("A-S {name}"),
            &plate(Which::Hyper, tri, true),
            &plate(Which::Twin, tri, true),
            false,
        );
        modal_row(
            &mut c,
            &format!("A-M {name}"),
            &plate(Which::Hyper, tri, false),
            &plate(Which::Twin, tri, false),
        );
    }
    adr0584::n2_n3_n4(&mut c);
    // K-0584, the last part (ADR 0746 D2): the setter stores the value and the getter returns it.
    let mut m = beam_b(Which::Hyper, false);
    let before = m.hyperelastic_structures();
    m.set_hyperelastic_structures(true);
    c.holds(
        "K-0584 the getter returns false by default and true after set(true)",
        !before && m.hyperelastic_structures(),
        "",
    );
    c.finish();
}

/// The set-ups and the rows N2, N3, N4 of `sa_hyper_structures.rs` (ADR 0584 D3), copied.
mod adr0584 {
    use super::{linear_refusal, Checks};
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::beam;
    use rcfd_fem::mat_hyper;
    use rcfd_fem::material::HyperelasticEnergy;
    use rcfd_fem::modal::{modal_analysis, ModalOptions};
    use rcfd_fem::solve::{solve_static, StaticOptions};
    use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
    use rcfd_linalg::eigen::ResidualTest;

    const RHO: f64 = 1000.0;

    fn nhc() -> HyperelasticEnergy {
        HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
    }

    fn hyper(e: HyperelasticEnergy) -> Material {
        Material::hyperelastic(e, RHO).unwrap()
    }

    /// The isotropic twin of an energy: `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)`, `s = 3K + G` (ADR 0584 D2, D3).
    fn twin(e: &HyperelasticEnergy) -> Material {
        let (g, k) = mat_hyper::small_strain_moduli(e);
        let s = 3.0 * k + g;
        Material::isotropic_elastic(9.0 * k * g / s, (3.0 * k - 2.0 * g) / (2.0 * s), RHO, 0.0).unwrap()
    }

    /// Beam B: Beam2 along `x`, `L = 1`, `n` elements, `rectangle_section(0.05, 0.03, 2.6e-7, 5/6)`, orientation `[0, 1, 0]`;
    /// node sets `all`, `x0`, `xl`; element set `all`.
    fn beam_mesh(n: usize) -> SolidMesh {
        let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
        let mut mesh = SolidMesh::new(nodes).unwrap();
        let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
        mesh.add_block(ElementKind::Beam2, conn).unwrap();
        let section = beam::rectangle_section(0.05, 0.03, 2.6e-7, 5.0 / 6.0).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
            .unwrap();
        mesh.add_element_set("all", (0..n).collect()).unwrap();
        mesh.add_node_set("all", (0..=n).collect()).unwrap();
        mesh.add_node_set("x0", vec![0]).unwrap();
        mesh.add_node_set("xl", vec![n]).unwrap();
        mesh
    }

    /// Beam B clamped at `x0`, with the material; `loads` adds the static loads of D3.
    fn beam_b(material: Material, opt_in: bool, loads: bool) -> SolidModel {
        let mut m = SolidModel::new(beam_mesh(10));
        m.add_material("all", material).unwrap();
        m.set_hyperelastic_structures(opt_in);
        for c in 0..6 {
            m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
        if loads {
            m.add_load(Load::nodal_force("xl", [0.1, 0.2, 0.3])).unwrap();
            m.add_load(Load::nodal_moment("xl", [0.01, 0.02, 0.03])).unwrap();
            m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
        }
        m
    }

    /// Plate P4 (`tri = false`) or P3: `[0, 1] x [0, 0.6]`, `10 x 6` cells, `shell(0.01)`, the column `x = 0` clamped, the
    /// corner `(1, 0.6)` set `c`; `loads` adds the static loads of D3.
    fn plate(material: Material, tri: bool, opt_in: bool, loads: bool) -> SolidModel {
        let (nx, ny, lx, ly) = (10usize, 6usize, 1.0, 0.6);
        let idx = |i: usize, j: usize| j * (nx + 1) + i;
        let mut nodes = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                nodes.push([lx * i as f64 / nx as f64, ly * j as f64 / ny as f64, 0.0]);
            }
        }
        let mut conn = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let (a, b, cc, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
                if tri {
                    conn.extend_from_slice(&[a, b, cc, a, cc, d]);
                } else {
                    conn.extend_from_slice(&[a, b, cc, d]);
                }
            }
        }
        let ne = if tri { 2 * nx * ny } else { nx * ny };
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
            .unwrap();
        mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        mesh.add_node_set("x0", (0..=ny).map(|j| idx(0, j)).collect()).unwrap();
        mesh.add_node_set("c", vec![idx(nx, ny)]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", material).unwrap();
        m.set_hyperelastic_structures(opt_in);
        for c in 0..6 {
            m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
        if loads {
            m.add_load(Load::nodal_force("c", [0.1, 0.2, 0.3])).unwrap();
            m.add_load(Load::nodal_moment("c", [0.01, 0.02, 0.005])).unwrap();
            m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
        }
        m
    }

    fn modal_opts() -> ModalOptions {
        ModalOptions::new(6).residual_test(ResidualTest::Backward)
    }

    /// The rows N2, N3 and N4 of `n1_to_n4_settings_and_refusals`, as they are there.
    pub fn n2_n3_n4(c: &mut Checks) {
        // N2: with the setting a temperature load stays refused.
        let mut m = beam_b(hyper(nhc()), true, true);
        let nn = m.mesh().nodes().len();
        m.add_load(Load::temperature(vec![303.0; nn], 293.0)).unwrap();
        let r = solve_static(&m, &StaticOptions::default());
        c.holds(
            "N2 Beam2 with the setting and a temperature load refused",
            linear_refusal(&r, "a temperature load"),
            &format!("{:?}", r.err()),
        );
        // N3: the setting does not change an isotropic model.
        let iso = twin(&nhc());
        for (name, on, off) in [
            (
                "Beam2",
                beam_b(iso.clone(), true, true),
                beam_b(iso.clone(), false, true),
            ),
            (
                "Shell4",
                plate(iso.clone(), false, true, true),
                plate(iso.clone(), false, false, true),
            ),
        ] {
            let a = solve_static(&on, &StaticOptions::default()).unwrap();
            let b = solve_static(&off, &StaticOptions::default()).unwrap();
            let same = a
                .displacement_values()
                .iter()
                .zip(b.displacement_values())
                .all(|(x, y)| x.to_bits() == y.to_bits());
            c.holds(&format!("N3 {name} static bits"), same, "");
            let a = modal_analysis(&on, &modal_opts()).unwrap();
            let b = modal_analysis(&off, &modal_opts()).unwrap();
            let same = a
                .eigenvalues
                .iter()
                .zip(&b.eigenvalues)
                .all(|(x, y)| x.to_bits() == y.to_bits());
            c.holds(&format!("N3 {name} eigenvalue bits"), same, "");
        }
        // N4: the default.
        let m = SolidModel::new(beam_mesh(2));
        c.holds("N4 hyperelastic_structures() default", !m.hyperelastic_structures(), "");
    }
}

// ----------------------------------------------------------------------------------------------- T3 restated: A-N1

/// `n1_refused_without_the_setting` of `sa_hyper_structures_f666.rs` (ADR 0593) restated by ADR 0746: its four calls,
/// as written there, accepted.
#[test]
fn n1_accepted_without_the_setting() {
    let mut c = Checks::new();
    let r = modal_analysis_harmonic(
        &sector(Which::Hyper, ElementKind::Shell4, false),
        &[],
        0,
        &modal_opts(3),
    );
    c.holds("A-N1 cyclic Q4", r.is_ok(), &format!("{:?}", r.err()));
    let b = beam_b(Which::Hyper, false);
    let r = RotatingPart::new(&b, [0.0, 0.0, 1.0], [0.0; 3], &RotatingOptions::new(6));
    c.holds("A-N1 rotating B", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    let r = reduce(&beam_b(Which::Hyper, true), "xl", &CmsOptions::new(4));
    c.holds("A-N1 cms B", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    let r = solve_static_mpc(
        &plate_mpc(Which::Hyper, false),
        &[],
        &[],
        &[],
        &StaticOptions::default(),
    );
    c.holds("A-N1 mpc P4", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    c.finish();
}

// ----------------------------------------------------------------------------------------------- a1 to a4 by default

#[test]
fn cy_q4_cyclic_paths_by_default() {
    let mut c = Checks::new();
    cyclic_static(&mut c, "A-CY CY-S-Q4", ElementKind::Shell4, Which::Twin);
    cyclic_harmonic(&mut c, "A-CY CY-H-Q4", ElementKind::Shell4);
    cyclic_modes(&mut c, "A-CY CY-M-Q4", ElementKind::Shell4);
    c.finish();
}

#[test]
fn ro_rotating_part_by_default() {
    let mut c = Checks::new();
    rotating_row(
        &mut c,
        "A-RO RO-B",
        &beam_b(Which::Hyper, false),
        &beam_b(Which::Twin, false),
        false,
    );
    rotating_row(
        &mut c,
        "A-RO RO-P4",
        &plate(Which::Hyper, false, false),
        &plate(Which::Twin, false, false),
        false,
    );
    c.finish();
}

#[test]
fn cm_cms_reduce_by_default() {
    let mut c = Checks::new();
    cms_row(
        &mut c,
        "A-CM CM-B",
        &beam_b(Which::Hyper, true),
        &beam_b(Which::Twin, true),
        false,
    );
    cms_row(
        &mut c,
        "A-CM CM-P4",
        &plate(Which::Hyper, false, true),
        &plate(Which::Twin, false, true),
        false,
    );
    c.finish();
}

#[test]
fn mp_mpc_transformation_by_default() {
    let mut c = Checks::new();
    mpc_row(
        &mut c,
        "A-MP MP-B",
        &beam_mpc(Which::Hyper),
        &beam_mpc(Which::Twin),
        false,
    );
    mpc_row(
        &mut c,
        "A-MP MP-P4",
        &plate_mpc(Which::Hyper, false),
        &plate_mpc(Which::Twin, false),
        false,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- X: the extras by default

#[test]
fn x_cy_q3_cyclic_paths_by_default() {
    let mut c = Checks::new();
    cyclic_static(&mut c, "X-CY-S-Q3", ElementKind::Shell3, Which::Twin);
    cyclic_harmonic(&mut c, "X-CY-H-Q3", ElementKind::Shell3);
    cyclic_modes(&mut c, "X-CY-M-Q3", ElementKind::Shell3);
    c.finish();
}

#[test]
fn x_rs_q4_rotating_sector_by_default() {
    let mut c = Checks::new();
    let h = sector(Which::Hyper, ElementKind::Shell4, false);
    let t = sector(Which::Twin, ElementKind::Shell4, false);
    let o = RotatingOptions::new(3).modal(modal_opts(3));
    match (RotatingSector::new(&h, &o), RotatingSector::new(&t, &o)) {
        (Ok(sh), Ok(st)) => {
            let rest: Option<Vec<f64>> = (0..=N / 2)
                .map(|k| match st.modes(k, 0.0) {
                    Ok(HarmonicSpeedModes::Modes(r)) => r.eigenvalues.first().copied(),
                    _ => None,
                })
                .collect();
            match rest {
                Some(l) => {
                    let l1 = l.iter().copied().fold(f64::INFINITY, f64::min);
                    let w = 0.2 * l1.sqrt();
                    println!("X-RS-Q4: lowest rest eigenvalues {l:?}, Omega = {w:.10e} rad/s");
                    for k in 0..=N / 2 {
                        match (sh.modes(k, w), st.modes(k, w)) {
                            (Ok(HarmonicSpeedModes::Modes(a)), Ok(HarmonicSpeedModes::Modes(b))) => {
                                harmonic_cmp(&mut c, "X-RS-Q4", k, &a, &b)
                            }
                            (a, b) => c.miss(&format!("X-RS-Q4 k = {k}"), &format!("{a:?} / {b:?}")),
                        }
                    }
                }
                None => c.miss("X-RS-Q4", "no modes at rest"),
            }
        }
        (a, b) => c.miss(
            "X-RS-Q4",
            &format!("RotatingSector::new: {:?} / {:?}", a.err(), b.err()),
        ),
    }
    c.finish();
}

#[test]
fn x_mp_p3_mpc_transformation_by_default() {
    let mut c = Checks::new();
    mpc_row(
        &mut c,
        "X-MP-P3",
        &plate_mpc(Which::Hyper, true),
        &plate_mpc(Which::Twin, true),
        false,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- E: the setting is a no-op

/// The values of E for the hyperelastic model `w`: name and values, in a fixed order. `omega` is the speed of RO-B.
fn e_values(w: Which, omega: f64) -> Vec<(&'static str, Vec<f64>)> {
    let st = |m: &SolidModel| {
        solve_static(m, &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec()
    };
    let ev = |m: &SolidModel| modal_analysis(m, &modal_opts(6)).unwrap().eigenvalues;
    let mut out = vec![
        ("solve_static Beam B", st(&beam_b(w, true))),
        ("solve_static P4", st(&plate(w, false, true))),
        ("solve_static P3", st(&plate(w, true, true))),
        ("modal_analysis Beam B", ev(&beam_b(w, false))),
        ("modal_analysis P4", ev(&plate(w, false, false))),
    ];
    let cy = solve_static_mpc(
        &sector(w, ElementKind::Shell4, true),
        &[],
        &[],
        &[],
        &StaticOptions::default(),
    )
    .unwrap();
    out.push(("CY-S-Q4", cy.result.displacement_values().to_vec()));
    let b = beam_b(w, false);
    let o = RotatingOptions::new(6).modal(modal_opts(6));
    let part = RotatingPart::new(&b, [0.0, 0.0, 1.0], [0.0; 3], &o).unwrap();
    match part.modes(omega).unwrap() {
        SpeedModes::Modes(r) => out.push(("RO-B", r.eigenvalues)),
        other => panic!("RO-B: no modes at Omega: {other:?}"),
    }
    let part = reduce(&beam_b(w, true), "xl", &CmsOptions::new(4)).unwrap();
    out.push(("CM-B stiffness", part.stiffness().to_vec()));
    out.push(("CM-B mass", part.mass().to_vec()));
    out.push(("CM-B load", part.load().to_vec()));
    let mp = solve_static_mpc(
        &plate_mpc(w, false),
        &[],
        &[],
        &reference_load(),
        &StaticOptions::default(),
    )
    .unwrap();
    out.push(("MP-P4", mpc_flat(&mp)));
    let wm = match w {
        Which::HyperSet => wheel::sector_of(wheel::default_options(), wheel::hyper(), true),
        Which::HyperSetUnset => {
            let mut m = wheel::sector_of(wheel::default_options(), wheel::hyper(), true);
            m.set_hyperelastic_structures(false);
            m
        }
        _ => wheel::sector_by_default(wheel::hyper()),
    };
    let r = modal_analysis_harmonic(&wm, &[], 0, &wheel::modal_options(4)).unwrap();
    out.push(("W k = 0", r.eigenvalues));
    out
}

#[test]
fn e_the_setting_is_a_no_op() {
    let mut c = Checks::new();
    // The speed of RO-B (ADR 0593 D3): 0.2 sqrt(lambda_1) of the twin at rest.
    let t = beam_b(Which::Twin, false);
    let o = RotatingOptions::new(6).modal(modal_opts(6));
    let l1 = rest_lambda(&RotatingPart::new(&t, [0.0, 0.0, 1.0], [0.0; 3], &o).unwrap()).unwrap();
    let omega = 0.2 * l1.sqrt();
    let default = e_values(Which::Hyper, omega);
    for (label, w) in [
        ("set(true)", Which::HyperSet),
        ("set(true), set(false)", Which::HyperSetUnset),
    ] {
        let other = e_values(w, omega);
        assert_eq!(default.len(), other.len());
        for ((name, a), (_, b)) in default.iter().zip(&other) {
            c.holds(
                &format!("E {name}: the default against {label}"),
                bits_equal(a, b),
                &format!("{} values, bits equal", a.len()),
            );
        }
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------- N-T, C

#[test]
fn n_t_temperature_load_still_refused_by_default() {
    let mut c = Checks::new();
    for (name, mut m) in [
        ("Beam B", beam_b(Which::Hyper, true)),
        ("P4", plate(Which::Hyper, false, true)),
    ] {
        let nn = m.mesh().nodes().len();
        m.add_load(Load::temperature(vec![303.0; nn], 293.0)).unwrap();
        let r = solve_static(&m, &StaticOptions::default());
        c.holds(
            &format!("N-T {name} by default with a temperature load refused"),
            linear_refusal(&r, "a temperature load"),
            &format!("{:?}", r.err()),
        );
    }
    c.finish();
}

#[test]
fn c_perturbed_twin_is_seen_by_default() {
    let mut c = Checks::new();
    static_row(
        &mut c,
        "C-B static",
        &beam_b(Which::Hyper, true),
        &beam_b(Which::Perturbed, true),
        true,
    );
    cyclic_static(&mut c, "C-CY", ElementKind::Shell4, Which::Perturbed);
    rotating_row(
        &mut c,
        "C-RO",
        &beam_b(Which::Hyper, false),
        &beam_b(Which::Perturbed, false),
        true,
    );
    cms_row(
        &mut c,
        "C-CM",
        &beam_b(Which::Hyper, true),
        &beam_b(Which::Perturbed, true),
        true,
    );
    mpc_row(
        &mut c,
        "C-MP",
        &plate_mpc(Which::Hyper, false),
        &plate_mpc(Which::Perturbed, false),
        true,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- T4 restated: A-W, K-0744

/// The wheel sector W of ADR 0595 with the shells option alone and the rows AB-a, H-D, H-T of
/// `sa_cyclic_beams_default_adr0744.rs` (ADR 0744), the builders copied.
mod wheel {
    use rcfd_fem::beam;
    use rcfd_fem::cyclic::modal_analysis_harmonic;
    use rcfd_fem::mat_hyper;
    use rcfd_fem::material::HyperelasticEnergy;
    use rcfd_fem::modal::ModalOptions;
    use rcfd_fem::model::CyclicOptions;
    use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Material, SolidMesh, SolidModel};
    use rcfd_linalg::eigen::ResidualTest;
    use std::f64::consts::PI;

    type Vec3 = [f64; 3];

    const N: usize = 6;
    const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
    /// Ring elements per sector.
    const NR: usize = 4;
    /// The loaded sector nodes (not the high node 4, not the hub 6).
    const LOADED: [usize; 5] = [0, 1, 2, 3, 5];

    fn axis() -> Vec3 {
        [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
    }

    fn cross(a: Vec3, b: Vec3) -> Vec3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn ea() -> Vec3 {
        let l = 5.0f64.sqrt();
        [2.0 / l, -1.0 / l, 0.0]
    }

    fn eb() -> Vec3 {
        cross(axis(), ea())
    }

    fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
        [0, 1, 2].map(|k| a * x[k] + b * y[k])
    }

    /// `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`.
    fn p(phi: f64, r: f64, z: f64) -> Vec3 {
        let (n, a, b) = (axis(), ea(), eb());
        let (s, c) = phi.sin_cos();
        [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
    }

    fn alpha() -> f64 {
        2.0 * PI / N as f64
    }

    fn ring_section() -> BeamSection {
        BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap()
    }

    fn spoke_section() -> BeamSection {
        BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap()
    }

    fn v_ring() -> Vec3 {
        comb(0.8, axis(), 0.6, ea())
    }

    fn v_spoke() -> Vec3 {
        comb(0.6, eb(), 0.8, axis())
    }

    pub fn modal_options(modes: usize) -> ModalOptions {
        let mut o = ModalOptions::new(modes);
        o.tolerance = 1e-12;
        o.residual_test = ResidualTest::Backward;
        o
    }

    /// The options of ADR 0744 D1: the shells option alone.
    pub fn default_options() -> CyclicOptions {
        CyclicOptions::new().shells(true)
    }

    fn clamp(m: &mut SolidModel) {
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
        }
    }

    /// W with the material `material` and the setting `SolidModel::set_hyperelastic_structures(setting)` (H-T).
    pub fn sector_of(options: CyclicOptions, material: Material, setting: bool) -> SolidModel {
        let mut m = SolidModel::new(sector_mesh());
        m.add_material("all", material).unwrap();
        m.set_hyperelastic_structures(setting);
        clamp(&mut m);
        m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
            .unwrap();
        m
    }

    /// W with the material `material` by default (ADR 0746): the setter is never called.
    pub fn sector_by_default(material: Material) -> SolidModel {
        let mut m = SolidModel::new(sector_mesh());
        m.add_material("all", material).unwrap();
        clamp(&mut m);
        m.add_constraint(Constraint::cyclic_with(
            "low",
            "high",
            axis(),
            ORIGIN,
            N,
            default_options(),
        ))
        .unwrap();
        m
    }

    /// The mesh of W.
    fn sector_mesh() -> SolidMesh {
        let a = alpha();
        let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
        nodes.push(p(a / 2.0, 0.65, 0.125));
        nodes.push(p(a / 2.0, 0.3, 0.25));
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
            .unwrap();
        mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(ring_section(), v_ring()).unwrap())
            .unwrap();
        mesh.set_block_property(1, BlockProperty::beam(spoke_section(), v_spoke()).unwrap())
            .unwrap();
        mesh.add_element_set("all", (0..6).collect()).unwrap();
        mesh.add_node_set("low", vec![0]).unwrap();
        mesh.add_node_set("high", vec![NR]).unwrap();
        mesh.add_node_set("hub", vec![6]).unwrap();
        for q in LOADED {
            mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
        }
        mesh
    }

    fn met(ok: bool) -> &'static str {
        if ok {
            "met"
        } else {
            "MISSED"
        }
    }

    fn nhc() -> HyperelasticEnergy {
        HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
    }

    pub fn hyper() -> Material {
        Material::hyperelastic(nhc(), 1000.0).unwrap()
    }

    /// The isotropic twin of the energy: `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)`, `s = 3K + G` (ADR 0584 D2).
    fn twin() -> Material {
        let (g, k) = mat_hyper::small_strain_moduli(&nhc());
        let s = 3.0 * k + g;
        Material::isotropic_elastic(9.0 * k * g / s, (3.0 * k - 2.0 * g) / (2.0 * s), 1000.0, 0.0).unwrap()
    }

    /// `max |lambda_h / lambda_t - 1|` over `k = 0, 1`, 4 eigenvalues each (the measure of H-T).
    fn against_the_twin(name: &str, h: &SolidModel, t: &SolidModel) -> f64 {
        let mut worst = 0.0f64;
        for k in 0..=1 {
            let a = modal_analysis_harmonic(h, &[], k, &modal_options(4)).unwrap();
            let b = modal_analysis_harmonic(t, &[], k, &modal_options(4)).unwrap();
            assert_eq!(a.eigenvalues.len(), b.eigenvalues.len());
            for (x, y) in a.eigenvalues.iter().zip(&b.eigenvalues) {
                worst = worst.max((x / y - 1.0).abs());
            }
            println!(
                "{name} k = {k}: frequencies {:?} / twin {:?}",
                a.frequencies, b.frequencies
            );
        }
        worst
    }

    /// `ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path` of `sa_cyclic_beams_default_adr0744.rs` (ADR 0744) restated by
    /// ADR 0746: H-D as acceptance (A-W), AB-a and H-T as they are.
    #[test]
    fn ab_a_h_d_h_t_after_adr0746() {
        let mut fails = Vec::new();
        let mut hold = |name: &str, ok: bool, what: String| {
            println!("{name}: {} ({what})", met(ok));
            if !ok {
                fails.push(name.to_string());
            }
        };
        // AB-a: the model of A-B (Beam B of ADR 0584 D3, hyperelastic with the setting; its set-up copied).
        let n = 10;
        let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
        let mut mesh = SolidMesh::new(nodes).unwrap();
        let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
        mesh.add_block(ElementKind::Beam2, conn).unwrap();
        let section = beam::rectangle_section(0.05, 0.03, 2.6e-7, 5.0 / 6.0).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
            .unwrap();
        mesh.add_element_set("all", (0..n).collect()).unwrap();
        mesh.add_node_set("x0", vec![0]).unwrap();
        mesh.add_node_set("xl", vec![n]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", hyper()).unwrap();
        m.set_hyperelastic_structures(true);
        for c in 0..6 {
            m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
        m.add_constraint(Constraint::cyclic_with(
            "x0",
            "xl",
            [0.0, 0.0, 1.0],
            [0.0; 3],
            N,
            default_options(),
        ))
        .unwrap();
        let mut o = ModalOptions::new(3);
        o.residual_test = ResidualTest::Backward;
        let r = modal_analysis_harmonic(&m, &[], 0, &o).map(|_| ());
        let beam_refusal = matches!(&r, Err(FemError::NotYetSupported { package, .. }) if *package == beam::FOLLOW_UP);
        hold("AB-a not the beam refusal", !beam_refusal, format!("{r:?}"));
        hold(
            "AB-a refused by the boundary match",
            matches!(&r, Err(FemError::InvalidValue { key: "cyclic", .. })),
            format!("{r:?}"),
        );
        // A-W (H-D as acceptance): W, hyperelastic, by default, against the twin.
        let h = sector_by_default(hyper());
        let t = sector_by_default(twin());
        let worst = against_the_twin("A-W", &h, &t);
        hold(
            "A-W by default max |lambda_h / lambda_t - 1| (limit <= 1e-10)",
            worst.is_finite() && worst <= 1e-10,
            format!("{worst:.4e}"),
        );
        // H-T: W, hyperelastic with the setting, against the twin.
        let h = sector_of(default_options(), hyper(), true);
        let t = sector_of(default_options(), twin(), false);
        let worst = against_the_twin("H-T", &h, &t);
        hold(
            "H-T max |lambda_h / lambda_t - 1| (limit <= 1e-10)",
            worst.is_finite() && worst <= 1e-10,
            format!("{worst:.4e}"),
        );
        assert!(fails.is_empty(), "{fails:?}");
    }
}
