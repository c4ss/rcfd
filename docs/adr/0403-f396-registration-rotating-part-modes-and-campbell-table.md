# ADR 0403: F-396 registration: modes of a rotating part (stress stiffening by the centrifugal pre-stress, spin softening), mode tracking and the Campbell table

Status: **REGISTERED 2026-11-13, written before any code of F-396 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-396 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done with one scratch script (`f396-scratch/eb_rotating.py`, numpy only, a Hermite-cubic
Euler-Bernoulli model of the rotating cantilever with 200 elements and the closed forms of the bar; it does not call rcfd;
disclosed under "Arithmetic"). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: the stub [ADR 0299](0299-rotating-part-spin-softening-and-campbell-diagram.md) (SI-9 of the intermediate-tier plan,
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md) section 7). Building blocks: geometric stiffness and pre-stressed
modes ([ADR 0390](0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md), on the transformation by [ADR
0398](0398-f493-registration-prestressed-modes-and-buckling-on-the-transformation.md)); the centrifugal body load (`Load::Centrifugal`,
ADR 0276); the modal analysis (ADR 0284, on the transformation ADR 0384); the Sturm count of `rcfd_linalg::eigen::count_below`
(ADR 0283 D8). Roadmap item F-396 (deps F-395, F-375, F-383). Number 0403 given by the lead. Branch `f396-rotating-part-modes`
from main bf03ed6d, worktree `w49i-work/f396`, scratch `w49i-work/f396-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room. Library edits:
`crates/rcfd-fem/src/rotating.rs` only (today a two-line stub declared by ADR 0291). `geometric.rs`, `modal.rs`, `mpc.rs`,
`constraint.rs`, `assemble.rs`, `mass.rs` and the element files are used read only, through their public and crate-public items
(two other packages work in parallel on F-397 and F-496). Tests: a new file `crates/rcfd-fem/tests/si_rotating.rs`. No existing
test asserts a refusal of rotating modes (`si_scaffold.rs` only names the module), so no existing test is edited.

## What F-396 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. At zero speed the result has the bits of the modal
> analysis of F-383. Radial bar clamped on the axis, nu = 0, axial modes: (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3 for
> n = 1, 2 at a speed with centrifugal stress below 1e-4 E. Radial beam of square section clamped on the axis (slenderness 20):
> the first out-of-plane (flap) and in-plane (lag) frequencies satisfy omega_flap^2 - omega_lag^2 = Omega^2 within 1e-4 Omega^2
> on a symmetric mesh; the stiffening (omega_flap^2 - omega_0^2)/Omega^2 equals at Omega = 0.2 omega_0 the Rayleigh quotient of
> the centrifugal tension with the mode at rest (1.19, recomputed in the registration) within 2 percent and stays between 1 and
> that value up to Omega = 5 omega_0. Campbell table: on a synthetic family omega^2 = omega_0^2 + S Omega^2 the crossings with
> orders 1 to 4 equal omega_0/sqrt(k^2 - S) within 1e-8, and on the beam within 1 percent; a flap and a lag mode of a
> rectangular section that cross are tracked without exchange (modal assurance above 0.9). A speed at which K_eff is not
> positive definite is reported as a loss of stiffness with the speed, never as a non-finite frequency.

Clauses: (a1) zero speed: the bits of `modal::modal_analysis` (F-383 is the modal analysis of ADR 0284); (a2) the radial bar;
(a3) the flap-lag identity; (a4) the stiffening at `0.2 omega_0` against the recomputed Rayleigh quotient; (a5) the stiffening
between 1 and that value up to `5 omega_0`; (a6) synthetic crossings within 1e-8; (a7) the beam's crossings within 1 percent;
(a8) tracking through a flap-lag crossing, modal assurance above 0.9; (a9) loss of stiffness reported with the speed, never a
non-finite frequency.

Two clauses cannot hold as literally written for the operator the plan names (`K_eff = K + Omega^2 (K_sigma,1 - M_perp)` on a
3-D solid). This is found by first-principles arithmetic before any code (below, arithmetic 2 and 3) and is registered here, not
discovered after a run.

**Reading of (a2) (an interpretation, taken on the maintainer's standing instruction of 2026-11-11, take the recommendation).**
The reference the stub names for the bar is `rho u_tt = E u_rr + rho Omega^2 u`, the rotating-frame equation **without** stress
stiffening. With the geometric stiffness of the centrifugal stress the axial motion also feels `sigma_rr u_r' v_r'` (the full
second variation, ADR 0390 D1, has `delta_ij`): its Rayleigh quotient per `Omega^2` is `Q_n = k_n^2 L^2/3 + 1/4` (arithmetic 2),
1.072 for n = 1 and 7.652 for n = 2, **independent of the speed**, because the stress is proportional to `Omega^2` (the condition
"centrifugal stress below 1e-4 E" makes `sigma/E` small but not `sigma k^2/(rho Omega^2)`, which is `O(1)`). So the full operator
gives `(omega_n,0^2 - omega_n^2)/Omega^2 = 1 - Q_n = -0.072` and `-6.65`, not 1. The clause is read as the check of the spin
softening term against its closed form: the operator with the stress stiffening switched off (`RotatingOptions::stress_stiffening
= false`, an option of this package) must give 1 within 1e-3 (A2a). The full operator is checked against its own first-order
form `1 - Q_n,h` (A2b, `Q_n,h = phi_n,0^T K_sigma,1 phi_n,0`, the discrete quotient) and `Q_n,h` against the closed form `Q_n`
(A2c). Open question 1 (the wording of the clause).

**(a3) is registered as written and predicted to miss.** For a 3-D solid the bending mode also moves along the beam (the section
rotation `u_x = -y v'` or `-z w'`), and that motion is perpendicular to the axis in both bending planes, so both modes are
softened by `f_x Omega^2`, `f_x` the axial share of the kinetic energy; and the lateral centrifugal stress `sigma_yy = rho Omega^2
(h^2/4 - y^2)/2` stiffens the lag mode (whose section rotation varies in `y`) and not the flap mode. Arithmetic 3: both terms
are `f_x = r_g^2 int v'^2 / int v^2 = 9.68e-4` at `L/h = 20`, so `(omega_flap^2 - omega_lag^2)/Omega^2 = 1 - 2 f_x = 0.99806`, a
deviation `1.9e-3` against the limit `1e-4`. The identity of the stub ("assumes the two bending planes are discretised
identically") is the Euler-Bernoulli one, where `f_x = 0`. A3 keeps the limit 1e-4; the companion A3b checks the diagnosis.

## What was read (branch base bf03ed6d, read only)

- `AGENT_RULES.md`; the brief; ADR 0299 (all), ADR 0290 section 2 and the SI-9 entry; ADR 0400 (head, for the form).
- `crates/rcfd-fem/src/geometric.rs` (all: `Prestress::{from_static, scaled, check_fits}`, `geometric_stiffness`,
  `add_on_pattern` (crate-public), `prestressed_modes` and its transformation branch), `modal.rs` (all: `ModalOptions`,
  `ModalResult::from_parts` (crate-public), `modal_analysis`, the shift rule), `mass.rs` (all), `loads.rs` (lines 1 to 200: the
  centrifugal body load `b = rho omega^2 r_perp`), `assemble.rs` (lines 1 to 80: `assemble_with`, `ElementContext`, the
  pattern), `mpc.rs` (signatures: `uses_transformation`, `transformation`, `Transformation::{reduce, expand, n_mesh,
  n_unknowns, independent}`, `add_supports`, `refuse_nonzero_values`, `unknown_name`, `zero_shift_refused`,
  `modal_analysis_mpc`), `model.rs` (`Load`, `ElasticSupport`, `SolidModel` accessors and `add_*`), `solve.rs` (`StaticOptions`,
  the dispatch of `solve_static_report`), `builders.rs` (`hex_box`), `rcfd-linalg/src/eigen.rs` (`Shift`, `SubspaceOptions`,
  `count_below`, `factor_shifted`, `lowest_eigenpairs` head), `skyline.rs` (`inertia`), the tests `si_geometric.rs` (lines 1 to
  260, the set-up and check helpers), `si_scaffold.rs` (lines 80 to 130), `sb_loads.rs` (the names of the rotating-bar tests).

## Design

Notation: the model rotates at the constant speed `Omega` about the unit axis `a` through the point `p`; displacements `u` are
measured in the frame that rotates with it; `P = I - a a^T` projects on the plane perpendicular to the axis.

### D1. The operator (first principles)

The position of a material point is `x = X + u` in the rotating frame; its velocity in the inertial frame is `u_t + Omega a x (x -
p)`. The kinetic energy is `T = 1/2 int rho |u_t + Omega a x (X - p + u)|^2 dV`. Expanded in `u`:

- `1/2 int rho |u_t|^2`: the mass `M`;
- `int rho u_t . (Omega a x u)`: the gyroscopic (Coriolis) term `G`, skew; **neglected** (D6);
- `int rho Omega^2 (a x (X - p)) . (a x u)`: linear in `u`, the work of the centrifugal body force `rho Omega^2 P (X - p)`, which
  is `Load::Centrifugal` (ADR 0276) and produces the static pre-stress;
- `1/2 int rho Omega^2 |a x u|^2 = 1/2 Omega^2 int rho u^T P u`: since `|a x u|^2 = |u|^2 - (a . u)^2`. It enters the Lagrangian
  `T - V` with the sign of a negative stiffness: **spin softening** `-Omega^2 M_perp`,

  `M_perp[(a, i), (b, j)] = int rho N_a N_b (delta_ij - a_i a_j) dV = m_ab P_ij`,

  `m_ab` the scalar consistent mass of ADR 0284 with the same rule (the element mass rule), so `M_perp = M` on the components
  perpendicular to the axis and 0 along it.

The potential energy about the pre-stressed state at speed `Omega` is `1/2 u^T (K + K_sigma(Omega)) u` (ADR 0390 D1, the second
variation of the Green-Lagrange strain energy at the fixed stress). The static centrifugal state is linear in `Omega^2`, so
`K_sigma(Omega) = Omega^2 K_sigma,1` with `K_sigma,1` the geometric stiffness of the stress of **one** static solve at `Omega = 1`
rad/s (ADR 0390's `Prestress::from_static` on a copy of the model whose only load is `Load::centrifugal(a, p, 1.0)`, with the
model's constraints and supports). Without the Coriolis term the free vibration `u = phi e^{i omega t}` satisfies

`K_eff(Omega) phi = omega^2 M phi`, `K_eff(Omega) = K + Omega^2 (s K_sigma,1 - c M_perp)`,

`s = 1` and `c = 1` by default (`RotatingOptions::stress_stiffening`, `spin_softening`; switching one off is for the checks and
for a user who wants the classical separate effects). The model's own loads are not part of the pre-stress (the analysis is of
the spinning part alone; a user pre-stress at unit speed can be passed instead, D5).

`K`, `K_sigma,1`, `M`, `M_perp` are assembled once on the stiffness pattern (`assemble_with`; `M_perp` by its own element
function from the element mass rule) and `K_eff` at each speed is formed value by value on the pattern, `k + Omega^2 (s g - c
m_perp)`, in that order of operations.

**Zero speed (a1).** At `Omega = 0` the analysis calls `modal::modal_analysis(model, &options.modal)` itself and returns its result,
so the bits are those of ADR 0284 (or ADR 0384 on the transformation) by construction, not by rounding luck.

### D2. Reductions and the eigensolution

As `geometric::prestressed_modes` (ADR 0390 D4, ADR 0398 D2), with `K_eff` in place of `K + K_sigma`:

- a model without linear constraints, rigid sets or supports: the symmetric elimination of its zero prescribed displacements
  (`constraint::reduce`), the modes expanded with zeros at prescribed equations;
- otherwise the transformation `x = T q` (`mpc::transformation`): `T^T (K_ext + Omega^2 (s K_sigma,1 - c M_perp)) T` with the
  supports added to `K` before the sum, `T^T M T`; zero constraint values only (refused naming the row, as ADR 0398).

The modes requested, the density check and the options are those of `ModalOptions` (residual test, tolerance, cap, subspace
size, threads). The default shift at `Omega > 0` is `Shift::Value(0.0)`, used only after D3 has established that the reduced
`K_eff` is positive definite.

### D3. Loss of stiffness (a9)

Before the eigensolution at `Omega > 0` the reduced pair is tested by Sylvester's law of inertia: `count_below(K_eff,r, M_r, 0,
Rcm)` (ADR 0283 D8: the negative pivots of `K_eff,r = L D L^T`). If the count is positive, or the factorisation refuses a zero
pivot (a speed exactly at the loss), the result at that speed is `SpeedModes::LossOfStiffness { speed, negative }` (`negative`
the count, `None` for the singular case) and no eigenvalue is computed: no frequency exists to be non-finite. A free body at `Omega
> 0` is reported in the same way (its in-plane translations have `-Omega^2` stiffness in this frame).

### D4. Speeds, tracking and the Campbell table

`RotatingPart::campbell(speeds)` takes a list of speeds, rad/s, finite, `>= 0` and strictly increasing (anything else refused,
key `"speeds"`, naming the index), solves each speed (D1 to D3) with `options.modal.modes` modes, and tracks the modes by shape:

- **modal assurance** (Allemang and Brown 1982, from memory; the mass-weighted form): `MAC(phi, psi) = (phi^T M psi)^2 /
  ((phi^T M phi)(psi^T M psi))`, with the mass of the model's equations (the mass does not depend on the speed, so the measure
  is the same at every speed);
- the first speed's modes start the tracks in frequency order; at each later speed with modes, the pairs (track, new mode) are
  assigned greedily by decreasing MAC against the track's last mode (ties by the lower track, then the lower mode), each track
  and each mode used once; the MAC of each assignment is stored. After a speed with a loss of stiffness, the tracks continue from
  their last modes;
- the table holds, per track and speed, the eigenvalue and frequency (`None` at a loss of stiffness) and the MAC with the
  previous assigned mode.

**Engine-order crossings** (a6, a7): the line of order `k` is `omega = k Omega`. Between two consecutive speeds where a track has
eigenvalues, `lambda` is interpolated **linearly in `Omega^2`** (`s = Omega^2`), `g(s) = lambda(s) - k^2 s`; a sign change of `g`
(or a zero at a node) gives `s* = s_a - g_a (s_b - s_a)/(g_b - g_a)`, `Omega* = sqrt(s*)`, `f* = k Omega*/(2 pi)`. For a family
`lambda = omega_0^2 + S Omega^2` this is exact: `g` is linear in `s` and the root is `s* = omega_0^2/(k^2 - S)`, so the
crossing is `omega_0/sqrt(k^2 - S)` to rounding. The same function is public on plain data
(`engine_order_crossings(speeds, eigenvalues, order)`), which is how a6 is checked.

**Output as data:** `CampbellTable::to_csv()` (one row per speed: speed in rad/s, speed in rev/min, then one frequency column per
track in Hz, empty at a loss of stiffness, then a `loss` column with the negative count) and `to_json()` (speeds, tracks with
frequencies and MAC, `null` at a loss, losses, crossings for the orders asked). Numbers are written with Rust's shortest
round-trip formatting, so the text parses back to the same bits. No serialisation crate is added.

### D5. Interfaces

`rotating.rs`:

- `spin_softening_mass(model, axis, threads) -> CsrMatrix` (`M_perp`, on the stiffness pattern; axis non-zero and finite);
- `RotatingOptions { modal: ModalOptions, stress_stiffening: bool, spin_softening: bool, static_options: StaticOptions }`,
  `RotatingOptions::new(modes)`;
- `RotatingPart::new(model, axis, origin, &options)` (one static solve at unit speed, the four matrices) and
  `RotatingPart::with_unit_prestress(model, axis, origin, prestress, &options)` (a pre-stress the caller computed at 1 rad/s, for
  example on a sector);
- `RotatingPart::{unit_prestress, operator(omega), modes(omega) -> SpeedModes, slope(mode), campbell(speeds) -> CampbellTable}`;
  `slope(phi) = phi^T (s K_sigma,1 - c M_perp) phi / phi^T M phi`, the first-order derivative `d omega^2 / d Omega^2` at a mode
  (the Rayleigh quotient of the added stiffness), with `stiffening_quotient(phi) = phi^T K_sigma,1 phi / phi^T M phi`;
- `SpeedModes::{Modes(ModalResult), LossOfStiffness { speed, negative }}`; `CampbellTable` with `speeds`, `tracks`, `losses`,
  `crossings(order)`, `to_csv`, `to_json`; `mac(m, phi, psi)`; `engine_order_crossings`.

The axis is refused if zero or non-finite (`InvalidValue`, key `"axis"`), a non-finite origin likewise (`"origin"`), a negative or
non-finite speed (`"speed"`).

### D6. What is neglected, said plainly (decision N5 of the stub)

The Coriolis matrix `G` (`2 Omega int rho N_a N_b [a x]_ij`, skew) is not included: the eigenproblem stays symmetric and real.
Modes that move in the plane of rotation (the lag mode, any in-plane disc mode) couple through `G` with the radial motion; for a
blade on a stiff hub the effect on the frequencies is small, for a disc or a soft hub it is not, and this package does not measure
it. The pre-stress is linear in `Omega^2` (no large untwist or stiffening of the static state; that is F-397's subject). The
modes are those of the whole model or of a sector at harmonic index 0 through its constraints; modes of a cyclic sector at
another harmonic index under rotation are not here (they need the complex harmonic pair of ADR 0386/0395 with `K_eff`; a
follow-up is proposed in the addendum if the lead wants one).

## Set-ups (fixed now)

Steel-like: `E = 2.0e11` Pa, `nu = 0`, `rho = 7850` kg/m^3 for all set-ups. The axis is `z` through the origin for all.

- **B, the radial bar.** `hex_box` from `(0, -0.025, -0.025)`, lengths `(1, 0.05, 0.05)`, divisions `[40, 1, 1]`, Hex8. `xmin`
  clamped (components 0, 1, 2), every node held in `y` and `z` (components 1, 2 of the set `all`), so the modes are the axial
  ones (with `nu = 0` the lateral holds do not change them). Two modes. Speed `Omega_B = sqrt(1e-4 E/(rho L^2)) = 50.48` rad/s,
  where the largest centrifugal stress `rho Omega^2 L^2/2 = 5e-5 E` (below `1e-4 E`).
- **F, the radial beam of square section.** `hex_box` from `(0, -h/2, -h/2)`, lengths `(L, h, h)`, `L = 1`, `h = 0.05`
  (slenderness `L/h = 20`, the length over the side, as the beam of ADR 0390), divisions `[20, 2, 2]`, upgraded to Hex20; mirror
  symmetric in `y` and in `z` and identical in the two bending planes. `xmin` clamped. Four modes, `ResidualTest::Backward`,
  tolerance `1e-12`. `omega_0` is the lowest frequency at rest (flap and lag are degenerate at rest). Flap = the mode with the
  larger kinetic share in `z` (along the axis), lag = in `y`.
- **R, the rectangular beam.** As F with the section `h_y = 0.06` (in the plane of rotation), `h_z = 0.05` (along the axis): the
  lag frequency at rest is about `1.2` times the flap frequency, and with the lag slope about one less than the flap slope they
  cross near `Omega = sqrt(1.44 - 1) omega_0f = 0.66 omega_0f` (arithmetic 5). Four modes, Backward, `1e-12`. `omega_0f` the
  lowest frequency at rest (flap).
- **K, the block on a foundation.** `hex_box` from `(0.1, -0.05, -0.05)`, lengths `(0.1, 0.1, 0.1)`, divisions `[4, 2, 2]`,
  Hex8; every node held in `y` and `z`; `ElasticSupport::foundation("xmin", 2.0e9)` (normal stiffness per area): a nearly rigid
  radial translation on a spring of `2e9 x 0.01 = 2e7` N/m, mass 7.85 kg, `omega_r0 = 1.6e3` rad/s, far below the block's own
  axial modes (`> 4e4` rad/s). Transformation path (the support). Two modes.
- **Synthetic families** for a6: `(omega_0, S) = (100, 0.19)`, `(100, 0.5)`, `(250, -0.3)` rad/s, speeds `0, 10, ..., 400` rad/s
  (41), `lambda_j = omega_0^2 + S Omega_j^2` computed in f64.

Speed lists: F and R, `Omega/omega_0 = 0, 0.1, ..., 1.2` (13 speeds); the stiffening of F at `Omega/omega_0 = 0.2, 0.5, 1, 2, 3,
5`; K at `0.5, 0.9, 1.1, 1.5 omega_r0`.

## Arithmetic (done before the code; disclosed)

1. `Omega_B^2 = 1e-4 x 2e11/(7850 x 1) = 2547.8`, `Omega_B = 50.476`; `sigma_max = 7850 x 2547.8/2 = 1.0e7 = 5e-5 E`.
   `omega_1,0^2 = (pi/2)^2 E/(rho L^2) = 6.29e7`, so `Omega_B^2/omega_1,0^2 = 4.1e-5` (second-order terms of A2b are of that
   relative size).
2. **The bar's stiffening quotient.** `sigma(x) = rho Omega^2 (L^2 - x^2)/2`, mode `sin(k_n x)`, `k_n = (2n - 1) pi/(2L)`:
   `Q_n = int (L^2 - x^2)/2 k^2 cos^2 / int sin^2 = k_n^2 L^2/3 + 1/4` (`int_0^L x^2 cos(2kx) dx = -L/(2k^2)` since `cos(2 k_n
   L) = -1`); `Q_1 = 1.0724670`, `Q_2 = 7.6522033` (closed form and quadrature agree to 8 digits, scratch script). Discrete
   prediction for A2a: on the tensor-product Hex8 mesh with `nu = 0` and the lateral holds, a field `u_x = f(x)` is invariant
   (`K = K_1D (x) M_yz`, `M = M_1D (x) M_yz`) and `M_perp = M` on it, so `K - Omega^2 M_perp` has the eigenvalues `lambda_n,0 -
   Omega^2` exactly: the ratio is 1 to rounding, `~ 1e-16 lambda_0/Omega^2 = 2.5e-12` to `1e-10` (the eigensolver's own accuracy
   at the default tolerance). A2c: linear elements, 40 along, the error of `Q_n,h` of order `(k_n h)^2/12`: `1.3e-4` (n = 1),
   `1.2e-3` (n = 2).
3. **The flap-lag identity in 3-D.** Euler-Bernoulli model (scratch script): `(S_flap - S_lag)` at `Omega/omega_0 = 0.2, 0.5, 1,
   2, 3, 5` is `1 + 3e-5, 1 - 2e-6, 1 + 7e-7, ...`: the identity holds to second order there. In 3-D (D1): flap `u_z = w`, `u_x =
   -z w'`; lag `u_y = v`, `u_x = -y v'`. Spin softening acts on `x` and `y`: on the flap's `u_x` (share `f_x`), on the lag's `u_x`
   and `u_y` (share `1 - f_z,lag`, `f_z,lag = 0` at `nu = 0` to this order). The geometric stiffness `sum_i grad u_i . S grad u_i`
   with `S = diag(sigma_xx, sigma_yy, 0)` gives the same `sigma_xx` terms to both (mirror symmetry) and `int sigma_yy v'^2` to the
   lag only; with `sigma_yy = rho Omega^2 (h^2/4 - y^2)/2` (lateral equilibrium of the in-plane body force `rho Omega^2 y`, free
   faces, `sigma_xy` neglected) this term per `Omega^2` is `(h^2/12) int v'^2/int v^2 = f_x`. Hence to first order `(omega_flap^2
   - omega_lag^2)/Omega^2 = 1 - 2 f_x`, `f_x = r_g^2 int w'^2/int w^2 = (h^2/12)(4.6478/L^2) = 9.683e-4` (the integral from the
   Hermite model), **predicted `0.99806`, deviation `1.94e-3`**; A3's limit 1e-4 is predicted to be missed by a factor 19.
4. **The stiffening.** Euler-Bernoulli: the Rayleigh quotient of the tension `T = rho A Omega^2 (L^2 - x^2)/2` with the first
   cantilever mode at rest, per `Omega^2`, `Q = int (L^2 - x^2)/2 w'^2 / int w^2 = 1.193336` (the stub's 1.19; the
   Southwell coefficient of the first flap mode). `S(Omega) = (omega_flap^2 - omega_0^2)/Omega^2`: `1.19278` at 0.2, `1.18977`
   at 0.5, `1.18022` at 1, `1.15325` at 2, `1.12766` at 3, `1.09226` at 5 (decreasing towards the string value 1, Southwell's
   bound `omega^2 >= omega_0^2 + Omega^2`). In 3-D the flap slope is lowered by `f_x = 9.7e-4` (D1, the softened section
   rotation): predicted `S(0.2) = 1.1918`, 0.13 percent below 1.193336, and `S(5) = 1.091`. Rigorous upper bound: by the min-max
   principle on the two-dimensional trial space of the rest flap and lag modes (decoupled by the mirror symmetry in `z`, under
   which the operator is invariant), `omega_flap^2(Omega) <= R_flap(Omega) = omega_0^2 + Omega^2 (Q_h - f_x,h)`, so `S(Omega) <=
   Q_h = phi_f,0^T K_sigma,1 phi_f,0` (the discrete quotient of the rest flap mode, `phi^T M phi = 1`). The acceptance's "that
   value" in (a5) is read as this quotient of the model (the "Rayleigh quotient of the centrifugal tension with the mode at
   rest"), and its continuum value 1.193336 is the reference of (a4).
5. **Crossings (Euler-Bernoulli, `Omega/omega_0`).** Flap: none with order 1 (Southwell), 0.59637 (k = 2), 0.35786 (3), 0.25987
   (4); with the rest slope, `1/sqrt(k^2 - Q)`: 0.59690, 0.35790, 0.25988. Lag: 1.10279 (1), 0.51229 (2), 0.33694 (3), 0.25152
   (4). All inside `0 .. 1.2`. Over a step of 0.1 in `Omega/omega_0` the slope `S` changes by at most `1.5e-3` (arithmetic 4), so
   the linear interpolation in `Omega^2` misplaces a crossing by about `0.5 dS/(k^2 - S) < 1e-3` relative: the 1 percent of (a7)
   is predicted to be met by a factor 10 or more. Rectangular `h_y/h_z = 1.2`: the flap-lag crossing at `Omega/omega_0f = 0.6639`
   (EB), between the table speeds 0.6 and 0.7, where the two eigenvalues differ by about 5 and 3 percent.
6. **K.** Rigid radial mode `lambda_0 = 2e7/7.85 = 2.55e6` (`omega_r0 = 1596`); `lambda(Omega) ~ lambda_0 - Omega^2 (1 - q)`, `q`
   the stiffening share of the nearly rigid mode (its strain energy share is about `k_f A a/(E A) = 1e-3`): at 0.9 `omega_r0`,
   `lambda ~ 0.19 lambda_0 > 0`; at 1.1, `-0.21 lambda_0 < 0` (one negative eigenvalue: the next mode is the block's axial mode,
   far above); at 1.5, `-1.25 lambda_0`. B without stiffening at `1.1 omega_1,0`: `lambda_1 = -0.21 lambda_1,0`, `lambda_2 = (9
   - 1.21) lambda_1,0 > 0`: one negative eigenvalue.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| A1 | a1 | `RotatingPart::modes(0.0)` on F and on K (transformation path) against `modal_analysis` of the same model: eigenvalues, frequencies, every mode value, residuals, iterations, shift, Sturm record, by `to_bits` | identical | by construction (D1) |
| A1b | a1 | the speed-0 column of F's Campbell table: the eigenvalues of `modal_analysis` by `to_bits` | identical | by construction |
| A2a | a2 | B, `stress_stiffening = false`, `Omega_B`: `|(omega_n,0^2 - omega_n^2)/Omega^2 - 1|`, n = 1, 2 | `<= 1e-3` | `<= 1e-10` (arithmetic 2) |
| A2b | a2 | B, full operator, `Omega_B`: `|(omega_n^2 - omega_n,0^2)/Omega^2 - (Q_n,h - 1)|`, n = 1, 2 | `<= 1e-3` | `~ 4e-5 Q_n` (arithmetic 1) |
| A2c | a2 | B: `|Q_n,h/Q_n - 1|`, `Q_n` closed form (1.0724670, 7.6522033) | `<= 1e-2` | `1.3e-4`, `1.2e-3` |
| A2d | a2 | B: the largest centrifugal stress at `Omega_B` (from the static result at 1 rad/s times `Omega_B^2`) over `E` | `< 1e-4` | `5e-5` |
| A3 | a3 | F at `0.2 omega_0`: `|(omega_flap^2 - omega_lag^2)/Omega^2 - 1|` | `<= 1e-4` | **predicted miss: 1.94e-3** (arithmetic 3) |
| A3b | a3 (diagnosis) | F at `0.2 omega_0`: `|(omega_flap^2 - omega_lag^2)/Omega^2 - (1 - 2 f_x,h)|`, `f_x,h` the `x` share of the flap mode's kinetic energy at that speed | `<= 5e-4` | the neglected `sigma_xy`, the root, second order: `<= 2e-4` |
| A4 | a4 | F: `|S(0.2 omega_0)/1.193336 - 1|` | `<= 2e-2` | `1.3e-3` (arithmetic 4) |
| A4b | a4 | F: `|Q_h/1.193336 - 1|` (rest flap mode, chosen in the degenerate pair as the combination with the largest `z` share) | `<= 2e-2` | `~1e-3` |
| A4c | a4 | F: first-order consistency, `|S(0.01 omega_0) - slope(phi_f,0)|` | `<= 1e-3` | `~1e-5` (second order `1e-4 x 0.014`) |
| A5 | a5 | F at `Omega/omega_0 = 0.2, 0.5, 1, 2, 3, 5`: `1 <= S <= Q_h` | each | `1.091 .. 1.192` |
| A6 | a6 | synthetic families: every crossing with orders 1 to 4 against `omega_0/sqrt(k^2 - S)` (relative), one crossing per order and family | `<= 1e-8` | `~1e-15` (exact interpolation) |
| A7 | a7 | F's table: each crossing (flap k = 2, 3, 4; lag k = 1 to 4) against the crossing of a direct solve (regula falsi, Illinois, on `omega(Omega) = k Omega` with the flap or lag mode of the speed, to `1e-10` relative in `Omega`), relative; no flap crossing with order 1 in the table | `<= 1e-2` | `< 1e-3` (arithmetic 5) |
| A8 | a8 | R's table: the track starting as flap keeps a `z` share `> 0.9` at all 13 speeds and the track starting as lag a `y` share `> 0.9`; the MAC of each of the two tracks with its previous mode `> 0.9` at every step; the two tracks' frequency order is reversed between `0.6` and `0.7 omega_0f` | each | MAC `~1`, shares `~0.999` |
| A9 | a9 | K at `0.5, 0.9 omega_r0`: modes with finite positive eigenvalues; at `1.1, 1.5 omega_r0`: `LossOfStiffness` with exactly the requested speed (bits) and `negative >= 1`; the Campbell table of the four speeds: frequencies `None` at the two losses, the losses listed with their speeds, every stored number finite, the CSV and the JSON contain no `NaN` or `inf` | each | arithmetic 6 |
| A9b | a9 | B with `stress_stiffening = false` at `1.1 omega_1,0` (elimination path): `LossOfStiffness`, `negative == Some(1)` | each | arithmetic 6 |
| A10 | D4 | the CSV of F's table parses back to the table's speeds and frequencies by `to_bits`; the JSON has `null` exactly at the losses of K's table | each | shortest round-trip formatting |
| A11 | D4 | refusals: a decreasing speed list, a negative speed, a NaN speed, a zero axis, each refused with its key | each | |
| A12 | rule 3 | the existing tests of `rcfd-fem` pass unchanged (`cargo test -p rcfd-fem --release`), and `cargo check -p rcfd-post --tests` | all pass | no shared file edited |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | F at `0.2 omega_0` with `spin_softening = false`: `|(omega_flap^2 - omega_lag^2)/Omega^2|` | `< 1e-2` (predicted `~ f_x = 1e-3`): the flap-lag identity needs the softening |
| N2 | F at `0.2 omega_0` with `stress_stiffening = false`: `|S_flap|` | `< 1e-2` (predicted `-f_x`): A4 needs the stiffening |
| N3 | R between `0.6` and `0.7 omega_0f`: the MAC of mode 1 at 0.6 with mode 1 at 0.7 (pairing by frequency order) | `< 0.1` (flap and lag are orthogonal in shape): A8 discriminates |
| N4 | B with the full operator: `(omega_1,0^2 - omega_1^2)/Omega^2` (the literal reading of (a2)) | `< 0` (predicted `1 - Q_1,h ~ -0.072`) |

Planned mutations (on a scratch copy of `rotating.rs`, each run once against `si_rotating`; what catches each is recorded):

1. `M_perp` with `delta_ij` only (no `- a_i a_j`): expected caught by A3/A3b, A4, A5.
2. `K_sigma,1` scaled by `Omega` instead of `Omega^2`: A2b, A4.
3. Sign of the spin softening flipped: A2a, A3.
4. Crossing interpolated linearly in `Omega` instead of `Omega^2`: A6.
5. Tracking by frequency order instead of MAC: A8.
6. The loss test of D3 removed (eigensolver called directly): A9, A9b.
7. The zero-speed shortcut removed (the general path at `Omega = 0`): A1 (it may survive, adding `0 x` terms to values; the
   outcome is recorded either way).
8. The greedy assignment by increasing instead of decreasing MAC: A8.

## Decision rule

- **done**: every check A1 to A12 met (A3 included) and N1 to N4 as expected.
- **partial**: A1, A2a to A2d, A4 to A12 met and N1 to N4 as expected, with A3 missed (an ignored `*_known_miss` test with the
  limit intact) and A3b met (the miss diagnosed as the 3-D section rotation and lateral stress of arithmetic 3), or any other
  single check missed with a diagnosed cause.
- **todo**: A1 missed, or a loss of stiffness produces a non-finite number (A9), or two or more of A2a, A4, A6, A7, A8 missed.

Given arithmetic 3 the registered expectation is **partial** (A3), with the proposal to amend (a3) for a solid model.

## Limits (what is not covered)

The Coriolis term (D6); a pre-stress beyond linear in `Omega^2`; harmonic indices other than 0 of a cyclic sector; modes of a
free body under rotation (reported as a loss of stiffness); beams and shells (no geometric stiffness there, ADR 0390); the
model's other loads in the pre-stress; output through the case files and the CLI (F-386/ADR 0306). The bar and beam checks use
`nu = 0`; with `nu > 0` the clamped root adds a local stress that the checks do not separate.

## Addendum 1: results (2026-11-13)

Code: `crates/rcfd-fem/src/rotating.rs` (new content of the stub; no other library file changed), tests
`crates/rcfd-fem/tests/si_rotating.rs`. The registered run of `si_rotating` was made once (release, `--test-threads=2`, 7.8 s
for the ten tests); A3 missed as predicted and its test became the ignored record `a3_flap_lag_identity_known_miss` with the limit
intact (the body unchanged, only the name and the `#[ignore]` reason). Log: `f396-scratch/run1.log`.

| # | Measured | Limit | Met |
|---|---|---|---|
| A1 F | speed 0 against `modal_analysis`, every field by `to_bits` | identical | yes |
| A1b F | speed-0 column of the table | identical | yes |
| A1 K | speed 0 on the transformation path | identical | yes |
| A2a | n = 1: 1.92e-9; n = 2: 1.80e-9 (`Omega_B = 50.475` rad/s) | `<= 1e-3` | yes |
| A2b | n = 1: 1.66e-6 (`Q_1,h = 1.0725406`, slope 0.0725389); n = 2: 3.12e-5 (`Q_2,h = 7.6604761`) | `<= 1e-3` | yes |
| A2c | n = 1: 6.86e-5; n = 2: 1.08e-3 | `<= 1e-2` | yes |
| A2d | 4.999e-5 | `< 1e-4` | yes |
| A3 | **1.922e-3** (`(w_flap^2 - w_lag^2)/W^2 = 0.99807796` at `0.2 omega_0 = 51.15` rad/s; predicted 1.94e-3) | `<= 1e-4` | **no** (known miss) |
| A3b | 1.12e-8 against `1 - 2 f_x,h`, `f_x,h = 9.610e-4` (predicted 9.68e-4) | `<= 5e-4` | yes |
| A4 | 2.18e-3 (`S(0.2 omega_0) = 1.1907368`) | `<= 2e-2` | yes |
| A4b | 8.73e-4 (`Q_h = 1.1922947`, z share of the rest flap mode 0.999037) | `<= 2e-2` | yes |
| A4c | 1.24e-6 (slope of the rest flap mode 1.1913316) | `<= 1e-3` | yes |
| A5 | S = 1.19074, 1.18771, 1.17799, 1.15061, 1.12462, 1.08852 at 0.2, 0.5, 1, 2, 3, 5 `omega_0`, all in `[1, 1.1922947]` | each | yes |
| A6 | 12 crossings, one per order and family, largest relative error 2.2e-16 | `<= 1e-8` | yes |
| A7 | flap k = 2, 3, 4: 3.2e-6, 8.7e-6, 4.5e-6; lag k = 1 to 4: 6.0e-6, 8.2e-6, 7.9e-6, 4.6e-6; no flap crossing with order 1 | `<= 1e-2` | yes |
| A8 | flap track z share min 0.999037, lag track y share min 0.998616, MAC min 0.999991; flap/lag eigenvalue ratio 0.9475 at 0.6, 1.0328 at 0.7 `omega_0f` | each | yes |
| A9 | K (`omega_r0 = 1595.9` rad/s): finite positive at 0.5 and 0.9; `LossOfStiffness { negative: Some(1) }` with the requested speed's bits at 1.1 and 1.5; table `None` at both, losses listed; CSV and JSON without NaN or inf | each | yes |
| A9b | B, softening only, `1.1 omega_1,0 = 8722.09` rad/s: `negative: Some(1)` | each | yes |
| A10 | CSV parses back by `to_bits`; JSON `null` exactly at the two losses | each | yes |
| A11 | decreasing, negative, NaN speed lists (`speeds`), zero axis (`axis`), negative speed (`speed`) refused | each | yes |
| A12 | `cargo test -p rcfd-fem --release`: 265 passed, 0 failed, 56 ignored (45 binaries); `cargo check -p rcfd-post --tests` clean | all pass | yes |
| N1 | 9.61e-4 | `< 1e-2` | as expected |
| N2 | 9.63e-4 | `< 1e-2` | as expected |
| N3 | 7.1e-19 | `< 0.1` | as expected |
| N4 | -7.254e-2 (predicted `1 - Q_1 = -0.0725`) | `< 0` | as expected |

Mutations (a `git archive` copy of the branch in `f396-scratch/mut_src`, `f396-scratch/mutate.py`, each run once, logs
`f396-scratch/M*.log`):

| Mutation | Caught by |
|---|---|
| M1 `M_perp` with `delta_ij` only | A3b, A4, A5 (all six), N2, A7, A8, N3 |
| M2 `K_sigma,1` scaled by `Omega` | A2b, N4, A3b, A4, A4c, A5 |
| M3 spin softening sign flipped | A2a, A2b, A9b, A3b, A4c, A5, A7, A8, N3, A9, A10 |
| M4 crossing linear in `Omega` | A6 (all twelve), A7 (k = 4) |
| M5 tracking by frequency order | A8 |
| M6 the inertia test removed | A9b and A9: the eigensolver refuses the indefinite factor, the tests fail with an error |
| M7 zero-speed shortcut removed | **not caught** (as the registration allowed): adding `0 x` terms leaves every value's bits, so the general path gives the bits of `modal_analysis` on these models; the shortcut stays as the guarantee |
| M8 greedy by increasing MAC | A8, A7 |

`cargo fmt -p rcfd-fem --check`: clean (after one layout-only commit). `cargo clippy -p rcfd-fem --all-targets --release -- -D
warnings`: clean. Shared files: none edited (`geometric.rs`, `modal.rs`, `mpc.rs`, `mass.rs`, `assemble.rs`, the element files
are used read only), so `baseline_hashes` was not required and was not run.

Deviations: none from the registered set-ups and limits. Not run: `baseline_hashes` (no shared solver file changed); the
whole workspace suite (the lead's gate).

**Decision by the registered rule: partial.** Every check except A3 is met and N1 to N4 are as expected; A3 missed by the
amount and for the cause predicted before the code (arithmetic 3), and A3b confirms the diagnosis to 1.1e-8: on a 3-D solid the
flap-lag difference is `(1 - 2 f_x) Omega^2`, `f_x` the axial (section-rotation) share of the bending mode's kinetic energy,
because both bending modes' section rotation is spin-softened and the lateral centrifugal stress stiffens the lag mode by the same
amount. The identity of the clause holds for an Euler-Bernoulli beam only. Clause (a2) was met in the reading registered before
the code (the spin-softening term against its closed form; the full operator against `1 - Q_n,h`).

Remaining questions:

1. **Wording of (a3).** Options: (a) restate it for a solid, `(omega_flap^2 - omega_lag^2)/Omega^2 = 1 - 2 f_x` within 1e-4,
   `f_x` the axial kinetic share of the flap mode (met: 1.1e-8), with the Euler-Bernoulli identity `1` within `3 f_x`; (b) keep it
   and check it on a beam element once beams have geometric stiffness; (c) loosen the limit to 5e-3. Recommendation (a): it is
   the identity of the operator the acceptance names, and with it F-396 is done.
2. **Wording of (a2).** Options: (a) state that the bar's check is of the spin softening with the stress stiffening off, and that
   the full operator gives `1 - Q_n` (`Q_n = k_n^2 L^2/3 + 1/4`); (b) leave it as read here. Recommendation (a).
3. **Coriolis.** Options: (a) a follow-up with the gyroscopic matrix and the quadratic eigenproblem (forward and backward whirl);
   (b) leave it out for the 120 mm fan blade (stiff hub). Recommendation (a), as a separate item after the fan's first blade
   Campbell diagram.
4. **Sector at harmonic index `h`.** Options: (a) a follow-up that applies `K_eff(Omega)` on the cyclic harmonic pair of ADR
   0386/0395 (the pre-stress from the index-0 sector, by `RotatingPart::new` through the sector's cyclic constraint or by
   `with_unit_prestress`); (b) only the full rotor. Recommendation (a): the fan rotor is meshed as a sector.

### For the lead: roadmap entries

F-396, proposed status **partial**, note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). ADR 0403, 2026-11-13: rotating.rs gives the modes of a part spinning about an axis in the rotating frame, K + Omega^2 (K_sigma,1 - M_perp) with the centrifugal pre-stress of one static solve at 1 rad/s and the spin-softening mass m_ab (delta_ij - a_i a_j), on the elimination and the transformation path; speed 0 gives the bits of the modal analysis; a speed at which K_eff is not positive definite is reported as a loss of stiffness with the speed and the negative count (Sylvester inertia), never as a frequency; modes over speeds tracked by the mass-weighted modal assurance, engine-order crossings linear in Omega^2 (exact for omega^2 = omega_0^2 + S Omega^2, 2e-16), the Campbell table as CSV and JSON. Radial bar: spin softening 1 within 1.9e-9, the full operator 1 - Q_n,h within 3.1e-5; square beam: stiffening 1.1907 at 0.2 omega_0 (2.2e-3 from 1.1933), between 1 and the rest quotient up to 5 omega_0, crossings within 8.7e-6 of direct solves; flap and lag of a rectangular section tracked through their crossing (MAC 0.99999). Partial: the flap-lag identity misses 1e-4 at 1.9e-3 (known miss, predicted in the registration): on a solid it is 1 - 2 f_x (met to 1.1e-8), f_x the axial kinetic share of the bending mode. Coriolis neglected. Follow-ups: the flap-lag clause restated for a solid, Coriolis coupling, rotating modes of a sector at a harmonic index.

Follow-up 1 (title "Flap-lag identity of a rotating solid beam: the acceptance restated", deps F-396), acceptance:

> Every limit not stated here is registered in the ADR before the run. On the square radial beam of F-396 (Hex20, slenderness 20, nu = 0) at Omega = 0.2 omega_0: (omega_flap^2 - omega_lag^2)/Omega^2 = 1 - 2 f_x within 1e-4, f_x the axial share of the flap mode's kinetic energy, and = 1 within 3 f_x; the radial bar's clause reads: with the stress stiffening off (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3, with it the slope equals Q_n,h - 1 within 1e-3 and Q_n,h equals k_n^2 L^2/3 + 1/4 within 1 percent. No new code is needed if the checks of ADR 0403 are accepted as these.

Follow-up 2 (title "Coriolis coupling of a rotating part: gyroscopic matrix, whirl modes and the Campbell table", deps F-396), acceptance:

> Every limit not stated here is registered in the ADR before the run. The gyroscopic matrix G = 2 Omega int rho N_a N_b [a x] is skew to rounding; the quadratic eigenproblem (K_eff - omega^2 M + i omega G) phi = 0 is solved by a symmetric linearisation; at Omega = 0 the frequencies are those of F-396; a point mass on an isotropic in-plane spring pair gives the closed-form whirl frequencies within 1e-8; the tracked forward and backward branches enter the Campbell table of F-396 with their direction.

Follow-up 3 (title "Rotating modes of a cyclic sector at a harmonic index", deps F-396, F-394), acceptance:

> Every limit not stated here is registered in the ADR before the run. K_eff(Omega) of F-396 on the cyclic harmonic pair of ADR 0386/0395 with the pre-stress of the index-0 sector: at each harmonic index h and speed the frequencies of the sector equal those of the full rotor mesh for that nodal-diameter family within 1e-8 relative; at Omega = 0 the bits of the cyclic modes of F-394; a loss of stiffness is reported per index and speed; the Campbell table holds one family of tracks per index, with the crossings with the engine orders.
