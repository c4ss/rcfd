# ADR 0523: F-607 registration: the energy-momentum step for hyperelastic materials (the discrete gradient of Gonzalez)

Status: **REGISTERED 2026-11-14, written before any code of F-607 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-607 was written, compiled or run in the repository. The files under "What was read"
were read; the arithmetic under "Arithmetic before this registration" was done by hand and by a plain Python/numpy probe in the
scratch folder `w49i-work/f607-scratch` (`probe_dg.py` with `probe_point.log` and `cube_dg.log`, `cube_mid.log`,
`cube_avg.log`; `probe_branch.py` with `probe_branch.log`; no rcfd code; disclosed, its numbers quoted below). The bit record of
the energy-momentum path of F-570 (B0) is a test file written and run on the base 615ad811 after this commit and before any
library line (D9). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-607 (deps F-570, F-425), follow-up 3 of ADR 0514. Number 0523 given by the lead. Branch `f607-work` from main
615ad811, worktree `w49i-work/f607`, scratch `w49i-work/f607-scratch`. House rules `w49i-work/AGENT_RULES.md`: limits fixed
here, a miss becomes an ignored `*_known_miss` test with the limit intact and the number recorded, each registered run made
once, defaults bit-identical, no edit of `baseline_hashes.rs`, the hash tables or any golden file, no new dependency, no unsafe
code, clean-room. The brief overrides rule 9: the F-607 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated in a
separate last commit (the svgs are not regenerated); the proposed text is also in the addendum. Files of other agents not
touched: the contact press-fit tests (F-604), `shell_corot.rs`, `arclength.rs` and the shell paths of the nonlinear transient
module (F-585), `fatigue.rs` (F-590). The change to `nl_transient.rs` (which F-585 may also touch) is kept small and isolated
(D8).

## What F-607 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A NeoHookean spinning bar (the bar of ADR 0479 with a
> NeoHookean material of the same small-strain moduli) over ten turns, linear momentum within 1e-10, angular momentum within
> 1e-6, energy within 1e-6 relative; on an isotropic elastic model the step equals the F-570 step within 1e-12.

Clauses: (c1) the spinning bar of ADR 0514 D6 (ADR 0479 D5 and arithmetic 4: `[10, 2, 2]` Hex8, `L = 1`, `b = 0.02`, `rho =
7850`, no constraints, spin `omega = 2 pi rad/s` about the axis `z` through `c = (L/2, 0, 0)` plus the translation `V = (0.5,
0.25, 0.1) m/s`, `dt = 0.01 s`, 1000 steps = ten turns) with the neo-Hookean material of D2 whose small-strain moduli are those of
`E = 2e9`, `nu = 0.3`: `mu = E/(2 (1 + nu)) = 7.692307692e8 Pa`, `kappa = E/(3 (1 - 2 nu)) = 1.666666667e9 Pa`; (c2) `max_n |P_n -
P_0| / |P_0| <= 1e-10`; (c3) `max_n |J_n - J_0| / |J_0| <= 1e-6` (about the origin, `Integrator::momentum`); (c4) `max_n |E_n -
E_0| / |E_0| <= 1e-6`, `E = T + W` with `W` the neo-Hookean strain energy summed by `Integrator::strain_energy` at the points of
the stiffness rule; `n = 1 .. 1000`; (c5) on an isotropic elastic (St. Venant-Kirchhoff) model, the step with the discrete
gradient equals the F-570 step (ADR 0514 D1, `S_alg = (S_n + S_{n+1})/2`) within `1e-12` (made precise as I1, I2 below). Every
aggregated value is asserted finite before it enters a maximum.

## The dependency F-425 has not landed (read before the design)

F-425 (ADR 0328, the hyperelastic materials) is `todo` on main 615ad811: `Material::Hyperelastic` and `HyperelasticEnergy::
{NeoHookean, MooneyRivlin}` exist as plain data (the scaffold of ADR 0440), `Material::pending()` names ADR 0328 and every path
refuses the variant (`Material::stress`, `SolidModel::check_elements`, so `nl_transient::Integrator::new` and the static driver
with `FemError::NotYetSupported`), and `mat_hyper.rs` is the four-line placeholder. There is no neo-Hookean stress, tangent or
energy in the repository. F-607's acceptance needs one. Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation; open question 1): F-607 carries its own neo-Hookean point functions (D2), private to the energy-momentum step
and used only by it, and leaves `Material::Hyperelastic` pending on every other path and `mat_hyper.rs` untouched; when F-425
lands, the step calls F-425's functions instead, and F-607's checks are rerun as the regression (the follow-up proposed below).
The form of the energy is F-425's decision; D2 takes a published compressible form whose small-strain moduli are `mu` and
`kappa`, which is all the acceptance fixes.

## What was read (main 615ad811, read only)

ADR 0514 (whole: D1 to D8, the conservation derivation, arithmetic 1 to 4, amendments 1 and 2 including F1ec and MU4, the
addendum and its open question 3 with the discrete gradient), ADR 0479 (D5, arithmetic 3 and 4, through ADR 0514's summary),
ADR 0328 (the stub of F-425), `crates/rcfd-fem/src/nl_transient.rs` (whole), `nl_transient/emc.rs` (whole), `material.rs`
(`HyperelasticEnergy`, `Material::stress`, `state_size`), `model.rs` (`Material::hyperelastic`, `pending`, `name`, `density`,
`with_materials`, `check_supported_nonlinear`, `check_elements`), `mat_hyper.rs`, `nonlinear.rs` (`displacement_gradient`,
`green_lagrange`, `b_nonlinear`), the tests `sa_emc.rs` (the bar, `Centrifugal`, `run_spin`, `Checks`, `worst`) and
`sa_emc_default_bits.rs`, `docs/reference-data.md` (the F-570 and F-423 rows), the F-425, F-570 and F-607 entries of
`docs/roadmap.json`.

## The discrete gradient (published source, from memory, and the derivation used)

Published sources, cited from memory and NOT read (volumes, pages and years as recalled, not checked; open question 4): O.
Gonzalez, "Time integration and discrete Hamiltonian systems", J. Nonlinear Sci. 6 (1996) 449-467 (the discrete gradient of a
function); O. Gonzalez, "Exact energy and momentum conserving algorithms for general models in nonlinear elasticity", Comput.
Methods Appl. Mech. Engrg. 190 (2000) 1763-1783 (its use for a hyperelastic stored energy of `C`, with the midpoint `C`). As
remembered: the algorithmic second Piola-Kirchhoff stress `S_alg = 2 DW(C_n, C_{n+1})`, `DW = grad W(C_mid) + [(W(C_{n+1}) -
W(C_n) - grad W(C_mid) : dC) / (dC : dC)] dC`, `C_mid = (C_n + C_{n+1})/2`, `dC = C_{n+1} - C_n`, with the midpoint gradient
taken when `dC = 0`. Everything used below is derived here.

In the Green-Lagrange strain (`C = I + 2 E`, `S = dW/dE`, `dC = 2 dE`, `C_mid = I + 2 E_m`, `E_m = (E_n + E_{n+1})/2`) the same
formula reads

    S_alg = S(E_m) + (nu / (dE : dE)) dE,     nu = W(E_{n+1}) - W(E_n) - S(E_m) : dE,     dE = E_{n+1} - E_n.        (1)

**Directionality (the energy identity).** `S_alg : dE = S(E_m) : dE + nu = W(E_{n+1}) - W(E_n)`, exactly, for every `W`.
Identity 1 of ADR 0514 (`E_{n+1} - E_n = sym(F_m^T DF)`, exact) then gives `Delta u . f_alg = sum_q w (W_{n+1} - W_n)` with
`f_alg = sum_q w B_NL(F_m)^T S_alg`: the energy argument of ADR 0514 (c4) holds unchanged, `(T + W)_{n+1} - (T + W)_n = Delta u
. f_m - h v_m . C v_m + Delta u . R`.

**Momenta.** `S_alg` is symmetric (a sum of symmetric tensors), so the linear momentum (partition of unity) and the angular
momentum (`F_m S_alg F_m^T` symmetric, ADR 0514 (c3)) arguments hold unchanged. Nothing in them used the St. Venant-Kirchhoff
form.

**Second order.** For a smooth `W`, `W(E_m + dE/2) - W(E_m - dE/2) = S(E_m) : dE + W'''(E_m)[dE, dE, dE]/24 + O(|dE|^5)`, so `nu
= O(|dE|^3)` and the correction `(nu / |dE|^2) dE = O(|dE|^2)`: `S_alg = S(E_m) + O(|dE|^2)`, the midpoint rule's order. A
correction of order `|dE|` (as with `S(E_{n+1})` in place of `S(E_m)`, whose `nu` is `O(|dE|^2)`) still satisfies the identity
but is first order (mutation MU2 and check P3c).

**St. Venant-Kirchhoff (clause c5).** `W = E . D E / 2` is quadratic: `W(E_{n+1}) - W(E_n) = (E_{n+1} + E_n) . D dE / 2 = S(E_m)
: dE` exactly, so `nu = 0` and `S_alg = S(E_m) = D E_m = (D E_n + D E_{n+1})/2`, the F-570 stress. In floating point the two
differ by rounding and `nu` is rounding only (the branch of D3 removes it).

**Tensor contraction in Voigt form.** The strains of the driver are Voigt with engineering shear (`[E11, E22, E33, 2 E12, 2 E23,
2 E31]`, `green_lagrange`), the stresses Voigt with tensor components. Then `S . e` (the plain Voigt dot product) is `S : E`; `dE
: dE = de_1^2 + de_2^2 + de_3^2 + (de_4^2 + de_5^2 + de_6^2)/2`; the correction in the stress slots is `(nu / (dE : dE)) [de_1,
de_2, de_3, de_4/2, de_5/2, de_6/2]` (the tensor components of `dE`). Its work on `de` is `nu (dE : dE)/(dE : dE) = nu`.

## Arithmetic before this registration (disclosed)

1. **Point probe** (`probe_dg.py point`, `probe_point.log`; the D2 energy with `mu = 7.692e8`, `kappa = 1.667e9`; random strain
   pairs at the scales `1e-6 .. 3e-1`): `|S_alg . de - dW| / sum |S_alg,i de_i| <= 4.2e-16`; the St. Venant-Kirchhoff discrete
   gradient against the average `<= 5.3e-16` (relative to the largest component); `S` against central differences of `W` (step
   `1e-6` of the scale) `<= 9.4e-10`; the neo-Hookean stress against linear elasticity with `mu`, `kappa` at the scale `1e-6`
   (largest component): `1.03e-5`, and `9.9e-5`, `1.1e-6`, `1.1e-7` at the scales `1e-5`, `1e-7`, `1e-8` (linear in the strain,
   the finite-strain terms; that small-strain check is F-425's with its own limit, P5 here is a sanity bound).
2. **The branch `|dE| -> 0`** (`probe_branch.py`, `probe_branch.log`, `mu = 1e5`, `kappa = 1e6`, base strains of scale `0.1` and
   `1e-3`, 100 random pairs per row): with the rule of D3 the correction is taken at none of the pairs with `|dE| <= 1e-6` at
   the base `0.1` and `1e-8` at `1e-3`, at some above (`18/100` at `1e-5`, `75/100` at `1e-6` respectively) and at all from
   `1e-4`, `1e-5`; `max |S_alg - S(E_m)|_inf / |S(E_m)|_inf` is `6.3 .. 10.6 |dE|^2` at the base `0.1` and `370 .. 1354 |dE|^2`
   at `1e-3`, i.e. `0.4 .. 1.4 |dE|^2 / |E|` with `|E|` the tensor norm of the base (about `2.1` times the scale). The
   mutation MU2 (`S(E_{n+1})` in place of `S(E_m)`) gives `3.6e-4` at `(0.1, 1e-5)` and `3.4e-3` at `(1e-3, 1e-6)`: four to six
   decades above.
3. **Large-strain cube** (`probe_dg.py cube MODE`, logs `cube_*.log`): `hex_box([0;3], [1;3], [2;2;2])`, neo-Hookean `mu =
   1e5`, `kappa = 1e6`, `rho = 1000`, free, `u_0 = 0`, `v_0 = omega e_z x r + V + alpha r + gamma r_y e_x` with `r = X - (0.5,
   0.5, 0.5)`, `omega = 3`, `V = (0.3, -0.2, 0.1)`, `alpha = 5`, `gamma = 4` (rotation, translation, dilatation, shear), `dt =
   0.004 s`; the step D1 with a finite-difference Jacobian (the probe's, not the Rust tangent), residual tolerance `1e-10`.
   `E_0 = 3611.667 J` (kinetic). Largest strain change per step at a point `3.4e-2`; every point-step took the correction. At
   step 45: discrete gradient `dE = 3.83e-11`, `dP = 1.0e-14`, `dJ = 4.4e-11`; the midpoint stress without the correction
   `S(E_m)`: `dE = 1.95e-3`; the average `(S_n + S_{n+1})/2`: `dE = 3.85e-3` at step 15, `4.44e-3` at step 60. The two
   uncorrected stresses keep the momenta (`dJ ~ 5e-11`): only the energy sees the correction.
4. **The neo-Hookean bar** was not probed (the dense probe is too slow at 297 unknowns). Estimate: the start is the steady spin
   of the St. Venant-Kirchhoff bar (D7; the static driver does not take the hyperelastic material), so the neo-Hookean bar
   oscillates about its own steady state with a strain amplitude of the order of the difference of the two materials at the
   strain `~1e-5` (arithmetic 1: `~1e-5` relative), about `1e-10`; the per-step `dE` is at most that, and at the base strain
   `1e-5` the rule of D3 takes the correction from about `|dE| ~ 1e-8` (arithmetic 2 scaled): most point-steps of the bar will
   take the midpoint branch, whose energy error per point-step is `|nu| <= beta` (D3), `~1e-14` of the point energy. The
   conservation bound of ADR 0514 arithmetic 3 (the residual) then dominates: per step `|Delta E|/E_0 <= 0.5 r`, `r <= 1e-7`;
   ADR 0514 measured `1.6e-13` on the St. Venant-Kirchhoff bar.
5. **Clause c5 as a step comparison.** Starting both steps from one state, the discrete-gradient step of the isotropic material
   solves a residual that differs from F-570's by the rounding of `D E_m` against `(D E_n + D E_{n+1})/2` (`~1e-16` relative)
   with the same tangent (`D/2`, `S_alg/2`): the Newton iterates differ by `~1e-16` of each correction and the iteration count is
   the same unless a residual lies within that of the tolerance. Estimate of `|Delta u_DG - Delta u_F570|_inf / |Delta
   u_F570|_inf`: `~1e-15`. A comparison of whole runs accumulates the per-step rounding over 1000 steps of a rotation; it is a
   record (I1r), not the registered check.

## Design

### D1. The step

The step of ADR 0514 D1 unchanged (kinematics `Delta u / h = v_m`, the midpoint balance, `f_alg = sum_q w B_NL(F_m)^T S_alg`,
the loads averaged, damping `C v_m`), with `S_alg` of D3 at each point of the stiffness rule.

### D2. The neo-Hookean point functions (`nl_transient/dg.rs`, new, private)

The compressible decoupled neo-Hookean energy (J. C. Simo and R. L. Taylor, Comput. Methods Appl. Mech. Engrg. 85 (1991)
273-310; G. A. Holzapfel, Nonlinear Solid Mechanics, Wiley, 2000, chapter 6: both NOT read, from memory)

    W = (mu/2) (J^{-2/3} I_1 - 3) + (kappa/2) (J - 1)^2,     I_1 = tr C,  J = sqrt(det C),

small-strain shear modulus `mu` and bulk modulus `kappa` (at `C = I + 2E`, `|E| -> 0`: `W -> mu dev E : dev E + kappa (tr E)^2/2`).
Evaluated in `E` without cancellation at small strain (the plain form loses `eps mu` against `W ~ mu |E|^2`, a relative `2e-6`
at `|E| = 1e-5`): `t = tr E`, `q = E : E`, `i3 = det E`, `d = det C - 1 = 2t + 2(t^2 - q) + 8 i3`, `g = J^{2/3} - 1 =
expm1(ln1p(d)/3)`, `J - 1 = d/(1 + J)`; `(1 + g)^3 = 1 + d` exactly gives `I_1 - 3 J^{2/3} = 2t - 3g = 2q - 2t^2 - 8 i3 + 3g^2 +
g^3`, and

    W = (mu/2) (2q - 2t^2 - 8 i3 + 3 g^2 + g^3)/(1 + g) + (kappa/2) (J - 1)^2,
    S = mu (1 + g)^{-1} (2 dev E) C^{-1} + kappa (J - 1) J C^{-1},     dev E = E - (t/3) I,

(`S = 2 dW/dC`: `2 d/dC [J^{-2/3} I_1] = J^{-2/3} (2 I - (2/3) I_1 C^{-1})` and `(C - (I_1/3) I) = 2 dev E`; `2 d/dC (J - 1)^2/2 =
(J - 1) J C^{-1}`), symmetrised (`(A + A^T)/2`) before it is written in Voigt form; `C^{-1}` by the adjugate over `det C = 1 + d`.
The magnitude of the terms of `W` (for D3) `m(W) = (mu/2) (2q + 2t^2 + 8|i3| + 3g^2 + |g|^3)/(1 + g) + (kappa/2) (J - 1)^2`. The
tangent `CC = 2 dS/dC` (fourth order, in the Voigt layout of the driver's `D`: `S_I = D_IJ e_J`)

    CC = -(2 mu j/3) [I (x) C^{-1} + C^{-1} (x) I - (I_1/3) C^{-1} (x) C^{-1} - I_1 II_{C^{-1}}]
         + kappa (2J^2 - J) C^{-1} (x) C^{-1} - 2 kappa (J^2 - J) II_{C^{-1}},

`j = J^{-2/3}`, `(II_{C^{-1}})_{ijkl} = (C^{-1}_ik C^{-1}_jl + C^{-1}_il C^{-1}_jk)/2`; at `C = I`: `2 mu II - (2 mu/3) I (x) I +
kappa I (x) I`, the isotropic `D` of `lambda = kappa - 2 mu/3` (derived here; checked by P4). A point with `det C <= 0` or `1 + d
<= 0` is the located inversion error of the step (as ADR 0514 D2). Mooney-Rivlin is not implemented (refused, D6).

### D3. The algorithmic stress and its branch at `|dE| -> 0`

At a point with `E_n`, `E_{n+1}` (Voigt): `E_m = (E_n + E_{n+1})/2`, `de = E_{n+1} - E_n`, `n2 = dE : dE` (tensor form above),
`S_m = S(E_m)`, `nu = W(E_{n+1}) - W(E_n) - S_m . de`, and the rounding bound `beta = 64 eps (m(W_n) + m(W_{n+1}) + sum_i |S_m,i
de_i|)`, `eps = f64::EPSILON`. Then

- **correction branch**: if `sqrt(n2) > delta` and `|nu| > beta`: `S_alg = S_m + (nu/n2) [de_1, de_2, de_3, de_4/2, de_5/2,
  de_6/2]`;
- **midpoint branch**: otherwise `S_alg = S_m`.

`delta = 1e-12` (a strain): below it the true `|nu| <= |W'''| delta^3/24`, below any energy the step can resolve (`mu delta^3 <
1e-27 Pa` for `mu <= 1e9`), and the division is guarded against an underflowing `n2`. The rounding test: `nu` is computed from
three quantities whose rounding is bounded by a few `eps` times their term magnitudes; when `|nu|` is within `64 eps` of them it
is rounding and the correction `nu/|dE|` would be noise of size `beta/|dE|` (for St. Venant-Kirchhoff always so, `nu = 0`
exactly). The energy error of the midpoint branch is `|nu| <= beta` per point-step, at rounding. Both branches give a symmetric
`S_alg`, so the momenta are exact on either. The tangent of the step: `(2/h^2) M + C/h + sum_q w [B_m^T (CC(E_m)/2) B_m + G^T
(S_alg/2) G]` (ADR 0514 D2 with `CC(E_m)` for `D`; the derivative of the correction is left out, a symmetric approximate
tangent as F-570's).

For the isotropic elastic material in the discrete-gradient mode (D4) the same formula with `S(E) = D E` from
`Material::stress`, `W = S(E) . E/2`, `m(W) = sum_i |S_i e_i|/2`.

### D4. The option (opt-in)

`nl_transient::AlgorithmicStress { Auto, DiscreteGradient, Average }` (`#[non_exhaustive]`, default `Auto`),
`NlTransientOptions::stress` and the builder `NlTransientOptions::algorithmic_stress(..)`, read by the energy-momentum step only:

- `Auto`: the isotropic elastic material takes the F-570 average (ADR 0514 D1, the same code and bits); the neo-Hookean material
  takes the discrete gradient of D3.
- `DiscreteGradient`: D3 for every material of the step (the isotropic one too: the check of clause c5).
- `Average`: `(S(E_n) + S(E_{n+1}))/2` for every material, with the tangent `CC(E_{n+1})/2` for the neo-Hookean one (the F-570
  stress; for the neo-Hookean material it does not conserve the energy: the negative control N1h).

The `Method` step ignores the option. It is not in the state bytes (like the Newton tolerances, it is an option of the solve, not
of the state): a state saved with one mode restores into an integrator of another.

### D5. The material in the integrator

`Integrator::new` with `StepKind::EnergyMomentum` and a model with a `Material::Hyperelastic` of the neo-Hookean energy builds the
integrator of the surrogate model (`SolidModel::with_materials`: each neo-Hookean material replaced by the isotropic elastic
material of its small-strain moduli, `E = 9 kappa mu/(3 kappa + mu)`, `nu = (3 kappa - 2 mu)/(2 (3 kappa + mu))`, the same
density, no expansion), so that every check of `Integrator::new` (elements, supports, the driver's model checks, the F-570
refusals of ADR 0514 D4), the consistent mass and the damping tangent `K_0` (the neo-Hookean tangent at `u = 0` is that `D`) come
from the existing code; it then holds the original model and marks it hyperelastic. On that integrator: the step's point
functions are D2 and D3; the internal force of the state (start, and after each step) is `sum_q w B_NL(F)^T S(E)` with the D2
stress (the step's element routine with `u_0 = u_1`, where `S_alg = S(E)` exactly, the midpoint branch); `strain_energy` sums the
D2 energy for a hyperelastic material (the energy of every other material as before). Every other path keeps refusing the
hyperelastic material (it stays pending for F-425): the `Method` step, the static driver, the linear paths.

### D6. Refusals (key `"nl_transient"`, reason naming ADR 0523)

With the energy-momentum step: a Mooney-Rivlin material (`Integrator::new`). Unchanged: the refusals of ADR 0514 D4 (reached
through the surrogate), the `Method` step's refusal of the hyperelastic material (`NotYetSupported`, ADR 0328).

### D7. The models and runs

- **NH bar (A0 to A3, R1h)**: clause c1. The steady start `u_0`, `v_0` of ADR 0514 D6 computed on the St. Venant-Kirchhoff bar
  (the `Centrifugal` contribution and `solve_with`, driver defaults), then the neo-Hookean model on the same mesh started from it.
  `NlTransientOptions::new(0.01).energy_momentum()` with the Newton options of ADR 0514 arithmetic 2 (residual `1e-7`, energy
  `1e-10`, 20 iterations, 5 halvings), stress `Auto`.
- **Isotropic bar (I1, I1r)**: the F-570 bar of ADR 0514 D6, the same options; the `Auto` run (the F-570 step) and at every step
  `n = 0 .. 999` one `DiscreteGradient` step from the `Auto` state `n` (`restore` into a second integrator).
- **Cube (K0 to K3, N1h, I2, T1h)**: arithmetic 3 (the model, the start), `dt = 0.004 s`, 500 steps (two seconds: about 0.95 of a
  turn and several periods of the lowest modes), the driver's Newton defaults (`NewtonOptions::new(1)`: residual `1e-10`, energy
  `1e-10`, 20 iterations, 5 halvings). The isotropic cube of I2: the same mesh and start with the isotropic elastic material of
  the same small-strain moduli (`E = 2.903e5`, `nu = 0.4516`).

### D8. Files

`crates/rcfd-fem/src/nl_transient/dg.rs` (new: D2, D3, the surrogate, unit tests P1 to P6), `crates/rcfd-fem/src/nl_transient/
emc.rs` (the dispatch of D3/D4 in the element routine, the hyperelastic internal force, the Mooney-Rivlin refusal),
`crates/rcfd-fem/src/nl_transient.rs` (the option and the field, the surrogate in `new`, the internal force at the start, the
hyperelastic branch of `strain_energy`: a few lines each, no change of the `Method` step), `crates/rcfd-fem/tests/sa_dg.rs`
(new: the checks), `crates/rcfd-fem/tests/sa_dg_default_bits.rs` (new: B0). `lib.rs`, `material.rs`, `model.rs`, `mat_hyper.rs`
unchanged.

### D9. The bit record of the energy-momentum path (B0)

`crates/rcfd-fem/tests/sa_dg_default_bits.rs`, written after this commit and run on the unchanged base 615ad811 before any
library line; its digests (FNV-1a 64) written into the file then. Content, every path of the energy-momentum step that D3 to D5
touch: the F-570 spinning bar, 50 steps (state bytes after each 10, the energies and momenta at the end); the F-570 pendulum
(ADR 0514 D6: gravity, the pin) with `Rayleigh { mass: 0.5, stiffness: 1e-5 }`, 30 steps; a split run of the bar (15 + bytes +
restore + 15). `sa_emc_default_bits.rs`, `sa_nl_transient_default_bits.rs`, `sa_emc.rs` and `sa_nl_transient.rs` stay as they
are and must pass.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| A0 | c1 (set-up) | the steady St. Venant-Kirchhoff solve converges; the NH integrator is built; 1000 steps converge (`Ok`), all measures finite | `Ok` | arithmetic 4 |
| A1 | c2 | NH bar `max_n |P_n - P_0| / |P_0|` | `<= 1e-10` | ADR 0514: `1.8e-13`; rounding |
| A2 | c3 | NH bar `max_n |J_n - J_0| / |J_0|` | `<= 1e-6` | ADR 0514: `6.6e-13`; arithmetic 4 |
| A3 | c4 | NH bar `max_n |E_n - E_0| / |E_0|`, `E = T + W_NH` | `<= 1e-6` | arithmetic 4: `0.5 r` per step bound; ADR 0514 measured `1.6e-13` |
| I1 | c5 | isotropic bar, at every step `n = 0 .. 999`: `|Delta u_DG - Delta u_570|_inf / |Delta u_570|_inf` and `|v_DG - v_570|_inf / |v_570|_inf`, the larger, maxed over `n` | `<= 1e-12` | arithmetic 5: `~1e-15` |
| I1r | c5 (record) | the same two models run 1000 steps each from the same start: `max_n |u_DG - u_570|_inf / |u_570|_inf` | record | arithmetic 5 |
| I2 | c5 | isotropic cube, the I1 comparison at every step of 500 (large strain changes, the rounding branch for St. Venant-Kirchhoff) | `<= 1e-12` | arithmetic 1: `5.3e-16` at a point |
| K0 | set-up | NH cube: 500 steps converge, all measures finite | `Ok` | arithmetic 3 |
| K1 | energy | NH cube `max_n |E_n - E_0| / |E_0|` | `<= 1e-7` | arithmetic 3: `3.8e-11` at step 45; bound per step `|Delta u| |R| / E_0 ~ 3e-11` at `r = 1e-10`, 500 steps of one sign `1.5e-8` |
| K2 | momenta | NH cube `max |P - P_0|/|P_0|`, `max |J - J_0|/|J_0|` | `<= 1e-10`, `<= 1e-8` | arithmetic 3: `1.0e-14`, `4.4e-11` |
| N1h | negative control | NH cube with `Average`, the steps that converge (at most 500, at least 15 required): `max_n |E_n - E_0| / |E_0|` | `>= 1e-4` | arithmetic 3: `3.85e-3` at step 15 |
| P1 | point (D3) | NH, 200 random pairs at each scale `1e-6, 1e-4, 1e-2, 1e-1` (base strain, change `0.3` of it): `|S_alg . de - (W_{n+1} - W_n)| / (sum_i |S_alg,i de_i| + m(W_n) + m(W_{n+1}))` | `<= 1e-12` | arithmetic 1: `4.2e-16`; the midpoint branch at most `64 eps` |
| P2 | point (c5) | isotropic, the same pairs: `|S_alg,DG - (S_n + S_{n+1})/2|_inf / |(S_n + S_{n+1})/2|_inf` | `<= 1e-12` | arithmetic 1: `5.3e-16` |
| P3a | branch | `de = 0` exactly, and `|dE| = 1e-13 < delta`: `S_alg` equals `S(E_m)` bit for bit (NH, base strains `0.1`) | exact | D3 |
| P3b | branch | NH, base `0.1`, `|dE| = 1e-8` (above `delta`, `nu` rounding): `S_alg` equals `S(E_m)` bit for bit at the 100 pairs | exact | arithmetic 2: corrected 0/100 at `1e-8` and `1e-6` |
| P3c | branch (second order) | NH, `|dE|` in `{1e-5, 1e-4, 1e-3}` at base `0.1` and `{1e-6, 1e-5, 1e-4}` at `1e-3`, 100 pairs each: `max |S_alg - S(E_m)|_inf / |S(E_m)|_inf / (|dE|^2 / |E_m|)` | `<= 5` | arithmetic 2: `0.4 .. 1.4` |
| P4 | tangent | NH `D(E)` against central differences of `S` (step `1e-7` of the scale) at 50 strains of scales `1e-3` and `0.1`: largest entry difference over the largest entry | `<= 1e-6` | derivation; central differences `~1e-9` |
| P5 | small strain | NH stress at random strains of scale `1e-6` against the isotropic `D` of `mu`, `kappa`: largest component difference over the largest component | `<= 1e-4` | arithmetic 1: `1.03e-5` |
| P6 | stress | NH `S` against central differences of `W` (step `1e-6` of the scale) at 50 strains of scales `1e-3`, `0.1` | `<= 1e-6` | arithmetic 1: `9.4e-10` |
| R1h | state | NH bar, 60 steps against 30 + bytes + restore + 30: equal state bytes | exact | D5 (no new state) |
| T1h | threads | NH cube, 50 steps with 1 and 4 threads: equal state bytes | exact | the assembly of ADR 0514 D2 |
| F1h | refusals | with the energy-momentum step: Mooney-Rivlin refused (key `"nl_transient"`, reason contains `ADR 0523`); the NH material with the `Method` step refused (`NotYetSupported`); the NH material in a BBar Hex8 block refused (key `"nl_transient"`, `ADR 0514`) | all | D5, D6 |
| B1 | defaults | B0: every digest of base 615ad811; `sa_emc_default_bits.rs`, `sa_nl_transient_default_bits.rs`, `sa_emc.rs`, `sa_nl_transient.rs` pass | exact; pass | D9 |
| B2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output of a case changed |

In P3c `|E_m|` is the tensor norm and the base scale the standard deviation of the random components. A check that would take more than about two minutes in release is an ignored
heavy record run once, with a fast test on the same code path.

## Negative controls and mutations

Negative control N1h (the F-570 average on the neo-Hookean cube). Planned mutations, on a scratch copy with its own target, one at
a time:

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the correction dropped (`S_alg = S(E_m)` always) | K1, P1 |
| MU2 | `S(E_{n+1})` in place of `S(E_m)` in (1) (still directional) | P3c, P2, I2 |
| MU3 | `B_NL` at `F_{n+1}` in the force of the hyperelastic path | A2 or K2, K1 |
| MU4 | the correction direction with engineering shear (`de_4..6` not halved) while `n2` keeps the tensor form | K1, P1 |
| MU5 | `delta = 1e-1` (the midpoint branch almost always) | K1, P3c |
| MU6 | `strain_energy` of the hyperelastic material as `S . E/2` | A3 or K1 |
| MU7 | the volumetric stress without the factor `J` (`kappa (J - 1) C^{-1}`) | P6 (the step itself conserves the energy of `W` whatever `S` is) |
| MU8 | the tangent without the `II_{C^{-1}}` terms | P4 |
| MU9 | the Mooney-Rivlin refusal removed | F1h |
| MU10 | the rounding test removed (`beta = 0`) | I2 or P3b; recorded, not required (the noise is bounded by D3's `delta`) |

## Decision rule

- **done**: every check of the table met (I1r is a record), MU1 to MU9 caught by at least one check, fmt and clippy clean, B1
  and B2 unchanged.
- **partial**: B1 and B2 unchanged, A1 to A3 and I1 met, and a miss of a companion check diagnosed and recorded as a
  `*_known_miss` test, or a mutation of MU1 to MU9 uncaught. Also partial if the lead rules that a step-local neo-Hookean energy
  does not meet "the hyperelastic material of F-425" (open question 1).
- **todo**: otherwise (in particular any change of the default bits or of the F-570 bits of B0).

## Limits (not covered)

- The neo-Hookean energy is F-607's own (D2), used by the energy-momentum step only; F-425's material, its Mooney-Rivlin form,
  the mean-dilatation element and the static driver are not part of this item.
- No Mooney-Rivlin, no other hyperelastic energy, no orthotropic or tabulated material, no F-bar Hex8, no shells, no
  contributions in the step (as ADR 0514).
- The tangent leaves out the derivative of the correction (symmetric approximate tangent); large strain changes per step
  converge linearly.
- The neo-Hookean bar starts from the St. Venant-Kirchhoff steady state (D7), not its own.
- No case-file key (open question 2).

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; none changes a default)

1. **F-425 not landed.** Options: (a) stop F-607 until F-425; (b) a step-local neo-Hookean energy (D2) used by this step only,
   replaced by F-425's when it lands. Recommendation (b), taken; F-425's registration should either adopt D2's form and
   evaluation or record that the energy-momentum step switches to its own, and rerun `sa_dg.rs`.
2. **Case-file key.** Not added (brief). Recommendation: with the key of ADR 0514 open question 1 (`step = "energy_momentum"`),
   `algorithmic_stress = "auto" | "discrete_gradient" | "average"` (default `"auto"`), refused unless the energy-momentum step is
   chosen.
3. **Mooney-Rivlin.** Recommendation: once F-425 gives its energy, the same D3 applies unchanged (it needs only `W`, `S`, `m(W)`
   and a tangent); refused until then.
4. **Citations.** The two Gonzalez papers, Simo and Taylor 1991 and Holzapfel 2000 are cited from memory, NOT read; the volumes
   and pages are as recalled and not checked. Recommendation: the maintainer checks them against a library copy; nothing in the
   design depends on them (D2, D3 are derived here).

## Amendment 1 (2026-11-14, written after the library code and the run of the point checks P1 to P6, before any run of the model checks)

Nothing of the model checks (A, I, K, N, R, T, F, B) has been run. The point checks P1 to P6 (unit tests of `dg.rs`, which do
not depend on this amendment) ran once (`f607-scratch/run1_unit.log`; the interleaved output of the parallel test threads hid
the P6 line, so the same deterministic tests were run a second time with one thread, `run1_unit_serial.log`, the same
numbers; disclosed).

1. **The neo-Hookean material is accepted only on request (D4, D5 corrected).** While writing D5 the existing test F1ec of ADR
   0514 amendment 1 (`sa_emc.rs`, `f1ec_refusals`) was read: it requires `Integrator::new` with the energy-momentum step and
   the default options to refuse a neo-Hookean model. With D4 as registered (`Auto` takes the discrete gradient for the
   neo-Hookean material) that existing test would fail, and existing tests are not edited. Corrected design: `Auto` (the
   default) is the F-570 step in every respect, including its refusal of the neo-Hookean material (the `NotYetSupported` of
   ADR 0328, unchanged); the neo-Hookean material is accepted, through the surrogate of D5, only with
   `AlgorithmicStress::DiscreteGradient` (the discrete gradient) or `AlgorithmicStress::Average` (the negative control). Every
   neo-Hookean run of D7 (the bar of A0 to A3 and R1h, the cube of K0 to K2 and T1h) uses `DiscreteGradient`; N1h uses
   `Average`; I1 and I2 compare `DiscreteGradient` with `Auto` as registered. Added to F1h (limit of the same kind, all): the
   neo-Hookean bar model with the energy-momentum step and `Auto` refused (any error), and accepted with `DiscreteGradient`.
   The decision rule is unchanged.
2. **The estimate of P3c (arithmetic 2) was misreckoned**, the limit is not changed: with `|E|` about `2.1` times the scale,
   the measured `6.3 .. 10.6 |dE|^2` at the base `0.1` is `1.3 .. 2.2 |dE|^2/|E|` and `370 .. 1354 |dE|^2` at `1e-3` is `0.8 ..
   2.8 |dE|^2/|E|` (not `0.4 .. 1.4`); the registered limit `5` stays. P3c ran before this amendment: `1.19`.
