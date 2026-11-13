# ADR 0407: F-500 registration: rotating modes of a cyclic sector at a harmonic index (nodal-diameter families of K_eff(Omega), loss of stiffness per index, the Campbell table by family)

Status: **REGISTERED 2026-11-13, written before any code of F-500 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-500 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done by hand (no script; it does not call rcfd). Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Parents: [ADR 0403](0403-f396-registration-rotating-part-modes-and-campbell-table.md) (F-396: `K_eff = K + Omega^2 (K_sigma,1 -
M_perp)`, the loss of stiffness by inertia, tracking by the mass-weighted modal assurance, engine-order crossings, the Campbell
table), [ADR 0386](0386-f394-registration-cyclic-symmetry-sector-statics-and-harmonic-modes.md) (F-394: the cyclic boundary, the
real cos/sin pair of harmonic index `k`, `modal_analysis_harmonic`, `full_rotor`, `FullRotor::expand`) and [ADR
0395](0395-f489-registration-cyclic-harmonic-loads-and-rigid-sets.md) (F-489: harmonic loads, rigid sets at `k > 0`). Roadmap item
F-500 (deps F-396, F-394), from ADR 0403 question 4. Number 0407 given by the lead. Branch `f500-rotating-cyclic-sector`,
worktree `w49i-work/f500`, scratch `w49i-work/f500-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room. Library edits, additive
only: new items appended to `crates/rcfd-fem/src/rotating.rs`; in `crates/rcfd-fem/src/cyclic.rs` two private functions
(`doubled_matrix`, `unknown_label`) become `pub(crate)` with their bodies unchanged; `mpc.rs` is used read only. No existing
function changes its behaviour, so every existing result is bit-identical by construction. Tests: a new file
`crates/rcfd-fem/tests/si_rotating_cyclic.rs`. No existing test asserts a refusal that this package retires (`si_rotating.rs` has
no cyclic refusal; `si_cyclic*.rs` refuse nothing about rotation), so no existing test is edited.

## What F-500 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. K_eff(Omega) of F-396 on the cyclic harmonic pair of
> ADR 0386/0395 with the pre-stress of the index-0 sector: at each harmonic index h and speed the frequencies of the sector equal
> those of the full rotor mesh for that nodal-diameter family within 1e-8 relative; at Omega = 0 the bits of the cyclic modes of
> F-394; a loss of stiffness is reported per index and speed; the Campbell table holds one family of tracks per index, with the
> crossings with the engine orders.

Clauses: (b1) the sector at every index `h` and speed equals the full rotor for that family within 1e-8 relative; (b2) at `Omega
= 0` the bits of `cyclic::modal_analysis_harmonic` (the cyclic modes of F-394); (b3) a loss of stiffness reported per index and
speed; (b4) the Campbell table: one family of tracks per index, with the engine-order crossings.

## What was read (branch base 23e00f8a, read only)

- `AGENT_RULES.md`; the brief; ADR 0403 (all, including addendum 1 and its question 4); ADR 0386 (D1 headings, the E checks);
  ADR 0395 (head).
- `crates/rcfd-fem/src/rotating.rs` (all), `cyclic.rs` (all: `boundary`, `CyclicBoundary`, `SectorMode`, `HarmonicModes`,
  `doubled_matrix`, `modal_analysis_harmonic`, `FullRotor::{expand, expand_field, expand_load}`, `unknown_label`), `mpc.rs`
  (signatures and heads: `uses_transformation`, `transformation`, `harmonic_transformation`, `Transformation::{reduce, expand,
  n_unknowns, n_mesh, independent}`, `add_supports`, `zero_shift_refused`, `unknown_name`, `refuse_nonzero_values`,
  `modal_analysis_mpc`), `geometric.rs` (`Prestress`, `element_geometric_stiffness` lines 31 to 80, `prestressed_modes` head),
  `modal.rs` (the dispatch to `modal_analysis_mpc`), `solve.rs` (the dispatch to `solve_static_mpc`), `rcfd-linalg/src/eigen.rs`
  (`count_below` signature).
- Tests: `si_cyclic_harmonic.rs` (lines 1 to 180 and 353 to 440: the disc sector D, `full_of`, the modal equivalence H5/H6),
  `si_cyclic.rs` (S1/S2, the spinning disc on the sector), `si_scaffold.rs` (the cyclic and rotating mentions).

## Design

Notation as ADR 0403: speed `Omega` about the unit axis `a` through `p`, `P = I - a a^T`; a rotor of `N` sectors, `alpha = 2 pi /
N`, `R` the rotation by `alpha` about `a`; harmonic index `k`, `theta = k alpha`.

### D1. Why the sector operator at index `k` is `K_eff` of the sector with the index-0 pre-stress (first principles)

The full rotor's matrices are sums over the `N` copies of the sector. Write `Q` for the map that moves a displacement field by one
sector: `(Q u)(R x) = R u(x)`. A full-rotor matrix `A` commutes with `Q` (`Q^T A Q = A`) when the element matrices of copy `j`
are those of copy 0 rotated, `A_e^(j) = (I_n (x) R^j) A_e^(0) (I_n (x) R^j)^T`, block by node pair. For such an `A` the
decomposition of ADR 0386 D1 (the eigenvectors of `Q` are the harmonic fields `u_j = R^j Re((a + i b) e^{i j theta})`) applies
with the **sector's own matrix**: the problem of index `k` is the sector matrix under the rows `a_H = R (cos theta a_L - sin theta
b_L)`, `b_H = R (sin theta a_L + cos theta b_L)`, and, because the sector matrix is real and symmetric, the real form is the
sector matrix doubled block-diagonally, `blockdiag(A_s, A_s)`, reduced by the transformation `T_k` of those rows
(`mpc::harmonic_transformation`); `k = 0` and `2k = N` are single (`mpc::transformation`, `a_H = -R a_L`). ADR 0386 used only
these properties of `K` and `M`; so it suffices to show that each of the three new terms has them.

1. **`M_perp`.** Its node block is `m_ab P` (ADR 0403 D1), `m_ab` the scalar consistent mass, which depends on the element's
   shape functions and Jacobian determinant only and so is the same on a rotated copy. `P` commutes with every rotation about `a`:
   `R P R^T = R R^T - (R a)(R a)^T = I - a a^T = P`, since `R a = a`. Hence `m_ab P = R^j (m_ab P) R^{jT}`: the full `M_perp` is
   the sum of rotated copies of the sector's, and at index `k` it enters as `blockdiag(M_perp,s, M_perp,s)` reduced by `T_k`. It
   does not couple the cosine and sine parts: the real pair sees `M_perp` exactly as it sees `M`. (The Coriolis matrix `G = 2
   Omega int rho N_a N_b [a x]`, skew, would couple them, `[[0, -G],[G, 0]]`-like after realification; it is neglected, as in ADR
   0403 D6.)
2. **The pre-stress.** The centrifugal load `rho Omega^2 P (x - p)` repeats in every sector (`b(R x) = R b(x)`), i.e. it is a
   load of index 0. The full rotor's static solution is unique (supports that hold the rotor, as in every set-up here), and `Q u*`
   solves the same problem as `u*` (the problem is `Q`-invariant), so `Q u* = u*`: the solution is of index 0 and is the sector
   solution under the rows `u_H = R u_L` (ADR 0386: `k = 0` statics through `mpc::transformation`; checked against the full disc in
   ADR 0386 S2 to 1e-8). Its stress satisfies `sigma(R x) = R sigma(x) R^T`. The geometric stiffness of an element (ADR 0390 D1,
   `geometric.rs` lines 56 to 80) is `G_ab I_3`, `G_ab = int g_a^T S g_b`; on copy `j` the gradients are `R^j g` and the stress
   `R^j S R^{jT}`, so `G_ab` is unchanged and `G_ab I_3 = R^j (G_ab I_3) R^{jT}`. Hence `K_sigma` of the full rotor is the sum of
   rotated copies of the sector's `K_sigma,1` from the **index-0** solve, and this one matrix serves every index `k`: the
   pre-stress is axisymmetric in the cyclic sense (one sector repeated), while the vibration has index `k`; the second variation
   is linear in the vibration and has the pre-stress as a coefficient, so no coupling of indices arises. (A pre-stress of index
   `k' != 0`, for example of a non-repeating load, would couple index `k` with `k +/- k'`; not this package.)
3. **`K`, `M`** as ADR 0386, with the supports added to `K` (ADR 0398; `RotatingPart` already does this on the transformation
   path).

So, per index `k` and speed: `T_k^T blockdiag(K_eff,s(Omega), K_eff,s(Omega)) T_k q = omega^2 T_k^T blockdiag(M_s, M_s) T_k q`
(single blocks for `k = 0` and `2k = N`), with `K_eff,s = K_s + Omega^2 (s K_sigma,1 - c M_perp,s)` formed value by value on the
sector's stiffness pattern exactly as `RotatingPart::operator` does (it is that function).

**Every eigenvalue of the full rotor is an eigenvalue of one index**, with multiplicity 2 for `0 < 2k < N` (the indices `k` and
`N - k` give one real pair), and the family's mode on the full mesh is `FullRotor::expand` of the sector mode, M-orthonormal.

### D2. Loss of stiffness per index (b3)

As ADR 0403 D3, per index: the reduced `K_eff` of index `k` is tested by `count_below(K_r, M_r, 0, Rcm)` before the eigensolution
at `Omega > 0`; a positive count or a zero pivot is reported as a loss of stiffness of that index at that speed, with the
negative count of the real problem (for a pair, an even number: every eigenvalue twice). **Inertia identity**: the expansion map
`q_0, ..., q_{N/2} -> x` of D1 is an invertible linear map from the direct sum of the index problems to the full admissible
space, and in that basis the full `K_eff` is block-diagonal (a congruence), so by Sylvester's law the full rotor's negative count
equals the sum over `k = 0 .. N/2` of the index counts (pairs counted with their doubling). This is checked exactly (L2).

### D3. Interfaces (appended to `rotating.rs`)

- `RotatingSector<'a>`: `RotatingSector::new(model, &options)` takes the axis and origin **from the model's one cyclic
  constraint** (the spin axis must be the cyclic axis for D1; there is no separate input to get wrong) and builds the
  `RotatingPart` of the sector with them (one static solve of the unit centrifugal load with the sector's constraints, i.e. the
  index-0 rows; ADR 0403 D5); `RotatingSector::with_unit_prestress(model, prestress, &options)` likewise with a caller's pre-stress
  at 1 rad/s. Accessors `part()` (the `RotatingPart`: `operator`, `slope`, `unit_prestress`, the matrices), `sectors()`, `axis()`,
  `origin()`.
- `RotatingSector::modes(harmonic, omega) -> HarmonicSpeedModes`: `Modes(HarmonicModes)` (the type of F-394, with
  `reduced_stiffness` the reduced `K_eff`) or `LossOfStiffness(HarmonicLoss { harmonic, multiplicity, loss: LossOfStiffness })`.
  At `omega == 0` it returns `cyclic::modal_analysis_harmonic(model, &[], harmonic, &options.modal)` itself (b2, by construction,
  as ADR 0403 D1). At `omega > 0`: `T_k` (`mpc::transformation` for `k = 0`, `mpc::harmonic_transformation` otherwise),
  `K_eff,s` from `RotatingPart::operator`, doubled for a pair, reduced; the modes and density checks of
  `modal_analysis_harmonic`; D2; then `lowest_eigenpairs` with `modes x multiplicity` eigenvalues at the shift `Shift::Value(0)`
  unless the options give one (ADR 0403 D2); the result packed as `modal_analysis_harmonic` packs it (pairs merged by their
  mean, the split recorded). Refusals: harmonic above `N/2` (`"harmonic"`), negative or non-finite speed (`"speed"`).
- `RotatingSector::campbell(harmonics, speeds) -> CyclicCampbell`: `speeds` as ADR 0403 (finite, `>= 0`, strictly increasing;
  key `"speeds"`); `harmonics` non-empty, strictly increasing, each at most `N/2` (key `"harmonics"`). One `HarmonicCampbell {
  harmonic, multiplicity, table: CampbellTable }` per index, the table of ADR 0403 (tracks, losses, `crossings`, `to_csv`,
  `to_json`) built from that index's solves.
- **Tracking of a pair** (the one new idea of the table). A doubled index returns each sector frequency as a degenerate pair
  whose basis inside the two-dimensional eigenspace is arbitrary (any rotation of the cos/sin pair, the standing-wave phase), so
  the vector MAC of ADR 0403 between two speeds is not defined by the physics. The track unit is therefore the **group** of a
  sector frequency (one vector, or the pair), and the measure is the subspace assurance `MAC(V, W) = tr(G_V^{-1} C G_W^{-1}
  C^T)/d`, `C = V^T M_2 W`, `G_V = V^T M_2 V`, `G_W = W^T M_2 W`, `d` the group size, `M_2 = blockdiag(M_s, M_s)` over the
  concatenated `(a, b)`: the mean squared cosine of the principal angles between the two eigenspaces (Bjorck and Golub 1973, from
  memory; derived here: for orthonormal bases it is `|C|_F^2/d`, invariant under a change of basis in either group). For `d = 1`
  it is the mass-weighted MAC of ADR 0403 exactly. The greedy assignment, tie rules, storage of the MAC and continuation after a
  loss are those of ADR 0403 D4; the track stores the group's first vector `(a, b)` concatenated, its eigenvalue is the pair's mean
  (`HarmonicModes::values`).
- **Engine orders on a cyclic rotor**: crossings are those of ADR 0403 D4 (linear in `Omega^2`) on each family's tracks.
  `engine_order_excites(order, harmonic, sectors)` is true when `order mod N` is `k` or `N - k`: a stationary force pattern of
  engine order `E` seen in the rotating frame has sector phase `e^{i E j alpha}`, whose projection on index `k` is `sum_j e^{i (E -
  k) j alpha}`, `N` when `E = k (mod N)` and 0 otherwise (and `-k` from the conjugate of the real pair); the zig-zag of the
  interference (SAFE) diagram (Singh, Thompson and others, from memory; derived here). `CyclicCampbell::crossings(order)` lists
  every family's crossings with the order and that flag; `to_csv()` (a `harmonic` column, then ADR 0403's columns, padded to the
  largest track count) and `to_json(orders)` (the families with their tables, the flagged crossings). Numbers in Rust's shortest
  round-trip form.

### D4. What stays as it was

`RotatingPart`, `CampbellTable::track`, `modal_analysis_harmonic`, the transformations and every existing function keep their
code; the two `cyclic.rs` helpers change visibility only. No default changes; nothing calls the new items unless asked.

## Set-up (fixed now)

**D, the disc sector of ADR 0395** (`si_cyclic_harmonic.rs` `d_mesh`, copied): `hex_box` from `(0.25, 0, -0.1)` lengths `(0.75,
alpha, 0.2)` divisions `[3, 2, 1]`, upgraded to Hex20 with the polar map `(r, phi, z) -> (r cos phi, r sin phi, z)`; `N = 12`,
`alpha = 30` degrees; `E = 2.0e11` Pa, `nu = 0.3`, `rho = 7850`; `xmin` (the hub face `r = 0.25`) clamped; `Constraint::cyclic(
"ymin", "ymax", z, origin, 12)`; no rigid set. Its full rotor is `full_rotor` with `xmin` clamped (`full_of(sector, None)` of the
same file). Axis `z` through the origin, taken from the constraint.

Options: sector and full `equivalence_options` of ADR 0395 (`tolerance 1e-9`, `max_iterations 400`); 4 sector frequencies per
index; `RotatingOptions::new(4).modal(that)` with both effects on unless a check says otherwise.

Derived speeds (rules fixed now; their values are computed by the run from the rest spectrum and printed, not chosen):

- `omega_ref = sqrt(min_k values_k[0])` at rest over `k = 0 .. 6`: the lowest sector frequency, rad/s.
- Comparison speeds `Omega in {0, 0.5, 0.9} omega_ref`. Bound for no loss there: if the centrifugal stress is tensile
  (`K_sigma,1` positive semidefinite), `K_eff >= K - Omega^2 M_perp >= K - Omega^2 M >= (omega_ref^2 - Omega^2) M > 0` for `Omega <
  omega_ref` (`M_perp <= M` since `P <= I`). A local compressive stress at the clamp could break this; a loss at a comparison
  speed is a defective set-up (asserted, then amended before any corrected run).
- **Loss speed** `Omega_L`: for every rest mode `(k, i)` (M-orthonormal, cos and sin parts) the softening excess `d_ki = phi^T
  (M_perp - K_sigma,1) phi` (both blocks); `Omega_L^2 = 2 min over d_ki > 0 of lambda_ki / d_ki`, `k*` the index attaining it. Then
  the Rayleigh quotient of that mode at `Omega_L` is `lambda - 2 lambda = -lambda < 0`, so by the min-max principle index `k*` must
  lose stiffness at `Omega_L`: a guaranteed loss, not a searched one. If no `d_ki > 0` (every rest mode stiffened more than
  softened), the loss checks run with `stress_stiffening = false` (then `d_ki = phi^T M_perp phi > 0` for any mode with in-plane
  motion), recorded as such. `Omega_L > 0.9 omega_ref` is asserted (it holds when `K_sigma,1 >= 0`, since then `d <= 1` and
  `Omega_L^2 >= 2 lambda >= 2 omega_ref^2`).
- Campbell speeds: `0, 0.1, ..., 0.9 omega_ref` (ten) and `Omega_L` (eleven), every index `0 .. 6`; engine orders `1 .. 2N = 24`.

## Arithmetic (done before the code; disclosed)

1. Sizes: 6 Hex20 elements (3 in `r`, 2 in `phi`, 1 in `z`); nodes: 24 corners and `18 + 16 + 12` edge nodes, 70 nodes, 210
   equations; the high face `phi = alpha` has 18 nodes, so the full rotor has `12 x (70 - 18) = 624` nodes, 1872 equations and
   72 elements: each full solve takes seconds in release, the 77 Campbell solves and the crossing solves of the sector are small.
2. Count for (b1): 4 frequencies per index give 4 eigenvalues at `k = 0` and `k = 6` and 8 at `k = 1 .. 5`, 48 real eigenvalues;
   below the cut-off `0.999 min_k max values_k` (ADR 0395 H5) somewhat fewer; the full rotor is solved with that count plus 8
   (as H5).
3. Accuracy of (b1): ADR 0395 H5/H6 met 1e-8 on this disc at rest; at speed the only new difference is `K_sigma,1` of the sector's
   index-0 solve against the full rotor's own static solve (elimination path), equal to rounding (ADR 0386 S2 measured 1e-8 on the
   displacement with a pinned full disc; here the full disc is clamped, unique, so the difference is the solvers' rounding,
   `~1e-13`): predicted `<= 1e-10`, limit 1e-8 of the clause.
4. The family check (C2) is the Rayleigh quotient of the expanded sector mode on the full rotor's own `K_eff` and `M`: it equals
   the sector Ritz value up to the pre-stress difference (`~1e-13`) and the eigenvector error squared (`(1e-9)` scale): predicted
   `<= 1e-10`, limit 1e-8.
5. Excitation projections (P4): `|sum_{j<12} e^{i (E - k) j alpha}|` is 12 or `<= 1e-13` (rounding of 12 terms).
6. Crossings (P3): over a step of `0.1 omega_ref` the slope `d omega^2/d Omega^2` changes by second-order terms (ADR 0403 arithmetic
   5 found `< 1e-3` relative misplacement on the beam at the same step relative to its frequency); limit the 1 percent of ADR 0403
   A7.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| C1 | b1 | at each comparison speed: the sector eigenvalues of all indices `0 .. 6` below the cut-off, sorted, against the full rotor's (`RotatingPart::new` on the full mesh), count equal and `max |sqrt(l_s/l_f) - 1|` | count equal, `<= 1e-8` | `<= 1e-10` (arithmetic 3) |
| C1b | b1 | the pair split `|l_2i+1 - l_2i|/mean` of every doubled index at every comparison speed | `<= 1e-8` | `~1e-12` (H5 measured on this disc) |
| C2 | b1 (family) | at each comparison speed, each index, each sector mode: `|RQ_full(expand_k(phi))/l - 1|` with the full rotor's `K_eff(Omega)` and `M` | `<= 1e-8` | `<= 1e-10` (arithmetic 4) |
| C3 | b2 | `modes(k, 0.0)` against `modal_analysis_harmonic(sector, &[], k, opts)`, `k = 0 .. 6`: eigenvalues, values, frequencies, splits, every mode value, residuals by `to_bits`, iterations, shift, unknowns equal | identical | by construction (D3) |
| C3b | b2 | the speed-0 column of every family's Campbell table: the `values` of `modal_analysis_harmonic` by `to_bits` | identical | by construction |
| C3c | b2 (continuity) | `modes(k, 1e-6 omega_ref)` against rest, every index and value: `|l/l_0 - 1|` | `<= 1e-8` | `1e-12 x slope` plus eigensolver `~1e-12` |
| C3d | D3 | index 0 at `0.5 omega_ref`: the eigenvalues against `RotatingPart::modes` of the same sector (ADR 0403's path, the same operator and transformation) by `to_bits` | identical | the same arithmetic in the same order |
| L1 | b3 | at `Omega_L`: index `k*` returns `LossOfStiffness` with `harmonic == k*`, `speed` the bits of `Omega_L`, `negative >= 1` (`>= 2` and even for a pair) | each | min-max (set-up) |
| L2 | b3 | at `Omega_L`: the full rotor's negative count (`RotatingPart::modes` on the full mesh, with the options of the loss checks) equals the sum over `k = 0 .. 6` of the index counts (0 for an index with modes), each pair count even | exact | Sylvester (D2) |
| L3 | b3 | at `0.9 omega_ref`, no index and not the full rotor reports a loss | each | the bound under "Derived speeds" |
| P1 | b4 | the Campbell table of indices `0 .. 6` over the eleven speeds: 7 families in index order with their multiplicities (1 at 0 and 6, 2 otherwise), each with 4 tracks; the loss of `k*` at `Omega_L` listed in its family with the frequencies `None` there; every stored number finite | each | D3 |
| P2 | b4 | every stored track MAC (subspace assurance, D3) at the speeds `0.1 .. 0.9 omega_ref` | `>= 0.9` | `~1` (shapes change by `O(Omega^2)` over a step; veering inside a family not expected on this disc but not excluded) |
| P3 | b4 | every family has at least one crossing with an order `1 .. 24` below `0.9 omega_ref`; each such crossing `(k, track, E, Omega_c)`: the sector value of index `k` at `Omega_c` (a direct `modes(k, Omega_c)`) nearest to `(E Omega_c)^2`, `|sqrt(l)/(E Omega_c) - 1|` | at least one per family; each `<= 1e-2` | `< 1e-3` (arithmetic 6) |
| P4 | b4 | `engine_order_excites(E, k, 12)` for `E = 0 .. 36`, `k = 0 .. 6` against the projection `|sum_j e^{i (E - k) j alpha}| + |sum_j e^{i (E + k) j alpha}| > 6` computed in the test | all equal | arithmetic 5 |
| P5 | b4 | `CyclicCampbell::to_csv`: 1 + 7 x 11 rows, the harmonic column, every frequency parses back to the table's bits; `to_json(1..=24)`: 7 families, no `NaN` or `inf`, `null` at the loss | each | shortest round-trip |
| R1 | D3 | refusals: `RotatingSector::new` on D without its cyclic constraint (no cyclic constraint); `modes(7, .)` (`"harmonic"`); `modes(k, -1)` and `modes(k, NaN)` (`"speed"`); `campbell` with a decreasing speed list (`"speeds"`), an empty, a decreasing and an above-`N/2` index list (`"harmonics"`); 0 modes (`"modes"`) | each refused with its key | D3 |
| R2 | rule 3 | the existing tests `si_rotating`, `si_cyclic`, `si_cyclic_harmonic` pass unchanged (release); `cargo check -p rcfd-post --tests` | all pass | no behaviour changed |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | C1 at `0.9 omega_ref` with the sector's `stress_stiffening = false` against the full rotor with it on | the C1 deviation `> 1e-3`: C1 sees the pre-stress at index `k` |
| N2 | C1 at `0.9 omega_ref` with the sector at rest against the full rotor at speed | `> 1e-3`: the speed effect is large against the limit |
| N3 | P2 with the vector MAC of the pair's first vectors (the ADR 0403 measure on an arbitrary basis) over the same table | reported; expected below 0.9 at some step of some doubled family, not asserted (the eigensolver may return a consistent basis by chance) |

Planned mutations (on a `git archive` copy in `f500-scratch`, each run once against `si_rotating_cyclic`; what catches each is
recorded):

1. The sine block without `M_perp` (`blockdiag(K_eff, K + Omega^2 s K_sigma)`): expected C1b, C1, C2.
2. `K_sigma,1` left out at `k > 0`: C1, C2.
3. The loss count divided by the multiplicity: L1 (even), L2.
4. The zero-speed shortcut removed (the general path at `Omega = 0`): C3 (the shift differs: `Auto` against `Value(0)`, so the
   bits are expected to differ; recorded either way).
5. The subspace assurance replaced by the vector MAC of the first vectors: P2 (may survive, as N3).
6. `engine_order_excites` with `order mod N == k` only: P4.
7. `harmonic_transformation(model, &[], k + 1)` for `0 < k < N/2 - 1` (a wrong family): C1, C2.

## Decision rule

- **done**: C1 to C3d, L1 to L3, P1 to P5, R1 and R2 met and N1, N2 as expected.
- **partial**: C1, C2, C3, L1, L2 met with one other check missed and its cause diagnosed (a `*_known_miss` record), or a clause
  of the acceptance met only in a registered reading.
- **todo**: C1, C2, C3 or L2 missed, or a loss of stiffness gives a non-finite number.

## Limits (what is not covered)

Coriolis coupling (it couples the cos and sin parts and makes the pair problem non-symmetric; F-499); a pre-stress of an index
other than 0 (a non-repeating load); mistuning (sectors not identical); rigid sets at `k > 0` with speed are handled by the same
transformation as ADR 0395 but not checked here (no rigid set in D); one disc geometry, solid elements only; the full rotor check
on a mesh of 72 Hex20 elements only (the 120 mm fan's sector is a separate item); output through the case files and the CLI.

## Amendment 1 (2026-11-13, written after the registered run of `si_rotating_cyclic` and before any corrected run)

**What happened.** The registered run (release, `--test-threads=2`, 10.1 s for six tests; log `f500-scratch/run1.log`) met
every check except **P2**: the smallest stored track assurance at `0.1 .. 0.9 omega_ref` was **0.8952** against the limit 0.9.
The test became the ignored record `p1_p2_p3_p5_campbell_by_family_known_miss` (body unchanged, name and reason only). The loss
speed rule took its registered fallback (no rest mode with `d_ki > 0` under both effects), so that test's Campbell table was
built with `stress_stiffening = false`; P1, P3, P5 and C3b were met on it.

**Diagnosis** (one diagnostic run of a throw-away test, `f500-scratch/diag_f500.rs`, log `f500-scratch/diag_p2.log`, printing each
track's assurance; it re-ran nothing to choose a result). The low values are all in the family `k = N/2 = 6`, tracks (0, 1) and
(2, 3), and nowhere else (every other family `>= 0.99996`). At rest those tracks are equal pairs, 1843.60527995505 Hz twice and
3361.11617132096 Hz twice (relative difference `3e-15`), and stay equal at every speed. Cause: the disc D is axisymmetric, so at
`2k = N` the anti-periodic sector problem `a_H = -R a_L` holds both the `cos 6 phi` and the `sin 6 phi` field of the continuum
(only the sector rows, not the structure, single out a phase), and they have one frequency; D3 grouped vectors by the
multiplicity of the index (1 at `k = N/2`), so each such vector was tracked alone, and the vector assurance between two arbitrary
bases of one eigenspace is arbitrary. The same diagnostic with both effects on gave 0.874 in the same family. It is the
degeneracy D3 handles for the pair, in a family where the registration did not expect it.

**Correction (library).** The tracker groups by eigenvalue clusters as well: at each speed, consecutive sector frequencies of a
family whose values differ by at most `1e-8` relative (the C1b pair-split limit; the pair splits measured `<= 1.1e-13`) form one
cluster, and the subspace of a frequency is the span of all vectors of its cluster (the cos/sin pair vectors included). The
assurance is generalised to subspaces of any dimension, `tr(G_V^{-1} C G_W^{-1} C^T)/min(d_V, d_W)` (the share of the smaller
space inside the larger: 1 when one contains the other); for two single vectors it is the formula of D3 unchanged. A track's
stored subspace is its frequency's cluster. Nothing else changes (the eigensolutions, `modes`, the losses, the crossings, the
CSV and JSON are as before).

**Corrected checks (fixed now, before the corrected run):**

| # | Check | Limit |
|---|---|---|
| P2b | the test of the record repeated with the corrected tracker (the same set-up, speeds and fallback): P1, P3, P5, C3b as registered, and P2 | P2 `>= 0.9`, the rest as registered |
| P2c | the Campbell table with **both effects on** over `0, 0.1, ..., 0.9 omega_ref` (no loss speed), indices `0 .. 6`: P2 (`>= 0.9`), P3 (at least one crossing per family with orders `1 .. 24`; each against a direct solve `<= 1e-2`), every number finite, 4 tracks per family | as stated |

Disclosed beforehand: the diagnostic printed the old tracker's assurances for both tables (the minima above), and every
non-degenerate track `>= 0.99996`; it did not run the corrected tracker. The decision rule is unchanged; P2 counts as missed in the
registered run with its cause diagnosed, and P2b/P2c are reported beside it.

## Addendum 1: results (2026-11-13)

Code: appended to `crates/rcfd-fem/src/rotating.rs` (lines 718 to 1330 of the branch tip; lines 1 to 717, the F-396 code, are
unchanged), `crates/rcfd-fem/src/cyclic.rs` lines 305 and 819 (`doubled_matrix`, `unknown_label` made `pub(crate)`, bodies
unchanged); `mpc.rs` not edited. Tests: `crates/rcfd-fem/tests/si_rotating_cyclic.rs`. The registered run (release,
`--test-threads=2`, 10.1 s, six tests, log `f500-scratch/run1.log`) was made once; P2 missed and its test became the ignored
record `p1_p2_p3_p5_campbell_by_family_known_miss`; amendment 1 (committed before the corrected run) changed the tracker and
registered P2b and P2c, run once (log `f500-scratch/run2.log`, 9.5 s). After that only layout and lint changes were made (rustfmt;
clippy: the `Modes` variant boxed, two lint rewrites in the test), and the four related test binaries were run once more to
confirm the same outcome (`f500-scratch/run3_verify.log`), not to pick a result.

Derived set-up values (printed by the run): `omega_ref = 1599.7736` rad/s (254.61 Hz, the lowest frequency, index 1); no rest mode
has `d_ki > 0` with both effects on (every rest mode is stiffened by more than it is softened), so the loss checks took the
registered fallback `stress_stiffening = false`: `Omega_L = 3395.5329` rad/s `= 2.1225 omega_ref`, `k* = 0`.

| # | Measured | Limit | Met |
|---|---|---|---|
| C1 | count 25 = 25, 25 = 25, 23 = 23 at 0, 0.5, 0.9 `omega_ref`; worst `|sqrt(l_s/l_f) - 1|` 8.8e-14, 5.3e-14, 1.4e-14 | equal, `<= 1e-8` | yes |
| C1b | pair splits at most 9.7e-14, 1.1e-13, 3.9e-14 | `<= 1e-8` | yes |
| C2 | worst family quotient 2.0e-13, 9.3e-14, 4.8e-14 (every index printed) | `<= 1e-8` | yes |
| C3 | `modes(k, 0)` against `modal_analysis_harmonic`, `k = 0 .. 6`, every field by `to_bits` and `==` | identical | yes |
| C3b | the speed-0 column of every family | identical | yes (run 1 and P2b) |
| C3c | 1.6e-12 at `1e-6 omega_ref` | `<= 1e-8` | yes |
| C3d | index 0 at `0.5 omega_ref`: the four eigenvalues have the bits of `RotatingPart::modes` | identical | yes |
| L1 | `k* = 0` at `Omega_L`: `HarmonicLoss { harmonic: 0, multiplicity: 1, negative: Some(1) }` with the speed's bits | each | yes |
| L2 | full rotor negative count 1 = sum over the indices 1 (indices 1 to 6 have modes); pair counts even | exact | yes |
| L3 | no loss at `0.9 omega_ref` (sector, every index; full rotor) | each | yes |
| P1 | 7 families, multiplicities 1, 2, 2, 2, 2, 2, 1, 4 tracks each, the loss of `k = 0` at `Omega_L` listed with `None` frequencies, all finite | each | yes |
| P2 | **0.8952** (family `k = 6` only, its degenerate pairs; amendment 1) | `>= 0.9` | **no** (known miss) |
| P2b | 0.9999999 (the record's table with the cluster tracking) | `>= 0.9` | yes |
| P2c | both effects on, 0 .. 0.9 `omega_ref`: smallest 0.99998 (per family 0.999979 to 0.999995); 4 tracks, no loss, finite | `>= 0.9` | yes |
| P3 | 463 crossings below `0.9 omega_ref` with orders 1 to 24, per family 80, 78, 74, 65, 59, 55, 52; worst against a direct solve 6.7e-8 (P2c table: 453 crossings, worst 2.3e-5) | `>= 1` each, `<= 1e-2` | yes |
| P4 | `engine_order_excites` equals the projection rule for `E = 0 .. 36`, `k = 0 .. 6` | all equal | yes |
| P5 | CSV: 78 rows, harmonic column, speeds and frequencies parse back by `to_bits`; JSON: 7 families, no NaN or inf, `null` at the loss, flagged crossings | each | yes |
| R1 | no cyclic constraint (`cyclic`), harmonic 7 (`harmonic`), speed -1 and NaN (`speed`), decreasing speeds (`speeds`), empty, decreasing and above-`N/2` index lists (`harmonics`), 0 modes (`modes`) | each | yes |
| R2 | `cargo test -p rcfd-fem --release`: 48 binaries, 281 passed, 0 failed, 59 ignored; `si_rotating`, `si_cyclic`, `si_cyclic_harmonic` unchanged and passing; `cargo check -p rcfd-post --tests` clean | all pass | yes |
| N1 | 0.330 (the sector without `K_sigma,1` against the full rotor at `0.9 omega_ref`) | `> 1e-3` | as expected |
| N2 | 0.324 (the rest spectrum against the full rotor at speed) | `> 1e-3` | as expected |
| N3 | 5.7e-9 (the vector MAC of the pairs' first vectors, reported): the subspace assurance is needed for a pair | reported | as expected |

Mutations (a `git archive` copy of the branch tip in `f500-scratch/mut_src`, `f500-scratch/mutate.py`, each run once against
`si_rotating_cyclic`, logs `f500-scratch/M*.log`):

| Mutation | Caught by |
|---|---|
| M1 the sine block without `M_perp` | C1 (counts and frequencies, 9.2e-3 and 2.9e-2), C1b (1.4e-2, 4.4e-2), C2 (1.8e-2, 5.6e-2) |
| M2 `K_sigma,1` left out at `k > 0` | C1 (counts, 0.15 and 0.31), C2 (0.37, 1.23) |
| M3 the loss count divided by the multiplicity | **not caught**: at `Omega_L` only the single family `k = 0` loses stiffness on D (one negative eigenvalue), so dividing by the multiplicity changes nothing; L1's evenness and L2 would see it only with a pair family in loss |
| M4 the zero-speed shortcut removed | **not caught** (as ADR 0403 M7): at rest the general path's `Shift::Value(0)` meets `modal_analysis_harmonic`'s automatic shift, which chose 0 on D, and the `0 x` terms leave every bit; the shortcut stays as the guarantee |
| M5 the vector MAC of the first vectors in the tracker | P2b (5.7e-9), P2c (1.3e-8) |
| M6 `engine_order_excites` with `order mod N == k` only | P4 |
| M7 the transformation of index `k + 1` | C1, C2, C3c, P2b, P2c, P3 |

`cargo fmt -p rcfd-fem --check`: clean (after the layout commit). `cargo clippy -p rcfd-fem --all-targets --release -- -D
warnings`: clean. `baseline_hashes` was not run: the brief limits the tests to `rcfd-fem` (and the check of `rcfd-post`), and no
existing function's code changed (the two `cyclic.rs` edits are visibility only), so no default result can change.

Deviations: (1) the loss speed took the registered fallback, so the Campbell test of the registration ran with the stress
stiffening off; the table with both effects on is P2c (amendment 1). A loss of stiffness of the full operator (both effects) is
not exercised on D: no rest mode of D is softened more than it is stiffened. (2) Amendment 1 says the other families' assurances
were `>= 0.99996`; the diagnostic printed four decimals (`1.0000`), so the honest bound is `>= 0.99995`. (3) M3 survived: the inertia identity was not exercised with a pair family in loss. (4) Not run: the
workspace suite (the lead's gate), `baseline_hashes` (above).

**Decision by the registered rule: partial.** C1, C2, C3, L1 and L2 are met with margins of five orders and more, and every
other check except P2 is met; P2 missed in the registered run with its cause diagnosed (degenerate pairs inside the single
family `k = N/2` of an axisymmetric disc, which the tracker grouped by the index's multiplicity only), and the amended tracker
meets it (P2b, P2c). Under the rule a single missed check with a diagnosed cause is partial. If the lead accepts amendment 1 as
the correction of a registered design gap (the precedent of ADR 0404 amendment 1), the item is done in substance.

Remaining questions:

1. **Status.** Options: (a) partial as the rule says, with P2b/P2c recorded; (b) done, accepting amendment 1. Recommendation (a)
   for the record, with no further code needed; the lead may close it as done at the next review.
2. **A loss of stiffness under the full operator.** D never loses stiffness with both effects on (every mode stiffens more than
   it softens), so L1/L2 used the softening only. Options: (a) a follow-up check on a sector with a soft in-plane mode (a blade on
   a thin hub, or an elastic support) where `K_eff` loses definiteness with both effects; (b) accept the inertia identity as shown.
   Recommendation (a), small: on the same disc only `k = 0` lost stiffness, so mutation M3 (pair counts halved) survived; a set-up in which a pair family loses stiffness closes that gap and comes naturally with the fan sector.
3. **Coriolis on the pair.** The gyroscopic term couples the cos and sin parts; F-499 should treat it on the sector as well.
   Recommendation: add that clause to F-499.

### For the lead: roadmap entries

F-500, proposed status **partial**, note:

> From ADR 0403 question 4, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0407, 2026-11-13: rotating.rs RotatingSector gives the modes of a cyclic sector spinning about its cyclic axis at any harmonic index k, K_eff = K + Omega^2 (K_sigma,1 - M_perp) of the sector with the pre-stress of the index-0 static solve on the real cos/sin pair of ADR 0386 (blockdiag(K_eff, K_eff), M_perp reduced like M; derived: the centrifugal stress repeats in every sector and M_perp commutes with the rotation, so the one K_sigma,1 serves every k); at Omega = 0 the bits of modal_analysis_harmonic; a loss of stiffness is reported per index and speed (HarmonicLoss), the full rotor's negative count equal to the sum over the indices (Sylvester); the Campbell table holds one family of tracks per index, tracked by the subspace assurance of each frequency's eigenvalue cluster (a cos/sin pair and the degenerate pairs of k = N/2 on an axisymmetric part), with the engine-order crossings flagged by the excitation rule E = +/- k mod N, as CSV and JSON. Disc of 12 sectors (Hex20): sector frequencies equal the full rotor's within 8.8e-14 at 0, 0.5 and 0.9 omega_ref, the expanded modes' Rayleigh quotients on the full rotor within 2e-13, the inertia identity exact, 463 crossings within 6.7e-8 of direct solves. Partial: the registered tracking check missed (0.895 against 0.9, the degenerate pairs of k = N/2), met after amendment 1 (0.99998). Coriolis neglected (F-499).

F-499, proposed new note sentence to append:

> ADR 0407 (2026-11-13): on a cyclic sector the gyroscopic matrix couples the cos and sin parts of a harmonic pair; the item should solve it on the sector as well as on the whole part.
