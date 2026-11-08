# ADR 0225: F-300 registration: transient RANS follow-ups of F-294 (the transient answer of the model registry, the time order of SST from a start that meets the walls, and a smooth wall eddy viscosity for the k-epsilon order miss)

Status: **REGISTERED 2026-11-08**, written before any code of F-300 and before any run of its checks. Results in addendum 1:
every registered check is met except S3s `bdf2`, a known miss (the smooth wall eddy viscosity leaves the k-epsilon BDF2 order as
it was, so the kink is not its cause); every negative control fired and every mutation made its check fail; defaults
bit-identical. F-300 is partial by the decision rule (question 5). Not yet accepted by the maintainer.

Parents: [ADR 0210](0210-f294-registration-transient-k-epsilon-and-spalart-allmaras-with-pimple.md) (F-294: questions 3, 5 and 7,
recommendations taken by the lead; the S1 test, the S3 k-epsilon BDF2 known miss and its diagnosis; probe P2),
[ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md) (F-286: S3, its BDF2 known miss and its question 6; part
B below is its amendment 4), [ADR 0212](0212-f296-registration-one-diffusivity-per-periodic-seam-pair-for-the-other-models.md)
(F-296: R1', which supersedes one case of ADR 0210 S1). Roadmap item F-300 (depends on F-294).

Number: the lead gave 0225 to F-300. Branch `f300-transient-rans` from main c201a770, worktree `w49i-work/f300`.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), as ADR 0199, ADR 0210 and ADR 0212 apply them):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded; it is diagnosed,
  not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run; the old test stays as an ignored
  record;
- every new path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded; no golden is edited or re-recorded; no case-schema key is added;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room: the time schemes, the models and the wall functions are those already in `crates/rcfd-fv/src/ddt.rs`,
`crates/rcfd-turb/src/{kepsilon,sa,sst,wall}.rs`. The smooth variant of part C uses Spalding's single-formula law of the wall
(Spalding, J. Appl. Mech. 28 (1961) 455-458), already implemented in `wall.rs` (`SpaldingLaw`) and recorded in
`docs/reference-data.md` (its row "Spalding (1961) law of the wall": the standard cubic form, confirmed by two secondary sources,
the original paper not read). No new source was read for F-300. No vendor code, text or install was used.

**Run before this text (disclosed).** No code of F-300 and no run of a check below. One acceptance probe of the existing code
(a scratch test in `crates/rcfd-api/tests/`, deleted, not committed; release build of main c201a770), printing only the
diagnostics of `Case::parse_toml`, `semantic_errors` and `Simulation::new`, never a field: the base texts of T1 below on
`simplec`, `piso` and `pimple`, and the V0b texts. Every text gave the diagnostics the code read predicts (item 2): no diagnostic,
except `turbulence.model` for k_epsilon, k_omega_sst and spalart_allmaras (DES off) on `piso`, for smagorinsky, wale and the
Reynolds stress model on `simplec` and for the test plug-in on `piso` and `pimple`, and `turbulence.sa.des` for SA-DES and
SA-DDES on `simplec`; CH-K with `pair` on `pimple` and the five V0b texts gave none. The registry lists, in this order,
`k_epsilon`, `k_omega_sst`, `spalart_allmaras`, `smagorinsky`, `wale`, `rsm`, `mixing_length` (and the probe's plug-in). These
results choose the base texts of T1 and the V0b texts and nothing else. The time-order numbers of part B are those of probe P2 of
ADR 0210 (existing code, run before ADR 0210), which this ADR does not re-run before the registered run.

## What F-300 asks

The roadmap entry: every limit is fixed by its registration ADR before any code. (a) `rcfd_turb::supports_transient` says which
models run transient on pimple (k-epsilon, SA without DES, SST), checked by a test against what the case tree accepts; (b) an
amendment to ADR 0199 re-registers S3 for SST from the wall-matching start that F-294 used (BDF2 order in the band in the probe),
the old S3 kept as an ignored record; (c) k-epsilon BDF2 order (known miss of ADR 0210, p = 0.80 / 1.49 / 1.53): test the suspected
cause, the kink of the wall-function eddy viscosity at `y* = 11.53`, with a smooth variant under a registered check; defaults
bit-identical. The lead's brief adds: the S1 test of ADR 0210 (`f294_transient_keps_sa.rs`, `s1_scope_of_transient_kepsilon_and_sa`)
is an ignored test, superseded in one case by ADR 0212 R1'; its other accept and refuse cases are restored under a registered,
corrected scope as part of (a), and the old S1 stays as an ignored record.

## What the code read shows (main c201a770)

1. **`rcfd_turb::supports_transient`** (`crates/rcfd-turb/src/model.rs`): `matches!(key, "smagorinsky" | "wale" | "mixing_length"
   | "rsm")`, with the doc "the RANS transport models and plug-ins are steady only". It is re-exported by `lib.rs`; nothing else in
   the workspace calls it (searched: only its definition, the re-export, ADR 0210 question 3 and the roadmap). Since F-286 and
   F-294 its answer is wrong for `k_epsilon`, `k_omega_sst` and `spalart_allmaras`.
2. **What the case tree accepts** (`turb::check`, `crates/rcfd-api/src/turb.rs`). With `solver.algorithm` transient: the Reynolds
   stress model on `piso` and `pimple` only; Spalart-Allmaras with `turbulence.sa.des` other than `off` on `piso` and `pimple` only;
   smagorinsky and wale on `piso` and `pimple` only; mixing_length on `simple`, `simplec`, `piso`, `pimple`; `k_omega_sst`,
   `k_epsilon` and `spalart_allmaras` (DES off) on `pimple` (ADR 0199 D3, ADR 0210 D4) and refused on `piso` at `turbulence.model`
   with a message that names `pimple`; a plug-in (`turbulence.model = "plugin"`) on the steady algorithms only. The registry keys
   are listed in the disclosure above.
3. **The S1 test of ADR 0210** (`crates/rcfd-api/tests/f294_transient_keps_sa.rs`). The lead's F-219 merge (50c4a83c) made it an
   ignored test with the reason "superseded by ADR 0212 R1' (F-296): ... the cases are in S1' of f296_seam_diffusivity.rs ...". One
   of its cases, "Located at `turbulence.periodic_seam.diffusivity`: both on `pimple` with `diffusivity = "pair"`", contradicts R1'
   (`pair` is accepted with both models). S1' of ADR 0212 checks `pair` with k_epsilon and spalart_allmaras on SIMPLEC only; the
   other cases of ADR 0210 S1 (the schemes, variants, wall options, the `piso`, energy, dynamic-mesh, error-tolerance and
   fractional-step refusals, the sector texts, the moving wall of amendment 1, the dispatch answers) are run by no test today.
4. **ADR 0199 S3** (`crates/rcfd-api/tests/f286_transient_sst.rs`). `s3_run` builds CH from the uniform start `(15, 0, 0)` next to
   walls at rest; `s3_bdf2_time_order_known_miss` (ignored, p 2.6694 / 2.4075 / 2.5729), `s3_euler_time_order` (gate, met) and
   `s3n_one_sweep_per_step_loses_the_order` (gate, met). ADR 0210 probe P2 ran the same three BDF2 runs from `ux = 15 y (2 - y)`:
   p 2.0450 / 2.0394 / 2.0306 (`ux` / k / omega), and Euler 1.0030 / 0.9800 / 1.0028.
5. **The kink of the k-epsilon wall eddy viscosity** (`crates/rcfd-turb/src/wall.rs`, `kepsilon.rs`).
   `WallConstants::nut_wall_from_k` is `nu (y* kappa / ln(E y*) - 1)` for `y* >= y*_lam` and 0 below, `y* = C_mu^(1/4) k^(1/2) y /
   nu`; at `y*_lam` (11.530 for kappa 0.41, E 9.8) its value is 0 from both sides and its slope jumps from 0 to `kappa (kappa y*_lam
   - 1) / (kappa y*_lam)^2 nu = 0.0684 nu`. It enters a k-epsilon run in two places, with the standard wall function on a smooth
   wall: (i) the wall-face eddy viscosity `wall_nut` (`KEpsilon::wall_face_nut`), the wall shear of the momentum equation; (ii) the
   wall-cell production of `wall_cell_sources_ext`, `log_prod = (nu + nut_w) u_t / y C_mu^(1/4) k^(1/2) / (kappa y)`, which the
   default `smooth` treatment multiplies by `r^6 / (1 + r^6)`, `r = y* / y*_lam`, and blends with the smooth epsilon
   `sqrt(eps_v^2 + eps_l^2)`. So with the default treatment the kink of `nut_w` is the only point where the wall-cell coefficients
   are not smooth in `y*`. Rough walls use `nut_wall_from_k_rough` (the same switch), the scalable and enhanced functions their own
   formulas. `SpaldingLaw` gives `y+(u+)`, `dy+/du+` and `u_tau(u, y)` (bisection); there is no `u+(y+)`.
6. **No route to a model option from a case-tree run.** `TurbState::new` passes the options `periodic_diffusivity`, `des`,
   `des_delta`, `c_des` and `pressure_strain` from the case; `KEpsilonModel::set_option` accepts `periodic_diffusivity` only; the
   `Simulation` has no accessor to its turbulence model. The engines have the accessors `simple_solver_mut` and
   `pimple_solver_mut` ("diagnostics and tests"), through which ADR 0214 reached its library opt-in from case-tree tests.

## Decisions (fixed now)

### Part A: the transient answer of the registry and the corrected S1 scope

**D1. `supports_transient`.** It returns `true` for `k_epsilon`, `k_omega_sst`, `spalart_allmaras`, `smagorinsky`, `wale`,
`mixing_length` and `rsm`, `false` for every other key. The doc states, per key, what item 2 shows: k_epsilon, k_omega_sst and
spalart_allmaras run transient with `pimple` (one sweep with the time derivative at the start of every outer iteration, ADR 0199
and ADR 0210) and are refused with `piso`, while spalart_allmaras with `turbulence.sa.des = "des"` or `"ddes"` runs on `piso` and
`pimple` with its once-per-step Euler term (ADR 0032); smagorinsky, wale and rsm are transient only; mixing_length runs steady and
transient; any other key (a plug-in's) answers `false`, and the case tree refuses `turbulence.model = "plugin"` on the transient
algorithms (ADR 0210 question 4). The answer is per key, so for spalart_allmaras it covers every DES mode (question 1).
`is_sgs_key` is unchanged.

**D2. S1c, the corrected scope of ADR 0210 S1.** A new test registers every case of ADR 0210 S1 and its amendment 1 unchanged,
except one: CH-K and CH-A on `pimple` with `[turbulence.periodic_seam] diffusivity = "pair"` are **accepted with no diagnostic**
(ADR 0212 R1') instead of located at the key. The old test `s1_scope_of_transient_kepsilon_and_sa` stays an ignored record; its
body is not changed, and the reason of its `#[ignore]` is replaced so that it names this ADR and the new test (the only edit of an
existing test file). `f286_transient_sst.rs` is not edited.

### Part B: ADR 0199 amendment 4

**D3.** S3 of ADR 0199 is re-registered as S3' with the start of ADR 0210 S3, `ux = 15 y (2 - y)` (zero on both walls, the same
centre speed), everything else as ADR 0199 S3 and S3N; the text is ADR 0199 amendment 4 (written into ADR 0199 with this
registration). The uniform start next to walls at rest is an impulsive start, whose solution is not smooth in time at `t = 0`, so
the registered levels of the old S3 were not in the asymptotic range of the scheme (ADR 0210 P2). The old
`s3_bdf2_time_order_known_miss` stays an ignored record with its reason; `s3_euler_time_order` and
`s3n_one_sweep_per_step_loses_the_order` stay gate tests as they are (both met in ADR 0199). No code changes for part B.

### Part C: a smooth wall eddy viscosity for the k-epsilon S3 miss

**D4. The smooth variant (library).** `WallConstants::nut_wall_from_k_spalding(nu, k, y) = max(0, nu (y* / u+_S(y*) - 1))`, with
`y* = C_mu^(1/4) max(k, 0)^(1/2) y / nu` and `u+_S` the root of Spalding's law `y+(u+) = y*` with the model's `kappa` and `E`
(`SpaldingLaw { kappa, e }` of the wall constants); 0 for `y* <= 0`. Reasoning: the standard function is `nu (y* / u+(y*) - 1)`
with the two-branch law `u+ = min(y*, ln(E y*) / kappa)`, whose `min` is the kink; Spalding's law is one analytic formula with the
same two limits and the same constants, the law that rcfd's SA and SST wall treatments already use, and it has no width or other
parameter to choose. It is larger than the standard value in the buffer layer (about `0.25 nu` at `y*_lam`, 6 percent more wall
viscosity at `y* = 30`) and tends to it in both limits (computed with the formulas above, outside the code). The `max(0, .)` only
removes a round-off sign; analytically `y* / u+_S > 1` for every `y* > 0`.

`SpaldingLaw::u_plus(y_plus)`: the root of `y_plus(u) = y+` by bisection on `[0, y+]` (the law is increasing, `y+(u) >= u`) until
the bracket is below `1e-3` of its upper end, then Newton's method from the upper end (the law is convex for `u >= 0`, so the
iterates decrease monotonically to the root) until a step is at most `1e-15 u`, at most 50 steps; 0 for `y+ <= 0`.

`kepsilon.rs`: `pub enum WallNutLaw { Standard (default), Spalding }` and the field `KEpsilon::wall_nut_law` (default
`Standard`). With `Spalding`, on a smooth wall with the standard wall function, both uses of item 5 take
`nut_wall_from_k_spalding`: the wall-face eddy viscosity of `wall_face_nut` and the `nut_w` of the wall-cell production, through a
new public `wall_cell_sources_law(.., law)`; `wall_cell_sources_ext` becomes a call of it with `Standard` (the same operations). Rough
walls, the scalable and the enhanced wall functions ignore the field. Every wall treatment (`smooth`, `switch`, `hysteresis`) takes
the law; only `smooth` is free of a switch.

`KEpsilonModel::set_option("wall_nut_law", "standard" | "spalding")`: sets the field; another value is refused with a message;
`spalding` is refused when the model's wall function is not `standard` or its wall is rough (the field would do nothing). The
existing `periodic_diffusivity` option and the message for an unknown option name are unchanged. No case key: the law is not
reachable from a case file (question 3).

**D5. The case-tree route of the checks** (`crates/rcfd-api/src/sim.rs`). `Simulation::turbulence_model_mut(&mut self) ->
Option<&mut dyn TurbulenceModel>`: mutable access to the turbulence model of a RANS run, "diagnostics and tests", as
`pimple_solver_mut`; its doc says that a change made through it is not part of the case and that `reconfigure_turbulence` or a new
simulation drops it. The checks of part C set `wall_nut_law = spalding` through it after `Simulation::new` and before the first
step. Before the first sweep the model's wall eddy viscosity is empty (`KEpsilon::new`), so a run with the option set there is the
run of a model built with it.

**D6. Code (the plan, with the files).** `crates/rcfd-turb/src/model.rs` (D1; `KEpsilonModel::set_option`, D4),
`crates/rcfd-turb/src/wall.rs` (`SpaldingLaw::u_plus`, `WallConstants::nut_wall_from_k_spalding`), `crates/rcfd-turb/src/kepsilon.rs`
(`WallNutLaw`, the field, `wall_cell_sources_law`, `wall_face_nut`, the sweep's call), `crates/rcfd-turb/src/lib.rs` (re-export of
`WallNutLaw` and `wall_cell_sources_law`), `crates/rcfd-api/src/sim.rs` (D5). Tests: `crates/rcfd-turb/tests/f300_wall_nut_law.rs`,
`crates/rcfd-api/tests/f300_transient_rans.rs`, `crates/rcfd-api/tests/f300_default_bits.rs`, and the `#[ignore]` reason of D2 in
`f294_transient_keps_sa.rs`. Documents: ADR 0199 amendment 4, a note at the end of ADR 0210, `docs/reference-data.md` (a short
F-300 section: no new source), the F-300 entry of `docs/ROADMAP.md` and `docs/roadmap.json`. `docs/case-schema.md` and
`schema.rs` are not changed (no key, no value, no message changes).

**D7. Why the defaults stay bit-identical.** No case key is added, so every settings value, content hash and written text is
unchanged. `supports_transient` has no caller. With `WallNutLaw::Standard` (the default, and the only value a case can reach)
`wall_face_nut` and the sweep run the same operations in the same order (`wall_cell_sources_ext` delegates with `Standard`, the
branch on the field selects the existing call). `turbulence_model_mut` is a new accessor that nothing calls. V0 checks this.

## Set-ups

**CH** (ADR 0199): `box_2d` 4 x 20, `lx = 1`, `ly = 2`, periodic in x, `nu = 1/395`, walls `ymin`, `ymax` at rest, the drive
acceleration `(1, 0, 0)` on all cells, momentum and turbulence convection `upwind`, initial velocity `(15, 0, 0)`. Model lines: SST
`k_initial = 1`, `omega_initial = 40`; **CH-K** `k_epsilon`, `k_initial = 1`, `epsilon_initial = 3.6`; **CH-A**
`spalart_allmaras`, `nutilde_ratio = 40` (ADR 0210). **PARABOLA**: `[initial] velocity = [0, 0, 0]`,
`velocity_expression_x = "15 [m/s] * (y / 1 [m]) * (2 - y / 1 [m])"` (ADR 0210 S3). The S4 sector texts of ADR 0210 (with
amendment 1). **RSMBOX**: the doubly periodic box of `rsm_tree.rs` (4 x 4, `nu = 0.001`, no wall) with `rsm`, `k_initial =
epsilon_initial = 1`. The S3 solver lines (ADR 0199 S3): `pimple`, `n_outer = 20`, `n_correctors = 2`, `momentum_rel_tol =
pressure_rel_tol = 1e-12`, `pressure_max_iter = 1000`, `T = 2`, `dt = T/80, T/160, T/320`; `e_ij = max_c abs(q_i - q_j) / max_c
abs(q_finest)` at `T`, `p = log2(e(80,160) / e(160,320))`. The set-ups are copied into the new test files from
`f286_transient_sst.rs`, `f294_transient_keps_sa.rs` and `rsm_tree.rs`; those files are not changed (D2 aside).

## Checks and limits (fixed now)

Every check is run once, in release. All are gate tests unless marked otherwise. "Accepted" means no diagnostic from
`Case::parse_toml`, `semantic_errors` and `Simulation::new`; "located at P" means P is among the diagnostic paths.

### Part A (`crates/rcfd-api/tests/f300_transient_rans.rs`)

The test plug-in: `register_model("f300_plugin", .., KEpsilonModel)`. Base texts: CH with the model lines of SST, CH-K or CH-A for
the three keys of D1's first group; CH with `model = "smagorinsky"`, `"wale"`, `"mixing_length"` (`length_scale = 1`); RSMBOX for
`rsm`; CH with `model = "plugin"`, `plugin = "f300_plugin"` for every key that is not built in. The pimple lines: `algorithm =
"pimple"`, `time_scheme = "euler"`, `n_outer = 3`, `dt = 0.1`, `end_time = 1`; the piso lines: `algorithm = "piso"`, `dt = 0.1`,
`end_time = 1`; the reference lines: `simplec` (`max_iterations = 10`) for k_epsilon, k_omega_sst, spalart_allmaras,
mixing_length and the plug-in, `piso` for smagorinsky, wale and rsm.

| Id | Check | Limit |
|---|---|---|
| T1-0 | precondition: every base text with its reference lines | accepted (a miss is a defective set-up) |
| T1 | for every key of `rcfd_turb::model_keys()` (the seven built-in keys and the test plug-in): the base text with the pimple lines; and spalart_allmaras with `turbulence.sa.des = "des"` and `"ddes"` | `supports_transient(key)` true and accepted, or false and located at `turbulence.model`, for every key; SA-DES and SA-DDES accepted (true is the answer for every DES mode) |
| T1N | negative control: main's predicate (copied into the test) against the same answers | it disagrees for exactly `k_epsilon`, `k_omega_sst`, `spalart_allmaras` |
| T1p | the doc's `piso` statement: every base text with the piso lines | k_epsilon, k_omega_sst, spalart_allmaras (DES off) and the plug-in located at `turbulence.model`; SA-DES, SA-DDES, smagorinsky, wale, mixing_length and rsm accepted |
| S1c | ADR 0210 S1 with amendment 1 and D2: accepted: CH-K on `pimple` with `euler`, `bdf2`, `bdf2_bounded`; with `variant = "rng"`, `"realizable"`; `wall_function = "scalable"`, `"enhanced"`; `wall_treatment = "hysteresis"`; CH-A on `pimple` with `euler`, `bdf2`, `bdf2_bounded`; `sa.wall_mode = "resolved"`, `"auto_sa_law"`; `sa.limiter = "min_0p3"`; `sa.negative_nutilde = false`; `sa.des = "ddes"` on `pimple` and on `piso`; the two S4 sector texts; **CH-K and CH-A on `pimple` with `periodic_seam.diffusivity = "pair"`**. Located at `turbulence.model`: CH-K and CH-A on `piso` (the message names `pimple` and contains `steady`); both on `pimple` with `energy.enabled`. At `dynamic_mesh.enabled`: both on `pimple` with a dynamic mesh. At `solver.error_tolerance`: both with `error_tolerance = 1e-3`. At `solver.time_advance`: both without the drive on `pimple` with `fractional_step`. At `boundaries.outer.velocity`: the k-epsilon sector with the moving wall. Registry: `updates_in_outer_iterations()` true for `k_epsilon` and for `spalart_allmaras` with `des = off`, false with `des` and `ddes` | as stated |

### Part B (ADR 0199 amendment 4; `crates/rcfd-api/tests/f300_transient_rans.rs`)

| Id | Check | Limit |
|---|---|---|
| S3' | CH (SST) from PARABOLA with the S3 solver lines, `q = ux, k, omega` | `bdf2`: `p` in [1.7, 2.3] for each `q`; `euler`: `p` in [0.8, 1.2] for each `q`. Reported: the largest `nut_change` of a step |
| S3'N | negative control: S3' `bdf2` with `n_outer = 1` | the smallest `p` of the three fields `<= 1.3` |

The bands are those of ADR 0199 S3 and S3N. The expectation is P2's numbers (BDF2 2.045 / 2.039 / 2.031, Euler 1.003 / 0.980 /
1.003), from the same code.

### Part C (`crates/rcfd-turb/tests/f300_wall_nut_law.rs`, `crates/rcfd-api/tests/f300_transient_rans.rs`)

`g(y*) = nut_wall_from_k_spalding(nu, k(y*), y) / nu` with `nu = 1e-3`, `y = 1e-2`, `k(y*) = (y* nu / (C_mu^(1/4) y))^2`, the
default wall constants (`kappa = 0.41`, `E = 9.8`, `C_mu = 0.09`); `g_std` the same with `nut_wall_from_k`; `y_l = y*_lam`;
`h = 1e-3 y_l`; `D- = (g(y_l) - g(y_l - h)) / h`, `D+ = (g(y_l + h) - g(y_l)) / h`.

**One-cell set-up OC.** `box_2d(1, 1, 1.0, 1.0)` with no periodic direction, the model's only wall `ymin` (y = 0.5), `KEpsilon`
standard, smooth treatment, standard wall function, `nu = 0.01`, `u = (1, 0, 0)` uniform, `phi = 0`, `eps0 = 1`, `relax_turb =
1`, linear solver `tol_rel = 1e-14`, `tol_abs = 1e-300`, `max_iter = 1000`; `k0 = k(y*)` for `y* = 5, y*_lam, 30` (with `nu`, `y`
of OC). One steady sweep. The k equation of the one cell is then `(eps_w / k0) V k' = G_w V` (no internal face, zero-gradient k on
every side, no flux, relaxation 1), so `k' = G_w k0 / eps_w` with `(G_w, eps_w) = wall_cell_sources_law(smooth, standard, smooth
wall, .., nu, k0, 0.5, 1, law)`.

| Id | Check | Limit |
|---|---|---|
| W1 | (i) for `y* = 10^(-2 + i/10)`, `i = 0 .. 60`: `abs(y+_S(u+_S(y*)) - y*) <= 1e-12 y*`; (ii) `g >= 0` on that grid; `g(1e-2) <= 1e-6`; `abs(g(1e3) / g_std(1e3) - 1) <= 1e-2`; (iii) smooth at the intersection: `abs(D+ - D-) <= 1e-2 max(abs(D+), abs(D-))` | as stated |
| W1N | negative control: (iii) for `g_std` | `abs(D+ - D-) >= 0.5 max(abs(D+), abs(D-))` |
| W2 | OC, each `y*`, each law (`Standard`, `Spalding`): precondition W2-0 (the cell's wall face is the `ymin` face, `y = 0.5` to `1e-14`); `abs(k' - G_w k0 / eps_w) <= 1e-12 k'`; the wall-face eddy viscosity after the sweep has the bits of the law's `nut(k', 0.5)`. Non-vacuity: `abs(k'_Spalding - k'_Standard) >= 1e-3 k'_Standard` at each `y*` | as stated |
| T3 | CH-K from PARABOLA with the S3 solver lines, `bdf2`, `dt = T/80`, `wall_nut_law = spalding` set through D5, 5 steps: every face of `ymin` and `ymax` in `momentum_face_viscosity()` has the bits of `nu + nut_wall_from_k_spalding(nu, k_c, y_f)` (`k_c` the field `k` of its owner, `y_f` the face's `dn`); refusals: `set_option("wall_nut_law", "kader")` is `Err`, and on CH-K with `wall_function = "scalable"` `set_option("wall_nut_law", "spalding")` is `Err` | exact; as stated |
| T3N | negative control: the T3 run without the option: every wall face has the bits of `nu + nut_wall_from_k(nu, k_c, y_f)`; the largest relative difference of the wall faces between the two runs | exact; `>= 1e-3` |
| S3s | CH-K from PARABOLA with the S3 solver lines and `wall_nut_law = spalding` (through D5, before the first step), `q = ux, k, epsilon` | `bdf2`: `p` in [1.7, 2.3] for each `q`; `euler`: `p` in [0.8, 1.2] for each `q`. Reported: the largest `nut_change` of a step, the wall-cell `y*` range of the T/320 `bdf2` run |
| S3sN | negative control: S3s `bdf2` with `n_outer = 1` | the smallest `p` `<= 1.3` |

**The reading of S3s (fixed now).** S3s differs from the ADR 0210 S3 run of record only by the law of the wall eddy viscosity. If
S3s `bdf2` meets its band, the kink is supported as the cause of the ADR 0210 S3 known miss (supported, not proven: the smooth law
also changes the buffer-layer viscosity, so the trajectory differs). If it misses, the kink is not shown to be the cause; S3s `bdf2`
becomes an ignored known miss with its numbers, and no other variant is tried under this ADR. In both cases the ADR 0210 known miss
`s3_kepsilon_bdf2_time_order_known_miss` stays as it is: the default law is unchanged.

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs`, `case_hashes_w48u.rs`, `f286_default_bits.rs`, `f294_default_bits.rs`, `f295_default_bits.rs`, `f296_default_bits.rs` unedited | pass |
| V0b | `crates/rcfd-api/tests/f300_default_bits.rs`: FNV-1a (as `f294_default_bits.rs`) of every cell field among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut` and of the face flux, recorded on main c201a770 with this text and the test only, committed before any code: CH-K on `pimple`, `bdf2`, `n_outer = 3`, `dt = 0.1` (20 steps, the F-294 path); the same with `euler` and `wall_treatment = "switch"`; with `wall_function = "scalable"`; with `roughness_height = 0.002`; steady SIMPLEC CH-K with `wall_treatment = "hysteresis"` (300 iterations); CH-A on `pimple`, `bdf2`, `n_outer = 3` (20 steps) | bit-identical after the code |
| V0c | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs, the gate lock) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the addendum.

| Id | Mutation | Must fail |
|---|---|---|
| MA1 | `supports_transient` returns main's answer | T1 |
| MA2 | `turb::check` accepts k_epsilon and spalart_allmaras with `piso` | S1c |
| MB1 | the hook of `Simulation::step` makes the turbulence sweep at outer iteration 0 only (ADR 0199 M3) | S3' `bdf2` |
| MB2 | the hook makes the sweep and returns `nu_face = None` (ADR 0199 M5) | S3' `bdf2` |
| MC1 | `nut_wall_from_k_spalding` returns `nut_wall_from_k` | W1 |
| MC2 | `KEpsilonModel::set_option("wall_nut_law", ..)` returns `Ok` without setting the field | T3 |
| MC3 | the sweep takes the wall-cell sources of `wall_cell_sources_ext` (the default law) | W2 |

### Classes

V0 is a precondition of landing: if V0a or V0b misses, the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control (T1N, S3'N,
W1N, T3N, S3sN, the non-vacuity parts of W2) or a mutation that does not make its check fail means that the check is not shown to
be able to fail: it is reported as a defective set-up, corrected by an amendment before any re-run, and never counted as a pass. A
miss of a precondition (T1-0, W2-0) is a defective set-up of its check.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | T1, T1p and S1c meet (the probe saw every base text behave as item 2 says) |
| PR2 | S3' reproduces P2 to the printed digits; S3'N gives `p` near 1 |
| PR3 | W1 (iii) near `1e-3`; W2 at round-off; T3 exact |
| PR4 | S3s `bdf2` meets its band (1.9 to 2.1); S3s `euler` meets; S3sN fires |

## Budgets

Every gate test above takes under one minute in release (ADR 0210: about 17 s for the three S3 runs of one scheme), the library
checks well under a second.

## Not in scope (and why)

- A case key for the law (question 3), a change of the default wall eddy viscosity, the scalable, enhanced and rough-wall
  functions.
- The enhanced wall function on `pimple` (ADR 0210 question 6, answered: accepted and documented for wall-resolved meshes).
- `pair` on `pimple` beyond the S1c acceptance (F-302 has the case-tree check of ADR 0212 question 4).
- The roadmap status of F-286 (question 4).

## Open questions for the maintainer (each with a recommendation)

1. **A per-key answer for Spalart-Allmaras.** `supports_transient("spalart_allmaras")` is true for every DES mode, though DES off
   runs on `pimple` only and DES on both. Options: (a) as registered, with the algorithms in the doc; (b) a new function with the
   algorithm and the DES mode. Recommendation: (a); nothing calls it, and the case tree is the place of the rule.
2. **`Simulation::turbulence_model_mut`** (D5). Options: (a) keep it as a diagnostics and tests accessor, as `pimple_solver_mut`;
   (b) remove it once a case key exists for what it is used for. Recommendation: (a).
3. **The Spalding wall eddy viscosity as a user option.** Options: (a) library only, as registered; (b) a case key (for example
   `turbulence.wall_nut_law`, `hash_omit_default`), which needs a registered schema change. Recommendation: (a) unless a case needs
   a k-epsilon wall function without a switch; then (b) with its own registration.
4. **F-286's roadmap line.** It records the ADR 0199 S3 BDF2 known miss. With amendment 4 the registered S3 is S3'. Recommendation:
   the lead adds the S3' result to the F-286 entry (this branch edits only F-300's entry, as the brief asks).

## Addenda

Results of the checks follow in an addendum, in the order of the tables.

### Amendment 1 (2026-11-08): two V0b runs with the wall cells in the log layer, written before any code and before their run

Found while recording V0b on c201a770 (this text and the test only, no code of F-300): the four transient CH-K runs of V0b start
from k 1, so the wall cells sit at `y*` about 10.8, below `y*_lam`, where `nut_wall_from_k` is 0 on both sides of the switch; in 20
steps they do not reach the log branch. The rough-wall run (`roughness_height = 0.002`) gave the bits of the first run, and so did
an unregistered attempt with `roughness_height = 0.02` (run once while recording, then dropped). The steady run reaches the log
layer (the steady wall cells sit near `y* = 19`, ADR 0210 P1), the transient ones do not, so V0b does not show that the log
branch of the transient path is unchanged. The six registered runs keep their records (recorded on c201a770: 0x89acd6f402fb0b3c,
0xdc8f65037faa4a96, 0x3f140c900c0a7c17, 0x89acd6f402fb0b3c, 0xc9540fae514ff3f8, 0x0ce33c79d483d023). V0b adds two runs, recorded on
the same commit before any code: CH-K on `pimple`, `bdf2`, `n_outer = 3`, `dt = 0.1`, 20 steps, from `k_initial = 2`,
`epsilon_initial = 14.4` (the start of the ADR 0210 S3 diagnosis, the same initial eddy viscosity; wall cells at `y*` about 15);
the same with `roughness_height = 0.02` (`k_s+` about 6, transitional). The test prints the wall-cell `y*` range at the end of each
run (report only). No limit changes.

### Addendum 1: what was built and the results (2026-11-08)

Commits: registration 64763621, amendment 1 9d0e28d0, the V0b record 35471ce9 (main c201a770 with this text and the test only,
before any code), code and tests d26ce83d, the S3s known miss marked 6e4e650a. Built as D6 says: `rcfd-turb/src/model.rs`
(`supports_transient`, `KEpsilonModel::set_option("wall_nut_law", ..)`), `rcfd-turb/src/wall.rs` (`SpaldingLaw::u_plus`,
`WallConstants::nut_wall_from_k_spalding`), `rcfd-turb/src/kepsilon.rs` (`WallNutLaw`, `KEpsilon::wall_nut_law`,
`wall_cell_sources_law`, the branch in `wall_face_nut`, the sweep's call), `rcfd-turb/src/lib.rs` (re-exports),
`rcfd-api/src/sim.rs` (`Simulation::turbulence_model_mut`); tests `crates/rcfd-turb/tests/f300_wall_nut_law.rs`,
`crates/rcfd-api/tests/f300_transient_rans.rs`, `crates/rcfd-api/tests/f300_default_bits.rs`, the `#[ignore]` reason of the ADR 0210
S1 record in `f294_transient_keps_sa.rs`. Documents: ADR 0199 amendment 4, the note at the end of ADR 0210, the F-300 section of
`docs/reference-data.md`, the F-300 entries of `docs/ROADMAP.md` and `docs/roadmap.json`. No case key, schema text, golden,
`baseline_hashes.rs` or `case_hashes_w48u.rs` was touched; `Cargo.lock` is unchanged. Every check was run once, in release, on
d26ce83d; the report-only diagnosis of the S3s miss (below) ran once on 6e4e650a.

Summary: every registered check is met except S3s `bdf2`, a known miss. `supports_transient` now agrees with the case tree for
every registry key (main's answer disagreed for k_epsilon, k_omega_sst and spalart_allmaras); the scope of ADR 0210 S1 is
restored as S1c; SST from the start that meets the walls is second order with BDF2 (S3', the numbers of P2 to the printed digits).
The Spalding wall eddy viscosity has no kink at `y*_lam` and reaches the production and the wall faces, but with it the k-epsilon
BDF2 order on the channel start-up is the same as with the standard law (0.80 / 1.48 / 1.43 against 0.80 / 1.49 / 1.53), and at
finer levels it rises towards 2 in the same way: the kink is not the cause of the ADR 0210 miss. Every negative control fired. The
defaults are bit-identical.

| Id | Measured | Limit | Result |
|---|---|---|---|
| T1-0 | the eight base texts (seven built-in keys and `f300_plugin`) accepted with their reference lines | accepted | met |
| T1 | registry keys `k_epsilon`, `k_omega_sst`, `spalart_allmaras`, `smagorinsky`, `wale`, `rsm`, `mixing_length`: `supports_transient` true, accepted on `pimple`; `f300_plugin`: false, located at `turbulence.model`; SA with `des = "des"` and `"ddes"` accepted on `pimple` | as stated | met |
| T1N | main's answer disagrees with the case tree for exactly `k_epsilon`, `k_omega_sst`, `spalart_allmaras` | as stated | met (T1 can fail) |
| T1p | `piso`: `k_epsilon`, `k_omega_sst`, `spalart_allmaras` and `f300_plugin` located at `turbulence.model`; `smagorinsky`, `wale`, `mixing_length`, `rsm`, SA-DES and SA-DDES accepted | as stated | met |
| S1c | every case as registered, `pair` with CH-K and CH-A on `pimple` accepted | as stated | met |
| S3' `bdf2` | `ux`: `e(80,160)` 1.5819e-5, `e(160,320)` 3.8332e-6, `p` 2.0450; k: 6.7420e-5, 1.6401e-5, 2.0394; omega: 1.5805e-5, 3.8685e-6, 2.0306; largest `nut_change` of a step 3.3e-14 at every level | [1.7, 2.3] | met |
| S3' `euler` | `ux`: 6.8600e-4, 3.4228e-4, 1.0030; k: 3.4130e-3, 1.7304e-3, 0.9800; omega: 6.1601e-4, 3.0740e-4, 1.0028; `nut_change` 6.9e-13 / 3.4e-14 / 3.4e-14 | [0.8, 1.2] | met |
| S3'N | `p` 0.8007 (`ux`), 0.8503 (k), 0.8038 (omega); largest `nut_change` of a step 0.52 to 0.54 | smallest `<= 1.3` | met (fired) |
| W1 | (i) largest `abs(y+_S(u+_S(y*)) - y*) / y*` on the grid 1.145e-15; (ii) `g >= 0` on the grid, `g(1e-2)` 1.202e-10, `g(1e3) / g_std(1e3) - 1` 4.325e-4; (iii) `D-` 5.014909e-2, `D+` 5.017801e-2, `abs(D+ - D-) / max` 5.763e-4; `g(y*_lam)` 0.2521 | 1e-12; 1e-6, 1e-2; 1e-2 | met |
| W1N | standard: `D-` 1.9e-14, `D+` 6.837783e-2 (the slope `0.0684 nu` of item 5), `abs(D+ - D-) / max` 1.000 | `>= 0.5` | met (fired) |
| W2 | W2-0 met in every run; `abs(k' - G_w k0 / eps_w) / k'` at most 4.4e-15 (Standard: 1.3e-15, 4.3e-16, 3.9e-16; Spalding: 4.4e-15, 0, 3.7e-16 at `y*` 5, 11.53, 30); the wall-face eddy viscosity has the law's bits in all six runs (Standard 0 in all three, because `k'` puts `y*` below `y*_lam`; Spalding 2.03e-7, 3.08e-4, 2.10e-3); `abs(k'_Spalding - k'_Standard) / k'_Standard` 2.23e-2, 0.252, 6.08e-2 | 1e-12; exact; `>= 1e-3` | met |
| T3 | 8 of 8 wall faces with the bits of `nu + nut_wall_from_k_spalding(nu, k_c, y_f)` after 5 steps (all `1.128601 nu`); `kader` refused (`'kader' is not a valid value of the option 'wall_nut_law' (standard or spalding)`); `spalding` with the scalable wall function refused (`wall_nut_law = spalding acts on the standard wall function on a smooth wall only (ADR 0225)`) | exact; as stated | met |
| T3N | 0 of 8 wall faces off the bits of `nu + nut_wall_from_k`; largest relative difference of the two runs' wall faces 0.1286 | exact; `>= 1e-3` | met (fired) |
| S3s `bdf2` | `ux`: `e(80,160)` 1.6613e-5, `e(160,320)` 9.5214e-6, `p` 0.8031; k: 2.8192e-4, 1.0097e-4, 1.4814; epsilon: 5.3269e-4, 1.9788e-4, 1.4287; largest `nut_change` of a step 1.8e-8 / 2.9e-12 / 5.9e-16 (T/80, T/160, T/320); wall-cell `y*` of the T/320 run in [8.8101, 13.4234] | [1.7, 2.3] | **missed** (below the band): ignored `s3s_kepsilon_spalding_bdf2_time_order_known_miss` |
| S3s `euler` | `ux`: 9.2443e-4, 4.7525e-4, 0.9599; k: 3.6407e-3, 1.8167e-3, 1.0029; epsilon: 7.1858e-3, 3.6482e-3, 0.9780 | [0.8, 1.2] | met |
| S3sN | `p` 1.0063 (`ux`), 0.7464 (k), 0.2002 (epsilon); largest `nut_change` of a step 0.54 to 0.61 | smallest `<= 1.3` | met (fired) |
| V0b | 8 of 8 runs bit-identical to the record of c201a770 (0x89acd6f402fb0b3c, 0xdc8f65037faa4a96, 0x3f140c900c0a7c17, 0x89acd6f402fb0b3c, 0x02fd48933dabaa94, 0x70a0889f7f89bccd, 0xc9540fae514ff3f8, 0x0ce33c79d483d023); the two runs of amendment 1 start near `y* = 15` and end with the wall-cell `y*` in [11.07, 11.23] (smooth wall) and [13.33, 14.26] (rough), so the log branch ran, and the smooth run crossed `y*_lam` | bit-identical | met |

**S3s, BDF2: the miss and its diagnosis.** The registered test asserted every field and stopped at `ux`; all three orders were
printed (table). It is now the ignored `s3s_kepsilon_spalding_bdf2_time_order_known_miss`, with the limit intact. By the reading
fixed before the run, the kink of the standard wall eddy viscosity is not shown to be the cause of the ADR 0210 miss, and no other
variant is tried here. The numbers say more than that: at the registered levels the orders with the Spalding law are within 0.1
of those with the standard law (`ux` 0.8031 against 0.7975, k 1.4814 against 1.4910, epsilon 1.4287 against 1.5303) and the errors
within 10 percent (`ux` `e(80,160)` 1.66e-5 against 1.52e-5), although the Spalding law changes the wall viscosity by 13 percent
(T3N) and is smooth where the wall cells cross `y*_lam`. A report-only diagnosis (`s3s_kepsilon_spalding_bdf2_finer_levels_diagnosis`,
ignored, run once) of the same runs at finer levels gives `p` 1.5755 / 1.7565 / 1.7427 (`ux` / k / epsilon) from T/160, T/320,
T/640 and 1.8089 / 1.8806 / 1.8753 from T/320, T/640, T/1280, where ADR 0210 recorded 1.5598 / 1.7638 / 1.7816 and 1.8119 / 1.8796 /
1.8874 for the standard law; the outer iterations converged throughout (`nut_change` at most 1.2e-15). So the order deficit is a
pre-asymptotic property of the k-epsilon start-up from k 1, epsilon 3.6 on this mesh that does not depend on the law of the wall
eddy viscosity, and with both laws the observed order approaches 2 with refinement. What makes the k-epsilon start-up slower to
reach the asymptotic range than the SST and SA start-ups is not found by these runs (question 6). ADR 0210's known miss and its
limit are unchanged.

**Mutations** (scratch, `f300-mut/apply.py`, each applied alone to 918aaaa4's tree, the named test run once in release, the file
reverted; the script asserts that its pattern matched exactly once):

| Id | Test | Measured | Result |
|---|---|---|---|
| MA1 | T1 | `k_epsilon`: `supports_transient` false but accepted on `pimple` (stops at the first key) | fails |
| MA2 | S1c | CH-K on `piso` accepted with no diagnostic (`turbulence.model: got []`) | fails |
| MB1 | S3' `bdf2` | `p` 0.8007 / 0.8503 / 0.8038 (`ux` / k / omega), the numbers of S3'N; largest `nut_change` of a step 0.52 to 0.54 | fails (below the band) |
| MB2 | S3' `bdf2` | `p` 1.0334 / 1.0393 / 1.0426; the outer iterations converge (`nut_change` 7.6e-15 to 3.4e-14) but the momentum keeps the face viscosity of the start of the step | fails (below the band) |
| MC1 | W1 | (iii) `D-` 1.9e-14, `D+` 6.837783e-2, ratio 1.000 (the standard law's kink); (i) and (ii) met | fails |
| MC2 | T3 | 8 of 8 wall faces at `nu` (1.000000 `nu`), off the Spalding bits | fails |
| MC3 | W2 | `y* = 5`, Spalding: `abs(k' - G_w k0 / eps_w) / k'` 2.229e-2 (the production of the standard law) | fails |

**Predictions.** PR1 met. PR2 met (S3' gives P2's numbers to every printed digit; S3'N 0.80). PR3 met (W1 (iii) 5.8e-4, W2 at most
4.4e-15, T3 exact). PR4 met for S3s `euler` and S3sN, falsified for S3s `bdf2`.

**Before the gate**, in release: the related `rcfd-api` tests (`baseline_hashes`, `case_hashes_w48u`, `des_tree`, `exp_rot_pc`,
`f286_default_bits`, `f286_transient_sst`, `f294_default_bits`, `f294_transient_keps_sa`, `f295_default_bits`,
`f295_seam_diffusivity`, `f296_default_bits`, `f296_seam_diffusivity`, `mixing_length_tree`, `physics_schema`, `regions`,
`rsm_tree`, `runctl`, `sgs_tree`, `sst_schema`, `transient_sources`, `turb_models`, `w48z_followups`, `wall_options`,
`f300_default_bits`) on 6e4e650a: 123 passed, 0 failed, 20 ignored (V0a: the hash files unedited, all pass); every `rcfd-turb`
test on 6e4e650a: 124 passed, 0 failed, 104 ignored. `cargo clippy -p rcfd-turb -p rcfd-api --all-targets -- -D warnings` found
one lint in the new test file (`excessive_precision` on the CH viscosity constant); 918aaaa4 writes it with clippy's digits, which
parse to the same `f64` (checked), and clippy and `cargo fmt --check` of both packages are then clean. After d26ce83d the only
changes to code or tests are the `#[ignore]` of the S3s miss with the report-only diagnosis (6e4e650a) and that literal (918aaaa4).

**Questions revised by the results** (each with a recommendation). Questions 1, 2 and 4 stand.

3. **The Spalding wall eddy viscosity** (revised). It did not change the k-epsilon order. Options: (a) keep it as the library
   opt-in it is (W1, W2, T3 test it; S3s and its diagnosis need it to be reproduced); (b) remove it, since it did not help; (c) a
   case key with its own registration. Recommendation: (a); (c) only if a case needs a k-epsilon wall function without a switch.
5. **The status of F-300.** By the decision rule F-300 is partial: S3s `bdf2` is a registered check and a known miss. The
   roadmap's (c) asks for the test of the suspected cause, which was made, and its reading was registered for both outcomes.
   Options: (a) partial, as the rule gives; (b) done by the maintainer's acceptance, with the miss recorded as the answer of (c).
   Recommendation: (a) by the rule; (b) is the maintainer's call.
6. **The k-epsilon BDF2 order on the channel start-up** (both known misses: ADR 0210 S3 and S3s). Options: (a) keep both, with the
   diagnosis (pre-asymptotic at the registered levels, the order rising to 1.8 to 1.9 at T/320 to T/1280 with either law);
   (b) a follow-up item that registers a k-epsilon time-order check from a resolved state (ADR 0199 question 6 (b): the steady
   state of R1 with the drive changed) or at the finer levels; (c) a follow-up that looks for what makes the k-epsilon start-up
   slower than SST and SA (for example the algebraic wall-cell epsilon, fixed in every sweep, against the transported one).
   Recommendation: (a) now; (b) only if a case-tree order record for transient k-epsilon is wanted, since ADR 0210 K1 and K2 show
   the time derivative of the k and epsilon equations exactly.

### Status by the decision rule

Every registered check of ADR 0225 is met except S3s `bdf2`, an ignored known miss with its numbers and a diagnosis; every negative
control fired and every mutation made its check fail (above). F-300 is **partial** (`[~]`; question 5). Maintainer acceptance is
still required.
