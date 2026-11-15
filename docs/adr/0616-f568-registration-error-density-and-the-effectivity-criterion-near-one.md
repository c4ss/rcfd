# ADR 0616: F-568 registration: the error indicator per unit volume and the effectivity criterion near one

Status: **REGISTERED 2026-11-15, written before any code of F-568 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-568 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand (no script, no finite-element code); the numbers of F-432 quoted
here are those of its logs (`w49i-work/f432-scratch/run_fast.log`, `run_heavy.log`) and of ADR 0478 addendum 1. The bit record
B1 of the existing estimate outputs is a test file written and run on the base after this commit and before any library code
(D4). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-568 (deps F-432), the follow-up proposed in [ADR 0478](0478-f432-registration-patch-stress-recovery-and-energy-norm-error-estimate.md)
(its remaining questions Q4 and Q5). Number 0616 given by the lead. Branch `f568-work` from local main b61032f4, worktree
`w49i-work/f568`, scratch `w49i-work/f568-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or ignored record
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-568 asks

> Every limit not stated here is registered in the ADR before the run. EnergyError gives the density eta_e/|K|^{1/2} beside eta_e, the absolute values keeping their bits; on the two-layer plate with a hole of ADR 0478 the tenth of the elements with the largest density contains every element with a face on the hole; on the manufactured solution of F-378 (Hex20 and Tet10 at n = 8, 16 and a finer heavy record where feasible) abs(theta - 1) on each mesh is at most max(abs(theta - 1) on the previous mesh, 1e-2); the results of F-432 keep their bits.

Clauses: **a0** every limit registered here; **a1** `EnergyError` gives the density `eta_e/|K|^{1/2}` beside `eta_e`; **a2** the
absolute values keep their bits; **a3** on the two-layer plate the tenth of largest density contains every element with a face
on the hole; **a4** on the manufactured solution, Hex20 and Tet10 at `n = 8, 16` and a finer heavy record where feasible,
`abs(theta - 1)` on each mesh at most `max(abs(theta - 1)` on the previous mesh, `1e-2)`; **a5** the results of F-432 keep their
bits.

Readings, registered:

- **(a1) `|K|`:** the volume of the element by the estimate rule of F-432 (`estimate_rule(kind)`: hexahedra `gauss_3`, tetrahedra
  `tet_14`), `|K| = sum_q w_q det J(xi_q)`, the same weights with which `eta_e^2` is integrated, so that `(eta_e/|K|^{1/2})^2`
  is the rule's mean of the integrand `(sigma* - sigma_h) . D^{-1} (sigma* - sigma_h)` over the element. Exact for affine
  tetrahedra and for parallelepipeds; on the curved Hex20 of the plate a quadrature value (checked against the plate's area in
  V1). "Beside": new accessors, the existing ones unchanged.
- **(a2, a5) "keep their bits":** every value the F-432 API returns (`element_errors`, `estimate`, `solution_norm`, `relative`,
  `ranked`, and the recovered nodal stress of `Recovery`) equal to the bit to its value at the base b61032f4 on four models
  (D4); and the existing tests of F-432 (`sa_estimate.rs`, `sa_estimate_default_bits.rs`, `si_hex8x_estimate.rs`,
  `si_hex8x_bits.rs`) pass unchanged. Reading also includes: the F-432 effectivities at `n = 4, 8` (fast, the same solver as
  F-432) equal to the printed digits of the F-432 log.
- **(a3) "the tenth of the elements with the largest density":** the `ceil(N/10)` elements of largest density, ties to the lower
  element index (the rule of `ranked`), `N = 1152`, the tenth 116; "every element with a face on the hole": every element with a
  face whose nodes all satisfy `|sqrt(x^2 + y^2) - r| <= 1e-9` m (the set of ADR 0478 reading a7, 48 elements). The plate,
  solver and options exactly those of `sa_estimate.rs` (`plate(2)`, `SparseDirect`, `PatchOptions::new()`).
- **(a4) "each mesh" and "the previous mesh":** the sequence `n = 8, 16, 24` per element kind; the criterion applies to `n = 16`
  (previous `8`) and `n = 24` (previous `16`); `n = 8` has no previous mesh in the clause. "A finer heavy record where
  feasible": `n = 24` (arithmetic 4: `n = 32` is estimated beyond the memory free on this shared machine; `n = 24` refines `h` by
  1.5, not 2, which the clause does not require). Case D of F-378 with the field, body force, `E = 1`, `nu = 0.3` and error rule
  of `sa_estimate.rs`; `theta = eta/||u - u_h||_E` measured exactly as `measure` there. Solver: `SupernodalDirect` (ADR 0503) at
  every level of the heavy record (arithmetic 4: `SparseDirect` took 519 s for Tet10 at `n = 16`); at `n = 8, 16` it gives the
  same solution as F-432's `SparseDirect` to rounding, so `theta` agrees with the F-432 log within `1e-5` (E2).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0478 whole (design D1 to D8, the addendum, amendment 1, Q4, Q5 and the follow-up text);
  ADR 0503 (the supernodal factor, its heavy record: 304 842 unknowns, 81 s, 5.04 GB); ADR 0572 and 0573 ("For the lead" style).
- `crates/rcfd-fem/src/estimate.rs` (`EnergyError`, `energy_error`, `parallel_map`, `estimate_rule`).
- `crates/rcfd-fem/tests/sa_estimate.rs` (the field, case D, `measure`, `run`, `check_effectivity`, the plate, `on_hole`, P2),
  `sa_estimate_default_bits.rs` (the digest pattern), the head of `si_hex8x_estimate.rs`; the users of `energy_error` by grep
  (only tests; `EnergyError` is built only in `estimate.rs`).
- `crates/rcfd-fem/src/solve.rs` (`StaticSolver`).

## Arithmetic done before the code (disclosed)

1. **Bits of the absolute values.** In `energy_error` each element accumulates `err += w_q (sigma* - sigma_h) . C (sigma* -
   sigma_h)` and `sol += w_q sigma_h . C sigma_h` in rule order and pushes `err.sqrt()`; the totals sum `err` and `sol` in element
   order. Adding a third accumulator `vol += w_q` in the same loop and returning it beside `(err, sol)` changes no operation on
   `err`, `sol` or the totals, so every absolute value keeps its bits; the density `eta_e/vol^{1/2}` is computed after, from the
   pushed `eta_e`. Nothing else in `estimate.rs` changes.
2. **Volumes (V1).** Hex8 and Hex20 on the unit cube at `n = 4`: `|K| = 1/64` from `gauss_3` (27 weights, `det J = 1/512`
   constant), rounding about `1e-16` relative per sum, limit `1e-13`. Tet10 (Kuhn split, affine): `|K| = 1/384` from `tet_14`
   (weights summing to `1/6` to rounding), limit `1e-13`. Plate: the sum of `|K|` against the exact volume `t (A B - pi r^2/4)`
   (`A = 0.1`, `B = 0.05`, `r = 0.005`, `t = 0.001`); the hole is a chain of 24 quadratic arcs of angle `pi/48`; the area between a
   circular arc of angle `a` and its interpolating parabola is of order `r^2 a^5/1000` (by hand, the third-order term of the
   chord sagitta series; an order estimate), about `1e-9 r^2` per arc, `2e-8 r^2` in all against an area of about `200 r^2`:
   `1e-10` relative; the radial lines are straight and the outer boundary straight. Limit `1e-6` relative (margin for the
   quadrature of the non-polynomial `det J` of the graded radial map).
3. **The plate density (P3), a crude estimate that says it may miss.** For a quadratic element the stress error is about `h^2`
   times the second derivatives of the stress, and the density measures it per unit volume (no `|K|^{1/2}` factor). By hand
   from Kirsch (`nu = 0`, `sigma_0` units, at the hole `rho = r`): `d^2 sigma_tt/drho^2 = (6 - 60 cos 2t)/(2 r^2)` (33 at 90
   degrees, -27 at 0), `d^2 sigma_rr/drho^2 = (-6 + 36 cos 2t)/(2 r^2)`, `d^2 sigma_rt/drho^2 = 24 sin 2t/r^2`, and the angular
   `d^2 sigma_tt/ds^2 = 8 cos 2t/r^2`: no angle of the hole is smooth. With ADR 0478's grading (radial size `0.0305 r` at the hole,
   growing by 1.18 per ring, angular step `0.0654 rho`) the sum `h_r^2 |sigma''_r| + h_t^2 |sigma''_t|` is about `0.065` on the
   first ring at 90 degrees and `0.057` on the fifth (`rho = 1.22 r`): the first ring larger by about 14 percent, the second by
   a few percent only. The outer elements have densities about `1e-3` of the hole's (sizes about `1.3 r`, second derivatives
   about `3e-4/r^2`), so with the density the competition is among the first four or five rings (48 elements each; the tenth is
   2.4 rings). In favour of the hole: the hole-edge nodes are extrapolated from the first interior ring (ADR 0478 D3), which
   adds to the error of the elements touching the hole. Against: in F-432 the hole-edge elements near 0 and 45 degrees ranked
   below 800 by the absolute indicator while those near 90 degrees ranked 38 to 65, all of the same volume, so their densities
   differ by the same factor, which the Kirsch estimate does not explain; if that factor is three or more, the second and third
   rings near 90 degrees outrank the first ring near 0 degrees. Estimate: undecided; the check is run as registered and may miss.
4. **The finer record.** Unknowns (case D): Hex20 `3 ((n - 1)^3 + 3 n (n - 1)^2)`: 4 557, 42 525, 150 765 at 8, 16, 24 and
   366 363 at 32; Tet10 `3 (2n - 1)^3`: 10 125, 89 373, 311 469 at 8, 16, 24 and 750 141 at 32. Nested dissection's top
   separator is a node plane: Hex20 at 24 about `23^2 + 2 x 23 x 24 = 1 633` nodes (4.9e3 unknowns), Tet10 at 24 `49^2 = 2 401`
   nodes (7.2e3), against 6 486 unknowns in ADR 0503's heavy cube (5.04 GB, 81 s): Hex20 at 24 estimated 2 to 3 GB and under two
   minutes, Tet10 at 24 4 to 7 GB and 1 to 5 minutes. At 32: Hex20 about 8.8e3, Tet10 about 1.27e4 separator unknowns, the factor
   growing about as the square, 10 GB and 20 GB or more: not feasible with the 8.7 GB free on this shared machine when this was
   written. Hence `n = 24`. The heavy record is started alone, as its own process, only when the free physical memory is at least
   10 GB (the rule of ADR 0503 H1); if that never happens on the day, `n = 24` is recorded as not run.
5. **Expected `theta` (E1).** F-432 (`SparseDirect`): Hex20 `1.02782, 0.99392` and Tet10 `1.00514, 0.99168` at `n = 8, 16`;
   so at 16 Hex20 `0.0061 <= max(0.0278, 0.01)` and Tet10 `0.0083 <= max(0.0051, 0.01) = 0.01` (the literal F-432 reading
   missed this pair: the floor is what admits it). At 24, a two-term model `theta - 1 = A h^p + B h^2` with `p` the excess of the
   recovered over the raw L2 order of F-432 (Tet10 `p = 1.26`, Hex20 `p = 1.52`, ADR 0478 M5), fitted to the two points: Tet10
   `A = -0.787`, `B = 4.00`, giving `-0.0074` at 24; Hex20 `A = -3.15`, `B = 10.3`, giving `-0.0072`. Both under `0.01`, so both
   pairs are expected to hold, the Tet10 pair by a margin of about 25 percent of the floor. A crude extrapolation, disclosed.

## Design

### D1. The density (`estimate.rs`)

`EnergyError` gains two private fields, `volume: Vec<f64>` (`|K|` of reading a1) and `density: Vec<f64>` (`eta_e/|K|^{1/2}`),
and three accessors: `element_volumes() -> &[f64]`, `element_densities() -> &[f64]`, and `ranked_by_density() -> Vec<usize>`
(descending density, ties to the lower index, the comparator of `ranked`). In `energy_error` the per-element closure returns
`(err, sol, vol)` with `vol += ip.weight` beside the two existing sums; the assembly loop pushes `vol` and `eta_e / vol.sqrt()`
after the existing pushes. A zero or non-finite volume cannot occur for an element that passed the Jacobian checks of
`integration_points` (positive `det J` at every point). The doc comment of `EnergyError` names ADR 0616. Nothing else changes
(no case-file key, no output field: Q1 of ADR 0478 stands).

### D2. The criterion near one (test helper)

`criterion(prev, next) = |theta_next - 1| <= max(|theta_prev - 1|, 1e-2)`, written in the test file `sa_density.rs` (the
roadmap clause is a property of measurements, not of the library; no library function).

### D3. Tests

`crates/rcfd-fem/tests/sa_density.rs`: the fast checks V1, V2, K1, P3, NC1, F2, E2 and the heavy record E1 (ignored, run once).
The field, case D, the measurement, the plate and `on_hole` are copied from `sa_estimate.rs` (test files share no module in this
crate; the copy is verbatim apart from the measurement returning the `EnergyError` too).

### D4. The bit record (B1)

`crates/rcfd-fem/tests/sa_density_bits.rs`, written and run on the base (after this commit, before any library code), using
only the API of the base: FNV digests of `element_errors`, `estimate`, `solution_norm`, `relative`, `ranked` and the recovered
nodal stress for the two-layer plate and for case D Hex8, Tet10 and Hex20 at `n = 4` (`StaticSolver::Direct`). After the
library change every digest must be equal.

## Checks (limits fixed now)

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| B1 | a2, a5 | the 24 digests of D4 (6 values x 4 models) | equal to the base | equal (arithmetic 1) |
| B2 | a2, a5 | the existing tests `sa_estimate` (fast set), `sa_estimate_default_bits`, `si_hex8x_estimate`, `si_hex8x_bits` | pass | pass |
| E2 | a5 | fast: Hex20 and Tet10 case D at `n = 4, 8` with `SparseDirect` (F-432's solver): `theta` against the F-432 log (`1.33686, 1.02782`; `1.09089, 1.00514`) | within `5e-6` (print rounding) | equal to the digit |
| V1 | a1 | `element_volumes` on Hex8, Hex20 at `n = 4` against `1/64`, Tet10 at `n = 4` against `1/384`, largest relative deviation; plate: `sum |K|` against `t (A B - pi r^2/4)`, relative | `<= 1e-13`; plate `<= 1e-6` | `1e-16`; `1e-10` |
| V2 | a1 | on the four models of B1: `element_densities()[e] == element_errors()[e] / element_volumes()[e].sqrt()` to the bit, every `e`; lengths equal to `n_elements` | holds | holds |
| K1 | a1 | on the four models: `ranked_by_density` is a permutation, its densities non-increasing, equal densities in ascending index | holds | holds |
| P3 | a3 | plate: the 116 elements of largest density contain all 48 hole-edge elements; recorded: the ranks of the 48, the largest density and its element, the density of rank 116 | holds (48 of 48) | undecided (arithmetic 3) |
| NC1 | neg. (a3) | the same plate ranked by the absolute `eta_e` (`ranked`): number of hole-edge elements outside the tenth | `== 30` (the F-432 measurement; a reading registered now: the absolute ranking fails the hole criterion) | 30 |
| F2 | a4 (fast path) | the criterion of D2 on `n = 4 -> 8` from the E2 runs, Hex20 and Tet10 | holds (two) | `0.337 -> 0.028`, `0.091 -> 0.0051` |
| E1 | a4 | heavy record: Hex20 and Tet10 case D at `n = 8, 16, 24`, `SupernodalDirect`: the criterion of D2 on `8 -> 16` and `16 -> 24`; recorded: `theta`, `eta`, the true error, the L2 errors and the time of each level | holds (four) | Hex20 `0.028 -> 0.006 -> 0.007`; Tet10 `0.005 -> 0.008 -> 0.007` (arithmetic 5) |
| E1r | a5 (record) | in E1: `theta` at `n = 8, 16` against the F-432 log | within `1e-5` | `1e-10` |
| NC2 | neg. (a4) | in E1: the literal reading of F-432 (`|theta_next - 1| <= |theta_prev - 1|`, no floor) on Tet10 `8 -> 16`: recorded whether it holds | reading registered now: it fails (as in F-432) | fails |

E1 is the only heavy run; F2 and E2 cover its code path in the fast set (the same `measure`, the same criterion function).

## Planned mutations (on a scratch copy, one at a time, the fast set of `sa_density` and `sa_density_bits`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | density `eta_e/|K|` (no square root) | V2, probably P3 |
| MU2 | volume as the sum of the reference weights (no `det J`) | V1 |
| MU3 | `ranked_by_density` ascending | K1, P3 |
| MU4 | `ranked_by_density` ties to the higher index | expected to survive unless an exact tie of densities occurs on the four models (K1 would catch it then) |
| MU5 | the indicator pushed as `(err / vol * vol).sqrt()` (the volume enters the absolute value; rounding only) | B1 |
| MU6 | the new accumulator touching the old one: `err = (err + vol) - vol` after each point (rounding only) | B1 |
| MU7 | density from `eta_e` of the next element (index shift by one) | V2, P3 |

## Decision rule

- **done:** every check above met (B1, B2, E2, V1, V2, K1, P3, NC1 as registered, F2, E1 with E1r, NC2 recorded), the heavy
  record run once at `n = 24`, the existing tests of `rcfd-fem` that cover `estimate.rs` passing, fmt and clippy clean.
- **partial:** a clause missed or not run (including E1 at `n = 24` not run for memory: then the clause "where feasible" is met
  by `n = 8, 16` only and the status is partial with the reason), the miss kept as a `*_known_miss` test and diagnosed.
- **todo:** no density produced.

## Limits (not covered)

- The density is not a new estimator: its sum of squares weighted by `|K|` is the estimate; it only changes the ranking. No
  refinement or adaptivity is driven by it (ADR 0310).
- The criterion of a4 is weak on its own: an estimator wrong by a constant factor (the `D` for `D^{-1}` mutation of ADR 0478,
  `theta = 2.25, 2.06, 1.97`) satisfies it; it is meaningful together with the band `[0.8, 1.2]` of F-432 (not re-checked here
  beyond E2 and E1r).
- One plate geometry and one grading; the hole criterion on other gradings is not covered.
- `n = 32` not run (arithmetic 4); `n = 24` refines by 1.5.
- No case-file key or output field (ADR 0478 Q1, the maintainer's call).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- **Q1. `|K|` exact or by the rule.** Options: (a) the estimate rule's `sum w det J` (consistent with `eta_e^2`, the density a
  rule mean); (b) a separate higher rule. Taken (a).
- **Q2. The finer level.** Options: (a) `n = 32` (halving, beyond the free memory); (b) `n = 24`; (c) none. Taken (b).
- **Q3. Solver of the heavy record.** Options: (a) `SparseDirect` as F-432 (bits of the F-432 log, about 20 minutes at 16 and
  infeasible at 24); (b) `SupernodalDirect` (rounding-level differences, E1r). Taken (b); the bits of F-432 are kept by B1 and E2.

## Amendment 1 (2026-11-15, written after the registered fast run, before any run of the copies and of the diagnosis record)

The registered fast run (commit c332da2c, `cargo test -p rcfd-fem --release --test sa_density --test sa_density_bits --
--nocapture --test-threads=1`, once, log `f568-scratch/run_fast.log`) met every fast check except **P3**: 44 of the 48
hole-edge elements lie in the 116 of largest density, 4 outside (density ranks 126, 127, 140, 141); the density of rank 116 is
`2.5082e-1`, the smallest at the hole edge `2.3720e-1` (5.4 percent below it); the largest density `6.1322e-1` is a hole-edge
element (192). No limit is changed. `p3_nc1_plate_density_ranking` becomes the ignored record
`p3_nc1_plate_density_ranking_known_miss` (limit intact, numbers in the ignore text); NC1, which was met (30 of 48 outside by
the absolute ranking, as F-432), is continued in `nc1_plate_absolute_ranking`.

Diagnosis record **DP3** (registered now, no limit, record only): for every element in the top 142 by density and for the four
missing hole-edge elements, the polar angle (degrees) and the ring (index of the radial layer, 0 at the hole, from the
element's position in `hex_box` order) of its centroid; the number of elements of each ring among the top 116; the density of
every hole-edge element against its angle. Hypothesis written before the record: the four missing elements are the two layers of
two angular positions near 0 or 45 degrees (where F-432's absolute ranks were lowest, arithmetic 3), outranked by elements of
the second and third rings near 90 degrees.

## Addendum 1 (2026-11-15): results

Runs. The fast set ran once at commit c332da2c (`f568-scratch/run_fast.log`). After amendment 1, the copies and the record DP3
ran once at dd7bf576 (`run_copies.log`, `run_dp3.log`). B1 was recorded at the base (cd19f29c, before any library line) and run
again after the change. The existing tests covering `estimate.rs` are in `run_b2.log` and `run_b2b.log`. **E1 was not run.**
The script `run_e1.ps1` waited for 10 GB of free physical memory, the condition registered in arithmetic 4. On the shared
machine free memory stayed between 3.8 and 8.4 GB. The session then stopped the waiting job for low system memory, before the
test process had started, and its instructions after such a stop are not to restart the job. No heavy number exists.

| Id | Limit | Measured | Met |
|---|---|---|---|
| B1 24 digests of the F-432 values (4 models) | equal to base | 24 of 24 equal | yes |
| B2 `sa_estimate` (fast), `sa_estimate_default_bits`, `si_hex8x_estimate`, `si_hex8x_bits`; also `sa_corot_shell_dyn`, `sa_nl_inertia_reference`, `sa_shell3_paths`, `si_hex8x_linear`, lib tests | pass | pass | yes |
| E2 `theta` against the F-432 log, Hex20 n = 4, 8; Tet10 n = 4, 8 | `<= 5e-6` | `1.3e-6, 4.1e-6; 7.7e-7, 2.0e-6` (print rounding) | yes |
| V1 Hex8, Tet10, Hex20 n = 4 largest volume deviation | `<= 1e-13` | `6.7e-16, 1.0e-15, 1.3e-15` | yes |
| V1 plate sum of volumes against `t (A B - pi r^2/4)` | `<= 1e-6` | `1.51e-10` | yes |
| V2 density equal to `eta_e/|K|^{1/2}` to the bit (4 models) | holds | 0 differ | yes |
| K1 ranking by density a permutation, descending, ties ascending | holds | holds (0 equal neighbours) | yes |
| P3 hole-edge elements in the tenth of largest density | 48 of 48 | **44 of 48** (ranks 126, 127, 140, 141; rank 116 `2.5082e-1`, smallest at the hole `2.3720e-1`) | **no** |
| NC1 absolute ranking: hole-edge elements outside the tenth | `== 30` | 30 | yes (the reading is confirmed: `eta_e` alone fails) |
| F2 criterion `4 -> 8`, Hex20, Tet10 | holds | `0.33686 -> 0.02782`; `0.09089 -> 0.00514` | yes |
| E1 criterion `8 -> 16 -> 24`, Hex20, Tet10 | holds (four) | not run (memory) | not run |
| E1r, NC2 | as registered | not run | not run |
| DP3 (record) | none | below | recorded |

DP3 diagnosis:

- Ranked by density, the hole edge comes first: ranks 0 to 9 are all hole-edge elements.
- Along the hole the density rises from `0.46` at 2 degrees to its maximum `0.613` at 32 degrees. It then falls to `0.257`,
  `0.245` and `0.237` at 77, 80 and 84 degrees, and rises to `0.292` at 88 degrees, in the cell on the symmetry plane.
- The top 116 hold 44 elements of ring 0, 34 of ring 1, 28 of ring 2 and 10 of ring 3.
- The four missing elements are the two layers of the cells at 80 and 84 degrees, 5 percent below the density of rank 116.
  This refutes the hypothesis of amendment 1, which expected 0 or 45 degrees.

Cause, by hand from `plate_map`. The radial size of the first ring is `g(1/24) = 3.39e-3` times the distance from the hole to
the outer boundary along the map line. That distance is `0.045` m at 90 degrees and `0.095` m at 0 degrees, so the hole-edge
elements are about `0.0305 r` long radially near 90 degrees and `0.064 r` near 0 degrees. Arithmetic 3 used the 90-degree size
at every angle. Near 80 to 85 degrees the hole-edge elements are therefore the finest of the mesh. With an `h^2` stress error,
their density falls below that of second- and third-ring elements at 0 to 60 degrees. The 88-degree cell is lifted by the
extrapolation at the symmetry-plane corner. The density itself works as designed; this mesh's grading is what makes the
criterion miss. ADR 0478 remarks that the hole-edge elements near 90 degrees ranked best by the absolute indicator. That remark
was not re-checked, because DP3 did not record the absolute ranks by angle. Here the density is lowest near 90 degrees.

Mutations. They ran on the scratch copy `f568-scratch/mut` at dd7bf576, using the fast set of `sa_density` and
`sa_density_bits` (`mutate.py`, logs `mut_MU*.log`). P3 is ignored in the fast set, so it could not catch any mutation.

| Id | Caught by |
|---|---|
| MU1 `eta_e/|K|` | V2 (every element, 4 models) |
| MU2 volume without `det J` | V1 (plate `1.9e9`, cubes `511`, `63`) |
| MU3 ascending ranking | K1 (4 models) |
| MU4 ties to the higher index | survived, as registered (no exact tie on the four models) |
| MU5 `(err / vol * vol).sqrt()` | B1 (22 of 24 equal) |
| MU6 `err = (err + vol) - vol` | B1 (15 of 24 equal) |
| MU7 density shifted by one element | V2 |

`cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --all-targets -- -D warnings` are clean. No shared solver file
was changed: in `estimate.rs` only `EnergyError`, its accessors and the loop of `energy_error` changed, all additively.
`baseline_hashes` was not run, because no default path is touched (the lead's gate).

**Decision by the rule: partial.**

- a1, a2 and a5 are met (B1, B2, E2, V1, V2, K1).
- a3 was missed by 4 of 48 elements. The cause is the grading along the hole. The miss is kept as
  `p3_nc1_plate_density_ranking_known_miss`.
- a4 was met only on the fast pair `4 -> 8`. The registered heavy record `8 -> 16 -> 24` was not run, for memory.

Remaining questions:

- **Q4. a3 on this plate.** Options: (a) keep the clause, which stays missed on this graded plate; (b) a follow-up on a plate
  whose first ring has one radial size at every angle; (c) a different indicator that also scales by element size (for
  example `eta_e/|K|^{5/6}` for an `h^2` error). Recommendation (b), for the maintainer.
- **Q5. E1.** Run `e1_criterion_near_one_heavy` once, alone, with at least 10 GB free (`f568-scratch/run_e1.ps1`). Arithmetic 5
  expects all four pairs to hold. Recommendation: the lead runs it when the machine is quiet. The test and its limits are
  committed unchanged.
- **Q1 of ADR 0478** (case-file block and output field): unchanged, not added.

## For the lead: roadmap entries

Proposed status and note of F-568:

> Proposed status: partial.

> Structural solver, advanced tier. Follow-up of F-432 (ADR 0478, 2026-11-14). PARTIAL. ADR 0616, 2026-11-15: rcfd_fem::estimate::EnergyError gives element_volumes (|K| by the estimate rule), element_densities (eta_e/|K|^{1/2}) and ranked_by_density beside the absolute values. The 24 digests of the F-432 values (element_errors, estimate, solution_norm, relative, ranked and the recovered nodal stress, on the two-layer plate and on case D Hex8, Tet10 and Hex20 at n = 4) are equal to base, and the F-432 effectivities at n = 4 and 8 are reproduced to the printed digit. Volumes are within 1.3e-15 on the cubes and 1.5e-10 on the plate. On the two-layer plate with a hole, the tenth of largest density holds 44 of the 48 hole-edge elements (MISSED: the cells at 80 and 84 degrees are 5 percent below rank 116, because plate_map makes the first ring half as thick near 90 degrees as near 0 degrees); the absolute ranking holds 18 of 48 (negative control, as in F-432). The criterion abs(theta - 1) <= max(previous, 1e-2) is met from n = 4 to 8 (Hex20 0.337 -> 0.028, Tet10 0.091 -> 0.005). The heavy record at n = 8, 16 and 24 (SupernodalDirect) is committed but NOT RUN: the registered 10 GB of free memory was never available. Six of seven mutations caught (ties to the higher index survived, as registered).

> Follow-up proposed: Hole criterion of the error density on a plate graded uniformly along the hole. Deps F-568. Acceptance: Every limit not stated here is registered in the ADR before the run. A two-layer Hex20 plate with a hole (r/W = 0.05, Kirsch load) whose first ring has one radial size at every angle: the tenth of the elements with the largest eta_e/|K|^{1/2} contains every element with a face on the hole; the recovered peak stress is within 2 percent of 3 sigma_0; the F-432 and F-568 results keep their bits.

> Follow-up proposed: Run the heavy effectivity record of ADR 0616. Deps F-568. Acceptance: Every limit not stated here is registered in the ADR before the run. e1_criterion_near_one_heavy of sa_density.rs is run once, alone, with at least 10 GB free: abs(theta - 1) at n = 16 and n = 24 is at most max(abs(theta - 1) on the previous mesh, 1e-2) for Hex20 and Tet10, theta at n = 8 and 16 is within 1e-5 of the F-432 log, and the time and peak memory are recorded.
