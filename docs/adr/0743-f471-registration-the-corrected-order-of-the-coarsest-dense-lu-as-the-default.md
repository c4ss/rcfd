# ADR 0743: F-471 registration: the corrected order of the coarsest dense LU as the default of the scalar AMG

Status: **REGISTERED 2026-11-19, written before any code of F-471 and before any run of its checks.** Before this text no
library or test code of this registration was written, nothing was built and no test was run. What was done is reading,
searches of the tree with `grep`, a sum over the test times of one earlier gate log, and the arithmetic below. Results go in
addenda at the end; nothing above them is changed after a run.

This is a further registration of roadmap item F-471 under rule 13: it adds no acceptance clause and makes the change the
item's gap names. It answers the brief [docs/adr/briefs/f471-coarse-lu-default.md](briefs/f471-coarse-lu-default.md) and
follows `docs/process/agent-rules.md`, version 3. Parent records:
[ADR 0369](0369-f474-registration-amg-definiteness-and-coarse-lu-order.md) (the defect, the opt-in, the scratch
measurements X1 to X3, its question Q1 and the maintainer's answer of 2026-11-18 at its end) and
[ADR 0363](0363-f466-registration-bounded-forcing-stop-on-t.md) (the stop of the T run and its records H1, H2, H3a, H3).
Read at main f67e106c. Branch `f471-coarse-lu-default`, scratch `w49i-work/f471-coarse-lu-default-scratch`.

## What is asked

The maintainer approved on 2026-11-18 that the corrected order of the coarsest dense LU solve becomes the default of
`rcfd_linalg::amg` ("Answers of the maintainer" of ADR 0369). This is an approved change of a default, so rule 3's "defaults
stay bit-identical" is lifted for exactly the results listed under "What changes, case by case" and for nothing else. The
approval does not cover a registered hash: `baseline_hashes.rs`, the case hash tables and every golden stay as they are, and
a hash that moves stops the work (decision rule).

The acceptance of F-471 in `docs/roadmap.json`, binding:

> A test shows whether the coarsest dense LU of rcfd_linalg::amg (DenseLu) interchanges rows on each fluid case of the
> baseline set and records where; DenseLu::solve applies every interchange before the forward substitution (as
> rcfd_linalg::amg_elastic after ADR 0305 amendment 1) so that a coarsest matrix that needs interchanges is solved to a
> relative residual of 1e-12; the rcfd-linalg, rcfd-fv and rcfd-api tests and the baseline hashes are run, and any changed
> hash is reported to the maintainer, not re-recorded.

Reading of the third clause, fixed by the brief: the tests that are run are the hash tests and the test files that use the
scalar AMG, by name (the list below with the reason for each); the whole suites are the lead's gate.

## What was read

- `docs/process/agent-rules.md` (version 3), `docs/process/tolerances.md`, the brief, ADR 0369 whole, ADR 0363 whole, ADR
  0741 (the newest registration of an approved default change, for the form), the F-471 entry of `docs/roadmap.json`.
- `crates/rcfd-linalg/src/amg.rs` whole; `backend.rs` lines 54 to 78 (`LinearOptions`); `matrix.rs` lines 8 to 45.
- `crates/rcfd-linalg/tests/amg_coarse_lu_f474.rs` whole; `crates/rcfd-fv/tests/immersed_bounded_stop_f466.rs` whole;
  `crates/rcfd-fv/tests/amg_definiteness_f474.rs` lines 1 to 30 and 255 to 785.
- Every place that builds a scalar AMG (`grep "Amg::new\|solve_amg"` over `crates/*/src`): `rcfd-fv` `fvmatrix.rs` 75 to
  78 (`FvMatrix::solve_amg`), `pressure.rs` 484, 500 and 620, `coupled.rs` 735, `overset.rs` 2027 and 2172, `potential.rs`
  99, `varden.rs` 564 and 1516; and what turns them on: `PimpleSettings::amg_pressure`, the VOF and overset
  `amg_pressure`, `CoupledSettings::amg` (default `true`), more than 2000 cells in `potential.rs` and `varden.rs`,
  `rcfd-api` `sim.rs` 2166 and `vof.rs` 839 (`amg_pressure = n_cells > 2000`).
- Every struct literal of `PressureControls` and of `LinearOptions` in the tree (`grep`).
- The test times of one full gate (`w49i-work/f849-gate-test-time-limit-scratch/runs/44276505780/.../times.tsv`), summed per
  test file for the list below: 11 400 s for the listed files, of which `immersed_solid_f219` 3350 s,
  `immersed_unsteady_force_f346` 3308 s, `immersed_piso_stop_f355` 2066 s and `vof_waves` 1543 s.
- The files of ADR 0363 H1 exist: `w49i-work/f466-scratch/runs` holds `state_2000.bin` to `state_2950.bin` and
  `h1_steps.csv`. They are read, never written, by the checks below.

## Readings, registered

**R1. The two orders differ only when the coarsest factorisation interchanges rows at a step `k >= 1`** (ADR 0369, reading
1 as corrected by its amendment 1). `DenseLu::new` swaps whole rows, so the stored factors satisfy `P A = L U`; the
interleaved solve is the solve of the other storage. With no interchange, or one at step 0 only, both orders perform the
same operations in the same order and give the same bits. The derivation is ADR 0369's on the 3 x 3 matrix `A3` (its
"Arithmetic before this registration"): interleaved `(-13, 15, -5)`, interchanges first `(1, 1, 1)` for
`b = A3 (1, 1, 1)`, both exact in binary floating point. The interchanges-first solve is the one of
`rcfd_linalg::amg_elastic` since ADR 0305 amendment 1. No other source is used.

**R2. The factorisation does not change.** `DenseLu::new`, the hierarchy (aggregation, prolongators, Galerkin operators) and
the smoothers are untouched, so `coarse_interchanges`, `coarse_pivots`, `level_sizes` and every operator are the same bits
under either order. Only `DenseLu::solve` takes another branch.

**R3. Which results can move.** Every result that passes through a scalar AMG cycle whose coarsest factorisation
interchanges at a step `k >= 1`. Known by measurement (ADR 0369): the pressure systems of the T run with the bounded
immersed forcing (coarsest 24 unknowns, steps 13, 16, 17 at step 2951 and 14, 16, 17, 20, 21 at step 2950). Known not to
move: the baseline set (ADR 0369 X1: only `kepsilon_channel` and `cavity-coupled` build an AMG, neither interchanges; X2:
`baseline_hashes` passed with the default switched), the M2 pressure systems of the DFG set-up (ADR 0369 F1: no
interchange), the 40 x 40 Laplacian (step 0 only, ADR 0369 L4b). Not known: every other AMG solve of the tree. The listed
test files are run to find them.

**R4. The old order has no caller that asks for it from a flow solver.** Every site that builds a scalar AMG passes
`AmgSettings::default()`. The records of ADR 0363 and ADR 0369 that assert the stop run a `PimpleSolver`, so an ignored
record of the old behaviour needs a way to ask for the old order through the pressure solve. `PressureControls` cannot get
a new field (two existing test files build it as a full struct literal and may not be edited); `LinearOptions`, which
`PressureControls` already carries as `linear` and `PimpleSettings` as `linear`, derives `Default` and is built as a full
literal only in two places of `rcfd-api` source.

## What changes, case by case

| Case | Before (the interleaved order) | After (interchanges first) |
|---|---|---|
| `Amg` with `AmgSettings::default()` on `A3`, `apply((2, 5, 7))` | `(-13, 15, -5)` (ADR 0369 L1) | `(1, 1, 1)`. **Changed.** |
| The same on the 20 x 20 tridiagonal of ADR 0369 L3 | relative residual 4.07e-3 | `<= 1e-12` (8.9e-19 measured for the opt-in). **Changed.** |
| DFG 2D-2 on the T mesh, bounded immersed forcing, `dt = 1e-3`, the default settings of ADR 0268 `run_2d2` (ADR 0268 R1, ADR 0363 H1 and H2) | stops at step 2951, pressure NaN | no stop; the run differs from the old one in the bits of every step after the first pressure solve whose coarsest factorisation interchanges at `k >= 1` (ADR 0369 X3, from the state of step 2900: Courant numbers differ from step 2917 on, no stop to step 2960, 10 iterations per solve). **Changed.** |
| The same with `pressure_ic_fallback` from the state of step 2950 (ADR 0363 H3a, H3) | one repeat, at (2951, 0, 1); to `t = 8`: max `C_D` 3.22695, max `C_L` 1.00253, `St` 0.30144 | no repeat; the extrema of a run to `t = 8` are measured here (check N6). **Changed.** |
| The stopping system `S` of ADR 0369 (step 2951 corrector 1) solved by default AMG-CG | `r^T z = -2.006e-9` after iteration 1, 3000 iterations, NaN | converged, every `r^T z > 0` (10 iterations measured for the opt-in, ADR 0369 H5). **Changed.** |
| Any other AMG solve whose coarsest factorisation interchanges at `k >= 1` | a coarse solve that is not the inverse | the exact coarse solve; other bits. **Changed, not known in advance (R3).** |
| Any AMG solve without an interchange at `k >= 1`; every solve that builds no scalar AMG; the elastic AMG | | the same bits. Not changed. |
| Every registered hash (`baseline_hashes`, the case hash tables) | | the same. Not changed (ADR 0369 X2); checked by rows A4 and A5. |

## Design

**D1. The default.** `AmgSettings::default()` has `coarse_lu_interchanges_first: true` (`crates/rcfd-linalg/src/amg.rs`).
The field stays, with its comment rewritten: `true` is the default by ADR 0743 on the maintainer's approval of 2026-11-18;
`false` asks for the interleaved order of before, kept so that earlier records can be reproduced. No line of
`DenseLu::new` or of either branch of `DenseLu::solve` changes, except a comment.

**D2. The way to ask for the old order** has two parts.

- On a hierarchy: `AmgSettings { coarse_lu_interchanges_first: false, ..AmgSettings::default() }`, as before.
- Through the pressure solve of the flow solvers: a new field `LinearOptions::amg_coarse_lu_interleaved: bool`
  (`crates/rcfd-linalg/src/backend.rs`), `false` by the derived `Default`, and a method
  `LinearOptions::amg_settings() -> AmgSettings` that returns the default settings with
  `coarse_lu_interchanges_first = !amg_coarse_lu_interleaved`. `FvMatrix::solve_amg_with(x, ctl, settings)` is new;
  `FvMatrix::solve_amg` calls it with `AmgSettings::default()` (the same operations as now). The three AMG builds of
  `crates/rcfd-fv/src/pressure.rs` (lines 484, 500, 620) use `ctl.linear.amg_settings()`. A test asks for the old order of
  a `PimpleSolver` with `solver.settings.linear.amg_coarse_lu_interleaved = true`.
- The two full literals of `LinearOptions` in `crates/rcfd-api/src/linsolver.rs` and `solid_parallel.rs` get
  `..LinearOptions::default()`. The new field is not a key of the case schema, so no golden is touched and no case can ask
  for the old order.
- Not reached by the flag: `coupled.rs` (the Schur complement), `overset.rs`, `potential.rs`, `varden.rs`. They keep
  `AmgSettings::default()`, so they get the corrected order and have no way back. No record of the old behaviour runs
  through them.

**D3. Existing tests that assert the old default.** Each is retired as an ignored record: its `#[ignore]` attribute names
ADR 0743 and says what replaces it, and no other character of it changes. Each is restated twice in the new files, under
the old order (asked for by D2) and under the new default.

| Existing test | What it asserts | Under the new default (prediction) | Old behaviour, restated | New default, restated |
|---|---|---|---|---|
| `rcfd-linalg` `amg_coarse_lu_f474::l1_default_interleaved_solve_is_wrong_on_a3` (ADR 0369 L1; runs in the gate today) | default `z = (-13, 15, -5)` | fails at its last assertion with `z = [1.0, 1.0, 1.0]` | N2 | N1 |
| `rcfd-fv` `immersed_bounded_stop_f466::h1_t_bounded_reproduce_and_checkpoint` (ADR 0363 H1, ignored, 0.5 to 1 h) | T from `t = 0` stops at step 2951 with the message of ADR 0268 R1 | would fail at "M1: H1 did not stop"; **not run**: it writes its states into `F466_DIR` and would overwrite the files of ADR 0363 | O1 (from the state of step 2900, against H1's own Courant numbers) | N6 |
| the same file, `h2_restart_capture_analysis` (ADR 0363 H2, ignored) | from the state of step 2900 the run stops at 2951 | fails at "M2: H2 did not stop" (seen in ADR 0369 X3) | O1 | N4 |
| the same file, `h3a_fallback_at_step_2951` (ADR 0363 H3a, ignored) | exactly one IC repeat, at (2951, 0, 1) | fails at "H3a: repeats" with an empty list | O2 | N5 |
| `rcfd-fv` `amg_definiteness_f474::h_stopping_system_definiteness` (ADR 0369 H, ignored) | from the state of step 2940 the run stops at 2951; the default hierarchy has `r^T z < 0` | fails at "H0: stop step" | O3 | O3 (its second half) |

Tests that keep passing but lose their meaning, because their "default" hierarchy and their "opt-in" hierarchy are now the
same settings: ADR 0369 L4b (`amg_coarse_lu_f474`), ADR 0369 F1 (`amg_definiteness_f474`), and the printed default residual
of ADR 0369 L3. They are not edited. N3b and P1 compare the old order with the new default in their place.
`immersed_bounded_stop_f466::h3_t_bounded_to_t8_with_fallback` (ADR 0363 H3, ignored) asserts nothing about the repeat; it
is not edited and not run, and its printed record would now show no repeat.

Documents that describe the old default are the roadmap notes of F-471, F-474 and F-466 (the lead's).

**D4. New test files.**

`crates/rcfd-linalg/tests/amg_coarse_lu_default_adr0743.rs` (header: ADR 0743, F-471), fast:

- N1: `AmgSettings::default().coarse_lu_interchanges_first` is `true`; a default `Amg` on `A3` gives `(1, 1, 1)`.
- N2: the old order on `A3`: `coarse_interchanges() == [1]`, `z = (-13, 15, -5)`.
- N3: the 20 x 20 tridiagonal of ADR 0369, `b = A x`, `x_i = 1 + i / 20`: default relative residual (A2); old-order
  relative residual (N3a); the 40 x 40 Laplacian with the right-hand side of ADR 0369 L4b: old order against default,
  differing values (N3b).

`crates/rcfd-fv/tests/amg_default_fluid_adr0743.rs` (header: ADR 0743, F-471). The DFG helpers, the state reader, the
window and the recorded CG are copies of those of `immersed_bounded_stop_f466.rs` and `amg_definiteness_f474.rs`.

- P1 (fast): DFG 2D-2 on M2, bounded forcing, 3 steps of `dt = 1e-3`, the old order (D2) against the default, with the
  pressure systems captured in the default run.
- P2 (fast): the 20 x 20 tridiagonal as an `FvMatrix` on a chain of 20 cells, solved from zero with `tol_rel = 1e-10`,
  `max_iter = 50`, by `solve_amg`, by `solve_amg_with` (default settings), by `solve_amg_with` (old order).
- O1 to O3, N4, N5 (ignored: they need the files of ADR 0363 H1 in `F466_DIR`; seconds to two minutes each) and N6
  (ignored, the heavy record), as in the table of checks. O1 to O3 set `linear.amg_coarse_lu_interleaved = true`.

N6 is T from `t = 0` to `t = 8` (8000 steps of `dt = 1e-3`) with the default settings and no fallback. Cost: ADR 0363 H3
took 2970 s for 5050 steps with four threads, so about 80 minutes; `RAYON_NUM_THREADS` 4. It prints the coefficients at
`t = 1` and `t = 2` beside those of ADR 0268 R1, the largest pressure iteration count of a step and its step, and the
extrema beside those of ADR 0363 H3; those prints have no limit.

## The test files that are run, and why each

Rule: a file is on the list if its source names the scalar AMG (`grep -i amg` over `crates/rcfd-linalg/tests`,
`crates/rcfd-fv/tests` and `crates/rcfd-api/tests`) and it has at least one test that is not ignored. Each is run by name,
unedited, in release, its ignored tests not run except where a row below says so.

- **Hash tests**: `rcfd-api` `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes`; `rcfd-fv` `varden_ideal_hash` (the
  variable-density solver takes `solve_amg` above 2000 cells).
- **`rcfd-linalg`**: `--lib` (the unit tests of `amg.rs`); `amg_coarse_lu_f474` (the checks of ADR 0369, builds 9
  hierarchies); `amg_contrast` (the scalar AMG on a coefficient contrast).
- **`rcfd-fv`**, `--lib`, and because the test sets `amg_pressure = true` on a PISO, PIMPLE or VOF solver:
  `consistent_correctors_f301`, `consistent_defaults_f301`, `consistent_extensions_f322`,
  `consistent_periodic_defaults_f367`, `consistent_periodic_f367`, `cylinder`, `immersed_bounded_stop_f466`,
  `immersed_dfg_convergence_f354`, `immersed_early_state_start_f501`, `immersed_lag_estimate_f483`,
  `immersed_lockin_f731`, `immersed_lockin_f750`, `immersed_lockin_f756`, `immersed_moving_f308`,
  `immersed_multistep_linearisation_f494`, `immersed_piso_stop_f355`, `immersed_single_corrector_f363`,
  `immersed_solid_f219`, `immersed_start_projection_f515`, `immersed_step_linearisation_f482`,
  `immersed_transverse_f707`, `immersed_unsteady_force_f346`, `piso_thin_gap_f299`, `sixdof_flow_f244`,
  `transient_buoyancy`, `vof_bubble_diagnosis_w41c`, `vof_bubble_refinement_w40d`, `vof_dambreak_amg`,
  `vof_height_function_w42b`, `vof_waves`; because it builds hierarchies itself: `amg_definiteness_f474`; because it sets
  the coupled solver's `amg`: `coupled`; because it builds `PressureControls` with the `amg` flag:
  `gradient_pressure_f291`.
- **`rcfd-api`**, `--lib`, and because the test reads or sets `amg_pressure` of a case run: `consistent_correctors_case_f301`,
  `w35b_lag_check`, `w48t_cylinder_spokes`; because it compares a VOF case with the library and names the AMG switch:
  `vof_wiring_tree`.
- **Named by the search and not run**: every test ignored (nothing would run): `rcfd-fv` `diag_cavity_ra1e6`,
  `warped_kernel_f285_diag`; `rcfd-api` `validation_cylinder_re100`, `validation_cylinder_re100_w31`,
  `validation_cylinder_re100_w33b`. The elastic AMG only (its own dense LU, not changed): `rcfd-linalg` `si_amg_elastic`,
  `rcfd-api` `sb_solid_case`, `sb_solid_case_iterative`.

## Checks

The acceptance clauses first (A1 to A5). Exact limits are from the tolerance table's row "Bit-identity of default output"
unless a source is given; where the estimate equals an exact limit the margin is written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | Clause "interchanges rows on each fluid case of the baseline set and records where": on a scratch copy of the code commit, an `eprintln` in `DenseLu::new` (size, steps with `piv[k] != k`) and in `baseline_hashes.rs`'s `run` (the head of the case text), `baseline_hashes -- --nocapture --test-threads=1`; per case the factorisations and their interchange steps are recorded | 0 factorisations with an interchange at a step `k >= 1` | ADR 0369 X1 (measured: none in 119 factorisations of two cases); tolerance table, bit-identity | 0 | 1 | unchanged default output (instrumented scratch copy) |
| A2 | Clause "applies every interchange before the forward substitution ... solved to a relative residual of 1e-12" (N1, N3): the default on `A3` gives `(1, 1, 1)` bit for bit; the default on the 20 x 20 tridiagonal has `norm(b - A z) / norm(b) <= 1e-12` | bit for bit; `1e-12` | acceptance of F-471; ADR 0369 arithmetic | exact; 8.9e-19 (ADR 0369 L3, opt-in) | 1; 1.1e6 | code verification |
| A3 | Clause "the rcfd-linalg, rcfd-fv and rcfd-api tests ... are run": every file of the list above, unedited, passes, with `l1_default_interleaved_solve_is_wrong_on_a3` ignored (D3) | 0 failures | rule 7; tolerance table, bit-identity | 0 (R3: no listed file is known to interchange at `k >= 1` outside its ignored records) | 1 | unchanged default output |
| A4 | Clause "the baseline hashes are run": `cargo test -p rcfd-api --release --test baseline_hashes`, unedited | every registered hash equal (the test passes) | rule 3 | equal (ADR 0369 X2) | 1 | unchanged default output |
| A5 | The case hash tests, unedited: `rcfd-api` `case_hashes_w48u`, `w48v_case_hashes`; `rcfd-fv` `varden_ideal_hash` | every registered hash equal (the tests pass) | rule 3 | equal | 1 | unchanged default output |
| N2 | The way back reproduces the old bits (D2): the old order on `A3` has `coarse_interchanges() == [1]` and `z = (-13, 15, -5)` | bit for bit | ADR 0369 L1 (the registered old result) | exact | 1 | unchanged default output (under the old order) |
| N3a | Negative control of A2: the old order on the 20 x 20 tridiagonal has a relative residual above the limit of A2 | `> 1e-12` | tolerance table (a negative control takes the limit of its check the other way round) | 4.07e-3 (ADR 0369 L3) | 4e9 | code verification (negative control) |
| N3b | Old order against default on the 40 x 40 Laplacian (interchange at step 0 only): `coarse_interchanges()` is a subset of `{0}`; `apply` differs in 0 values | subset of `{0}`; 0 | ADR 0369 L4b; bit-identity | `[0]`; 0 | 1 | unchanged default output |
| P1 | A flow case without an interchange at `k >= 1` does not move: M2, bounded, 3 steps, old order (D2) against default: `u`, `p`, `phi` differ in 0 values; none of the 6 captured systems has an interchange at `k >= 1` | 0; none | bit-identity; ADR 0369 F1 (measured: no interchange in the 6 systems) | 0; none | 1 | unchanged default output |
| P2 | `FvMatrix::solve_amg` uses the corrected order: on the tridiagonal, `solve_amg` converges with `norm(b - A x) / norm(b) <= 1e-10`; `solve_amg_with` (default settings) equals it in every bit and in the iteration count | `1e-10`; 0 differing | tolerance table, "Linear and eigenvalue solver residual"; bit-identity | below 1e-15 (the preconditioner is the exact inverse, so one iteration); 0 | 1e5; 1 | code verification |
| P2a | Negative control of P2: `solve_amg_with` (old order) differs from `solve_amg` | at least 1 differing value of 20 | bit-identity, the other way round | 20 | 20 | code verification (negative control) |
| O1 | ADR 0363 M2 under the old order: T restored from `state_2900.bin`, `linear.amg_coarse_lu_interleaved = true`, stepped to the stop | stop at step 2951; the message equal to ADR 0268 R1's character for character; every Courant number of steps 2901 to 2950 bit-equal to `h1_steps.csv` (0 differing) | ADR 0363 M2 (the registered old result) | as measured there | 1 | unchanged default output (under the old order) |
| O2 | ADR 0363 H3a under the old order: from `state_2950.bin`, old order, `pressure_ic_fallback`, one step | no stop; `pressure_fallbacks == [(2951, 0, 1)]`; pressure finite | ADR 0363 H3a | as measured there | 1 | unchanged default output (under the old order) |
| O3 | ADR 0369 H0, H1 and H5 on the stopping system: from `state_2940.bin`, old order, the capture on from step 2950. Old order: stop at 2951; the captured `S` (2951, corrector 1) has 3000 iterations and non-finite cells; its coarsest interchanges are `[13, 16, 17]`; AMG-CG on `S` with the old-order hierarchy has `r^T z` after iteration 1 printed `-2.006e-9`. New default on the same `S` from its start pressure: converged, every `r^T z > 0`, iterations `<= 100` | as stated; `<= 100` | ADR 0369 H0, H1, H5 (registered limits and measured values) | as measured there; 10 iterations | 1; 10 | code verification (the old order, then the new default) |
| N4 | ADR 0363 H2 under the new default: from `state_2900.bin`, steps 2901 to 2960 | no stop; 0 non-finite pressure values after every step | ADR 0369 X3 (measured on the scratch copy); acceptance of F-474 ("AMG-CG converges on that system") | no stop | 1 | code verification |
| N4a | Negative control of O1 (the default does move this run): of the Courant numbers of steps 2901 to 2950, the number that differ in bits from `h1_steps.csv` | at least 1 | bit-identity, the other way round; ADR 0369 X3 | 34 (from step 2917 on) | 34 | code verification (negative control) |
| N5 | ADR 0363 H3a under the new default: from `state_2950.bin`, `pressure_ic_fallback`, one step | no stop; `pressure_fallbacks` empty; pressure finite | ADR 0369 H5 (the default solve of `S` now converges, so nothing is repeated) | empty | 1 | code verification |
| N6 | The heavy record: T, bounded, `dt = 1e-3`, from `t = 0` to step 8000 with the default | no stop; S2 of ADR 0268 on the window from `t = 5.5`: crossings `>= 6`, period spread `<= 1e-2`, maximum spread `<= 2e-2`, amplitude `>= 1.0` | ADR 0268 S2 as registered again by ADR 0363 amendment 1 (H3) | 7; 5.45e-6; 1.86e-4; 2.041 (ADR 0363 H3) | 1.2; 1800; 108; 2.0 | solution verification |
| D1 | The retired records, each run once with `--ignored` on the code commit: ADR 0369 L1, ADR 0363 H2, ADR 0363 H3a, ADR 0369 H | each fails at the assertion predicted in D3 | D3 | as D3 | 1 | code verification (negative control) |

No row is of kind validation: the acceptance of F-471 claims no agreement with a measured case. N6 prints the differences
of max `C_D`, max `C_L` and `St` from the FeatFlow values of `docs/reference-data.md` and evaluates clause A2-T-2 of ADR
0268 as ADR 0363 H3 did; that evaluation belongs to F-354 and has no limit here.

### Negative controls and planned mutations

N3a, P2a, N4a and D1 are the negative controls. Mutations, each alone on a scratch copy of the code commit (its own target
directory), the named test run once in release, the file restored and compared byte for byte:

| Mutation | Must fail |
|---|---|
| M1 (committed, rule 16): `Default` of `AmgSettings` back to `coarse_lu_interchanges_first: false`. Patch `crates/rcfd-linalg/tests/amg_coarse_lu_default_adr0743.mutation.patch` | N1 and N3 (the default clauses of A2) |
| M2: `DenseLu::solve` takes the interchanges-first branch always (the flag ignored) | N2, N3a |
| M3: `LinearOptions::amg_settings` returns the default settings always (the way back cut in `rcfd-linalg`) | O2 (with the files of ADR 0363) |
| M4: `FvMatrix::solve_amg` passes the old order | P2 |
| M5: `pressure.rs` line 484 calls `pm.solve_amg` (the way back cut in the plain pressure path) | O2 (with the files of ADR 0363) |

The heavy record N6 and the listed existing files are not run under mutations.

## Size

Size (agent): done; 0; 1; 0; 235000

The one heavy run is N6. Four listed existing files are long as well (25 to 55 minutes each in the gate log read above):
`immersed_solid_f219`, `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`, `vof_waves`.

## Decision rule

- **done**: A1 to A5, N2 to N6, P1, P2, O1 to O3 and D1 are met; no registered hash changed; every mutation is caught by
  the test it must fail without an amendment after a run; `fmt` and `clippy` are clean.
- **partial**: a row is missed or not run (recorded by rule 2); or a listed existing test fails that D3 does not list (it
  is then a changed result this registration did not list: it is reported with the file, the test and both values, and it
  is not edited here); or a mutation is not caught.
- **A changed registered hash stops the work**: nothing is re-recorded, the case and both values are reported, and the
  item stays partial.
- **todo** is not an outcome of this registration: the default is one line.

## What is not covered

- Test files that reach a scalar AMG without naming it: the coupled solver (its `amg` is on by default), the overset
  pressure, `potential.rs` and `varden.rs` above 2000 cells, the periodic and coupled-face pressure path of `pressure.rs`,
  and the `rcfd-api` case runs above 2000 cells. 41 files of `rcfd-fv/tests` and 36 of `rcfd-api/tests` name the coupled
  solver alone. They are the lead's full gate. A result of theirs moves only where a coarsest factorisation interchanges at
  `k >= 1`.
- The old order through the coupled, overset, potential and variable-density solvers (D2).
- ADR 0363 H1 and H3 are not run under the new default (D3). The T run of N6 starts from `t = 0`, not from ADR 0363's
  state of step 2950, so its extrema are a new record and not a repetition of H3's.
- Whether other meshes or time steps had a silent wrong coarse solve under the old default, beyond what the listed files
  show.
- The `rz <= 0` guard in `cg` (ADR 0363 Q6) and making `pressure_ic_fallback` the default (ADR 0363 Q5) are not touched.

## Open questions

None is taken and none is open at registration. The default itself is the maintainer's decision of 2026-11-18.

## Addendum 1: results (2026-11-19)

Built as registered, on the code commit 828c9375 (after the registration d1129c93). All runs in the release profile, each
registered run made once, on Windows, `RAYON_NUM_THREADS` 4 for the flow runs, `F466_DIR` = `w49i-work/f466-scratch/runs`
(read only; its 13 files are unchanged). Logs are in `w49i-work/f471-coarse-lu-default-scratch/logs`. No limit was changed
and no amendment was written.

**The work stopped at a changed registered fingerprint** (decision rule, third point): the test
`rcfd-fv` `immersed_single_corrector_f363::n0_fingerprints_of_the_defaults` (ADR 0374 N0) fails at its fingerprint FP4.
It is not edited and nothing is re-recorded. What was already running was let finish; the four long listed files were not
started. Details under "The changed fingerprint".

### Results

| # | Check | Limit | Measured | Met |
|---|---|---|---|---|
| A1 | interchanges on the baseline set (instrumented scratch copy of 828c9375, `baseline_hashes`, one thread) | 0 factorisations with an interchange at `k >= 1` | 12 case runs; 119 factorisations, in two of them: the coupled cavity (59, coarsest 71 unknowns) and `kepsilon_channel` (60, coarsest 56); no interchange at any step in any of them; the other runs build no AMG; the instrumented test passes | yes |
| A2 | the default on `A3`; on the 20 x 20 tridiagonal | `(1, 1, 1)` bit for bit; `<= 1e-12` | `(1, 1, 1)`; 8.863e-19 (interchanges at steps 0, 2, ..., 18) | yes |
| A3 | every listed file passes unedited | 0 failures | **1 failure in 42 files run; 4 files not run.** Passed: `rcfd-linalg` `--lib` 20, `amg_coarse_lu_f474` 5 (2 ignored), `amg_contrast` 2; `rcfd-fv` `--lib` 115 and 29 files (list below); `rcfd-api` `--lib` 159 and 4 files. Failed: `immersed_single_corrector_f363` (3 passed, 1 failed: FP4). Not run: `immersed_solid_f219`, `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`, `vof_waves` | **no** |
| A4 | `baseline_hashes` | every hash equal | 3 passed, 1 ignored (as on main) | yes |
| A5 | `case_hashes_w48u`, `w48v_case_hashes`, `varden_ideal_hash` | every hash equal | 1 passed; 0 run (its one test is ignored on main, it only prints); 1 passed | yes |
| N2 | the old order on `A3` | `[1]`; `(-13, 15, -5)` bit for bit | `[1]`; `(-13, 15, -5)` | yes |
| N3a | the old order on the tridiagonal | `> 1e-12` | 4.071e-3 | yes |
| N3b | old order against default on the 40 x 40 Laplacian | subset of `{0}`; 0 differing | `[0]`; 0 | yes |
| P1 | M2, bounded, 3 steps, old order against default | 0 differing; no interchange at `k >= 1` | 0; no interchange at all in the 6 systems (coarsest 89 to 93 unknowns) | yes |
| P2 | `FvMatrix::solve_amg` on the tridiagonal | `<= 1e-10`; 0 differing | converged in 1 iteration, 8.863e-19; `solve_amg_with` (default) 0 differing, 1 iteration | yes |
| P2a | `solve_amg_with` (old order) differs | at least 1 of 20 | 20 of 20 (50 iterations, not converged, relative residual 2.8e-2) | yes |
| O1 | from the state of step 2900, old order | stop at 2951; R1's message; 0 Courant numbers differing | stop at 2951; the message equal; 0 differing (20 s) | yes |
| O2 | from the state of step 2950, old order, IC repeat | no stop; `[(2951, 0, 1)]`; finite | no stop; `[(2951, 0, 1)]`; finite | yes |
| O3 | the stopping system, old order, then the default | stop 2951; 3000 iterations, non-finite; `[13, 16, 17]`; `-2.006e-9`; default converged, `rz > 0`, `<= 100` | stop 2951; 3000 iterations, 53 136 non-finite cells; levels 53 136, 10 745, 1653, 206, 24; `[13, 16, 17]`; `-2.006e-9` (start 5.520616e-9); default: converged in 10 iterations, every `rz > 0` | yes |
| N4 | from the state of step 2900, default, to step 2960 | no stop; 0 non-finite | no stop; 0; at most 20 pressure iterations in a step | yes |
| N4a | Courant numbers of 2901 to 2950 differing from H1 | at least 1 | 32, the first at step 2917 (estimate 34) | yes |
| N5 | from the state of step 2950, default, IC repeat | no stop; no repeat; finite | no stop; `[]`; finite | yes |
| N6 | T, bounded, from `t = 0` to step 8000, default | no stop; crossings `>= 6`; period spread `<= 1e-2`; maximum spread `<= 2e-2`; amplitude `>= 1.0` | no stop; 7; 5.170e-6; 1.861e-4; 2.0408 (1620 s) | yes |
| D1 | the retired records, run once with `--ignored` | each fails as predicted in D3 | ADR 0369 L1: fails at its last assertion, `z = [1.0, 1.0, 1.0]`; ADR 0363 H2: "M2: H2 did not stop" (42 Courant numbers differ by its own count, which includes steps 2951 to 2960); ADR 0363 H3a: "H3a: repeats", left `[]`; ADR 0369 H: "H0: stop step", left `None` | yes |

A3, the files that passed (passed / ignored): `rcfd-fv` `consistent_correctors_f301` 12/4, `consistent_defaults_f301` 6/0,
`consistent_extensions_f322` 8/0, `consistent_periodic_defaults_f367` 1/0, `consistent_periodic_f367` 13/2, `cylinder`
2/1, `immersed_bounded_stop_f466` 7/4, `immersed_dfg_convergence_f354` 6/12, `immersed_early_state_start_f501` 3/2,
`immersed_lag_estimate_f483` 5/7, `immersed_lockin_f731` 6/1, `immersed_lockin_f750` 8/3, `immersed_lockin_f756` 8/2,
`immersed_moving_f308` 7/2, `immersed_multistep_linearisation_f494` 4/6, `immersed_start_projection_f515` 4/2,
`immersed_step_linearisation_f482` 3/6, `immersed_transverse_f707` 4/3, `transient_buoyancy` 3/0,
`vof_bubble_diagnosis_w41c` 1/20, `vof_bubble_refinement_w40d` 1/12, `vof_dambreak_amg` 1/0, `vof_height_function_w42b`
2/2, `amg_definiteness_f474` 1/1, `coupled` 2/1, `gradient_pressure_f291` 3/0, `varden_ideal_hash` 1/0,
`sixdof_flow_f244` 8/1, `piso_thin_gap_f299` 18/2; `rcfd-api` `consistent_correctors_case_f301` 4/8, `w35b_lag_check` 6/0,
`w48t_cylinder_spokes` 3/13, `vof_wiring_tree` 4/0. The new files: `amg_coarse_lu_default_adr0743` 4/0,
`amg_default_fluid_adr0743` 2/6 (the six run one by one as rows O1 to N6).

**The record N6** (the default T run, the changed result of ADR 0268 R1 and ADR 0363 H1 and H3): no stop to `t = 8`. At
`t = 1` and `t = 2` the printed coefficients are those of ADR 0268 R1 to their five digits (2.98321, -0.35990; 3.13571,
-0.74178). Max `C_D` 3.22695 (+0.216 percent), max `C_L` 1.00253 (+1.687 percent), `St` 0.30144 (-0.146 percent); relative
to ADR 0363 H3 (the old default with one IC repeat): +2.9e-7, -2.1e-6, -8.5e-7. Clause A2-T-2 of ADR 0268 evaluates as in
ADR 0363 (drag 0.216 to 0.136, lift 1.687 to 0.174; both true). The largest pressure iteration count of a step is 20
(two correctors), at step 939; the old default needed 406 in corrector 0 of step 2951 and then broke down.

### The changed fingerprint (the reason for the stop)

- **Test**: `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs`, `n0_fingerprints_of_the_defaults` (ADR 0374 N0: "the
  fingerprints of the defaults", measured on the tree before any code of F-363), assertion "FP4" at line 269.
- **Case**: FP4 is the FNV hash of `u` and `p` after 20 steps of `dt = 1e-3` of the set-up R of ADR 0254 on M2 (DFG 2D-2,
  18 304 cells, BDF2, linear upwind, `amg_pressure`), forcing `Sa`, one PISO corrector, `flux_weight_check` on.
- **Values**: registered `0x279c4c4ed4cf89be`; with the new default `0xda894fae1d52a3ad`.
- FP1 (`0xdd07702a17b9a36f`), FP2 (`0xe4d227de316e6de4`) and both FP3 values of the same test are unchanged.
- **Cause, measured on the scratch copy** (not a registered check; made to report the case; logs `fp4-default.log`,
  `fp4-old.log`): in the `Sa` run exactly one coarsest factorisation interchanges rows at a step `k >= 1`: the pressure
  solve of step 3, coarsest 90 unknowns, step 35. No factorisation of the F and B runs does. With the old order asked for
  through `linear.amg_coarse_lu_interleaved` the test passes with FP4 `0x279c4c4ed4cf89be`. So the registered FP4 holds one
  pressure solve with a wrong coarse correction, and the change is the default switch and nothing else.

By the decision rule this is the changed registered hash that stops the work. It is also the case R3 named "not known":
a listed file whose result moves.

### Mutations (each alone on a scratch copy of 828c9375, the named test once, the file restored byte for byte every time)

| Mutation | Registered to fail | Result |
|---|---|---|
| M1 (committed as `crates/rcfd-linalg/tests/amg_coarse_lu_default_adr0743.mutation.patch`) | N1 and N3 | caught: N1 fails at the default flag (line 51), N3 at the default residual (line 118) |
| M2 (`DenseLu::solve` always interchanges first) | N2, N3a | caught: N2 (line 70), N3a (line 119) |
| M3 (`LinearOptions::amg_settings` ignores the flag) | O2 | caught: "O2: repeats", repeats `[]` |
| M4 (`FvMatrix::solve_amg` passes the old order) | P2 | caught: "P2: relative residual 2.776e-2" |
| M5 (`pressure.rs` line 484 calls `pm.solve_amg`) | O2 | caught: "O2: repeats", repeats `[]` |

### Checks of rule 7

`cargo fmt -p rcfd-linalg --check`, `-p rcfd-fv --check`, `-p rcfd-api --check`: clean. `cargo clippy -p rcfd-linalg -p
rcfd-fv -p rcfd-api --all-targets -- -D warnings`: clean. `cargo test -p rcfd-cli --release --test docs_site`: 2 passed.
`python scripts/process_report.py --check`: one failure, `docs/adr/index.md` not current (the lead's file).

Shared solver files changed (line numbers of 828c9375):

- `crates/rcfd-linalg/src/amg.rs`: 30 to 34 (the comment of the field), 47 (`coarse_lu_interchanges_first: true`), 125 and
  142 (comments in `DenseLu::solve`). No statement of `DenseLu::new` or `DenseLu::solve` changed.
- `crates/rcfd-linalg/src/backend.rs`: 67 to 71 (`LinearOptions::amg_coarse_lu_interleaved`), 75 to 83
  (`LinearOptions::amg_settings`).
- `crates/rcfd-fv/src/fvmatrix.rs`: 76 to 82 (`solve_amg` calls the new `solve_amg_with`).
- `crates/rcfd-fv/src/pressure.rs`: 18 (the import), 484, 500, 620 (`ctl.linear.amg_settings()`).
- `crates/rcfd-api/src/linsolver.rs` 47 and `solid_parallel.rs` 773 (`..LinearOptions::default()`).

Test files: the two new files and the mutation patch; one `#[ignore]` attribute added in `amg_coarse_lu_f474.rs` and the
text of four existing `#[ignore]` attributes changed in `immersed_bounded_stop_f466.rs` (three) and
`amg_definiteness_f474.rs` (one). `immersed_single_corrector_f363.rs` is untouched.

### Deviations and disclosures

- The registration commit was amended once, minutes after it was made and before any code, build or run: the qualifiers in
  the kind column were written in square brackets, which `process_report.py` does not read as a qualifier; they are now in
  round brackets. No other character changed. The first commit of the branch is the amended one, d1129c93.
- The prediction of D3 for ADR 0363 H2 named the message, not the line: the test fails at its line 959 (the branch for "no
  captured solve returned a non-finite pressure"), with the predicted message.
- The diagnosis of FP4 on the scratch copy (an `eprintln` in `DenseLu::new`, the old order through an environment variable
  in a copy of the test) was not registered. Nothing of it is committed.
- N4a measured 32 where 34 was estimated (two of the steps from 2917 on have the same Courant bits as H1).
- ADR 0369 L4b and F1 pass and are now vacuous, as registered (D3).

### Not run, not done

- **The four long listed files** (`immersed_solid_f219`, `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`,
  `vof_waves`; 25 to 55 minutes each): not started, because the work stopped at FP4. Whether they hold another moved
  result is not known.
- The test files that reach a scalar AMG without naming it (registered as not covered).
- ADR 0363 H1 and H3 under the new default (registered as not run).
- **The branch as it stands fails one gate test** (`n0_fingerprints_of_the_defaults`), so it cannot be merged before
  question 1 is answered.

### Size

Size (agent): partial; 1; 1; 1; about 250000

### Decision

By the registered rule: **partial**, and the work is stopped. Every row but A3 is met, no hash of `baseline_hashes` or of
the case hash tables moved, and all five mutations are caught. A3 is missed: one listed existing test fails that D3 does
not list, its failing value is a registered fingerprint of a default result, and four listed files were not run.

## Open questions for the maintainer

1. **The fingerprint FP4 of ADR 0374 N0** (`immersed_single_corrector_f363::n0_fingerprints_of_the_defaults`) moves with
   the approved default: registered `0x279c4c4ed4cf89be`, new `0xda894fae1d52a3ad`. The approval of 2026-11-18 does not
   cover a hash. Options: (a) approve this move: a further registration under F-471 retires that test as an ignored record
   (its `#[ignore]` attribute only) and restates it with FP1 to FP3 unchanged, FP4 `0x279c4c4ed4cf89be` under the old
   order and `0xda894fae1d52a3ad` under the default, and runs the four long files and, if the lead wants it, the wider set
   of "What is not covered" before the merge; (b) withdraw the default switch (the constant back to `false`, the rest of
   this branch kept as an opt-in route); (c) approve in advance every fingerprint of a default result that is shown to
   reproduce bit for bit under the old order and to come from a coarsest factorisation with an interchange at `k >= 1`,
   so that a further one found by the gate does not need a new question. Recommendation: **(a) with (c)**. The registered
   FP4 pins a run in which one pressure solve had a wrong coarse correction; the old order reproduces it exactly, and the
   same evidence can be produced for any further fingerprint.

## For the lead: roadmap entries

Status proposed for F-471: partial.

> ADR 0743, 2026-11-19, partial: the corrected order is the default of AmgSettings on the branch, with a way back (coarse_lu_interchanges_first false; LinearOptions::amg_coarse_lu_interleaved for the pressure solve) that reproduces the old stop of the T run bit for bit; baseline_hashes and the case hash tests pass and no baseline case interchanges; the default T run reaches t = 8 without a stop (max C_D 3.22695, max C_L 1.00253, St 0.30144, at most 20 pressure iterations a step); five of five mutations caught; stopped because the fingerprint FP4 of ADR 0374 N0 moves (0x279c4c4ed4cf89be to 0xda894fae1d52a3ad, one coarsest interchange at step 35 in step 3 of the Sa run on M2), which the approval does not cover; four long listed files not run.

> Gap: the branch of ADR 0743 is not mergeable: immersed_single_corrector_f363 n0_fingerprints_of_the_defaults fails at FP4 (registered 0x279c4c4ed4cf89be, new 0xda894fae1d52a3ad). Needs the maintainer's answer to ADR 0743 question 1; then a further registration retires and restates that test, runs immersed_solid_f219, immersed_unsteady_force_f346, immersed_piso_stop_f355 and vof_waves, and the full gate shows whether any other default result moves.

No new item is proposed: what remains is a further registration under F-471 (rule 13). Notes proposed for F-474 and
F-466, each one line:

> ADR 0743, 2026-11-19 (F-471): with the corrected order as the default on its branch the stopping system of step 2951 converges in 10 iterations with every r^T z > 0, the T run does not stop, and the old breakdown is reproduced under the old order (stop at 2951, r^T z -2.006e-9, interchanges at 13, 16, 17).

> ADR 0743, 2026-11-19 (F-471): the default T run with the bounded forcing reaches t = 8 without the IC repeat on the branch of ADR 0743; its extrema equal those of ADR 0363 H3 to 2e-6 relative (max C_D 3.22695, max C_L 1.00253, St 0.30144), so A2-T-2 of ADR 0268 stays met.

## Answer of the maintainer (2026-11-19), written into this ADR by the lead's brief

Asked by the lead on 2026-11-19 (question 1 of ADR 0743, addendum 1), as three options with a recommendation. The maintainer
answered on 2026-11-19: "Approve the move". That is option (a): the move of FP4 of ADR 0374 N0 under the corrected order of
the coarsest dense LU is approved, under a further registration that restates the test and runs the four long files before
anything merges. Option (c), an approval in advance of further fingerprints, was not given: any other registered hash or
fingerprint that moves stops the work again and is reported.

The further registration is ADR 0747 (brief `docs/adr/briefs/f471-fp4-move.md`).
