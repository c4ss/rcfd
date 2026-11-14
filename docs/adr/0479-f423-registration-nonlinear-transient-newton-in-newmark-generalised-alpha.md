# ADR 0479: F-423 registration: nonlinear transient dynamics, the Newton driver of ADR 0404 inside the Newmark and generalised-alpha step

Status: **REGISTERED 2026-11-14, written before any code of F-423 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-423 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with plain Python/numpy scripts in the scratch folder
(`w49i-work/f423-scratch`: `reg_arith.py` with `reg_arith.log`, the probes `probe_truss.py`, `probe2.py` to `probe5.py` with
`probe4.log`, `probe5.log`; no rcfd code; disclosed, their numbers quoted below). The bit record B0 of the default paths is a
test file written and run on the base after this commit and before any library code (D7). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Registration of the stub [ADR 0326](0326-nonlinear-transient-dynamics.md) (work package SA-16 of the plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-423 (deps F-408, F-397, F-398). Number 0479 given by
the lead. Branch `f423-work` from main 23d6b8e6 (with the J2 plasticity of F-422), worktree `w49i-work/f423`, scratch
`w49i-work/f423-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except the records-and-copies scheme of ADR 0291 in `sa_scaffold.rs`; clean-room.

## What F-423 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. At a load factor of 1e-6 the history equals the linear one of F-398 within 1e-9 relative. A stiff slender bar pinned at one end and released from the horizontal under gravity swings with the period 2 pi sqrt(2 L/(3 g)) (2/pi) K(sin 45 degrees) (factor 1.1803, recomputed in the registration) within 1 percent, the total energy constant within 1e-3 of its swing over five periods at the registered step. A free spinning elastic bar keeps its linear momentum within 1e-10 and its angular momentum within 1e-6 relative over ten turns. Large axial oscillation of a bar element with nu = 0 from the stretch 1.2: period equal to the quadrature of the energy integral (computed in the registration) within 0.5 percent. With the spectral radius 1 the generalised-alpha result equals Newmark within 1e-10. A run split by save and restore has the bits of the unsplit run, with a material state included. A step that does not converge after the registered halvings is an error with the time.

The brief adds: (b1) reuse the nonlinear static Newton driver of `nonlinear.rs` (with the J2 trial-state path of F-422) and the
linear integrators of F-398, no second Newton loop; (b2) "with a material state included" means the J2 history of F-422
survives save and restore; (b3) models that do not use the new module keep their bits, the baseline hashes stay unchanged, a
default-bits test records digests at the base before any library change; (b4) a case-file key is the maintainer's call
(recommended, not added); (b5) probe a quantity before a `1e-12`-class limit, `1e-11` for sums over about `1e3` terms, the
`1e-10` momentum limit over ten turns probed first; (b6) the records-and-copies scheme of ADR 0291 in `sa_scaffold.rs` (the
latest X1 copy `x1_modules_and_pending_after_f415_f420_f422_f416_f417` becomes an ignored record citing ADR 0291, an
`_after_f423` copy added; minimal edit, another branch edits the file).

Clauses: (a0) every limit registered here; (a1) load factor `1e-6` against F-398, `1e-9` relative; (a2) the compound pendulum
released from the horizontal: period within 1 percent, energy within `1e-3` of its swing over five periods; (a3) the free
spinning bar: linear momentum `1e-10`, angular momentum `1e-6` relative, ten turns; (a4) the large axial oscillation from the
stretch 1.2, `nu = 0`: period within 0.5 percent of the energy-integral quadrature; (a5) generalised-alpha with `rho_inf = 1`
equals Newmark within `1e-10`; (a6) a split run with a material state has the bits of the unsplit run; (a7) a step that does not
converge after the registered halvings is an error with the time.

Readings, registered:

- **(a1)** "The linear one of F-398" is `transient::Integrator` (ADR 0389) on the same model with the same load history. The
  "load factor" multiplies the load of F-398's own registered loaded case: bar B of ADR 0389 D5 with the step load of the
  pattern `M phi_1` (case A5 there, mass-normalised first mode, constant table). At factor 1 that load already gives a peak
  strain of about `8e-9` (arithmetic 1), so at `1e-6` the geometric nonlinearity is below `1e-14` relative and the check tests
  the time-integration algebra of the nonlinear step against the linear one (the purpose of the clause). Measure: `max_n |u_nl,n
  - u_lin,n|_inf / max_n |u_lin,n|_inf` over every step, full displacement vectors. Three runs: Newmark average acceleration
  undamped; Newmark with the Rayleigh damping of F-398 (`zeta = 0.02` at `omega_1`); generalised-alpha `rho_inf = 0.8` with that
  damping. A companion at factor 1 (L1b) and a negative control at factor `1e5` (N1) show that the nonlinearity is there.
- **(a2)** "Stiff": the axial and bending frequencies far above the swing (arithmetic 2), so that the elastic deformation
  changes the period by less than `1e-3`; the modulus is not that of steel because the rounding of the internal force of a
  steel bar under large rotation is at the Newton residual tolerance (arithmetic 5). "Pinned at one end": the line of the
  nodes at `x = 0, z = 0` (ADR 0326: "a pin on a solid mesh is a line of constrained nodes"). The period is read from the angle
  `psi = atan2(x_tip, -z_tip)` of the node on the axis at the free end: the crossings of `psi = 0` (the bottom, where the
  angular acceleration vanishes) located by linear interpolation in a step, `T = 2 (t_last - t_first)/(N - 1)` over the `N`
  crossings of the run. "Total energy": kinetic `v^T M v/2` plus the St. Venant-Kirchhoff strain energy plus the potential of
  the dead gravity load `-f . u`; "its swing": the drop of the potential to the bottom `m g L/2`; measure `max_n |E_n - E_0| /
  (m g L/2)` over the run of at least five periods.
- **(a3)** "Over ten turns": ten revolutions at the spin rate. Linear momentum `P = sum over the nodes of (M v)_node`, angular
  momentum about the origin `J = sum over the nodes of (X + u)_node x (M v)_node` (consistent mass, arithmetic 3). Relative to
  their initial values: the bar is given a translation velocity as well, so that `|P_0| > 0`. Measures `max_n |P_n - P_0| /
  |P_0|` and `max_n |J_n - J_0| / |J_0|`. The bar starts in its steady spin (the stretched state of the rotating frame, solved by
  the static driver, D5): a bar started unstretched carries an axial transient that the trapezoidal rule cannot follow at a
  step it takes for the swing, and the probe shows the angular momentum then off by `1.6e-3` (arithmetic 4).
- **(a4)** "A bar element": one Hex8, `nu = 0`, lateral displacements zero, the end `x = 0` held axially; the four end nodes at
  `x = L` move together (uniform stretch, exact for Hex8 at its 8-point rule), so the model is the one-unknown oscillator of ADR
  0326 with the St. Venant-Kirchhoff bar force and the consistent mass `rho A L/3` (arithmetic 6). Released at rest from
  `lambda = 1.2`. Period from the crossings of `lambda = 1` in the same direction (downwards), linear interpolation, over the
  run; `T_FE = (t_last - t_first)/(N - 1)`.
- **(a5)** "The result": the displacement history; measure as (a1) between the two runs. On the axial oscillation of (a4) (G1)
  and on the J2 bar of (a6) (G2).
- **(a6)** "The bits": the saved state bytes (D3: time, step, displacement, velocity, acceleration, internal force, every
  material state value) of the run of `n` steps against the run split at step `n/2` by `to_bytes`, `from_bytes` and `restore`
  into a newly built integrator; on a J2 bar that has yielded before the split, for Newmark and for generalised-alpha `rho_inf =
  0.8` (the latter keeps the internal force of the last step in its state).
- **(a7)** "The registered halvings": `NewtonOptions::max_halvings` of the driver, default 5, here the halvings of the time
  step. "An error with the time": `FemError::Solver { solver: "nl_transient" }` whose message names the step and its start and
  end times (`{:e}`) and the halvings; the committed state is unchanged by the failed step. Companion H2: the halving path
  itself (a step that fails once and is taken in two halves gives the bits of two steps at half the step).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0326 (stub), ADR 0310 (the section on ADR 0326, the module table, section 2.3), the roadmap
  entries of F-423, F-397, F-398, F-408, F-422; ADR 0475 for the form of a registration and its default-bits record.
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `transient.rs` (whole: `Method::parameters`, `Rayleigh`, `LoadTable`,
  `LoadHistory`, `TransientState` bytes, `Integrator` steps), `nl_transient.rs` (the stub), `mass.rs` (`consistent_with`),
  `constraint.rs` (`Prescribed`, `prescribed_equations`, `reduce`, `check_rigid_body_motions`), `model.rs` (`DofMap`, the
  `check_elements` family, `Material::j2`, `Load::gravity`), `mesh.rs` (`add_node_set`), `element.rs` (heads), `mat_j2.rs`
  (heads).
- Tests: `si_transient.rs` (bar B, `loaded_damped_run`, `max_rel_diff`), `sa_j2_default_bits.rs` (the digest scheme),
  `sa_scaffold.rs` (the X1 records).

## Arithmetic done before the code (disclosed)

1. **Bar B under the A5 load** (ADR 0389 D5: `hex_box([0;3], [1, 0.1, 0.1], [20, 1, 1])`, `E = 2e11`, `nu = 0`, `rho = 7850`,
   lateral displacements zero, `x = 0` held). Mass `78.5 kg`; the mass-normalised first mode has a tip value of about
   `sqrt(2/78.5) = 0.16`; `omega_1 = (pi/2) sqrt(E/rho) = 7928 rad/s`; the static response to `M phi_1` is `phi_1/omega_1^2`, tip
   `2.5e-9 m`, peak strain `4e-9`, dynamic peak about `8e-9`. The St. Venant-Kirchhoff nominal stress is `E (eps + 3 eps^2/2 +
   eps^3/2)` (uniaxial, `nu = 0`), relative deviation from linear `1.5 eps`: at factor 1 about `1.2e-8`, at `1e-6` about
   `1.2e-14`, at `1e5` (peak strain `8e-4`) about `1.2e-3`. Rounding: the nonlinear step solves for `u_{n+1}` and forms `a_{n+1} =
   (u_{n+1} - u~)/(beta dt^2)`; the cancellation costs about `eps_mach (u/(beta dt^2 a)) = eps_mach 4/(omega dt)^2` in the first
   mode, `4e-14` at `omega_1 dt = 2 pi/40`. The limit `1e-9` of (a1) is four decades above the estimate; L1b gets `1e-7`.
2. **The compound pendulum.** A bar of length `L` pivoting about one end: `I = m L^2/3` (`m (L^2/3 + h^2/12)` for the height
   `h` about the mid-height of the end: relative `h^2/(4 L^2) = 1e-4`, period `5e-5`), gravity torque `m g (L/2) sin theta`; small
   swings `T_0 = 2 pi sqrt(2 L/(3 g))`; released from the horizontal (`theta_0 = 90` degrees) the energy integral gives `T = T_0
   (2/pi) K(k)`, `k = sin(theta_0/2) = sin 45 degrees` (first principles; the elliptic form as in any mechanics text, e.g.
   Landau and Lifshitz, Mechanics, section 11, from memory, NOT re-read; re-derived). `K(sin 45 degrees)` by the
   arithmetic-geometric mean: `1.854074677301372` (a 20000-point midpoint quadrature of `int_0^{pi/2} dphi/sqrt(1 - sin^2 phi/2)`:
   `1.854074677301369`); the factor `(2/pi) K = 1.180340599016096` (the acceptance's 1.1803). With `L = 1 m`, `g = 9.81 m/s^2`:
   `T_0 = 1.637946585913 s`, `T = 1.933334854373 s`. The consistent mass of a Hex8 bar integrates a rigid rotation exactly
   (linear field), so the discrete bar has the inertia `I` and the torque of the continuum.
   **The registered bar**: `L = 1`, square section `b = 0.02` (slenderness 50), `hex_box([0, -b/2, -b/2], [L, b, b], [10, 2,
   2])` (the mid lines `y = 0`, `z = 0` exist as node lines), `E = 2e9 Pa`, `nu = 0.3`, `rho = 7850`. Axial strain under the
   tension at the bottom (at most about `2.5 m g` at the pivot): `2.5 rho g L/E = 9.6e-5`, the period longer by about `5e-5`;
   axial first frequency `(pi/2) sqrt(E/rho) = 793 rad/s`, bending of the pinned-free bar `15.4 sqrt(E b^2/(12 rho)) = 45 rad/s`
   (higher for the fully integrated Hex8, which locks), against the swing `2 pi/T = 3.25 rad/s`. Step `dt = 0.005 s` (387 steps per
   period), Newmark average acceleration; the trapezoidal period error `(omega dt)^2/12 = 2.2e-5` for the swing. 1940 steps
   (`9.70 s >= 5 T = 9.667 s`).
   Probe (`probe4.log`): the two-node St. Venant-Kirchhoff truss with consistent mass is the same pendulum (the free node carries
   `m/3` and half the weight); Newmark average acceleration with Newton: `E = 2e9`, `dt = 0.01`: `T/T_0 = 1.180875`, energy
   `1.10e-3` of the swing; `dt = 0.005`: `T/T_0 = 1.180504` (`1.4e-4` long), energy `2.76e-4`. With `E = 2e11` Newton did not
   reach a relative residual of `1e-10` (arithmetic 5). The energy error is the oscillating error of the trapezoidal rule, about
   `(omega dt)^2`-scaled (factor 4 per halving). The limit `1e-3` of the acceptance is 3.6 times the probe at the registered step;
   the Hex8 bar adds a bending vibration started by the release (the sag under its weight), whose energy is a small part of the
   swing (strain energy of the sag about `(3.25/45)^2/2 = 2.6e-3` of `m g L/2` at most; it is exchanged, not lost, by the
   trapezoidal rule in the linear range). Not probed on Hex8.
3. **Momentum of the semi-discrete system.** With the consistent mass `M_ab I` (scalar blocks) and an internal force that is
   invariant under translation and rotation (total Lagrangian, objective), `sum_a f_int,a = 0` and `sum_a x_a x f_int,a = 0`
   for every `u`; the semi-discrete `P = sum_a p_a`, `p_a = sum_b M_ab v_b`, and `J = sum_a x_a x p_a` are constant (`dJ/dt = sum_ab
   M_ab v_a x v_b - sum_a x_a x f_int,a = 0`, the first sum antisymmetric). The trapezoidal rule keeps `P` exactly for the
   translation-invariant force: `P_{n+1} - P_n = (dt/2) sum (M a_n + M a_{n+1})` and `sum M a = -sum f_int - sum R`; the
   translational part of the Newton residual is linear in `u` (`t^T R(u) = -c t^T M (u - u~)`), so one correction removes it and
   only rounding remains: per step about `dt eps_mach |M a| sqrt(N)`, relative `eps_mach omega dt (omega L/|V|) sqrt(N) = 1e-16`,
   ten turns `~1e-14` (random walk). The angular momentum is not kept by the trapezoidal rule in general (J. C. Simo and N.
   Tarnow, ZAMP 43 (1992) 757, and Simo, Tarnow and Wong, CMAME 100 (1992) 63, cited from memory, NOT read): the probe decides.
4. **Probe of the spinning bar** (`probe4.log`, `probe5.log`, two-node truss, `L = 1`, `A = 4e-4`, `rho = 7850`, spin `2 pi rad/s`
   about the centre, translation `(0.3, 0.2, 0) m/s`). Started unstretched: Newmark average acceleration did not converge (the
   axial transient at `omega dt = 175`, the known non-robustness of the trapezoidal rule in nonlinear stiff problems),
   generalised-alpha `rho_inf = 0.9` and `0.5` ran with `|dP|/|P| = 1.3e-12` and `|dJ|/|J| = 1.6e-3` (100 steps per turn), `4.0e-4`
   (200): the error of the start transient, independent of `E` (`2e9`, `2e8`). Started in the steady spin (stretched for the
   centripetal force): Newmark and generalised-alpha `rho_inf = 1` `|dJ|/|J| = 7.2e-12` and `2.8e-12` (100 steps per turn, ten
   turns), generalised-alpha `rho_inf = 0.9` `7.9e-6` (100) and `2.0e-6` (200, order 2). In the steady discrete spin of the
   trapezoidal rule the chord and the mean velocity give `|v| = (2 r/dt) tan(phi/2)` and the mean acceleration `omega^2 r` for the
   initial `|v| = omega r`, the centripetal force of the continuum, so the stretched start is a steady solution of the scheme
   and `J` is constant to rounding. Registered: Newmark average acceleration from the steady spin; the limits of the acceptance
   (`1e-10`, `1e-6`) are two and five decades above the probe; the `1e-10` on `P` sums `1e3` steps of rounding (b5: probed at
   `1e-12`).
   **The registered bar**: as the pendulum bar (`L = 1`, `b = 0.02`, `[10, 2, 2]` Hex8, `E = 2e9`, `nu = 0.3`, `rho = 7850`), no
   constraints, spin `omega = 2 pi rad/s` about the axis `z` through the centre `c = (L/2, 0, 0)`, translation `V = (0.5, 0.25,
   0.1) m/s`, `dt = 0.01 s` (100 steps per turn), 1000 steps. Centrifugal strain at the centre `rho omega^2 L^2/(8 E) = 1.9e-5`;
   lowest free-free bending frequency `22.4 sqrt(E b^2/(12 rho)) = 65 rad/s` against `omega = 6.3`.
5. **Rounding floor of the Newton residual.** Under a large rotation the displacement gradient `H` is of order 1 and the
   Green-Lagrange strain is a cancellation of `sym H` and `H^T H/2` down to the elastic strain; the stress then carries an
   absolute rounding of about `E eps_mach`, a nodal force about `E A_e eps_mach`. Against a force scale of the weight `m g`: for
   steel (`E = 2e11`, `A = 4e-4`, `m g = 31 N`) `2.6e-10` per node, above the driver's residual tolerance `1e-10`; for `E = 2e9`
   `2.6e-12`. Hence `E = 2e9` for the pendulum and the spinning bar (still "stiff", arithmetic 2 and 4).
6. **The axial oscillator.** One Hex8, length `L`, section `A`, `nu = 0`, lateral displacements zero, `u_x = 0` at `x = 0`: the
   uniform stretch `lambda = 1 + q/L` gives `E_xx = (lambda^2 - 1)/2`, the strain energy `W = (E A L/8)(lambda^2 - 1)^2` (exact
   at the 8-point rule: the field is constant), the end force `dW/dq = (E A/2) lambda (lambda^2 - 1)`, the consistent mass of the
   four moving nodes `rho A L/3` (`rho A int_0^L (x/L)^2 dx`). With `nu = 0` and zero lateral strain the lateral stresses vanish,
   so no lateral force arises (`F_yx = 0`). Energy: `(rho A L/6) qdot^2 + W(lambda) = W(1.2)`. The other turning point:
   `(lambda^2 - 1)^2 = 0.44^2`, `lambda_min = sqrt(0.56) = 0.748331477354788`. Period `T = 2 int dq/qdot`; with `s = lambda^2 - 1
   = 0.44 sin phi` the endpoint singularities cancel: `T/T_lin = (1/pi) int_{-pi/2}^{pi/2} dphi / sqrt(1 + 0.44 sin phi)`, `T_lin =
   2 pi sqrt(rho L^2/(3 E))` (the linear period of the same oscillator). Computed (`reg_arith.log`): midpoint rule with 64,
   128, 256, 1024, 4096 points `1.040744328890327 .. 329`, and the series `sum_k C(4k,2k)/16^k C(2k,k)/4^k 0.44^{2k}` (the
   binomial series averaged over `phi`) `1.040744328890326`. **`T/T_lin = 1.0407443288903`.** With `L = 1`, `A = 0.1 x 0.1`, `E =
   2e11`, `rho = 7850`: `T_lin = 7.186858124315e-4 s`, `T = 7.479693e-4 s`. Step `dt = T_lin/200`; the trapezoidal period error
   `(omega dt)^2/12` with the stiffest local frequency `omega sqrt(1.66)` at `lambda = 1.2`: below `1.4e-4`. 1200 steps (`5.77 T`;
   at least five downward crossings of `lambda = 1`).
7. **Generalised-alpha in the nonlinear step** (D1). With `rho_inf = 1`: `alpha_m = alpha_f = 1/2`, `beta = 1/4`, `gamma =
   1/2`; the step equation `(M a_{n+1} + f_int,n+1 - f_{n+1})/2 + (M a_n + f_int,n - f_n)/2 = 0` is the Newmark equilibrium at
   `n+1` if the one at `n` holds: the residual of the generalised-alpha step at `n+1` is minus the one at `n` plus its own Newton
   residual, so the two histories differ by the converged Newton residuals propagated through a stable step (an alternating
   sum, no growth), not by `O(dt)`. Estimate `<= 1e-13` relative; the limit `1e-10` of the acceptance.
8. **The J2 bar of (a6).** One Hex8 `[1, 0.1, 0.1]`, lateral displacements zero, `x = 0` held; `Material::j2` on the isotropic
   base `E = 2e11, nu = 0.3, rho = 7850`, `sigma_y = 2e8`, `H_iso = 2e9`, `H_kin = 1e9`. Uniaxial strain: von Mises `|sigma_xx -
   sigma_yy| = 2 mu eps`, yield at `eps_y = sigma_y/(2 mu) = 1.30e-3`; the constrained modulus `E (1 - nu)/((1 + nu)(1 - 2 nu)) =
   2.692e11`, `omega = sqrt(3 x 2.692e11/7850)/L = 10144 rad/s`. Released from rest at `u = 0` with the axial velocity `50 m/s` of
   the four end nodes: elastic amplitude `v/(omega L) = 4.9e-3`, first yield after about `arcsin(1.30/4.93)/omega = 2.6e-5 s`.
   `dt = 6e-6 s`, 60 steps (`0.58` of the elastic period), split at 30 (yielded, R0).

## Design

### D1. The step (first principles; the generalised-alpha method of Chung and Hulbert, J. Appl. Mech. 60 (1993) 371, and Newmark, J. Eng. Mech. Div. ASCE 85 (1959) 67, as ADR 0389 D1; the averaged internal force as in Kuhl and Crisfield, IJNME 45 (1999) 569, and Erlicher, Bonaventura and Bursi, Comput. Mech. 28 (2002) 83, cited from memory, NOT read; derived here)

`M a + C v + f_int(u) = f(t)`, `f_int` the total Lagrangian internal force of `nonlinear.rs`, `M` the consistent mass, `C =
alpha_R M + beta_R K_0` (Rayleigh, `K_0` the tangent at `u = 0` with the initial states: a constant matrix), `f(t) = sum_j
table_j(t) pattern_j` (`transient::LoadHistory`). Parameters `(alpha_m, alpha_f, beta, gamma)` of `transient::Method::parameters`
(Newmark: `alpha_m = alpha_f = 0`). One step of size `h` from `(u_n, v_n, a_n, f_int,n)`: the unknown is `u_{n+1}`,

    u~ = u_n + h v_n + h^2 (1/2 - beta) a_n,   v~ = v_n + h (1 - gamma) a_n,
    a_{n+1} = (u_{n+1} - u~)/(beta h^2),       v_{n+1} = v~ + gamma h a_{n+1},
    M [(1 - alpha_m) a_{n+1} + alpha_m a_n] + C [(1 - alpha_f) v_{n+1} + alpha_f v_n]
      + (1 - alpha_f) f_int(u_{n+1}) + alpha_f f_int,n = (1 - alpha_f) f_{n+1} + alpha_f f_n.

The internal force enters as the weighted mean of its end values (not at the weighted displacement), so that with `rho_inf = 1`
the step is the trapezoidal rule (arithmetic 7) and the linear case is the equation of ADR 0389 D1 exactly (`K u` linear). For
the driver the equation is divided by `1 - alpha_f`: `f_int(u) + g(u) = F` with the load `F = ((1 - alpha_f) f_{n+1} + alpha_f
f_n)/(1 - alpha_f)` and the inertia contribution `g(u) = (M a_m + C v_f + alpha_f f_int,n)/(1 - alpha_f)` of tangent
`[(1 - alpha_m)/(beta h^2) M + (1 - alpha_f) gamma/(beta h) C]/(1 - alpha_f)`. `beta = 0` (explicit) is refused (key
`"method"`). The Newton iteration is the driver's (D2), started from `u_n`; at convergence `a_{n+1}`, `v_{n+1}` follow from the
formulas above and `f_int,n+1` is the driver's internal force at the converged iterate; the material states are the driver's
trial states of the converged iterate (from the states committed at `n`: the J2 path of ADR 0475 D3).

### D2. The driver (`nonlinear.rs`, no second loop)

1. `pub fn solve_from(model, prescribed, load, start, states, options, contributions) -> Result<Equilibrium>`: one attempt of
   ADR 0404 D5 step 4 (`Problem::attempt`, the same function) at the load factor 1 from 1 (so the prescribed values are held,
   no increment), from the start displacement `start` and the committed `states`, with the full load vector `load` and the
   contributions. It refuses what `solve_with` refuses (the model checks, the options, lengths) but does not require the
   prescribed equations to remove the rigid-body motions (the inertia contribution makes the step's tangent regular; a
   singular tangent is still refused by the pivot test). A failed attempt is returned as the error `solve_with` would give after
   its halvings (the located inversion, or `FemError::Solver { solver: "newton" }` with the cause). `Equilibrium { u, states,
   internal_force, residuals, energies }`, `internal_force` without the contributions.
2. `Contribution::in_residual_reference(&self) -> bool`, default `false`: when a contribution returns `true`, the reference of
   the relative residual is also at least the norm of its force and of the internal force without the contributions. Without
   it a free body under its inertia alone (`f = 0`) could not converge: the driver's reference `max(|lambda f_F|, |f_int +
   g|)` is then the norm of the residual itself. With the default `false` the reference is the one of ADR 0404 bit for bit.
3. `Attempt` keeps the internal force of its converged iterate (moved, not recomputed; the default path computes the same
   values in the same order); `displacement_gradient` and `green_lagrange` become `pub(crate)` for the strain energy (D3).

Nothing else of the driver changes: `solve` and `solve_with` execute the operations they executed (B0).

### D3. The integrator (`nl_transient.rs`)

`PENDING = None`. `NlTransientOptions { method, time_step, damping, newton }` (`newton: NewtonOptions`, its `max_iterations`,
tolerances and `threads` per step and its `max_halvings` as the halvings of the time step; its `steps` unused).
`Integrator::new(model, loads, options)`: refuses the transformation path (linear constraints, rigid sets, cyclic constraints,
elastic supports) and a non-zero prescribed value (key `"nl_transient"` and `"constraint"`, the zero-value constraints of the
model are its prescribed equations, as in ADR 0389), a temperature load, what the nonlinear driver refuses, a non-positive step,
`beta = 0`, invalid damping, a load pattern of the wrong length; builds `M` (`mass::consistent_with`), `C`. `start(t0, u0, v0)`:
full vectors (zero at the prescribed equations), states empty (`initial_states`), `f_int,0 = internal_force(u0)`, `a0` from `M
a0 = f(t0) - C v0 - f_int,0` on the free equations; a model with a material with history starts from `u0 = 0` only (its state at
a non-zero `u0` is not defined; refused, key `"initial state"`). `step()`, `run(n)` (coverage of the load tables refused before
any step, as ADR 0389), `step_with(contributions)` (extra driver contributions in each Newton iteration, after the inertia; used
by H2), each returning a `StepReport { halvings, iterations }` per step. `state()`/`restore(state)`; `NlTransientState` with
`to_bytes`/`from_bytes` (magic `RCFDNLT1`, little-endian: `n`, `step`, element count as `u64`, the bits of `t0`, `dt`, the four
parameters, `u`, `v`, `a`, `f_int` (all equations), then per element its value count (`u64`) and values); `restore` refuses
another length, element layout, time step or method (bits). The time of the state is `t0 + n dt`, never accumulated.
`displacement()`, `velocity()`, `acceleration()`, `kinetic_energy()` (`v^T M v/2`), `strain_energy()` (`sum_q w_q S . E/2` at
the stiffness rule, the St. Venant-Kirchhoff potential; refused for a material with history, key `"strain energy"`),
`momentum()` (`P`, `J` of arithmetic 3 about the origin).

### D4. Halving

A step `n -> n+1` is tried at the levels `l = 0, 1, ..., max_halvings`: at level `l` as `2^l` sub-steps of `h = dt/2^l` (exact),
sub-step `k` from `t_n + k h` to `t_n + (k + 1) h` (the last ends at `t_{n+1} = t0 + (n + 1) dt`), each one driver attempt; if
a sub-step fails with a driver failure (`FemError::Solver { solver: "newton" }` or a located inversion `FemError::Element`) the
level is abandoned and the next starts again from the committed state of `n`. Other errors return at once. After the last level
the error is `FemError::Solver { solver: "nl_transient", reason }` with "step `n+1` from t = `t_n:e` to t = `t_{n+1}:e` did not
converge after `max_halvings` halvings of the time step: `<cause>`"; the committed state is unchanged. A converged step reports
its level as `halvings`.

### D5. The models

- Bar B (L1, L1b, N1): arithmetic 1; `dt = T_1/40`, 80 steps, the table constant `1` over `[0, 10 T_1]`, start at rest.
- Pendulum (P1, P2, T1): arithmetic 2; node sets `pin` (the three nodes with `x = 0, z = 0`: `u_x = u_z = 0`) and `pin_mid` (the
  node `(0, 0, 0)`: `u_y = 0`); `Load::gravity([0, 0, -9.81])` scaled by a constant table over `[0, 10 s]`; start at rest,
  undeformed; tip node `(L, 0, 0)`.
- Spinning bar (S1, S2): arithmetic 4. The steady spin: `nonlinear::solve_with` of the same mesh without constraints, with the
  symmetry planes `x = L/2` (`u_x = 0`), `y = 0` (`u_y = 0`), `z = 0` (`u_z = 0`) given as `Prescribed` and the centrifugal
  contribution of the rotating frame, force `-omega^2 M P (X + u - c)` (`P` the projection on the plane `x, y`) and tangent
  `-omega^2 M P`, `in_residual_reference` true, one load step, driver defaults. The dynamic model has no constraints; start
  `u0` the steady solution, `v0 = omega e_z x (X + u0 - c) + V`.
- Axial oscillator (A1, G1, H1, H2): arithmetic 6, `hex_box([0;3], [1, 0.1, 0.1], [1, 1, 1])`; constraints `all` `u_y = u_z = 0`,
  `xmin` `u_x = 0`; `u0 = 0.2 X` in `x`, at rest.
- J2 bar (R0, R1, G2): arithmetic 8.

### D6. What is not offered

The transformation path (linear constraints, rigid sets, cyclic constraints, elastic supports), prescribed motion histories and
non-zero prescribed values; temperature loads; Beam2 and shell elements and the formulations the nonlinear driver refuses;
follower loads and contact in dynamics (contributions can be passed, none is provided); no energy-momentum scheme (ADR 0326); a
history material started from a non-zero displacement; the strain energy of a material with history; the damping matrix is
built from the initial tangent (not updated); no case-file key (open question 1).

### D7. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_nl_transient_default_bits.rs`, written after this commit and run on the unchanged base 23d6b8e6 before
any library code; its digests (FNV-1a 64 over the bits) are written into the file then. Content: `nonlinear::solve` of an elastic
stretched Hex8 bar (displacement, reactions, residuals and energies of every increment); `solve_with` with a spring contribution
of non-zero force and tangent (the contribution path of the residual reference); `solve_with` with a zero contribution that
refuses one call (the halving path); `nonlinear::solve` of the J2 bar of F-422's U4 kind (states included); the linear
`transient::Integrator` of bar B under the A5 load, Newmark and generalised-alpha `0.8` with damping (state bytes after 20 steps);
`mass::consistent` of bar B. The tests of `sa_j2_default_bits.rs` and `si_transient.rs` stay as they are and must pass.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| L1 | a1 | bar B, A5 load times `1e-6`, against `transient::Integrator`: Newmark, Newmark with damping, generalised-alpha `0.8` with damping; `max |du|_inf / max |u_lin|_inf` | `<= 1e-9` each | arithmetic 1: `1.2e-14` nonlinear, `4e-14` cancellation |
| L1b | a1 (companion) | the same at factor 1 | `<= 1e-7` each | arithmetic 1: `1.2e-8` |
| N1 | negative control | the same at factor `1e5` (Newmark undamped) | `>= 1e-6` | arithmetic 1: `1.2e-3` |
| P0 | a2 (set-up) | pendulum: crossings of `psi = 0` in the run | `>= 9` | five periods, two crossings each |
| P1 | a2 | pendulum period `|T_FE - T|/T`, `T = 1.933334854373 s` | `<= 1e-2` | probe `1.4e-4`; elastic stretch `5e-5` |
| P2 | a2 | pendulum energy `max |E_n - E_0| / (m g L/2)`, 1940 steps of `0.005 s` | `<= 1e-3` | probe `2.76e-4` (truss) |
| S0 | a3 (set-up) | the steady spin solve converges; 1000 steps of `0.01 s` = ten turns | `Ok` | small strain `1.9e-5` |
| S1 | a3 | linear momentum `max |P_n - P_0|/|P_0|` | `<= 1e-10` | probe `1e-12` (truss); arithmetic 3 `~1e-14` |
| S2 | a3 | angular momentum `max |J_n - J_0|/|J_0|` | `<= 1e-6` | probe `7.2e-12` (truss, steady start) |
| A1 | a4 | axial oscillator period `|T_FE/T_lin - 1.0407443288903| / 1.0407443288903` | `<= 5e-3` | arithmetic 6: `<= 1.4e-4` |
| A0 | a4 (set-up) | downward crossings of `lambda = 1` in the run; the four end nodes equal (bits) at every step | `>= 5`; all | 5.77 periods; symmetry |
| G1 | a5 | generalised-alpha `rho_inf = 1` against Newmark, axial oscillator, 1200 steps, `max |du|_inf / max |u|_inf` | `<= 1e-10` | arithmetic 7: `<= 1e-13` |
| G2 | a5 | the same on the J2 bar, 60 steps | `<= 1e-10` | arithmetic 7 |
| R0 | a6 (set-up) | J2 bar: some point has an equivalent plastic strain `> 0` at the split (step 30) | holds | arithmetic 8 |
| R1 | a6 | J2 bar, 60 steps against 30 + save, bytes, restore into a new integrator, 30: equal state bytes; Newmark and generalised-alpha `0.8` | exact | D3 |
| R2 | a6 (companion) | `from_bytes(to_bytes(s)).to_bytes()` equals `to_bytes(s)`; a wrong magic, a truncated byte string, another time step, another method, another element layout refused (key `"state"`) | exact; all | D3 |
| H1 | a7 | axial oscillator with `max_iterations = 1` (a correction never confirmed: the energy criterion needs two): the first step returns `FemError::Solver { solver: "nl_transient" }`, the message contains `t = 0e0`, `t = <dt:e>` and `after 5 halvings`; the state after the error has the bytes of the state before | holds | D4 |
| H2 | a7 (companion) | axial oscillator, a zero contribution that refuses its second call once: the first step reports 1 halving, and its state has the bits of `u, v, a, f_int`, states of two steps of an integrator at `dt/2` with the same contribution never refusing | exact | D4: `h = dt/2` exact |
| T1 | companion | pendulum, 50 steps with 1 and 4 assembly threads: equal state bytes | exact | the driver's assembly (ADR 0404 D4) |
| F1 | refusals | `Integrator::new` refuses: a non-positive step (`"time step"`), Newmark `beta = 0` (`"method"`), a model with a `Constraint::Linear` (`"nl_transient"`), a non-zero prescribed value (`"constraint"`), a load pattern of the wrong length (`"load pattern"`); `run` refuses a table that does not cover the run (`"load table"`) before any step; `start` of a J2 model with `u0 != 0` (`"initial state"`); `strain_energy` of the J2 model (`"strain energy"`) | all, with the key | D3 |
| B1 | b3 | B0: every digest of the base 23d6b8e6 | exact | D7 |
| B2 | b3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output path of a case changed |
| X1 | b6 | `x1_modules_and_pending_after_f415_f420_f422_f416_f417` becomes an ignored record citing ADR 0291, its `_after_f423` copy (`nl_transient` landed) passes | pass | the scheme of ADR 0291 |

Files: `crates/rcfd-fem/tests/sa_nl_transient.rs` (L1 to T1, F1), `sa_nl_transient_default_bits.rs` (B1). A check that would
take more than about two minutes in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the term `alpha_f f_int,n` dropped from the inertia contribution | L1 (generalised-alpha), G1, G2 |
| MU2 | the term `alpha_m a_n` dropped from the alpha-point acceleration | L1 (generalised-alpha), G1 |
| MU3 | the predictor `u~` with `1/2` in place of `1/2 - beta` | L1, A1 |
| MU4 | `f_int` not written by `to_bytes` (restored as zero) | R1 (generalised-alpha), R2 |
| MU5 | the material states not written (restored as the initial states) | R1 |
| MU6 | `in_residual_reference` ignored by the driver | S0 or S1, S2 (the free spin cannot converge) |
| MU7 | the halving retries the same step size | H2 |
| MU8 | the velocity update with `gamma = 1/2` in place of the method's `gamma` | L1 (generalised-alpha) |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B1 and B2
  unchanged.
- **partial**: B1 and B2 unchanged, and at least the clauses (a1), (a5), (a6), (a7) met (L1, G1, G2, R1, H1), with a miss of any
  of (a2) to (a4) diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The trapezoidal rule and the generalised-alpha method in the averaged-force form; no energy-momentum or energy-decaying scheme
  (ADR 0326); the trapezoidal rule is not robust for a stiff nonlinear transient at large steps (arithmetic 4): the checks start
  the spin in its steady state, and the pendulum is not run at other steps.
- One mesh per check, no convergence study; the pendulum and the spinning bar use `E = 2e9` (arithmetic 5).
- What D6 lists as not offered; no follower load, contact, flag benchmark (ADR 0337) in dynamics.
- The damping matrix from the initial tangent; with a material with history the damping keeps its elastic stiffness part.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Case-file key.** Not added (the brief: the maintainer's call). Recommendation for the lead: `[study] kind = "transient"`
   with `nonlinear = true` selecting `nl_transient::Integrator` (the method, step, damping and load tables of the linear
   transient study; the Newton options of the nonlinear static study).
2. **The averaged-force form of generalised-alpha** (D1): taken, because it makes `rho_inf = 1` the trapezoidal rule (clause
   a5) and the linear case ADR 0389's equation; the form with the internal force at the weighted displacement is the other
   published choice (midpoint-like at `rho_inf = 1`) and would not meet (a5).
3. **The load factor of (a1)** read on F-398's own loaded case (A5), with L1b and N1 beside it: taken.

## Amendment 1 (2026-11-14, written after the registered run of commit 409fbfe4 and before any corrected run)

The registered run (`cargo test -p rcfd-fem --release --test sa_nl_transient`, log `f423-scratch/run1.log`) met L1, L1b, N1,
A1, G1, G2, R0 to R2, H1, H2 and F1. Two set-ups were defective; nothing of their measures was read beyond what is written
here.

1. **P0 to P2, S0 to S2 and T1: the residual tolerance below the rounding floor.** The pendulum stopped at step 8 and the
   spinning bar at step 1 with `FemError::Solver { solver: "nl_transient" }`: after 20 iterations the relative residual stood at
   `1.354e-10` (pendulum) and `1.577e-10` (spin) with the energy ratio at `1.1e-27` and `2.7e-29`, i.e. Newton had converged to
   the rounding floor and the residual criterion `1e-10` of the driver defaults could not be met. Arithmetic 5 underestimated
   the floor. Diagnosis (a scratch test that ran the pendulum's first 40 steps with the residual tolerance `1e-6` and printed only
   the final residual of each step, `f423-scratch/diag/zz_diag_f423.rs`, log `diag1.log`; no period, energy or momentum was
   computed): the floor grows with the rotation, `1.2e-11` at step 2, `2.5e-10` at step 8, `9e-10 .. 1.75e-9` at steps 20 to 38
   (rotation about `0.27 rad` at step 38), and is proportional to the modulus (`E = 2e7`: `2.5e-11` at step 40, 70 times lower).
   It is the rounding of the internal force, whose terms scale with `E` and with `|H|` (at most 2 for a rotation by `pi`) and
   cancel to the small elastic force. Extrapolated with `|H|`: at most about `1.75e-9 x 2/0.27 = 1.3e-8` for the half-turn of the
   pendulum and a full turn of the spin.
   **Corrected set-up (P0c to P2c, S0c to S2c, T1c):** the same models, steps, methods and measures, with the Newton residual
   tolerance `1e-7` (a decade above the extrapolated floor; the energy criterion `1e-10` is kept, so each step still iterates to
   the quadratic floor: the residual criterion only stops the iteration from being refused at the floor). Limits unchanged: P0
   `>= 9`, P1 `<= 1e-2`, P2 `<= 1e-3`, S0 `Ok`, S1 `<= 1e-10`, S2 `<= 1e-6`, T1 exact. The steady spin solve (S0) keeps the
   driver defaults (it converged in the registered run). The estimate of S1 changes: a residual floor `r` of the inertia scale
   leaves a translational residual only through the rounding of `sum f_int`, so arithmetic 3 stands, but this is now the check
   most at risk. Run once. The registered tests stay as ignored records.
2. **A0, "the four end nodes equal (bits) at every step": missed.** The bits differ from step 43 on (the element sums run in
   another order for each node); diagnosis (`f423-scratch/diag/zz_diag2_f423.rs`, log `diag2.log`, the spread only): the
   largest spread of the four end displacements is `1.9e-14` of the initial `0.2 L`. A0 is recorded as a known miss (its limit,
   bits, intact). **A0b** (corrected set-up check): the spread of the four end displacements at every step at most `1e-11` of
   `0.2 L` (the class of a rounding accumulated over about `1e3` steps, b5). The other part of A0 (crossings `>= 5`, measured
   6) and A1, G1 (met) are unaffected: the period of the one-unknown oscillator is measured on one end node, and a spread of
   `1e-14` changes it by less than that.

The library gains one diagnostic field after the run, `StepReport::residual` (the final relative residual of the step's last
sub-step), used by the scratch diagnosis; no computed value changes.

## Amendment 2 (2026-11-14, written after the mutation runs and before the run of the added check)

Mutation MU6 (`in_residual_reference` ignored) was caught by no active check (`f423-scratch/mutations.log`): every active
model has prescribed equations, whose reactions enter the driver's reference `|f_int + g|` over all equations, so only a free
body needs the flag, and the one free body (S0c to S2c) is a known miss. **S3** (added, limit fixed now): the axial oscillator
mesh of D5 without any constraint (a free Hex8), started undeformed with the uniform velocity `(0.3, 0.2, 0.1) m/s` plus `10
m/s` along `x` at the four nodes of `x = L`, Newmark, `dt = T_lin/200`, 50 steps: every step converges (`Ok`) and `max |P_n -
P_0| / |P_0| <= 1e-11` (arithmetic 3: rounding only, about 50 steps; the `1e-11` class of b5). Run once, then MU6 again.
