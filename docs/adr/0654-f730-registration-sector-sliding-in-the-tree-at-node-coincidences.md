# ADR 0654: F-730 registration: sector sliding in the case tree at node coincidences

Status: **REGISTERED 2026-11-16, written before any code of F-730 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library, tree or test code of F-730 was written and nothing was run; the code of the base was
read only. The arithmetic below was done in `w49i-work/f730-scratch/arith.py` (disclosed; it runs no solver, it repeats the
tree's binary64 operations on the time and the turned angle). Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Roadmap item F-730 (deps F-711, F-721), proposed by [ADR 0646](0646-f711-registration-periodic-sector-sliding-in-the-case-tree.md)
(F-711) question Q5, recommendation (a). Parents: ADR 0646 (the sector path of the tree in `crates/rcfd-api/src/sim/sliding.rs`
and `sim.rs`, its set-ups FS / FS-A, FW, US and its tests `crates/rcfd-api/tests/sliding_sector_case_f711.rs`),
[ADR 0641](0641-f721-registration-periodic-sliding-at-coinciding-nodes-without-the-parts-symmetry.md) (F-721: the opt-in
`rcfd_fv::sliding_periodic::periodic_sliding_step_coupled`, its open question Q1 on the default and its follow-up on the
threshold scale, F-724), [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md) (F-710: the full-annulus
`sliding_step`, coupled at coincidences by default), [ADR 0637](0637-f303-registration-sliding-interface-in-the-case-tree.md)
(F-303: the step `dt = pitch / 8`), [ADR 0640](0640-f720-registration-periodic-sliding-at-coinciding-nodes.md) (F-720: the
base periodic step's latent defect N1). Number 0654 given by the lead. Branch `f730-work` from main 5374d521 (F-711 and F-721
merged), worktree `w49i-work/f730`, scratch `w49i-work/f730-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no new method. The coupling is ADR 0641's (a weighted minimum-norm flow on a graph is solvable exactly when every
connected part's targets sum to zero; edges between parts join them; the sector graph with seam pairs is the quotient of the
full-annulus graph), the loop ADR 0646's. No source was read; no vendor code, text or install was used. The reference is the
repository's own full-annulus tree run (ADR 0637's pair, coupled at coincidences since F-710).

## What F-730 asks

The acceptance, binding (from the lead's brief):

> The FS set-up of ADR 0646 with ADR 0637's step pitch / 8 (a node coincidence every rotor pitch on both runs) through the coupled periodic step of F-721, the sector against the full-annulus tree run within a registered limit; the F-711 results unchanged where no coincidence occurs. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **c1** ADR 0646's FS (the sector pair SP, the m = 6 vortical field, rotor walls at rest) run by the tree with ADR 0637's step
  `dt = pitch / 8 = 0.00545415391248228` (coincidences on both runs every rotor pitch) through `periodic_sliding_step_coupled`;
- **c2** the sector against the full-annulus tree run (FS-A of ADR 0646 with the same step) within a registered limit;
- **c3** the F-711 results unchanged where no coincidence occurs.

The brief adds: the way the tree selects the coupled step is a schema / default question; defaults and existing case hashes
stay bit-identical; negative controls (the base step at coincidences) and mutations are registered before the run.

## What the code read shows (5374d521, read only)

1. **The sector loop** (`SlidingRun::step_sector`, `sim/sliding.rs`) calls `periodic_sliding_step` (the base); the step's
   `flux_residual` (ADR 0629's `max_c abs(sum_f sigma_cf phi_old,f) / max_f abs(S_f)` of the remapped old flux) is not
   reported. The monitors are pushed in `sim.rs` (the step's match arm, `sliding_angle`, `sliding_interface_flux`,
   `sliding_gcl`, `sliding_non_orthogonality`, and `sliding_image` for a sector).
2. **`periodic_sliding_step_coupled`** has the arguments and the result type of `periodic_sliding_step`; on a step where every
   part's target mean is below `1e-10` of the largest target, or no coupling edge exists (a connected graph: no coincidence),
   it makes the operations of the base in the same order (ADR 0641 D2; B1 there: 350 of 350 steps bit-equal off
   coincidences).
3. **Optional objects** (`rcfd_core::settings`, ADR 0155): a non-optional field of a present table is written into the data
   with its default when the input leaves it out (`coerce`), so a new plain field of `[mesh.sliding]` would add
   `coincidences = ...` to the data and the serialisation (`to_toml`, `to_json`) of every case that writes `[mesh.sliding]`.
   An optional object stays out of the data until the input writes it, and a path into it reads the schema defaults.
4. **No golden or hash table holds `[mesh.sliding]`**: no case file under the repository writes it (search for
   `[mesh.sliding]` in `*.toml`), the MCP goldens hold no case-schema key (ADR 0637 read 5), `case_hashes_w48u.rs` needs no
   entry for an optional table.
5. **The full annulus** (`Pair::Annulus`) calls `sliding_step`, coupled at coincidences by default since F-710.
6. **The join drops slivers** (`rcfd_mesh::sliding_periodic`, the overlap loop): an overlap of area below `1e-10` of the
   smaller face's is dropped (`n_dropped`).

## Arithmetic before any run (disclosed, `arith.py`)

- `dt = 0.00545415391248228` is `pi / 576` in binary64 (difference 0); the pitch of the tree is `60f64.to_radians() =
  60 (pi / 180)`, equal to `pi / 3` in binary64. The tree's time is a sum (`s.time += dt`) and the turned angle `1.0 (s.time +
  dt)`; repeating these operations: over **400 steps** (2.18 rad, 2.083 pitches) the reduced offset lies within `1.8e-14` rad
  of a multiple of `pi / 360` (the offsets at which rotor nodes `i pi/72` meet stator nodes `j pi/60`) on exactly the 50 steps
  `n % 8 == 0`, and no closer than `1.09e-3` rad on every other step. The coincidences on the steps `8 k` are at whole rotor
  pitches `k pi / 72`: 4 coinciding nodes per sector (ADR 0641 arithmetic); the slivers of width `<= 1.8e-14` rad against a
  chord of `pi / 72` are `<= 4e-13` of a face, below the join's `1e-10`, so they are dropped (read 6).
- **Re-imaging steps 192 and 384**: `reduce` gives image 1 with reduced offset **exactly 0** at step 192 (both seams
  coincide; no wrap pair), image 2 with offset `1.6e-14` at step 384 (the wrap slivers dropped: no wrap pair). The rotor seam
  lies on a coinciding node at the 8 steps `48 m` (`m = 1..8`). The exact `r = 0` step of a run was covered by ADR 0641 only in
  its unit test T1 (its run had `P - 2.2e-16`); here it occurs in the run.
- The full annulus at the same angles: coincidence on the same 50 steps, 24 coinciding nodes, 240 interface faces (`144 + 120 -
  24`) on them and 264 elsewhere (ADR 0638 S2, ADR 0646 X1). The sector: direct faces plus wrap pairs `24 + 20 - 4 = 40` on
  the coincidence steps, 44 elsewhere.
- Courant number `dt / (r_in pi/72) = 0.125` (ADR 0646: 0.115). F-711's step 0.005 over 440 steps never comes closer than
  `9.95e-6` rad to a coincidence (ADR 0646), so its runs never take the coupled branch.

## Design (additive, opt-in)

**S1. The key** (question Q1): a new optional table **`[mesh.sliding.coincidences]`** with one field **`coupled`** (bool,
default false). `coupled = true` makes the sector loop call `periodic_sliding_step_coupled` in place of
`periodic_sliding_step` (the same arguments); every other operation of the loop is unchanged. An optional object (read 3), so
a case that writes `[mesh.sliding]` without it keeps its data, its serialisation and its run byte for byte; absent or `coupled
= false` is the base step as before.

**S2. The full annulus** is coupled at coincidences always (F-710), so the table means nothing there: with the generator
`annulus_3d_cylindrical_interface` the table is refused by the static check (`rcfd validate`), located at
`mesh.sliding.coincidences` (question Q2).

**S3. A monitor on the opt-in path only**: with `coupled = true` the sector loop also pushes **`sliding_flux_residual`** (the
step's `flux_residual`, read 1); without it no monitor is added (the default runs' monitor files unchanged).

**S4. Nothing else**: no library change (`rcfd-fv`, `rcfd-mesh` untouched), the F-303 and the default sector loops make the
same calls in the same order. Files: `crates/rcfd-api/src/sim/sliding.rs` (the schema of the sub-table, the static check, the
flag, the branch, the record), `crates/rcfd-api/src/sim.rs` (the monitor push, a few lines), `docs/case-schema.md`,
`docs/dynamic-mesh.md` (prose). New test file `crates/rcfd-api/tests/sliding_sector_coincidence_f730.rs`.

## Set-ups

ADR 0646's texts (sector SP: `nx = 24`, `nx_right = 20`, `ny = ny_right = 8`, `nz = 1`, `r_in = 1`, `r_interface = 1.5`,
`r_out = 2`, `lz = 0.1`, `sector_angle = 60`; full annulus TP: `nx = 144`, `nx_right = 120`; TS solver: piso, central, 6
non-orthogonal correctors, tolerances `1e-13` / `1e-16`, 5000 iterations; `nu = 0.01`; `zmin`, `zmax` symmetry, `inner`,
`outer` walls), `U0 = 1`.

- **FC** (c1): FS of ADR 0646 (`angular_velocity = 1`, `rotor_walls = "rest"`, the stream-function field `psi = C f(r) sin 6
  theta`, `C = 1/12`) with `dt = 0.00545415391248228` and `[mesh.sliding.coincidences] coupled = true`; **400 steps**.
- **FC-A** (c2): FS-A of ADR 0646 (the full annulus, the same field) with the same `dt`; 400 steps.
- **LC** (the library loop, coupled): the solver of a Simulation of FC (never stepped by the tree) advanced by ADR 0646's
  library loop FSL (`PeriodicSlidingInterface::sector` of the generator's blocks, `mesh_turned` with the zone,
  `periodic_rotor_wall_conditions(.., ["inner"], 0, dt)`, `step_sliding_periodic`) with `periodic_sliding_step_coupled`; on
  every step the base step data `periodic_sliding_step` are also built from the same inputs (not used to advance).
- **LB** (negative control: the base step at coincidences): the same loop with `periodic_sliding_step`, on a Simulation of
  FC.
- FC, FC-A, LC, LB advance in lockstep (four solvers, as ADR 0646 F); after every step every sector cell is matched to the
  nearest annulus cell (ADR 0646 F0).
- **FT** (fast, the same code path): FC and **FB** (FC without `[mesh.sliding.coincidences]`: the default) by the tree, LC and
  LB, 16 steps (coincidences at 8 and 16).
- **R** (c3): ADR 0646's FS, FW and US texts as they are (`dt = 0.005`, 440 steps, no coincidence) each run twice by the tree,
  without and with `[mesh.sliding.coincidences] coupled = true`, in lockstep.

## Registered checks

All in release. "Every step" is the largest value over the steps; checks over FC are also printed over the 50 coincidence
steps and over the two re-imaging steps alone, with the same limit.

### A. The coupled step in the tree (c1)

| Id | Check | Limit |
| --- | --- | --- |
| A1 (c1) | FC, the monitor `sliding_flux_residual` on every step from step 2 (399 steps; step 1, the start flux of the face interpolation, printed: ADR 0641 Q3) | `<= 1e-12` each |
| L1c (c1) | FC (tree) against LC after 400 steps: `max_c abs(u_tree - u_lib) / U0`, `max_f abs(phi_tree - phi_lib) / (U0 max abs(S_f))` | `<= 1e-12` each |
| X1 (set-up) | FC every step: direct faces plus wrap pairs 40 exactly on the steps `n % 8 == 0` and 44 on every other; no wrap pair exactly on the steps 192 and 384; FC-A: interface faces 240 exactly on `n % 8 == 0`, 264 elsewhere | exact |
| C3 (set-up) | FC: the steps where `sliding_image` increases are exactly `{192, 384}`; final `sliding_angle >= 2 pi / 3`, image 2 | exact |
| X0 (set-up) | FC, FC-A every step `sliding_non_orthogonality` | `<= 70 deg` |
| M1 | FC every step: `abs(sliding_interface_flux) / (U0 A_I)`, `sliding_gcl`, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` (ADR 0646 M1) |
| N1 (control of A1) | LB's `flux_residual` on each of the 50 coincidence steps, divided by LB's largest cell speed `max_c abs(u)` before the step | `>= 1e-5` each |

### F. The sector against the full annulus (c2)

| Id | Check | Limit |
| --- | --- | --- |
| F0 (set-up) | every step every FC cell's nearest FC-A cell within `1e-9`, one-to-one | as stated |
| F1 (c2) | every step `max_c abs(u_FC - u_FC-A) / U0` | `<= 2e-2` |
| F2 | last step, the volume-weighted rms of the same difference | `<= 1e-2` |

Printed, no limit: LB against FC-A (the base sector at coincidences against the annulus), FC against LB (the coupling's effect
on the velocity), F1 before the first coincidence step and over the coincidence steps, the times.

### B. Unchanged where no coincidence occurs (c3) and the default

| Id | Check | Limit |
| --- | --- | --- |
| B1 (c3) | LC: the coupled step data against the base step data from the same inputs (`mesh_flux`, `phi_old`, `old_defect`, `vol_old`, `gcl_residual`, `flux_residual`, `images`, `reimage`) | bit-equal on exactly the 350 steps without coincidence; differing on each of the 50 coincidence steps |
| R1 (c3) | R: FS, FW and US with the table against without, after every one of the 440 steps: cell velocities, pressure, face flux, and every monitor the run without the table writes | bit-equal; the run with the table writes `sliding_flux_residual`, the run without does not |
| L1b (default at coincidences) | FT: FB (tree, no table) against LB after 16 steps, as L1c; FB writes no `sliding_flux_residual` | bit-equal (0); as stated |
| T1 (fast cover) | FT: FC against LC after 16 steps (as L1c); FC's `sliding_flux_residual` on steps 2 to 16; LB's on steps 8 and 16 as N1 | `<= 1e-12`; `<= 1e-12`; `>= 1e-5` |
| K0c | FB: the data has no `/mesh/sliding/coincidences`, `to_toml` has no `coincidences`; FC: the data holds `/mesh/sliding/coincidences/coupled = true` | as stated |
| E1c | `Simulation::new` and the static check of FS-A with `[mesh.sliding.coincidences] coupled = true` | `Err`, located at `mesh.sliding.coincidences` |
| D1 (c3) | `cargo test -p rcfd-api --release --test baseline_hashes` (shared files `sim.rs`, `sim/sliding.rs`) | pass (unedited) |
| D2 (c3) | unedited test files that exercise the changed lines: `rcfd-api` `sliding_sector_case_f711` (the F-711 runs bit for bit as recorded there: its L1s, F1, F3, U1s), `sliding_case_f303`, `sliding_coincidence_f710`, the `rcfd-api` library tests; `rcfd-cli` `sliding_sector_case_f711`, `sliding_case_f303`, `docs_site` | pass |

Estimates. **A1**: on a step whose extended graph is connected and whose total target is zero, the coupled solve closes every
cell to its rounding (ADR 0641 A1: `2.5e-14` from step 2, `2.6e-15` on the coincidence steps, on the same sector pair with an
inlet of `1.5 U_in`); FS's speeds are below `0.89 U0` and decay, so the same or smaller. The exact `r = 0` step 192 is ADR
0641 T1's case (residual `2.1e-16` there). **L1c**: the tree's loop makes the library loop's calls (ADR 0646 L1s measured 0);
the only new operation is the branch. **X1, C3**: the arithmetic above. **M1**: ADR 0646 M1 (`1.2e-14`, `1.1e-14`,
`4.0e-12`); the coupling changes only the old-level flux. **N1**: ADR 0641 N1 measured the base's residual `3.98e-4` to
`2.19e-3` on the coincidence steps of a flow whose largest speed is `1.5 U_in`, so the ratio to the largest speed is at least
`2.7e-4`; FS has no symmetry in the coincidence spacing `pi / 12` (`sin 6 theta` shifted by `pi / 12` is `cos 6 theta`) and
a radial velocity across the interface of order `0.73 U0 cos 6 theta` at the start, so its part means are of the same order
relative to its speed; the ratio limit `1e-5` leaves a factor 27, and being relative it follows FS's decay (`0.89` to about
`0.016 U0`, ADR 0646). **F1, F2**: ADR 0646 F1 measured `1.15e-2` (FS, `dt = 0.005`, limit `2e-2`), its largest difference in
the start transient before any re-imaging; the start transient's projection removes the start field's discrete divergence,
`dt grad p'` with `lap p' = div / dt`, so it does not grow with `dt` to first order (here `+9 %`); ADR 0641 Q measured the
coupled sector step equal to the coupled annulus step from the same state to `2.2e-14`, so the coincidences should add no
difference beyond rounding; the same limits as ADR 0646 (factor 1.7 over its measured value; F2's `3.5e-4` against `1e-2`).
**B1**: ADR 0641 B1 (350 equal, 50 differ on the same step pattern); off coincidences the graph is connected and the coupled
step takes the base's operations (read 2). **R1**: no step of R comes within `9.95e-6` rad of a coincidence (arithmetic),
so every step takes the base's operations; the monitor is the only addition. **L1b**: the default loop is unchanged (S4).

Time: the lockstep is ADR 0646's FS comparison (four solvers, 89 s for 440 steps) with 400 steps, the coupling solves on 50
steps and one extra base step data per step: about 90 s. R runs six sector solvers (a sixth of the annulus's cells each) over
440 steps: about 60 s. If either takes more than about 120 s it is marked `#[ignore]` afterwards as a heavy record (no limit
changes), FT covering the code path fast.

### Mutations (scratch copy, not committed)

Each applied alone to the final tree by an exact text replacement on a `git archive` copy, the named tests run once; each
must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the sector loop calls `periodic_sliding_step` with `coupled = true` (the key read, the base step taken) | A1 or T1 |
| MU2 | the sector loop calls `periodic_sliding_step_coupled` whatever the key (coupled always) | L1b |
| MU3 | the refusal of the table on the full annulus removed | E1c |
| MU4 | the monitor `sliding_flux_residual` pushed on the default path too | L1b or R1 |
| MU5 | `periodic_sliding_step_coupled` (library) calls the body with `couple = false` | A1 or T1 |

### Decision rule

**Done**: A1, L1c (c1), F1 with F0 held (c2), B1 and R1 (c3), L1b, K0c, E1c, D1, D2 met; the set-up checks X0, X1, C3 held;
the control N1 held; every mutation caught; no golden or hash table edited. **Partial**: A1 met but F1, F2, B1, R1, L1b, a
set-up check, the control or a mutation missed (named). **Todo**: the tree cannot run FC through the coupled step (A1 missed or
the run fails). M1, F2, T1 are supporting and reported.

## Limits (not covered)

The coupling stays opt-in on the sector path (the default question is F-721's Q1, open); FW and US at `pitch / 8` are not run
(the acceptance names FS); the threshold scale of the coupled solves (F-724: a rounding-level target can still trigger the
coupled solve, changing rounding only); no `rcfd run` of the key (a case file is parsed by `Case::parse_toml` as in the tests;
ADR 0646 C1 showed the binary equal to the in-process run); the sector about `z`, forward rotation and the other limits of
ADR 0646; the Studio preview does not know the key.

## Open questions taken on the standing instruction

- **Q1. How the tree selects the coupled step.** (a) An optional sub-table `[mesh.sliding.coincidences]` with `coupled`
  (default false) (taken: opt-in; a case that writes `[mesh.sliding]` keeps its data and serialisation, read 3); (b) a plain
  field `coincidence_coupling` in `[mesh.sliding]` (adds a default to the data of every case with the table); (c) always
  coupled on the sector path (changes the default results of sector runs on coincidence steps: not taken under rule 6, see
  open). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **Q2. The table on the full annulus.** (a) Refused, located (taken: the full annulus is coupled always, a `coupled =
  false` there would not be honoured); (b) accepted and ignored. Taken (a) on the standing instruction.
- **Q3. The monitor.** (a) `sliding_flux_residual` on the opt-in path only (taken: the default monitor files unchanged); (b)
  on every sector run (changes the default runs' output). Taken (a) on the standing instruction.

## Open (not taken)

- **Q4. Coupled by default on the sector path** (with F-721's Q1 and F-724): changes the default results of sector runs on
  coincidence steps and F-720's N1 test; needs the maintainer.

## Amendment 1 (2026-11-16, written after the registered run and before any corrected run)

The registered run (code and tests f3e57a2d, run once, four tests on four threads, log `w49i-work/f730-scratch/run.log`) met
every check of the lockstep, R1 and FT, and failed the test of E1c. No limit is changed; the results go in addendum 1.

**E1c: a defective set-up of the test, not of the code.** The test parses the case with a helper that panics on a parse
error, and `Case::parse_toml` already runs the static checks of `[mesh.sliding]` (through `dynmesh::check`): the parse
returned exactly the registered refusal, `Diagnostic { path: "mesh.sliding.coincidences", line: Some(18), .. }` (printed in
the panic message), so the test stopped before it could assert it. Corrected test **E1cc**: the refusal located at
`mesh.sliding.coincidences` from `Case::parse_toml`, or, if the parse passes, from both `dynmesh::check` and
`Simulation::new`; limit as E1c (an `Err` located at the key). The registered test stays as an ignored record.

**Times.** The lockstep took 167 s and R1 172 s (four tests at once on a shared machine), above the two minutes of a heavy
record: by the registration's own provision both are marked `#[ignore]` as heavy records (no limit changes, their results
stand from the registered run), FT (2.6 s of solving) covering the code path fast.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above and in amendment 1; no limit was changed. Release builds on Windows. The
registration is 66e42f4e (alone, first); the code and the tests f3e57a2d (committed before their run); amendment 1, the
heavy-record marks and the corrected test E1cc a5a991ae (committed before the corrected run). The registered run (four tests,
four threads, log `w49i-work/f730-scratch/run.log`) was made once; the corrected E1cc ran once. The runs are deterministic;
none was repeated and no diagnostic run was made.

**Built as registered** (S1 to S4). Shared files: `crates/rcfd-api/src/sim/sliding.rs` (the import, lines 19 to 21; the
optional sub-table `coincidences` in the schema with `COINCIDENCES` and `coincidences_present`, lines 52 to 73; the refusal
on the full annulus in `check`, lines 136 to 142; the field `coupled` of `SlidingRun`, lines 164 to 165, and its reading in
`new_sliding`, lines 319 and 325; the record's `flux_residual`, lines 338 to 339 and 394; `step_sector` with the flag and the
branch, lines 349 to 357, 401 to 408, 427 to 433 and 452), `crates/rcfd-api/src/sim.rs` (the monitor push, lines 4360 to
4363). No library file changed. New `crates/rcfd-api/tests/sliding_sector_coincidence_f730.rs` (the lockstep and R1 as heavy
records, FT and E1cc as ordinary tests, E1c as the ignored record). Docs: `docs/case-schema.md`, `docs/dynamic-mesh.md`.
**No golden, no hash table, no existing test edited; no golden diff is needed.**

### Results

| Check | Limit | Measured (every step; coincidence steps; re-imaging steps) | Verdict |
| --- | --- | --- | --- |
| **A1 (c1)** FC `sliding_flux_residual`, steps 2 to 400 | `<= 1e-12` | `1.04e-13`; `7.2e-14`; `3.3e-14` (step 1, the start flux: `2.58e-3`, printed) | **met** |
| **L1c (c1)** FC tree against LC after 400 steps | `1e-12` each | velocity 0, flux 0 | **met** |
| X1 (set-up) pieces 40 / 44, no wrap pair exactly at 192 and 384, annulus 240 / 264 | exact | every step as stated | held |
| C3 (set-up) re-imaging steps; final angle, image | `{192, 384}`; `>= 2 pi/3`, 2 | `[192, 384]`; 2.181662 rad, image 2 | held |
| X0 (set-up) interface non-orthogonality | `70 deg` | 48.56 deg (sector and annulus) | held |
| M1 FC net interface flux, GCL, continuity | `1e-10`, `1e-12`, `1e-9` | `1.56e-14`, `1.85e-14`, `4.49e-12` | met |
| N1 (control) LB `flux_residual / max abs(u)` on the 50 coincidence steps | `>= 1e-5` each | smallest `6.88e-3` (50 of 50; smallest absolute `1.40e-4`) | held |
| F0 (set-up) cell match | `1e-9`, one-to-one | `7.4e-15`, one-to-one | held |
| **F1 (c2)** FC against FC-A, `max abs(du) / U0` | `<= 2e-2` | `1.25e-2`; `1.25e-2`; `3.44e-3` | **met** |
| F2 rms at the last step | `<= 1e-2` | `3.96e-4` | met |
| **B1 (c3)** coupled = base step data off coincidences; differing on them | 350 equal, 50 differ | 350, 50 | **met** |
| **R1 (c3)** ADR 0646's FS, FW, US with the table against without, 440 steps | bit-equal; monitor only with the table | bit-equal on every step (all three); monitor only with the table (largest value FS `9.8e-14`, FW `9.8e-14`, US `1.7e-16`) | **met** |
| L1b FB (default) against LB at two coincidences | 0; no monitor | 0, 0; no monitor | met |
| T1 FT: FC against LC; FC residual; LB ratio | `1e-12`; `1e-12`; `>= 1e-5` | 0, 0; `7.97e-14`; `6.88e-3` | met |
| K0c data and text | as stated | as stated | met |
| E1c (as registered) | `Err` at the key | the test panicked at the parse, whose error was the registered refusal at `mesh.sliding.coincidences` (line 18), printed: a defective test (amendment 1) | **test failed** (set-up) |
| E1cc (amendment 1) | `Err` at the key | `Case::parse_toml` refuses at `mesh.sliding.coincidences` | met |
| D1 (c3) `baseline_hashes` | pass | 3 passed, 1 ignored | met |
| D2 (c3) the named suites, unedited | pass | `rcfd-api` lib 141/4, `sliding_sector_case_f711` 5/0, `sliding_case_f303` 7/4, `sliding_coincidence_f710` 1/0; `rcfd-cli` `sliding_sector_case_f711` 2/0, `sliding_case_f303` 2/0, `docs_site` 2/0 (passed / ignored) | met |

Printed, no limit: the base sector LB against the annulus `1.31e-2` (coincidence steps `1.31e-2`, re-imaging steps
`3.42e-3`); FC against LB `9.8e-4 U0` (the coupling's effect on the velocity); F1 before the first coincidence step
`5.4e-3`; largest speed at the end `0.0169` (sector) and `0.0166` (annulus). Times: lockstep 167 s, R1 172 s (four tests in
parallel on a shared machine), hence heavy records (amendment 1); FT 2.6 s, E1cc 0.01 s.

### Mutations (scratch copy `w49i-work/f730-scratch/mut` of a5a991ae, script `mutate.py`, log `mutations.log`, target `target-mut`)

| Mutation | Must fail | Caught by |
| --- | --- | --- |
| MU1 the base step taken with `coupled = true` | A1 or T1 | T1: FC's residual `4.9e-3`, FC against LC `6.8e-4` |
| MU2 the coupled step whatever the key | L1b | L1b: FB against LB `6.8e-4` (velocity), `2.8e-4` (flux) |
| MU3 the refusal on the full annulus removed | E1c | E1cc (the corrected test): no error from the check or `Simulation::new` |
| MU4 the monitor on the default path too | L1b or R1 | L1b: FB writes `sliding_flux_residual` |
| MU5 the library's coupled step calls the base body | A1 or T1 | T1: FC's residual `4.9e-3` |

Five of five caught (MU3 by E1cc, which replaced E1c by amendment 1). Each file restored and compared.
`cargo fmt -p rcfd-api --check`: clean. `cargo clippy --release -p rcfd-api --all-targets -- -D warnings`: clean. Not run:
the whole workspace suite and whole-crate integration suites (the daily gate, ADR 0653).

### Reading

- c1 is met: the tree runs FS with ADR 0637's step through `periodic_sliding_step_coupled` (the tree loop equals the library
  loop bit for bit), and the remapped old flux is divergence-free to `1.0e-13` on every step from the second, including the
  50 coincidence steps and the re-imaging step 192 at offset exactly 0. The base step leaves `1.4e-4` there (at least
  `6.9e-3` of the speed).
- c2 is met: the sector stays within `1.25e-2 U0` of the full-annulus tree run over 2.08 pitches (F-711: `1.15e-2` at `dt =
  0.005`), rms `4.0e-4` at the end. The coupling changes the velocity by about `1e-3 U0` (FC against LB), and the base
  sector's difference from the annulus is `1.31e-2`, so at this field's size the velocity comparison alone could not tell
  the two apart. That is why the control is on the residual, as registered.
- c3 is met: off coincidences the coupled step data equal the base's bit for bit (350 of 350), ADR 0646's three runs are
  bit-identical with the table, the default loop at coincidences is unchanged (L1b), and every named suite passes unedited.

### Decision

By the registered rule: A1, L1c, F1 with F0, B1, R1, L1b, K0c, D1, D2 met; X0, X1, C3 held; N1 held; five of five mutations
caught; no golden or hash table edited. E1c's registered test failed because of a defect in the test: the case was refused at
the key, as registered, one call earlier than the test expected (amendment 1). The corrected E1cc is met. Proposed status:
**done**, with E1c's test defect disclosed; E1c is not an acceptance clause. If the lead counts the failed E1c test as a
missed check under the rule, the status is partial.

### Remaining questions

- **Q4 (open, not taken): coupled by default on the sector path.** Options: (a) make `coupled` default true (or drop the
  key) after F-724's threshold scale, together with F-721's Q1 (changes default results at coincidence steps and F-720's N1
  test, so it needs the maintainer's approval); (b) keep it opt-in. Recommendation (a) after F-724, for consistency with the
  full annulus.
- **Q5. FW at `pitch / 8`.** Not asked by the acceptance (FS only). Options: (a) run ADR 0646's FW lockstep at `pitch / 8`
  in a later item; (b) leave it, since R1 and B1 show the coupling acts only on coincidence steps and A1 is a property of the
  step. Recommendation (b).
- **Q6. The Studio preview** does not know the key (as ADR 0646 Q6). Recommendation: handle it with ADR 0646 Q6 in a UI item.

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0654, 2026-11-16: sector sliding in the case tree at node coincidences. New optional table [mesh.sliding.coincidences] with coupled = true (default false; for mesh.generator = "annulus_3d_sector_cylindrical_interface", refused on the full annulus, which is coupled always since F-710): the sector loop takes its step data from F-721's periodic_sliding_step_coupled and writes the monitor sliding_flux_residual; without the table the run is unchanged. ADR 0646's FS (the m = 6 vortical field, walls at rest) with ADR 0637's step pitch / 8 over 400 steps (50 coincidence steps, re-imaging at 192 with offset exactly 0 and at 384): the remapped old flux divergence-free to 1.0e-13 on every step from the second (limit 1e-12; the base step leaves 1.4e-4, at least 6.9e-3 of the speed, on the coincidence steps); the tree loop equals the library loop bit for bit; against the full-annulus tree run within 1.25e-2 U0 (limit 2e-2), rms 4.0e-4 at the end; mass across the interface 1.6e-14, GCL 1.8e-14. Off coincidences the coupled step data equal the base's bit for bit (350 of 350 steps); ADR 0646's FS, FW and US runs are bit-identical with the table; the default at coincidences unchanged; baseline hashes and the F-303, F-710 and F-711 suites pass unedited; no golden or hash table edited. Five of five mutations caught. One registered refusal test was defective (the parse refused the table at the key before the test looked; the corrected test is met). Limits: opt-in (the default question is F-721's Q1 with F-724), FW and US not run at pitch / 8, the sector limits of ADR 0646.

> F-711 note: change its limit "steps that land on node coincidences are not coupled on the sector path for flows without the node sets' symmetry (ADR 0640)" to "steps that land on node coincidences are coupled on the sector path only with [mesh.sliding.coincidences] coupled = true (F-730, ADR 0654)".

> F-721 note: append "Case tree (sector, opt-in [mesh.sliding.coincidences]): F-730 (ADR 0654)."
