# ADR 0472: F-415 registration: the Coriolis term of a rotating part on the modal subspace (the gyroscopic quadratic problem by the general eigensolver of ADR 0450, the split Campbell table, the stability flag)

Status: **REGISTERED 2026-11-14, written before any code of F-415 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-415 (deps F-414, F-396), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
work package SA-8, stub [ADR 0318](0318-coriolis-term-rotating-frame-campbell.md); the module `rcfd_fem::gyro` declared by
[ADR 0440](0440-f408-registration-advanced-tier-scaffold.md) (F-408) with `PENDING = Some("ADR 0318 (F-415)")`. Number 0472
given by the lead. Branch `f415-work` from main 4abff5fe, worktree `w49i-work/f415`, scratch `w49i-work/f415-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is reached
only through new functions, with the existing option `RotatingOptions::coriolis` that is off by default); no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no existing test edited (the scaffold row follows the standing
pattern of ADR 0291); no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-415 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. G is skew within 1e-13 of its norm and x^T G x = 0 within 1e-12. Without the option the table has the bits of F-396; with it at zero speed the frequencies equal those of F-396 within 1e-10. A mass on isotropic springs in the plane of rotation (two unknowns): frequencies |w0 - Omega| and w0 + Omega within 1e-10; the same as a stiff block on elastic supports within 1e-3. Thin ring spinning about its axis (thickness/radius 0.05, nu = 0, full model): the two travelling waves of in-plane mode n differ in frequency by 4 n Omega/(n^2 + 1) within 2 percent for n = 2 and 3 at Omega = 0.1 w_2. Radial beam of square section: the flap frequency equals that of F-396 within 1e-6 relative and the lag frequency moves by the registered second-order estimate within 20 percent of that estimate. Crossings of split modes are tracked without exchange (modal assurance on the complex shapes above 0.9). A speed at which an eigenvalue has a positive real part is reported as unstable with the speed.

Clauses: **c1** G skew within 1e-13 of its norm; **c2** `x^T G x = 0` within 1e-12; **c3** without the option the table has the
bits of F-396; **c4** with it at zero speed the frequencies equal F-396's within 1e-10; **c5** the mass on isotropic springs (two
unknowns): `|w0 - Omega|` and `w0 + Omega` within 1e-10; **c6** the same for a stiff block on elastic supports within 1e-3;
**c7** the ring, `n = 2` and `3`: the split of the travelling waves `4 n Omega/(n^2 + 1)` within 2 percent at `Omega = 0.1 w_2`;
**c8** the radial beam: flap frequency equal to F-396's within 1e-6 relative; **c9** the lag frequency moves by the registered
second-order estimate within 20 percent of it; **c10** crossings of split modes tracked without exchange (complex modal assurance
above 0.9); **c11** a speed with an eigenvalue of positive real part reported as unstable with the speed.

## What was read (main 4abff5fe, read only)

ADR 0318 (stub) and the roadmap entry; ADR 0403 (F-396: `RotatingPart`, `modes`, `campbell`, `CampbellTable`, the inertia test of
D3, the tracker of D4); ADR 0411 (F-499: `coriolis_matrix`, the whirl modes by a symmetric linearisation, `WhirlCampbell` and its
tracker on complex lines, the cube on springs CB and the point mass PM, the sector D5); ADR 0416 (F-505: the state matrix, the
`Stability` flag and `STABILITY_TOLERANCE`); ADR 0408 (F-498: the radial beam F, Hex20 20 x 2 x 2, and its flap and lag
identification); ADR 0450 (F-414: `rcfd_linalg::eigen_general::quadratic_eigen_with`, `damped_modes` and its projection on
undamped modes); ADR 0462 (F-544) and ADR 0465 (F-547) for the cyclic refusals; the source files `rotating.rs`,
`rotating/whirl.rs`, `rotating/whirl/state.rs`, `rotating/whirl/state/springs.rs`, `damped.rs`, `gyro.rs` (the stub),
`eigen_general.rs` (the quadratic problem, its Cholesky congruence and balancing), `model.rs` (`ElasticSupport`), `cyclic.rs`
(`full_rotor`); the tests `si_rotating_whirl.rs`, `si_rotating_flap_lag.rs`, `sa_cms_default_bits.rs` (the digest pattern) and
`sa_scaffold.rs` (the X1 copies).

What already exists matters for the scope. F-499 (ADR 0411) built the Coriolis matrix `C` (`G = 2 Omega C`, skew bit for bit)
and the whirl modes of the whole reduced problem by a dense symmetric linearisation (up to 1500 unknowns) or a sparse Lanczos
process (ADR 0427), with forward and backward whirl and a Campbell table with directions; F-505 (ADR 0416) and F-519 (ADR 0428)
added the state matrix at any speed and the stability flag. What F-415 adds, as planned in ADR 0318 ("the modes of `lambda^2 M
+ lambda G + K_eff(Omega) = 0` on the modal subspace (ADR 0317)"), is the gyroscopic problem **projected on the F-396 modes**
and solved by the general quadratic eigensolver of F-414 (no new eigensolver is written): a split Campbell table of any model
size the F-396 subspace solver takes, at every speed (also above a loss of stiffness, on a rest basis), with the growth rate and
the stability flag per speed; and the checks of the acceptance, which no earlier item ran (the ring of Bryan, the radial beam's
second-order lag shift, the instability of anisotropic supports).

Published sources, cited from memory, **not read** for this work: G. H. Bryan, On the beats in the vibrations of a revolving
cylinder or bell, Proc. Camb. Phil. Soc. 7 (1890) 101 (the factor of the travelling waves of a spinning ring); L. Meirovitch,
Methods of Analytical Dynamics (McGraw-Hill 1970) and Computational Methods in Structural Dynamics (Sijthoff and Noordhoff 1980)
(gyroscopic systems, the spectrum symmetric about both axes, gyroscopic stabilisation); the rotating mass on unequal supports
(the instability between the two critical speeds), as in G. Genta, Dynamics of Rotating Systems (Springer 2005), recalled only
for the name; the Euler-Bernoulli cantilever root `beta_1 L = 1.875104068711961` and the fixed-free rod modes, textbook values
re-derived below (the root solves `cos x cosh x = -1`). Every formula used is derived below from first principles; the sources
are cited for the names, not for any number.

## Design

### D1. The equations (first principles; ADR 0403 D1, ADR 0411 D1)

In the frame rotating at the constant speed `Omega` about the unit axis `a`, the free vibration about the centrifugally
pre-stressed state is `M u_tt + G u_t + K_eff(Omega) u = 0`, `K_eff = K + Omega^2 (s K_sigma,1 - c M_perp)` (the operator of
F-396, `RotatingPart::operator`), `G = 2 Omega C`, `C[(a, i), (b, j)] = 1/2 (m_ab + m_ba) [a x]_ij` (`coriolis_matrix` of ADR
0411, from the relative-acceleration term `2 Omega a x u_t` of the kinetic energy). `C` is skew bit for bit (each element block is
`m_ab [a x]` with `[a x]` skew and `m_ab` symmetrised; the assembly adds the same element numbers in the same order to `(p, q)`
and `(q, p)`). With `u = x e^{lambda t}`: `(lambda^2 M + lambda G + K_eff) x = 0`, and for an oscillation `lambda = i omega`:
`(K_eff - omega^2 M + i omega G) x = 0`, the convention of ADR 0411 (D3 there: the whirl number `kappa = Im(x^H C x)/x^H M x =
2 re^T C im / (re^T M re + im^T M im)`, forward for `kappa > 0`, the frequency of a forward mode falling as `omega_0 - kappa
Omega`).

New public function `gyro::gyroscopic_matrix(model, axis, omega, threads) -> CsrMatrix`: `G = 2 Omega C` on the stiffness
pattern (each value of `C` multiplied by `2 Omega`; exactly skew, since `x (-c) = -(x c)` in floating point).

### D2. The modal subspace (Rayleigh-Ritz)

At the speed `Omega` the basis `Phi` (the model's equations, `m` columns) is the set of F-396 modes at that speed,
`RotatingPart::modes(Omega)` (the eigenvectors of `K_eff(Omega)`, `M`-normalised, the count of `options.modal.modes`) when
`K_eff(Omega)` is positive definite: **speed basis**. At a speed where F-396 reports a loss of stiffness it has no modes; the
basis is then the F-396 modes at rest, `modes(0)` (the modal analysis): **rest basis**, reported with the loss. Both are
Ritz bases of the admissible motions (zero at the fixed equations; on the transformation path `Phi = T q`), so the projection is
`M_r = Phi^T M Phi`, `K_r = Phi^T K_eff(Omega) Phi`, `C_r = Phi^T C Phi` (`m x m`, dense, by `m` sparse products and dot
products on the full equations), the first two symmetrised `(A + A^T)/2`, `C_r` skew-symmetrised `(A - A^T)/2`; the defects
before that are reported (`max|a_ij - a_ji|/max|a|` of `K_r`, `M_r`; `max|c_ij + c_ji|/max|c|` of `C_r`). `G_r = 2 Omega
C_r`. The shapes are `x = Phi q`.

The Coriolis option is the existing `RotatingOptions::coriolis` (default false; ADR 0411). No new option field is added.

### D3. The solution by the solver of ADR 0450

`rcfd_linalg::eigen_general::quadratic_eigen_with(M_r, G_r/tau, K_r/tau^2, None, m, default options)` in the scaled time
`lambda = tau mu`, `tau = sqrt(max_i |K_r,ii|/M_r,ii) + 2 Omega` (`tau = 1` if that is 0): the companion matrix of the solver
then has entries of order one (the largest basis frequency, or the gyroscopic rate when it dominates), so its rounding is
`eps` relative to `tau` on every root rather than to `tau^2`. The `2 m` roots `lambda = tau mu` and the vectors `q`
(`q^H M_r q = 1`, the scaling of the solver; `M_r` is not changed by `tau`). The residual of each root is the solver's (on the
scaled problem). The same routine is public for dense matrices, `gyro::split_modes_dense(m, c, omega, k, n)` (`G = 2 omega C`,
`C` skew-symmetrised; refuses wrong lengths, key `"matrix"`; a non-finite or negative speed, `"speed"`; a mass that is not
positive definite, `"mass"`; a non-finite entry, `"matrix"`).

### D4. Reporting, stability

A root is oscillating when `Im lambda > REAL_TOLERANCE |lambda|` (`REAL_TOLERANCE = 1e-6`, the value of ADR 0450 D3.4, taken from
`damped`). Each oscillating root with `Im lambda > 0` is a **split mode**: `omega = Im lambda` (rad/s, the rotating-frame
frequency), `eigenvalue = omega^2`, `frequency = omega/(2 pi)`, `growth = Re lambda`, the shape `re + i im = Phi q`, the whirl
number of D1 (from `C_r`, `M_r`: `kappa = 2 Re(q)^T C_r Im(q) / (q^H M_r q)`), the direction `Whirl::of(kappa)`
(ADR 0411's tolerance), the residual. The modes are sorted by `omega` ascending (ties by the root order). The roots with `|Im
lambda| <= REAL_TOLERANCE |lambda|` are listed as `real_roots` (their real parts). All `2 m` roots are kept.

The spectrum of a real gyroscopic problem (`M`, `K` symmetric, `G` skew) is symmetric about both axes (`lambda`, `conj lambda`,
`-lambda`, `-conj lambda`: the characteristic polynomial `det(lambda^2 M + lambda G + K)` is unchanged by `lambda -> -lambda`
because `G^T = -G`), so a root off the imaginary axis implies one with a positive real part. The speed is **stable** when `max Re
lambda <= STABILITY_TOLERANCE max |lambda|` (`STABILITY_TOLERANCE = 1e-8`, ADR 0416's constant, reused), otherwise **unstable**
with `growth = max Re lambda` over all `2 m` roots (oscillating and real: flutter and divergence alike): `Stability::Unstable {
growth }` of ADR 0416.

### D5. The split Campbell table

`RotatingPart::split_modes(Omega) -> SplitModes` (one speed) and `RotatingPart::split_campbell(speeds) -> SplitCampbell` (speeds
checked as F-396: finite, `>= 0`, strictly increasing, key `"speeds"`). With `options.coriolis` **off**, `split_campbell` returns
the F-396 table itself, `RotatingPart::campbell(speeds)` (no reduced solve: the bits of F-396 by construction, clause c3), with no
whirl numbers, directions, growth or stability; `split_modes` returns the F-396 modes at the speed as real split modes (`kappa =
0`, growth 0, no stability). With it **on**, at every speed (zero included: no zero-speed shortcut; `G = 0` there) D2 to D4, and
the modes are tracked by the tracker of ADR 0411 D4 (`track_whirl`: the subspace assurance of the complex lines `{(re, im), (-im,
re)}` of the eigenvalue clusters, `M`-weighted; the clusters by F-396's relative tolerance), which gives a `WhirlCampbell` (the
F-396 table of eigenvalues, frequencies, the complex MAC with the previous assigned mode, the shapes; per track and speed the
whirl number and the direction; CSV and JSON and the crossings with engine orders linear in `Omega`). `SplitCampbell` adds per
track and speed the growth rate, and per speed the stability, the basis (speed or rest) and the loss of stiffness of F-396 if
any; `SplitCampbell::unstable()` lists `(speed, growth)` of every unstable speed (clause c11); `to_csv` is the `WhirlCampbell`
CSV with two columns `stability` (`stable`/`unstable`) and `growth` appended to every row. The only changes to existing files are
visibility (`pub(crate)` on `track_whirl`, its `Entry`, and two read accessors of `RotatingPart`: its model and options) and the
re-exports; no existing computation is changed.

### D6. Cyclic symmetry and other refusals

A model with a cyclic constraint is refused by `split_modes` and `split_campbell` (key `"cyclic"`, the message naming
`RotatingSector::whirl_modes` of ADR 0411 D5, which solves the complex sector problem with the skew part per harmonic index on
the whole reduced sector). On the modal subspace a sector needs the harmonic modes of each index as a complex (cos/sin) basis and
the doubled Coriolis matrix `T_k^T blockdiag(C_s, C_s) T_k` of ADR 0411 D5; that is a follow-up, not built here (ADR 0318
foresaw that a bladed disc gets the term only as a full rotor). The interaction with ADR 0465 (F-547, which refuses a cyclic
model with non-coaxial pre-stress) and ADR 0462 (F-544) is therefore none: F-415 takes no cyclic model. A negative or non-finite
speed is refused (`"speed"`). Elastic supports and linear constraints are taken (the basis lives on the admissible motions).

### D7. Interfaces (all new, in `gyro.rs`; `gyro::PENDING = None`)

`gyroscopic_matrix`, `split_modes_dense`, `DenseSplit`, `SplitMode`, `SplitModes`, `Basis`, `SplitSpeed`, `SplitCampbell`,
`RotatingPart::{split_modes, split_campbell}`. Case-file key: none is added (the maintainer's call); the recommendation is in the
addendum.

## Set-ups (fixed now)

Steel `E = 2e11 Pa`, `rho = 7850 kg/m^3`; axis `e_z` through the origin; `nu = 0.3` for PM, CB, CBA (as ADR 0411), `nu = 0` for
the ring R and the beam F.

**PM (two unknowns):** `m = 7.85 kg`, `w0 = 1000 rad/s`, `M = m I_2`, `C = m [[0, -1], [1, 0]]`, `K_eff = m (w0^2 - Omega^2)
I_2` (the spin softening of a point mass in the plane), by `split_modes_dense`. Closed form (ADR 0411 set-up PM, `z = x + i y`):
`s^2 + 2 Omega s - (w0^2 - Omega^2) = 0`, `s = -Omega +/- w0`, the frequencies `|w0 - Omega|` and `w0 + Omega` at every `Omega
!= w0`. Speeds `Omega/w0 in {0.1, 0.3, 0.5, 0.7, 0.9, 1.2, 1.5}`. **PMA** the same with `K_eff = m diag(w0^2 - Omega^2, 2.25 w0^2 -
Omega^2)` at `Omega/w0 in {0.5, 1.2, 1.8}`: the characteristic equation `(mu + a - Omega^2)(mu + b - Omega^2) + 4 Omega^2 mu = 0`,
`mu = lambda^2`, `a = w0^2`, `b = 2.25 w0^2`, has a positive root `mu` exactly when `(a - Omega^2)(b - Omega^2) < 0`; then `lambda =
+/- sqrt(mu+)` real (divergence), growth `sqrt(mu+)`.

**CB (stiff block on elastic supports):** the cube of ADR 0411 (one Hex8 of side 0.1 m centred on the origin, `m = 7.85 kg`, nodal
springs `k_n = m w0^2/8` at all 8 nodes in `x` and `y`, `u_z = 0` at all nodes, both effects of ADR 0403 on), `modes = 3` (the
translation pair and the torsion on the springs), `coriolis(true)`. ADR 0411 set-up CB shows the translation pair spans an exact
invariant subspace with the closed form of PM. Speeds as PM; Campbell speeds `0.07 j w0`, `j = 0 .. 12`. **CBA** the same with
the `y` springs `2.25 k_n` (anisotropic supports), Campbell speeds `{0, 0.5, 1.2, 1.8} w0`.

**R (thin ring, full model):** a sector `hex_box([R - h/2, 0, -b/2], [h, 2 pi/N, b], [1, 1, 1])` mapped to `(r cos t, r sin t,
z)` and upgraded to Hex20 (`upgrade_to_hex20_mapped`), `R = 1 m`, `h = 0.05 m` (radial thickness, `h/R = 0.05`), `b = 0.05 m`
(axial width), `N = 64`, with its cyclic constraint `ymin`/`ymax`, expanded by `full_rotor` to the 360 degree mesh, on which a
new model is made (no cyclic constraint): `nu = 0`, `u_z = 0` at every node (the in-plane problem: with `nu = 0` and the
mid-plane symmetry the in-plane modes are exactly those of the unconstrained ring that are even in `z`, and the Coriolis term
couples nothing across the parity, since `a x u` has no `z` part), and an isotropic in-plane foundation: nodal springs `k_n` in
`x` and in `y` at every node, `k_n = m_R (0.2 w2c)^2 / N_nodes`, `m_R = rho 2 pi R h b`, `w2c` the closed-form inextensional
frequency below (the springs remove the three rigid in-plane motions, which would make `K_eff` indefinite at any speed, without
breaking the axisymmetry: `k (u_x^2 + u_y^2) = k (u_r^2 + u_theta^2)`). Both effects on, `modes = 9` (the three foundation modes
near `0.2 w2c`, the pairs `n = 2, 3, 4`; the cut falls between the pairs 4 and 5), modal tolerance 1e-10, residual test
Backward. `w_2` = the mean of the two rest frequencies of the `n = 2` pair of this model (F-396 at rest, rad/s), `Omega_R = 0.1
w_2`. The `n` pairs at rest: modes 3, 4 (`n = 2`) and 5, 6 (`n = 3`) of the rest result (set-up checks: each pair degenerate
within 1e-6 relative; `w_3/w_2` within 5 percent of `w3c/w2c = 2.828`).

Closed form (derived here; Bryan's result): inextensional in-plane motion of a thin ring, `u_r = w`, `u_theta = v`, `w = -dv/dtheta`
; the wave pair `v = cos(n t) q1 + sin(n t) q2`. Bending: `w_n^2 = (E I/(rho A R^4)) n^2 (n^2 - 1)^2/(n^2 + 1)`, `I = b h^3/12`,
`A = b h`: `w2c = 195.6 rad/s`, `w3c/w2c = sqrt(57.6/7.2) = 2.828`. Kinetic energy in the rotating frame: the mass of each
coordinate `rho A R pi (1 + n^2)`; the Coriolis term `Omega rho A R int (v_t w - w_t v) dt = 2 pi n Omega rho A R (q1 q2_t - q2
q1_t)`; Lagrange's equations: `q1_tt - g q2_t + w^2 q1 = 0`, `q2_tt + g q1_t + w^2 q2 = 0`, `g = 4 n Omega/(n^2 + 1)` (the same
`w^2` for both, with any axisymmetric stiffness, pre-stress or foundation); roots `omega = sqrt(w^2 + g^2/4) +/- g/2`: the split
of the two travelling waves is exactly `g = 4 n Omega/(n^2 + 1)` (`1.6 Omega` for `n = 2`, `1.2 Omega` for `n = 3`), whatever
`w`, so the foundation and the pre-stress do not enter it at this order. The ring's whirl numbers are `+/- 2 n/(n^2 + 1)`.

**F (radial beam of square section):** the beam F of ADR 0408 (`hex_box([0, -0.025, -0.025], [1, 0.05, 0.05], [20, 2, 2])` to
Hex20, clamped at `x = 0`, `nu = 0`), both effects on, `modes = 14`, modal tolerance 1e-12, residual test Backward (ADR 0408's
options with 14 modes). `w_0` = the lowest rest F-396 frequency, `Omega_F = 0.2 w_0` (ADR 0408's speed). The flap mode is the
lowest returned mode with a kinetic-energy share above 0.5 in `z`, the lag mode the lowest with a share above 0.5 in `y` (ADR
0408's rule; for a complex shape the share is `(re^T M_c re + im^T M_c im)/(re^T M re + im^T M im)`). Set-up check: the speed
basis at `Omega_F` contains a mode with a share above 0.5 in `x` (the first axial mode, near 7.9e3 rad/s; the 14 modes reach
about 1.2e4 rad/s by the Euler-Bernoulli estimates of arithmetic 4).

## Arithmetic before this registration (disclosed)

1. PM through numpy (`f415-scratch/pm_probe.py`, the construction of D3 with `numpy.linalg.eigvals` on the companion matrix of
   the Cholesky-congruent problem in the scaled time): at the seven speeds the frequencies match `|w0 - Omega|`, `w0 + Omega`
   within `1.8e-15 w0`; `max|Re lambda|/max|lambda| <= 1.2e-16`. PMA at `1.2 w0`: the roots `+/- 0.239998 w0` (real) and `+/-
   2.48749 i w0`; the closed form `sqrt(mu+) = 0.2399980785 w0`.
2. CB: ADR 0411 measured the translation pair of the whole-model whirl solve within `3.2e-13 w0` (P1b) on the same set-up; here
   the speed basis of 3 modes contains the invariant translation subspace exactly (rounding of the element stiffness on a rigid
   motion `~2e-11` relative, ADR 0411 arithmetic 3). Estimate `<= 1e-10` against the limit 1e-3. At `1.2, 1.5 w0` the translations
   have `K_eff < 0` (F-396 reports a loss) and the torsion `~(3 - Omega^2/w0^2) m w0^2 > 0`; the rest basis contains the same
   invariant subspace, so the same estimate holds. The pair is gyroscopically stable (the PM closed form: real frequencies).
3. Ring (closed forms above): `w2c = 195.6 rad/s` (`E h^2/(12 rho) = 5308 m^2/s^2`); foundation frequency `0.2 w2c`, the
   translation pair at `Omega_R` has `K_eff ~ (0.04 - 0.0104) w2c^2 m > 0`, so the speed basis applies. Expected deviation of the
   FE split from `g`: through-thickness and rotary terms of order `(h/R)^2 n^2/12 ~ 1e-3`, the extensional coupling of order
   `(h/R)^2`, the foundation's departure from mass proportionality on smooth modes small; estimate `<= 5e-3` against 2e-2.
4. Beam (`f415-scratch/lag_estimate.py`, `flap_estimate.py`, numpy quadrature with 200001 points): second-order perturbation of
   the quadratic problem in `M`-orthonormal modal coordinates (`K = diag(w_k^2)`, `G` skew): for mode `i` with `q_j = -i omega
   G_ji q_i/(w_j^2 - omega^2)`, row `i` gives `w_i^2 - omega^2 - omega^2 sum_j G_ij^2/(w_j^2 - omega^2) = 0`, so `omega ~ w_i (1 -
   1/2 sum_j G_ij^2/(w_j^2 - w_i^2))`. The lag mode (in-plane, `u_y`) couples through `[a x]` only to motion along `x` of the same
   parity (even in `z` and in `y`): the axial modes; the flap mode (`u_z`, with `u_x = -z w'`) only to modes odd in `z` with a
   `u_y` part: the torsion modes. Euler-Bernoulli cantilever `phi(x)` (`beta_1 L = 1.875104068711961`, `int rho A phi^2 = 1`),
   fixed-free rod modes `psi_j = sqrt(2/(rho A L)) sin((2j - 1) pi x/(2L))`, `w_j = (2j - 1) (pi/2) sqrt(E/rho)/L`: `G_lag,j = -2
   Omega int rho A phi psi_j`; `S = sum_j (int rho A phi psi_j)^2/(w_j^2 - w_l^2) = 1.4772e-8 s^2` (`w_l = 256.159 rad/s`, 200
   terms; the first carries 99.07 percent), and the **registered second-order estimate** of the lag shift is
   `Delta omega_lag,est = -2 Omega^2 S omega_lag,F396(Omega)` (at `Omega = 0.2 w_l`: `-7.754e-5` relative). The flap shift by the
   torsion modes (`theta_j = sqrt(2/(rho I_p L)) sin(...)`, `w_t,j = (2j-1)(pi/2) sqrt(G J/(rho I_p))/L`, `J = 0.1406 b^4`, overlap
   `int rho I_y w' theta_j`): `-9.5e-8` relative at `0.2 w_l`, below the limit 1e-6 by a factor 10. Expected deviation of the FE
   lag shift from the estimate: the first axial mode only in the basis (-0.9 percent), three-dimensional and shear effects on the
   shapes (about 1 percent), the change of the shapes with `Omega^2` (about 1 percent): within 5 percent, against 20.
5. `x^T G x`: `y = G x` by rows then `x . y`; with `G` exactly skew each product pair `x_i G_ij x_j`, `x_j G_ji x_i` cancels in
   exact arithmetic, so the result is rounding only, bounded by `(nnz_row + n) eps |x|^T |G| |x|` (`~ (100 + 2.3e3) 1.1e-16 =
   2.6e-13` for the ring, the largest model) and typically `eps sqrt(n)` of it (`~ 5e-15`).
6. Zero speed (c4): `G = 0`, the basis the F-396 modes at rest, `K_r` diagonal to rounding (the Ritz values of the subspace
   solver; their quotients agree with the reported eigenvalues to `eps` times the conditioning of the projected problem, and to
   the square of the residual tolerance), the quadratic solve in the scaled time `eps` relative to the largest basis frequency;
   the largest basis frequency over the smallest is about 35 (beam), 30 (ring, the foundation modes), 1.7 (CB): estimate
   `<= 1e-13` against 1e-10.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_gyro_default_bits.rs` (B2) and `crates/rcfd-fem/tests/sa_gyro.rs` (the rest),
release profile. Frequency errors are relative unless stated.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| G1 | c1 | `gyroscopic_matrix` of CB at `0.5 w0`, R at `Omega_R`, F at `Omega_F`: `max|G_ij + G_ji| / max|G_ij|` | `<= 1e-13` | 0 (D1) |
| G2 | c2 | the same three `G`, `x` three pseudo-random vectors (a fixed linear congruential sequence in `[-1, 1)`) and the real and imaginary parts of the returned split modes at that speed: `|x^T G x| / (|x|^T |G| |x|)` | `<= 1e-12` | `<= 5e-15` typical, `2.6e-13` bound (arithmetic 5) |
| G3 | D2 | the raw skew defect of `C_r` (before the skew-symmetrisation), CB, R, F at those speeds | `<= 1e-11` | `~1e-14` |
| B1 | c3 | `coriolis(false)`: `split_campbell` of CB over the 13 Campbell speeds plus `1.2 w0` (a loss) and of F over `{0, Omega_F}` against `campbell` of the same part: every eigenvalue, frequency, MAC and mode value equal by `to_bits`, the losses equal, `to_csv` identical; `split_modes(Omega)` off: frequencies by `to_bits` equal to `modes(Omega)` (CB at `0.5 w0`) | identical | by construction (D5) |
| B2 | rule 3 | digests printed at main 4abff5fe by `sa_gyro_default_bits.rs` (written with the API of that commit, committed before any library line): CB `campbell` (eigenvalues, frequencies, MAC) over the 13 speeds, CB `whirl_campbell` with `coriolis(true)` (eigenvalues, whirl numbers), CBA `whirl_stability` over `{0, 0.5, 1.2, 1.8} w0` (growth rates), F `modes(Omega_F)` (eigenvalues, the first mode), all equal after the change; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds (only visibility changes in shared files) |
| Z1 | c4 | `coriolis(true)`, `split_modes(0)` of CB, R, F: count equal to F-396's, and `max |omega_split/omega_F396 - 1|` over the modes in ascending order | count; `<= 1e-10` | `<= 1e-13` (arithmetic 6) |
| P1 | c5 | PM at the seven speeds: the two frequencies, sorted, against `{|w0 - Omega|, w0 + Omega}` sorted: `max |diff| / w0` | `<= 1e-10` | `1.8e-15` (arithmetic 1) |
| P2 | D4 | PM: stable at every speed; at `Omega < w0` the lower mode `kappa >= 1 - 1e-8` and `Forward`, the upper `kappa <= -1 + 1e-8` and `Backward` | each | exact (ADR 0411 P2) |
| P3 | c6 | CB `split_modes` at the seven speeds: the two modes of largest translation share (each share `>= 1 - 1e-8`; the share is ADR 0411's, the projection on `span{1_x, 1_y}` in the `M` norm) against PM's two frequencies: `max |omega_CB/omega_PM - 1|` | `<= 1e-3` | `<= 1e-10` (arithmetic 2) |
| P4 | D2, c6 | CB: the basis is `Speed` at `Omega <= 0.9 w0` and `Rest` with a loss at `1.2, 1.5 w0`; stable at all seven | each | D2, arithmetic 2 |
| R1 | c7 | R at `Omega_R`: for `n = 2` and `n = 3`, the returned split modes with complex subspace assurance `>= 0.9` against the span of the rest pair of `n` (exactly two, else a defective set-up): `|(omega_hi - omega_lo) / (4 n Omega_R/(n^2 + 1)) - 1|` | `<= 2e-2` each | `<= 5e-3` (arithmetic 3) |
| R2 | D1 | R: the lower mode of each pair `Forward`, the upper `Backward` | each | D1 |
| L1 | c8 | F at `Omega_F`: `|omega_flap,split / omega_flap,F396 - 1|` (F-396 = `modes(Omega_F)` of the same part) | `<= 1e-6` | `9.5e-8` (arithmetic 4) |
| L2 | c9 | F at `Omega_F`: `Delta = omega_lag,split - omega_lag,F396`; `|Delta / Delta_est - 1|`, `Delta_est = -2 Omega_F^2 S omega_lag,F396`, `S` evaluated in the test by composite Simpson quadrature (20000 intervals, 200 rod terms) of arithmetic 4 | `<= 0.2` | `<= 0.05` (arithmetic 4) |
| L3 | set-up | `S` of the test against the registered `1.4772e-8 s^2`; the basis at `Omega_F` contains a mode with an `x` share above 0.5 | `<= 1e-3`; holds | holds |
| T1 | c10 | CB `split_campbell` over the 13 Campbell speeds, `coriolis(true)`: every stored MAC (`j >= 1`, every track) `>= 0.9`; the two tracks whose mode at `j = 1` has a translation share `>= 1 - 1e-8` keep a translation share `>= 1 - 1e-8` at every speed and follow `w0 -/+ Omega` within `1e-8 w0` at every speed `> 0` (forward the lower); the backward translation track crosses the torsion track (the sign of their frequency difference changes between `j = 0` and `j = 12`; else a defective set-up) | each | MAC ~1 (exact invariant subspace) |
| U1 | c11 | CBA `split_campbell` over `{0, 0.5, 1.2, 1.8} w0`: `unstable()` is exactly one entry, its speed equal by bits to `1.2 w0`, its growth against the PMA closed form `sqrt(mu+)`: relative | one entry; `<= 1e-8` | `~1e-10` (arithmetic 1, 2) |
| U2 | c11 | PMA by `split_modes_dense`: unstable at `1.2 w0` with growth within `1e-10` relative of `sqrt(mu+)`, stable at `0.5 w0` and `1.8 w0` | each | `1e-15` (arithmetic 1) |
| C1 | D5 | CBA `to_csv`: `1 + 4` rows, the row of `1.2 w0` has `unstable` and the growth that parses back to the bits of `unstable()`; the others `stable` | each | D5 |
| X1 | D6 | refusals: `split_modes` of the sector of R (with its cyclic constraint) (`"cyclic"`); `split_modes(-1.0)`, `split_modes(NaN)` (`"speed"`); `split_campbell` with a decreasing list (`"speeds"`); `split_modes_dense` with a wrong length (`"matrix"`) and with `M = diag(1, -1)` (`"mass"`) | each refused with its key | D6 |
| X2 | D3 | F `split_modes(Omega_F)` with the modal threads 1 and 4: every frequency and shape value equal by `to_bits` | identical | the assembly is thread-independent (ADR 0284); the dense algebra sequential |
| E1 | scaffold | `sa_scaffold.rs`: the copy `x1_modules_and_pending_after_f430` becomes an ignored record (ADR 0291 pattern, the attribute only) and a new copy `x1_modules_and_pending_after_f415` with `gyro` landed passes; `gyro::PENDING == None` | pass | holds |

Records (reported, not in the decision rule): the ring's whirl numbers against `2 n/(n^2 + 1)`; the ring split with the whole-model
whirl solve of ADR 0411 is not run (2304 equations, above `MAX_WHIRL_UNKNOWNS`); the asymmetries of `K_r`, `M_r`; the residuals of
every returned root; the flap and lag shifts in absolute terms; G2 also relative to `||G||_F ||x||^2`.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | R with `coriolis(false)` at `Omega_R`: the `n = 2` pair's split over `1.6 Omega_R` | `< 1e-3` (degenerate) |
| N2 | R with the option: the measured `n = 2` split against the rigid-frame value `2 Omega_R`: `|split/(2 Omega_R) - 1|` | `> 0.1` (expected 0.2) |
| N3 | F with `modes = 6` (no axial mode in the basis): `|Delta / Delta_est|` | `< 0.1` (expected ~0: the lag couples only to the axial modes) |
| N4 | CB at `1.2, 1.5 w0`: F-396 `modes` reports `LossOfStiffness`, `split_modes` reports `Stable` (gyroscopic stabilisation, not a loss read as instability) | each |

Mutations (on a scratch copy of the worktree; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `G = Omega C` (the factor 2 dropped) | P1, P3, R1, T1 |
| MU2 | the sign of `G` reversed | P2, R2, T1 |
| MU3 | no rest basis: a loss speed returns no modes | P3, P4, U1 |
| MU4 | the stability flag set unstable at every loss of stiffness | P4, N4, U1 (1.8 w0) |
| MU5 | the option-off path solves the reduced problem with `G = 0` instead of returning F-396's table | B1 |
| MU6 | the tracker fed the real parts only (`im` set to 0 in the lines) | T1 |
| MU7 | the growth taken over the oscillating roots only | U1, U2 (divergence: real roots) |
| MU8 | the time scale not undone (`lambda = mu`) | Z1, P1 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an amendment
made after its run. **todo**: the module does not build or the ring and beam runs cannot be made.

## Limits (not covered)

- The modal subspace is only as good as its basis: a coupling to modes outside it is lost (N3 shows it for the lag of the beam);
  no static correction vectors (`K_eff^-1 G phi`) are added. The rest basis at a loss speed is a poorer Ritz basis than the speed
  basis; no check bounds its error except on the exact invariant subspaces of CB and CBA.
- Cyclic models are refused (D6); the sector on the modal subspace is a follow-up. The whole-sector problem exists (ADR 0411 D5).
- A part with nodal springs added by `RotatingPart::with_nodal_springs` (ADR 0424) takes at a loss speed the rest modes of its
  model without those springs (F-396's `modes(0)` is the model's modal analysis): a valid but poorer basis; not checked.
- No damping (the quadratic problem has `G` only; viscous or structural damping in the rotating frame would add `C + G` to the
  same solver), no stationary-frame rotor (F-416), no case-file key, no CLI or post-processing output beyond CSV and JSON.
- The ring is checked in plane (`u_z = 0`, `nu = 0`); out-of-plane modes and `nu != 0` are not covered.
