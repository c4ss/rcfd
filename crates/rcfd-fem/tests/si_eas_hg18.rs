//! F-780 (ADR 0719): the driver retry with the indefinite tangent after a refused factor (`NewtonOptions::indefinite_retry`)
//! and the inertia of every accepted state (`LoadStep::inertia`, `LoadStep::retried`). The fast test (D4): the refusals, the
//! default record, the saddle path of the ADR 0628 patch (the plain element, confined compression to `F_33 = 0.93`) with the
//! default, the retry and the envelope factor, rule L on the same path, the `8^3` punch block, the column of ADR 0694 (a
//! failure that is not a refused factor), a retry that also fails. The heavy record: the `16^3` punch block of ADR 0694 with
//! the default, the indefinite tangent and the retry. The helpers are copies of those of `si_eas_hg14.rs` (existing tests
//! are not edited). Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions};
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::contact::{self, ContactOptions};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{
    initial_states, solve, solve_from, solve_in_time, solve_increment, solve_with, tangent, Contribution,
    DriverExtensions, LoadStep, NewtonOptions, NonlinearResult, StepControl, TimeSteppingOptions,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::skyline::PivotRule;
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeSet;
use std::time::Instant;

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
    fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;
const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;

fn nh(mu: f64) -> Material {
    Material::hyperelastic(
        HyperelasticEnergy::neo_hookean(mu, 2.0 * mu * (1.0 + NU) / (3.0 * (1.0 - 2.0 * NU))).unwrap(),
        1000.0,
    )
    .unwrap()
}

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

/// The options of si_eas_hg14: four threads, the record of the active set, the supernodal factor.
fn options(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps)
        .threads(4)
        .record_active_set(true)
        .factor(SUPERNODAL)
}

fn key_of<T>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn fmt_records(steps: &[LoadStep]) -> String {
    steps
        .iter()
        .map(|s| {
            format!(
                "{:.6} (step {}, halvings {}, {} iterations, retried {}, inertia {:?})",
                s.load_factor,
                s.step,
                s.halvings,
                s.residuals.len() - 1,
                s.retried,
                s.inertia
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

// ============================================================================ the free tangent and its inertia

/// SY: the symmetric part `(K + K^T) / 2`.
fn symmetrise(a: &CsrMatrix) -> CsrMatrix {
    let mut t = Vec::with_capacity(2 * a.val.len());
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[p];
            t.push((r, c, 0.5 * a.val[p]));
            t.push((c, r, 0.5 * a.val[p]));
        }
    }
    CsrMatrix::from_triplets(a.n, t)
}

/// The inertia: the number of negative pivots of the supernodal `L D L^T` with the `NonZero` rule (Sylvester); `None` if
/// the factor refuses a pivot.
fn inertia(a: &CsrMatrix) -> Option<usize> {
    SupernodalLdlt::factor_with(
        a,
        &SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        },
    )
    .ok()
    .map(|f| f.inertia().0)
}

/// The test's own inertia of a state: the reduced tangent (the model's constraints and `pres`), symmetrised.
fn inertia_at(m: &SolidModel, pres: &[Prescribed], u: &[f64], states: &[ElementState]) -> Option<usize> {
    let k = tangent(m, u, states, Some(4)).unwrap();
    let fixed = prescribed_equations(m, pres).unwrap();
    let zero: Vec<(usize, f64)> = fixed.iter().map(|&(eq, _)| (eq, 0.0)).collect();
    let sys = reduce(&k, &vec![0.0; k.n], &zero).unwrap();
    inertia(&symmetrise(&sys.matrix))
}

// ============================================================================ the punch block (ADR 0678 reading c1)

const DELTA: f64 = 0.1;
const TOL: f64 = 1e-9;

fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
    mesh.node_set(set)
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| keep(&mesh.nodes()[n]))
        .collect()
}

fn block_mesh(n: usize) -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + TOL && p[1] <= 0.5 + TOL);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < TOL && (p[1] - 1.0).abs() < TOL);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    mesh
}

/// The block with rule L, the neo-Hookean material and the die constraints of reading c1, the punch at `-0.1`.
fn block_model(mesh: &SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh.clone(), INC));
    m.set_incompatible_stabilisation(LIN);
    m.add_material("all", nh(MU)).unwrap();
    for (set, comp, value) in [
        ("xmin", 0, 0.0),
        ("xmax", 0, 0.0),
        ("ymin", 1, 0.0),
        ("ymax", 1, 0.0),
        ("zmin", 2, 0.0),
        ("punch", 2, -DELTA),
    ] {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
    }
    m
}

/// `R = -sum_punch r_z / mu` and `w_c` of a driver result.
fn measures(m: &SolidModel, r: &NonlinearResult) -> (f64, f64) {
    let re = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    let punch = m.mesh().node_set("punch").unwrap();
    let rz: f64 = punch.iter().map(|&n| re[dofs.equation(n, 2).unwrap()]).sum();
    let corner = m.mesh().node_set("corner").unwrap()[0];
    (-rz / MU, r.result.displacement(corner)[2])
}

/// The error text of the EC1 solve of ADR 0678 (`f747-scratch/run3-heavy.log`, line 23).
const ADR0678_TEXT: &str = "solver newton: load step 10 of 10 (load factor 9.625e-1 to 9.656250000000001e-1) after 5 \
                            halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal \
                            2.8253460477227545e6)";

/// The recording hook of reading c1: returns the driver's proposed load factor unchanged and records every attempt.
#[derive(Default)]
struct Recorder {
    pending: (f64, f64),
    /// `(committed, attempted, converged)` of every attempt in order.
    log: Vec<(f64, f64, bool)>,
}

impl StepControl for Recorder {
    fn name(&self) -> &str {
        "recorder"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        self.pending = (committed, proposed);
        Ok(proposed)
    }
    fn converged(&mut self, increment: &LoadStep) {
        self.log.push((self.pending.0, increment.load_factor, true));
    }
    fn failed(&mut self, committed: f64, attempted: f64) {
        self.log.push((committed, attempted, false));
    }
}

/// A hooked solve of the block: the outcome (measures or error text), the hook's attempts, and the result of a solve that
/// passes.
struct Hooked {
    outcome: Result<(f64, f64), String>,
    attempts: Vec<(f64, f64, bool)>,
    result: Option<NonlinearResult>,
}

fn solve_hooked(m: &SolidModel, tag: &str, o: &NewtonOptions) -> Hooked {
    let mut rec = Recorder::default();
    let t0 = Instant::now();
    let out = solve_with(
        m,
        &[],
        &[],
        o,
        DriverExtensions {
            contributions: Vec::new(),
            step_control: Some(&mut rec),
        },
    );
    let secs = t0.elapsed().as_secs_f64();
    let text: Vec<String> = rec
        .log
        .iter()
        .map(|(c, a, ok)| format!("{c:.6}->{a:.6}{}", if *ok { "" } else { " x" }))
        .collect();
    println!("{tag}: hooked attempts [{}]", text.join(", "));
    let outcome = match &out {
        Ok(r) => {
            let (rz, wc) = measures(m, r);
            println!("{tag}: R/mu {rz:.8}, w_c {wc:+.8}, {secs:.1} s");
            println!("{tag}: records {}", fmt_records(&r.steps));
            Ok((rz, wc))
        }
        Err(e) => {
            println!("{tag}: solve failed: {e}; {secs:.1} s");
            Err(e.to_string())
        }
    };
    Hooked {
        outcome,
        attempts: rec.log,
        result: out.ok(),
    }
}

// ============================================================================ the ADR 0628 patch

/// SplitMix64.
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
    out
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

const HG_SEED: u64 = 0xF685_0614;

/// The ADR 0628 patch: the unit cube on `n^3` Hex8, interior nodes moved by `0.1 h (2U - 1)` (seed `0xF685_0614`),
/// `nu = 0.49`.
fn patch(n: usize, rule: IncompatibleStabilisation) -> SolidModel {
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
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), INC));
    m.set_incompatible_stabilisation(rule);
    m.add_material("all", nh(MU)).unwrap();
    m
}

/// The homogeneous field `u = (F - I) X` at every node.
fn homogeneous_f(m: &SolidModel, f: &[[f64; 3]; 3]) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            let mut v = 0.0;
            for j in 0..3 {
                v += (f[c][j] - if c == j { 1.0 } else { 0.0 }) * x[j];
            }
            u[dofs.equation(n, c).unwrap()] = v;
        }
    }
    u
}

/// The saddle path of D4: the patch, every boundary node prescribed at the homogeneous field of `F = diag(1, 1, 0.93)`
/// (scaled by the load factor), and that field.
fn saddle_path(rule: IncompatibleStabilisation) -> (SolidModel, Vec<Prescribed>, Vec<f64>) {
    let m = patch(6, rule);
    let uh = homogeneous_f(&m, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.93]]);
    let dofs = m.dof_map();
    let mut pres = Vec::new();
    for &n in &boundary_nodes(m.mesh()) {
        for c in 0..3 {
            pres.push(Prescribed::new(n, c, uh[dofs.equation(n, c).unwrap()]));
        }
    }
    (m, pres, uh)
}

fn dist_inf(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |w, (x, y)| w.max((x - y).abs()))
}

// ============================================================================ the column of ADR 0694 D4

const COLUMN_PUNCH: f64 = -1.2;

/// Two Full Hex8 on `[0, 1]^2 x [0, 2]`, `mu = 1e6` below and `99e6` above, uniaxial strain, the top at `-1.2`.
fn column_model() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 2.0], [1, 1, 2]).unwrap();
    let every: Vec<usize> = (0..mesh.nodes().len()).collect();
    mesh.add_node_set("every", every).unwrap();
    mesh.add_element_set("lower", vec![0]).unwrap();
    mesh.add_element_set("upper", vec![1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower", nh(1e6)).unwrap();
    m.add_material("upper", nh(99e6)).unwrap();
    for (set, comp, value) in [
        ("every", 0, 0.0),
        ("every", 1, 0.0),
        ("zmin", 2, 0.0),
        ("zmax", 2, COLUMN_PUNCH),
    ] {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
    }
    m
}

/// A contribution that adds nothing (the observer of si_eas_hg14 without its log).
struct Nothing;

impl Contribution for Nothing {
    fn name(&self) -> &str {
        "nothing"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        Ok(())
    }
}

const NOTE: &str = "; retried with the indefinite tangent (ADR 0719 D1) after: ";

// ============================================================================ the fast test

#[test]
fn f780_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh8 = block_mesh(8);
    let m8 = block_model(&mesh8);
    let states8 = initial_states(&m8).unwrap();
    let zero8 = vec![0.0; m8.dof_map().n_equations()];

    // RF: the eight refusals.
    let retry1 = || NewtonOptions::new(1).indefinite_retry(true);
    let mut rows: Vec<(&str, Option<&'static str>, String)> = Vec::new();
    let mut push = |tag: &'static str, key: Option<&'static str>, text: String| rows.push((tag, key, text));
    let r = solve(&m8, &[], &[], &retry1().indefinite_tangent(true));
    push("with indefinite_tangent", key_of(&r), format!("{:?}", r.err()));
    let r = solve(&m8, &[], &[], &retry1().unsymmetric(true));
    push("with unsymmetric", key_of(&r), format!("{:?}", r.err()));
    let nothing = Nothing;
    let r = solve_with(
        &m8,
        &[],
        &[],
        &retry1(),
        DriverExtensions {
            contributions: vec![&nothing],
            step_control: None,
        },
    );
    push("solve_with with a contribution", key_of(&r), format!("{:?}", r.err()));
    let r = trace(
        &m8,
        &[],
        &[],
        &ArcLengthOptions::new(0.1).newton(retry1()),
        &[],
        &mut |_| true,
    );
    push("trace", key_of(&r), format!("{:?}", r.err()));
    let r = solve_increment(&m8, &[], &zero8, &zero8, &states8, 0.0, 0.1, &retry1(), &[]);
    push("solve_increment", key_of(&r), format!("{:?}", r.err()));
    let r = solve_from(&m8, &[], &zero8, &zero8, &states8, &retry1(), &[]);
    push("solve_from", key_of(&r), format!("{:?}", r.err()));
    let r = solve_in_time(&m8, &[], &[1.0], &TimeSteppingOptions::new(1.0).newton(retry1()));
    push("solve_in_time", key_of(&r), format!("{:?}", r.err()));
    let r = contact::solve_with(&m8, &[], &[], &[], &ContactOptions::new(1, 1e-3).newton(retry1()), &[]);
    push("contact::solve_with", key_of(&r), format!("{:?}", r.err()));
    for (tag, key, text) in &rows {
        ch.holds(&format!("RF {tag}: {text}"), *key == Some("indefinite_retry"));
    }

    // DF0, RT8: the 8^3 block, default and with the retry.
    let d8 = solve_hooked(&m8, "DF0 block 8^3 default", &options(10));
    let r8 = solve_hooked(&m8, "RT8 block 8^3 retry", &options(10).indefinite_retry(true));
    match (&d8.result, &r8.result) {
        (Some(d), Some(r)) => {
            ch.holds(
                "DF0: inertia None and retried false at every record",
                d.steps.iter().all(|s| s.inertia.is_none() && !s.retried),
            );
            ch.holds(
                &format!("RT8: 10 attempts, all converged ({})", r8.attempts.len()),
                r8.attempts.len() == 10 && r8.attempts.iter().all(|a| a.2),
            );
            ch.holds(
                "RT8: u bit-equal to the default solve's",
                bits_equal(d.result.displacement_values(), r.result.displacement_values()),
            );
            ch.holds("RT8: no increment retried", r.steps.iter().all(|s| !s.retried));
            ch.holds(
                "RT8: inertia Some(0) at all ten",
                r.steps.len() == 10 && r.steps.iter().all(|s| s.inertia == Some(0)),
            );
            let (rr, ww) = r8.outcome.clone().unwrap();
            ch.at_most("RT8: |R / 0.36659596 - 1|", (rr / 0.36659596 - 1.0).abs(), 1e-6);
            ch.at_most("RT8: |w_c / 0.06631664 - 1|", (ww / 0.06631664 - 1.0).abs(), 1e-6);
        }
        _ => ch.holds("DF0, RT8: both 8^3 solves pass", false),
    }

    // SP-D, SP-R, SP-E: the saddle path.
    let (ms, pres, uh) = saddle_path(OFF);
    let h6 = 1.0 / 6.0;
    let sp_d = solve(&ms, &pres, &[], &options(10));
    let t = sp_d.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    println!(
        "SP-D: {:?}",
        sp_d.as_ref().map(|r| fmt_records(&r.steps)).map_err(|e| e.to_string())
    );
    ch.holds(
        "SP-D: the default fails (Solver, load step 8 of 10, singular tangent)",
        matches!(sp_d, Err(FemError::Solver { .. }))
            && t.contains("load step 8 of 10")
            && t.contains("singular tangent"),
    );
    let mut sp_r_record: Option<Vec<(bool, Option<usize>)>> = None;
    for (tag, factor) in [("SP-R", SUPERNODAL), ("SP-E", DirectFactor::Envelope)] {
        let o = options(10).factor(factor).indefinite_retry(true);
        let r = solve(&ms, &pres, &[], &o);
        match &r {
            Ok(res) => {
                println!("{tag}: records {}", fmt_records(&res.steps));
                let rec: Vec<(bool, Option<usize>)> = res.steps.iter().map(|s| (s.retried, s.inertia)).collect();
                let d = dist_inf(res.result.displacement_values(), &uh) / h6;
                ch.at_most(&format!("{tag}: |u - u_hom|_inf / h at full load"), d, 1e-6);
                if tag == "SP-R" {
                    ch.holds(
                        &format!("SP-R: 10 records, none halved ({})", res.steps.len()),
                        res.steps.len() == 10 && res.steps.iter().all(|s| s.halvings == 0),
                    );
                    let retried: Vec<usize> = res.steps.iter().filter(|s| s.retried).map(|s| s.step).collect();
                    ch.holds(
                        &format!("SP-R: retried exactly at 8, 9, 10 ({retried:?})"),
                        retried == vec![8, 9, 10],
                    );
                    let inertia: Vec<Option<usize>> = res.steps.iter().map(|s| s.inertia).collect();
                    let ok = inertia.len() == 10
                        && inertia[..7].iter().all(|&v| v == Some(0))
                        && inertia[7..9].iter().all(|&v| v.is_some_and(|x| x >= 1))
                        && inertia[9] == Some(21);
                    ch.holds(
                        &format!("SP-R: inertia 0 at 1..7, >= 1 at 8, 9, 21 at 10 ({inertia:?})"),
                        ok,
                    );
                    let own = inertia_at(&ms, &pres, res.result.displacement_values(), &res.states);
                    ch.holds(
                        &format!("SP-R: the test's own inertia at the result ({own:?}) equal to the record's"),
                        own.is_some() && own == inertia.last().copied().flatten(),
                    );
                    sp_r_record = Some(rec);
                } else {
                    ch.holds(
                        "SP-E: retried and inertia equal to SP-R's at every increment",
                        sp_r_record.as_ref() == Some(&rec),
                    );
                }
            }
            Err(e) => ch.holds(&format!("{tag}: the retry solves to full load ({e})"), false),
        }
    }

    // NC-L: rule L on the same path.
    let (ml, pres_l, _) = saddle_path(LIN);
    let dl = solve(&ml, &pres_l, &[], &options(10));
    let rl = solve(&ml, &pres_l, &[], &options(10).indefinite_retry(true));
    match (&dl, &rl) {
        (Ok(d), Ok(r)) => {
            println!("NC-L: records {}", fmt_records(&r.steps));
            ch.holds("NC-L: no increment retried", r.steps.iter().all(|s| !s.retried));
            ch.holds(
                "NC-L: inertia Some(0) at all ten",
                r.steps.len() == 10 && r.steps.iter().all(|s| s.inertia == Some(0)),
            );
            ch.holds(
                "NC-L: u bit-equal to the default solve's",
                bits_equal(d.result.displacement_values(), r.result.displacement_values()),
            );
        }
        _ => ch.holds("NC-L: both solves pass", false),
    }

    // NR: the column, one increment, no halving.
    let mc = column_model();
    let mut oc = NewtonOptions::new(1);
    oc.max_halvings = 0;
    let dc = solve(&mc, &[], &[], &oc)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    let rc = solve(&mc, &[], &[], &oc.clone().indefinite_retry(true))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    println!("NR: default {dc}");
    println!("NR: retry {rc}");
    ch.holds("NR: the default fails with an inversion (det F)", dc.contains("det F"));
    ch.holds(
        "NR: the retry's error text equal to the default's",
        !rc.is_empty() && rc == dc,
    );

    // RFF: the retry also fails.
    let mut of = options(1);
    of.max_halvings = 0;
    of.max_iterations = 2;
    of.residual_tolerance = 1e-300;
    let df = solve(&ms, &pres, &[], &of)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    let rf = solve(&ms, &pres, &[], &of.clone().indefinite_retry(true))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    println!("RFF: default {df}");
    println!("RFF: retry {rf}");
    let cause = df.split("after 0 halvings: ").nth(1).unwrap_or("").to_string();
    ch.holds(
        &format!("RFF: the default cause starts with 'singular tangent' ({cause})"),
        cause.starts_with("singular tangent"),
    );
    ch.holds(
        "RFF: the retry's text has 'not converged in 2 iterations'",
        rf.contains("not converged in 2 iterations"),
    );
    ch.holds(
        "RFF: the retry's text ends with the note and the default cause",
        !cause.is_empty() && rf.ends_with(&format!("{NOTE}{cause}")),
    );

    println!("f780_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy record

#[test]
#[ignore = "ADR 0719: the heavy record of the retry on the 16^3 punch block of ADR 0694 (OB16, IN16, RT16), run once with --ignored; f780_fast covers the retry and the inertia on the saddle patch, the 8^3 block and the column"]
fn f780_nh16_retry_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16);

    let ob = solve_hooked(&m, "OB16 block 16^3 default", &options(10));
    let text = ob.outcome.as_ref().err().cloned().unwrap_or_default();
    ch.holds("OB16: the error text of ADR 0678", text == ADR0678_TEXT);

    let in16 = solve_hooked(
        &m,
        "IN16 block 16^3 indefinite_tangent",
        &options(10).indefinite_tangent(true),
    );
    ch.holds("IN16: full load", in16.outcome.is_ok());

    let rt = solve_hooked(
        &m,
        "RT16 block 16^3 indefinite_retry",
        &options(10).indefinite_retry(true),
    );
    match (&rt.result, &rt.outcome) {
        (Some(r), Ok((rz, wc))) => {
            ch.holds("RT16: full load", true);
            ch.holds(
                &format!("RT16: 10 attempts, all converged ({})", rt.attempts.len()),
                rt.attempts.len() == 10 && rt.attempts.iter().all(|a| a.2),
            );
            let bit = in16
                .result
                .as_ref()
                .is_some_and(|i| bits_equal(i.result.displacement_values(), r.result.displacement_values()));
            ch.holds("RT16: u at full load bit-equal to IN16's", bit);
            ch.at_most("RT16: |R/mu / 0.33937627 - 1|", (rz / 0.33937627 - 1.0).abs(), 3e-8);
            ch.at_most("RT16: |w_c / 0.06245308 - 1|", (wc / 0.06245308 - 1.0).abs(), 1.6e-7);
            let retried: Vec<usize> = r.steps.iter().filter(|s| s.retried).map(|s| s.step).collect();
            println!("RT16: retried increments {retried:?}");
            ch.holds(
                &format!("RT16: retried at 8 and 10 ({retried:?})"),
                retried.contains(&8) && retried.contains(&10),
            );
            let inertia: Vec<Option<usize>> = r.steps.iter().map(|s| s.inertia).collect();
            let ok = inertia.len() == 10 && inertia[..9].iter().all(|&v| v == Some(0)) && inertia[9] == Some(1);
            ch.holds(&format!("RT16: inertia 0 at 1..9, 1 at 10 ({inertia:?})"), ok);
            let own = inertia_at(&m, &[], r.result.displacement_values(), &r.states);
            ch.holds(
                &format!("RT16: the test's own inertia at full load ({own:?}) = 1 = the record's"),
                own == Some(1) && inertia.last().copied().flatten() == Some(1),
            );
        }
        _ => ch.holds("RT16: the retry solves to full load", false),
    }
    println!("f780_nh16_retry_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
