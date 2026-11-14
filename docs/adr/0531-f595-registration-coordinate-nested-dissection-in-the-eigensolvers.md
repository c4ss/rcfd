# ADR 0531: F-595 registration: coordinate nested dissection in the eigensolvers

Status: **REGISTERED 2026-11-14, written before any code of F-595 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-595 was written, compiled or run: the files under "What was read" were read; the
only arithmetic done beforehand is disclosed below (by hand, from the measured numbers of ADR 0509; no probe was run).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-595 (deps F-594), structural solver, advanced tier; the follow-up proposed by
[ADR 0509](0509-f594-registration-supernodal-ldlt-in-modal-buckling-harmonic-and-transformation.md) (its addendum, "For the
lead: roadmap entries", and its remaining question 5). Number 0531 given by the lead. Branch `f595-work` from 291a210c,
worktree `w49i-work/f595`, scratch `w49i-work/f595-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour is reached only through a new option field whose default is `None` / `false`); no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe code, the crate graph unchanged;
clean-room. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-595 asks

The acceptance, binding (copied verbatim from ADR 0509's addendum):

> Every limit not stated here is registered in the ADR before the run. The subspace and buckling eigensolvers take optional point coordinates for the supernodal factor's nested dissection and rcfd-fem modal and buckling pass the node coordinates of the free unknowns: on the clamped cube of 104 544 unknowns nnz(L) at most 0.85 of the level-structure order's (both recorded) and the frequencies within 1e-9 of the level-structure run; results bit-identical for 1 and 8 threads; the default solver's results keep their bits.

Clauses: **c1** limits registered before the run (this document); **c2** the subspace eigensolver takes optional point
coordinates for the supernodal factor's nested dissection; **c3** the buckling eigensolver too; **c4** `rcfd-fem` modal passes
the node coordinates of the free unknowns; **c5** `rcfd-fem` buckling too; **c6** on the clamped cube of 104 544 unknowns
nnz(L) by the coordinate order at most 0.85 of the level-structure order's, both recorded; **c7** its frequencies within 1e-9
of the level-structure run; **c8** results bit-identical for 1 and 8 threads; **c9** the default solver's results keep their
bits.

## What was read (base 291a210c, read only)

1. `AGENT_RULES.md`; ADR 0503 (D1, D1b: the level-structure and the coordinate dissection; the static solve's points); ADR
   0509 (all: D3 `Factorisation::Supernodal` and `factor_by`, its checks E1, E2, E7, H1 with the measured nnz(L) 110 872 668
   by level structures and 92 953 458 by coordinates on `K_FF` of `cube(32)`, the mutation M5 lesson, remaining question 5).
2. Code: `crates/rcfd-linalg/src/eigen.rs` lines 1 to 140, 440 to 1135 (`Factorisation`, the private `Factor`, `factor_by`,
   `SubspaceOptions`, `count_unchecked`, `count_below`, `factor_shifted`, `lowest_eigenpairs` up to the iteration,
   `BucklingEigenOptions`, `buckling_count`, `lowest_buckling_factors` up to its factor); `crates/rcfd-linalg/src/sparse_chol.rs`
   lines 1 to 60, 430 to 470 (`SparseOrdering`, `order_of`, `factor_count`) and 760 to 930 (`split_points`, `dissect`,
   `nested_dissection_order`, `nested_dissection_order_with_points`, which panics on a wrong length or a non-finite point);
   `crates/rcfd-fem/src/modal.rs` lines 1 to 245; `crates/rcfd-fem/src/buckling.rs` lines 1 to 240;
   `crates/rcfd-fem/src/solve.rs` lines 640 to 700 (`supernodal_direct`: the points of the free equations, `nodes[at(r).0]`);
   `crates/rcfd-fem/src/assemble.rs` lines 185 to 215; `crates/rcfd-fem/src/geometric.rs` lines 278 to 290.
3. Tests read for set-ups: `crates/rcfd-fem/tests/sa_supernodal_modes.rs` (whole: model S, E1, E2, E7, H1 and its symbolic
   count), `crates/rcfd-fem/tests/sa_f594_bits.rs` lines 1 to 120 (the digest of ADR 0291, model S, model R).
4. Every struct literal of `SubspaceOptions` and `BucklingEigenOptions` in the workspace (grep): each ends with
   `..Default::default()`, so a new field breaks none. `ModalOptions` and `BucklingOptions` are `#[non_exhaustive]` and built
   only by `new` in `rcfd-fem`.
5. **Not read:** any sparse-direct or eigensolver code or documentation outside this repository. The coordinate dissection is
   ADR 0503 D1b (derived there); nothing new is cited.

Findings that shape the design:

1. Every factor of both eigensolvers is built by `factor_by` (the shifted factor and each trial of `Shift::Auto`, the Sturm
   counts of `lowest_eigenpairs`, the factor of `K` and the counts of `lowest_buckling_factors`). The points reach every one of
   them by one added argument of `factor_by` and of its three callers (`factor_shifted`, `count_unchecked`, `buckling_count`).
2. Each factored matrix (`K + sigma M`, `K - s M`, `K + s K_sigma`, `K`) has the unknowns of `K`: one point per row of `K` is
   valid for all of them. `combine` returns `k.clone()` at a zero shift and the union pattern otherwise; the consistent mass
   and the geometric stiffness are assembled on the pattern of `K`, so for the models here every factor has the pattern of
   `K_FF`.
3. The inertia (the Sturm count) does not depend on the elimination order (Sylvester), so the order a Sturm-count factor
   used is not visible in any result; only the shifted factor's order changes the bits of the result (the solves). This is a
   limit of what a check can see (see "Limits").
4. `nested_dissection_order_with_points` panics on a wrong length or a non-finite coordinate; the eigensolvers return errors,
   so they check the points first.
5. `EigenError` is not `#[non_exhaustive]`; a new variant would break an exhaustive match outside the workspace. The existing
   `DimensionMismatch` (a wrong number of points) and `NonFinite { matrix: "points", row, col }` (a non-finite coordinate)
   describe the two refusals without a new variant.
6. The transformation path (`mpc.rs`) has reduced unknowns of reference nodes and of master combinations that have no single
   node; F-595 asks for "the node coordinates of the free unknowns", the elimination path. The coordinate option is refused
   on the transformation path (a limit, not a silent fallback).

## Arithmetic before this registration (disclosed)

By hand, from ADR 0509's measured numbers; no code run.

- **c6.** ADR 0509 H1 measured, by the same two ordering functions (`nested_dissection_order` and
  `nested_dissection_order_with_points` with the free equations' node points) on `K_FF` of `cube(32, 7850)`, nnz(L)
  110 872 668 and 92 953 458: ratio 0.8384. Both orderings are deterministic functions of the pattern (and the points) and
  their code is unchanged since 0509 (`git log` of `sparse_chol.rs` read: the last change is F-594's); the eigensolver's
  matrix at the zero shift of a clamped model is `K_FF` itself (finding 2). Expected: **the same two numbers, ratio 0.838**,
  0.012 below the limit 0.85. Any other number means the points or the pattern differ from 0509's (to be diagnosed).
- **c7.** Two backward-stable factors of the same matrix in two orders: ADR 0509 E1 measured 1.5e-15 between the supernodal
  and the envelope frequencies on S; between two orders of the same supernodal kernel the difference is of the same rounding
  origin, about 1e-14 on the large cube too (the frequencies are converged to a residual 1e-10, an eigenvalue error of order
  its square).
- **Heavy record time and memory.** ADR 0509 H1: one level-structure modal analysis of the cube in 78 s at a peak working
  set of 2.9 GB. The coordinate order stores 16 percent fewer entries; two runs one after the other and the symbolic counts:
  **about 3 minutes, peak about 3 GB** (the first run's memory is freed before the second). It is run alone when the free
  physical memory is at least 10 GB.
- **c8.** The coordinate order is computed sequentially from the pattern and the points; the factor's only parallel step is
  ADR 0503 D3's later-column update, whose summation order does not depend on the split (ADR 0509 E7 measured bit identity on
  S); nothing parallel is added.

## Design

### D1. The eigensolvers (`crates/rcfd-linalg/src/eigen.rs`)

`SubspaceOptions` and `BucklingEigenOptions` gain `pub points: Option<Vec<[f64; 3]>>` (default `None`): "the coordinates of
each unknown (`points[i]` of row `i` of `K`) for the nested dissection of [`Factorisation::Supernodal`] (ADR 0503 D1b, ADR
0531); `None`: level structures. The other factorisations do not read them." `factor_by(a, opts, factorisation, points:
Option<&[[f64; 3]]>)`: the `Supernodal` arm orders by `SparseOrdering::Given(nested_dissection_order_with_points(a, p))` when
`points` is `Some(p)` and by `SparseOrdering::NestedDissection` (as before, the same expression) when `None`; the `Envelope`
and `Sparse` arms are unchanged and ignore `points`. `factor_shifted`, `count_unchecked` and `buckling_count` take the points
and pass them on; `count_below` passes `None`. `lowest_eigenpairs` and `lowest_buckling_factors`, when the factorisation is
`Supernodal` and `points` is `Some`, check after the existing validation and before any factor: `points.len() == n` (else
`EigenError::DimensionMismatch { what: "<len> points for <n> unknowns" }`) and every coordinate finite (else
`EigenError::NonFinite { matrix: "points", row: i, col: d }`, the first in row order). With the default (`None`) every
executed expression is the one of base.

### D2. `rcfd-fem` modal and buckling (`crates/rcfd-fem/src/modal.rs`, `crates/rcfd-fem/src/buckling.rs`)

`ModalOptions` and `BucklingOptions` gain `pub coordinate_order: bool` (default `false`) and the builder
`coordinate_order(bool)`: "with [`Factorisation::Supernodal`], the factor's nested dissection by the coordinates of each
free unknown's node (ADR 0531) instead of level structures". On the elimination path, when it is `true`, the eigensolver's
options get `points = Some(p)` with `p[r]` the coordinates of the node of free equation `free[r]` (`model.mesh().nodes()[
dofs.node_component(free[r]).0]`, the rule of `solve.rs` `supernodal_direct`), built by one `pub(crate)` helper
`free_points` in `modal.rs` that buckling uses too. Refusals, both analyses, `FemError::InvalidValue { key: "factorisation" }`,
checked before any assembly: `coordinate_order` with a factorisation other than `Supernodal`; `coordinate_order` on the
transformation path (finding 6). With the default `false` nothing is built and the options are those of base.

### D3. Files

`crates/rcfd-linalg/src/eigen.rs`, `crates/rcfd-fem/src/modal.rs`, `crates/rcfd-fem/src/buckling.rs`; tests: a
`#[cfg(test)]` module at the end of `eigen.rs` (U1), `crates/rcfd-linalg/tests/sa_eigen_points.rs`,
`crates/rcfd-fem/tests/sa_coordinate_modes.rs`, `crates/rcfd-fem/tests/sa_f595_bits.rs`. Not touched: `sparse_chol.rs`,
`mpc.rs`, `solve.rs`, `harmonic_direct.rs`, `damped.rs`, `estimate.rs`, any contact file, any `rcfd-api` file, any golden.

## Checks (limits fixed now)

Release profile, each run once: `cargo test --release -p rcfd-linalg --lib eigen::f595` (U1), `cargo test --release -p
rcfd-linalg --test sa_eigen_points -- --test-threads=1 --nocapture`, `cargo test --release -p rcfd-fem --test
sa_coordinate_modes --test sa_f595_bits -- --test-threads=1 --nocapture`; the heavy record by name with `--ignored`, alone.

Model **S**: the clamped Hex8 cube `cube(11, 7850)` of `sa_supernodal_modes.rs` (4 752 unknowns; E = 2e11, nu = 0.3, `xmin`
clamped, traction (0, 0, -1e6) on `xmax`); its pre-stress `[-1e8, 0, 0, 0, 0, 0]` (uniform). Model **R**: S with
`Constraint::rigid_set("xmax", [1.0, 0.5, 0.5])`. The heavy cube **C**: `cube(32, 7850)`, 104 544 unknowns. Linalg
matrices: the tridiagonal (2, -1) of order 200 with `points[i] = (i, 0, 0)`; the 7-point Laplacian on `m^3` (diagonal 6) with
`points[i + m j + m^2 k] = (i, j, k)`. "Coordinates" below means `coordinate_order(true)` with `Factorisation::Supernodal`;
"levels" means `Factorisation::Supernodal` alone (the F-594 path). **Every aggregated error is asserted finite before it is
compared.**

**Bit record procedure (c9).** `sa_f595_bits.rs` uses only the API of base 291a210c. Before any library code it is run once
in the worktree at base plus this ADR and the file itself; it prints one FNV-1a 64 digest (ADR 0291) per quantity; the
printed digests become its constants and the file is committed before the library code. After the change it runs unchanged
and compares. Cases: **G1** `modal_analysis` of S, 6 modes, default options (frequencies, every mode, residuals, iterations,
the Sturm record); **G2** the same with `Factorisation::Supernodal` (levels; frequencies, modes); **G3** `buckling::factors`
of S with S's pre-stress, 2 factors, default options (factors, modes, residuals); **G4** the same with `Supernodal`
(factors, modes); **G5** `modal_analysis` of R, 6 modes, default options and with `Supernodal` (frequencies, modes);
**G6** `lowest_eigenpairs` of the 7-point `12^3` Laplacian with `M = I`, 5 pairs, default options and with `Supernodal`
(values, vectors); **G7** `lowest_buckling_factors` of the same `K` with `K_sigma = -I`, 3 factors, default and `Supernodal`
(values, vectors). `sa_f594_bits.rs`, `sa_nd_bits.rs` and `baseline_hashes` (rcfd-api) are run unchanged.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| U1 | c2, c3 | unit test in `eigen.rs`: `factor_by` of the 7-point `10^3` Laplacian with `Supernodal` and its points: `order() ==` `nested_dissection_order_with_points(a, points)`; with `None`: `order() == nested_dissection_order(a)`; the two orders differ | equal; equal; differ | D1 |
| P1 | c2 | tridiagonal, `lowest_eigenpairs(K, I, 5)` with `Supernodal` and its points: `max_j |lambda_j - (2 - 2 cos(j pi / 201))| / lambda_j`; Sturm `counted == expected` | `<= 1e-9`; equal | as ADR 0509 L5 (6.1e-14) |
| P2 | c3 | tridiagonal, `lowest_buckling_factors(K, -I, 3)` with `Supernodal` and its points: the same closed form; Sturm equal | `<= 1e-9`; equal | as ADR 0509 L5 (6.2e-14) |
| P3 | D1 | refusals: (a) `lowest_eigenpairs` with 199 points: `DimensionMismatch`; (b) a NaN at point 7 component 1: `NonFinite { matrix: "points", row: 7, col: 1 }`; (c) `lowest_buckling_factors` with 201 points: `DimensionMismatch`; (d) `Envelope` with 3 points (wrong length) is accepted and bit-equal to `Envelope` without points (the points not read) | as stated | D1 |
| E1 | c4, c7 (S) | S, `modal_analysis`, 6 modes, coordinates against levels: `max_j |f_c - f_l| / f_l`; every coordinate residual `<= 1e-10`; Sturm `counted == expected` | `<= 1e-9`; as stated | about 1e-14 |
| E2 | c5 | S, `buckling::factors`, 2 factors, coordinates against levels: `max_j |lambda_c - lambda_l| / lambda_l`; 2 factors | `<= 1e-9`; 2 | about 1e-14 |
| E3 | c4, c5 (wiring) | S: `K_FF`, `M_FF`, `K_sigma,FF` rebuilt in the test by the public `stiffness_with`, `consistent_with`, `geometric_stiffness`, `prescribed_equations`, `reduce`; the points `p[r]` = node of `free[r]` built in the test; `lowest_eigenpairs(K_FF, M_FF, 6)` with `Shift::Value(0)`, `Supernodal`, `points = Some(p)` and the other options of `ModalOptions::new(6)`: its values and vectors bit-equal to E1's coordinate run (values, modes at the free equations); `lowest_buckling_factors(K_FF, K_sigma,FF, 2)` with `Supernodal`, `Some(p)` and the options of `BucklingOptions::new(2)`: bit-equal to E2's coordinate run | bit for bit | D2 |
| E4 | c8 | inside rayon pools of 1 and of 8 threads, the options' threads 1 and 8: E1's coordinate frequencies and modes, E2's factors and modes | bit for bit | arithmetic c8 |
| E5 | D2 | refusals, `FemError::InvalidValue` key `"factorisation"`: (a) `modal_analysis` of S with `coordinate_order(true)` and the default factorisation; (b) `buckling::factors` of S the same; (c) `modal_analysis` of R with coordinates; (d) `buckling::factors` of R with coordinates | as stated | D2 |
| H1 | c6, c7 | (heavy, `#[ignore]`) C: `modal_analysis`, 6 modes, levels, then coordinates: both complete with 6 finite positive ascending frequencies, every residual `<= 1e-10`, Sturm `counted == expected`; `max_j |f_c - f_l| / f_l`; then (symbolic) `factor_count` of `K_FF` by `nested_dissection_order` and by `nested_dissection_order_with_points` with the points of D2: ratio `nnz_c / nnz_l`; both nnz printed; recorded: the wall time of each `modal_analysis`, the peak working set of the process, the iterations | as stated; `<= 1e-9`; `<= 0.85` | ratio 0.838 (110 872 668 and 92 953 458); about 1e-14; about 3 min, about 3 GB |
| B | c9 | `sa_f595_bits.rs` G1 to G7 digests equal to base; `sa_f594_bits.rs` and `sa_nd_bits.rs` pass; `cargo test -p rcfd-api --release --test baseline_hashes` passes | equal; pass; pass | |

**H1's nnz and the factor the eigensolver used.** The eigensolver does not report its factor; H1 counts the symbolic factor
of `K_FF` by the two orders. That these are the orders the eigensolver used is shown by U1 (the order of `factor_by`'s
factor) and E3 (the points `rcfd-fem` passes are those of D2), and that the factored matrix has the pattern of `K_FF` by
finding 2 (at the zero shift of a clamped model the shifted matrix is `K_FF` itself).

**H1's memory.** The test binary is built first; the test is started alone, as its own process, by
`f595-scratch/heavy.ps1` (ADR 0509's script with the names changed), which polls the process's `PeakWorkingSet64` every
second until exit and records the largest value and the wall time. It is started only when the free physical memory is at
least 10 GB, waiting otherwise; if that never happens on the day it is recorded as not run with the estimate. No memory
limit is stated by the acceptance; none is added (record).

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | E1: the coordinate frequencies or modes are not bit-equal to the levels run's (the coordinate order changed the factor) | holds |
| N2 | E2: the coordinate buckling factors or modes are not bit-equal to the levels run's | holds |
| N3 | P1: the coordinate eigenvalues or vectors are not bit-equal to those of `Supernodal` without points | holds |

### Planned mutations

On a `git archive` copy of the final tree in `w49i-work/f595-scratch/mut` with its own target directory
(`target-f595-mut`); each alone, by exact text replacement; the named test files run once; the file restored.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `factor_by`'s `Supernodal` arm ignores the points (always level structures) | U1 and N1 |
| M2 | `free_points` takes the free equations in reverse (`free.iter().rev()`): coordinates permuted | E3 |
| M3 | `free_points` takes the node of reduced index `r` instead of `free[r]` (coordinates of the wrong unknowns) | E3 |
| M4 | `buckling.rs` ignores `coordinate_order` (passes `None`) | E3 (buckling) and N2 |
| M5 | `modal.rs` does not refuse `coordinate_order` with a factorisation other than `Supernodal` | E5 (a) |
| M6 | the eigensolvers' point-length check removed | P3 (a) |
| M7 | `ModalOptions::new` sets `coordinate_order: true` | B (G1 or G2) |
| M8 | `factor_shifted` passes `None` to `factor_by` (the shifted factor in level-structure order) | N3 and E3 |

## Decision rule

**Done** if every check meets its limit on its registered run (H1 included), every negative control holds and every mutation
is caught by a check named for it on the first run; **partial** if a check misses or is not run, a mutation survives or is
caught only by another check, or a check is met only after an amendment made after its run; **todo** if the modules do not
build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- The order of a Sturm-count factor is not visible in any result (finding 3): that the counts use the points is shown by the
  code (one argument passed through `count_unchecked` and `buckling_count`), not by a check.
- The transformation path refuses the coordinate option (finding 6); it orders by level structures as in ADR 0509.
- The other eigen callers (cyclic, rotating, follower, geometric pre-stressed modes, CMS) build their own options with
  `points: None`; not changed.
- No case-file key (open question 1); the option is reached from Rust only.
- `EigenError` is reused (finding 5); `SubspaceOptions` and `BucklingEigenOptions` are not `#[non_exhaustive]`: a struct
  literal outside the workspace without `..Default::default()` would need the new field (none exists inside).
- H1's memory is the process peak working set polled once a second.

## Open questions

1. **Case-file key.** Not added (it would change the schema golden). Options: (a) `modal.coordinate_order = true`; (b) let
   the pending single key `solver.factor` (ADR 0509 remaining question 2) take a value `"supernodal_coordinates"` read by
   modal and buckling; (c) none. Recommendation: (b), with that key, after the maintainer approves the golden change.
2. Whether the coordinate order becomes the order of `Factorisation::Supernodal` in `rcfd-fem` (without the flag) changes
   the bits of the F-594 path: not taken, open.
3. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the refusals reuse `EigenError`'s
   `DimensionMismatch` and `NonFinite` and `FemError::InvalidValue` with key `"factorisation"`; the transformation path refuses
   the option instead of ignoring it.

## Amendment 1 (2026-11-14, before any run of any check and before any library or test code except the bit record)

Two defects of the set-up found on re-reading `split_levels` and `split_points` (`sparse_chol.rs` lines 760 to 850), corrected
here before anything is run:

1. **N3 on the tridiagonal is not a control.** On a path graph the level structure from a pseudo-peripheral end has single
   vertices as levels, and the least `|L_l| / (below above)` is the middle level; the coordinate split takes the median vertex
   and the smaller (tied: the `B`-side) boundary, the same middle vertex. Both dissections can give the same order, so the
   results can be bit-equal without any defect. **N3 is moved** to the 7-point `10^3` Laplacian with `M = I`, its points `(i,
   j, k)`: `lowest_eigenpairs(K, I, 5)` with `Supernodal` and its points must not be bit-equal (values or vectors) to the same
   without points (U1 asserts that the two orders of this matrix differ). P1 and P2 stay on the tridiagonal (closed form).
2. **M8 is not caught by E3.** E3 compares the `rcfd-fem` path with a direct call of the same (mutated) eigensolver, so a
   mutation inside the eigensolver passes it. M8 (`factor_shifted` passes `None`) must fail **N3** (and N1, N2 by the same
   effect); the "Must fail" entry of M8 is N3 alone.

Nothing else changes.
