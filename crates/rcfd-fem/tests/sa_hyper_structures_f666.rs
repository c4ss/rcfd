//! F-666 (ADR 0593): hyperelastic Beam2, Shell4 (and Shell3) elements on the cyclic, rotating, cms and mpc-transformation
//! paths, opted in by `SolidModel::set_hyperelastic_structures(true)`, against the isotropic twin of the same small-strain
//! moduli. Checks CY, A-B, RO, CM, MP, the extras X, the controls C, N1, N2 of the registration; set-ups of ADR 0593 D3. Every
//! value is printed; a non-finite value, an error or a loss of stiffness is a miss.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::cms::{reduce, CmsOptions};
use rcfd_fem::cyclic::{modal_analysis_harmonic, solve_static_harmonic, HarmonicLoad, HarmonicModes};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_hyper::{self, LINEAR_FOLLOW_UP};
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc, ReferenceLoad};
use rcfd_fem::rotating::{HarmonicSpeedModes, RotatingOptions, RotatingPart, RotatingSector, SpeedModes};
use rcfd_fem::solve::StaticOptions;
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

// ----------------------------------------------------------------------------------------------- materials (D2)

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

/// The material of a row: the hyperelastic model (with the setting), the twin, or the perturbed twin (both without it).
#[derive(Clone, Copy, PartialEq)]
enum Which {
    Hyper,
    Twin,
    Perturbed,
    /// The twin with the setting on (N2).
    TwinOptedIn,
    /// The hyperelastic material without the setting (N1).
    HyperDefault,
}

fn material(w: Which) -> Material {
    match w {
        Which::Hyper | Which::HyperDefault => hyper(),
        Which::Twin | Which::TwinOptedIn => twin(),
        Which::Perturbed => twin_perturbed(),
    }
}

fn opted_in(w: Which) -> bool {
    matches!(w, Which::Hyper | Which::TwinOptedIn)
}

fn modal_opts(n: usize) -> ModalOptions {
    ModalOptions::new(n).residual_test(ResidualTest::Backward)
}

// ----------------------------------------------------------------------------------------------- set-ups (D3)

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
    m.set_hyperelastic_structures(opted_in(w));
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

/// Plate P4 (`tri = false`) or P3 of ADR 0584 D3 with the node sets `xl` (x = 1), `p1` (5, 6), `p0` (5, 0).
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
    m.set_hyperelastic_structures(opted_in(w));
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

/// The mpc constraints of D3 on Beam B: the rigid set `tip` (constraint 6) and `u_y = u_z` at node 5.
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

/// The mpc constraints of D3 on the plate: the rigid set `xl` (constraint 6) and `u_z(p1) = u_z(p0)`.
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
/// of D3.
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
    m.set_hyperelastic_structures(opted_in(w));
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

// ----------------------------------------------------------------------------------------------- measures (D4)

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
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
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

// ----------------------------------------------------------------------------------------------- the path rows

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

/// The harmonic load of D3 (`k = 1`).
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

/// `RotatingPart::modes(Omega)` of the hyperelastic model against `other` (the twin, or the perturbed twin for a control),
/// `Omega = 0.2 sqrt(lambda_1)` of `other` at rest (D3). Returns the two results when both have modes.
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

/// `solve_static_mpc` (with the reference load) and `modal_analysis_mpc`. With `control`, only the static `e_u` against
/// `CONTROL`.
fn mpc_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, control: bool) {
    let rl = [ReferenceLoad::new(6, [0.1, -0.2, 0.05], [0.01, 0.0, 0.02])];
    match (
        solve_static_mpc(h, &[], &[], &rl, &StaticOptions::default()),
        solve_static_mpc(t, &[], &[], &rl, &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => {
            let flat = |s: &rcfd_fem::mpc::MpcSolution| -> Vec<f64> {
                let mut v = s.result.displacement_values().to_vec();
                for r in &s.references {
                    v.extend(r.translation);
                    v.extend(r.rotation);
                }
                v
            };
            c.row(
                &format!("{name} static e_u"),
                rel(&flat(&a), &flat(&b)),
                if control { CONTROL } else { LIMIT },
                control,
            );
        }
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

// ----------------------------------------------------------------------------------------------- a1: cyclic

#[test]
fn cy_q4_cyclic_paths() {
    let mut c = Checks::new();
    cyclic_static(&mut c, "CY-S-Q4", ElementKind::Shell4, Which::Twin);
    cyclic_harmonic(&mut c, "CY-H-Q4", ElementKind::Shell4);
    cyclic_modes(&mut c, "CY-M-Q4", ElementKind::Shell4);
    c.finish();
}

#[test]
#[ignore = "ADR 0744: retired record, A-B asserts the Beam2 refusal that is lifted by default; restated in sa_cyclic_beams_default_adr0744.rs"]
fn a_b_beam2_cyclic_refused_for_its_own_reason() {
    let mut c = Checks::new();
    let mut m = beam_b(Which::Hyper, false);
    m.add_constraint(Constraint::cyclic_with(
        "x0",
        "xl",
        [0.0, 0.0, 1.0],
        [0.0; 3],
        N,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    let r = modal_analysis_harmonic(&m, &[], 0, &modal_opts(3));
    let ok = matches!(&r, Err(FemError::NotYetSupported { package, .. }) if *package == beam::FOLLOW_UP);
    c.holds("A-B", ok, &format!("{:?}", r.err()));
    c.finish();
}

// ----------------------------------------------------------------------------------------------- a2: rotating

/// ADR 0593 amendment 1: the record of run 1 (missed: the rotating rows refused by the unit pre-stress copy, which lost the
/// setting); body unchanged. The corrected run is `ro_rotating_part_amended`.
#[test]
#[ignore = "ADR 0593 amendment 1: run-1 record, a known miss (the rotating pre-stress copy refused the material)"]
fn ro_rotating_part_known_miss() {
    let mut c = Checks::new();
    rotating_row(
        &mut c,
        "RO-B",
        &beam_b(Which::Hyper, false),
        &beam_b(Which::Twin, false),
        false,
    );
    rotating_row(
        &mut c,
        "RO-P4",
        &plate(Which::Hyper, false, false),
        &plate(Which::Twin, false, false),
        false,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- a3: cms

#[test]
fn cm_cms_reduce() {
    let mut c = Checks::new();
    cms_row(
        &mut c,
        "CM-B",
        &beam_b(Which::Hyper, true),
        &beam_b(Which::Twin, true),
        false,
    );
    cms_row(
        &mut c,
        "CM-P4",
        &plate(Which::Hyper, false, true),
        &plate(Which::Twin, false, true),
        false,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- a4: mpc transformation

#[test]
fn mp_mpc_transformation() {
    let mut c = Checks::new();
    mpc_row(&mut c, "MP-B", &beam_mpc(Which::Hyper), &beam_mpc(Which::Twin), false);
    mpc_row(
        &mut c,
        "MP-P4",
        &plate_mpc(Which::Hyper, false),
        &plate_mpc(Which::Twin, false),
        false,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- registered extras

#[test]
fn x_cy_q3_cyclic_paths() {
    let mut c = Checks::new();
    cyclic_static(&mut c, "X-CY-S-Q3", ElementKind::Shell3, Which::Twin);
    cyclic_harmonic(&mut c, "X-CY-H-Q3", ElementKind::Shell3);
    cyclic_modes(&mut c, "X-CY-M-Q3", ElementKind::Shell3);
    c.finish();
}

/// ADR 0593 amendment 1: the record of run 1 (missed: the rotating rows refused by the unit pre-stress copy, which lost the
/// setting); body unchanged. The corrected run is `x_rs_q4_rotating_sector_amended`.
#[test]
#[ignore = "ADR 0593 amendment 1: run-1 record, a known miss (the rotating pre-stress copy refused the material)"]
fn x_rs_q4_rotating_sector_known_miss() {
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
fn x_mp_p3_mpc_transformation() {
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

// ----------------------------------------------------------------------------------------------- controls

/// ADR 0593 amendment 1: the record of run 1 (missed: the rotating rows refused by the unit pre-stress copy, which lost the
/// setting); body unchanged. The corrected run is `c_perturbed_twin_is_seen_amended`.
#[test]
#[ignore = "ADR 0593 amendment 1: run-1 record, a known miss (the rotating pre-stress copy refused the material)"]
fn c_perturbed_twin_is_seen_known_miss() {
    let mut c = Checks::new();
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

#[test]
fn n1_refused_without_the_setting() {
    let mut c = Checks::new();
    let what = "a structural element";
    let r = modal_analysis_harmonic(
        &sector(Which::HyperDefault, ElementKind::Shell4, false),
        &[],
        0,
        &modal_opts(3),
    );
    c.holds("N1 cyclic Q4", linear_refusal(&r, what), &format!("{:?}", r.err()));
    let b = beam_b(Which::HyperDefault, false);
    let r = RotatingPart::new(&b, [0.0, 0.0, 1.0], [0.0; 3], &RotatingOptions::new(6));
    c.holds(
        "N1 rotating B",
        linear_refusal(&r, what),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = reduce(&beam_b(Which::HyperDefault, true), "xl", &CmsOptions::new(4));
    c.holds("N1 cms B", linear_refusal(&r, what), &format!("{:?}", r.as_ref().err()));
    let r = solve_static_mpc(
        &plate_mpc(Which::HyperDefault, false),
        &[],
        &[],
        &[],
        &StaticOptions::default(),
    );
    c.holds(
        "N1 mpc P4",
        linear_refusal(&r, what),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

#[test]
fn n2_setting_on_an_isotropic_model_changes_no_bit() {
    let mut c = Checks::new();
    let on = solve_static_mpc(
        &sector(Which::TwinOptedIn, ElementKind::Shell4, true),
        &[],
        &[],
        &[],
        &StaticOptions::default(),
    )
    .unwrap();
    let off = solve_static_mpc(
        &sector(Which::Twin, ElementKind::Shell4, true),
        &[],
        &[],
        &[],
        &StaticOptions::default(),
    )
    .unwrap();
    c.holds(
        "N2 cyclic Q4 static",
        bits_equal(on.result.displacement_values(), off.result.displacement_values()),
        "displacement bits equal",
    );
    let mut dummy = Checks::new();
    let r = rotating_row(
        &mut dummy,
        "N2 rotating B (setting on vs off)",
        &beam_b(Which::TwinOptedIn, false),
        &beam_b(Which::Twin, false),
        false,
    );
    let ok = r.is_some_and(|(a, b)| bits_equal(&a.eigenvalues, &b.eigenvalues));
    c.holds("N2 rotating B", ok, "eigenvalue bits equal at Omega");
    c.finish();
}

// ----------------------------------------------------------------------------------------------- amendment 1: corrected runs

/// ADR 0593 amendment 1: RO-B, RO-P4 after D5, the body of the run-1 record.
#[test]
fn ro_rotating_part_amended() {
    ro_rotating_part_known_miss();
}

/// ADR 0593 amendment 1: X-RS-Q4 after D5, the body of the run-1 record.
#[test]
fn x_rs_q4_rotating_sector_amended() {
    x_rs_q4_rotating_sector_known_miss();
}

/// ADR 0593 amendment 1: C-CY, C-RO, C-CM, C-MP after D5, the body of the run-1 record.
#[test]
fn c_perturbed_twin_is_seen_amended() {
    c_perturbed_twin_is_seen_known_miss();
}
