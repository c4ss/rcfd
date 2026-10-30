//! ADR 0172 (job 49J) limit LH1: `SteadyDivergence::message` is byte-identical to its text before the job, and
//! `message_with_hint` replaces only the closing advice.

use rcfd_fv::simple::SteadyDivergence;

fn d() -> SteadyDivergence {
    SteadyDivergence {
        iteration: 15,
        kind: "velocity_runaway",
        quantity: "velocity",
        cell: Some(1),
        value: 5.0e3,
        limit: 1.0e3,
        last_good: 9,
        last_good_continuity: 2.5e-3,
    }
}

/// The text before ADR 0172, written out (copied from the format string of the unchanged code).
const BEFORE_WITH_CENTRES: &str =
    "steady divergence guard: velocity_runaway: velocity is 5.0000e3 (limit 1.0000e3) at iteration 15, \
cell 1 at (1.5000e-1, 2.5000e-1, 0.0000e0); last good iteration 9 (continuity 2.500e-3); lower the relaxation factors \
(solver.alpha_u, solver.alpha_p) or use a more dissipative convection scheme";

const BEFORE_NO_CENTRES: &str =
    "steady divergence guard: velocity_runaway: velocity is 5.0000e3 (limit 1.0000e3) at iteration 15, \
cell 1; last good iteration 9 (continuity 2.500e-3); lower the relaxation factors \
(solver.alpha_u, solver.alpha_p) or use a more dissipative convection scheme";

#[test]
fn lh1_message_is_byte_identical_to_the_text_before() {
    let centres = [[0.0, 0.0, 0.0], [0.15, 0.25, 0.0]];
    assert_eq!(d().message(Some(&centres)), BEFORE_WITH_CENTRES);
    assert_eq!(d().message(None), BEFORE_NO_CENTRES);
    assert_eq!(d().message_with_hint(None, None), BEFORE_NO_CENTRES);
}

#[test]
fn lh1_a_hint_replaces_only_the_closing_advice() {
    let m = d().message_with_hint(None, Some("use the limiter"));
    assert_eq!(
        m,
        BEFORE_NO_CENTRES.replace(
            "lower the relaxation factors (solver.alpha_u, solver.alpha_p) or use a more dissipative convection scheme",
            "use the limiter"
        )
    );
    assert!(!m.contains("relaxation"));
}
