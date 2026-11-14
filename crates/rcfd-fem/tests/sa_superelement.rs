//! F-553 (ADR 0536): reduced parts of ADR 0469 as superelements in an ordinary model (`rcfd_fem::cms::superelement`). The
//! checks of ADR 0536 with their registered limits; every measured number is printed. Every aggregated error must be finite.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::superelement::{SuperHarmonicOptions, SuperLoad, SuperelementModel};
use rcfd_fem::cms::{reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{generalized_eigen_dense, ResidualTest};
use rcfd_linalg::CsrMatrix;
use std::collections::HashMap;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    /// `measured <= limit` and finite.
    fn le(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured.is_finite() && measured <= limit;
        println!(
            "{id}: {measured:.3e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} (limit <= {limit:e})"));
        }
    }
    /// `measured >= limit` and finite.
    fn ge(&mut self, id: &str, measured: f64, limit: f64) {
        let ok = measured.is_finite() && measured >= limit;
        println!(
            "{id}: {measured:.3e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {measured:e} (limit >= {limit:e})"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, detail: &str) {
        println!("{id}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{id}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "{:?}", self.failed);
    }
}

/// The largest of `a` and `b`, NaN if either is NaN (so that a NaN reaches the check).
fn nan_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |s, x| nan_max(s, x.abs()))
}

fn rel_err(got: &[f64], want: &[f64]) -> f64 {
    assert_eq!(got.len(), want.len());
    got.iter().zip(want).fold(0.0, |s, (a, b)| nan_max(s, (a - b).abs())) / max_abs(want)
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

/// Which loads a set-up model carries.
#[derive(Clone, Copy, PartialEq)]
enum Case {
    /// The static loads of ADR 0536 (on the part of the model that holds them) and gravity.
    Static,
    /// The corner load of the harmonic checks (solid bar only).
    Corner,
}

// --------------------------------------------------------------------------------------------------------- set-ups

/// A solid box `[x0, x0 + len] x 0.1 x 0.08`, `nx x 2 x 2`, clamped on `xmin` when `x0 = 0`, with the loads of `case`.
fn solid(x0: f64, len: f64, nx: usize, case: Case) -> SolidModel {
    let mut mesh = hex_box([x0, 0.0, 0.0], [len, 0.1, 0.08], [nx, 2, 2]).unwrap();
    let tip = x0 + len == 1.0;
    if tip {
        let c: Vec<usize> = (0..mesh.nodes().len())
            .filter(|&i| mesh.nodes()[i] == [1.0, 0.1, 0.08])
            .collect();
        assert_eq!(c.len(), 1);
        mesh.add_node_set("corner", c).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    if x0 == 0.0 {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    match case {
        Case::Static => {
            if tip {
                m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
            }
            m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
        }
        Case::Corner => {
            if tip {
                m.add_load(Load::nodal_force("corner", [1.0, 1.0, 1.0])).unwrap();
            }
        }
    }
    m
}

/// A Beam2 line on `[x0, x0 + len]`, `n` elements, the section of ADR 0536, clamped at x = 0 when `x0 = 0`, the static loads.
fn beam_line(x0: f64, len: f64, n: usize, _case: Case) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [x0 + len * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let s = beam::rectangle_section(0.1, 0.08, 9.2e-6, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    if x0 == 0.0 {
        for c in 0..6 {
            m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
    }
    if x0 + len == 1.0 {
        m.add_load(Load::nodal_force("xl", [1.0e3, 2.0e3, -3.0e3])).unwrap();
        m.add_load(Load::nodal_moment("xl", [10.0, 20.0, -30.0])).unwrap();
    }
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// A Shell4 strip `[x0, x0 + len] x [0, 0.1]`, `nx x 2` cells, thickness 0.01, clamped on x = 0 when `x0 = 0`, the static
/// loads.
fn strip(x0: f64, len: f64, nx: usize, _case: Case) -> SolidModel {
    let ny = 2;
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([x0 + len * i as f64 / nx as f64, 0.1 * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..nx * ny).collect()).unwrap();
    mesh.add_node_set("x0", (0..=ny).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("xl", (0..=ny).map(|j| idx(nx, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    if x0 == 0.0 {
        for c in 0..6 {
            m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
        }
    }
    if x0 + len == 1.0 {
        m.add_load(Load::nodal_force("xl", [50.0, 20.0, -10.0])).unwrap();
    }
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

type Build = fn(f64, f64, usize, Case) -> SolidModel;

/// One family of set-ups: its builder, the element counts of the full model, the interfaces of the halves and the
/// registered tolerances.
struct Family {
    name: &'static str,
    build: Build,
    n_full: usize,
    iface_a: &'static str,
    iface_b: &'static str,
    static_limit: f64,
    below: f64,
    nest: f64,
}

const SOLID: Family = Family {
    name: "solid",
    build: solid,
    n_full: 16,
    iface_a: "xmax",
    iface_b: "xmin",
    static_limit: 1e-10,
    below: -1e-10,
    nest: 1e-9,
};
const BEAM: Family = Family {
    name: "beam",
    build: beam_line,
    n_full: 16,
    iface_a: "xl",
    iface_b: "x0",
    static_limit: 1e-10,
    below: -1e-10,
    nest: 1e-9,
};
const SHELL: Family = Family {
    name: "shell",
    build: strip,
    n_full: 16,
    iface_a: "xl",
    iface_b: "x0",
    static_limit: 1e-8,
    below: -1e-8,
    nest: 1e-8,
};

impl Family {
    fn full(&self, case: Case) -> SolidModel {
        (self.build)(0.0, 1.0, self.n_full, case)
    }
    fn half_a(&self, case: Case) -> SolidModel {
        (self.build)(0.0, 0.5, self.n_full / 2, case)
    }
    fn half_b(&self, case: Case) -> SolidModel {
        (self.build)(0.5, 0.5, self.n_full / 2, case)
    }
}

/// Arrangement I (A reduced, B the host) or II (A the host, B reduced).
#[derive(Clone, Copy, PartialEq)]
enum Arr {
    I,
    II,
}

impl Arr {
    fn tag(self) -> &'static str {
        match self {
            Arr::I => "I",
            Arr::II => "II",
        }
    }
}

/// The part's model, the reduced part and the host of an arrangement (the loads of `case`).
fn arrangement(f: &Family, arr: Arr, case: Case, modes: Option<usize>) -> (SolidModel, ReducedPart, SolidModel) {
    let (pm, iface, host) = match arr {
        Arr::I => (f.half_a(case), f.iface_a, f.half_b(case)),
        Arr::II => (f.half_b(case), f.iface_b, f.half_a(case)),
    };
    let m = match modes {
        Some(m) => m,
        None => reduce(&pm, iface, &CmsOptions::new(0).threads(1)).unwrap().n_interior(),
    };
    let part = reduce(&pm, iface, &CmsOptions::new(m).threads(1)).unwrap();
    (pm, part, host)
}

fn combined(host: SolidModel, part: ReducedPart) -> SuperelementModel {
    let mut s = SuperelementModel::new(host).unwrap();
    assert_eq!(s.add_part(part).unwrap(), 0);
    s
}

/// A field over the full model's equations from the host's and the part's mesh fields (matched by exact node position).
fn on_full(full: &SolidModel, pieces: &[(&SolidModel, &[f64])]) -> Vec<f64> {
    let fd = full.dof_map();
    let mut at: HashMap<[u64; 3], usize> = HashMap::new();
    for (i, p) in full.mesh().nodes().iter().enumerate() {
        at.insert([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()], i);
    }
    let mut out = vec![f64::NAN; fd.n_equations()];
    for (m, u) in pieces {
        let d = m.dof_map();
        for (i, p) in m.mesh().nodes().iter().enumerate() {
            let fi = at[&[p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]];
            assert_eq!(d.components(i), fd.components(fi));
            for c in 0..d.components(i) {
                out[fd.equation(fi, c).unwrap()] = u[d.equation(i, c).unwrap()];
            }
        }
    }
    out
}

fn modal_opts() -> ModalOptions {
    ModalOptions::new(8)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
        .threads(1)
}

fn omegas(values: &[f64]) -> Vec<f64> {
    values.iter().map(|l| l.signum() * l.abs().sqrt()).collect()
}

fn full_omegas(f: &Family) -> Vec<f64> {
    omegas(
        &modal_analysis(&f.full(Case::Static), &modal_opts())
            .unwrap()
            .eigenvalues,
    )
}

fn two_part_omegas(f: &Family) -> Vec<f64> {
    let a = reduce(&f.half_a(Case::Static), f.iface_a, &CmsOptions::new(5).threads(1)).unwrap();
    let b = reduce(&f.half_b(Case::Static), f.iface_b, &CmsOptions::new(5).threads(1)).unwrap();
    omegas(&Assembly::new(vec![a, b]).unwrap().eigen().unwrap().values)
}

// ------------------------------------------------------------------------------------------------------------- statics

fn statics(c: &mut Checks, id: &str, f: &Family, arr: Arr) {
    let full = f.full(Case::Static);
    let uf = solve_static(&full, &StaticOptions::default()).unwrap();
    let (pm, part, host) = arrangement(f, arr, Case::Static, Some(5));
    let s = combined(host.clone(), part);
    let st = s.solve_static(Some(1)).unwrap();
    let up = s.recover_part_static(0, &pm, &st.coordinates).unwrap();
    let u = on_full(&full, &[(&host, &st.host_displacement), (&pm, &up)]);
    let e = rel_err(&u, uf.displacement_values());
    c.le(
        &format!("{id} {} {} statics (n = {})", f.name, arr.tag(), s.n_unknowns()),
        e,
        f.static_limit,
    );
    if f.static_limit != 1e-10 {
        println!(
            "  record: against the acceptance number 1e-10: {}",
            if e <= 1e-10 { "below" } else { "above" }
        );
    }
}

/// ST1 to ST4 (c1, c2, c5, c8).
#[test]
fn st_statics_of_the_combined_models() {
    let mut c = Checks::new();
    statics(&mut c, "ST1", &SOLID, Arr::I);
    statics(&mut c, "ST2", &SOLID, Arr::II);
    statics(&mut c, "ST3", &BEAM, Arr::I);
    statics(&mut c, "ST3", &BEAM, Arr::II);
    statics(&mut c, "ST4", &SHELL, Arr::I);
    statics(&mut c, "ST4", &SHELL, Arr::II);
    c.finish();
}

// --------------------------------------------------------------------------------------------------------------- modal

fn modes_of(c: &mut Checks, id: &str, f: &Family, arr: Arr, wf: &[f64], w2: &[f64]) {
    let (_, part, host) = arrangement(f, arr, Case::Static, Some(5));
    let s = combined(host, part);
    let w = omegas(&s.modal_analysis(&modal_opts()).unwrap().eigenvalues);
    let err: Vec<f64> = (0..8).map(|k| w[k] / wf[k] - 1.0).collect();
    let e2: Vec<f64> = (0..3).map(|k| w2[k] / wf[k] - 1.0).collect();
    println!(
        "  {} {}: errors of the eight frequencies {err:?}; two-part {e2:?}",
        f.name,
        arr.tag()
    );
    let tag = format!("{id} {} {}", f.name, arr.tag());
    for k in 0..3 {
        c.le(&format!("{tag} f{} within 1e-3", k + 1), err[k], 1e-3);
        c.ge(&format!("{tag} f{} not below", k + 1), err[k], f.below);
        c.le(
            &format!("NB1 {} {} f{} - two-part", f.name, arr.tag(), k + 1),
            err[k] - e2[k],
            f.nest,
        );
    }
}

/// MO1 to MO4 and NB1 (c3, c6, c7, c8).
#[test]
fn mo_first_three_frequencies_of_the_combined_models() {
    let mut c = Checks::new();
    for (ids, f) in [
        (["MO1", "MO2"], &SOLID),
        (["MO3", "MO3"], &BEAM),
        (["MO4", "MO4"], &SHELL),
    ] {
        let wf = full_omegas(f);
        let w2 = two_part_omegas(f);
        println!("{}: full {wf:?}", f.name);
        modes_of(&mut c, ids[0], f, Arr::I, &wf, &w2);
        modes_of(&mut c, ids[1], f, Arr::II, &wf, &w2);
    }
    c.finish();
}

/// AM1 (c3): every fixed-interface mode kept.
#[test]
fn am1_every_mode_kept_is_exact() {
    let mut c = Checks::new();
    let wf = full_omegas(&SOLID);
    for arr in [Arr::I, Arr::II] {
        let (_, part, host) = arrangement(&SOLID, arr, Case::Static, None);
        let ni = part.n_interior();
        let s = combined(host, part);
        let w = omegas(&s.modal_analysis(&modal_opts()).unwrap().eigenvalues);
        let e = (0..8).fold(0.0, |m, k| nan_max(m, (w[k] / wf[k] - 1.0).abs()));
        c.le(&format!("AM1 {} m = n_i = {ni}", arr.tag()), e, 1e-8);
    }
    c.finish();
}

fn dense(a: &CsrMatrix) -> Vec<f64> {
    let n = a.n;
    let mut d = vec![0.0; n * n];
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            d[r * n + a.col[k]] = a.val[k];
        }
    }
    d
}

/// C1 (design): the subspace solution of the combined pencil against its dense eigen-solution.
#[test]
fn c1_subspace_against_dense() {
    let mut c = Checks::new();
    let (_, part, host) = arrangement(&SOLID, Arr::I, Case::Static, Some(5));
    let s = combined(host, part);
    let mats = s.matrices(Some(1)).unwrap();
    let n = mats.stiffness.n;
    let d = generalized_eigen_dense(&dense(&mats.stiffness), &dense(&mats.mass), n).unwrap();
    let sub = s.modal_analysis(&modal_opts()).unwrap();
    let e = (0..8).fold(0.0, |m, k| nan_max(m, (sub.eigenvalues[k] / d.values[k] - 1.0).abs()));
    c.le(&format!("C1 (n = {n})"), e, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ harmonic

/// The full bar's complex response (`DirectHarmonic`, loss 0.02 on `all`) at `omega`.
fn full_response(full: &SolidModel, dh: &DirectHarmonic, omega: f64) -> (Vec<f64>, Vec<f64>) {
    let r = dh.response(&HarmonicLoad::from_model(full).unwrap(), omega).unwrap();
    (r.re, r.im)
}

/// The harmonic solver of an arrangement with the corner load: (combined model, solver, load, part model, host).
struct HarmonicSetup {
    s: SuperelementModel,
    h: rcfd_fem::cms::superelement::SuperHarmonic,
    load: SuperLoad,
    pm: SolidModel,
    host: SolidModel,
}

fn harmonic_setup(arr: Arr, modes: Option<usize>) -> HarmonicSetup {
    let (pm, part, host) = arrangement(&SOLID, arr, Case::Static, modes);
    let load = match arr {
        Arr::I => {
            let hc = SOLID.half_b(Case::Corner);
            SuperLoad::new().host(HarmonicLoad::from_model(&hc).unwrap())
        }
        Arr::II => {
            let pc = SOLID.half_b(Case::Corner);
            let pattern = HarmonicLoad::from_model(&pc).unwrap().re().to_vec();
            let r = part.reduce_load(&pm, &pattern).unwrap();
            SuperLoad::new().part(0, HarmonicLoad::real(r).unwrap())
        }
    };
    let s = combined(host.clone(), part);
    let h = s.harmonic(&SuperHarmonicOptions::new().loss(0.02).threads(1)).unwrap();
    HarmonicSetup { s, h, load, pm, host }
}

fn harmonic_error(full: &SolidModel, dh: &DirectHarmonic, hs: &HarmonicSetup, omega: f64) -> f64 {
    let (fre, fim) = full_response(full, dh, omega);
    let r = hs.h.response(&hs.load, omega).unwrap();
    let hre = hs.s.host_displacement(&r.re);
    let him = hs.s.host_displacement(&r.im);
    let pre = hs.s.expand_part(0, &hs.pm, &r.re).unwrap();
    let pim = hs.s.expand_part(0, &hs.pm, &r.im).unwrap();
    let cre = on_full(full, &[(&hs.host, &hre), (&hs.pm, &pre)]);
    let cim = on_full(full, &[(&hs.host, &him), (&hs.pm, &pim)]);
    let mut num = 0.0;
    let mut den = 0.0;
    for e in 0..fre.len() {
        num = nan_max(num, (cre[e] - fre[e]).hypot(cim[e] - fim[e]));
        den = nan_max(den, fre[e].hypot(fim[e]));
    }
    num / den
}

fn full_harmonic() -> (SolidModel, DirectHarmonic) {
    let full = SOLID.full(Case::Corner);
    let dh = DirectHarmonic::new(
        &full,
        &DirectHarmonicOptions::new()
            .loss_factor("all", LossFactor::Constant(0.02))
            .threads(1),
    )
    .unwrap();
    (full, dh)
}

/// HA1 and HA2 (c4): every fixed-interface mode kept.
#[test]
fn ha_harmonic_every_mode_kept() {
    let mut c = Checks::new();
    let (full, dh) = full_harmonic();
    let w1 = full_omegas(&SOLID)[0];
    for arr in [Arr::I, Arr::II] {
        let hs = harmonic_setup(arr, None);
        let e = [300.0, 1000.0, 2000.0, 5000.0]
            .iter()
            .fold(0.0, |m, &w| nan_max(m, harmonic_error(&full, &dh, &hs, w)));
        c.le(&format!("HA1 {} (300, 1000, 2000, 5000 rad/s)", arr.tag()), e, 1e-8);
        c.le(
            &format!("HA2 {} at the first resonance {w1:.3}", arr.tag()),
            harmonic_error(&full, &dh, &hs, w1),
            1e-6,
        );
    }
    c.finish();
}

/// HB1 (c4): five modes, the band up to 1000 rad/s.
#[test]
fn hb1_harmonic_five_modes_band() {
    let mut c = Checks::new();
    let (full, dh) = full_harmonic();
    let wf = full_omegas(&SOLID);
    for arr in [Arr::I, Arr::II] {
        let hs = harmonic_setup(arr, Some(5));
        let mut points: Vec<f64> = (1..=30).map(|j| j as f64 * 1000.0 / 30.0).collect();
        points.extend(wf.iter().copied().filter(|&w| w < 1000.0));
        let mut worst = (0.0, 0.0);
        for &w in &points {
            let e = harmonic_error(&full, &dh, &hs, w);
            println!("  HB1 {} omega {w:.2}: {e:.3e}", arr.tag());
            if e.is_nan() || e > worst.0 || worst.0.is_nan() {
                worst = (e, w);
            }
        }
        c.le(
            &format!("HB1 {} (largest at {:.2} rad/s)", arr.tag(), worst.1),
            worst.0,
            1e-2,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ refusals

fn refused<T: std::fmt::Debug>(r: Result<T, FemError>, needle: &str) -> (bool, String) {
    match r {
        Err(e) => {
            let s = format!("{e}");
            (s.contains(needle), s)
        }
        Ok(v) => (false, format!("taken: {v:?}")),
    }
}

/// X1 to X4 (refusals).
#[test]
fn x_refusals() {
    let mut c = Checks::new();
    let part_a = reduce(&SOLID.half_a(Case::Static), "xmax", &CmsOptions::new(5).threads(1)).unwrap();
    // X1: the host's interface nodes shifted by 1e-3 in x.
    let shifted = solid(0.501, 0.499, 8, Case::Static);
    let mut s = SuperelementModel::new(shifted).unwrap();
    let (ok, msg) = refused(s.add_part(part_a.clone()), "host nodes within");
    c.holds("X1 no host node at the position", ok, &msg);
    // X2: a beam part at a solid node.
    let beam_a = reduce(&BEAM.half_a(Case::Static), "xl", &CmsOptions::new(5).threads(1)).unwrap();
    let mut cube = SolidModel::new(hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [1, 1, 1]).unwrap());
    cube.add_material("all", steel()).unwrap();
    let mut s = SuperelementModel::new(cube).unwrap();
    let (ok, msg) = refused(s.add_part(beam_a), "component 3 of node");
    c.holds("X2 a component missing on the host node", ok, &msg);
    // X3: the interface fixed in the host.
    let mut hb = SOLID.half_b(Case::Static);
    for k in 0..3 {
        hb.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let mut s = SuperelementModel::new(hb).unwrap();
    let (ok, msg) = refused(s.add_part(part_a.clone()), "is fixed or the dependent");
    c.holds("X3 a host interface unknown fixed", ok, &msg);
    // X4: a non-zero prescribed value in the host.
    let mut hb = SOLID.half_b(Case::Static);
    hb.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let (ok, msg) = refused(SuperelementModel::new(hb), "zero values only");
    c.holds("X4 a non-zero constraint value", ok, &msg);
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- design

/// Z1: the wrapper without a part against the ordinary static solve.
#[test]
fn z1_no_part_equals_the_static_solve() {
    let mut c = Checks::new();
    let mut hb = SOLID.half_b(Case::Static);
    for k in 0..3 {
        hb.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let want = solve_static(&hb, &StaticOptions::default()).unwrap();
    let s = SuperelementModel::new(hb).unwrap();
    let got = s.solve_static(Some(1)).unwrap();
    c.le("Z1", rel_err(&got.host_displacement, want.displacement_values()), 1e-10);
    c.finish();
}

/// T1: the combined matrices do not depend on the threads.
#[test]
fn t1_threads_bit_identical() {
    let mut c = Checks::new();
    let (_, part, host) = arrangement(&SOLID, Arr::I, Case::Static, Some(5));
    let s = combined(host, part);
    let a = s.matrices(Some(1)).unwrap();
    let b = s.matrices(Some(4)).unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let same = a.stiffness.row_ptr == b.stiffness.row_ptr
        && a.stiffness.col == b.stiffness.col
        && bits(&a.stiffness.val) == bits(&b.stiffness.val)
        && a.mass.col == b.mass.col
        && bits(&a.mass.val) == bits(&b.mass.val)
        && bits(&a.load) == bits(&b.load);
    c.holds("T1 threads 1 and 4", same, "K, M, f");
    c.finish();
}
