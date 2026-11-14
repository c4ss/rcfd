# ADR 0485: F-410 registration: linear viscoelasticity, a Prony series with a recursive update of the internal stresses, the complex modulus and the time-temperature shift, on the Newton driver and the nonlinear transient integrator

Status: **REGISTERED 2026-11-14, written before any code of F-410 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-410 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f410-scratch/probe.py`, log `probe.log`; no rcfd code; disclosed, its numbers quoted below). The bit record B0 of the
default paths is a test file written and run on the base after this commit and before any library code (D8). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Registration of the stub [ADR 0313](0313-linear-viscoelasticity-prony-series.md) (work package SA-3 of the plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-410 (deps F-408, F-397, F-398). Number 0485 given by
the lead. Branch `f410-work` from 2e0e3213 (which contains F-422, ADR 0475, and F-423, ADR 0479), worktree `w49i-work/f410`,
scratch `w49i-work/f410-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except by the records-and-copies scheme of ADR 0291 (`sa_scaffold.rs`, and
`sa_j2_default_bits.rs`, whose last digest records the refusal of the pending variant this item lands, D9); clean-room.

## What F-410 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Relaxation of one element after a strain step: sigma(t) = eps0 (E_inf + sum E_i exp(-t/tau_i)) within 1e-10 at every step end for three branches. Creep of the standard solid under constant stress: eps(t) = sigma0 (1/E_inf - (1/E_inf - 1/E_0) exp(-t/tau_c)), tau_c = tau E_0/E_inf, within 0.1 percent at 20 steps per tau with observed order >= 1.9. The complex modulus function returns E' = E_inf + sum E_i w^2 tau_i^2/(1 + w^2 tau_i^2) and E'' = sum E_i w tau_i/(1 + w^2 tau_i^2) within 1e-13, and a strain cycled in time dissipates pi eps0^2 E'' per cycle and volume within 1 percent at 200 steps per cycle. Shift: the relaxation curve at a temperature with shift factor a_T equals the reference curve at t/a_T within 1e-10. Free vibration of a bar of the standard solid with a loss factor near 0.05 at its first frequency: logarithmic decrement pi E''/E' within 3 percent. A material without branches gives the bits of the elastic one; the internal stresses are saved and restored bit for bit with the integrator state; a non-positive relaxation time or modulus is refused with the key.

The brief adds: (b1) reuse the history path of F-422 (`nonlinear.rs`, `element_trial_state`, the committed and trial states of
the attempt) and the transient drivers of F-398 and F-423 (`nl_transient.rs`): no new history store, no new Newton loop; (b2)
look at how the harmonic path takes a loss factor (F-399, F-412) so that the complex modulus can feed it, and wire it only if
small; (b3) models without the new material keep their bits, a default-bits test records digests at 2e0e3213 before any
library change; (b4) a case-file key is the maintainer's call (recommended, not added); (b5) probe a quantity before a
`1e-12`-class limit (the `1e-10` relaxation over many steps was probed: arithmetic 7); (b6) the records-and-copies scheme of ADR
0291 for the retired pending rows of `sa_scaffold.rs`.

Clauses: (a0) every limit registered here; (a1) relaxation of one element after a strain step, three branches, `1e-10` at every
step end; (a2) creep of the standard solid under constant stress, 0.1 percent at 20 steps per `tau`, observed order `>= 1.9`;
(a3) the complex modulus function, `1e-13`; (a4) the dissipation per cycle of a cycled strain, 1 percent at 200 steps per
cycle; (a5) the shifted relaxation curve equals the reference curve at `t/a_T`, `1e-10`; (a6) the logarithmic decrement of the
free vibration of a bar of the standard solid with a loss factor near 0.05, `pi E''/E'` within 3 percent; (a7) a material
without branches gives the bits of the elastic one; (a8) the internal stresses saved and restored bit for bit with the
integrator state; (a9) a non-positive relaxation time or modulus refused with the key.

Readings, registered:

- **Moduli of the acceptance.** The Prony terms of the data model (ADR 0440 D2) carry relative weights `g_i` (shear) and `k_i`
  (bulk) of the instantaneous base and a relaxation time. With equal weights `g_i = k_i = w_i` the whole elasticity matrix
  relaxes in proportion, `D(t) = D_0 (w_inf + sum w_i exp(-t/tau_i))`, `w_inf = 1 - sum w_i`, so the uniaxial relaxation
  modulus is `E(t) = E_inf + sum E_i exp(-t/tau_i)` with `E_i = w_i E_0`, `E_inf = w_inf E_0` (the form of the acceptance) and
  the Poisson ratio is constant (arithmetic 1). The clauses (a1), (a2), (a4), (a5), (a6) use equal weights; the general
  shear/bulk split is checked separately (R2).
- **(a1)** R1: one Hex8 element (cube of side 1, symmetry planes `x = 0`, `y = 0`, `z = 0`), the strain step `u_x = eps0 = 1e-3`
  on `x = 1` (the lateral faces free), three branches `w = (0.3, 0.2, 0.1)`, `tau = (0.01, 0.1, 1) s`, `E_0 = 2e9 Pa`, `nu =
  0.35`. The step is an increment of zero length (`dt = 0`, D2), then 100 increments of `dt = 0.02 s` at constant strain, each
  by `nonlinear::solve_from_at` (D4) from the committed states. On the total Lagrangian driver the law acts on the
  Green-Lagrange strain `E` and the second Piola-Kirchhoff stress `S` (as ADR 0475 arithmetic 6), so the measure is in the
  driver's own measures: at every step end and every point `|S_xx - E_xx E(t)| / (E_0 E_xx)`, `E(t)` the closed form, `E_xx`
  the computed strain of the point. The Newton options of R1: residual tolerance `1e-13`, energy tolerance `1` (the residual
  criterion alone: an increment at constant strain starts at a residual of rounding size and its first correction has an
  energy of rounding size, arithmetic 6). Companion R1m: the same law at the material point with the strain `eps0 (1, -nu,
  -nu, 0, 0, 0)` (uniaxial stress for equal weights), `|sigma_xx - eps0 E(t)| / (E_0 eps0)`.
- **(a2)** C1: one Hex8 element as R1 under a dead axial load `sigma0 A` on `x = 1` (four nodal forces of `sigma0/4`), the
  standard solid `w = 0.5`, `tau = 1 s` (`E_inf = E_0/2`, `tau_c = 2 s`), `sigma0 = 1e-8 E_inf` (a strain of `1e-8`, so that the
  geometric terms of the total Lagrangian driver are of relative size `1e-8`, arithmetic 5), the load applied in an increment of
  zero length, then increments of `dt = tau/20` to `t = 5 tau_c` (200 increments); the strain `eps = u_x(x = 1)`. Measure
  `max over the step ends |eps - eps_closed| / eps_closed`, limit `1e-3`. C2: the same at `dt = tau/40` (400 increments); the
  observed order `log2(e_20 / e_40) >= 1.9`. Driver defaults (tolerances `1e-10`). Companions C1m, C2m: the same at the
  material point under stress control (the strain `eps (1, -nu, -nu, 0, 0, 0)`, `eps` from `sigma_xx(eps) = sigma0`, linear in
  `eps`, solved with the tangent).
- **(a3)** M1: `mat_visco::complex_moduli` of the three-branch material of R1 at 61 frequencies `omega = 10^(-3 + k/10) /
  tau_2`, `k = 0 .. 60`, against the acceptance's formulas evaluated in the test with `E_i = w_i E_0`, `E_inf = w_inf E_0`:
  `max(|E' - E'_f|, |E'' - E''_f|) / |E'_f + i E''_f|`, limit `1e-13`. Companion M2: the shear and bulk moduli of the material
  with unequal weights of the scaffold (`(0.3, 0.1, 1e-3)`, `(0.2, 0, 0.1)`, `(0.1, 0.05, 10)`) against `G_0 (g_inf + sum g_i
  x^2/(1 + x^2))`, `G_0 sum g_i x/(1 + x^2)` and the same for `K`, same measure and limit.
- **(a4)** D1: at the material point, the three-branch material of R1, `eps(t) = eps0 sin(omega t) (1, -nu, -nu, 0, 0, 0)`,
  `eps0 = 1e-4`, `omega = 10 rad/s` (`= 1/tau_2`), 200 steps per cycle, 10 cycles from the virgin state; the work of the tenth
  cycle by the trapezoidal sum `sum (sigma_n + sigma_{n+1})/2 . (eps_{n+1} - eps_n)` (tensor inner product, engineering shear);
  measure `|W - pi eps0^2 E''(omega)| / (pi eps0^2 E''(omega))`, limit `1e-2`. Companion D2: one Hex8 element as R1 with the
  prescribed `u_x(x = 1, t) = eps0 sin(omega t)`, the work of the tenth cycle `sum (R_n + R_{n+1})/2 . (u_{n+1} - u_n)` over the
  four nodes of `x = 1` per unit volume, same limit.
- **(a5)** S1: the shift of WLF with `c1 = 17.44`, `c2 = 51.6 K`, `T_ref = 373 K` (inputs; arithmetic 4), the element of R1 at
  `T = 383 K` (`a_T = 10^(-174.4/61.6) = 1.475133e-3`, arithmetic 4) given in the point context of every increment, the
  strain step then 100 increments of `dt = 0.02 a_T`; measure at every step end and point `|S_xx - E_xx E(t/a_T)| / (E_0
  E_xx)`, limit `1e-10` (Newton options of R1). Companion S1m at the material point with the WLF shift and with an Arrhenius
  shift (`E_a = 1e5 J/mol`, `T_ref = 373 K`, `T = 393 K`), and 200 increments of uneven length `dt_k = 0.013 a_T (1 + 0.5 sin
  k)`, same measure and limit. Companion S2: `mat_visco::shift_factor` against the formulas evaluated in the test at `T - T_ref
  = -20, -5, 0, 10, 40 K`, relative, limit `1e-13`.
- **(a6)** V1: a bar of 20 Hex8 elements, `L = 1 m`, section `0.05 x 0.05 m`, `nu = 0`, `rho = 1200 kg/m^3`, `E_0 = 2e9 Pa`,
  `u = 0` at `x = 0`, `u_y = u_z = 0` at every node (axial motion only: a one-dimensional rod, arithmetic 3), the standard solid
  `w = 2/21` with `tau = 1/omega_1` and `omega_1 = sqrt(lambda_1 E'(omega_1)/E_0) = sqrt((20/21) lambda_1)`, `lambda_1` the first
  eigenvalue of the elastic base model (`modal::modal_analysis` with `E_0`): the loss factor at the first frequency is then
  `eta = E''/E' = 0.05` exactly (arithmetic 3). `nl_transient::Integrator`, average acceleration, `dt = T_1/200`, `T_1 = 2
  pi/omega_1`, start from `u = 0` with `v_0 = A phi_1` (`phi_1` the mass-normalised mode, `A` such that the largest strain is
  about `1e-6`), 8 periods (1600 steps). The modal coordinate `q_n = phi_1^T M u_n` at every step end; its maxima located by the
  parabola through the three samples around each discrete maximum; `delta = ln(p_2/p_7)/5` (the first maximum carries the start
  transient, arithmetic 3). Measure `|delta/(pi eta) - 1|`, limit `3e-2` (acceptance). Companion V2: `delta` against the
  decrement `2 pi |Re s|/Im s` of the root `s` of `s^2 + lambda_1 (w_inf + w s tau/(1 + s tau)) = 0` near `i sqrt(lambda_1)`
  (Newton in complex arithmetic in the test), relative, limit `5e-3`. Set-up check: `omega_1 tau = 1` within `1e-12`.
- **(a7)** B1: `Material::viscoelastic(base, vec![], None)` (no branch, D1) against the base: `state_size() = 0`; `Material::stress`
  of 20 random strains (seed `0xF410`), bits; `nonlinear::solve_from_at` of the R1 element at the step and two increments, bits
  of `u`, the states and the internal force; `nl_transient` of the V1 bar for 100 steps, bits of `u`, `v`, `a` and the internal
  force at every step. Pass: every bit equal.
- **(a8)** H1: the V1 bar with the three-branch material of R1 (`tau` of R1 scaled so that `tau_2 = 1/omega_1`), 40 steps
  unsplit against 17 steps, `to_bytes`, `from_bytes`, `restore` into a new `Integrator` of the same model, 23 steps. Pass: the
  bits of `u`, `v`, `a`, the internal force and every state value equal at the end; set-up check: the states non-zero, the
  bytes equal at the split (save, load, save gives the same bytes).
- **(a9)** RF1: refused with the key: `PronyTerm::new` with `tau = 0`, `-1`, NaN (key `"tau"`, existing); a term with both
  weights 0 (a branch of zero modulus, key `"shear"`, new); a weight `-0.1` (keys `"shear"`, `"bulk"`, existing); the base with
  `E = 0`, `-1` (key `"e"`, existing); `Material::viscoelastic` with shear weights summing to 1 (zero long-term shear modulus,
  key `"shear"`, new) and bulk weights summing to 1 (key `"bulk"`, new). Arithmetic 8 says why the two new ones are readings of
  "a non-positive modulus".

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0313 (stub), ADR 0310 (sections 2.3, 2.4 item 5, the SA-3 entry), ADR 0440 (D2 the
  variants, D3 the state layout, the M2 and M3 rows), ADR 0475 (whole: the form of a registration, D3 the history in the
  driver), ADR 0479 (D1 to D4 by its code).
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `nl_transient.rs` (whole), `material.rs` (Prony term, shift, point context,
  `stress`), `model.rs` (the `Material` enum, `viscoelastic`, `pending`, `check_elements`), `mat_visco.rs`, `mat_j2.rs`,
  `harmonic_direct.rs` (the `LossFactor` and `ModulusTable` types), `damped.rs` (its loss factors), `modal.rs` (signatures).
- Tests: `sa_scaffold.rs` (the pending variants, `m1_to_m4_after_f422`, `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423`),
  `sa_j2_default_bits.rs` (its last digest is the refusal of the pending `Viscoelastic`).

## Arithmetic done before the code (disclosed)

1. **The generalised Maxwell solid and its recursive update** (first principles; the hereditary form and the recursion as in
   R. L. Taylor, K. S. Pister and G. L. Goudreau, Int. J. Numer. Meth. Engng 2 (1970) 45, and J. C. Simo and T. J. R. Hughes,
   Computational Inelasticity, Springer, 1998, chapter 10, cited from memory, NOT re-read; derived here). Instantaneous
   isotropic base `G_0 = E_0/(2(1 + nu))`, `K_0 = E_0/(3(1 - 2 nu))`; `sigma_0(eps) = 2 G_0 dev eps + K_0 tr eps 1` (the base's
   elastic stress). Branch `i` carries the internal stress `h_i`, `h_i(t) = int_0^t exp(-(xi(t) - xi(s))/tau_i) d/ds P_i
   sigma_0(s) ds` with `P_i sigma_0 = g_i dev sigma_0 + k_i (tr sigma_0/3) 1` and the reduced time `xi` (arithmetic 4); the
   stress `sigma = (1 - sum g_i) dev sigma_0 + (1 - sum k_i)(tr sigma_0/3) 1 + sum h_i`. A strain step `eps0` at `t = 0` gives
   `h_i = P_i sigma_0(eps0) exp(-t/tau_i)` and `sigma = 2 G(t) dev eps0 + K(t) tr eps0 1` with `G(t) = G_0 (g_inf + sum g_i
   exp(-t/tau_i))`, `K(t)` alike: the Prony series. With `g_i = k_i = w_i`, `sigma = (w_inf + sum w_i exp(-t/tau_i))
   sigma_0(eps0)`: uniaxial stress for the strain `eps0 (1, -nu, -nu)` and `E(t) = E_inf + sum E_i exp(-t/tau_i)`.
   **Update for a strain linear in the step** (the exact integral): with `x_i = dxi/tau_i`, `e_i = exp(-x_i)`, `gamma_i = (1 -
   e_i)/x_i` (`= -expm1(-x_i)/x_i`; 1 at `x_i = 0`), `h_i^{n+1} = e_i h_i^n + gamma_i P_i (sigma_0^{n+1} - sigma_0^n)`
   (`int_0^dxi exp(-(dxi - s)/tau)/dxi ds = gamma tau/tau = gamma`). Exact for a strain held constant (the factor `e_i` per step:
   relaxation exact up to rounding, arithmetic 7), second order for a smooth strain history (arithmetic 7). The update needs
   `sigma_0^n`, i.e. the strain of the last committed step: the state of a point is `[eps^n (6), h_1 (6), ..., h_N (6)]`,
   `6 (N + 1)` values (`0` for `N = 0`: a material without branches has no history), not the `6 N` declared by ADR 0440 D3
   (deviation, D1).
   **Tangent**: `d sigma^{n+1}/d eps^{n+1} = 2 G_t I_dev + K_t 1 (x) 1`, `G_t = G_0 (1 - sum g_i (1 - gamma_i))`, `K_t = K_0 (1
   - sum k_i (1 - gamma_i))`, symmetric, positive for weights in `[0, 1]` summing below 1; in Voigt with engineering shear strain
   the shear diagonal is `G_t`. Exact (the update is linear in `eps^{n+1}`).
2. **The complex modulus** (first principles): `eps = Re(eps* e^{i omega t})` in the steady state gives `h_i = Re(P_i sigma_0(eps*)
   i omega tau_i/(1 + i omega tau_i) e^{i omega t})`, so `G*(omega) = G_0 (g_inf + sum g_i i x_i/(1 + i x_i))`, `x_i = omega a_T
   tau_i`: storage `G_0 (g_inf + sum g_i x_i^2/(1 + x_i^2))`, loss `G_0 sum g_i x_i/(1 + x_i^2)`; `K*` alike; the uniaxial
   `E* = 9 K* G*/(3 K* + G*)` (the correspondence of the elastic formula), which for equal weights is `E_0 (w_inf + sum w_i i
   x_i/(1 + i x_i))`: the acceptance's `E'` and `E''`. **Dissipation per cycle and volume**: `W = int_0^{2 pi/omega} sigma d eps =
   pi eps0^2 E''` for the uniaxial `eps0 sin(omega t)` (`int sin cos = 0`, `int cos^2 = pi/omega`). Probe (D1 set-up): `E' =
   1.203960e9`, `E'' = 2.792079e8 Pa` at `omega = 10`; the complex form and the real formulas agree to rounding (`0`).
3. **Free vibration of the rod and the decrement** (first principles). With equal weights and the small-strain limit the
   internal force is `K_0` applied to the hereditary integral of `u` with the scalar kernel `E(t)/E_0`, so the modes of `(K_0, M)`
   decouple exactly; the first one is `q'' + lambda_1 (r * dq)(t) = 0`, `r(t) = w_inf + w exp(-t/tau)`. The free motion
   `e^{st}` needs `s^2 + lambda_1 r^(s) = 0`, `r^(s) = w_inf + w s tau/(1 + s tau)`. With `s = i omega + eps`, to first order in
   the loss: `omega^2 = lambda_1 E'(omega)/E_0` and `Re s = -lambda_1 r''/(2 omega - lambda_1 dr'/d omega)`, so the logarithmic
   decrement `delta = 2 pi |Re s|/omega = pi eta / (1 - (omega/2)(dE'/d omega)/E')`, `eta = E''/E'`. The acceptance's `pi E''/E'`
   neglects the dispersion term `c = (omega/2)(dE'/d omega)/E'`; for the standard solid at the loss peak `omega tau = 1`, `omega
   dE'/d omega = E_0 w/2`, `c = (w/4)/(1 - w/2)`. Registered set-up `w = 2/21`: `E'(1/tau) = (20/21) E_0`, `E'' = E_0/21`, `eta =
   0.05` exactly, `c = 1/40`, `delta/(pi eta) - 1 = 2.56e-2` to first order: **the margin to the 3 percent limit is 0.4
   percent**, set by the physics of the standard solid, not by the numerics. The exact root (probe): `delta = 0.161046` against
   `pi eta = 0.157080`, `+2.525e-2`. The start from `u = 0`, `v_0 = A phi_1` with zero internal stresses also excites the real
   root of the relaxation; the first maximum carries it (probe: first decrement `0.16731`, the later ones `0.16101`), hence
   `delta` from the maxima 2 to 7. `nu = 0` with the lateral motion held makes the rod one-dimensional (`lambda = 0`); the
   decoupling holds for any `nu` with equal weights.
4. **Time-temperature shift** (thermorheologically simple material; M. L. Williams, R. F. Landel and J. D. Ferry, J. Am. Chem.
   Soc. 77 (1955) 3701, and J. D. Ferry, Viscoelastic Properties of Polymers, 3rd ed., Wiley, 1980, chapter 11, cited from
   memory, NOT re-read). Every relaxation time scales with `a_T`: `tau_i(T) = a_T tau_i`, equivalently the reduced time `dxi =
   dt/a_T` (at constant temperature `xi = t/a_T`, so the curve at `T` is the reference curve at `t/a_T`). WLF `log10 a_T = -c1 (T
   - T_ref)/(c2 + T - T_ref)` (refused for `c2 + T - T_ref <= 0`, key `"temperature"`), Arrhenius `ln a_T = (E_a/R)(1/T -
   1/T_ref)` with `R = 8.314462618 J/(mol K)` (CODATA 2018, the exact product of the defined Avogadro and Boltzmann constants;
   refused for `T <= 0` or `T_ref <= 0`, key `"temperature"`), as in the doc of `TimeTemperatureShift`. The constants `17.44`,
   `51.6 K` of S1 are the "universal" WLF values as commonly quoted from Williams, Landel and Ferry (from memory): inputs of the
   check, not reference data; `a_T(383 K; T_ref = 373 K) = 10^(-174.4/61.6) = 1.475133e-3` (probe). A temperature that changes
   between steps takes the step-end temperature (first order in the temperature history; Limits).
5. **Geometric terms of the element checks.** R1, S1: compared in the driver's measures (`S_xx` against `E_xx E(t)`), exact for
   the homogeneous uniaxial state of one Hex8 with symmetry planes: with equal weights `S = r(t) D_0 E` keeps `S_yy = S_zz = 0` at
   constant `E`, so the lateral displacement does not move after the step. C1, D2: the dead load and the reaction are nominal
   (first Piola-Kirchhoff) quantities and the closed forms small-strain: the relative difference is of the order of the strain,
   `1e-8` in C1 (against `1.2e-5` of the scheme, so the order of C2 is not disturbed) and `1e-4` in D2 (inside 1 percent).
6. **Convergence criterion at constant strain.** The driver stops at `k >= 1` with `r <= tol_r` and `e_last <= tol_e e_first`
   (ADR 0404). An increment of R1 or S1 at constant strain starts at a residual of rounding size; its first correction has an
   energy of rounding size and the next ones too, so `e_last/e_first` stays near 1 and the default `tol_e = 1e-10` would not
   be met: R1 and S1 use `tol_e = 1` (the residual alone, always true at `k = 1`) and `tol_r = 1e-13`. C1, C2, D2, V1 change the
   state every step and use the defaults.
7. **Probe** (`probe.py`, `probe.log`; the scalar law of arithmetic 1 with equal weights, exactly the uniaxial reduction):
   relaxation of R1m over 101 step ends `1.7e-16`; creep C1m `1.208e-5` at 20 steps per `tau`, `3.020e-6` at 40, order
   `1.9999` (10 to 20: `1.9995`); dissipation D1 tenth cycle `-2.5e-4` at 200 steps per cycle (`-9.9e-4` at 100; the first cycle
   `-1.2e-1`: the start); shift S1m with WLF `1.7e-16`, uneven steps `1.2e-16`; free vibration of the registered set-up at 100,
   200, 400 steps per period: `delta/(pi eta) - 1 = 2.425e-2, 2.502e-2, 2.521e-2`, against the exact root `2.525e-2` (V2:
   `2.2e-4` at 200 steps). The limits `1e-10` of R1, S1 are six decades above the material-point probe; the element adds the
   Newton residual (`1e-13`, arithmetic 6) and the rounding of the element sums (`1e-15`).
8. **"A non-positive relaxation time or modulus"**, three moduli: the instantaneous one (the base `E`, refused by
   `Material::isotropic_elastic`, key `"e"`), the modulus of a branch (`g_i G_0`, `k_i K_0`: a term with both weights 0 has none
   and is refused, key `"shear"`; a negative weight is refused by the existing `[0, 1]` range), and the long-term modulus
   (`(1 - sum g_i) G_0`, `(1 - sum k_i) K_0`: a sum of 1 makes it zero, refused with the key `"shear"` or `"bulk"`; the existing
   refusal covered only a sum above 1). The relaxation time: `PronyTerm::new` refuses it when not positive and finite (key
   `"tau"`, existing).

## Design

### D1. The material (`mat_visco.rs`)

`PENDING = None`; `FOLLOW_UP = "a follow-up of ADR 0485"` (the package named where the material is refused).
`state_size(terms) = if terms == 0 { 0 } else { 6 (terms + 1) }` (arithmetic 1; deviation from ADR 0440 D3's `6 terms`, whose
M3 row is retired by D9). `update(material, strain, state, point) -> Result<Update>` with `Update { stress, tangent, state }`:
the recursion of arithmetic 1 from the committed `state`, the reduced increment `dxi = dt/a_T` from `point.time_increment`
(`None`: `dt = 0`, the instantaneous response from the committed state) and `point.temperature` (needed when the material has a
shift: refused without one, key `"temperature"`, as the tabulated material). A material without branches returns `apply(&D_0,
strain)` and `D_0 = isotropic_elasticity(e, nu)` (the bits of its base, (a7)). An orthotropic base is refused
(`NotYetSupported`, location `"an orthotropic base"`, package `FOLLOW_UP`); a state of the wrong length or with a non-finite value
is refused (key `"state"`). Also: `shift_factor(shift, temperature)`, `complex_moduli(material, omega, temperature) ->
ComplexModuli { shear, bulk, young }` (`rcfd_linalg::complex::C64`, arithmetic 2; refuses `omega` negative or not finite, key
`"frequency"`), and `modulus_table(material, frequencies_hz, temperature) -> ModulusTable` (D7).

### D2. The constitutive call and the constructors (`material.rs`, `model.rs`)

`Material::stress` of a viscoelastic material returns `update`'s stress and tangent for the committed state at the point context
given (the state is not changed: the driver gets the new state from the evaluation, D3). `Material::elasticity_matrix` of it is
refused (`NotYetSupported`, location `"the element-constant elasticity matrix"`, package `mat_visco::FOLLOW_UP`), as J2.
`check_elements` refuses it where the history flag is off (every path but the nonlinear driver), naming `mat_visco::FOLLOW_UP`.
`Material::viscoelastic` accepts an empty list (a material without branches, (a7); ADR 0440 D2 refused it: deviation) and
refuses a sum of the shear or bulk weights of 1 or more (arithmetic 8); `PronyTerm::new` refuses both weights 0 (key `"shear"`).

### D3. History in the Newton driver (`nonlinear.rs`, no second loop)

The arm of `evaluate` that gives J2 its trial state (ADR 0475 D3) gains the viscoelastic material: `mat_visco::update` from the
committed state of the attempt at the point context of the attempt. `evaluate` and `force_and_tangent` take the point context;
`Problem` holds it. Every public function that existed keeps its signature and passes `PointContext::default()` (so every
existing caller evaluates as before: the context is read by the orthotropic and tabulated materials only through its axes and
temperature, which the default does not set, and by nothing else).

### D4. Time on the driver: `solve_from_at`

`pub fn solve_from_at(model, prescribed, load, start, states, options, contributions, point: &PointContext) -> Result<Equilibrium>`
is `solve_from` (ADR 0479 D2) at a point context (time, time increment, temperature for every point); `solve_from` calls it with
the default context. A quasi-static time history (R1, C1, D2, S1) is a loop of `solve_from_at` over the time steps, each from the
committed states and displacement of the last, written in the test (no new driver). `nonlinear::solve` and `solve_with` (the
static load-factor driver, no time) refuse a viscoelastic material with branches (`NotYetSupported`, location `"the nonlinear
static solve (no time)"`, package `FOLLOW_UP`): load steps without a time would silently give the instantaneous response.

### D5. Time in the nonlinear transient step (`nl_transient.rs`)

`Integrator::substep` calls `solve_from_at` with `PointContext::new().with_time(t_b, h)` (the end time and the length of the
sub-step) instead of `solve_from`. The context is read only by the viscoelastic material (D3), so every other model takes the
operations it took (B0). No temperature is given (a shifted material is refused by the transient path, key `"temperature"`;
Limits). The saved state already carries the element states (ADR 0479 D3): the internal stresses are saved with it (a8).

### D6. The modal decoupling for V1

No library code: the test takes `lambda_1`, `phi_1` from `modal::modal_analysis` of the base model (elastic, `E_0`), sets `tau`
from them (a6) and builds the viscoelastic model on the same mesh.

### D7. The harmonic path (b2)

`harmonic_direct::LossFactor::Table(ModulusTable)` takes the storage modulus and the loss factor against frequency in Hz (linear
between points) for an isotropic material, the stiffness scaled by `E'(f)/E` (ADR 0448). `mat_visco::modulus_table` gives that
table from the complex modulus at the frequencies given (`E'` and `E''/E'` of `young`), for a material with equal weights only
(otherwise the table, which scales the whole matrix, is not the material; refused, key `"terms"`). The direct harmonic solve is
then given the base material and that table. No harmonic solve is changed. Check M3: the table's values at its points equal
`complex_moduli` bit for bit. An end-to-end harmonic check and the material taken directly by the harmonic and damped-mode paths
are a follow-up (proposed in the addendum).

### D8. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_visco_default_bits.rs`, written after this commit and run on the unchanged base 2e0e3213 before any
library code; its digests (FNV-1a 64 over the bits) are written into the file then. Content: `nonlinear::solve` of an elastic
stretched Hex8 bar and of a J2 bar into the plastic range (every value of the result, as ADR 0475 D7); `solve_from` of the Hex8
bar with a zero contribution; `nl_transient` of an elastic Hex8 bar under a step load and of a J2 bar started by a velocity (30
steps each, `u`, `v`, `a`, the internal force and the bytes of the state); `internal_force` and `tangent` of a distorted Tet10
model; `Material::stress` of the isotropic material and of a J2 material from a non-zero state; `solve_static` and the
frequencies of `modal_analysis` of a Hex20 cantilever; the refusal messages of J2 on `solve_static`.

### D9. Records and copies

`sa_scaffold.rs`: `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423` (row `mat_visco`) and `m1_to_m4_after_f422`
(M1, M3 and M4 rows of `Viscoelastic`) become ignored records citing ADR 0291; their copies `..._f423_after_f410` and
`m1_to_m4_after_f410` pass with `mat_visco` landed, `Viscoelastic` pending `None`, its state size 24 (three terms, D1) and its
M4 row: `elasticity_matrix` refused naming `FOLLOW_UP`, `stress` at the default context refused with the key `"temperature"`
(the scaffold's material has a WLF shift), `stress` at `T = T_ref` with no time increment `Ok`. `sa_j2_default_bits.rs`: its test
becomes an ignored record (its last digest records the refusal message of the pending variant) and a copy
`..._after_f410` checks the ten other digests unchanged.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| R1 | a1 | one Hex8, strain step, three branches, 100 increments: `|S_xx - E_xx E(t)| / (E_0 E_xx)` at every step end and point | `<= 1e-10` | probe `1.7e-16`; Newton `1e-13` (arithmetic 6, 7) |
| R1m | a1 (companion) | the same at the material point | `<= 1e-10` | probe `1.7e-16` |
| R2 | a1 (companion) | material point, unequal weights (the scaffold's terms), strain step `(1e-3, -2e-4, 3e-4, 5e-4, -1e-4, 2e-4)`, 100 increments of `dt = 0.05 s`: each component against `2 G(t) dev eps + K(t) tr eps 1`, relative to `max |sigma_0|` | `<= 1e-10` | arithmetic 1 |
| C1 | a2 | one Hex8, dead load, standard solid, 20 steps per `tau` to `5 tau_c`: `max |eps - eps_closed| / eps_closed` | `<= 1e-3` | probe `1.208e-5`; geometric `1e-8` (arithmetic 5) |
| C2 | a2 | the same at 40 steps per `tau`; `log2(e_20/e_40)` | `>= 1.9` | probe `1.9999` |
| C1m, C2m | a2 (companions) | the same at the material point | as C1, C2 | probe |
| M1 | a3 | `complex_moduli(...).young` of the R1 material at 61 frequencies against the acceptance's formulas | `<= 1e-13` | rounding of the complex quotient, `1e-16` |
| M2 | a3 (companion) | shear and bulk moduli, unequal weights | `<= 1e-13` | rounding |
| M3 | b2 | `modulus_table` values at its points equal `young` (`E'`, `E''/E'`) bit for bit; refused for unequal weights (key `"terms"`) | exact | D7 |
| D1 | a4 | material point, 200 steps per cycle, tenth cycle: `|W - pi eps0^2 E''| / (pi eps0^2 E'')` | `<= 1e-2` | probe `2.5e-4` |
| D2 | a4 (companion) | one Hex8, prescribed cycled `u_x`, work of the reactions in the tenth cycle per volume | `<= 1e-2` | probe `2.5e-4` plus `O(eps0) = 1e-4` |
| S1 | a5 | one Hex8 at `T = 383 K`, WLF, `dt = 0.02 a_T`: `|S_xx - E_xx E(t/a_T)| / (E_0 E_xx)` | `<= 1e-10` | probe `1.7e-16` |
| S1m | a5 (companion) | material point, WLF and Arrhenius, uneven steps | `<= 1e-10` | probe `1.2e-16` |
| S2 | a5 (companion) | `shift_factor` against the formulas, five temperatures, both laws | `<= 1e-13` relative | rounding of `10^x`, `exp` |
| V1 | a6 | bar of 20 Hex8, `eta = 0.05`, 200 steps per period: `|delta/(pi eta) - 1|` | `<= 3e-2` | probe `2.502e-2` (first-order dispersion `2.56e-2`, arithmetic 3) |
| V2 | a6 (companion) | `delta` against the decrement of the exact root | `<= 5e-3` | probe `2.2e-4` |
| B1 | a7 | no branches: state size 0, `stress` (20 strains), `solve_from_at` (step and two increments), `nl_transient` (100 steps) against the base | bits equal | D1 |
| H1 | a8 | split run by save and restore against the unsplit run, three-branch bar | bits equal; states non-zero | ADR 0479 D3 |
| RF1 | a9 | the refusals of reading (a9) | each with its key | arithmetic 8 |
| T1 | design | material tangent against central differences (`h = 1e-7`, strains `1e-3`), unequal weights, `dt = 0.03 s`, a non-zero committed state, `max |C - C_fd| / max |C|` | `<= 1e-6` | the update is linear: rounding `1e-12` |
| N1 | b3 | B0: every digest of the base 2e0e3213 | exact | D8 |
| N2 | negative control | the linear paths refuse the material (`solve_static`, `modal_analysis`, `transient::Integrator::new`, `stiffness`) naming `mat_visco::FOLLOW_UP`; `elasticity_matrix` refused; `nonlinear::solve` refused (location `"the nonlinear static solve (no time)"`); an orthotropic base refused at the call; a shifted material without a temperature refused (key `"temperature"`); WLF with `c2 + T - T_ref <= 0` refused | all | D2, D4 |
| N3 | negative control | the R1 element through `solve_from` (default context, no time): the stress does not relax (`S_xx` at the end equals the stress of the step bit for bit) | exact | D1: `dt = 0` |
| N4 | b3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| X1 | b6 | the records and copies of D9 | pass | ADR 0291 |

Files: `crates/rcfd-fem/tests/sa_visco.rs` (R1 to T1, N2, N3), `sa_visco_default_bits.rs` (N1). A check that would take more
than about two minutes in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `gamma_i = 1` (the strain taken constant at its end value over the step) | C2 (order 1), C1, D1 |
| MU2 | the reduced increment ignores the shift (`dxi = dt`) | S1, S1m |
| MU3 | the tangent of the instantaneous base (`gamma_i` dropped from `G_t`, `K_t`) | T1 |
| MU4 | the committed strain not updated in the state | C1, C2, D1, H1 |
| MU5 | `nl_transient` passes the default context (no time) | V1, V2 |
| MU6 | the loss modulus with `x^2` in place of `x` in the numerator | M1, M2 |
| MU7 | `check_elements` ignores the history flag for the viscoelastic material | N2 |
| MU8 | the shear and bulk weights swapped in the update | R2, T1 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0 and
  the baseline hashes unchanged.
- **partial**: B0 and the baseline hashes unchanged, and at least the clauses (a1), (a3), (a5), (a7), (a8), (a9) met, with a
  miss of any of (a2), (a4), (a6) diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Isotropic base only (an orthotropic base is refused at the call): the shear/bulk split of a Prony series on an orthotropic
  matrix is not defined by the data model.
- Small-strain viscoelasticity on the Green-Lagrange strain and the second Piola-Kirchhoff stress of the total Lagrangian driver
  (large rotation, small strain), as J2; no finite-strain viscoelasticity.
- The temperature of a step is the step-end temperature of the point context (a temperature that changes in time is first
  order); the nonlinear transient integrator gives no temperature, so a shifted material is refused there; no thermal strain.
- The static load-factor driver has no time and refuses the material; a quasi-static time history is a loop of
  `solve_from_at` in the caller (a driver for it is a follow-up).
- The linear paths (static, modal, linear transient, harmonic, damped modes) refuse the material; the harmonic solve takes its
  complex modulus through `modulus_table` and the base material (D7), linear between the table's points.
- One mesh per check, no convergence study; no polymer data (decision A1 of ADR 0310): every constant is an input of a check.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Case-file key.** Not added (the brief: the maintainer's call). Recommendation for the lead: a material entry `viscoelastic
   = { base = <isotropic material>, terms = [{ shear, bulk, tau }], shift = { wlf = { reference, c1, c2 } } or { arrhenius = {
   reference, activation } } }` read into `Material::viscoelastic`, offered with the nonlinear transient study only.
2. **No time on the static driver.** Refused (D4), taken: a silent instantaneous response is worse than a refusal.
3. **The state layout.** `6 (N + 1)` values (D1), taken: the exact update needs the strain of the last step.
