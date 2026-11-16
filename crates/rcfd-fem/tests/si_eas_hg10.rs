//! F-748 (ADR 0668): the warm start of the local mode iteration of the stabilised incompatible-modes Hex8 (the remedy W of
//! ADR 0660) with a creep material and a viscoelastic material on a small Incompatible block through the quasi-static
//! time-stepping path of ADR 0489 (`solve_in_time`), against the cold S of ADR 0658: the record of the panic of the creep
//! reading before the fix (P0), the creep rows CR1 to CR6, CI, NCP and the viscoelastic rows VE1 to VE5, NCV. Every
//! aggregated value is required finite before it is compared; a failed solve misses. The helpers are copies of those of
//! `si_eas_hg9.rs` (existing tests are not edited).

use rcfd_fem::builders::hex_box;
use rcfd_fem::element::ElementState;
use rcfd_fem::material::PronyTerm;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{solve_in_time, TimeHistory, TimeSteppingOptions};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};

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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.4e} (limit in [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo:e}, {hi:e}]"));
        }
    }
    fn equals(&mut self, name: &str, value: usize, expected: usize) {
        let ok = value == expected;
        println!(
            "{name}: {value} (expected {expected}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} != {expected}"));
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

const E_MOD: f64 = 2.0e11;
const POINTS: usize = 8;
const MODES: usize = 9;
const CREEP_K: usize = 7;
const VISCO_K: usize = 12;
const TOL: f64 = 1e-9;
const TIMES: [f64; 4] = [0.25, 0.5, 0.75, 1.0];

/// The steel of `si_eas.rs`.
fn steel() -> Material {
    Material::isotropic_elastic(E_MOD, 0.3, 7850.0, 1.2e-5).unwrap()
}

/// The creep material of arithmetic 2 (ADR 0608 RF1 amendment 1).
fn creep() -> Material {
    Material::creep(steel(), 5e-33, 4.0, 0.0).unwrap()
}

/// The viscoelastic material of arithmetic 3: one branch, equal weights 0.5, tau 0.25 s.
fn visco() -> Material {
    Material::viscoelastic(steel(), vec![PronyTerm::new(0.5, 0.5, 0.25).unwrap()], None).unwrap()
}

/// The opt-ins of a solve: the backtracking of the modes (ADR 0655 D1), its decrease test (ADR 0658 D1), the warm start
/// (ADR 0660 D1). No line search.
#[derive(Clone, Copy)]
struct Opts {
    name: &'static str,
    backtracking: bool,
    decrease: bool,
    warm: bool,
}

/// The variant S of ADR 0658 (cold).
const S: Opts = Opts {
    name: "S",
    backtracking: true,
    decrease: true,
    warm: false,
};
/// The remedy W of ADR 0660 reading c3.
const W: Opts = Opts {
    name: "W",
    backtracking: true,
    decrease: true,
    warm: true,
};

fn block_mesh() -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [4, 2, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::Incompatible))
        .unwrap();
    mesh
}

/// The block of arithmetic 2: Incompatible with rule L, `xmin` clamped, the traction `[0, 1e6, 0]` on `xmax`.
fn model_of(opts: Opts, material: Material) -> SolidModel {
    let mut m = SolidModel::new(block_mesh());
    m.set_incompatible_stabilisation(IncompatibleStabilisation::LinearStress);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
    m.set_incompatible_mode_warm_start(opts.warm);
    m.add_material("all", material).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 1e6, 0.0])).unwrap();
    m
}

fn run(opts: Opts, material: Material, limit: f64) -> (SolidModel, rcfd_fem::Result<TimeHistory>) {
    let m = model_of(opts, material);
    let h = solve_in_time(&m, &[], &TIMES, &TimeSteppingOptions::new(limit));
    (m, h)
}

fn run_creep(opts: Opts) -> (SolidModel, rcfd_fem::Result<TimeHistory>) {
    run(opts, creep(), 5e-4)
}

fn run_visco(opts: Opts) -> (SolidModel, rcfd_fem::Result<TimeHistory>) {
    run(opts, visco(), f64::INFINITY)
}

/// `max |a - b| / max |b|` over paired slices; NaN if a length differs or a value is not finite.
fn rel_diff<'a>(pairs: impl Iterator<Item = (&'a [f64], &'a [f64])>) -> f64 {
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for (a, b) in pairs {
        if a.len() != b.len() {
            return f64::NAN;
        }
        for (x, y) in a.iter().zip(b) {
            if !x.is_finite() || !y.is_finite() {
                return f64::NAN;
            }
            num = num.max((x - y).abs());
            den = den.max(y.abs());
        }
    }
    if den > 0.0 {
        num / den
    } else if num == 0.0 {
        0.0
    } else {
        f64::NAN
    }
}

/// The material part of an element state (`POINTS x k` values; the whole state if shorter).
fn material_part(s: &ElementState, k: usize) -> &[f64] {
    let v = s.values();
    &v[..(POINTS * k).min(v.len())]
}

/// The mean `u_y` of the `xmax` nodes.
fn tip(m: &SolidModel, u: &[f64]) -> f64 {
    let dofs = m.dof_map();
    let nodes = m.mesh().node_set("xmax").unwrap();
    nodes.iter().map(|&n| u[dofs.equation(n, 1).unwrap()]).sum::<f64>() / nodes.len() as f64
}

/// The reading of `creep_increment` at 7b0740f2 (chunks of `STATE_SIZE = 7` of the whole state, index 6), copied for NCP.
fn old_creep_increment(old: &[ElementState], new: &[ElementState]) -> f64 {
    let ns = CREEP_K;
    let mut worst = 0.0f64;
    for (o, n) in old.iter().zip(new) {
        for (po, pn) in o.values().chunks(ns).zip(n.values().chunks(ns)) {
            worst = worst.max(pn[ns - 1] - po[ns - 1]);
        }
    }
    worst
}

/// The largest increment of the equivalent creep strain over the material parts (CI).
fn own_creep_increment(old: &[ElementState], new: &[ElementState]) -> f64 {
    let mut worst = 0.0f64;
    for (o, n) in old.iter().zip(new) {
        for (po, pn) in material_part(o, CREEP_K)
            .chunks(CREEP_K)
            .zip(material_part(n, CREEP_K).chunks(CREEP_K))
        {
            worst = worst.max(pn[CREEP_K - 1] - po[CREEP_K - 1]);
        }
    }
    worst
}

fn print_history(name: &str, m: &SolidModel, h: &TimeHistory) {
    for s in &h.steps {
        println!(
            "{name} t = {:.2}: tip u_y {:.6e}, halvings {}, sub-steps {}, iterations {}, max creep increment {:.4e}",
            s.time,
            tip(m, &s.u),
            s.halvings,
            s.substeps,
            s.iterations,
            s.max_creep_increment
        );
    }
}

/// The elements of `h` whose committed state at an entry has another length than `n`.
fn wrong_lengths(h: &TimeHistory, n: usize) -> usize {
    h.steps
        .iter()
        .flat_map(|s| s.states.iter())
        .filter(|s| s.values().len() != n)
        .count()
}

/// The largest change of a committed mode between two entries (reported).
fn largest_mode_change(a: &[ElementState], b: &[ElementState], k: usize) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| {
            let (x, y) = (x.values(), y.values());
            let n = POINTS * k;
            (0..MODES).filter_map(move |j| match (x.get(n + j), y.get(n + j)) {
                (Some(p), Some(q)) => Some((q - p).abs()),
                _ => None,
            })
        })
        .fold(0.0, f64::max)
}

#[test]
#[ignore = "record of ADR 0668 P0: the W creep run panicked in creep_increment before D1 (run 0, before the fix); after D1 it no longer panics"]
#[should_panic(expected = "index out of bounds")]
fn p0_w_creep_panics_before_the_fix() {
    let (_, h) = run_creep(W);
    println!("P0: W creep run returned {:?}", h.map(|h| h.steps.len()));
}

#[test]
fn w_creep_on_the_time_stepping_path() {
    let mut ch = Checks::new();
    let (ms, hs) = run_creep(S);
    let hs = hs.expect("the cold S creep run");
    print_history(&format!("{} creep", S.name), &ms, &hs);
    let (mw, hw) = run_creep(W);
    let ok = hw.is_ok();
    ch.holds(&format!("CR1 W creep solve_in_time Ok ({:?})", hw.as_ref().err()), ok);
    let Ok(hw) = hw else {
        ch.finish();
        return;
    };
    print_history(&format!("{} creep", W.name), &mw, &hw);
    ch.equals("CR1 W creep entries", hw.steps.len(), 5);
    let n = hw.steps.len().min(hs.steps.len());
    let du = rel_diff((0..n).map(|k| (hw.steps[k].u.as_slice(), hs.steps[k].u.as_slice())));
    ch.at_most("CR2 creep displacement W against S", du, TOL);
    let ds = rel_diff((0..n).flat_map(|k| {
        hw.steps[k]
            .states
            .iter()
            .zip(&hs.steps[k].states)
            .map(|(a, b)| (material_part(a, CREEP_K), material_part(b, CREEP_K)))
    }));
    ch.at_most("CR3 creep material states W against S", ds, TOL);
    let inc_w: Vec<f64> = hw.steps.iter().map(|s| s.max_creep_increment).collect();
    let inc_s: Vec<f64> = hs.steps.iter().map(|s| s.max_creep_increment).collect();
    let di = rel_diff(std::iter::once((inc_w.as_slice(), inc_s.as_slice())));
    ch.at_most("CR4 creep max_creep_increment W against S", di, TOL);
    let same_steps = hw.steps.len() == hs.steps.len()
        && hw
            .steps
            .iter()
            .zip(&hs.steps)
            .all(|(a, b)| a.halvings == b.halvings && a.substeps == b.substeps);
    ch.holds("CR4 creep halvings and sub-steps of W equal S's", same_steps);
    ch.equals(
        "CR5 creep W elements with a state length other than 56 + 9",
        wrong_lengths(&hw, POINTS * CREEP_K + MODES),
        0,
    );
    ch.equals(
        "CR5 creep S elements with a state length other than 56",
        wrong_lengths(&hs, POINTS * CREEP_K),
        0,
    );
    let ratio = tip(&mw, &hw.steps[n - 1].u) / tip(&mw, &hw.steps[0].u);
    ch.above("CR6 creep W tip u_y(1) / u_y(0)", ratio, 1.1);
    // CI: the library's increment against the test's own over the material parts, steps with one sub-step.
    let mut compared = 0;
    let mut equal = true;
    for k in 1..hw.steps.len() {
        if hw.steps[k].substeps != 1 {
            println!("CI: step {k} has {} sub-steps, not compared", hw.steps[k].substeps);
            continue;
        }
        let own = own_creep_increment(&hw.steps[k - 1].states, &hw.steps[k].states);
        let lib = hw.steps[k].max_creep_increment;
        println!("CI step {k}: library {lib:e}, own {own:e}");
        compared += 1;
        equal &= own.to_bits() == lib.to_bits();
    }
    ch.holds(
        "CI W max_creep_increment bitwise the own reading of the material parts",
        equal,
    );
    ch.holds(&format!("CI steps compared ({compared}) >= 3"), compared >= 3);
    println!(
        "reported: the largest change of a committed mode of W between steps 1 and 2: {:e}",
        largest_mode_change(&hw.steps[1].states, &hw.steps[2].states, CREEP_K)
    );
    // NCP: the old reading on W's states panics; on S's states it is the library's.
    let (a, b) = (hw.steps[1].states.clone(), hw.steps[2].states.clone());
    let old_w = std::panic::catch_unwind(move || old_creep_increment(&a, &b));
    ch.holds("NCP the old reading on W's committed states panics", old_w.is_err());
    let old_s = old_creep_increment(&hs.steps[1].states, &hs.steps[2].states);
    let one = hs.steps[2].substeps == 1;
    println!(
        "NCP S step 2: old reading {old_s:e}, library {:e}, sub-steps {}",
        hs.steps[2].max_creep_increment, hs.steps[2].substeps
    );
    ch.holds(
        "NCP the old reading on S's states equals S's max_creep_increment of step 2 bitwise (one sub-step)",
        one && old_s.to_bits() == hs.steps[2].max_creep_increment.to_bits(),
    );
    ch.finish();
}

#[test]
fn w_viscoelastic_on_the_time_stepping_path() {
    let mut ch = Checks::new();
    let (ms, hs) = run_visco(S);
    let hs = hs.expect("the cold S viscoelastic run");
    print_history(&format!("{} visco", S.name), &ms, &hs);
    let (mw, hw) = run_visco(W);
    let ok = hw.is_ok();
    ch.holds(
        &format!("VE1 W viscoelastic solve_in_time Ok ({:?})", hw.as_ref().err()),
        ok,
    );
    let Ok(hw) = hw else {
        ch.finish();
        return;
    };
    print_history(&format!("{} visco", W.name), &mw, &hw);
    ch.equals("VE1 W viscoelastic entries", hw.steps.len(), 5);
    let n = hw.steps.len().min(hs.steps.len());
    let du = rel_diff((0..n).map(|k| (hw.steps[k].u.as_slice(), hs.steps[k].u.as_slice())));
    ch.at_most("VE2 viscoelastic displacement W against S", du, TOL);
    let ds = rel_diff((0..n).flat_map(|k| {
        hw.steps[k]
            .states
            .iter()
            .zip(&hs.steps[k].states)
            .map(|(a, b)| (material_part(a, VISCO_K), material_part(b, VISCO_K)))
    }));
    ch.at_most("VE3 viscoelastic material states W against S", ds, TOL);
    ch.equals(
        "VE4 viscoelastic W elements with a state length other than 96 + 9",
        wrong_lengths(&hw, POINTS * VISCO_K + MODES),
        0,
    );
    ch.equals(
        "VE4 viscoelastic S elements with a state length other than 96",
        wrong_lengths(&hs, POINTS * VISCO_K),
        0,
    );
    let zero = hw
        .steps
        .iter()
        .chain(&hs.steps)
        .all(|s| s.max_creep_increment.to_bits() == 0.0f64.to_bits());
    ch.holds(
        "VE4 viscoelastic max_creep_increment 0.0 at every entry of W and S",
        zero,
    );
    let ratio = tip(&mw, &hw.steps[n - 1].u) / tip(&mw, &hw.steps[0].u);
    ch.within("VE5 viscoelastic W tip u_y(1) / u_y(0)", ratio, 1.6, 2.0);
    let chunks9 = hw.steps[n - 1]
        .states
        .iter()
        .filter(|s| s.values().chunks(VISCO_K).count() != POINTS)
        .count();
    ch.equals(
        "NCV W viscoelastic elements read in chunks of 12 with other than 8 chunks",
        chunks9,
        16,
    );
    println!(
        "reported: the largest change of a committed mode of W between steps 1 and 2: {:e}",
        largest_mode_change(&hw.steps[1].states, &hw.steps[2].states, VISCO_K)
    );
    ch.finish();
}
