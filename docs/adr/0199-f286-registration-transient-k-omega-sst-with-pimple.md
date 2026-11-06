# ADR 0199: F-286 registration: transient k-omega SST with pimple on a static mesh

Status: **REGISTERED 2026-11-06**, written before any code of F-286. Not yet accepted by the maintainer. Before this text, one
probe of an existing capability was run on main 9edfbda (not committed): the steady SIMPLEC run of the channel of R1 below,
and the refusal of the same case with `pimple`. Its numbers are quoted where they set a run length. Results go in the addendum
at the end.

Summary of the results (addendum): k-omega SST runs with `pimple` on a static mesh, with the rotating frame too, and the
defaults are bit-identical. Three set-ups were found defective by their first run and corrected by amendments 2 and 3 before
the corrected runs (the periodic seam of H and C1, the fractional-step case of S1). Every corrected check met its limit except
S3 for BDF2, whose observed order on the channel start-up (2.4 to 2.7) is above its band; it is an ignored known miss, with a
diagnosis. Every mutation made its check fail. The case text of ADR 0183 check H builds, and its 200-step smoke on B2 is
finite. The amendment 2 record also names an existing property of the kernel: a periodic seam is not conservative for a
diffusivity that differs across it.

Parents: [ADR 0183](0183-exp-rot-registration.md) (EXP-ROT; D5 check H, amendment P-C and its question 8, the decision record
of 2026-11-06, D9), [ADR 0136](0136-fidelity-tiers-xhigh-to-xlow.md) (URANS listed as missing), ADR 0015 and ADR 0070 (the SST
model and its term audit), ADR 0032 (the once-per-step time derivative of SA-DES and of the Reynolds stress model), ADR 0045
(the transient source hook, called at the start of every outer iteration), ADR 0016 (the time schemes, bounded BDF2), ADR 0069
(transient accuracy). Roadmap item F-286 (depends on F-043).

Number: 0197 is the highest on main; the lead gave 0199 to F-286.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test (by an amendment written before the corrected run), and the old one stays as an
  ignored record;
- every new code path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the model equations are those already in `crates/rcfd-turb/src/sst.rs` (Menter 2003 in the NASA TMR form, audited
term by term in ADR 0070). The closed form of homogeneous decay below is derived here from those equations. The time schemes are
those of `crates/rcfd-fv/src/ddt.rs`. No vendor code, text or install was used.

## What F-286 asks

The roadmap entry: `turbulence.model = k_omega_sst` runs with the `pimple` algorithm on a static mesh, which the case tree
refuses today with a located error at `turbulence.model` (ADR 0183 PC-5). The k and omega equations get their time derivative
and are updated within the outer iterations (ADR 0183 amendment P-C, question 8 (a)), also with a `rotating_frame` source, so
that check H of ADR 0183 can run as registered. The transient terms are checked against closed forms named in the ADR, within
limits fixed here. Steady SST runs and every existing transient run are unchanged bit for bit.

## What the code read shows (main 9edfbda)

1. **The refusal.** `turb::check` (`crates/rcfd-api/src/turb.rs`) ends with a branch that refuses every transported RANS model
   (`k_epsilon`, `k_omega_sst`, `spalart_allmaras` without DES, `plugin`) with any algorithm other than `simple` / `simplec`,
   located at `turbulence.model`. The probe gives, for the R1 channel with `pimple`: `k-omega SST is available with the steady
   algorithms (simple, simplec) only, not with solver.algorithm = "pimple"`, at line 23 of the text, from `Case::parse_toml`.
2. **The transient step** (`Simulation::step`, `Engine::Transient`). Once per step, before the flow step, the turbulence
   model gets `begin_step(dt)` and one `update(u^n, phi^n)` on the old level, and `nu_face` is set from it. The momentum
   sources, lumped components and two-way particles go through `PimpleSolver::step_with` with a hook called at the start of
   every outer iteration with the current velocity and flux (ADR 0045). The hook returns an `OuterUpdate` (source force,
   sink, face jumps, patch pressures); it carries no face viscosity. `step_with` reads the face viscosity once per step
   (`gamma_nu` from `self.nu_face`) and uses it in the momentum predictor of every outer iteration.
3. **The SST sweep** (`KOmegaSst::update`). One sweep solves k, then omega, on the given velocity and flux, with all
   coefficients from the iterate: wall-cell law and omega fix, F1, F2, the eddy viscosity `nu_t* = a1 k / max(a1 omega, S F2)`,
   the limited production, the cross-diffusion split, the floors `k >= 1e-12`, `omega >= 1e-10`, the cap `nu_t <= 1e5 nu`.
   Both matrices get the implicit under-relaxation `relax_turb`; the eddy viscosity and the wall eddy viscosity are relaxed by
   `relax_nut` after the first sweep. There is no time derivative, and `begin_step` is the trait's no-op.
4. **The other transient closures.** Spalart-Allmaras (DES and DDES only) and the Reynolds stress model store the old level in
   `begin_step` and add an implicit-Euler `V / dt` term to the next update, without relaxation, then clear the step
   (`step_dt = None`): one update per step, Euler whatever `solver.time_scheme` says. k-epsilon has no time derivative.
5. **The time derivative of the scalar equations.** `rcfd_fv::ddt::add_ddt` with a `History` (old, older, oldest levels and the
   step sizes) gives implicit Euler, variable-step BDF2 (Euler on the first step) and bounded BDF2; the energy equation and the
   transported scalars use it with the momentum's time scheme. The case tree offers `euler`, `bdf2` and `bdf2_bounded`.
6. **Refusals that stay.** A dynamic mesh refuses any turbulence model (`dynamic_mesh.enabled`, `dynmesh.rs`); step-doubling
   error control refuses it (`solver.error_tolerance`, `timectl.rs`); the fractional step refuses it and needs `piso`; VOF,
   the energy equation with RANS, checkpoints, warm starts and interpolation refuse it.
7. **A registered test asserts the refusal.** The last case of ADR 0183 PC-5
   (`exp_rot_pc.rs`, `pc5_scope_of_the_rotating_frame_on_the_transient_path`) asserts that `k_omega_sst` with `pimple` and a
   `rotating_frame` source is located at `turbulence.model`.
8. **The probe** (steady SIMPLEC, the R1 channel, `ny = 20`, existing code). The largest change of a cell velocity per
   iteration is 2.7e-6 at iteration 2000, 1.3e-12 at 6000 and 2.7e-13 at 8000, then stays between 1.5e-13 and 2.6e-13
   (round-off of `u` about 18) up to 40 000. The normalised residuals stay near 1e-3 in this fully developed case, so R1 reads
   the change, not `tolerance_u`. 40 000 iterations took 37 s in release. Maxima at iteration 40 000: `ux` 18.6089, k 3.6991,
   omega 181.67, nut 0.13447.

## Decisions (fixed now)

**D1. Formulation.** Step `n -> n+1` of size `dt`, outer iterations `m = 0 .. N-1` of `pimple` (`N = solver.n_outer`). At the
start of outer iteration `m` the model makes one sweep on the current iterate `(u^(m), phi^(m))`. The sweep is the steady
sweep of `KOmegaSst::update` (point 3 above: the same constants, blending functions, limiters, cross-diffusion split, floors,
cap, wall treatment of either `wall_omega` mode, roughness and `sst_variant`), with two changes:

- the implicit under-relaxation of the k and omega matrices is replaced by the time derivative of `add_ddt` with the
  momentum's time scheme:
  `(V / dt)(c0 q - c1 q^n + c2 q^(n-1))` for `q = k, omega`, with `(c0, c1, c2) = (1, 1, 0)` for Euler (and the first step of
  either BDF2), `((1 + 2 r)/(1 + r), 1 + r, r^2/(1 + r))` with `r = dt / dt_old` for BDF2, and the per-cell blend of ADR 0016
  for `bdf2_bounded`. The levels `q^n`, `q^(n-1)` and `dt_old` are fixed for the whole step;
- the eddy viscosity and the wall eddy viscosity take the new value of the sweep (no `relax_nut`).

So, with `P~`, `F1`, `nu_t*`, `S`, `CD`, `omega*` evaluated from the iterate exactly as in the steady sweep:

```text
(V/dt)(c0 k - c1 k^n + c2 k^(n-1))         + conv(phi^(m), k) - diff(nu + sigma_k nu_t*, k) = V (P~ - beta* omega* k)
(V/dt)(c0 w - c1 w^n + c2 w^(n-1))         + conv(phi^(m), w) - diff(nu + sigma_w nu_t*, w) = V (gamma P~/nu_t* - beta omega* w + (1 - F1) CD)
```

The eddy viscosity of the sweep sets the face viscosity of the momentum predictor of that outer iteration. If the outer
iterations converge, their fixed point is the fully implicit coupled step (velocity, pressure, k and omega at `n+1` together).
With `n_outer = 1` the sweep sees only the old level, which is a once-per-step update lagged by one step. The old levels are
stored when the step starts: the first step starts the history from the present fields, every later step pushes the fields at
the end of the previous step with that step's size. The steady path never starts a step, so it is unchanged.

**D2. Where the sweep runs.** Before the flow step, the SST model only starts the step (D1) and hands its present face
viscosity to the solver (what the lag checks see before the first outer iteration). The flow step goes through `step_with`
with the hook of ADR 0045. At the start of every outer iteration the hook evaluates the sources (unchanged), makes the SST
sweep on `(u^(m), phi^(m))`, and returns the face viscosity (laminar regions masked, as today). `OuterUpdate` gets the field
`nu_face: Option<Vec<f64>>`: `Some` replaces the face viscosity of the momentum predictor of that outer iteration and of the
solver from then on, `None` (the default, every existing hook) changes nothing. The `res_nut` monitor is the change of the last
sweep of the step.

**D3. Scope.**

- Accepted: `turbulence.model = "k_omega_sst"` with `solver.algorithm = "pimple"` (constant density, the iterative time
  advance, a static mesh), every `time_scheme` of the case tree, every `n_outer`, both `sst_variant`s, both `wall_omega` modes,
  roughness, laminar regions, with or without sources (`rotating_frame` included), lumped components and two-way particles.
- Located at `turbulence.model`: `k_omega_sst` with `piso`. P-C refused `rotating_frame` with `piso` for the same reason:
  one evaluation per step is a lagged term. `pimple` with `n_outer = 1` is that same lagged update and stays allowed, as P-C
  allows it for the frame (question 1).
- Unchanged refusals: `k_epsilon`, `spalart_allmaras` without DES and `plugin` with `piso` / `pimple` (question 2); every
  refusal of point 6.

**D4. No under-relaxation on the transient path.** `turbulence.relax` and `turbulence.relax_nut` are not used with `pimple`, as
for SA-DES today. The time derivative takes the place of the relaxation. Under-relaxation would not change the fixed point of
the outer iterations, but it would change the iterate at `n_outer = 1`, and D1 is the simpler record.

**D5. Code (the plan, with the files).**

- `crates/rcfd-turb/src/sst.rs`: `KOmegaSst::begin_step(dt, scheme)` and a private transient state (`None` on the steady path);
  in `update`, the branch of D1 where the steady code calls `relax` and chooses the relaxation factor of the eddy viscosity.
- `crates/rcfd-turb/src/model.rs`: `TurbulenceModel::begin_time_step(dt, scheme)` (default: `begin_step(dt)`, so SA, the
  Reynolds stress model and the algebraic models do exactly what they do now) and
  `TurbulenceModel::updates_in_outer_iterations()` (default `false`; `true` for the SST model).
- `crates/rcfd-fv/src/pimple.rs`: `OuterUpdate::nu_face` (D2).
- `crates/rcfd-api/src/turb.rs`: the scope branch of D3, the message of the `piso` refusal for SST.
- `crates/rcfd-api/src/sim.rs`: the start of the step and the hook of D2, only when the model updates in the outer iterations.
- `crates/rcfd-api/src/schema.rs` doc strings and `docs/case-schema.md` lines of the SST section; no case key is added or
  changed.
- `crates/rcfd-api/tests/exp_rot_pc.rs`: the last case of PC-5 is superseded by D3. It becomes two cases: `k_omega_sst` with
  `pimple` and the frame is accepted, `k_omega_sst` with `piso` is located at `turbulence.model`. The other lines stay.

**D6. Why the defaults stay bit-identical.** No case key is added, so every settings value, content hash and written text is
unchanged and `case_hashes_w48u.rs` needs no edit. Every case that was valid before is either steady, which never starts a
transient SST step, so its sweep runs the same operations in the same order, or a transient run of a model other than SST,
whose start of the step, hook and dispatch are unchanged: the new branch is taken only when
`updates_in_outer_iterations()` is true. Every existing hook returns `nu_face = None`. V0 checks this.

## Checks and limits (fixed now)

Every check is run once, in release. All but V3H are gate tests.

### Library checks (`crates/rcfd-turb/tests/f286_transient_sst.rs`)

**Homogeneous set-up H.** `box_2d(4, 4, 1.0, 1.0)`, periodic in x; `ymin` and `ymax` are given to the model as its walls, and
the test sets k and omega to zero gradient on them (replacing the Menter wall value of omega). `u = 0`, `phi = 0`, `nu = 0.1`,
`k0 = 1`, `omega0 = 10`, default variant and wall mode, linear solver `tol_rel = 1e-14`, `tol_abs = 1e-300`,
`max_iter = 1000`. Then `500 nu/(d^2 omega) >= 500 * 0.1/(0.375^2 * 10) = 35.6` in every cell, so `F1 = tanh(arg1^4) = 1` in
floating point; `S = 0`, so `P~ = 0`; `(1 - F1) CD = 0`; the wall law is inactive (wall speed 0). Every cell then obeys
`dk/dt = -beta* omega k`, `d omega/dt = -beta1 omega^2` (`beta* = 0.09`, `beta1 = 0.075`), with the closed form

```text
omega(t) = omega0 / (1 + beta1 omega0 t),   k(t) = k0 (1 + beta1 omega0 t)^(-beta*/beta1)
```

`M` sweeps per step (calls of `update` with `u`, `phi` fixed) stand for the outer iterations. The fully implicit recursion of
a step is `omega' = (-c0 + sqrt(c0^2 + 4 a R_w)) / (2 a)` with `a = dt beta1`, `R_w = c1 omega^n - c2 omega^(n-1)`, and
`k' = R_k / (c0 + dt beta* omega')` with `R_k = c1 k^n - c2 k^(n-1)`. The lagged recursion of one sweep (Euler) is
`omega' = omega^n / (1 + dt beta1 omega^n)`, `k' = k^n / (1 + dt beta* omega^n)`.

| Id | Set-up | Limit |
|---|---|---|
| H0 | precondition of every H run, every sweep | `F1 = 1` exactly in every cell; k and omega uniform to `1e-13` relative (a miss is a defective set-up) |
| D1 | H, `euler` and `bdf2`, `dt = 0.1 / (beta1 omega0) = 0.13333`, 40 steps, `M = 30`, against the fully implicit recursion | every step, k and omega: relative error `<= 1e-12` |
| D1v | H, `bdf2`, steps alternating `1.2 h` and `0.8 h` with `h = T / 40` (`T` below), 40 steps, `M = 30`, against the variable-step recursion | every step: `<= 1e-12` |
| D1b | H, `euler`, `M = 1`, the D1 steps, against the lagged recursion | every step: `<= 1e-12` |
| D1N | negative control: the D1b run against the fully implicit recursion | largest relative difference `>= 1e-3` (the recursions differ by 4.07e-2) |
| D2 | H, `M = 30`, `T = 1 / (beta1 omega0) = 4/3` s (omega halves), `N = 40` and `80` equal steps; `e` = relative error at `T` against the closed form, k and omega; `p = log2(e(40) / e(80))` | `euler`: `p` in [0.9, 1.1] and `e(80) <= 1e-2`; `bdf2`: `p` in [1.9, 2.1] and `e(80) <= 1e-4` |
| D2v | H, `bdf2`, the D1v alternation with `N = 80` and `160` | `p` in [1.9, 2.1]; `e(160) <= 5e-5` |
| D2b | H, `bdf2_bounded`, `N = 40` and `80` | report only |
| C1 | conservation: `box_2d(16, 4, 1.0, 1.0)` periodic in x, `ymin` / `ymax` the walls with k, omega and velocity zero gradient, `u = (1, 0, 0)` uniform (so `S = 0`), `phi` its face flux; `k = 1 + 0.5 sin(2 pi x)`, `omega = 10`, `nu = 0.1`; `euler` and `bdf2`, `dt = 0.05`, 10 steps, `M = 30`, k and omega convection `upwind` and `linear_upwind`, the H solver controls. `B = sum_c V_c ((c0 k' - c1 k^n + c2 k^(n-1)) / dt + beta* omega*_c k'_c)` with `omega*` the omega at the start of the last sweep | every step `abs(B) <= 1e-10 sum_c V_c beta* omega*_c k'_c`; H0's `F1 = 1` in every cell; non-vacuity: `max_c abs(k' - k^n) >= 1e-3` every step |

The values the recursion gives (computed with the formulas above, outside the code): Euler `e(40)` 1.174e-2 / 8.55e-3 (k /
omega), `e(80)` 5.91e-3 / 4.30e-3, `p` 0.990 / 0.991; BDF2 `e(40)` 2.00e-4 / 1.55e-4, `e(80)` 5.02e-5 / 3.89e-5, `p` 1.993 /
1.997; alternating BDF2 `e(80)` 9.59e-5 / 7.07e-5, `e(160)` 2.42e-5 / 1.78e-5, `p` 1.986 / 1.989.

### Case-tree checks (`crates/rcfd-api/tests/f286_transient_sst.rs`)

**Channel CH** (the probe's text): `box_2d` 4 x 20, `lx = 1`, `ly = 2`, periodic in x, `nu = 1/395`, `ymin` and `ymax`
`velocity = "fixed"` (walls at rest), `sources.drive` an acceleration `(1, 0, 0)` on all cells (so `u_tau = 1`,
`Re_tau = 395`), `k_omega_sst` with `k_initial = 1`, `omega_initial = 40`, initial velocity `(15, 0, 0)`, momentum and
turbulence convection `upwind`. The first cell centre is at `y = 0.05`, `y+` about 20, on the omega-fix branch of the wall
treatment.

| Id | Set-up | Limit |
|---|---|---|
| S1 | scope, through `Case::parse_toml`, `semantic_errors` and `Simulation::new` | accepted with no diagnostic: CH on `pimple` with `euler`, `bdf2`, `bdf2_bounded`; with `sst_variant = "bsl"`; with `wall_omega = "low_re"`; the S4 text. Located at `turbulence.model`: CH on `piso` (the message names `pimple`); CH with `k_epsilon` and with `spalart_allmaras` (DES off) on `pimple`; CH on `pimple` with `energy.enabled`. Located at `dynamic_mesh.enabled`: CH on `pimple` with a dynamic mesh. Located at `solver.error_tolerance`: CH on `pimple` with `error_tolerance = 1e-3`. Located at `solver.time_advance`: CH on `piso` with `fractional_step` |
| R1 | reduction to the steady solution: CH on `simplec` (the probe's settings), 10 000 iterations; CH on `pimple`, `euler`, `dt = 0.2`, `n_outer = 2`, 5000 steps (t = 1000, about 100 times the slowest relaxation time `U_b h / (2 u_tau^2)` of about 9) | settled: the last step's largest cell change of `ux` `<= 1e-11 max abs(ux)` (else R1 is NOT SETTLED, a known miss, no re-run without an amendment); then `max_c abs(q_P - q_S) <= 1e-8 max_c abs(q_S)` for `q = ux, k, omega, nut`, and `max abs(uy) <= 1e-8 max abs(ux)` in both runs |
| S3 | time order through the case tree: CH on `pimple`, `n_outer = 20`, `n_correctors = 2`, `momentum_rel_tol = pressure_rel_tol = 1e-12`, `pressure_max_iter = 1000`, to `T = 2`, `dt = T/80, T/160, T/320`; `e_ij = max_c abs(q_i - q_j) / max_c abs(q_finest)` at `T` for `q = ux, k, omega`; `p = log2(e(80,160) / e(160,320))` | `bdf2`: `p` in [1.7, 2.3] for each `q`; `euler`: `p` in [0.8, 1.2] for each `q`. Reported: the largest `res_nut` of a step (the change of the last sweep) |
| S3N | negative control: S3 `bdf2` with `n_outer = 1` (one sweep per step at the old level) | the smallest `p` of the three fields `<= 1.3` |
| S4 | rotating frame: the ADR 0183 PC-3 sector text (`annulus_3d_sector` 8 x 6 x 8, `rotating_frame` 2 rad/s, `pimple`, `bdf2`, `n_outer = 3`, Barth-Jespersen `linear_upwind`, `dt = 0.05`) with `k_omega_sst`, `inner` a wall, `outer` the `moving_wall` with `turbulence = "wall"`, the inlet `turbulence_spec = "values"`, `k_value = 0.01`, `omega_value = 10`; 10 steps | every step finite, no guard trip, k and omega positive; seam pair sum `max abs(phi_f + phi_g) / max abs(phi_f) <= 1e-8` (the PC-3 limit). Reported: `max nut / nu` |
| S6 | check H builds: the ADR 0183 D5 text of H(dt) (the ADR 0086 bladed B2 case of `fan01_gci_w37c.rs` with `algorithm = "pimple"`, `n_outer = 5`, `time_scheme = "bdf2"`, `dt = 4.605e-5`) | `Case::parse_toml`, `semantic_errors` and `Simulation::new` succeed with no diagnostic |
| V3H | report only, ignored test, run once after the code lands (the V3 smoke of ADR 0183 D3 for H(dt), allowed here only because it is cheap): S6's text, 200 steps | none; finite or not, guard trip or not, wall seconds per step |

S3's expectation: with converged outer iterations the step is fully implicit, so BDF2 is second order; a sweep lagged by one
step adds an `O(dt)` error to k, which S3N shows.

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs`, unedited | pass |
| V0b | `crates/rcfd-api/tests/f286_default_bits.rs`: FNV-1a (as `baseline_hashes.rs`) of the bits of every cell field the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut` and of the face flux, recorded on 9edfbda (this text plus the test, no code of F-286) and committed before the code: steady SIMPLEC CH (300 iterations); steady SIMPLEC SST on the S4 sector with the frame (100 iterations); SA-DDES (`des_tree.rs` text) on `piso` (20 steps) and on `pimple` with `n_outer = 2` and a drive acceleration (20 steps, the hook path with a turbulence model); the Reynolds stress decay box of `rsm_tree.rs` (20 steps); Smagorinsky on the Taylor-Green box of `sgs_tree.rs` (20 steps); laminar `pimple` with the frame (the PC-3 text, 5 steps); laminar `pimple` with an acceleration (the PC-2b text, 50 steps) | bit-identical after the code |
| V0c | the full gate (fmt, clippy `-D warnings`, every test in release) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each mutation is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the
addendum.

| Id | Mutation | Must fail |
|---|---|---|
| M1 | the k equation keeps the steady relaxation on the transient path (no time derivative in k) | D1 |
| M2 | the SST sweep uses Euler whatever the scheme | D2 (`bdf2`) |
| M3 | the hook makes the SST sweep at outer iteration 0 only | S3 (`bdf2`) |
| M4 | the history push takes the new step size instead of the previous one | D1v |
| M5 | the hook makes the sweep but returns `nu_face = None` | S3 (`bdf2`) |

### Classes

V0 is a precondition of landing: if V0a or V0b misses, the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control
(D1N, S3N) or a mutation that does not make its check fail means that the check is not shown to be able to fail: it is
reported as a defective set-up, corrected by an amendment before any re-run, and never counted as a pass. If H0 misses, the
closed-form checks D1 to D2b are a defective set-up of the same kind.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | D1, D1v and D1b meet at round-off (below 1e-14), and D2 / D2v reproduce the values of the recursion above |
| PR2 | R1 meets at round-off, near 1e-12: the steady discrete equations of the two algorithms are the same in a fully developed channel (no pressure gradient, no Rhie-Chow defect, the fixed points of the relaxations equal) |
| PR3 | S3 gives `p` near 2 for `bdf2` and near 1 for `euler`; S3N gives `p` near 1 for k |
| PR4 | S4 meets; the seam pair sum stays near round-off as in PC-3 |
| PR5 | V3H on B2 is finite with no guard trip, at about 1 to 3 s per step |

## Budgets

Every gate test above takes under one minute in release (the probe: about 1 ms per SIMPLEC iteration of CH; S3 is about
1100 steps of 20 outer iterations on 80 cells). V3H has a cap of 30 minutes of wall time; a run that hits it stops and is
reported as such.

## Not in scope (and why)

- k-epsilon, Spalart-Allmaras without DES and plugin models on `pimple` (question 2). The k-epsilon kernel has no time
  derivative. The SA kernel has one, but per step and Euler only, and moving SA-DES onto the outer iterations would change
  existing transient results.
- SST on `piso` (question 1), on moving meshes (F-277), with the energy equation, in checkpoints.
- A steady-state comparison on a developing (non-periodic) flow, where the Rhie-Chow interpolation of the two algorithms
  differs by a term that depends on `dt`: R1 would then measure that term, not the turbulence equations.
- Check H itself (ADR 0183 D5): its outer-iteration rule needs the velocity change of each outer iteration, which the case
  tree does not record today (question 4).

## Open questions for the maintainer (each with a recommendation)

1. **SST with `piso`.** Options: (a) refused at `turbulence.model`, as built (D3; the same rule as P-C for the frame);
   (b) accepted, with the sweep once per step at the old level (the result of `pimple` with `n_outer = 1`). Recommendation:
   (a). A user who wants the lagged update can ask for it with `pimple` and `n_outer = 1`.
2. **The other transported models.** Options: (a) SST only, as built; (b) a follow-up item that puts k-epsilon and SA without
   DES on the same outer-iteration path, each with its own registration and closed-form checks (the k-epsilon decay
   `k ~ (1 + t / t0)^(-1/(C2 - 1))` is the analogue of D2), with SA-DES and the Reynolds stress model kept on their path;
   (c) accept SA without DES on its existing once-per-step Euler path now (no kernel change, a first-order lagged URANS).
   Recommendation: (b).
3. **SA-DES and the Reynolds stress model.** They stay on the once-per-step path (D6). Moving them to the outer iterations
   would change existing transient results, so it needs the maintainer to lift the bit-identity rule for them.
   Recommendation: leave them unless a registered comparison shows a gain.
4. **Check H's outer-iteration rule.** ADR 0183 D5 asks for the velocity change of the 5th outer iteration against the 1st
   in every step of the window, which `StepInfo` does not carry. Recommendation: the H job adds a read-only per-outer record
   to the transient solver (opt-in, defaults bit-identical) under its own amendment; F-286 does not.

## Addenda

Results of the checks follow in an addendum, in the order of the tables.

### Amendment 1 (2026-11-06): the fractional-step case of S1, written before S1 ran

Found while writing the S1 test, before any check was run. The S1 row names "CH on `piso` with `fractional_step`, located at
`solver.time_advance`". That case cannot show the row's refusal: `piso` with SST is a static error at `turbulence.model`
(D3), and the static errors stop the case before `Simulation::new`, where the fractional-step rule (`fractional_step needs
solver.algorithm = "piso"`, and on `piso` `is not available with a turbulence model`) is checked. The case of record becomes
CH on `pimple` with `time_advance = "fractional_step"`, located at `solver.time_advance` (the rule that the fractional step
needs `piso`, reached because `pimple` with SST is no longer a static error). CH on `piso` with the fractional step stays in
S1 as a second case, located at `turbulence.model`. No limit changes.

### Amendment 2 (2026-11-06): H and C1 on a closed box, written after their first run and before the corrected run

The first run of the library checks (code of D5 in the working tree, release, each test once) found both set-ups defective,
for a reason of the existing kernel, not of the transient code:

- every H run stopped at H0 in step 1, sweep 1: the spread of k was 1.889e-2 and of omega 1.458e-2 (limit 1e-13). F1 was 1 in
  every cell, as derived. The cells next to the periodic seam differ from the others because a periodic face takes the partner
  cell's value of the previous iterate (`rcfd_fv::bc::Periodic`, lagged), so within one sweep the seam acts like a wall at the
  old value. D1 to D2b therefore did not run past H0;
- C1 (Euler, upwind, the first case) gave `abs(B) / sum V beta* omega* k'` = 2.876e-2 (limit 1e-10). A scratch probe (not
  committed) shows the sweeps converging (change 7e-10 at sweep 30) and the budget converging to -1.494e-3 against a
  destruction of 5.196e-2. The cause is the periodic seam again: `rcfd_turb::eddy::effective_viscosity` gives a boundary face,
  periodic faces included, the diffusivity `nu + nu_t` of its owner cell only, so the two faces of a seam pair carry different
  diffusivities when `nu_t` differs across the seam, and the diffusive fluxes of the pair do not cancel. C1 then measures this
  property of the steady kernel (it is the same in a steady SST sweep and in the momentum face viscosity of every RANS run with
  periodic seams), not the transient term.

The records stay as ignored tests (`d1_..._periodic_record` and the others, `c1_periodic_record`), with their limits
unchanged. The corrected set-ups remove the seam; nothing else changes (constants, step sizes, sweeps, limits, reference
values):

- **H (corrected).** `box_2d(4, 4, 1.0, 1.0)` with no periodic direction; all four sides (`xmin`, `xmax`, `ymin`, `ymax`) are
  the model's walls, with k and omega at zero gradient on them. Every cell centre is still at most 0.375 from its nearest side,
  so the argument for `F1 = 1` holds, and zero-gradient sides keep a uniform field uniform in every sweep.
- **C1 (corrected).** `box_2d(16, 16, 1.0, 1.0)` with no periodic direction, all four sides the walls with k and omega at zero
  gradient. The model gets `u = 0` (so `S = 0` and the wall law is inactive) and the flux of the stream function
  `psi = (1/pi) sin(pi x) sin(pi y)` (largest speed 1, Courant number 0.8 at `dt = 0.05`): on each side face from point A to
  point B of the xy plane, `phi_f = +-(psi(B) - psi(A)) depth`, the sign that of the area vector against the right-hand normal
  of `B - A`. This flux is divergence-free in every cell by construction and zero on every side, since `psi = 0` there.
  Precondition C1-0 (a miss is a defective set-up): `max_c abs(sum_f phi_f) <= 1e-14 max_f abs(phi_f)` and `phi_f = 0` on
  every boundary face. `k = 1 + 0.5 sin(2 pi x)`, `omega = 10`, the schemes, step, steps, sweeps, convection schemes, budget
  and limits are those of the C1 row.

Found, not changed (out of scope, for the maintainer): on a periodic seam the scalar transport of k and omega (and the
momentum face viscosity of RANS runs) is not conservative where `nu_t` differs across the seam (above). Check S4 and check H of
ADR 0183 have rotational seams.

### Amendment 3 (2026-11-06): the fractional-step case of S1 again, written after its run and before the corrected run

S1 as amended by amendment 1 ran once (release, code of D5 in the working tree) and missed at the fractional-step case: CH on
`pimple` with `time_advance = "fractional_step"` is located at `sources.drive.type` only, because `sources::check` refuses
momentum sources with the fractional step, a static rule that again stops the case before `Simulation::new`. Every S1 case
before it in the test met (the accepted texts, `piso`, k-epsilon and SA on `pimple`, the energy equation, the dynamic mesh,
`error_tolerance`); the `piso` case with the fractional step after it was not reached. The test was run a second time only to
print the panic message, with the same result. The S1 test as run stays as an ignored record (`s1_amendment1_record`).

The case of record becomes CH without `[sources.drive]` on `pimple` with the fractional step, located at
`solver.time_advance`. Without the source and with `pimple` the case has no static error, so the rule of `Simulation::new`
(`fractional_step needs solver.algorithm = "piso"`) is what refuses it. Every other S1 case is unchanged. No limit changes.

### Addendum: results (2026-11-06)

Code in 3505192 (after the registration 6cc77f7, the V0b record b204c7a and amendments 1 to 3): `crates/rcfd-turb/src/sst.rs`
(`KOmegaSst::begin_step`, the transient branch of `update`), `crates/rcfd-turb/src/model.rs` (`begin_time_step`,
`updates_in_outer_iterations`), `crates/rcfd-fv/src/pimple.rs` (`OuterUpdate::nu_face`), `crates/rcfd-api/src/turb.rs` (D3),
`crates/rcfd-api/src/sim.rs` (D2), the doc strings of `schema.rs`, the SST section of `docs/case-schema.md`, and PC-5 of
`exp_rot_pc.rs` as D5 says. Tests: `crates/rcfd-turb/tests/f286_transient_sst.rs`, `crates/rcfd-api/tests/f286_transient_sst.rs`,
`crates/rcfd-api/tests/f286_default_bits.rs`. e5f974e gives the V0b record type a name for clippy, with the hashes unchanged. Every check was run once, in release; the exceptions are named
where they occur (amendments 2 and 3, and the S3 diagnosis below).

| Id | Measured | Limit | Result |
|---|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited; both pass in the gate of e5f974e | pass | met |
| V0b | 8 of 8 runs bit-identical to the record of 9edfbda (steady SIMPLEC CH 0xe0c06d93f89a6026, steady SIMPLEC SST sector 0xc2f1699892d63041, SA-DDES `piso` 0x5815ab1e8181474b and `pimple` with a drive 0xb22cd2c49440a812, RSM 0xc39e0dbb60c181bc, Smagorinsky 0x50bfd09892f59843, laminar `pimple` sector 0x01737054b51921c3, laminar `pimple` channel 0x83467df6f8e66875) | bit-identical | met |
| V0c | gate of e5f974e (git archive, `scripts/gate_test.py`, 3 jobs): fmt per package 0, clippy `--workspace --all-targets -D warnings` 0; 2941 passed, 3 failed, 834 ignored, 2944 test processes, wall 4627 s. The 3 failures are on the list of known flakes: `runner_w47b` s1 and s9 failed again alone (three times each), and so did the same two tests of the pre-F-286 build of the ADR 0196 gate (`target-dec1`) run alone at the same time (its sampler saw no `rcfd` process: "max by OS 0"), so they do not come from this change; `journal_f273` c3s passed alone | pass, apart from the known flakes | met |
| H0 | `F1 = 1` and uniform k, omega in every sweep of every corrected H run (the first, periodic run missed, amendment 2) | as stated | met (corrected set-up) |
| D1 | Euler 4.466e-15, BDF2 1.754e-14 | `<= 1e-12` | met |
| D1v | 1.813e-14 | `<= 1e-12` | met |
| D1b | 2.276e-15 | `<= 1e-12` | met |
| D1N | 4.067e-2 | `>= 1e-3` | met (D1 can fail) |
| D2 | Euler k: `e(40)` 1.1738e-2, `e(80)` 5.9084e-3, `p` 0.9903; omega: 8.5548e-3, 4.3046e-3, 0.9909. BDF2 k: 1.9984e-4, 5.0212e-5, 1.9927; omega: 1.5510e-4, 3.8869e-5, 1.9965 | Euler [0.9, 1.1], `<= 1e-2`; BDF2 [1.9, 2.1], `<= 1e-4` | met |
| D2v | k: `e(80)` 9.5928e-5, `e(160)` 2.4215e-5, `p` 1.9860; omega: 7.0734e-5, 1.7820e-5, 1.9889 | [1.9, 2.1]; `<= 5e-5` | met |
| D2b | bounded BDF2 k: `e(40)` 6.5047e-4, `e(80)` 1.6806e-4, `p` 1.9525; omega: 4.4305e-4, 1.1388e-4, 1.9600 | report only | reported |
| C1 | `abs(B)` / destruction: Euler upwind 2.098e-14, Euler `linear_upwind` 2.214e-14, BDF2 upwind 2.614e-14, BDF2 `linear_upwind` 2.559e-14; smallest per-step change 4.027e-2 to 4.191e-2; C1-0: largest cell divergence 1.117e-16 of the largest flux, boundary flux 0 | `<= 1e-10`; `>= 1e-3` | met (corrected set-up) |
| S1 | every case as registered (the case of amendment 3 at `solver.time_advance`) | as stated | met (the run of amendment 1 missed, amendment 3) |
| R1 | settled: last-step change of `ux` 7.637e-16 `max abs(ux)` after 5000 steps (t = 1000); `ux` 2.657e-10, k 1.251e-9, omega 1.681e-9, nut 3.668e-10; `max abs(uy)` / `max ux` 3.264e-13 (pimple), 2.311e-13 (simplec) | settled `<= 1e-11`; `<= 1e-8`; `<= 1e-8` | met |
| S3 `bdf2` | `ux`: `e(80,160)` 6.0811e-5, `e(160,320)` 9.5591e-6, `p` 2.6694; k: 2.8774e-4, 5.4233e-5, 2.4075; omega: 7.2853e-5, 1.2244e-5, 2.5729; largest `nut_change` of a step 1.0e-11 / 2.7e-13 / 4.6e-14 (T/80, T/160, T/320) | [1.7, 2.3] | **missed** (above the band): ignored `s3_bdf2_time_order_known_miss` |
| S3 `euler` | `ux`: `e(80,160)` 4.7734e-4, `e(160,320)` 2.3997e-4, `p` 0.9922; k: 1.6647e-3, 8.5471e-4, 0.9617; omega: 8.9465e-4, 4.6180e-4, 0.9541 | [0.8, 1.2] | met |
| S3N | `bdf2`, `n_outer = 1`: `p` 0.7719 (`ux`), 0.8612 (k), 0.7297 (omega); largest `nut_change` of a step 0.74 to 0.76 | smallest `<= 1.3` | met (S3 can fail) |
| S4 | 10 of 10 steps finite, no guard trip; worst seam pair sum 1.065e-15; smallest k 2.19e-4, smallest omega 1.83; `max nut / nu` 8.717e-2 | `<= 1e-8` | met |
| S6 | H(dt) builds with no diagnostic: 13 627 cells, 1.44 s | as stated | met |
| V3H | 200 of 200 steps of H(dt) on B2 (13 627 cells), no failure and no guard trip, every field finite; 1.315 s per step (measured while another test ran on the machine) | report only | reported |

**S3, BDF2: the miss and its diagnosis.** The registered S3 test asserted BDF2 before Euler and stopped at the BDF2 miss, so the
Euler half was run afterwards, once (it had not run). The two halves are now separate tests: `s3_euler_time_order` (gate) and the
ignored `s3_bdf2_time_order_known_miss` (limit intact). Diagnosis, report only (`s3_bdf2_finer_levels_diagnosis`, ignored, run
once): the same runs one and two levels finer give `p` 3.3618 / 2.7810 / 2.7219 (`ux` / k / omega) from T/160, T/320, T/640 and
1.5526 / 2.2684 / 2.0082 from T/320, T/640, T/1280, with the outer iterations converged throughout (`nut_change` below 5e-14).
The observed order first rises above 2 and then falls back, so the registered levels are not in the asymptotic range of
this start-up. A likely cause, not proven here: the start goes from a uniform state into a channel whose near-wall k relaxes
on a time scale of about 0.06 (`1 / (beta* omega)` with omega up to 182, the steady maximum of R1), only 2.4 steps of
`dt = T/80 = 0.025`. At the finest pair, k and omega show the second order of the BDF2 time derivative (2.27, 2.01). `ux` shows 1.55 there, at an absolute
difference of 5.7e-6 m/s (3.2e-7 of `max ux`); these runs do not explain it, and the momentum side of the outer iteration is not
monitored by them. The library checks D1, D1v and D2 show the scheme of the k and omega equations exactly. The limit is not
changed.

**Mutations** (scratch, each applied alone to 3505192 and reverted; the named test run once):

| Id | Test | Measured | Result |
|---|---|---|---|
| M1 | D1 | fails at its precondition H0 in step 1, sweep 1: spread of k 4.937e-2 (the relaxed k equation does not keep a uniform field uniform) | fails |
| M2 | D2 `bdf2` | k: `e(40)` 1.1738e-2, `e(80)` 5.9084e-3, `p` 0.9903 (the Euler numbers) | fails |
| M3 | S3 `bdf2` | `p` 0.7721 / 0.8612 / 0.7297 (`ux` / k / omega; the numbers of S3N); largest `nut_change` of a step 0.74 to 0.76 | fails (below the band; the code of record fails above it) |
| M4 | D1v | 4.184e-3 | fails |
| M5 | S3 `bdf2` | `p` 1.0480 / 1.0877 / 0.9622; the outer iterations converge (`nut_change` 1.6e-10 to 4.6e-14) but the momentum keeps the face viscosity of the previous step | fails (below the band) |

**Predictions.** PR1 met (D1, D1v, D1b below 2e-14; D2 and D2v reproduce the recursion to the printed digits). PR2 falsified
in size: R1 met, at 2.7e-10 to 1.7e-9, not near round-off; these runs do not separate the convergence of the two iterations
from a difference between their fixed points at that level. PR3 met for Euler (0.95 to 0.99) and for S3N (0.86 for k), falsified
for BDF2 at the registered levels (2.4 to 2.7). PR4 met (1.1e-15). PR5 met (1.3 s per step).

**Check H of ADR 0183 is now runnable as registered** (S6), with the gap of question 4: its outer-iteration rule needs a
per-outer record that the case tree does not keep. The registered runs H(dt) and H(dt/2) were not made; they belong to the
EXP-ROT job.

Before the gate, on 3505192 in release: the non-ignored tests of `baseline_hashes`, `case_hashes_w48u`, `des_tree`,
`exp_rot_pab`, `exp_rot_pc` (PC-5 as amended), `f286_default_bits`, `f286_transient_sst`, `physics_schema`, `rsm_tree`,
`sgs_tree`, `sst_schema`, `transient_sources`, `turb_models` and `wall_options`: 85 passed, 0 failed; the `rcfd-fv` and
`rcfd-turb` suites: 600 passed, 0 failed, when the one-hour cap of that run stopped it (the gate runs them all).
`cargo clippy -p rcfd-turb -p rcfd-fv -p rcfd-api --all-targets -D warnings` and `cargo fmt --check` of the three packages
are clean on e5f974e.

**Questions added by the results** (each with a recommendation):

5. **The periodic seam of the scalar transport** (amendment 2). On a periodic seam `effective_viscosity` gives each face of a
   pair the diffusivity of its own owner cell, so the k, omega (and momentum) diffusion is not conservative across a seam where
   `nu_t` differs; FAN-01 and check H have rotational seams. Options: (a) leave it; (b) a roadmap item that gives the two faces
   of a pair one diffusivity (opt-in, defaults bit-identical), checked by C1's budget on the periodic box of the first set-up.
   Recommendation: (b), before check H is read.
6. **The S3 BDF2 miss.** Options: (a) keep it as the known miss it is, with the diagnosis above (as done); (b) register a new
   time-order check that starts from a resolved state (for example from the R1 steady state with the drive changed) and run it
   once. Recommendation: (a) now, (b) only if the maintainer wants a case-tree order record for URANS; D1 and D2 already show
   the order of the k and omega time derivative exactly.
