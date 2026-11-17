# ADR 0675: F-754 registration: Rhie-Chow ddt correction on coupled faces

Status: **REGISTERED 2026-11-17, written before any code of F-754 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-754 was written and no run of any kind was made; the numbers quoted
from earlier work are those printed in ADR 0672 and ADR 0674. Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Roadmap item F-754 (deps F-752), proposed by [ADR 0672](0672-f752-registration-pressure-part-of-the-non-orthogonal-correction-on-coupled-faces.md)
remaining question 2. Parents: ADR 0672 (F-752: the skewed translational periodic channel `P` against its unrolled twin `T`,
run with `ddt_correction = false` because the default transient Rhie-Chow term acts only on internal faces),
[ADR 0674](0674-f753-registration-pressure-part-of-the-coupled-non-orthogonal-correction-on-rotational-pairs.md) (F-753: the
warped 60-degree sector `S` against its unrolled annulus `A`, the same settings, also with `ddt_correction` off),
[ADR 0667](0667-f744-registration-non-orthogonal-correction-on-coupled-faces.md) (`coupled_non_orth`), ADR 0080
(`coupled_body_pair_mean`). Number 0675 given by the lead. Branch `f754-work` from main 9bfc53e4 (F-752 and F-753 merged),
worktree `w49i-work/f754`, scratch `w49i-work/f754-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The transient Rhie-Chow correction is the repository's own (the
`ddt_defect` of `pimple.rs`: the old flux less the face-interpolated old velocity, times `rAU_f / dt`, the textbook
time-step-consistent momentum interpolation, cited here from memory as the idea of making the face flux independent of `dt` at
steady state; nothing beyond the repository's formula is used). The coupled-face form below is that formula with the partner
cell in the place of the neighbour, derived from the code read. The arithmetic below was done by hand before any run.

## What F-754 asks

The acceptance, binding (from the lead's brief):

> An opt-in giving the coupled (periodic) pairs the ddt correction the internal faces carry, defaults bit-identical; on ADR 0672's channel against its twin with ddt_correction on, the periodic run is within a registered limit of the twin on every step where the run without the opt-in is not. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **b1** an opt-in giving the coupled pairs the internal faces' ddt correction;
- **b2** defaults bit-identical;
- **b3** on ADR 0672's channel against its twin with `ddt_correction` on, the periodic run with the opt-in is within a
  registered limit of the twin on every step where the run without the opt-in is not.

The lead's note (2026-11-17, after F-753 merged): ADR 0674's sector against its annulus with `ddt_correction` on is a second,
supporting case; on rotational pairs the face velocity defect needs `R`. Registered below as supporting checks (S-checks) with
a mutation dropping `R`; it is cheap (ADR 0674's lockstep of six runs took 28 s).

**Reading of b3 (registered).** With `G_V(n) = max_c |u_V(n)_c - u_T(n)_m(c)| / U_ref` (ADR 0672's map `m`), the check is
`G_on(n) <= eps` on every step (P1), stronger than "on every step where the control is not". The clause is non-vacuous only
when the control `G_off(n) > eps` on at least one step (NC1, binding). At step 1 the old flux and the old velocity are both zero,
so the defect is zero on every face and the control equals the opt-in run at step 1 (to rounding): the control can only be
outside `eps` from step 2 on. NC1 therefore asks "at least one step", not every step.

**The twin with the opt-in (registered, Q1).** `T`'s own cut (at `P`'s `x = L/2`) is a pair of coupled faces of `T`, and those
faces are internal faces of `P`. With `ddt_correction` on, `P`'s internal faces there carry the term; for `T` to be the same
discrete problem its cut must carry it too, so the reference is `T` with the opt-in (`T_on`). This is not circular: every
coupled face of `P` is checked against an internal face of `T` and every coupled face of `T` against an internal face of `P`,
so a wrong coupled-face term shows in `G_on` whichever mesh carries it. `T` without the opt-in (`T_off`) is run and printed (the
size of the term on `T`'s cut, in the middle band where the cells are rectangles), no limit. In the supporting case the
annulus `A` has no coupled face at all, so its reference is free of the new term by construction.

## What the code read shows (9bfc53e4, read only)

1. `pimple.rs` `step_with`: the defect is computed once per step,
   `ddt_defect[f] = c1 (phi_old_f - U_old_f . S_f) - c2 (phi_older_f - U_older_f . S_f)` for `f < n_int`, with
   `U_f = w U_o + (1 - w) U_n` (`face_flux`, `geom.weights`), `(c1, c2) = ddt_history_weights` (`(1, 0)` for implicit Euler;
   the BDF2 weights otherwise). On a moving mesh (`self.ale`) the defect is `a.defect` instead. In every corrector, with
   `ddt_correction`, `phi_hby_a[f] += gamma_p[f] / dt * ddt_defect[f]` for `f < n_int` only. `gamma_p = face_interpolate(rAU)`
   (owner value on boundary faces), or the harmonic face value with `rhie_chow_step_consistent`.
2. Coupled faces get `phi_hby_a` from `boundary_hby_a`: `HbyA_f = w HbyA_r + (1 - w) R HbyA_c` with `w = dn_g / (dn_f +
   dn_g)` (`R` the pair's rotation, `I` for a translation), and the pressure equation and `corrected_flux` take the pair mean
   `coupled_gamma(gamma_p)` (`w gamma_r + (1 - w) gamma_c`) on the coupled faces, so the pair carries opposite fluxes.
3. `phi_hist.old` and `hist_u[k].old` hold the previous step's flux (every face, coupled ones included) and cell velocity;
   `History::push` at the end of the step.
4. The fractional-step path (`pimple/fractional.rs`) has its own copy of the defect; it is not touched here.

## Design

### The opt-in

`PimpleSettings::coupled_ddt_correction: bool`, default `false`, read only with `ddt_correction` and without a moving mesh.
With it, every coupled face `f` (a boundary face whose pressure condition is coupled, `pbc.coupled[i] = Some(g)`, not empty)
gets, in every corrector, the internal faces' term with the partner cell `c` in the place of the neighbour:

`phi_hby_a[f] += gamma_f / dt * D_f`, `gamma_f = w rAU_r + (1 - w) rAU_c` (the pair mean, `coupled_gamma`),

`D_f = c1 (phi_old_f - (w U_old_r + (1 - w) R U_old_c) . S_f) - c2 (phi_older_f - (w U_older_r + (1 - w) R U_older_c) . S_f)`.

Derivation: for a translational pair the pair is ADR 0672's internal face of the twin, with `w` the twin face's
`geom.weights` (ADR 0672 read 3: the same ratio), the old flux the twin face's old flux and `U_c` its neighbour's velocity;
the internal face's term is then `gamma_f / dt * D_f` term for term. For a rotational pair the partner's velocity is a vector
in the partner's frame, so it is rotated into the owner's frame by `R`, as `boundary_hby_a` does for `HbyA` (ADR 0629,
0664). The face value `w U_r + (1 - w) R U_c` is computed by `boundary_hby_a` itself (applied to the old velocities), so the
coupled face's velocity interpolation is the one its `HbyA` already uses. Conservation: on the partner face `g`, `w_g = 1 -
w_f`, `S_g = -R^T S_f` and `phi_old_g = -phi_old_f`, so `D_g = -D_f` and the pair mean `gamma` is the same on both: the two
faces receive opposite increments (to rounding), as the internal face gives its two cells.

Code: one helper in `pimple.rs` returning the list `(f, gamma_f D_f)`, built once per outer iteration after `pbc` and
`gamma_p` exist (`None` unless the opt-in, `ddt_correction` and no moving mesh), and three lines in the corrector after the
internal faces' term. Nothing else changes; with the field `false` no new operation runs (b2). Not reached: the fractional
step, a moving mesh (`ale`: the field is ignored there), and `rhie_chow_step_consistent` (the internal faces then take the
harmonic face value of `rAU`; the coupled faces here take the arithmetic pair mean; with that option the coupled face is not the
internal face, a limit stated below). No case key (a key changes the schema golden).

### The cases

**Binding: ADR 0672's channel.** Exactly ADR 0672's meshes, flow and settings (`P`, `T`, `box_2d(32, 16, 2, 1)`, tilt
`0.3`, `nu = 0.1`, body acceleration `(0.8, -2.0)`, hydrostatic start, `PimpleSettings::piso(0.1)`, central, one outer, two
correctors, six non-orthogonal passes, `TIGHT` solvers, `coupled_body_pair_mean = true`, `coupled_non_orth = true` (ADR 0667's
full setting, ADR 0672's `F`), `dt = 0.01`, 200 steps), with `ddt_correction = true`. Runs: `P_on` (the opt-in), `P_off` (no
opt-in, the control), `T_on` (the reference), `T_off` (printed), and for D3 `P_X` (opt-in with `ddt_correction = false`)
against `P_Y` (ADR 0672's `P_F` exactly, `ddt_correction = false`, no opt-in).

**Supporting: ADR 0674's sector.** Exactly ADR 0674's meshes, flow and settings (warped 60-degree sector, its six-copy
annulus, the `cos 6 theta` potential and the swirl driver, `rotational_periodic_implicit = true`, `coupled_non_orth = true`,
the rest as above), with `ddt_correction = true`. Runs: `S_on`, `S_off` (control), `A_on` (reference), `A_off` (`A` without
the opt-in, for D4).

## Estimates (before any run)

- **Size of the term.** `gamma / dt = rAU / dt = 4.9e-3 / 0.01 = 0.49` on the channel (ADR 0672's `rAU`); with Euler `c1 = 1`,
  `c2 = 0`. The defect `D` of a face is the old flux less the interpolated old velocity: `U_c = HbyA_c - rAU_c (grad p_c -
  a_c)`, `phi_f = HbyA_f . S - gamma_f (grad_f p - a_f) . S + 0.49 D_old`, the `HbyA` parts cancel, so `D` is the
  Rhie-Chow pressure-smoothing difference `interp(rAU (grad p - a)) . S - gamma (grad_f p - a_f) . S` plus `0.49` of the
  previous defect (a geometric recursion with ratio about `0.49`, bounded). For an exactly linear pressure with exact gradients
  and a uniform body force it vanishes; on the channel it comes from the pressure's departure from the hydrostatic line,
  caused by the trapezoidal cells of the tilt band (centroids off the column mid-line by `dy^2 t' / 12`, up to `3e-4` with
  `t' = 0.94`, at a velocity gradient up to `4 U_ref / H`: a flux imbalance of order `1e-3` of `U_ref h dz` in that band) and
  by the six-pass non-orthogonal iteration. I cannot derive it to better than two orders: estimate `D / (U_ref h dz)` between
  `1e-7` and `1e-5` at the cut, so the control's velocity defect `G_off` between `5e-8` and `5e-6` from step 2 on (the term's
  flux `0.49 D` spread over a cell face of `h dz`). On the sector the potential `cos 6 theta` is not linear across a cell:
  the least-squares gradient's and the face interpolation's error `h^2 / 8 |p'''|`, `h = r_m dtheta = 0.11`, `|p'''| about G
  (6 / r_m)^2 = 46`, `0.07` in the pressure gradient, times `gamma |S| = 5e-3 * 6e-3`: `D` up to `2e-6`, `3e-4` of
  `U_ref dr dz = 6.25e-3`; `G_off` up to about `1e-4` (likely less: the body force cancels much of it).
- **P1 (b3).** `P_on` against `T_on`: by the derivation every coupled face of either mesh carries the other's internal-face
  term; what differs is rounding and the iterative solves, as in ADR 0672 (`5.8e-15` there). The new term is a contraction
  (`0.49`) and adds no growth. Limit **`eps = 1e-8`** per step (ADR 0672's P1 limit; margin over `1e-14` large).
- **NC1 (b3, the control "is not").** `G_off(n) > 1e-8` on **at least one step**. From the estimate, `5e-8` or more from step
  2; margin 5 at the low end of an estimate I hold uncertain by two orders (if missed, the clause is vacuous on this flow: the
  decision rule then gives partial).
- **P2 (supporting).** Pressure `max_c |p_on - p_Ton| / (|g_y| H)`: **`1e-8`** per step (ADR 0672).
- **D1L (supporting, conservation).** `max_c |div_Pon(c) - div_Ton(m(c))| / (U_ref h dz)`: **`1e-9`** per step (ADR 0672).
- **C1 (b1, the pair carries opposite increments).** On `P_on`, `max_f |phi_f + phi_g| / (U_ref h dz)` over `P`'s 16 coupled
  pairs, every step: the default pair is antisymmetric to rounding (read 2; terms of `1e-4` to `1e-3` in flux, rounding
  `1e-16` relative); the new term is antisymmetric to rounding (derivation). Limit **`1e-12`** per step. Printed for `P_off`.
- **D3 (b2).** `P_X` against `P_Y`: velocity, pressure, flux, every step, **bit for bit** (the field is not read without
  `ddt_correction`).
- **D4 (b2).** `A_on` against `A_off`: every step **bit for bit** (no coupled face: the list is empty and nothing is added).
- **G0.** Both cell maps as registered in ADR 0672 and ADR 0674 (`1e-12`, one-to-one; the annulus maps a bijection).
- **S1 (supporting, the sector).** `S_on` against `A_on` over the six copies (ADR 0674's `vdiff`): **`1e-8`** per step.
- **SNC1 (supporting).** `S_off` against `A_on`: `> 1e-8` on at least one step (estimate up to `1e-4` from step 2).
- **SD1L (supporting).** Per-cell divergence of `S_on` against `A_on` over the six copies, in `U_ref dr dz`: **`1e-9`** per
  step.
- **B1 (b2).** `cargo test -p rcfd-api --release --test baseline_hashes`; `rcfd-fv --lib`; the named files
  `skewed_periodic_channel_f752`, `rotational_sector_pressure_part_f753`, `sliding_annulus_inlet_f744`, `periodic`,
  `consistent_periodic_f367`, `seam_no_harm_f344`, `non_conformal`, `piso_thin_gap_f299`: pass.
- **Cost.** The channel lockstep: six runs of 200 steps (ADR 0672's took 66 s); the sector: four runs (ADR 0674's six took
  28 s). Both under two minutes: no heavy record; the registered runs are the fast tests.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| P1 | b3 | channel, 200 steps | every step `G_on(n) = max_c abs(u_Pon - u_Ton) / U_ref` | `<= 1e-8` each step |
| NC1 | b3 (non-vacuous) | same | `G_off(n)` | `> 1e-8` on at least one step |
| P2 | b3 (supporting) | same | every step `max_c abs(p_Pon - p_Ton) / (abs(g_y) H)` | `<= 1e-8` each step |
| D1L | b1 (supporting) | same | every step `max_c abs(div_Pon(c) - div_Ton(m(c))) / (U_ref h dz)` | `<= 1e-9` each step |
| C1 | b1 | same | every step `max abs(phi_f + phi_g) / (U_ref h dz)` over `P_on`'s coupled pairs | `<= 1e-12` each step |
| D3 | b2 | same | `P_X` against `P_Y`, velocity, pressure, flux | bit for bit every step |
| G0 | set-up | the meshes | ADR 0672's and ADR 0674's cell maps | `1e-12`, one-to-one (bijection for `A`) |
| S1 | supporting (rotational) | sector, 200 steps | every step `max_j max_c abs(R_j u_Son - u_Aon) / U_ref` | `<= 1e-8` each step |
| SNC1 | supporting | same | `S_off` against `A_on` | `> 1e-8` on at least one step |
| SD1L | supporting | same | every step per-cell divergence `S_on` against `A_on` / (`U_ref dr dz`) | `<= 1e-9` each step |
| D4 | b2 | same | `A_on` against `A_off` | bit for bit every step |
| B1 | b2 | the suites named above | pass | as stated |

Printed, no limit: `G_on`, `G_off` at steps 1 and 2, their largest and its step; `T_on` against `T_off` (the term on `T`'s own
cut); the largest old-level defect `D / (U_ref h dz)` on `T_on`'s internal faces at the images of `P`'s cut (the term the
control lacks); C1 of `P_off`; the same for the sector (`S_off` largest, the largest `D` on `A`'s faces at the images of `S`'s
wrap faces, `phi_f + phi_g` of `S_on`); elapsed times.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the new test file run once per mutation;
each must fail the named checks):

- **MT1** the partner cell's old velocity from the wrong time level (`older`, level `n-1`, in place of `old`): must fail P1.
- **MT2** the coupled term with the opposite sign: must fail P1.
- **MT3** the owner's `rAU` in place of the pair mean `gamma_f`: must fail C1 or P1.
- **MT4** the rotation dropped in the partner's old velocity (`U_old_c` unrotated): must fail S1. (On the channel `R = I`, so
  P1 cannot see it: that is the reason for the supporting case.)
- **MT5** the field not read (the coupled term never added): must fail P1.

Diagnostic mutation (reported, not in the decision rule): **MT6** the weights swapped (`(1 - w) U_r + w R U_c`).

## Decision rule

**Done** when P1, NC1, C1, D3 and B1 are met (b1 to b3), P2, D1L, G0 held, and MT1, MT2, MT3, MT5 caught; S1, SNC1, SD1L, D4
and MT4 are supporting: reported, and their miss is reported as a finding but moves done to partial only if S1 or D4 is missed
(the rotational form or the default would then be wrong). **Partial** when P1 is met but NC1, C1, a supporting check or a
required mutation is missed, or P1 is missed with its cause diagnosed. **Todo** when P1 is missed without a diagnosis, or b2
fails (D3, D4 or B1).

## Limits (not covered)

- The PISO step only; the fractional step's own defect is not changed (follow-up if wanted); the field is ignored on a moving
  mesh; not with `rhie_chow_step_consistent` (harmonic face `rAU` on the internal faces).
- Implicit Euler only in the runs (`c2 = 0`): the BDF2 part of the coupled term is written by the same formula but not
  exercised.
- One channel and one sector, both 2-D (one layer), the settings of ADR 0672 and ADR 0674.
- The default stays: periodic runs with the default `ddt_correction` still lack the term on coupled faces. Making the opt-in
  the default changes default periodic results and the baseline hashes: open, for the maintainer.
- No case key.

## Open questions

- **Q1. The reference of the channel.** (a) `T` with the opt-in (taken: the twin is then the same discrete problem; every
  coupled face of either mesh is checked against an internal face of the other); (b) `T` without it (its own cut lacks the term,
  so `P_on` would differ from it by the term on `T`'s cut whatever this item does). Taken (a) on the maintainer's standing
  instruction of 2026-11-11 (take the recommendation).
- **Q2. NC1 "at least one step".** (a) at least one step (taken: at step 1 the defect is zero by construction on every face, so
  "every step" is impossible for any control); (b) every step from step 2. Taken (a) on the standing instruction; (b) is
  printed.
- **Q3. The default.** Whether the opt-in should become the default for periodic runs changes default results and hashes: not
  taken, open for the maintainer.
