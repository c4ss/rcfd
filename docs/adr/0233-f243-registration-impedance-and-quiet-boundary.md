# ADR 0233: F-243 registration: impedance boundary and quiet boundary of the compressible solver, against closed forms

Status: **REGISTERED 2026-11-12**, written before any code of F-243 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0006](0006-compressible-boundary-conditions.md) (F-062, the characteristic boundary conditions),
[ADR 0066](0066-wave32b-aeroacoustics.md) (wave 32B, the part of F-243 that exists),
[ADR 0207](0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md) (F-258, characteristic conditions
on moving faces).

Number: the lead gave 0233. Branch `f243-impedance-boundary` from main 0d250c75. Worktree `E:/projects/cfd/w49i-work/f243`.

## What was run and read before this text (disclosed)

- **Read:** the F-243 and F-062 entries of `docs/roadmap.json`; ADR 0006 in full; the F-062 result lines of `docs/ROADMAP.md`
  (check B: fixed-pressure outlet 0.941, outlet with `sigma = 0` "0.0000", the post-hoc sigma sweep); the S1/S2 lines of
  ADR 0207 (non-reflecting outlet, `sigma = 0`, on a moving face: `R = 3.84e-4`); `crates/rcfd-fv/src/compressible_bc.rs`,
  `crates/rcfd-fv/src/compressible.rs` (the `CompBc` enum, `flux_sum`, `static_face_state`, `characteristic_face_state`,
  `max_dt`, `run_to`), `crates/rcfd-fv/tests/compressible_bc.rs`; `crates/rcfd-api/src/comp.rs` (`check_characteristic`,
  `characteristic_bc`), the `compressible_bc` field of `crates/rcfd-api/src/schema.rs`, the head of
  `crates/rcfd-api/tests/case_hashes_w48u.rs`; the outline of `docs/acoustics.md`; the heads of ADR 0224 and ADR 0226 (style).
- **Searched:** `docs/adr` and `crates` for "non-reflecting", "nonreflecting", "characteristic", "impedance", "NSCBC". The word
  "impedance" does not occur in any library source file.
- **Arithmetic by hand, no rcfd code and no script:** the closed-form values in the check tables below (reflection
  coefficients, standing-wave ratios) and the estimates under each limit. Two of the F-062 numbers above are used in the
  estimates (they are earlier registered results, not runs of this job).

No library or test code of F-243 existed when this text was written, nothing was built and no check below was run.

House rules: limits are fixed here and are not changed after a run; a missed limit becomes an ignored `*_known_miss` test with
the limit intact and the measured number recorded, and is diagnosed, not tuned away; a defective set-up gets a corrected test
by an amendment committed before the corrected run, the old test staying as an ignored record; each registered run is made
once; defaults stay bit-identical; `baseline_hashes.rs`, the hash records and the goldens are not touched; no new third-party
crate, no unsafe code; unsigned commits, no trailers, no push.

## What F-243 asks

Acceptance (binding): "Impedance-tube reflection coefficient matches the closed form; a quiet boundary reflects a pulse by less
than 1 percent". The note says the acceptance is not met: "no impedance boundary, no quiet boundary, no wave solver, no modal
export". The title also names non-reflecting forcing, broadband-noise estimates, a wave-equation solver with sponge layers and
modal export; the acceptance text names none of these.

## What the code read shows (main 0d250c75)

1. **A quiet boundary exists, from F-062.** `CompBc::NonReflecting { p, sigma }` with `sigma = 0` returns the interior state as
   the face state, and the solver takes the physical flux of it. In the boundary cell the flux of the incoming normal
   characteristic then enters and leaves with the same value, so its normal-derivative term is zero: this is Thompson's
   non-reflecting condition (Thompson 1987). ADR 0006 check B registered `R <= 0.02` for it and measured "0.0000" on a 400-cell
   tube; ADR 0207 S2 registered `R <= 1e-2` on a moving outlet and measured `3.84e-4`. The roadmap note's "no quiet boundary" is
   therefore out of date in part: what is missing is a check of the F-243 limit (below 1 percent) registered for F-243, a
   statement of what happens at oblique incidence, and a quiet boundary that holds the ambient pressure (with `sigma = 0` no
   pressure is imposed, so a domain bounded by it keeps any uniform pressure offset for ever; ROADMAP, F-062: "it cannot hold a
   back pressure").
2. **No impedance boundary exists.** The reflecting boundaries are `Slip` (rigid, `R = +1`) and `PressureOutlet` (pressure
   release, `R = -1`). `NonReflecting` with `0 < sigma < 1` is not an impedance: its reflection depends on the cell size and
   the pulse length (ADR 0006 known miss, 0.461 at `sigma = 0.02`).
3. **Reach from the case schema.** `boundaries.<patch>.compressible_bc` is an enumeration in `schema.rs`; `comp.rs` maps it to
   `CompBc` with the keys `pressure_value`, `reflection_value` and others. `Case::to_toml` writes keys at their defaults, and
   `case_hashes_w48u.rs` compares that text for every case file with a record made at 6867e40; each schema key added since then
   needed an edit of that test (ADR 0163, 0181, 0183, 0190, 0193). An impedance needs a new key, so wiring it needs such an edit.
   This was read, not tried.

So the job is: the impedance boundary (library), the closed-form acceptance of the impedance boundary and of the quiet
boundaries, and nothing in the case schema (decision D6).

## Derivations

Notation: ambient state `rho0`, `p0`, sound speed `c = sqrt(gamma p0/rho0)`, at rest; outward unit normal `n` of the boundary;
`u_n = u.n`; perturbations `p'`, `u'`. Linear acoustics (Pierce 1989, ch. 1; Kinsler et al. 2000, ch. 5): `p'_t + rho0 c^2 div u'
= 0`, `rho0 u'_t + grad p' = 0`. The normal characteristic variables are `w+ = p' + rho0 c u_n'` (travelling towards the
boundary, outgoing) and `w- = p' - rho0 c u_n'` (travelling into the domain, incoming).

**DR1. Normal-incidence impedance wall.** A locally reacting wall with real specific acoustic impedance `Z` imposes `p' = Z
u_n'` at the wall. An incident plane wave `p'_i = A f(t - x/c)` has `u'_i = p'_i/(rho0 c)`; the reflected wave `p'_r = R A f(t +
x/c)` has `u'_r = -p'_r/(rho0 c)`. At the wall `A (1 + R) = Z A (1 - R)/(rho0 c)`, so

    R = (Z - rho0 c)/(Z + rho0 c).

`Z = 0` gives `R = -1` (pressure release), `Z = rho0 c` gives `R = 0`, `Z -> infinity` gives `R = +1` (rigid). `R` is real and
independent of frequency, so a pulse is reflected without change of shape, scaled by `R`. The wall pressure is `A (1 + R)`.
(Same result: Pierce 1989, sec. 3-3; Kinsler et al. 2000, ch. 6. Equation numbers are not quoted: the repository holds no copy.)

**DR2. Oblique incidence on an impedance wall.** Wall at `x = 0`, domain `x < 0`, incidence angle `theta` from the normal:
`p' = A [e^{i(k x cos + k y sin - omega t)} + R e^{i(-k x cos + k y sin - omega t)}]`, with `u_n' = cos (p'_i - p'_r)/(rho0 c)`.
`p' = Z u_n'` at `x = 0` gives

    R(theta) = (Z cos(theta) - rho0 c)/(Z cos(theta) + rho0 c).

For the matched wall `Z = rho0 c`: `R = (cos - 1)/(cos + 1)`, negative, zero only at `theta = 0`.

**DR3. Thompson's condition at oblique incidence.** With the face state equal to the interior state the boundary cell obeys
`w-_t + rho0 c^2 v'_y = 0` (the transverse term stays, the normal-derivative term of `w-` is dropped). With the fields of DR2:
`w- = A [(1 - cos) + R (1 + cos)]`, `v' = sin A (1 + R)/(rho0 c)`, `d/dt -> -i omega`, `d/dy -> i k sin`, `omega = c k`:
`-(1 - cos) - R (1 + cos) + sin^2 (1 + R) = 0`, so

    R(theta) = (1 - cos(theta))/(1 + cos(theta)),

positive: the same size as the matched wall of DR2, the opposite sign. (Size as the first absorbing condition of Engquist and
Majda 1977.) If the boundary cell keeps a fraction `alpha` of the normal derivative (a limited reconstruction can do that),
`w-_t - alpha c w-_x + rho0 c^2 v'_y = 0` gives the same `R` for every `alpha != 1`; this was worked through by hand.

Both quiet boundaries therefore pass 1 percent only for `(1 - cos)/(1 + cos) < 0.01`, that is `theta < 11.4 degrees`. This is a
property of every condition built on one-dimensional characteristics and is registered as a limit, with its closed form checked
(checks O).

**DR4. Standing-wave ratio.** For a steady tone in front of the wall of DR1, at distance `s` from it, `|p'|(s) = A sqrt(1 + R^2
+ 2 R cos(2 k s))`. The ratio of the largest to the smallest amplitude is `SWR = (1 + |R|)/(1 - |R|)`; for real `Z > rho0 c` it
is `Z/(rho0 c)` with the pressure maximum at the wall, for `Z < rho0 c` it is `rho0 c/Z` with the pressure minimum at the wall
(the impedance-tube method; Kinsler et al. 2000, ch. 10).

**DR5. The face state (finite amplitude).** The solver's characteristic conditions take the interior cell state `(rho_i, u_i,
p_i)` and return a face state whose physical flux is used. For the impedance wall the face state keeps the entropy, the
tangential velocity and the outgoing Riemann invariant `u_n + 2a/(gamma - 1)` of the interior (the exact simple-wave relations
that `pressure_outlet` already uses; Toro 2009, ch. 3) and satisfies `p_f - p_ref = Z (u_n,f - w_n)`, `w_n` the normal speed of
the face (zero on a fixed mesh). With the unknown `u = u_n,f` in the frame of the face:

    a_f(u) = a_i - (gamma - 1)/2 (u - u_n,i),   p_f(u) = p_i (a_f/a_i)^(2 gamma/(gamma - 1)),
    g(u) = p_ref + Z u - p_f(u) = 0,            g'(u) = Z + rho_f a_f > 0.

`g` is strictly increasing for `Z >= 0`, so the root is unique. Linearised about the ambient state this gives `w+_f = w+_i` and
`w-_f = R w+_i` with the `R` of DR1. In the boundary cell the incoming characteristic then relaxes towards `R w+` with the time
constant `dx/c`, which is consistent with `p' = Z u_n'` at the wall.

## Decisions

- **D1. One new boundary, `CompBc::Impedance { p, z }`**: `p` the ambient (reference) static pressure, `z` the real specific
  acoustic impedance in Pa s/m, `z >= 0` and finite. Face state by DR5 in a new function
  `compressible_bc::impedance_wall(gas, p_ref, z, interior, n)`.
- **D2. Solution of `g(u) = 0`**: Newton's method started from the linearised root `u_n,i + (p_i - p_ref - Z u_n,i)/(Z + rho_i
  a_i)`, safeguarded by a bracket (upper end the vacuum speed `u_n,i + 2 a_i/(gamma - 1)`, lower end `u_n,i - 2^k a_i` for the
  first `k` with `g < 0`); a step that leaves the bracket is replaced by its midpoint. Fixed iteration cap, no randomness.
  `z == 0.0` returns `pressure_outlet(gas, p_ref, 1.0, ..)` itself, so a zero impedance is the existing pressure outlet bit for
  bit. A supersonic outflow (`u_n,i >= a_i`) returns the interior state, as `pressure_outlet` does. If `g` is not positive at
  the vacuum speed (an inflow faster than five times the sound speed through the wall) the interior state is returned.
- **D3. Moving faces**: `characteristic_face_state` evaluates the impedance wall in the frame of the face, like the pressure
  outlet (ADR 0207 M3), so the relation reads the normal velocity relative to the wall.
- **D4. Quiet boundaries**: two, both kept. (a) the existing `NonReflecting { sigma: 0 }` (Thompson), which imposes nothing;
  (b) the new `Impedance { p, z: rho0 c }` (matched wall), which sets the incoming characteristic to the ambient state and so
  holds the ambient pressure. No new variant for (b); `rho0 c` is the caller's number.
- **D5. Refusal**: `CompressibleSolver::set_bc` refuses an `Impedance` whose `z` is negative or not finite or whose `p` is not
  positive and finite. (A rigid wall is `Slip`.)
- **D6. No case-schema wiring in this job.** By point 3 of the code read, wiring needs a new key and with it an edit of
  `case_hashes_w48u.rs`; that is left to an approved follow-up step. No file of `rcfd-api` is edited. Taken on the maintainer's
  standing instruction of 2026-11-11 (take the recommendation); the alternative, reusing `reflection_value` as the reflection
  coefficient, was rejected because that key is limited to `>= 0` (no soft wall) and has the default 0.02.
- **D7. Real, frequency-independent impedance only.** A complex or frequency-dependent impedance needs a time-domain model with
  memory (for example Tam and Auriault 1996) and is not built.

Code: `crates/rcfd-fv/src/compressible_bc.rs` (the function and its unit tests), `crates/rcfd-fv/src/compressible.rs` (the
variant, the two face-state matches, the refusal), a new test file `crates/rcfd-fv/tests/impedance_f243.rs`. No existing test
file is edited.

## Checks and limits (registered)

Common to all runs: `gamma = 1.4`, `rho0 = 1`, `p0 = 1/1.4`, `c = 1`, `Z0 = rho0 c = 1`; solver defaults (HLLC, MUSCL van Leer,
SSP-RK3, CFL 0.4), every step `max_dt()`; perturbation amplitude `eps = 1e-4 p0` unless stated; initial waves are exact
isentropic simple waves: `rho = rho0 (p/p0)^(1/gamma)`, `a = c (p/p0)^((gamma-1)/(2 gamma))`, `u = 2 (a - c)/(gamma - 1) d`,
`d` the unit propagation direction. `R_cf` is the closed form.

### A. Face-state algebra (fast unit tests, `compressible_bc.rs`)

| id | check | limit |
|---|---|---|
| A1 | `z = 0`: `impedance_wall` equals `pressure_outlet(sigma = 1)`, 20 pseudo-random subsonic states | bit for bit |
| A2 | 20 states x `z/(rho_i a_i)` in {0.1, 1, 10}: residual `|p_f - p_ref - z u_n,f|/p_i`; entropy `p/rho^gamma`; invariant `u_n + 2a/(gamma-1)`; tangential velocity | each `<= 1e-12` relative |
| A3 | linear limit: perturbations of size `1e-6` about the ambient state with `(p', u_n')` in three mixes, `z/Z0` in {1/3, 1, 3, 19}: `(w-_f)/(w+_i)` against `R` of DR1 | `<= 1e-5` absolute |
| A4 | `z = 1e12 Z0`: `|u_n,f| / a_i` | `<= 1e-10` |
| A5 | supersonic outflow returns the interior state | exactly |
| A6 | `characteristic_face_state` with `wn != 0` equals the static state of the interior shifted by `-wn n`, shifted back; with `wn = 0` equals `impedance_wall` | `1e-14` relative; bit for bit |
| A7 | `set_bc` refuses `z < 0`, `z = NaN`, `z = inf`, `p <= 0` and accepts `z = 0` | error / ok |

Estimates: A2 Newton converges to round-off (`g'` is bounded below by `z`); A3 the nonlinear terms are of the order of the
perturbation, `1e-6`.

### I, Q. The tube with a pulse (fast, `impedance_f243.rs`): acceptance

Set-up P: `box_2d(1600, 1, 1.0, 1/1600)`, initial right-running pulse `p - p0 = eps exp(-((x - 0.5)/0.05)^2)`, `xmin`
`Fixed(ambient)`, `xmax` the boundary under test, run until `t >= 1.0`. Probe: the first cell with centre `x > 0.75`. `A_inc` =
the value of `p - p0` at the step where `|p - p0|` is largest among the steps with `t < 0.5`; `A_ref` likewise among `0.5 <= t
<= 1.0`; `R = A_ref/A_inc` (signed). The incident peak passes the probe at `t = 0.25`, the reflected one at `t = 0.75`; at `t =
0.5` both are 5 widths away (`e^-25`).

| id | `xmax` | `R_cf` | limit |
|---|---|---|---|
| I1 | `Impedance z = 0` | -1 | `|R - R_cf| <= 0.01` |
| I2 | `Impedance z = Z0/3` | -0.5 | `|R - R_cf| <= 0.01` |
| I3 = Q2 | `Impedance z = Z0` (matched, quiet) | 0 | `|R| <= 0.01` |
| I4 | `Impedance z = 3 Z0` | +0.5 | `|R - R_cf| <= 0.01` |
| I5 | `Impedance z = 19 Z0` | +0.9 | `|R - R_cf| <= 0.01` |
| Q1 | `NonReflecting sigma = 0` (quiet) | 0 | `|R| <= 0.01` |

Estimate of the limit: the only error that does not cancel in `R` is the loss of peak height over the 0.5 of travel between the
two passes. F-062 check B measured a loss of 5.9 percent (0.941) over 0.75 of travel with 12 cells per width (25 widths). Here
there are 80 cells per width and 10 widths of travel. Scaled linearly in the cell size and in the travel this is `0.059 x
(12/80) x (10/25) = 0.35` percent of `|R|`, scaled with the square of the cell size 0.05 percent. Nonlinear terms: `eps/p0 =
1e-4`. Time sampling of the peak: `(dt/2w)^2 < 1e-5`. The limit 0.01 is about three times the larger estimate at `|R| = 1`.
For Q1 the earlier measurements are "0.0000" and `3.84e-4`; for I3 the expected value is the order of `eps/p0`.

Q3 (pressure anchoring). `box_2d(400, 1, 1.0, 1/400)`, at rest, uniform overpressure `p = p0 (1 + 1e-3)` (isentropic density),
`Impedance { p0, Z0 }` on `xmin` and `xmax`, run until `t >= 1.5`. Linear theory: each end sends an expansion of half the
offset into the tube and nothing comes back, so after `t = 1` the pressure is `p0`. Measure `D = max |p - p0| / (1e-3 p0)`
over the cells. Limit `D <= 0.01` (estimate: nonlinear remainder of order `1e-3`; the smeared fronts left 200 cells earlier).

### T. The tube with a tone (fast): the standing-wave method

Set-up T: `box_2d(2400, 1, 6.0, 6/2400)` (`dx = 0.0025`), wavelength `lambda = 0.5` (200 cells), initial right-running train
`p - p0 = eps E(x) sin(2 pi x/lambda)`, `E = 1` on `[1.5, 5.0]`, `sin^2` ramps of length 0.5 on `[1.0, 1.5]` and `[5.0, 5.5]`,
zero elsewhere; `xmin` `Fixed(ambient)`, `xmax` under test at `x = 6`; run until `t >= 3.75`. Envelope: for every cell with
centre `x >= 5.25` (within 1.5 wavelengths of the wall) the largest `|p - p0|` over the steps with `1.75 <= t <= 3.75` (in that
window the incident and the reflected plateau both cover those cells). `SWR` = largest / smallest envelope value over the cells.

| id | `xmax` | closed form (DR4) | limit |
|---|---|---|---|
| T1 | `Impedance z = 3 Z0` | `SWR = 3`, pressure maximum at the wall | `|SWR/3 - 1| <= 0.02`; envelope of the wall cell `>= 0.98 x` largest |
| T2 | `Impedance z = Z0/3` | `SWR = 3`, pressure minimum at the wall | `|SWR/3 - 1| <= 0.02`; envelope of the wall cell `<= 1.02 x` smallest |

Estimate: the cell centres miss the node by half a cell, which raises the smallest value by `|R| (k dx)^2/(2 (1-|R|)^2)`-order,
about 0.1 percent here (`2 k s = 0.031`); numerical loss between the wall and the first node (a quarter wavelength of extra
travel) and the drift of the incident amplitude over a quarter period are each below 0.5 percent by the scaling used for I
(200 cells per wavelength); steepening: shock distance `lambda/(pi (gamma+1) M)` with `M = 7e-5` is above 900, the travel is 5.

### O. Oblique incidence in two dimensions (heavy, `#[ignore]`, release profile): the closed forms DR2 and DR3

Set-up O: `box_2d(160, 320, 0.8, 1.6)` (`dx = 0.005`), wall `xmax` at `x = 0.8`, wall point `W = (0.8, 0.8)`; `xi = (x - 0.8)
cos(theta) + (y - 0.8) sin(theta)`; initial plane pulse `p - p0 = eps exp(-((xi + 0.5)/0.04)^2)` travelling along `(cos, sin)`
(8 cells per width); `xmin`, `ymin`, `ymax` `Extrapolate`; run until `t >= 0.65`. Probe: the cell of the column next to the
wall with the smallest centre `y > 0.8`. `P` = the largest `p - p0` at the probe over all steps (the incident peak reaches `W`
at `t = 0.5`; the wall pressure peak is `A (1 + R)`, DR1/DR2). Calibration by the rigid wall, as in a tube measurement: `P_rigid`
from the same run with `Slip` on `xmax` (`R = 1` at every angle by symmetry), and `R = 2 P/P_rigid - 1`.

Why the other boundaries and the start do not matter: every other boundary is at least 0.8 from `W` and signals travel at `c =
1`, so nothing reaches `W` before `t = 0.8`. The initial field lacks the reflected wave where the pulse already overlaps the
wall; a wall point at distance `r` below `W` holds the initial pulse at `(0.5 - r sin)/0.04` widths from its centre and can
reach `W` only if `r <= 0.5`, so the nearest relevant source sits `0.5 (1 - sin)/0.04` widths out: 6.2 widths at 30 degrees
(`e^-39`), 3.7 widths at 45 degrees (`1.5e-6`).

| id | `xmax` | theta | `R_cf` | limit |
|---|---|---|---|---|
| O0 | `Slip` | 30, 45 | `P_rigid/(2 eps)` near 1 | in `[0.90, 1.02]` |
| O1 | `Impedance z = 3 Z0` | 30 / 45 | +0.4441 / +0.3592 (DR2) | `|R - R_cf| <= 0.03` |
| O2 | `Impedance z = Z0` | 30 / 45 | -0.0718 / -0.1716 (DR2) | `|R - R_cf| <= 0.03` |
| O3 | `NonReflecting sigma = 0` | 30 / 45 | +0.0718 / +0.1716 (DR3) | `|R - R_cf| <= 0.03` |

Estimates: O0, loss of peak height over 0.5 of travel at 8 cells per width, by the linear scaling of I: `0.059 x (12/8) x
(12.5/25) = 4.4` percent. O1 to O3: the incident loss cancels in `P/P_rigid`; what remains is the difference between the
boundary closures (the relaxation of DR5 delays the reflected part by about `dx/c`, a relative peak change of the order
`(dx/w)^2 = 1.6` percent of the reflected part, and the rigid wall's mirror state has its own closure error of the same order):
about 0.7 percent of `P/P_rigid`, doubled in `R`: 0.015. The limit is twice that. This estimate is the weakest of this
registration; the O checks are for the oblique closed forms, which the acceptance text does not name (decision rule).

### Negative controls

- NC1: set-up P with `Slip` (the existing reflecting wall): `R >= 0.9`, so it fails the quiet limit of Q1 and Q2 by a factor 90.
- NC2: set-up P with `PressureOutlet { p0 }`: `R <= -0.9`.
- NC3: set-up Q3 with `NonReflecting { p0, sigma: 0 }` on both ends: `D >= 0.99` (the Thompson boundary does not anchor the
  pressure; this is why D4 keeps two quiet boundaries).
- NC4: in O at 45 degrees the quiet boundaries exceed 1 percent: `|R| >= 0.10` for O2 and O3 (the limit of DR3 is real).

### Defaults

`crates/rcfd-fv/tests/compressible_bc.rs`, `compressible_ale_f258.rs`, `compressible_ale_f258_records.rs` (bit records of the
fixed-mesh and ALE steps with the four existing characteristic conditions) and the unit tests of `compressible.rs` pass
unchanged and unedited. No existing boundary's code path is changed: the new variant adds one arm to two matches.

## Mutations (registered; scratch copies, not committed)

Each is applied to `compressible_bc.rs` or `compressible.rs`, then `cargo test -p rcfd-fv --lib compressible_bc` and `cargo
test -p rcfd-fv --test impedance_f243` (the fast tests) are run once.

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | sign of the impedance term: `g = p_ref - Z u - p_f` | A2, A3, I2, I4, I5, T1, T2 |
| MU2 | outgoing invariant dropped: `u_n,f = u_n,i`, `p_f = p_ref + Z u_n,i` | A2, A3, I4 (would give `R = (Z - Z0)/(2 Z0) = 1`), not Q2 |
| MU3 | reference pressure ignored: `p_ref` replaced by `p_i` | A2, I1, I4, Q3 |
| MU4 | moving-face frame dropped for `Impedance` in `characteristic_face_state` | A6 |
| MU5 | `Impedance` returns the interior state (a silent `NonReflecting sigma = 0`) | A2, I1, I2, I4, I5, Q3; not I3 |

A mutation that no fast check catches is reported as surviving.

## Decision rule (registered)

- **Acceptance met** when I1 to I5, Q1, Q2, Q3, T1, T2 and A1 to A7 meet their limits, NC1 to NC3 hold, the defaults tests
  pass and every mutation is caught. Then the acceptance text of F-243 is met in the library.
- **Proposed roadmap status.** `done` needs, beyond "acceptance met", that the boundary can be selected from a case file. By D6
  that step is not part of this job, so the status this job can reach is **partial**, with the note saying which part is met
  and that the wiring waits for approval. If an I, Q, T or A check misses, or a mutation survives, the status is **partial**
  with the miss named and the acceptance "not met". **todo** does not apply (the item is already partial through wave 32B).
- **The O checks and NC4** do not decide "acceptance met" (the acceptance names the tube). A miss there is kept as a
  `*_known_miss` test, diagnosed, and named in the roadmap note.
- The parts of the title that the acceptance does not name (wave-equation solver, sponge layers, modal export, forcing) are not
  judged here; a proposal for them goes to the lead with the results.

## Limits

- Real, frequency-independent impedance; no porous-liner or resonator model, no complex impedance (D7).
- Locally reacting wall on one-dimensional characteristics: the quiet boundaries reflect `(1 - cos)/(1 + cos)` of an oblique
  wave (DR2, DR3); no Giles-type or sponge treatment.
- The matched impedance uses the caller's `rho0 c`; with a mean flow or a different local state the match is off by the ratio
  of the impedances. Checks are in a medium at rest.
- Perfect gas, inviscid solver; the wall has no boundary layer.
- Library only: not reachable from the case schema, the CLI or the MCP tools (D6).
- The checks are verification against closed forms on Cartesian grids; no measurement is compared.

## Sources

- K. W. Thompson, J. Comput. Phys. 68 (1987) 1-24 (characteristic non-reflecting condition).
- T. J. Poinsot, S. K. Lele, J. Comput. Phys. 101 (1992) 104-129.
- B. Engquist, A. Majda, Math. Comp. 31 (1977) 629-651 (reflection of the first absorbing condition at oblique incidence).
- A. D. Pierce, *Acoustics: An Introduction to Its Physical Principles and Applications*, Acoustical Society of America, 1989,
  ch. 1 and sec. 3-3 (specific impedance, plane-wave reflection).
- L. E. Kinsler, A. R. Frey, A. B. Coppens, J. V. Sanders, *Fundamentals of Acoustics*, 4th ed., Wiley, 2000, ch. 5, 6, 10
  (reflection at normal and oblique incidence, standing-wave ratio).
- E. F. Toro, *Riemann Solvers and Numerical Methods for Fluid Dynamics*, 3rd ed., Springer, 2009, ch. 3 (simple waves, Riemann
  invariants).
- C. K. W. Tam, L. Auriault, AIAA J. 34 (1996) 917-923 (time-domain impedance; cited for what is not built).

Every formula used is derived above from the linear acoustic equations or the isentropic simple-wave relations; the books are
cited for the same results, from memory of their content, without equation numbers. No vendor code or documentation was used.

## Open questions (with recommendations)

- Q-A. Case-schema wiring (`compressible_bc = "impedance"`, a new key `impedance_value` in Pa s/m, reference pressure from
  `pressure_value`). Needs the `case_hashes_w48u.rs` normalisation for the new key at its default, as ADR 0163 and others did.
  Not taken (it needs approval). Recommendation: approve it as a follow-up item.
- Q-B. Whether F-243's title parts outside the acceptance text become separate items. Recommendation: yes; proposal with the
  results.
