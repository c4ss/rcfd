# ADR 0729: F-847 registration: the element entry points of the incompatible Hex8 given a committed state with the nine modes of the warm start (a length rule that leaves every current call bit-identical, the modes as the start of the local iteration, and the text of the transient restore's refusal)

Status: **REGISTERED 2026-11-17, written before any code of F-847 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-847 was written and nothing of this item was run (no probe). The
numbers quoted below come from the ADR texts and from reading the code; the arithmetic was done by hand (disclosed). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-847 (deps F-846), the follow-up proposed by
[ADR 0727](0727-f846-registration-what-switching-the-default-local-mode-update-of-the-incompatible-hex8-to-the-warm-start-would-move.md)
(its remaining questions 1 (b) and 3 (a)). Number 0729 given by the lead. Branch `f847-work` from b1f0ac2e (ADRs 0660, 0725,
0726, 0727 merged), worktree `w49i-work/f847`, scratch `w49i-work/f847-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing
test edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at
the end. **The warm-start default is not switched** (ADR 0727 remaining question 1, the maintainer's call). `arclength.rs` is
not touched (another item works on it).

## What F-847 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The public element-level entry points of the incompatible Hex8 (modes_and_strains and its variants, condensed_force_and_tangent and its variants, the mode iteration records, enhanced_force_and_tangent and its variants, and nonlinear::hex8_* with the Incompatible formulation) accept a state of the material layout followed by nine modes, by an opt-in or by a rule that leaves every current call bit-identical: the material part used as now and the modes as the start of the local iteration, measured against the driver's own evaluation on the ADR 0727 patch tests under the warm start; the transient restore's refusal names the first element and its two lengths; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the listed public entry points accept a state of the material layout
followed by nine modes; **c2** by an opt-in or by a rule that leaves every current call bit-identical; **c3** the material part
used as now and the modes as the start of the local iteration; **c4** measured against the driver's own evaluation on the ADR
0727 patch tests under the warm start; **c5** the transient restore's refusal names the first element and its two lengths;
**c6** the default bits unchanged.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0727 (whole, in particular class (iv) of NT, the NCR1 text and remaining questions 1 and
  3); ADR 0660 D1 (the warm start and its start rule: from the committed modes, and if that iteration fails, from `alpha = 0`);
  ADR 0666 D2 (`modes_and_strains_warm`, the crate-internal reading of the transient energy record) and D3 (the restore rule);
  ADR 0668 D1 (the cut of a warm state to its material part); ADR 0725 (the roadmap section); ADR 0572, 0573 (the style of the
  roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (whole: `check_state`, `extended`, `converged`, `trust_region`, `evaluate`,
  `warm_state`, every `pub fn`), `nonlinear.rs` 599 to 700 (`hex8_evaluate`, `hex8_force_and_tangent_at`,
  `hex8_internal_force_at`, `hex8_second_piola_kirchhoff`, `hex8_strains`), `nl_transient.rs` 1049 to 1100 (`restore`),
  `error.rs` (the display of `InvalidValue` and `FieldLength`).
- The tests: `si_eas.rs` (the patch test `pt1_to_pt4_pm_ps_patch_at_finite_strain` and its helpers), `si_eas_hg.rs`
  (`r_pt1_...`, `opted`, `STAB = true`), `f703/r_body.rs` with `si_eas_hg3_rd.rs` (`Deviatoric`) and `si_eas_hg3_rl.rs`
  (`LinearStress`), `si_eas_warm_default_f846.rs` (`bar`, `integrator`, NCR1), `si_eas_hg14.rs` (`element_modes`,
  `element_tangents`, `local`, `warm_states`), `si_eas_hg20.rs`, `si_eas_hg21.rs`, `si_eas_local_update_f844.rs`
  (`element_of`, `material_part`, `rec`, `commit_driver`), `sa_nl_transient.rs` (R2, the layout refusal).

## Readings, registered

### R1: who calls the entry points, and with what

`grep` over `crates/` for every listed entry point: the only callers outside `rcfd-fem/tests` are the entry points themselves
(`modes_and_strains` calls `modes_and_strains_with`, `enhanced_force_and_tangent` calls `_with`). No library code, no rcfd-api,
rcfd-post or rcfd-cli code calls them. In the tests:

- The default-model tests pass states of the material layout (the driver's states of a default model, `initial_states`, or
  states built by hand of `8 x state_size` values).
- The tests whose models set the warm start themselves (`si_eas_hg14`, `si_eas_hg20`, `si_eas_hg21`,
  `si_eas_local_update_f844`, `si_eas_warm_default_f846`) either cut the state to its material part before an entry point
  (`material_part` in `hg20`, `hg21`, `f844`), call `condensed_force_and_tangent_warm` (which already accepts both), or use
  the warm model only through the driver (`hg14`'s `warm_states` feeds `internal_force` of the warm model; its `element_modes`,
  `element_tangents` and `local` are given default states).
- No test asserts the refusal of a state of `8 x state_size + 9` values at an entry point: `grep` for `FieldLength`,
  `is_err`, `unwrap_err` near the entry points in the 35 files of the element finds none; the only `.ok()` over an entry point
  (`hg14` `element_modes`) is given default states.

So **every current call that succeeds passes exactly `8 x state_size` values**, and a call with nine more values is refused
today (`FieldLength { key: "state", expected: 8 ns, found: 8 ns + 9 }`) and that refusal is relied on by no caller.

### R2: why a rule, not an opt-in

An opt-in (a new argument or a new set of functions) would leave the four patch tests of ADR 0727 class (iv) broken under the
switch: they call `modes_and_strains` and `modes_and_strains_with` with the driver's states and cannot be edited. A rule on the
length makes them work without an edit and changes no call that succeeds today (R1). The rule is unambiguous: the material is
given to every entry point, so `n = 8 x material.state_size()` is known, and `n + 9 != n`.

## Design

### D1: the length rule (the same at every listed entry point)

Let `n = points x material.state_size()` (`points = 8`, the stiffness rule) and `L` the length of the given state.

- **`L = n`: today's code, unchanged.** The same function is called with the same arguments as before; every value and every
  error text is the same bits by construction (no arithmetic moves; the diff shows the branch).
- **`L = n + 9`: a warm state.** The first `n` values are the material state, used exactly as a material state is used today;
  the last nine are the committed modes (order `3 k + c`, the order of `evaluate`'s trial state, ADR 0660 D1).
- **Any other `L`: refused as today**, `FieldLength { key: "state", expected: n, found: L }` (the text of `check_state`, not of
  `warm_state`, so the error of every refused call is the same bits as today).

### D2: what each entry point does with the modes

| Entry point | Warm state (`L = n + 9`) |
|---|---|
| `modes_and_strains`, `modes_and_strains_with` | the crate's `modes_and_strains_warm` (ADR 0666 D2) with the update `Plain` (the update these entries use today): the local iteration from the committed modes; if it fails, from `alpha = 0` (ADR 0660 D1's start rule) |
| `condensed_force_and_tangent`, `_backtracked`, `_update` | `evaluate` with `warm = true` (the driver's own evaluation, ADR 0660 D1), the update of the entry (`Plain`, `Backtracking`, the given one); force, tangent and stress returned as today |
| `condensed_force_and_tangent_warm` | unchanged (it accepts both lengths since ADR 0660) |
| `mode_iteration_record` | the record of the iteration whose result the driver uses: from the committed modes; if its outcome is an error, the record and outcome of the iteration from `alpha = 0` |
| `mode_iteration_record_from`, `trust_region_record` (`start` given) | `start = Some(s)`: the caller's start wins (the material part used, the iteration from `s`, no fallback, as today); `start = None`: as `mode_iteration_record` (the committed modes, then `alpha = 0`) for `_from`; for `trust_region_record` the trust-region iteration from the committed modes, then from `alpha = 0` |
| `enhanced_force_and_tangent`, `_with` | the material part used; the trailing modes **ignored** (the entry evaluates the extended element at the modes given as an argument; there is no local iteration to start) |
| `nonlinear::hex8_force_and_tangent_at`, `hex8_internal_force_at`, `hex8_second_piola_kirchhoff`, `Incompatible` | `evaluate` with `warm = true` and the rule `Off`, update `Plain` (as today's `hex8_evaluate`) |
| the same with `Full` or `BBar` | unchanged: refused as today (the rule is of the Incompatible element only) |
| `hex8_strains` | unchanged (takes no state; refuses `Incompatible`) |

`point_gradients`, `stabilisation_coefficient*`, `stabilisation_energy*` take no state: unchanged.

Derivation of "the modes as the start" (c3): the committed modes are the converged root of the local equations at the
committed displacement (ADR 0660 D1). Started from them, Newton's local iteration (ADR 0608 D2) at the same displacement and
material state evaluates the residual at the root first; at the driver's own committed point that residual is the one the
driver accepted, so the iteration returns at iterate 0. The fallback keeps the result defined where the start is not in the
basin of the root (ADR 0660 D1).

### D3: the text of the transient restore's refusal (c5)

`Integrator::restore` keeps its test and changes only the text of the per-element case. The element-count case keeps today's
text. When the counts agree and element `e` is the first whose length is not accepted:

- default model, or warm model and an element that is not an Incompatible Hex8: `the material state of element {e} has
  {found} values where the model's layout has {k}`;
- warm model and an Incompatible Hex8: the same followed by ` (or {k + 9} with the nine modes of the warm start)`.

(The display of `InvalidValue` prefixes `key 'state': `.) This changes an error text only; no test asserts the old text (R1:
`grep "material states of\|model's layout"` over the tests finds none; `sa_nl_transient` R2 refuses on the element count,
whose text is kept, and asserts only the key).

## Instruments

- **New test file** `crates/rcfd-fem/tests/si_eas_warm_entry_f847.rs` (copies of the patch helpers of `si_eas.rs` and
  `si_eas_hg.rs`; existing tests not edited):
  - `wp_patch_tests_under_the_warm_start`: the five cases of the patch test (PT1 nu 0.3 and 0.4999, PT2, PT3, PT4) for the
    four rules (`Off` as `si_eas`, `RestStress` as `si_eas_hg`, `Deviatoric` as `si_eas_hg3_rd`, `LinearStress` as
    `si_eas_hg3_rl`): 20 solves of the 36-element patch on a model with the warm start set, the same increments as the
    originals.
  - `wu_rule_on_one_element`: the distorted Hex8 of `si_eas.rs` with the neo-Hookean case (e) and the J2 case (c) of `t1_...`.
  - `rt_restore_refusal_text`: the NT1 bar of `si_eas_warm_default_f846.rs` (4 elements, 3 steps).
  - `db_default_digest`: a digest (FNV-1a 64 over the `to_bits` of every value and the bytes of every error text) of every
    listed entry point given material-layout states and of the refusals of the wrong lengths of NC1, printed. It asserts
    nothing beyond finiteness; it is compared across the two copies (DB).
- **Scratch (never committed).** (i) `base`: `git archive` of b1f0ac2e plus the new test file, run of `db_default_digest`
  only (the other tests of the file need the new code). (ii) `flip`: `git archive` of the code commit with
  `incompatible_mode_warm_start: false` changed to `true` in `SolidModel::new` (as ADR 0727's flip), run of the four patch
  tests by name.

## Checks

The "driver's own evaluation" (c4) is `condensed_force_and_tangent_warm` (the public face of `evaluate` with `warm = true`,
the function the driver calls per element, ADR 0660 D1) given the same displacement, the same warm state, the same rule and
the same update. "Values differing" counts `to_bits` differences.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| WL | set-up | every committed state of the 20 warm solves has `n + 9` values | holds | ADR 0727 WL |
| A1 | c1 | the listed entry points given the warm states of the 20 solves (36 elements each) and the unit warm states: refusals | `0` | D1 |
| WD1 | c3, c4 | `modes_and_strains_with` (the solve's rule) against the driver's evaluation's trial modes, 20 x 36 x 9 values: values differing | `0` | the same `converged` call from the same start and material part |
| WD2 | c3, c4 | `condensed_force_and_tangent` (rule) force, tangent, stress against the driver's evaluation (`Plain`); `_backtracked` against it with `Backtracking`; `_update(BacktrackingWithDecrease)` against it with that update: values differing | `0` each | `evaluate` itself |
| WD3 | c3, c4 | the three `hex8_*` (the 5 `Off` solves) against the driver's evaluation with `Off`: values differing | `0` | `evaluate` itself |
| WD4 | c3, c4 | the modes of `modes_and_strains_with` against the committed modes (the state's last nine) | elastic (16 solves): `0` values differing; J2 (4): `max |d alpha| / (max |F - I| h) <= 2e-10` | elastic: the residual at the committed root is the one the driver accepted (D2 derivation), iterate 0; J2: the material state is the committed one, not the one the driver started from, so the root may move by roundoff; `2e-10` is twice the PM limit (both within PM by the triangle inequality) |
| WP | c4 | the patch tests' own rows under the warm start, their own limits: interior error `<= 1e-10` and PM `<= 1e-10` (20 each), PS `<= 1e-10` (4: `hex8_second_piola_kirchhoff` for `Off`, `condensed_force_and_tangent` otherwise, as the originals), PT4 every point yielded (4, read on the material part) | as stated | the originals pass on the default with the same limits; the warm solve moves the last bits only (ADR 0727 c5: `<= 7.8e-11` relative) |
| WI | c3 | case (e): `mode_iteration_record` cold: iterates; given the material state followed by the cold converged modes `a*`: iterates, outcome; `modes_and_strains` of that state against `a*` | cold `>= 2`; warm `1`, outcome `a*` with `0` values differing; `0` | iterate 0 at the root (D2) |
| WS | c3 | case (e): the warm state with `a* (1 + 1e-3)` through `modes_and_strains`: values differing from the cold modes; `max |d alpha| / max |a*|` | `> 0`; `<= 1e-8` | the start is used (another iterate sequence), the root is the same within the local tolerance `1e-12` of the residual scale, with a condition number of the mode block allowance of `1e4` |
| NC1 | c2 | lengths `n + 1`, `n + 8`, `n + 10`, `n + 18` for `n = 0` (case e) and `n = 104` (case c), at each of the 13 entry points of D2 that take a state (`modes_and_strains`, `_with`, `condensed_force_and_tangent`, `_backtracked`, `_update`, `mode_iteration_record`, `_from`, `trust_region_record`, `enhanced_force_and_tangent`, `_with`, `hex8_force_and_tangent_at`, `hex8_internal_force_at`, `hex8_second_piola_kirchhoff`): refused with `FieldLength { key: "state", expected: n, found: L }` | `104` of `104` | D1 |
| NC2 | c3 | case (e), the warm state with nine NaN modes: `modes_and_strains`, `condensed_force_and_tangent` and `mode_iteration_record` (iterates and outcome) against the cold calls: values differing | `0` each | the warm iteration fails at iterate 0 (residual not finite), the fallback is the cold iteration (ADR 0660 D1) |
| NC3 | c1 | `hex8_force_and_tangent_at` with `Full` and `BBar` given a state of `9` values on an elastic material | refused (`FieldLength`, key `"state"`), 2 of 2 | D2: unchanged |
| EX | c3 | case (e): `mode_iteration_record_from(warm state, Some(s))` and `trust_region_record(warm, Some(s))` against the same with the material state (`s = a* (1 + 1e-3)`): values differing (iterates and outcome); `mode_iteration_record_from(warm, None)` against `mode_iteration_record(warm)` | `0` each | D2 |
| EN | c3 | cases (e) and (c): `enhanced_force_and_tangent` and `_with(RestStress)` given the warm state (any trailing modes, also NaN) against the material state: values differing | `0` each | D2 (ignored) |
| J | c3 | case (c), J2 (`n = 104`): `condensed_force_and_tangent(Off)` and `modes_and_strains` given the material state followed by the cold modes against the driver's evaluation of the same state: values differing | `0` | as WD1, WD2 |
| RT1 | c5 | the default bar's integrator restoring the warm bar's state at step 3: the error's display | `key 'state': the material state of element 0 has 9 values where the model's layout has 0` | D3 |
| RT2 | c5 | the warm bar's integrator restoring that state with element 2's values replaced by 5 zeros | `key 'state': the material state of element 2 has 5 values where the model's layout has 0 (or 9 with the nine modes of the warm start)` | D3 |
| RT3 | c5 | the warm integrator restoring the state with its last element removed | `key 'state': material states of 3 elements that do not match the model's layout (4 elements)` (today's text) | D3 |
| RT4 | c5 | the warm integrator restoring the warm state unchanged | accepted | ADR 0727 R3 |
| DB | c2, c6 | `db_default_digest` on base and on the code commit | equal | D1 (the `L = n` path is the old code) |
| DBT | c6 | `si_eas_default_bits`, `si_eas_hg_default_bits`, `baseline_hashes` (rcfd-api, release) | pass | no default changes |
| FP | c4 | the flip copy, the four patch tests by name (`si_eas` `pt1_...`, `si_eas_hg`, `si_eas_hg3_rd`, `si_eas_hg3_rl` `r_pt1_...`): the failures that name `FieldLength` | `0` of 4 | see estimate E1 |
| S1 | | the existing tests named in the brief: `si_eas`, `si_eas_hg`, `si_eas_hg3_rd`, `si_eas_hg3_rl`, `si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8`, `si_eas_default_bits`, `si_eas_hg_default_bits`, `si_eas_local_update_f844`, `si_eas_warm_default_f846`, `--lib` (release) | pass | they pass material-layout states only (R1), except `f846` NCR1 (asserts only the refusal, kept) |
| S2 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |
| S3 | | every commit builds | builds | |

**Estimate E1 (registered, not a limit): the four patch tests under the switch still fail, at a test-side line.** Their PT4
set-up reads the J2 states by `chunks(mat_j2::STATE_SIZE)` and indexes `p[12]`; a warm state of `8 x 13 + 9 = 113` values gives
a last chunk of 9 values, so `p[12]` panics (index out of bounds) once PT1 to PT3 pass. Expected in the flip copy: each of the
four tests passes its PT1 to PT3 rows and panics at the PT4 set-up line, with no `FieldLength`. That reading of the state is in
the existing tests and is not edited here; it is an edit the switch item would need (reported, remaining questions).

### Negative controls and planned mutations (each on a scratch copy of the code commit, run once against the new file)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the rule cuts a warm state to its material part and drops the modes (start `None`) everywhere | WI (iterates `> 1`), WS (`0` differing) |
| MU2 | no fallback: the error of the warm iteration returned | NC2 (refused or differing) |
| MU3 | the restore's per-element text reverted to the count text | RT1, RT2 |
| MU4 | `mode_iteration_record_from` with `Some(s)` and a warm state starts from the committed modes | EX |
| MU5 | `enhanced_force_and_tangent_with` passes the whole warm state (no cut) | EN (refused) |
| MU6 | `hex8_evaluate` keeps `warm = false` | WD3, A1 (refused) |
| MU7 | lengths other than `n + 9` above `n` accepted as warm with the extra values ignored (the test `L > n`) | NC1 |

## Decision rule

**done** if every row is met, DB equal, DBT and S1 pass, and every mutation is caught; **partial** if a row misses (recorded
as `*_known_miss`, the limit intact), a clause is not run, or a mutation is not caught; **todo** if the rule cannot be built
without changing a current call's bits.

## Limits (what is not covered)

- The default is not switched; ADR 0727's records B1, B3 and its class (iii) rows are not touched.
- The PT4 set-up line of the four existing patch tests (E1) is not changed; under a switch they need that edit.
- The heavy ignored records of the element are not run.
- The static driver has no file save; the restore text is of the transient path only (ADR 0727 arithmetic 5).
- The rule is of the public element entry points; the assembled readings (`yielded_points`, the creep reading) already cut a
  warm state (ADR 0668 D1) and are not changed.

## Amendment 1 (2026-11-17, before the corrected run of the patch rows)

The registered run of the new file (`f847-scratch/run1.log`, once) made `wu_rule_on_one_element`, `rt_restore_refusal_text`
and `db_default_digest` pass and their rows are final. `wp_patch_tests_under_the_warm_start` **panicked at a defect of its
set-up**: it computed the exact stress `D E(F)` of the PS row for every case, and the J2 material of PT4 has no
element-constant elasticity matrix (`NotYetSupported { item: "material", variant: "J2", ... }`), so the first J2 case
(`si_eas` PT4) stopped the test before any of WL, A1, WD1 to WD4 was evaluated. The originals compute it for PT2 only. The WP
rows printed before the panic (`si_eas` PT1 nu 0.3, PT1 nu 0.4999, PT2 with PS, PT3: interior errors `1.7e-16` to `1.0e-15`,
PM `0`, PS `1.8e-15`, all within their limits) are kept as the record of that run.

Correction: `wp_patch_tests_under_the_warm_start_amended`, the same test with the exact stress computed for PT2 only. No limit,
case, row or parameter changes. The defective test stays as an ignored record. The corrected test is run once.

## Addendum 1: results (2026-11-17)

Runs, each made once: `f847-scratch/run1.log` (the new file as registered: `wu_rule_on_one_element`, `rt_restore_refusal_text`,
`db_default_digest` final; `wp_patch_tests_under_the_warm_start` panicked at the set-up defect of Amendment 1),
`run2.log` (`wp_patch_tests_under_the_warm_start_amended`, once), `db-base.log` (the digest on base), `fp-*.log` (the four
patch tests on the flip copy), `s1.log` (the named existing tests and `--lib`), `bh.log`, `docs.log`, `mut-MU*.log`. All release.

| Id | Limit | Measured | Met |
|---|---|---|---|
| WL | holds | holds (20 solves, every state `n + 9` values) | yes |
| A1 (patch) | `0` refusals | `0` | yes |
| A1 (unit) | `0` refusals | `0` | yes |
| WD1 | `0` | `0` of 6480 modes | yes |
| WD2 | `0` | `0` | yes |
| WD3 | `0` | `0` | yes |
| WD4 elastic | `0` | `0` | yes |
| WD4 J2 | `<= 2e-10` | `0` | yes |
| WP interior error (20) | `<= 1e-10` | largest `1.05e-15` | yes |
| WP PM (20) | `<= 1e-10` | `0` in every case | yes |
| WP PS (4) | `<= 1e-10` | `1.79e-15` each | yes |
| WP PT4 yielded (4) | holds | holds | yes |
| WI | cold `>= 2`; warm `1`, `0` differing; `0` | cold 4 iterates; warm 1, `0`; `0` | yes |
| WS | `> 0`; `<= 1e-8` | 9 of 9; `3.4e-15` | yes |
| NC1 | 104 of 104 | 104 of 104 | yes |
| NC2 | `0` | `0` | yes |
| NC3 | 2 of 2 | 2 of 2 | yes |
| EX | `0` | `0` | yes |
| EN | `0` (both cases) | `0`, `0` | yes |
| J | `0` | `0` | yes |
| RT1 | the registered text | equal | yes |
| RT2 | the registered text | equal | yes |
| RT3 | today's text | equal | yes |
| RT4 | accepted | accepted | yes |
| DB | equal | `0x2b98a16cbb48d51c` over 10387 values on base and on the code commit | yes |
| DBT | pass | `si_eas_default_bits` 1 passed, `si_eas_hg_default_bits` 1 passed, `baseline_hashes` 3 passed (1 ignored as registered there; 102 s with the build) | yes |
| FP | `0` of 4 name `FieldLength` | `0` of 4 | yes |
| S1 | pass | `--lib` 15; `si_eas` 8 (1 ignored), `si_eas_hg` 8 (1), `si_eas_hg3_rd` 6, `si_eas_hg3_rl` 6, `si_eas_hg6` 4 (2), `si_eas_hg7` 4 (2), `si_eas_hg8` 5 (2), `si_eas_default_bits` 1, `si_eas_hg_default_bits` 1, `si_eas_local_update_f844` 4 (3), `si_eas_warm_default_f846` 3; 94 s with the build | yes |
| S2 | clean | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | yes |
| S3 | builds | 45ded9e0 (docs), a07b039b (code and test, built), f4ff0ed1 (amendment, built), the results commit (docs) | yes |
| docs_site | pass | `cargo test -p rcfd-cli --release --test docs_site`: 2 passed | yes |

**E1 held.** On the flip copy each of the four patch tests passes its nine rows of PT1 to PT3 (with PS) through the public
entry points given the warm states, then panics at its PT4 set-up line (`index out of bounds: the len is 9 but the index is
12`, `si_eas.rs` 526, `si_eas_hg.rs` 906, `f703/r_body.rs` 518). The refusal of ADR 0727 class (iv) is gone; what is left is
a reading of the J2 state in the tests themselves, which assumes the material layout.

**A limit of c4 on the patch tests.** The patch fields are homogeneous, and the converged modes are exactly `0` in every case
(PM `0`). On the patch the committed modes are `0`, so starting from them is starting from `alpha = 0`. There WD1 to WD4 show
that the entry points accept the warm states and return the driver's own force, tangent, stress and modes bit for bit. They
cannot show that the modes are used as the start. The unit rows show that: WI (from the committed root, 1 iterate instead of
4), WS (another start, other last bits), NC2 (the fallback), EX, and the mutations MU1 and MU2.

### Mutations (each on a scratch copy of f4ff0ed1, the new file run once)

| Id | Mutation | Caught | By |
|---|---|---|---|
| MU1 | `warm_state` returns no modes (the start dropped everywhere) | yes | WI (4 iterates), WS (`0` differing) |
| MU2 | no fallback in `evaluate`, `modes_and_strains_warm` and the records | yes | NC2 (`inf`: refused), A1 (2 refusals) |
| MU3 | the per-element text reverted to the count text | yes | RT1, RT2 |
| MU4 | `mode_iteration_record_from` with `Some(s)` and a warm state starts from the committed modes | yes | EX (`inf`: the records differ in length) |
| MU5 | `enhanced_force_and_tangent_with` passes the whole warm state | yes | A1 (12 refusals). EN itself did not miss: a refused pair is counted in A1 and skipped in EN |
| MU6 | `hex8_evaluate` keeps `warm = false` | yes | A1 (180 refusals). WD3 itself did not miss, for the same reason |
| MU7 | `given_warm` true for every `L > n` | yes | NC1 (0 of 104: the refusals name `expected n + 9`) |

### Deviations

- The patch test as registered had a set-up defect (Amendment 1). Its corrected copy ran once and met every row.
- For MU5 and MU6 the registered catching rows were EN and WD3. The catch came from A1, because the test counts a refused
  call as a refusal and does not compare it. Every mutation was caught, but by a different row in these two cases.
- MU7 as run makes `given_warm` true for every length above `n`. `warm_state` then refuses with `expected n + 9`, so the error
  changes; the extra values are not silently ignored. The registered wording was "accepted ... with the extra values ignored".
  Both forms break the rule "refused as before", and NC1 catches this one.

### Not run

The heavy ignored records of the element; the other test files of the element that call the entry points with material
states only (`si_eas_hg14`, `hg20`, `hg21` and the rest; R1, rule 7, the daily gate); the whole rcfd-fem suite.

### Decision

**done**, by the registered rule. Every row was met: the patch rows after Amendment 1, which fixed a set-up defect before
their corrected run and changed no limit. DB was equal, DBT and S1 passed, and all seven mutations were caught (two by A1
rather than their registered row). One limit of c4 is recorded above: the patch modes are exactly zero, so the use of the
modes as the start is shown by the unit rows and the mutations, not by the patch tests.

### Remaining questions

1. **The PT4 set-up line of the four existing patch tests (E1).** Under a switched default they still fail, at
   `chunks(STATE_SIZE)` and `p[12]` on a 113-value state. Options: (a) the switch item edits those four lines to read the
   material part (`&values[..8 * STATE_SIZE]`); (b) leave the tests on the opt-out by setting the flag to `false`; (c) keep the
   opt-in. Recommendation: (a), as part of the switch item, which edits existing tests anyway (ADR 0727 remaining question 1).
   Not taken here: it edits existing tests, and the switch is the maintainer's decision.
2. **The default switch** stays open, as in ADR 0727 question 1. The element entry points no longer block it.

## For the lead: roadmap entries

> Proposed status: done.

> F-847 (ADR 0729, 2026-11-17): the public element entry points of the incompatible Hex8 accept a committed state of the warm start by a length rule. A state of the material layout (8 x state_size values) takes the code it took before. A state with nine more values is a warm state: its material part is used as now, and its last nine values are the committed modes, the start of the local iteration (if that iteration fails, it restarts from zero, ADR 0660 D1). Every other length is refused with the error it got before. The rule covers modes_and_strains{,_with}, condensed_force_and_tangent{,_backtracked,_update}, mode_iteration_record{,_from} and trust_region_record (a start given by the caller wins), enhanced_force_and_tangent{,_with} (the modes are given, the trailing ones ignored), and nonlinear::hex8_force_and_tangent_at, hex8_internal_force_at and hex8_second_piola_kirchhoff with Incompatible. On the ADR 0727 patch tests under the warm start (4 rules, 5 cases, 36 elements each), every entry point returns the driver's own evaluation bit for bit and every patch row meets its limit. The patch modes are exactly zero, so the use of the modes as the start is shown on one element: from the committed root 1 local iterate instead of 4, other bits from another start, and the restart from zero for NaN modes. The transient restore now names the first element and its two lengths. A digest of every default call and every refusal is equal before and after; the default bit records and baseline_hashes pass. Under a flipped default the four patch tests no longer fail at an entry point, but still fail at their own PT4 set-up line, which reads the J2 state by the material layout. Seven of seven mutations were caught.

> F-846 (ADR 0727, 2026-11-17), add at the end of its note: ADR 0729 makes the element entry points accept a warm state; under the switch the four patch tests of class (iv) then fail only at their own PT4 set-up line (index 12 of a 9-value chunk), an edit for the switch item.
