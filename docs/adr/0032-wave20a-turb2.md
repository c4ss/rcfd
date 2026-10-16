# ADR 0032: Wave 20A, full viscous stress, SA-based DES/DDES, Reynolds stress model

Status: accepted (checks registered before implementation). Roadmap: F-045 (transposed-term gap), F-207 (SA-based DES/DDES only),
F-046 (Reynolds stress model only; no transition model). All three stay `partial`; the gaps are named at the end.
Process template: ADR 0016 and ADR 0026. Clean-room: published-literature algorithms only. No limit below is changed after the
first run; a miss becomes an ignored `*_known_miss` test with the registered assertion intact and a diagnosis in an addendum.

## Decisions

### D1 Full viscous stress (option, default unchanged)
* New setting `solver.viscous_stress` in {`laplacian` (default), `full`}; library field `PimpleSettings::full_viscous_stress: bool`
  (default `false`). Only the transient algorithms (piso, pimple) support `full`; with simple/simplec/coupled the value `full` is a
  located error on `solver.viscous_stress`.
* `full` adds, in the momentum predictor, the explicit (deferred-correction) source `S_i = (1/V) sum_f nu_f (grad u)_f[i][j] S_f[j]`
  (that is `div(nu_eff (grad u)^T)`, momentum component `i`: `d_j (nu d_i u_j)`), on the velocity of the last outer iterate. `(grad u)_f` is
  the linear interpolation of the cell Green-Gauss gradients; `nu_f` is the effective face viscosity of the solver (`nu_face`, else
  the uniform `nu`), i.e. the FULL `nu_eff`, not only the eddy part. For a constant `nu` the continuum term is `nu grad(div u) = 0`;
  the discrete term is then a small O(h^2) number, measured by A2. Boundary faces contribute nothing (documented limitation: the
  term is omitted on boundary faces, tests restrict to periodic boxes and interior cells). The source is added to the matrix
  right-hand side before relaxation (so the H operator and Rhie-Chow flux contain it).
* With `full_viscous_stress = false` no new arithmetic is executed: baseline hashes stay bit-identical.
* Library function `rcfd_fv::pimple::transposed_stress_source(geom, u, nu_face) -> Vec<Vec3>` is public for the operator check.

### D2 SA-based DES / DDES
Sources (verification status in the table at the end): Spalart et al. 1997 (DES), Spalart et al. 2006 (DDES).
* SA destruction and the `r`, `S~` terms use the length `d~` instead of the wall distance `d`:
  DES97: `d~ = min(d, C_DES Delta)`; DDES: `d~ = d - f_d max(0, d - C_DES Delta)`,
  `f_d = 1 - tanh[(8 r_d)^3]`, `r_d = (nu_t + nu)/(max(sqrt(U_ij U_ij), 1e-10) kappa^2 d^2)` with `U_ij = d u_i/d x_j` and `nu_t` the SA
  eddy viscosity (`nuT fv1`, the previous-level value); `C_DES = 0.65`. `kappa` is the SA `kappa` coefficient (0.41).
* `Delta`: `max_extent` (default, DES97 definition `max(dx, dy, dz)`: the largest distance between two face centres of the cell,
  empty-direction faces excluded) or `cbrt_volume`.
* SA gets a time derivative (implicit Euler, per step) used only when the model is stepped by the transient algorithm:
  trait method `begin_step(dt)` (default no-op) called once per step before `update`. `nuT` is advanced once per step (one sweep on the
  flux and velocity of the old time level, like the algebraic models of ADR 0026). SA with `des = "off"` stays steady-only
  (unchanged behaviour); `des != "off"` is transient-only.
* Settings: `turbulence.sa.des` in {`off` (default), `des`, `ddes`}, `turbulence.sa.des_delta` in {`max_extent`, `cbrt_volume`},
  `turbulence.sa.c_des` (default 0.65, range 0.1 to 2). Options pass through the new trait method
  `set_option(name, value) -> Result<(), String>` (default: unknown option error), so `ModelContext` is not changed.

### D3 Reynolds stress model (`turbulence.model = "rsm"`, registry key `rsm`)
* Six transport equations for `R_ij` (symmetric) and one for `epsilon`:
  `D R_ij/Dt = P_ij + Phi_ij - (2/3) eps delta_ij + d_k[(nu delta_kl + C_s (k/eps) R_kl) d_l R_ij]`,
  `P_ij = -R_ik d_k U_j - R_jk d_k U_i`, `D eps/Dt = Ce1 (eps/k) P - Ce2 eps^2/k + d_k[(nu delta_kl + C_eps (k/eps) R_kl) d_l eps]`,
  `k = R_ii/2`, `P = P_kk/2` (generalised gradient diffusion).
* Pressure-strain, option `turbulence.rsm.pressure_strain`:
  `ssg` (default; Speziale, Sarkar, Gatski), in the anisotropy form `a_ij = R_ij/k - 2/3 delta_ij`:
  `Phi_ij = -(C1 eps + C1s P) a_ij + C2 eps (a_ik a_kj - a_kl a_kl delta_ij/3) + (C3 - C3s sqrt(a_kl a_kl)) k S*_ij
  + C4 k (a_ik S_jk + a_jk S_ik - 2/3 a_kl S_kl delta_ij) + C5 k (a_ik W_jk + a_jk W_ik)`
  with `C1 = 1.7`, `C1s = 0.9`, `C2 = 1.05`, `C3 = 0.8`, `C3s = 0.65`, `C4 = 0.625`, `C5 = 0.2` (the "outer" set of the published
  SSG/LRR-omega model, converted from the original `b_ij` form), `S* = S - tr(S)/3 delta` (the TMR form has `1/2 C1s P_kk = C1s P`).
  `lrr_ip` (isotropisation of production): `Phi_ij = -C1 (eps/k)(R_ij - 2/3 k delta_ij) - C2 (P_ij - 2/3 P delta_ij)`, `C1 = 1.8`, `C2 = 0.6`.
  The wall-reflection terms (Gibson-Launder) are NOT implemented (named gap); `ssg` needs none for its high-Reynolds use.
* `Ce1 = 1.44`, `Ce2 = 1.83` (ssg) / `1.92` (lrr_ip), `C_s = 0.22`, `C_eps = 0.18`.
* Momentum coupling (new trait method `momentum_source(u) -> Option<Vec<Vec3>>`, default `None`): the solver sees the stress through
  `nu_face = nu + nu_t` (`nu_t = 0.09 k^2/eps`, implicit, as stabiliser) plus the explicit remainder
  `a_i = -d_j R_ij - d_j(nu_t d_j u_i)` (cell values; face-interpolated balanced-force treatment through the solver body force, the
  Rhie-Chow-consistent form). The static pressure is the solver pressure (`2/3 k delta` is part of `R_ij` in the source).
* Walls: NO wall treatment is delivered for the RSM. The settings tree refuses a case with wall patches (located error). Consequently
  the RSM is validated in homogeneous and periodic settings; the registered channel check C2 is conditional (see below).
* Transient algorithms only (piso, pimple); steady use is a located error.
* Plug-in switching: `rsm` is a registry entry, so `turbulence.model = "plugin"`, `turbulence.plugin = "rsm"` builds the same model.
* Realizability: no clipping is applied in the model (clipping would hide the check); `epsilon` and `k` are limited from below at `1e-30`
  only to avoid division by zero. Start values: `R_ij = 2/3 k0 delta_ij`, `eps0` from `turbulence.k_initial` / `epsilon_initial` or the
  existing viscosity-ratio default.

## Registered checks (limits fixed here)

### A. Viscous stress option (files `rcfd-fv/tests/viscous_stress.rs`, `rcfd-turb/tests/viscous_stress_full.rs`, `rcfd-api/tests/viscous_stress_tree.rs`)
* A1 operator exactness: box_2d 14x12 (non-periodic, `L = 1.4 x 1.2`), `u = G x` (G = [[0.3,-0.7,0],[0.5,0.2,0],[0,0,0]]), exact Dirichlet
  boundary values, `nu(x) = 1 + 0.4 x + 0.25 y` evaluated at the face centres. For cells with all neighbours interior (not touching the boundary,
  and not adjacent to such a cell): `S_i = G_ji d_j nu`, relative error (to `max |S|`) `<= 1e-12`.
* A2 constant viscosity: laminar Taylor-Green, `n = 16`, `nu = 0.05`, 10 steps `dt = 0.01`: `|K_full - K_lap| <= 0.02 (K_0 - K_lap)` where `K` is
  the mean kinetic energy after the 10 steps. (The full term is analytically zero.)
* A3 (the ORIGINAL TG1 assertion with the option on, Smagorinsky): extra-dissipation ratio `meas/(1.44101 (Cs Delta)^2 decay)` within `1 +- 0.15`.
* A4 (WALE analogue, closed form `0.070217 (Cw Delta)^2 decay` of ADR 0026 A1): ratio within `1 +- 0.15`.
* A5 (the ORIGINAL TG3 ordering with the option on): `K_W(4) > K_S(4)` at step 400, and `K_S(4)/K_L(4) <= 0.94` (ADR 0026 predicted 0.920 for the full
  stress), `K_W(4)/K_L(4) <= 0.995`.
* A6 energy sign: no kinetic-energy increase over 400 steps with `full` for both models (count 0).
* A7 default unchanged: `PimpleSettings` default has `full_viscous_stress = false`; a Smagorinsky run with the field explicitly `false`
  equals the default run bit for bit (10 steps); `baseline_hashes` bit-identical.
* A8 tree: `solver.viscous_stress` default `laplacian`; `full` with `solver.algorithm = "simple"` is a located error on `solver.viscous_stress`;
  an invalid value is refused by the schema with the located path; a pimple run through the tree with `full` differs from the default run by
  `> 1e-9` for a Smagorinsky model and `full`/`laplacian` laminar constant-viscosity runs agree to `<= 1e-3` of the velocity scale after 10 steps.
* If A3 or A4 or A5 pass unchanged, the corresponding original ignored test (`tg1_..._known_miss`, `tg3_..._known_miss`) stays ignored in its
  file; the passing assertions live in the new file under new names (the original text is not edited).

### B. SA-based DES / DDES (`rcfd-turb/tests/des.rs`, `rcfd-api/tests/des_tree.rs`)
* B1 DES length scale, prescribed values, `<= 1e-12`: `(d, Delta, C) = (0.3, 0.2, 0.65) -> 0.13`; `(0.1, 0.2, 0.65) -> 0.1`; `(0.13, 0.2, 0.65) -> 0.13` (tie).
* B2 DDES shielding, `nu = 1e-5`, `|grad u| = 100`, `d = 0.1`, `kappa = 0.41`, `C_DES Delta = 0.065` (`Delta = 0.1`), hand values computed with double precision
  `math.tanh` outside the code: `nu_t = 1e-3`: `r_d = 0.006008328375966686`, `f_d = 0.9998889468350446`, `d~ = 0.06500388686077344`;
  `nu_t = 2e-2`: `r_d = 0.11903628792385483`, `f_d = 0.30189679720847473`, `d~ = 0.0894336120977034`; `nu_t = 0.1`: `r_d = 0.594943486020226`, `f_d = 0`, `d~ = 0.1`.
  Tolerance relative `1e-12`.
* B3 `Delta` definition: on a uniform box_2d mesh `dx = 0.2`, `dy = 0.1` (empty z): `max_extent` gives 0.2 in every cell (`<= 1e-12`), `cbrt_volume` gives
  `(dx dy dz)^(1/3)` for the 3-D mesh of the same kind (box_3d if available) to `1e-12`.
* B4 RANS limit: a transient SA channel run (walls y, periodic x, body-force driven, 30 steps) with `des` = `des` and = `ddes` and `C_DES Delta > d` everywhere
  (`C_DES = 1000`) equals the SA run with `des = "off"` in the same transient mode: `max |u|, max |nuTilda|` difference `<= 1e-12` (target: bit for bit).
* B5 LES limit fixed point: pure function check of the local source (`sa_local_source`): for uniform `Omega = 10`, `nu = 1e-14`, `d~ = 0.01` and `0.02`, the
  root `nuT*` of `cb1 S~ nuT = cw1 fw (nuT/d~)^2` satisfies `|source| <= 1e-10 cb1 Omega nuT*` and the ratio `nuT*(0.02)/nuT*(0.01)` is `4 +- 4e-3`.
* B6 time derivative: periodic-free closed-form check on a wall box: one transient SA step on a uniform field with `u = 0` equals the hand-evaluated
  implicit-Euler cell update `(1/dt + cw1 fw nuT_old/d~^2) nuT_new = nuT_old/dt + cb1 S~ nuT_old`, `relax_turb = 1`, convection and diffusion absent
  (uniform `nuT`, zero-gradient boundaries), relative `<= 1e-10` at every cell. (This measures the `ddt` term.)
* B7 tree: `turbulence.sa.des = "ddes"` through the tree with `piso`/`pimple` runs; with `simple` it is a located error on `turbulence.sa.des`; `des`
  options with `model != spalart_allmaras` are located errors; `c_des` out of range is a located error; `ddes` reduces to the same result as a
  plugin-built library model (twin, `<= 1e-12`); live switch SA(des) to `smagorinsky` keeps `u`, `p` bit for bit and changes `nu_t` next step.

### C. Reynolds stress model (`rcfd-turb/tests/rsm.rs`, `rcfd-api/tests/rsm_tree.rs`)
* C1 term invariants (prescribed random-but-fixed `R_ij` (positive definite) and `grad U`, relative `<= 1e-12` of the scale of the terms): `tr(P_ij) = 2P`;
  `tr(Phi_ij) = 0` for `ssg` and `lrr_ip`; `Phi_ij` symmetric; `Phi_ij = 0` for `R_ij` isotropic and zero shear. The summed trace of the model source terms equals
  `2(P - eps)` (the `k` equation).
* C2 homogeneous shear ODE (library function `homogeneous_rates`, RK4 `dt = 1e-3 k0/eps0`, `S k0/eps0 = 5`, `U_1,2 = S`, isotropic start `R = 2/3 k0 delta`, 40 eddy turnovers):
  (a) realizability: smallest eigenvalue of `R_ij` stays `>= -1e-12 k` and `k > 0` throughout, for both pressure-strain variants;
  (b) `k` budget: `d k/dt = P - eps` from the integrated `R` to `1e-6` relative along the run; (c) the `R_12` sign is negative (`<0`) from the first step on; (d) energy
  stays finite (`k(t_end) < 1e3 k0`, i.e. the model does not blow up). No literature value of the equilibrium is claimed.
* C3 isotropic decay: RK4 of the model ODE with zero shear equals the closed form `k = k0 (1 + (Ce2 - 1) eps0 t/k0)^(-1/(Ce2 - 1))` to `1e-8` relative, and `R_ij` stays isotropic
  (`a_ij <= 1e-12`). The power-law exponent is the model's own `1/(Ce2 - 1)` (1.2048 for ssg, 1.0870 for lrr_ip); the experimental exponent of grid turbulence is NOT claimed
  (no verified reference in the tree; reported as unverified).
* C4 PDE against ODE: periodic box_2d `n = 8`, `u = 0`, anisotropic uniform start (`R = diag(1.2, 0.5, 0.3) k0`), the transport model stepped by `begin_step(dt)` +
  `update` (implicit Euler): `R_ij`, `eps` agree with the implicit-Euler step of the homogeneous ODE system (same linearisation, hand-implemented in the test) to `<= 1e-8` relative after 20 steps
  (spatially uniform fields: diffusion and convection vanish).
* C5 momentum source: `u = 0`, `R_ij` linear in `x` on a uniform non-periodic box (cells away from the boundary): `momentum_source = -d_j R_ij` exactly (relative `<= 1e-12`).
* C6 through the tree: `turbulence.model = "rsm"` runs in a periodic box with pimple (anisotropic start via `turbulence.rsm.*` initial settings are NOT provided; start isotropic): `k` decays per
  C3 (tree vs closed form `<= 1e-6` relative after 20 steps, `u = 0`); `plugin = "rsm"` equals `model = "rsm"` bit for bit; located errors for: `rsm` with a steady algorithm, `rsm` with a wall patch,
  `turbulence.rsm.*` options with another model, a coefficient out of range (`turbulence.coefficients.rsm.<name>`), `rsm` with energy enabled.
* C7 stress budget in a fully developed channel: CONDITIONAL on a wall treatment, which this ADR does not include. If no wall treatment is delivered by the end of the wave
  C7 is reported NOT MET / not delivered. (Registered for the record so the gap is visible.)
* C8 two-way coupling smoke: a pimple run of a periodic box with a prescribed non-trivial `u` (Taylor-Green) and the RSM completes 100 steps with finite fields, `k > 0`, no kinetic-energy increase
  (the model only dissipates), `R_ij` realizable (min eigenvalue `>= -1e-6 k`).

### H. Hygiene
* H1 `baseline_hashes` bit-identical, not re-recorded; existing tests unchanged (no struct-literal changes of `ModelContext`).
* H2 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast` on a `git archive` copy of the final commit,
  `CARGO_TARGET_DIR=<target dir>`.
* H3 unverifiable references marked in `docs/reference-data.md`; every deviation disclosed in Results.
* H4 statuses stay `partial` (F-045, F-207, F-046).

## Reference verification status (web search, this wave)
| Item | Status |
| --- | --- |
| SSG pressure-strain coefficients (outer set), GGDH coefficient 0.22, `P_ij`, `a_ij` conventions | verified against the NASA Turbulence Modeling Resource SSG/LRR-omega page (https://tmbwg.github.io/turbmodels/rsm-ssglrr.html); the original 1991 SSG paper not read |
| LRR-IP `C1 = 1.8` | consistent with the LRR coefficient of the same TMR page; `C2 = 0.6` NOT verified (memory) |
| `Ce1 = 1.44`, `Ce2 = 1.83` / `1.92`, `C_eps = 0.18` | NOT verified (memory) |
| DES `min(d, C_DES Delta)`, `Delta = max(dx,dy,dz)` | verified in secondary sources (https://wmles.umd.edu/hybrid-les-rans-models/des/); original 1997 paper not read |
| `C_DES = 0.65` | secondary sources only (arXiv 2512.18609 summary, public encyclopedia entry); NOT verified against the primary paper |
| DDES `f_d = 1 - tanh[(8 r_d)^3]`, `r_d` with `nu + nu_t` over `kappa^2 d^2 |grad u|`, `d~ = d - f_d max(0, d - C Delta)` | verified in secondary sources (https://wmles.umd.edu/hybrid-les-rans-models/des/, arXiv 1606.05114); 2006 primary paper not read |
| Experimental decay exponent of grid turbulence | no verified reference; no claim made |

## Not in this wave (planned gaps)
Transition model (F-046), RSM wall treatment and wall reflection, omega-based RSM, steady RSM, RSM with energy, IDDES, SAS, stress-blended hybrid, dynamic
Smagorinsky, Vreman, sigma, WMLES, synthetic inflow, embedded LES zones, decaying isotropic turbulence spectrum, channel LES.
