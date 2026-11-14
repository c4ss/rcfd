# ADR 0475: F-422 registration: J2 plasticity with linear isotropic and kinematic hardening, radial return and consistent tangent, on the Newton driver with history at the integration points

Status: **REGISTERED 2026-11-14, written before any code of F-422 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-422 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f422-scratch/probe.py`, log `probe.log`; no rcfd code; disclosed, its numbers quoted below). The bit record B0 of the
default paths is a test file written and run on the base after this commit and before any library code (D7). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Registration of the stub [ADR 0325](0325-j2-plasticity-radial-return.md) (work package SA-15 of the plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-422 (deps F-408, F-389, F-397). Number 0475 given by
the lead. Branch `f422-work` from main 4abff5fe, worktree `w49i-work/f422`, scratch `w49i-work/f422-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except the records-and-copies scheme of ADR 0291 in `sa_scaffold.rs`; clean-room.

## What F-422 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6 in the elastic and in the plastic branch. Uniaxial bar, bilinear: the stress follows sigma_y + (E H/(E + H)) (eps - sigma_y/E) within 1e-10 in one step and in 100; on reversal the bar yields again at -sigma_max with isotropic and at sigma_max - 2 sigma_y with kinematic hardening within 1e-10. Thick sphere b/a = 2, perfectly plastic, internal pressure: the pressure at plastic radius c equals 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3) within 1 percent at c = 1.25 a and 1.5 a, the solve converges at 0.98 of the limit pressure 2 sigma_y ln(b/a), and the residual hoop stress at the bore after unloading equals the closed form within 2 percent. Beam of rectangular section in pure bending: M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2 within 1 percent up to kappa = 5 kappa_y. The last three residuals of each step show an order >= 1.8; a halved step restores the state bit for bit; a non-positive yield stress and a softening curve are refused with the key.

The brief adds: (b1) J2 is a material of the existing nonlinear static path (no second Newton loop), with history variables at
the integration points; (b2) the beam model chosen and registered, the simplest that tests the material honestly; (b3) models
without the new material keep their bits, a default-bits test records digests at 4abff5fe before any library change; (b4) a
case-file key is the maintainer's call (recommended, not added); (b5) probe a quantity before a `1e-12`-class limit; (b6) the
records-and-copies scheme of ADR 0291 for the retired pending rows of `sa_scaffold.rs`.

Clauses: (a0) every limit registered here; (a1) tangent against central differences, elastic and plastic branch, `1e-6`; (a2)
uniaxial bilinear bar in one step and in 100, `1e-10`; (a3) reversal at `-sigma_max` (isotropic) and `sigma_max - 2 sigma_y`
(kinematic), `1e-10`; (a4) sphere, pressure at the plastic radius `c = 1.25 a` and `1.5 a`, 1 percent; (a5) the solve converges
at `0.98 p_lim`; (a6) residual hoop stress at the bore after unloading, 2 percent; (a7) pure bending `M/M_y` up to `5 kappa_y`, 1
percent; (a8) the last three residuals of each step show an order `>= 1.8`; (a9) a halved step restores the state bit for bit;
(a10) a non-positive yield stress and a softening curve refused with the key.

Readings, registered:

- **(a1)** At two levels. T1, the material point: the tangent `C` of the return map against central differences of its stress
  in the six strains (the committed state frozen), for four hardening sets (isotropic only, kinematic only, mixed, perfectly
  plastic), each in an elastic and in a plastic branch; T2, the element: the tangent of `nonlinear::element_tangent` (material
  part plus geometric part) against central differences of `nonlinear::element_internal_force` on a distorted Hex8 and a
  distorted Hex20 with a J2 material, in an elastic state and in a plastic state with a non-zero committed history. Measure
  `max |C - C_fd| / max |C|` (entry-wise maximum).
- **(a2)** The "uniaxial bar" at the material point, U1: the strain `eps_xx` is given and the five other strains are solved such
  that the five other stresses vanish (mixed control, Newton with the consistent tangent, to `|sigma_other| <= 1e-16 |sigma_xx|`
  or 30 iterations). "In one step": one increment from the virgin state to `eps_max = 5e-3`; "in 100": 100 equal increments, the
  stress of every increment compared with `E eps` (elastic) or the bilinear form. `H` of the acceptance is the uniaxial plastic
  modulus, `H = H_iso + H_kin` (arithmetic 2). Both hardening kinds (`H_iso = 2e9, H_kin = 0` and `H_iso = 0, H_kin = 2e9`).
  Measure `|sigma - sigma_closed| / sigma_closed(eps_max)`. A companion U4 at the model level (D6): one Hex8 bar stretched
  under displacement control through `nonlinear::solve`, in 1 step and in 100, its second Piola-Kirchhoff `S_xx` at every
  point against the bilinear form of the Green-Lagrange `E_xx` (the law of D1 applied to the measures of the total Lagrangian
  driver, exact for a homogeneous uniaxial state), limit `1e-8` (the Newton residual tolerance is `1e-10`).
- **(a3)** U2, U3, at the material point: after the 100 increments to `eps_max` the strain is reversed; the re-yield stress is
  measured from computed points only, as the intersection of the computed unloading line (through the computed points at
  `eps_max` and `eps_max - 1e-3`, elastic) with the computed reversed plastic line (through the computed points at
  `eps_r - 2e-3` and `eps_r - 4e-3`, `eps_r` the closed-form re-yield strain, each reached from the state at `eps_max` in one
  increment). Measure `|sigma_ry - sigma_closed| / |sigma_closed|`.
- **(a4)** S1, S2: the sphere of D5 loaded by the bore pressure `p(c)` of the closed form (arithmetic 3) for `c = 1.25 a` and
  `1.5 a`. The plastic radius of the computed solution `c_FE` is read from the elastic shell: in the elastic region of a partly
  plastic sphere `sigma_theta - sigma_r = sigma_y (c/r)^3` (arithmetic 3), so each integration point that has not yielded
  (equivalent plastic strain exactly 0) gives `c_i = r_i (sigma_eq,i / sigma_y)^(1/3)`, `c_FE` their mean. Measure
  `|p(c_FE) - p| / p`, limit `1e-2`. Reading the boundary between yielded and elastic points directly is limited by the point
  spacing (`0.04 a`, i.e. 2.4 percent of `p`): it is the companion C1 (every point with `r < c - 0.05 a` has yielded and every
  point with `r > c + 0.05 a` has not).
- **(a5)** S3: `nonlinear::solve` of the sphere at `0.98 p_lim` returns `Ok` (D5 steps and the driver's default halvings).
- **(a6)** S4: the sphere loaded to `p1 = p(1.25 a)` and unloaded to zero in one solve (D5), the hoop stress
  `(tr S - e_r . S e_r)/2` of every point of the innermost element layer fitted by least squares with a quadratic in `r` and
  evaluated at `r = a`, against `sigma_theta,res(a) = sigma_y - p1 - (5/7) p1 = -0.628903 sigma_y` (arithmetic 4). Measure
  relative, limit `2e-2`. The fit alone, on the exact field at the three Gauss radii of the layer, is off by `4.7e-3`
  (arithmetic 4).
- **(a7)** P1: the narrow beam of D6 in pure bending by a prescribed rotation of its end section; `M` from the reactions of the
  end section, `M_y = sigma_y w h^2 / 6`, `kappa_y = 2 sigma_y / (E h)`; at `kappa/kappa_y = 1.25, 1.5, 2, 3, 4, 5`, each its own
  solve. Measure `|M/M_y - (1.5 - 0.5 (kappa_y/kappa)^2)| / (1.5 - 0.5 (kappa_y/kappa)^2)`, limit `1e-2`. Companion C2: the
  elastic `kappa = 0.5 kappa_y`, `M/M_y = 0.5`, same limit.
- **(a8)** O1: the order `ln(r_k/r_{k-1}) / ln(r_{k-1}/r_{k-2})` of the last three residuals above the rounding floor `1e-12`
  (the reading of ADR 0404 for F-397), on every converged increment of S1, S2, S3 and P1; an increment with fewer than three
  residuals above `1e-12` counts as a miss of O1.
- **(a9)** H1: the beam of P1 at `kappa = 3 kappa_y` solved (A) in one load step with a driver contribution of zero force that
  refuses its third call once (the first attempt 0 to 1 fails after two plastic iterations; the driver halves to 0.5 and goes on
  to 1), and (B) in two load steps with the same contribution never refusing. Pass: A has exactly one halving (set-up check),
  and A and B give the same bits of the displacement, the reactions, every state value, and the load factors, residuals and
  energies of their increments.
- **(a10)** R1: `Material::j2` with `yield_stress = 0`, `-2e8` and NaN refused with `FemError::InvalidMaterial { key:
  "yield_stress" }`; a softening curve, `isotropic = -1e9` and `kinematic = -1e9`, refused with the keys `"isotropic"` and
  `"kinematic"`. The hardening here is linear (D1), so a softening curve is a negative modulus; a tabulated curve is a follow-up
  (Limits).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0325 (stub), ADR 0310 (sections on SA-15, the module table, the order of the packages),
  ADR 0440 (D2 the variants, D3 the state layout, D5 the refusals, D7 the extension points), ADR 0404 by its code;
  ADR 0470 for the form of a registration.
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `material.rs` (whole from line 255), `model.rs` (the `Material` enum, `j2`,
  `pending`, `check_elements` and the `check_*` family), `mat_j2.rs`, `mat_visco.rs`, `mat_creep.rs`, `mat_ortho.rs` (head and
  `check_model`), `element.rs` (`ElementState`, `IntegrationPoint`, `PointGeometry`), `hex20.rs` (upgrade, face loads),
  `builders.rs` (`hex_box`), `constraint.rs` (`Prescribed`).
- Tests: `sa_scaffold.rs` (the pending variants, `m1_to_m4_after_f409`, `x1_modules_and_pending_after_f430`, `entries`, the
  records), `si_nonlinear.rs` (the order of A4), `sa_shell4_membrane_bits.rs` (the digest of a bit record).

## Arithmetic done before the code (disclosed)

1. **Radial return** (first principles; the algorithm of Simo and Taylor 1985 and Simo and Hughes, Computational
   Inelasticity, 1998, chapter 3, cited from memory, NOT re-read; every formula below derived here and checked by the prototype
   against central differences). Elastic `lambda, mu`, `kappa = lambda + 2 mu/3`. State: plastic strain `eps_p`, back stress
   `beta` (deviatoric), equivalent plastic strain `alpha`. Trial: `sigma_tr = D (eps - eps_p)`, `xi_tr = dev sigma_tr - beta`,
   `f = |xi_tr| - sqrt(2/3) (sigma_y + H_iso alpha)`. If `f <= 0` the step is elastic: stress `D (eps - eps_p)`, tangent `D`,
   state unchanged. Else, `n = xi_tr / |xi_tr|`, flow `eps_p += dg n`, `beta += (2/3) H_kin dg n`, `alpha += sqrt(2/3) dg`. The
   consistency `|xi| = sqrt(2/3) K(alpha)` with `|xi| = |xi_tr| - (2 mu + (2/3) H_kin) dg` and `K = sigma_y + H_iso (alpha +
   sqrt(2/3) dg)` is linear in `dg`: `dg = f / (2 mu + (2/3)(H_iso + H_kin))`, exact (no local iteration). Stress `sigma =
   sigma_tr - 2 mu dg n`.
2. **Consistent tangent.** `d dg = 2 mu n : d eps / (2 mu + (2/3) H)`, `H = H_iso + H_kin`; `d n = (2 mu / |xi_tr|) (I_dev - n
   (x) n) d eps`. Hence `C = kappa 1 (x) 1 + 2 mu theta I_dev - 2 mu theta_bar n (x) n`, `theta = 1 - 2 mu dg / |xi_tr|`,
   `theta_bar = 1/(1 + H/(3 mu)) - (1 - theta)`. Voigt: stress with tensor shear, strain with engineering shear, so `I_dev =
   diag(1, 1, 1, 1/2, 1/2, 1/2) - (1/3) 1 (x) 1` and the column of `n (x) n` for a shear strain is the tensor component of `n`.
   Prototype against central differences (`h = 1e-9`, random states, seed `0xF422`): elastic branches `1.2e-11 .. 2.3e-11`,
   plastic `2.1e-10 .. 6.9e-10` (four hardening sets). The limit `1e-6` of (a1) is three decades above.
   **Uniaxial reduction.** For `sigma = diag(s, 0, 0)` and the deviatoric `beta = diag(1, -1/2, -1/2) beta_xx`, the yield condition reads `|s -
   (3/2) beta_xx| = sigma_y + H_iso alpha`, with `(3/2) beta_xx = H_kin eps_p,xx` and `alpha = |eps_p,xx|` on a monotone path:
   the uniaxial plastic modulus is `H = H_iso + H_kin` and the tangent `E H/(E + H)`. Reversal: isotropic, the elastic range
   grows to `[-sigma_max, sigma_max]`; kinematic, it is `[H eps_p - sigma_y, H eps_p + sigma_y]`, re-yield at `sigma_max - 2
   sigma_y`.
   Prototype U1 to U3 (`E = 2e11, nu = 0.3, sigma_y = 2e8, H = 2e9, eps_max = 5e-3`): one step `1.0e-15`, 100 steps `2.9e-16`,
   `sigma_max = 2.079208e8` (closed form the same), re-yield isotropic `-2.079207921e8` (rel `1.4e-15`), kinematic
   `-1.920792079e8` (rel `1.2e-15`). The limits `1e-10` of (a2), (a3) are five decades above the probe (b5). Backward Euler is
   exact here because the hardening is linear and the flow direction fixed within a plastic increment.
3. **The partly plastic sphere** (first principles; the result as in Hill, The Mathematical Theory of Plasticity, 1950, chapter
   5, from memory, NOT read; re-derived). Spherical symmetry gives `sigma_theta = sigma_phi`, so Tresca and von Mises coincide:
   `sigma_eq = |sigma_theta - sigma_r|`. Plastic zone `a < r < c`, perfectly plastic: equilibrium `d sigma_r/dr = 2 (sigma_theta
   - sigma_r)/r = 2 sigma_y / r`, so `sigma_r = -p + 2 sigma_y ln(r/a)`. Elastic shell `c < r < b`, Lame: `sigma_r = -A (b^3/r^3
   - 1)`, `sigma_theta = A (b^3/(2 r^3) + 1)`, `sigma_theta - sigma_r = (3/2) A b^3/r^3`; yield at `r = c`: `A = (2/3) sigma_y
   c^3/b^3`, so in the elastic shell `sigma_theta - sigma_r = sigma_y (c/r)^3` (the reading of (a4)), and `sigma_r(c) = -(2/3)
   sigma_y (1 - c^3/b^3)`. Continuity of `sigma_r` at `c`: `p = 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3)` (the acceptance).
   With `a = 1, b = 2`: first yield `p_e = 0.583333 sigma_y`, `p(1.25) = 0.950193 sigma_y` (`dp/dc = 1.2094`), `p(1.5) =
   1.196347 sigma_y` (`dp/dc = 0.7708`), `p_lim = 2 ln 2 = 1.386294 sigma_y`, `0.98 p_lim = 1.358568 sigma_y` at `c = 1.80789`.
   Sensitivity of the measure: an error `delta` in `sigma_eq` at an elastic point gives `delta/3` in `c_i` and `0.53 delta` in
   `p(c_FE)/p` at `c = 1.25`; a 1 percent limit tolerates a 2 percent error of the elastic stresses.
4. **Residual stress after unloading** from `p1 = p(1.25 a)`. Elastic unloading changes the stresses by the Lame field of `-p1`:
   for `b/a = 2`, `sigma_theta,el(r) = (p1/7)(1 + 4 a^3/r^3)`, `sigma_theta - sigma_r = (12/7) p1 a^3/r^3`. Residual at the bore
   `sigma_theta = sigma_y - p1 - (5/7) p1 = sigma_y - (12/7) p1 = -0.628903 sigma_y` (`sigma_r = 0`). Reversed yielding would
   need `(12/7) p1 > 2 sigma_y`, i.e. `p1 > 1.1667 sigma_y`: from `p(1.25)` the unloading is elastic (the residual `|sigma_theta -
   sigma_r|` at the bore is `0.629 sigma_y < sigma_y`); from `p(1.5) = 1.196` it is not, which is why S4 unloads from `p(1.25)`.
   The quadratic through the three Gauss radii `1.02818, 1.125, 1.22182` of the layer `[1, 1.25]` of the exact residual field
   `sigma_y - p1 + 2 sigma_y ln r - (p1/7)(1 + 4/r^3)` gives `-0.625958` at `r = 1` (`4.7e-3` low): the fit's own share of the
   2 percent.
5. **Pure bending of a rectangle** (first principles, elastic-perfectly plastic fibres in uniaxial stress): with the elastic
   core `|y| < y_e = (h/2) kappa_y/kappa`, `M = sigma_y w (h^2/4 - y_e^2/3)`, `M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2` (the
   acceptance): `1.18, 1.277778, 1.375, 1.444444, 1.46875, 1.48` at `kappa/kappa_y = 1.25, 1.5, 2, 3, 4, 5`. A narrow section in
   plane stress keeps the fibres uniaxial: with `sigma_zz = 0` (narrow) and no variation along `x`, the in-plane compatibility
   `d2 eps_xx/dy2 + d2 eps_yy/dx2 = 2 d2 eps_xy/dx dy` holds for any `eps_yy(y)`, so the different lateral contraction of the
   plastic fibres (`1/2`) and the elastic ones (`nu`) induces no `sigma_yy`; the out-of-plane incompatibility of `eps_zz(y)`
   is of the order of `(w/h)^2` of the plane-stress approximation (not estimated further).
6. **Strain measure.** The nonlinear driver is total Lagrangian (Green-Lagrange `E`, second Piola-Kirchhoff `S`). The law of 1
   is applied to `(E, S)`: small-strain plasticity valid for large rotation and small strain. With `eps <= 5e-3` here the
   difference to the small-strain closed forms of 3 to 5 is of the order of the strain, `<= 5e-3` relative in the sphere and
   `<= 1e-5` in the beam (rotation `kappa L = 2e-3`, `theta^2/2`), inside the 1 and 2 percent limits; the 1e-10 limits are at
   the material point (no kinematics) and U4 compares in the driver's own measures.

## Design

### D1. The material (`mat_j2.rs`)

`PENDING = None`; `FOLLOW_UP = "a follow-up of ADR 0475"` (the package named where J2 is refused). The state of one point,
`STATE_SIZE = 13` as declared by ADR 0440 D3: `[eps_p (6, Voigt, engineering shear), beta (6, Voigt, tensor shear), alpha]`.
`return_map(e, nu, yield_stress, h_iso, h_kin, strain, state) -> Result<Update>` (arithmetic 1, 2) with `Update { stress,
tangent, state, plastic }`; `update(material, strain, state)` for a `Material::J2`; accessors `plastic_strain`, `back_stress`,
`equivalent_plastic_strain` of a state slice (the results of ADR 0325's goal). The elastic branch computes `apply(&D, &(eps -
eps_p))` with `D = isotropic_elasticity(e, nu)`, so that a J2 material that never yields gives the bits of its elastic base
(N2). A state of the wrong length or with a non-finite value is refused (key `"state"`).

### D2. The constitutive call (`material.rs`, `model.rs`)

`Material::stress` of a J2 material returns `return_map`'s stress and tangent for the given committed state (the state is not
changed: the driver gets the new state through `mat_j2::update`, D3). `Material::elasticity_matrix` of J2 is refused
(`NotYetSupported`, item `"material"`, variant `"J2"`, location `"the element-constant elasticity matrix"`, package
`mat_j2::FOLLOW_UP`). `SolidModel::check_elements` gains a flag `history`: without it, a J2 material is refused naming its set
and `mat_j2::FOLLOW_UP` (every path but the nonlinear static solve); `pub(crate) check_supported_nonlinear` (elements with the
flag, then the supports) is what `nonlinear.rs` calls. Only the J2 arms are new; every other material takes the lines it took.

### D3. History in the Newton driver (`nonlinear.rs`, no second loop)

`evaluate` computes, at each point, the new state of a J2 material from the committed state of the attempt (`mat_j2::update`)
and returns the element's new states with its force and tangent; other materials take `Material::stress` as before and give
an empty new state. `force_and_tangent` returns the trial states with the force and the tangent (one slot per element, as the
forces). `Problem::attempt` sets its trial states to those of the latest evaluation, always from the committed states
`states_c` (backward Euler from the start of the increment, not from the previous iterate); at convergence the trial states of
the converged iterate are returned and committed by `solve_with` as before; a failed attempt drops them (the roll-back of ADR
0404 D5, now with something to roll back: H1). For a model without history the trial states equal the committed empty states,
so `NonlinearResult::states` and every value of the driver keep their bits (B0). `internal_force`, `tangent` and the element
functions evaluate from the states given (the committed ones).

### D4. What is not offered

J2 on Tet4, Hex8, Tet10 and Hex20 solids of the `Full` formulation through `nonlinear::solve` and `solve_with` only. Every
other path refuses it (D2). No B-bar or mixed element for the isochoric flow (ADR 0325 risk; the sphere and the beam use
Hex20); no tabulated hardening curve; no temperature or rate dependence; no case-file key (open question 1).

### D5. The sphere (S1 to S4)

`a = 1 m`, `b = 2 m`; `E = 2e11 Pa`, `nu = 0.3`, `sigma_y = 2e8 Pa`, perfectly plastic (`H_iso = H_kin = 0`). One octant
(symmetry planes `x = 0`, `y = 0`, `z = 0`, normal displacement fixed), as a cubed sphere: the three faces `x = 1`, `y = 1`, `z =
1` of the unit cube, each split `2 x 2` uniformly in its two tangential coordinates, carried to radius `r` by `p -> p |p|_inf /
|p|_2` (a cube-surface point `q`, `|q|_inf = 1`, at radius `r` is the parameter point `r q`); 8 uniform radial layers (`dr = 0.125
a`), 96 Hex8 cells upgraded by `upgrade_to_hex20_mapped` with that map, so that every node lies on its sphere. Bore pressure by
`Load::pressure` on the inner surface set (S1 to S3, dead load, the driver's load factor). Steps: S1 8, S2 10, S3 20 (driver
defaults: 20 iterations, tolerances `1e-10`, 5 halvings). S4: the model without loads and a driver contribution of force `-g
(lambda) f1` (`f1` the nodal load vector of `p1` on the bore, `g = 1 - |2 lambda - 1|`, no tangent), 16 load steps (8 up, 8 down;
the kink at `lambda = 0.5` falls on a step end). The radius of a point is the norm of its reference position; its stress is
`element_second_piola_kirchhoff` with the converged displacement and committed state.

### D6. The beam (P1, C2, H1, O1) and the bar (U4)

Beam (registered choice for (b2)): solid elements over the depth, the simplest model in which the material, the element and the
driver all act and nothing is integrated by hand. A narrow rectangular bar `L = 0.02`, `h = 0.1` (along `y`), `w = 0.01` (along
`z`; narrow, arithmetic 5), `hex_box([0, -h/2, -w/2], [L, h, w], [1, 16, 1])` upgraded to Hex20; same material as D5 (perfectly
plastic). Pure bending by prescribed displacements: `u_x = 0` on `x = 0`, `u_x = -kappa L y` on `x = L` (end section rotated as
a plane, free to contract and slide); rigid motions removed by `u_y = 0` at the node `(0, 0, 0)` and `u_z = 0` at the nodes `(0,
0, 0)` and `(0, h/2, 0)`. `M = |sum over the nodes of x = L of R_x y|`. Steps `ceil(4 kappa/kappa_y)`. A fibre model (beam
section integration of the uniaxial law) was rejected: it tests nothing but the quadrature of the closed form.
Bar U4: `hex_box([0; 3], [1, 0.1, 0.1], [1, 1, 1])` (Hex8), `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`, `u_x =
5e-3` on `xmax`; isotropic hardening `H_iso = 2e9`.

### D7. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_j2_default_bits.rs`, written after this commit and run on the unchanged base 4abff5fe before any
library code; its digests (FNV-1a 64 over the bits) are written into the file then. Content: `nonlinear::solve` of an
elastic stretched bar on Hex8 and Hex20 (displacement, reactions, residuals and energies of every increment, states); an
elastic Hex20 cantilever with a tip force in 2 load steps (same); `solve_with` with a zero contribution that refuses one call
(the halving path; same); `internal_force` and `tangent` of a distorted Tet10 model; `element_internal_force` and
`element_tangent` of a distorted Hex8; `Material::stress` of the isotropic material; `solve_static` of the Hex20 cantilever;
the error message of a model with a Viscoelastic material on `nonlinear::solve` and on `solve_static` (a pending variant,
unchanged).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| T1 | a1 | material tangent vs central differences (`h = 1e-9`), 4 hardening sets x elastic/plastic, `max abs diff / max abs` | `<= 1e-6` each | probe `1.2e-11 .. 6.9e-10` (arithmetic 2) |
| T2 | a1 | element tangent vs central differences of the internal force (`h = 1e-9` times the element size), distorted Hex8 and Hex20 (seed `0xF422_0475`), elastic and plastic with committed history | `<= 1e-6` each | F-397 measured `2e-11 .. 8e-11` for the elastic TL element; the plastic part as T1 |
| U1 | a2 | material point, bilinear, one step and 100 steps, isotropic and kinematic | `<= 1e-10` each | probe `1.0e-15`, `2.9e-16` |
| U2 | a3 | re-yield at `-sigma_max`, isotropic | `<= 1e-10` | probe `1.4e-15` |
| U3 | a3 | re-yield at `sigma_max - 2 sigma_y`, kinematic | `<= 1e-10` | probe `1.2e-15` |
| U4 | a2 (companion) | one Hex8 bar through `nonlinear::solve`, `S_xx` vs bilinear in `E_xx`, 1 and 100 steps | `<= 1e-8` each | residual tolerance `1e-10` of the driver |
| S1 | a4 | sphere at `p(1.25 a)`: `|p(c_FE) - p| / p` | `<= 1e-2` | arithmetic 3 (sensitivity 0.53); Hex20 bore displacement of F-389 within `7.3e-5` |
| S2 | a4 | the same at `p(1.5 a)` | `<= 1e-2` | as S1 (sensitivity 0.39) |
| C1 | a4 (companion) | points with `r < c - 0.05 a` yielded, with `r > c + 0.05 a` elastic, at S1 and S2 | all | point spacing `0.04 a` |
| S3 | a5 | the solve at `0.98 p_lim` converges | `Ok` | the discrete model is stiffer than the continuum, its limit above `p_lim` |
| S4 | a6 | residual hoop stress at the bore after unloading from `p(1.25 a)`, quadratic fit of the inner layer at `r = a`, vs `-0.628903 sigma_y` | `<= 2e-2` | fit alone `4.7e-3` (arithmetic 4) |
| P1 | a7 | `M/M_y` vs `1.5 - 0.5 (kappa_y/kappa)^2` at `kappa/kappa_y = 1.25, 1.5, 2, 3, 4, 5` | `<= 1e-2` each | arithmetic 5; 16 Hex20 over the depth |
| C2 | a7 (companion) | elastic `kappa = 0.5 kappa_y`: `M/M_y = 0.5` | `<= 1e-2` | exact for the narrow section in plane stress up to `(w/h)^2` |
| O1 | a8 | order of the last three residuals above `1e-12`, every increment of S1, S2, S3, P1 | `>= 1.8` each | Newton with the consistent tangent (arithmetic 2); F-397 measured `2.02 .. 2.08` |
| H1 | a9 | halved step against two steps, beam at `3 kappa_y`: one halving, equal bits of displacement, reactions, states, increments | exact | D3: the attempt starts from the committed states |
| R1 | a10 | `Material::j2` refusals: `yield_stress` 0, `-2e8`, NaN (key `"yield_stress"`); `isotropic = -1e9` (key `"isotropic"`), `kinematic = -1e9` (key `"kinematic"`) | all, with the key | the constructor of ADR 0440 D2 |
| N1 | b3 | B0: every digest of the base 4abff5fe | exact | D7 |
| N2 | negative control | a J2 material with `sigma_y = 1e30` (never yields) on the U4 bar and the Hex20 cantilever of B0 through `nonlinear::solve`: the bits of its elastic base (displacement, reactions, residuals) | exact | D1 elastic branch |
| N3 | negative control | a J2 model refused by `solve_static`, `solve_static_with`, `modal_analysis_with`, `stiffness`, `Integrator::new`, `solve_static_mpc` (item `"material"`, variant `"J2"`, location `"material 0, element set 'all'"`, package `mat_j2::FOLLOW_UP`); `elasticity_matrix` refused (D2) | all | D2 |
| N4 | b3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| X1 | b6 | the retired rows of `sa_scaffold.rs` (`x1_modules_and_pending_after_f430`: `mat_j2`; `m1_to_m4_after_f409`: M1 and M4 of J2) become ignored records citing ADR 0291, and their `_after_f422` copies pass | pass | the scheme of ADR 0291 |

Files: `crates/rcfd-fem/tests/sa_j2.rs` (T1 to H1, R1, N2, N3), `sa_j2_default_bits.rs` (N1). A check that would take more than
about two minutes in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | tangent `theta_bar = 1/(1 + H/(3 mu))` (the `(1 - theta)` dropped) | T1, T2 (plastic), O1 |
| MU2 | `dg = f / (2 mu + H)` (the factor `2/3` dropped) | U1 (both kinds), U2, U3 |
| MU3 | back stress not updated | U3 |
| MU4 | `alpha` not updated | U2 |
| MU5 | driver: the trial states not taken from the evaluation (committed states kept) | U4 (100 steps), S1, P1 |
| MU6 | driver: each iteration evaluated from the previous iterate's trial states | U4, P1 |
| MU7 | `check_elements` ignores the history flag (J2 taken by every path) | N3 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0 and
  the baseline hashes unchanged.
- **partial**: B0 and the baseline hashes unchanged, and at least the clauses (a1), (a2), (a3), (a9), (a10) met, with a miss of
  any of (a4) to (a8) diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Linear hardening only; a tabulated (multilinear) isotropic curve, a nonlinear kinematic rule (Armstrong-Frederick) and
  softening are not offered; a softening curve is refused at the constructor (R1).
- Small-strain plasticity on the Green-Lagrange strain and the second Piola-Kirchhoff stress of the total Lagrangian driver:
  correct for large rotation and small strain, not a finite-strain plasticity (no multiplicative split).
- No element for the isochoric flow (B-bar, mixed): fully integrated Hex20 may lock under large plastic flow; the checks stay in
  the contained plastic range (strains `<= 5e-3`, the sphere near its limit only for convergence).
- No temperature or rate dependence (polymer yield, ADR 0325); no plastic results on the linear paths (refused, D2).
- One mesh per check, no convergence study.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Case-file key.** Not added (the brief: the maintainer's call). Recommendation for the lead: a material entry `j2 = {
   base = <isotropic material>, yield_stress = <Pa>, isotropic = <Pa>, kinematic = <Pa> }` read into `Material::j2`, offered
   with the nonlinear static study only.
2. **Where J2 is refused.** Every path but the nonlinear static solve refuses it naming `mat_j2::FOLLOW_UP` (taken: a material
   with history on a linear path would silently be elastic).
3. **The beam model.** Solid elements over the depth of a narrow section (D6), taken for (b2).

## Amendment 1 (2026-11-14, written after the registered run of commit cb17790f and before any corrected run)

The set-up of T2 for the Hex20 elastic branch was defective: the random nodal displacement of amplitude `2e-4` yielded 3 of
the 27 points of the distorted Hex20 (its set-up check "points yielded" missed: `3 of 27`), so the branch was not elastic. The
Hex20 gradients of a random nodal field are larger than those of Hex8 (mid-edge nodes at half the distance). The measured value
of that run (`6.9e-11`) is kept as recorded, not counted for (a1).

**T2b** (corrected set-up, limit unchanged): the distorted Hex20 of T2 drawn from its own stream `Rng::new(3)` (seed `0xF422_0475
+ 3`), the J2 material of T2 (`H_iso = 1.5e9`, `H_kin = 1e9`), zero committed state, random nodal displacement of amplitude
`5e-5` (a quarter: the largest point strain of T2's draw scaled by four below the yield strain), set-up check: no point
yielded; measure and limit as T2, `max |K - K_fd| / max |K| <= 1e-6`, `h = 1e-9`. The record of the run of cb17790f stays as the
ignored test `t2_element_tangent_central_differences`; T2b is `t2b_hex20_elastic_tangent`. Run once.
