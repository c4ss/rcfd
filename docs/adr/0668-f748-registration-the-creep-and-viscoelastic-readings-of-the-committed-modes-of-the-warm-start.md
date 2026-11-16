# ADR 0668: F-748 registration: the creep and viscoelastic readings of the committed modes of the warm start (the remedy W of ADR 0660) on the quasi-static time-stepping path of ADR 0489

Status: **REGISTERED 2026-11-16, written before any code of F-748 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-748 was written, compiled or run: the files listed under "What was
read" were read and the arithmetic below was done by hand (disclosed). No probe was run for this item. Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-748 (deps F-742), the follow-up proposed by
[ADR 0666](0666-f742-registration-the-warm-start-of-the-incompatible-mode-iteration-on-a-history-material-and-on-the-transient-and-rotating-paths.md)
(its Limits and remaining question 2, option (a)). The element is the incompatible-modes Hex8 at finite strain of ADR 0608
with the stabilisation rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L); W is the remedy of
[ADR 0660](0660-f740-registration-a-warm-start-of-the-incompatible-mode-iteration-beside-a-trust-region-on-the-mode-step.md)
reading c3 (warm start, backtracking, decrease test, no line search); the cold control S is the variant of ADR 0658
(backtracking and decrease, no warm start); the time-stepping path is `nonlinear::solve_in_time` /
`solve_in_time_with` of [ADR 0489](0489-f411-registration-power-law-creep-time-hardening.md) D5. Number 0668 given by the
lead. Branch `f748-work` from 7b0740f2 (F-740 / ADR 0660 and F-742 / ADR 0666 merged), worktree `w49i-work/f748`, scratch
`w49i-work/f748-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour only with the opt-in `set_incompatible_mode_warm_start(true)` of ADR 0660); no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-748 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. W of ADR 0660 with a creep material and a viscoelastic material on a small Incompatible block through the time-stepping path of ADR 0489: no reader of the element states (creep_increment and every other) reads the nine committed modes as a point, a solve that panics or reads them today shown first, the results equal to the cold S to a registered tolerance; the mutations of the new code caught by the item's own checks; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** W with a creep material on a small Incompatible block through
`solve_in_time`; **c2** the same with a viscoelastic material; **c3** no reader of the element states reads the nine committed
modes as a point (`creep_increment` and every other: the audit below, each reader listed); **c4** a solve that panics or reads
them today shown first (before the fix); **c5** the results of c1 and c2 equal to the cold S to a registered tolerance; **c6**
the mutations of the new code caught by this item's own checks; **c7** the default bits unchanged.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0666 (whole), ADR 0660 (D1), ADR 0489 (D3 to D5).
- `crates/rcfd-fem/src/nonlinear.rs`: `point_update`, `evaluate`, `evaluate_fbar`, `hex8_evaluate`, `initial_states`,
  `check_fields`, `internal_force_field`, `force_and_tangent` (the two Incompatible call sites with the warm flag),
  `yielded_points`, `StateLayout`, `attempt_with` around the record, `solve_with` (the refusal of creep and viscoelastic
  materials on the static solve), `solve_from_field_reusing`, `is_creep_iteration`, `creep_increment`, `solve_in_time_with`,
  `thermal_start`, `TimeStep`, `TimeSteppingOptions`.
- `nl_incompatible.rs`: `check_state`, `extended`, `evaluate` (the warm split), `warm_state`, `modes_and_strains_warm`.
- `nl_transient.rs`: `NlTransientState::to_bytes`, `from_bytes`, `Integrator::new` (the layout), `restore` (ADR 0666 D3),
  `strain_energy` (ADR 0666 D2), the sub-step; `nl_transient/emc.rs` (`substep`).
- The other modules that take `&[ElementState]`: `arclength.rs`, `contact.rs`, `follower.rs` (`tangent_modes`),
  `harmonic_direct.rs` (`about_state`), `rotating.rs`; a search of `rcfd-api`, `rcfd-cli`, `rcfd-mcp` for element states
  (none read them).
- `mat_creep.rs` (`STATE_SIZE = 7`, the state layout, `equivalent_creep_strain`), `mat_visco.rs` (`state_size`, the update),
  `material.rs` (`Material::state_size`, `PronyTerm`), `model.rs` (the opt-ins).
- Tests: `si_eas.rs` (RF1, the creep row on an Incompatible block, cold), `si_eas_hg9.rs` (helpers, `S`, `W`, `model_of`),
  `sa_creep.rs`, `sa_qs_visco.rs` (the constructors).

## The audit of the readers of the element states (c3)

With W the committed state of an Incompatible Hex8 is its material state (`8 x state_size` values) followed by its nine
converged modes (ADR 0660 D1). Every place in the library that reads the values of an element state, by reading:

| # | Reader | How it reads | With W | Status |
|---|---|---|---|---|
| R1 | `nl_incompatible::evaluate` (the driver's two call sites in `nonlinear.rs`, `internal_force_field` and `force_and_tangent`, with `model.incompatible_mode_warm_start()`) | `warm_state` splits the material part (`8 x k`) and the modes, then `check_state` and `point_update` per point | correct | ADR 0660 D1 |
| R2 | `yielded_points` (the active-set record of ADR 0515 D2) | chunks of `k` | cut to `8 x k` with the opt-in | ADR 0666 D1 |
| R3 | **`creep_increment`** (`solve_in_time_with`, the sub-step acceptance of ADR 0489 D5 and `TimeStep::max_creep_increment`) | chunks of `mat_creep::STATE_SIZE = 7` of the whole state, index `6` of each chunk | **misreads**: `65 = 9 x 7 + 2` values give a ninth chunk of seven modes (mode `6` read as an equivalent creep strain) and a tenth chunk of two values whose index `6` is out of bounds: **a panic** from the first sub-step after `t = 0` (arithmetic 1) | **the defect of this item: D1** |
| R4 | `Integrator::restore` (`nl_transient.rs`) | the length per element against the layout | accepts `k` or `k + 9` with the opt-in | ADR 0666 D3 |
| R5 | `NlTransientState::to_bytes`, `from_bytes` | length-prefixed values per element | layout-agnostic | correct |
| R6 | `Integrator::strain_energy` | the modes from `modes_and_strains_warm` with the opt-in; a material with history refused (ADR 0479 D3) | correct; creep and viscoelastic refused before any reading | ADR 0666 D2 |
| R7 | `hex8_force_and_tangent_at`, `hex8_internal_force_at`, `hex8_second_piola_kirchhoff` (`hex8_evaluate`, warm `false`), the public checks of `nl_incompatible` (`condensed_force_and_tangent*`, `modes_and_strains*`, the records) | `check_state`: the length must be `8 x k` | a warm state is **refused** (`FemError::FieldLength`), not misread | correct (a refusal) |
| R8 | `evaluate` (Hex8 Full and the other solids), `evaluate_fbar` | the length must be `points x k` | not Incompatible elements: never a warm state | n/a |
| R9 | the pass-through paths: `solve_from_field*`, `arclength`, `contact`, `follower::tangent_modes`, `harmonic_direct::about_state*`, `nl_transient` sub-steps and `emc::substep` | hand the states to R1 (the assembly) | correct through R1 | correct |
| R10 | `StateLayout::offset`, `total` (public) | describe the material part of one flat array | no library reader flattens states by them; a caller that does would misread a warm state | documented (D2), not a reader of the library |
| R11 | `mat_creep::equivalent_creep_strain`, `creep_strain`, `mat_j2::equivalent_plastic_strain`, `plastic_strain` (public, one point's slice) | the slice the caller passes | correct when the caller slices the material part | not a reader of the library |
| R12 | `rcfd-api`, `rcfd-cli`, `rcfd-mcp`, the Studio crates | do not read element states | n/a | n/a |

So with a creep material R3 is the only reader on the time-stepping path that reads the modes as a point; with a
viscoelastic material no reader on that path does (R3 skips an element that is not creep, R2 is cut, R1 splits), so the
viscoelastic solve with W is expected to run today and to equal S (c4 for the viscoelastic material is the record that it
does, run before the fix).

## Arithmetic done before the code (disclosed)

1. **The panic (c4).** A creep Hex8 (8 points) has `56` material values; with W its committed state has `65`.
   `solve_in_time_with` calls `creep_increment(&creep, &start, &eq.states)` after the attempt at `t = 0` with `start` of 56
   values (the initial states) and `eq.states` of 65: the zip of the chunks stops at the 8 chunks of `start`, so the first
   entry is read correctly by accident. At the first sub-step both arguments have 65 values: chunks 1 to 8 are the points,
   chunk 9 is modes 0 to 6 (`pn[6] - po[6]` the change of mode 6, read as a creep increment), chunk 10 has 2 values and
   `pn[6]` panics ("index out of bounds: the len is 2 but the index is 6"). The panic is on the calling thread (the function
   is serial), so a `should_panic` record sees it.
2. **The creep set-up (c1).** The block of ADR 0608 RF1 amendment 1 (`si_eas.rs`), refined: `hex_box([0; 3], [2, 1, 1], [4,
   2, 2])` (16 elements, 45 nodes), all elements Incompatible with rule L, `xmin` clamped (three components), the traction
   `[0, 1e6, 0]` Pa on `xmax`, the creep material `Material::creep(steel, 5e-33, 4, 0)` on the steel of `si_eas.rs` (`E =
   2e11`, `nu = 0.3`, density `7850`, expansion `1.2e-5`). End times `[0.25, 0.5, 0.75, 1.0]` s,
   `TimeSteppingOptions::new(5e-4)` (the limit of RF1), the Newton defaults. The resultant `1e6` N (traction over the
   `1 x 1` m face) acts at `x = 2` m, so the bending moment at the clamp is `M = 2e6` N m and the outer-fibre stress `sigma = 6
   M / (b h^2) = 1.2e7` Pa; Norton rate `5e-33 x (1.2e7)^4 = 1.0e-4` 1/s; the elastic strain `6e-5`. Per step of `0.25` s a
   creep increment of about `2.6e-5` at the outer fibre (a stress concentration of 1.5 at the clamp corners gives `1.5^4 = 5`
   times: `1.3e-4`), below `5e-4`: no halving expected (reported, not a limit; W and S
   must agree on it, CR4). Over 1 s the outer-fibre creep strain is about `1e-4`, so the tip deflection grows by a factor of
   about 1.5 to 3 (CR6, `> 1.1`). The deformation is bending, so the modes are not zero and change with every step.
3. **The viscoelastic set-up (c2).** The same block, load and times; `Material::viscoelastic(steel, [PronyTerm::new(0.5,
   0.5, 0.25)], None)`: one branch with equal shear and bulk weights, so the whole elasticity relaxes uniformly to `E_inf =
   0.5 E_0`. Under a constant load this is the standard linear solid with retardation time `tau E_0 / E_inf = 0.5` s: the
   creep compliance `J(t) = 1/E_inf - (1/E_inf - 1/E_0) exp(-t / 0.5)`, `E_0 J(1) = 2 - e^-2 = 1.865`. At strain `6e-5` the
   geometric nonlinearity is negligible and the structural ratio is the material one: `u_y(1) / u_y(0)` about `1.865`, less
   the error of the update over steps of `tau` (exact for a strain linear in the step; the strain is not linear here): VE5 in
   `[1.6, 2.0]`. `state_size = 12`; the W state `96 + 9 = 105` values; `creep_increment` is `0` (no creep element); `TimeSteppingOptions::new(f64::INFINITY)` (as `sa_creep.rs`
   for a path without a creep limit), the Newton defaults.
4. **W against S (c5).** On every attempt W and S solve the same global equations; per element both local iterations meet
   the same test (`1e-12` of the mode scale, ADR 0608 D2) from different starts, so the converged modes differ by about that
   relative amount, the global iterates by the same order (ADR 0666 Addendum 1: W against S within `5.5e-13` and `4.0e-15`).
   The material states are functions of the converged strains: the same order. Estimate `1e-12` relative; limit `1e-9`
   (three decades: the creep update's local tolerance `1e-14` and the Newton tolerance do not amplify a `1e-12` difference
   by more than a few). The halvings and sub-steps of each step are decided by the creep increment against `5e-4`, which W and
   S compute within `1e-12` relative of a value expected at `2e-5` to `1.3e-4` (not near the limit): equal (exact).
5. **The test's own creep increment (CI).** With one sub-step per step, `TimeStep::max_creep_increment` of step `k` is the
   largest `s_k[6 + 7 p] - s_{k-1}[6 + 7 p]` over the creep points, computed by the same `f64::max` of the same differences:
   bitwise equal to the test's reading of the material parts of the committed states of steps `k - 1` and `k`.
6. **Cost.** 16 elements, 135 unknowns, five attempts per run, two runs per material: under a second each in release.

## Design

### D1. The creep reading with the committed modes (`nonlinear.rs`, `creep_increment`, `solve_in_time_with`)

`creep_increment(creep, cut, old, new)` takes `cut: Option<&[usize]>`, the number of material values per element. With
`Some(cut)` the values of element `e` (old and new) are cut to their first `cut[e]` values (`min` with the length; the
material part) before the chunks are read; with `None` the operations of 7b0740f2. `solve_in_time_with` passes, with
`model.incompatible_mode_warm_start()`, `Some` of `StateLayout::of(model)`'s `points(e) x values_per_point(e)` per element
(computed once, before the attempt at `t = 0`), and without it `None` at both call sites. No solve value depends on the cut;
only the acceptance of a sub-step and `TimeStep::max_creep_increment` read it. The cut is the rule of ADR 0666 D1.

### D2. Documentation

`StateLayout`: a doc sentence that with the warm start of ADR 0660 an Incompatible element's state carries nine more values
after the `points x values_per_point` of the layout (R10). `TimeStep::states`: the same note. No code.

### D3. Tests

- Unit test (`nonlinear.rs`, `#[cfg(test)] mod`), `creep_increment_reads_the_material_part_with_the_warm_start`: two
  elements, the first creep with 8 points and nine trailing values, the second not creep; the equivalent creep strain of
  point 7 (the last point) of the first element increases by `0.3`, of points 0 to 6 by `0.1`, the trailing nine values change
  by `+1e3` (mode 6 included) and the second element's values change by `5.0`. U1: `creep_increment(.., Some([56, 56]),
  ..)` equals `0.3` bitwise. U2: it equals `creep_increment(.., None, ..)` on the same states without the trailing values,
  bitwise. U3: `creep_increment(.., None, ..)` on states without trailing values and with no creep flag set is `0.0`.
- Integration file `crates/rcfd-fem/tests/si_eas_hg10.rs` (helpers copied from `si_eas_hg9.rs`; existing tests are not
  edited):
  - `p0_w_creep_panics_before_the_fix` (ignored record, `should_panic(expected = "index out of bounds")`): the W creep run of
    arithmetic 2. Run once with `--include-ignored` on the test commit before D1 (run 0); after D1 it no longer panics and
    stays ignored as the record of c4.
  - `w_creep_on_the_time_stepping_path`: W and S creep runs; CR1 to CR6, CI, NCP.
  - `w_viscoelastic_on_the_time_stepping_path`: W and S viscoelastic runs; VE1 to VE5, NCV.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses). "Material part" is the
first `8 x state_size` values of an element state. Relative differences are `max |a_W - a_S| / max |a_S|` over the named set.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| P0 | c4 | run 0 (before D1): the W creep run | panics, "index out of bounds" | arithmetic 1 |
| P0v | c4 | run 0 (before D1): `w_viscoelastic_on_the_time_stepping_path` | passes (every VE row met) | the audit: no reader on the path misreads |
| CR1 | c1, c3 | after D1: W creep `solve_in_time` | `Ok`, 5 entries | D1 |
| CR2 | c5 | creep: displacement of W against S over the 5 entries | `<= 1e-9` | arithmetic 4: `1e-12` |
| CR3 | c5 | creep: material parts of W's committed states against S's, over all entries and elements | `<= 1e-9` | arithmetic 4 |
| CR4 | c3, c5 | creep: `max_creep_increment` of W against S over the steps (relative to S's largest); halvings and sub-steps of every step | `<= 1e-9`; equal | arithmetic 4 |
| CR5 | c1 | creep: elements of W whose committed state at an entry has another length than `56 + 9` (and of S another than `56`) | `0` | ADR 0660 D1 |
| CR6 | c1 | creep: mean `u_y` of the `xmax` nodes of W, `t = 1` over `t = 0` | `> 1.1` | arithmetic 2: 1.5 to 3 |
| CI | c3 | creep, steps with one sub-step: W's `max_creep_increment` against the test's largest increment of the equivalent creep strain over the material parts of the committed states of the step and the one before; steps compared | bitwise equal; `>= 3` steps | arithmetic 5 |
| NCP | c3, c4 | the test's copy of the reading of 7b0740f2 (chunks of 7, index 6) on W's committed states of steps 1 and 2 (in `catch_unwind`); the same copy on S's states of steps 1 and 2 against S's `max_creep_increment` of step 2 (one sub-step) | panics; bitwise equal | arithmetic 1: the hazard D1 removes is in the states; the copy is the library's old reading |
| VE1 | c2, c3 | W viscoelastic `solve_in_time` | `Ok`, 5 entries | the audit |
| VE2 | c5 | viscoelastic: displacement of W against S | `<= 1e-9` | arithmetic 4 |
| VE3 | c5 | viscoelastic: material parts of W's committed states against S's | `<= 1e-9` | arithmetic 4 |
| VE4 | c2 | viscoelastic: elements of W whose committed state has another length than `96 + 9` (of S than `96`); `max_creep_increment` of W and S at every entry | `0`; `0.0` | ADR 0660 D1; no creep element |
| VE5 | c2 | viscoelastic: mean `u_y` of the `xmax` nodes of W, `t = 1` over `t = 0` | in `[1.6, 2.0]` | arithmetic 3: `1.865` |
| NCV | c3 | viscoelastic: elements of W whose committed state read in chunks of 12 gives another number of chunks than 8 | `= 16` (all) | the modes are there; no reader reads them so (audit) |
| U1, U2, U3 | c3, c6 | the unit test of D3 | bitwise as stated | D1 |
| B | c7 | the bit records `sa_creep_arrhenius_default_bits`, `sa_qs_visco_default_bits`, `sa_temp_history_default_bits`, `si_eas_default_bits`, `si_eas_hg_default_bits` | pass | `None` passed without the opt-in |
| S5 | c7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | c7 | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg9`, `si_eas_hg8`, `si_eas_hg7`, `si_eas`, `sa_creep`, `sa_creep_arrhenius`, `sa_qs_visco` (the existing tests of the changed lines) | pass | |

Reported without a limit: every difference W against S (the numbers behind CR2 to CR4, VE2, VE3), the halvings, sub-steps
and iterations of every step of the four runs, the tip deflections at every entry, the largest change of a committed mode
between steps 1 and 2 (what the old reading would have read in its ninth chunk).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| P0 | the W creep run before D1 | panics: the defect is real, not only read |
| NCP | the old reading on W's states (and on S's) | panics on W; equals the library on S: D1 is what removes the panic |
| NCV | the naive chunk reading of the viscoelastic W states | 9 chunks: the modes are in the state; only R3 would misread them, and it skips a non-creep element |

### Planned mutations (on a scratch copy of the D1 commit, each run once against the default tests of `si_eas_hg10.rs` and the unit test)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MC1 | `creep_increment` ignores the cut (the reading of before also with the opt-in) | CR1 (the panic), U1 (the panic) |
| MC2 | the cut one point long (`cut[e] + 7`) | U1 (mode 6 read as a point: `1e3`) |
| MC3 | the cut one point short (`cut[e] - 7`) | U1 (point 7 missed: `0.1`), possibly CI |
| MC4 | `solve_in_time_with` passes the cut only to the entry at `t = 0`, `None` in the sub-steps | CR1 (the panic) |
| MJ1 | the driver ignores the warm start (ADR 0660 MW1: `false` at both call sites of R1) | CR5, VE4 |

## Decision rule

- **done**: P0, P0v, CR1 to CR6, CI, NCP, VE1 to VE5, NCV, U1 to U3, B, S5, S6, S7 met in their single registered run; every
  mutation MC1 to MC4, MJ1 caught by a check of this item; no amendment.
- **partial**: B, S5 met, but another check missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B or S5 fails (a default bit changed), or the W creep run still fails after D1.

## Limits (not covered)

- One small block, one load level, one creep law (Norton time hardening with `m = 0`, no Arrhenius factor, no temperature)
  and one Prony branch; no distorted mesh; no comparison with F-bar (the item compares W with S, ADR 0666 compared with F-bar).
- The creep and viscoelastic materials on the nonlinear transient path with W: not run. By reading (R4 to R6, R9) the
  restore accepts the warm state, the bytes are layout-agnostic and the energy record refuses a history material before any
  reading; no time-path reader of `creep_increment` exists there.
- The audit is by reading of the library at 7b0740f2; a reader added later is not covered. The public `StateLayout::offset`
  and `total` (R10) and the point accessors (R11) are documented, not changed.
- The default stays cold (ADR 0660 open question 2).

## Open questions (taken or open)

1. **The design of D1** (the cut of `creep_increment` to the material part behind the opt-in, the rule of ADR 0666 D1) rather
   than a refusal of W with a creep material: taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation of ADR 0666 remaining question 2, option (a)).
2. **Default.** W stays opt-in (ADR 0660 open question 2): open for the maintainer, not taken.

## Addendum 1: results (2026-11-16)

Run 0 (before D1) was made once on 3a43cd85 with `--include-ignored` (`run0.log` in the scratch folder, 0.2 s): P0 panicked
in `creep_increment` ("index out of bounds: the len is 2 but the index is 6", `nonlinear.rs` line 2926), the creep test
failed by the same panic, the viscoelastic test passed (P0v). Run 1 (after D1) was made once on e8067fef (`run1.log`, the
default tests of `si_eas_hg10.rs` 0.3 s and the unit test). The viscoelastic test thus ran in both runs, as registered (P0v
and VE); every printed value is identical in the two. The mutations once each on a scratch copy of e8067fef
(`mutations.log`, `mut-M*.log`); S5, S6, S7, B once (`checks.sh`, `fmt.log`, `s7-lib.log`, `s7-tests.log`,
`s5-baseline.log`, `clippy.log`). No limit, set-up or measure was changed after a run; no amendment.

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| P0 | panics | panicked, "index out of bounds: the len is 2 but the index is 6" | yes |
| P0v | passes | passed (every VE row met before D1) | yes |
| CR1 | `Ok`, 5 entries | `Ok`, 5 | yes |
| CR2 | `<= 1e-9` | `6.4e-16` | yes |
| CR3 | `<= 1e-9` | `1.8e-14` | yes |
| CR4 | `<= 1e-9`; equal | `2.7e-15`; equal (no halving, one sub-step per step, 3 iterations each, W and S) | yes |
| CR5 | `0`, `0` | `0`, `0` | yes |
| CR6 | `> 1.1` | `1.213` | yes |
| CI | bitwise; `>= 3` steps | bitwise on all 4 steps | yes |
| NCP | panics; bitwise | the old reading panics on W's states; on S's equals S's `3.9615e-6` bitwise | yes |
| VE1 | `Ok`, 5 entries | `Ok`, 5 | yes |
| VE2 | `<= 1e-9` | `1.7e-16` | yes |
| VE3 | `<= 1e-9` | `1.4e-13` | yes |
| VE4 | `0`, `0`; `0.0` | `0`, `0`; `0.0` at every entry | yes |
| VE5 | in `[1.6, 2.0]` | `1.859` | yes |
| NCV | `16` | `16` | yes |
| U1 to U3 | bitwise | pass | yes |
| B | pass | `sa_creep_arrhenius_default_bits`, `sa_qs_visco_default_bits`, `sa_temp_history_default_bits`, `si_eas_default_bits`, `si_eas_hg_default_bits` pass | yes |
| S5 | pass | 3 passed, 1 ignored | yes |
| S6 | clean | fmt and clippy clean | yes |
| S7 | pass | `--lib` 15; `si_eas_hg9` 4, `si_eas_hg8` 5, `si_eas_hg7` 4, `si_eas` 8, `sa_creep` 9, `sa_creep_arrhenius` 6, `sa_qs_visco` 6 passed | yes |

Reported. The tip `u_y` of W and S agree to the printed seven digits at every entry (creep `1.7497e-4` to `2.1218e-4`,
viscoelastic `1.7497e-4` to `3.2529e-4`). The largest change of a committed mode between steps 1 and 2: `2.3e-7` (creep),
`7.0e-7` (viscoelastic); the largest creep increment per step `4.4e-6` to `3.4e-6`.

### Deviations

- Arithmetic 2 over-estimated the creep: the increments are `4.4e-6` per step (estimate `2.6e-5` to `1.3e-4`) and the tip
  ratio `1.21` (estimate 1.5 to 3; the limit `1.1` still met). Only the estimate was off; the check is the existence of
  creep. VE5 `1.859` against the closed-form `1.865`.

### Mutations (scratch copy of e8067fef, each run once)

| Id | Caught | By |
|---|---|---|
| MC1 (no cut) | **yes** | CR1 (the panic), U1 (the panic) |
| MC2 (cut one point long) | **yes** | U1 (`1e3`: mode 6 read as a point); not by the integration tests (the mode changes, `2.3e-7`, are below the creep increments) |
| MC3 (cut one point short) | **yes** | U1 (`0.1`), CI |
| MC4 (`None` in the sub-steps) | **yes** | CR1 (the panic) |
| MJ1 (the driver ignores the warm start) | **yes** | CR5 (`80`), NCP, VE4 (`80`), NCV (`0`) |

Five of five caught.

### Not run

Nothing registered was skipped. The whole rcfd-fem suite was not run (rule 7). Not run by design (Limits): creep and
viscoelastic materials with W on the nonlinear transient path, a distorted mesh, an Arrhenius or temperature case.

### Decision

**done**, by the registered rule: every check met in its single registered run, five of five mutations caught, no amendment,
the default bits unchanged (B, S5). The audit lists twelve readers; `creep_increment` was the only one on the time-stepping
path that read the modes as a point, and it now reads the material part with the warm start.

### Remaining questions

1. **Default.** W stays opt-in (ADR 0660 open question 2): open for the maintainer.
2. **The nonlinear transient path with creep or viscoelastic and W.** By reading correct (R4 to R6, R9); not run. Options:
   (a) a small check on the transient integrator; (b) leave it to the reading. Recommendation: (b): no reader there reads the
   state by points, and the restore is covered by ADR 0666 TS1. No follow-up proposed.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, intermediate tier. Follow-up of F-742 (ADR 0666). DONE. ADR 0668, 2026-11-16: the warm start W of ADR 0660 with a creep material (Norton, A = 5e-33, n = 4) and a viscoelastic material (one Prony branch, weights 0.5, tau 0.25 s) on a 16-element Incompatible block (rule L, cantilever under an end traction) through the time-stepping path of ADR 0489 (solve_in_time, four steps of 0.25 s), against the cold S of ADR 0658. An audit of the twelve readers of the element states (listed in the ADR) found creep_increment the only one that read the nine committed modes as a point; before the fix the W creep run panicked there (index out of bounds, kept as an ignored record), the viscoelastic run did not. Opt-in change: with the warm start creep_increment reads only the material part of each element (the rule of ADR 0666 D1). Met: W against S within 6.4e-16 (displacement), 1.8e-14 (creep states), 2.7e-15 (creep increments) and 1.7e-16, 1.4e-13 (viscoelastic), the same halvings and sub-steps, the reported creep increment equal bit for bit to the test's own reading of the material parts, the viscoelastic tip ratio 1.859 against the closed form 1.865. No default bit changed (five bit records, the rcfd-api baseline hashes). Five of five mutations caught. Not covered: creep and viscoelastic with W on the nonlinear transient path (correct by reading), a distorted mesh, the Arrhenius factor.

> F-742 (note, ADR 0668, 2026-11-16): the creep reading of the committed modes (creep_increment) is fixed behind the warm start (F-748); the viscoelastic material had no reader that misread them.
