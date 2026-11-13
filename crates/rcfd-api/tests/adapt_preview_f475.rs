//! ADR 0377 (F-475) D1: `rcfd_api::sim::adapted_mesh`, the run's adaptive loop for the Studio preview, polls its
//! `cancelled` once before each solve and stops there; uncancelled it ends on the mesh of the run's first step.

use rcfd_api::case::Case;
use rcfd_api::sim::{adapted_mesh, Simulation};
use std::cell::Cell;
use std::path::Path;
use std::time::Instant;

fn shipped() -> Case {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/cavity/adapt_re100.toml");
    Case::load(f).unwrap_or_else(|d| panic!("{d:?}"))
}

/// `adapted_mesh` with a `cancelled` that is true from poll `from` on (`None`: never); (cells and hash, polls, seconds).
fn timed(case: &Case, from: Option<usize>) -> (Option<(usize, u64)>, usize, f64) {
    let polls = Cell::new(0usize);
    let cancelled = || {
        polls.set(polls.get() + 1);
        from.is_some_and(|k| polls.get() >= k)
    };
    let t = Instant::now();
    let r = adapted_mesh(case, &cancelled).unwrap_or_else(|e| panic!("{e}"));
    let s = t.elapsed().as_secs_f64();
    (r.map(|a| (a.mesh.n_cells(), a.mesh.content_hash())), polls.get(), s)
}

#[test]
fn d1_the_adaptive_loop_stops_at_the_poll_before_a_solve() {
    let case = shipped();
    let (a, a_polls, t_a) = timed(&case, None);
    let mut sim = Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}"));
    sim.step();
    let run = (sim.geom.n_cells(), sim.geom.mesh.content_hash());
    eprintln!(
        "D1 (a): {a:?}, {a_polls} polls, {t_a:.3} s; the run after its first step: {} cells, {:016x}",
        run.0, run.1
    );
    assert_eq!(a, Some(run), "D1 (a) the run's mesh");
    assert_eq!(run.0, 2799, "D1 (a) cells");
    assert_eq!(run.1, 0x05bb_8bea_e5da_57da, "D1 (a) the hash of ADR 0373 A1");
    assert_eq!(a_polls, 3, "D1 (a) polls");
    let (b, b_polls, t_b) = timed(&case, Some(1));
    eprintln!("D1 (b): {b:?}, {b_polls} polls, {t_b:.3} s = {:.3} T_a", t_b / t_a);
    assert_eq!(b, None, "D1 (b)");
    assert_eq!(b_polls, 1, "D1 (b) polls");
    assert!(t_b <= 0.10 * t_a, "D1 (b) {t_b:.3} s > 0.10 T_a");
    let (c, c_polls, t_c) = timed(&case, Some(2));
    eprintln!("D1 (c): {c:?}, {c_polls} polls, {t_c:.3} s = {:.3} T_a", t_c / t_a);
    assert_eq!(c, None, "D1 (c)");
    assert_eq!(c_polls, 2, "D1 (c) polls");
    assert!(t_c <= 0.35 * t_a, "D1 (c) {t_c:.3} s > 0.35 T_a");
}
