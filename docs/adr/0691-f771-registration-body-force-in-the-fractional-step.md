# ADR 0691: F-771 registration: body force in the fractional step

Status: **REGISTERED 2026-11-17, written before any code of F-771 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-771 was written and no run of the solver was made; the numbers
quoted from earlier work are those printed in ADR 0672, ADR 0675 and ADR 0687. The one computation made beforehand is a 1-D
model of the start-up channel (below, disclosed, script `w49i-work/f771-scratch/est.py`, not the solver). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-771 (deps F-757), proposed by [ADR 0687](0687-f757-registration-rhie-chow-ddt-correction-on-coupled-faces-in-the-fractional-step-and-with-bdf2.md)
remaining question 1 (Q2 there: the fractional step reads no body force, so its twin check ran from an initial velocity).
Parents: ADR 0687 (F-757: the fractional step's coupled-face `Gamma R + (Gamma / dt) D` with `fractional_step_consistent` and
`coupled_ddt_correction`, `fractional_periodic_implicit`, the test file `coupled_ddt_fractional_bdf2_f757.rs`),
[ADR 0672](0672-f752-registration-pressure-part-of-the-non-orthogonal-correction-on-coupled-faces.md) (the skewed periodic
channel `P`, its unrolled twin `T`, the body acceleration `g = (0.8, -2)`, the hydrostatic start), ADR 0356
(`fractional_step_consistent`), ADR 0037 (the fractional step), ADR 0080 (balanced body force on periodic faces). Number 0691
given by the lead. Branch `f771-work` from ff7e9989 (F-752, F-754, F-757 merged), worktree `w49i-work/f771`, scratch
`w49i-work/f771-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The balanced-force treatment is the repository's own (the PISO step and
the steady kernel: the body acceleration enters the face flux through its face value `a_f . S_f` exactly where the pressure
gradient enters through its compact face difference, and the walls take the pressure gradient that balances it; the idea is
the textbook "balanced-force" momentum interpolation, cited from memory, nothing beyond the repository's formulas is used). The
fractional-step form below is derived from the fractional step's own treatment of the pressure (read 1). The closed form of
the start-up channel is the textbook Fourier series of the diffusion equation with a uniform source and no-slip walls,
derived below.

## What F-771 asks

The acceptance, binding (from the lead's brief):

> The fractional step reads PimpleSolver::body_force behind an opt-in (the predictor's cell term, the balanced-force face term of the intermediate flux and the walls' pressure gradient as in the PISO step), defaults bit-identical; on ADR 0672's channel with its body force the fractional run with coupled_ddt_correction is within a registered limit of its twin on every step where the run without the opt-in is not, and a hydrostatic rest state stays at rest within a registered limit. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** behind an opt-in the fractional step reads `PimpleSolver::body_force`: the predictor's cell term, the balanced-force
  face term of the intermediate flux, the walls' pressure gradient;
- **a2** defaults bit-identical;
- **a3** on ADR 0672's channel with its body force, the fractional run with `coupled_ddt_correction` is within a registered
  limit of its twin on every step where the run without `coupled_ddt_correction` is not;
- **a4** a hydrostatic rest state stays at rest within a registered limit.

**Reading of a3 (registered, as ADR 0675 and ADR 0687).** "The opt-in" of a3 is `coupled_ddt_correction` (the clause names it);
every run of a3 has the new body-force opt-in on. With `G_V(n) = max_c |u_V(n)_c - u_T(n)_m(c)| / U_ref` (ADR 0672's map
`m`), the check is `G_on(n) <= eps` on every step (stronger than "on every step where the control is not"), non-vacuous only
when the control `G_off(n) > eps` on at least one step (binding). The reference is `T` with the opt-in (`T_on`, ADR 0675 Q1).

## What the code read shows (ff7e9989, read only)

1. `pimple/fractional.rs` `step_fractional`: no use of `body_force`; `pressure_bc` without the body force (the walls keep the
   zero pressure gradient). The incremental form's intermediate flux is `phi* = interp(u*) . S + c_f R_f(p^n)` with
   `R_f = interp(grad p^n) . S - (compact difference of p^n) |S|`-type terms: internal faces `c_f = dte` (default) or `Gamma_f`
   (`fractional_step_consistent`, plus `(Gamma_f / dt) K_f`); coupled faces `c_f = dte` or the pair-harmonic `Gamma_f` (with
   `coupled_ddt_correction` too, plus `(Gamma_f / dt) D_f`), `gf = a grad_o + (1 - a) grad_c`; boundary faces with free normal
   velocity (fixed pressure) `dte (grad p_o . S - |S| dp/dn)`; fixed-velocity faces nothing (their flux is the prescribed one).
   This replaces the pressure force carried by the interpolated cell velocity by the compact face one. The Chorin form
   (`incremental = false`) carries no pressure in the predictor and adds no face term.
2. PISO (`step_with`, pimple.rs lines 1833 to 1848, 2054 and 2231 to 2250): (i) where the normal velocity is prescribed and the
   face is not coupled, the pressure condition becomes the gradient `bf.face / |S|` (coefficients `a = 1, b = dn grad, c = 0,
   d = grad`); (ii) the predictor's right-hand side `+ V b_cell`; (iii) `phi_HbyA += c bf.face` on every face, `c` the face
   coefficient of the pressure term (`gamma_p`, or the pair mean with `coupled_body_pair_mean`). In PISO `HbyA` carries
   neither pressure nor body force, so the face value is added alone; on a fixed-velocity face it cancels against the wall
   gradient in the corrected flux.
3. `BodyForce` (simple.rs): `cell` and `face` (`a_f . S_f`), public fields; `uniform(a)` gives `face = a . S`.
4. The fractional step's increment `q` has homogeneous conditions (`homogeneous`: `b = d = 0` on every non-coupled face), so a
   wall gradient set on the pressure condition does not reach the Poisson problem in the incremental form.
5. `step_with` calls `step_fractional` before any hook: the hook's `source_force` is never set on a fractional run.

## Design

### The opt-in

`PimpleSettings::fractional_body_force: bool`, default `false`. Read only by the fractional step, and there only when
`PimpleSolver::body_force` is `Some` (the hook's `source_force` is not read, read 5). With the field `false`, or no body force,
every operation of the fractional step is the previous one. With the field `true` and the Chorin form the step panics
("fractional_body_force: incremental form only"): the Chorin form carries no pressure in the predictor, so the balanced-force
pair of read 1 does not exist there; a refusal is safer than a silently unbalanced force (taken on the standing instruction).

### The three places (body acceleration `b`, face value `bf_f = b_f . S_f`)

1. **Walls' pressure gradient.** After `pressure_bc`, the PISO step's coefficients of read 2 (i) on every fixed-velocity,
   non-coupled face. Shared: the PISO code becomes a public helper `pressure::apply_body_force_wall_gradient(geom, bf, &mut
   pbc)` (the same operations in the same order: the PISO bits unchanged) called by both steps. It acts on the old-level
   pressure's boundary values (so on `grad p^n`), not on the increment (read 4).
2. **Predictor cell term.** `sys.b[c] += V_c b_cell[c][k]` after the pressure term, as in PISO.
3. **Balanced-force face term.** The cell velocity `u*` now carries the body force as it carries `-grad p^n`; the face flux
   takes the face value in place of the interpolated cell value, with the face's pressure coefficient `c_f` of read 1:
   `phi*_f += c_f (bf_f - B_f . S_f)`, `B_f` the interpolation the face uses for `grad p` (internal: `w b_o + (1 - w) b_n`;
   coupled: `a b_o + (1 - a) b_c`; free-velocity boundary: `b_o`); nothing on fixed-velocity faces. Derivation: with
   `u* ~ (V / a_P)(b - grad p) + ...`, `interp(u*) . S` contains `c_f (B_f - G_f) . S`; the existing pressure term turns
   `-c_f G_f . S` into `-c_f (compact p difference)` and this term turns `c_f B_f . S` into `c_f bf_f`, so the face flux of a
   discrete potential force `bf_f = (compact difference of Phi)` and `p = Phi` is exactly zero: rest (a4). The PISO form of read
   2 (iii) is the same statement for a cell velocity `HbyA` without the force. For a uniform `b` (ADR 0672's) `bf_f - B_f . S_f
   = 0` to rounding on every face: the face term then does nothing and the drive is the cell term; the rest test below uses a
   non-uniform discrete potential force so that the face term is tested.

On the twin (a3) every term of a coupled face of `P` is the term of `T`'s internal face (ADR 0687's design for `R_f` and
`D_f`; the body terms: the same `c_f`, the same `bf_f = g . S`, the same weights). The pair-harmonic `Gamma` of the coupled face
(ADR 0687) is used for the body term too, not ADR 0080's pair mean (`coupled_body_pair_mean` is not read by the fractional
step): the body term must take the coefficient of the face's pressure term for the balance of the derivation.

## The runs (registered)

Common: ADR 0687's fractional settings (`PimpleSettings::piso(0.1)`, central convection, `TIGHT` solvers, `ddt_correction =
true`, `fractional = Some(default)` (incremental, Kim-Moin), `non_orth_correctors = 0` (ADR 0687 read 4)), implicit Euler,
`dt = 0.01`, walls `ymin`, `ymax` no-slip, `fractional_body_force = true` unless stated.

- **Twin (a3).** ADR 0672's meshes `P`, `T` and map `m` (copied from ADR 0687's test file), ADR 0672's flow: start from rest,
  `p0 = g_y (y - H/2)`, `body_force = BodyForce::uniform((0.8, -2, 0))`, `p_ref = (cell 0 of P, p0 there)` and its matched cell
  in `T`; `fractional_step_consistent = true`, `fractional_periodic_implicit = true`; 200 steps; `P_on`, `T_on` with
  `coupled_ddt_correction`, `P_off`, `T_off` without.
- **Rest (a4).** Mesh `P`. A discrete potential force: `Phi = g_y (y - H/2) + C cos(2 pi x / L)`, `C = 0.1`, at the cell
  centres; `bf_f = lap_f (Phi_n - Phi_o)` on internal faces, `|S| (Phi_c - Phi_o) / (dn_f + dn_g)` on coupled faces, `g_y S_y`
  on the walls; `b_cell = green_gauss(Phi)` with the boundary values of `pressure_bc` after `apply_body_force_wall_gradient`
  (what the step computes for `grad p`). Start `u = 0`, `p = Phi`, `p_ref = (0, Phi_0)`. 200 steps. **R1**: the twin settings
  with `coupled_ddt_correction` (every coupled-face coefficient the pair-harmonic one); **R2**: ADR 0687's plain fractional
  step (`fractional_step_consistent`, `fractional_periodic_implicit`, `coupled_ddt_correction` all `false`: every coefficient
  `dte`); **NR1**: R1 with `fractional_body_force = false` (the force set, not read).
- **Start-up channel (supporting, a1 drives with the right size).** `box_2d(32, 16, 2, 1, periodic x)` unwarped, `g = (0.8, -2)`,
  start from rest, `p = p0`, 200 steps to `t = 2`; **S-a** plain fractional, **S-b** the twin settings with
  `coupled_ddt_correction`; **NS** S-a with `fractional_body_force = false`. On this mesh the flow is `u(y, t)` only, the
  pressure stays hydrostatic and `q = 0`, so the discrete problem is the 1-D cell-centred implicit-Euler diffusion
  `(dy/dt)(u_j^{n+1} - u_j^n) = nu (u_{j+1} - 2 u_j + u_{j-1}) / dy + dy F_x` with the wall rows `nu (0 - u) / (dy/2)`;
  the test solves that model itself (tridiagonal, in lockstep) as the reference. Closed form of the continuous problem:
  `U_b(t) = F H^2 / (12 nu) - sum_{n odd} 8 F H^2 / (nu pi^4 n^4) exp(-nu n^2 pi^2 t / H^2)` (the Fourier sine series of the
  start-up flow integrated across the channel; at `t = 0` the sum is `F H^2 / (12 nu)` by `sum_{odd} n^-4 = pi^4 / 96`).
- **No-op checks (a2).** On `P` from ADR 0687's initial velocity (fractional, plain, 20 steps): **DB1** body force set and the
  opt-in off against no body force, bit for bit every step; **DB2** the opt-in on without a body force against the opt-in off,
  bit for bit; **DB3** the opt-in with the Chorin form panics (`should_panic`).

## Estimates (before any run)

- **P1 (a3).** As ADR 0687's P1f (`1.2e-14`): the new terms are the same on both meshes term for term; rounding and the
  `TIGHT` solves. Limit **`1e-8`** per step.
- **NC1 (a3 non-vacuous).** `P_off` against `T_on`. From rest with the hydrostatic pressure, `R_f` on the tilted cut is
  `k . grad p` with `|grad p| = 2`, `|k| = 0.3 |S|`, `|S| = 4.1e-3`: `R` about `2.5e-3`, `Gamma R` about `1.2e-5` (`3e-3 U_ref h
  dz`) at step 1, the carried defect of the same order from step 2 (ADR 0687's estimate with the pressure's gradient set by
  gravity from the start). `G_off` estimate `1e-6` to `1e-3` from step 2. **`> 1e-8` on at least one step.**
- **P2, D1L, C1 (supporting, as ADR 0687).** Pressure `max |p_Pon - p_Ton| / (|g_y| H)` `<= 1e-8`; per-cell divergence
  difference `/ (U_ref h dz)` `<= 1e-9`; pair sums `/ (U_ref h dz)` on `P_on` `<= 1e-12`; each step. G0 as ADR 0672.
- **R1, R2 (a4).** With `p = Phi`, `grad p^n` is the same function of the same numbers as `b_cell`, so the predictor's
  right-hand side is zero to rounding, `u* = 0`; every face term is `c_f (B_f . S - compact) + c_f (compact - B_f . S)`, zero to
  rounding; `q = 0`. Rounding: terms of size `|Phi| |S| / V = 16`, times `1e-16`, times `dte`: `1e-17` per step; with the
  solves (`tol_abs 1e-16`) `1e-14` over 200 steps. Limit **`max_c |u| <= 1e-10`** (velocity units; `sqrt(|g_y| H) = 1.4`) on
  every step, and **`max_c |p - Phi| / (|g_y| H) <= 1e-10`** on every step (R1p, R2p).
- **NR1 (a4 non-vacuous).** The force not read: the predictor has `-V grad p` alone, `u* ~ dt |grad p| = 0.02`, removed by the
  projection only in part (near the walls the predictor's no-slip rows): `max |u|` `1e-3` or more. **`> 1e-6` on at least one
  step.**
- **S1a, S1b (supporting).** The 1-D model above, computed beforehand (`est.py`): `U_b = 0.57854` at `t = 2` against the closed
  form `0.57540`, relative `5.5e-3` (space and time discretisation). Limit **`|U_b - U_b,exact| / U_b,exact <= 2e-2`** at the
  last step (`U_b` the volume mean of `u_x`).
- **S2a, S2b (supporting, the cell term exactly).** Against the 1-D discrete model in lockstep: `max_c |u_x - u_1D| / U_b,1D`
  and `max_c |u_y| / U_b,1D` **`<= 1e-9`** every step (the same equations; the solves at `tol_rel 1e-13`).
- **NS (control).** No drive: `U_b = 0`, relative error 1. **`> 0.5`** at the last step.
- **DB1, DB2** bit for bit; **DB3** panics.
- **B1 (a2).** `cargo test -p rcfd-api --release --test baseline_hashes`; `rcfd-fv --lib`; the named files that run the
  changed lines: `coupled_ddt_fractional_bdf2_f757`, `coupled_ddt_correction_f754`, `skewed_periodic_channel_f752`,
  `fractional_step`, `fractional_step_consistent_f454`, `w33b_default_bits`, `w33b_transient_order`, `periodic`, and the PISO
  body-force files that run the shared wall-gradient helper: `transient_buoyancy` (PISO with a body force and walls, a
  stratified fluid at rest), `rotational_sector_pressure_part_f753`, `consistent_periodic_f367`,
  `step_consistent_paths_f443`, `piso_thin_gap_f299`: pass.
- **Cost.** Twin 4 runs, rest 3, start-up 3, no-ops 4 short: fractional runs of 512 cells, about 1 to 2 s each in release (ADR
  0687's 7 fractional runs took about 8 s). No heavy record.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| P1 | a3 | twin, 200 steps | every step `G_on(n)` | `<= 1e-8` each step |
| NC1 | a3 (non-vacuous) | same | `G_off(n)` | `> 1e-8` on at least one step |
| P2 | supporting | same | `max abs(p_Pon - p_Ton) / (abs(g_y) H)` | `<= 1e-8` each step |
| D1L | supporting | same | `max abs(div_Pon - div_Ton) / (U_ref h dz)` | `<= 1e-9` each step |
| C1 | supporting | same | `max abs(phi_f + phi_g) / (U_ref h dz)` on `P_on` | `<= 1e-12` each step |
| G0 | set-up | the meshes | ADR 0672's cell map | `1e-12`, one-to-one |
| R1 | a4 | rest, twin settings | `max_c abs(u)` | `<= 1e-10` each step |
| R1p | a4 | same | `max_c abs(p - Phi) / (abs(g_y) H)` | `<= 1e-10` each step |
| R2 | a4 | rest, plain fractional | `max_c abs(u)` | `<= 1e-10` each step |
| R2p | a4 | same | as R1p | `<= 1e-10` each step |
| NR1 | a4 (non-vacuous) | R1 without the opt-in | `max_c abs(u)` | `> 1e-6` on at least one step |
| S1a, S1b | supporting | start-up, plain / twin settings | `abs(U_b - U_b,exact) / U_b,exact` at `t = 2` | `<= 2e-2` |
| S2a, S2b | supporting | same | against the 1-D discrete model | `<= 1e-9` each step |
| NS | control | S-a without the opt-in | as S1 | `> 0.5` |
| DB1, DB2 | a2 | `P`, 20 steps | no-op | bit for bit every step |
| DB3 | a1 | Chorin form with the opt-in | panics | panics |
| B1 | a2 | the suites above | pass | as stated |

Printed, no limit: `G_on`, `G_off` at steps 1 and 2, their largest and its step; `T_on` against `T_off`; the largest old-level
defect on `T_on`'s faces at the images of `P`'s cut; R1 and R2 largest `|phi|`; the start-up `U_b`, its 1-D model value and the
closed form; elapsed times.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the new test file run once per mutation;
each must fail the named checks):

- **MB1** `fractional_body_force` not read (the force never applied): must fail R1 (and S1a).
- **MB2** the predictor's cell term dropped: must fail R1 and S1a.
- **MB3** the balanced-force face term dropped on every face: must fail R1 (not the twin: uniform force, the term vanishes).
- **MB4** the face term dropped on the coupled faces only: must fail R1.
- **MB5** the walls' pressure gradient dropped (fractional step only): must fail R1.
- **MB6** the face term with `dte` in place of the face's pressure coefficient `c_f` (internal and coupled faces): must fail R1
  (R2 stays met: there `c_f = dte`).
- **MF1** (ADR 0687's MF1 on the body-force flow) the fractional coupled ddt term not read: must fail P1.

## Decision rule

**Done** when P1, NC1, R1, R2, R1p, R2p, NR1, DB1, DB2, DB3 and B1 are met (a1 to a4), P2, D1L, C1, G0 held, S1a, S1b, S2a, S2b
and NS met, and MB1 to MB6 and MF1 caught. **Partial** when P1 and R1 are met but a control, a supporting check or a required
mutation is missed, or P1 or R1 (R2) is missed with its cause diagnosed. **Todo** when P1 or R1 is missed without a diagnosis,
or a2 fails (DB1, DB2 or B1).

## Limits (not covered)

- Incremental form only (the Chorin form refuses the opt-in); implicit Euler only in the runs (BDF2 uses the same `c_f`, not
  run); `non_orth_correctors = 0` (the fractional step has no coupled non-orthogonal correction); translational pairs only
  (on rotational pairs the coupled `B_f` takes the partner's cell value unrotated, as the coupled `gf` does, ADR 0687);
  fixed-pressure (free-velocity) boundary faces carry the term in the code but no run has such a face; no moving mesh; the
  hook's `source_force` is not read (the fractional step runs no hook); no case key (the opt-in is a solver setting).
- The rest state is a discrete potential force on `P`; a physical non-uniform force (Boussinesq) is balanced only to the
  truncation of its face values, as in PISO.
- The defaults stay: whether the fractional step should read the body force by default is open (changes default results of
  any fractional run with a body force; none in the hash tables is known, not checked).

## Open questions

- **Q1. The Chorin form.** (a) refuse the opt-in (taken); (b) apply the cell term and a `dte` face term there (unbalanced:
  the predictor's `V / a_P` is not `dte`); (c) silently ignore. Taken (a) on the maintainer's standing instruction of
  2026-11-11 (take the recommendation).
- **Q2. The coupled faces' coefficient.** (a) the coefficient of the face's pressure term (taken: needed for the balance);
  (b) ADR 0080's pair mean under `coupled_body_pair_mean` as in PISO (breaks the balance with the harmonic `Gamma`). Taken (a)
  on the standing instruction.
- **Q3. The default.** Open (changes default results).

## Amendment 1 (2026-11-17): S2a missed; diagnosis, the record kept, a corrected set-up

Written after the one registered run of `fractional_body_force_f771.rs` (b2ac49e3, release, 3.3 s, log
`w49i-work/f771-scratch/fast.log`) and before any code or run of the corrected set-up. Nothing above this amendment is changed.

**What the registered run gave.** The twin test (P1, NC1, P2, D1L, C1, G0), the rest test (R1, R1p, R2, R2p, NR1), the
no-ops (DB1, DB2) and the refusal (DB3) passed (numbers in Addendum 1). In the start-up test S1a, S1b, S2b and NS were met and
**S2a missed**: the plain fractional run S-a against the 1-D model `5.79e-2` (relative to `U_b,1D`, largest at step 1, limit
`1e-9`), while S-b, the same flow with the twin settings, is within `5.4e-14` of the model.

**Diagnosis (from the code and the numbers, no further run).** S-a and S-b differ by `fractional_step_consistent`,
`coupled_ddt_correction` and `fractional_periodic_implicit`. With the last one off the fractional predictor keeps the periodic
partner lagged (ADR 0687 read 3): at the cut the `x` diffusion and convection couple the cell to the partner's old value, so
the flow is not uniform in `x` at the cut and the 2-D run is not the 1-D model. At step 1 the partner's old value is `0` and
the cell's new one about `dt F = 8e-3`; the lagged diffusion coefficient `nu |S| / dx = 0.1` against the diagonal `V / dt +
...` of about `0.4 + 0.2` takes some ten to twenty percent off the cells at the cut, a few percent of the mean: the measured
`5.8e-2` at step 1. This is the pre-existing lagged partner of the default fractional step (ADR 0687 measured it as NCL,
`1.9e-2` on the channel), not the body force: the body terms are identical in S-a and S-b except that S-a takes `dte` where
S-b takes `Gamma`, and for the uniform force of this flow the face term is zero on every face. S1a (the bulk against the
closed form, `3.6e-4`) is met; the lag moves the cut cells, not the bulk to first order.

**The record.** The registered test is kept as `fractional_start_up_channel_against_the_model_and_the_closed_form_known_miss`,
`#[ignore]`d, limits intact, the measured numbers in its doc comment.

**The corrected set-up (registered now).** **S-a'**: S-a with `fractional_periodic_implicit = true` only
(`fractional_step_consistent` and `coupled_ddt_correction` stay `false`, so every coefficient of the body terms is `dte`, the
path S-a was to test). Check **S2a'**: against the 1-D model, `max_c (|u_x - u_1D|, |u_y|) / U_b,1D <= 1e-9` every step (limit
as S2a; estimate rounding, as S2b's `5.4e-14`); **S1a'** the bulk at `t = 2` against the closed form `<= 2e-2` (estimate the
model's `5.5e-3`). New test `fractional_start_up_channel_with_the_implicit_partner`, run once.

**Decision rule (amended).** In the registered rule read S2a' and S1a' in place of S2a (S1a stays as measured). S2a stays
recorded as missed with the cause above. If S2a' is missed, partial as registered.

## Addendum 1: results (2026-11-17)

Measured against the limits registered above and in amendment 1; no limit was changed. Release builds on Windows, on a machine
shared with other building agents. Commits: the registration bf69d23e (alone, first); the library change and the test file
b2ac49e3 (before their run); amendment 1 bce614fa (after the registered run's S2a miss, before any code of the correction); the
known-miss record and the corrected test 0a7d2693 (before its run). Each registered run was made once: the test file at b2ac49e3
(5 tests, 3.3 s, `fast.log`), the amendment-1 test at 0a7d2693 (0.3 s, `amend1.log`), `rcfd-fv --lib`, the named files,
`baseline_hashes`, `docs_site`. Logs in `w49i-work/f771-scratch`.

**Built.** `crates/rcfd-fv/src/pressure.rs` 91 to 110: `apply_body_force_wall_gradient` (the PISO step's wall-gradient code
moved, same operations). `crates/rcfd-fv/src/pimple.rs`: the import (29 to 30), the field `fractional_body_force` with its doc
(222 to 230), its default (1059), the PISO call of the helper in place of the inline loop (1846).
`crates/rcfd-fv/src/pimple/fractional.rs`: the module doc, the opt-in and its Chorin refusal, the wall gradient after
`pressure_bc`, the predictor's cell term, the face term `c_f (b_f . S - B_f . S)` on internal, coupled and free-velocity
boundary faces. The new test file `crates/rcfd-fv/tests/fractional_body_force_f771.rs`. One passage of
`docs/dynamic-mesh.md`. No case key, no golden, no hash table.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P1 (a3) twin `G_on(n)`, 200 steps | `<= 1e-8` each step | `1.84e-14` (step 32; step 1 `1.5e-15`) | met |
| NC1 (a3 control) | `> 1e-8` on one step | largest `1.50e-3` (step 1); every step at least `4.4e-4` | held |
| P2 pressure / `(abs(g_y) H)` | `<= 1e-8` each step | `3.8e-14` | met |
| D1L divergence / `(U_ref h dz)` | `<= 1e-9` each step | `5.3e-15` | met |
| C1 pair sums / `(U_ref h dz)` | `<= 1e-12` each step | `4.4e-16` | met |
| G0 | `1e-12`, one-to-one | `8.9e-16`, yes | met |
| R1, R1p (a4, twin settings) | `<= 1e-10` each step | `0`, `0` (exactly at rest) | met |
| R2, R2p (a4, plain fractional) | `<= 1e-10` each step | `0`, `0` | met |
| NR1 (a4 control) | `> 1e-6` on one step | `2.21e-3` (step 1) | held |
| S1a (as registered) | `<= 2e-2` | `3.57e-4` | met |
| S1b | `<= 2e-2` | `5.46e-3` | met |
| S2a (as registered) | `<= 1e-9` each step | `5.79e-2` (step 1) | **missed** (known miss, amendment 1) |
| S2b | `<= 1e-9` each step | `5.43e-14` | met |
| NS | `> 0.5` | `1.0` | held |
| S1a' (amendment 1) | `<= 2e-2` | `5.46e-3` | met |
| S2a' (amendment 1) | `<= 1e-9` each step | `5.43e-14` | met |
| DB1, DB2 | bit for bit, 20 steps | equal | met |
| DB3 Chorin refusal | panics | panics with the message | met |
| B1 | pass | see below | met |

Printed, no limit: `T_on` against `T_off` `1.3e-7 U_ref`; the largest old-level defect on `T_on`'s faces at the images of
`P`'s cut `5.6e-3 U_ref h dz` (estimate `3e-3`); C1 of `P_off` `3.3e-16`; `T_on`'s mean `u_x` at `t = 2` `0.5795`; R1 and R2
largest `abs(phi)` `0`; the start-up `U_b` S-a `0.575194`, S-b and S-a' `0.578542` (the 1-D model `0.578542`, the closed form
`0.575399`). NC1's control is largest at step 1, not from step 2 as in ADR 0687: with the hydrostatic start `P_off` and `T_on`
already differ at step 1 by the coupled faces' coefficient (`dte` in `P_off` against the harmonic `Gamma` of `T`'s internal
face, in front of the non-zero `R_f` of the hydrostatic pressure on the tilted cut), not by the carried defect; that is the
same opt-in's other half (ADR 0687), so NC1 still measures `coupled_ddt_correction`.

Suites (release, run once, all passed; ignored records not run): `baseline_hashes` 3 passed, 1 ignored; `rcfd-fv --lib` 115
passed, 6 ignored; `coupled_ddt_fractional_bdf2_f757` 5 (1 ignored); `coupled_ddt_correction_f754` 2;
`skewed_periodic_channel_f752` 3; `fractional_step` 13 (3 ignored); `fractional_step_consistent_f454` 3; `w33b_default_bits`
1; `w33b_transient_order` 6 (9 ignored); `periodic` 3; `transient_buoyancy` 3; `rotational_sector_pressure_part_f753` 3;
`consistent_periodic_f367` 13 (2 ignored); `step_consistent_paths_f443` 5 (2 ignored; 57 s); `piso_thin_gap_f299` 18 (2
ignored; 52 s). `cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean.
`docs_site` 2 passed.

### Mutations (scratch copy `w49i-work/f771-scratch/mut` of 0a7d2693, script `mutate.py`, target `target-mut`; the new test file run once per mutation)

| Mutation | Must fail | Result |
| --- | --- | --- |
| M0 (harness, none) | none | every test passes with the registered numbers |
| MB1 opt-in not read | R1 (S1a) | caught: R1 `2.2e-3`, R1p, R2, R2p; S1a' `1.0`, S2a' |
| MB2 cell term dropped | R1, S1a | caught: R1 `1.2e-2`, R2; S1a' `1.0`, S2a' |
| MB3 face term dropped | R1 | caught: R1 `5.4e-3`, R2 `5.7e-3`; the twin and start-up met (uniform force) |
| MB4 coupled faces' face term dropped | R1 | caught: R1 `3.3e-3`, R2 `3.5e-3` |
| MB5 wall gradient dropped | R1 | caught: R1 `1.2e-2`, R2; S2a' `0.86` |
| MB6 `dte` in place of `c_f` | R1 (R2 met) | caught: R1 `5.7e-3`, R1p; R2 met as predicted |
| MF1 coupled ddt term not read | P1 | caught: P1 `1.5e-3` (the control's value), P2 |

Seven of seven required mutations caught. MB1 and MB2 are judged on S1a' (amendment 1); the known-miss record is ignored and
was not run under the mutations.

### Deviations

- Amendment 1 (registered before its code and run): the plain fractional start-up run missed S2a by the pre-existing lagged
  periodic partner of the fractional predictor (ADR 0687 read 3), not by the body force; the record is kept as
  `fractional_start_up_channel_against_the_model_and_the_closed_form_known_miss`; S2a' with `fractional_periodic_implicit`
  met at `5.4e-14`.
- NC1's control peaks at step 1 (explained above); the registered check ("at least one step") is unaffected.

### Not covered

The Chorin form (refused); BDF2 (same coefficients, not run); `non_orth_correctors > 0` (the fractional step has no coupled
non-orthogonal correction); rotational pairs (the coupled `B_f` takes the partner's cell value unrotated, as `gf` does);
fixed-pressure boundary faces (the term is coded, no run has one); moving meshes; the hook's `source_force`; a physical
non-uniform force (the rest state is a discrete potential force); no case key; the defaults unchanged.

### Decision

By the rule as amended: P1, NC1, R1, R2, R1p, R2p, NR1, DB1, DB2, DB3 and B1 met (a1 to a4); P2, D1L, C1, G0 held; S1a, S1b,
S1a', S2a', S2b and NS met; MB1 to MB6 and MF1 caught. S2a of the first set-up is missed with its cause diagnosed (amendment
1). **Done** on the amended rule; on the rule as first registered a missed supporting check gives partial.

### Remaining questions

1. **The default.** Should the fractional step read the body force without the opt-in? Options: (a) make
   `fractional_body_force` the default (changes default results of every fractional run with a body force, which today
   silently ignores it; hashes not checked for such a case); (b) keep it opt-in; (c) keep the default but refuse a body force
   without the opt-in. Recommendation (a) together with ADR 0687's coupled-face opt-ins, for the maintainer; (c) as the
   smaller step if (a) is not wanted. Not taken (changes default results).
2. **The Chorin form.** Refused (Q1 taken). Options: (a) keep the refusal; (b) a balanced Chorin form as a follow-up.
   Recommendation (a) unless Chorin runs with gravity are needed.
3. **The plain fractional step's lagged periodic partner.** It costs `5.8e-2` of the start-up channel's mean at step 1 at the
   cut (S2a) and `1.9e-2 U_ref` on the channel (ADR 0687 NCL). Recommendation: make `fractional_periodic_implicit` the
   default with ADR 0687's Q3 (maintainer).

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0691, 2026-11-17: with the opt-in PimpleSettings::fractional_body_force (incremental form only, the Chorin form refused; default false, bit-identical) the fractional step reads PimpleSolver::body_force in the PISO step's three places: the walls' balancing pressure gradient (pressure::apply_body_force_wall_gradient, now shared with the PISO step), the predictor's cell term, and the balanced-force face term c_f (b_f . S - B_f . S) with each face's pressure coefficient (dte, or the harmonic Gamma with fractional_step_consistent and on coupled faces with coupled_ddt_correction). On ADR 0672's channel with its body force (0.8, -2) from the hydrostatic start the fractional run with coupled_ddt_correction stays within 1.8e-14 U_ref of its unrolled twin on every one of 200 steps (limit 1e-8) against 4.4e-4 to 1.5e-3 without it; pair fluxes opposite to 4.4e-16. A discrete hydrostatic rest state (a non-uniform potential force on the skewed periodic mesh) stays exactly at rest (limit 1e-10) with the twin settings and with the plain fractional step, against 2.2e-3 without the opt-in; the start-up channel matches its 1-D discrete model to 5.4e-14 and the closed form to 5.5e-3. The plain start-up run first missed its model by 5.8e-2 (kept as a known miss): the lagged periodic partner of the default fractional predictor, not the body force; with fractional_periodic_implicit it is within 5.4e-14. Seven of seven mutations caught (opt-in not read, cell term, face term, coupled face term, wall gradient, dte in place of Gamma, coupled ddt term). Not covered: BDF2, non-orthogonal correction, rotational pairs, fixed-pressure faces, the hook's source_force; whether the opt-in becomes the default is open for the maintainer.

> Follow-up proposed: "Fractional-step coupled-face opt-ins as defaults". Deps F-771, F-757. Acceptance: the maintainer decides whether fractional_body_force, fractional_periodic_implicit and coupled_ddt_correction (with fractional_step_consistent) become the fractional step's defaults; if so, the changed default results are listed case by case, the hash tables are re-recorded only with the maintainer's approval, and ADR 0687's and ADR 0691's twin checks pass with the defaults alone.

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-775, remaining questions 1 and 3 (first asked as Q3).** Asked: `fractional_body_force` and the coupled-face opt-ins of ADR 0687 as the fractional step's defaults. Answer: Held. The default change is not approved now: it moves existing results, and it waits until the fan path needs it. Not to be issued.
