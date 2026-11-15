# ADR 0538: F-579 registration: mutual radiation impedance matrix of a baffled plane and the light-fluid added mass, damping and frequency shift of structural modes

Status: **REGISTERED 2026-11-14, written before any code of F-579 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-579 was written, compiled or run in the repository: the files listed under "What was
read" were read, and one numpy script was run in the scratch folder (disclosed below, its numbers quoted as estimates). The bit
record DB1 is a test file written and run on the base after this commit and before any library line. Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-579 (deps F-551), the follow-up proposed by [ADR 0488](0488-f551-registration-radiation-impedance-analytic-self-face-integrals.md)
(F-551, its remaining question 2). Structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)).
Number 0538 given by the lead. Branch `f579-work` from dfd2720d, worktree `w49i-work/f579`, scratch `w49i-work/f579-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; existing tests not edited; no new
dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). The lesson of ADR 0488
amendment 1 is applied from the start: **every aggregated test error propagates NaN and every compared value is required
finite** (a NaN is a miss).

## What F-579 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The radiation impedance matrix Z_mn = (1/2) int P[V_m] conj(V_n) dS of several velocity fields of a baffled plane by the surface pressure of F-551, symmetric within a registered tolerance, its diagonal equal to the impedance of F-551 within 1e-10; for two small baffled pistons the mutual term equal to (i omega rho/(2 pi)) S1 S2 e^{-i k d}/d within 1 percent at k d = 1 and 3; the modal added mass and radiation damping of the first modes of a baffled clamped plate reported, and the shift of its first frequency in a light fluid compared with the first-order estimate from the added mass within a registered limit.

Clauses: **b1** the matrix `Z_mn` of several velocity fields by the surface pressure of F-551; **b2** symmetric within a
registered tolerance; **b3** its diagonal equal to the impedance of F-551 within 1e-10; **b4** two small baffled pistons: the
mutual term equal to `(i omega rho/(2 pi)) S1 S2 e^{-i k d}/d` within 1 percent at `k d = 1` and 3; **b5** the modal added
mass and radiation damping of the first modes of a baffled clamped plate reported; **b6** the shift of its first frequency in a
light fluid compared with the first-order estimate from the added mass within a registered limit. The brief of the lead adds:
check the time convention of F-551 against the mutual formula and register any conjugation, with the textbook source in
`docs/reference-data.md` (uncertain citations flagged); register how the coupled frequency is obtained and the first-order
estimate; opt-in, defaults bit-identical (a default-bits record first), `baseline_hashes` unchanged, no case-file key (open
question); register the mutations; every aggregated error finite.

## What was read (dfd2720d, read only)

ADR 0488 whole (derivations 1 to 6, design, checks, amendment 1, addendum 1 and its follow-up text, which is this item);
`crates/rcfd-post/src/solid_acoustic.rs` whole (`VibratingSurface`, `BaffledPlane::new`, `surface_sum`, `surface_pressures`,
`radiation_impedance`, `RadiationImpedance`); `crates/rcfd-post/tests/sa_radiation_impedance_default_bits.rs` (the digest
pattern), `sa_radiation_impedance_amend1.rs` (the NaN-propagating `Checks`), `sa_solid_acoustic.rs` (`w3_two_pistons`, the
monopole-pair power of F-418); `crates/rcfd-fem/src/modal.rs` (`ModalOptions`, `ModalResult`, M-orthonormal modes,
`modal_analysis_with`), `crates/rcfd-fem/tests/sa_ortho_modal.rs` (`shell_plate`: a `Shell4` plate built from nodes and
connectivity, edge clamped by `Prescribed` on all six components); `rcfd_linalg::eigen` (dense eigensolvers; **not used**:
`rcfd-post` does not depend on `rcfd-linalg` and the graph is not changed, see D4); the F-551 section of
`docs/reference-data.md`, the F-551 paragraph of `docs/acoustics.md`; `f551-scratch/estimate.py` (its edge-sum code copied
into this item's script).

Published sources, cited **from memory, not read** for this work (every formula used is derived below): L. E. Kinsler, A. R.
Frey, A. B. Coppens and J. V. Sanders, *Fundamentals of Acoustics* (the baffled simple source and the `e^{j omega t}`
convention); A. D. Pierce, *Acoustics* (the Rayleigh integral, `e^{-i omega t}` convention); P. M. Morse and K. U. Ingard,
*Theoretical Acoustics* (`e^{-i omega t}`); R. L. Pritchard, "Mutual acoustic impedance between radiators in an infinite rigid
plane", J. Acoust. Soc. Am. 32 (1960) 730 (the point-source leading term of the mutual impedance; journal page from memory,
uncertain); M. C. Junger and D. Feit, *Sound, Structures, and Their Interaction* (the modal radiation impedances and the
light-fluid frequency shift); F. Fahy and P. Gardonio, *Sound and Structural Vibration* (the same). The time conventions
attributed to each book are from memory and recorded as an open question (OQ2); nothing here depends on them, because the
mutual formula is checked against F-551's own convention by derivation D2.

### Arithmetic done before this registration (disclosed)

One numpy script, no rcfd code: `w49i-work/f579-scratch/estimate.py` (logs `estimate.log`, whole run, and
`estimate_pistons.log`, part (1) rerun after a NaN fix). Part (1): the two square pistons below by the planned F-551 scheme
and by a 16^4 Gauss reference of the exact mutual integral. Part (2): the clamped plate below with **Rayleigh-Ritz modes of
products of clamped-clamped beam functions** (4 x 4 functions, not the FE modes of the test), sampled at the nodes of the
registered 24 x 16 grid and bilinear between them: the F-551 scheme re-implemented in numpy against the spectral reference
D5 of the same bilinear field, the added-mass ratio, the first-order and the iterated frequency, and water. The first run of
part (1) printed NaN for the scheme (`0 * inf` at a coincident point of a field that is zero there, a defect of the script,
not a number); it was fixed (`np.where`) and part (1) rerun once (`estimate_pistons.log`). Every other number below is from
the single run. The FE plate of the test will differ from the Ritz plate (the FE modes and frequencies are not known before
the run); estimates of part (2) are therefore of order, and the limits of the plate checks carry a factor of at least 3 over
them, or are physics-derived.

## Derivation

Convention of F-551 (ADR 0468, 0488): complex peak amplitudes, `x(t) = Re(X e^{+i omega t})`, `k = omega/c`, outgoing waves
`e^{i(omega t - k r)}`; on the plane `P(x) = (i omega rho/(2 pi)) int V_n(y) e^{-i k R}/R dS(y)`, computed by F-551's
`surface_sum` (the singular part `1/R` analytic per face).

**D1. The matrix and its symmetry.** For velocity fields `V_1 .. V_N` on one plane (same nodes, faces and options), with
`P_m = P[V_m]` the F-551 surface pressure at the quadrature points `(y_i, w_i)`,

    Z_mn = (1/2) sum_i w_i P_m,i conj(V_n,i)        (the acceptance's Z_mn; units W for velocities in m/s).

Written out with F-551's scheme, `P_m,i = (i omega rho/(2 pi)) [V_m,i (I(y_i) - sum_{j != i} w_j/R_ij) + sum_{j != i} w_j
V_m,j cos(k R_ij)/R_ij - i sum_j w_j V_m,j s(R_ij)]`, so `sum_i w_i P_m,i U_i = U^T A V_m` with `A_ij = w_i w_j K(R_ij)` for
`i != j` and a diagonal: `A` is complex **symmetric** (`K(R) = (i omega rho/(2 pi)) e^{-i k R}/R` depends on `R_ij = R_ji`).
Hence `Z_mn = (1/2) conj(V_n)^T A V_m`, and

- for real fields (modal shapes, real-amplitude pistons) `Z_mn = Z_nm` (reciprocity); the discrete matrix is symmetric up to
  rounding, so the registered tolerance is a rounding tolerance;
- for complex fields the identity is `Z_nm[V] = Z_mn[conj V]` (both are `(1/2) V_n^T A conj V_m`);
- `Z` is linear in the first field and conjugate-linear in the second: `Z(a U, W) = a Z(U, W)`, `Z(U, a W) = conj(a) Z(U, W)`.

The diagonal `Z_mm` is F-551's complex power `Pi` of the field `V_m` (same points, same sums), so `2 Z_mm/<|V_m|^2>` is F-551's
impedance `Z = 2 Pi/<|V_n|^2>`. Both forms are checked (D-checks), the first being the literal reading of b3.

**D2. Two small pistons and the time convention.** Pistons 1 and 2 of areas `S1`, `S2`, uniform normal velocities `U1`, `U2`
(real), centres `d` apart, `d` large against their size and `k` times their size small. The pressure of piston 1 on piston 2:
`P ~ (i omega rho/(2 pi)) U1 S1 e^{-i k d}/d`; the force `F_2 = int_2 P dS = (i omega rho/(2 pi)) U1 S1 S2 e^{-i k d}/d`.
With D1 and the field `V_1 = U1` on piston 1 (zero on 2), `V_2 = U2` on piston 2: `Z_12 = (1/2) F_2 conj(U2)`, so the
**mutual impedance in the force-per-velocity form of the acceptance's formula is `z_12 = 2 Z_12/(U1 conj U2)`**:

    z_12 = (i omega rho/(2 pi)) S1 S2 e^{-i k d}/d         (in F-551's e^{+i omega t} convention).

This is the formula of the acceptance **as written, with no conjugation**: the acceptance and F-551 use the same convention.
In the `e^{-i omega t}` convention (attributed from memory to Pierce and to Morse and Ingard) the same physics reads
`(-i omega rho/(2 pi)) S1 S2 e^{+i k d}/d`, the complex conjugate; Kinsler et al. (from memory) use `e^{+j omega t}` and the
form of the acceptance. Registered: the check compares `z_12` and `z_21` with the formula unconjugated; the factor 2 between
`Z_12` and the force form is registered here (D1's 1/2), not tuned.

Physics of the leading term (not a discretisation error): the exact mutual term of two finite pistons differs from the
point-source form by the directivity of each piston along the line of centres and the curvature of the kernel across them,
of order `(k L)^2` and `(L/d)^2` (square pistons of side `L`). Estimate (numpy, 16^4 Gauss of the exact integral):
`L = 0.01 m`, `d = 30 L = 0.3 m`: `9.3e-5` at `k d = 1`, `7.9e-4` at `k d = 3` (complex relative), the real and imaginary
parts separately at most `1.4e-4` and `1.2e-3`. The F-551 scheme on 4 x 4 faces per square agrees with that exact integral to
`2.5e-15`. At `d = 2 L` (pistons not small against their distance) the leading term misses by `2.1e-2` and `0.17`: the
negative control NC1.

**D3. Modal radiation impedance, added mass and damping.** A plate in the plane `z = 0` of a rigid baffle radiating into
`z > 0`, transverse displacement `w = sum_j phi_j q_j` with real mode shapes `phi_j` (the `z` component of FE modes,
M-orthonormal: modal masses `mu_j = 1`), `q_j ~ e^{i omega t}`. The velocity field of a unit modal velocity is `V_j = phi_j`.
The fluid force on mode `n` is `f_n = -int P phi_n dS = -sum_m Zr_nm dq_m/dt` with the **modal radiation impedance**

    Zr_nm = int P[phi_m] phi_n dS = 2 Z_mn          (real fields, D1),

so the modal equations are `mu_n (omega_n^2 - omega^2) q_n + i omega sum_m Zr_nm q_m = 0`. Splitting `Zr = C + i omega M_a`:
the **added-mass matrix** `M_a = Im Zr/omega` (kg for unit modal coordinates) and the **radiation-damping matrix** `C = Re Zr`
(kg/s), both symmetric (D1); the modal damping ratio `zeta_n = C_nn/(2 omega_n (mu_n + M_a,nn))`. Reported (b5): `M_a,nn`,
`C_nn`, `zeta_n` of the first six modes, each at its own `omega_n`, and the matrices at `omega_1`.

**D4. The light-fluid first frequency (b6).** Registered meaning of "the coupled frequency": the lowest real root `omega_c` of
the **fluid-loaded modal eigenproblem truncated to the first `N` modes, damping left out**,

    K q = omega^2 (Mu + M_a(omega)) q,    K = diag(mu_j omega_j^2), Mu = diag(mu_j),

with `M_a(omega)` evaluated at the root (a nonlinear eigenproblem). Obtained by the fixed point `omega^(0) = omega_1`,
`omega^(k+1) = sqrt(lambda_min(K, Mu + M_a(omega^(k))))`, stopped when `|omega^(k+1) - omega^(k)| <= 1e-12 omega^(k)` (the
result is `omega^(k+1)`), refused after 50 iterations. `lambda_min` by inverse iteration on the dense `N x N` pencil (`K`
diagonal, so each step is `x = K^{-1} (Mu + M_a) v`; Rayleigh quotient `v^T K v/v^T (Mu + M_a) v`; start `e_1`; stopped when
the quotient changes by at most `1e-15` relatively, refused after 2000 steps): no dense eigensolver of `rcfd-linalg` is
needed, so `rcfd-post`'s dependencies are unchanged. The added-mass matrix is symmetrised (`(M + M^T)/2`) before the solve.
Damping left out: it moves the damped frequency of a mode by `omega (1 - sqrt(1 - zeta^2)) ~ zeta^2/2`, about `5e-7`
relative at the estimated `zeta_1 ~ 1e-3`, `1e-4` of the shift; reported, not solved (a complex nonlinear eigenproblem, OQ3).
Registered `N = 6` (all six computed modes; with the parity selection of D6 the first mode couples to the third symmetric
mode and beyond).

The **first-order estimate** from the added mass: the Rayleigh quotient of the vacuum mode with the added mass at the vacuum
frequency, `omega_1^2 mu_1/(mu_1 + M_a,11(omega_1))`, to first order in `eps = M_a,11(omega_1)/mu_1`:

    omega_first = omega_1 (1 - eps/2),     Delta_first = omega_first - omega_1 = -omega_1 eps/2.

Expected difference (registered derivation of the limit): for one mode with a frequency-independent added mass the exact root
is `omega_1/sqrt(1 + eps)`, so `Delta_c/Delta_first = (1 - (1 + eps)^{-1/2})/(eps/2) = 1 - 3 eps/4 + 5 eps^2/8 - ...`; the
cross-mode coupling adds a second-order term `~ eps (M_a,1j^2/M_a,11)/(omega_j^2/omega_1^2 - 1)` per coupled mode `j`, and the
frequency dependence of `M_a` between `omega_1` and `omega_c` a term `~ (d ln M_a/d ln omega) eps/2`. Estimate (numpy, Ritz
plate, D7 set-up): `eps = 1.27e-2`, first-order shift `-6.37e-3`, coupled (`N = 6`) `-6.32e-3`, `Delta_c/Delta_first - 1 =
-9.1e-3` (`N = 1`: `-9.2e-3`; `-3 eps/4 = -9.6e-3`). Registered limit: `|Delta_c/Delta_first - 1| <= 3e-2` (about `2.4 eps`,
three times the estimate). For water (`rho = 1000`, `c = 1500`) the estimate is `eps = 10.8` and `Delta_c/Delta_first - 1 =
-0.87`: the light-fluid estimate fails, the negative control NC2.

**D5. Spectral reference of the modal impedance (independent of the Rayleigh scheme).** For a velocity `V(x, y)` on the plane
(zero on the baffle) with transform `Vhat(kappa) = int V e^{-i kappa . x} dS`, the field `p = (1/(4 pi^2)) int Phat
e^{i kappa . x - i k_z z} d^2 kappa`, `k_z = sqrt(k^2 - kappa^2)` for `kappa < k` and `-i sqrt(kappa^2 - k^2)` above (outgoing
and decaying for `e^{+i omega t}`); the Euler equation `i omega rho V = -dp/dz` at `z = 0` gives `Phat = omega rho Vhat/k_z`.
By Parseval,

    Zr_mn = int P[V_m] conj(V_n) dS = (omega rho/(4 pi^2)) [ int_{kappa<k} Vhat_m conj(Vhat_n)/sqrt(k^2 - kappa^2) d^2 kappa
                                                          + i int_{kappa>k} Vhat_m conj(Vhat_n)/sqrt(kappa^2 - k^2) d^2 kappa ].

(Static check: the added mass `(rho/(4 pi^2)) int |Vhat|^2/kappa d^2 kappa = (rho/(2 pi)) int int V V/R dS dS`, the
transform of `1/R` in the plane being `2 pi/kappa`; for the piston it gives `8 rho a^3/3` by `int_0^inf J1(x)^2/x^2 dx =
4/(3 pi)`, from memory, not used.) For the FE field, bilinear on the rectangles of a tensor grid with **zero nodal values on
the clamped boundary**, `Vhat(kx, ky) = sum_ij phi_ij hx_i(kx) hy_j(ky)` over interior nodes with the hat transform
`h_i(kappa) = h sinc^2(kappa h/2) e^{-i kappa x_i}` (`h` the cell size, `sinc x = sin x/x`, 1 at 0): the same velocity field
as the one the Rayleigh scheme integrates, so the comparison isolates the Rayleigh quadrature. Quadrature (registered): polar
`kappa, theta`; `theta` by the 256-point midpoint rule on `[0, 2 pi)`; `kappa < k` by `kappa = k sin t`, 64-point
Gauss-Legendre in `t` on `[0, pi/2]` (removes the inverse square root); `k < kappa < 2k` by `kappa = k cosh u`, 64-point
Gauss in `u` on `[0, acosh 2]`; `2 k < kappa < kappa_max = 16 pi/h` in 50 panels (the first `[2k, 4k]`, then 49 geometric
panels from `4k` to `kappa_max`), 8-point Gauss each. Estimate (numpy, Ritz plate): the scheme against this reference at
`omega_1`, modes 1 to 3: resistance `4.8e-13, 1.2e-13, 2.0e-14`, reactance `4.9e-6, 1.7e-5, 3.3e-5`; the reference refined
(96 points, 100 panels, `kappa_max` doubled) changes by `1.1e-12` and `2.2e-10`.

**D6. Parity selection.** The rectangle and the grid are symmetric about both centre lines; each mode is even or odd about
each. For two modes of different parity class in either direction `Zr_mn = 0` exactly in the continuum and to the accuracy of
the mode shapes (FE residual tolerance 1e-10) and rounding in the discrete sums. The class of each FE mode is read from the
sign of `sum phi(x) phi(mirror x)` over the nodes, separately for each axis.

**D7. Set-ups.**

- Pistons (b4): two squares of side `L = 0.01 m`, centres `(-d/2, 0, 0)` and `(d/2, 0, 0)`, `d = 30 L`, each meshed 4 x 4
  Quad4, one `VibratingSurface` (disjoint node sets), Gauss order 3 (default `RayleighOptions`); air `rho = 1.2`, `c = 340`;
  `omega = c (k d)/d` for `k d = 1, 3`; field 1 `U = 1e-3 m/s` along `z` on square 1 and zero on square 2, field 2 the
  converse. Exact areas `S1 = S2 = L^2`.
- Complex fields (b2, b3, sesquilinearity): on the same two-square surface at `k d = 3`, `F1 = U (1 + x/d) (1 + 0.3 i)` on both
  squares, `F2 = i F1`, `F3 = U (0.5 - 0.2 i + (0.7 + 0.4 i) y/L)` on square 1 and `U (0.2 + 0.9 i)` on square 2.
- Plate (b5, b6): steel `E = 2e11 Pa`, `nu = 0.3`, `rho_s = 7850 kg/m^3`, `0.3 x 0.2 m`, `h = 1 mm`, on `[-0.15, 0.15] x
  [-0.1, 0.1]` in `z = 0`, 24 x 16 `Shell4` (cells 12.5 mm), every boundary node with all six components prescribed zero
  (clamped), `modal_analysis_with` six modes, default options. The `VibratingSurface` of mode `j` has nodes and Quad4 faces of
  the shell mesh (counter-clockwise, normal `+z`) and velocity `[0, 0, phi_j,z]` at each node (unit modal velocity). Air as
  above; water `rho = 1000`, `c = 1500` for NC2. `N = 6`, `mu_j = 1`, `omega_j = sqrt(eigenvalue_j)`.
  The Ritz estimate of the frequencies: 164.2, 253.7, 402.2, 404.9, 485.8, 613.5 Hz; `k_1 = 3.0 /m`, wavelength 2.1 m against
  cells of 12.5 mm.

## Design

New code only in `crates/rcfd-post/src/solid_acoustic.rs`, additive: no existing function, struct field or default changes,
so every existing result keeps its bits (DB1, DB2). No new dependency, no unsafe code, the crate graph unchanged.

- `BaffledPlane::with_velocity(&self, velocity: Vec<[Phasor; 3]>) -> Result<BaffledPlane>`: the same nodes, faces and options
  with another nodal velocity (the plane rebuilt by `BaffledPlane::new` on the new surface, so normal, origin and points are
  those of a fresh plane). Refuses a velocity list of another length or values not finite (by `VibratingSurface::new`).
- `pub fn radiation_impedance_matrix(planes: &[BaffledPlane], rho, c, omega) -> Result<RadiationImpedanceMatrix>`: D1, each
  `P_m` by F-551's `surface_pressures`, the sums in the order of the points, single-threaded. Refuses an empty list, planes
  whose nodes, faces or options differ from the first's (compared exactly), and `rho`, `c`, `omega` not positive.
- `RadiationImpedanceMatrix { omega, power: Vec<Vec<Phasor>> }` (`power[m][n] = Z_mn`, W) with `len()`,
  `impedance(m, n) = 2 Z_mn` (for fields of unit amplitude the force per velocity, N s/m; for modal shapes the modal
  radiation impedance `Zr_nm`), `added_mass(m, n) = Im(2 Z_mn)/omega`, `damping(m, n) = Re(2 Z_mn)`.
- `pub struct LightFluidOptions { tolerance: 1e-12, max_iterations: 50 }` (builder methods) and `pub fn
  light_fluid_first_frequency(modes: &[BaffledPlane], omegas: &[f64], modal_masses: &[f64], rho, c, options) ->
  Result<LightFluidFrequency>`: D4. `LightFluidFrequency { omega_vacuum, added_mass, damping, damping_ratio, omega_first_order,
  omega_coupled, iterations, shape (modal coordinates of the coupled mode, unit max entry positive), added_mass_matrix (at
  omega_coupled) }` (`added_mass`, `damping`, `damping_ratio` of mode 1 at `omega_1`). Refuses lengths that differ, `omegas`
  not finite or not positive, `omegas[0] >= omegas[j]` for any `j > 0` (the first must be the lowest), modal masses not
  positive, a non-convergence (both loops), and a non-positive denominator of the Rayleigh quotient.

## Registered checks (limits fixed now)

Tests in `crates/rcfd-post/tests/sa_impedance_matrix.rs` and `sa_impedance_matrix_default_bits.rs`, release profile. "rel" is
`|measured/reference - 1|` for reals and `|measured - reference|/|reference|` for complex values. The largest of several errors
propagates NaN, and every value compared must be finite (a NaN or an infinity is a miss).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| Y1 | b2 | real fields: `max_mn |Z_mn - Z_nm| / max_m |Z_mm|`, pistons at `k d = 1`, 3 and the plate (six modes at `omega_1`) | `<= 1e-10` | 1.7e-17 (plate, numpy) |
| Y2 | b2 | complex fields `F1, F2, F3`: `max_mn |Z_nm[F] - Z_mn[conj F]| / max_m |Z_mm|` | `<= 1e-10` | rounding |
| Q1 | b1 | sesquilinearity: `|Z(F2, F3) - i Z(F1, F3)|`, `|Z(F3, F2) + i Z(F3, F1)|` over `max |Z|` | `<= 1e-12` | rounding |
| D1 | b3 | diagonal vs F-551: `|Z_mm - Pi_m|/|Pi_m|`, `Pi_m` the `complex_power` of `radiation_impedance` of a plane built by `BaffledPlane::new` on a fresh surface with field `m` (pistons both `k d`, complex `F1..F3`, the six plate modes) | `<= 1e-10` | rounding (same sums) |
| D2 | b3 | `|2 Z_mm/<|V_m|^2> - Z_F551|/|Z_F551|`, the same fields | `<= 1e-10` | rounding |
| M1 | b4 | `|z_12 - f|/|f|` and `|z_21 - f|/|f|`, `z = 2 Z/U^2`, `f = (i omega rho/(2 pi)) L^4 e^{-i k d}/d`, `k d = 1`, 3 | `<= 1e-2` (each) | 9.3e-5, 7.9e-4 |
| M2 | b4 | real and imaginary parts separately: `|Re z_12/Re f - 1|`, `|Im z_12/Im f - 1|`, both `k d` | `<= 1e-2` (each) | 1.4e-4, 1.2e-3 |
| M3 | b4 (resistance route) | `Re z_12` vs the F-418 power route `(W_pair - W_1 - W_2)/U^2`, `W` = `BaffledPlane::power` (rel) | `<= 1e-6` | rounding (same `sin` kernel) |
| R1 | b5 | report: `M_a,nn`, `C_nn`, `zeta_n` of the six plate modes, each at its own `omega_n`, and the matrices at `omega_1`; FE frequencies | printed, finite | `M_a,11 ~ 1.3e-2 kg`, `C_11 ~ 2.2 kg/s` |
| S1 | b5 (accuracy) | spectral reference D5 at `omega_1`, modes 1 to 3 (diagonal): `Im Zr` rel | `<= 1e-3` (each) | 4.9e-6 .. 3.3e-5 |
| S2 | b5 | the same, `Re Zr` rel | `<= 1e-6` (each) | 2.0e-14 .. 4.8e-13 |
| S3 | b5 | off-diagonal `Zr_1j`, `j` the first mode of the parity class of mode 1 after it: `|Zr_1j - ref|/|Zr_11|` | `<= 1e-3` | (as S1, not estimated separately) |
| P1 | b5 (selection) | `|Zr_mn|/sqrt(|Zr_mm Zr_nn|)` for every pair of different parity class, six modes at `omega_1` | `<= 1e-6` | ~0 (numpy printed -0.) |
| C1 | b5 (damping route) | `C_nn = Re Zr_nn` vs `2 W_n`, `W_n` = F-418 `power` of mode `n`'s field, six modes at `omega_1` | `<= 1e-6` | rounding |
| F1 | b6 | `|Delta_c/Delta_first - 1|`, air, `N = 6` | `<= 3e-2` | 9.1e-3 |
| F2 | b6 (root) | the returned `omega_c` solves the problem: `N = 1`: `|omega_1 sqrt(mu_1/(mu_1 + M_a,11(omega_c))) - omega_c|/omega_c` with `M_a,11(omega_c) = Im(2 Pi)/omega_c`, `Pi` recomputed by F-551 `radiation_impedance` of mode 1's field | `<= 1e-10` | ~1e-12 (stop rule) |
| F3 | b6 (root, N = 6) | `|lambda_min(K, Mu + M_a(omega_c)) - omega_c^2|/omega_c^2` with `M_a(omega_c)` recomputed by `radiation_impedance_matrix` and `lambda_min` by the test's own Jacobi rotations on `L^{-1} K L^{-T}`, `L` the Cholesky factor of `Mu + M_a` | `<= 1e-10` | ~1e-12 |
| F4 | b6 (first-order) | `omega_first_order == omega_1 (1 - M_a,11(omega_1)/(2 mu_1))` recomputed in the test from F-551's `radiation_impedance` | `<= 1e-12` rel | rounding |
| F5 | b6 (report) | `Delta_c/Delta_first - 1` against `-3 eps/4`, the `N = 1` and `N = 6` values, `zeta_1^2/2` | printed | -9.1e-3, -9.6e-3 |
| NC1 | b4 (negative control) | M1 at `d = 2 L` (pistons not small) | must be `> 1e-2` (each `k d`) | 2.1e-2, 0.17 |
| NC2 | b6 (negative control) | F1 in water (`rho = 1000`, `c = 1500`) | must be `> 3e-2` | 0.87 |
| NC3 | b2 (negative control) | Y1's measure on complex fields without the conjugation of D1 (`|Z_nm[F] - Z_mn[F]|/max|Z_mm|`) | must be `> 1e-6` | O(1) |
| N1 | (refusals) | never a panic, each an error: `radiation_impedance_matrix` of no plane, of planes of other nodes, faces or options, `rho`, `c`, `omega` not positive; `with_velocity` of another length or a NaN; `light_fluid_first_frequency` with lengths that differ, an empty list, `omegas` not positive or NaN, `omegas[0]` not the lowest, a modal mass not positive, `max_iterations = 1` (non-convergence) | holds | holds |
| T1 | (cost) | wall times printed (plate matrix, coupled iteration); a test above about 120 s in release becomes an ignored heavy record with a fast test of the same path | printed | seconds |
| DB1 | (defaults) | digests of the F-551 results (`surface_pressures`, `radiation_impedance`, `inverse_distance_potential`) and F-418 `power` on the F-551 piston with a complex velocity, printed at dfd2720d before any library line, equal after | `==` | holds |
| DB2 | (defaults) | the existing `sa_solid_acoustic`, `sa_solid_acoustic_default_bits`, `sa_radiation_impedance` (with its ignored records), `sa_radiation_impedance_amend1`, `sa_radiation_impedance_default_bits` pass unedited | pass | holds |

(M3 and F2 are written above with their definitions as registered: M3 `= (W_pair - W_1 - W_2)/U^2` where `W_pair` is the
power of both pistons moving with `U`, since `W_pair = W_1 + W_2 + Re(z_12) U^2` for real, equal velocities; F2 uses
`M_a,11 = Im(2 Pi)/omega` from F-551's `complex_power` `Pi` of mode 1's field, which for a unit modal velocity is `Im Zr_11/omega`.)

Mutations (on a scratch copy of the library, one at a time, own `CARGO_TARGET_DIR`; each must be caught by at least one
registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the conjugation of `V_n` dropped in `Z_mn` | Q1, Y2, D1, D2 |
| MU2 | the matrix transposed (`power[m][n]` takes `P_n` and `conj V_m`) | Q1 |
| MU3 | the factor 1/2 of `Z_mn` dropped | D1, D2, M1, S1, S2 |
| MU4 | `added_mass` from the real part (`Re(2 Z)/omega`) | F1, F2, F4 |
| MU5 | first-order estimate `omega_1 (1 - eps)` | F1, F4 |
| MU6 | the fixed point stopped after one step (no update of `M_a`) | F2, F3 |
| MU7 | `with_velocity` keeps the old normal velocities of the points | D1, D2, M1 |
| MU8 | the added mass left out of the coupled pencil (`Mu` only) | F1 |
| MU9 | the shared-geometry check of `radiation_impedance_matrix` removed | N1 |

## Decision rule

**done**: every check met at its registered run, every negative control as registered, every mutation caught, no amendment
after a run. **partial**: a check missed (recorded as `*_known_miss` with its limit), a negative control not as registered, a
mutation not caught, or a check met only after an amendment made after its run. **todo**: the module does not build or the
plate or piston runs cannot be made.

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation, unless marked open)

- OQ1 **open (not taken)**: a case-file key for the modal radiation (a post-processing request on a modal or harmonic run).
  It would change the schema golden; not done. Recommendation: none until the fan workflow asks for it.
- OQ2 the time conventions attributed to Pierce, Morse and Ingard (`e^{-i omega t}`) and Kinsler et al. (`e^{j omega t}`) and
  the Pritchard citation are from memory; nothing depends on them (D2 derives the formula in F-551's convention). Recorded in
  `docs/reference-data.md` as from memory.
- OQ3 damping in the coupled frequency (the complex nonlinear eigenproblem with `C`): left out, its effect `zeta^2/2` reported.
- OQ4 higher fluid-loaded modes (tracking mode `j > 1` through the fixed point): left out; only the first frequency is asked.

## Limits (what is not covered)

- Plane radiator in a rigid baffle, one side; no unbaffled or two-sided (fluid on both faces) plate, no curved radiator.
- Light fluid: the coupled frequency is a modal truncation (`N` vacuum modes) with the added mass at the root; damping not in
  the root; no heavy-fluid coupled BEM-FE solve, no feedback into the FE matrices.
- Cost `O(N P^2)` per matrix (`N` fields, `P` points), each fixed-point step one matrix.
- References: closed forms, the spectral integral and the F-551/F-418 routes; no measured data.

## Addendum 1: results (2026-11-14)

Commits: 398867f3 (registration, alone), 9aed348d (the DB1 bit record, constants printed at dfd2720d before any library
line), 6fdc0155 (library and checks, `cargo fmt` applied before the run), then the module docs, `docs/acoustics.md`,
`docs/reference-data.md` and this addendum. The registered run (`cargo test --release -p rcfd-post --test
sa_impedance_matrix -- --test-threads=1 --nocapture`, commit 6fdc0155, log `f579-scratch/run_checks.log`) was made once:
5 tests, all met, 21 s. Release profile throughout. No amendment.

| Id | Limit | Measured | Met |
|---|---|---|---|
| Y1 asymmetry, real fields (pistons `k d = 1`, 3; plate six modes at `omega_1`) | `<= 1e-10` | 3.8e-18; 8.8e-16 | yes |
| Y2 complex fields `|Z_nm[F] - Z_mn[conj F]|/max|Z_mm|` | `<= 1e-10` | 3.4e-16 | yes |
| Q1 sesquilinearity, both sides | `<= 1e-12` | 0, 0 | yes |
| D1 diagonal vs F-551 complex power (pistons, complex fields, six plate modes) | `<= 1e-10` | 0 (bit-equal) | yes |
| D2 `2 Z_mm/<|V|^2>` vs F-551 impedance, the same | `<= 1e-10` | 0 | yes |
| M1 `|z_12 - f|/|f|`, `|z_21 - f|/|f|`, `k d = 1`, 3 | `<= 1e-2` each | 9.26e-5, 9.26e-5; 7.91e-4, 7.91e-4 | yes |
| M2 real and imaginary parts, `k d = 1`; 3 | `<= 1e-2` each | 5.9e-5, 1.44e-4; 1.21e-3, 7.8e-4 | yes |
| M3 `Re z_12` vs `(W_pair - W_1 - W_2)/U^2` (F-418) | `<= 1e-6` | 1.7e-15, 1.6e-14 | yes |
| R1 report (finite) | printed | see below | yes |
| S1 `Im Zr_nn` vs the spectral reference, modes 1 to 3 | `<= 1e-3` each | 4.9e-6, 1.67e-5, 3.73e-5 | yes |
| S2 `Re Zr_nn`, the same | `<= 1e-6` each | 5.9e-15, 2.9e-14, 2.1e-14 | yes |
| S3 `|Zr_13 - ref|/|Zr_11|` (mode 3 is the first of mode 1's parity) | `<= 1e-3` | 8.9e-7 | yes |
| P1 different parity `|Zr_mn|/sqrt(|Zr_mm Zr_nn|)` | `<= 1e-6` | 8.3e-12 | yes |
| C1 `C_nn` vs `2 W_n` (F-418), six modes | `<= 1e-6` | 1.4e-13 | yes |
| F1 `|Delta_c/Delta_first - 1|`, air, `N = 6` | `<= 3e-2` | 9.20e-3 | yes |
| F2 `N = 1` root by F-551 | `<= 1e-10` | 0 | yes |
| F3 `N = 6` root by the matrix and Jacobi | `<= 1e-10` | 0 | yes |
| F4 first-order estimate recomputed | `<= 1e-12` | 0 | yes |
| F5 report | printed | `N = 6` -9.20e-3, `N = 1` -9.28e-3, `-3 eps/4` -9.65e-3, `zeta^2/2` 5.8e-7 | (record) |
| NC1 pistons at `d = 2 L` | must be `> 1e-2` | 2.12e-2, 0.167 | as registered |
| NC2 water | must be `> 3e-2` | 0.870 | as registered |
| NC3 complex fields without the conjugation | must be `> 1e-6` | 2.0 | as registered |
| N1 refusals | holds | 16 of 16, no panic | yes |
| T1 times | printed | plate modal 0.1 s, plate matrix (6 fields, 3456 points) 1.3 s, spectral 0.25 s, coupled `N = 6` 5.7 s (4 steps), water 7.4 s (5 steps) | (record) |
| DB1 two digests of the F-551 results | `==` | 2 of 2 | yes |
| DB2 existing `solid_acoustic` tests unedited | pass | pass (in `cargo test -p rcfd-post --release`, every binary ok) | yes |

Reported (b5), the FE plate (24 x 16 `Shell4`, clamped, steel 1 mm, 0.3 x 0.2 m), air, each mode at its own frequency:

| Mode | f (Hz) | parity (x, y even) | added mass (kg) | radiation damping (kg/s) | zeta |
|---|---|---|---|---|---|
| 1 | 165.653 | even, even | 1.2873e-2 | 2.2683 | 1.076e-3 |
| 2 | 256.291 | odd, even | 7.0740e-3 | 1.9058e-1 | 5.88e-5 |
| 3 | 413.219 | even, even | 5.0926e-3 | 1.7937 | 3.44e-4 |
| 4 | 413.923 | even, odd | 5.8452e-3 | 5.6807e-1 | 1.09e-4 |
| 5 | 497.119 | odd, odd | 4.6699e-3 | 9.3079e-2 | 1.48e-5 |
| 6 | 637.012 | odd, even | 4.1889e-3 | 1.8216 | 2.27e-4 |

(unit modal masses; the matrices at `omega_1` are printed in `run_checks.log`; the largest off-diagonal added mass is
`M_a,13 = 2.26e-3 kg`). First frequency in air: vacuum 165.653 Hz, `eps = 1.287e-2`; first-order shift `-6.436e-3`,
coupled (`N = 6`) `-6.377e-3`, i.e. 164.597 Hz against 164.587 Hz first-order. In water the light-fluid estimate fails
(`eps = 10.96`; first-order `-5.48`, coupled `-0.712`).

Mutations (scratch copy `f579-scratch/copy`, git archive of 6fdc0155, own target `target-copy`, `mutate.py`, logs `MU*.log`):

| Id | Expected | Caught by |
|---|---|---|
| MU1 conjugation dropped | Q1, Y2, D1, D2 | Q1, Y2, D1, D2 (and NC3 no longer as registered) |
| MU2 matrix transposed | Q1 | Q1 (both sides, alone, as registered) |
| MU3 factor 1/2 dropped | D1, D2, M1, S1, S2 | D1, D2, M1, M2, M3, S1, S2, S3, C1, F2, F4 |
| MU4 added mass from the real part | F1, F2, F4 | F2, F4; **not F1** (`Delta_c/Delta_first - 1` became 2.6e-3, inside 3e-2: the wrong added mass enters both shifts alike) |
| MU5 first-order `omega_1 (1 - eps)` | F1, F4 | F1, F4 |
| MU6 fixed point stopped after one step | F2, F3 | F2, F3, N1 (`max_iterations = 1` no longer refused) |
| MU7 `with_velocity` keeps the old velocities | D1, D2, M1 | D1, D2, M1, M2, M3, Q1, NC3 |
| MU8 added mass left out of the coupled pencil | F1 | F1, F2, F3, N1 |
| MU9 shared-geometry check removed | N1 | N1 |

Nine of nine caught by at least one registered check; MU4 by two of its three expected checks.

Run besides: `cargo test -p rcfd-post --release` (62 test binaries and doc-tests, all ok, log `run_post_all.log`, on the tree of f94a5ee5; it ran the registered tests a second time as part of the suite (passed), not used for any decision); `cargo test -p rcfd-api --release --test baseline_hashes` (3 passed, 1 ignored as before) and `cargo test -p rcfd-cli --release --test docs_site` (2 passed), log `run_gates.log`; `cargo test -p rcfd-fem --release` (no rcfd-fem file changed; log `run_fem_all.log`, exit 0, 222 test binaries and doc-tests ok, none failed); `cargo fmt -p rcfd-post --check` clean; `cargo clippy -p rcfd-post --all-targets --release -- -D warnings` clean after commit f94a5ee5. Not run: the whole workspace (the lead runs the gate). In the library only `crates/rcfd-post/src/solid_acoustic.rs` changed: module docs (lines 15 to 20) and new code appended at the end (from the section "impedance matrix (ADR 0538)"); no shared solver file; no `rcfd-fem` file.

Deviations: (1) after the registered run clippy flagged three `&[a.clone()]` in the refusal test (`cloned_ref_to_slice_refs`); they were replaced by `std::slice::from_ref(&a)` (commit f94a5ee5), the same calls, no check, set-up or limit changed. (2) The registered expectation of MU4 listed F1, which did not catch it (it was caught by F2 and F4). The estimate script's first run of part (1) printed NaN for the scheme (a script
defect, disclosed in the registration and fixed before it).

**Decision by the registered rule: done.** Every check met at its single registered run, every negative control as registered, every mutation caught by at least one registered check, no amendment. Every clause of the acceptance met: b1 (the matrix by F-551's surface pressure), b2 by Y1 and Y2 (8.8e-16 against 1e-10), b3 by D1 and D2 (bit-equal against 1e-10), b4 by M1 (9.3e-5 and 7.9e-4 against 1e-2), b5 by R1 with S1 to S3, P1 and C1, b6 by F1 (9.2e-3 against 3e-2) with F2 to F4.

Remaining questions:
1. OQ1 (open, not taken): a case-file key for the modal radiation of a modal run. Options: (a) none; (b) a post-processing
   key on modal and harmonic runs naming the surface set and the medium. Recommendation: (a) until the fan workflow needs it
   (it changes the schema golden, the maintainer's step).
2. OQ3 damping in the coupled root. Options: (a) leave out (`zeta^2/2 = 5.8e-7` here); (b) a complex fixed point with `C`.
   Recommendation: (a) for light fluids; (b) only with a heavy-fluid item.
3. OQ4 higher fluid-loaded modes. Options: (a) only the first (as asked); (b) track mode `j` by the largest modal overlap.
   Recommendation: (b) as a small follow-up if the fan needs fluid-loaded frequencies beyond the first.
4. OQ2 citations from memory (the time conventions of Pierce, Morse and Ingard, Kinsler et al.; Pritchard 1960). Nothing
   depends on them. Recommendation: confirm when the books are at hand; no code change expected.

### For the lead: roadmap entries

F-579, status proposed **done**, new note:

> ADR 0538, 2026-11-14: rcfd-post solid_acoustic: radiation_impedance_matrix (Z_mn = (1/2) sum w P[V_m] conj(V_n) by F-551's surface pressure; impedance 2 Z, added_mass Im(2Z)/omega, damping Re(2Z)), BaffledPlane::with_velocity, light_fluid_first_frequency (modal added mass, damping and damping ratio of the first mode, the first-order frequency omega_1 (1 - M_a/(2 mu)) and the lowest root of K q = omega^2 (Mu + M_a(omega)) q on the vacuum modes by a fixed point in omega, damping left out); all additive, the F-551 results bit-identical (two digests), baseline hashes unchanged. Matrix symmetric for real fields within 8.8e-16 (limit 1e-10), Z_nm[V] = Z_mn[conj V] for complex fields within 3.4e-16, diagonal bit-equal to F-551's complex power and impedance. Two square pistons (side L, d = 30 L): mutual term within 9.3e-5 (k d = 1) and 7.9e-4 (k d = 3) of (i omega rho/(2 pi)) S1 S2 e^{-i k d}/d, the formula in F-551's e^{+i omega t} convention without conjugation, z_12 = 2 Z_12/(U1 conj U2); 2.1e-2 and 0.17 at d = 2 L (negative control). Clamped steel plate 0.3 x 0.2 x 1 mm (24 x 16 Shell4, FE modes): modal added mass 1.29e-2 kg and radiation damping 2.27 kg/s of the first mode (zeta 1.08e-3), six modes reported; against an independent wavenumber-integral reference within 3.7e-5 (reactance) and 3e-14 (resistance); different-parity coupling 8.3e-12. First frequency 165.65 Hz in vacuum, first-order shift -6.436e-3, coupled (six modes) -6.377e-3: ratio within 9.2e-3 of 1 (limit 3e-2, about -3 eps/4); water 0.87 (negative control). Nine of nine mutations caught. Not covered: damping in the coupled root, fluid-loaded modes above the first, two-sided or unbaffled plates, a case-file key.

F-551 note: unchanged except its last sentence, from "Optional follow-up F-579." to:

> Follow-up F-579 (ADR 0538, 2026-11-14).

No follow-up item is proposed. Optional, only if the fan needs it (not added):

> Fluid-loaded frequencies above the first (track mode j through the fixed point by the largest modal overlap) and the radiation damping in the coupled root (complex fixed point); registered against the first-order estimates in a light fluid.
