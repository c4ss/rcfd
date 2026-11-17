# ADR 0687: F-757 registration: Rhie-Chow ddt correction on coupled faces in the fractional step and with BDF2

Status: **REGISTERED 2026-11-17, written before any code of F-757 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-757 was written and no run of any kind was made; the numbers quoted
from earlier work are those printed in ADR 0672, ADR 0675 and ADR 0356. Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-757 (deps F-754), proposed by [ADR 0675](0675-f754-registration-rhie-chow-ddt-correction-on-coupled-faces.md)
remaining question 2. Parents: ADR 0675 (F-754: `PimpleSettings::coupled_ddt_correction`, the helper `coupled_ddt_term`, the
list `ddt_cp` of the PISO step; BDF2 written, not run; the fractional step not touched),
[ADR 0672](0672-f752-registration-pressure-part-of-the-non-orthogonal-correction-on-coupled-faces.md) (the skewed periodic
channel `P` and its unrolled twin `T`), ADR 0356 (`fractional_step_consistent`: the fractional step's carried face defect),
ADR 0046 (implicit periodic partner of the PISO momentum predictor), ADR 0037 (the fractional step). Number 0687 given by the
lead. Branch `f757-work` from 5b852ad5 (F-752, F-753, F-754 merged), worktree `w49i-work/f757`, scratch `w49i-work/f757-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The fractional step's transient Rhie-Chow form is the repository's own
(ADR 0356: `Gamma R(p^n) + (Gamma / dt) K` on internal faces); the coupled-face form below is that formula with the partner
cell in the place of the neighbour, derived from the code read, exactly as ADR 0675 did for the PISO step. The BDF2 weights
are the repository's `ddt_history_weights`. The arithmetic below was done by hand before any run.

## What F-757 asks

The acceptance, binding (from the lead's brief):

> The fractional step's ddt correction gives the coupled pairs the term behind the same opt-in, defaults bit-identical; on ADR 0672's channel against its twin with ddt_correction on, the fractional-step run and a BDF2 PISO run are each within a registered limit of the twin on every step where the run without the opt-in is not. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **c1** the fractional step's ddt correction gives the coupled pairs the term, behind `coupled_ddt_correction`;
- **c2** defaults bit-identical;
- **c3f** on ADR 0672's channel against its twin, the fractional-step run with the opt-in is within a registered limit of the
  twin on every step where the run without the opt-in is not;
- **c3b** the same for a BDF2 PISO run.

**Reading of c3 (registered, as ADR 0675).** With `G_V(n) = max_c |u_V(n)_c - u_T(n)_m(c)| / U_ref` (ADR 0672's map `m`), the
check is `G_on(n) <= eps` on every step (stronger than "on every step where the control is not"), and the clause is
non-vacuous only when the control `G_off(n) > eps` on at least one step (binding). At step 1 the old flux and the old velocity
give a zero defect on every face (the initial flux is the interpolated initial velocity, `PimpleSolver::new`), so the control
equals the opt-in run at step 1 to rounding: "at least one step" (ADR 0675 Q2).

**The twin with the opt-in (registered, ADR 0675 Q1).** The reference is `T` with the opt-in (`T_on`): `T`'s own cut is an
internal face of `P`. `T_off` is printed.

## What the code read shows (5b852ad5, read only)

1. `pimple/fractional.rs` `step_fractional`: the fractional step has no `ddt_correction`; its transient Rhie-Chow form is
   ADR 0356's opt-in `fractional_step_consistent` (incremental form only, `sc`): internal faces get
   `phi* += Gamma_f R_f(p^n) + Gamma_f / dt K_f` with `Gamma = harmonic_face(V / (a_P / n_active))`, `K = carried_defect(c1,
   c2)` (the same expression as PISO's `ddt_defect`), `R_f = interp(grad p).S - lap_coeff (p_n - p_o)`. Coupled faces get
   `dte (gf . S - |S| (p_c - p_o) / span)` with `gf = a grad_o + (1 - a) grad_c`, whether `sc` or not: neither the harmonic
   `Gamma` nor the carried defect. **This is the coupled faces' missing term in the fractional step.**
2. **The fractional step does not read `body_force`** (no use of it in `fractional.rs`; `pressure_bc` is called without it).
   ADR 0672's channel is driven by a body force (`(0.8, -2.0)`): in the fractional step that flow does not exist (the run
   would start from the hydrostatic pressure with no gravity).
3. **The fractional step's momentum predictor keeps the periodic partner lagged.** `convection` and `laplacian` put the
   partner of a coupled face on the right-hand side with the old value; the PISO step moves it back into the matrix
   (`PeriodicLinks::build` / `fix_rhs` / `solve`, ADR 0046), the fractional step does not (its comment: under BDF2 the lagged
   partner is extrapolated so that the explicit coupling is not first order). With a lagged partner the periodic run is not
   its twin's discrete problem whatever the ddt term does: the partner's diffusion coefficient over the diagonal,
   `(nu |S| / dx) / a_P = 0.025 / 0.049 = 0.5`, times the velocity change of a step (`1e-3` to `1e-2` on the flow below),
   gives `G` of order `1e-3` per step. A check of c3f is impossible without it.
4. Non-orthogonal correction: the fractional step does not read `coupled_non_orth` (ADR 0667's doc); with
   `non_orth_correctors > 0` the skewed coupled faces of `P` lack the correction that `T`'s internal faces at the same place
   carry (ADR 0672's NC1 gap, `1e-4` and more). With `non_orth_correctors = 0` neither mesh carries any non-orthogonal
   correction: internal `lap_coeff = |S|^2 / (S . d) = |S| / (n . d)` and the coupled `|S| / span` with `span = dn_f + dn_g =
   n . d` (geometry read) are the same number; the momentum Laplacian takes no gradient (central convection needs none); the
   Green-Gauss face values on coupled faces take the `dn` weights, the internal faces' `geom.weights` (ADR 0672 read 3).
5. Convection on a coupled face (`convection.rs`): implicit upwind between owner and lagged partner plus the central value by
   deferred correction with the lagged cells, the internal faces' scheme (upwind implicit, central deferred); with
   `PeriodicLinks` the upwind partner part and the Laplacian partner part are implicit, as on the internal face. Under BDF2 the
   fractional step's lagged cells are the extrapolated `conv_cells` and the coupled faces' `b`, `d` are overridden with the
   extrapolated partner (`bc_extrapolated`), so the deferred parts agree with the internal faces'.
6. PISO with BDF2 (`step_with`): `ddt_history_weights` gives `(1, 0)` while `older` is absent (step 1) and the BDF2 weights
   from step 2; `coupled_ddt_term` evaluates `c2 (phi_older - U_older_f . S)` with `boundary_hby_a` on the older velocities.
   Not yet run (ADR 0675).
7. `coupled_ddt_term` multiplies the defect by `coupled_gamma(gamma_p)` (arithmetic pair mean, weight `w = dn_g / (dn_f +
   dn_g)`). The doc comment of `ddt_history_weights` sits above `coupled_ddt_term` since F-754 (moved back here; a comment).

## Design

### The opt-in in the fractional step (how "the same opt-in" is reached, registered)

`PimpleSettings` is the fractional step's settings struct (`self.settings`), so the same field carries it:
`coupled_ddt_correction` is read in the fractional step **with `fractional_step_consistent` and the incremental form** (the
fractional step's ddt correction; the step does not read `ddt_correction`, and the runs below set it `true` as the acceptance
says, without effect on the fractional step). With the three set, every coupled face `f` (pressure condition coupled, partner
face `g`, owner `r`, partner cell `c`, not empty) gets, in place of `dte R_f`:

`phi*_f += Gamma_f R_f(p^n) + Gamma_f / dt D_f`, `Gamma_f = 1 / (w / Gamma_r + (1 - w) / Gamma_c)`, `w = dn_g / (dn_f + dn_g)`,

`R_f` the existing coupled `gf . S - |S| (p_c - p_o) / span`, `D_f` ADR 0675's coupled defect `c1 (phi_old_f - U_old_f . S_f)
- c2 (phi_older_f - U_older_f . S_f)`, `U_f = w U_r + (1 - w) R U_c` (`boundary_hby_a`). Derivation: for a translational
pair this is the internal face of the twin term for term (`harmonic_face` with `geom.weights = w`, `R_f` with `lap = |S| /
span` (read 4), `K_f = D_f` (ADR 0675)). Conservation: `w_g = 1 - w_f` and the cells swapped give the same `Gamma` (the two
sums are the same two products, added in the other order: equal in floating point); `R_g = -R_f` (existing) and `D_g = -D_f`
(ADR 0675): opposite increments.

Code: the defect part of `coupled_ddt_term` becomes `coupled_ddt_defect` returning `(f, D_f)`; `coupled_ddt_term` multiplies
by `coupled_gamma` as before (same operations, PISO bits unchanged); the fractional step builds `D` and the pair-harmonic
`Gamma` only with the three settings. Without them every operation of the fractional step is the previous one.

### A second opt-in: the implicit periodic partner in the fractional step (registered, Q1)

`PimpleSettings::fractional_periodic_implicit: bool`, default `false`: the fractional step's momentum predictor moves the
lagged partner of the translational periodic faces into the matrix as the PISO step does (`PeriodicLinks::build` with the
predictor's `phi_conv`, `gamma_nu`, `bc`; `fix_rhs` with the cell values the assembly used, `conv_cells`; `solve` and
`residual` with the links). Needed by read 3 for any twin check of the fractional step; a separate field so that the control
`P_off` differs from `P_on` by the ddt term only. Rotational pairs keep the lagged partner (as `PeriodicLinks` does). With the
field `false` nothing changes; on a mesh without translational pairs the links are empty and every call is the previous one.

### The flow of the fractional-step runs (registered deviation, Q2)

By read 2 the body-force drive of ADR 0672's channel is not available to the fractional step. The fractional runs use ADR
0672's meshes `P` and `T`, its map `m`, its walls (no-slip `ymin`, `ymax`), `nu = 0.1`, `dt = 0.01`, 200 steps, central
convection, `TIGHT` solvers, and in place of the body force an initial velocity (no body force, `p = 0` at the start,
`p_ref = (cell 0 of P, 0)` and its matched cell in `T`):

`u = U0 4 y (H - y) / H^2 + A (pi / H) sin(2 pi y / H) cos(2 pi x / L)`, `v = A (2 pi / L) sin^2(pi y / H) sin(2 pi x / L)`,

`U0 = 1`, `A = 0.5 H / pi` (the cellular part `0.5 U_ref`): the decaying start of a plane shear flow with a periodic cellular
part (the streamfunction `A sin^2(pi y / H) cos(2 pi x / L)`; zero at the walls), evaluated at the cell centres, with `x + L/2`
on `T` (its cells are `P`'s shifted by `-L/2`). Settings: `PimpleSettings::piso(0.1)` with `fractional =
Some(FractionalSettings::default())` (incremental, Kim-Moin), `fractional_step_consistent = true`, `ddt_correction = true`,
`fractional_periodic_implicit = true`, `non_orth_correctors = 0` (read 4), the opt-in on or off. Courant at most `1.5 * 0.01 /
0.0625 = 0.24`.

### The BDF2 PISO runs

Exactly ADR 0675's channel lockstep (ADR 0672's flow and settings, `ddt_correction = true`, the body force, 6 passes,
`coupled_non_orth`, `coupled_body_pair_mean`) with `time_scheme = TimeScheme::Bdf2`.

## Estimates (before any run)

- **Size of the fractional term.** `Gamma = V / (a_P / 2)` with `a_P` the sum of the two components' diagonals, each `V / dt
  + 4 nu |S| / dx` plus convection (`0.0244 + 0.025`): `Gamma` about `4.9e-3`, so `Gamma / dt = 0.49` (Euler). The defect
  after a step is `D = Gamma R(p^n) + 0.49 D_old - dte R(q)`; with no non-orthogonal correction `R` on a skewed face contains
  `k . grad p`, `|k| = 0.3 |S|`, `|S| = 4.1e-3`; the flow's pressure from the convective and cellular terms is of order `0.1
  to 0.5` with gradient up to `2` in `y`: `R` up to `2.5e-3`, `Gamma R` up to `1.2e-5` (`3e-3 U_ref h dz`), the ddt term
  `0.49 D` of the same order. Uncertain by two orders: `G_off` of the fractional run between `1e-6` and `1e-3` from step 2.
- **P1f (c3f).** `P_on` against `T_on`, fractional, Euler: every term of every coupled face is the twin's internal face
  (design, read 4, read 5); what differs is rounding and the iterative solves (`TIGHT`, BiCGStab with the links in both
  meshes, both have coupled faces), ADR 0672 and 0675 measured `6e-15`. Limit **`eps = 1e-8`** per step.
- **NC1f (c3f non-vacuous).** `P_off` against `T_on`: **`> 1e-8` on at least one step** (estimate `1e-6` or more; margin 100
  at the low end).
- **P1b (c3b).** `P_on` against `T_on`, PISO BDF2: as ADR 0675's P1 (`5.6e-15` there); BDF2 adds an older level evaluated by
  the same `boundary_hby_a` and no growth (the term is a contraction). **`1e-8`** per step.
- **NC1b (c3b).** `P_off` against `T_on`, PISO BDF2: ADR 0675's Euler control was `5.1e-6` to `1.4e-5`; BDF2's `c1 = 2`,
  `c2 = 0.5` from step 2 and `Gamma / dt` at `V / (1.5 V / dt)`: the same order. **`> 1e-8` on at least one step.**
- **P2f, P2b (supporting).** Pressure `max_c |p_Pon - p_Ton|`, divided by `0.5 U_ref^2` (fractional) or `|g_y| H` (PISO):
  **`1e-8`** per step.
- **D1Lf, D1Lb (supporting).** `max_c |div_Pon(c) - div_Ton(m(c))| / (U_ref h dz)`: **`1e-9`** per step (ADR 0672).
- **C1f, C1b (c1).** `max |phi_f + phi_g| / (U_ref h dz)` over `P_on`'s 16 pairs: **`1e-12`** per step (ADR 0675: `3.3e-16`).
- **NCL (supporting, the second opt-in is needed).** Fractional `P_on` without `fractional_periodic_implicit` against `T_on`
  (with it): **`> 1e-6` on at least one step** (read 3: of order `1e-3`).
- **D5 (c2).** Fractional `P` with `coupled_ddt_correction = true` and `fractional_step_consistent = false` against both
  `false` (links on in both): every step **bit for bit** (the field is not read without `sc`).
- **D6 (c2).** Fractional `P` with `fractional_periodic_implicit = true` on the twin-free mesh `box_2d(32, 16, 2, 1, false,
  false)` (no periodic pair, `xmin`/`xmax` walls set to no-slip) against `false`: 20 steps **bit for bit** (the links are
  empty).
- **S1fb, SNC1fb (supporting, the fractional step with BDF2).** The fractional runs with `time_scheme = Bdf2`: `P_on` against
  `T_on` **`<= 1e-8`** every step; `P_off` against `T_on` `> 1e-8` on at least one step.
- **G0.** ADR 0672's cell map: `1e-12`, one-to-one.
- **B1 (c2).** `cargo test -p rcfd-api --release --test baseline_hashes`; `rcfd-fv --lib`; the named files
  `coupled_ddt_correction_f754`, `skewed_periodic_channel_f752`, `rotational_sector_pressure_part_f753`, `periodic`,
  `consistent_periodic_f367`, and the fractional-step files that run the changed lines `fractional_step`,
  `fractional_step_consistent_f454`, `w33b_default_bits`, `w33b_transient_order`, `step_consistent_paths_f443`: pass.
- **Cost.** Fractional: 7 Euler runs and 3 BDF2 runs of 200 steps on 512 cells (one momentum and one Poisson solve a step):
  under 20 s. PISO BDF2: 4 runs of 200 steps (ADR 0675's six took 23 s): about 15 s. No heavy record; the registered runs are
  the fast tests.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| P1f | c3f | fractional, Euler, 200 steps | every step `G_on(n)` | `<= 1e-8` each step |
| NC1f | c3f (non-vacuous) | same | `G_off(n)` | `> 1e-8` on at least one step |
| P2f | supporting | same | every step `max_c abs(p_Pon - p_Ton) / (0.5 U_ref^2)` | `<= 1e-8` each step |
| D1Lf | supporting | same | every step `max_c abs(div_Pon - div_Ton) / (U_ref h dz)` | `<= 1e-9` each step |
| C1f | c1 | same | every step `max abs(phi_f + phi_g) / (U_ref h dz)` on `P_on` | `<= 1e-12` each step |
| NCL | supporting | same | `P_on` without the implicit partner against `T_on` | `> 1e-6` on at least one step |
| D5 | c2 | same mesh | opt-in without `sc` against neither | bit for bit every step |
| D6 | c2 | no periodic pair, 20 steps | `fractional_periodic_implicit` on against off | bit for bit every step |
| P1b | c3b | PISO BDF2, 200 steps | every step `G_on(n)` | `<= 1e-8` each step |
| NC1b | c3b (non-vacuous) | same | `G_off(n)` | `> 1e-8` on at least one step |
| P2b | supporting | same | every step `max_c abs(p_Pon - p_Ton) / (abs(g_y) H)` | `<= 1e-8` each step |
| D1Lb | supporting | same | per-cell divergence, as D1Lf | `<= 1e-9` each step |
| C1b | c1 | same | pair sums on `P_on` | `<= 1e-12` each step |
| S1fb | supporting | fractional BDF2, 200 steps | every step `G_on(n)` | `<= 1e-8` each step |
| SNC1fb | supporting | same | `G_off(n)` | `> 1e-8` on at least one step |
| G0 | set-up | the meshes | ADR 0672's cell map | `1e-12`, one-to-one |
| B1 | c2 | the suites named above | pass | as stated |

Printed, no limit: `G_on`, `G_off` at steps 1 and 2, their largest and its step; `T_on` against `T_off`; the largest old-level
defect on `T_on`'s internal faces at the images of `P`'s cut; C1 of `P_off`; elapsed times.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the new test file run once per mutation;
each must fail the named checks):

- **MF1** the fractional coupled term not read (the coupled faces keep `dte R_f` with the opt-in): must fail P1f.
- **MF2** the fractional coupled faces take `Gamma_f / dt D_f` but keep `dte` in front of `R_f`: must fail P1f.
- **MF3** the fractional coupled defect with the opposite sign: must fail P1f.
- **MB1** the BDF2 weights swapped in the coupled defect where `c2 != 0` (`c2` in place of `c1` and `c1` in place of `c2`;
  only BDF2 steps from step 2 are touched): must fail P1b (and S1fb); P1f (Euler from start to end) must stay met.
- **MB2** the older level replaced by the old level in the coupled defect's `c2` part: must fail P1b.
- **ML1** `fractional_periodic_implicit` not read: must fail P1f.

Diagnostic (reported, not in the decision rule): **MF4** the arithmetic pair mean in place of the harmonic one in the
fractional coupled `Gamma` (the cells next to the cut have nearly equal `Gamma`: may survive).

## Decision rule

**Done** when P1f, NC1f, P1b, NC1b, C1f, C1b, D5, D6 and B1 are met (c1 to c3), P2f, P2b, D1Lf, D1Lb, G0 and NCL held, and
MF1, MF2, MF3, MB1, MB2, ML1 caught; S1fb and SNC1fb are supporting (a miss reported, moving done to partial only if S1fb is
missed). **Partial** when P1f and P1b are met but a control, C1, a supporting check or a required mutation is missed, or P1f
or P1b is missed with its cause diagnosed; and in any case partial if the maintainer rejects Q2 (the fractional flow is not
ADR 0672's body-force flow). **Todo** when P1f or P1b is missed without a diagnosis, or c2 fails (D5, D6 or B1).

## Limits (not covered)

- The fractional step's flow is not ADR 0672's body-force flow (read 2): the meshes, the twin, the walls, the viscosity and the
  step are; the drive is an initial velocity. Body force in the fractional step is a follow-up.
- The fractional runs take `non_orth_correctors = 0` (read 4: no coupled non-orthogonal correction in the fractional step).
- Translational pairs only in the runs; on rotational pairs the fractional step's coupled `R_f` takes the partner's
  pressure gradient unrotated (existing code, not changed) and the partner stays lagged.
- Incremental form only (`fractional_step_consistent` acts only there); not on a moving mesh; not with
  `rhie_chow_step_consistent` on the PISO side.
- One channel, 2-D, one layer. The defaults stay: whether the opt-ins become defaults is open (changes default results).

## Open questions

- **Q1. The implicit partner of the fractional step.** (a) a separate opt-in `fractional_periodic_implicit` (taken: the twin
  check needs it, read 3, and the control then differs by the ddt term only); (b) read it under `coupled_ddt_correction` (one
  field, but the control would carry the lagged partner and NC1f would measure the partner, not the term); (c) leave the
  partner lagged (c3f cannot be met). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation).
- **Q2. The fractional flow.** (a) ADR 0672's meshes with an initial velocity in place of the body force (taken: the fractional
  step reads no body force); (b) add a body force to the fractional step first (a new capability with its own balanced-force
  face treatment and wall condition, outside this item). Taken (a) on the standing instruction; flagged for the maintainer,
  who may read "ADR 0672's channel" as its flow (then the decision is partial, as registered).
- **Q3. The defaults.** Open (changes default results and hashes), as ADR 0675 Q3.

## Amendment 1 (2026-11-17): the BDF2 PISO run misses P1b; diagnosis, the record kept, a corrected set-up

Written after the one registered run of `coupled_ddt_fractional_bdf2_f757.rs` (5bb12a91, release, 8.4 s, log
`w49i-work/f757-scratch/fast.log`) and before any code or run of the corrected set-up. Nothing above this amendment is changed.

**What the registered run gave.** The fractional-step test (P1f, NC1f, P2f, D1Lf, C1f, NCL, D5, G0), the no-op test (D6) and
the fractional BDF2 test (S1fb, SNC1fb) passed (numbers in Addendum 1). The BDF2 PISO test missed **P1b** (`G_on` largest
`4.22e-4` U_ref at step 85, limit `1e-8`; step 1 `2.06e-15`, step 2 `2.20e-6`) and **P2b** (`8.28e-4`, limit `1e-8`); NC1b
held (`G_off` largest `4.19e-4`), D1Lb met (`1.39e-11`), C1b met (`4.4e-16`), G0 met. `G_on` and `G_off` are of the same size
from step 2: the opt-in removes the ddt-term part (`G_off` at step 2 is `9.2e-6`, `G_on` `2.2e-6`) but a second difference,
not the ddt term, remains.

**Diagnosis (from the code and the numbers, no further run).** In `step_with`, on the first outer iteration of a BDF2 step
(`extrap = Some(c1 - 1)`, from step 2) the convected flux and the deferred-correction state are extrapolated in time: the
internal faces' central correction is formed from `conv_cells = (1 + r) u^n - r u^{n-1}` on both sides. On a coupled face the
owner side is `conv_cells` but the partner value in the boundary coefficients (`co.b` of `self.u.boundary_coeffs`) is the
lagged `u^n`, so the deferred central part `phi_f (co.a u_r + co.b - u_upwind)` of `P`'s cut differs from that of `T`'s
internal face there by `phi_f (1 - a) r (u^n - u^{n-1})_c`. Size at step 2: `phi_f` about `8e-3 * 3.9e-3 = 3e-5`, `(1 - a) =
0.5`, `r = 1`, `u^1 - u^0` about `0.8 dt = 8e-3`: `1.2e-7` in the momentum row, over `a_P` about `1.5 V / dt + 0.025 = 0.062`:
`2e-6`, the measured `2.2e-6` at step 2. It is not in the Euler runs (no extrapolation; ADR 0675's P1 at `5.6e-15`) and not in
the fractional BDF2 run, whose predictor already overrides the coupled faces' `b`, `d` with the extrapolated partner
(`bc_extrapolated`, ADR 0043 D1; S1fb met at `1.1e-14`): the contrast isolates it. It is a pre-existing difference of the
default PISO BDF2 step on periodic faces, independent of the ddt term.

**The record.** The registered test is kept as `bdf2_piso_channel_against_the_twin_known_miss`, `#[ignore]`d, limits intact,
the measured numbers in its doc comment.

**The corrected set-up (registered now).** A third opt-in `PimpleSettings::periodic_partner_extrapolated: bool`, default
`false`: in the PISO step, on the first outer iteration of a BDF2 step (when `extrap` is set), the translational coupled faces'
boundary coefficients `b`, `d` of every velocity component take those of the extrapolated field (exactly the fractional step's
`bc_extrapolated`), and the implicit links remove that value (`fix_rhs` with `conv_cells` in place of `u_iter`). Rotational
pairs keep the lagged partner (their implicit links, ADR 0664, remove `u_iter`). Not read without `extrap` (implicit Euler,
later outer iterations, step 1 of BDF2): no new operation runs. Not read by the fractional step (which does it already). The
face values `ub` given to the least-squares momentum gradient stay lagged (not used with `coupled_non_orth`, which takes the
partner-point gradients from the extrapolated cells, nor by central convection).

Corrected checks, limits as registered for the originals (estimates: with the partner extrapolated every term of the coupled
face is the internal face's, as for Euler, so rounding, `1e-14`):

| Id | Set-up | Check | Limit |
| --- | --- | --- | --- |
| P1b' | ADR 0675's lockstep, BDF2, `periodic_partner_extrapolated` on in all four runs | every step `G_on(n)` | `<= 1e-8` each step |
| NC1b' | same | `G_off(n)` (no `coupled_ddt_correction`) | `> 1e-8` on at least one step (estimate `G_off` step 2 `9e-6` as measured) |
| P2b', D1Lb', C1b' | same | as P2b, D1Lb, C1b | `1e-8`, `1e-9`, `1e-12` each step |
| D7 | ADR 0675's lockstep, implicit Euler, 20 steps | `periodic_partner_extrapolated` on against off | bit for bit every step |
| D8 | BDF2 PISO on the mesh without pairs (D6's mesh, ADR 0675's settings without the body force, from D6's initial velocity), 20 steps | on against off | bit for bit every step |

Mutations added: **MX1** `periodic_partner_extrapolated` not read: must fail P1b'. MB1 and MB2 are judged on P1b' (the
original P1b is missed without them). B1 adds `w33b_transient_order` and `piso_thin_gap_f299` already in the list; nothing else
changes.

**Decision rule (amended).** c3b is judged on the corrected set-up ("a BDF2 PISO run"): in the registered rule read P1b',
NC1b', C1b', P2b', D1Lb' in place of P1b, NC1b, C1b, P2b, D1Lb, and add D7 and D8 to c2 and MX1 to the required mutations.
P1b and P2b stay recorded as missed with the cause above. If P1b' is missed, partial (diagnosed) or todo as registered.

## Addendum 1: results (2026-11-17)

Measured against the limits registered above and in amendment 1; no limit was changed. Release builds on Windows, on a machine
shared with other building agents. Commits: the registration 1a6b5031 (alone, first); the library change and the test file
5bb12a91 (before their run); amendment 1 8bd69bfd (after the registered run's P1b miss, before any code of the correction);
the correction and its tests fae938d8 (before their run). Each registered run was made once: the test file at 5bb12a91 (4
tests, 8.4 s, `fast.log`), the two amendment-1 tests at fae938d8 (7.4 s, `amend1.log`), `rcfd-fv --lib`, the named files,
`baseline_hashes`, `docs_site`. Logs in `w49i-work/f757-scratch`.

**Built.** In `crates/rcfd-fv/src/pimple.rs`: the fields `fractional_periodic_implicit` and `periodic_partner_extrapolated`
with their docs and defaults, the doc of `coupled_ddt_correction` (it is now read by the fractional step); the BDF2
extrapolated partner of the PISO predictor (`bc_extrapolated`, the override of the translational coupled faces' `b`, `d`, and
`q_lag = conv_cells` for the links, all only with the new field and an extrapolated step); `coupled_ddt_term` split into
itself and `coupled_ddt_defect` (the same operations: the PISO term's bits unchanged, F-754's test passes); the doc comment
of `ddt_history_weights` back above its function. In `crates/rcfd-fv/src/pimple/fractional.rs`: `coupled_harmonic`, the
implicit links of the predictor (`PeriodicLinks::build`, `fix_rhs` with `conv_cells`, `residual`, `solve`), and the coupled
faces' `Gamma R + (Gamma / dt) D` with `fractional_step_consistent` and `coupled_ddt_correction`. The new test file
`crates/rcfd-fv/tests/coupled_ddt_fractional_bdf2_f757.rs`. One passage of `docs/dynamic-mesh.md`. No case key, no golden, no
hash table.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P1f (c3f) fractional `G_on(n)`, 200 steps | `<= 1e-8` each step | `1.18e-14` (step 52; step 1 `2.67e-15`) | met |
| NC1f (c3f control) | `> 1e-8` on one step | largest `3.19e-4` (step 6); every step from 2 at least `2.05e-6`; step 1 `2.67e-15` | held |
| P2f pressure / `(0.5 U_ref^2)` | `<= 1e-8` each step | `1.38e-13` | met |
| D1Lf divergence / `(U_ref h dz)` | `<= 1e-9` each step | `5.5e-15` | met |
| C1f pair sums / `(U_ref h dz)` | `<= 1e-12` each step | `4.4e-16` | met |
| NCL without the implicit partner | `> 1e-6` on one step | `1.89e-2` (step 16) | held |
| D5 opt-in without `sc` | bit for bit | equal every step | met |
| D6 implicit partner without pairs | bit for bit, 20 steps | equal | met |
| P1b (c3b, as first registered) | `<= 1e-8` each step | `4.22e-4` (step 85) | **missed** (known miss, amendment 1) |
| NC1b | `> 1e-8` on one step | `4.19e-4` | held |
| P2b | `<= 1e-8` each step | `8.28e-4` | **missed** |
| D1Lb, C1b | `1e-9`, `1e-12` | `1.39e-11`, `4.4e-16` | met |
| P1b' (c3b, amendment 1) | `<= 1e-8` each step | `3.41e-15` (step 24) | met |
| NC1b' | `> 1e-8` on one step | largest `1.84e-5` (step 6); every step from 2 at least `8.07e-6`; step 1 `2.06e-15` | held |
| P2b', D1Lb', C1b' | `1e-8`, `1e-9`, `1e-12` | `8.4e-14`, `6.4e-15`, `4.4e-16` | met |
| D7 new field under Euler, 20 steps | bit for bit | equal | met |
| D8 new field without pairs, BDF2, 20 steps | bit for bit | equal | met |
| S1fb (supporting) fractional BDF2 | `<= 1e-8` each step | `1.09e-14` | met |
| SNC1fb | `> 1e-8` on one step | `5.31e-4` (step 3) | held |
| G0 | `1e-12`, one-to-one | `8.9e-16`, yes | met |
| B1 | pass | see below | met |

Printed, no limit: the fractional `T_on` against `T_off` `5.9e-6 U_ref` (Euler), `1.4e-5` (BDF2); the largest old-level
defect on `T_on`'s faces at the images of `P`'s cut `3.3e-3 U_ref h dz` (fractional; estimate up to `3e-3`), `4.3e-5` (PISO
BDF2, as ADR 0675's Euler); the fractional BDF2 pressure `3.2e-13`, divergence `5.5e-15`, pair sums `4.4e-16`. `G_off` of the
fractional run `2e-6` to `3.2e-4` against the estimate `1e-6` to `1e-3`.

Suites (release, run once, all passed; ignored records not run): `baseline_hashes` 3 passed, 1 ignored; `rcfd-fv --lib` 115
passed, 6 ignored; `coupled_ddt_correction_f754` 2; `skewed_periodic_channel_f752` 3; `rotational_sector_pressure_part_f753`
3; `periodic` 3; `consistent_periodic_f367` 13 (2 ignored); `fractional_step` 13 (3 ignored); `fractional_step_consistent_f454`
3; `w33b_default_bits` 1; `w33b_transient_order` 6 (9 ignored); `step_consistent_paths_f443` 5 (2 ignored);
`piso_thin_gap_f299` 18 (2 ignored; 75 s); `sliding_annulus_inlet_f744` 2 (1 ignored). `cargo fmt -p rcfd-fv --check` clean;
`cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean. `docs_site` (this ADR and the `docs/dynamic-mesh.md` passage): 2 passed.

### Mutations (scratch copy `w49i-work/f757-scratch/mut` of fae938d8, script `mutate.py`, target `target-mut`; the new test file run once per mutation)

| Mutation | Must fail | Result |
| --- | --- | --- |
| M0 (harness, none) | none | every test passes with the registered numbers |
| MF1 fractional coupled term not read | P1f | caught: P1f `3.19e-4` (the control's value), P2f; S1fb `5.3e-4` |
| MF2 `dte` kept in front of `R_f` | P1f | caught: P1f `1.13e-3`, P2f; S1fb |
| MF3 the coupled defect's sign | P1f | caught: P1f `1.21e-3`, P2f; S1fb |
| MB1 BDF2 weights swapped where `c2 != 0` | P1b' (S1fb); P1f met | caught: P1b' `2.86e-5`, P2b'; S1fb `2.3e-3`; P1f met |
| MB2 older level replaced by the old one | P1b' | caught: P1b' `2.65e-6`, P2b'; S1fb `2.1e-4` |
| ML1 `fractional_periodic_implicit` not read | P1f | caught: P1f `1.73e-2`, P2f; S1fb |
| MX1 `periodic_partner_extrapolated` not read | P1b' | caught: P1b' `4.2194e-4`, equal to the known-miss record |
| MF4 (diagnostic) arithmetic pair mean | none | caught too: P1f `2.24e-6`, S1fb `2.1e-6` |

Seven of seven required mutations caught; D5 to D8 held under every mutation.

### Deviations

- Amendment 1 (registered before its code and run): the first BDF2 PISO set-up missed P1b and P2b by a pre-existing
  difference (the lagged periodic partner of the BDF2 deferred-correction state), not by the ddt term; the record is kept as
  `bdf2_piso_channel_against_the_twin_known_miss`; c3b is met on the corrected set-up with a third opt-in.
- Amendment 1 says B1 adds `piso_thin_gap_f299` "already in the list": it was not in the registered B1 list; it was run
  (passed) because the PISO step changed. `sliding_annulus_inlet_f744` was run as well.

### Not covered

The fractional step's flow is not ADR 0672's body-force flow (the fractional step reads no body force; Q2); the fractional
runs have no non-orthogonal correction; translational pairs only (the fractional step's rotational pairs keep a lagged
partner and an unrotated partner pressure gradient; `periodic_partner_extrapolated` leaves rotational pairs lagged); the
incremental form only; not on a moving mesh or with `rhie_chow_step_consistent`; one channel, 2-D; the least-squares momentum
gradient's coupled face values stay lagged under BDF2 (not used by this set-up); no case key; the defaults unchanged (a
default BDF2 periodic PISO run still differs from its twin by up to `4.2e-4 U_ref` on this channel).

### Decision

By the rule as amended: P1f, NC1f, P1b', NC1b', C1f, C1b', D5, D6, D7, D8 and B1 met; P2f, P2b', D1Lf, D1Lb', G0 and NCL
held; MF1, MF2, MF3, MB1, MB2, ML1 and MX1 caught; S1fb and SNC1fb met. P1b and P2b of the first set-up are missed with the
cause diagnosed (amendment 1). **Done** on the amended rule, conditional on Q2: the registered rule gives partial if the
maintainer reads "ADR 0672's channel" as its body-force flow for the fractional run, which the fractional step cannot run.

### Remaining questions

1. **Q2, the fractional flow.** Options: (a) accept ADR 0672's meshes with an initial velocity (taken); (b) add a body force
   to the fractional step (follow-up below) and repeat the fractional check on the body-force flow. Recommendation (a) for
   this item, (b) as a follow-up if the fractional step is to be used with gravity or a driving body force.
2. **The BDF2 periodic default (new).** A default BDF2 periodic PISO run differs from its twin by the lagged partner of the
   extrapolated state (`4.2e-4 U_ref` here). Options: (a) make `periodic_partner_extrapolated` the default with BDF2 (changes
   default BDF2 periodic results and possibly hashes; maintainer); (b) keep it opt-in. Recommendation (a), together with ADR
   0675 Q3 and ADR 0667 Q2 (all make the coupled face the internal face); not taken (changes default results).
3. **The fractional step's implicit partner as default.** As 2 for `fractional_periodic_implicit` (the lagged partner gives
   `1.9e-2 U_ref` here). Recommendation: the same single change, by the maintainer.

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0687, 2026-11-17: the opt-in PimpleSettings::coupled_ddt_correction now also acts in the fractional step with fractional_step_consistent (ADR 0356's carried-defect flux; the coupled faces take Gamma R + Gamma/dt D with the pair-harmonic Gamma and ADR 0675's coupled defect, shared as coupled_ddt_defect), with a new opt-in fractional_periodic_implicit (the implicit periodic partner of the fractional predictor, without which the periodic run differs from its twin by 1.9e-2 U_ref). On ADR 0672's meshes (from an initial velocity: the fractional step reads no body force) the fractional run stays within 1.2e-14 U_ref of its unrolled twin on every one of 200 steps (limit 1e-8) against 2e-6 to 3.2e-4 without the opt-in. The BDF2 PISO run first missed (4.2e-4, kept as a known miss): the lagged periodic partner of the BDF2 deferred-correction state, a pre-existing difference; with a new opt-in periodic_partner_extrapolated the BDF2 PISO run with ADR 0672's flow stays within 3.4e-15 of the twin against 8e-6 to 1.8e-5 without coupled_ddt_correction; the fractional step with BDF2 within 1.1e-14. Pair fluxes opposite to 4.4e-16. Seven of seven mutations caught (term not read, dte kept, sign, BDF2 weights swapped, older level dropped, either new opt-in not read). Defaults bit-identical (baseline_hashes, the periodic, fractional and PISO suites pass). Translational pairs only, incremental form, no non-orthogonal correction in the fractional runs; whether the three coupled-face opt-ins become defaults is open for the maintainer.

> Follow-up proposed: "Body force in the fractional step". Deps F-757. Acceptance: the fractional step reads PimpleSolver::body_force behind an opt-in (the predictor's cell term, the balanced-force face term of the intermediate flux and the walls' pressure gradient as in the PISO step), defaults bit-identical; on ADR 0672's channel with its body force the fractional run with coupled_ddt_correction is within a registered limit of its twin on every step where the run without the opt-in is not, and a hydrostatic rest state stays at rest within a registered limit. Every limit not stated here is registered in the ADR before the run.
