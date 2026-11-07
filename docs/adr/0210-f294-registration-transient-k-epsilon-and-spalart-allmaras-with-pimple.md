# ADR 0210: F-294 registration: transient k-epsilon and Spalart-Allmaras (without DES) with pimple on a static mesh

Status: **REGISTERED 2026-11-07**, written before any code of F-294 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, two probes of existing capabilities were run on main 765ac64 (scratch tests, not committed): P1,
the steady SIMPLEC channel of ADR 0199 R1 with k-epsilon and with Spalart-Allmaras; P2, the ADR 0199 S3 time-order runs of
transient SST, as registered there and with an initial velocity that meets the walls. Their numbers are quoted in items 9 and 10,
where they set a run length and the start of S3. Results go in the addendum at the end.

Summary of the results (addendum): k-epsilon (every variant, wall function and wall treatment) and Spalart-Allmaras without DES
run with `pimple` on a static mesh, with the rotating frame too, and the defaults are bit-identical. Three set-ups were found
defective and corrected by amendments before their corrected runs (the S4 moving wall; twice the solver controls of HK, whose
wall row in block B set the stopping rule). Every corrected check met its limit except S3 for k-epsilon with BDF2, whose
observed order on the channel start-up (0.80 to 1.53) is below its band and rises towards 2 at finer levels (1.81 to 1.89); it
is an ignored known miss, with a diagnosis. Every negative control fired and every mutation made its check fail.

Parents: [ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md) (F-286: the outer-iteration path this item
reuses; question 2, option (b), recommendation taken by the lead; question 3, SA-DES and the Reynolds stress model stay on their
path; the addendum's S3 BDF2 known miss and question 6), [ADR 0205](0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md)
(F-295: the seam option, SST only), ADR 0014 (the SA model), ADR 0015 and ADR 0017 (the k-epsilon family, wall functions,
realizable `A0`), ADR 0168 (the SA inner-law wall mode), ADR 0032 (SA-DES and its once-per-step time derivative), ADR 0045 (the
transient source hook), ADR 0016 (the time schemes). Roadmap item F-294 (depends on F-286).

Number: the lead gave 0210 to F-294.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new code path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded, and no golden is re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the model equations are those already in `crates/rcfd-turb/src/kepsilon.rs` and `crates/rcfd-turb/src/sa.rs`. The
closed forms of homogeneous decay below are derived here from those equations. The time schemes are those of
`crates/rcfd-fv/src/ddt.rs`. No vendor code, text or install was used.

## What F-294 asks

The roadmap entry: k-epsilon and Spalart-Allmaras without DES run with the `pimple` algorithm on a static mesh, which the case
tree refuses today; each model gets its own registration with its time derivative (the existing k-epsilon kernel has none, and
the Spalart-Allmaras one is once per step and Euler only), checked against closed forms named in the ADR; steady runs and every
existing transient run are unchanged bit for bit. The note: SA-DES and the Reynolds stress model stay on their once-per-step
path (ADR 0199 question 3).

"Each model gets its own registration": this ADR has two self-contained parts, K (k-epsilon) and A (Spalart-Allmaras), each
with its own formulation, checks, limits, negative control and mutations, under the one number the lead gave (question 2).

## What the code read shows (main 765ac64)

1. **The refusal.** `turb::check` (`crates/rcfd-api/src/turb.rs`) accepts `k_omega_sst` with `pimple` (F-286), refuses it with
   `piso`, and refuses every other transported RANS model (`k_epsilon`, `spalart_allmaras` without DES, `plugin`) with any
   algorithm other than `simple` / `simplec`, located at `turbulence.model`, with the message `... is available with the steady
   algorithms (simple, simplec) only, not with solver.algorithm = "..."`. SA with `turbulence.sa.des` other than `off` is
   accepted with `piso` and `pimple` only.
2. **The outer-iteration path of F-286.** `TurbulenceModel::begin_time_step(dt, scheme)` (default: `begin_step(dt)`) and
   `TurbulenceModel::updates_in_outer_iterations()` (default `false`; `true` for SST). `Simulation::step` (`sim.rs`) asks the
   model: if `true`, the step only starts (`begin_time_step` with `solver.time_scheme`) and the hook of `step_with` makes one
   sweep at the start of every outer iteration and returns its face viscosity in `OuterUpdate::nu_face`; if `false`, the old
   path: `begin_step(dt)` and one `update` on the old level before the flow step. Nothing in that dispatch names SST, so a model
   that answers `true` takes the path with no change to `sim.rs`.
3. **The k-epsilon sweep** (`KEpsilon::update`). The k equation, then the epsilon equation, on the given velocity and flux. The
   k matrix: convection, diffusion `nu + nu_t* / sigma_k` (the eddy viscosity of the previous sweep on the faces), the
   production `G` (from `nu_t* S^2`, or the wall-function value in a wall cell) on the right, the dissipation linearised on the
   diagonal as `eps* / k*` (both from the start of the sweep; in a wall cell the wall-function epsilon). Then `relax(relax_turb)`,
   the solve, the floor `k >= 1e-10`. The epsilon matrix uses the new k `k~`: standard `C1 G eps*/k~` on the right and
   `C2 eps*/k~` on the diagonal; RNG with `C2* = C2 + C_mu eta^3 (1 - eta/4.38)/(1 + 0.012 eta^3)`, `eta = S k~/eps*` (a
   negative `C2*` goes to the right); realizable `C1 S eps*` on the right and `C2 eps* / (k~ + sqrt(nu eps*))` on the diagonal.
   Then `relax(relax_turb)`, the wall-cell epsilon fixed by a dominant diagonal, the solve, the floor `eps >= 1e-10`, the
   enhanced-wall blending (`Re_y < 200`). The eddy viscosity `C_mu k^2/eps` (variable `C_mu` for realizable, capped at
   `1e5 nu`) and the wall eddy viscosity are relaxed by `relax_nut` after the first sweep. There is no time derivative, and
   `KEpsilonModel` has the trait's no-op `begin_step`.
4. **The SA sweep** (`SpalartAllmaras::update`). One `nuT` equation: convection, diffusion `(nu + fn nuT)/sigma`, `cb2/sigma
   |grad nuT|^2` and the production `cb1 max(S~, 0) nuT*` on the right, the destruction linearised on the diagonal as
   `cw1 fw nuT*/d^2`; the negative branch (SA-neg) puts `cb1 Omega nuT* + cw1 nuT*^2/d^2` on the right. Then either the existing
   transient term or `relax(relax_turb)`, the wall-cell `nuT` fixed by a dominant diagonal (wall modes `auto` above `y+ = 5`, and
   `auto_sa_law`), the solve. The eddy viscosity, the wall eddy viscosity and (`auto_sa_law`) the internal-face eddy viscosity
   are relaxed by `relax_nut` after the first steady sweep.
5. **The existing SA time derivative.** `SpalartAllmaras::begin_step(dt)` stores `step_dt` and the old `nuT`; the next `update`
   adds `V/dt` on the diagonal and `V nuT^n/dt` on the right in place of the relaxation, takes the new eddy viscosity, and clears
   `step_dt`. This is SA-DES and SA-DDES on `piso` and `pimple`: once per step, before the flow step, Euler whatever
   `solver.time_scheme` says. The library tests of `crates/rcfd-turb/tests/des.rs` call it too. SA without DES never reaches it
   from the case tree (refused).
6. **The time derivative of the scalar equations** (`rcfd_fv::ddt::add_ddt` with a `History`): implicit Euler, variable-step
   BDF2 (Euler on the first step), bounded BDF2, as used by the SST sweep of F-286 (`KOmegaSst::begin_step`, `SstTime`).
7. **A wall is needed.** `rcfd_turb::wall::wall_distance` returns an error when no wall face is given, so both models need a
   wall patch, and the case tree refuses a RANS case without one. A cell with a wall face is a wall cell: k-epsilon fixes its
   epsilon from the wall function; SA in wall mode `auto` fixes its `nuT` only when the tangential speed is not zero.
8. **Registered tests that assert the refusal.** ADR 0199 S1 (`crates/rcfd-api/tests/f286_transient_sst.rs`,
   `s1_scope_of_transient_sst`, and the ignored record `s1_amendment1_record`) asserts that the CH channel with `k_epsilon` and
   with `spalart_allmaras` (DES off) on `pimple` is located at `turbulence.model`. Every other test found that asserts a refusal
   of these models (`sgs_tree.rs` T3 and T4, `des_tree.rs`, `w48z_followups.rs`, `runctl.rs`) uses `piso`, which stays refused,
   and T4 asserts that the message contains `steady`, which the new message keeps.
9. **Probe P1** (steady SIMPLEC, the CH channel of ADR 0199 R1, existing code). k-epsilon (`k_initial = 1`,
   `epsilon_initial = 3.6`, the `beta* k omega` of the SST text): the largest change of a cell `ux` per iteration is 8.5e-6 at
   2000, 8.1e-9 at 4000, 7.6e-12 at 6000, then between 1.6e-13 and 3.4e-13 (round-off of `ux` about 21) up to 40 000; maxima at
   40 000: `ux` 20.8727, k 3.0887, epsilon 43.957, nut 0.095279; `max uy / max ux` 5.3e-13; 40 000 iterations in 48 s. SA
   (`nutilde_ratio = 40`): 5.1e-6 at 2000, 3.5e-9 at 4000, 2.3e-12 at 6000, then 1.8e-13 to 3.6e-13 up to 40 000; maxima:
   `ux` 18.8642, nuTilda 0.094955, nut 0.094316; `max uy / max ux` 7.7e-13; 40 s. Both settle by 8000 iterations, as SST did.
10. **Probe P2** (transient SST on `pimple`, the code of F-286). The ADR 0199 S3 runs (CH from `u = 15` everywhere, walls at
    rest, `bdf2`, `n_outer = 20`, T = 2, T/80, T/160, T/320) give `p` 2.6694 / 2.4075 / 2.5729 (`ux` / k / omega), the numbers
    of the ADR 0199 addendum to the printed digits. The same runs from the initial velocity `ux = 15 y (2 - y)` (zero on both
    walls, the same centre speed) give `p` 2.0450 / 2.0394 / 2.0306 for `bdf2` and 1.0030 / 0.9800 / 1.0028 for `euler`. At
    T = 0.5 the uniform start gives 1.8048 / 1.7578 / 1.8046 and the parabolic one 1.9489 / 2.0771 / 1.9489. So the S3 BDF2 miss
    of ADR 0199 comes from the start, not from the coupled step: a uniform velocity next to a wall at rest is an impulsive start,
    whose solution is not smooth in time at `t = 0`. S3 below starts from the parabola.

## Decisions (fixed now)

**D1 (part K). The k-epsilon formulation.** Step `n -> n+1` of size `dt`, outer iterations `m = 0 .. N-1` of `pimple`. At the
start of outer iteration `m` the model makes one sweep on the current iterate `(u^(m), phi^(m))`. The sweep is the steady sweep
of `KEpsilon::update` (point 3: the same constants, variants, wall functions, wall treatments, roughness, floors, cap and
enhanced blending), with two changes:

- the implicit under-relaxation of the k and epsilon matrices is replaced by the time derivative of `add_ddt` with the momentum's
  time scheme, `(V/dt)(c0 q - c1 q^n + c2 q^(n-1))` for `q = k, epsilon`, with the weights of ADR 0199 D1 (Euler, and the first
  step of either BDF2: `(1, 1, 0)`; BDF2: `((1 + 2r)/(1 + r), 1 + r, r^2/(1 + r))`, `r = dt/dt_old`; bounded BDF2: the per-cell
  blend of ADR 0016). The levels `q^n`, `q^(n-1)`, `q^(n-2)` and the step sizes are fixed for the whole step. The wall-cell
  epsilon is fixed after the time term, as the steady sweep fixes it after its relaxation;
- the eddy viscosity and the wall eddy viscosity take the new value of the sweep (no `relax_nut`).

So, with `G`, `nu_t*`, `eps*`, `k*` from the start of the sweep and `k~` the k of this sweep (standard variant written out):

```text
(V/dt)(c0 k - c1 k^n + c2 k^(n-1)) + conv(phi^(m), k) - diff(nu + nu_t*/sigma_k, k) = V (G - (eps*/k*) k)
(V/dt)(c0 e - c1 e^n + c2 e^(n-1)) + conv(phi^(m), e) - diff(nu + nu_t*/sigma_e, e) = V (C1 G eps*/k~ - C2 (eps*/k~) e)
```

If the outer iterations converge, their fixed point is the fully implicit coupled step. The old levels are stored when a step
starts: the first step starts the history from the present fields, every later step pushes the fields at the end of the previous
step with that step's size (as `KOmegaSst::begin_step`). The steady path never starts a step, so it is unchanged.

**D2 (part A). The Spalart-Allmaras formulation (DES off).** The same structure for `nuT`: one sweep of
`SpalartAllmaras::update` at the start of every outer iteration, with the steady sweep's operations (point 4: every wall mode,
limiter, the negative branch, the cb2 term), the relaxation replaced by the time derivative of `add_ddt` with the momentum's
scheme (levels fixed for the step; the wall-cell `nuT` fixed after the time term), and the eddy viscosity, the wall eddy
viscosity and the `auto_sa_law` face eddy viscosity taking the new value. The history is a new state started by a new method
`SpalartAllmaras::begin_time_step(dt, scheme)` (first step from the present field, later steps push the end of the previous step
with its size). It is separate from the existing `begin_step(dt)` (point 5), which is unchanged in what it does for SA-DES and
SA-DDES: in `update` the existing `step_dt` branch is tested first, then the new state, then the relaxation; `begin_step` also
clears the new state (which is empty on every existing path), so a model that changes `des` in a live session takes one path or
the other, never both.

**D3. Where the sweep runs.** No change to `sim.rs` beyond its comment: the F-286 dispatch (point 2) is chosen by the model.

- `KEpsilonModel`: `updates_in_outer_iterations()` is `true`; `begin_time_step(dt, scheme)` starts the step of D1;
  `begin_step(dt)` starts an Euler step of D1 (as `SstModel` does).
- `SpalartAllmaras` (the trait impl): `updates_in_outer_iterations()` is `des == off`; `begin_time_step(dt, scheme)` starts the
  step of D2 when `des == off` and calls `begin_step(dt)` otherwise; `begin_step(dt)` is the existing one.

SA-DES and SA-DDES therefore keep the once-per-step Euler path before the flow step (ADR 0199 question 3).

**D4. Scope.**

- Accepted: `turbulence.model = "k_epsilon"` (every `variant`, `wall_function`, `wall_treatment`, roughness, `realizable_a0`)
  and `turbulence.model = "spalart_allmaras"` with `turbulence.sa.des = "off"` (every `sa.wall_mode`, `sa.limiter`,
  `sa.negative_nutilde`), with `solver.algorithm = "pimple"` (constant density, the iterative time advance, a static mesh), every
  `time_scheme` of the case tree, every `n_outer`, laminar regions, with or without sources (`rotating_frame` included), lumped
  components and two-way particles.
- Located at `turbulence.model`: both models with `piso` (DES off), with a message that names `pimple` and keeps the word
  `steady`. `pimple` with `n_outer = 1` is the lagged once-per-step update and stays allowed, as for SST (ADR 0199 question 1).
- Unchanged: SA-DES and SA-DDES with `piso` and `pimple`; `plugin` refused with `piso` and `pimple`; the Reynolds stress model;
  `turbulence.periodic_seam.diffusivity = "pair"` refused for these models at its key (ADR 0205, F-296); every refusal of ADR
  0199 point 6 (dynamic mesh, step-doubling error control, the fractional step, VOF, the energy equation with RANS, checkpoints,
  warm starts, interpolation).

**D5. No under-relaxation on the transient path.** `turbulence.relax` and `turbulence.relax_nut` are not used by these models
with `pimple` (DES off), as for SST (ADR 0199 D4). The time derivative takes the place of the relaxation.

**D6. Code (the plan, with the files).**

- `crates/rcfd-turb/src/kepsilon.rs`: a private transient state (scheme, step size, the k and epsilon `History`; `None` on the
  steady path), `KEpsilon::begin_step(dt, scheme)` and `KEpsilon::is_transient()`; in `update`, the branch of D1 where the steady
  code calls `relax` and chooses the relaxation factor of the eddy viscosity.
- `crates/rcfd-turb/src/sa.rs`: the same state for `nuT`, `SpalartAllmaras::begin_time_step(dt, scheme)` and
  `SpalartAllmaras::is_transient()`; in `update`, the branch of D2; `begin_step` clears the state.
- `crates/rcfd-turb/src/model.rs`: the trait methods of D3.
- `crates/rcfd-api/src/turb.rs`: the scope branch of D4 and its message.
- `crates/rcfd-api/src/sim.rs`: the comment of the dispatch (it names SST); no code.
- `crates/rcfd-api/src/schema.rs` doc strings and the k-epsilon, SA and solver lines of `docs/case-schema.md`; no case key is
  added or changed.
- `crates/rcfd-api/tests/f286_transient_sst.rs`: the two S1 cases of point 8 are superseded by D4. They become "CH with
  `k_epsilon` and with `spalart_allmaras` on `pimple` is accepted with no diagnostic"; the other cases of ADR 0199 S1 stay, and
  a comment names this ADR. This is the only edit of an existing test.

**D7. Why the defaults stay bit-identical.** No case key is added, so every settings value, content hash and written text is
unchanged and `case_hashes_w48u.rs` needs no edit. Every case that was valid before is one of:

- steady: the model never starts a step, the new state stays `None`, and the sweep runs the same operations in the same order;
- SA-DES or SA-DDES on `piso` / `pimple`: `updates_in_outer_iterations()` is `false`, so the dispatch, `begin_step` and the
  `step_dt` branch of `update` are those of today, and the new state stays `None`;
- a transient run of another model (SST, the Reynolds stress model, the algebraic models, laminar): no line it runs changes.

V0 checks this.

## Checks and limits (fixed now)

Every check is run once, in release. Every check is a gate test unless marked otherwise.

### Part K: library checks (`crates/rcfd-turb/tests/f294_transient_kepsilon.rs`)

**Homogeneous set-up HK.** A two-block mesh. Block A: `box_2d(4, 4, 1.0, 1.0)` with no periodic direction; its sides `xmin`,
`xmax`, `ymin`, `ymax` are not walls, and k and epsilon have their default zero gradient there. Block B: `box_2d(1, 1, 1.0, 1.0)`
translated by `(3, 0, 0)` (`rcfd_mesh::edit::transform_mesh`), its four sides combined into one patch `wall`
(`rcfd_mesh::edit::merge_patches`). The mesh is `merge_meshes(A, B, 1e-9)`; `wall` is the model's only wall. B gives the model
the wall that `wall_distance` needs (point 7); no cell of A is a wall cell and A and B share no face, so the equations of A in a
sweep do not see B. Model: `KEpsilon::new` with walls `["wall"]`, standard wall function and smooth treatment (the defaults),
`nu = 0.01`, `k0 = 1`, `eps0 = 1`, `u = 0`, `phi = 0`, linear solver `tol_rel = 1e-14`, `tol_abs = 1e-300`, `max_iter = 1000`.
In A: `S = 0`, so `G = 0` and (RNG) `eta = 0`, `C2* = C2`; a uniform field has no convection or diffusion with zero-gradient
sides; every cell of A obeys `dk/dt = -eps`, `d eps/dt = -C2 eps^2 / k` (standard `C2 = 1.92`, RNG 1.68), with the closed form

```text
k(t) = k0 (1 + t/t0)^(-n),   eps(t) = eps0 (1 + t/t0)^(-n-1),   n = 1/(C2 - 1),   t0 = n k0/eps0
```

Realizable (`C2 = 1.9`): `d eps/dt = -C2 eps^2 / (k + sqrt(nu eps))`, no closed form; its recursion below.

`M` sweeps per step (calls of `update` with `u`, `phi` fixed) stand for the outer iterations. The fully implicit recursion of a
step, with `R_k = c1 k^n - c2 k^(n-1)`, `R_e = c1 e^n - c2 e^(n-1)` (the fixed point of the sweep: `k* = k~ = k'`,
`eps* = e'`): `k' = (R_k - dt e') / c0` and, for standard and RNG,
`e' = 2 R_e R_k / (b + sqrt(b^2 + 4 a R_e R_k))` with `a = dt c0 (C2 - 1)`, `b = c0 R_k + dt R_e` (the positive root of
`a e'^2 + b e' - R_e R_k = 0`); for realizable `e'` is the root in `(0, R_e/c0)` of
`c0 e' - R_e + dt C2 e'^2 / ((R_k - dt e')/c0 + sqrt(nu e')) = 0`, found by 200 bisection halvings. The lagged recursion of one
sweep (Euler): `k' = k^n / (1 + dt e^n/k^n)`, `e' = e^n / (1 + dt C2 e^n/k')`.

| Id | Set-up | Limit |
|---|---|---|
| HK0 | precondition of every HK run, every sweep | k and epsilon uniform over A to `1e-13` relative; no face of `wall` has an owner in A; no internal face joins A and B (a miss is a defective set-up) |
| K1 | HK, standard and RNG, `euler` and `bdf2`, `dt = 0.1 k0/(C2 eps0)`, 40 steps, `M = 30`, against the fully implicit recursion | every step, k and epsilon of A: relative error `<= 1e-12` |
| K1r | HK, realizable, `euler` and `bdf2`, `dt = 0.1/1.9`, 40 steps, `M = 30`, against the realizable recursion | every step: `<= 1e-12` |
| K1v | HK, standard, `bdf2`, steps alternating `1.2 h` and `0.8 h`, `h = t0/40`, 40 steps, `M = 30` | every step: `<= 1e-12` |
| K1b | HK, standard, `euler`, `M = 1`, the K1 steps, against the lagged recursion | every step: `<= 1e-12` |
| K1N | negative control: the K1b run against the fully implicit recursion | largest relative difference `>= 1e-3` (the recursions differ by 5.15e-2) |
| K2 | HK, standard and RNG, `M = 30`, `T = t0`, `N = 160` and `320` equal steps; `e` = the largest relative error over the steps against the closed form, k and epsilon; `p = log2(e(160)/e(320))` | `euler`: `p` in [0.9, 1.1], `e(320) <= 5e-3`; `bdf2`: `p` in [1.9, 2.1], `e(320) <= 1e-4` |
| K2v | HK, standard, `bdf2`, the K1v alternation with `h = t0/N`, `N = 160` and `320` | `p` in [1.9, 2.1]; `e(320) <= 1e-4` |
| K2b | HK, standard, `bdf2_bounded`, `N = 160` and `320` | report only |
| KC1 | conservation: block A `box_2d(16, 16, 1.0, 1.0)` with block B as in HK; the flux of the stream function `psi = (1/pi) sin(pi x) sin(pi y)` on A as ADR 0199 amendment 2 (with its precondition C1-0, B's faces carry 0), `u = 0`, `k = 1 + 0.5 sin(2 pi x)`, `eps = 1`, `nu = 0.01`, standard; `euler` and `bdf2`, `dt = 0.05`, 10 steps, `M = 30`, convection `upwind` and `linear_upwind`, the HK solver controls. `B_K = sum over A of V_c ((c0 k' - c1 k^n + c2 k^(n-1))/dt + (eps*_c/k*_c) k'_c)`, with `eps*`, `k*` the fields at the start of the last sweep | every step `abs(B_K) <= 1e-10 sum over A of V_c (eps*_c/k*_c) k'_c`; non-vacuity: `max over A abs(k' - k^n) >= 1e-3` every step |

K2 reads the largest error over the steps, not the error at `T`: the leading `dt^2` term of the BDF2 k error at a fixed time
changes sign near `t = 1.5 t0` (computed with the recursion: at `T = t0` the end-time k order is 2.86 at N = 80 and 2.99 at
N = 1280), so the end-time k error does not show the order of the scheme there.

The values the recursions give (computed with the formulas above, outside the code). K1 step: standard 0.052083, RNG 0.059524.
K2 standard: Euler k `e(160)` 1.5069e-3, `e(320)` 7.5430e-4, `p` 0.9983; epsilon 5.8787e-3, 2.9421e-3, 0.9987; BDF2 k 6.1050e-5,
1.5823e-5, 1.9480; epsilon 1.7448e-4, 4.5079e-5, 1.9526. K2 RNG: Euler k 2.4149e-3, 1.2085e-3, 0.9988; epsilon 7.5375e-3,
3.7705e-3, 0.9993; BDF2 k 9.7507e-5, 2.5307e-5, 1.9460; epsilon 2.3122e-4, 5.9866e-5, 1.9494. K2v: k 7.8347e-5, 2.0288e-5,
1.9493; epsilon 2.2414e-4, 5.7828e-5, 1.9546.

### Part A: library checks (`crates/rcfd-turb/tests/f294_transient_sa.rs`)

**Homogeneous set-up HA.** `box_2d(4, 1, 1.0, 1.0)` with no periodic direction: one row of four cells between the walls `ymin`
and `ymax`, the model's walls, with `nuT` at zero gradient on them (replacing the wall value 0); `xmin`, `xmax` at their default
zero gradient. Every cell centre is at `d = 0.5` from both walls. Model: `SpalartAllmaras::new` with the default coefficients,
limiter (Allmaras), negative branch on and wall mode `auto` (inactive at zero speed, point 7), `nu = 0.01`, `nuT0 = 16 nu`,
`u = 0`, `phi = 0`, the HK solver controls. With `Omega = 0` and `chi = nuT/nu` in (1.0028, 18.397), `fv2(chi) < 0`, so
`sbar < 0 = -c2 Omega`, the Allmaras limiter gives `S~ = 0`, so `r = 10`, the production is 0, and `grad nuT = 0`; every cell
obeys `dnuT/dt = -a nuT^2`, `a = cw1 fw(10)/d^2 = 25.979588` (`cw1` 3.2390678, `fw(10)` 2.0051747), with the closed form

```text
nuT(t) = nuT0 / (1 + a nuT0 t),   T = 1/(a nuT0) = 0.24057347 (nuT halves, chi from 16 to 8)
```

The fully implicit recursion of a step: `nuT' = 2 R / (c0 + sqrt(c0^2 + 4 a dt R))`, `R = c1 nuT^n - c2 nuT^(n-1)`; the lagged
recursion of one Euler sweep: `nuT' = nuT^n / (1 + dt a nuT^n)`.

| Id | Set-up | Limit |
|---|---|---|
| HA0 | precondition of every HA run, every sweep | `nuT` uniform to `1e-13` relative; the wall distance of every cell `0.5` to `1e-13` relative; at the start of every sweep, in every cell, `sa_s_fw(nuT_c, 0, d_c, nu, coefficients, Allmaras)` is `(0, fw(10))` exactly (a miss is a defective set-up) |
| A1 | HA, `euler` and `bdf2`, `dt = 0.1 T`, 40 steps, `M = 30`, against the fully implicit recursion | every step: relative error `<= 1e-12` |
| A1v | HA, `bdf2`, steps alternating `1.2 h` and `0.8 h`, `h = T/40`, 40 steps, `M = 30` | every step: `<= 1e-12` |
| A1b | HA, `euler`, `M = 1`, the A1 steps, against the lagged recursion | every step: `<= 1e-12` |
| A1N | negative control: the A1b run against the fully implicit recursion | largest relative difference `>= 1e-3` (the recursions differ by 3.45e-2) |
| A2 | HA, `M = 30`, to `T`, `N = 40` and `80` equal steps; `e` = relative error at `T` against the closed form; `p = log2(e(40)/e(80))` | `euler`: `p` in [0.9, 1.1], `e(80) <= 1e-2`; `bdf2`: `p` in [1.9, 2.1], `e(80) <= 1e-4` |
| A2v | HA, `bdf2`, the A1v alternation with `h = T/N`, `N = 80` and `160` | `p` in [1.9, 2.1]; `e(160) <= 5e-5` |
| A2b | HA, `bdf2_bounded`, `N = 40` and `80` | report only |
| AC1 | conservation: `box_2d(16, 16, 1.0, 1.0)` with no periodic direction, all four sides the model's walls with `nuT` at zero gradient, `u = 0`, the stream-function flux of KC1 (C1-0), `nuT = nu (10 + 5 sin(2 pi x))`, `nu = 0.01`; `euler` and `bdf2`, `dt = 0.05`, 10 steps, `M = 30`, `upwind` and `linear_upwind`, the HK solver controls. With the start of the last sweep `x*` and its terms in each cell as the kernel forms them (point 4; `x* >= 0`: `P* = cb1 max(S~*, 0) x*`, `D* = cw1 fw* x*/d^2` with `(S~*, fw*) = sa_s_fw(x*, 0, d, nu)`; `x* < 0`: `P* = cw1 x*^2/d^2`, `D* = 0`; `Q* = (cb2/sigma) abs(grad x*)^2`, Green-Gauss with the field's boundary values): `B_A = sum_c V_c ((c0 x' - c1 x^n + c2 x^(n-1))/dt - P* - Q* + D* x')` | every step `abs(B_A) <= 1e-10 sum_c V_c (abs(P*) + Q* + abs(D* x'))`; non-vacuity: `max abs(x' - x^n) >= 1e-3 max abs(x^n)` every step |

A1 and A2 obey the equation of the SST omega of ADR 0199 D2 in normalised time (`d w/dt = -beta1 w^2` there), so the recursion
gives the same numbers: Euler `e(40)` 8.5548e-3, `e(80)` 4.3046e-3, `p` 0.9909; BDF2 1.5510e-4, 3.8869e-5, 1.9965; alternating
BDF2 `e(80)` 7.0734e-5, `e(160)` 1.7820e-5, `p` 1.9889. Over every A run `chi` stays in [3.3, 16], inside the range of HA0.

### Case-tree checks (`crates/rcfd-api/tests/f294_transient_keps_sa.rs`)

**Channels CH-K and CH-A.** The CH text of ADR 0199 (`box_2d` 4 x 20, `lx = 1`, `ly = 2`, periodic in x, `nu = 1/395`, walls
`ymin`, `ymax` at rest, the drive acceleration `(1, 0, 0)` on all cells, momentum and turbulence convection `upwind`) with, in
place of its SST lines, `model = "k_epsilon"`, `k_initial = 1`, `epsilon_initial = 3.6` (CH-K) or `model = "spalart_allmaras"`,
`nutilde_ratio = 40` (CH-A, `nuT0` 0.101, near the steady maximum 0.095 of P1). The initial velocity is `(15, 0, 0)` except in
S3. Wall defaults: k-epsilon standard wall function, smooth treatment; SA wall mode `auto` (first cell centre at `y+` about 20).

| Id | Set-up | Limit |
|---|---|---|
| S1 | scope, through `Case::parse_toml`, `semantic_errors` and `Simulation::new` | accepted with no diagnostic: CH-K on `pimple` with `euler`, `bdf2`, `bdf2_bounded`; with `variant = "rng"`, `"realizable"`; with `wall_function = "scalable"`, `"enhanced"`; with `wall_treatment = "hysteresis"`; CH-A on `pimple` with `euler`, `bdf2`, `bdf2_bounded`; with `sa.wall_mode = "resolved"`, `"auto_sa_law"`; with `sa.limiter = "min_0p3"`; with `sa.negative_nutilde = false`; CH-A with `sa.des = "ddes"` on `pimple` and on `piso` (unchanged); the two S4 texts. Located at `turbulence.model`: CH-K and CH-A on `piso` (the message names `pimple` and contains `steady`); CH-K and CH-A on `pimple` with `energy.enabled`. Located at `dynamic_mesh.enabled`: both on `pimple` with a dynamic mesh. Located at `solver.error_tolerance`: both on `pimple` with `error_tolerance = 1e-3`. Located at `solver.time_advance`: both without the drive on `pimple` with `fractional_step`. Located at `turbulence.periodic_seam.diffusivity`: both on `pimple` with `diffusivity = "pair"`. Through the registry (`rcfd_turb::create_model` and `set_option`): `updates_in_outer_iterations()` is `true` for `k_epsilon` and for `spalart_allmaras` with `des = off`, `false` with `des = des` and `ddes` |
| R1 | reduction to the steady solution, each of CH-K and CH-A: `simplec`, 10 000 iterations; `pimple`, `euler`, `dt = 0.2`, `n_outer = 2`, 5000 steps (t = 1000) | settled: the last step's largest cell change of `ux` `<= 1e-11 max abs(ux)` (else NOT SETTLED, a known miss, no re-run without an amendment); then `max_c abs(q_P - q_S) <= 1e-8 max_c abs(q_S)` for `q = ux, k, epsilon, nut` (CH-K) and `q = ux, nuTilda, nut` (CH-A), and `max abs(uy) <= 1e-8 max abs(ux)` in all four runs |
| S3 | time order through the case tree, each of CH-K and CH-A from the initial velocity `ux = 15 y (2 - y)` (`velocity_expression_x = "15 [m/s] * (y / 1 [m]) * (2 - y / 1 [m])"`, P2): `pimple`, `n_outer = 20`, `n_correctors = 2`, `momentum_rel_tol = pressure_rel_tol = 1e-12`, `pressure_max_iter = 1000`, to `T = 2`, `dt = T/80, T/160, T/320`; `e_ij = max_c abs(q_i - q_j) / max_c abs(q_finest)` at `T`, `q = ux, k, epsilon` (CH-K), `q = ux, nuTilda` (CH-A); `p = log2(e(80,160)/e(160,320))` | `bdf2`: `p` in [1.7, 2.3] for each `q`; `euler`: `p` in [0.8, 1.2] for each `q`. Reported: the largest `nut_change` of a step |
| S3N | negative control: S3 `bdf2` with `n_outer = 1`, each model | the smallest `p` of its fields `<= 1.3` |
| S4 | rotating frame: the S4 sector text of ADR 0199 with `k_epsilon` (inlet `turbulence_spec = "values"`, `k_value = 0.01`, `epsilon_value = 0.009`) and with `spalart_allmaras` (inlet default `nuT = 3 nu`), `outer` the `moving_wall` with `turbulence = "wall"`; 10 steps each | every step finite, no guard trip; k-epsilon: k and epsilon positive; seam pair sum `max abs(phi_f + phi_g) / max abs(phi_f) <= 1e-8`. Reported: `max nut / nu`, the smallest `nuTilda` |

S3 keeps the bands of ADR 0199 S3 and changes only the start, for the reason of P2: from the impulsive start the SST runs miss
the BDF2 band above it, from the parabola they meet it.

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs`, unedited; `f286_default_bits.rs` and `f295_default_bits.rs`, unedited | pass |
| V0b | `crates/rcfd-api/tests/f294_default_bits.rs`: FNV-1a (as `f286_default_bits.rs`) of the bits of every cell field the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut` and of the face flux, recorded on 765ac64 (this text plus the test, no code of F-294) and committed before the code: steady SIMPLEC CH-K (300 iterations); steady SIMPLEC CH-K realizable with the enhanced wall function (300); steady SIMPLEC CH-K RNG with the scalable wall function and the hysteresis treatment (300); steady SIMPLEC k-epsilon on the S4 sector with the frame (100); steady SIMPLEC CH-A (300); steady SIMPLEC CH-A with `auto_sa_law` (300); SA-DES (`des = "des"`, the `des_tree.rs` text) on `piso` (20 steps); SA-DDES on `pimple` with `n_outer = 2` and no source (20 steps, the non-hook path); transient SST CH on `pimple`, `bdf2`, `n_outer = 3` (20 steps, the F-286 path) | bit-identical after the code |
| V0c | the full gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each mutation is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the addendum.

| Id | Mutation | Must fail |
|---|---|---|
| M1 | the k-epsilon transient branch keeps the relaxation of the k matrix (no time derivative in k) | K1 |
| M2 | the k-epsilon transient sweep uses Euler whatever the scheme | K2 (`bdf2`) |
| M3 | the k-epsilon history push takes the new step size instead of the previous one | K1v |
| M4 | `KEpsilonModel::updates_in_outer_iterations` returns `false` (the once-per-step path) | S3 CH-K (`bdf2`) |
| M5 | the SA transient branch keeps the relaxation (no time derivative) | A1 |
| M6 | the SA transient sweep uses Euler whatever the scheme | A2 (`bdf2`) |
| M7 | the SA history push takes the new step size instead of the previous one | A1v |
| M8 | SA's `updates_in_outer_iterations` returns `false` with DES off (the once-per-step Euler path of point 5) | S3 CH-A (`bdf2`) |
| M9 | `turb::check` accepts both models with `piso` | S1 |

### Classes

V0 is a precondition of landing: if V0a or V0b misses, the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control (K1N, A1N,
S3N) or a mutation that does not make its check fail means that the check is not shown to be able to fail: it is reported as a
defective set-up, corrected by an amendment before any re-run, and never counted as a pass. If HK0 or HA0 misses, the closed-form
checks of that part are a defective set-up of the same kind.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | K1, K1r, K1v, K1b, A1, A1v and A1b meet at round-off (below 1e-13); K2, K2v, A2 and A2v reproduce the recursion values above to the printed digits |
| PR2 | KC1 and AC1 close near 1e-14 |
| PR3 | R1 meets for both models at 1e-10 to 1e-9 (as SST in ADR 0199), not at round-off |
| PR4 | S3 meets for both models: `bdf2` near 2, `euler` near 1 (as SST from the parabola in P2); S3N gives `p` near 1 |
| PR5 | S4 meets; the seam pair sum stays near round-off |

## Budgets

Every gate test above takes under one minute in release (P1: about 1.2 ms per SIMPLEC iteration of CH; P2: 17 s for the three S3
runs of one scheme), except R1 (one test per model, expected one to two minutes: 10 000 SIMPLEC iterations and 5000 steps of two
outer iterations).

## Not in scope (and why)

- SA-DES, SA-DDES and the Reynolds stress model on the outer iterations (ADR 0199 question 3: it would change existing transient
  results).
- Plugin models on `pimple`: a plugin can answer `updates_in_outer_iterations`, but the case tree does not accept one (question 4).
- Both models on `piso` (question 1), on moving meshes (F-277), with the energy equation, in checkpoints.
- The seam option `pair` for these models (F-296).
- A change of ADR 0199's registered S3 for SST (question 5).

## Open questions for the maintainer (each with a recommendation)

1. **k-epsilon and SA with `piso`.** Options: (a) refused at `turbulence.model`, as for SST (D4); (b) accepted with one sweep per
   step at the old level. Recommendation: (a), the rule of ADR 0199 question 1; `pimple` with `n_outer = 1` gives the lagged
   update to a user who wants it.
2. **One ADR for two models.** The roadmap asks for a registration per model. Options: (a) one ADR with two self-contained parts
   K and A, as written; (b) split into two ADRs. Recommendation: (a); the parts share only the case-tree harness.
3. **`rcfd_turb::supports_transient`.** Its doc says the RANS transport models are steady only, and it returns `false` for
   them; nothing in the workspace calls it (F-286 left it as it was). Options: (a) leave it; (b) make it return `true` for
   `k_epsilon`, `k_omega_sst` and `spalart_allmaras` with a doc that names `pimple`. Recommendation: (b) in a small follow-up,
   since it is public API and a change of its answer is not part of this item.
4. **Plugin models on `pimple`.** Options: (a) refused, as today; (b) accepted when the plugin answers
   `updates_in_outer_iterations() = true`. Recommendation: (a) until a plugin asks; there is no registered check for one.
5. **ADR 0199 S3 for SST** (its question 6). P2 shows that the SST BDF2 miss comes from the impulsive start and that the same
   runs from the parabola meet the band (2.03 to 2.05). Options: (a) leave the known miss with this diagnosis added to ADR
   0199; (b) an amendment of ADR 0199 that registers S3 from the parabola for SST as a new test (the old one stays an ignored
   record). Recommendation: (b), a small amendment by the lead; this ADR does not edit ADR 0199's checks.

## Addenda

Results of the checks follow in an addendum, in the order of the tables.

### Amendment 1 (2026-11-07): the outer wall of the S4 sector, written before any code and before any run of S1 and S4

Found while recording V0b, before any code of F-294: its case "steady SIMPLEC k-epsilon on the S4 sector with the frame" stops
at `Case::semantic_errors` with `boundaries.outer.velocity`: `moving_wall, shear, partial_slip and marangoni walls are not
supported by the turbulence wall treatments`. `turb::check` accepts a `moving_wall` for `k_omega_sst` only (ADR 0078 S2), so the
S4 text of ADR 0199, whose `outer` is a `moving_wall` counter-rotating at `-2` rad/s, cannot carry k-epsilon or SA. No run of the
V0b record had been kept (the record had not been written) and S1 and S4 had not run.

The S4 texts of record for k-epsilon and SA (and the V0b case) take `outer` as `velocity = "fixed"` with no value (a wall at rest
in the rotating frame) and `turbulence = "wall"`, in place of the moving wall; everything else is the text of the S4 row (the
frame, the rotational seams, the inlet expressions, the inlet turbulence values, the solver). S1 adds one located case: the
k-epsilon S4 text with the moving wall of ADR 0199 is located at `boundaries.outer.velocity` (the rule as it is). No limit
changes.

### Amendment 2 (2026-11-07): the solver controls of HK, written after the first run of part K and before the corrected run

The first run of the part K library checks (code of D6 in the working tree, release, each test once) found the HK set-up
defective: every HK test stopped at HK0 in step 1, sweep 1, with k uniform (spread 2.2e-16 to 4.7e-16) and epsilon not (spread
1.017e-6 in K1, K1b and K1N, 1.255e-6 in K1r, 1.355e-7 in K1v, 1.820e-6 in K2 and K2b, 3.107e-6 in K2v; limit 1e-13). KC1 ran
and met its limits (addendum). The cause is the linear solver, not the transient term: its stopping rule is
`residual <= max(tol_abs, tol_rel r0)` on the 2-norm of the whole system, and the epsilon of block B's wall cell is fixed by a
diagonal 1e8 times its own (point 3), so that row dominates `r0` by about 1e8 and the rows of A stop near
`1e-14 x 1e8 = 1e-6` of their own scale. The k matrix has no such row. A scratch probe (not committed) of three sweeps of the
first K1 step gives an epsilon spread of 1.0e-6, 1.8e-7, 2.3e-8 with the registered controls and 3.6e-16 to 4.9e-16 (k 3.5e-16
to 5.8e-16) with `tol_rel = 0`.

The records stay as ignored tests (`*_tol_record`), with their limits unchanged. The corrected HK takes the solver controls
`tol_rel = 0`, `tol_abs = 1e-300`, `max_iter = 1000`: the HK matrices are symmetric (`phi = 0`), so the solver is CG, which then
stops at an exactly zero residual or after 1000 iterations, past round-off in every row of the 17-cell system. Nothing else
changes (mesh, constants, steps, sweeps, limits, reference values). KC1 keeps its registered controls and its run of record.

### Amendment 3 (2026-11-07): block B of HK made thin, written after the run of amendment 2 and before the corrected run

The HK checks as corrected by amendment 2 ran once (release, each test once). HK0 held to round-off in K1 (standard and RNG,
both schemes), K1r, K1v, K1b, K1N, K2b, K2v and the standard half of K2, and these met their limits with the numbers of the
recursion, but K2 stopped at HK0 in its RNG Euler run with `N = 160`, step 16, sweep 2: epsilon spread 2.676e-8 (k 0). So the
corrected set-up is still defective, and by the classes every closed-form check of part K under it is a record. A scratch probe
(not committed) of the eight K2 runs (both variants, both schemes, `N = 160` and `320`) repeats the miss in that one run
(2.676e-8 at step 16, sweep 2; the seven others 3.4e-15 to 9.4e-15) and gives 3.5e-15 to 1.0e-14 in all eight with the
correction below. The cause is again B's wall row: with `tol_rel = 0` CG runs its 1000 iterations on a system whose B row is
about 1e9 times A's, and past round-off its iterate can drift in the rows of A.

The runs of amendment 2 stay as ignored tests (`*_amendment2_record`), with their limits unchanged. Corrected HK: block B is
scaled by `(1, 1, 1e-12)` (`Affine::scaling`) before its translation, so its one cell keeps its wall distance 0.5 in the plane
and its volume is 1e-12; its rows, the wall row included, are then about 1e-4 of A's instead of 1e8 times, and the solver
controls go back to those registered (`tol_rel = 1e-14`, `tol_abs = 1e-300`, `max_iter = 1000`). HK0's mesh part is unchanged
and also asserts that volume. Nothing else changes. KC1 keeps its run of record.

### Addendum: results (2026-11-07)

Code in affe302 (after the registration d739244, amendment 1 ad04582, the V0b record a7a4970 and amendments 2 ba6b89f and 3
cb212be): `crates/rcfd-turb/src/kepsilon.rs` (`KEpsilon::begin_step`, `is_transient`, the transient branch of `update`),
`crates/rcfd-turb/src/sa.rs` (`SpalartAllmaras::begin_time_step`, `is_transient`, the transient branch of `update`, and
`begin_step` clearing the new state), `crates/rcfd-turb/src/model.rs` (the trait methods of D3), `crates/rcfd-api/src/turb.rs`
(D4), the comments of the dispatch in `crates/rcfd-api/src/sim.rs`, the doc strings of `schema.rs`, the k-epsilon and SA
sections of `docs/case-schema.md`, and the two S1 cases of ADR 0199 in `f286_transient_sst.rs`, as D6 says. Tests:
`crates/rcfd-turb/tests/f294_transient_kepsilon.rs`, `crates/rcfd-turb/tests/f294_transient_sa.rs`,
`crates/rcfd-api/tests/f294_transient_keps_sa.rs`, `crates/rcfd-api/tests/f294_default_bits.rs` (a7a4970). Every check was run
once, in release; the exceptions are named where they occur (amendments 2 and 3, and the S3 diagnosis below).

Summary: both models run with `pimple` on a static mesh, with the rotating frame too, and the defaults are bit-identical. Three
set-ups were found defective and corrected by amendments 1 to 3 before their corrected runs (the S4 moving wall, and twice the
solver controls of HK, whose wall row in block B set the stopping rule). Every corrected check met its limit except S3 for
k-epsilon with BDF2, whose observed order on the channel start-up (0.80 to 1.53) is below its band; it is an ignored known miss,
with a diagnosis. S3 for SA with BDF2 met (1.93, 2.20), from the start that P2 chose. Every negative control fired and every
mutation made its check fail; M4 acts on the known-miss check, and a report-only run of the diagnosis under M4 separates it.

| Id | Measured | Limit | Result |
|---|---|---|---|
| V0a | `baseline_hashes.rs`, `case_hashes_w48u.rs`, `f286_default_bits.rs`, `f295_default_bits.rs` unedited; all pass on affe302 in release (and see the gate line below) | pass | met |
| V0b | 9 of 9 runs bit-identical to the record of 765ac64 (steady SIMPLEC CH-K 0xdb8fab77628384d0, realizable with the enhanced wall function 0x629f58307f83bf41, RNG scalable hysteresis 0x8d6a2b47a3568ecb, the k-epsilon sector with the frame 0xc9cd944de516406b, CH-A 0xfe68bf9b402352b6, CH-A `auto_sa_law` 0x0e47cb237b676340, SA-DES `piso` 0xe59a424a65564884, SA-DDES `pimple` without a source 0xf53cf8acd9b6ae13, transient SST CH on `pimple` 0xdada28263c5403a0) | bit-identical | met |
| V0c | the gate of affe302 (git archive, `scripts/gate_test.py`, 3 jobs): fmt 0, clippy 0, 3063 passed, 2 failed, 884 ignored (3065 processes, 591 binaries, wall 6730 s); the two failures are the Studio timing tests `preview_gpu_g1` g10 (known flake) and `preview_cost_g1` g8 (passes alone, 2.1 to 2.6 ms; in the crate this change does not touch), both under load because this gate ran without the lock, alongside the F-257 gate, with free memory down to 9 GB (lead, 2026-11-07) | pass, apart from the known flakes | met, with that deviation |
| HK0 | k and epsilon of A uniform to 1.0e-14 or better in every sweep of every corrected HK run (the registered set-up missed, 1.0e-6, amendment 2; the set-up of amendment 2 missed once, 2.7e-8, amendment 3) | as stated | met (corrected set-up) |
| K1 | standard Euler 1.244e-15, BDF2 1.546e-15; RNG Euler 5.162e-16, BDF2 1.054e-15 | `<= 1e-12` | met |
| K1r | Euler 1.988e-15, BDF2 6.080e-15 | `<= 1e-12` | met |
| K1v | 1.651e-15 | `<= 1e-12` | met |
| K1b | 1.434e-15 | `<= 1e-12` | met |
| K1N | 5.148e-2 | `>= 1e-3` | met (K1 can fail) |
| K2 | standard Euler k: `e(160)` 1.5069e-3, `e(320)` 7.5430e-4, `p` 0.9983; epsilon: 5.8787e-3, 2.9421e-3, 0.9987. Standard BDF2 k: 6.1050e-5, 1.5823e-5, 1.9480; epsilon: 1.7448e-4, 4.5079e-5, 1.9526. RNG Euler k: 2.4149e-3, 1.2085e-3, 0.9988; epsilon: 7.5375e-3, 3.7705e-3, 0.9993. RNG BDF2 k: 9.7507e-5, 2.5307e-5, 1.9460; epsilon: 2.3122e-4, 5.9866e-5, 1.9494 | Euler [0.9, 1.1], `<= 5e-3`; BDF2 [1.9, 2.1], `<= 1e-4` | met |
| K2v | k: 7.8347e-5, 2.0288e-5, 1.9493; epsilon: 2.2414e-4, 5.7828e-5, 1.9546 | [1.9, 2.1]; `<= 1e-4` | met |
| K2b | bounded BDF2 k: 6.1642e-5, 1.5932e-5, 1.9520; epsilon: 1.7717e-4, 4.5610e-5, 1.9577 | report only | reported |
| KC1 | `abs(B_K)` / dissipation: Euler upwind 4.795e-15, Euler `linear_upwind` 5.160e-15, BDF2 upwind 3.428e-15, BDF2 `linear_upwind` 3.582e-15; smallest per-step change 4.806e-2 to 5.252e-2; C1-0: largest cell divergence 1.117e-16 of the largest flux, boundary flux 0 (the first run, registered set-up, which KC1 keeps) | `<= 1e-10`; `>= 1e-3` | met |
| HA0 | met in every sweep of every HA run | as stated | met |
| A1 | Euler 6.387e-16, BDF2 3.254e-15 (`a` 25.97958794, `T` 0.2405734847 as computed) | `<= 1e-12` | met |
| A1v | 7.834e-16 | `<= 1e-12` | met |
| A1b | 5.898e-16 | `<= 1e-12` | met |
| A1N | 3.449e-2 | `>= 1e-3` | met (A1 can fail) |
| A2 | Euler `e(40)` 8.5548e-3, `e(80)` 4.3046e-3, `p` 0.9909; BDF2 1.5510e-4, 3.8869e-5, 1.9965 | Euler [0.9, 1.1], `<= 1e-2`; BDF2 [1.9, 2.1], `<= 1e-4` | met |
| A2v | `e(80)` 7.0734e-5, `e(160)` 1.7820e-5, `p` 1.9889 | [1.9, 2.1]; `<= 5e-5` | met |
| A2b | bounded BDF2 4.4305e-4, 1.1388e-4, 1.9600 | report only | reported |
| AC1 | `abs(B_A)` / sources: Euler upwind 7.472e-16, Euler `linear_upwind` 8.954e-16, BDF2 upwind 1.075e-15, BDF2 `linear_upwind` 1.093e-15; smallest per-step change 9.050e-2 to 9.526e-2 of `max nuT`; one cell negative at the last BDF2 upwind step, so the negative branch was in the budget; C1-0 as KC1 | `<= 1e-10`; `>= 1e-3` | met |
| S1 | every case as registered and as amendment 1 adds | as stated | met |
| R1 CH-K | settled: last-step change of `ux` 1.362e-15 `max abs(ux)` after 5000 steps (t = 1000); `ux` 1.452e-10, k 5.620e-11, epsilon 1.621e-9, nut 3.245e-10; `max abs(uy)` / `max ux` 7.524e-13 (pimple), 5.307e-13 (simplec); 12.0 s and 18.1 s | settled `<= 1e-11`; `<= 1e-8`; `<= 1e-8` | met |
| R1 CH-A | settled 1.695e-15; `ux` 8.452e-11, nuTilda 4.194e-10, nut 5.440e-10; `max abs(uy)` / `max ux` 1.156e-12, 7.692e-13; 10.0 s and 14.3 s | as CH-K | met |
| S3 CH-K `bdf2` | `ux`: `e(80,160)` 1.5225e-5, `e(160,320)` 8.7598e-6, `p` 0.7975; k: 3.0625e-4, 1.0895e-4, 1.4910; epsilon: 7.0086e-4, 2.4264e-4, 1.5303; largest `nut_change` of a step 1.8e-8 / 2.8e-12 / 1.3e-15 (T/80, T/160, T/320) | [1.7, 2.3] | **missed** (below the band): ignored `s3_kepsilon_bdf2_time_order_known_miss` |
| S3 CH-K `euler` | `ux`: 1.0200e-3, 5.1677e-4, 0.9809; k: 3.9828e-3, 1.9872e-3, 1.0030; epsilon: 2.9452e-3, 1.5088e-3, 0.9650 | [0.8, 1.2] | met |
| S3N CH-K | `p` 0.9877 (`ux`), 0.7762 (k), 0.5144 (epsilon); largest `nut_change` of a step 0.55 to 0.61 | smallest `<= 1.3` | met (fired) |
| S3 CH-A `bdf2` | `ux`: 1.6466e-5, 4.3211e-6, 1.9300; nuTilda: 9.4881e-5, 2.0677e-5, 2.1981; largest `nut_change` 4.1e-8 / 4.8e-10 / 5.5e-13 | [1.7, 2.3] | met |
| S3 CH-A `euler` | `ux`: 5.6058e-4, 2.7904e-4, 1.0064; nuTilda: 4.5722e-3, 2.3340e-3, 0.9700 | [0.8, 1.2] | met |
| S3N CH-A | `p` 0.6143 (`ux`), 0.7914 (nuTilda); largest `nut_change` 0.97 | smallest `<= 1.3` | met (fired) |
| S4 k-epsilon | 10 of 10 steps finite, no guard trip; worst seam pair sum 1.216e-15; smallest k 2.49e-3, smallest epsilon 2.02e-3; `max nut / nu` 0.9488 | `<= 1e-8` | met |
| S4 SA | 10 of 10 steps finite, no guard trip; worst seam pair sum 1.047e-15; smallest nuTilda 1.24e-3; `max nut / nu` 0.1353 | `<= 1e-8` | met |

**Records of the defective set-ups.** The registered HK (first run): every HK test stopped at HK0 in step 1, sweep 1 (epsilon
spread 1.0e-6 to 3.1e-6, K1v 1.4e-7), amendment 2; ignored `*_tol_record`. The HK of amendment 2 (`tol_rel = 0`): K1 (standard
Euler 8.645e-16, BDF2 2.465e-15, RNG 1.124e-15, 1.593e-15), K1r (6.686e-16, 5.409e-15), K1v 3.346e-15, K1b 8.250e-16, K1N
5.148e-2, K2v and K2b with the numbers of the table, and the standard half of K2 with them too, met; K2 stopped at HK0 in its RNG
Euler run (2.676e-8 at `N = 160`, step 16, sweep 2), amendment 3; ignored `*_amendment2_record`. The corrected runs of amendment 3
are those of the table: the recursion and closed-form numbers are the same to the printed digits in all three set-ups wherever a
run reached them.

**S3, k-epsilon with BDF2: the miss and its diagnosis.** The registered test asserted every field and stopped at `ux`; all
three orders were printed (table). It is now the ignored `s3_kepsilon_bdf2_time_order_known_miss`, with the limit intact.
Diagnosis, report only (`s3_kepsilon_bdf2_diagnosis` and `s3_kepsilon_bdf2_diagnosis_enhanced`, ignored, each run once):

- the outer iterations had converged in the run of record (largest `nut_change` of a step 1.3e-15 at T/320), and Euler is first
  order in all three fields (S3 Euler), as are both schemes in the library checks K1 and K2, which show the time derivative of
  the k and epsilon equations exactly;
- the same runs one and two levels finer give `p` 1.5598 / 1.7638 / 1.7816 (`ux` / k / epsilon) from T/160, T/320, T/640 and
  1.8119 / 1.8796 / 1.8874 from T/320, T/640, T/1280: the observed order rises towards 2, so the registered levels are not in
  the asymptotic range of this start-up for k-epsilon (they were for SA and, in P2, for SST);
- a likely contributor, not proven by these runs: the standard wall function's wall-face eddy viscosity
  (`WallConstants::nut_wall_from_k`) is 0 below `y*_lam` = 11.530107 and the log-law value above it, a kink in `y*`, and in the
  T/320 run the wall-cell `y*` (8.78 to 12.48) crosses `y*_lam` twice in the window (at t 0.481 and t 1.425). A coefficient
  with a kink in time gives the BDF2 error a part whose size depends on where the crossing falls inside a step. A start with
  k 2 and epsilon 14.4 (the same initial eddy viscosity) also crosses (3 steps with a crossing) and gives 3.5073 / 1.8581 /
  1.7352, so it does not isolate the cause;
- with the enhanced wall function (no switch at `y*_lam`) on this mesh (`y+` about 20, not the wall-resolved mesh that wall
  function is meant for) the 20 outer iterations do not converge at T/80 and T/160 (largest `nut_change` of a step 4.7e-2 and
  1.0e-4) and neither scheme shows an order (BDF2 -0.17 / -0.20 / 0.04, Euler 0.46 / 0.46 / 0.58), so that run does not
  separate the kink either. It is a finding of its own (question 6).

The limit is not changed.

**Mutations** (scratch, each applied alone to affe302 and reverted; the named test run once):

| Id | Test | Measured | Result |
|---|---|---|---|
| M1 | K1 | fails at its precondition HK0 in step 1, sweep 1: spread k 8.074e-2, epsilon 9.291e-3 (the relaxed k equation does not keep a uniform field uniform) | fails |
| M2 | K2 (`bdf2`) | standard BDF2 k: `e(160)` 1.5069e-3, `e(320)` 7.5430e-4, `p` 0.9983 (the Euler numbers) | fails |
| M3 | K1v | 7.382e-3 | fails |
| M4 | S3 CH-K (`bdf2`) | `p` 0.9502 / 0.7856 / 0.5844 (`ux` / k / epsilon) at errors `e(80,160)` 9.4e-3 / 3.3e-2 / 4.5e-2, 600 to 60 times those of the code; largest `nut_change` of a step 0.55 to 0.61 | fails (below the band, where the code of record also misses; see below) |
| M5 | A1 | fails at its precondition HA0 in step 1, sweep 2: spread of nuTilda 9.499e-2 | fails |
| M6 | A2 (`bdf2`) | `e(40)` 8.5548e-3, `e(80)` 4.3046e-3, `p` 0.9909 (the Euler numbers) | fails |
| M7 | A1v | 3.475e-3 | fails |
| M8 | S3 CH-A (`bdf2`) | `p` 0.5602 / 0.7359 (`ux` / nuTilda); largest `nut_change` of a step 0.97 | fails (the code of record meets) |
| M9 | S1 | CH-K on `piso` accepted with no diagnostic | fails |

M4 makes a check fail that the code of record also misses, so on its own it does not show that S3 separates the coupled step
from the lagged one for k-epsilon. The diagnosis of the miss was therefore run once more with M4 applied (report only): the
finer levels give `p` 1.0141 / 0.7784 / 0.6161 (T/160, T/320, T/640) and 1.0425 / 0.8345 / 0.7611 (T/320, T/640, T/1280),
where the code gives 1.56 to 1.78 and 1.81 to 1.89. The order that rises towards 2 comes from the sweep in the outer iterations.

**Predictions.** PR1 met (every recursion check at or below 6.1e-15; K2, K2v, A2, A2v reproduce the recursion to the printed
digits). PR2 met (1e-15). PR3 met (R1 at 5.6e-11 to 1.6e-9). PR4 met for SA and for k-epsilon Euler and S3N, falsified for
k-epsilon BDF2 at the registered levels (0.80 to 1.53). PR5 met (1.0e-15 and 1.2e-15).

Before the gate, on affe302 in release: the non-ignored tests of 18 related `rcfd-api` files (`bc_catalogue`, `des_tree`,
`dispersion_f255`, `exp_rot_pab`, `exp_rot_pc`, `f286_transient_sst` with D6, `f295_seam_diffusivity`, `init_methods`,
`physics_schema`, `rsm_tree`, `rules_w19c`, `runctl`, `sgs_tree`, `sst_schema`, `transient_sources`, `turb_models`,
`w48z_followups`, `wall_options`): 144 passed, 0 failed; the `rcfd-turb` unit tests and `channel_kepsilon`, `des`,
`f286_transient_sst`, `f294_transient_kepsilon`, `f294_transient_sa`, `f295_seam_diffusivity`, `flat_plate_sa`, `rsm`: 76
passed, 0 failed. `cargo clippy -p rcfd-turb -p rcfd-fv -p rcfd-api --all-targets -- -D warnings` and `cargo fmt --check` of
the three packages are clean.

**Questions added by the results** (each with a recommendation):

6. **The enhanced wall function on `pimple`.** On the CH mesh (`y+` about 20) its outer iterations do not converge in 20 at
   `dt` = 0.025 and 0.0125 (above). Options: (a) leave it accepted, documented as meant for wall-resolved meshes, as today for
   the steady algorithms; (b) a follow-up item that measures it on a wall-resolved channel on `pimple`. Recommendation: (a) now,
   (b) if a case needs it.
7. **The S3 k-epsilon BDF2 miss.** Options: (a) keep it as the known miss it is, with the diagnosis above (as done); (b) a
   registered follow-up that makes the standard wall-function eddy viscosity smooth at `y*_lam` (opt-in, defaults
   bit-identical) and repeats S3 with it, which would test the likely cause. Recommendation: (a) now; (b) only if transient
   k-epsilon with wall functions needs a case-tree order record, since K1 and K2 show the scheme exactly.

### Merge note (lead, 2026-11-07)

Merging into main (after F-291, ADR 0204) conflicted in the imports of `kepsilon.rs` and `sa.rs` only (F-291's
`laplacian_opt`/`GradientOptions` next to this change's `add_ddt`/`History`/`TimeScheme`); both are kept. After the
merge the F-294, F-291, F-286 and F-295 test files of rcfd-turb and rcfd-api pass in release, and clippy is clean.
