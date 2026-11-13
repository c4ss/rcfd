# ADR 0377: F-475 registration: the Studio preview of a case with [mesh.adapt] shows the adapted mesh

Status: **REGISTERED 2026-11-12**, written before any code of F-475 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parent: [ADR 0262](0262-f342-registration-case-key-for-the-adaptive-steady-loop.md) (F-342, the case key `[mesh.adapt]`; its
"Addendum: gate 4, 2026-11-13", preview gap, from which this item comes); [ADR 0373](0373-f476-registration-field-store-of-an-adaptive-run.md)
(F-476, the store of an adaptive run opens after its first step); ADR 0138 (the mesh preview, G1 to G9), ADR 0144, 0147, 0148.
Roadmap item F-475 (deps F-342). Number 0377, branch `f475-adaptive-preview` from main 2b14108c, worktree `w49i-work/f475`,
given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults bit-identical; no hash table or golden
touched; no new crate, no unsafe code; unsigned commits without trailers; no push.

## What F-475 asks

Acceptance (`docs/roadmap.json`, binding in full):

> The Studio mesh preview of a case with [mesh.adapt] shows the mesh the run solves on (the adapted mesh of Simulation::new,
> with its cell count and mesh hash), and preview_g1's g1_g7_shipped_cases_preview_the_solvers_mesh compares
> cases/cavity/adapt_re100.toml with Simulation::new like every other case, without the base-mesh exception added on
> 2026-11-13.

Clauses: (A) the Studio preview of a `[mesh.adapt]` case shows the mesh the run solves on, the adapted mesh; (A') with its cell
count and mesh hash; (B) G1/G7 compares the shipped adaptive case with `Simulation::new` like every other case, the exception
removed. The brief adds: (C) decide what "the mesh the run solves on" means for the preview and how the preview reports the
cost of the adaptive loop; (D) the preview stays cancellable.

## What was read and run before this text (disclosed)

- **Read:** the F-475 and F-476 entries of `docs/roadmap.json`; ADR 0262's gate 4 addendum (both copies); ADR 0373 (design,
  addendum, Q3); `crates/rcfd-studio/tests/preview_g1.rs` (`solver_mesh`, `is_adaptive`, `assert_same`, G1/G7 at lines 132 to
  196, G5 at 427); `crates/rcfd-studio/tests/pv/mod.rs`; `crates/rcfd-studio/src/preview.rs` (module doc, `Built`,
  `build_text`, `refine_offer`, `resolution_text`, `Preview::request`, `spawn`, `drain`, `install`) and
  `crates/rcfd-studio/src/preview_ui.rs` (the Mesh side, `counts`); `crates/rcfd-api/src/sim/adapt.rs` (`present`,
  `base_lattice`, `new_adaptive`, `run`, `step_adaptive`); `crates/rcfd-fv/src/adaptive.rs` (signatures of
  `run_adaptive_simple*`: `setup` is an `FnMut` called once before each solve of the loop, no `Send` bound; read only, not
  changed); the F-342 and F-476 tests `crates/rcfd-api/tests/adapt_case_f342.rs` (R1b: cycles 576/97, 1269/122, 2799/188, all
  converged), `adapt_store_f476.rs` (D3: a run stopped before its first step stores the base lattice, 576 cells);
  `cases/cavity/adapt_re100.toml`; `docs/tutorials/gallery.md` (`rcfd check` prints `ok, 576 cells`).
- **Logs read:** `w49i-work/f342-scratch/run_r1.log` ("R1 tree run 6.4 s", measured beside a 68.7 s run in the same test
  binary) and `w49i-work/f476-scratch/run1-api-new.log` (`adapt_store_f476`, three tests including one full adaptive run, 1.68
  s in all). ADR 0373 A1 measured the run's mesh hash after its first step: `05bb8beae5da57da`, 2799 cells.
- **Run:** nothing of F-475. A release build of the unchanged worktree (dependencies only, `cargo build`/`--no-run`) was
  started to warm the target directory; no test was run.
- **Arithmetic:** the cell-iterations of the shipped loop, from R1b: 576 x 97 = 55,872; 1269 x 122 = 154,818; 2799 x 188 =
  526,212; sum 736,902. Shares: solve 1 7.6 %, solves 1 and 2 28.6 %, solve 3 71.4 %. A solve's cost is taken proportional
  to its cell-iterations (SIMPLEC with linear solves whose work per iteration is about linear in the cells at this size); used
  only for the cancellation estimates below.

## The contradiction in the acceptance, and the reading taken

`Simulation::new` of a `[mesh.adapt]` case holds the **base lattice** (`amr.to_mesh()`, 576 cells), not the adapted mesh: the
loop runs in the run's first step (ADR 0262 D3). It must keep doing so: ADR 0373 D2 and its test D3 (a run stopped before its
first step stores the base lattice, 576 cells) and `rcfd check` (`ok, 576 cells`, gallery.md) depend on it. So "the adapted
mesh of Simulation::new" cannot be `Simulation::new`'s `geom.mesh`, and G1's comparison with `Simulation::new` (clause B) is a
comparison with the base lattice. The two clauses are met together by a preview that has **two stages** for an adaptive case:

- **the set-up mesh**: the mesh `Simulation::new` sets the run up on (the base lattice, with the solver's numbering and hash;
  until now the preview built `build_mesh`, the generator's numbering, another hash). This is what `preview::build` returns as
  its `Built` and what G1 compares (clause B);
- **the mesh the run solves on**: the mesh after the run's first step, i.e. after the adaptive loop (2799 cells for the shipped
  case): the mesh of the run's last solve, of its final fields and of its field store (ADR 0373). The preview computes it
  with the run's own loop and the Studio **shows this one** (clause A), with its cell count and mesh hash (A').

Reading (C), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): "the mesh the run solves
on" is the mesh after the first step of the run, built by the same loop the step runs (`adapt::run`), so it equals that mesh
bit for bit. The alternative (making `Simulation::new` run the loop) is not taken: it breaks ADR 0373 D3, changes `rcfd
check`, makes every `Simulation::new` of an adaptive case (validate, check, MCP, Studio) cost seconds with no way to cancel,
and changes ADR 0262 D3, a registered design. Listed as Q1 for the maintainer.

## Design

**P1. rcfd-api: a cancellable loop and a public entry.** In `crates/rcfd-api/src/sim/adapt.rs`, `run(case)` becomes
`run_until(case, cancelled)`, whose `setup` closure (called by `rcfd_fv::adaptive` once before each solve) first polls
`cancelled()`; when it returns true the closure records it and returns an error, so the loop stops before that solve. `run(case)`
is `run_until(case, &|| false)`: the run path's loop, its arguments and its arithmetic are unchanged (the poll returns false and
is the only added work), so `step_adaptive`, every run and every F-342 / F-476 result stay bit-identical. New public items,
re-exported from `rcfd_api::sim`:

- `pub fn has_mesh_adapt(case) -> bool` (the existing `present`: the case writes the table);
- `pub struct AdaptedMesh { pub mesh: Arc<Mesh>, pub cycles: Vec<CycleRecord> }`;
- `pub fn adapted_mesh(case, cancelled: &dyn Fn() -> bool) -> Result<Option<AdaptedMesh>>`: the run's loop (`run_until`);
  `Ok(None)` when cancelled (polled exactly once before each of the `cycles + 1` solves, and nowhere else); the mesh is the
  final solver's `geom.mesh`, the one `step_adaptive` puts in `sim.geom`.

No `rcfd-fv` file is changed (other agents work there).

**P2. rcfd-studio: the build of an adaptive case.** In `preview::build_text`, after the parse and the size limits as today:

- a case without `[mesh.adapt]`: exactly today's code (`build_mesh`, one `Built`); the new fields are `None`;
- a case with `[mesh.adapt]`: the set-up mesh is `Simulation::new(&case)`'s `geom.mesh`. If `Simulation::new` refuses, the
  mesh is `build_mesh` as before (G1's `solver_mesh` falls back the same way) and the report says why it was not adapted
  (the located first diagnostic). Otherwise, after the set-up `Built` is assembled (surface, patches, 2D overlay, as today),
  `adapted_mesh(&case, cancelled)` runs with the preview's own `cancelled` (the generation check): `None` is
  `Refusal::Cancelled`; an error leaves the set-up `Built` with the error in the report; `Some` is assembled into a second
  `Built` (same text, hashes and source; its own mesh, surface, patches, overlay, counts and bounds) put in the first's new
  field `adapted: Option<Arc<Built>>`.
- New field `adapt: Option<AdaptReport>` on both: `base_cells`, `base_hash` (the set-up mesh), `solves: Vec<(cells,
  iterations, converged)>` (the loop's records), `loop_s` (wall time of `adapted_mesh`), `refused: Option<String>`.

**P3. The Studio shows the adapted mesh.** In `Preview::drain`, an `Ok` result whose `adapted` is `Some` is replaced by that
adapted `Built` before anything else looks at it; so `built`, `shown`, the viewport surface, the 2D overlay, the hover
readout, the counts line and the triangle limit are all of the mesh the run solves on, through the code they use today.

**P4. Cost and counts on the Mesh side.** `preview::adapt_text(&Built) -> Option<String>` (none without `[mesh.adapt]`), shown
under the counts line:

- adapted: `[mesh.adapt]: the mesh the run solves on after its first step, 2,799 cells, mesh hash 05bb8beae5da57da; adapted
  from the base lattice of 576 cells by 3 solves (576, 1,269, 2,799 cells; 97 + 122 + 188 iterations) in <loop_s> s`
  (a solve that did not converge is marked "not converged");
- not adapted: `[mesh.adapt]: not adapted (<reason>); shown is the base mesh`.

While the loop runs the status is the existing "Building the mesh... <s> s" with its elapsed seconds (the loop is part of the
build); the counts line's "built in <s> s" includes the loop. "Refine x2" doubles the base counts as before; for an adapted
`Built` its cells before are the base lattice's (`base_cells`), not the adapted count, so its hover is about the counts it
edits.

**P5. Cancellation.** The preview's existing rule holds (ADR 0138: a newer request makes an older build stop at its next phase
boundary and its result is discarded). The loop adds one boundary before each solve; a solve itself cannot be interrupted
without touching `rcfd-fv`, so the worst latency is one solve (for the shipped case solve 3, 71 % of the loop's work).

Derivation: equality to the run's mesh follows from running the same function (`run_until` is `run` plus a poll that does not
fire), deterministic as the F-342 tests show (tree equals library bit for bit). Showing the adapted mesh by replacing the
`Built` at the one point results enter the Studio state keeps every other preview rule (generation, skip of unchanged text,
camera keep, limits) unchanged and in force for it.

**Defaults.** No case without `[mesh.adapt]` takes a new code path in the preview (only the `has_mesh_adapt` test and two
`None` fields); no run, store, hash, event or monitor of any case changes (`run` is `run_until` with a poll that is never true).

**Test edit (approved by the brief).** In `preview_g1.rs`, the `is_adaptive` helper and the `if is_adaptive(..) { .. } else`
branch of `g1_g7_shipped_cases_preview_the_solvers_mesh` (the gate 4 comment, note and `build_mesh (base)` comparison) are
removed, so every TOML case goes through `solver_mesh`. Nothing else in that file changes.

## Checks (limits fixed now)

New tests go in a new file `crates/rcfd-studio/tests/adaptive_preview_f475.rs` (Studio, release) and
`crates/rcfd-api/tests/adapt_preview_f475.rs` (library). All runs in release.

| Id | Clause | Check | Limit |
|---|---|---|---|
| A1 | A, A' | `preview::build` of `cases/cavity/adapt_re100.toml` (repository root); `b.adapted` against the mesh of `Simulation::new` + one `step()` of the same case: mesh hash, cells/faces/points, patches (name, kind, size), the surface field by field bit for bit, 2D | `adapted` is `Some`; all equal; 2799 cells; mesh hash `05bb8beae5da57da` (ADR 0373 A1) |
| A2 | A, A' | Studio: a project with the case's text as `cases/adapt.toml`; `pv::show_case` | status `Ready`; `displayed()` and `built` are the adapted mesh (2799 cells, A1's hash); the viewport's surface is `displayed().surface` (same `Arc`); `adapt_text(displayed)` contains `2,799 cells` and `05bb8beae5da57da` |
| B1 | B | `g1_g7_shipped_cases_preview_the_solvers_mesh` with the exception removed (the approved edit), otherwise unedited | passes; the adaptive case compared "Simulation::new ... identical" |
| C1 | C | A1's `adapt` report and text | `base_cells` 576; `base_hash` = `Simulation::new`'s mesh hash; `solves` = [(576, 97, true), (1269, 122, true), (2799, 188, true)] (R1b); `0 < loop_s <= adapted.build_s`; the text contains `3 solves`, `576, 1,269, 2,799 cells` and `97 + 122 + 188 iterations` |
| D1 | D | Library: `adapted_mesh` of the shipped case with a counting `cancelled`: (a) never true; (b) true from poll 1; (c) true from poll 2. `T_a` = wall time of (a) | (a) `Some`, 3 polls, 2799 cells, hash = A1's; (b) `None`, 1 poll, time <= 0.10 `T_a`; (c) `None`, 2 polls, time <= 0.35 `T_a` |
| D2 | D | Studio: `T_full` = wall time of one uncancelled `preview::build` of the case (measured first, same test); open `cases/adapt.toml`, at 0.15 `T_full` after the request open `cases/cavity.toml` | set-up: the adaptive build still running at the switch; the cavity shown `Ready`; the adaptive result never shown after the switch; `stats.discarded >= 1`; the adaptive build thread ends (running 0) <= 0.6 `T_full` after its request |
| E1 | defaults | For every shipped TOML case without `[mesh.adapt]`: `preview::build` gives `adapted` `None` and `adapt` `None` | all 10 |
| E2 | defaults | Every `rcfd-studio` test file, release, unedited except B1's edit (binaries `rcfd-runner`, `rcfd-cli`, `fake-claude` built first, same target directory) | all pass (ignored stay ignored) |
| E3 | defaults | `rcfd-api` `baseline_hashes`, `adapt_case_f342`, `adapt_store_f476`, unedited, release | all pass (ignored stay ignored) |
| F1 | negative control | The shipped case's text with `velocity_expression_x = "1.0"` under `[boundaries.lid]` (refused by `[mesh.adapt]`'s check): `preview::build` | `Ok`; `adapted` `None`; `adapt.refused` contains `velocity_expression_x`; mesh hash = `build_mesh`'s; `adapt_text` starts `[mesh.adapt]: not adapted` |

fmt (`cargo fmt -p rcfd-api --check`, `-p rcfd-studio --check`) and clippy (`-p rcfd-api -p rcfd-studio --all-targets -- -D
warnings`) clean.

Estimates: A1, A2 need P1 to P3; the loop is the run's, so the cells and the hash are ADR 0373 A1's. C1's iteration counts are
R1b's (the same loop). D1 (b): no solve before the first poll, only the base lattice (well under 10 % of a loop of three solves);
(c): solve 1 is 7.6 % of the loop's work plus the per-solve set-up, against 0.35. D2: at 0.15 `T_full` the loop is in solve 2
(the set-up stage and solve 1 end near 0.08 to 0.10), which ends near 0.29 to 0.31; the thread then stops at the next poll, so it
ends near 0.3 `T_full`, against 0.6; without the poll it would run to about 1.0 `T_full`. The machine is shared with other
builds, so wall times move; the limits are ratios within one test to absorb that.

## Mutations (on a scratch copy, each caught by the named check)

| Id | Mutation | Caught by |
|---|---|---|
| M1 | `drain` does not replace the result by its adapted `Built` (the Studio shows the set-up mesh) | A2 |
| M2 | The set-up mesh of an adaptive case is `build_mesh` (the code before F-475) | B1 |
| M3 | The poll in `run_until`'s `setup` removed (never cancels) | D1 |
| M4 | The report's `solves` left empty | C1 |
| M5 | `has_mesh_adapt` true for every case in the preview | E1 |

## Decision rule

Done: A1, A2, B1, C1, D1, D2, E1, E2, E3, F1 met; fmt and clippy clean; each of M1 to M5 caught by its registered check.
Partial (Rule I): any registered check missed or not run, or a registered mutation not caught by its registered check, or caught
only after a post-run amendment. Todo: the Studio still shows a mesh other than the run's after its first step.

## Limits (not covered)

- A solve in progress cannot be interrupted (P5): cancelling the shipped case during its last solve waits up to that solve
  (about 70 % of the loop). Interrupting a solve needs a hook in `rcfd_fv::SimpleSolver::run`, not touched here.
- Each build of an adaptive case's text (each debounced edit in the case editor) runs the whole loop again; nothing is cached.
- The `start` event of an adaptive run still reports 576 cells (ADR 0373 Q3), and `rcfd check` the base mesh; unchanged.
- The set-up mesh (`Built` of `preview::build`) is checked by G1; the shown mesh by A1 and A2. G1 itself does not look at the
  adapted mesh (Q2).

## Amendment 1 (2026-11-13, after run 1, before the corrected run of F1): a defective set-up of F1

Run 1 of F1 (log `f475-scratch/run1-studio-new.log`) failed in its set-up, not at its limit: `preview::build` of the edited text
returned `Failed` at the parse, "boundaries.lid.velocity_expression_x: expression has dimension '1' but 'm s^-1' is required"
and "... a velocity expression is not available with [mesh.adapt]". The `[mesh.adapt]` check (`adapt::check`) runs in case
validation (through `dynmesh.rs`), so every refusal it lists stops the parse, before the preview's adaptive path; the text never
reached the path F1 is about (`Simulation::new` refusing a case that parses). Only refusals that need the mesh reach it.

Corrected set-up (F1b, new test `f1b_a_refused_adaptive_case_is_not_adapted` in the same file; the run-1 test stays as an
ignored record): the shipped case's text with a boundary that is not a patch of the mesh, `[boundaries.nosuch]` with `velocity =
"fixed"`, inserted before `[boundaries.walls]`. The parse accepts it (patch names are not known before the mesh); `Simulation::new`
refuses it ("boundaries.nosuch: mesh has no patch 'nosuch'", seen with `rcfd check` of that text in `f475-scratch/probe`, a set-up
probe, not a check). Limits unchanged except the reason looked for: `Ok`; `adapted` `None`; `adapt.refused` contains `nosuch`; mesh
hash = `build_mesh`'s; `adapt_text` starts `[mesh.adapt]: not adapted`. Run once.

Run 1 of A1, A2, C1, D1, D2 and E1 is counted as made (results in the addendum).

## Amendment 2 (2026-11-13, after the mutation run, before the corrected run of M4): a defective mutation script

In the mutation run (`f475-scratch/mutations.log`) M4 did not compile (`error[E0283]: type annotations needed`): the script
replaced `report.solves = a.cycles...collect();` by `report.solves = Vec::new(); let _ = a.cycles...collect();`, and the
`collect` of the discarded value has no type. The mutation was therefore not run; C1 was not exercised by it. Corrected M4 (the
same mutation, the solves left empty): the discarded value is typed, `let _: Vec<(usize, usize, bool)> = a.cycles...collect();`.
Run once, on the same copy, with the same check (C1, `a1_c1_the_preview_adapts_like_the_run`). M1, M2, M3, M5 of that run are
counted.

## Addendum 1: results (2026-11-13)

Code: `crates/rcfd-api/src/sim/adapt.rs` (`present` public, lines 90 to 91; `run` delegates to the new `run_until`, `AdaptedMesh`,
`adapted_mesh`, lines 532 to 567; the poll in `run_until`'s `setup`, lines 586 to 591) and `crates/rcfd-api/src/sim.rs` line 56 (the
re-export); no `rcfd-fv` file changed. `crates/rcfd-studio/src/preview.rs` (`Built::adapted`, `Built::adapt`, `AdaptReport`;
`build_text` split into `assemble` and `build_adaptive`; `adapt_text`; `drain` shows the adapted `Built`; `refine_offer` counts the
base cells) and `preview_ui.rs` (the `[mesh.adapt]` line). `preview_g1.rs`: the gate 4 exception removed; the file is now byte for
byte its text before gate 4 (92dbc164). New tests `crates/rcfd-api/tests/adapt_preview_f475.rs` (D1) and
`crates/rcfd-studio/tests/adaptive_preview_f475.rs` (A1/C1, A2, D2, E1, F1 ignored record, F1b). All runs in release, own target
directory `f475-scratch/target` (the binaries the Studio tests spawn built there first). Logs in `w49i-work/f475-scratch`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| A1 | adapted = run after first step, bit for bit; 2799 cells; hash 05bb8beae5da57da | equal (hash, counts, patches, surface, 2D); 2799; 05bb8beae5da57da; set-up mesh 576 cells 23b7ec6d5b8cfb1e (run 1) | yes |
| A2 | Studio shows the adapted mesh, viewport surface the same `Arc`, text has `2,799 cells` and the hash | shown and built 2799 cells, 05bb8beae5da57da; same `Arc`; text "... 2,799 cells, mesh hash 05bb8beae5da57da; adapted from the base lattice of 576 cells by 3 solves (576, 1,269, 2,799 cells; 97 + 122 + 188 iterations) in 2.59 s" (run 1) | yes |
| B1 | G1/G7 passes with the exception removed | passes (run 2, inside E2; run without `--nocapture`, so its per-case line was not printed) | yes |
| C1 | base 576 and `Simulation::new`'s hash; solves of R1b; 0 < loop_s <= build_s; text pieces | 576, equal; [(576, 97, true), (1269, 122, true), (2799, 188, true)]; loop 2.469 s of a 2.481 s build; all three text pieces (run 1) | yes |
| D1 | (a) Some, 3 polls, run's mesh; (b) None, 1 poll, <= 0.10 T_a; (c) None, 2 polls, <= 0.35 T_a | (a) 2799 cells, run's hash, 3 polls, T_a 13.229 s (a loaded machine); (b) 1 poll, 0.003 s = 0.000 T_a; (c) 2 polls, 0.694 s = 0.052 T_a (run 1) | yes |
| D2 | set-up holds; cavity shown; adaptive never shown after the switch; discarded >= 1; thread ends <= 0.6 T_full | T_full 3.256 s; switch at 0.151 T_full with the build running; shown 0 times; discarded 1; thread ended at 0.288 T_full (run 1) | yes |
| E1 | 10 other cases: `adapted`, `adapt`, text `None` | 10 of 10 (run 1) | yes |
| E2 | every other `rcfd-studio` test file, lib and bins | 304 passed, 0 failed, 22 ignored, 70 test binaries (run 2) | yes |
| E3 | `baseline_hashes`, `adapt_case_f342`, `adapt_store_f476` | 3 passed 1 ignored; 6 passed 2 ignored; 3 passed (as before) | yes |
| F1 | (run 1) | defective set-up: refused at the parse (Amendment 1); kept as an ignored record | not counted |
| F1b | Ok; not adapted; refused contains `nosuch`; `build_mesh` hash; text | Ok; `adapted` None; "the run refuses the case: boundaries.nosuch: mesh has no patch 'nosuch' ..."; 576 cells 7a4f4b999618bee6 = `build_mesh`; text "[mesh.adapt]: not adapted (...)" | yes |
| fmt, clippy | clean | `cargo fmt -p rcfd-api --check`, `-p rcfd-studio --check` clean; `cargo clippy --release -p rcfd-api -p rcfd-studio --all-targets -- -D warnings` clean | yes |

Run 1 of the new tests was made before `cargo fmt` reformatted the test files and two places of `preview.rs` / `sim.rs`
(line breaks and import order only; no change of behaviour); the committed code is the formatted one, and E2, E3, F1b, clippy and
the mutations ran on it.

Mutations (copy `f475-scratch/mut`, target `f475-scratch/target-mut`, log `mutations.log`; restored and compared identical):

| Id | Caught by | Result |
|---|---|---|
| M1 no swap to the adapted `Built` | A2 | A2 failed ("A2 shown") |
| M2 set-up mesh from `build_mesh` | B1 | G1/G7 failed ("cases/cavity/adapt_re100.toml: mesh hash") |
| M3 no poll in `run_until` | D1 | D1 failed ("D1 (a) polls") |
| M4 solves left empty | C1 | first run did not compile (script defect, Amendment 2); the corrected M4, run once, failed C1 ("C1 solves") |
| M5 every case adaptive | E1 | E1 failed at `axial_fan_cgrid.toml` |

Deviations: F1's set-up was defective and corrected by Amendment 1 (F1b); the M4 script was defective and corrected by Amendment
2. Both amendments were committed before their corrected run; no limit was changed. Not run: the whole workspace suite (the
lead's gate); the CLI's `adapt_case_f342` (the run path is covered by the `rcfd-api` tests of E3).

Decision by the registered rule: **partial** (read strictly). Every registered check is met and every mutation failed its
registered check, but M4 was caught only after a post-run amendment (Amendment 2), which Rule I counts as partial. If the
lead reads a set-up-only amendment as ADR 0373 Q2 (a) does, the item is done (Q3).

Questions (Q1 taken on the maintainer's standing instruction of 2026-11-11, take the recommendation):

- **Q1 (what "the adapted mesh of Simulation::new" means).** Options: (a) as registered: `Simulation::new` keeps the base
  lattice, the preview builds it as the set-up mesh (G1 compares it) and shows the mesh after the run's first step; (b) make
  `Simulation::new` run the loop, so its own mesh is the adapted one (breaks ADR 0373 D3 and `rcfd check`'s `576 cells`, makes
  every `Simulation::new` of an adaptive case slow and uncancellable). Recommendation (a), taken.
- **Q2 (G1 checks the set-up mesh, not the shown one, for the adaptive case).** Options: (a) leave it (A1 and A2 check the shown
  mesh); (b) a later edit of G1 that also compares `Built::adapted` with the mesh after a step. Recommendation (a); (b) needs the
  maintainer's approval of a test edit.
- **Q3 (status).** Options: (a) partial, Rule I read strictly (M4 caught after Amendment 2); (b) done, the amendment fixed a
  script that did not compile, not a check or a limit. Recommendation: (b) if ADR 0373 Q2 was accepted as (a); otherwise (a).
- **Q4 (cancellation within a solve).** The last solve of the shipped case (about 70 % of the loop) cannot be interrupted.
  Options: (a) leave it (D2: 0.29 T_full); (b) a cancel hook in `SimpleSolver::run` (rcfd-fv). Recommendation (a) for now.

## For the lead: roadmap entries

**F-475**, proposed status: partial (strict Rule I; done if the M4 script amendment is read as ADR 0373 Q2 (a)). New note:

> From the gate 4 fix (ADR 0262 addendum, 2026-11-13). ADR 0377, 2026-11-12: the Studio preview of a case with [mesh.adapt] builds the mesh Simulation::new sets the run up on (the base lattice, 576 cells for cases/cavity/adapt_re100.toml, with the solver's numbering and hash), then runs the run's adaptive loop (rcfd_api::sim::adapted_mesh, the first step's loop with a cancellation poll before each solve) and shows its final mesh, the mesh the run solves on after its first step (2799 cells, mesh hash 05bb8beae5da57da, equal bit for bit to Simulation::new plus one step); the Mesh side gives its cells and mesh hash and the loop's cost (3 solves of 576, 1,269 and 2,799 cells, 97 + 122 + 188 iterations, about 2.5 s); a case the run refuses is shown unadapted with the reason. g1_g7_shipped_cases_preview_the_solvers_mesh compares the adaptive case with Simulation::new like every other case, without the 2026-11-13 exception (the file is again its text before gate 4). A newer request stops the loop at the next solve (D2: the thread ended at 0.29 of a full build); no other case's preview, run, store or hash changes (E1, all rcfd-studio tests, baseline_hashes, the F-342 and F-476 tests pass unedited). Five mutations each failed their tests, M4 only after a corrected script (ADR 0377 amendment 2), hence partial by Rule I. Open: ADR 0377 Q1 (Simulation::new keeps the base lattice), Q2, Q4 (a solve in progress is not interrupted).
