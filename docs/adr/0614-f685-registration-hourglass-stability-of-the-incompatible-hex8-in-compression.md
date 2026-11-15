# ADR 0614: F-685 registration: hourglass stability of the incompatible-modes Hex8 at finite strain in compression (the lowest eigenvalue of the condensed tangent on a distorted patch, and an opt-in stabilisation of the modes)

Status: **REGISTERED 2026-11-15, written before any code of F-685 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-685 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the
scratch folder (`w49i-work/f685-scratch`; no rcfd code; disclosed, their numbers quoted below): `probe_hg.py` (the patch
sweep, logs `probe_nh03_n4_stress.log`, `probe_nh03_n6_fine.log`, `probe_nh03_n4_reg.log`), `probe_var.py` (variants of the
element at the homogeneous state, used for the diagnosis below), `probe_need.py` (the penalty coefficient needed, log
`probe_need_n6.log`), `probe_bloch.py` (the Bloch analysis of the regular mesh, log `probe_bloch.log`), `probe_rule.py` (the
registered coefficient and two rejected ones, logs `probe_rule_n6_mean.log`, `probe_rule_n6_quad.log`,
`probe_rule_n4_quad.log`) and `probe_cant_stab.py` (the elastica cantilever of ADR 0608 CB1, CB2 with the stabilised element,
on top of `w49i-work/f596-scratch/probe_eas.py` and `probe_cant.py`; logs `probe_cant_stab.log` (the rejected coefficient),
`probe_cant_stab_quad.log` (the registered one)). The bit record B1 of the default paths is a test file written and run on the
base after this commit and before any library code (D6). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-685 (deps F-596), the follow-up proposed by [ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md)
(remaining question 2 there). Number 0614 given by the lead. Branch `f685-work` from the local main tip a9efc22e, worktree
`w49i-work/f685`, scratch `w49i-work/f685-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end. The lead's reading of ADR 0608 (a check met only after an amendment makes the proposal partial) is applied
in the decision rule.

## What F-685 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The ADR 0608 element under homogeneous compression of a distorted patch to a registered stretch: the lowest eigenvalue of the condensed tangent tracked against the Full Hex8 and a closed-form bifurcation-free reference, the stretch at which a spurious zero eigenvalue appears reported; if it appears within the registered range, a stabilisation from the published literature that keeps the ADR 0608 checks within their limits; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the ADR 0608 element under homogeneous compression of a distorted patch to a
registered stretch; **c2** the lowest eigenvalue of the condensed tangent tracked against the Full Hex8; **c3** against a
closed-form bifurcation-free reference; **c4** the stretch of a spurious zero eigenvalue reported; **c5** if it is within the
registered range, a stabilisation from the published literature; **c6** the stabilisation keeps the ADR 0608 checks within
their limits; **c7** the default bits unchanged.

### Readings, registered

- **c1: the patch, the compression and the range.** The unit cube on 6 x 6 x 6 Hex8 (`hex_box([0, 0, 0], [1, 1, 1], [6, 6,
  6])`), the interior nodes moved by `0.1 h (2U - 1)` per component (`h = 1/6`; the SplitMix64 of `si_fbar.rs` seeded
  `0xF685_0614`, drawn in node order, three per interior node), every boundary node prescribed. The compression is homogeneous
  uniaxial stress along `z`: `F(lambda) = diag(t, t, lambda)` with the lateral stretch `t(lambda)` solving `S_11 = 0` for the
  material (`S_22 = 0` by symmetry, the shears zero), `u = (F - I) X` at every node (an equilibrium state of both elements: the
  patch test of ADR 0608 arithmetic 4, the modes zero). The material is the neo-Hookean energy of ADR 0541 with `mu = 1e6 Pa`,
  `kappa = 2 mu (1 + nu) / (3 (1 - 2 nu))`, `nu = 0.3` (`kappa = 2.1667e6 Pa`), density 1000. Registered range: `lambda in
  [0.5, 1]` (to a nominal compression of 50 per cent), on the grid `lambda_j = 1 - 0.01 j`, `j = 0..50`.
- **c2: the lowest eigenvalue, and of what.** With every boundary node prescribed, the stability matrix of the fundamental path
  is the free-free block `K_ff` of the assembled tangent (`nonlinear::tangent` of the model at `u`), of size 375. Its
  eigenvalues are not invariant to the scaling of the unknowns, so the reading is the lowest eigenvalue of the pencil `(K_ff,
  L_ff)` with `L` the vector Laplacian of the Full Hex8 on the same mesh (`L[(a,i),(b,j)] = delta_ij sum_q w_q g_a . g_b`, the
  2 x 2 x 2 rule; positive definite on the free block), divided by `mu`: `m_X(lambda)` for `X = Full`, `Inc` (ADR 0608), `Stab`
  (D1 below). A zero of `m_X` is a zero eigenvalue of `K_ff` (the pencil and `K_ff` have the same inertia, Sylvester).
- **c3: the closed-form bifurcation-free reference.** For a homogeneous state the first elasticity tensor `A_iJkL = dP_iJ /
  dF_kL = delta_ik S_JL + F_iM F_kN C_MJNL` (`C = dS/dE`) is constant. For any displacement `v` vanishing on the boundary,
  extended by zero, Plancherel gives `int Grad v : A : Grad v = int |k|^2 v^* Q(n) v dk >= q int |Grad v|^2` with the acoustic
  tensor `Q_ik(n) = A_iJkL n_J n_L`, `n = k / |k|`, and `q = min_{|n| = 1} lambda_min(Q(n))` (the argument of Van Hove's
  theorem; recalled from memory, derived here in one line). So `q(lambda) / mu` is a closed-form lower bound of the continuum's
  Rayleigh quotient against the Laplacian: while `q > 0` the continuum with its boundary fixed has no bifurcation, and every
  zero eigenvalue of a discrete tangent is spurious. For a conforming discretisation with exact integration the same bound holds
  for the discrete pencil (`m_Full >= q / mu`, a Galerkin subspace); on the distorted patch the 2 x 2 x 2 rule is not exact, so
  the bound is a check with a margin (H2), not a theorem. `A` is formed from the material's `S` and `D` (`Material::stress` at
  `E(F)`) and `Q(n)` minimised over a 1-degree grid of the sphere (`theta = 0..180`, `phi = 0..359` degrees, the axes included).
- **c4: the stretch of a spurious zero.** `lambda_s` is the largest grid stretch at which `m_Inc <= 0` while the previous grid
  stretch had `m_Inc > 0`, refined by 20 bisections of `lambda` between the two (each one more eigenvalue solve), so to `1e-8`.
  "Spurious": at `lambda_s` both `q / mu` and `m_Full` stay positive with a margin (H5).
- **c5: the stabilisation.** The probes below show the zero inside the range (probe `lambda_s ~ 0.607`), so the stabilisation is
  built. Reading registered (D1): a deformation-dependent penalty on the enhanced deformation gradient, opt-in per model; its
  literature basis, what is taken from it and what is derived here are stated in "The stabilisation" below.
- **c6: the ADR 0608 checks.** Every ADR 0608 check that the stabilised element can be put through is re-run with the
  stabilisation on, with the limit of ADR 0608: SS1, SS2, SS3, PT1 to PT4, PM, PS, T1 to T4, X1, Q1, CB1 to CB3 and NT1 (the
  copies R-* below); CE (the test's elastica) is copied as part of CB. RF1, RF2 (refusal rows) and the controls CL, CN, N3, N4 (of
  the Full element) do not involve the element's response and are not copied; the original `si_eas.rs` is re-run unchanged as part
  of S1.
- **c7: default bits.** A model that does not opt in evaluates every element as before, bit for bit: B1 (new digests of the
  Incompatible paths, recorded on the base), B0 of ADR 0608 and `baseline_hashes`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0608 (whole); ADR 0506 (the follow-up text, D2); ADR 0541 D1 (the neo-Hookean energy);
  ADR 0584 D1 and ADR 0593 D5 (the model opt-in `set_hyperelastic_structures` and its copy in `rotating.rs`, the pattern of D2);
  ADR 0572, 0573 (the form of the roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (whole), `hex8x.rs` (`mode_gradients`, `modes`, `mode_points`, `element_stiffness`),
  `nonlinear.rs` (`hex8_evaluate`, `hex8_force_and_tangent_at` and its siblings, `hex8_strains`, `is_incompatible`,
  `internal_force_field`, `force_and_tangent`, `point_update`), `nl_transient.rs` (`strain_energy`), `model.rs` (`SolidModel`,
  `new`, `set_hyperelastic_structures`), `material.rs` (`HyperelasticEnergy`, `linear_elasticity_at`), `mat_hyper.rs` (head),
  `rotating.rs` (`unit_centrifugal_model`), `builders.rs` (`box_nodes`, `hex_box`), `rcfd-linalg/src/eigen.rs`
  (`symmetric_eigen`, `generalized_eigen_dense`: Cholesky reduction and cyclic Jacobi), `rcfd-linalg/src/matrix.rs`
  (`CsrMatrix`).
- Tests: `si_eas.rs` (head, the check helpers, the list of tests, `csr_rel_diff`), `si_fbar.rs` (`Rng`, `patch_mesh`,
  `patch_prescribed`).

## Arithmetic done before the code (disclosed)

1. **The plain element on the patch** (c2, c4; `probe_hg.py`, the patch of c1 with the same RNG and seed; numpy's `eigvalsh` on
   the pencil reduced by Cholesky). The probe's `A` is the complex-step derivative of the neo-Hookean `P(F)` (exact to rounding).

   | `lambda` | `t` | `q / mu` | `m_Full` | `m_Inc` |
   |---|---|---|---|---|
   | 1.00 | 1.00000 | 1.0000 | 1.0095 | 0.9767 |
   | 0.80 | 1.06678 | 1.0491 | 1.0719 | 0.9354 |
   | 0.70 | 1.10447 | 1.0711 | 1.1165 | 0.5815 |
   | 0.62 | 1.13429 | 1.0904 | 1.1659 | 0.0922 |
   | 0.61 | 1.13776 | 1.0932 | 1.1734 | 0.0160 |
   | 0.60 | 1.14112 | 1.0962 | 1.1801 | -0.0644 |
   | 0.55 | 1.15537 | 1.1148 | 1.2169 | -0.5396 |
   | 0.50 | 1.16100 | 1.1451 | 1.2698 | -1.1722 |

   `m_Inc` crosses zero between 0.61 and 0.60 (linear interpolation `0.608`), where `q / mu = 1.09` and `m_Full = 1.17`: a
   spurious zero inside the range. On a 4 x 4 x 4 patch (81 unknowns) the crossing is at `~0.556` (distorted) and `~0.558`
   (regular); the Bloch analysis of the regular infinite mesh (`probe_bloch.py`, the condensed element of the unit cube, the
   lowest generalised eigenvalue over a 12^3 grid of wave vectors and the hourglass corners) crosses between 0.65 (`+0.045`) and
   0.60 (`-0.387`): the instability is an element-level (hourglass) mode, the boundary of a finite patch delays it, and a larger
   patch moves the crossing towards the Bloch value. `m_Full >= q / mu` held at every probed stretch (smallest margin `9.5e-3` at
   `lambda = 1`). The probed `t(lambda)` has a maximum near 0.5 and decreases below (the volumetric term `(kappa/2)(J - 1)^2`);
   the range stops at 0.5.
2. **Diagnosis** (`probe_var.py`, 4 x 4 x 4). Dropping the geometric stiffness of the modes from `K_aa` changes `m_Inc` by at
   most `0.045` (`-0.577` to `-0.564` at 0.5); enhancing the Green-Lagrange strain instead of `F` (`E = E(F_c) + sym(sum_k alpha_k
   (x) G_k)`, the reference-frame enhancement of Buechter, Ramm and Roehl 1994, cited from memory) gives `-0.567`. So the
   instability is not the modes' own geometric stiffness: the modes relieve the material energy of an hourglass field (as they
   relieve the parasitic shear in bending, which is the point of the element) and leave an incompatible, rotation-like `delta F`
   whose geometric energy `delta F S delta F^T` is negative in compression; a compatible field cannot do this (a non-uniform
   rotation field is not a gradient), which is why `m_Full` and `q` stay positive.
3. **A penalty on the enhanced gradient.** Adding `(c/2) sum_q w_q |F_enh,q|^2` to the element energy (`F_enh = sum_k alpha_k (x)
   G_k`, `c` a stress) moves the element towards the Full Hex8 (`c -> infinity` forces `alpha = 0`). The `c` needed for `m >= 0`
   (`probe_need.py`, bisection, the patch of c1): `0` at 0.65, `0.047 mu` at 0.60, `0.419 mu` at 0.55, `0.970 mu` at 0.50,
   `1.797 mu` at 0.45; the Bloch cell needs `0.066`, `0.443`, `1.003`, `1.841 mu` (a 6^3 wave-vector grid).
4. **The coefficient** (`probe_rule.py`, `probe_cant_stab.py`). Three rules, each with the rest elasticity `D0` of the material
   and a stress proxy `s_hat = D0 : e_hat` of a strain of the compatible gradient `F0` at the element centre (so that `c` depends
   on `u` only and its derivatives are closed-form):
   - (i) `c = |D0 : E0|` (Green strain): `m_Stab` at 0.50 `-0.049` (patch), `-0.233` (Bloch): **not enough** (it is the MU3
     mutation below).
   - (ii) `c = |D0 : e0|`, `e0 = (C0 - C0^{-1})/4` (the mean of the Green strain and the Piola strain `(I - C0^{-1})/2`, equal to
     the Hencky strain to second order): patch `m_Stab >= 0.736`, Bloch `>= 0.646` over [0.5, 1]; but the norm has a derivative
     of order one at `e0 = 0`, so the energy couples the membrane strain to the square of the bending modes, and the cantilever
     CB2 (25 x 1 x 1, alpha 2) moves to `|d/d_ref - 1| = 2.30e-3`, above ADR 0608's limit `2e-3` (`probe_cant_stab.log`):
     **rejected**.
   - (iii) **registered**: `c = (s_hat : s_hat) / G0` with `s_hat = D0 : e0` of (ii) (`:` the tensor inner product, Voigt with
     the shears doubled), `G0` the mean of the three shear diagonals of `D0` (engineering shear, the shear modulus of an isotropic
     material). Smooth everywhere (a rational function of `F0`), zero with its first derivative at `C0 = I`, so the stabilised
     element equals the ADR 0608 element at rest and differs from it in the energy at fourth order in the strain (`c P`, `c =
     O(e^2)`, `P = O(alpha^2)`). Patch (`probe_rule_n6_quad.log`): `c / mu = 0.018, 0.377, 1.130, 3.055, 8.832` at 0.95, 0.80,
     0.70, 0.60, 0.50 (each above the need of item 3); `m_Stab = 0.9767` at 1 (equal to `m_Inc`), `0.9815` at 0.95, `1.000` at
     0.80, `1.034` at 0.70, `1.100` at 0.60, `1.212` at 0.50; smallest over the range `0.9767` (at `lambda = 1`); Bloch `>= 0.945`.
     4 x 4 x 4 (`probe_rule_n4_quad.log`): `m_Stab` `0.9988, 1.0391, 1.1476, 1.2605` at 1, 0.8, 0.6, 0.5, `m_Inc` `0.9988,
     1.0110, 0.2837, -0.5773`, `m_Full` `1.0401, 1.0934, 1.1947, 1.3078`, `q / mu` `1.0, 1.0491, 1.0962, 1.1451`. Cantilever
     (`probe_cant_stab_quad.log`, SVK `nu = 0`, the probe of ADR 0608 arithmetic 6): 50 x 1 x 1 `1.53e-4`, `3.44e-4` (alpha 1),
     `2.26e-4`, `4.64e-4` (alpha 2); 25 x 1 x 1 `1.73e-4`, `2.04e-4` and `1.39e-4`, `1.49e-4`: within `1e-6` of the plain
     element's numbers of ADR 0608 (the centre of a one-layer element is on the neutral axis, `c ~ G e0^2` is tiny there).
     Over-stabilisation in deep compression (`c = 8.8 mu` at 0.5 against a need of `1.0 mu`) brings the element towards the
     Full Hex8 there (`m_Stab = 1.21` against `m_Full = 1.27` at 0.5); that is the intended side to err on.
5. **Derivatives of the penalty** (D1). `P(alpha) = sum_q w_q |F_enh,q|^2 = alpha^T M alpha`, `M[3k+i, 3l+j] = delta_ij sum_q
   w_q G_k(q) . G_l(q)` (constant per element). `F0 = I + sum_a u_a (x) g_a(0)` is linear in `u` (`dF0_iJ / du_{a,i} = g_a(0)_J`).
   With `C0 = F0^T F0`, `dC = dF^T F0 + F0^T dF`, `d2C[a,b] = dF_a^T dF_b + dF_b^T dF_a`, `d(C^{-1}) = -C^{-1} dC C^{-1}`,
   `d2(C^{-1})[a,b] = C^{-1} dC_a C^{-1} dC_b C^{-1} + C^{-1} dC_b C^{-1} dC_a C^{-1} - C^{-1} d2C_ab C^{-1}`: `de0 = (dC -
   d(C^{-1}))/4`, `d2e0 = (d2C - d2(C^{-1}))/4`; `s_hat` linear in `e0`; `c = s:s / G0`, `dc = 2 s:ds / G0`, `d2c = 2 (ds_a:ds_b +
   s:d2s_ab) / G0`, over the nine directions `dF = e_i (x) e_J`, then mapped to `u` by `g_a(0)`. The element (33 unknowns):
   `f_u += (P/2) dc/du`, `f_alpha += c M alpha`; `K_uu += (P/2) d2c/du2`, `K_ualpha += dc/du (M alpha)^T` (and its transpose),
   `K_alphaalpha += c M`: the exact Hessian of the penalty, so the condensed tangent stays the exact derivative of the condensed
   force (ADR 0608 arithmetic 2). At a homogeneous state `alpha = 0` gives zero penalty force and an unchanged `R_a = 0`: the patch
   test is kept exactly; at `u = 0` `c = 0`, `dc = 0`: the tangent at rest is the F-390 condensed stiffness exactly.
6. **Cost of the registered sweep.** `generalized_eigen_dense` reduces by Cholesky and solves by cyclic Jacobi; at `n = 375` the
   order is `~10` sweeps of `~n^3` work, about a second in release. The sweep is 51 stretches x 3 elements plus 20 bisections,
   `~170` solves: a few minutes. It is therefore a heavy record (`#[ignore]`, run once with `--ignored`), with a fast companion
   (HF) on the 4 x 4 x 4 patch through the same code.

## The stabilisation: literature basis, what is taken and what is derived here

- The instability itself: enhanced-strain and incompatible-mode bricks at finite strain develop spurious hourglass modes under
  large homogeneous compression (P. Wriggers and S. Reese, "A note on enhanced strain methods for large deformations", Comput.
  Methods Appl. Mech. Engrg. 135 (1996) 201-209; cited from memory, not re-read). Arithmetic 1 reproduces it on the ADR 0608
  element; nothing of the paper's numbers is used.
- Stabilisations published since (all cited from memory, not re-read): a modified enhanced field (S. Glaser and F. Armero,
  Engineering Computations 14 (1997) 759-791), hourglass stabilisation of reduced integration with an EAS-derived stiffness (S.
  Reese and P. Wriggers, Int. J. Numer. Meth. Engng 48 (2000) 79-109), improved Taylor-expanded EAS bricks (J. Korelc and P.
  Wriggers, Comput. Mech. 19 (1996) 30-40; J. Korelc, U. Solinc and P. Wriggers, Comput. Mech. 46 (2010) 641-659), and a
  deformation-dependent stabilisation of EAS elements that vanishes in the small-strain limit (W. A. Wall, M. Bischoff and E.
  Ramm, "A deformation dependent stabilization technique, exemplified by EAS elements at large strains", Comput. Methods Appl.
  Mech. Engrg. 188 (2000) 859-871).
- **Taken**: the idea of Wall, Bischoff and Ramm as recalled: a stabilising term on the enhanced part of the element, scaled by
  the deformation so that it vanishes at small strain (the element keeps its small-strain behaviour) and grows with the
  deformation. **Not taken**: their stabilisation tensor and its scaling, which are not recalled reliably; nothing is reproduced
  from the paper. **Derived here**: the form of D1 (an energy term, so the tangent stays consistent; quadratic in `F_enh`; its
  coefficient (iii) of arithmetic 4 from the rest elasticity and the centre strain, smooth, of second order at rest). That the
  form is the repository's own and only its idea is from the literature is open question 1.

## Design

### D1. The stabilised element (`crates/rcfd-fem/src/nl_incompatible.rs`)

The extended element of ADR 0608 D1 gains, when asked for, the penalty of arithmetic 5: `Pi_s(u, alpha) = (c(u)/2) alpha^T M
alpha`, `c(u) = (s_hat : s_hat) / G0`, `s_hat = D0 e_hat(e0)` (Voigt, `e_hat` the engineering-shear Voigt vector of `e0 = (C0 -
C0^{-1})/4`, `C0 = F0^T F0`, `F0 = I + sum_a u_a (x) g_a(0)` with the shape gradients at the element centre), `D0 =
material.linear_elasticity_at(context)`, `G0 = (D0_33 + D0_44 + D0_55)/3` (refused, key `"internal modes"`, unless positive
and finite). Force and tangent as arithmetic 5; the local iteration and the condensation of ADR 0608 D2 unchanged, the mode scale
of the convergence test taking also `|c (M alpha)_k|` for mode `k` when stabilised. With the stabilisation off every operation
of ADR 0608 is made as before (the new terms are in a separate branch after the loop over the points).

Public, added: `condensed_force_and_tangent(coords, material, displacement, state, context, stabilised) -> (Vec<f64> /* 24 */,
ElementMatrix /* 24 */, Vec<[f64; 6]> /* stress */)`; `modes_and_strains_with(.., stabilised)`; `enhanced_force_and_tangent_with(..,
stabilised)`; `stabilisation_coefficient(coords, material, displacement, context) -> f64` (`c`); `stabilisation_energy(coords,
material, displacement, modes, context) -> f64` (`Pi_s`). The existing public functions keep their signatures and call the new
ones with `stabilised = false`. The crate-private `evaluate` gains `stabilised: bool`.

### D2. The opt-in (`model.rs`)

`SolidModel` gains `incompatible_stabilisation: bool` (default `false`, set in `new`), `set_incompatible_stabilisation(on)` and
`incompatible_stabilisation()`, documented with ADR 0614 (the pattern of `set_hyperelastic_structures`, ADR 0584 D1).

### D3. Dispatch (`nonlinear.rs`, shared solver file)

`internal_force_field` and `force_and_tangent` pass `model.incompatible_stabilisation()` to `nl_incompatible::evaluate`;
`hex8_evaluate` (behind `hex8_force_and_tangent_at` and its siblings, which take no model) passes `false` (the ADR 0608 element;
the stabilised element-level API is D1's). No other change.

### D4. `nl_transient.rs` and `rotating.rs`

`Integrator::strain_energy`: an Incompatible element of a stabilised model takes its strain from `modes_and_strains_with(..,
true)` and adds `stabilisation_energy` at those modes; otherwise as before. `rotating::unit_centrifugal_model` copies the opt-in
to its copy of the model (as ADR 0593 D5 for ADR 0584's).

### D5. The test files

`crates/rcfd-fem/tests/si_eas_hg.rs` (every check below but B1), `crates/rcfd-fem/tests/si_eas_hg_default_bits.rs` (B1). The
helpers are copies of `si_eas.rs` (existing tests not edited). The patch of c1 is built by `hex_box` and the RNG of `si_fbar.rs`;
the material by `Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e6, kappa), 1000)`; `t(lambda)` by 200 bisections on
`[1, 2]` of `S_11` from `Material::stress` at `E(F)` (and `S_11(t) < 0 < S_11(2)` checked); `L` assembled in the test from
`element_for(Hex8).integration_points`; the free block of `nonlinear::tangent(model, u, states, Some(4))` read from the CSR into a
dense matrix; the pencil by `rcfd_linalg::eigen::generalized_eigen_dense`. The copies R-* are the corresponding tests of
`si_eas.rs` with every Incompatible model opted in (`set_incompatible_stabilisation(true)`) and every element-level call
replaced by D1's stabilised function (`hex8_force_and_tangent_at(Incompatible, ..)` by `condensed_force_and_tangent(.., true)`,
`hex8_internal_force_at` and `hex8_second_piola_kirchhoff` by its force and stress, `modes_and_strains` and
`enhanced_force_and_tangent` by their `_with(.., true)`); the measures, set-ups, seeds and limits of ADR 0608 unchanged. R-NT1's
reference is `sum_q w_q S . E / 2` plus the test's own `Pi_s` (the test computes `c` from `F0` by its own code: the closed form of
arithmetic 4 (iii), not D1's function).

### D6. The bit record B1 (`crates/rcfd-fem/tests/si_eas_hg_default_bits.rs`)

Written after this commit and run on the base a9efc22e before any library code; FNV-1a 64 digests of the bits, the expected
values printed by that run and written into the file then (committed with the test, before any library code). Content (API of the
base only), every model without the opt-in: `nonlinear::tangent` of the c1 patch, Incompatible and Full, at `lambda = 0.6`;
`nonlinear::internal_force` of the same; `nonlinear::solve` of the sheared block of ADR 0608 Q1 (Incompatible, 4 steps; the
displacement); `nonlinear::solve` of the 25 x 1 x 1 elastica cantilever (Incompatible, alpha 1; the displacement);
`modes_and_strains` and `enhanced_force_and_tangent` on the distorted element of ADR 0608 T1 (b); `hex8_force_and_tangent_at
(Incompatible)` on that element; `nl_transient` of the ADR 0608 NT1 bar (Incompatible, 3 steps; displacement and strain energy).

## Checks

Every measure is required finite before it is compared (a non-finite value misses). `m_X` as c2 (divided by `mu`).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| H0 | c1 | `t(lambda)` on the grid: `|S_11| / |S_33|` at the solved `t` | `<= 1e-12` | bisection to rounding |
| H1 | c3 | `q(lambda) / mu` on the grid | `>= 0.9` each | `>= 1.0` (arith. 1) |
| H2 | c2, c3 | `m_Full(lambda) - q(lambda)/mu` on the grid | `>= -1e-9` each | `>= 9.5e-3` |
| H3 | c1 | the homogeneous state is an equilibrium: `max |f_free| / max |f|` of `internal_force` at `lambda = 0.75` and `0.5`, Full, Inc, Stab | `<= 1e-10` each | rounding (arith. 5) |
| H4 | c4 | `lambda_s` exists in the range and is reported (refined to `1e-8`) | in `[0.55, 0.66]` | `0.608` |
| H5 | c4 | at `lambda_s`: `q / mu` and `m_Full` | `>= 0.9` each | `1.09`, `1.17` |
| H6 | c2 | report: the table `lambda, t, q/mu, m_Full, m_Inc, m_Stab` on the grid; the hourglass content of the `m_Inc` eigenvector at the first grid stretch below `lambda_s` (the largest over the three components of `|sum_n (-1)^(i+j+k) v_n,c| / sum_n |v_n,c|` over the interior nodes `(i, j, k)`) | report | |
| S1 | c5 | `m_Stab(lambda)` on the grid | `>= 0.5` each | `>= 0.9767` |
| S2 | c5, c6 | at `lambda = 1` (`u = 0`): `max |K_Stab - K_Inc| / max |K_Inc|` (assembled) | `<= 1e-14` | 0 (`c = 0`) |
| S3 | c5 | `stabilisation_coefficient` at the c1 patch's element 0 for `lambda = 0.6` against the test's closed form of arithmetic 4 (iii) | `<= 1e-12` relative | rounding |
| HF | c2, c4, c5 | fast companion on the 4 x 4 x 4 patch (same seed), `lambda in {1, 0.8, 0.6, 0.5}`: `m_Full - q/mu >= -1e-9`; `m_Inc(0.8) >= 0.5`; `m_Inc(0.5) <= -0.1`; `m_Stab >= 0.5` each | as stated | arith. 4 |
| R-SS1 | c6 | ADR 0608 SS1 with the stabilisation (element via `condensed_force_and_tangent`, assembled via opted-in models) | `<= 1e-13` each | `1.9e-16` (`c = 0` at rest) |
| R-SS2 | c6 | ADR 0608 SS2 (modes via `modes_and_strains_with`) | `<= 1e-5`, `<= 1e-3`; ratio in `[30, 300]` | as ADR 0608 (the penalty changes `alpha` at `O(eps^3)`) |
| R-SS3 | c6 | ADR 0608 SS3 (opted-in cantilever) | `<= 1e-6` | `4.8e-7` |
| R-PT1..4, R-PM, R-PS | c6 | ADR 0608 PT1 to PT4, PM, PS (opted-in patches, modes and stress via D1) | `<= 1e-10` each | rounding (arith. 5) |
| R-T1, R-T2 | c6 | ADR 0608 T1 (a) to (e), T2 with `condensed_force_and_tangent(.., true)` | `<= 1e-6`; `<= 1e-12` | `<= 3e-8`; rounding |
| R-T3 | c6 | ADR 0608 T3 with `enhanced_force_and_tangent_with(.., true)` | `<= 1e-6` | `~2e-9` |
| R-T4 | c6 | ADR 0608 T4 at the modes of `modes_and_strains_with(.., true)` | `<= 1e-10` | rounding |
| R-X1 | c6 | ADR 0608 X1 with the stabilised modes | `<= 1e-12` | rounding |
| R-Q1 | c6 | ADR 0608 Q1 on the opted-in sheared block | 0 violations; an informative pair every step | as ADR 0608 (smooth, consistent) |
| R-CB1..3, R-CE | c6 | ADR 0608 CB1 to CB3 and CE, opted-in cantilevers | `<= 2e-3` each; CE `<= 1e-6` | arith. 4 (iii): `<= 4.64e-4` |
| R-NT1 | c6 | ADR 0608 NT1 on the opted-in bar: `strain_energy` against `sum_q w_q S . E / 2 + Pi_s` (test's own `c`) | `<= 1e-12` relative | rounding |
| B1 | c7 | `si_eas_hg_default_bits.rs`: every digest equal to the base's | bit-equal | D1 to D4 |
| B0 | c7 | `si_eas_default_bits.rs` of ADR 0608 | passes | |
| S4 | c7 | `cargo test -p rcfd-fem --release` (including `si_eas.rs` unchanged) | 0 failed | |
| S5 | c7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

The registered run of H0 to H6, S1 and S2 is the heavy test `h_sweep_on_the_distorted_patch_heavy` (ignored, run once with
`--ignored`); HF, S3, H3 and the R-* run by default.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the plain element itself (H4): the zero appears inside the range | `m_Inc(0.5) < 0` (probe `-1.17`) |
| N2 | the Full element on the grid (H2) | no zero in the range |

### Planned mutations (on a scratch copy, each run once against `si_eas_hg.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the penalty force on the modes (`c M alpha`) dropped from `f_alpha`, the tangent kept | R-T1, R-T4 |
| MU2 | the `(P/2) d2c/du2` term dropped from `K_uu` | R-T1 (b), (e) |
| MU3 | `c` from the Green strain (`e0 = (C0 - I)/2`, rule (i) squared) | S1 / HF (stability at 0.5; probe of rule (i) squared not run: expected `m_Stab(0.5)` below 0.5) |
| MU4 | the opt-in ignored in the assembly (`false` passed in D3) | S1, HF |
| MU5 | the penalty always on (the opt-in ignored, `true` passed in D3) | B1, H4, HF (`m_Inc(0.5) <= -0.1`) |
| MU6 | the `(P/2) dc/du` force term dropped | R-T1 |
| MU7 | `strain_energy` without `Pi_s` | R-NT1 |

The heavy sweep is not re-run for the mutations: the fast companion HF covers the same code path (mutations caught only by the
heavy test are not claimed).

## Decision rule

- **done**: every check above met in its single registered run, the negative controls as stated, every planned mutation caught by
  a registered check, `baseline_hashes` unchanged, no amendment needed.
- **partial**: H0 to H5, S1, B1 and S4 met, but an R-* check is missed (recorded as `*_known_miss`, diagnosed), or a mutation is
  not caught, or a check is met only after an amendment.
- **todo**: B1, B0 or S4 fails and cannot be fixed without changing a default result, or H2 or S1 is missed.
- If H4 finds no zero in the range (against the probes), the result is reported as "no spurious zero in [0.5, 1]" and the
  stabilisation and its checks are still reported as built, but the status is decided on H0 to H3, H5 and B1 alone.

## Limits (not covered)

- One material (neo-Hookean, `nu = 0.3`), one load path (homogeneous uniaxial stress along `z`), one distorted patch with all
  boundary nodes fixed, `lambda >= 0.5`. Confined compression, other Poisson ratios (the near-incompressible limit, where `kappa`
  dominates `c` and the stabilised element moves towards the volumetrically locking Full Hex8 in compression), tension, shear-
  dominated paths and free boundaries are not checked. The crossing depends on the patch size (arithmetic 1); the Bloch value is a
  probe, not a check.
- The stabilisation is the repository's form with the idea of a published technique (open question 1); it is not a reproduction
  of any paper's element, and no comparison with a published table is made.
- The stabilisation's coefficient uses the rest elasticity; for J2, creep and viscoelastic materials it is the base elasticity
  (no state dependence) and only R-T1 (c), (d) and R-PT4 cover them.
- The element-level public helpers of `nonlinear` (`hex8_force_and_tangent_at` and its siblings) stay the ADR 0608 element; the
  stabilised element-level API is D1's.
- No case-file key (the block property follow-up of ADR 0506 covers formulations; the opt-in would join it).
- The energy-momentum step, temperature fields and `hex8_strains(Incompatible)` stay refused as in ADR 0608.

## Open questions (taken or open)

1. **Which published stabilisation.** Options: (a) the repository's penalty of D1 with the idea of Wall, Bischoff and Ramm 2000
   (deformation-dependent, vanishing at small strain), derived here; (b) a specific published element (for example the
   stabilisation of Wall, Bischoff and Ramm in its exact form, or Korelc, Solinc and Wriggers 2010) reproduced from the paper.
   Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the repository does not have the
   papers, and the house rules forbid reproducing a form from memory without deriving it. Open for the maintainer: whether (a)
   satisfies "from the published literature" or (b) is wanted with the paper in hand.
2. **The coefficient.** Rule (iii) of arithmetic 4, chosen before the run from the derivation (smooth, second order at rest,
   above the need of arithmetic 3) and checked against the probes; rules (i) and (ii) rejected for the stated reasons. Taken on the
   same instruction.
3. **Default.** The stabilisation is opt-in (the brief and c7); whether it should become the default for Incompatible blocks in
   the nonlinear driver changes default results and is left open.
