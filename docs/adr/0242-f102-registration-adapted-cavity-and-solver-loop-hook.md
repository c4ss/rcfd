# ADR 0242: F-102 registration: the adapted lid-driven cavity and a solver-loop adaptation hook

Status: **REGISTERED 2026-11-12**, written before any code of F-102 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0054](0054-wave28c-adaptation-turbo-post.md) (wave 28C, F-102: `rcfd_mesh::adapt`, decisions D1 to D7, checks A1
to A6, addenda 1 and 2 with the A4 miss and A4b). Roadmap item F-102 (depends on F-010 and F-028; F-028 is the lid-driven
cavity validation against Ghia, Ghia and Shin (1982), `crates/rcfd-fv/tests/cavity.rs`).

Number: the lead gave 0242 to F-102. Branch `f102-adapt-acceptance` from main 379e0aba, worktree `w49i-work/f102`.

House rules (as ADR 0231 and 0236):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- no case-schema key, no option of an existing struct; `crates/rcfd-api/src/sim.rs` and `schema.rs` are not touched; neither
  `crates/rcfd-api/tests/baseline_hashes.rs` nor any hash table nor any golden is edited or re-recorded, and no existing test is
  edited (`amr_solve.rs`, `amr_diag.rs`, `adapt.rs` tests, `cavity.rs`, `octree_cavity.rs` stay as they are);
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, the crate graph unchanged (`rcfd-fv` already depends on
  `rcfd-mesh`), unsigned commits, no trailers, no push.

## What F-102 asks

The roadmap entry (acceptance, quoted, binding): "Adapted cavity matches fine uniform solution with fewer cells".

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entry F-102 (title, deps, status partial, acceptance, note); ADR 0054 in full for F-102 (audit,
  D1 to D7, A1 to A6, addenda 1 and 2); `crates/rcfd-mesh/src/adapt.rs` at 379e0aba in full; `crates/rcfd-fv/tests/amr_solve.rs`
  and the head of `amr_diag.rs`; `crates/rcfd-fv/tests/cavity.rs` and `octree_cavity.rs` in full; in
  `crates/rcfd-fv/src/simple.rs` `SimpleSettings`, `SimpleSettings::new`, `SimpleSolver`'s public fields, `SimpleSolver::new`,
  `run`, `run_until`; `rcfd_fv::diffusion::laplacian`'s documentation; `rcfd_fv::gradient::least_squares`'s signature;
  `cases/cavity/reference/ghia1982_{u,v}_centerline*.csv` and the Ghia section of `docs/reference-data.md`; ADR 0231 and 0236 for
  the style.
- **Searched:** every use of `AmrMesh` and `rcfd_mesh::adapt` outside `adapt.rs`: only the tests `rcfd-mesh/tests/adapt.rs`,
  `rcfd-fv/tests/amr_solve.rs` and `amr_diag.rs`. No solver, no API path and no case key calls the adaptation; no flow
  (momentum-pressure) solve has ever been made on an `AmrMesh` mesh with hanging nodes (`octree_cavity.rs` O9 uses the octree
  fill at level 0, so without hanging nodes).
- **One calibration run (an existing test, not a check of this ADR):** `cargo test --release -p rcfd-fv --test cavity --
  cavity_re100_simplec` at 379e0aba: 40 x 40, SIMPLEC, linear upwind, Re 100: 197 iterations, converged, 1.5 s; max
  `|u - Ghia| = 0.00276` (y = 0.8516), max `|v - Ghia| = 0.00855` (x = 0.8594). It was run to size the meshes and the cost.
- **Arithmetic on paper (no run of anything below):**
  - discretisation error at the Ghia rows, taking the 40 x 40 v error 0.0086 as `C h^2` (it also contains Ghia's own error, so
    this over-estimates the fine-mesh error): 24 x 24 about `0.0086 (40/24)^2 = 0.024`; 48 x 48 about 0.006; 96 x 96 about 0.0015.
    The coarse-to-fine difference `D_C` (defined below) is then about `0.024 - 0.0015`, i.e. 0.02 to 0.03 (the dense sample
    sees somewhat more than the Ghia rows);
  - cell counts of the adapted mesh (base 24 x 24 = 576, fixed fraction 0.4, two cycles, balance on): cycle 1 refines 230
    cells, `576 + 3 * 230 = 1266` plus the balance closure (estimate 5 to 10 percent), about 1350; cycle 2 refines 540,
    `1350 + 1620 = 2970` plus balance, about 3100 to 3300, i.e. 0.34 to 0.36 of the fine 96 x 96 = 9216;
  - cost: per SIMPLEC iteration about `7.6 ms` at 1600 cells; at 9216 cells about `7.6 * 5.8 * 2.4 = 105 ms` (cells times the
    growth of the pressure CG iterations with `n`), and about 2.5 times the iterations of 40 x 40 (500), so about 1 minute for
    the fine solve in release; every other solve is smaller.

No code of this ADR existed when this text was written, and no check below was run.

## What exists and what is missing

Exists (ADR 0054, unchanged by this item): the octree/quadtree lattice `AmrMesh` with hanging-node polyhedra, `adapt` with marks,
balance and the `Transfer` map, `transfer_field` (injection or linear, conservative), the indicators (gradient, Hessian, ZZ,
region), `MarkSpec` with dead band and level limits, `combine_marks`; a steady diffusion solve on an adapted mesh (A4 missed at
3.71 against 3, A4b passed with a wider band).

Missing, and the job of this item:

1. **The acceptance itself**: no adapted lid-driven cavity was ever solved; ADR 0054 tested "fewer cells" on a diffusion problem
   instead (its A4 text says so).
2. **A solver-loop hook**: nothing connects `AmrMesh` to a flow solver. The cavity needs one: solve, compute the indicator from the
   solution, mark, adapt, move `U` and `p` to the new mesh, re-solve.

Out of this item (stay listed as not delivered): a case-file key for the loop (it needs `crates/rcfd-api/src/schema.rs` and
`sim.rs`, which this branch does not touch: the remaining step, reported to the lead); target-node-count, edge-length-weighted,
anisotropic and register-based criteria; merge-based polyhedral conversion; precoarsening for multigrid; parallel adaptation;
adaptation of transient runs.

## Design (registered)

### The hook: `rcfd_fv::adaptive` (new file `crates/rcfd-fv/src/adaptive.rs`, one `pub mod adaptive;` line in `lib.rs`)

New public items only; no existing function, struct or default changes, so every existing result is bit-identical by
construction (no existing code path calls the new module).

- `AdaptiveSettings { cycles: usize, refine_fraction: f64, balance: bool, transfer: RefineTransfer, scale_by_length: bool,
  max_iter: usize, tol: f64 }`: `cycles` adaptation steps (so `cycles + 1` solves); per solve `SimpleSolver::run(max_iter, tol)`.
- `velocity_gradient_indicator(solver, axes, scale_by_length) -> Result<Vec<f64>>`: per cell
  `eta_c = |grad U|_F h_c` (or without `h_c`), `|grad U|_F = sqrt(sum_i |grad u_i|^2)` the Frobenius norm of the velocity
  gradient, each `grad u_i` from `rcfd_fv::gradient::least_squares` with the boundary values of `U` (so the wall and lid shear is
  seen, unlike `adapt::lsq_gradient`, which uses face neighbours only), `h_c` = `adapt::cell_lengths` (the geometric mean of the
  extents along the active axes). The scaled form is the "gradient jump" `|grad phi| h` of ADR 0054 D3 applied to the velocity;
  it is dimensionless after division by the speed, and it falls by about half when a cell is split, so cells already refined drop
  in the ranking (an equidistribution of `|grad U| h`).
- `fixed_fraction_marks(indicator, levels, max_level, fraction) -> Result<Vec<Mark>>`: the fixed-fraction (fixed-rate) marking
  strategy: among the cells with `level < max_level`, the `k = ceil(fraction * n_cells)` cells with the largest indicator (all
  eligible cells if fewer) are marked `Refine`, the rest `Keep`; ties are broken by the lower cell index (deterministic); no
  coarsening. `fraction` must be finite in `[0, 1]`, the indicator finite and of the right length; otherwise a located error.
  Source: the fixed-fraction strategy of the adaptive finite-element literature (e.g. Bangerth and Rannacher, "Adaptive Finite
  Element Methods for Differential Equations", Birkhauser 2003; cited from memory). It is chosen over a relative threshold
  because ADR 0054's A4 miss came from a relative threshold whose band was too narrow, and because the cell growth per cycle is
  then known in advance (`1 + 3 fraction` in 2-D before balance), which is what "fewer cells" needs.
- `run_adaptive_simple(amr, setup, settings) -> Result<AdaptiveRun>` and `run_adaptive_simple_with(amr, setup, settings,
  indicator)`:
  - `setup: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>` makes the fields with their boundary
    conditions (the patches keep their names across cycles, `AmrMesh::to_mesh`) and the solver settings for a mesh;
  - cycle 0: mesh = `amr.to_mesh()`, `(u, p, s) = setup(geom)`, `SimpleSolver::new(geom, u, p, s)`, `run(max_iter, tol)`;
  - cycle `k >= 1`: indicator of the last solution (`velocity_gradient_indicator` with `amr.refined_axes()` and
    `scale_by_length`, or the caller's closure in `_with`), marks by `fixed_fraction_marks` with `amr.max_level()`,
    `amr.adapt(marks, balance)`, new mesh, `setup` on it, then the cell values of `u` (each component) and `p` replaced by
    `transfer_field(old, new, transfer, ..)` of the last solution (`settings.transfer`, axes `amr.refined_axes()`), then
    `SimpleSolver::new` (which builds the face flux from the transferred `u`) and `run`;
  - result `AdaptiveRun { amr, solver, cycles: Vec<CycleRecord> }` with the final lattice and the final solver (its mesh is
    `amr.to_mesh()`), and per solve `CycleRecord { cells, iterations, converged, refined, ignored_at_max_level }` (the `adapt`
    counters of the step that made this mesh; zero for cycle 0);
  - a solve that does not converge is recorded, not an error (the caller decides); an error of `setup`, of the indicator, of
    `adapt`, `to_mesh` or `FvGeometry::new` is returned with the cycle number in its text.

### The cavity set-up (registered)

- Unit square, slab of thickness 0.1, `AmrMesh::new` with uniform base coordinates, `refine = [true, true, false]`, patches
  `walls` (xmin, xmax, ymin, Wall), `lid` (ymax, Wall), `frontAndBack` (zmin, zmax, Empty), as `amr_solve.rs`.
- Flow: Re 100 (`nu = 0.01`, lid `U = (1, 0, 0)`, walls `U = 0`, zero initial fields), `SimpleSettings::new(0.01,
  Coupling::Simplec)` with `convection = LinearUpwind`, `non_orth_correctors = 1` and `simplec_non_orth_compensation = true`
  (ADR 0050 D1: the compensation is needed with SIMPLEC and correctors; on the uniform meshes the non-orthogonal parts vanish).
  The same settings on every mesh. `run(max_iter = 20000, tol = 1e-7)`.
- Meshes: **C** coarse = base 24 x 24 (576 cells), **F** fine = base 96 x 96 (9216 cells), **U48** = 48 x 48 (2304 cells, a
  recorded reference only); **A** adapted = `run_adaptive_simple` from base 24 x 24 with `max_level = 2` (finest cells those of F),
  `cycles = 2`, `refine_fraction = 0.4`, `balance = true`, `transfer = Linear`, `scale_by_length = true`; **Ainv** = the same but
  through `run_adaptive_simple_with` with the indicator `max(eta) - eta` (refines the quietest cells).
- Sampler (in the test): the velocity at a point `x` is the mean over every cell whose closed bounding box contains `x` of
  `U_c + G_c . (x - x_c)`, `G_c` the least-squares gradient of each component with the boundary values (the cells are
  axis-aligned boxes, so the box test is exact; `x = 0.5` lies on faces of every mesh here and gets the mean of the two sides).
- Dense comparison set: `u` on `x = 0.5` at `y = j/100` and `v` on `y = 0.5` at `x = j/100`, `j = 1..99` (198 points).
  `D(M) = max over the dense set |U_M - U_F|` (component by component, lid speed 1). Ghia set: the 15 interior rows
  (`0 < y < 1`) of `ghia1982_u_centerline.csv` (column Re 100) and the 15 interior rows of `ghia1982_v_centerline_Re100.csv`;
  `G(M) = max |U_M - Ghia|` over them. Ghia's values come from the repository files (`docs/reference-data.md`, verified there).

## Registered checks

New test file `crates/rcfd-fv/tests/adapt_cavity_f102.rs`. The cavity checks share one computation (a `OnceLock` in the test
binary: C, F, U48, A, Ainv solved once per test run) and are marked `#[ignore = "heavy ..."]` (the fine solve is about a minute in
release, much longer in debug); they are run once, in release, with `--include-ignored`. The H checks are ordinary tests.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| C1a (acceptance: "fewer cells") | `N_A` against `N_F = 9216` | `N_A < 9216` | 3100 to 3300 |
| C1b | the saving is material | `N_A <= 0.5 N_F = 4608` | 0.34 to 0.36 `N_F` |
| C2 (acceptance: "matches fine uniform solution") | `D(A)` | `D(A) <= 0.01` (half of F-028's 2-percent Ghia tolerance) | 0.004 to 0.008: the lid shear layer and the top corners, where `|grad U| h` is largest, are at the fine size; the core at 1/24 or 1/48 |
| C3 | adaptation gains over the base mesh | `D(A) <= 0.5 D(C)` | `D(C)` 0.02 to 0.03, `D(A)/D(C)` 0.2 to 0.4 |
| C4 | against the published data | `G(A) <= 0.02` and `G(F) <= 0.02` (F-028's limit) | `G(F)` about 0.003 to 0.005 (the 40 x 40 run gave 0.0086), `G(A)` 0.005 to 0.01 |
| C5 | the solves and the adapted mesh are sound | every solve of C, F, A (all three cycles) has `converged = true`; A's final `Mesh::validate` Ok; `max_face_level_jump <= 1`; A has cells at level 2 (`level_histogram`) and `CycleRecord::cells` strictly increases | met; SIMPLEC with the compensation converged on every orthogonal cavity of F-028 |
| NC1 | C2 is not vacuous | `D(C) > 0.01` (the coarse mesh must fail C2) | 0.02 to 0.03 |
| NC2 | the criterion matters | `D(Ainv) > 0.01` (refining the quietest cells must fail C2) | 0.015 to 0.025: the lid layer stays at 1/24 |
| H1 | `cycles = 0` is the plain solver | `run_adaptive_simple` with `cycles = 0` on the 24 x 24 base, `max_iter = 50`, against `SimpleSolver::new` + `run(50, 1e-7)` with the same `setup`: `u`, `p` values and `phi` | bit for bit (`==` on every `f64`) |
| H2 | the transfer is applied | 12 x 12 base, `max_level = 2`, `cycles = 2`, `max_iter = 0` (no iteration, so the final fields are the transferred ones); the `setup` closure gives on its first call `u = (1 + 2x - 3y, 0.5 - x + 4y, 0)` and `p = 2 - x + 0.5 y` in the cells, and zero fields on later calls; at the end: `solver.geom.n_cells() == amr.n_cells() > 144`, and every cell value of `u` and `p` equals the linear field at the final cell centre | `max |error| <= 1e-12 (max - min)` of each field |
| H3 | the marking | `fixed_fraction_marks` on 100 cells with distinct indicators `(37 i) mod 100`, fraction 0.3, all levels 0, `max_level` 1 | exactly the 30 largest marked `Refine`, the rest `Keep`; with cells 0..9 at `max_level` (and the same indicator) the 30 largest among cells 10..99 marked and none of 0..9; equal indicators: fraction 0.5 on four cells of indicator 1 marks cells 0 and 1; fraction 0 marks none; fraction 1 marks every eligible cell |
| H4 | located errors | `fixed_fraction_marks` with fraction -0.1, 1.5, NaN; a NaN indicator; indicator and levels of different lengths; `run_adaptive_simple` with `setup` failing on its second call | `Err` whose text names the argument (`fraction`, `indicator`, `levels`) or, for the setup, contains `cycle 1` |

Recorded, not counted: `D(U48)`, `G(U48)`, `G(C)`, the cycle records of A and Ainv (cells, iterations, refined, balance count),
the level histogram of A, every solve's iteration count and time.

### Negative controls

NC1 and NC2 above: the coarse mesh and the adaptation driven by the inverted indicator must fail C2; if either passes, C2's
limit is too loose for the set-up to show anything, the set-up is defective, and the item is not done whatever C2 gives.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final commit by an exact text replacement (each pattern found exactly once),
the H tests (and, where named, the cavity tests) are run once in release in their own target dir.

| Id | Mutation | Must |
| --- | --- | --- |
| MU1 | the transferred values not written into the new fields (the solver starts from `setup`'s fields every cycle) | H2 fails |
| MU2 | `fixed_fraction_marks` takes the smallest indicators | H3 fails |
| MU3 | the `transfer` setting ignored, `Injection` used | H2 fails |
| MU4 | the last cycle's solve skipped (the returned solver is the one before the last adaptation) | H2 fails |
| MU5 | `balance` ignored (`adapt(.., false)`) | reported, not counted: C5's level jump on the cavity (a heavy run) |
| MU6 | `scale_by_length` ignored (unscaled indicator) | reported, not counted: H tests and, if time allows, the cavity numbers |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | C1a, C1b, C4, C5, NC1, H1 to H4 met |
| PR2 | C2 met with `D(A)` between 0.004 and 0.008; C3 with `D(A)/D(C)` between 0.2 and 0.4 |
| PR3 | NC2 holds (`D(Ainv)` above 0.015) |
| PR4 | `D(U48)` about 0.005, of the same size as `D(A)`, so the adapted mesh at 3200 cells is not clearly better than the uniform 48 x 48 at 2304 (Re 100 has no thin feature away from the lid) |

## Decision rule (registered)

- **done**: C1a, C1b, C2, C3, C4, C5, NC1, NC2, H1 to H4 met, and MU1 to MU4 each make their check fail. The case-file key is
  then a follow-up item (it is not in the acceptance), as are the scope items listed as out of this item.
- **partial**: C1a and C5 met, H1 to H4 met, but C2, C3, C1b or C4 missed (each recorded as a `*_known_miss` with its number and
  diagnosis), or a mutation of MU1 to MU4 not caught.
- **todo**: H1 fails (the hook changes the plain solve), C5 fails for the adapted solve (no converged flow on the adapted mesh), or
  a negative control passes (NC1 or NC2: the check cannot discriminate); or any existing test changes.

MU5 and MU6 are reported and do not count.

## Limits of this item

- One flow (Re 100), one base, one fraction, two cycles. The fraction 0.4 and the cycle count are chosen here from the cell
  arithmetic above, not from a run; they are not tuned after it.
- "Matches" is defined on the two centrelines (198 points), the quantity F-028 validates, not over the whole field.
- Steady SIMPLE only; no transient adaptation, no coarsening in the loop (`fixed_fraction_marks` never coarsens), no load
  balancing; the face interpolation at a 2:1 face has no skewness correction (the face centre is off the centre line), which is
  a first-order error at those faces and is part of what is measured.
- No case key: the loop is reachable from Rust only.

## Open questions and choices for the maintainer

- **Q1. Status of F-102.** Recommendation: by the rule, with the maintainer's acceptance.
- **Q2. Marking strategy of the hook.** Options: (a) fixed fraction (this design); (b) `MarkSpec` thresholds on the scaled
  indicator; (c) both behind an enum. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):
  (a) now; (c) when the case key is added (a new key, no default changes).
- **Q3. Indicator.** Options: (a) `|grad U|_F h` with boundary values (this design); (b) vorticity magnitude times `h`; (c) the
  ZZ indicator of `adapt` per component. Taken on the same standing instruction: (a), the D3 gradient criterion of ADR 0054
  applied to the velocity; the `_with` entry point takes any other.
- **Q4. The case-file key** (`mesh.adapt` or a solver section): not taken here; it needs `schema.rs` and `sim.rs` and a new key
  (no default changes, but a schema change is the lead's to schedule). Listed for the lead.

## Addendum 1: results (2026-11-12)

The registered run was made once at 0efc2b48 (the hook and the tests, committed before the run):
`cargo test --release -p rcfd-fv --test adapt_cavity_f102 -- --include-ignored --nocapture --test-threads=4`, 12 passed, 83.5 s.
No limit, set-up or test was changed after it, and no amendment was needed.

### Results

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| C1a (acceptance) | `N_A < 9216` | `N_A = 2799` | yes |
| C1b | `N_A <= 4608` | 2799 (0.304 `N_F`) | yes |
| C2 (acceptance) | `D(A) <= 0.01` | `D(A) = 0.00231` | yes |
| C3 | `D(A) <= 0.5 D(C)` | `D(C) = 0.01624`, `D(A)/D(C) = 0.142` | yes |
| C4 | `G(A) <= 0.02`, `G(F) <= 0.02` | `G(A) = 0.00916`, `G(F) = 0.00912` | yes |
| C5 | every solve converged; validate; jump <= 1; level 2 present; cells increase | C 97 it., F 853 it., A cycles 97 / 122 / 188 it., all converged; validate Ok; jump <= 1; levels {0: 338, 1: 449, 2: 2012}; cells 576, 1269, 2799 | yes |
| NC1 | `D(C) > 0.01` | 0.01624 | yes |
| NC2 | `D(Ainv) > 0.01` | 0.01662 (2811 cells) | yes |
| H1 | bit for bit | `u`, `p`, `phi` equal (`==`), 50 iterations each | yes |
| H2 | `<= 1e-12` of the range | 702 cells; worst 1.1e-15, 1.8e-15, 8.9e-16 (u_x, u_y, p) | yes |
| H3 | as registered | all assertions | yes |
| H4 | as registered | all assertions | yes |

Recorded, not counted: `D(U48) = 0.00302` (2304 cells), `G(U48) = 0.00874`, `G(C) = 0.01300`, `G(Ainv) = 0.01329`; A's
cycles marked 231 and 508 cells (`ceil(0.4 N)`) and split 231 and 510 (`refined` includes the balance splits: 0 and 2), none
ignored at the maximum level; times C 0.2 s, F 77.9 s, U48 1.9 s, A 2.2 s, Ainv 1.1 s.

Against the predictions: PR1 met. PR2: `D(A)` 0.0023, below the predicted 0.004 to 0.008, and `D(A)/D(C)` 0.14, below 0.2 to 0.4;
`D(C)` (0.016) was smaller than the paper estimate (0.02 to 0.03). PR3 met (0.0166 > 0.015). PR4 roughly: the adapted mesh
(2799 cells, `D` 0.0023) is better than the uniform 48 x 48 (2304 cells, `D` 0.0030) but not by much, and per cell about even.
`G(F)` (0.0091) was above the predicted 0.003 to 0.005 and above `G(U48)` (0.0087): the fine-mesh difference to Ghia does not
fall with the mesh, so at this level it is dominated by something other than our discretisation (Ghia's own error at its 129 x
129 grid, and the linear sampler at the near-wall rows); this is why C2 compares with the fine solution and not with Ghia.

### Mutations (scratch copy `w49i-work/f102-mut`, a `git archive` of 0efc2b48, target dir `target-f102-mut`, release)

Each applied alone by exact replacement (each pattern found once), the file restored and checked byte for byte afterwards.

| Id | Mutation | Result |
| --- | --- | --- |
| MU1 | transferred values not written | H2 fails (caught) |
| MU2 | smallest indicators marked | H3 fails (caught) |
| MU3 | `Injection` forced | H2 fails (caught) |
| MU4 | last solve skipped | H2 fails (caught) |
| MU5 (reported) | balance ignored | C5 fails at the level-jump assertion (cavity run with `--include-ignored`, filter `c5_`) |
| MU6 (reported) | indicator never scaled | no H test fails (H tests do not depend on the scaling); the cavity was not run for it |

### Format and lint

`cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean. Only `rcfd-fv`
was touched (one line in `lib.rs`, the new `adaptive.rs` and the new test file).

### Deviations and what was not run

- None of the registered set-up was changed. The H tests, though cheap, ran in the same invocation as the heavy tests.
- Not run: the whole workspace suite (by instruction), the rest of the `rcfd-fv` suite (no existing code path changed; the new
  module is called by nothing else), the cavity numbers under MU6, the gate on an archive copy.

### Decision by the registered rule

**done**: C1a, C1b, C2, C3, C4, C5, NC1, NC2 and H1 to H4 met; MU1 to MU4 each caught. The acceptance "adapted cavity matches
fine uniform solution with fewer cells" is met in the library: 2799 cells against 9216, centreline difference 0.0023 of the lid
speed. The case-file key and the scope items of the old note go to follow-up items.

### Remaining questions

- **R1. The case-file key.** Options: (a) a new optional `adapt` block (cycles, fraction, max level, indicator) on steady
  incompressible runs of box meshes, absent = today's path bit for bit; (b) keep the loop Rust-only. Recommendation (a), as a
  follow-up item (it needs `schema.rs` and `sim.rs`, which this branch did not touch).
- **R2. Adaptation pays little on Re 100.** The uniform 48 x 48 is nearly as close per cell. Options: (a) add a case with a
  localised feature (Re 1000 corner eddies, or a step) to the follow-up; (b) accept. Recommendation (a), recorded, no limit
  proposed here.
- **R3. Scope items of the old note** (target-node-count, edge-length, anisotropic, register-based criteria, merge-based
  polyhedral conversion, multigrid precoarsening, parallel adaptation). Recommendation: one follow-up item for the criteria and
  leave the rest in the follow-up's note.

### For the lead: roadmap entries

Proposed status of F-102: **done**. New note:

> DONE (WAVE 28C ADR 0054; ADR 0242): rcfd_mesh::adapt (gradient, Hessian, ZZ and region indicators, dead-band marks, hanging-node quadtree/octree refinement and coarsening with conservative transfer, polydual) and the solver-loop hook rcfd_fv::adaptive (run_adaptive_simple: SIMPLE solve, |grad U| h indicator, fixed-fraction marks, AmrMesh::adapt with balance, linear transfer of U and p, re-solve). Acceptance met in the library (ADR 0242): Re 100 lid-driven cavity, 24 x 24 base with two cycles at fraction 0.4 gives 2799 cells against 9216 of the uniform 96 x 96, centreline difference to the fine solution 0.0023 of the lid speed (limit 0.01; coarse 0.0162, inverted indicator 0.0166), Ghia difference 0.0092 (fine 0.0091). The ADR 0054 diffusion check A4 stays a known miss (3.71). Not delivered (follow-ups): a case-file key; target-node-count, edge-length, anisotropic and register-based criteria; merge-based polyhedral conversion; multigrid precoarsening; parallel and transient adaptation.

Follow-up item 1: title "Case-file key for the adaptive steady loop", deps F-102, acceptance: "An optional adapt block on a
steady incompressible box-mesh case runs rcfd_fv::adaptive and reproduces the ADR 0242 cavity numbers; cases without the block
give identical hashes."

> TODO (from ADR 0242): rcfd_fv::adaptive exists in the library only; the key needs crates/rcfd-api/src/schema.rs and sim.rs. Absent key = today's path bit for bit.

Follow-up item 2: title "Further adaptation criteria (target cell count, edge-length weighting, anisotropic, register-based)",
deps F-102, acceptance: "Each criterion has a closed-form marking check, and a target-count run lands within 10 percent of its
target on the ADR 0242 cavity."

> TODO (from the F-102 scope list, ADR 0054 D7 and ADR 0242): criteria not yet built; merge-based polyhedral conversion, multigrid precoarsening and parallel adaptation stay listed here until split.
