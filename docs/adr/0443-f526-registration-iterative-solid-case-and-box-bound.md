# ADR 0443: F-526 registration: the iterative solve of a solid case file to its tolerance, and the box bound

Status: **REGISTERED 2026-11-14, written before any code of F-526 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-526 was written, compiled or run: the documents under "What was read" were read, and
the arithmetic under "Arithmetic before this registration" was done by hand from closed forms and from numbers recorded in ADR
0282 and ADR 0438 (no code run). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0438](0438-f386-registration-solid-case-file.md) (F-386: the solid case file; its check A14, a known miss: all six
preconditioners stopped at a true residual of 5.7e-10 to 6.2e-10 of `|b|`, diagnosed as a rounding floor; its mutation MU4, the
exclusive box upper bound, not caught), [ADR 0282](0282-preconditioned-cg-for-elasticity.md) (F-381: the Hex8 cantilever, the
residual floor, amendment 4). Roadmap item F-526. Size S, test-mostly. Branch `f526-solid-case-iterative` from main 79012f86,
worktree `w49i-work/f526`, scratch `w49i-work/f526-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical: the design below changes **no
file under `crates/*/src`**, so every file run, flow case, schema, CLI text, MCP tool list and hash table gives what it gave; no
edit of `baseline_hashes.rs`, of the hash tables or of any golden, no existing test edited; no dependency, no unsafe code;
clean-room (closed forms and the repository's own numbers only).

## What F-526 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A solid case file of the F-381 Hex8 cantilever (or of a
> model whose load is not small against its internal nodal forces, the ratio stated in the registration) run with method = cg and
> each preconditioner key reaches the relative residual 1e-10 and agrees with the direct file run within 1e-7; the residual floor
> eps ||K|| ||u|| / ||b|| of that model and of the ADR 0438 Tet10 cantilever is computed and printed; a node-set box whose upper
> bound equals a node coordinate exactly selects that node.

Clauses: (c1) the model is the F-381 Hex8 cantilever as a solid case file; (c2) its ratio of load to internal nodal forces is
stated in the registration; (c3) a file run with `method = "cg"` and each of the six preconditioner keys of ADR 0438 D5
(`block_jacobi`, `ic0`, `ic_fill`, `amg`, `amg_quadratic`, `amg_quadratic_smoothed`) reaches the relative residual 1e-10; (c4)
and agrees with the direct file run within 1e-7; (c5) the floor `eps ||K|| ||u|| / ||b||` of that model is computed and printed;
(c6) the same for the ADR 0438 Tet10 cantilever; (c7) a node-set box whose upper bound equals a node coordinate exactly selects
that node; (c8) (from the brief) the box test changes the selection of no existing case file; (c9) (from ADR 0438) MU4 is
registered and run.

## What was read (main 79012f86, read only)

- ADR 0438 (all of it: D3 the inclusive box, D5 the solver keys, A13, A14, K1, MU4, amendments 1 and 2, its addendum and its
  follow-up text), ADR 0282 (arithmetic 5 the fast cantilever, arithmetic 7 the floor, amendment 4: the componentwise floor, the
  24 m defect of H1, the measured numbers on the fast case).
- Code: `crates/rcfd-api/src/solid_case.rs` (the node-set selection in `SolidCase::build`: `p[c] >= lo[c] && p[c] <= hi[c]`, that
  is already inclusive on both bounds; the solver keys `tolerance`, `max_iterations` at lines 570 to 580), the test file
  `crates/rcfd-api/tests/sb_solid_case.rs` (helpers `run_file`, `static_of`, `compare`, `swap`, `cg_text`, `a14_rows`) and the case
  files of `tests/solid_cases/`, `crates/rcfd-fem/src/assemble.rs` (`stiffness(model)`, the full unreduced matrix with the
  constrained rows and columns, `CsrMatrix { row_ptr, col, val }`), `crates/rcfd-fem/src/loads.rs` (`nodal_load_vector`),
  `docs/solid-case-file.md` (line 19: "bounds inclusive": the document already states the rule).
- Consequence: the library already selects a node on a bound that equals its coordinate. F-526 is test-only: it adds case files
  and tests, and (c7) is a check that the stated behaviour holds and that the mutation that breaks it is caught. If a check shows
  the library does not do what the document says, that is a miss recorded as such, not a library change made under this item.

## Arithmetic before this registration (disclosed)

1. **The F-381 model** (ADR 0282 arithmetic 5): Hex8 box `[0, 8] x [0, 1.5] x [0, 1.5]` m, divisions `[32, 6, 6]` (cubes of
   `a = 0.25` m), 33 x 7 x 7 = 1617 nodes, 4851 equations, clamp of `xmin` (49 nodes, all three components), 4704 unknowns; tip
   traction `(0, 0, -1e6)` Pa on `xmax`, `F = 2.25e6` N; `E = 2e11` Pa, `nu = 0.3`; Euler-Bernoulli tip deflection
   `4 F L^3 / (E h^4) = 4.6e-3` m. All box coordinates are exact binary fractions (`0.25 i`, `i` an integer), so a box bound can
   equal a node coordinate exactly.
2. **The norm choice of the floor.** `eps = 2.220446e-16` (`f64::EPSILON`). `||K||` is the **infinity norm** (the largest row sum
   of absolute values) of the full unreduced stiffness matrix `rcfd_fem::assemble::stiffness(model)` (an upper bound of that of
   the reduced matrix, the same order); `||u||` and `||b||` are the 2-norms of the full displacement vector and of
   `loads::nodal_load_vector(model, &[])` (the load alone, constrained components included, which carry no load). The floor is
   `phi = eps ||K||_inf ||u||_2 / ||b||_2` and the **ratio** of the load to the internal nodal forces is
   `R = ||b||_2 / (||K||_inf ||u||_2)`, so `phi = eps / R`. (`||K|| ||u||` bounds the size of the internal nodal forces `K u`
   that the load `b` has to balance to the working precision; `R` small means a residual `b - K u` formed by cancellation of
   large terms.)
3. **Estimate for the F-381 Hex8 model**, from ADR 0282 arithmetic 7 (the same formula, `||K|| ~ 2 E a = 1.0e11` N/m,
   `||u||_2 ~ delta sqrt(N/3) = 4.6e-3 x 23.2 = 0.107` m, `||b||_2 = F / sqrt(m) = 2.25e6 / 7 = 3.2e5` N, m = 49 tip nodes):
   `||K|| ||u|| / ||b|| = 3.3e4`, so `R ~ 3e-5` and `phi ~ 7e-12`. ADR 0282 amendment 4 found the componentwise bound four times
   the `||A|| ||x||` estimate (1.28e-11 against 3.6e-12), so the factor of uncertainty of this estimate is 4 upward: `phi` between
   7e-12 and 3e-11, a margin of 3 to 14 below the tolerance 1e-10. Direct-solve residual measured on this model by ADR 0282:
   6.5e-12.
4. **Estimate for the ADR 0438 Tet10 model** (`L = 2`, `h = 0.1`, Tet10 `40 x 2 x 2`, 6047 unknowns, `|b| = 263.5` N from ADR
   0438 amendment 1, tip deflection 1.6e-3 m): node spacing 0.025 m; `||K|| ~ 20 E x 0.025 = 1.0e11` (ADR 0282: Tet10 about ten
   times the `2 E a` of the same spacing; order only); `N ~ 2025` nodes; `||u||_2 ~ 1.6e-3 sqrt(2025/3) = 0.042` m. Then
   `R ~ 263.5 / (1.0e11 x 0.042) = 6e-8` and `phi ~ 2.2e-16 / 6e-8 = 3.5e-9`. The measured stall of ADR 0438 (5.7e-10 to
   6.2e-10) is a factor 6 below this estimate, consistent with ADR 0282's calibration constant `c ~ 0.2 to 0.5` between the
   floor formula and the residual attained. The estimate is of order only (a factor 3 either way).
5. **The agreement with the direct run** (ADR 0282 arithmetic 9): a residual `<= 1e-10 ||b||` bounds the relative error of `u` by
   about `1e-10 ||b|| / (lambda_min ||u||)`, of order 2e-10 in the 2-norm and at most `sqrt(N)` worse in the maximum norm, below
   1e-8; the direct solution's own error is of the same order as its residual floor. Limit 1e-7 (the roadmap's): margin 10 or
   more. The measure is the largest `|u_cg - u_direct| / max |u_direct|` over all displacement values, the `compare` of the
   test file.
6. **The box bound.** The node `(8, 0, 0.75)` has `x = 32 x 0.25 = 8`, `z = 3 x 0.25 = 0.75`, both exact in binary floating
   point, whichever way `hex_box` forms them (`origin + i d` with `d = 8 / 32 = 0.25` exact; or `origin + L i / n`, exact too).
   A box with `box_max = [8, 0, 0.75]` equals that node's coordinates on three components at once.

## Design

D1. **A case file of the F-381 model**, `crates/rcfd-api/tests/solid_cases/cantilever_f381_hex8.toml`: the keys of
`cantilever_hex8.toml` (ADR 0438) with `lengths = [8.0, 1.5, 1.5]`, `divisions = [32, 6, 6]`, one constraint `[constraints.clamp]`
`set = "xmin"`, `components = ["x", "y", "z"]`, one load `[loads.tip]` `kind = "traction"`, `set = "xmax"`,
`traction = [0.0, 0.0, -1.0e6]`, `method = "direct"`, material `youngs_modulus = 2.0e11`, `poisson_ratio = 0.3`. The CG runs are
this text with `method = "direct"` replaced by `method = "cg"`, `preconditioner = "<key>"`, `tolerance = 1e-10` (the unchanged
defaults of ADR 0438 D1: `fill_level = 1`, `ordering = "natural"`, `max_iterations = 20000`).
D2. **The floor** is computed in the test from the model of the run (`SolidRun::setup.model`) and the direct result, by the
definition of arithmetic 2, printed with `||K||_inf`, `||u||_2`, `||b||_2`, `R` and `phi`, for the F-381 model and for
`cantilever_tet10.toml`.
D3. **The box test** uses a second file `box_exact_bounds.toml` on the same Hex8 mesh (so that the node coordinates are known
exactly) with node sets whose bounds equal coordinates, and the node sets of every existing case file are compared with an
independent inclusive filter written in the test from the parsed box values.
D4. No library, schema, CLI, MCP, golden or hash file is changed. Tests are in a new file
`crates/rcfd-api/tests/sb_solid_case_iterative.rs`.

## Registered checks

All in `sb_solid_case_iterative.rs`, release profile, run once (`cargo test -p rcfd-api --release --test
sb_solid_case_iterative -- --nocapture --test-threads=1`), after the registration is committed. Outputs go to
`CARGO_TARGET_TMPDIR`.

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| P1 | c1 | the file model: 1617 nodes, 4704 unknowns, 49 nodes in `xmin`; the z load on `xmax` summed from `nodal_load_vector` is `-2.25e6` N within 1e-9 relative; the direct run's `relative_residual` from the report is recorded | counts as stated; load `<= 1e-9`; residual recorded | arithmetic 1; direct residual 6.5e-12 |
| P2 | c2, c5 | `phi` of the F-381 Hex8 model (arithmetic 2) | `<= 5e-11` | 7e-12 to 3e-11 |
| P3 | c2 | `R` of the F-381 Hex8 model | `>= 4.4e-6` (that is `phi <= 5e-11`, stated as a ratio) | 3e-5 |
| P4 | c3 | for each of the six keys: the file run with `method = "cg"`, tolerance 1e-10 returns a result (no "not converged" diagnostic) and its report's `solver.relative_residual` | `<= 1e-10` each | floor 7e-12 to 3e-11 |
| P5 | c4 | for each of the six keys: largest `\|u_cg - u_direct\| / max \|u_direct\|` against the direct file run | `<= 1e-7` each | arithmetic 5: below 1e-8 |
| P6 | c6 | `phi` and `R` of `cantilever_tet10.toml`, printed; the registered fact is that this model's floor is above the tolerance and within a factor 10 of the stall measured by ADR 0438 (6e-10): `6e-11 <= phi <= 6e-9` | in that interval | 3.5e-9 |
| P7 | c2 | the contrast: `R_hex8 / R_tet10` | `>= 100` | 500 |
| Q1 | c7 | `box_exact_bounds.toml`, set `corner_hi`: `box_min = [7.8, -0.1, 0.7]`, `box_max = [8.0, 0.0, 0.75]` (all three upper bounds equal the coordinates of the node `(8, 0, 0.75)`): the set holds exactly that node (coordinates compared bitwise) | exactly 1 node | arithmetic 6 |
| Q2 | c7 | set `face_hi`: `box_min = [7.8, -1.0, -1.0]`, `box_max = [8.0, 2.0, 2.0]` (only the x upper bound equals a coordinate): equals the node set `xmax` of the mesh (49 nodes, the same indices) | equal | arithmetic 6 |
| Q3 | c7 | set `corner_lo`: `box_min = [8.0, 0.0, 0.75]`, `box_max = [8.2, 0.1, 0.8]` (lower bounds equal the coordinates): exactly that node | exactly 1 node | inclusive both ways |
| Q4 | c7 | set `point`: `box_min = box_max = [8.0, 0.0, 0.75]` (a degenerate box): exactly that node | exactly 1 node | |
| Q5 | c8 | for each of the 7 existing case files of `tests/solid_cases/` and the new ones: every `[node_sets.*]` of the file, selected by `SolidCase::build`, equals the selection of an independent inclusive componentwise filter written in the test from the box values read from the file by `toml`, over the mesh of the built model; each set non-empty | equal (indices) | no library change |
| S1 | c8 | `git diff 79012f86 --stat -- crates` lists only the new test file and the new case files (no `src` file, no existing test) | as stated | D4 |
| S2 | c8 | existing tests unedited and passing: `rcfd-api` `sb_solid_case` (the whole file: 8 pass, 4 ignored as before), `case_hashes_w48u`, `rules_w19c`, `physics_schema` | all pass | no `src` change |

(`baseline_hashes` and the `rcfd-cli` and `rcfd-mcp` tests need to run only if a `src` file changes; none does, S1. If any does
by the time of the run, they are run.)

### Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | P5's comparison applied to the `ic_fill` run against a direct run of a model with `poisson_ratio = 0.3000000001` | above 0, below 1e-6 (the comparison can fail) |
| N2 | a CG run of the **Tet10** file with `tolerance = 1e-10` (`amg_quadratic`) | an error containing `not converged` (the fact of ADR 0438 amendment 2; it shows P4's pass is the model's, not a loosened solver) |
| N3 | the set `face_hi` with `box_max[0] = 7.999999999` (1e-9 below the coordinate) | selects none of the 49 nodes of `xmax` (so Q2's equality can fail) |

### Planned mutations (scratch copy `w49i-work/f526-scratch/tree` from `git archive` of the final commit, not committed)

| Id | Mutation (in `crates/rcfd-api/src/solid_case.rs`) | Expected to be caught by |
| --- | --- | --- |
| MU4 | the upper bound exclusive: `p[c] < hi[c]` | Q1 (empty selection: a diagnostic), Q2, Q4; Q5 on the ADR 0438 files must still pass (their bounds extend 1e-9 beyond the point) |
| MU4b | the lower bound exclusive: `p[c] > lo[c]` | Q3, Q4 |
| MU11 | the solver `tolerance` key ignored (fixed 1e-6) | P4 (residual above 1e-10) |

## Decision rule

- **done**: every P, Q, S check met on the single run, N1 to N3 as expected, every mutation caught by at least one check, no
  amendment made after a run.
- **partial**: any check missed (kept as a `*_known_miss` test with the limit intact and the measured number, the cause
  diagnosed), a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- One model for the iterative claim (F-381's Hex8 cantilever); the Tet10 model is covered only by the printed floor and N2.
  Quadratic elements at a tolerance of 1e-10 are not shown to converge on any model here.
- The floor is an estimate of the attainable residual by one norm choice (arithmetic 2), not a bound; the measured residual is
  reported beside it, and compared by a limit only in P2, P4, P6.
- The box test covers the existing inclusive rule; no tolerance key, no surface or element boxes, no CLI or MCP entry (golden
  edits: the maintainer's step, ADR 0287 M3).
- Wall time of the six CG runs is recorded, not limited; block Jacobi may need some thousand iterations (ADR 0282 arithmetic 8).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The model is the F-381 Hex8 cantilever (the roadmap's first choice), not a new one.
2. The floor uses the infinity norm of the full unreduced `K` (arithmetic 2); the 2-norm is not computed.

## Amendment 1 (2026-11-14, after the registered run, before the corrected run): N3's set-up asked the library for a refused model

The registered run (`f526-scratch/run1.log`, run once) ran all three tests. `p1_to_p7_n1_n2_iterative_f381_and_floors` and
`q5_s_existing_case_files_selection_equals_an_independent_inclusive_filter` passed; every P check, N1, N2 and Q5 was met (numbers
in the addendum). `q1_to_q4_n3_box_bounds_equal_to_coordinates` printed Q1, Q2, Q3 and Q4 as met, then panicked in the N3 part,
at the `unwrap` of `SolidCase::build` on the file with `box_max[0] = 7.999999999`: the library refused the model with the
diagnostic `node_sets.face_hi.box_min`, line 18, "the box of node set 'face_hi' selects no node". That is the library's
registered behaviour (ADR 0438 D2, N4): the box `[7.8, 7.999999999]` in x holds no node (the nearest planes are `x = 7.75` and
`x = 8`), so there is no node set to compare with `xmax`. N3's measure ("selects none of the 49 nodes") was stated for a set
that exists; it is met, and by the stronger fact that the model is refused. The set-up was defective: it did not allow the
refusal.

Corrections, written and committed before their run:

- `q1_to_q4_n3_box_bounds_equal_to_coordinates` stays as an ignored record (`#[ignore]`, citing this amendment).
- `q1_to_q4_n3b_box_bounds_equal_to_coordinates` repeats Q1 to Q4 unchanged (the limits as registered) and replaces the N3 part
  by N3b: the file with `box_max[0] = 7.999999999` must be refused by `build` with a diagnostic whose path is
  `node_sets.face_hi.box_min` and whose message contains `selects no node`, and no `xmax` node is selected by any set of it
  (there is no set). The Q results of the registered run stand as measured (met); the corrected test repeats them only so that
  the passing record is one test.

By the registered rule the item is partial whatever the corrected run gives (a negative control met only after an amendment).

## Addendum 1: results (2026-11-14)

Runs (release, `--test-threads=1`, each made once): run 1, the registered test file (`f526-scratch/run1.log`: 2 of 3 tests
passed, the third failed in its N3 part, amendment 1); run 2, the corrected `q1_to_q4_n3b_...` (`run2.log`, passed). Existing
tests after the code (S2): `sb_solid_case` 8 passed, 4 ignored as before; `case_hashes_w48u` 1; `physics_schema` 11; `rules_w19c`
8 (1 ignored as before): all passed, none edited. No `src` file changed, so `baseline_hashes`, `rcfd-cli` and `rcfd-mcp` were not
run (the registered condition).

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| P1 file model: nodes, `xmin` nodes, values; z load sum against `-2.25e6` | 1617, 49, 4851; `<= 1e-9` | 1617, 49, 4851; 2.2e-16; direct run residual 6.5e-12 | yes |
| P2 floor `phi` of the F-381 Hex8 model | `<= 5e-11` | 2.17e-11 (`\|\|K\|\|_inf` 3.70e11, `\|\|u\|\|_2` 9.09e-2, `\|\|b\|\|_2` 3.44e5) | yes |
| P3 `R` of the F-381 Hex8 model | `>= 4.4e-6` | 1.02e-5 | yes |
| P4 block_jacobi relative residual (iterations) | `<= 1e-10` | 9.64e-11 (156) | yes |
| P4 ic0 | `<= 1e-10` | 9.54e-11 (85) | yes |
| P4 ic_fill | `<= 1e-10` | 5.39e-11 (58) | yes |
| P4 amg | `<= 1e-10` | 3.24e-11 (15) | yes |
| P4 amg_quadratic | `<= 1e-10` | 3.24e-11 (15) | yes |
| P4 amg_quadratic_smoothed | `<= 1e-10` | 2.28e-11 (12) | yes |
| P5 the same six against the direct run | `<= 1e-7` | 3.2e-13, 3.3e-13, 2.4e-13, 5.8e-13, 5.8e-13, 1.2e-12 | yes |
| P6 floor of the ADR 0438 Tet10 model (`\|\|K\|\|_inf` 1.78e11, `\|\|u\|\|_2` 3.53e-2, `\|\|b\|\|_2` 263.5, `R` 4.18e-8) | `6e-11 <= phi <= 6e-9` | 5.31e-9 (nine times the ADR 0438 stall of 6e-10; inside the interval, near its upper edge) | yes |
| P7 `R_hex8 / R_tet10` | `>= 100` | 245 | yes |
| Q1 `corner_hi` (all three upper bounds equal the coordinates) | exactly 1 node, bitwise `(8, 0, 0.75)` | 1 node `[8.0, 0.0, 0.75]` | yes |
| Q2 `face_hi` equals `xmax` | equal, 49 nodes | equal, 49 | yes |
| Q3 `corner_lo` | exactly that node | 1 node | yes |
| Q4 `point` (degenerate box) | exactly that node | 1 node | yes |
| Q5 node sets of the 9 case files (16 sets) against an independent inclusive filter | equal indices, non-empty | all 16 equal, each non-empty | yes |
| S1 `git diff 79012f86 --stat -- crates` | new test file and new case files only | 3 files added (the test and two `.toml`), no `src` file, no existing file | yes |
| S2 existing tests | pass | as listed above | yes |
| N1 comparison sees `nu + 1e-10` | above 0, below 1e-6 | 1.2e-11 | as expected |
| N2 Tet10 `amg_quadratic` at 1e-10 | error `not converged` | `not converged in 54 iterations and 3 restart(s) (true residual 1.5e-7, target 2.6e-8)` | as expected |
| N3 bound 1e-9 below the coordinate | selects none of the 49 nodes | the set-up panicked: the library refused the model (no node in the box); corrected N3b below | set-up defective |
| N3b (amendment 1) the same file | refused with path `node_sets.face_hi.box_min`, a line and `selects no node` | refused, line 18, message as stated | yes, after amendment |

Estimates against measurements: `||K||_inf` of the Hex8 model was 3.70e11 against the `2 E a = 1.0e11` of the estimate (a factor
3.7 higher: the row sum of absolute values of a Hex8 stiffness row is larger than `2 E a`), `||u||_2` 0.091 against 0.107,
`||b||_2` 3.44e5 against 3.2e5; the floor 2.2e-11 is inside the registered 7e-12 to 3e-11. The Tet10 floor 5.3e-9 is above the
estimate 3.5e-9 (`||K||_inf` 1.78e11 against 1.0e11). Each floor lies above the residual attained (Hex8: direct 6.5e-12 against
floor 2.2e-11, CG 2.3e-11 to 9.6e-11 which is 1 to 4.4 times the floor; Tet10: stall 6e-10 against floor 5.3e-9), so the CG
residuals on the Hex8 model sit at or just above the floor, and the floor orders the two models and sizes the effect without
predicting the stall to better than a factor 10.

### Mutations (scratch copy `f526-scratch/tree` from `git archive` of the committed tests, script `mut.sh`, logs `MU*.log`, diffs `MU*.diff`)

| Id | Mutation | Caught by |
| --- | --- | --- |
| MU4 | upper bound exclusive (`p[c] < hi[c]`) | `q1_to_q4_n3b` (the model is refused: every box with an upper bound equal to a coordinate is empty) and Q5 (the new `box_exact_bounds.toml` is refused) |
| MU4b | lower bound exclusive (`p[c] > lo[c]`) | `q1_to_q4_n3b` and Q5 (the same refusal of `corner_lo` and `point`) |
| MU11 | `tolerance` key ignored (1e-6) | P4 for all six keys (residuals 1.8e-7 to 7.7e-7) and N2 (the Tet10 run converged) |

All three caught. The registered remark "Q5 on the ADR 0438 files must still pass under MU4" was not shown separately: under MU4 the
refusal of the new file ends Q5 before it compares the other files (their bounds extend 1e-9 beyond the point, and ADR 0438's
MU4 left their tests passing).

### Deviations

- Amendment 1: N3's set-up (a box that selects nothing gives no set to compare with `xmax`); the library's refusal is the
  stronger fact and became N3b. The old test is an ignored record.
- No library change was needed or made (the rule was already inclusive and documented); (c7) and (c8) are shown by tests.
- `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets --release -- -D warnings` clean.
- Not run: `baseline_hashes`, `rcfd-cli`, `rcfd-mcp` tests (no `src` or doc file changed); the Tet10 model at 1e-10 converging
  (a registered non-convergence, N2).

### Decision by the registered rule

**Partial.** Every check of the acceptance is met on the registered run (six preconditioners reach 1e-10 on the F-381 Hex8 file
with agreement within 1.2e-12, both floors computed and printed, the exact box bounds select their node, all three mutations
caught), but the rule says an amendment after a run makes the item partial: N3 (a negative control) was met only after
amendment 1. No registered limit of a P, Q or S check was missed.

### Remaining questions

1. Status. Options: (a) accept partial by the literal rule; (b) treat the amendment as a control's defect only and mark done.
   Recommendation: (a) by the rule; the lead may set done, since no acceptance clause is open.
2. The Tet10 file cantilever still cannot reach 1e-10 (floor 5.3e-9). Options: (a) leave it documented (N2, ADR 0438); (b) a
   backward-error statistic relative to `|| |K| |u| || + ||b||` in the solver's report, so that the stall is not read as a
   failure (ADR 0282 remaining question); (c) a Tet10 model with a distributed load. Recommendation: (b), a separate item.
3. `docs/solid-case-file.md` could note that a CG tolerance below `eps ||K|| ||u|| / ||b||` cannot be reached; not edited here.

## For the lead: roadmap entries

F-526: proposed status **partial**. Note:

> ADR 0443, 2026-11-14: tests only, no library change. The F-381 Hex8 cantilever as a solid case file (box 8 x 1.5 x 1.5 m, 32 x 6 x 6, 4704 unknowns), run with method = cg and each of the six preconditioner keys, reaches the relative residual 1e-10 (9.6e-11 block_jacobi, 9.5e-11 ic0, 5.4e-11 ic_fill, 3.2e-11 amg and amg_quadratic, 2.3e-11 amg_quadratic_smoothed; 156, 85, 58, 15, 15, 12 iterations) and agrees with the direct file run within 1.2e-12 (limit 1e-7). The floor eps ||K||_inf ||u||_2 / ||b||_2 is 2.2e-11 for that model (ratio R = ||b|| / (||K|| ||u||) 1.0e-5) and 5.3e-9 for the ADR 0438 Tet10 cantilever (R 4.2e-8, 245 times smaller), above its measured stall 6e-10, and that model still stops with "not converged" at 1e-10. Node-set boxes whose upper bound, lower bound or both equal a node coordinate exactly select that node (the rule was already inclusive; no library change); the node sets of the nine solid case files equal an independent inclusive filter. Mutations: exclusive upper bound (ADR 0438 MU4), exclusive lower bound and an ignored tolerance key all caught. One negative control (N3) was met only after an amendment, so partial by the registered rule.
