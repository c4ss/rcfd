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
