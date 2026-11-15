# ADR 0627: F-695 registration: the hole criterion of the error density on a plate graded uniformly along the hole

Status: **REGISTERED 2026-11-15, written before any code of F-695 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-695 was written in the worktree and no rcfd code was compiled or run
for it. Two Python scripts were written and run on Kirsch's closed form only (no finite-element code of the repository,
disclosed under "Arithmetic"). The numbers of F-568 quoted here are those of its logs (`w49i-work/f568-scratch/run_fast.log`,
`run_dp3.log`) and of ADR 0616 addendum 1. Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Roadmap item F-695 (deps F-568), the follow-up proposed in [ADR 0616](0616-f568-registration-error-density-and-the-effectivity-criterion-near-one.md)
(its remaining question Q4, option (b)). Number 0627 given by the lead. Branch `f695-work` from local main 9cf95fc3, worktree
`w49i-work/f695`, scratch `w49i-work/f695-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or ignored record
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-695 asks

> Every limit not stated here is registered in the ADR before the run. A two-layer Hex20 plate with a hole (r/W = 0.05, Kirsch load) whose first ring has one radial size at every angle: the tenth of the elements with the largest eta_e/|K|^{1/2} contains every element with a face on the hole; the recovered peak stress is within 2 percent of 3 sigma_0; the F-432 and F-568 results keep their bits.

Clauses: **a0** every limit registered here; **a1** a two-layer Hex20 plate with a hole, `r/W = 0.05`, Kirsch load, whose first
ring has one radial size at every angle; **a2** the tenth of largest density contains every element with a face on the hole;
**a3** the recovered peak stress within 2 percent of `3 sigma_0`; **a4** the F-432 and F-568 results keep their bits.

Readings, registered:

- **(a1) the plate.** The plate of ADR 0478 and 0616 (quarter plate, `r = 0.005`, half length `A = 0.1`, half width `B = 0.05`,
  so `W = 2B = 0.1`, `r/W = 0.05`; two layers through the half thickness `t = 0.001`; `E = 2e11`, `nu = 0`; symmetry `u_y = 0`
  on `y = 0`, `u_x = 0` on `x = 0`, `u_z = 0` on `z = 0`; traction `sigma_0 = 50` MPa along `x` on `x = A`; `SparseDirect`;
  `PatchOptions::new()`), meshed with the same 24 x 24 x 2 parameter box, the same 25 straight node lines from `C(q)` on the
  hole to `O(q)` on the outer path and the same angular cells: only the spacing of the nodes along each line changes (D1).
  1152 Hex20, the tenth 116, 48 hole-edge elements. "The first ring has one radial size at every angle": the corner nodes of the
  first ring (`s = 1/24`) lie at `|x| = r + h_1` on every one of the 25 lines, `h_1` the first-ring size of ADR 0478 at 90
  degrees, `h_1 = (B - r) g(1/24) = 1.5219e-4` m `= 0.03044 r` (U1). The midside nodes of the first ring are the map's points at
  `s = 1/48`, on the circle to about 1 percent of `h_1` (not part of the reading).
- **(a2)** as ADR 0616 reading a3: the `ceil(N/10) = 116` elements first in `ranked_by_density()`; every element with a face
  whose nodes all satisfy `|sqrt(x^2 + y^2) - r| <= 1e-9` m, 48 elements.
- **(a3) "the recovered peak stress" and "3 sigma_0".** As ADR 0478 reading a6: the largest recovered nodal `sigma_xx`
  (`Recovery::nodal`) over all nodes. **The 2 percent is against Kirsch's `3 sigma_0` of the infinite plate** (the clause, a
  closed form, `docs/reference-data.md`). The finite width adds about `+0.8` percent at `r/W = 0.05` (D3); this is registered as
  the expected part of the deviation, recorded against the finite-width value (P2r), and the finite-width value itself is checked
  against the plate's own converged finite-element value (FW). The limit of the clause stays against `3 sigma_0`.
- **(a4) "keep their bits".** F-695 is test-only: no file under `crates/*/src` changes (B0, checked by `git diff --stat` from
  the base to the tip). The existing tests that pin the F-432 and F-568 values pass unchanged at the tip: `sa_density` (fast set:
  `v1_v2_k1_volumes_densities_ranking`, `nc1_plate_absolute_ranking`, `e2_f2_tet10_hex20_fast`), `sa_density_bits` (the 24
  digests of the F-432 values of ADR 0616 B1), `sa_estimate_default_bits` (B2). Beside them the negative control NC1 recomputes
  the F-568 plate in the new test and compares its numbers to the F-568 log.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0616 whole (readings, arithmetic 3, amendment 1, addendum 1 with DP3, Q4, the follow-up
  text); ADR 0478 readings a6, a7, arithmetic 5, 6, P1 and the results table; ADR 0449 (the plate, `factors.py`, the
  finite-width polynomial); ADR 0572 and 0573 ("For the lead" style); `docs/reference-data.md` (the Kirsch and finite-width rows).
- `crates/rcfd-fem/tests/sa_density.rs` (the plate, `plate_map`, `hole_edge`, the P3 code), `sa_estimate.rs` (P1 code:
  `recover`, `nodal_average`, the peak), the accessor list of `crates/rcfd-fem/src/estimate.rs`, `recover::nodal_average`'s
  signature, `StaticSolver::SupernodalDirect` in `solve.rs`.
- `w49i-work/f568-scratch/run_dp3.log` (the DP3 record: angle, ring and density of the 142 elements of largest density and of
  the 48 hole-edge elements on the ADR 0616 mesh).

## Arithmetic done before the code (disclosed)

1. **The map (D1).** Lines as `plate_map`: `C(q) = r (cos th, sin th)`, `th = pi q/2`, `O(q)` at arc length `q (A + B)` along
   `(A, 0) -> (A, B) -> (0, B)`, `L(q) = |O - C|`, `u = (O - C)/L`. Their angle to the radial direction is 0 at 0 and 90
   degrees, 3.6 degrees at the corner line (`q = 1/3`) and about 12 degrees at 45 degrees (by hand at `q = 1/3` and `1/2`). A
   point at distance `d` along the line has `|x| = r + h` for `d_1 = -r c + sqrt(r^2 c^2 + 2 r h + h^2)`, `c = e_r . u`
   (the law of cosines). Spacing `d(s, q) = L (e^{a s} - 1)/(e^a - 1)`, with `a(q)` the root of `d(1/24, q) = d_1(q)` (the left
   side decreases in `a`; `d_1/L <= 3.4e-3 < 1/24`, so a positive root exists). At `q = 1`, `c = 1` and `L = B - r`, so `a = 4`
   and the line is that of `plate_map`. Script `f695-scratch/proxy.py` (bisection, 200 halvings in `[1e-6, 20]`): `a` from 4.0
   (90 degrees) to 5.12 (corner line), the growth per ring `e^{a/24}` from 1.181 to 1.238; the first-ring radial size `0.03045 r`
   at all 25 lines in the script; on `plate_map` it is `0.03045 r` to `0.07215 r` (the cause of the F-568 miss, ADR 0616
   addendum 1). The test solves `a(q)` with the same bisection (deterministic; rounding of `d_1` and `d` about `1e-16`
   relative, so U1 is expected near `1e-13` of `h_1`; limit `1e-9`).
2. **Volume (U2).** The boundary is unchanged (the hole as 24 quadratic arcs, straight outer edges), so the sum of the element
   volumes has the F-568 deviation from `t (A B - pi r^2/4)`, measured there `1.51e-10`; limit `1e-6` as ADR 0616 V1.
3. **Two proxies for the ranking, run on Kirsch's closed form and found not predictive.** Kirsch's stress (from memory:
   `sigma_rr = s/2 (1 - a2) + s/2 (1 - 4 a2 + 3 a4) cos 2t`, `sigma_tt = s/2 (1 + a2) - s/2 (1 + 3 a4) cos 2t`,
   `sigma_rt = -s/2 (1 + 2 a2 - 3 a4) sin 2t`, `a2 = r^2/rho^2`) and displacement (plane stress, `nu = 0`, `kappa = 3`, from
   memory) were checked in `proxy2.py`: the strain of the displacement reproduces the stress within `1.1e-9` (central
   differences) and the hole is traction free to `1e-15`. Proxy 1 (`proxy.py`): the density of the error of the P1 L2
   projection of Kirsch's stress per cell. Proxy 2 (`proxy2.py`): the density of the stress error of the 8-node serendipity
   interpolant of Kirsch's displacement (the in-plane Hex20), 4 x 4 Gauss points. Calibration on the ADR 0616 mesh, where the
   answer is known (DP3: hole-edge cells at 80.5 and 84.3 degrees outside, everything else inside): proxy 1 puts every hole-edge
   cell inside (wrong), proxy 2 puts 11 of the 24 hole-edge cells outside (wrong; F-568 measured 2). Neither reproduces F-568, so
   neither is used to predict P3, and no parameter of D1 was chosen with them (the map was fixed by reading a1 before they ran:
   the 90-degree first ring of ADR 0478 at every angle). For the record only: on the new mesh proxy 1 predicts 6 hole-edge cells
   outside (0 to 20 degrees), proxy 2 predicts 14. The proxies leave out what DP3 shows dominates on the measured mesh: the hole
   edge is ranked first (ranks 0 to 31 all hole-edge) because its recovered stress is extrapolated from the first interior ring
   (ADR 0478 D3), an error neither proxy contains.
4. **P3 from the measured F-568 densities (the estimate used).** On the ADR 0616 mesh (DP3) the hole-edge densities run from
   `0.613` (32 degrees) down to `0.237` (84 degrees); the competitors that pushed the 80 and 84 degree cells out are rings 1 to 3
   at 0 to 65 degrees (densities `0.24` to `0.39`). Near 80 to 90 degrees the new mesh is almost the old one (`a` from 4.0 to
   about 4.1 on the last four lines), so the weakest hole-edge cells are expected to keep densities near `0.24` to `0.29`. At 0
   to 65 degrees the new rings are thinner radially by a factor 2 to 2.4 (ring 0: `0.0305 r` against `0.064 r` to `0.072 r`;
   ring `k`: `1.18^k` to `1.24^k` times that), with the angular step `0.0654 rho` unchanged. With a stress error of order `h^2`
   and radial and angular parts of similar size on rings 1 to 3 of the old mesh (radial `0.075 r` against angular `0.068 r` on
   ring 1 at 0 degrees), the density of those rings falls by a factor of about `sqrt(2)/sqrt(1 + 1/16)`, about 1.37, so the 17
   ring-1 cells at 0 to 60 degrees go from `0.27` to `0.39` to about `0.20` to `0.28`, and rings 2 and 3 lower still. The
   criterion needs at most 68 non-hole elements (34 cells) above the weakest hole-edge element. Against: the hole-edge cells at 0
   to 65 degrees become thinner too and lose density; at 0 degrees the old ring 0 (`0.463`) was only 20 percent above ring 1, so
   if the hole-edge extrapolation error falls with the patch size as fast as the interior error, a new hole-edge cell near 0
   degrees may become the weakest one. **Estimate: holds more likely than not; undecided.** The check is run as registered.
5. **Peak (P2).** F-432 measured the recovered peak at `(0, r, t)` `+1.4085e-2` of `3 sigma_0` on the old mesh, the nodal average
   `+1.2340e-2`. The node `(0, r)` and its nearest interior corner nodes lie on the lines at `q = 1` and `q = 23/24`, where the
   new spacing equals the old one at `q = 1` and differs by about 2 percent in size at `q = 23/24`. Estimate `+1.2` to `+1.6`
   percent of `3 sigma_0`, of which about `+0.8` percent is the finite width (D3); limit (the clause) 2 percent.
6. **Finite width (FW).** Polynomial of Peterson's handbook fitted to Howland's results (from memory, already listed in
   `docs/reference-data.md` as not read): `K_tn = 3 - 3.14 (d/W) + 3.667 (d/W)^2 - 1.527 (d/W)^3`, at `d/W = 0.1`
   `K_tn = 2.721143`, gross `K_tg = K_tn/(1 - d/W) = 3.023492`, `+0.783` percent above 3. Heywood's formula, also from memory,
   `K_tn = 2 + (1 - d/W)^3 = 2.729`, gross `3.032222` (`+1.074` percent): the two memories differ by 0.29 percent. Howland's
   strip is infinitely long; this plate is loaded at `20 r` from the hole (`A/W = 1`), a Saint-Venant distance of one width,
   estimated below 0.2 percent at the hole (not derived). Verification registered: the converged finite-element value of this
   geometry, by the nodal average (F-376) at `(0, r)` on single-layer meshes of the same map with 24, 48 and 96 cells per
   direction (`nu = 0`, so the layer count does not change the solution). Unknowns at 96: `97^2 x 2 + 2 x 96 x 97 x 2 + 97^2 =
   65 475` nodes, `196 425` equations, a nearly two-dimensional graph whose top separator is about 300 nodes (900 equations):
   a few seconds of factorisation with `SupernodalDirect`, assembly of 9216 Hex20 dominating; estimated under two minutes but
   registered as an ignored heavy record run once, the 24-level path covered by the fast test (FW24 recorded there). Expected:
   the 24-cell nodal average about `+1.2` percent of `3 sigma_0` (the F-432 number on the old mesh), the mesh part (about `0.4`
   percent at 24) falling by about 4 per halving: `K_96` within about `0.1` percent of the converged value, and that within
   `0.3` percent of `K_tg = 3.023492` if the Howland memory is right. Limit `|K_96/3.023492 - 1| <= 5e-3`.

## Design

### D1. The uniformly graded map (test-local)

`uniform_map(p)` in the new test file: for parameter `(s, q, z)`, the line `C(q) -> O(q)` of `plate_map` (the same `C`, `O`
and the same special case `q == 1`), the distance `d(s, q) = L (e^{a s} - 1)/(e^a - 1)` along it with `a(q)` of arithmetic 1
(bisection, 200 halvings in `[1e-6, 20]`, the midpoint returned), `s == 0` returning `C(q)` and `s == 1` returning `O(q)`
exactly, `z` unchanged. The mesh `upgrade_to_hex20_mapped(hex_box([0; 3], [1, 1, t], [n, n, layers]), uniform_map)` with the
surface set `right` and the model of `plate` in `sa_density.rs` (copied verbatim except for the map and the cell count). `h_1`
is a constant of the map computed as in `plate_map` at 90 degrees: `(B - r) (e^{4/24} - 1)/(e^4 - 1)`. The map is defined for
every `n` with the same `a(q)` (fixed by `s = 1/24`), so the 48- and 96-cell meshes of FW are nested refinements of the 24-cell
one. Prefer test-local (the brief): no library builder is added, so every existing builder keeps its bits trivially.

### D2. The checks in one file

`crates/rcfd-fem/tests/sa_uniform_hole.rs`: U1, U2, P3, NC2, P2, P2r, FW24 in the fast set, NC1 (the F-568 mesh) fast, FW (24,
48, 96) and P2b as an ignored heavy record. The plate, `plate_map`, `hole_edge`, `on_hole` and `Checks` copied verbatim from
`sa_density.rs` (test files share no module in this crate).

### D3. References for the peak

The limit of the clause: `3 sigma_0` (Kirsch, closed form). Recorded beside it (P2r): the deviation against `K_tg sigma_0` with
`K_tg = 3.023492` (Howland via Peterson's polynomial, from memory, arithmetic 6) and against `K_96 sigma_0` (FW). The
finite-width polynomial is not read; FW is its check by the repository's own converged finite-element value on this geometry.

## Checks (limits fixed now)

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| U1 | a1 | the 25 lines x 3 corner levels (`z = 0, t/2, t`): `||x| - r - h_1|/h_1` at the first-ring corner nodes, largest; also the same on the ADR 0616 mesh recorded (largest ratio of first-ring radial sizes) | `<= 1e-9`; record | `1e-13`; ratio 2.37 |
| U2 | a1 | `sum element_volumes` against `t (A B - pi r^2/4)`, relative; element count 1152, hole-edge count 48 | `<= 1e-6`; counts equal | `1.5e-10` |
| P3 | a2 | the 116 elements of largest density contain all 48 hole-edge elements; recorded: their density ranks, the density of rank 116, the smallest hole-edge density, and per hole-edge cell its angle and density, the count per ring in the top 116 | holds (48 of 48) | holds more likely than not; undecided (arithmetic 4) |
| NC1 | neg. (a2), a4 | the ADR 0616 mesh (`plate_map`) in this file: hole-edge elements outside the tenth by density; their ranks; density of rank 116 | `== 4`; ranks `[126, 127, 140, 141]`; `2.5082e-1` to the printed digit (`5e-5`) | as F-568 |
| NC2 | neg. (a2) | the new mesh ranked by the absolute `eta_e` (`ranked`): hole-edge elements outside the tenth | `>= 1` (reading registered now: the absolute indicator fails the criterion on this mesh too) | 30 or more (the hole-edge elements are smaller than on the old mesh at 0 to 65 degrees) |
| P2 | a3 | the recovered peak nodal `sigma_xx` against `3 sigma_0`, relative (absolute value) | `<= 2e-2` | `+1.2` to `+1.6` percent |
| P2r | a3 (record) | the same peak against `K_tg sigma_0 = 3.023492 sigma_0`; its node; the nodal average (F-376) peak against `3 sigma_0` | record | `+0.4` to `+0.8` percent |
| FW24 | a3 (fast path of FW) | single-layer 24-cell mesh of D1: nodal average `sigma_xx` at the node `(0, r, 0)` over `sigma_0` | record | about `3.036` |
| FW | a3 (verification) | heavy: the same at 48 and 96 cells (`SupernodalDirect`); `K_96` against `K_tg = 3.023492`; recorded: observed order `log2((K_24 - K_48)/(K_48 - K_96))` and the Richardson value | `|K_96/3.023492 - 1| <= 5e-3` | within `3e-3` |
| P2b | a3 (heavy) | the recovered peak of P2 against `K_96 sigma_0` | `<= 1e-2` | `+0.3` to `+0.9` percent |
| B0 | a4 | files under `crates/*/src` changed from 9cf95fc3 to the tip | none | none |
| B2 | a4 | `sa_density` (fast set), `sa_density_bits`, `sa_estimate_default_bits` at the tip | pass | pass |

## Planned mutations (on a scratch copy, one at a time, the fast set of `sa_uniform_hole`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `a(q) = 4` on every line (the `plate_map` spacing) | U1 (ratio 2.37); P3 then expected to miss as NC1 |
| MU2 | `d_1 = h_1` along the line (no law-of-cosines correction) | U1 (about 2 percent near 45 degrees) |
| MU3 | library, scratch copy: `ranked_by_density` ascending | P3 |
| MU4 | library, scratch copy: density `eta_e/|K|` (no square root) | undecided (a stronger weight on small elements may still hold P3); recorded |
| MU5 | test: the tenth `n/10` (115) instead of `ceil(n/10)` | expected to survive unless a hole-edge element has density rank 115 |
| MU6 | test: the peak from `sigma_yy` (component 1) instead of `sigma_xx` | P2 |
| MU7 | test: the traction `sigma_0/2` in the model, the limit unchanged | P2 |

## Decision rule

- **done:** U1, U2, P3, NC1, NC2, P2, FW, P2b, B0, B2 met; fmt and clippy clean.
- **partial:** P3 or P2 missed (kept as `*_known_miss` with the limit and the measured numbers, diagnosed), or FW / P2b not
  run or missed (then a3 is met against `3 sigma_0` but its finite-width verification is not).
- **todo:** the uniform mesh not built.

## Limits (not covered)

- One plate, one first-ring size (`0.0304 r`, the 90-degree size of ADR 0478) and 24 x 24 cells; another size or cell count is
  not covered. Other kinds (Tet10, Hex8) not covered.
- The density's ranking is a property of this estimator on this mesh; no adaptive loop is driven by it (ADR 0310).
- The finite-width factor is from memory (not read); FW checks it only on this plate, whose finite length (`A/W = 1`) is not
  Howland's infinite strip.
- No library builder is added (test-local map); a reusable builder would be a separate item.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- **Q1. Builder or test-local map.** Options: (a) a test-local map (no library change, all bits kept trivially); (b) a library
  builder `plate_with_hole_uniform`. Taken (a); the brief prefers it and no other user exists.
- **Q2. Which first-ring size.** Options: (a) the 90-degree size of ADR 0478 (`h_1 = 0.0304 r`, the mesh near 90 degrees
  unchanged, the peak region the same as F-432); (b) the 0-degree size (`0.064 r`); (c) the mean. Taken (a): it changes the old
  mesh least where the peak and the F-568 misses were, so the comparison isolates the grading along the hole.
- **Q3. Which reference the 2 percent is against.** Options: (a) Kirsch's `3 sigma_0` (the clause literally), the finite width
  recorded and verified beside it; (b) `K_tg sigma_0` from the unread polynomial. Taken (a).

## Amendment 1 (2026-11-15, written after the registered fast run, before any run of the copies, of the records and of the mutations)

The registered fast run (commit cd92e896, `cargo test -p rcfd-fem --release --test sa_uniform_hole -- --nocapture
--test-threads=1`, once, log `f695-scratch/run_fast.log`) met U1 (`7.5e-15`), U2 (`1.51e-10`, 1152 and 48), NC1 (4 outside,
ranks 126, 127, 140, 141, rank-116 density `3.8e-6` from the F-568 digit), NC2 (48 of 48 outside by `eta_e`), P2 (`+1.4061e-2`
of `3 sigma_0`) and FW24 (`3.037015`). **P3 missed: 20 of the 48 hole-edge elements lie outside the 116 of largest density**
(density ranks 117 to 365; the density of rank 116 `2.5521e-1`, the smallest at the hole edge `1.7055e-1` at 5.6 degrees). The
hole-edge density is `0.376` at 35 to 39 degrees, falls to `0.17` to `0.22` at 2 to 13 degrees and to `0.23` to `0.245` at 69
to 84 degrees, and is `0.292` at 88 degrees; the top 116 hold 28, 22, 22, 22, 20 and 2 elements of rings 0 to 5. No limit is
changed.

Changes before any further run:

- `u1_u2_p3_nc2_p2_uniform_plate` becomes the ignored record `u1_u2_p3_nc2_p2_uniform_plate_known_miss` (body unchanged, the
  numbers in the ignore text). U1, U2, NC2, P2 and P2r, which were met, are continued unchanged in `u1_u2_nc2_p2_uniform_plate`.
- Diagnosis record **DQ1** (registered now, no limit, record only, ignored, run once): on the uniform plate, layer 0, the density
  of rings 0 to 5 at each of the 24 angular cells, and for each hole-edge cell outside the tenth the number of elements of each
  ring that outrank it. Hypothesis written before the record: along each line the density of rings 0 to 4 is nearly constant
  (within about 20 percent), because the graded mesh roughly equidistributes `h^2 sigma''` near the hole; the ranking is then
  decided by the angle, and the hole-edge cells at 0 to 15 and 65 to 85 degrees are outranked by rings 1 to 4 at 20 to 60
  degrees.
- The heavy record FW / P2b is run once as registered.
- Mutations are run on the fast set as registered; since P3 is now ignored, the known-miss record is also run under each
  mutation (`--include-ignored` for that test only) and its count of hole-edge elements outside is recorded, not judged.
