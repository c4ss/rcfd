//! F-846 (ADR 0727): what switching the default local mode update of the Incompatible Hex8 to the warm start of ADR 0660 D1
//! would move, the in-process part (instrument B). The incompatible-element cases of the two default bit records
//! (`si_eas_hg_default_bits.rs`, `si_eas_hg3_bits.rs`, copied: existing tests are not edited) are built twice, the default and
//! a copy with `set_incompatible_mode_warm_start(true)`, and compared bit by bit; the default copies reproduce the records'
//! digests (DF). The committed element states with their nine modes are taken through the two save and restart paths: the
//! transient integrator's bytes and restore, and the static driver's `solve_increment` from committed states. No default is
//! changed. The census of every driver solve of the 34 test binaries of the element under the switched default (instrument A)
//! is made on scratch copies and reported in the ADR.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{initial_states, internal_force, solve, solve_increment, tangent, NewtonOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;
/// The registered tolerance of a relative change (ADR 0727 reading c5).
const T: f64 = 1e-6;
/// The nine modes of an Incompatible Hex8.
const MODES: usize = 9;

// ------------------------------------------------------------------------------------------------------------- checks

struct Checks {
    missed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { missed: Vec::new() }
    }
    fn row(&mut self, name: &str, ok: bool, text: String) {
        println!("{} {name}: {text}", if ok { "met   " } else { "MISSED" });
        if !ok {
            self.missed.push(format!("{name}: {text}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool) {
        self.row(name, ok, format!("{ok}"));
    }
    fn at_most(&mut self, name: &str, x: f64, limit: f64) {
        self.row(name, x.is_finite() && x <= limit, format!("{x:e} <= {limit:e}"));
    }
    fn above(&mut self, name: &str, x: f64, limit: f64) {
        self.row(name, x.is_finite() && x > limit, format!("{x:e} > {limit:e}"));
    }
    fn report(&mut self, name: &str, x: f64) {
        println!(
            "report {name}: {x:e}{}",
            if x <= T { " (within T)" } else { " (ABOVE T)" }
        );
    }
    fn finish(self) {
        assert!(self.missed.is_empty(), "missed rows: {:#?}", self.missed);
    }
}

/// Values whose bits differ (`f64::INFINITY` for different lengths).
fn bits(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return f64::INFINITY;
    }
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count() as f64
}

/// `|b - a|_inf / |a|_inf` (0 for two zero vectors; infinite for different lengths or `a = 0 != b`).
fn rel(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return f64::INFINITY;
    }
    let scale = a.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let diff = a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    if diff == 0.0 {
        0.0
    } else if scale == 0.0 {
        f64::INFINITY
    } else {
        diff / scale
    }
}

fn flat(states: &[ElementState]) -> Vec<f64> {
    states.iter().flat_map(|s| s.values().iter().copied()).collect()
}

// ---------------------------------------------------------------------------------------- copies of the two bit records

/// FNV-1a 64 over the little-endian bytes of the bits of each value (as both records).
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// SplitMix64 from a seed.
struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 1.2e-5).unwrap()
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
    mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    mesh
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

const MU: f64 = 1.0e6;

fn neo_hookean(nu: f64) -> Material {
    let kappa = 2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa).unwrap(), 1000.0).unwrap()
}

/// The c1 patch of ADR 0614 on `n^3` Hex8 (seed `0xF685_0614`), Incompatible; `opted` as `si_eas_hg3_bits.rs`.
fn patch(n: usize, nu: f64, opted: bool, warm: bool) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng(0xF685_0614);
    let h = 1.0 / n as f64;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), INC));
    if opted {
        m.set_incompatible_stabilisation(true);
    }
    m.set_incompatible_mode_warm_start(warm);
    m.add_material("all", neo_hookean(nu)).unwrap();
    m
}

fn s11(mat: &Material, t: f64, lambda: f64) -> f64 {
    let e = [
        0.5 * (t * t - 1.0),
        0.5 * (t * t - 1.0),
        0.5 * (lambda * lambda - 1.0),
        0.0,
        0.0,
        0.0,
    ];
    mat.stress(&e, &PointContext::default(), &[]).unwrap().0[0]
}

fn lateral(mat: &Material, lambda: f64) -> f64 {
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if s11(mat, mid, lambda) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// `u = (F - I) X + beta (x z, y z, 0)` with `F = diag(t, t, lambda)`.
fn field(m: &SolidModel, t: f64, lambda: f64, beta: f64) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        let v = [
            (t - 1.0) * x[0] + beta * x[0] * x[2],
            (t - 1.0) * x[1] + beta * x[1] * x[2],
            (lambda - 1.0) * x[2],
        ];
        for c in 0..3 {
            u[dofs.equation(n, c).unwrap()] = v[c];
        }
    }
    u
}

/// The ADR 0608 Q1 block (seed `0xF596_0608`), as `si_eas_hg_default_bits.rs`.
fn block(warm: bool) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [3, 3, 3]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng(0xF596_0608);
    let h = 1.0 / 3.0;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), INC));
    m.set_incompatible_mode_warm_start(warm);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    for (c, v) in [0.3, 0.0, -0.1].into_iter().enumerate() {
        m.add_constraint(Constraint::displacement("zmax", c, v)).unwrap();
    }
    m
}

/// The CB2 cantilever `n x 1 x 1` at `alpha`; `opted` as `si_eas_hg3_bits.rs`.
fn cantilever(n: usize, alpha: f64, opted: bool, warm: bool) -> SolidModel {
    let (l, h, e) = (1.0, 0.02, 2.0e11);
    let mut m = SolidModel::new(with_formulation(hex_box([0.0; 3], [l, h, h], [n, 1, 1]).unwrap(), INC));
    if opted {
        m.set_incompatible_stabilisation(true);
    }
    m.set_incompatible_mode_warm_start(warm);
    m.add_material("all", Material::isotropic_elastic(e, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let p = alpha * e * (h * h.powi(3) / 12.0) / (l * l);
    m.add_load(Load::traction("xmax", [0.0, p / (h * h), 0.0])).unwrap();
    m
}

/// The ADR 0608 NT1 bar `4 x 1 x 1` (steel, tip load `1e8` in `y`), as `si_eas_hg_default_bits.rs`.
fn bar(warm: bool) -> (SolidModel, Vec<LoadHistory>) {
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(),
        INC,
    ));
    m.set_incompatible_mode_warm_start(warm);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let n = m.dof_map().n_equations();
    let mut pattern = vec![0.0; n];
    for &nd in m.mesh().node_set("xmax").unwrap() {
        pattern[m.dof_map().equation(nd, 1).unwrap()] = 1.0e8;
    }
    let loads = vec![LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())];
    (m, loads)
}

fn integrator(m: &SolidModel, loads: &[LoadHistory]) -> Integrator {
    Integrator::new(m, loads, &NlTransientOptions::new(1e-5).threads(4)).unwrap()
}

fn started(m: &SolidModel, loads: &[LoadHistory]) -> Integrator {
    let n = m.dof_map().n_equations();
    let mut it = integrator(m, loads);
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it
}

// -------------------------------------------------------------------------------------------------------- the records

/// The tangent (values and pattern) and the internal force at `u` from the initial states.
fn tangent_and_force(m: &SolidModel, u: &[f64]) -> Vec<f64> {
    let states = initial_states(m).unwrap();
    let k = tangent(m, u, &states, Some(4)).unwrap();
    let mut v = k.val.clone();
    v.extend(k.row_ptr.iter().map(|&p| p as f64));
    v.extend(k.col.iter().map(|&p| p as f64));
    v.extend(internal_force(m, u, &states).unwrap());
    v
}

/// One driver solve: the digest vector of the records (`u` then every increment's residuals), and the groups compared.
struct Solved {
    record: Vec<f64>,
    u: Vec<f64>,
    reactions: Vec<f64>,
    residuals: Vec<Vec<f64>>,
    halvings: Vec<usize>,
    states: Vec<ElementState>,
}

fn solved(m: &SolidModel, steps: usize) -> Solved {
    let r = solve(m, &[], &[], &NewtonOptions::new(steps).threads(4)).unwrap();
    let mut record = r.result.displacement_values().to_vec();
    for s in &r.steps {
        record.extend_from_slice(&s.residuals);
    }
    Solved {
        record,
        u: r.result.displacement_values().to_vec(),
        reactions: r.result.reactions().map(|x| x.to_vec()).unwrap_or_default(),
        residuals: r.steps.iter().map(|s| s.residuals.clone()).collect(),
        halvings: r.steps.iter().map(|s| s.halvings).collect(),
        states: r.states,
    }
}

/// WB, Z2, WL for one solve of the default `d` and the warm `w`.
fn compare_solves(ch: &mut Checks, name: &str, d: &Solved, w: &Solved) -> f64 {
    let counts = |s: &Solved| s.residuals.iter().map(|r| r.len()).collect::<Vec<_>>();
    println!(
        "{name}: default increments {}, halvings {:?}, residual counts {:?}; warm increments {}, halvings {:?}, residual \
         counts {:?}",
        d.residuals.len(),
        d.halvings,
        counts(d),
        w.residuals.len(),
        w.halvings,
        counts(w)
    );
    ch.holds(
        &format!("WB {name}: same increments and halvings (estimate)"),
        d.halvings == w.halvings,
    );
    println!(
        "report WB {name}: iterations default {}, warm {}",
        counts(d).iter().map(|c| c - 1).sum::<usize>(),
        counts(w).iter().map(|c| c - 1).sum::<usize>()
    );
    let bu = bits(&d.u, &w.u);
    let br = bits(&d.reactions, &w.reactions);
    println!(
        "report WB {name}: values differing: u {bu} of {}, reactions {br} of {}",
        d.u.len(),
        d.reactions.len()
    );
    ch.report(&format!("WB {name}: largest relative change of u"), rel(&d.u, &w.u));
    ch.report(
        &format!("WB {name}: largest relative change of the reactions"),
        rel(&d.reactions, &w.reactions),
    );
    ch.at_most(
        &format!("Z2 {name}: first increment's residuals, bits differing"),
        bits(&d.residuals[0], &w.residuals[0]),
        0.0,
    );
    let wl = d.states.iter().all(|s| s.values().is_empty()) && w.states.iter().all(|s| s.values().len() == MODES);
    ch.holds(
        &format!("WL {name}: default states 0 values, warm states 9 values (state_size 0)"),
        wl,
    );
    bu
}

#[test]
/// DF, Z1, Z2, NCW, WB, WL on the incompatible-element cases of the two default bit records.
fn f846_default_records_with_the_warm_start() {
    let mut ch = Checks::new();
    let df = |ch: &mut Checks, name: &str, v: &[f64], expected: u64| {
        let d = digest(v);
        ch.holds(
            &format!("DF {name}: default digest 0x{d:016x} equals the record's 0x{expected:016x}"),
            d == expected,
        );
    };
    // si_eas_hg_default_bits: the c1 patch, Incompatible, tangent and force at 0.6.
    {
        let (d, w) = (patch(6, 0.3, false, false), patch(6, 0.3, false, true));
        let t = lateral(&neo_hookean(0.3), 0.6);
        let (vd, vw) = (
            tangent_and_force(&d, &field(&d, t, 0.6, 0.0)),
            tangent_and_force(&w, &field(&w, t, 0.6, 0.0)),
        );
        df(
            &mut ch,
            "c1 patch Incompatible tangent at 0.6",
            &vd,
            0xef09_d43e_801b_008b,
        );
        ch.at_most("Z1 c1 patch tangent and force: values differing", bits(&vd, &vw), 0.0);
    }
    // si_eas_hg3_bits: the opted patches, tangent and force.
    let t = lateral(&neo_hookean(0.3), 0.6);
    for (name, n, nu, t, lambda, expected) in [
        (
            "opted 6^3 patch nu 0.3 U30 0.6 tangent and force",
            6,
            0.3,
            t,
            0.6,
            0x8dfb_6def_f559_f6c5,
        ),
        (
            "opted 4^3 patch nu 0.49 C49 0.5 tangent and force",
            4,
            0.49,
            1.0,
            0.5,
            0xd81a_4d0c_aab5_435b,
        ),
    ] {
        let (d, w) = (patch(n, nu, true, false), patch(n, nu, true, true));
        let (vd, vw) = (
            tangent_and_force(&d, &field(&d, t, lambda, 0.0)),
            tangent_and_force(&w, &field(&w, t, lambda, 0.0)),
        );
        df(&mut ch, name, &vd, expected);
        ch.at_most(&format!("Z1 {name}: values differing"), bits(&vd, &vw), 0.0);
    }
    // The solves.
    let mut ncw = f64::NAN;
    for (name, steps, opted, expected) in [
        ("ADR 0608 Q1 block Incompatible solve", 4, None, 0x13e4_b296_f155_b725),
        (
            "ADR 0608 CB2 cantilever 25 x 1 x 1 alpha 1 solve",
            5,
            Some(false),
            0x55fb_06a7_1181_49e4,
        ),
        (
            "opted CB2 cantilever 25 x 1 x 1 alpha 1 solve",
            5,
            Some(true),
            0xf8f3_0439_1236_9867,
        ),
    ] {
        let build = |warm: bool| match opted {
            None => block(warm),
            Some(o) => cantilever(25, 1.0, o, warm),
        };
        let (d, w) = (solved(&build(false), steps), solved(&build(true), steps));
        df(&mut ch, name, &d.record, expected);
        let bu = compare_solves(&mut ch, name, &d, &w);
        if opted == Some(false) {
            ncw = bu;
        }
    }
    ch.above("NCW CB2 cantilever: values of u differing", ncw, 0.0);
    // The NT1 bar on the nonlinear transient path, 3 steps.
    {
        let record = |warm: bool| {
            let (m, loads) = bar(warm);
            let mut it = started(&m, &loads);
            let reports = it.run(3).unwrap();
            let mut v = it.displacement().unwrap();
            v.push(it.strain_energy().unwrap());
            (v, reports, it.state().unwrap())
        };
        let ((vd, rd, sd), (vw, rw, sw)) = (record(false), record(true));
        df(
            &mut ch,
            "ADR 0608 NT1 bar nl_transient Incompatible",
            &vd,
            0xf143_bd47_2aac_31d3,
        );
        println!("NT1 bar: default reports {rd:?}; warm {rw:?}");
        ch.holds(
            "WB NT1 bar: same halvings per step (estimate)",
            rd.iter().map(|r| r.halvings).eq(rw.iter().map(|r| r.halvings)),
        );
        println!(
            "report WB NT1 bar: values differing: u and W_s {} of {}, v {}, a {}",
            bits(&vd, &vw),
            vd.len(),
            bits(&sd.v, &sw.v),
            bits(&sd.a, &sw.a)
        );
        ch.report("WB NT1 bar: largest relative change of u", rel(&sd.u, &sw.u));
        ch.report("WB NT1 bar: largest relative change of v", rel(&sd.v, &sw.v));
        ch.report("WB NT1 bar: largest relative change of a", rel(&sd.a, &sw.a));
        ch.report(
            "WB NT1 bar: relative change of the strain energy",
            rel(&vd[vd.len() - 1..], &vw[vw.len() - 1..]),
        );
        ch.holds(
            "WL NT1 bar: default states 0 values, warm states 9 values",
            sd.states.iter().all(|s| s.values().is_empty()) && sw.states.iter().all(|s| s.values().len() == MODES),
        );
    }
    // si_eas_hg3_bits: the opted 4^3 patch E state, strain energy and internal force.
    {
        let state = |warm: bool| {
            let m = patch(4, 0.3, true, warm);
            let u = field(&m, 1.0, 0.6, 0.2);
            let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(1e-5).threads(4)).unwrap();
            it.start(0.0, &u, &vec![0.0; u.len()]).unwrap();
            let w = it.strain_energy().unwrap();
            let f = internal_force(&m, &u, &initial_states(&m).unwrap()).unwrap();
            (w, f)
        };
        let ((wd, fd), (ww, fw)) = (state(false), state(true));
        df(
            &mut ch,
            "opted 4^3 patch E state strain_energy",
            &[wd],
            0xaebe_4e6e_424f_4544,
        );
        df(
            &mut ch,
            "opted 4^3 patch E state internal_force",
            &fd,
            0x5579_aa4f_2c6e_bb58,
        );
        ch.at_most(
            "Z1 opted 4^3 patch E state: strain energy and internal force, values differing",
            bits(&[wd], &[ww]) + bits(&fd, &fw),
            0.0,
        );
    }
    ch.finish();
}

// -------------------------------------------------------------------------------------- the transient save and restart

#[test]
/// R1, R2, R3, NCR1, NCR2, RM: the NT1 bar's warm state through the bytes and the restore.
fn f846_transient_save_and_restart() {
    let mut ch = Checks::new();
    let (mw, loads) = bar(true);
    let (md, _) = bar(false);
    // The reference: 6 warm steps uninterrupted.
    let mut reference = started(&mw, &loads);
    reference.run(6).unwrap();
    let r = reference.state().unwrap();
    // 3 warm steps, the state through the bytes.
    let mut first = started(&mw, &loads);
    first.run(3).unwrap();
    let saved = first.state().unwrap();
    ch.holds(
        "R1 the warm state at step 3: every element has 9 values",
        saved.states.iter().all(|s| s.values().len() == MODES),
    );
    let bytes = saved.to_bytes();
    let back = NlTransientState::from_bytes(&bytes).unwrap();
    let r2 = bits(&saved.u, &back.u)
        + bits(&saved.v, &back.v)
        + bits(&saved.a, &back.a)
        + bits(&saved.internal_force, &back.internal_force)
        + bits(&flat(&saved.states), &flat(&back.states));
    println!("R2: {} bytes", bytes.len());
    ch.at_most(
        "R2 the state through to_bytes and from_bytes: values differing",
        r2,
        0.0,
    );
    // The restore into a fresh warm integrator, 3 more steps.
    let state_bits = |a: &NlTransientState, b: &NlTransientState| {
        bits(&a.u, &b.u)
            + bits(&a.v, &b.v)
            + bits(&a.a, &b.a)
            + bits(&a.internal_force, &b.internal_force)
            + bits(&flat(&a.states), &flat(&b.states))
    };
    let mut restarted = integrator(&mw, &loads);
    restarted.restore(back).unwrap();
    restarted.run(3).unwrap();
    let r3 = restarted.state().unwrap();
    ch.at_most(
        "R3 after the restore and 3 steps against the uninterrupted 6: values differing",
        state_bits(&r, &r3),
        0.0,
    );
    // NCR1: the default integrator refuses the warm state.
    let mut default = integrator(&md, &loads);
    let refusal = default.restore(saved.clone());
    println!("NCR1: {refusal:?}");
    ch.holds("NCR1 the default integrator refuses the warm state", refusal.is_err());
    // NCR2: the warm state with its modes cut off (state_size 0: no values left).
    let mut cut = saved.clone();
    cut.states = cut.states.iter().map(|_| ElementState::new(Vec::new())).collect();
    let mut no_modes = integrator(&mw, &loads);
    let accepted = no_modes.restore(cut).is_ok();
    ch.holds("NCR2 the warm integrator accepts the state without modes", accepted);
    let ncr2 = if accepted && no_modes.run(3).is_ok() {
        let s = no_modes.state().unwrap();
        bits(&r3.u, &s.u) + bits(&r3.v, &s.v) + bits(&r3.a, &s.a)
    } else {
        f64::NAN
    };
    ch.above("NCR2 u, v, a differing from R3's", ncr2, 0.0);
    // RM: the default state at step 3 (no modes) restored into a warm integrator.
    let mut d3 = started(&md, &loads);
    d3.run(3).unwrap();
    let mut migrated = integrator(&mw, &loads);
    let accepted = migrated.restore(d3.state().unwrap()).is_ok();
    ch.holds("RM the warm integrator accepts the default state", accepted);
    if accepted && migrated.run(3).is_ok() {
        let s = migrated.state().unwrap();
        println!(
            "report RM: values differing from the warm reference: u {}, v {}, a {}; states with 9 values {} of {}",
            bits(&r.u, &s.u),
            bits(&r.v, &s.v),
            bits(&r.a, &s.a),
            s.states.iter().filter(|x| x.values().len() == MODES).count(),
            s.states.len()
        );
        ch.report("RM largest relative change of u", rel(&r.u, &s.u));
        ch.report("RM largest relative change of v", rel(&r.v, &s.v));
        ch.report("RM largest relative change of a", rel(&r.a, &s.a));
    } else {
        ch.holds("RM the migrated run of 3 steps", false);
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------- the static driver restart

/// The chain of `solve_increment` from `(u, lc, states)` over the load factors `lfs`; the states after each link.
fn chain(
    m: &SolidModel,
    u: Vec<f64>,
    lc: f64,
    states: Vec<ElementState>,
    lfs: &[f64],
) -> Result<(Vec<f64>, Vec<Vec<ElementState>>), String> {
    let n = m.dof_map().n_equations();
    let (mut u, mut lc, mut states) = (u, lc, states);
    let mut links = Vec::new();
    for &lt in lfs {
        let eq = solve_increment(
            m,
            &[],
            &vec![0.0; n],
            &u,
            &states,
            lc,
            lt,
            &NewtonOptions::new(4).threads(4),
            &[],
        )
        .map_err(|e| e.to_string())?;
        u = eq.u;
        states = eq.states;
        lc = lt;
        links.push(states.clone());
    }
    Ok((u, links))
}

/// The displacement at load factor 0 (the prescribed equations carry `0 v`, with the sign of `v`).
fn start_of(m: &SolidModel) -> Vec<f64> {
    let mut u = vec![0.0; m.dof_map().n_equations()];
    for (eq, v) in prescribed_equations(m, &[]).unwrap() {
        u[eq] = 0.0 * v;
    }
    u
}

#[test]
/// S1, S2, NCS1, NCS2, SM: the Q1 block's warm states through the static driver's restart from committed states.
fn f846_static_restart() {
    let mut ch = Checks::new();
    let (mw, md) = (block(true), block(false));
    let reference = solve(&mw, &[], &[], &NewtonOptions::new(4).threads(4)).unwrap();
    let lfs: Vec<f64> = reference.steps.iter().map(|s| s.load_factor).collect();
    println!("reference: load factors {lfs:?}");
    let (u, links) = chain(&mw, start_of(&mw), 0.0, initial_states(&mw).unwrap(), &lfs).unwrap();
    let last = links.last().unwrap();
    ch.at_most(
        "S1 the chain of solve_increment against solve: u and states, values differing",
        bits(reference.result.displacement_values(), &u) + bits(&flat(&reference.states), &flat(last)),
        0.0,
    );
    ch.holds(
        "S2 every element's state after every link has 9 values",
        links.iter().all(|l| l.iter().all(|s| s.values().len() == MODES)),
    );
    // The committed displacement at the first link, for the restarts at the second.
    let (u1, l1) = chain(&mw, start_of(&mw), 0.0, initial_states(&mw).unwrap(), &lfs[..1]).unwrap();
    // NCS1: the default model given the warm states.
    let n = md.dof_map().n_equations();
    let refusal = solve_increment(
        &md,
        &[],
        &vec![0.0; n],
        &u1,
        &l1[0],
        lfs[0],
        lfs[1],
        &NewtonOptions::new(4).threads(4),
        &[],
    );
    let text = refusal.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    println!("NCS1: {text}");
    ch.holds(
        "NCS1 the default model refuses the warm states (key \"state\")",
        refusal.is_err() && text.contains("state"),
    );
    // NCS2: the warm chain with the modes cut off at the second link.
    let cut: Vec<ElementState> = l1[0].iter().map(|_| ElementState::new(Vec::new())).collect();
    let ncs2 = match chain(&mw, u1.clone(), lfs[0], cut, &lfs[1..]) {
        Ok((uc, _)) => bits(&u, &uc),
        Err(e) => {
            println!("NCS2: {e}");
            f64::NAN
        }
    };
    ch.above("NCS2 final u differing from S1's", ncs2, 0.0);
    // SM: the default to the first link, then the warm model from its states (no modes).
    let (ud, ld) = chain(&md, start_of(&md), 0.0, initial_states(&md).unwrap(), &lfs[..1]).unwrap();
    match chain(&mw, ud, lfs[0], ld[0].clone(), &lfs[1..]) {
        Ok((um, lm)) => {
            ch.holds("SM the warm model accepts the default states", true);
            println!(
                "report SM: final u values differing from the warm reference {} of {}; states with 9 values {} of {}",
                bits(&u, &um),
                u.len(),
                lm.last().unwrap().iter().filter(|s| s.values().len() == MODES).count(),
                lm.last().unwrap().len()
            );
            ch.report("SM largest relative change of u", rel(&u, &um));
        }
        Err(e) => ch.row("SM the warm model accepts the default states", false, e),
    }
    ch.finish();
}
