# ADR 0476: F-416 registration: a flexible rotor on bearings in the stationary frame (shaft of F-391 beams with the gyroscopic matrix, rigid discs, bearings with cross-coupling; whirl, critical speeds, stability, unbalance response; joined to the rigid rotor of ADR 0120)

Status: **REGISTERED 2026-11-14, written before any code of F-416 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-416 (deps F-414, F-391, F-393), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
work package SA-9, stub [ADR 0319](0319-flexible-rotor-on-bearings-stationary-frame.md); the modules `rcfd_fem::rotor` and
`rcfd_post::solid_rotor` declared by [ADR 0440](0440-f408-registration-advanced-tier-scaffold.md) (F-408) with `PENDING =
Some("ADR 0319 (F-416)")`. Number 0476 given by the lead. Branch `f416-work` from main 57808e24, worktree `w49i-work/f416`,
scratch `w49i-work/f416-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is reached
only through new types and functions); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no existing
test edited (the scaffold row follows the standing pattern of ADR 0291); no new dependency, no unsafe code; clean-room. Every
choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation).

## What F-416 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. At zero speed the frequencies equal those of the beam model of F-391 within 1e-10. Disc on a light shaft between two bearings: critical speed sqrt(k/m) and unbalance amplitude e r^2/sqrt((1 - r^2)^2 + (2 zeta r)^2) within 1e-3 at r = 0.5, 1 and 2. In the stiff-shaft limit the critical speeds and the response equal rotordyn::critical_speeds and rotordyn::response for the same rotor within 1e-6. Uniform shaft on pins with rotary inertia, 20 elements: forward and backward whirl of modes 1 and 2 equal the roots of E I k^4 - rho A w^2 - rho I k^2 w^2 +/- 2 rho I k^2 Omega w = 0, k = n pi/L, within 0.5 percent at three speeds. Overhung disc on a massless cantilever shaft: the four whirl speeds equal the roots of the two-unknown closed form (derived in the registration) within 0.5 percent. Anisotropic bearings split the critical speed into sqrt(kx/m) and sqrt(ky/m) within 1e-3. A cross-coupled stiffness k_c on a damped disc is stable below c w_n and unstable above, the threshold within 1 percent. Forward and backward are labelled by the sense of the orbit; an unstable speed is reported, never dropped.

Clauses: **c1** zero-speed frequencies equal F-391's within 1e-10; **c2** Jeffcott critical speed `sqrt(k/m)` within 1e-3; **c3**
Jeffcott unbalance amplitude within 1e-3 at `r = 0.5, 1, 2`; **c4** stiff-shaft critical speeds equal `rotordyn::critical_speeds`
within 1e-6; **c5** stiff-shaft response equals `rotordyn::response` within 1e-6; **c6** the spinning Rayleigh shaft on pins, 20
elements: forward and backward whirl of modes 1 and 2 within 0.5 percent at three speeds; **c7** the overhung disc: four whirl
speeds within 0.5 percent; **c8** anisotropic bearings: `sqrt(kx/m)`, `sqrt(ky/m)` within 1e-3; **c9** cross-coupling: stable
below `c w_n`, unstable above, the threshold within 1 percent; **c10** forward and backward labelled by the sense of the orbit;
**c11** an unstable speed reported, never dropped.

## What was read (main 57808e24, read only)

ADR 0319 (stub) and the roadmap entry; ADR 0310 (sections 2.5 and 7, the files and the `rotordyn` rule "that file is not
edited; `solid_rotor` converts"); ADR 0120 and the whole of `crates/rcfd-post/src/rotordyn.rs` (the rigid rotor: complex
coordinates `w = x + i y`, `psi = theta_y - i theta_x`, the 2 x 2 dynamic stiffness `[[kt - m lam^2, kc], [kc, kr - It lam^2 + Ip
spin lam]]`, `critical_speeds` as the positive roots `s = spin^2` of `(kt - m s)(kr - (It - Ip) s) = kc^2`, `natural_frequencies`,
`response` with the forward and backward circular parts, `unbalance_load`, `ellipse_major`); ADR 0417 and `beam.rs` (F-391:
the interdependent Timoshenko element, `plane_stiffness`, `plane_mass`, `plane_shape_coefficients`, `local_stiffness`,
`local_mass`, `circle_section`, `BeamSection`); ADR 0450 and `eigen_general.rs` (`quadratic_eigen_with`: general `C` and `K`,
`M` symmetric positive definite, vectors with `x^H M x = 1`, residuals); ADR 0472 and `gyro.rs` (F-415: `split_modes_dense`, the
scaled time `tau`, `SplitCampbell`, `SplitSpeed`, the stability rule with `STABILITY_TOLERANCE`, the growth per track); ADR 0411
and `rotating/whirl.rs` (F-499: `Whirl::of` with `WHIRL_TOLERANCE = 1e-6`, `WhirlMode`, `WhirlModes`, `Entry`, the complex-line
tracker `track_whirl`, `WhirlCampbell::{to_csv, crossings}`); `rotating/whirl/state.rs` (`Stability`, `STABILITY_TOLERANCE =
1e-8`); `damped.rs` (`REAL_TOLERANCE = 1e-6`); `modal.rs`, `assemble.rs`, `mass.rs` (`consistent`, `mass_properties`),
`model.rs` (`Material`, `ElasticSupport`, `Constraint`); the tests `si_beam.rs`, `sa_gyro.rs`, `sa_gyro_default_bits.rs` (the
digest pattern), `sa_scaffold.rs` (the X1 copies), `w46c_rotordyn.rs`.

What exists matters for the scope. `rotordyn` is the rigid rotor on two isotropic bearings; F-499 and F-415 solve the rotating
frame of a solid part. Nothing solves a shaft in the stationary frame with bearings: F-416 adds it as a new dense model in
`rcfd_fem::rotor` that **reuses** the F-391 element matrices of one bending plane, the quadratic solver of ADR 0450 with the
time scaling of ADR 0472 D3, the whirl direction `Whirl` of ADR 0411 D3, the complex-line tracker of ADR 0411 D4 and the
`SplitCampbell` table and stability rule of ADR 0472 D4, D5. No new eigensolver of the quadratic problem is written; a small
Hermitian Jacobi routine is written for the (linear, Hermitian) critical-speed problem and a dense complex LU for the response.

Published sources, cited from memory, **not read** for this work: G. Genta, Dynamics of Rotating Systems (Springer 2005) (the
Jeffcott rotor, forward and backward whirl, the cross-coupled instability threshold, the spinning Rayleigh beam); M. I. Friswell,
J. E. T. Penny, S. D. Garvey, A. W. Lees, Dynamics of Rotating Machines (Cambridge 2010) (the finite-element rotor with the
gyroscopic matrix, the Campbell diagram, unbalance response, the overhung disc); D. W. Childs, Turbomachinery Rotordynamics (Wiley
1993) (cross-coupled stiffness and the threshold `k_c = c w_n`); M. Lalanne, G. Ferraris, Rotordynamics Prediction in Engineering
(Wiley, 2nd ed. 1998) (the gyroscopic shaft element); H. D. Nelson, J. M. McVaugh, J. Eng. Ind. 98 (1976) 593 (the rotor element
with rotary inertia and gyroscopic moments). Every formula used is derived below from first principles; the sources are cited for
the names, not for any number.

## Design

### D1. The model and its unknowns

A `RotorModel` is a list of stations along the spin axis `z` (positions `z_0 < z_1 < ...`, strictly increasing, finite; key
`"nodes"`), with four unknowns per station in stationary axes, `(u_x, u_y, theta_x, theta_y)` (small rotations, right-handed),
spin `Omega >= 0` about `+z`. Its parts:

* **shaft elements** between two stations (`add_shaft(a, b, section, material)`): a `BeamSection` of F-391 and an isotropic
  elastic `Material` (density may be 0: a massless shaft). The section must be axisymmetric, `I_y == I_z` and `shear_y ==
  shear_z` exactly (a non-axisymmetric shaft has time-varying matrices in the stationary frame); else refused, key `"section"`.
* **discs** at a station (`add_disc(node, Disc { mass, transverse, polar })`, each finite and `>= 0`, key `"disc"`): rigid,
  centred.
* **bearings** at a station (`add_bearing(node, Bearing { k, c })`, `k`, `c` real 2 x 2 matrices on `(u_x, u_y)`, finite, key
  `"bearing"`): the force on the rotor is `f = -k u - c u_t`; `kxy != kyx` is a cross-coupled (circulatory) stiffness;
  `Bearing::isotropic(k, c)` gives `k I`, `c I`. A bearing with `k = 0` is an external damper (used for the Jeffcott damping).
* **pins** (`pin(node)`: `u_x = u_y = 0`) and **clamps** (`clamp(node)`: all four zero). A station index out of range is
  refused, key `"node"`.

The matrices on the free unknowns: `M` (shaft consistent mass with rotary inertia, disc `m` on translations and `I_t` on
rotations), `K` (shaft stiffness plus the bearing `k`), `C` (bearing `c`), `G` (the gyroscopic matrix per unit spin, skew). The
equations of motion are `M q_tt + (C + Omega G) q_t + K q = f(t)`.

### D2. The shaft element (F-391's plane matrices, the gyroscopic matrix from the same rotations)

Per element each bending plane is the F-391 plane of ADR 0417 arithmetic 2 and 3: the plane `X` with the unknowns `(u_x, theta_y)`
(`u_x' = theta_y` for a slender shaft) and the plane `Y` with `(u_y, -theta_x)` (`u_y' = -theta_x`: a rotation about `x` turns
`e_z` towards `-e_y`). In the F-391 variables `(d_0, psi_0, d_1, psi_1)` each plane has the stiffness `plane_stiffness(E I,
kappa G A, L)` and the mass `plane_mass(rho A, rho I, E I, kappa G A, L)` (interdependent shape functions, four-point Gauss,
exact). These two private functions of `beam.rs` become `pub(crate)`; nothing else in `beam.rs` changes.

The gyroscopic matrix (first principles): a section of polar inertia per length `rho I_p` (`I_p = I_y + I_z`) spinning at
`Omega` with section rotations `theta_x, theta_y` has the angular momentum `rho I_p Omega n`, `n = (theta_y, -theta_x, 1)` to
first order; its rate `rho I_p Omega (theta_y,t, -theta_x,t, 0)` is the moment the section needs, so the distributed gyroscopic
moments are `m_x = rho I_p Omega theta_y,t`, `m_y = -rho I_p Omega theta_x,t` (the disc: `M_x = I_t theta_x,tt + I_p Omega theta_y,t`,
`M_y = I_t theta_y,tt - I_p Omega theta_x,t`, the same as `rotordyn`'s `kr - It lam^2 + Ip spin lam` in `psi = theta_y - i
theta_x`). With the section rotation of each plane interpolated by F-391's functions `N_psi` (`theta_y = N_psi q_X`, `-theta_x =
N_psi q_Y`), the consistent generalised forces are `Omega G q_t` with `G = [[0, P], [-P, 0]]` in `(q_X, q_Y)`, `P = int rho I_p
N_psi^T N_psi dz = plane_mass(0, rho I_p, E I, kappa G A, L)`: skew by construction. In station unknowns: `G[theta_y, theta_x]
= -I_p` and `G[theta_x, theta_y] = +I_p` for a disc, and the element `P` placed with the sign `-1` on every `theta_x`. The disc
and element entries are added to `(p, q)` and to `(q, p)` with opposite signs from the same numbers, so `G` is skew bit for bit.

### D3. Condensation of massless unknowns

A free unknown whose row and column are exactly zero in `M`, `C` and `G` (a node of a massless shaft without a damper or a disc
rotation without inertia) carries no inertia and no velocity force: it is condensed statically, `K* = K_aa - K_ab K_bb^-1 K_ba`
(dense LU with partial pivoting; exact in exact arithmetic), and recovered as `q_b = -K_bb^-1 K_ba q_a`. If `K_bb` is singular
the model is refused (key `"stiffness"`). After condensation `M_aa` must be positive definite (the solver's Cholesky), else
refused (key `"mass"`: a massless unknown with a damper is a first-order unknown, not taken).

### D4. Whirl at one speed

`RotorModel::whirl(Omega) -> RotorWhirl`: the quadratic problem `(lambda^2 M + lambda (C + Omega G) + K) x = 0` on the condensed
unknowns by `quadratic_eigen_with` in the scaled time `lambda = tau mu`, `tau = sqrt(max_i |K_ii|/M_ii) + max_i (sum_j |C_ij +
Omega G_ij|)/M_ii` (`tau = 1` if 0; ADR 0472 D3 with the damping and gyroscopic rate). All `2 n` roots are kept. A root with `Im
lambda > REAL_TOLERANCE |lambda|` and `Im lambda > 0` is a **whirl mode** `u = Re(x e^{lambda t})`: `omega = Im lambda`, `growth
= Re lambda`, the damping ratio `-Re lambda/|lambda|`, the shape on every station (fixed unknowns 0, condensed ones recovered),
the residual of the solver, and the **whirl number** (D5) with its direction. The others are listed as `real_roots`. The speed is
**unstable** when `max Re lambda > STABILITY_TOLERANCE max |lambda|` over all roots, `Stability::Unstable { growth: max Re
lambda }`, else `Stable` (ADR 0472 D4; with damping a stable speed has every `Re lambda < 0`). **No mode is dropped for being
unstable**: a mode with `growth > 0` stays in the list (clause c11). Modes sorted by `omega`.

### D5. Forward and backward: the sense of the orbit

The orbit of station `i` in mode `x` is `(Re(X_i e^{i omega t}), Re(Y_i e^{i omega t}))`, `X_i, Y_i` the `u_x, u_y` entries;
`x y_t - y x_t` averaged over a period is `omega Im(X_i conj(Y_i))`, so the orbit turns in the sense of the spin (counter-clockwise
seen from `+z`) when `Im(X_i conj Y_i) > 0` (forward) and against it when `< 0` (backward). The mode's **whirl number** weights
all unknowns by the mass, as ADR 0411 D3 does: with the plane vectors `q_X = (u_x, theta_y)` and `q_Y = (u_y, -theta_x)` per
station (the tilt `psi = theta_y - i theta_x` turns like `w = x + i y`), `kappa = 2 (Re q_X^T (-P_m) Im q_Y + Re q_Y^T P_m Im q_X)
/ x^H M x`, `P_m = (M_X + M_Y)/2` the mass of one plane (the `X` and `Y` blocks of `M`, `Y` with the signs; equal for an
axisymmetric rotor). For a circular forward orbit `q_Y = -i q_X` and `kappa = +1`, backward `-1`, a straight line 0.
`Whirl::of(kappa)` labels it (`WHIRL_TOLERANCE = 1e-6`, ADR 0411). `kappa = x^H S_h x / x^H M x` with the Hermitian `S_h = -i S`,
`S = [[0, -P_m], [P_m, 0]]`, a Rayleigh quotient in `[-1, 1]`.

Degenerate roots: roots within `1e-8 |lambda|` of each other (an isotropic rotor without gyroscopic coupling: forward and backward
coincide) form a cluster whose vectors are an arbitrary basis; the cluster is resolved by the Hermitian pencil `(Z^H S_h Z, Z^H M
Z)` (Cholesky of the second, Jacobi of D7), whose eigenvectors are the circular forward and backward modes. The same is done for
the critical speeds (D6).

### D6. Critical speeds

Undamped synchronous whirl: `lambda = i Omega` with spin `Omega` gives `(K - Omega^2 M + i Omega^2 G) z = 0`, i.e. `K z = s (M -
i G) z`, `s = Omega^2`, `M - i G` Hermitian (G real skew). With `K` symmetric positive definite (after D3, damping ignored as in
`rotordyn`): `K = L L^T`, `A = L^-1 (M - i G) L^-T` Hermitian, its eigenvalues `mu` by the Jacobi method of D7; each `mu > 0`
gives a critical speed `Omega = 1/sqrt(mu)` with the mode `z = L^-T y` and its whirl number (D5; clusters resolved as in D5,
relative tolerance 1e-8 on `mu`). Every real critical speed is listed with its direction: **forward** ones are those `rotordyn`
lists, **backward** ones are reached only by a backward-rotating excitation (anisotropy), a **neutral** (straight-line orbit)
one belongs to anisotropic bearings. `mu <= 0` (a disc mode with `I_p >= I_t` has no forward crossing) gives none. A `K` that is
not symmetric bit for bit (cross-coupled bearings) is refused (key `"cross_coupling"`: no real critical-speed problem; use
`whirl`), a `K` not positive definite (key `"stiffness"`).

### D7. Hermitian Jacobi and complex LU (new private routines)

Cyclic Jacobi for a Hermitian `n x n` matrix: for the pivot `a_pq = |a_pq| e^{i phi}` the phase `D = diag(1, e^{-i phi})` makes
it real and the real rotation (`tau = (a_qq - a_pp)/(2 |a_pq|)`, `t = sgn(tau)/(|tau| + sqrt(1 + tau^2))`, `c = 1/sqrt(1 + t^2)`,
`s = t c`; Golub and Van Loan's symmetric Jacobi) zeroes it; `U = D R` applied as `A <- U^H A U`, `V <- V U`; sweeps until the
off-diagonal Frobenius norm is `<= 1e-15` of the whole (cap 100 sweeps, refused beyond: key `"solver"`). Complex LU with partial
pivoting (by modulus) for the response; a zero pivot is refused (key `"singular"`: an undamped resonance).

### D8. Harmonic and unbalance response

`harmonic_response(Omega, omega, loads)`: nodal phasors `[F_x, F_y, M_x, M_y]` (`f(t) = Re(F e^{i omega t})`) at stations; the
complex system `(K - omega^2 M + i omega (C + Omega G)) X = F` on all free unknowns (no condensation needed: `M` may be singular),
`omega > 0`. The result: per station the phasors `[u_x, u_y, theta_x, theta_y]` and the **orbit**: forward radius `|X + i Y|/2`,
backward radius `|conj X + i conj Y|/2`, semi-major axis their sum (`rotordyn::ellipse_major`), semi-minor `|r_f - r_b|`, the sense
`Whirl::of((r_f^2 - r_b^2)/(r_f^2 + r_b^2))` (`r_f^2 - r_b^2 = Im(X conj Y)`). `unbalance_response(Omega, unbalances)` with
`Unbalance { node, amount, angle }` (`amount = m e`, kg m): the load `F_x = amount Omega^2 e^{i angle}`, `F_y = -i F_x` (the
force rotating with the rotor, the convention of `rotordyn::unbalance_load`) at `omega = Omega > 0`.

### D9. Campbell table

`campbell(speeds) -> gyro::SplitCampbell` (speeds finite, `>= 0`, strictly increasing, key `"speeds"`): D4 at every speed, the
modes tracked by `track_whirl` of ADR 0411 D4 (the complex lines, `M`-weighted on the free unknowns), the growth per track and
speed, the stability per speed (`SplitCampbell::unstable()` lists `(speed, growth)` of every unstable speed; `to_csv` adds the
stability columns), `coriolis = true`, `basis = None`, `loss = None`. The engine-order crossings of `WhirlCampbell::crossings`
come with it.

### D10. Joining the rigid rotor (`rcfd_post::solid_rotor`)

* `flexible_from_rigid(rotor: &rotordyn::Rotor, shaft: &Shaft) -> RotorModel`: stations at the bearing positions and the centre
  of mass (merged if equal), the disc `(mass, it, ip)` at the centre, isotropic bearings `(k_j, c_j)`, shaft elements of the given
  section and material between consecutive stations; refuses a bearing with `eta != 0` (key `"eta"`, no hysteretic term here).
* `rigid_from_flexible(model) -> rotordyn::Rotor`: total mass, centre, `I_t` about the centre and `I_p` of the discs and the
  shaft elements (rigid-body inertia of each element: `rho A L`, `rho A (L^3/12 + L d^2) + rho I L` transverse, `rho I_p L` polar),
  exactly two bearings, each isotropic (`kxx = kyy`, `kxy = kyx = 0`, the same for `c`; key `"bearing"`); positions measured from
  the centre.
* `nodal_load(load: &HarmonicLoad) -> [C64; 4]`: `[F_x, F_y, M_x, M_y]` of a `rotordyn` load in `rcfd_linalg::complex::C64`;
  `response_to_rigid` is not needed (the station phasors are compared directly).
* `disc_from_solid(model: &SolidModel) -> SolidDisc`: the mass properties of a solid part by the consistent mass
  (`mass::consistent`, `mass::mass_properties`, ADR 0284): `mass`, `I_t = (I_xx + I_yy)/2`, `I_p = I_zz` about the centre, the
  centre; refused when the centre is off the axis by more than `1e-9` of the part's size (key `"centre"`) or `|I_xx - I_yy| > 1e-9
  (I_xx + I_yy)` or a product of inertia with `z` above that (key `"inertia"`: not axisymmetric enough for a rigid disc).

`rotordyn.rs` is not edited. `solid_rotor::PENDING = None`, `rotor::PENDING = None`.

### D11. Interfaces (all new)

`rcfd_fem::rotor`: `RotorModel`, `Disc`, `Bearing`, `Unbalance`, `RotorWhirl`, `RotorWhirlMode`, `CriticalSpeed`,
`RotorResponse`, `Orbit`, `C64` (re-export), and the methods above; `rcfd_post::solid_rotor`: `Shaft`, `SolidDisc` and the four
functions. Case-file key: none is added (the maintainer's call); the recommendation is in the addendum.

## Set-ups (fixed now)

Steel `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3` unless stated; solid circular sections `circle_section(r, kappa)` (`A = pi
r^2`, `I = pi r^4/4`, `I_p = 2 I`).

**Z (zero speed, F-391):** shaft `L = 1 m`, `r = 0.05 m`, `kappa = 0.9`, 20 equal elements; **Z-a** pinned at both ends; **Z-b**
on isotropic undamped bearings `k = 1e7 N/m` at both ends. The F-391 model: 21 nodes on the `z` axis, one Beam2 block with the
same section, orientation `(1, 0, 0)`, `u_z = theta_z = 0` at every node (axial and torsion removed), Z-a `u_x = u_y = 0` at the
ends, Z-b `ElasticSupport::nodal_spring` `1e7` in `x` and `y` at the ends; `modal_analysis` with 8 modes, tolerance 1e-12,
residual test Backward.

**J (Jeffcott: disc on a light shaft between two bearings):** `L = 0.4 m`, `r = 0.01 m`, `kappa = 0.9`, density 0, 4 elements,
point disc `m = 2 kg` (`I_t = I_p = 0`) at the middle station, isotropic undamped bearings `k_b = 1e6 N/m` at the ends, an
external damper (bearing with `k = 0`, `c I`) at the disc. Closed form (Timoshenko shaft under a central load, exact for the
element): `k_s = 1/(L^3/(48 E I) + L/(4 kappa G A))`, `k = 1/(1/k_s + 1/(2 k_b))`, `w_n = sqrt(k/m)`; `zeta = 0.05`, `c = 2 zeta
sqrt(k m)`; unbalance `amount = m e`, `e = 1e-4 m`, at the disc, angle 0; `r = Omega/w_n in {0.5, 1, 2}`. The amplitude is the
semi-major axis of the disc orbit (circular).

**X (cross-coupling on the damped disc):** J with a bearing at the disc of `k = [[0, k_c], [-k_c, 0]]` added (`f_x = -k_c y`,
`f_y = k_c x`: energy into a forward orbit) at the spin `0.5 w_n`. Closed form (complex coordinate `w = x + i y` of the disc
translation after the exact condensation): `m s^2 + c s + k - i k_c = 0`; at `s = i w`: `w^2 = k/m`, `c w = k_c`, so the
threshold is `k_c* = c w_n` exactly, independent of `zeta` and of the spin (no gyroscopic coupling of a central point disc); the
growth at `k_c` is `max Re s` of the two roots.

**K (stiff-shaft limit, `rotordyn`):** `rotordyn::Rotor { mass 1.0, it 4e-3, ip 2e-3, bearings z = -0.04 (k 2e5, c 20, eta 0),
z = +0.06 (k 3e5, c 30, eta 0) }`; the flexible model `flexible_from_rigid` with a massless shaft `r = 0.05 m`, `kappa = 0.9`,
`E = 5e14 Pa` (`nu = 0.3`, density 0): the shaft stiffness over the bearing stiffness about `1e8` (arithmetic 4). Unbalance:
`rotordyn::unbalance_load` of two point unbalances `(mass 1e-3, radius 0.01, angle 0, z -0.02)` and `(1e-3, 0.01, 2.0,
0.03)` at the spins `{0.5, 1.0, 2.0} w_c1`, `w_c1` the first value of `rotordyn::critical_speeds`; the same load through
`nodal_load` at the centre station.

**R (Rayleigh shaft on pins):** `L = 1 m`, `r = 0.05 m`, 20 elements, `kappa = 1e8` (the closed form has no shear: a shear factor
of `1e8` gives `Phi = 12 E I/(kappa G A L_e^2) = 1.3e-8` per element, a frequency effect of order `1e-8`), pinned at both ends.
Closed form (first principles): the Rayleigh beam spinning at `Omega`, `w = u_x + i u_y`: `E I w'''' + rho A w_tt - rho I w_tt''
+ i rho I_p Omega w_t'' = 0` (the distributed moments of D2 with `theta' ~ w''`); with `w = sin(k z) e^{i omega t}`, `k = n pi/L`:
`E I k^4 - rho A omega^2 - rho I k^2 omega^2 + 2 rho I k^2 Omega omega = 0` (`I_p = 2 I`); for `omega > 0` the root is forward,
and the backward orbit `w ~ e^{-i omega t}` gives the minus sign: `omega_{F,B} = (+/- b + sqrt(b^2 + 4 a e))/(2 a)`, `a = rho (A +
I k^2)`, `b = 2 rho I k^2 Omega`, `e = E I k^4`. `w_1R` = the root at `Omega = 0`, `n = 1` (1241.61 rad/s); speeds `Omega in {2,
10, 30} w_1R`. Mode identification: among the six lowest FE whirl modes, the mode number of each is `argmax_{n in 1..4} (|sum_i
X_i s_n(z_i)|^2 + |sum_i Y_i s_n(z_i)|^2)/sum_i s_n(z_i)^2`, `s_n = sin(n pi z/L)`, over the stations; for `n = 1, 2` the
forward and the backward one are the modes of that number with `Forward` and `Backward` direction (exactly one each, else a
defective set-up).

**O (overhung disc on a massless cantilever):** `L = 0.1 m`, `r = 0.01 m`, `kappa = 0.9`, density 0, 4 elements, clamped at `z =
0`, disc at the tip `m = 1 kg`, `I_t = 2.5e-3`, `I_p = 5e-3 kg m^2`. Closed form (the two-unknown model, first principles): the
tip flexibility of the Timoshenko cantilever in one plane, `a11 = L^3/(3 E I) + L/(kappa G A)`, `a12 = L^2/(2 E I)`, `a22 = L/(E
I)` (tip force and tip moment), `[[k11, k12], [k12, k22]]` its inverse; in `w = u_x + i u_y`, `psi = theta_y - i theta_x` and `e^{i
lam t}`: `det [[k11 - m lam^2, k12], [k12, k22 - I_t lam^2 + I_p Omega lam]] = 0`, a quartic with four real roots (undamped,
gyroscopically stable): two `lam > 0` (forward whirl speeds) and two `lam < 0` (backward, speeds `|lam|`). `w_ref` = the lowest
positive root at `Omega = 0` (1690.98 rad/s); speeds `Omega in {0.5, 1, 2} w_ref`. The test finds the quartic's roots by a sign
scan on `[-2 lam_max, 2 lam_max]` (`lam_max` the Cauchy bound) in `1e6` steps and bisection to `1e-14` relative (independent of
the library's solvers). The FE values: the four whirl modes, signed `omega sgn(kappa)`, sorted, against the four roots sorted.

**A (anisotropic bearings):** point disc `m = 1 kg` (`I_t = I_p = 0`) at the middle of a steel massless shaft `L = 0.1 m`, `r =
0.05 m`, `kappa = 0.9`, 2 elements, bearings at the ends each `k = diag(kx/2, ky/2)`, `kx = 2e5`, `ky = 4.5e5 N/m`, undamped:
`sqrt(kx/m) = 447.214`, `sqrt(ky/m) = 670.820 rad/s`. **A0** (control) the same with `ky = kx`.

**U (an unstable band of speeds):** O with a bearing at the disc `k = [[0, k_c], [-k_c, 0]]`, `c I`, `c = 2 (0.02) m w_ref =
67.639 N s/m`, `k_c = 1.5 c w_ref`; Campbell speeds `Omega = 0.25 j w_ref`, `j = 0 .. 8`.

**S (joining):** S1 the rigid rotor of K through `flexible_from_rigid` (density 0) and back through `rigid_from_flexible`. S2 a
Hex8 box `hex_box([-0.05, -0.05, -0.01], [0.1, 0.1, 0.02], [2, 2, 1])`, steel: `m = rho 2e-4`, `I_t = m (0.1^2 + 0.02^2)/12`, `I_p
= m (0.1^2 + 0.1^2)/12` about the centre `(0, 0, 0)`; S2n the box `[0.1, 0.08, 0.02]` (not axisymmetric) and the box shifted by
`(0.01, 0, 0)` (off the axis).

## Arithmetic before this registration (disclosed)

1. **Zero-speed probe** (`f416-scratch/probe`, a scratch crate on the base commit's `rcfd-fem`): the F-391 model of Z-a and Z-b
   by `modal_analysis` (8 modes; tolerance 1e-10 default and 1e-12 Backward) against a dense solve of the same assembled `K`, `M`
   (`quadratic_eigen` with `C = 0` in the scaled time): `max |omega_modal/omega_dense - 1|` = `4.4e-12` (Z-a) and `1.06e-11`
   (Z-b), both tolerances; frequency spread `w_max/w_min` of the dense problem 247 (Z-a) and 596 (Z-b), `tau = 2.0e5 rad/s`. The
   rotor solve of D4 on the same element matrices (assembled from the plane matrices instead of the rotated 12 x 12, so rounding
   differs) is a dense solve of the same kind: estimate `<= 2e-11` against the limit 1e-10.
2. **Prototype** (`f416-scratch/py/estimates.py`, numpy: the matrices of D2 (the F-391 plane formulas re-typed), the condensation
   of D3, the state matrix by `numpy.linalg.eig`, the critical speeds by `numpy.linalg.eigh` of D6, the response of D8, the
   whirl number of D5). R (`r_set.py`): forward and backward of modes 1 and 2 within `8.0e-6` of the closed form at all three
   speeds (mode 2 the larger: the 20-element discretisation), `kappa = +/-1.0000`, the split at `30 w_1R` is `+/- 18` percent (so
   the check sees the gyroscopic term; N1). J (`j_set.py`): `k_s = 1.17175e6`, `k = 7.38867e5 N/m`, `w_n = 607.810 rad/s`, `c =
   121.562 N s/m`; critical speed `sqrt(k/m)` within `3e-15`, the amplitudes within `3.1e-15`. X: growth at `0.98, 1.02, 1.5 c
   w_n` = `-0.606, +0.606, +15.12 1/s`, against the closed form within `1.3e-13`; the unstable mode forward (`kappa = +1`). O
   (`o_set.py`): zero-speed roots `+/- 1690.98, +/- 6173.83 rad/s`; FE against the quartic within `3.5e-15` at all three speeds.
   A (`a_set.py`): with steel `sqrt(kx/m)` and `sqrt(ky/m)` within `6.7e-6`, `1.5e-5` (the shaft in series), straight-line orbits
   (`kappa = 0`). U (`u_set.py`): growth `+9.96, +8.09, +6.00, +3.82, +1.68` at `j = 0 .. 4` and `-0.31, -2.09, -2.36, -1.88 1/s`
   at `j = 5 .. 8`: unstable exactly at `Omega <= 1.0 w_ref` (gyroscopic stiffening of the forward mode raises the threshold).
3. **The stiff-shaft limit** (`k_set.py`): `rotordyn` critical speeds `595.8616, 919.2111 rad/s`; the flexible model's forward
   critical speeds and responses against them, as a function of the fictitious modulus: `E = 2e11`: `2.7e-5` (critical) and
   `3.3e-4` (response); `1.5e14`: `3.4e-8`, `2.8e-7`; `3e14`: `1.2e-8`, `2.4e-7`; `5e14`: `3.3e-9`, `2.2e-8`; `1e15`: `1.4e-8`,
   `2.7e-7`; `1.5e15`: `1.5e-8`, `1.2e-7`. The error is the shaft's compliance (`~ k_b/k_s`, falling as `1/E`) plus the rounding
   of the condensation and of the solve (`~ eps k_s/k_b`, rising as `E`); `E = 5e14` sits at the minimum, but the scatter of the
   rounding between `3e14` and `1e15` (up to `2.7e-7`) is the honest estimate: `<= 3e-7` against the limit 1e-6. The backward
   critical speeds are `425.53, 743.14 rad/s` (not in `rotordyn`).
4. **Natural frequencies** against `rotordyn::natural_frequencies` (K0 = K without bearing damping, spin `w_c1`; with damping
   the bearing stations of the massless shaft are first-order unknowns, refused by D3): the same compliance and rounding budget,
   estimate `<= 3e-7` (Durand-Kerner of `rotordyn` polished by Newton, accurate to rounding for simple roots).
5. **Whirl number of a degenerate cluster:** J has `G = 0` and isotropic bearings: every root is double (forward and backward
   circular orbits, or any two independent ellipses); without the resolution of D5 the labels are arbitrary (the prototype's
   `eigh` gave `kappa = 0` for both critical-speed vectors); with it `kappa = +/- 1` within rounding.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_rotor_default_bits.rs` (B1), `crates/rcfd-post/tests/sa_solid_rotor_default_bits.rs`
(B2), `crates/rcfd-fem/tests/sa_rotor.rs` (Z, J, X, R, O, A, U, L, G, X1) and `crates/rcfd-post/tests/sa_solid_rotor.rs` (K, S),
release profile. Frequency errors are relative unless stated.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| Z1 | c1 | Z-a and Z-b: `whirl(0)` frequencies (the 8 lowest, ascending) against the F-391 `modal_analysis` frequencies (8, ascending): `max |w_rotor/w_F391 - 1|` | `<= 1e-10` | `<= 2e-11` (arithmetic 1) |
| Z2 | D4 | Z-a, Z-b at 0: stable; every root's residual | stable; `<= 1e-12` | `~1e-15` |
| J1 | c2 | J `critical_speeds()`: exactly two values, both within the limit of `sqrt(k/m)` (the closed form above), one `Forward`, one `Backward` (D5 cluster) | `<= 1e-3`; labels | `3e-15` (arithmetic 2) |
| J2 | c3 | J `unbalance_response` at `r = 0.5, 1, 2`: semi-major axis of the disc orbit against `e r^2/sqrt((1 - r^2)^2 + (2 zeta r)^2)` | `<= 1e-3` each | `3e-15` |
| J3 | c10 | J at the three `r`: the disc orbit is `Forward` (sense `>= 1 - 1e-9`) and circular (`minor/major >= 1 - 1e-9`) | each | exact (isotropic) |
| X1 | c9 | X: stable at `k_c = 0.98 c w_n`, unstable at `1.02 c w_n` | each | growth `-/+ 0.606` (arithmetic 2) |
| X2 | c9 | X: the threshold by 40 bisections of `whirl(0.5 w_n).stability` on `k_c in [0.5, 2] c w_n`: `|k_c*/(c w_n) - 1|` | `<= 1e-2` | `~1e-8` (the flag's tolerance) |
| X3 | c11 | X at `k_c = 1.5 c w_n`: `Unstable { growth }` with `|growth - g_cf| <= 1e-8 w_n`; the mode with `growth > 0` is in `modes`, `Forward` | each | `1.3e-13` |
| K1 | c4 | K: the `Forward` critical speeds of the flexible model against `rotordyn::critical_speeds`: same count (2), `max` relative difference | `<= 1e-6` | `<= 3e-7` (arithmetic 3) |
| K2 | c5 | K: `harmonic_response` with `nodal_load` of the unbalance at the centre station against `rotordyn::response` at the three spins: `max_c |X_c - X_rd,c| / max_c |X_rd,c|` over `c = u_x, u_y, theta_x, theta_y` | `<= 1e-6` each | `<= 3e-7` |
| K3 | D4 | K0 (K with `c_j = 0`: the bearing stations of a massless shaft with damping are refused by D3) at spin `w_c1`: the four whirl modes against `rotordyn::natural_frequencies` (`omega <-> |Re lam_rd|`, `Forward <-> Re lam_rd > 0`): `max |omega - |Re lam_rd|| / |Re lam_rd|` and the directions | `<= 1e-6`; each | `<= 3e-7` (arithmetic 4) |
| K4 | D6 | K: two `Backward` critical speeds, record against arithmetic 3 (`425.53`, `743.14`) | within `1e-4` | `~1e-8` |
| R1 | c6 | R at `{2, 10, 30} w_1R`: forward and backward of `n = 1, 2` (identified as in the set-up) against `omega_F`, `omega_B`: 12 values | `<= 5e-3` each | `<= 8e-6` (arithmetic 2) |
| R2 | c10 | R: the identified forward modes have `kappa >= 1 - 1e-6`, the backward `<= -1 + 1e-6` | each | `+/- 1.0000` |
| O1 | c7 | O at `{0.5, 1, 2} w_ref`: exactly four whirl modes; signed `omega sgn(kappa)` sorted against the four quartic roots sorted | `<= 5e-3` each | `3.5e-15` (arithmetic 2) |
| A1 | c8 | A: exactly two critical speeds, against `sqrt(kx/m)` and `sqrt(ky/m)` | `<= 1e-3` each | `1.5e-5` |
| A2 | c10 | A: both `Neutral` (`|kappa| <= 1e-6`: straight-line orbits) | each | 0 |
| L1 | c10 | Every whirl mode of J (spin `w_n`), R (three speeds), O (three speeds), U (nine speeds): its direction equals the sense of the orbit at the station of largest translation `|X|^2 + |Y|^2`, `sgn Im(X conj Y)` (forward `> 0`) | each | D5 |
| U1 | c11 | U `campbell`: `unstable()` lists exactly the speeds `j = 0 .. 4` (bits equal to the input speeds), each growth equal by bits to `whirl(speed).stability`'s; `speeds[j].stability` Unstable there and Stable at `j = 5 .. 8`; `to_csv` rows carry `unstable` at `j = 0 .. 4` | each | arithmetic 2 |
| U2 | c11 | U: at every unstable speed `whirl(speed).modes` contains a mode with `growth > 0` (not dropped), `Forward` | each | arithmetic 2 |
| T1 | D9 | O `campbell` over `Omega = 0.2 j w_ref`, `j = 0 .. 10`: four tracks; every stored MAC (`j >= 1`) `>= 0.9`; each track keeps one direction at every speed `> 0`; the frequencies of each track at each speed `> 0` equal O1's quartic root of the same sign and rank within `5e-3` | each | MAC ~1 |
| G1 | D2 | `G` skew: `max |G_ij + G_ji|` over Z-a with a disc of `I_p = 0.01` at the middle station (`gyroscopic_matrix()` accessor, dense) | `== 0` | 0 (D2) |
| S1 | D10 | K's rotor through `flexible_from_rigid` and `rigid_from_flexible`: mass, `it`, `ip`, the bearings' `z`, `k`, `c` | `<= 1e-12` relative each | rounding |
| S2 | D10 | `disc_from_solid` of the box S2: `mass`, `I_t`, `I_p` against the closed forms; the centre | `<= 1e-12` relative; `<= 1e-15` m | consistent mass exact for rigid motions (ADR 0284) |
| S3 | D10 | S2n: the non-square box refused (`"inertia"`), the shifted box (`"centre"`); K with `eta = 0.01` refused by `flexible_from_rigid` (`"eta"`); `rigid_from_flexible` of A (anisotropic) refused (`"bearing"`) | each | D10 |
| X0 | D1, D3, D6, D9 | refusals: a section with `I_y != I_z` (`"section"`); `critical_speeds` of X (`"cross_coupling"`); `critical_speeds` of a free rotor (no bearing, no pin) (`"stiffness"`); `whirl` of J with the damper moved to a bearing station (a damped massless unknown, `"mass"`); `whirl(-1)` and `whirl(NaN)` (`"speed"`); `campbell` with a decreasing list (`"speeds"`); `add_disc` at station 99 of a 5-station model (`"node"`); stations not increasing (`"nodes"`); a disc with negative mass (`"disc"`); a bearing with a NaN (`"bearing"`); `harmonic_response` with `omega = 0` (`"frequency"`) | each refused with its key | D1 |
| B1 | rule 3 | digests printed at main 57808e24 by `sa_rotor_default_bits.rs` (written with the API of that commit, committed before any library line): Z-a and Z-b `modal_analysis` (eigenvalues, the first mode), `beam::local_stiffness` and `beam::local_mass` of one element (all 144 values each); all equal after the change; `cargo test -p rcfd-fem --release` passes; `cargo test -p rcfd-api --release --test baseline_hashes` passes | `==`; pass | holds (only visibility changes in `beam.rs`) |
| B2 | rule 3 | digests at 57808e24 by `sa_solid_rotor_default_bits.rs`: K's `rotordyn::critical_speeds`, `natural_frequencies(w_c1)`, `response` at the three spins (every phasor); equal after; `w46c_rotordyn` passes | `==`; pass | holds (`rotordyn.rs` not edited) |
| E1 | scaffold | `sa_scaffold.rs`: the copy `x1_modules_and_pending_after_f415_f420` becomes an ignored record (ADR 0291 pattern, the attribute only) and a new copy `x1_modules_and_pending_after_f416` with `rotor` landed passes; `rotor::PENDING == None`, `solid_rotor::PENDING == None` | pass | holds |

Records (reported, not in the decision rule): Z1 also against the dense solve of the F-391 matrices of arithmetic 1; every root
residual; the growth of X at the bisected threshold; K's backward critical speeds; the U table as CSV; the condensed size of each
set-up.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | R: `whirl(0)` frequency of `n = 1` against `omega_F` at `30 w_1R` | `> 0.1` (expected 0.18: the check sees the gyroscopic split) |
| N2 | K with steel (`E = 2e11`): K1 measure | `> 1e-6` (expected `2.7e-5`: the limit is about the stiff limit) |
| N3 | A0 (`ky = kx`): two critical speeds equal within `1e-12`, one `Forward`, one `Backward` | each |
| N4 | X at `k_c = 0`: stable, both modes with damping ratio `zeta` within `1e-10` | each |

Mutations (on a scratch copy of the worktree; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the sign of the whole `G` reversed | R1, R2 (forward and backward swap), O1, K1, K3, T1 |
| MU2 | the shaft's polar inertia `rho I` instead of `rho (I_y + I_z)` | R1 |
| MU3 | the disc's `I_p` dropped from `G` | O1, K1, K3 |
| MU4 | the cross-coupled bearing transposed (`k` used as `k^T`) | X3 (the unstable mode backward), U1 |
| MU5 | the condensation keeps `K_aa` only (the coupling term dropped) | J1, O1, K1, A1 (J2 and K2 do not condense) |
| MU6 | unstable modes dropped from `modes` (growth `> 0` filtered) | X3, U2 |
| MU7 | the whirl number's sign reversed (`S` transposed) | R2, L1, K1 (the forward list), X3 |
| MU8 | the time scale not undone (`lambda = mu`) | Z1, R1, O1, K3 |
| MU9 | the unbalance force rotating backward (`F_y = +i F_x`) | K2, J3 |
| MU10 | the cluster resolution of D5 skipped | J1 (labels), N3 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an amendment
made after its run. **todo**: the module does not build or the Z, J, R, O and K runs cannot be made.

## Limits (not covered)

- Axisymmetric shafts and discs only (a non-axisymmetric shaft has periodic matrices in the stationary frame); rigid discs (blade
  flexibility is not coupled; `disc_from_solid` lumps a solid part into a rigid disc); bearings are linear, speed-independent
  `k`, `c` matrices given as inputs (no fluid-film or ball-bearing model: no data in the repository, ADR 0310); no hysteretic
  bearing damping (`rotordyn`'s `eta` is refused by the join), no internal (rotating) damping, no foundation or housing
  flexibility, no axial or torsional motion, constant speed.
- Critical speeds are undamped and refused with cross-coupled stiffness (D6); a damped "critical speed" is the peak of the
  unbalance response or the crossing of a Campbell track with the engine order (`crossings`), which is reported but not checked.
- Dense algebra: the model is meant for shafts of up to a few hundred unknowns (the solver is `O(n^3)` on `2 n`).
- Massless unknowns with damping are refused (D3), not solved as first-order unknowns.
- No case-file key, no CLI or post-processing output beyond the CSV of the Campbell table.

## Amendment 1 (2026-11-14, written before any run of any check of F-416)

While writing the test of O1, before it or any other check was run: the scan range of the quartic's roots in set-up O,
`[-2 lam_max, 2 lam_max]` with `lam_max` the **Cauchy** bound `1 + max_i |a_i/a_4|`, is defective. The constant term over the
leading one is `(k11 k22 - k12^2)/(m I_t)`, of order `4e13` for O, so the scan step would be about `8e7 rad/s` and would step over
every root (they lie between `7e2` and `1e4 rad/s`). The corrected set-up, fixed now: `lam_max` is **Fujiwara's** bound `2
max(|a_3/a_4|, |a_2/a_4|^(1/2), |a_1/a_4|^(1/3), |a_0/(2 a_4)|^(1/4))` (every root has `|lam| <= lam_max`), of order `1e4 rad/s`
for O, so the step of the `1e6`-step scan on `[-2 lam_max, 2 lam_max]` is about `0.05 rad/s`, far below the root spacing (over
`300 rad/s`, arithmetic 2). Everything else of O1 and T1 (the limits, the speeds, the bisection to `1e-14`) is unchanged.

## Amendment 2 (2026-11-14, written after the registered run of `sa_rotor.rs` and `sa_solid_rotor.rs` and before any run of the copy below)

The registered run (`f416-scratch/sa_rotor_run1.log`, `sa_solid_rotor_run1.log`, commit 71091f70) met every check except **L1
on R**: of the 80 whirl modes of R at each speed, 3 (at `2 w_1R`), 2 (at `10 w_1R`) and 3 (at `30 w_1R`) have a direction
different from the sense of the orbit at the station of largest translation: `omega = 2.938e5` (Backward, sense forward),
`2.973e5` (Forward, sense backward), `7.449e5` (Backward, sense forward) rad/s at `2 w_1R`, and the like at the other speeds
(`2.868e5`, `3.046e5`; `2.701e5`, `3.234e5`, `7.140e5`). J, O and U (17 runs) met L1. The limit (equality of the labels) stays.
A diagnostic on a scratch copy (`f416-scratch/diag_l1`, not a registered check) printed for every mode of R at `2 w_1R` the
translational over rotational kinetic energy (diagonal of `M`) and the senses of the translation and tilt orbits at every
station: the mismatched modes are **translation-free** modes of the 20-element pinned shaft (energy ratio `3.7e-28` and
`4.4e-28`: `u_x, u_y` are rounding, about `1e-14` of the rotations), in which the orbit at "the station of largest translation"
is the orbit of rounding noise; their tilt orbits `(theta_y, -theta_x)` turn in the sense of the label at every station. So the
label is right and the reference of L1 is undefined for a mode without translation: a defect of the check, found by its run. The
miss is kept, per rule 2: the test as run is renamed `l1_direction_is_the_sense_of_the_orbit_known_miss` and ignored with the
measured numbers in the attribute. It continues in a copy `l1b_direction_is_the_sense_of_the_orbit_after_amendment_2`, the same
runs and the same equality, with the reference defined for every mode: the sense of the translation orbit at the station of
largest `|X|^2 + |Y|^2` when the mode's translational kinetic energy (the `u_x, u_y` rows and columns of `M`) is at least `1e-6`
of `x^H M x`, otherwise the sense of the tilt orbit `(theta_y, -theta_x)` at the station of largest `|theta_x|^2 + |theta_y|^2`
(`sgn Im(theta_y conj(-theta_x))`). By the decision rule the item is at best **partial** (a check met only after an amendment
made after its run).

