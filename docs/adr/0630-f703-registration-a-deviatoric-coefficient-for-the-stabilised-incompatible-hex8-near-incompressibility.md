# ADR 0630: F-703 registration: a deviatoric coefficient for the stabilised incompatible-modes Hex8 near incompressibility (and why no coefficient without the bulk modulus can stabilise confined compression there)

Status: **REGISTERED 2026-11-16, written before any code of F-703 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-703 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the
scratch folder (`w49i-work/f703-scratch`; no rcfd code; disclosed, their numbers quoted below): `probe_f703.py` (the ADR 0628
sweep with the candidate coefficients, the F-bar Hex8 of ADR 0506 on the same patch, and the coefficient needed for a
non-negative lowest eigenvalue by bisection; logs `p4_*.log` (4^3 patch), `p6_*.log` (6^3 patch)), `check_fbar.py` (the
probe's F-bar quadratic form against a second difference of the F-bar energy: agreement to `3.8e-8`, the difference step),
`probe_scan.py` (the lowest eigenvalue against the coefficient at fixed states, 4^3; quoted in arithmetic 2) and
`probe_energy.py` (a copy of the F-693 energy probe with the coefficient as argument; logs `e_rest.log`, `e_dev.log`,
`e_lin.log`). The probes `probe_hg.py`, `probe_need.py`, `probe_f693.py` are copied unchanged from `w49i-work/f693-scratch`.
The bit record B3 of the ADR 0614 coefficient is a test file written and run on the base after this commit and before any
library code (D7). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-703 (deps F-693), the follow-up proposed by [ADR 0628](0628-f693-registration-the-stabilised-incompatible-hex8-in-confined-and-nearly-incompressible-compression.md)
(remaining question 1 there). The element is the incompatible-modes Hex8 at finite strain of
[ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) with the opt-in stabilisation of
[ADR 0614](0614-f685-registration-hourglass-stability-of-the-incompatible-hex8-in-compression.md) D1. Number 0630 given by the
lead. Branch `f703-work` from the local main tip 3f9ce17a, worktree `w49i-work/f703`, scratch `w49i-work/f703-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end. The lead's reading of ADR 0608 (a check met only after an amendment makes the proposal partial) is applied
in the decision rule.

## What F-703 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. An opt-in coefficient of the ADR 0614 stabilisation without the bulk modulus, from the published literature or derived in the ADR; the ADR 0628 sweep repeated for it in all six cases, the lowest eigenvalue kept above a registered positive limit over [0.5, 1]; a locking measure of the compressed state against a locking-free reference of the repository (an F-bar or mixed Hex8 on the same patch) reported for the ADR 0614 and the new coefficient; the ADR 0608 checks within their limits; the default bits unchanged.

The brief adds: the new coefficient is a separate opt-in, and the coefficient of ADR 0614 is unchanged when it is not selected.

Clauses: **c0** every limit registered here; **c1** an opt-in coefficient of the ADR 0614 stabilisation without the bulk
modulus, from the literature or derived here; **c2** the ADR 0628 sweep repeated for it in all six cases; **c3** its lowest
eigenvalue kept above a registered positive limit over `[0.5, 1]`; **c4** a locking measure of the compressed state against a
locking-free reference of the repository on the same patch, reported for the ADR 0614 and the new coefficient; **c5** the ADR
0608 checks within their limits; **c6** the default bits unchanged, and the ADR 0614 coefficient unchanged when not selected.

### The finding of the probes, stated first

The probes (arithmetic 1 and 2) show that **the coefficient needed to remove the spurious zero in confined compression near
incompressibility grows with the pressure, and so with the bulk modulus**: at `F = diag(1, 1, 0.5)` on the 6^3 patch it is
`1.12 mu` at `nu = 0.3` and `38.4 mu` at `nu = 0.49`, while at `diag(1, 1, 0.95)` it is `0` at `nu = 0.3` and `0.015 mu` at
`nu = 0.49`. Arithmetic 2 derives why from first principles: the volumetric energy's second variation contains the pressure
times a null Lagrangian, which integrates to zero on every compatible field (so the continuum and the Full Hex8 do not feel it)
but not on the incompatible enhanced field, whose energy it lowers by up to the largest compressive principal value of the
volumetric stress. A coefficient that is a function of the strain and of the shear modulus alone (no bulk modulus) is the same
function at `nu = 0.3` and `nu = 0.49` for the same `F`; to stabilise C49 it would have to be about 34 times the need of C30
at `diag(1, 1, 0.5)`, which is a calibration to one Poisson ratio, not a coefficient without the bulk modulus.

The registration therefore builds what the acceptance asks (c1: rule **D**, the deviatoric coefficient, without the bulk
modulus, derived here) and registers, from the probes, that it **misses c3 in the cases C45 and C49** while meeting it in U30,
C30, U45 and U49. To leave the item with a usable answer it also builds, as a second, diagnostic opt-in, rule **L**: the ADR
0614 rest stress taken linearly rather than squared at large stress, in which the bulk modulus enters linearly (the order of
the need) instead of squared over the shear modulus (ADR 0614's 6507 `mu` at C49, 0.5, against a need of 38.4 `mu`). Rule L
is not offered as meeting c1 (it contains the bulk modulus); it is measured by the same checks so that the follow-up can be
decided on numbers (open question 1).

### Readings, registered

- **c1: the coefficient without the bulk modulus (rule D).** `c_D = (dev s : dev s) / G0` with `s = D0 : e0` of ADR 0614 D1
  (`e0 = (C0 - C0^{-1})/4` of the compatible centre gradient `F0`, `D0` the rest elasticity, `G0` the mean of its three shear
  diagonals) and `dev s = s - (tr s / 3) I` (Voigt: the mean of the three normal components subtracted from them, the shears
  unchanged; `:` the tensor inner product). For an isotropic `D0 = 3 K0 P_vol + 2 G0 P_dev`, `dev(D0 : e) = 2 G0 dev e`, so
  `c_D = 4 G0 |dev e0|^2`: **the bulk modulus does not appear**. For an anisotropic `D0` the deviatoric part of the rest stress
  still carries the coupling moduli; that is the natural extension and it is stated as a limit. Derivation in arithmetic 3.
- **c1, the opt-in.** A rule selected per model (D3): `Off` (default), `RestStress` (ADR 0614 D1, what
  `set_incompatible_stabilisation(true)` selects, bit for bit as before), `Deviatoric` (rule D), `LinearStress` (rule L).
- **c2: the six cases.** ADR 0614's U30 (uniaxial stress, `nu = 0.3`) and ADR 0628's C30, C45, C49 (confined, `F = diag(1,
  1, lambda)`, `nu = 0.3, 0.45, 0.49`), U45, U49 (uniaxial stress `F = diag(t, t, lambda)`, `S_11(t) = 0`); the 6 x 6 x 6
  distorted patch of ADR 0614 c1 (seed `0xF685_0614`), every boundary node prescribed, the homogeneous state `u = (F - I) X`,
  the neo-Hookean energy of ADR 0541 with `mu = 1e6 Pa`, `kappa = 2 mu (1 + nu) / (3 (1 - 2 nu))`; the pencil `(K_ff, L_ff)`
  with `L` the Full Hex8 vector Laplacian, its eigenvalues over `mu`; the grid `lambda_j = 1 - 0.01 j`, `j = 0..50`.
- **c3: the registered positive limit.** `m_D >= 0.5` at every grid stretch of every case (the limit of ADR 0614 S1 and ADR
  0628 S1). Predicted missed at C45 and C49 (arithmetic 1); a miss is recorded as `*_known_miss` and is the decision of the
  rule, not a reason to change the coefficient.
- **c4: the locking measure and the locking-free reference.** The reference is the F-bar Hex8 of ADR 0506 (the `BBar`
  formulation in the nonlinear driver: `F_bar = (J_bar / det F)^(1/3) F`, `J_bar` the element mean of `det F`, its consistent
  symmetric tangent) on the same patch, the same state, the same pencil. It is locking-free in the sense that matters here: at
  rest at `nu = 0.49` its lowest eigenvalue is `1.0018` against the continuum's exact infimum 1 (the closed form `q / mu` of ADR
  0628 arithmetic 2, which for a constant-coefficient operator with the boundary fixed is the infimum of the Rayleigh quotient:
  wave packets along the minimising direction approach it, Plancherel gives the bound; argument of ADR 0614 c3), where the Full
  Hex8 gives `1.190` and the plain incompatible element `1.0555` (probe). The measures, at every grid stretch of every case,
  for `X = RestStress, Deviatoric, LinearStress` (and Full, plain Inc for context): `Lambda1_X = m_X,1 / m_Fbar,1 - 1` (the
  lowest eigenvalues) and `Lambda10_X = sum_{i <= 10} m_X,i / sum_{i <= 10} m_Fbar,i - 1` (the ten lowest). Report, no
  accuracy limit (the acceptance says "reported"); the F-bar reference is checked to be stable and near the continuum (F1, F2).
  Positive `Lambda` is stiffer than the reference (locking), negative softer (towards the instability).
- **c5: the ADR 0608 checks.** Every check of ADR 0608 that ADR 0614 copied (R-SS1 to R-SS3, R-PT1 to R-PT4, R-PM, R-PS, R-T1 to
  R-T4, R-X1, R-Q1, R-CB1 to R-CB3, R-CE, R-NT1, the copies in `si_eas_hg.rs`) re-run with rule D (the acceptance) and rule L
  (diagnostic), the limits of ADR 0608; plus the large-strain energy checks E1 and E3 of ADR 0628 for both rules (R-NT1 alone
  cannot see the stabilisation energy, ADR 0614 MU7).
- **c6: default bits.** A model that does not opt in evaluates every element as before (B1 of ADR 0614, B0 of ADR 0608,
  `baseline_hashes`), and a model that opts in with `set_incompatible_stabilisation(true)` gets the ADR 0614 element bit for bit
  (B3, new digests recorded on the base 3f9ce17a before any library code), and the existing tests of `si_eas_hg.rs` and
  `si_eas_hg2.rs` pass unchanged.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0628 (whole); ADR 0614 (whole: c1 to c7, arithmetic 1 to 6, D1 to D6, the checks, the
  mutations, addendum 1); ADR 0506 (head, the acceptance); ADR 0572, 0573, 0628 (the form of the roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (whole), `model.rs` (`SolidModel`, `new`, the opt-in, the module head on
  non-exhaustive enums), `nonlinear.rs` (the module head on the F-bar Hex8, `evaluate_with` for formulations, the dispatch in
  `internal_force_field` and `force_and_tangent`), `nl_transient.rs` (`strain_energy`), `rotating.rs`
  (`unit_centrifugal_model`), `material.rs` (public functions), `rcfd-linalg/src/eigen.rs` (`generalized_eigen_dense`, values
  ascending).
- Tests: `si_eas_hg.rs` (whole), `si_eas_hg2.rs` (whole), `si_eas_hg_default_bits.rs` (whole).
- The probes of `w49i-work/f685-scratch` and `w49i-work/f693-scratch`.

## Arithmetic done before the code (disclosed)

1. **The sweep with the candidates** (`probe_f703.py`, 6^3 patch, every case at `lambda = 1, 0.95, 0.9, 0.85, 0.8, 0.7,
   0.6, 0.5`; the probe reproduces ADR 0628's measured `m_Full`, `m_Inc`, `m_Stab` (= `m_R`) to their printed digits). Over
   `mu`; `need` is the smallest `c` with `m >= 0` (bisection); `c_R` the ADR 0614 coefficient, `c_D`, `c_L` the rules of
   arithmetic 3:

   | case | `lambda` | `q/mu` | `m_Full` | `m_Fbar` | `m_Inc` | `m_R` | `m_D` | `m_L` | `c_R` | `c_D` | `c_L` | need |
   |---|---|---|---|---|---|---|---|---|---|---|---|---|
   | all nu 0.3 | 1.00 | 1 | 1.0095 | 1.0015 | 0.9767 | 0.9767 | 0.9767 | 0.9767 | 0 | 0 | 0 | 0 |
   | U30 | 0.70 | 1.0710 | 1.1165 | 1.0977 | 0.5815 | 1.0336 | 0.9615 | 0.9315 | 1.130 | 0.634 | 0.548 | 0 |
   | U30 | 0.60 | 1.0962 | 1.1801 | 1.1464 | -0.0644 | 1.1000 | 0.8576 | 0.7470 | 3.055 | 1.452 | 1.112 | 0.047 |
   | U30 | 0.50 | 1.1449 | 1.2698 | 1.2337 | -1.1722 | 1.2117 | 0.7720 | **0.5590** | 8.832 | 3.163 | 2.224 | 0.970 |
   | C30 | 0.70 | 1.2290 | 1.2729 | 1.2607 | 0.3650 | 1.2008 | 0.7095 | 0.9844 | 2.518 | 0.401 | 0.973 | 0 |
   | C30 | 0.60 | 1.3120 | 1.3970 | 1.3769 | -0.3033 | 1.3380 | **0.5388** | 0.8408 | 6.120 | 0.974 | 1.762 | 0.243 |
   | C30 | 0.50 | 1.3966 | 1.5271 | 1.4960 | -1.2346 | 1.4699 | 0.5439 | 0.7272 | 14.72 | 2.344 | 3.044 | 1.123 |
   | C45 | 0.85 | 1.1128 | 1.1455 | 1.1148 | 0.5851 | 1.1257 | 0.6964 | 1.0855 | 7.742 | 0.073 | 2.047 | 0 |
   | C45 | 0.80 | 1.1568 | 1.1884 | 1.1598 | -0.1888 | 1.1731 | **0.1800** | 1.1202 | 15.05 | 0.142 | 3.085 | 0.068 |
   | C45 | 0.50 | 1.5036 | 1.5972 | 1.5384 | -10.226 | 1.5928 | **-2.5334** | 1.3557 | 248.7 | 2.344 | 14.83 | 6.087 |
   | C49 | 1.00 | 1 | 1.1901 | 1.0018 | 1.0555 | 1.0555 | 1.0555 | 1.0555 | 0 | 0 | 0 | 0 |
   | C49 | 0.95 | 1.0348 | 1.2145 | 1.0365 | -0.0547 | 1.1689 | **-0.0292** | 1.1078 | 19.55 | 0.007 | 3.606 | 0.015 |
   | C49 | 0.80 | 1.1597 | 1.3006 | 1.1600 | -33.299 | 1.2919 | **-28.389** | 1.1383 | 393.8 | 0.142 | 18.89 | 4.532 |
   | C49 | 0.60 | 1.3982 | 1.4859 | 1.3843 | -129.78 | 1.4821 | -69.584 | 1.2066 | 2705 | 0.974 | 51.03 | 20.21 |
   | C49 | 0.50 | 1.5664 | 1.6427 | 1.5394 | -189.29 | 1.6402 | **-66.980** | 1.2505 | 6507 | 2.344 | 79.68 | 38.40 |
   | U45 | 0.50 | 1.0068 | 1.0636 | 0.9929 | -0.9868 | 1.0553 | 1.0201 | 1.0169 | 24.24 | 4.321 | 4.092 | 0.496 |
   | U49 | 0.60 | 1.0017 | 1.1386 | 0.9709 | 0.7366 | 1.1155 | 1.0515 | 1.0826 | 50.68 | 2.000 | 6.242 | 0 |
   | U49 | 0.50 | 1.0005 | 1.1181 | 0.9582 | -0.1445 | 1.1116 | 1.0511 | 1.0815 | 307.4 | 4.542 | 16.59 | 0.064 |

   The smallest probed values: rule D `0.539` (C30, 0.6) in U30, C30, U45, U49, and negative in C45 (from between 0.85 and 0.8)
   and C49 (already at 0.95: `-0.029`); rule L `0.559` (U30, 0.5) in all six. At 0.9 C49: `m_D = -5.275`, need `1.149`.
   `m_Fbar` stays between `0.958` (U49, 0.5) and `1.539` (C49, 0.5), within `7.8` per cent of `q / mu` at every probed state
   (largest at U30 0.5, `1.2337` against `1.1449`; `0.15` per cent at rest; a discretisation gap, not locking: it is largest at
   `nu = 0.3`), and positive: the F-bar Hex8 has no spurious zero in any case. On the 4^3 patch (`p4_*.log`) rule D at 0.5 is `0.933`
   (U30), `0.806` (C30), `-1.430` (C45), `-50.43` (C49), `1.064` (U45), `1.201` (U49); at C49 0.8 `-19.28`; rule L
   (`csoft` there) was not on the 4^3 run (added after it); the 4^3 F-bar is within `10.5` per cent of `q` everywhere (C30, 0.5).
2. **Why the need follows the pressure** (first principles; `probe_scan.py` for the numbers). For `W = W_iso(F) + U(J)` and a
   direction `delta F`, with `A = F^{-1} delta F`, `J(s) = det(F + s delta F) = J (1 + s tr A + s^2 ((tr A)^2 - tr(A^2))/2 +
   O(s^3))`, so `d^2 U / ds^2 = U'' J^2 (tr A)^2 + U' J ((tr A)^2 - tr(A^2))`. In compression `U' = -p < 0` (`p = kappa (1 -
   J)` the pressure of this energy), and the second term is `p J (tr(A^2) - (tr A)^2)`. For a compatible field (`delta F = Grad
   v`, `v = 0` on the boundary) `J ((tr A)^2 - tr(A^2))` is the spatial `(div w)^2 - grad w : grad w^T` (`w = v` on the current
   configuration, `A` similar to `grad w`), whose integral vanishes (two integrations by parts): a null Lagrangian, the
   second-order term of `det` (that the minors of the gradient are null Lagrangians is classical, e.g. J. M. Ball, Arch.
   Rational Mech. Anal. 63 (1977) 337-403; cited from memory, the one-line derivation above is what is used). So the pressure
   does not destabilise the continuum or the conforming Full Hex8: `q` grows with the confined compression (`1.57 mu` at C49,
   0.5). The enhanced field `H = F_enh` is not a gradient and the integral does not vanish; for a rotation-like `F^{-1} H` (skew
   `A`: `tr A = 0`, `tr(A^2) = -|A|^2`) the term is `-p J |F^{-1} H|^2 >= -p J lambda_max(C^{-1}) |H|^2`. A penalty `c |H|^2`
   compensates it only if `c` is of the order of `p J lambda_max(C^{-1})`, the largest compressive principal value of the
   volumetric second Piola-Kirchhoff stress `U' J C^{-1}`: at C49, 0.5 that is `24.8 * 0.5 * 4 = 49.7 mu`, at C49, 0.8 `12.4
   mu`, against the probed needs `38.4` and `4.53 mu` (6^3; `30.8`, `3.40` on 4^3). It is a bound of the destabilising term,
   not of the need (the mode shape and the material part enter), but it fixes the scaling: **the need is linear in the
   pressure, hence in `kappa` at a fixed volumetric strain**. `probe_scan.py` (4^3) shows the shape: at C49, 0.5 `m` against
   `c / mu` is `-102.7, -56.0, -16.4, +0.22, +1.41, +1.61, +1.75` at `0.5, 2, 8, 32, 64, 128, 512`, F-bar `1.564`; the
   eigenvalue rises steeply past the need and saturates at the Full Hex8 (`1.80`), so `c` of twice the need already gives
   `0.9 m_Fbar`, and ADR 0614's `c = 6507 mu` gives the Full Hex8 to `0.2` per cent.
3. **The coefficients** (derivation; D2).
   - Rule D: ADR 0614 (iii) with the rest stress replaced by its deviatoric part. It keeps the properties ADR 0614 chose (iii)
     for: smooth (a rational function of `F0`), zero with its first derivative at `C0 = I` and on rigid motions (so the element
     at rest and the patch test are ADR 0608's), `c_D <= c_R` (`|dev s| <= |s|`). Derivatives: `dev` is linear, symmetric and
     idempotent in the inner product, so `dc_D = 2 dev s : ds / G0`, `d2c_D = 2 (dev ds_a : dev ds_b + dev s : d2s_ab) / G0`
     (the formulas of ADR 0614 arithmetic 5 with `s, ds, d2s` replaced by their deviatoric parts).
   - Rule L: `c_L = (s : s) / (G0 + |s|)`, `|s| = sqrt(s : s)`, `s` of ADR 0614. For `|s| << G0` it is `c_R (1 - |s| / G0 +
     ...)`, so it equals ADR 0614's coefficient to second order at rest (the cantilever checks of ADR 0608 see the same element
     to within the size of `c_R`, which ADR 0614 measured within `1e-6` of the plain element); for `|s| >> G0` it is `|s| - G0
     + ...`, linear in the stress: the order of the need of arithmetic 2. It is `C^2` in `s` (the `|s|^3` term of the
     expansion has a continuous Hessian), `c_L <= c_R`. With `r = |s|`, `a(r) = (2 G0 + r) / (G0 + r)^2`, `b(r) = -(3 G0 + r) /
     ((G0 + r)^3 r)` (`r > 0`): `dc_L = a s : ds`, `d2c_L = a (ds_a : ds_b + s : d2s_ab) + b (s : ds_a)(s : ds_b)`, the last term
     taken as 0 at `r = 0` (its limit). The rational blend is the simplest smooth function with those two limits; no factor is
     fitted. The probe's need is below `c_L` at every probed state (arithmetic 1).
   - Neither is from a paper. The idea of a stabilisation of the enhanced field scaled by the deformation is the one ADR 0614
     took from W. A. Wall, M. Bischoff and E. Ramm (Comput. Methods Appl. Mech. Engrg. 188 (2000) 859-871; cited from memory,
     not re-read); a volumetric-deviatoric split of hourglass stabilisation stiffness is common in the reduced-integration
     literature (for example S. Reese and P. Wriggers, Int. J. Numer. Meth. Engng 48 (2000) 79-109; cited from memory, nothing
     taken from it). The forms above and arithmetic 2 are the repository's.
4. **The ordering of the ten lowest eigenvalues** (O2). ADR 0628 arithmetic 3: at a homogeneous state the modes are zero, the
   penalty adds only `c M` to the modes' block, and the condensed tangent `K_uu - K_ua (K_aa + c M)^{-1} K_au` is non-decreasing
   in `c` in the Loewner order while every `K_aa` is positive definite (K0). The state is homogeneous, so every element has the
   same `F0` and the same `c`; `0 <= c_D, c_L <= c_R`, so by Courant-Fischer for the pencil with the same `L`, **every** ordered
   eigenvalue satisfies `m_Inc,i <= m_D,i <= m_R,i <= m_Full,i` and `m_Inc,i <= m_L,i <= m_R,i`. A check that holds whatever
   the accuracy of the elements; it ties the locking measures (`Lambda_D, Lambda_L <= Lambda_R`) to the coefficients.
5. **The energy set-up** (`probe_energy.py`, the E state of ADR 0628 c1: 4^3 patch, `nu = 0.3`, `u = (F - I) X + 0.2 (x z, y
   z, 0)`, `F = diag(1, 1, 0.6)`): the stabilisation energy over the total `7.14e-4` (rule D), `7.19e-4` (rule L), `5.33e-4`
   (ADR 0614, ADR 0628's measured `5.328e-4` reproduced). E1s registered `>= 1e-4` for both.
6. **Cost.** As ADR 0628 arithmetic 4: about a second per dense 375 x 375 pencil in release. A case of the heavy record is 51
   stretches x 6 pencils (Full, F-bar, Inc, R, D, L), about five minutes; each case is a heavy record (`#[ignore]`, run once),
   with the fast companion HF3 on the 4^3 patch through the same code.

## Design

### D1. The rules (`crates/rcfd-fem/src/nl_incompatible.rs`)

`pub enum IncompatibleStabilisation { Off, RestStress, Deviatoric, LinearStress }` (`#[non_exhaustive]`, `Default = Off`,
`From<bool>`: `false -> Off`, `true -> RestStress`). `penalty` computes `s = D0 : e0`, its nine first and 45 second
derivatives as before, then the coefficient and its derivatives by the rule: `RestStress` with the same expressions in the same
order as before (bit-identical), `Deviatoric` with `s, ds, d2s` projected (arithmetic 3), `LinearStress` as arithmetic 3. The
rest of the element (the penalty's force and tangent, the local iteration, the condensation) is unchanged.

### D2. Public functions

The parameter `stabilised: bool` of `modes_and_strains_with`, `condensed_force_and_tangent` and
`enhanced_force_and_tangent_with` becomes `stabilised: impl Into<IncompatibleStabilisation>` (every existing call with a
`bool` compiles and means what it meant). Added: `stabilisation_coefficient_with(coords, material, displacement, context,
rule)` and `stabilisation_energy_with(.., modes, context, rule)` (0 for `Off`); the existing `stabilisation_coefficient` and
`stabilisation_energy` call them with `RestStress`. The crate-private `evaluate` takes the rule.

### D3. The opt-in (`model.rs`)

The field `incompatible_stabilisation` becomes an `IncompatibleStabilisation` (default `Off`).
`set_incompatible_stabilisation(rule: impl Into<IncompatibleStabilisation>)` (so `true` / `false` keep their meaning);
`incompatible_stabilisation() -> bool` unchanged in meaning (`rule != Off`); added `incompatible_stabilisation_rule()`.

### D4. Dispatch (`nonlinear.rs`, shared solver file), `nl_transient.rs`, `rotating.rs`

`internal_force_field` and `force_and_tangent` pass `model.incompatible_stabilisation_rule()` (two lines), `evaluate_with` passes
`Off` (one line). `strain_energy` takes the modes and the energy of the model's rule. `unit_centrifugal_model` copies the rule.

### D5. The test files

- `crates/rcfd-fem/tests/si_eas_hg3.rs`: the heavy records (G4) and every fast check below that is not a copy; helpers copied
  from `si_eas_hg2.rs`, generalised to the rule and the `BBar` formulation; the test's own closed forms of `c_D`, `c_L` (own
  code, not the library's).
- `crates/rcfd-fem/tests/si_eas_hg3_rd.rs`, `si_eas_hg3_rl.rs`: the R-* copies, each a two-line file setting the rule and
  including the shared body `crates/rcfd-fem/tests/f703/r_body.rs` (a copy of the R-* part of `si_eas_hg.rs` and its helpers,
  with `STAB` the rule and the R-NT1 own coefficient by the rule; measures, set-ups, seeds and limits of ADR 0608 unchanged).
- `crates/rcfd-fem/tests/si_eas_hg3_bits.rs`: the bit record B3 (D7).

### D6. Heavy record G4 per case

For each case and grid stretch: `m_X,i` (`i <= 10`) for Full, Fbar, Inc, R, D, L; `q_cf / mu` (the closed form of ADR 0628
arithmetic 2, copied); K0 as ADR 0628; the table printed (R2). Tests `g4_sweep_<case>_heavy` for `u30, c30, c45, c49, u45,
u49`, ignored, each run once with `--ignored`.

### D7. The bit record B3 (`si_eas_hg3_bits.rs`)

Written after this commit and run on the base 3f9ce17a before any library code; FNV-1a 64 digests, the expected values printed
by that run and written into the file then (committed with the test, before any library code). Content (API of the base only),
every model opted in by `set_incompatible_stabilisation(true)` and every element call with `true`: `nonlinear::tangent` and
`internal_force` of the 6^3 patch (Incompatible, `nu = 0.3`) at U30 0.6 and of the 4^3 patch at C49 0.5;
`condensed_force_and_tangent`, `modes_and_strains_with`, `enhanced_force_and_tangent_with` on the ADR 0608 T1 (b) element;
`stabilisation_coefficient` and `stabilisation_energy` there; `nonlinear::solve` of the opted 25 x 1 x 1 elastica cantilever
(alpha 1); `Integrator::strain_energy` of the opted 4^3 patch at the E state of ADR 0628.

## Checks

Every measure is required finite before it is compared (a non-finite value misses). `m_X,i` the `i`-th lowest eigenvalue of
ADR 0614 c2 over `mu`, `m_X = m_X,1`. G4 rows are per case (U30, C30, C45, C49, U45, U49) over the 51 grid stretches of the
6^3 patch (the heavy records).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| S1D | c1, c2, c3 | G4: `m_D` at every grid stretch, per case | `>= 0.5` | met U30 (`>= 0.77`), C30 (`~0.53`, margin small), U45 (`>= 1.0`), U49 (`>= 1.05`); **missed C45** (`0.18` at 0.8, `-2.53` at 0.5), **missed C49** (`-0.029` at 0.95, `-67` at 0.5) |
| S1L | (diag.) | G4: `m_L` at every grid stretch, per case | `>= 0.5` | met in all six, smallest `0.559` (U30, 0.5) |
| S1R | c4 | G4: `m_R` at every grid stretch, per case (ADR 0628 S1 repeated, the U30 row of ADR 0614 S1) | `>= 0.5` | `>= 0.9767` |
| K0 | c4 | G4: the plain `K_aa` positive definite at every stretch (ADR 0628 K0) | 51 of 51 | `>= 143 mu h^3` |
| O2 | c4 | G4: for `i <= 10`, `m_Inc,i - m_D,i`, `m_D,i - m_R,i`, `m_R,i - m_Full,i`, `m_Inc,i - m_L,i`, `m_L,i - m_R,i` at every stretch where K0 | `<= 1e-9` each | `<= 0` (arithmetic 4) |
| F1 | c4 | G4: `m_Fbar` at every grid stretch | `>= 0.5` | `>= 0.958` |
| F2 | c4 | G4: `|m_Fbar / (q_cf / mu) - 1|` at every grid stretch | `<= 0.15` | `<= 0.078` (U30, 0.5) |
| LK | c4 | G4: report `Lambda1_X`, `Lambda10_X` for `X = R, D, L, Full, Inc` at every stretch; summary rows | report | `Lambda1` at C49 0.5: R `+0.066`, L `-0.188`, Full `+0.067`; at C49 0.95: R `+0.128`, L `+0.069`; at rest `nu = 0.49`: Inc `+0.054`, Full `+0.188` |
| R2 | c2 | G4: the table `lambda, t, q/mu, m_Full, m_Fbar, m_Inc, m_R, m_D, m_L, c_R, c_D, c_L` | report | arithmetic 1 |
| HF3 | c2, c3, c4 | fast companion, 4^3 patch, six cases at `lambda in {1, 0.8, 0.5}`: O2 on the lowest eigenvalue; `m_L >= 0.5`; `m_Fbar >= 0.5`; `m_D >= 0.5` for U30, C30, U45, U49; negative controls `m_D <= -1` at C49 0.8 and C45 0.5 | as stated | arithmetic 1 (4^3) |
| S2b | c6 | at `u = 0`: `nonlinear::tangent` of the D and L models against the plain model's (6^3 patch, `nu = 0.49`), `max |K_X - K_Inc| / max |K_Inc|` | `<= 1e-14` | 0 (`c = 0`) |
| S3b | c1 | `stabilisation_coefficient_with(.., Deviatoric / LinearStress)` against the test's own closed forms, at element 0 of the 6^3 patch at C49 0.6, U30 0.6, and on the ADR 0608 T1 (b) element at its displacement (a general `F0`) | `<= 1e-12` relative | rounding |
| S3c | c6 | `stabilisation_coefficient_with(.., RestStress)` equal in its bits to `stabilisation_coefficient` and to the base's (via B3); `.., Off)` equal to 0; at the C49 0.6 state `c_D` and `c_L` below `c_R` | bit-equal; `== 0`; holds | |
| S3d | c1 | the bulk modulus does not enter rule D: `c_D` of two isotropic materials with the same `mu` and `kappa / mu = 2.17` and `49.7` at the same `F0` (T1 (b) element) | `<= 1e-12` relative | `0` (rounding) |
| H3c | c5 | the homogeneous state is an equilibrium of the D and L models: `max |f_free| / max |f|`, 6^3 patch, C49 and U30 at 0.5 | `<= 1e-10` each | rounding |
| E1sD, E1sL | c5 | set-up of E1: own `Pi_s` over the total at the E state | `>= 1e-4` | `7.14e-4`, `7.19e-4` |
| E1D, E1L | c5 | `strain_energy` of the D / L model at the E state against the test's own elastic energy at the rule's modes plus the test's own `Pi_s` (own coefficient by the rule) | `<= 1e-12` relative | rounding |
| E3D, E3L | c5 | central difference of `strain_energy` against `internal_force . d` (ADR 0628 E3, `h = 1e-4`, seed `0xF693_0628`) | `<= 1e-6` | `~2e-9` |
| R-*D, R-*L | c5 | ADR 0608 SS1 to SS3, PT1 to PT4, PM, PS, T1 to T4, X1, Q1, CB1 to CB3, CE, NT1 with rule D, and with rule L | the limits of ADR 0608 (as in ADR 0614's table) | as ADR 0614 (`c_D, c_L <= c_R`) |
| B3 | c6 | `si_eas_hg3_bits.rs`: every digest equal to the base's | bit-equal | D1 |
| B4 | c6 | `si_eas_hg_default_bits.rs` (ADR 0614 B1), `si_eas_default_bits.rs` (ADR 0608 B0), the default tests of `si_eas_hg.rs`, `si_eas_hg2.rs`, `si_eas.rs` | pass | |
| S4 | c6 | `cargo test -p rcfd-fem --release` | 0 failed | |
| S5 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | rule D in confined compression near incompressibility (S1D at C45, C49; HF3) | negative, the finding of arithmetic 2 |
| N2 | the plain element (`m_Inc`) in every case | a zero inside the range (ADR 0628 Z), visible in R2 |
| N3 | S3d with rule R instead of D (printed, not a check) | `c_R` changes by the factor `~(kappa/mu)^2` |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg3.rs`, `si_eas_hg3_rd.rs`, `si_eas_hg3_rl.rs`, `si_eas_hg3_bits.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU13 | rule D without the projection (`c_D = c_R`) | S3b, S3d, HF3 (negative controls) |
| MU14 | rule D: the `dev s : d2s` term dropped from `d2c` | R-T1 D (b), (e) |
| MU15 | rule L: the `b (s : ds_a)(s : ds_b)` term dropped | R-T1 L (b), (e) |
| MU16 | rule L: `c_L = s : s / G0` (the softening dropped from `c` only, derivatives kept) | S3b, R-T1 L |
| MU17 | the dispatch passes `RestStress` for any rule that is not `Off` | HF3 (D negative controls), E1D, E1L |
| MU18 | `strain_energy` adds the ADR 0614 energy whatever the rule | E1D, E1L |
| MU19 | `From<bool>` maps `true` to `Deviatoric` | B3 |
| MU20 | rule L: `a(r)` with `G0 + r` in place of `(G0 + r)^2` | R-T1 L, E3L |

The heavy records are not re-run for the mutations; the fast tests cover the same code paths (mutations caught only by a heavy
test are not claimed).

## Decision rule

- **done**: every check above met in its single registered run (S1D in all six cases included), the negative controls as
  stated, every planned mutation caught, B3, B4, S4, S5 met, no amendment needed. (Not expected: arithmetic 1 predicts S1D
  missed at C45 and C49.)
- **partial**: B3, B4, S4, S5, S3b, S3d, O2, F1, F2 and the R-*D checks met, but S1D missed in a case (recorded as
  `*_known_miss`, diagnosed), or a mutation not caught, or a check met only after an amendment, or a diagnostic check of rule L
  missed.
- **todo**: B3, B4, S4 or S5 fails and cannot be fixed without changing a default result, or an R-*D check is missed, or a heavy
  record cannot be run.
- The checks of rule L (S1L, R-*L, E1L, E3L, HF3's `m_L`) are diagnostic: they decide the recommendation of open question 1,
  not the clause c1. A missed S1D is a finding about every coefficient without the bulk modulus (arithmetic 2), not a reason to
  tune rule D here.

## Limits (not covered)

- One material law (neo-Hookean), one patch (6^3, every boundary node prescribed), two homogeneous load paths in compression to
  `lambda = 0.5`; no tension, shear, free boundary or other mesh density in the heavy record.
- The locking measure is the lowest (and the ten lowest) eigenvalues of the fixed-boundary patch against the F-bar Hex8 on the
  same patch: a stiffness measure of the softest modes, not the response to a load. In deep confined compression the softest
  mode is not dominated by volumetric locking (arithmetic 1: the Full Hex8 is within 7 per cent of F-bar at C49 0.5, 19 per cent
  at rest), so the measure there reads mostly the stabilisation's softness or stiffness, not locking alone.
- The arithmetic 2 argument is a scaling bound of the destabilising term, not a proof of the need.
- For anisotropic rest elasticity rule D removes the mean normal stress, not a bulk modulus (there is none); J2, creep and
  viscoelastic materials take their elastic base, as ADR 0614.
- No case-file key (as ADR 0614); the element-level helpers of `nonlinear` stay the ADR 0608 element.

## Open questions (taken or open)

1. **Which coefficient for nearly incompressible compression.** Options: (a) rule D as the acceptance asks, documented as
   unstable in confined compression near incompressibility; (b) rule L (the bulk modulus linear, through the stress); (c) a
   coefficient from the current stress of the material (the volumetric second Piola-Kirchhoff stress of arithmetic 2), which
   needs a third derivative of the energy for its exact tangent (not available from `Material::stress`). Taken for this item:
   build (a) and (b) as separate opt-ins, measure both, recommend after the run, on the maintainer's standing instruction of
   2026-11-11 (take the recommendation). Open for the maintainer: whether "without the bulk modulus" is to be read literally
   (then the item ends at the finding of arithmetic 2) or as "without the bulk modulus squared" (then rule L is the candidate of
   a follow-up acceptance).
2. **The locking-free reference.** F-bar (ADR 0506) rather than a mixed `u-p` Hex8 (none in the nonlinear driver). Taken on
   the same instruction.
3. **Default.** All rules stay opt-in; open for the maintainer, as ADR 0614 and ADR 0628.
