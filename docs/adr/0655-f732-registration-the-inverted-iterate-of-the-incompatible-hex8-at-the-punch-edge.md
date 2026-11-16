# ADR 0655: F-732 registration: the inverted iterate of the incompatible-modes Hex8 at the punch edge of a distorted mesh (ADR 0649 DB4: its cause, the driver's line search, and an opt-in backtracking of the mode update)

Status: **REGISTERED 2026-11-16, written before any code of F-732 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-732 was written, compiled or run: the files listed under "What was
read" and the logs of ADR 0649 in `w49i-work/f719-scratch` were read, and the arithmetic below was done by hand (disclosed). No
probe was run for this item. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-732 (deps F-719), the follow-up proposed by
[ADR 0649](0649-f719-registration-the-stabilised-incompatible-hex8-under-load-on-distorted-meshes.md) ("For the lead",
follow-up proposed; its remaining question 1, option (b)). The element is the incompatible-modes Hex8 at finite strain of
[ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) with the stabilisation of ADR 0614 D1 and the
rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L); the reference is the F-bar Hex8 of ADR 0506. Number
0655 given by the lead. Branch `f732-work` from main 5374d521 (F-719 / ADR 0649 merged), worktree `w49i-work/f732`, scratch
`w49i-work/f732-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden
file; no existing test edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that
the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-732 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The punch block of ADR 0649 on the 8^3 mesh at 0.25 h (seed 0xF719_0008) with rule L: the cause of the inverted point at 67.5 per cent of the load located (the incompatible-mode field of the element against the compatible one along the path); the driver's line search, and an opt-in backtracking of the mode update when a point inverts, each measured on the same set-up; if one passes the load, reaction and displacement within the ADR 0636 tolerance (10 per cent) of the F-bar Hex8 and the loaded state positive definite; the plain element on the 4^3 mesh at 0.25 h reported; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the set-up: the punch block of ADR 0649 on `8^3` at `a = 0.25` (seed
`0xF719_0008`), rule L; **c2** the cause of the inverted point located, by the incompatible-mode field of the element against
the compatible one along the path; **c3** the driver's line search measured on c1; **c4** an opt-in backtracking of the mode
update when a point inverts, measured on c1; **c5** if one of c3, c4 passes the load: `e_R`, `e_w` against F-bar `<= 0.10` and
the loaded state positive definite; **c6** the plain element (`Off`) on `4^3` at `a = 0.25` reported; **c7** the default bits
unchanged. The brief adds: any library change opt-in; `baseline_hashes` if a shared solver file changes; negative controls and
mutations registered before the run.

### Readings, registered

- **c1: the set-up.** Exactly that of ADR 0649 DB4: `hex_box` `[0, 1]^3` on `8^3`, the interior nodes moved by `0.25 h (2U -
  1)`, SplitMix64 seed `0xF719_0008` (the generator of ADR 0649 D2, copied), `neo_hookean(mu = 1e6, kappa(nu = 0.49))`, the
  confined punch (`xmin`, `xmax` `u_x = 0`, `ymin`, `ymax` `u_y = 0`, `zmin` `u_z = 0`, the punch `x, y <= 0.5` on `zmax` at
  `u_z = -0.1`), `NewtonOptions::new(10).threads(4)` (the driver's defaults otherwise), the measures `R = -sum_punch r_z / mu`
  and `w_c = u_z(1, 1, 1)`. The helpers of `si_eas_hg5.rs` are copied (existing tests are not edited).
- **c2: "located", "the incompatible-mode field against the compatible one", "along the path".** At an element displacement
  `u_e` the deformation gradient at the 8 points of the stiffness rule is `F = I + H_c + F_m`, `H_c = sum_a u_a (x) g_a` (the
  compatible part) and `F_m = sum_k alpha_k (x) G_k` (the incompatible-mode part, ADR 0608 arithmetic 1). The measures, per
  state, of element 476 (the inverted element of ADR 0649; `i = 4, j = 3, k = 7`):
  - `dc` = the smallest over the points of `det(I + H_c)` (the compatible field alone, `alpha = 0`);
  - `de` = the smallest `det F` at the converged modes;
  - `d1` = the smallest `det F` at the first local Newton iterate of the modes from `alpha = 0` (the full step);
  - `dmin` = the smallest `det F` over the local iterates of the plain iteration (stopped at the first inverted iterate);
  - `q` = the largest over the points of `|F_m| / |H_c|` (Frobenius norms) at the converged modes, and `|alpha|_inf`;
  - the number of local iterates, and the index of the first inverted one if any.

  "Along the path": the converged rule L states at the punch displacements `0.05`, `0.06`, `0.065`, `0.067` (10 increments
  each; ADR 0649 D4 converged at all four), and the trial `u_T = u(0.067) + (0.0678 - 0.067) / (0.067 - 0.065) (u(0.067) -
  u(0.065))` (a linear extrapolation to the punch at `0.0678`, inside the failing increment of ADR 0649, `0.06773` to
  `0.06781`). At `u_T` the backtracked iteration of D2 is run as well (does it converge, its `de`, its halvings).
  **Located** means exactly one of the registered hypotheses below holds by its criteria (check CA).
- **c3: the line search.** `NewtonOptions::line_search(true)` (ADR 0607 D1) on c1, nothing else changed. Its outcome (converged
  or the error) is the measurement; c5's limits apply only if it converges.
- **c4: the backtracking.** The opt-in of D1 below on c1, nothing else changed; the measurement as c3. Both together are run
  once and reported (no limit).
- **c5: the limits of a passing remedy.** `e_R = |R / R_Fbar - 1| <= 0.10`, `e_w = |w_c / w_Fbar - 1| <= 0.10` (ADR 0636, refined
  mesh), F-bar solved on the same mesh in the same test; the loaded state positive definite: no negative eigenvalue of the free
  block `K_ff` of the driver's tangent at the converged state (`count_below`, shift 0) and its lowest eigenvalue over its
  largest diagonal `> 0` (`lowest_eigenpairs`), SL of ADR 0649. The driver's tangent is that of the model, so with the opt-in it
  is evaluated with the backtracking.
- **c6: the plain element on `4^3` at `0.25 h`.** `IncompatibleStabilisation::Off`, the block of c1 on `4^3` (seed
  `0xF719_0004`): the default, the line search, the backtracking, each solved once; `e_R`, `e_w` against F-bar on the same mesh,
  SL, or the error; and the c2 measures of element 53 (the inverted element of ADR 0649 amendment 1) at `0.05`, `0.06`,
  `0.065`, `0.067`. Reported, no limit.
- **c7: default bits.** Without the opt-in the library performs the same operations in the same order (D1); the records are
  the bit tests `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs`,
  `cargo test -p rcfd-api --release --test baseline_hashes` (`nonlinear.rs` is a shared solver file), and, with the opt-in on a
  set-up that does not need it, the bitwise equality of the results (NC2).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0649 (whole: the registration, amendments 1 and 2, Addendum 1 and its roadmap section);
  ADR 0636 (its tolerance rationale); ADR 0607 D1 (the line search) through its code; ADR 0572 and 0573 (the form of the
  roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (whole: `extended`, `ModeFactor`, `converged`, `evaluate`, the public functions),
  `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `Failure`, `classify`, `attempt_with`, `trial_slope`, `step_length`,
  `solve`'s halving loop, the three call sites of `nl_incompatible::evaluate`), `model.rs` (`SolidModel`, the stabilisation
  setter), `rotating.rs` (`unit_centrifugal_model`), `nl_transient.rs` (the energy record of an Incompatible block),
  `hex8x.rs` (`ModePoint`, `mode_points`).
- `crates/rcfd-fem/tests/si_eas_hg5.rs` (whole) and the logs `run1-default.log`, `a1-diagnosis.log`, `a2-diagnosis.log` of
  ADR 0649.

## Arithmetic done before the code (disclosed)

1. **What the logs of ADR 0649 already say.** The failure is at the punch `0.0673` to `0.0678` for increments from `0.01` down
   to `0.0003` (D1, D2), always element 476, and the inverted `det F` is large: `-8.05e-2` (registered run), `-5.69e-2` (D2,
   40 increments), **`-3.84`** (D1, at point 2). The free tangent stays positive definite with its lowest eigenvalue within 5
   per cent of its value at half the load (D4). A trial a fraction `3e-4` of the punch beyond a converged state cannot move the
   compatible field of one element far enough to make `det(I + H_c) = -3.84` from a positive value: the compatible field moves
   by `O(increment)`. A `det F` of `-3.84` is a large `F`, and the only large unknowns of the element evaluation are the
   iterates of the modes. The local iteration of ADR 0608 D2 starts every evaluation at `alpha = 0` and takes the full Newton
   step; when the converged modes are large (an element strongly enhanced at the singular punch edge), the first step from 0
   can overshoot into an inverted `F` at a point although the converged modes do not invert. Since the converged modes and
   the first iterate are smooth in `u`, the overshoot appears beyond a threshold of `u` along the path, whatever the increment:
   this is the wall at `0.0678`. **Hypothesis H1** (expected). The alternatives: **H2**, the compatible field inverts (`dc <=
   0`; then the iteration fails at `alpha = 0` and no update of the modes helps); **H3**, the converged enhanced field itself
   approaches inversion (`de -> 0`; a limit of the element, no root with `det F > 0` beyond the wall).
2. **The line search cannot pass under H1** (first principles, the code of ADR 0607 D1). Iteration 0 of each attempt carries the
   prescribed increment and is not searched (`alpha = 1`); iterations `>= 1` are, and a failed trial evaluation is a bracket
   end (halved). The line search changes the global iterate, not the element's local iteration: under H1 every `u` beyond the
   threshold makes the element evaluation fail, the equilibrium at the punch `> 0.0678` is beyond it, so no attempt can converge
   there. Expected: the line search fails at load factor `0.675` to `0.68` (element 476). Registered as an expectation, not a
   limit (c3 is a measurement).
3. **The backtracking passes under H1** (first principles). Along a mode step `alpha(t) = alpha_j - t delta`, `F(t)` at each
   point is affine in `t`, `det F(t)` a cubic with `det F(0) > 0` (the iterate `alpha_j` was evaluated). Its smallest positive
   root `t*` exists if the full step inverts; halving `t` reaches `t < t*` after `ceil(log2(1 / t*))` halvings. With `H = 20`
   halvings any `t* >= 2^-20 ~ 1e-6` is reached. The damped Newton iteration then moves toward the converged modes, which
   under H1 have `det F > 0`; the full step is taken again once it no longer inverts, and the quadratic convergence of ADR 0608
   D2 returns near the root. The cap of 25 Newton steps (`LOCAL_ITERATIONS`) is kept (halvings are not counted as steps): with
   `t* ~ 0.5` per step (the overshoot of ADR 0649 is `det F` from about `+0.3` to `-3.8`, a cubic of the step), at most a few
   damped steps are expected. The modes at convergence are a root of the same equations, so the condensed force and tangent
   are those of ADR 0608 D2 at that root, and the tangent stays the exact derivative of the force (implicit function theorem);
   check T-FD. Expected values from the probe of ADR 0649 arithmetic 2 (which updated the modes by the linearised condensation,
   without the inversion test, and converged on this mesh): `R = 0.367871`, `w_c = 0.065734`, so `e_R = 0.0750`, `e_w = 0.0287`
   against F-bar `0.342202`, `0.063899`.
4. **Where the backtracking does not act, it changes no bit** (first principles, D1): the opt-in only alters what happens after
   an evaluation of the extended element returned the inversion error, which without it is returned at once. On a set-up whose
   default solve converges no such error occurs in a converged attempt; a failed attempt of the default solve (a halved
   increment) could differ. The set-ups of NC2 converge with no halving in ADR 0649 (every solve that converged took 10
   increments), so their results must be bitwise equal with and without the opt-in.
5. **T-FD** (the estimate of ADR 0649 arithmetic 5): the central difference of the force, step `1e-7`, against a consistent
   tangent, is within about `1e-9` relative; limit `1e-6` (ADR 0506 T1, ADR 0649 FR1).
6. **The plain element on `4^3` at `0.25 h`** (c6): the probe of ADR 0649 converged (`R = 0.409471`, `e_R = 0.0687` against
   F-bar `0.383162`); rcfd failed at `0.675` (element 53). Expected: the same cause as c2 and the backtracking passes. Reported.
7. **Cost.** Each `8^3` solve takes seconds in release (ADR 0649: the whole of `si_eas_hg5.rs`, about 30 solves, 21.7 s). This
   item: about 12 solves on `8^3` and 8 on `4^3`; expected well under two minutes; no heavy record.

## Design

### D1. The opt-in backtracking of the mode update (library, `nl_incompatible.rs`, opt-in per model)

- In the local iteration `converged`: after a Newton step `alpha_{j+1} = alpha_j - delta` whose evaluation of the extended
  element fails with the inversion error (`key == INVERSION_KEY`, a point with `det F <= 0`), the step is halved,
  `alpha_{j+1} = alpha_j - 2^-m delta`, `m = 1, 2, ..., 20`, and evaluated again; the first that does not invert is the iterate,
  and the iteration continues from it (the next step full again). The halvings of a step are not counted as local iterations;
  after 20 halvings, or an inverted `alpha = 0`, or any other error, the error is returned as before. The convergence test,
  the tolerance, the cap of 25 steps and the condensation at the converged modes are those of ADR 0608 D2. Without the opt-in
  the error is returned at once: the code path and the arithmetic of ADR 0608, 0614, 0630 are unchanged.
- Opt-in: `SolidModel::set_incompatible_mode_backtracking(on: bool)` (default `false`) and the getter
  `incompatible_mode_backtracking()`; the driver's two evaluations of an Incompatible Hex8 (`internal_force_field`,
  `force_and_tangent` in `nonlinear.rs`) pass the model's flag; `hex8_evaluate` (the formulation-only evaluation of
  `hex8_force_and_tangent_at`) passes `false`. The energy record of `nl_transient.rs` takes the model's flag (the modes of its
  energy are those of the driver; `false` unchanged), the centrifugal copy of `rotating.rs` keeps it (as ADR 0614 D4).
- For checks: `nl_incompatible::condensed_force_and_tangent_backtracked` (the element with the backtracking, for T-FD) and
  `nl_incompatible::point_gradients(coords, displacement, modes)` (per point of the stiffness rule the compatible gradient
  `H_c` and the mode part `F_m`; the measures of c2).
- Source: the backtracking of a Newton step to keep the iterate in the domain of the function is the standard damped Newton
  method (e.g. Dennis and Schnabel, Numerical Methods for Unconstrained Optimization and Nonlinear Equations, 1983, the
  backtracking line search, cited from memory); used here with the domain test `det F > 0` only (no sufficient-decrease test),
  derived in arithmetic 3.

### D2. The test file `crates/rcfd-fem/tests/si_eas_hg6.rs`

Helpers copied from `si_eas_hg5.rs`; the local iteration of the modes replicated in the test for the c2 measures (the extended
force and tangent by `enhanced_force_and_tangent_with`, the 9 x 9 mode block solved by Gaussian elimination with partial
pivoting, `det F` from `point_gradients`; stopped at `|delta|_inf <= 1e-12 max(1, |alpha|_inf)` or 25 steps or the first
inverted iterate; the backtracked variant halving as D1). The replica's converged modes are compared with the library's
`modes_and_strains_with` at the path states (reported: `max |alpha_replica - alpha_lib| / |alpha_lib|_inf`).

- `ca_cause_of_the_inverted_point`: the path states and `u_T` of reading c2, the measures, the classification CA.
- `db4_line_search_and_backtracking`: NC1, NC3, the line search (c3), the backtracking (c4) with DB4b, SLb, T-FD and BA, both
  together (reported).
- `nc2_backtracking_changes_no_bit_where_not_needed`: NC2.
- `plain_element_4_cubed_reported`: c6.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| CA | c2 | the classification of the cause: exactly one of H1, H2, H3 holds by the criteria below | one holds | H1 (arithmetic 1) |
| LS | c3 | the line search on c1: converged, or the error (element, load factor) | measured (no limit); if converged, DB4l and SLl | fails at `0.675` to `0.68` (arithmetic 2) |
| DB4l | c5 | only if LS converges: `e_R`, `e_w` | `<= 0.10` each | |
| SLl | c5 | only if LS converges: negative eigenvalues; lowest / max diagonal | `0`; `> 0` | |
| BT | c4 | the backtracking on c1: converged, or the error | measured; if converged, DB4b and SLb | converges (arithmetic 3) |
| DB4b | c5 | the backtracking: `e_R`, `e_w` | `<= 0.10` each | `0.0750`, `0.0287` (probe) |
| SLb | c5 | the backtracking at its loaded state: negative eigenvalues of `K_ff`; lowest / max diagonal | `0`; `> 0` | 0; about `2e-3` (ADR 0649 D4) |
| P | c5 | one of LS, BT converges with its DB4 and SL rows met | holds | BT |
| T-FD | c4 | at the loaded state of BT, element 476: `max |K - K_fd| / max |K|` of `condensed_force_and_tangent_backtracked`, central differences of its force, step `1e-7` | `<= 1e-6` | `~1e-9` (arithmetic 5) |
| BA | c4 | at the loaded state of BT, element 476 evaluated without the backtracking (`condensed_force_and_tangent_with`, rule L) | returns the inversion error | H1: the converged state is beyond the threshold |
| NC1 | c1, c7 | the default (no line search, no backtracking) on c1 | fails, element 476, at a load factor in `[0.67, 0.68]` | ADR 0649 |
| NC2 | c7 | rule L on the `8^3` block at `0.1 h` and on the `10 x 2 x 2` cantilever at `0.25 h` (ADR 0649 DC2's set-up), with and without the backtracking: displacement and reaction values | bitwise equal | arithmetic 4 |
| NC3 | c5 | F-bar on c1: `|R / 0.34220247 - 1|`, `|w_c / 0.06389947 - 1|` (the values of ADR 0649's registered run) | `<= 1e-6` each | rounding of the 8 printed digits (`1.5e-8`) |
| B6 | c7 | `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` | pass | |
| S5 | c7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | | the existing tests that exercise the changed lines: `cargo test -p rcfd-fem --release --lib`, `--test si_eas_hg5`, `--test si_eas_hg3`, `--test si_hex8x`, `--test si_eas` | pass | |

**The hypotheses of CA** (the measures of reading c2, element 476):

- **H1 (the mode iterate)**: `dc > 0` and `de > 0` at the four path states; at `u_T`: `dc > 0`, the plain iteration reaches an
  inverted iterate (`dmin <= 0`), and the backtracked iteration converges with `de > 0`.
- **H2 (the compatible field)**: `dc <= 0` at `u_T` or at a path state.
- **H3 (the converged enhanced field)**: `dc > 0` everywhere, and at `u_T` the backtracked iteration fails or converges with
  `de <= 0`, or `de` at `0.067` is at most `0.05` of `de` at `0.05`.

If none holds (for instance the plain iteration converges at `u_T`, so `u_T` is not a failing trial), the cause is not located
by these measures (CA missed). H2 and H3 can hold together only if the criteria overlap on a state; then CA reports both and
misses (not exactly one).

Reported without a limit: every c2 measure at every state, `q` and `|alpha|_inf` along the path, the replica against the
library; the increments and halvings of every solve; the step lengths of the line search if it converges; both remedies
together; F-bar's SL; c6 (all of it).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the default on c1 (checked above) | fails as in ADR 0649: the set-up reproduces the miss, and the opt-in is off by default |
| NC2 | the opt-in where nothing inverts (checked above) | bitwise equal: the backtracking acts only on an inverted iterate |
| NC3 | the F-bar reference against its recorded values (checked above) | the reference is the one of ADR 0649 |
| BA | element 476 at the BT loaded state without the backtracking (checked above) | the inversion error: the pass is due to the backtracking |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg6.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MB1 | the driver ignores the opt-in (passes `false` at both call sites) | BT, DB4b, SLb (the solve fails), P |
| MB2 | the halved step not halved (`2^-m` replaced by 1) | BT, P (the same failure as the default) |
| MB3 | the backtracking always on (the driver passes `true` at both call sites) | NC1 (the default passes); not NC2 |
| MB4 | at the first inverted iterate the previous iterate is returned as converged (no further step) | T-FD, or DB4b (a force not in modal equilibrium); BT may converge |
| MB5 | the halved step taken from the inverted iterate (`alpha_{j+1} - 2^-m delta`) instead of from `alpha_j` | BT, P |

## Decision rule

- **done**: CA, P (with its DB4 and SL rows), T-FD, BA, NC1, NC2, NC3, B6, S5, S6, S7 met in their single registered run; LS
  measured (its outcome recorded, its DB4l and SLl met if it converges); c6 reported; every planned mutation caught by at least
  one of the tests it is run against; no amendment.
- **partial**: B6, S5, NC2 met, but neither remedy passes (P missed), or a row of the passing remedy missed, or CA, T-FD, BA,
  NC1 or NC3 missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B6, S5 or NC2 fails (a default bit changed), or neither remedy can be run.

## Limits (not covered)

- One mesh, one seed, one material and Poisson ratio, one load case (the set-up of ADR 0649 DB4), plus the coarse plain
  element; no statistics over seeds; the other meshes of ADR 0649 only through NC2's bitwise equality.
- The backtracking tests only the domain (`det F > 0`), not a decrease of the mode residual or energy; a local iteration that
  oscillates without inverting is refused as before (25 steps).
- The element stays without state: every evaluation starts from `alpha = 0` (no warm start from the previous modes; that would
  add a state, a different design).
- The extrapolated trial `u_T` of c2 stands in for the driver's failing trial, which the driver does not return.
- `hex8_force_and_tangent_at` (formulation-only, no model) has no backtracking; the linear paths are unchanged (the modes are
  linear there).
- The default stays without the backtracking (open question 2).

## Open questions (taken or open)

1. **The design of the opt-in**: a per-model flag next to the stabilisation rule, the domain test only, 20 halvings per step:
   taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Default.** Making the backtracking the default would change default results only where the default fails today (a solve
   that errors), but it would change the bits of a failed attempt that later converges after a halving; it is open for the
   maintainer (not taken: it would change default results).

## Amendment 1 (2026-11-16, written after the single registered run and before any further run)

The registered run (test commit 5ae69e54, log `run1-default.log` in the scratch folder, 102 s) met CA (H1, the only hypothesis
that holds), NC1, NC2 (both set-ups), NC3, and measured the line search (it fails exactly as the default: element 476, load
factor `0.675`). **The backtracking alone does not pass the load**: it passes the wall of element 476 at `0.675` but fails at
load step 9 (load factor `0.890625` to `0.89375`, after 5 halvings) with `element 475: the modes of an Incompatible Hex8 did not
converge in 25 local iterations (relative mode residual 1.07e-1)`. So P, T-FD and BA missed (T-FD and BA have no BT state to be
measured at). **The two together pass** (reported, no limit by the registration): `R = 0.36787102`, `w_c = 0.06573382`, `e_R =
0.0750`, `e_w = 0.0287` against F-bar (the probe's `0.0750`, `0.0287` of ADR 0649), no negative eigenvalue, lowest over max
diagonal `1.81e-3`; 11 increments, 2 halved, 3 searched corrections. The plain element on `4^3` at `0.25 h` (c6) passes with the
backtracking alone (`e_R = 0.0687`, `e_w = 0.0376`, positive definite), and its cause is H1 too (element 53).

Records, as the house rules require: the test `db4_line_search_and_backtracking` is kept unchanged as the ignored record
`db4_line_search_and_backtracking_known_miss` (the limits intact, the measured values in its ignore text); no limit is changed.
The registered rule gives **partial** whatever follows (P missed); the combination is not counted as a pass of c5.

### A1.1 The met rows as the default test

`db4_met_rows`: NC1, NC3 and the line-search measurement of the registered test, same code and limits; it is a regression
test of rows already met (as ADR 0649 A1.1), not a new registered run.

### A1.2 Diagnosis (registered now, no limit, run once, ignored record `a1_diagnosis_bt_element_475`)

Questions: (i) why the backtracked local iteration of element 475 does not converge at load factor `0.89` with the backtracking
alone; (ii) at the loaded state of the two together, what T-FD and BA would have measured. Runs, each once, rule L on the `8^3`
mesh at `a = 0.25`:

- **E1**: the backtracking alone, the punch at `0.088` and at `0.089` (10 increments each, otherwise c4); if both converge, the
  trial `u_T = u(0.089) + 0.2 (u(0.089) - u(0.088))` (the punch at `0.0892`, inside the failing increment `0.0890625` to
  `0.089375`); at `u_T`, element 475: the c2 measures (`dc`; the plain replica's iterates) and the backtracked replica run to
  100 steps (not 25), its `|R_a|_inf` and smallest `det F` per iterate and its halvings; the library's backtracked and plain
  evaluations (Ok or the error). Interpretation registered: the backtracked replica converging after more than 25 steps (with
  the halvings spread over many steps) says the cap is hit by the slow damped iteration; not converging in 100 steps (a
  residual that does not fall below `1e-3` of its first value) says the damped iteration without a decrease test stalls or
  cycles (the limit registered under "Limits"); the plain replica converging at `u_T` while the backtracked fails says the
  backtracking itself leads the iteration astray. If a run fails, it is printed as such.
- **E2**: at the loaded state of the two together, element 476: T-FD's measure and BA's (as registered for BT), and the number
  of elements whose plain evaluation at that state returns the inversion error (the elements whose modes the backtracking
  holds). Reported, not counted as T-FD or BA.

## Amendment 2 (2026-11-16, written after the diagnosis of amendment 1 and before the mutation runs)

The mutations of the registration were planned against the default tests of `si_eas_hg6.rs` of the test commit, in which the
remedy test was a default test. They are run, each once, on a scratch copy of the **test commit 5ae69e54** (the registered
tests as registered), and since that remedy test already misses unmutated (P, T-FD, BA), **"caught"** is read as: a test that
passed in `run1-default.log` fails, or a row met there is missed, under the mutation. A mutation whose only effect is on rows
that already missed, or on reported rows, is recorded as not caught (with what changed in the reported rows). No limit and no
test is changed.

## Addendum 1: results (2026-11-16)

The registered run was made once on the test commit 5ae69e54 (`run1-default.log` in the scratch folder, 102 s, the four tests
with `--test-threads=1`); the met rows of A1.1 once (`run2-met-rows.log`, every row identical to the registered run); the
diagnosis of A1.2 once (`a1-diagnosis.log`); the mutations once each on a scratch copy of 5ae69e54 (amendment 2;
`mutations.log`, `mut-MB*.log`); B6, S5, S7 once (`b6.log`, `s5-baseline.log`, `s7-lib.log`, `s7-tests.log`, `s7-tests2.log`).
No limit, set-up, seed or measure was changed after a run.

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| CA | exactly one of H1, H2, H3 | **H1** only (element 476; criteria below) | yes |
| LS (c3) | measured | **fails as the default**: element 476 inverted, load factor `0.675` to `0.678`, after 5 halvings | measured |
| DB4l, SLl | only if LS converges | not applicable | |
| BT (c4) | measured | **fails**: passes `0.675`, then element 475, load factor `0.890625` to `0.89375`, the modes not converged in 25 local iterations (relative residual `1.07e-1`) | measured |
| DB4b `e_R`, `e_w` | `<= 0.10` | NaN (no state) | **no** |
| SLb | `0`; `> 0` | NaN | **no** |
| P | one remedy passes | neither alone | **no** |
| T-FD | `<= 1e-6` | NaN (no BT state); at the state of the two together `9.1e-10` (E2, reported) | **no** |
| BA | the inversion error | no BT state; at the state of the two together the plain evaluation **converges** (E2) | **no** |
| NC1 | fails, element 476, `[0.67, 0.68]` | fails, element 476, `0.675` (`det F = -8.05e-2`, as ADR 0649) | yes |
| NC2 block `8^3` `0.1 h`, cantilever `10 x 2 x 2` `0.25 h` | 0 bits differ | `0`, `0` | yes |
| NC3 | `<= 1e-6` | `8.9e-9`, `7.3e-8` | yes |
| B6 | pass | the four bit records pass | yes |
| S5 | pass | 3 passed, 1 ignored | yes |
| S6 | clean | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | yes |
| S7 | pass | `--lib` 14 passed; `si_eas_hg5` 3, `si_eas_hg3` 4, `si_hex8x` 8, `si_eas` 8 passed; also `si_eas_hg` 8, `si_eas_hg2` 3 (they exercise the transient energy and the rotating copy) | yes |

**CA in numbers** (element 476, rule L; `dc`, `de`, `d1` at the punch `0.05`, `0.06`, `0.065`, `0.067`): the compatible field
`dc = 0.907, 0.892, 0.886, 0.883`; the converged enhanced field `de = 0.984, 0.981, 0.980, 0.980`; the mode part `q = |F_m| /
|H_c| = 0.37, 0.35, 0.34, 0.34`, `|alpha|_inf = 2.9e-3` to `3.4e-3`. Neither field approaches inversion along the path. The
plain local iteration from `alpha = 0` is not monotone: at `0.067` its iterates have `det F` from `0.88` down to `0.878` and up
again (10 iterates). At the trial `u_T` (punch `0.0678`): `dc = 0.882`, the first iterate `0.962`, **the second iterate
`det F = -1.31`** (inverted), the library's plain evaluation fails (`det F = -0.746` at point 0, the same kind of value as the
`-8.05e-2`, `-5.69e-2`, `-3.84` of ADR 0649); the backtracked replica converges in 12 iterates with one halving to `de = 0.980`
(the converged modes are as valid as at `0.067`), and the library's backtracked evaluation converges. The replica agrees with
the library's modes to `8.8e-11` at the path states. So the inverted point is a **Newton iterate of the local mode iteration
overshooting** from a state at which both the compatible field and the converged enhanced field are far from inversion (`det
F >= 0.88`), not an inverted compatible field (H2) and not an exhausted element (H3).

**Reported (no limit).** The line search and the backtracking **together pass the load**: `R = 0.36787102`, `w_c =
0.06573382`, `e_R = 0.0750`, `e_w = 0.0287` against F-bar (`0.34220247`, `0.06389947`; the probe of ADR 0649 predicted `0.0750`,
`0.0287`), no negative eigenvalue of `K_ff`, lowest over max diagonal `1.81e-3` (F-bar `2.39e-3`); 11 increments, 2 halved, 3
corrections searched (shortest step `0.105`). These are within the ADR 0636 tolerance and positive definite, but the
combination was registered as reported, so it does not count as P.

**c6, the plain element on `4^3` at `0.25 h`** (element 53; F-bar `R = 0.38316202`, `w_c = 0.07044819`): the default and the
line search fail at `0.675` (element 53, `det F = -7.13e-2`); **the backtracking alone passes**: `e_R = 0.0687` (probe
`0.0687`), `e_w = 0.0376`, no negative eigenvalue, lowest over max diagonal `7.35e-3` (11 increments, 2 halved); the two
together the same values in 10 increments. Its cause is H1 too, and there the first iterate is the one that overshoots: `d1 =
0.979, 0.912, 0.696, 0.320` along the path and `-0.059` at `u_T` while `dc >= 0.915` and `de >= 0.976`.

**Diagnosis A1.2.** E1: the backtracking alone converges to the punch `0.088` and `0.089` (10 increments each, no halving). At
`u_T` (punch `0.0892`) element 475 has `dc = 0.771`; the plain iteration inverts at its first iterate (`det F = -6.28`); the
backtracked replica converges in **19 steps** (3 halvings), but not monotonically: after the first halved step its mode
residual rises from `1.6e5` to `6.7e6` and `det F` falls to `0.024` for six iterates before the quadratic phase; the library's
backtracked evaluation converges at `u_T`. So `u_T` is not the failing trial (none of the registered readings of E1 applies as
written: the iteration converges, under the cap); the driver's failing trial is a global Newton iterate farther from
equilibrium than `u_T`, where the same slow, non-monotone damped iteration needs more than 25 steps. E2: at the loaded state
of the two together element 476's backtracked tangent matches the central differences of its force to `9.1e-10`, and **no
element** needs the backtracking there: the plain evaluation converges on all 512 elements. The overshoot of the plain
iteration is a band of the path (entered at `0.675` of the load and left before the end), not a half-line.

### Deviations

- **P, T-FD, BA, DB4b, SLb** (amendment 1): the backtracking alone fails at `0.89` of the load; the known-miss record is
  `db4_line_search_and_backtracking_known_miss`, the met rows run as `db4_met_rows`.
- **Arithmetic 2 and 3** were half right: the line search alone fails as predicted, but the backtracking alone does not pass;
  the backtracking without a decrease test lets the damped local iteration wander (residual up by a factor 40, `det F` down
  to `0.02`) and the cap of 25 steps is reached at a far global iterate. The line search keeps the global iterates nearer
  equilibrium, where the damped iteration converges within the cap; together they pass.
- **BA's premise** ("the converged state is beyond the threshold") was wrong: the threshold is a band.
- **A naming slip in the registration**: BA was registered with `condensed_force_and_tangent_with`; the function is
  `condensed_force_and_tangent` (it takes the rule). Same measure.
- **c6**: the cause measures of element 53 were run with the plain element's own path (rule `Off`), as registered.

### Mutations (scratch copy of 5ae69e54, the default tests of `si_eas_hg6.rs`, each run once; amendment 2)

| Id | Caught | What changed |
|---|---|---|
| MB1 (the driver ignores the opt-in) | **no** | only rows that already missed (P, T-FD, BA) and reported rows: the backtracking runs fail at `0.675` like the default, the two together fail (element 476), the c6 backtracking fails (element 53) |
| MB2 (the step not halved) | **no** | the same as MB1 (the rows that change are reported ones) |
| MB3 (the backtracking always on) | **yes** | NC1: the default fails at element 475, load factor `0.8906` (not element 476 in `[0.67, 0.68]`); NC2 passes, as expected; c6's default now passes |
| MB4 (the previous iterate returned as converged) | **no** | the backtracking runs fail at `0.675` with a singular global tangent (pivot `-3.1e5`); the two together and the c6 backtracking fail |
| MB5 (the halving from the inverted iterate) | **no** | the backtracking runs fail at `0.675` (element 476, `det F = -3.53`); the two together and the c6 backtracking fail |

One of five caught. Every mutation of the backtracking is visible in the reported rows (the two together and the c6
backtracking stop passing), but the checks it was registered against are the ones that missed unmutated, and the passing
combination was registered as reported only. The registered checks do not guard the remedy that works.

### Not run

Nothing registered was skipped. The whole rcfd-fem suite was not run (the daily gate runs it, rule 7): `si_eas_hg6.rs`, the
lib tests, `si_eas_hg5`, `si_eas_hg3`, `si_hex8x`, `si_eas`, `si_eas_hg`, `si_eas_hg2`, B6 and S5.

### Decision

**partial**, by the registered rule: B6, S5, NC2 met (no default bit changed; the opt-in changes no bit where it is not
needed), CA met (the cause located: H1, an overshooting Newton iterate of the local mode iteration from `alpha = 0`, with the
compatible and the converged enhanced fields at `det F >= 0.88`), NC1, NC3 met, the line search and the backtracking each
measured; but neither alone passes the load (P, T-FD, BA missed) and four of five mutations are not caught. The two together
pass within the ADR 0636 tolerance (`e_R = 0.0750`, `e_w = 0.0287`) with a positive definite loaded state, reported, not counted.
The plain element on `4^3` at `0.25 h` is reported: it passes with the backtracking alone (`e_R = 0.0687`).

### Remaining questions

1. **Which remedy to register.** Options: (a) a follow-up that registers the line search with the backtracking as the remedy
   and checks it (with mutation-catching rows) on the eight distorted meshes of ADR 0649 and further seeds; (b) a sufficient
   decrease test in the backtracking (halve also when the mode residual does not fall), so that the backtracking alone passes,
   measured on the same set-up; (c) a warm start of the modes from the committed state (adds element state). Recommendation:
   (a) with (b) measured beside it: (a) is what passed here; (b) addresses the slow damped iteration that E1 shows. Proposed
   below.
2. **Default.** The backtracking (and the line search) stay opt-in; making them the default would change default results
   where a default solve fails today and where a failed attempt later converges. Open for the maintainer.
3. **The registered checks do not guard the passing combination** (mutations). Recommendation: the follow-up of 1 registers
   its checks on the combination, so the mutations of the backtracking are caught.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, intermediate tier. Follow-up of F-719 (ADR 0649 DB4). PARTIAL. ADR 0655, 2026-11-16: the inverted point of the stabilised incompatible-modes Hex8 (rule L) at the punch edge of the 8^3 block at 0.25 h (seed 0xF719_0008). Cause located: not the compatible field and not the converged enhanced field (both keep det F >= 0.88 along the path at element 476), but a Newton iterate of the element's local mode iteration from alpha = 0 overshooting into det F < 0 (the second iterate at -1.31 at the failing load), within a band of the path entered at 67.5 per cent of the load. New opt-in SolidModel::set_incompatible_mode_backtracking (default off; an inverted mode iterate halves its step, at most 20 times): no default bit changed (the bit records of ADR 0506, 0608, 0614, 0630 and the rcfd-api baseline hashes pass; with the opt-in, bitwise identical results on two set-ups that do not need it). Measured on the same set-up: the driver's line search alone fails as the default (0.675); the backtracking alone passes 0.675 but fails at 0.89 (element 475, the damped mode iteration not converged in 25 steps); the two together pass: punch reaction and corner displacement within 7.5 and 2.9 per cent of the F-bar Hex8, loaded state positive definite (reported, not a registered pass). The plain element on the 4^3 block at 0.25 h passes with the backtracking alone (6.9 and 3.8 per cent of F-bar), the same cause. One of five mutations of the backtracking caught by the registered checks (the others only change reported rows). Not covered: a sufficient-decrease test in the backtracking, the combination on other meshes and seeds, other materials.

> F-719 (note, ADR 0655, 2026-11-16): the DB4 known miss is located (an overshooting local Newton iterate of the modes, not a loss of stability or an inverted compatible field); with the opt-in backtracking of ADR 0655 and the driver's line search together rule L passes the 8^3 block at 0.25 h within the ADR 0636 tolerance (7.5 and 2.9 per cent), reported.

> Follow-up proposed: The line search with the backtracking of the incompatible-mode update as a registered remedy on distorted meshes (ADR 0655 question 1). Deps F-732. Acceptance: Every limit not stated here is registered in the ADR before the run. Rule L with NewtonOptions::line_search and the opt-in backtracking of ADR 0655 on the eight distorted meshes of ADR 0649 and on the 8^3 block at 0.25 h with three further registered seeds: punch reaction, corner bulge, tip reaction and shortening within the ADR 0636 tolerances of the F-bar Hex8 and the loaded state positive definite; a sufficient-decrease test of the mode update measured beside it on the same set-ups (does the backtracking alone then pass); the mutations of the backtracking caught by the item's own checks; the default bits unchanged.
