# ADR 0387: F-487 registration: a consistent cell pressure gradient in the overset geometry corrections

Status: **REGISTERED 2026-11-13, written before any code of F-487 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-487 was written, compiled or run: the code and the documents listed under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0382](0382-f472-registration-overset-geometry-corrections-with-a-varying-pressure.md) (F-472: the flow V, the
runs P3o, W3o, ..., the diagnostic opt-in L `OversetSolver::set_least_squares_pressure_gradient`, its question Q1 option (b)
and Q2 option (b), from which this item comes; `crates/rcfd-fv/tests/overset_f472.rs`, whose set-up code is reused),
[ADR 0367](0367-f368-registration-overset-skewness-and-non-orthogonal-corrections.md) (F-368: the opt-in
`OversetSolver::set_geometry_corrections`, the linear shear F and the uniform flow, their limits A1 to A4, K7, K7p;
`crates/rcfd-fv/tests/overset_f368.rs`, whose set-up code is reused), [ADR 0255](0255-f357-registration-uniform-flow-on-a-warped-overset-mesh.md)
(the motion `P(t)` of the component on B3 and B3d). Roadmap item F-487 (depends on F-472).

Number 0387 (given by the lead). Branch `f487-overset-consistent-gradient`, worktree `w49i-work/f487`, scratch
`w49i-work/f487-scratch`.

House rules (the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not changed after a run; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment committed
before the corrected run; existing tests and records are not edited; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash
tables and the goldens are not edited or re-recorded; each registered run is made once (runs shared by several tests are
computed once per test process through a `OnceLock`); no new crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing outside this repository was opened. The weighted least-squares gradient is the library's own
`gradient::least_squares` (textbook practice, cited from memory as in ADR 0382: Ferziger and Peric, Computational Methods for
Fluid Dynamics, chapter on complex geometries); the change of its boundary rows below is derived here from first principles
(the residual of a linear field under a prescribed normal gradient). The flow V is ADR 0382's, verified there by substitution.

## What F-487 asks

The roadmap acceptance (quoted, binding): "OversetSolver::set_geometry_corrections takes the cell pressure gradient of the
momentum predictor and of the velocity correction from the least-squares gradients of the corrections (or another gradient
exact for linear fields on B3d), with the defaults bit-identical: the Taylor-Green vortex with an axial velocity of ADR 0382
on B3 and B3d with the component moving as in ADR 0255 has an error of the calculated and fringe cells on B3d at most twice
that on B3 with the default and the quadratic donor interpolation, continuity within 1e-9 at every step, ADR 0367's linear
shear and uniform flow within their limits, and a run of the warped background alone that names the share of the warped
background in the remainder."

## What ADR 0382's opt-in L changed, and why it is not yet the acceptance

L (`set_least_squares_pressure_gradient`, only with the corrections) replaces, in `piso`, both `green_gauss(&g, p, bvals_p,
0)` calls (the predictor's `grad_p` and the correction's `grad_new`) by `GeometryCorrections::gradients(&g, p, bvals_p,
corr_conn)`: the weighted least-squares gradient over the face neighbours and the boundary faces (boundary value
`bvals_p`), donor-interpolated in a fringe cell, zero in a hole cell. Nothing else. It met the default-stencil ratio (`1.22`)
and missed the quadratic one (`2.33`). It is not yet the acceptance for four reasons:

1. **It is a separate diagnostic flag**, not the behaviour of the corrections.
2. **The quadratic ratio misses.** ADR 0382's addendum: the largest error of W3qoL (and of W3oL's background class, the same
   `5.44e-3`) is in the background calculated cell `13327` at step 40, and the class BC of the warped background does not fall
   with the quadratic stencils. Read now from `rcfd_mesh::gen::build_masked` (cells numbered `i` fastest, then `j`, then `k`;
   no run): `13327 = 23 * 576 + 3 * 24 + 7`, the cell `(i, j, k) = (7, 3, 23)`: **a cell of the top layer, with a face on the
   outflow patch `zmax`**. The background cell of W3o's largest error, `4205 = (5, 7, 7)`, is interior.
3. **Its gradient is not exact for linear fields on B3d in the solver**, only with exact boundary values (ADR 0382 K1). In the
   step every patch has a fixed velocity, so `pressure_bc` gives every boundary face the zero-gradient coefficients `(a, b, c,
   d) = (1, 0, 0, 0)` and `bvals_p` is the owner's value. Derivation: the least-squares row of a boundary face is `w (p_b - p_c
   - g . d_b)^2` with `d_b = x_f - x_c`. For a linear `p = p0 + c . x` that satisfies the condition (`n . c = G`, `n` the unit
   normal), the exact face value is `p_c + c . d_b`, while the condition's value is `p_c + dn G` with `dn = d_b . n`. They agree
   only when `d_b` is normal to the face (`d_b = dn n`), which holds on B3 and not on B3d: there the boundary points stay on the
   box but the interior points are displaced, so a boundary cell's centre has a tangential offset `t_b = d_b - dn n` from its
   face centre, and the row carries the residual `-c . t_b` at the exact gradient. The weight of this row is the largest of the
   cell's (`w = 1 / abs(d_b)^2`, `abs(d_b)` about `h/2`), so the gradient of a wall cell is wrong by about `abs(c_t) abs(t_b) /
   dn` in its normal component, a fixed fraction of the tangential pressure gradient that does not shrink with `h`. At `zmax`
   of V that normal component is `d p / d z`, which drives `u_z` directly.
4. Neither the linear shear and uniform flow of ADR 0367 with the new gradient nor a run of the warped background alone was
   made.

## The decision on bit-identity (registered here)

The acceptance's "with the defaults bit-identical", read by the rule of the brief: `set_geometry_corrections` may change the
behaviour of runs that opt into the corrections only if no existing test or hash covers them. **Existing tests cover them**:
`overset_f472.rs` `d1b_opt_in_fingerprint_equals_the_base_tree` asserts the bits of 5 steps of V on B3d with
`set_geometry_corrections(true)` and without L (two fingerprints recorded on the base 4bcfd339), and `overset_f472.rs`
`a5_l_share_of_the_green_gauss_pressure_gradient` (L0) asserts that L changes the error of W3o, which fails if the corrections
take L's gradient themselves. `overset_f368.rs` asserts only limits for the opt-in (its D1 fingerprints are of the defaults).
So `set_geometry_corrections` is **not** changed and the consistent gradient is **a new opt-in** (below), whose entry point
switches the corrections on together with it. No hash table, golden or existing test is edited.

## The design: the consistent least-squares pressure gradient C (new opt-in, library)

**C**, per cell: the weighted least-squares gradient of the pressure over the same rows as `gradient::least_squares` (each
internal face, for both of its cells, `d = x_n - x_o` and the difference of the cell values; each boundary face not on an
`Empty` patch; weight `1 / abs(d)^2`), except that **a boundary face whose pressure condition prescribes the normal gradient**
(not periodic-coupled, coefficients `a = 1`, `c = 0`: the forced zero gradient of a fixed-velocity face, or the wall gradient
of `AleStep::apply_wall_gradients`, `d = G`) **contributes the row of its normal component only**: direction `m = dn n`
(`dn = d_b . n`, `n` the unit area vector), increment `dn G`, weight `1 / dn^2`. Every other boundary face (a prescribed value,
a mixed or coupled condition) keeps the row of `least_squares` with the value `boundary_values_pbc`. Then, as ADR 0367 G: the
donor interpolation of the gradients in a fringe cell and zero in a hole cell.

**Exactness (derivation).** For a linear `p` that satisfies every boundary condition (the prescribed values and normal
gradients), every row's residual vanishes at `g = c`: the internal rows as in `least_squares`, a value row because the
prescribed value is the exact face value, a normal row because `dn G = dn (n . c) = m . c`. The normal matrix `sum w m m^T`
is positive definite when the rows' directions span space (a wall cell keeps its tangential neighbours and its normal
neighbour), so `g = c` is the unique minimiser: **exact for linear fields on B3d with the pressure conditions the solver
actually applies**, not only with exact boundary values. On B3 a boundary cell's `d_b` is normal to its face (`m = d_b` up to
round-off, `dn G = b`), so C equals L there to round-off; in a cell without a boundary face C is L bit for bit.

**The entry point.** `OversetSolver::set_consistent_geometry_corrections(on: bool)`: `set_geometry_corrections(on)` and the
flag of C set to `on` (getter `consistent_pressure_gradient()`). With the flag on, both cell pressure gradients of `piso`
(`grad_p` of the predictor and `grad_new` of the correction) are C (`GeometryCorrections::consistent_pressure_gradients(&g,
p, &pbc, corr_conn)`, public so that the operator checks call the same code). A step with the flag on and the corrections off
is refused, and a step with the flag and L both on is refused (one gradient at a time), each message naming ADR 0387. Off
(the default) is the solver as it is, bit for bit, with the corrections and L on or off. The library changes: in
`crates/rcfd-fv/src/overset.rs` the field, the setter and getter, the two refusals, the function C, the two call sites of
`piso` (a third branch beside L's); `GeometryCorrections::gradients` keeps its arithmetic (its fringe and hole part may be
moved into a private helper that both use, with the same operations in the same order); in `crates/rcfd-fv/src/gradient.rs`
`solve_masked` becomes `pub(crate)` (no change of its body). Nothing else: C4's pressure gradients, the velocity gradients and
L are not touched.

**What C does not change.** C acts only on the cell pressure gradient of a cell with a boundary face whose condition prescribes
the normal gradient; the interior first-order error of the least-squares gradient on B3d (ADR 0382 K4: `2.86e-2` against the
planar `1.60e-2`, relative) stays as with L. On the component (whose boundary is the overset fringe, not a wall) C equals L.

## The runs

Set-ups exactly as ADR 0382 (V, PS, B3, B3d, `P(t)`, 40 steps of `dt = 0.005`, the boundary values `u_ex` at the face centres
before each step, `p_ref` at cell 0) and ADR 0367 (the shear F and the uniform flow). The opt-in is set right after
`OversetSolver::new`.

- **P3oC, P3qoC, W3oC, W3qoC**: V on B3 / B3d, default / quadratic interpolation, `set_consistent_geometry_corrections(true)`.
- **P3, P3q**: V on B3 with the default solver (no opt-in).
- **W3oL, W3qoL**: V on B3d with the corrections and L (ADR 0382's runs, repeated in this test process for C0 and the share).
- **B0, BdL, BdC**: V on the background alone at rest (no component, no coupling; `OversetSolver::new` with `None`, stepped by
  `move_mesh` at the same points and `advance_mesh`, as ADR 0367's `single_run`, the six patches getting `u_ex(x_f, k dt)`
  before step `k`): B0 the planar background with the default solver; BdL the warped background with the corrections and L;
  BdC the warped background with C.
- **SP3oC, SP3qoC, SW3oC, SW3qoC, SP3, SP3q**: ADR 0367's linear shear F (its runs P3o, ..., P3q) with C in place of the
  corrections alone (the last two with the default solver, as there).
- **SU3oC, SU3qoC**: ADR 0367's uniform flow on B3d with the component moving, default / quadratic, with C.
- **SSBoC**: ADR 0367's SBo (the shear on the warped background alone at rest) with C.

`E` of a run as ADR 0382 (V) or ADR 0367 (F), over the non-hole cells and the steps. Printed for every V run as ADR 0382, and
in addition the largest error of the background calculated cells **with a boundary face** (`BCw`) and **without** (`BCi`).

## Arithmetic before this registration (disclosed, by hand)

- The size of the wall-row error with L on B3d: a wall cell has four corners on the box and four displaced by up to `0.015`
  per coordinate (uniform), so its centre (the mean of eight corners for this estimate) is offset per tangential coordinate
  by `4 / 8` of a mean of four uniform displacements (standard deviation `0.0087 / 2`): about `0.0022`, so `abs(t_b)` rms
  `0.003`, at most about `0.007`; `dn` about `0.05`. The normal component's error is about `abs(c_t) abs(t_b) / dn` times the share of
  the wall row in the normal fit (`w_b dn^2 / (w_b dn^2 + w_n h^2) = 1 / 2` for `dn = h/2`): `0.654 * 0.005 / 0.05 / 2 = 0.03`,
  up to `0.05`; the drift of `u_z` over `t = 0.2`: `0.006` to `0.01`, of the order of W3oL's and W3qoL's BC `5.44e-3` (the
  drift is shortened at the outflow, where the fluid leaves). This supports reading 2 above; it is an estimate, not a
  measurement.
- With C the wall rows are exact for the linear part, and the wall cells keep the interior first-order error: the class BC of
  W3qoC is then the interior one, which I estimate from K4's excess of the least-squares error over the planar one (`(2.86 -
  1.60) e-2 * 0.654 = 8e-3` at most, rms about a third) drifting over `t = 0.2`: about `1.6e-3` above B3's BC (`1.24e-3`),
  `BC(W3qoC)` between `1.5e-3` and `3.5e-3`.
- The component classes do not change from L (C equals L there): `CC(W3qoL) = 3.28e-3`, `CF(W3oL) = 5.83e-3`. So `E(W3qoC)` is
  about `max(3.3e-3, BC)`, between `3.3e-3` and `3.8e-3`, and `E(W3oC)` about `max(5.83e-3, BC)` = `5.8e-3`. With `E(P3qoC)`
  and `E(P3q)` equal to ADR 0382's `2.335e-3` to round-off and `E(P3oC) = E(P3) = 4.797e-3`: **A1, A1d about `1.22`; A2, A2d
  between `1.4` and `1.65`**, under the limit `2`. If reading 2 is wrong and BC stays at `5.44e-3`, A2 misses again at `2.33`.
- `E(B0)`: the planar background alone, the BC class of P3q (`1.24e-3`) to P3 (`1.52e-3`); `E(BdL)` near `5.4e-3` (if the
  remainder is the background's own), `E(BdC)` between `1.5e-3` and `3.5e-3`.
- The shear F and the uniform flow: their exact pressure is constant (the computed one below `6e-4` of `abs(U0)^2`, ADR 0367),
  so C changes them by round-off-sized pressure gradients only: the shear ratios near ADR 0367's `1.143` and `1.153`, the
  uniform flow near `1e-14`, SSBoC near `1.5e-4`.
- Operator level on B3d (below, K checks): plain least squares with the Neumann values `p_c + dn G` of a linear field off by
  `abs(c_t) abs(t_b) / dn / 2`, about `0.02` to `0.07` of `abs(c)`; C round-off.
- Cost: ADR 0382's runs took 6 to 9 s each, the shear runs of ADR 0367 7 to 14 s; 8 runs of V, 3 background runs (a third of
  the cells), 9 runs of F: about 2.5 minutes of release spread over the tests; no test above two minutes alone.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f487.rs`, release (`cargo test --release -p rcfd-fv --test overset_f487`). No
existing test is edited.

### A. The acceptance

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1 | default interpolation: `E(W3oC) / E(P3oC)` | `<= 2` | `1.22` |
| A1d | default interpolation: `E(W3oC) / E(P3)` | `<= 2` | the same |
| A2 | quadratic: `E(W3qoC) / E(P3qoC)` | `<= 2` | `1.4` to `1.65` (`2.33` if reading 2 is wrong) |
| A2d | quadratic: `E(W3qoC) / E(P3q)` | `<= 2` | the same |
| A3 | continuity of the calculated cells, every step of P3oC, P3qoC, W3oC, W3qoC | `<= 1e-9` each | `2e-14` |
| A4 | ADR 0367's shear with C: `E(SW3oC) / E(SP3oC)`, `E(SW3oC) / E(SP3)`, `E(SW3qoC) / E(SP3qoC)`, `E(SW3qoC) / E(SP3q)` | `<= 2` each | `1.14`, `1.15` |
| A4c | continuity every step of SP3oC, SP3qoC, SW3oC, SW3qoC | `<= 1e-9` | `2e-14` |
| A4u | ADR 0367's uniform flow with C, SU3oC and SU3qoC, every step, the non-hole cells: velocity `max abs(u - U0) / abs(U0)`, pressure `max abs(p) / abs(U0)^2`; continuity | `<= 1e-10`, `<= 1e-10`; `<= 1e-9` | `1e-14`, `1e-13`; `1e-14` |
| A4s | ADR 0367's K7, K7p with C (SSBoC): `E`; the largest `abs(p) / abs(U0)^2` | `<= 5e-4`; `<= 1e-3` | `1.5e-4`; `5e-5` |
| A6 | the share of the warped background in the remainder, named: for X = L and X = C, the remainder `R_X = E(W3qoX) - E(P3q)` (the excess on B3d over B3, quadratic) and the share `rho_X = (E(BdX) - E(B0)) / R_X` (when `R_X > 0`); printed with the default-interpolation counterparts (`E(W3oX) - E(P3)`), with `BCw` and `BCi` of every V run and of BdL, BdC | named, no limit | `rho_L` near `1` (the remainder is the background's own, at its walls); `rho_C` below `rho_L` |

### C. Set-up and refusals

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| C0 (set-up) | C acts: `abs(E(W3qoC) - E(W3qoL)) / E(W3qoL)` | `>= 1e-6` | `0.3` to `0.4` |
| R1 | a step with the flag of C on and the corrections off (switched off after `set_consistent_geometry_corrections(true)`) is refused, naming ADR 0387 | refused | |
| R2 | a step with the flag of C and L both on is refused, naming ADR 0387 | refused | |
| U4 | kind changes over the steps, P3oC and W3oC | `>= 20` each | `99`, `92` |
| Q1 | P3qoC, W3qoC, every step: share of the fringe cells with a quadratic stencil | `>= 0.95` | `1.0` |
| U5 | unconverged linear solves of every run | printed | `0` |

### K. The gradient at operator level

On the warped background B3d, the planar background B3, and the warped component (at `P(0)`). `c = (0.3, -0.7, 0.4)`, `p = 2 +
c . x`. A pressure condition built in the test: every boundary face a prescribed normal gradient `(a, b, c, d) = (1, dn G, 0,
G)` with `G = n . c` (the box faces are planar and stay on the box, so this `p` satisfies it exactly).

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | C of `p` with that condition, warped background: `max_c abs(g_c - c) / abs(c)` | `<= 1e-10` | round-off |
| K2 (negative control) | plain least squares (`GeometryCorrections::gradients` with `boundary_values_pbc` of that condition), warped background | `>= 1e-3` | `0.02` to `0.07` |
| K3 | the same plain least squares on the planar background | `<= 1e-10` | round-off (`d_b` normal) |
| K4 | C against plain least squares on the planar background and the planar component, with the condition of the step (`pressure_bc` of V's fixed velocities: zero gradient), for `p_ex` of V: `max_c abs(g_C - g_LS) / max abs(grad p_ex)` | `<= 1e-12` | round-off |
| K5 | `p_ex` of V with the zero-gradient condition (exact for V), warped background, the cells with a boundary face: `max abs(g_C - grad p_ex)` over `max abs(g_LS - grad p_ex)` (both printed, also for the interior cells) | `<= 0.7` | `0.3` to `0.5` |
| K6 | C with that zero-gradient condition and with exact boundary values agree on the interior cells of the warped background (bit for bit: no boundary row) | equal | equal |

### D. The defaults and the earlier opt-ins

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| D1 | ADR 0367's `d1_defaults_fingerprint_equals_the_base_tree` (eight default fingerprints), run unedited on the final tree | passes | C behind one flag |
| D1b | ADR 0382's `d1b_opt_in_fingerprint_equals_the_base_tree` (the corrections without L), unedited | passes | |
| D1c | two fingerprints of the corrections with L (5 steps of V on B3d, default and quadratic interpolation, as D1b with L on), recorded by a record test on the tree before any library change of F-487 and asserted on the final tree | equal, both | L's path keeps its arithmetic |
| D2 | `cargo test --release -p rcfd-api --test baseline_hashes`, unedited | passes | |
| D3 | the existing overset test files of `rcfd-fv` (`overset_f134`, `_f134_records`, `_f314`, `_f334`, `_f335`, `_f335_records`, `_f336`, `_f338`, `_f349`, `_f353`, `_f357`, `_f359`, `_f368`, `_f445`, `_f450`, `_f472`, `overset_motion_f442`) and `cargo test --release -p rcfd-fv --lib`, unedited, their ignored tests left ignored | pass | |

### Mutations (scratch copy, not committed)

Each alone, applied by a script with exact replacements that must occur once to a `git archive` copy of the final tree in
`E:/projects/cfd/w49i-work/f487-scratch` (own target directory); the named test is run once; the named check must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the normal rows not used: a gradient-type boundary face keeps the full `d_b` with the value `boundary_values_pbc` | K1 (and C0) |
| MU2 | the flag of C on in `OversetSolver::new` | D1 (the default steps refused), D1c |
| MU3 | `piso` with the flag takes `GeometryCorrections::gradients` (L's gradient) | C0 |
| MU4 | the increment of a normal row with the opposite sign (`-dn G`) | K1 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### Reading of a missed acceptance ratio (fixed now)

For each of A1, A1d, A2, A2d that misses: the class and place of the largest error of the B3d run are named (`BCw`, `BCi`,
`BF`, `CC`, `CF`); if it is `BCi` or `BCw` and `rho_C >= 0.5`, the cause is named as **the warped background's own error**
(with `BCw` against `BCi`: at its walls or inside), else as **the coupling with the component** (the classes BF, CC, CF);
reported without a further diagnosis by this item.

### Classes

A miss of A1, A1d, A2 or A2d is kept as an ignored `*_known_miss` test with the limit intact and read by the rule above. A miss
of A3, A4, A4c, A4u, A4s, R1, R2 or D1 to D3 is a defect of the opt-in or of its isolation (reported; the defaults are never
changed to meet a check). A miss of K1, K3 to K6 is a finding against the derivation (reported, not tuned). A miss of C0, U4,
Q1 or the control K2 is a defective set-up, corrected by an amendment before any corrected run.

## Predictions (mine, before any run)

K1, K3, K4 near `1e-14`; K2 near `0.04`; K5 near `0.4`; K6 equal. `E(P3oC) = E(P3) = 4.797e-3`, `E(P3qoC) = E(P3q) = 2.335e-3`
(ADR 0382's numbers to round-off); `E(W3oC)` near `5.8e-3` (A1 `1.22`), `E(W3qoC)` near `3.5e-3` (A2 near `1.5`), the largest
error of W3qoC in the component (CC); A3 near `2e-14`; the shear ratios near `1.14` and `1.15`, the uniform flow near `1e-14`,
SSBoC near `1.5e-4`; `E(B0)` near `1.3e-3`, `E(BdL)` near `5e-3` with its largest error in a wall cell (`BCw`), `E(BdC)` near
`2.5e-3`; `rho_L` near `1.2`; C0 near `0.35`; D1, D1b, D1c equal; D2, D3 pass; the mutations fail their checks.

## Decision rule

- **done**: A1, A1d, A2, A2d, A3, A4, A4c, A4u, A4s met; the share A6 named; D1, D1b, D1c, D2, D3 met; C0, R1, R2, K1 to K6,
  U4, Q1 met; the control K2 held; MU1 to MU4 caught by their registered checks.
- **partial**: A3, A4c, A4u and D1 to D3 met, the share named, and any other registered check missed (every missed acceptance
  ratio read by the rule above), or a mutation not caught by its registered check.
- **todo**: A3 or one of D1 to D3 missed, or the runs could not be made.

## Limits of this registration

- One flow with a varying pressure, one warp, one resolution, one motion, 40 steps; no order of convergence.
- C removes the inconsistency of the wall rows only; the interior first-order error of the least-squares gradient on an
  irregular stencil stays (a gradient exact for quadratic fields is not attempted). C4's pressure gradients and the velocity
  gradients of the corrections are not changed.
- C is an opt-in with the corrections; it is not combined with BDF2, the prescribed fringe flux or `rhie_chow_step_consistent`
  (the corrections refuse them). It is not proposed as a default.
- `set_geometry_corrections` keeps its behaviour (existing fingerprints and checks cover it); no default, golden or hash
  table changes.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
