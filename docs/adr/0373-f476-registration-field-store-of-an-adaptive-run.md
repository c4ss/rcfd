# ADR 0373: F-476 registration: the field store of a run with [mesh.adapt] writes the adapted mesh

Status: **REGISTERED 2026-11-12**, written before any code of F-476 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parent: [ADR 0262](0262-f342-registration-case-key-for-the-adaptive-steady-loop.md) (F-342, the case key `[mesh.adapt]`; its
"Addendum: gate 4, 2026-11-13", store gap, from which this item comes); ADR 0124 (the field store of the run path, F-263).
Roadmap item F-476 (deps F-342, F-263). Number 0373, branch `f476-adaptive-field-store` from main ca949c78, worktree
`w49i-work/f476`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults bit-identical; no hash table or golden
touched; no new crate, no unsafe code; unsigned commits without trailers; no push.

## What F-476 asks

Acceptance (`docs/roadmap.json`, binding in full):

> A run of cases/cavity/adapt_re100.toml with output.store_interval set writes a readable field store whose mesh is the
> adapted mesh (2799 cells for the shipped case) with no warning, and sf2_every_shipped_case_writes_a_readable_store runs
> that case through the store like the other ten without the skip added on 2026-11-13.

Clauses: (A) the shipped adaptive case with `output.store_interval` set writes a store that opens; (B) its mesh is the adapted
mesh, 2799 cells; (C) no warning; (D) SF2 runs the case through the store without the skip.

## What was read and run before this text (disclosed)

- **Read:** the F-476 entry of `docs/roadmap.json`; ADR 0262's gate 4 addendum; `crates/rcfd-api/src/fieldstore.rs` (the
  `Hook`: `open` reads `output.store_*`, `open_in` creates the `rcfd_tsstore::Writer` on `sim.geom.mesh` at once, `after_step`
  and `finish` write snapshots whose fields must have the writer's cell count); `crates/rcfd-api/src/runctl.rs` (`drive`: the
  hook is opened once before the step loop, line 291 to 300; `after_step` after each step, line 445; `finish` after the loop,
  line 560); `crates/rcfd-api/src/sim/adapt.rs` (`new_adaptive` builds the base lattice and sets `AdaptState { pending: true
  }`; `step_adaptive`, the first step, runs the whole loop of ADR 0242 and replaces `sim.geom` and the engine by the final
  solver's, returning `done: true`; `adapt_pending`, `pub(super)`); `crates/rcfd-studio/tests/shipped_store_sf2.rs` (read; the
  skip at its lines 70 to 75); `crates/rcfd-studio/tests/store_plan_sf.rs` (the plan of the case sets
  `output.store_interval=20`); `crates/rcfd-api/tests/fieldstore_w47c.rs` (H1 to H4) and `fieldstore_w48d.rs`;
  `crates/rcfd-api/tests/adapt_case_f342.rs` (R1: the shipped case ends on 2799 cells).
- **Run:** nothing. No code of F-476 exists, nothing was compiled or run.
- **Arithmetic:** none beyond the cell counts quoted from ADR 0262 (576 base, 2799 final).

## Cause

`drive` opens the store before the first step, so `Writer::create` records the mesh `Simulation::new` holds. For a case with
`[mesh.adapt]` that is the base lattice (576 cells); the first step swaps `sim.geom` for the adapted mesh (2799 cells), and the
first snapshot's fields no longer fit the recorded mesh. The store has no notion of a mesh that changes (ADR 0124 stores one
mesh per store), and none is needed here: the adaptive loop changes the mesh only inside its single first step, and every
later state of the run (there is none after it, `done: true`, but a `finish` snapshot follows) lives on the final mesh.

## Design

**D1. Open the store on the mesh the run solves on.** In `runctl::drive`, when the simulation's adaptive loop is still pending
(`Simulation::adapt_pending()`, made `pub(crate)`), the hook is not opened before the loop; it is opened after the first step
(before that step's `after_step`), when `sim.geom` is the adapted mesh. Everything else of the hook is unchanged: the same
`Hook::open` with the same `out_dir` / `store_dir`, the same warnings (an open-time warning of an adaptive run, such as an
unknown `output.store_fields` name, is emitted after the first step instead of before it).

**D2. A run that ends before its first step.** If the loop ends before any step (stop requested, step limit 0, wall-clock
limit), the deferred hook is opened at the end, before `finish`, on the base lattice the simulation then still holds, and
`finish` writes the final state there: the same store the code before F-476 writes in that case (the base lattice, one
snapshot at step 0), so nothing an un-stepped run produced is lost.

**D3. Defaults.** When the loop is not pending (every case without `[mesh.adapt]`, and every simulation not built by
`new_adaptive`) the code path is exactly the one before F-476: the hook is opened at the same place with the same arguments.
No byte of any store, monitor, event stream or hash of a non-adaptive run changes. No solver file is touched.

Derivation: the requirement "the store's mesh is the mesh of every stored field" is ADR 0124's invariant (the writer refuses
otherwise); with one mesh change at a known point (the first step of a pending adaptive run), opening after that point is
the minimal change that meets it. A store that records a mesh per snapshot (a format change of `rcfd_tsstore`) would also
work but changes the format, the readers and the Studio viewport, and is not needed for a mesh that changes once before the
first stored state.

**Test edit (approved by the brief).** In `shipped_store_sf2.rs`, `sf2_every_shipped_case_writes_a_readable_store`, the skip
of the `[mesh.adapt]` case (the comment and the `if ... { eprintln!(...); continue; }` block added on 2026-11-13) is removed.
The assertion `assert_eq!(ok, 10)` held only because of that skip (ADR 0262's addendum: "the [mesh.adapt] case is skipped
... so ok stays 10 of 11 cases"); with the skip removed and the case stored, `ok` is 11, so the removal includes `10 -> 11`
in that line, restoring the pre-skip form "every shipped case" (`ok == cases.len()`). No other assertion of the file changes.
This reading is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and listed as Q1.

## Checks (limits fixed now)

| Id | Clause | Check | Limit |
|---|---|---|---|
| A1 | A, B | New test `crates/rcfd-api/tests/adapt_store_f476.rs` `a1_the_shipped_adaptive_case_stores_its_adapted_mesh`: `cases/cavity/adapt_re100.toml` with `output.store_interval=20` (its start plan's value) and `output.store_precision=f32` (as SF2), `runctl::drive` with `max_steps` 3, `out_dir` and `store_dir`; `rcfd_tsstore::Reader::open(store_dir)` | opens; `n_cells() == 2799`; `mesh_hash() ==` the final `sim.geom.mesh.content_hash()`; `mesh()` loads with 2799 cells; at least 1 snapshot; every stored field of every snapshot has 2799 values; names `ux, uy, uz, p` |
| A2 | C | A1's event stream | 0 warning events whose message contains "field store"; and 0 warning events of any kind |
| A3 | D | `sf2_every_shipped_case_writes_a_readable_store` with the skip removed (and `ok` 11, Q1), run unedited otherwise | passes; its note prints 11 of 11 |
| D1 | defaults | New test `crates/rcfd-studio/tests/shipped_store_hashes_f476.rs`: the ten other shipped cases through SF2's set-up (start plan's `set()`, 3 steps of `drive`, same folders); FNV-1a 64 of `fields/store.rts` and of the index file, per case, against a table measured with this same test file on an archive copy of main ca949c78 (before any code of F-476 is compiled into it) | all 20 hashes equal |
| D2 | defaults | Existing tests unedited, release: `rcfd-api` `fieldstore_w47c`, `fieldstore_w48d`, `adapt_case_f342` (non-ignored), `baseline_hashes`, lib tests of `runctl` and `fieldstore`; `rcfd-studio` `store_plan_sf`, `saved_store_r`, `store_runs_sf4_sf5_sf8`, `viewport_store`, `stop_aa` | all pass |
| D3 | D2 of the design | New test in `adapt_store_f476.rs` `d3_an_adaptive_run_stopped_before_its_first_step_stores_the_base_lattice`: the shipped case with `output.store_interval=20`, `max_steps` 0 | store opens; `n_cells() == 576`; exactly 1 snapshot, at step 0; 0 warnings |
| N1 | negative control | In `adapt_store_f476.rs`: the shipped case with `output.store_fields=["nosuch"]`, `max_steps` 3 | a warning containing "field store disabled"; no store file |

fmt (`cargo fmt -p rcfd-api --check`, `-p rcfd-studio`) and clippy (`-p rcfd-api -p rcfd-studio --all-targets -- -D warnings`)
clean.

Estimates: A1 needs the fix; the run of the case is ADR 0262's R1 tree run (cells 576, 1269, 2799), single first step, then
`done`, so `drive` stops after one step with a `finish` snapshot (step index = the last solve's iteration count; whether the
interval 20 also fires at that index does not matter for the limits). D1 assumes the store bytes are deterministic run to run
(the store has no time stamps; the baseline hashes of the solver are deterministic); if the base measurement of D1 is not
reproducible on the base itself (two runs differ), D1 is a defective set-up and is amended before the branch run.

## Mutations (on a scratch copy, each caught by the named check)

| Id | Mutation | Caught by |
|---|---|---|
| M1 | No deferral: the hook is opened before the loop for adaptive runs too (the code before F-476) | A1 (and A2, A3) |
| M2 | D2 removed: a deferred hook not opened by a step is never opened | D3 |
| M3 | Deferral for every run (condition always true) | D2 (`fieldstore_w47c` h4: a second fresh run would append to the earlier store) or D1 |
| M4 | The deferred open passes `None` for `store_dir` (store goes to `<out_dir>/fields`) | A1 |

## Decision rule

Done: A1, A2, A3, D1, D2, D3, N1 met; fmt and clippy clean; each of M1 to M4 caught by its registered check. Partial (Rule I):
any registered check missed or not run, or a registered mutation not caught by its registered check, or caught only after a
post-run amendment. Todo: the store of the adaptive case still refused.

## Limits (not covered)

- The Studio preview of an adaptive case (F-475) is not touched; the `start` event still reports the base lattice's cell
  count (576), as ADR 0262 records.
- A store that follows a mesh changing after the first stored state (a moving or repeatedly adapting mesh) is not designed
  here; no case does that today.
- Continuing an adaptive run into an existing store: adaptive runs write no checkpoint (ADR 0262 D4), so there is no such
  continuation.

## Amendment 1 (2026-11-13, before the corrected runs): a defective build set-up

The first runs used one `CARGO_TARGET_DIR` (`w49i-work/f476-scratch/target`) for both the archive copy of main ca949c78 (the
D1 base measurement) and the worktree. Cargo's artifact names of workspace members do not depend on the workspace's absolute
path, and its freshness check compares the recorded build time with the sources' modification times; the archive copy was
built after the worktree's `runctl.rs` was last modified, so a later build in the worktree can link the base's `rcfd-api`
without recompiling it. Run 1 of A3 (SF2) failed with exactly the base's message ("field store: io error: tsstore: field 'ux'
has 2799 values, the mesh has 576", log `f476-scratch/run1-studio-new.log`, and once more in a diagnostic run,
`diag-sf2.log`), while A1 in the `rcfd-api` test binary passed with the adapted mesh: the two binaries linked different
`rcfd-api` builds. Run 1 of A3, D1 and D2 is therefore a defective set-up and is not counted. Run 1 of A1, A2, D3 and N1
(`run1-api-new.log`) is counted: A1 passing with 2799 cells and the run's mesh hash cannot come from the base code, so that
binary linked the branch's `rcfd-api` (recorded in the addendum; not re-run).

Corrected set-up: the worktree's checks A3, D1 and D2 run once more, in a fresh target directory used for the branch only
(`f476-scratch/target-br`), with the binaries the Studio tests spawn built there first. The D1 table (measured in the base copy,
built first in an empty target directory, and reproduced by a second run with identical hashes) is valid and unchanged. The
mutations run in their own copy and use `target-br` only after every branch check is done (each mutation and the restore
rewrite `runctl.rs`, so it is newer than any build). No limit changes.
