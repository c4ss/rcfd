//! F-606 (ADR 0521): the active-set record of the Newton driver on the J2 load and unload sphere S4 of ADR 0475, against a
//! count of the points whose trial state differs from the committed state made independently from the states: the iterates
//! are captured by a recording contribution, the trial states come from the public element function, the committed states
//! are chained from `initial_states`. Checks A0 to A5, D1, D2, B1, F1 of the ADR.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::element_dofs;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::nonlinear::{
    element_trial_state, initial_states, solve_with, Contribution, DriverExtensions, NewtonOptions, NonlinearResult,
};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::cell::RefCell;
use std::collections::HashMap;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const SY: f64 = 2.0e8;
const RHO: f64 = 7850.0;
/// State values per J2 point; `alpha` is the last.
const STATE: usize = 13;

fn j2(h_iso: f64, h_kin: f64) -> Material {
    Material::j2(
        Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap(),
        SY,
        h_iso,
        h_kin,
    )
    .unwrap()
}

const SPHERE_N: usize = 2;
const SPHERE_M: usize = 8;
const ELEMENTS_PER_LAYER: usize = 3 * SPHERE_N * SPHERE_N;

fn p_of_c(c: f64) -> f64 {
    2.0 * c.ln() + 2.0 / 3.0 * (1.0 - c * c * c / 8.0)
}

/// The octant of the sphere `a = 1`, `b = 2` of D5 (cubed sphere, 2 x 2 cells per face, 8 radial layers, Hex20 on the
/// spheres), element `layer * 12 + cell`; with the bore pressure `p` (Pa) if given.
fn sphere(mat: Material, p: Option<f64>) -> SolidModel {
    let (n, m) = (SPHERE_N, SPHERE_M);
    let mut index: HashMap<([usize; 3], usize), usize> = HashMap::new();
    let mut params: Vec<Vec3> = Vec::new();
    let mut node = |q: [usize; 3], k: usize, params: &mut Vec<Vec3>| -> usize {
        *index.entry((q, k)).or_insert_with(|| {
            let r = 1.0 + k as f64 / m as f64;
            params.push([
                r * q[0] as f64 / n as f64,
                r * q[1] as f64 / n as f64,
                r * q[2] as f64 / n as f64,
            ]);
            params.len() - 1
        })
    };
    let mut conn = Vec::new();
    for k in 0..m {
        for a in 0..3 {
            for j in 0..n {
                for i in 0..n {
                    let q = |i: usize, j: usize| {
                        let mut v = [0; 3];
                        v[a] = n;
                        v[(a + 1) % 3] = i;
                        v[(a + 2) % 3] = j;
                        v
                    };
                    for kk in [k, k + 1] {
                        for (ii, jj) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)] {
                            let id = node(q(ii, jj), kk, &mut params);
                            conn.push(id);
                        }
                    }
                }
            }
        }
    }
    let nn = params.len();
    let mut mesh = SolidMesh::new(params).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        let set: Vec<usize> = (0..nn).filter(|&v| mesh.nodes()[v][c] == 0.0).collect();
        mesh.add_node_set(name, set).unwrap();
    }
    mesh.add_surface_set(
        "bore",
        (0..ELEMENTS_PER_LAYER)
            .map(|e| FaceRef { element: e, face: 0 })
            .collect(),
    )
    .unwrap();
    let mesh = upgrade_to_hex20_mapped(&mesh, |p| {
        let inf = p[0].abs().max(p[1].abs()).max(p[2].abs());
        let two = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] * inf / two, p[1] * inf / two, p[2] * inf / two]
    })
    .unwrap()
    .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        model.add_constraint(Constraint::displacement(name, c, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::pressure("bore", p)).unwrap();
    }
    model
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Every value of a nonlinear result in a fixed order: displacement, reactions, per increment the step, load factor,
/// halvings, residuals and energies, then the state values.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap_or(&[]));
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} {detail}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The bore pressure as a driver contribution `-g(lambda) f1`, `g = 1 - |2 lambda - 1|` (ADR 0475 D5), that records the
/// iterate and the load factor of every call.
struct Recorder {
    f1: Vec<f64>,
    calls: RefCell<Vec<(Vec<f64>, f64)>>,
}

impl Contribution for Recorder {
    fn name(&self) -> &str {
        "bore pressure, loaded and unloaded, recording"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.calls.borrow_mut().push((u.to_vec(), load_factor));
        let g = 1.0 - (2.0 * load_factor - 1.0).abs();
        for (f, v) in force.iter_mut().zip(&self.f1) {
            *f = -g * v;
        }
        Ok(())
    }
}

/// S4 with `steps` load steps; the result, the recorded calls and the model.
fn s4(steps: usize, record: bool) -> (NonlinearResult, Vec<(Vec<f64>, f64)>, SolidModel) {
    let p1 = p_of_c(1.25) * SY;
    let f1 = nodal_load_vector(&sphere(j2(0.0, 0.0), Some(p1)), &[]).unwrap();
    let model = sphere(j2(0.0, 0.0), None);
    let rec = Recorder {
        f1,
        calls: RefCell::new(Vec::new()),
    };
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: None,
    };
    let r = solve_with(
        &model,
        &[],
        &[],
        &NewtonOptions::new(steps).record_active_set(record),
        ext,
    )
    .unwrap();
    let calls = rec.calls.into_inner();
    (r, calls, model)
}

/// The trial states of every element at the displacement `u` from the committed states.
fn trial(model: &SolidModel, u: &[f64], committed: &[ElementState]) -> Vec<ElementState> {
    let mat = &model.materials()[0].1;
    let el = element_for(ElementKind::Hex20);
    let dofs = model.dof_map();
    let mesh = model.mesh();
    (0..mesh.n_elements())
        .map(|e| {
            let nodes = mesh.element_nodes(e);
            let x: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
            let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| u[q]).collect();
            element_trial_state(el, &x, mat, &ue, &committed[e]).unwrap()
        })
        .collect()
}

/// Per point (element-major): whether some value of the trial state differs bitwise from the committed one (A), and
/// whether `alpha` is greater (B).
fn differing(committed: &[ElementState], trial: &[ElementState]) -> (Vec<bool>, Vec<bool>) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (c, t) in committed.iter().zip(trial) {
        for (p, q) in c.values().chunks(STATE).zip(t.values().chunks(STATE)) {
            a.push(p.iter().zip(q).any(|(x, y)| x.to_bits() != y.to_bits()));
            b.push(q[STATE - 1] > p[STATE - 1]);
        }
    }
    (a, b)
}

fn count(v: &[bool]) -> usize {
    v.iter().filter(|&&x| x).count()
}

/// What the independent count found on one solve.
struct Independent {
    /// Per increment, per iteration: count A.
    a: Vec<Vec<usize>>,
    mismatch_a: usize,
    mismatch_flag: usize,
    mismatch_b: usize,
    iterations: usize,
    aligned: bool,
    chain_equals_states: bool,
}

fn independent(r: &NonlinearResult, calls: &[(Vec<f64>, f64)], model: &SolidModel) -> Independent {
    let total: usize = r.steps.iter().map(|s| s.residuals.len()).sum();
    let mut out = Independent {
        a: Vec::new(),
        mismatch_a: 0,
        mismatch_flag: 0,
        mismatch_b: 0,
        iterations: total,
        aligned: r.steps.iter().all(|s| s.halvings == 0) && calls.len() == total + 1,
        chain_equals_states: false,
    };
    let mut committed = initial_states(model).unwrap();
    let mut i = 0;
    for s in &r.steps {
        let mut counts = Vec::new();
        let mut previous: Option<Vec<bool>> = None;
        let mut last = None;
        for k in 0..s.residuals.len() {
            let (u, lf) = &calls[i];
            i += 1;
            out.aligned &= (lf - s.load_factor).abs() <= 1e-12;
            let t = trial(model, u, &committed);
            let (a, b) = differing(&committed, &t);
            let flag = previous.as_ref().is_some_and(|p| *p != a);
            counts.push(count(&a));
            out.mismatch_b += usize::from(a != b);
            out.mismatch_a += usize::from(s.yielded.get(k) != Some(&count(&a)));
            out.mismatch_flag += usize::from(s.set_changed.get(k) != Some(&flag));
            previous = Some(a);
            last = Some(t);
        }
        out.a.push(counts);
        committed = last.expect("an increment has an iteration");
    }
    out.chain_equals_states = committed.len() == r.states.len()
        && committed.iter().zip(&r.states).all(|(c, s)| {
            c.values().len() == s.values().len()
                && c.values()
                    .iter()
                    .zip(s.values())
                    .all(|(x, y)| x.to_bits() == y.to_bits())
        });
    out
}

/// Points with `alpha > 0` in the states of a result.
fn plastic_points(r: &NonlinearResult) -> usize {
    r.states
        .iter()
        .map(|s| s.values().chunks(STATE).filter(|p| p[STATE - 1] > 0.0).count())
        .sum()
}

#[test]
fn a_d_f_the_record_against_the_independent_count_on_s4() {
    let mut c = Checks::new();
    for steps in [16usize, 2] {
        let id = format!("S4 with {steps} steps");
        let (r, calls, model) = s4(steps, true);
        let finite = r.steps.iter().all(|s| s.residuals.iter().all(|x| x.is_finite()));
        c.holds(&format!("F1 {id}: every residual finite"), finite, "");
        let ind = independent(&r, &calls, &model);
        println!(
            "  {id}: {} increments, {} iterations, {} recorded calls, halvings {:?}",
            r.steps.len(),
            ind.iterations,
            calls.len(),
            r.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
        );
        c.holds(
            &format!("A0 {id}: no halving, calls = iterations + 1, load factors of the calls"),
            ind.aligned,
            &format!("{} calls for {} iterations", calls.len(), ind.iterations),
        );
        let lens = r
            .steps
            .iter()
            .all(|s| s.yielded.len() == s.residuals.len() && s.set_changed.len() == s.residuals.len());
        c.holds(&format!("A1 {id}: vectors as long as the residuals"), lens, "");
        c.holds(
            &format!("A1 {id}: record equals the independent count in every iteration"),
            ind.mismatch_a == 0,
            &format!("{} mismatches of {}", ind.mismatch_a, ind.iterations),
        );
        let first_false = r.steps.iter().all(|s| s.set_changed.first() == Some(&false));
        c.holds(
            &format!("A2 {id}: record of the set change equals the independent flag"),
            ind.mismatch_flag == 0 && first_false,
            &format!("{} mismatches of {}", ind.mismatch_flag, ind.iterations),
        );
        c.holds(
            &format!("A3 {id}: the points whose alpha increases are the points whose state differs"),
            ind.mismatch_b == 0,
            &format!("{} increments-iterations with a different set", ind.mismatch_b),
        );
        c.holds(
            &format!("A4 {id}: chained states equal the result states"),
            ind.chain_equals_states,
            "",
        );
        let last = r
            .steps
            .last()
            .and_then(|s| s.yielded.last())
            .copied()
            .unwrap_or(usize::MAX);
        let plastic = plastic_points(&r);
        c.holds(
            &format!("A5 {id}: last yielded 0 while points have alpha > 0"),
            last == 0 && plastic > 0,
            &format!("last yielded {last}, {plastic} points with alpha > 0"),
        );
        let dec_from_0 = r.steps.iter().any(|s| s.yielded.windows(2).any(|w| w[1] < w[0]));
        let dec_corrections = r
            .steps
            .iter()
            .any(|s| s.yielded.windows(2).skip(1).any(|w| w[1] < w[0]));
        let n_dec = r
            .steps
            .iter()
            .filter(|s| s.yielded.windows(2).any(|w| w[1] < w[0]))
            .count();
        let n_changed = r.steps.iter().filter(|s| s.set_changed.iter().any(|&x| x)).count();
        let n_dec_independent = ind.a.iter().filter(|a| a.windows(2).any(|w| w[1] < w[0])).count();
        let strictly3 = ind.a.iter().any(|a| a.windows(3).any(|w| w[2] < w[1] && w[1] < w[0]));
        for (s, a) in r.steps.iter().zip(&ind.a) {
            if s.yielded.windows(2).any(|w| w[1] < w[0]) {
                println!(
                    "    increment {} (load factor {:.4}): record {:?}, independent {:?}",
                    s.step, s.load_factor, s.yielded, a
                );
            }
        }
        println!("  {id}: increments with a decrease {n_dec} (independent {n_dec_independent}), with a set change {n_changed}");
        if steps == 16 {
            c.holds(
                "D1 S4 with 16 steps: some iteration records a smaller count than the one before",
                dec_from_0,
                "",
            );
            c.holds(
                "D2 S4 with 16 steps: increments with a decrease and with a set change both > 0",
                n_dec > 0 && n_changed > 0 && n_dec_independent > 0,
                &format!("{n_dec}, {n_changed}, {n_dec_independent}"),
            );
        } else {
            c.holds(
                "D1 S4 with 2 steps: a decrease between two corrections (k >= 1)",
                dec_corrections,
                "",
            );
            c.holds(
                "D2 S4 with 2 steps: the independent count strictly decreases over 3 consecutive iterations",
                strictly3,
                "",
            );
        }
    }
    c.finish();
}

#[test]
fn b1_the_record_is_opt_in_on_the_contribution_path() {
    let mut c = Checks::new();
    let (off, calls_off, _) = s4(2, false);
    let (on, calls_on, _) = s4(2, true);
    let (d_off, d_on) = (digest(&flat(&off)), digest(&flat(&on)));
    c.holds(
        "B1 S4 with 2 steps: digests with the record off and on equal",
        d_off == d_on,
        &format!("{d_off:#018x} {d_on:#018x}"),
    );
    let same_calls = calls_off.len() == calls_on.len()
        && calls_off
            .iter()
            .zip(&calls_on)
            .all(|(a, b)| a.1.to_bits() == b.1.to_bits() && digest(&a.0) == digest(&b.0));
    c.holds(
        "B1 the iterates seen by the contribution are bit-identical",
        same_calls,
        "",
    );
    let empty = off
        .steps
        .iter()
        .all(|s| s.yielded.is_empty() && s.set_changed.is_empty());
    let filled = on
        .steps
        .iter()
        .all(|s| !s.yielded.is_empty() && !s.set_changed.is_empty());
    c.holds(
        "B1 vectors empty with the record off, filled with it on",
        empty && filled,
        "",
    );
    c.finish();
}
