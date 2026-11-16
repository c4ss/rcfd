# ADR 0628: F-693 registration: the stabilised incompatible-modes Hex8 beyond the F-685 set-up (its strain energy at a large strain, confined compression, and nu = 0.45 and 0.49)

Status: **REGISTERED 2026-11-16, written before any code of F-693 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test or library code of F-693 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the
scratch folder (`w49i-work/f693-scratch`; no rcfd code; disclosed, their numbers quoted below): `probe_f693.py` (the ADR 0614
sweep for each registered case, with the closed form of arithmetic 2; on top of the ADR 0614 probes `probe_hg.py` and
`probe_need.py`, copied unchanged from `w49i-work/f685-scratch`; logs `p_n4_nu*_*.log`, `p_n6_nu*_*.log`), `probe_ls.py` (the
zero of the plain element by 20 bisections; logs `ls_*.log`) and `probe_energy.py` (the set-up of the energy checks E: the
modes by a Newton minimisation of the element energy; its log is quoted in arithmetic 5). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-693 (deps F-685), the follow-up proposed by [ADR 0614](0614-f685-registration-hourglass-stability-of-the-incompatible-hex8-in-compression.md)
(remaining questions 2 and 3 there). The element is the incompatible-modes Hex8 at finite strain of
[ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) (F-596) with the opt-in stabilisation of ADR 0614
D1 (`SolidModel::set_incompatible_stabilisation`). Number 0628 given by the lead. Branch `f693-work` from the local main tip
9cf95fc3, worktree `w49i-work/f693`, scratch `w49i-work/f693-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end. The lead's reading of ADR 0608 (a check met only after an amendment makes the proposal partial) is applied
in the decision rule.

## What F-693 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The strain energy of the ADR 0614 stabilised element at a large strain against the test's own stabilisation energy (the missed mutation MU7 of ADR 0614 caught); the ADR 0614 sweep repeated for confined compression and for nu = 0.45 and 0.49, the lowest eigenvalue of the stabilised element reported against the Full Hex8 and the closed-form bound over a registered range; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the strain energy of the stabilised element at a large strain against the
test's own stabilisation energy, MU7 of ADR 0614 caught; **c2** the ADR 0614 sweep repeated for confined compression; **c3** and
for `nu = 0.45` and `0.49`; **c4** the lowest eigenvalue of the stabilised element reported against the Full Hex8; **c5** and
against the closed-form bound; **c6** over a registered range; **c7** the default bits unchanged.

### Readings, registered

- **c1: the strain energy at a large strain.** `nl_transient::Integrator::strain_energy` (the path of ADR 0614 D4, the one MU7
  removed a term from) evaluated at a state given to `Integrator::start` (no step taken): the 4 x 4 x 4 distorted patch of ADR
  0614 (seed `0xF685_0614`), the neo-Hookean material of ADR 0614 (`mu = 1e6 Pa`, `nu = 0.3`), no constraint (`start` takes no
  non-zero value at a prescribed equation, so nothing is prescribed), the field `u = (F - I) X + beta (x z, y z, 0)` with `F =
  diag(1, 1, 0.6)` (40 per cent confined compression) and `beta = 0.2` (a bending part: the modes are active). The reference
  is the test's own: the elastic energy `sum_q w_q W(E_q)` of the test's own neo-Hookean `W` at the enhanced strains of
  `modes_and_strains_with(.., true)`, plus the test's own `Pi_s` (the coefficient by the closed form of ADR 0614 arithmetic 4
  (iii), the penalty from the test's own mode gradients, as in ADR 0614 R-NT1). The probe (arithmetic 5) puts `Pi_s` at
  `5.3e-4` of the total, nine orders above the `1e-12` limit, so MU7 cannot hide below the limit as it did in ADR 0614 (`2.5e-13`).
  A second, independent reading of the same path: the strain energy is the potential of the stabilised internal force (E3).
- **c2, c3: the cases.** The ADR 0614 sweep (the 6 x 6 x 6 patch of the unit cube with interior nodes moved by `0.1 h (2U - 1)`,
  seed `0xF685_0614`, every boundary node prescribed; the homogeneous state `u = (F - I) X`; the lowest eigenvalue `m_X` of the
  pencil `(K_ff, L_ff)` over `mu` with `L` the Full Hex8 vector Laplacian; the neo-Hookean energy of ADR 0541 with `mu = 1e6
  Pa`, `kappa = 2 mu (1 + nu) / (3 (1 - 2 nu))`, density 1000) for five cases: **C30, C45, C49** confined compression `F =
  diag(1, 1, lambda)` at `nu = 0.3, 0.45, 0.49`, and **U45, U49** uniaxial stress `F = diag(t, t, lambda)`, `S_11(t) = 0` (as
  ADR 0614), at `nu = 0.45, 0.49`. (U30 is ADR 0614's run and is not repeated.) `kappa / mu = 2.1667, 9.6667, 49.667`.
- **c6: the range.** `lambda in [0.5, 1]` on the grid `lambda_j = 1 - 0.01 j`, `j = 0..50`, as ADR 0614, for every case.
- **c4: "against the Full Hex8".** The ratio `m_Stab / m_Full` is reported at every grid stretch, with the inequality `m_Inc <=
  m_Stab <= m_Full` of arithmetic 3 as a check (O1): it holds whatever the accuracy of either element. Near incompressibility the
  Full Hex8 locks (at rest on the 6^3 patch `m_Full = 1.190` against `m_Inc = 1.056` at `nu = 0.49`, probe), so the Full Hex8 is
  not a reference of the continuum there and **no accuracy limit is set on `m_Stab / m_Full`**: it is reported (R1), and a ratio
  near 1 is read as the stabilised element approaching the locking Full Hex8, not as accuracy.
- **c5: the closed-form bound.** `q(lambda) = min_{|N| = 1} lambda_min(Q(N))` of the acoustic tensor (ADR 0614 c3; Van Hove's
  argument, recalled from memory there and derived in one line: while `q > 0` the continuum with its boundary fixed has no
  bifurcation and every zero eigenvalue of a discrete tangent is spurious; for a conforming element with exact integration `m_Full
  >= q / mu`). For the neo-Hookean energy at `F = diag(tau, tau, lambda)` (both load paths) it is derived in closed form in
  arithmetic 2, as the brief asks, and checked against ADR 0614's numerical minimisation over a 1-degree sphere grid of `A` built
  from `Material::stress` (G2).
- **c7: default bits.** No library code is planned (the item is tests). The default bits are then unchanged by construction; the
  check is that no file under `crates/*/src` differs from the base 9cf95fc3 at the tip (B1), and the bit records B0 of ADR 0608
  and B1 of ADR 0614 pass (B2). If a diagnosed defect needs a library change, an amendment registers it before the change (opt-in,
  and then `baseline_hashes` is run).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0614 (whole: c1 to c7, arithmetic 1 to 6, D1 to D6, the checks, the mutations, addendum 1
  and its remaining questions); ADR 0608 (head, D1, D4); ADR 0572, 0573 and 0614 (the form of the roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (`rest_moduli`, `centre_gradient`, `mean_strain`, `penalty`, the penalty branch of
  `extended`, `condensed_force_and_tangent`, `stabilisation_coefficient`, `stabilisation_energy`, `enhanced_force_and_tangent_with`,
  `modes_and_strains_with`), `nl_transient.rs` (`Integrator::new` head, `start`, `current`, `strain_energy`), `nl_transient/dg.rs`
  (`energy`), `mat_hyper.rs` (`energy_parts`, `energy`), `model.rs` (`set_incompatible_stabilisation`, `validate` head), `lib.rs`
  (the public modules).
- Tests: `crates/rcfd-fem/tests/si_eas_hg.rs` (whole: the helpers, `hg_patch`, `lateral`, `homogeneous`, `interior`,
  `dense_free`, `laplacian`, `lowest`, `q_min`, `own_coefficient`, the heavy sweep, HF, H3/S3, R-NT1).
- The ADR 0614 probes `w49i-work/f685-scratch/probe_hg.py`, `probe_need.py`, `probe_rule.py`, `probe_bloch.py` (head).

## Arithmetic done before the code (disclosed)

1. **The sweep for the five cases** (`probe_f693.py`, the 6^3 patch, step 0.05; the probe reproduces ADR 0614's measured U30
   numbers on the 4^3 patch to the printed digits: `m_Stab` `0.99877, 1.03909, 1.14762, 1.26045` at 1, 0.8, 0.6, 0.5). Over `mu`:

   | case | `lambda` | `t` | `q / mu` | `m_Full` | `m_Inc` | `m_Stab` | `c / mu` |
   |---|---|---|---|---|---|---|---|
   | all | 1.00 | 1 | 1.0000 | 1.0095 (nu .3), 1.0380 (.45), 1.1901 (.49) | 0.9767, 1.0051, 1.0555 | = `m_Inc` | 0 |
   | C30 | 0.80 | 1 | 1.1475 | 1.1671 | 0.8608 | 1.0913 | 0.891 |
   | C30 | 0.50 | 1 | 1.3966 | 1.5271 | -1.2346 | 1.4699 | 14.72 |
   | C45 | 0.80 | 1 | 1.1568 | 1.1884 | -0.1888 | 1.1731 | 15.05 |
   | C45 | 0.50 | 1 | 1.5036 | 1.5972 | -10.226 | 1.5928 | 248.7 |
   | C49 | 0.95 | 1 | 1.0348 | 1.2145 | -0.0547 | 1.1689 | 19.55 |
   | C49 | 0.80 | 1 | 1.1597 | 1.3006 | -33.30 | 1.2919 | 393.8 |
   | C49 | 0.50 | 1 | 1.5664 | 1.6427 | -189.29 | 1.6402 | 6506.5 |
   | U45 | 0.80 | 1.10631 | 1.0098 | 1.0482 | 0.9882 | 1.0097 | 0.502 |
   | U45 | 0.50 | 1.36938 | 1.0068 | 1.0636 | -0.9868 | 1.0553 | 24.24 |
   | U49 | 0.80 | 1.11575 | 1.0018 | 1.1793 | 1.0410 | 1.0763 | 1.024 |
   | U49 | 0.50 | 1.40583 | 1.0005 | 1.1181 | -0.1445 | 1.1116 | 307.4 |

   In every case `m_Full >= q / mu` (smallest margin `9.5e-3`, C30 at rest) and `m_Inc <= m_Stab <= m_Full` at every probed
   stretch; `m_Stab` is smallest at rest in every case (`0.9767`, `1.0051`, `1.0555`) and grows with the compression. The plain
   element has a spurious zero in every case (`probe_ls.py`, 20 bisections): **C30 `0.64179`, C45 `0.80937`, C49 `0.95075`,
   U45 `0.56444`, U49 `0.51033`** (U30 of ADR 0614: `0.60797`). Confined compression is far worse for the plain element than
   uniaxial stress, and more so near incompressibility (at `nu = 0.49` the zero is at 5 per cent compression, and `m_Inc = -189`
   at 0.5): the destabilising geometric term `delta F S delta F^T` of ADR 0614 arithmetic 2 carries the pressure, which in
   confined compression is `~kappa (1 - lambda)`. The coefficient `c` grows with it (`c ~ |D0 : e0|^2 / G0 ~ lambda_L^2 e^2 /
   mu`: 6506 `mu` at C49, 0.5), and `m_Stab / m_Full` approaches 1 (`0.9985` at C49, 0.5; `0.994` at U49, 0.5): the stabilised
   element is driven towards the Full Hex8, which locks near incompressibility (c4 reading). On the 4^3 patch (`p_n4_*.log`):
   `m_Inc` at 0.8 `1.0114` (C30), `0.5172` (C45), `-23.014` (C49); at 0.5 `-0.5795` (C30), `-7.4009` (C45), `-0.1771` (U45),
   `+0.7285` (U49, no zero on the 4^3 patch); `m_Stab >= 0.9988` everywhere.
2. **The closed-form acoustic bound for the neo-Hookean energy at `F = diag(tau, tau, lambda)`** (c5). `W(F) = (mu/2)(J^{-2/3}
   I1 - 3) + (kappa/2)(J - 1)^2`, `I1 = F : F`, `J = det F` (ADR 0541 D1). The acoustic tensor is `a . Q(N) a = d^2/ds^2 W(F +
   s a (x) N)` at `s = 0`. For the rank-one direction: `J(s) = J (1 + s a . m)` exactly, `m = F^{-T} N` (`det(F + s a (x) N) =
   J det(I + s F^{-1} a (x) N) = J (1 + s N . F^{-1} a)`), so `J'' = 0`; `I1(s) = I1 + 2 s a . b + s^2 |a|^2`, `b = F N`. With
   `(J^{-2/3})' = -(2/3) J^{-2/3} (a . m)`, `(J^{-2/3})'' = (10/9) J^{-2/3} (a . m)^2`:
   `d^2/ds^2 (J^{-2/3} I1) = J^{-2/3} [ (10/9) I1 (a . m)^2 - (8/3) (a . m)(a . b) + 2 |a|^2 ]` and `d^2/ds^2 (kappa/2)(J - 1)^2 =
   kappa J^2 (a . m)^2`. Hence
   `Q(N) = mu J^{-2/3} [ I + alpha m (x) m - gamma (m (x) b + b (x) m) ]`, `alpha = (5/9) I1 + (kappa / mu) J^{8/3}`, `gamma = 2/3`.
   The bracket is the identity plus a symmetric operator of rank at most two on `span{m, b}`: `U B U^T` with `U = [m b]`, `B =
   [[alpha, -gamma], [-gamma, 0]]`; its non-zero eigenvalues are those of `B G`, `G = U^T U = [[|m|^2, 1], [1, |b|^2]]` (`m . b =
   N . N = 1`): trace `T = alpha |m|^2 - 2 gamma`, determinant `-gamma^2 (|m|^2 |b|^2 - 1) <= 0` (Cauchy-Schwarz), so
   `mu_- = T/2 - sqrt(T^2/4 + gamma^2 (|m|^2 |b|^2 - 1)) <= 0` is the smallest, and every direction orthogonal to `m` and `b` has
   the eigenvalue 1. Therefore **`lambda_min(Q(N)) = mu J^{-2/3} (1 + mu_-(N))`** (also when `m` is parallel to `b`, on the axes:
   then `mu_- = min(T, 0)`). For `F = diag(tau, tau, lambda)` and `N = (sin th cos ph, sin th sin ph, cos th)`: `|m|^2 = sin^2 th
   / tau^2 + cos^2 th / lambda^2`, `|b|^2 = tau^2 sin^2 th + lambda^2 cos^2 th`, `J = tau^2 lambda`, `I1 = 2 tau^2 + lambda^2`,
   independent of `ph` (the state is transversely isotropic about `z`), so **`q = min_{th in [0, pi/2]} mu J^{-2/3} (1 +
   mu_-(th))`**, a one-dimensional minimisation of a closed-form function. Confined (`tau = 1`): `|m|^2 |b|^2 - 1 = sin^2 th cos^2
   th (lambda - 1/lambda)^2`; on the axes `th = 0, pi/2` the value is `mu J^{-2/3} >= mu` (there `T = alpha / lambda^2 - 4/3 >=
   5/3 - 4/3 > 0` and `T = alpha - 4/3`, positive over the range), so the minimum is interior (probe: `th_min` from 46 to 63
   degrees over the range). The test evaluates it on the 9001-point grid `th_i = (i / 9000)(pi/2)` (0.01 degree), which contains
   every integer degree of the sphere grid of G2, so the closed form is at most the grid value up to rounding. Probe (`q_cf`
   against ADR 0614's complex-step `A` on a 2-degree sphere grid): agreement to `<= 4e-5` relative in every case (the grid's
   discretisation; at 1 degree about a quarter of it), `q_cf <= q_grid` everywhere; U30 at 0.5 reproduces ADR 0614's `1.1449`.
   This derivation is the repository's (first principles); no published closed form is used.
3. **The ordering `m_Inc <= m_Stab <= m_Full`** (O1, c4). At a homogeneous state the modes are zero (ADR 0614 arithmetic 5),
   so the penalty adds only `c M` to the modes' block: `K_aa,Stab = K_aa + c M` (`c >= 0`, `M` positive semi-definite), while
   `K_uu` and `K_ua` are those of the plain element and `K_uu` is the Full Hex8's tangent (the same 2 x 2 x 2 rule at `F_enh =
   0`). If `K_aa` is positive definite in every element, `0 < (K_aa + c M)^{-1} <= K_aa^{-1}` (Loewner order), so element by
   element `K_Inc = K_uu - K_ua K_aa^{-1} K_au <= K_Stab = K_uu - K_ua (K_aa + c M)^{-1} K_au <= K_uu = K_Full`; the order is
   kept by assembly and by taking the free block, and the Courant-Fischer theorem for the pencil with the same `L` gives `m_Inc <=
   m_Stab <= m_Full` exactly, at every stretch where every `K_aa` is positive definite (K0). Probe: the smallest eigenvalue of
   `K_aa` over the elements, over `mu h^3`, is `>= 143` in every case and stretch (it is not `K_aa` that fails; the plain
   element's zero is in the condensed tangent, ADR 0614 arithmetic 2). So the inequality is a check that holds whether or not
   either element is accurate, which is what "against the Full Hex8" can mean where the Full Hex8 locks.
4. **Cost.** As ADR 0614 arithmetic 6: about a second per dense 375 x 375 pencil in release; a case is 51 x 3 pencils plus 20
   bisections of `m_Inc` plus two at `lambda_s`, about three minutes. Each case is a heavy record (`#[ignore]`, run once with
   `--ignored`), with the fast companion HF2 on the 4^3 patch through the same code.
5. **The E set-up** (`probe_energy.py`, c1): the modes of each element minimise `sum_q w_q W(F_q) + (c/2) alpha^T M alpha`
   (Newton, `|grad| < 1e-13`). Stabilised: elastic energy `0.32389377 mu`, stabilisation energy `1.7266e-4 mu` (unit cube),
   ratio `5.33e-4`; plain: elastic energy `0.32335492 mu`. Rejected before choosing (same probe, disclosed): the fields `beta (y
   z, z x, x y)` (`beta = 0.1`, ratio `1.8e-6`) and `beta (z^2, x^2, y^2)` (`beta = 0.1`, `0.2`: `9.1e-7`, `3.3e-6`), whose
   gradients a Hex8 represents without parasitic shear so the modes stay small; `beta (x z, 0, 0)` gave `2.4e-4`, `beta (x z, y
   x, z y)` `8.6e-4`. `beta (x z, y z, 0)` is taken (bending in two planes, `det(F + beta grad) = 0.6 (1 + beta z)^2 > 0`).
   The central difference of E3: `h = 1e-4` on a direction `d` of size `1e-2` (displacement step `1e-6`): truncation `O(h^2 |d|^3
   W''')` negligible, rounding `~ eps W / (h |d|) ~ 2e-16 * 0.32 mu / 1e-6 ~ 6e-11 mu` against `f . d ~ 1e-2 mu`: `~1e-8`
   relative; the local mode iteration's error enters at second order (the condensed energy is stationary in the modes).

## Design

### D1. No library change

The checks need no library code: every function used is public since ADR 0614 (`set_incompatible_stabilisation`,
`modes_and_strains_with`, `enhanced_force_and_tangent_with`, `stabilisation_coefficient`, `Integrator::start` and
`strain_energy`, `nonlinear::tangent`, `internal_force`). A library change is made only for a diagnosed defect, by an amendment
written before it (opt-in, defaults bit-identical, `baseline_hashes` run).

### D2. The test file `crates/rcfd-fem/tests/si_eas_hg2.rs`

New (existing tests not edited); the helpers are copies of `si_eas_hg.rs` generalised to a Poisson ratio and a load path:
`material(nu)`, `patch(n, nu, formulation, stabilised)`, `lateral(nu, lambda)` (200 bisections of `S_11` on `[1, 2]`, as ADR
0614), `state(case, lambda) -> (tau, lambda)`, `homogeneous`, `interior`, `dense_free`, `laplacian`, `lowest`, `q_grid(nu, tau,
lambda)` (ADR 0614's `q_min`, the 1-degree sphere grid of `A = delta S + F F C` from `Material::stress`), `q_closed(nu, tau,
lambda)` (arithmetic 2), `kaa_min(model, u)` (the smallest eigenvalue of the 9 x 9 modes block of
`enhanced_force_and_tangent_with(.., modes = 0, false)` over the elements, over `mu h^3`), `own_coefficient` (copy), and the
test's own neo-Hookean energy `own_w(E)` (`C = I + 2E`, `J = sqrt(det C)`, `W` of arithmetic 2). Tests:
`g_sweep_c30_heavy`, `g_sweep_c45_heavy`, `g_sweep_c49_heavy`, `g_sweep_u45_heavy`, `g_sweep_u49_heavy` (ignored, each run once
with `--ignored`), `hf2_fast_companion_on_a_4_cube_patch`, `h3b_equilibrium_near_incompressibility`, `e_strain_energy_at_a_large_strain`.

## Checks

Every measure is required finite before it is compared (a non-finite value misses). `m_X` as ADR 0614 c2 (over `mu`). G, K0, O1,
S1, Z, R1 run per case (C30, C45, C49, U45, U49) on the 6^3 patch over the 51 grid stretches (the heavy records).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| G0 | c3 | U45, U49: `|S_11| / |S_33|` at the solved `t` (50 stretches `< 1`), `|S_11| / mu` at 1 | `<= 1e-12` | `1.7e-14` (ADR 0614) |
| G1 | c5 | `q_cf / mu` (arithmetic 2) at every grid stretch | `>= 0.9` | `>= 1.0` |
| G2 | c5 | the closed form against the sphere grid: `(q_grid - q_cf) / q_cf` at every grid stretch | in `[-1e-10, 1e-4]` | `0` to `~1e-5` |
| G3 | c5 | `m_Full - q_cf / mu` at every grid stretch | `>= -1e-9` | `>= 9.5e-3` |
| K0 | c4 | the smallest eigenvalue of `K_aa` (plain) over the elements, over `mu h^3`, at every grid stretch | `> 0` (the condition of O1; reported) | `>= 143` |
| O1 | c4 | `m_Inc - m_Stab` and `m_Stab - m_Full` at every grid stretch where K0 holds | `<= 1e-9` each | `<= 0` (arithmetic 3) |
| S1 | c4, c6 | `m_Stab` at every grid stretch | `>= 0.5` | `>= 0.9767` (C30), `1.0051` (C45, U45), `1.0555` (C49, U49) |
| Z | c2, c3 | `lambda_s` of the plain element (the first grid sign change of `m_Inc` refined by 20 bisections) | within `0.01` of the probe: C30 `0.64179`, C45 `0.80937`, C49 `0.95075`, U45 `0.56444`, U49 `0.51033` | the probe |
| Z5 | c5 | at `lambda_s`: `q_cf / mu` and `m_Full` | `>= 0.9` each | `>= 1.0` |
| R1 | c4, c5 | report: the table `lambda, t, q_cf/mu, m_Full, m_Inc, m_Stab, m_Stab/m_Full, m_Stab/(q_cf/mu), c/mu` at every grid stretch; the Full Hex8's locking excess `m_Full / m_Inc` at rest (`nu = 0.45`, `0.49`) | report (no limit, c4 reading) | `1.033`, `1.128` |
| HF2 | c2 to c5 | fast companion, 4^3 patch, the five cases at `lambda in {1, 0.8, 0.5}`: G2, G3, O1 (with K0), `m_Stab >= 0.5`; negative controls `m_Inc <= -1` at C49 0.8 and C45 0.5, `m_Inc <= -0.1` at C30 0.5 | as stated | arithmetic 1 (4^3) |
| H3b | c2, c3 | the homogeneous state is an equilibrium of the stabilised element: `max |f_free| / max |f|` of `internal_force`, 6^3 patch, C49 and U49 at 0.5 | `<= 1e-10` each | rounding (`1.7e-15` at nu 0.3, ADR 0614) |
| E0 | c1 | plain model (not opted in): `strain_energy` against the test's `sum_q w_q own_w(E_q)` at the plain modes, relative | `<= 1e-12` | rounding |
| E1s | c1 | set-up: the test's own `Pi_s` over the total of the stabilised model | `>= 1e-4` | `5.3e-4` |
| E1 | c1 | stabilised model: `strain_energy` against `sum_q w_q own_w(E_q)` at the stabilised modes plus the test's own `Pi_s`, relative | `<= 1e-12` | rounding |
| E3 | c1 | stabilised model: `(W(u + h d) - W(u - h d)) / (2h)` of `strain_energy` against `internal_force(u) . d`, relative (`h = 1e-4`, `d` SplitMix64 seed `0xF693_0628` uniform in `[-1e-2, 1e-2]` per equation) | `<= 1e-6` | `~1e-8` |
| B1 | c7 | `git diff --stat 9cf95fc3 -- crates/*/src` at the tip | empty | D1 |
| B2 | c7 | `si_eas_default_bits.rs` (ADR 0608 B0), `si_eas_hg_default_bits.rs` (ADR 0614 B1), `si_eas_hg.rs` (default tests) | pass | unchanged code |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

`cargo test -p rcfd-fem --release` as a whole and `baseline_hashes` are not run unless a library file changes (rule 3 and 7: no
shared solver file is touched; the lead runs the gate).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the plain element in each case (Z): a zero inside the range | yes in all five (arithmetic 1) |
| N2 | the plain element on the 4^3 patch (HF2) | `m_Inc(0.8) = -23.0` (C49), `m_Inc(0.5) = -7.40` (C45), `-0.580` (C30) |
| N3 | E0 against E1: the plain model's strain energy carries no stabilisation energy | E0 met with the plain modes and no `Pi_s` |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg2.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU7 | (ADR 0614's) `strain_energy` without `Pi_s` (the `w += stabilisation_energy(..)` line removed) | E1, E3 |
| MU8 | `Pi_s` added for every Incompatible element (the `if stabilised` guard in `strain_energy` removed) | E0 |
| MU9 | `stabilisation_energy` returns `p.c * pa` (the factor 1/2 dropped) | E1, E3 |
| MU10 | `strain_energy` of a stabilised model at the plain modes (`modes_and_strains_with(.., false)`), `Pi_s` still added | E1, E3 |
| MU11 | the `c M` term dropped from the modes' tangent block in `extended` (force kept) | HF2 (`m_Stab` at C49 0.8 below 0.5) |
| MU12 | `c` from the Green strain (`e0 = (C0 - I)/2`, ADR 0614 MU3) | E1 (the test's own coefficient) |

The heavy records are not re-run for the mutations; the fast tests cover the same code paths (mutations caught only by a heavy
test are not claimed).

## Decision rule

- **done**: every check above met in its single registered run, the negative controls as stated, every planned mutation caught by
  a registered check (MU7 in particular), B1 and B2 met, no amendment needed.
- **partial**: E1 and B1, B2 met (MU7 caught), but a sweep check (G, O1, S1, Z) is missed (recorded as `*_known_miss`,
  diagnosed), or another mutation is not caught, or a check is met only after an amendment.
- **todo**: E1 missed or MU7 not caught, or B1 or B2 fails, or a heavy record cannot be run.
- A missed S1 is a finding about the stabilisation (reported with its stretch and case), not a reason to tune its coefficient
  here: a changed coefficient changes the opt-in's results and is a follow-up.

## Limits (not covered)

- One material law (neo-Hookean), one patch (6^3, every boundary node prescribed), two homogeneous load paths in compression to
  `lambda = 0.5`; no tension, no shear, no free boundary, no other mesh density in the heavy record (the 4^3 companion only).
- "Against the Full Hex8" near incompressibility is the inequality O1 and a reported ratio, not an accuracy measure: whether the
  stabilised element locks in nearly incompressible compression (its `m_Stab / m_Full` near 1 in the probes) is reported, not
  checked against a locking-free reference (none in the repository for this state).
- The energy checks use one large-strain state of one material; the dynamic path (energy drift of a stabilised run) is not
  checked.
- The coefficient of ADR 0614 D1 is used as built; its over-stabilisation near incompressibility (the bulk modulus enters `c`
  squared through `D0`) is measured, not changed (open question 2).

## Open questions (taken or open)

1. **The cases.** Reading c2 and c3 as five cases (confined at three Poisson ratios, uniaxial at the two new ones) rather than
   two (confined at 0.3; uniaxial at 0.45, 0.49 only). Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation): the full 2 x 3 table minus ADR 0614's run costs three more heavy records and answers the near-incompressible
   confined case, the probes' worst.
2. **The coefficient near incompressibility.** Whether `c` should take only the deviatoric part of the rest stress (so that the
   bulk modulus does not drive the element to the locking Full Hex8) changes the opt-in's results and is left open: a follow-up.
3. **Default.** As ADR 0614: the stabilisation stays opt-in; open for the maintainer.

## Addendum 1: results (2026-11-16)

Registered runs, each made once at 490d2a86 (release, the test commit; no library file changed): the default tests of
`si_eas_hg2.rs` (`f693-scratch/run1-default.log`, `--test-threads 2`, 3 passed in 1.1 s), the five heavy records with
`--ignored` (`run1-heavy.log`, `--test-threads 3`, 5 passed in 332 s), B2 (`run1-b2.log`). No amendment was written.

| Id | Limit | Measured | Met |
|---|---|---|---|
| G0 U45; U49 | `<= 1e-12` | `1.01e-13`; `4.38e-13` | yes |
| G1 `q_cf / mu`, every case | `>= 0.9` | `1.0000` (at rest; at 0.5: C30 `1.3966`, C45 `1.5036`, C49 `1.5664`, U45 `1.0068`, U49 `1.0005`) | yes |
| G2 `(q_grid - q_cf) / q_cf` smallest; largest | `>= -1e-10`; `<= 1e-4` | `>= -1.2e-14`; C30 `3.8e-5`, C45 `1.0e-5`, C49 `3.4e-6`, U45 `8.5e-6`, U49 `2.1e-6` | yes |
| G3 `m_Full - q_cf / mu` | `>= -1e-9` | C30 `9.4e-3`, C45 `3.1e-2`, C49 `7.6e-2`, U45 `3.8e-2`, U49 `0.118` | yes |
| K0 `K_aa` positive definite; smallest over `mu h^3` | `> 0` | 51 of 51 stretches in every case; `156.6`, `156.6`, `143.1`, `156.6`, `155.6` | yes |
| O1 `m_Inc - m_Stab`; `m_Stab - m_Full` | `<= 1e-9` | never positive at any of the 255 stretches (the aggregate starts at 0 and printed `0`) | yes |
| S1 `m_Stab` | `>= 0.5` | C30 `0.9767`, C45 `1.0051`, C49 `1.0555`, U45 `1.0051`, U49 `1.0553` (at or next to rest) | yes |
| Z `|lambda_s - probe|` | `<= 0.01` | C30 `0.6417912` (`1.2e-6`), C45 `0.8093661` (`3.9e-6`), C49 `0.9507467` (`3.3e-6`), U45 `0.5644384` (`1.6e-6`), U49 `0.5103290` (`1.0e-6`) | yes |
| Z5 `q_cf / mu`; `m_Full` at `lambda_s` | `>= 0.9` | `>= 1.0007`; `>= 1.0584` | yes |
| R1 | report | the table below | report |
| HF2 (15 stretches, every row) | as registered | every row met; N2 `m_Inc` `-23.01` (C49 0.8), `-7.401` (C45 0.5), `-0.5795` (C30 0.5) | yes |
| H3b C49; U49 | `<= 1e-10` | `8.7e-16`; `1.6e-14` | yes |
| E0 | `<= 1e-12` | `1.8e-16` | yes |
| E1s | `>= 1e-4` | `5.328e-4` (`Pi_s = 1.7266e-4 mu` of `0.32407 mu`) | yes |
| E1 | `<= 1e-12` | `5.4e-16` | yes |
| E3 | `<= 1e-6` | `2.0e-9` | yes |
| B1 `git diff --stat 9cf95fc3 -- crates/*/src` | empty | empty | yes |
| B2 `si_eas_default_bits`, `si_eas_hg_default_bits`, `si_eas_hg` | pass | 1, 1, 8 passed | yes |
| S6 fmt; clippy | clean | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | yes |

R1, the lowest eigenvalue over `mu` (selected rows of 51 per case; `s/f = m_Stab / m_Full`, `s/q = m_Stab / (q_cf / mu)`):

| case | `lambda` | `q_cf/mu` | `m_Full` | `m_Inc` | `m_Stab` | `s/f` | `s/q` | `c/mu` |
|---|---|---|---|---|---|---|---|---|
| C30 | 1.00 | 1.0000 | 1.0095 | 0.9767 | 0.9767 | 0.9675 | 0.9767 | 0 |
| C30 | 0.80 | 1.1475 | 1.1671 | 0.8608 | 1.0913 | 0.9351 | 0.9511 | 0.891 |
| C30 | 0.50 | 1.3966 | 1.5271 | -1.2346 | 1.4699 | 0.9626 | 1.0525 | 14.72 |
| C45 | 1.00 | 1.0000 | 1.0380 | 1.0051 | 1.0051 | 0.9683 | 1.0051 | 0 |
| C45 | 0.80 | 1.1568 | 1.1883 | -0.1888 | 1.1731 | 0.9871 | 1.0140 | 15.05 |
| C45 | 0.50 | 1.5036 | 1.5972 | -10.226 | 1.5928 | 0.9972 | 1.0593 | 248.7 |
| C49 | 1.00 | 1.0000 | 1.1901 | 1.0555 | 1.0555 | 0.8869 | 1.0555 | 0 |
| C49 | 0.95 | 1.0348 | 1.2145 | -0.0547 | 1.1689 | 0.9625 | 1.1296 | 19.55 |
| C49 | 0.80 | 1.1597 | 1.3006 | -33.299 | 1.2919 | 0.9934 | 1.1141 | 393.8 |
| C49 | 0.50 | 1.5664 | 1.6427 | -189.29 | 1.6402 | 0.9985 | 1.0471 | 6506.5 |
| U45 | 0.80 | 1.0098 | 1.0482 | 0.9882 | 1.0097 | 0.9632 | 0.9999 | 0.502 |
| U45 | 0.50 | 1.0068 | 1.0635 | -0.9868 | 1.0553 | 0.9923 | 1.0482 | 24.24 |
| U49 | 0.80 | 1.0018 | 1.1792 | 1.0410 | 1.0763 | 0.9127 | 1.0743 | 1.024 |
| U49 | 0.50 | 1.0005 | 1.1181 | -0.1445 | 1.1116 | 0.9943 | 1.1110 | 307.4 |

The Full Hex8's locking excess at rest, `m_Full / m_Inc`: `1.0336` (`nu = 0.3`), `1.0327` (0.45), `1.1276` (0.49).

With the stabilisation of ADR 0614 the lowest eigenvalue stays `>= 0.977 mu` over `[0.5, 1]` in all five new cases. It lies
between the plain element and the Full Hex8, as arithmetic 3 says, and is above the closed-form bound `q` except at small
compression at `nu = 0.3` (`s/q` down to `0.949`; a non-conforming element is not bound by `q`). The plain element has a
spurious zero in every case. The zero comes much earlier in confined compression and near incompressibility: `lambda_s =
0.9507` at C49 (5 per cent compression), and `m_Inc = -189` at 0.5. In confined compression at `nu >= 0.45`, and in deep
uniaxial compression at `nu = 0.49`, the stabilised element is driven to within 0.15 to 0.6 per cent of the Full Hex8 (`c` up to
`6507 mu`), and the Full Hex8 locks there. So the stabilisation removes the hourglass zero but gives up the element's
locking-free behaviour in nearly incompressible compression. The measured numbers match the registered probes to their printed
digits.

### Deviations

- None in the checks. MU11 also made the E test panic (an element inverted in the local mode iteration), so that test fails
  too; HF2 is the registered catch.

### Mutations

Each on the scratch copy `f693-scratch/mut` (a git archive of 490d2a86), run once against the default tests of `si_eas_hg2.rs`,
then reverted (`f693-scratch/mutate.py`, logs `mutations.log`, `mut-MU*.log`).

| Id | Expected | Caught by | Caught |
|---|---|---|---|
| MU7 `strain_energy` without `Pi_s` | E1, E3 | E1 (`5.33e-4`), E3 (`3.56e-3`) | yes |
| MU8 `Pi_s` for every Incompatible element | E0 | E0 (`9.45e-3`) | yes |
| MU9 `Pi_s` without the factor 1/2 | E1, E3 | E1 (`5.33e-4`), E3 (`3.56e-3`) | yes |
| MU10 plain modes in a stabilised model's energy | E1, E3 | E1 (`7.23e-3`), E3 (`3.92e-2`) | yes |
| MU11 `c M` dropped from the modes' tangent | HF2 | HF2 (`m_Stab = -23.0` at C49 0.8, and four more rows); the E test (panic) | yes |
| MU12 `c` from the Green strain | E1 | E1 (`2.63e-3`), E3 (`4.91e-4`) | yes |

MU7 of ADR 0614 is caught by two registered checks, by margins of eight and three orders of magnitude.

### What was not run

Nothing registered was skipped. `cargo test -p rcfd-fem --release` as a whole and `baseline_hashes` were not run (no library
file changed; registered so). The heavy records were not re-run under the mutations (registered so).

### Decision by the registered rule

**done.** Every check was met in its single registered run, the negative controls behaved as stated, all six planned mutations
were caught (MU7 of ADR 0614 by E1 and E3), B1 and B2 were met, and no amendment was written. Every clause of the acceptance is
met:

- c1: the strain energy at a large strain against the test's own stabilisation energy, with MU7 caught.
- c2, c3: the sweep for confined compression and for `nu = 0.45` and `0.49`.
- c4: the lowest eigenvalue of the stabilised element reported against the Full Hex8, through the ordering O1 and the ratio,
  with no accuracy limit where the Full Hex8 locks (as registered).
- c5, c6: the same eigenvalue against the closed-form acoustic bound derived in arithmetic 2, over `[0.5, 1]`.
- c7: defaults unchanged (no library change).

### Remaining questions

1. **Over-stabilisation near incompressibility.** Options: (a) a follow-up coefficient built from the deviatoric part of the rest
   stress (or from the shear modulus only), checked against the probes' need and a locking measure; (b) leave the coefficient
   and document that the stabilised element approaches the locking Full Hex8 in nearly incompressible compression.
   Recommendation: (a), as a follow-up (it changes the opt-in's results, not the defaults). It needs a locking-free reference of
   the compressed state, for example the repository's F-bar Hex8 on the same patch.
2. **Default.** Recommendation: keep it opt-in, as ADR 0614. The plain element's zero at 5 per cent confined compression at `nu
   = 0.49` is a strong argument for using the stabilisation, but turning it on by default changes results.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, intermediate tier. Follow-up of F-685 (ADR 0614). DONE. ADR 0628, 2026-11-16: the stabilised incompatible-modes Hex8 of ADR 0614 beyond its set-up, tests only (no library change; defaults unchanged by construction, the bit records of ADR 0608 and ADR 0614 pass). Strain energy at a large strain: Integrator::strain_energy of a stabilised 4 x 4 x 4 distorted neo-Hookean patch at 40 per cent confined compression with bending (stabilisation energy 5.3e-4 of the total) equals the test's own elastic plus stabilisation energy to 5.4e-16 (the plain model's to 1.8e-16) and is the potential of the stabilised internal force to 2.0e-9 (central difference); the missed mutation MU7 of ADR 0614 is now caught (by 5.3e-4 and 3.6e-3), and five more mutations are caught. The ADR 0614 sweep (6 x 6 x 6 patch, every boundary node fixed, 51 stretches on [0.5, 1]) repeated for confined compression at nu = 0.3, 0.45, 0.49 and uniaxial stress at nu = 0.45, 0.49, against a closed-form acoustic bound for the neo-Hookean energy at diag(t, t, lambda) derived in the ADR (q = min over the polar angle of mu J^-2/3 (1 + mu_-), equal to the numerical sphere minimum to 3.8e-5): the stabilised element's lowest eigenvalue stays >= 0.977 mu in every case and lies between the plain element and the Full Hex8 at every stretch (an ordering derived in the ADR, which holds whether or not the Full Hex8 locks); the Full Hex8 stays above q everywhere. The plain element has a spurious zero in every case: lambda_s = 0.6418 (confined, nu 0.3), 0.8094 (0.45), 0.9507 (0.49: 5 per cent compression), 0.5644 and 0.5103 (uniaxial, 0.45, 0.49). Finding: near incompressibility the coefficient (up to 6507 mu) drives the stabilised element to within 0.15 to 0.6 per cent of the Full Hex8, which locks there (at rest the Full Hex8 is 13 per cent stiffer than the plain element at nu = 0.49). Not covered: tension, shear and free boundaries, other materials, a locking-free reference of the compressed state, a coefficient without the bulk modulus.

> F-685 (note, ADR 0628, 2026-11-16): F-693 catches the missed mutation MU7 and measures the stabilisation in confined and nearly incompressible compression (lowest eigenvalue >= 0.977 mu in five new cases; near incompressibility the element approaches the locking Full Hex8).

> Follow-up proposed: A deviatoric coefficient for the stabilised incompatible-modes Hex8 near incompressibility. Deps F-693. Acceptance: Every limit not stated here is registered in the ADR before the run. An opt-in coefficient of the ADR 0614 stabilisation without the bulk modulus, from the published literature or derived in the ADR; the ADR 0628 sweep repeated for it in all six cases, the lowest eigenvalue kept above a registered positive limit over [0.5, 1]; a locking measure of the compressed state against a locking-free reference of the repository (an F-bar or mixed Hex8 on the same patch) reported for the ADR 0614 and the new coefficient; the ADR 0608 checks within their limits; the default bits unchanged.
