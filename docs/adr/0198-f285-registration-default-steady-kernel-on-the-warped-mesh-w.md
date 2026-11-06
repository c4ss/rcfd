# ADR 0198: F-285 registration: why the default steady kernel diverges on F-208's warped mesh W, and an opt-in fix

Status: **REGISTERED 2026-11-06**, written before any run of F-285 and before any code. Not yet accepted by the maintainer.
Nothing was run for this text; it rests on reading the code of main 9edfbda and on the runs ADR 0193 amendment 1 reports.
This text fixes the diagnosis runs, their readings, the fix candidates with the rule that chooses among them, and the
acceptance limits of the fix. Amendment 1, written after the diagnosis and before any fix code, names the chosen fix and
may add checks; it does not loosen a limit fixed here. Results go in the addendum.

Parents: [ADR 0193](0193-f246-registration-pressure-interpolation-variants.md) (F-246: the manufactured solution M, its
kernel set-up and stop rule, the meshes, amendment 1 finding 1, Q9 and its decision record),
[ADR 0084](0084-wave37a-sheared-mesh-simplec-linearisation.md) (the period-two pressure mode of SIMPLEC on sheared meshes,
the mismatch between the cell and the face corrections, the fix R = `velocity_correction = "face_flux"`),
[ADR 0016](0016-wave13-numerics.md) D2 (the F-208 meshes), [ADR 0050](0050-wave27b-robustness-adaptive-stepping.md) D1.
Roadmap item F-285.

Number: 0197 is the highest on main and no branch holds a 0198; the number was assigned to this work by the lead.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0193):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test (by an amendment written before the corrected run), and the old one stays as an
  ignored record;
- the default stays bit-identical: every new code path is opt-in, `crates/rcfd-api/tests/baseline_hashes.rs` and
  `crates/rcfd-api/tests/case_hashes_w48u.rs` are not edited and pass, no baseline or golden is re-recorded; if the diagnosis
  shows that the cause cannot be fixed without changing converged defaults, the work stops there and reports;
- no new third-party crate, no commit trailers.

Clean-room: every formula below is derived here from the code read and from first principles (Fourier symbols on a uniform
grid, the algebra of one iteration). No published source is quoted, and no vendor code, text or install was used.

## What F-285 asks

The roadmap entry: registered diagnosis runs locate the growing mode by which the default steady SIMPLE/SIMPLEC kernel
diverges on W from n = 32 (growth about 1.8 times per iteration under every set-up ADR 0193 amendment 1 tried; the
pressure-velocity step from `HbyA` to the corrected velocity is suspected, not proven); a fix makes the kernel converge on W
at n = 32 and 64 under F-246's registered set-up, within limits fixed here, with the defaults bit-identical; afterwards
F-246's missed M1 check on W (`m1_w_v1_known_miss`, `m1_w_v2_known_miss`) is run again with its registered set-up and limits
unchanged; how the fix is switched on for that run is fixed here.

## What the code read shows (main 9edfbda, nothing run)

1. **One iteration of `SimpleSolver::iterate`** (kinematic pressure; SIMPLE, so `rAtU = rAU`): the predictor solves the
   relaxed momentum equation with the source `-V grad_p`, `grad_p` the Green-Gauss gradient of the stored (relaxed) pressure
   `p_old`; `HbyA = H / a_P` (plus `rAU f` with a body force); the pressure equation `L p_new = D(HbyA)` with `L` the compact
   operator `gamma_f lap_f (p_N - p_P)`, `lap_f = |S|^2 / (S . d)`, and with correctors the deferred `k . grad p` part; the
   face flux `phi = phiHbyA - gamma_f (lap_f (p_N - p_P) + k . grad p_f)`; the cell velocity `u = HbyA - rAtU G(p_new)`, `G`
   the same Green-Gauss gradient (`grad_corr`); then `p = p_old + alpha_p (p_new - p_old)`, which is read only by the next
   predictor. `D` is the divergence of the linearly interpolated face flux of `HbyA` (prescribed velocity on fixed-velocity
   faces).
2. **Why no relaxation, viscosity or convection scheme changes the rate (derivation).** Let `e` be a velocity perturbation
   with `a_P^-1 H e = h e` (the momentum stencil's neighbour sum over the diagonal; `h` near 1 for a perturbation that is
   smooth on that stencil). The relaxed predictor gives `u* = (1 - alpha_u) / (1 - alpha_u h) e` and `HbyA = u* + rAU G p_old`,
   so for `h` near 1 the perturbation of `HbyA` is `e` for every `alpha_u`. The cell velocity's perturbation is then `Pi e`
   with `Pi = I - rAtU G L^-1 D`, and `alpha_p` does not enter this path; the viscosity, the convection scheme and a pseudo
   time step enter only through `h`. So a mode of `Pi` outside the unit disc, smooth on the momentum stencil, grows at a rate
   that none of those settings changes, which is what ADR 0193 amendment 1 saw. The non-zero eigenvalues of `rAtU G L^-1 D`
   are those of `L^-1 D rAtU G`, the "wide" Laplacian over the compact one. On a uniform orthogonal grid their symbol ratio
   is `cos^2(kh/2)` in `[0, 1]`, so the eigenvalues of `Pi` lie in `[0, 1]`; growth needs a ratio above 2 (an eigenvalue of
   `Pi` below -1, a period-two mode) or below 0. This is the hypothesis to test; nothing here proves that W has such a mode.
3. **`face_flux` (ADR 0084's R)** replaces the cell correction by `u = HbyA - rAtU G(p_old) + R(phi(p_new) - phi(p_old))`,
   `R(psi)_c = (1/V_c) sum_f s_cf psi_f (x_f - x_c)`. The same reading gives `Pi_R = I - R Lambda L^-1 D`, `Lambda q =
   gamma_f lap_f (q_N - q_P)`: `R Lambda` takes the place of `G`. ADR 0193 amendment 1 found that `face_flux` does not stop
   the divergence on W.
4. **Where W is poor (derivation from the map).** W's box faces are not planar (for example `x' = 1 + 0.1 sin(2 pi y)
   sin(2 pi z)` on `xmax`). Near the edge `x = y = 1` the two walls are `x' - 1 = s (y - 1)` and `y' - 1 = s (x - 1)` with
   `s = 0.2 pi sin(2 pi z)`, so the interior angle between them is `acos(2 s / (1 + s^2))`: 25.7 degrees where
   `sin(2 pi z) = 1` and 154.3 degrees where it is -1; every edge has such an acute part. The edge cells there tend, as `n`
   grows, to thin parallelepipeds whose non-orthogonality is `acos((1 - s^2) / (1 + s^2)) = 64.2` degrees, the limit of the
   61.3, 63.5 and 64.1 degrees of ADR 0193 at n = 16, 32, 64. The largest error ADR 0193 reports (cell centre near
   x = y = 0.97) lies in such an edge cell.
5. **Fixed-point-keeping changes of the cell correction.** Any cell correction `u = HbyA - rAtU G(p_old) - rAtU C(q)`,
   `q = p_new - p_old`, with `C(0) = 0`, has the fixed points of the default: at a fixed point `q = 0`, so the cell velocity
   is `HbyA - rAtU G(p)` as in the default, and the face flux, the pressure equation and the predictor are not changed. The
   default is `C(q) = G(p_new) - G(p_old)` (the Green-Gauss gradient of `q` with homogeneous boundary values, as `G` is affine
   in the pressure); R is `C = -R Lambda`. A fix of this kind leaves the discretisation, and so F-246's variants (which act on
   `G` and on the flux), unchanged; only the path to the fixed point changes.

## Diagnosis runs (registered; each run once)

Harness: a new file `crates/rcfd-fv/tests/warped_kernel_f285_diag.rs`, ignored tests run by name in release. The meshes (W,
W2, K, G) and the manufactured solution M are copied unchanged from `crates/rcfd-fv/tests/pressure_interp_f246.rs`; set-up A
is ADR 0193's registered kernel set-up (SIMPLE, `alpha_u = 0.7`, `alpha_p = 0.3`, `central`, one corrector, linear-solver
`tol_abs = 1e-300`, the data of M). "Edge cells" are the cells with at least two of their logical indices `(i, j, k)` (taken
from the cell centres of the unwarped `box_mesh`) in `{0, n - 1}`; "wall cells" those with at least one. At n = 32 the edge
cells are 1.1 % of the cells and the wall cells 17.6 %.

**D0 (the kernel, V0).** Set-up A on W at n = 32 and at n = 16, and set-up L (A with `alpha_u = 0.2`, `alpha_p = 0.1`) on W
at n = 32; 400 iterations each, or until an iterate is not finite. Measured:

- `g`, the growth factor `(max_du_400 / max_du_200)^(1/200)`;
- `c`, the correlation `<du_400, du_399> / (|du_400| |du_399|)` of the last two velocity increments (volume-weighted over all
  cells and components): near -1 for a negative real eigenvalue (period two), near +1 for a positive one;
- `f_edge` and `f_wall`, the fractions of `sum_c V_c |du_400,c|^2` in the edge cells and in the wall cells;
- the logical indices of the cell with the largest `|du_400|` and the sign of `sin(2 pi z)` there (the acute or the obtuse
  part of its edge);
- reported: `max |dp_400| / max |du_400|`.

**D1 (the correction step alone).** Linear maps of a cell velocity field `v` built from the public functions of `rcfd-fv`,
with `rAtU = 1` in every cell (so no momentum coefficient enters), zero velocity on every boundary face (so zero boundary flux
and the zero-gradient pressure condition of fixed-velocity walls), the pressure level fixed as the kernel fixes it (cell 0),
and pressure solves to `tol_rel = 1e-12`:

- `Pi_GG(v) = v - G(q)`, `q` from the pressure solve `L q = D v` without correctors, `G` the Green-Gauss gradient with the
  pressure-condition boundary values;
- `Pi_GG1`: the same with one non-orthogonal corrector in the pressure solve (starting from `q = 0`);
- `Pi_R(v) = v + R(-Lambda q)` (internal faces; zero on the boundary faces);
- `Pi_LS(v) = v - LS(q)`, `LS` the least-squares gradient with the same boundary values;
- `Pi_GG,beta(v) = v - beta G(q)` for `beta` in {0.75, 0.5, 0.25}.

Each map on W at n = 16 and 32, on W2 and K at n = 32 (controls on which V0 converges), on the orthogonal box G at n = 32;
`Pi_GG` also on W at n = 64. Power iteration from a fixed pseudo-random field (all components, a linear congruential
generator with a stated seed), 300 steps (200 at n = 64), the field normalised in every step in the volume-weighted norm.
Measured: `rho`, the geometric mean of `|Pi v| / |v|` over the last 100 steps; `c`, the correlation of `Pi v` and `v` at the
last step; `f_edge` of the last field. `Pi` keeps the fields with `D v = 0`, so `rho` is at least 1 on every mesh; a mode
outside the unit disc shows as `rho > 1.01`.

**D3 (fix candidates in the kernel).** A test-only switch of `SimpleSolver` (hidden, default off, read only when set, so the
default path is unchanged by construction) selects the cell correction of item 5:

- **C-beta**: `C(q) = beta (G(p_new) - G(p_old))`, i.e. `u = HbyA - rAtU ((1 - beta) grad_p + beta grad_corr)` with the
  gradients the kernel already forms (so it acts with every pressure-interpolation variant), `beta` in {0.75, 0.5, 0.25};
- **C-LS**: `C(q) = LS(q)`, the least-squares gradient of `q` with homogeneous boundary values;
- **R**: the existing `reconstructed_correction` (`face_flux`), not a new switch.

Each candidate with set-up A, V0, on W at n = 32 (cap 5000 iterations) and on the controls W at n = 16, K and W2 at n = 32
(cap 5000), with ADR 0193's stop rule. Measured: converged or not, iterations, and on the controls the largest cell
difference of `u` and of `p - mean(p)` against the V0 run without the switch (the same test run computes both).

## Hypotheses (registered with their falsifiers; each reading is final)

| id | hypothesis | stands iff | falsified iff |
|----|------------|------------|---------------|
| H1 | the divergence is a linear mode whose rate does not depend on the relaxation | `g(A, W32) > 1.05` and `g(L, W32) > 1.05` and `abs(g(L) / g(A) - 1) <= 0.1` | `g(L, W32) <= 1` |
| H2 | the mode lives in the correction step (item 2) | `rho(Pi_GG, W32) > 1.01` | `rho(Pi_GG, W32) <= 1.01` |
| H3 | it lives in the edge cells | `f_edge > 0.5` both in D0 (A, W32) and in D1 (`Pi_GG`, W32) | `f_edge <= 0.5` in both |
| H4 | it is a period-two mode (a ratio above 2 in item 2) | `c < -0.9` both in D0 (A, W32) and in D1 (`Pi_GG`, W32) | `c > 0` in both |
| H5 | the controls have no such mode | `rho(Pi_GG) <= 1.01` on G, K, W2 at n = 32 and on W at n = 16 | `rho(Pi_GG) > 1.01` on one of them |

A reading between "stands" and "falsified" is reported as "partly" with the numbers. Also reported: whether `rho(Pi_GG, W32)`
predicts `g(A, W32)` (their ratio), and which of `Pi_GG1`, `Pi_R`, `Pi_LS`, `Pi_GG,beta` have `rho <= 1.01` on W at n = 32.

## The fix (candidates and the choice rule, registered now)

**Qualifies**: a candidate of D3 that converges on W at n = 32 within its cap and on every control (W16, K32, W2-32), and on
each control has the largest cell difference against V0 of at most `1e-8` for `u` and for `p - mean(p)`. **Choice**: of the
qualifying candidates the one with the fewest iterations on W at n = 32; within 5 % of that count, the first of R, C-LS,
C-beta 0.75, C-beta 0.5, C-beta 0.25. **None qualifies**: amendment 1 records it; it may add candidates that D0 to D3 point at
(disclosed as added after the diagnosis, under the same rule), or it stops F-285 with the evidence and the work reports
without a fix. A candidate that changes the fixed point is not one (item 5): changing converged defaults is not in scope.

**The opt-in switch (registered now; its name in amendment 1).** A public field of `SimpleSolver`, default off: the existing
path, unchanged and bit-identical by construction. It is a library switch only; no case key (Q2). The test-only switch of D3
is removed once the public field exists, or becomes it.

## Acceptance of the fix (limits fixed now; amendment 1 may add checks, not loosen these)

All in release, in `crates/rcfd-fv/tests/warped_kernel_f285.rs` unless stated; the harness is `pressure_interp_f246.rs`'s
`mms` (set-up, data, stop rule, cap 30000 and errors unchanged) with the fix switched on through an added argument.

- **A1 (W converges).** With the fix, V0 converges under the stop rule on W at n = 16, 32 and 64 (heavy test for n = 64,
  ignored, run by name). A run that reaches the cap is a miss.
- **A2 (the same fixed point where V0 converges).** With the fix, V0 on K, S and W2 at n = 32 and on W at n = 16: converged,
  `abs(e_u(fix) / e_u(V0) - 1) <= 1e-6` and the same for `e_p`, and the largest cell difference against V0 without the fix at
  most `1e-8` for `u` and for `p - mean(p)` (the stop rule leaves an estimated iteration error of `1e-10 U` in each run).
- **A3 (W's fixed point is the default's).** From the converged fix state on W at n = 32, one iteration of the default kernel
  (the switch off) changes the velocity by at most `1e-8` (`max_du`).
- **A4 (defaults).** The switch off is the existing path (by construction); `baseline_hashes.rs` and `case_hashes_w48u.rs`
  pass unedited; every non-ignored test of `pressure_interp_f246.rs` passes, its existing tests unchanged in behaviour.
- **A5 (F-246's M1 on W, run again).** Two new ignored heavy tests in `pressure_interp_f246.rs`, `m1_w_v1_f285` and
  `m1_w_v2_f285`, run `sequence(W, "V1")` and `sequence(W, "V2")` exactly as `m1_w_v1_known_miss` and `m1_w_v2_known_miss`
  do (set-up, stop rule, cap and the limits 1.7 and 1.2 unchanged) with the fix switched on (Q1). The known-miss tests stay
  unchanged as the record without the fix. The result is recorded as it comes; a miss becomes a `*_known_miss` test with the
  limits intact. V0 on W with the fix (the sequence of A1) is reported with its orders, without a limit (as in F-246).
- **A6 (can fail).** One mutation, restored afterwards and compared byte for byte with the kept copy: the line that reads the
  fix switch in `iterate` removed; then A1 at n = 32 must fail. `scripts/limit_audit.py` on the new tests.
- **Gate.** fmt per package, clippy `-D warnings`, every test in release on a `git archive` copy, one process per test, three
  jobs: no failure; a known flaky failure is rerun alone and reported with its name.

## Not in F-285

A default change (Q3); the transient kernel and the other solvers; a case key (Q2); a new gradient scheme or a change of the
discretisation (item 5); reopening the dropped warped-face item (ADR 0193 decision record, Q9 (c) not taken).

## Open questions

**Q1. How the fix is switched on for M1 on W.** (a) New heavy tests beside the known-miss ones, the harness taking the fix as
an added argument that the existing tests leave off (A5); (b) edit the two known-miss tests to switch the fix on. Recommendation
(a): the known-miss tests stay the record of the default kernel. Proceeding on (a).

**Q2. A case key.** (a) Library only in F-285; (b) wire a key now under F-256's checks. Recommendation (a): the roadmap's
acceptance is on the library harness, and a key needs its own wiring checks. Proceeding on (a).

**Q3. The default.** The fix keeps the fixed point but changes every iterate, so it cannot be the default without changing the
baseline hashes; it stays opt-in. Making it the default needs the maintainer to lift the bit-identity rule.

## Amendment 1 (2026-11-06): more of the correction step, implicit non-orthogonal candidates, and the linear onset

Written after D0, after D1 on every mesh but W at n = 64, and after D3 on W at n = 16 (and the V0 rows of K and W2), and
before any other run; D3 on W, K and W2 at n = 32 was running. No limit is changed. What was known:

- D1 `Pi_GG` on W at n = 32: `rho` 1.1991, `c` -1.0000 (a period-two mode, a ratio of about 2.2 in item 2), the mode in the
  interior near the cells (27, 28, 28), not at the edges (`f_edge` 0.0049); `Pi_GG1` 1.3995 with `c` +1.0000 (a positive
  mode), `Pi_LS` 1.1045 (`c` -1), `Pi_R` 1.0000, `Pi_GG,beta` 1.0000 for each `beta`. On W at n = 16, W2, K and G at n = 32
  every map has `rho` 1.0000.
- D0: the run on W at n = 32 has `max_du` 8.8e-4, 4.6e-2, 1.4e8, 1.8e16, 3.0e24, 4.5e32 at iterations 1, 10, 100, 200, 300,
  400, and the pressure change is 1.3e33 times the velocity change at iteration 400 (L: 2.1e42 and 2.0e43). So D0's
  registered window (200 to 400) lies far in the nonlinear runaway (velocities of 1e16 and more, pressures like their
  square), not in the linear regime where the mode starts: its `g`, `c` and `f_edge` describe the runaway. They are reported
  and H1 is read on them as registered; the linear onset is measured by D6 below. That is a defect of the registered D0
  window; D0 stays as the record.
- D3 on W at n = 16: every candidate converges in 218 iterations, as V0, with the largest differences against V0 of 2e-13
  to 7e-13 (`u`) and 7e-13 to 6e-12 (`p`).

Reading of what is known, not a result: with no corrector the correction step has a period-two mode on W at n = 32 that
`Pi_R` and every `Pi_GG,beta` remove; with one corrector a positive mode appears, which a smaller `beta` cannot remove
(`1 - beta mu` exceeds 1 for a negative `mu`). The registered kernel set-up has one corrector, so the deferred
non-orthogonal part of the pressure equation is the next suspect.

**D4 (more sub-maps; the same meshes, power iteration, `rho`, `c` and `f_edge` as D1).**

- `X_p`: the deferred non-orthogonal step of the pressure equation, `q -> A^-1 b_N(q)`, `A` the compact matrix of the
  pressure equation (zero-gradient walls, the reference cell as the kernel pins it), `b_N(q)` the explicit non-orthogonal
  part of its right-hand side for the least-squares gradient of `q` (homogeneous boundary values). Its spectral radius above 1
  means the deferred correction cannot converge, however many correctors.
- `X_u`: the same for the momentum Laplacian (`nu = 1`, zero velocity on every wall, the Dirichlet boundary part included),
  one scalar component.
- `Pi_R1`: `Pi_R` with the one-corrector pressure solve of `Pi_GG1` and the face-flux change including its non-orthogonal
  part (as the kernel's `face_flux` with one corrector).
- `Pi_GG,imp`, `Pi_R,imp` and `Pi_GG,imp,beta` (`beta` in {0.75, 0.5}): `q` from the pressure equation with its
  non-orthogonal part solved implicitly, `F(q) = A q - b_N(q) - (-D v) = 0` by restarted GMRES (restart 30) right-preconditioned
  with one AMG V-cycle of `A`, to `tol_rel = 1e-12`; for `Pi_R,imp` the face-flux change includes the non-orthogonal part.

**D5 (implicit candidates in the kernel).** A second test-only switch (hidden, default off) solves the kernel's pressure
equation as in D4 (`A` and `b_N` the kernel's own assembly, GMRES from `p_old` to the kernel's pressure tolerance, restart 30,
the iteration cap of `pressure_solver`) in place of the corrector passes; the flux and the cell correction then use that
pressure as the default does. It needs `non_orth_correctors > 0`. Its fixed points are those of the default with
correctors (at a fixed point the corrector passes solve the same equation `A p = b_N(p) - D(HbyA)`). Candidates, added to
D3's list under the registered qualifying and choice rules: **I-GG** (the implicit solve alone), **I-R** (with
`face_flux`), **I-beta 0.75** and **I-beta 0.5** (with C-beta). The tie order of the choice rule is extended: R, C-LS, C-beta
0.75, 0.5, 0.25, then I-R, I-GG, I-beta 0.75, I-beta 0.5 (the corrector path first, as the cheaper one). Each on W at n = 32
and on the controls W16, K32, W2-32, cap 5000, as D3.

**D6 (the linear onset).** If a candidate converges on W at n = 32: from its converged state plus `1e-8 w` (`w`: the
`x` velocity of cell `c` times `sin(c)`), the default kernel (switches off), until `max |u_k - u*|` exceeds 1e-3 or 2000
iterations: the growth factor as the geometric mean of the ratio of successive `max |u_k - u*|` over the iterations where it
lies in [1e-7, 1e-3], and `c`, `f_edge`, `f_wall` and the largest cell of `u_k - u*` at the last of those iterations. The
readings H3 and H4 are given again on D6 beside the registered ones (the registered ones stand as read).

## Amendment 2 (2026-11-06): the diagnosis read, the fix chosen, and its checks

Written after every run of D0 to D5 and before any fix code; D6 was running. No limit is changed. The numbers are those of
the addendum's diagnosis table (binaries of d698fe8 for D0, D1, D3 and of 91210eb for D4, D5).

### Readings (final, by the registered rules)

- **H1 stands as registered**: `g(A, W32)` 1.2078, `g(L, W32)` 1.2940, ratio 1.071 (within 10 %); read on the runaway
  window (amendment 1), so it says that the runaway does not depend on the relaxation, not what the onset rate is (D6).
- **H2 stands**: `rho(Pi_GG, W32)` 1.1991; at n = 64 it is 1.3200.
- **H3 is falsified**: `f_edge` 0.0109 (D0) and 0.0049 (D1). The modes are not at the edges where W is most
  non-orthogonal. The correction-step mode sits at interior points such as cell (27, 28, 28) at n = 32 (and (7, 8, 56) at
  n = 64), where the map compresses the cells along a body diagonal: at `(7/8, 7/8, 7/8)` its Jacobian is
  `(1 - a) I + a 1 1^T` with `a = 0.1 pi sin(4 pi 7/8) = -0.314`, so the cells shrink to 0.37 of their size along `(1, 1, 1)`
  (derived from the map). The mode of `X_p` sits near a corner (cell (1, 29, 1), `f_wall` 0.67, `f_edge` 0.16).
- **H4 partly**: `c` +0.9915 in D0 (the runaway) and -1.0000 in D1.
- **H5 stands**: `rho(Pi_GG)` 1.0000 on G, K, W2 at n = 32 and on W at n = 16.

### The mechanism (stated with the measurements)

With the registered set-up (one non-orthogonal corrector) the kernel solves the pressure equation `A p = b_N(p) - D(HbyA)`
by deferred correction: each corrector pass and each outer iteration (through `p_old`) applies the step `X_p`. On W at
n = 32 `X_p` has an eigenvalue of about -1.18 (`rho` 1.1781, `c` -1); on W at n = 16, W2 and K it is 0.861, 0.813 and 0.619.
A real eigenvalue below -1 cannot be damped by any relaxation of that iteration, and the truncated correction then flips
the sign of the cell correction on some modes (`Pi_GG1` and `Pi_R1` have positive modes of 1.40 and 1.37). Without
correctors the correction step has a second, period-two mode (`Pi_GG` -1.20) that `face_flux` or a smaller `beta` removes,
but then the non-orthogonal part of the discretisation is missing. With the non-orthogonal part of the pressure equation
solved implicitly both modes are gone: `Pi_GG,imp`, `Pi_R,imp` and `Pi_GG,imp,beta` have `rho` 1.0000 on W at n = 32, and
every implicit candidate converges there in 716 iterations, while every corrector-path candidate (R, C-LS, C-beta 0.75,
0.5, 0.25) diverges (not finite at iterations 1155 to 1550). The pressure-velocity step was suspected (ADR 0193 Q9);
the diagnosis locates the growing mode in the deferred non-orthogonal correction of the pressure equation, and on the
cell-correction step only when that part is left out. The momentum's deferred step `X_u` (pure diffusion, zero walls) is
1.0525 on W at n = 32 (0.73 to 0.75 on W2 and W16, 0.57 on K); it does not stop the implicit candidates from converging and
is reported, not acted on.

### The fix chosen (the registered rule)

Qualifying: I-R, I-GG, I-beta 0.75 and I-beta 0.5 (each converges on W at n = 32 in 716 iterations and on W16, K32 and W2-32,
with largest differences against V0 of at most 3.0e-12 in `u` and 4.3e-12 in `p`; limit 1e-8). None of R, C-LS and C-beta
qualifies (each diverges on W at n = 32). All four take the same count, so the tie order chooses **I-R**: the pressure
equation with its non-orthogonal part solved implicitly, with the existing `face_flux` cell correction.

**The switch.** The D5 switch becomes the public field `SimpleSolver::implicit_non_orth_pressure: bool` (default `false`,
the existing path), and `pressure::solve_pressure_implicit_non_orth` becomes a documented public function. With `true` and
`non_orth_correctors > 0` the pressure equation is solved by it in place of the corrector passes (with
`BodyForceWeighted` the V2-c gradient); with no corrector the field is ignored; periodic faces are refused by a panic with a
message; the linear-solver device is not used by it (CPU). **F-285's fix** is `implicit_non_orth_pressure = true` together
with `reconstructed_correction = true`. The D3 switch (`correction_test_only`, `CorrectionTestOnly`) is removed and the
velocity correction of `iterate` returns to the code of main; the diagnosis harness keeps the tests that do not need it (its
D3 tests and the I-beta rows of D5 were run from the binaries named above).

### Checks (A1 to A6 as registered, made concrete; A7 added)

The harness is `pressure_interp_f246.rs`'s, so the checks live there (the registration's "unless stated"): `mms` becomes
`mms_run(kind, n, v, fix)` returning the outcome and the solver, `mms(kind, n, v)` is `mms_run(kind, n, v, false).0`,
`sequence(kind, name)` is `sequence_with(kind, name, false)`; `fix` sets the two fields of the fix after
`pressure_interpolation`. Every existing test keeps its code path, its limits and its printed lines.

- **A1**: `f285_a1_a3_w_16_32` (V0 with the fix on W at n = 16 and 32, each converged under the stop rule within the cap)
  and the heavy `m1_w_v0_f285` (the V0 sequence 16, 32, 64 with the fix: every run converged; its orders reported).
- **A2**: `f285_a2_k32`, `f285_a2_s32`, `f285_a2_w2_32` and `f285_a2_w16`, the limits as registered.
- **A3**: in `f285_a1_a3_w_16_32`, from the converged fix state on W at n = 32, both fields set back to `false`, one
  iteration: `max_du <= 1e-8`.
- **A4**: as registered.
- **A5**: the heavy `m1_w_v1_f285` and `m1_w_v2_f285` as registered.
- **A6**: the mutation replaces the line `let implicit = self.implicit_non_orth_pressure && s.non_orth_correctors > 0;`
  of `iterate` by `let implicit = false;`; then `f285_a1_a3_w_16_32` must fail.
- **A7 (reported, no limit)**: the iterations and the time per iteration with and without the fix in A2.
- **Heavy rule** as F-246's: a test that takes more than 15 minutes in release is ignored and run by name.

### Questions added

**Q4. I-R or I-GG.** The tie order chose I-R (two switches); I-GG (the implicit solve alone) took the same 716 iterations on
W at n = 32 and is one switch. Options: (a) keep I-R as chosen; (b) make I-GG the fix in a later change. Recommendation (a),
the rule's outcome; I-GG remains available as the field alone.

**Q5. The momentum's deferred step.** `X_u` is 1.0525 on W at n = 32, above 1, without stopping convergence here. Options:
(a) record it; (b) a later item for an implicit non-orthogonal momentum Laplacian. Recommendation (a) until a case diverges
through it.

## Where to go next

- [ADR 0193](0193-f246-registration-pressure-interpolation-variants.md) for the manufactured solution, the meshes and the M1
  check this item runs again.
- [ADR 0084](0084-wave37a-sheared-mesh-simplec-linearisation.md) for the related period-two mode on sheared meshes.
