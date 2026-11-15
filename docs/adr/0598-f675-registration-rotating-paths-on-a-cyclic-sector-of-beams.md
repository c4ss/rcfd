# ADR 0598: F-675 registration: rotating paths on a cyclic sector of beams (the Coriolis matrix of the Beam2 section)

Status: **REGISTERED 2026-11-15, written before any code of F-675 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-675 was written. The files under "What was read" were read, one
entry-path probe was run on the unchanged library (disclosed below: it printed only `Ok` or the error of each call, no number of
any registered check; the probe file was deleted and never committed), and the arithmetic below was done by hand (no script;
disclosed). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-675 (dep F-672), the follow-up proposed by
[ADR 0595](0595-f672-registration-beam2-elements-in-the-cyclic-constraint.md) (its remaining question 2). Number 0598 given by the
lead. Branch `f675-work` from the local main tip d3d178e9 (which holds F-672 `CyclicOptions::beams` and the `rotating.rs` fix of
F-666), worktree `w49i-work/f675`, scratch `w49i-work/f675-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-675 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector on the ADR 0595 wheel sector (stress stiffening and spin softening, and with Coriolis the whirl modes) at two speeds gives the frequencies of each harmonic of RotatingPart on the full ring of beams within 1e-8; the defaults bit-identical.

Clauses: **c0** every limit registered here; **c1** `RotatingSector` on the ADR 0595 wheel sector with stress stiffening and spin
softening (`RotatingSector::modes`) at two speeds gives the frequencies of each harmonic of `RotatingPart` on the full ring of
beams within `1e-8`; **c2** the same with Coriolis, the whirl modes (`RotatingSector::whirl_modes` against
`RotatingPart::whirl_modes`) within `1e-8`; **c3** the defaults bit-identical.

## What was read

`AGENT_RULES.md`; the brief; ADR 0595 (all of it: the wheel sector W, the oblique axis, `N = 6`, the independently built ring
with orientations `R^j v`, D3 the objectivity of the Timoshenko element, the backward residual test because the default relative
residual has a floor `~ c eps lambda_max / lambda_1`; its measured rest frequencies, `k = 0`: 21.57 Hz the lowest); ADR 0561 (the
rotating cyclic paths with shells: D1 the opt-in routes, D2 the Coriolis matrix of the shell director made skew bit for bit and
the lesson of its amendment 2, that `to_global` symmetrises a skew matrix to zero; D4 the list protocol; W0 the vacuity check);
ADR 0573 (the backward-whirl sign lesson; NC1's limit `10 n_q eps omega_max / omega_lo` for the dense solver's rounding); ADR
0593 D5 (`unit_centrifugal_model` keeps the hyperelastic opt-in); ADR 0411 (D1 `C = int rho N^T [a x] N`, `G = 2 Omega C`; D2 the
dense symmetric linearisation; D5 the sector); ADR 0403 (`K_eff = K + Omega^2 (K_sigma,1 - M_perp)`); ADR 0473 (D3 the beam's
spin-softening mass from the section kinematics `u_c + theta x s`, D5 its centrifugal load, D6 the whirl paths keep the solid
routines and refuse a beam); ADR 0417 (the Beam2 frame, the interdependent functions, the consistent mass with rotary inertia
`rho I_z` in the x-y plane, `rho I_y` in the x-z plane, `rho (I_y + I_z)` in torsion). Sources at d3d178e9: `rotating.rs`
(`RotatingOptions`, `unit_centrifugal_model`, `RotatingPart::new`, `with_unit_prestress`, `operator`, `check_model`,
`RotatingSector::new`, `modes`), `rotating/whirl.rs` (`coriolis_matrix`, `coriolis_matrix_with_shells`, `coriolis_assembly`,
`RotatingPart::coriolis`, `whirl_at` of the part and of the sector, `solve_dense`), `beam.rs` (`local_mass`, `to_global`,
`field_rows`, `local_spin_softening_mass`, `element_spin_softening_mass`, `property_or_error`, `model_element_mass`), `shell.rs`
(`coriolis_factors`, `skew`, `element_coriolis`), `cyclic.rs` (`beams_opted_in`, `shells_opted_in`), the test
`sa_cyclic_beams_f672.rs` (its builders are copied into the new test, not edited).

### The audit (the brief: "if a rotating path refuses beams or the option, audit why before changing anything")

**Entry-path probe (disclosed).** A temporary test file built W (with `shells(true).beams(true)`) and the ring of ADR 0595 and
printed only `Ok` or the error of each call on the unchanged library: `RotatingSector::new` (both effects), `modes(1, 30)`,
`modes(0, 30)`, `campbell(&[0, 1], &[0, 30])`, the ring's `RotatingPart::new` and `modes(30)`: `Ok`. `RotatingSector::whirl_modes(1,
30)` with and without `coriolis(true)`, and the ring's `whirl_modes(30)` with and without `RotatingOptions::shells(true)`:
`Element { element: 0, source: NotYetSupported { variant: "Beam2", location: "element routines", package: beam::SOLID_ROUTINES }
}`.

**Why.** Every whirl path calls `RotatingPart::coriolis` (also with `coriolis(false)`: the matrix is assembled and then multiplied
by zero), which assembles `coriolis_assembly`; for a Beam2 element that falls through to `element_mass(element_for(Beam2), ..)`,
the solid-routine placeholder of ADR 0417 D7, which refuses. ADR 0473 D6 left it so on purpose: no Coriolis matrix of the beam's
section kinematics exists. So the modes paths (c1) need no library line (F-672's option already lets them through: test-only),
and the whirl paths (c2) need the Coriolis matrix of the Beam2 element (D2) behind an opt-in route (D1).

**Tests that assert the refusal this lifts.** Searched every test of the workspace naming `Beam2` together with `whirl`,
`coriolis`, `split_modes` or `gyroscopic`, and every test naming `beam::SOLID_ROUTINES`: `sa_rotating_cyclic_shells.rs` NC1
(`full_rotor`, not changed here), `sa_rotor.rs` (the stationary rotor model, its own matrices, not changed), `si_beam.rs`,
`si_beam_loads.rs`, `si_nonlinear.rs`, `si_scaffold.rs` (static, nonlinear and load paths, not changed). None asserts that a
whirl or Coriolis path refuses a beam. Even so the lift is opt-in (D1), so every model without the option keeps its refusal
(NC1, NC2).

## Design

### D1. The opt-in routes

1. **The sector.** With `CyclicOptions::beams` on the model's cyclic constraint (ADR 0595 D1; it implies `shells`),
   `RotatingPart::coriolis` (and so every whirl, whirl-state, whirl-Campbell and split path of `RotatingPart`, `RotatingSector`
   and `gyro` that calls it) assembles the Coriolis matrix with the beam routine of D2 for Beam2 elements and with the shell
   routine of ADR 0561 for Shell4 and Shell3.
2. **The whole part.** A part without a cyclic constraint (the reference ring) takes the beam routine only with a new option
   `RotatingOptions::beams: bool` (default `false`, builder `RotatingOptions::beams(on)`); the shells keep their own option
   (`RotatingOptions::shells`), so `beams(true)` alone gives Beam2 the beam routine and shells the solid placeholder as before.
3. **Unchanged.** The public `coriolis_matrix` and `coriolis_matrix_with_shells` keep their behaviour (Beam2 refused, NC2);
   `gyro::gyroscopic_matrix` likewise. Without either beams option `RotatingPart::coriolis` makes exactly the call it made
   before (the same function, the same arguments): a model without the option executes no new line but the test of two
   booleans.

### D2. The Coriolis matrix of the Beam2 section (first principles)

ADR 0411 D1: the term of the kinetic energy in the rotating frame, `T = 1/2 int rho |w_t + Omega a x (X - p + w)|^2 dV`, bilinear
in `w_t` and `w` is `Omega int rho w_t . (a x w) dV`; with `w = N q` it is `Omega q_t^T C q`, `C = int rho N^T [a x] N dV`, and
Lagrange gives `G = 2 Omega C`. The displacement of the material point at the section position `s = y e_2 + z e_3` of a beam is
(ADR 0473 D3, the kinematics of the consistent mass of ADR 0417) `w = u + theta x s`, `u` and `theta` the fields of the twelve
local unknowns (`field_rows`: rows 0 to 2 the translations, 3 to 5 the section rotations). With the centroid on the axis (`int s
dA = 0`) and principal section axes (`int y z dA = 0`) the cross terms between `u` and `theta` vanish, and

`int_A rho (theta_t x s) . (a x (theta x s)) dA`: with `a x (theta x s) = theta (a . s) - s (a . theta)` and `(theta_t x s) . s =
0` this is `int rho (a . s) (theta_t x s) . theta dA = int rho (a . s) s . (theta x theta_t) dA = (theta x theta_t) . b = theta_t
. (b x theta)`, with `b = int rho s (s . a) dA = rho (I_z a_2 e_2 + I_y a_3 e_3)` (`I_z = int y^2 dA`, `I_y = int z^2 dA`, the
convention of `local_mass`: the x-y plane takes `rho I_z`).

So, in local axes (`a` in local components, `b = (0, rho I_z a_2, rho I_y a_3)`):

`C_l = int_0^L (rho A R_u^T [a x] R_u + R_theta^T [b x] R_theta) dx`,

`R_u`, `R_theta` the rows of `field_rows` at `x`, by the four-point Gauss rule of `local_mass` (the integrands are polynomials of
degree at most 5: exact). The term `[b x]` has no `b_1`: the torsion rotation couples to the bending rotations, the two bending
rotations do not couple to each other. A shell check of the form: with `s = z e_3` only, `b = m2 a_3 e_3` and `[b x] = m2 a_3
[[0, -1, 0], [1, 0, 0], [0, 0, 0]] = m2 W^T [a x] W`, the rotation block of ADR 0561 D2.

**The second-order kinematics adds nothing** (checked here because the linear kinematics is used): with the exact rotation `w = u +
theta x s + 1/2 theta x (theta x s) + O(theta^3)`, the quadratic part of `T` in `Omega` also has `Omega int rho (w_2)_t . (a x (X
- p))`. Its part with the centroid position is zero (`w_2` is linear in `s`, `int s dA = 0`), and its part with `s` is `Omega d/dt
int rho w_2 . (a x s) dA` (`a x s` is constant in time), a total time derivative of a function of `q`, which adds nothing to the
equations of motion. A check by a known case: a thin rigid disc normal to the axis has `s . a = 0`, `b = 0`: no rotary Coriolis
coupling of its tilt in the rotating frame, which is the known rotating-frame result `(I_p - 2 I_d) Omega = 0` for a thin disc
(from memory; the derivation above is what is used).

The global matrix is the congruence `T^T C_l T` (`T` four copies of the frame rotation) **without** the symmetrisation of
`to_global` (the lesson of ADR 0561 amendment 2: `to_global` ends with `(c_ij + c_ji) / 2`, which annihilates a skew matrix), then
made skew in floating point by `shell::skew` (`c_ij <- (c_ij - c_ji) / 2`, `c_ji <- -c_ij`, `c_ii <- 0`). `to_global` is split
into the congruence and the symmetrisation, the same operations in the same order (its result keeps its bits; B1).

**Objectivity** (needed by the sector, as ADR 0595 D3 for the stiffness and mass): the local components of the axis are the same
in sector `j` (`R^j a = a`, the frame `R^j` times the frame of sector 0 when the orientation is `R^j v`), so the element `C` of
sector `j` is `Q C Q^T`, and ADR 0411 D5's argument holds with ADR 0556's six-unknown rows.

**Uniform rigid fields (the closed forms of CB1).** For one element and two rigid motions `w = v + theta x (X - c)`, `w' = v' +
theta' x (X - c)` about the element's midpoint `c` (nodal translations `v + theta x (x_i - c)`, rotations `theta`; the element
reproduces rigid motions exactly), `q^T C q' = int rho w . (a x w') dV`. With `int rho (X - c) dV = 0`:

- translations: `v^T C v' = rho A L v . (a x v')`;
- rotations: `theta^T C theta' = (theta' x theta) . (J_c a)`, `J_c = int rho r r^T dV = rho (A L^3 / 12 e_1 e_1^T + L I_z e_2 e_2^T
  + L I_y e_3 e_3^T)` (the identity of the derivation above with `r = X - c`);
- mixed: `v^T C theta' = 0`.

### D3. The models and the set-up (fixed now)

W and the ring of ADR 0595, rebuilt in the new test by copies of its builders (`sector`, `ring(true)`): axis `n = (1, 2, 2)/3`
through `O = (0.3, -0.2, 0.5)`, `N = 6`, ring and spoke Beam2 blocks, oblique orientations (`R^j v` on the ring), hubs clamped on
six components, `CyclicOptions::new().shells(true).beams(true)`. The ring is built independently (from `P`, not by `full_rotor`,
which refuses beams). No loads (the rotating paths add the centrifugal load at 1 rad/s themselves).

Modal options (ADR 0595): `tolerance 1e-12`, `ResidualTest::Backward`; `opts(m) = RotatingOptions::new(m).modal(modal_options(m))`.
Sector: `RotatingSector::new(&W, &opts(4))` (4 modes per harmonic, 8 entries for a pair); ring: `RotatingPart::new(&ring, n, O,
&opts(30).beams(true))`. Effect sets: **A** both effects (the defaults), **B** pre-stressed modes (`spin_softening(false)`), **C**
spin-softened modes (`stress_stiffening(false)`), **W** = A with `coriolis(true)` (dense whirl solver, the default); the ring of
set W has `beams(true)` (the sector takes the route by its cyclic option, D1.1).

**The two speeds** (by rule, not by search): `Omega in {0.2 omega_1, 0.4 omega_1}`, `omega_1 = sqrt(min_k lambda_{k,1})` the
lowest sector eigenvalue at rest over `k = 0 .. 3` (`modal_analysis_harmonic`, `modal_options(4)`). From ADR 0595's addendum
(21.57 Hz) the speeds are about 27.1 and 54.2 rad/s. Below `0.4 omega_1` set C keeps `K_eff` positive definite: the beam's
`M_perp` is `M` minus a positive semi-definite term (ADR 0473 D3), so `lambda_1(Omega) >= lambda_1 - Omega^2 >= 0.84 lambda_1`; a
loss of stiffness at a registered speed is a set-up failure (reported, not tuned).

### D4. The measures (ADR 0561 D4)

**Frequencies (MA, MB, MC, W).** At each speed and set, per harmonic `k = 0 .. 3` the sector's list (modes: every entry of
`RotatingSector::modes(k, Omega).eigenvalues`, a pair's two entries; whirl: the `omega` of every mode of `whirl_modes(k, Omega)`);
the cut-off `c = (1 - 1e-6) min_k (largest entry of k)`; the sector list = every entry below `c` of every harmonic; the ring list =
the ring's entries below `c` (`RotatingPart::modes(Omega).eigenvalues`, or the `omega` of `RotatingPart::whirl_modes(Omega)`);
counts equal and the ring's largest entry `>= c` (else a set-up failure); after sorting both lists, `max |f_s / f_r - 1|` with
`f` the frequency (`sqrt` of an eigenvalue; the whirl `omega` itself), reported per harmonic as the maximum over its entries (each
sector entry is paired with the ring entry of the same rank in the sorted lists).

## Arithmetic (by hand, disclosed)

1. **Sizes.** W: 30 independent unknowns at `k = 0, 3`, 60 at `k = 1, 2` (ADR 0595); the ring 180 free. Sector `4 x multiplicity`
   entries per harmonic, 24 in all; the ring's 30 cover them (ADR 0595 at rest: the cut-off `2.9e6` below the ring's 30th
   eigenvalue `5.3e6`).
2. **The largest frequency of the whirl problem** (for the dense solver's rounding). Element by element, the ring element (`L =
   0.261` m): axial `12 E / (rho L^2) = 12 x 2e11 / (7850 x 0.0681) = 4.5e9`; torsion `12 G J / (rho (I_y + I_z) L^2) = 12 x 7.7e10
   x 3.7e-7 / (7850 x 6.75e-7 x 0.0681) = 9.5e8`; bending of an interior rotation between two elements `420 E I / (rho A L^4)`, `I =
   5.4e-7`: `6.9e8`. The spoke element (`L = 0.37`): axial `2.2e9`. So `lambda_max ~ 5e9`, `omega_max ~ 7e4` rad/s; with a factor of
   up to 3 for the assembly, `omega_max <= 2e5`. (ADR 0595 arithmetic 3 wrote `9e12` from a formula with a wrong dimension; it
   only made its floor argument more conservative and is corrected here, not there.)
3. **Whirl rounding.** The dense solver works on `omega` directly (the skew matrix `[[G^, B], [-B^T, 0]]`, ADR 0411 D2), so its
   absolute error is `c eps omega_max` with `c <= 10 n_q` (ADR 0573 NC1, from ADR 0569: observed 30 to 350); relative to the
   lowest frequency `omega_lo >= 135` rad/s: ring `10 x 180 x 2.2e-16 x 2e5 / 135 = 5.9e-10`, sector smaller (`n_q <= 60`). The
   sum `<= 8e-10`; expected (`350 eps`) `~ 1e-10`. Margin 12 under `1e-8`.
4. **Modes (sets A, B, C).** At rest ADR 0595 measured `8.2e-13` (backward residual test). At a speed the operator adds `Omega^2
   (K_sigma,1 - M_perp)`: `M_perp` is assembled per element (objective, ADR 0473 D3) and `K_sigma,1` comes from the sector's
   static `k = 0` solution, equal to the ring's to ADR 0595 S0's `6.6e-13`; its share of `K_eff` in a low mode is `<= Omega^2 /
   omega_1^2 x (a stiffening ratio of order 1) <= 0.16`: `<= 1e-13` more. Expected `~ 1e-12`; limit `1e-8` (the acceptance).
5. **Coriolis effect (W0).** At `0.4 omega_1` a `k = 1` pair with in-plane motion splits by `~ 2 kappa Omega`, `kappa` of order
   `0.1 .. 1` (ADR 0411 D3) and `Omega / omega ~ 0.35` for the lowest `k = 1` mode (24.41 Hz): a relative shift of `1e-2 .. 0.3`;
   the out-of-plane modes shift much less (`a x w ~ 0` for `w` along the axis), but the spokes rise along the axis (`0.25` m over
   `0.7` m) and couple both. `1e-4` is far below the expected value and far above rounding.
6. **CB1.** The entries of `C` are `~ rho A L = 8.2` kg; the forms are sums of `~ 150` products of `O(1)` vectors with them:
   `~ 1e-15` relative; the rotary part of the rotation form (`rho L I_z ~ 2.5e-3` against `rho A L^3 / 12 ~ 0.23`) is `1e-2` of it,
   so a wrong section term moves the form by `>= 1e-3` relative. Limit `1e-13`.

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| CB1 | c2 (set-up of the matrix) | one Beam2 element, nodes `(0.2, -0.1, 0.4)`, `(0.5, 0.3, 0.1)`, the ring section of W (`I_z = 4 I_y`), orientation `v_r` of W, steel, axis `n`; `beam::element_coriolis`: the translation form and the rotation form of D2 against their closed forms (relative), the mixed form over `rho A L^2 |v| |theta'|`; `v = (0.3, -0.7, 0.5)`, `v' = (-0.4, 0.2, 0.9)`, `theta = (0.6, 0.1, -0.8)`, `theta' = (-0.2, 0.9, 0.3)` | `<= 1e-13` each | arithmetic 6 |
| CB2 | c2 | the element matrix of CB1, `RotatingSector::part().coriolis()` of W and the ring part's `coriolis()`: skew bit for bit (`c_ij = -c_ji`, `c_ii = 0`) and not zero | exact; non-zero | D2 skew-symmetrisation; the assembly adds `(i, j)` and `(j, i)` in the same order |
| MA | c1 | set A (both effects), each speed: counts equal, per harmonic `max |f_s / f_r - 1|` | equal; `<= 1e-8` | arithmetic 4 |
| MB | c1 | set B (pre-stressed modes), as MA | as MA | arithmetic 4 |
| MC | c1 | set C (spin-softened modes), as MA | as MA | arithmetic 4 |
| W | c2 | set W (whirl, Coriolis on, dense): `RotatingSector::whirl_modes` against the ring's `whirl_modes`, each speed, counts equal, per harmonic `max |omega_s / omega_r - 1|` | equal; `<= 1e-8` | arithmetic 3; the reported `omega_max` (the largest of `frequencies_all`) and `10 n_q eps omega_max / omega_lo` are printed beside it |
| W0 | c2 (vacuity) | at `0.4 omega_1`: the largest `|omega_W / omega_A - 1|` between the sorted sector whirl list of `k = 1` and the sorted `sqrt` of the sector's set-A `k = 1` eigenvalues, over the entries below the `k = 1` cut-off of both | `>= 1e-4` | arithmetic 5 |
| SM | c2 (smoke) | `RotatingSector::split_modes(k, 0.4 omega_1)` (`coriolis(true)`), `whirl_state(k, 0.4 omega_1)` for `k = 0 .. 3`, `whirl_campbell(&[0, 1, 2, 3], speeds)` and `campbell(&[0, 1, 2, 3], speeds)` return `Ok` with finite frequencies | `Ok`, finite | they call the same `coriolis()` and `modes` (D1); no accuracy claimed |
| B1 | c3 | the existing rcfd-fem tests over the changed lines (release): the lib unit tests, `sa_cyclic_beams_f672`, `sa_rotating_cyclic_shells`, `sa_rotating_cyclic_shells_default_bits`, `sa_gyro`, `sa_gyro_default_bits`, `sa_gyro_rayleigh`, `sa_gyro_sector*`, `si_rotating*`, `sa_geometric_struct`, `sa_geometric_struct_bits`, `si_beam`, `si_beam_loads`, `sa_prestress_mixed`, `sa_prestress_mixed_bits`, `sa_disc_whirl_shells` | pass | D1.3, D2 (the split of `to_global` is the same operations) |
| B2 | c3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | as B1 |
| B3 | c3 | (a) a Hex8 box part (`hex_box` 1 x 0.1 x 0.1 m, 8 x 1 x 1, clamped at `xmin`, steel, axis `(0, 0, 1)` through the origin) with `opts(4).coriolis(true)` and with `.beams(true)` added: the `coriolis()` matrices and the `whirl_modes(200)` omegas equal bits; (b) W's Coriolis matrix from `RotatingSector` with `opts(4)` and with `opts(4).beams(true)` (the option is redundant on the sector): equal bits | equal bits | D1: the option is read only for Beam2 elements |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the ring as a `RotatingPart` without `RotatingOptions::beams`: `whirl_modes(Omega_1)` with `coriolis(true)`, and with `shells(true)` | refused as before: `Element { NotYetSupported { variant: "Beam2", package: beam::SOLID_ROUTINES } }` |
| NC2 | the public `coriolis_matrix` and `coriolis_matrix_with_shells` of W | refused as NC1 |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| X1 | the rotation block of the beam Coriolis matrix zero (`b = 0`) | CB1 (rotation form) |
| X2 | `I_y` and `I_z` exchanged in `b` | CB1 (rotation form) |
| X3 | the translation block with `P = I - a a^T` in place of `[a x]` | CB1, CB2 |
| X4 | the congruence by `to_global` (with its symmetrisation) in place of the unsymmetrised one | CB1, CB2 (zero), W0 |
| X5 | `b` from the global components of the axis in place of the local ones (not objective) | CB1, W (sector against ring) |
| X6 | `RotatingPart::coriolis` takes the beam route only when neither beams option is on (inverted guard) | NC1 (`Ok`), W, CB2, SM (refused) |

No mutation is declared equivalent.

## Decision rule

**Done**: every check CB1 to B3 met in its registered run, NC1 and NC2 as stated, every mutation caught by at least one check.
**Partial**: a registered check missed (recorded as a `*_known_miss`), a check met only after a post-run amendment, a check not
run, or a mutation not caught. **Todo**: MA to W not run.

## Limits (what is not covered)

- `full_rotor` and `FullRotor::expand*` with beams (one orientation per block; ADR 0595 D4; its own follow-up), so no expanded-mode
  orthonormality or residual on the ring is checked here; the frequencies are.
- The accuracy of the split modes (ADR 0472/0481), the whirl state and the sparse whirl solvers on beams: smoke only (SM); the
  whirl numbers and directions of the beam whirl modes are not compared (only the frequencies).
- The beam Coriolis matrix is checked by its rigid-field forms (CB1) and by the sector-ring consistency, not against an
  independent whirl solution of a spinning beam structure.
- Offset sections (centroid off the beam axis) and non-principal section axes are not represented by `BeamSection`; D2 assumes
  both.
- The public `coriolis_matrix`, `coriolis_matrix_with_shells` and `gyro::gyroscopic_matrix` stay without beams.
- The warping beam (seven unknowns) is not on these paths; no case key for `RotatingOptions::beams`.
