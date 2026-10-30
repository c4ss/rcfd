//! ADR 0171 (job 49H): the NACA 0012 trailing-edge divergence at fine wall spacing. The limits of the registered runs,
//! read from the committed `docs/investigations/job49h-te-divergence/results.json`; a missed limit is an ignored
//! `*_known_miss` test with the limit intact. No code of the solver changed in this job, so there is no wiring test.

use std::path::PathBuf;

fn results() -> Option<serde_json::Value> {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job49h-te-divergence/results.json");
    let t = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&t).ok()
}

fn met(id: &str) -> bool {
    let r = results().expect("results.json of ADR 0171 not committed");
    r["evaluation"][id]["met"]
        .as_bool()
        .unwrap_or_else(|| panic!("{id}: no evaluation"))
}

// ---------------------------------------------------------------------------------------- Phase 1: reproduction

/// LR1: at least one of RA and RB (M1, SA, 0 and 6 degrees) ends in a guard stop before iteration 300.
#[test]
fn lr1_divergence_reproduced() {
    assert!(met("LR1"));
}

/// LR2: every guard stop of RA to RE is at a trailing-edge wall cell.
#[test]
fn lr2_locus_is_the_trailing_edge() {
    assert!(met("LR2"));
}

/// LR3: M1w05 at 0 degrees is clean (ADR 0168's runs).
#[test]
fn lr3_consistent_with_adr_0168() {
    assert!(met("LR3"));
}

/// LR4: M1w05 at 6 degrees ends in a guard stop.
#[test]
fn lr4_angle_matters() {
    assert!(met("LR4"));
}

/// LR5: M1w15 at 6 degrees is clean.
#[test]
fn lr5_coarser_wall_spacing_is_clean() {
    assert!(met("LR5"));
}

/// LR6: SST on M1 at 6 degrees ends in a guard stop.
#[test]
fn lr6_sst_diverges_too() {
    assert!(met("LR6"));
}

// ---------------------------------------------------------------------------------------- Phase 2: mechanism

/// LM1 (H-084 predicted `face_flux` clean): PA diverged at iteration 18.
#[test]
#[ignore = "ADR 0171 LM1 known miss: PA (velocity_correction = face_flux) stops at iteration 18, cell 255"]
fn lm1_face_flux_clean_known_miss() {
    assert!(met("LM1"));
}

/// LM2 (H-084 predicted `alpha_p` 0.3 clean): PB diverged at iteration 25.
#[test]
#[ignore = "ADR 0171 LM2 known miss: PB (alpha_p 0.3) stops at iteration 25, cell 0"]
fn lm2_low_alpha_p_clean_known_miss() {
    assert!(met("LM2"));
}

/// LM3: laminar diverges at the trailing edge between iteration 10 and 20 (not the turbulence model).
#[test]
fn lm3_not_turbulence() {
    assert!(met("LM3"));
}

/// LM4 (predicted first-order upwind also stops): PF is clean. The prediction was wrong, and PF is confounded (it also
/// drops the non-orthogonal viscous correction); QA is the discriminating run.
#[test]
#[ignore = "ADR 0171 LM4 known miss: PF (convection = upwind) is clean, 300 iterations"]
fn lm4_upwind_stops_known_miss() {
    assert!(met("LM4"));
}

/// LM5: the unrotated mesh with a rotated free stream stops at the trailing edge between iteration 10 and 20.
#[test]
fn lm5_not_the_mesh_transform() {
    assert!(met("LM5"));
}

/// LM6 (H-084 predicted SIMPLE clean): PD stops at iteration 12.
#[test]
#[ignore = "ADR 0171 LM6 known miss: PD (SIMPLE 0.7/0.3) stops at iteration 12, cell 255"]
fn lm6_simple_clean_known_miss() {
    assert!(met("LM6"));
}

// ---------------------------------------------------------------------------------------- Phase 3: the limiter

/// LN1 to LN5: the Barth-Jespersen limiter of the linear-upwind extrapolation makes the five 300-iteration runs clean.
#[test]
fn ln1_to_ln5_limiter_removes_the_stop() {
    assert!(met("LN1") && met("LN2") && met("LN3") && met("LN4") && met("LN5"));
}

// ---------------------------------------------------------------------------------------- Phase 4: full-length runs

/// LL1, LL6, LL8, LL9: the limited runs end without a stop, LB6 is steady (ADR 0159 D4), the default at 0 degrees also
/// completes 8000 iterations, and QE is clean.
#[test]
fn ll_stability_limits() {
    assert!(met("LL1") && met("LL6") && met("LL8") && met("LL9"));
}

/// LL3: LB0 `cd_veff` within 6 % of the TMR friction drag (measured +4.6 %).
#[test]
fn ll3_friction_drag_at_0_degrees() {
    assert!(met("LL3"));
}

/// LL4: the limiter's cost at 0 degrees is at most 3 % of `cd_eff` (measured +2.99 %, a margin of 0.01 %; the cost is in
/// the pressure drag: cd_p 0.001632 against 0.001390).
#[test]
fn ll4_limiter_cost_at_0_degrees() {
    assert!(met("LL4"));
}

/// LL5: LB6 `cl_eff` within 5 % of Ladson's 0.645 (measured 0.64425).
#[test]
fn ll5_lift_at_6_degrees() {
    assert!(met("LL5"));
}

/// LL2: LB0 `cd_eff` within 6 % of TMR's 0.008192. Measured 0.008776 (+7.1 %); the unlimited LA0 is +4.0 %.
#[test]
#[ignore = "ADR 0171 LL2 known miss: LB0 cd_eff is +7.1 % against TMR 0.008192 (limit 6 %)"]
fn ll2_total_drag_at_0_degrees_known_miss() {
    assert!(met("LL2"));
}

/// LL7: LB6 `cd_eff` within 10 % of 49D's 0.00988. Measured 0.011140 (+12.7 %), cd_p 0.004218 against 0.00323.
#[test]
#[ignore = "ADR 0171 LL7 known miss: LB6 cd_eff is +12.7 % against 49D's 0.00988 (limit 10 %)"]
fn ll7_total_drag_at_6_degrees_known_miss() {
    assert!(met("LL7"));
}
