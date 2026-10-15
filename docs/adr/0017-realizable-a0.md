# ADR 0017: Realizable k-epsilon constant A0 (4.04 versus 4.0) and the registered step-flow misses (F-042)

Status: accepted. The checks, numeric limits and decision rules below were written before any run of the new tests and
before any code change for this ADR (the code and the source were only read; the reading is recorded in "Provenance").
They are not changed, widened, replaced or deleted after a run. A miss is reported as measured and moved to an ignored
`*_known_miss` test with the registered limit intact. If a test-set-up defect is found, a NEW corrected test is added
and the old one stays as a record. Addenda may be appended at the end without touching the registered limits.
Roadmap: F-042. Owner: agent A0 (`kepsilon.rs` coefficient plumbing, one new test file, this ADR).
Clean-room: published literature only.

## Question

`docs/verification-report.md` (section 2.1) found that the Realizable variable-C_mu formula
`C_mu = 1 / (A0 + A_s U* k / eps)` uses A0 = 4.04 in `crates/rcfd-turb/src/kepsilon.rs`, whereas the original report
prints A0 = 4.0. The Realizable step-flow reattachment checks are registered known misses (ADR 0015): F-042 old set-up
6.402 against 6.26 +- 0.10, and BFS-C medium mesh 6.413 against the window [6.16, 6.36]. The audit left open whether the
sub-1-percent change of A0 could change those outcomes. This ADR measures it, without changing the default.

## Provenance of the two values (checked here, 2026-10-15)

* A0 = 4.0: Shih, Liou, Shabbir, Yang and Zhu, "A New k-epsilon Eddy Viscosity Model for High Reynolds Number Turbulent
  Flows", NASA TM-106721 (1994; the report version of Computers and Fluids 24(3), 227-238, 1995). The report PDF
  (https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf) was fetched and its text extracted: the
  section "Calibration of the model coefficient A0" states "This leads to A0 = 4.0 which corresponds to C_mu = 0.09 in the
  inertial sublayer" and the homogeneous-shear check uses A0 = 4.0. VERIFIED against the primary source.
* A0 = 4.04: the value used by the library up to now; it is a commonly cited commercial-code variant (the commercial
  documentation prints 4.04). Not verified against any primary source here and not used as a reference.

With A_s U* k/eps -> 0 the two values give C_mu = 0.2475 versus 0.2500 in the limit of vanishing strain, and for a
typical C_mu near 0.09 (A0 + A_s U* k/eps = 11.1) the change is -0.4 percent in the denominator, i.e. C_mu rises by about
0.4 percent with A0 = 4.0.

## Change (default unchanged)

* `KEpsilon` gets a public field `a0`, default `KEPS_REALIZABLE_A0_DEFAULT = 4.04`, used only by the Realizable variant.
  The default build path is therefore bit-identical.
* Settings tree: `turbulence.realizable_a0` (float, 0 = the library default 4.04; allowed 3.0 to 5.0; only with
  `turbulence.model = k_epsilon` and `variant = realizable`, located errors otherwise). The existing coefficient table
  `turbulence.coefficients.k_epsilon.*` is NOT extended: ADR 0014 test K4 fixes its size at 5 entries and may not be
  edited, so A0 does not fit that table without breaking a registered test. The documentation (`docs/case-schema.md`,
  `docs/reference-data.md`) states both values with the provenance above.

## Registered checks and limits

All on the BFS-C set-up of ADR 0015 (Re_h 36000, inlet at x/h = -4, measured inlet profile, SIMPLEC 0.8, linearUpwind
momentum, upwind k/epsilon, smooth wall treatment), Realizable, release builds, convergence criterion unchanged (max
update < 1e-7 and nu_t change < 1e-6 within the iteration cap of the existing test, 6000). Meshes MESH_COARSE /
MESH_MEDIUM / MESH_FINE of `tests/common/step.rs`.

* P1 (the question): A0 = 4.0, medium mesh: converged and x_r/h in [6.16, 6.36] (the original window, not widened).
  PASS / MISS as measured.
* P2 (reproduction): A0 = 4.04 set explicitly through the new field gives x_r/h bit-equal (`==`) to the run with the
  default options, and the default run equals the ADR 0015 value 6.413 to within 5e-4 (the recorded precision).
* P3 (grid check, as ADR 0015 A2): A0 = 4.0 on coarse, medium, fine: all three converge; "grid-converged" iff
  |x_r(fine) - x_r(medium)| <= 0.10 h. The three values are reported; the window is also reported on fine (information,
  no separate verdict).
* P4 (effect size, reported with a verdict on the hypothesis only): delta(x_r) = x_r(4.0) - x_r(4.04) on the medium mesh.
  Hypothesis registered before the run: |delta| < 0.053 (the distance of 6.413 to the window edge 6.36), i.e. A0 alone
  does not flip the verdict. This is a prediction from the 0.4 percent change in C_mu, not an acceptance criterion.
* P5 (regression): `crates/rcfd-api/tests/baseline_hashes.rs` passes unmodified (default options bit-identical), and
  the existing `step_kepsilon.rs`, `step_kepsilon_corrected.rs`, `channel_kepsilon.rs`, `turb_models.rs` tests are
  unchanged and pass or stay in their recorded state.
* P6 (schema, default tests): `turbulence.realizable_a0` default 0 accepted; 4.0 and 4.04 accepted with the realizable
  variant; 2.9 and 5.1 rejected with a diagnostic whose path is exactly `turbulence.realizable_a0`; any non-zero value
  with variant standard or rng, or with another model, rejected at that path; a short tree run with 4.0 differs from the
  default run in the reported eddy viscosity (so the key is applied, not ignored) and with 4.04 equals the default run
  bit for bit.

## Decision rules (registered)

* The default stays 4.04 whatever the numbers. If P1 passes, the result is reported to the user, who decides; a pass is
  not proof that 4.0 is "right" or that the model is validated: the same window was passed by RNG marginally (0.026 inside
  on the medium mesh, less than the 0.107 sensitivity to the k/epsilon convection scheme found in ADR 0015) and the
  Realizable result moves by -0.14 with second-order k/epsilon convection. A pass is called marginal if it lies within
  0.14 h of a window edge or fails P3.
* A miss is kept in the ignored test `step_c_realizable_a0_4_known_miss`, window unchanged. If the pass is marginal or
  not grid-converged, that is stated in the verdict.
* F-042 stays `partial` (the energy and moving-wall sub-criteria, low-Re variants and the Standard variant are unaffected
  by A0 and still open).

## Hygiene

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release
--no-fail-fast` on a `git archive` copy of the final commit; `CARGO_TARGET_DIR=<target dir>`.

## Results

Measured 2026-10-15, release builds, `CARGO_TARGET_DIR=<target dir>`, BFS-C, Realizable. All six runs met the
convergence criterion (max update < 1e-7, nu_t change < 1e-6; 489 to 1635 iterations). No registered limit was changed.

| Check | Measured | Limit | Verdict |
| --- | --- | --- | --- |
| P1 A0 = 4.0, medium x_r/h | 6.3486 | [6.16, 6.36] | PASS, marginal (0.011 below the upper edge) |
| P2 explicit 4.04 vs default | 6.412696 vs 6.412696 (bit-equal); default vs 6.413 | `==`; within 5e-4 | PASS |
| P3 A0 = 4.0 coarse / medium / fine | 6.174 / 6.349 / 6.453 | abs(fine - medium) <= 0.10 | MISS (0.105; not grid-converged) |
| P4 delta x_r(4.0) - x_r(4.04), medium | -0.064 | hypothesis abs(delta) < 0.053 | hypothesis REFUTED (A0 alone moves x_r by 0.064) |
| P5 baseline hashes and existing turbulence/API tests | `baseline_hashes` and `turb_models` unchanged and passing | bit-identical | PASS |
| P6 schema and tree run | `crates/rcfd-api/tests/realizable_a0.rs` passes | as registered | PASS |

For reference, the default (A0 = 4.04) series of ADR 0015 is 6.234 / 6.413 / 6.520 (coarse / medium / fine); with A0 = 4.0
every mesh is lower by 0.060 / 0.064 / 0.067.

Reading, with the caveats registered above:

* A0 = 4.0 moves the Realizable medium-mesh value from a miss (6.413, 0.053 above the window) to a pass (6.349, 0.011
  below the upper edge). The P4 hypothesis (that a 0.4 percent change of C_mu cannot flip the verdict) was wrong; the
  response is larger than that estimate, about 0.064 for a 0.4 percent C_mu change, so the step flow is sensitive to the
  C_mu level.
* The pass is marginal by the registered definition: it is within 0.14 h of a window edge (0.011), it is smaller than the
  0.14 shift that second-order k/epsilon convection causes (ADR 0015), and it fails the grid check P3 (|fine - medium| =
  0.105). On the fine mesh x_r = 6.453 is outside the window (0.093 above the edge), and x_r still increases with
  refinement. The window verdict therefore depends on the mesh used; the pass is not a grid-converged result and it is
  not evidence that 4.0 makes the model correct, only that the previous 0.05 excess was within the sensitivity to A0.
* The default stays 4.04. The decision to change it is left to the user. The ignored test
  `step_c_realizable_a0_4_grid_study_known_miss` records the P3 miss; `step_c_realizable_a0_4_window` passes (ignored as
  heavy). The old Realizable known-miss tests (A0 = 4.04) stay as they are and still miss.
* No set-up defect was found. The old set-up (ADR 0015 "old") was not rerun with A0 = 4.0.
* F-042 stays `partial`: of the registered step window only RNG meets it grid-converged; Realizable with A0 = 4.0 meets it
  on the medium mesh only.
