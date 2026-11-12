# ADR 0284: Mass matrix and linear modal analysis of a solid model

Status: **REGISTERED 2026-11-12, written before any code of F-383 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-383 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done with a short Python
script of closed forms, transcendental roots and one-dimensional estimates (`w49i-work/f383-scratch/arith.py`, output
`arith.log`). It runs no library code and computes no three-dimensional finite-element model. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Work package SB-14 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0284). Roadmap item F-383 (depends on F-374 and F-382, both done). Size M. Number 0284, fixed by the plan; the
file name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written").
Branch `f383-mass-modal` from main adede15c, worktree `w49i-work/f383`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only the two stub files `crates/rcfd-fem/src/mass.rs` and `crates/rcfd-fem/src/modal.rs` (one line
  each, declared by ADR 0271 for this package) are filled, and one test file `crates/rcfd-fem/tests/sb_modal.rs` is added.
  `lib.rs` and `error.rs` are not edited (the refusals use the existing `FemError::InvalidValue` and `FemError::Solver`); no
  file outside `rcfd-fem`, no shared solver file, no golden, no hash table is touched; recovery code (ADR 0277, being built in
  parallel) is not touched;
- no new dependency, no unsafe code, the graph unchanged (`rcfd-fem` keeps `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`);
- clean-room: the consistent mass matrix (the kinetic energy of the interpolated velocity), rigid-body mass properties, the
  bar and Euler-Bernoulli and Timoshenko beam frequencies, the min-max bounds and the rounding estimates are derived below from
  first principles; books and papers are cited from memory for orientation only.

## What F-383 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Consistent mass matrix for Tet4, Hex8 and Tet10: for a
> box, the total mass, the centre of mass and the inertia tensor obtained from rigid motions r^T M r equal the closed forms
> within 1e-12. Modal analysis: a free box has six frequencies below 1e-6 of the seventh; a fixed-free bar with nu = 0 has f_n
> = (2n - 1) sqrt(E/rho)/(4 L), n = 1, 2, 3, within 0.5 percent on a registered mesh, and the error of f_1 falls at an observed
> order >= 1.8 with Hex8; a cantilever of slenderness 20 and square section has its first bending frequency within 1 percent of
> (1.8751)^2 sqrt(E I/(rho A L^4))/(2 pi) and its second (4.6941) within 2.5 percent with Tet10, the Euler-Bernoulli value less
> the shear and rotary-inertia correction estimated in the registration (0.15 and 0.9 percent); on a symmetric Hex8 mesh the
> two bending planes give the same frequency within 1e-6 relative; mode shapes are M-orthonormal within 1e-9.

Clauses: (a1) a consistent mass matrix for Tet4, Hex8, Tet10; (a2) on a box, the total mass, (a3) the centre of mass and (a4)
the inertia tensor from `r^T M r` with rigid motions `r`, each equal to the closed form within 1e-12; (a5) a free box: six
frequencies below 1e-6 of the seventh; (a6) a fixed-free bar, `nu = 0`: `f_n = (2n - 1) c / (4 L)`, `n = 1, 2, 3`, within 0.5
percent on a registered mesh; (a7) the error of `f_1` falls at an observed order `>= 1.8` with Hex8; (a8) a cantilever of
slenderness 20 and square section with Tet10: the first bending frequency within 1 percent and (a9) the second within 2.5
percent of the Euler-Bernoulli value less the shear and rotary-inertia correction estimated here; (a10) on a symmetric Hex8
mesh the two bending planes give the same frequency within 1e-6 relative; (a11) mode shapes M-orthonormal within 1e-9. The
brief adds: choose the eigen route on top of ADR 0283 and register it; register how the free box's rigid modes are shifted;
derive the Timoshenko corrections and the mesh sizes that should meet the limits; heavy runs ignored with fast covering tests;
modes identified by shape, not by index (the stub).

Reading of "slenderness 20": `L / b = 20` with `b` the side of the square. This is the reading under which the stub's
estimates (0.15 and 0.9 percent) come out (arithmetic 3); slenderness as `L / r` (`r = b / sqrt 12`) would be a stocky beam
of `L / b = 5.8`.

## What was read (main adede15c, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 2.2 (conventions; determinism for any thread count; refusals located), 2.3 (`mass`,
   `modal` are this package's), 2.4 (the `Element` trait leaves room for the mass), 2.5 item 1 (`mass::consistent(&SolidModel)
   -> CsrMatrix` is a named interface), the entries 0283, 0284, 0288 of section 3, and the decision record; this ADR's stub;
   `docs/roadmap.json` F-383 (acceptance, note).
2. ADR 0271 (density 0 accepted by the data model, its remaining question 3: "the mass matrix of ADR 0284 refuses a model with
   zero mass"); ADR 0273 in full with its addendum (`Element::mass_rule`: `tet_4` for Tet4, `gauss_3` for Hex8, `tet_14` of
   degree 5 for Tet10, proposed for this package, its remaining question 2 recommends taking them); ADR 0275 (D1, D2:
   `assemble::assemble_with(model, threads, f)` the generic hook, bit-identical for any thread count; D3, D5: constraints by
   symmetric elimination with `constraint::reduce`, the rigid-motion check `check_rigid_body_motions`; its note for F-383);
   ADR 0276 (body loads by the mass rule, the perturbed-box helpers of `sb_loads.rs`); ADR 0283 in full with its addendum
   (`lowest_eigenpairs`, `Shift::{Auto, Value}`, `SubspaceOptions`, the Sturm record, its remaining question 1: `Shift::Auto` and
   `m >= 7` for a free body; measured: free chain zero eigenvalue at 5e-17 of `lambda_max`); ADR 0274 D4 (pivot rules).
3. Code: `crates/rcfd-fem/src/{lib.rs, element.rs, quadrature.rs (signatures), assemble.rs, constraint.rs, solve.rs,
   builders.rs, model.rs (signatures), error.rs, mass.rs, modal.rs}`, `crates/rcfd-fem/tests/sb_loads.rs` (lines 1 to 160:
   `Rng`, `Checks`, `rebuild`, `box_of`), `crates/rcfd-linalg/src/eigen.rs` (public signatures, lines 445 to 700).
4. The machine: 16 threads, 23.9 GB free at the time of reading (another agent builds in the same crate).

## Arithmetic before this registration (disclosed)

All from `f383-scratch/arith.py` (log `arith.log`).

1. **Euler-Bernoulli cantilever roots** of `cos x cosh x + 1 = 0` by bisection: `beta_1 L = 1.8751040687`, `beta_2 L =
   4.6940911330`, `beta_3 L = 7.8547574382`; `(beta L)^2 = 3.51601527, 22.03449156, 61.69721441`.
2. **Cantilever geometry** (nondimensional, `L = 1`, `b = 1/20`, `E = rho = 1`, `nu = 0.3`): `r / L = 1.443376e-2`;
   `omega_EB = 0.050749, 0.318040, 0.890523` (units `c / L`, `c = sqrt(E/rho)`); first torsion (Saint-Venant, `J = 0.1406 b^4`
   from memory) `0.8947`; first axial `1.5708`. EB effective mass fractions of the modes: 0.6131, 0.1883, 0.0647.
3. **Timoshenko corrections** (D6 below), `kappa = 5/6`: exact Timoshenko cantilever / EB - 1 = **-1.98760e-3** (mode 1),
   **-1.36110e-2** (mode 2), -3.13946e-2 (mode 3); shear alone -1.5074e-3, -1.0379e-2; rotary inertia alone -4.8383e-4,
   -3.3605e-3; the determinant with `kappa -> 1e12` and no rotary inertia returns EB to 1.3e-14 (check of the code). With
   Cowper's `kappa = 10 (1 + nu)/(12 + 11 nu) = 0.8497`: -1.95874e-3, -1.34167e-2 (sensitivity 1.5 percent of the
   correction). First-order Rayleigh estimate with the cantilever mode shape, `(r^2 / 2) J_n (1 + E/(kappa G))`, `J_n =
   int w'^2 / int w^2 = int w'''^2 / int w''^2 = 4.6478, 32.4174, 77.2989`: -1.99e-3 and -1.37e-2, agreeing with the exact
   roots. The stub's 0.15 and 0.9 percent are the simply-supported form with `(beta_n L)^2` in place of `J_n L^2`: 1.509e-3
   and 9.456e-3 (arithmetic only; see D6 for why `J_n` is the right factor).
4. **Fixed-free bar, Hex8 = the 1-D linear chain** (D5): relative error of `f_n`, `n = 1, 2, 3`:
   8 elements: 1.607e-3, 1.451e-2, 4.048e-2; 16: 4.016e-4, 3.618e-3, 1.007e-2; **32: 1.004e-4, 9.038e-4, 2.512e-3**; 64:
   2.510e-5, 2.259e-4, 6.276e-4. Observed order of `f_1`: 8 -> 16 2.0005, 16 -> 32 2.0001, 32 -> 64 2.0000.
5. **1-D quadratic chain** (bound for Tet10, D5): 4 elements 1.6e-5, 1.25e-3, 8.6e-3; **8: 1.03e-6, 8.21e-5, 6.15e-4**; 16:
   6.4e-8, 5.2e-6, 4.0e-5 (computed numerically from the 3 x 3 element matrices; an estimate, not a reference).
6. **Free box** (1.0 x 0.6 x 0.4, steel, 3 x 2 x 2 cells): Irons' element bound of `lambda_max` for Hex8 `<= 12 (h_x^-2 +
   h_y^-2 + h_z^-2) (3 lambda_L + 2 mu) / rho = 3.45e10`; first elastic mode estimated by free-free torsion `omega = 8.4e3`
   (bending about the thin axis, EB, 1.3e4): `lambda_7 ~ 7.0e7`, ratio `lambda_max / lambda_7 <= 4.9e2` (Hex8), `~7.4e3` with a
   factor 15 for Tet10 (1-D quadratic element `lambda_max h^2 = 60` against 12 for linear, times a margin 3 for the Kuhn tets).
   Rounding of a rigid eigenvalue (D7) `~ eps lambda_max / sqrt(N)`: for a ratio of 1e4, 2.1e-13 (N = 108) and 9.7e-14 (N =
   525) of `lambda_7`, against the limit 1e-12 (a frequency ratio 1e-6 is an eigenvalue ratio 1e-12).
7. **Mass box** (origin (0.1, -0.2, 0.3), lengths (1.2, 1.0, 0.8), `rho = 7850`): `m = 7536`, centre (0.7, 0.3, 0.7), means of
   `x_k^2` over the box 0.61, 0.17333, 0.54333.
8. **Sizes** (Tet10 cantilever on the Kuhn box: nodes on the half-spacing grid `(2 n_x + 1)(2 n_y + 1)(2 n_z + 1)`): 20 x 1 x
   1: 1107 unknowns; 40 x 2 x 2: 6075, envelope ~9e5; **80 x 4 x 4: 39123 unknowns, envelope ~1.9e7 (152 MB), factor ~4.6e9
   multiply-adds** (about 1 s at ADR 0274's 4.6e9 per second; with ~15 iterations of 14 solves, ~10 s).

## Design

**D1. Consistent mass matrix.** The kinetic energy of the interpolated velocity `v_h = sum_a N_a v_a` is `T = 1/2 int rho v_h
. v_h dV = 1/2 v^T M v` with

`M_{(a,i),(b,j)} = delta_ij int_e rho N_a N_b dV ~= delta_ij sum_q rho_e w_q det J_q N_a(xi_q) N_b(xi_q)`.

The element routine `mass::element_mass(element, coords, density)` sums over `Element::integration_points(coords,
mass_rule())` (ADR 0273), computes the scalar matrix `m_ab` for `a <= b` and mirrors it (exactly symmetric), and places
`m_ab` on the three diagonal component pairs `(3a + i, 3b + i)` (node-major, ADR 0273). `element_mass_with_rule` takes a given
rule (negative control N1). The global matrix: `mass::consistent(model)` = `consistent_with(model, None)` =
`assemble::assemble_with(model, threads, |ctx| element_mass(element_for(ctx.kind), ctx.coords, ctx.material.density()))`, so
it has the stiffness pattern and is bit-identical for any thread count (ADR 0275 D2). The material is per element set and
constant per element, so `rho_e` is outside the sum.

*Rules* (taken from ADR 0273 D3, its remaining question 2, on the maintainer's standing instruction of 2026-11-11, take the
recommendation): Tet4 `tet_4` (degree 2), Hex8 `gauss_3` (degree 5 per variable), Tet10 `tet_14` (degree 5). The integrand
`N_a N_b det J` has degree 2 on Tet4, 4 on a straight-sided Tet10 (`det J` constant) and at most `1 + 1 + 2 = 4` per variable
on any trilinear Hex8 (`det J` of a trilinear map has degree at most 2 per variable): all exact. On a curved Tet10 (`det J`
of degree 3, integrand 7) no fixed rule is exact; that is a limit, not a check here.

*Rigid-body mass properties.* A rigid motion `r(x) = t + w x (x - p)` is linear in `x`; the isoparametric map interpolates
`x` exactly, so `r` is in every element space and its nodal vector `R` (values at the nodes) represents it exactly. Hence
`R^T M S = int rho r . s dV` for two rigid motions, an integrand of degree 2 in `x`, integrated exactly by the rules above (on
straight tetrahedra and trilinear hexahedra). With the six fields `e_k` (translations) and `e_k x (x - p)` (rotations about
`p`) the 6 x 6 Gram matrix is

`[[m I, -m [x_c - p]_x], [m [x_c - p]_x, I_p]]` in blocks, where `[a]_x` is the cross-product matrix, `m = int rho`,
`m (x_c - p) = int rho (x - p)`, `I_p = int rho (|x - p|^2 I - (x - p)(x - p)^T)`;

(the translation-rotation block: `e_i . (e_k x (x - p))` integrated is `m e_i . (e_k x (x_c - p))`). Public helpers:
`mass::rigid_motions(model, about) -> [Vec<f64>; 6]` and `mass::mass_properties(model, &M) -> MassProperties { mass, centre,
inertia }` (inertia about the centre of mass: the centre from the translation rows about the origin, then the rotation block
recomputed about the centre, so that no subtraction of the parallel-axis term loses digits).

*Closed forms of a box* `[o, o + l]` (checks C1 to C3): `m = rho l_x l_y l_z`, `x_c = o + l/2`, and about the global origin
`I_xx = m (<y^2> + <z^2>)`, `I_xy = -m <x><y>` (and cyclic) with `<x_k^2> = ((o_k + l_k)^3 - o_k^3) / (3 l_k)` and `<x_k> =
o_k + l_k / 2` (the coordinates are independent over a box).

**D2. Modal analysis: the problem and its reduction.** Undamped free vibration `M u'' + K u = 0`, `u = phi e^{i omega t}`:
`K phi = lambda M phi`, `lambda = omega^2`, `f = omega / (2 pi)`. Zero prescribed displacements are imposed by the same
symmetric elimination as the static solve (ADR 0275 D3): with the prescribed equations `P` from
`constraint::prescribed_equations(model, extra)`, `K_FF phi_F = lambda M_FF phi_F` (both reduced by `constraint::reduce` with
a zero right-hand side), and the full mode is `phi = (phi_F, 0)`. The principal submatrices of a symmetric (positive
semi-definite, respectively definite) matrix keep those properties, and `phi^T M phi = phi_F^T M_FF phi_F`: M-orthonormality
of the reduced vectors is that of the full ones. A non-zero prescribed value is refused (a modal analysis is about the
reference state; `InvalidValue { key: "constraint" }` naming the node and component). The model's loads are ignored (they do
not enter a linear modal analysis at rest; rotation effects are intermediate, ADR 0270 1.2).

*Massless unknowns.* `M_FF` must be positive definite (ADR 0283 D3). A free unknown whose diagonal `m_ii` is not positive
(a node touched only by elements of density 0) is refused before the eigensolver, `InvalidValue { key: "density" }` naming
the node and component (ADR 0271 remaining question 3, option (a), taken on the standing instruction). A semi-definite `M` of
another origin is caught by the eigensolver (`RankDeficient` or the Cholesky of the reduced mass) and reported as
`FemError::Solver { solver: "lowest_eigenpairs" }` with its text.

**D3. Eigen route.** `rcfd_linalg::eigen::lowest_eigenpairs(K_FF, M_FF, m, opts)` of ADR 0283 (subspace iteration on the
envelope factor of `K + sigma M`, Rayleigh-Ritz, residual test `1e-10` per pair, clusters crossing `m` completed, Sturm count
verifying that no eigenvalue below the last returned one was missed). It is enough for the basic tier: the checks need at
most 7 modes on at most 4e4 unknowns (arithmetic 8). No Lanczos or second shift-invert layer is added (limits). Options
passed through: tolerance (default `1e-10`), iteration cap (200), the shift (D4), the subspace size (ADR 0283 D7 default
`max(2 m, m + 8)`), RCM ordering.

**D4. The shift.** ADR 0283 D3: the factored matrix is `A = K_FF + sigma M_FF`, `lambda = mu - sigma`, `A` SPD iff `sigma >
-lambda_1`. Rule of this package (`ModalOptions::shift = None`, the default):

- if the prescribed equations remove every rigid motion of every connected part (`constraint::check_rigid_body_motions` is
  `Ok`), `K_FF` is SPD and `sigma = 0` (`Shift::Value(0.0)`): the fastest rate `lambda_i / lambda_{q+1}` and no loss of
  accuracy. `Shift::Auto` is not used here because its pivot-ratio floor (`min d_i / a_ii >= 1e-8`) can reject `sigma = 0`
  for a slender constrained body (the cantilever: `lambda_max / lambda_1 ~ 1e9`), and its next candidate `1e-4 rho` (`rho =
  tr K / tr M`, about `lambda_max / 3`) would then lie far above the lowest modes and slow the iteration to a rate near 1;
- otherwise (a free or partly free body; the check returns `RigidBodyMotionFree`), `Shift::Auto` (ADR 0283 D3, its
  remaining question 1 recommends it): `sigma = 0` is rejected (the factor of a singular `K` has six pivots at rounding level),
  and `sigma = 1e-4 rho` is taken. For the free box `rho <= lambda_max / 3`-ish, `sigma / lambda_7 <~ 1e-4 x 4.9e2 / 3 ~ 2e-2`
  (Hex8), well below the first elastic eigenvalue, so the rate of the elastic pairs is barely changed and `lambda = mu -
  sigma` loses nothing (`eps mu`).
- `ModalOptions::shift = Some(s)` passes `s` to the eigensolver as given (`Shift::Auto` or `Shift::Value`).

Any other error of the rigid-motion check (a free node of no element) is returned as it is.

**D5. Fixed-free bar, `nu = 0`: the closed form and why the elements should meet it.** With `nu = 0` the Lame constant
`lambda_L = E nu / ((1 + nu)(1 - 2 nu)) = 0` and `mu = E / 2`. The field `u = (U(x) sin, 0, 0)` with `U'' + k^2 U = 0`, `k =
omega / c`, `c = sqrt(E / rho)`, has stress `sigma_xx = E U'`, all other components 0; the lateral surfaces are traction-free
and the equations of motion `rho u_tt = div sigma` reduce to the wave equation: an exact 3-D solution. `U(0) = 0` and `U'(L)
= 0` give `k_n L = (2n - 1) pi / 2`, `f_n = (2n - 1) c / (4 L)`.

*Constraints of the bar* (D3 set-up `BAR`): `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`. The axial field
satisfies both rollers (`u_y = u_z = 0`, shear tractions `sigma_xy = sigma_xz = 0`), so the axial modes are unchanged; the
rollers remove every rigid motion (each rotation moves one roller face normal to itself) and make the bar the quarter of a bar
of section `2b x 2b`, symmetric in `y` and `z`, which removes bending and torsion: the lowest continuum modes are the axial
ones; the next family (lateral, `u_y(y)` between a roller and a free face) has `f >= c / (4 b) = 2.5 c / L` for `b = L / 10`,
twice `f_3 = 1.25 c / L`. Modes are still identified by shape (D8).

*Hex8 is exactly the 1-D linear chain.* Let `v = (F(x_i), 0, 0)` with `F` piecewise linear on the planes `x_i`. With
`lambda_L = 0` the bilinear forms are `a(v, w) = E int F' d_x w_x dV` and `m(v, w) = rho int F w_x dV` for any trial `w`. On a
structured Hex8 box the section integral `A wbar(x) = int w_x dy dz` of a trilinear field is linear in `x` within a slab, so
`a(v, w) = E A int F' wbar' dx` and `m(v, w) = rho A int F wbar dx` with `wbar` in the 1-D linear space. Hence if `(lambda, F)`
is an eigenpair of the 1-D chain (stiffness `(E A / h)(2, -1)`, consistent mass `(rho A h / 6)(4, 1)`, the free end halved),
`v` satisfies `K v = lambda M v` in 3-D. The chain's eigenvalues are exact: the fixed-free chain of `n` elements is the
symmetric half of the fixed-fixed chain of `2n` elements, whose eigenvectors are `sin(i j pi / (2n))` (Toeplitz), so `k = (2j -
1) pi / (2L)` and

`lambda_h = (6 c^2 / h^2) (1 - cos k h) / (2 + cos k h)`, relative frequency error `~ (k h)^2 / 24` (arithmetic 4).

So the Hex8 bar has the discrete closed form to rounding (extension check B3), and the error of `f_1` has order 2 (2.0001 from
16 to 32). Mesh registered for a6: **32 x 1 x 1**, errors 1.0e-4, 9.0e-4, 2.5e-3 against the limit 5e-3.

*Tet4 and Tet10 on the Kuhn box are bounded by the 1-D chains.* On a Kuhn tetrahedron each edge steps along at most one grid
direction at a time, so its vertices lie on two grid planes `x_i`, `x_{i+1}`; the Tet4 interpolant of nodal values `F(x_node)`
is then the linear function of `x` alone that takes them, and the Tet10 interpolant (mid-edge nodes on `x_i`, `x_i + h/2`,
`x_{i+1}`) is the quadratic of `x` alone through them (unique in the 10-dimensional space). So the 1-D field `F(x)` of either
degree lies in the 3-D space, with the same strain energy and kinetic energy as in 1-D. By the min-max principle the 3-D
eigenvalues are at most the 1-D ones (`lambda_n,3D <= max over span(F_1..F_n) of the Rayleigh quotient = lambda_n,1D`), and
being conforming with consistent mass they are at least the continuum ones. Hence `0 <= e_n(Tet4) <= e_n(1-D linear)` and `0
<= e_n(Tet10) <= e_n(1-D quadratic)` at the same `n_x`, provided the lowest three 3-D modes are the axial ones (D8 checks it).
Unlike Hex8 the tetrahedral split is not symmetric across a section, so the 3-D modes are not exactly 1-D (the section
integral of a Kuhn hat function is not the 1-D hat: corner (0,0,0) of a cell lies in six tetrahedra, (0,1,0) in two). Meshes:
Tet4 **32 x 1 x 1** (bound 2.5e-3), Tet10 **8 x 1 x 1** (bound 6.2e-4).

**D6. Cantilever: Euler-Bernoulli, Timoshenko, and the registered reference.** A cantilever of length `L`, square section `b
x b`, `L / b = 20`, `A = b^2`, `I = b^4 / 12`, `r^2 = I / A`. Euler-Bernoulli: `EI w'''' = rho A omega^2 w`, `w(0) = w'(0) = 0`,
`w''(L) = w'''(L) = 0`; with `beta^4 = rho A omega^2 / (EI)` the general solution `cosh, sinh, cos, sin` and the four conditions
give `cos(beta L) cosh(beta L) + 1 = 0`, so `f_n^EB = (beta_n L)^2 sqrt(EI / (rho A L^4)) / (2 pi)` (arithmetic 1).

*Timoshenko* (Timoshenko, Phil. Mag. 41 (1921) 744, from memory; written here from the two balance laws). Deflection `w`,
section rotation `psi`, bending moment `EI psi'`, shear force `kappa G A (w' - psi)`:

`kappa G A (w'' - psi') + rho A omega^2 w = 0`, `EI psi'' + kappa G A (w' - psi) + rho I omega^2 psi = 0`,

clamped `w(0) = psi(0) = 0`, free `psi'(L) = 0`, `w'(L) - psi(L) = 0`. With `w, psi ~ e^{s x}` the determinant of the 2 x 2
system is `P Q s^4 + s^2 omega^2 (P rho I + Q rho A) + rho A omega^2 (rho I omega^2 - P) = 0`, `P = kappa G A`, `Q = EI`, a
quadratic in `s^2` with one positive root `alpha^2` and one negative `-beta^2` below the cut-off `omega^2 < P / (rho I)`. The
four solutions `w = cosh, sinh (alpha x), cos, sin (beta x)` carry `psi = g sinh, g cosh, -h sin, h cos` with `g = alpha + c /
alpha`, `h = beta - c / beta`, `c = rho A omega^2 / P`, and the shear angle `w' - psi` is `-(c/alpha) sinh, -(c/alpha) cosh,
-(c/beta) sin, (c/beta) cos` (written out, no cancellation). The 4 x 4 determinant of the boundary conditions vanishes at the
Timoshenko frequencies; its roots near the EB values were found by a scan and bisection (arithmetic 3), with `kappa = 5/6`
(the stub's value; the rectangle's `kappa` of Cowper, Trans. ASME J. Appl. Mech. 33 (1966) 335, from memory, `10 (1 + nu) /
(12 + 11 nu)`, changes the correction by 1.5 percent of itself).

*The first-order form, and why the stub's numbers are low.* By Rayleigh's quotient with the EB shape `w`, rotary inertia adds
the kinetic energy `rho I omega^2 int w'^2` and shear adds the compliance `int V^2 / (kappa G A)` with `V = EI w'''`, so

`omega / omega_EB - 1 ~= -(r^2 / 2) (J_1 + (E / (kappa G)) J_2)`, `J_1 = int w'^2 / int w^2`, `J_2 = int w'''^2 / int w''^2`.

For EB modes `w'''' = beta^4 w`, and integration by parts with the cantilever conditions (all boundary terms vanish:
`w = w' = 0` at the root, `w'' = w''' = 0` at the tip) gives `int w''^2 = beta^4 int w^2` and `int w'''^2 = beta^4 int w'^2`,
so `J_1 = J_2 = J_n`. For a simply supported beam `J_n = (n pi / L)^2`; for the cantilever `J_n L^2 = 4.6478, 32.417` against
`(beta_n L)^2 = 3.516, 22.034`. The stub used `(beta_n L)^2` (giving 0.151 and 0.946 percent); with `J_n` the first-order
values are 0.199 and 1.37 percent, and the exact Timoshenko roots 0.1988 and 1.3611 percent.

*Registered reference*: `f_n^ref = f_n^EB (1 - c_n)`, **`c_1 = 1.988e-3`, `c_2 = 1.3611e-2`** (exact Timoshenko, `kappa = 5/6`).
The checks a8 and a9 are `|f_n^h / f_n^ref - 1| <= 1e-2` and `<= 2.5e-2`. The acceptance also names the pure EB value as
the anchor of the percentages; the same limits against `f_n^EB` are registered beside them (T1b), so that either reading is
checked.

*Why Tet10 at 80 x 4 x 4 should meet it.* Cells are cubes of side `h = b / 4 = L / 80`. Along the beam the mode varies on
the scale `1 / beta_n`: `beta_2 h = 0.059`, and a quadratic interpolation of a smooth mode has an eigenvalue error of order
`(beta h)^4` (the 1-D quadratic chain at `k h = 0.2` gives 6e-4 in frequency, arithmetic 5): `<= 1e-4`. Across the section
the bending strain is linear (exact in Tet10) and the transverse shear strain of a square section is close to parabolic
(Tet10 strains are linear per element, four cells across): the shear energy, which is 1.04 percent of mode 2's frequency
(arithmetic 3), is then approximated to a few percent of itself, `<~ 0.05` percent. The 3-D clamp (all components fixed on
`xmin`, including the Poisson contraction) adds a local stiffening of order `nu^2 b / L`-ish, `<~ 0.3` percent, and the 3-D
section warping differs from Timoshenko's single `kappa` by a fraction of the shear correction. Estimate: `|f_1^h / f_1^ref -
1| <~ 0.4` percent, `|f_2^h / f_2^ref - 1| <~ 0.6` percent, against 1 and 2.5 percent. Conforming elements with consistent
mass overestimate the 3-D continuum frequencies, so the expected sign is positive.

*Symmetric Hex8 mesh* (a10): the `hex_box` of `n_y = n_z` cells over a square section is invariant under the rotation by 90
degrees about the beam axis through the section centre (it maps nodes to nodes, elements to elements, the clamp to itself,
and the Gauss rules to themselves); the bending modes in `y` and `z` form a pair exchanged by it, so their frequencies are
equal in exact arithmetic. The computed pair is a cluster resolved by Rayleigh-Ritz; its split is at the level of the residual
squared over the gap plus rounding, `<< 1e-10`. The Kuhn split is invariant only under the reflection `y <-> z`, which maps
the diagonal bending directions to themselves, so for Tet10 the pair is split by the discretisation; that split is recorded,
not limited.

**D7. Free box: why six frequencies fall below 1e-6 of the seventh.** The six rigid motions are in the space (D1) and `K R =
0` exactly in exact arithmetic, so `(K, M)` has a six-fold zero eigenvalue; the elastic spectrum starts at `lambda_7 > 0`.
With `sigma = 1e-4 rho` (D4) the subspace iteration resolves the rigid cluster at `mu = sigma`. Error of a computed rigid
eigenvalue `lambda = mu - sigma`: (i) convergence: the residual test gives a second-order eigenvalue error `(tau sigma)^2 /
lambda_7`, negligible; (ii) rounding: the factor and the products carry a backward error `delta A` of relative size `c eps`
per entry with random signs; seen by a smooth (rigid) vector the Rayleigh quotient moves by `~ c eps lambda_max / sqrt(N)` (ADR
0283's free chain: 5e-17 `lambda_max` at N = 100; ADR 0273: element rigid modes at 1.5e-16 `lambda_max`). With `lambda_max /
lambda_7 <= 7.4e3` (arithmetic 6, Tet10 with a margin), `|lambda_rigid| / lambda_7 <~ 1e-13`, a frequency ratio `<~ 3e-7`,
against 1e-6. The free box is kept coarse (3 x 2 x 2 cells) for that reason: the ratio grows like `(L / h)^2`. Frequencies of
negative computed eigenvalues are reported as `-sqrt(|lambda|) / (2 pi)` (`modal::frequency`), and the check uses `|f|`.

**D8. Identification of modes by shape** (in the tests, from the returned M-orthonormal modes and the rigid fields of D1 with
`p` = the centre of mass): the translational participation `Gamma_k = r_k^T M phi` (`r_k` the unit translation `e_k`) and the
effective mass fraction `e_k = Gamma_k^2 / m`; the axial kinetic fraction `a_x = phi_x^T M phi_x` (`phi_x` the `x` components of
`phi`; `M` is block-diagonal by components, so `a_x + a_y + a_z = 1`).

- Bar: a mode is axial if `a_x >= 0.99`; `f_1, f_2, f_3` are the first three axial modes in ascending order (expected: modes
  1, 2, 3, recorded).
- Cantilever: a mode is transverse bending if `e_y + e_z >= 0.02` (EB fractions 0.613, 0.188, 0.065 for the first three
  bending modes, arithmetic 2; torsion and axial modes have `e_y + e_z ~ 0` by the section's symmetry); bending modes in
  ascending order pair up as (1, 2) = first bending, (3, 4) = second bending; both members of each pair are checked.
- Free box: modes 1 to 6 are the rigid ones; extension F2 checks that they span the rigid motions.

**D9. API** (`crates/rcfd-fem/src/mass.rs`, `modal.rs`; no re-export from `lib.rs`):

- `mass::element_mass(element: &dyn Element, coords: &[Vec3], density: f64) -> Result<ElementMatrix>`,
  `mass::element_mass_with_rule(.., rule: &QuadratureRule)`; `mass::consistent(model) -> Result<CsrMatrix>`,
  `mass::consistent_with(model, threads: Option<usize>)`; `mass::rigid_motions(model, about: Vec3) -> [Vec<f64>; 6]`;
  `mass::mass_properties(model, m: &CsrMatrix) -> Result<MassProperties>`, `MassProperties { mass, centre, inertia: [[f64;
  3]; 3] }` (inertia about the centre).
- `modal::ModalOptions { modes, shift: Option<Shift>, tolerance, max_iterations, subspace_size, threads }`
  (`#[non_exhaustive]`, `ModalOptions::new(modes)` with the defaults of D3, D4, builder methods `shift`, `threads`);
  `modal::modal_analysis(model, &ModalOptions) -> Result<ModalResult>`, `modal::modal_analysis_with(model, extra:
  &[Prescribed], &ModalOptions)`; `ModalResult { eigenvalues, frequencies, modes (full vectors, one value per equation of
  the model's `DofMap`), residuals, iterations, shift, sturm: SturmCheck, unknowns, rigid_free: bool }`; `modal::frequency(lambda)
  -> f64`.
- Refusals: `modes == 0` or `modes >= unknowns` (`InvalidValue { key: "modes" }`), a non-zero prescribed value
  (`InvalidValue { key: "constraint" }`), a massless free unknown (`InvalidValue { key: "density" }`), every error of the
  eigensolver (`Solver { solver: "lowest_eigenpairs", reason }`), and the errors of the model validation, the constraint
  lists and the assembly as they are.

## Set-ups

New test file `crates/rcfd-fem/tests/sb_modal.rs`; seed `0xF383_0284` (SplitMix64 as in ADR 0273 D6, tag per set-up). Steel
`E = 2e11 Pa`, `rho = 7850 kg/m^3`; `nu = 0.3` except the bar (`nu = 0`). All runs in release.

- **MB** (mass box): origin (0.1, -0.2, 0.3), lengths (1.2, 1.0, 0.8), 4 x 3 x 3 cells; interior grid nodes moved by `0.1 h_k
  (2U - 1)` per coordinate (tags 10, 11, 12 for Tet4, Hex8, Tet10), Tet10 by `upgrade_to_tet10` of the moved Tet4 box
  (straight edges); `nu = 0.3`. Exact for all three rules (D1).
- **FB** (free box): origin (0.2, 0.1, -0.1), lengths (1.0, 0.6, 0.4), 3 x 2 x 2 cells, interior nodes moved as MB (tags 20,
  21, 22), no constraint; `modes = 7`; default options (D4: `Shift::Auto`).
- **BAR**: `L = 1`, `b = 0.1`, origin 0, `nu = 0`; `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`; `modes = 4`;
  Hex8 `n x 1 x 1`, `n = 8, 16, 32`; Tet4 32 x 1 x 1; Tet10 8 x 1 x 1 (upgrade of the Tet4 box). `c = sqrt(E / rho)`.
- **CANT**: `L = 1`, `b = 0.05`, origin 0, `nu = 0.3`; all three components 0 on `xmin`; `modes = 6`. Tet10 80 x 4 x 4 (the
  registered run, heavy, `#[ignore]`), Tet10 40 x 2 x 2 (fast covering test) and 20 x 1 x 1 (recorded in the heavy test);
  Hex8 40 x 2 x 2 (symmetric mesh, fast).
- Norms: M-orthonormality `max_ij |phi_i^T M phi_j - delta_ij|` with the full `M` assembled in the test by
  `mass::consistent`; frequency errors relative.

## Registered checks

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | a1, a2 | MB, each kind: `|R_t^T M R_t - m I|` entries `/ m` (the translation block, total mass and cross terms) | `<= 1e-12` | D1 exact; rounding `~ eps sqrt(nnz)`: `~1e-14` |
| C2 | a3 | MB, each kind: centre from the translation-rotation block about the origin, `max_k |x_c,h - x_c| / |x_c|` | `<= 1e-12` | as C1 |
| C3 | a4 | MB, each kind: rotation block about the origin against `I_O`, `max_ij |I_h - I_O| / max_ij |I_O|`; and `mass_properties` (inertia about the centre) against `I_O - m(|x_c|^2 I - x_c x_c^T)`, `/ max |I_c|`, and its mass and centre | `<= 1e-12` each | as C1; the parallel-axis subtraction in the closed form loses a factor `max|I_O| / max|I_c| ~ 6` |
| C4 | a1 (extension) | MB, each kind: `M` exactly symmetric; `M` bit-identical for 1 and 8 threads | `= 0`; equal | D1 mirrored; ADR 0275 D2 |
| N1 | negative control | MB Tet4 with `tet_1` (degree 1) in `element_mass_with_rule`: C3 error about the origin | `> 1e-6` | one-point rule error of `int x^2` per tetrahedron `~ V h^2 / 80`, relative `~1e-3` |
| F1 | a5, a11 | FB, each kind: `max_{i<=6} |f_i| / f_7`; M-orthonormality | `<= 1e-6`; `<= 1e-9` | D7: `<~ 3e-7`; ADR 0283 measured `~1e-15` |
| F2 | extension | FB, each kind: for `i <= 6`, the M-norm of `phi_i - P_R phi_i` (`P_R` the M-orthogonal projector on the rigid motions) | `<= 1e-8` | eigenvector error `<~ tau sigma / lambda_7` plus rounding `eps lambda_max / lambda_7`: `<~ 1e-12` |
| F3 | D4 (extension) | FB Hex8: the shift taken is `> 0` and `< 0.1 lambda_7`; record `sigma / lambda_7`, iterations | as stated | D4: `~2e-2` |
| N2 | negative control | FB Hex8 with `shift = Some(Shift::Value(0.0))`: an error (`Solver`, the shift refused) | `Err` | ADR 0283 E10 control |
| B1 | a6, a11 | BAR Hex8 32: `|f_n / f_n^exact - 1|`, `n = 1, 2, 3` (first three axial modes); M-orthonormality | `<= 5e-3`; `<= 1e-9` | arithmetic 4: 1.0e-4, 9.0e-4, 2.5e-3 |
| B2 | a7 | BAR Hex8 8, 16, 32: observed order of the error of `f_1`, `log2(e_16 / e_32)` (registered) and `log2(e_8 / e_16)` | `>= 1.8` both | 2.0001, 2.0005 |
| B3 | extension (D5) | BAR Hex8 8, 16, 32: `f_n` against the discrete closed form `sqrt(lambda_h) / (2 pi)` | `<= 1e-9` relative | D5 exact; rounding `eps lambda_max / lambda_1 ~ 2e-12` |
| B4 | a6 (extension) | BAR Tet4 32 and Tet10 8: `|f_n / f_n^exact - 1|`, `n = 1, 2, 3`; and `f_n^h >= f_n^exact (1 - 1e-12)` and `e_n(Tet4) <= e_n(Hex8 32) + 1e-9`; M-orthonormality | `<= 5e-3`; as stated; `<= 1e-9` | D5 bounds: Tet4 `<= 2.5e-3`, Tet10 `<= 6.2e-4` |
| B5 | D8 (extension) | BAR, every mesh: the first three modes are axial (`a_x >= 0.99`) and they are modes 1, 2, 3 | as stated | D5: next family at `>= 2 f_3` |
| T1 | a8, a9, a11 | CANT Tet10 80 x 4 x 4 (heavy, ignored record): both members of the first bending pair against `f_1^ref`, of the second against `f_2^ref`; M-orthonormality | `<= 1e-2`; `<= 2.5e-2`; `<= 1e-9` | D6: `<~ 4e-3`, `<~ 6e-3` |
| T1b | a8, a9 (the other reading) | the same against the pure EB values `f_n^EB` | `<= 1e-2`; `<= 2.5e-2` | D6: T1 values minus `c_n` (0.2, 1.36 percent) |
| T2 | covering (fast) | CANT Tet10 40 x 2 x 2: the same pairs against `f_n^ref`; M-orthonormality; record the Tet10 pair splits of both meshes and the 20 x 1 x 1 result | `<= 3e-2`; `<= 6e-2`; `<= 1e-9` | D6 with `h` doubled: errors `<~ 4` times T1's |
| S1 | a10, a11 | CANT Hex8 40 x 2 x 2: `|f_a - f_b| / f_a` of the first bending pair; the same for the second pair (extension); M-orthonormality; record the Hex8 frequencies against `f^ref` (locking, not limited) | `<= 1e-6`; `<= 1e-6`; `<= 1e-9` | D6: `<< 1e-10` |
| D1 | ADR 0270 2.2 (extension) | BAR Hex8 16 with `threads = 1` and `8`: eigenvalues, modes, iterations | bit for bit | ADR 0275 D2, ADR 0283 E15 |
| X1 | refusals (extension) | `modes = 0` and `modes >= unknowns` (`InvalidValue "modes"`); BAR with a non-zero `u_x` on `xmin` (`InvalidValue "constraint"`); a model whose element set has density 0 (`InvalidValue "density"`, naming a node) | as stated | D2, D9 |
| E1 | existing tests | `cargo test -p rcfd-fem --release`: every existing test passes, no existing file edited except the two stubs | as stated | D9 |

**Negative controls**: N1 (a rule of too low a degree is seen by C3), N2 (the free box needs the shift).

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f383-scratch`, own target directory; each applied alone
by exact text replacement, the named tests run once, the file restored and compared by SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | element mass weight `w` instead of `w det J` | C1 |
| M2 | element mass placed on every component pair `(3a + i, 3b + j)` | C1 |
| M3 | density not applied (`rho = 1`) | C1 |
| M4 | Hex8 mass with `gauss_2` | C3 (Hex8, moved nodes) |
| M5 | `frequency` without the `2 pi` | B1 |
| M6 | the shift rule always `Shift::Value(0.0)` | F1 |
| M7 | the prescribed entries of a mode set to 1 instead of 0 | B1 (M-orthonormality with the full `M`) |
| M8 | Tet10 mass with `tet_4` (degree 2) | B4 (Tet10) |
| M9 | the shift rule always `Shift::Auto` | B1 or T2 (rate, or a shifted spectrum; record what happens) |

**Decision rule.** F-383 is proposed **done** (pending the maintainer's acceptance) if C1 to C3 (three kinds), F1 (three
kinds), B1, B2, T1, S1 and E1 are met (every acceptance clause, with a11 on every modal run), and M1 to M8 each fail a
registered test (M9 is informative: the constrained cases may still converge with `Auto`). A miss among the extensions (C4,
F2, F3, B3 to B5, T1b, T2, D1, X1) is a `*_known_miss` record and a follow-up item and does not alone block done. A miss or
an unrun check among the done set, or an uncaught mutation M1 to M8 after the test is checked to cover it: **partial**, with
the miss named. No modal result at all: **todo**.

## Limits (what is not covered)

- Consistent mass only; no lumped (row-sum or HRZ) mass, which explicit dynamics would need (intermediate tier).
- Mass on curved Tet10 is integrated by `tet_14`, not exactly (D1); the checks use straight-sided Tet10.
- The eigensolver is ADR 0283's subspace iteration: a few tens of modes on up to ~1e5 unknowns; no Lanczos, no spectrum
  slicing, no shift into the interior; a free body uses one automatic shift.
- Zero prescribed displacements only; no multi-point constraints, no cyclic symmetry, no pre-stress, spin softening, stress
  stiffening or Campbell diagram (intermediate tier).
- No damping, no harmonic or transient response.
- Mode identification by participation is in the tests, not a library feature; the library returns participation-ready
  rigid fields (`rigid_motions`) only.
- The cantilever reference is Timoshenko's beam with `kappa = 5/6`; the 3-D solid differs from it by the root and warping
  effects estimated in D6, which the limits must cover.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The mass rules of ADR 0273 (`tet_4`, `gauss_3`, `tet_14`) are kept (ADR 0273 remaining question 2, option (a)).
2. A massless free unknown is refused by the modal analysis; `mass::consistent` itself accepts density 0 (ADR 0271 remaining
   question 3, option (a)).
3. The shift rule of D4: `sigma = 0` when the constraints hold every rigid motion, `Shift::Auto` otherwise (ADR 0283 remaining
   question 1, option (a), refined for constrained slender bodies).
4. "Slenderness 20" read as `L / b = 20`; the correction registered is the exact Timoshenko one (0.199 and 1.361 percent), not
   the stub's 0.15 and 0.9 percent, for the reason in D6; both the corrected and the pure EB references are checked.
5. A non-zero prescribed displacement is refused by the modal analysis (D2).

## Amendment 1 (2026-11-12, written after the first run of the fast tests and before any corrected run)

**What happened.** The registered run of the fast tests (`cargo test -p rcfd-fem --release --test sb_modal -- --nocapture
--test-threads=1`, log `f383-scratch/run1.log`, run once) passed C1 to C4, N1, F1 to F3, N2, B1 to B5, D1 and X1, and the two
cantilever tests did not reach their checks: `modal_analysis` returned `Solver { lowest_eigenpairs: NotConverged }` after 200
iterations. Residuals of the six wanted pairs at the cap: Hex8 40 x 2 x 2 (S1) 2.60e-10, 1.96e-10, 7.4e-12, 5.9e-12, 1.0e-12,
7.6e-13; Tet10 40 x 2 x 2 (T2) 2.66e-9, 2.51e-9, 7.2e-11, 6.3e-11, 9.1e-12, 8.2e-12, against the tolerance 1e-10. T1 was not
run (ignored).

**Diagnosis.** Only the first bending pair misses, by a factor 2.6 (Hex8) and 27 (Tet10), and the other pairs are far below the
tolerance: the iteration has converged and the residual of the lowest pair sits at its rounding floor. ADR 0283 D6 states that
floor as `rho_i ~ c eps ||A|| / mu_i` (the backward error of the solve seen relative to `||A x_i|| ~ mu_i ||M x_i||`); with
`sigma = 0` it is `c eps lambda_max / lambda_1`. The registration did not estimate it for the slender cantilever, where the
ratio is large: Hex8 with cubes of `h = 0.025` has `lambda_max <= 36 (3 lambda_L + 2 mu) / (rho h^2) = 3.7e12` (arithmetic 6's
bound) and `lambda_1 = (2 pi f_1)^2 ~ 6.6e4` (EB), a ratio `~6e7`, so `eps x ratio = 1.2e-8` and the measured 2.6e-10 means
`c ~ 0.02`; Tet10 on the same cells has a `lambda_max` about ten times larger (arithmetic 6's factor), and its floor is ten
times higher (2.7e-9), as the formula says. The bar and the free box have ratios below 1e5 and converged under 1e-10. This is
a defect of the set-up (the tolerance passed to the eigensolver), not of a limit: no frequency, split or orthonormality was
measured.

**Correction.** `ModalOptions` gains the builder `tolerance(t)` (new code; the default stays `1e-10`, with which the passed
runs were made). The corrected tests `s1_amended_symmetric_hex8_cantilever_bending_planes`, `t2_amended_tet10_cantilever_covering`
and `t1_amended_tet10_cantilever_heavy` (ignored, heavy) make the same checks with the same limits (S1, T2, T1, T1b) and pass
`tolerance = 1e-7` to the eigensolver. The value: the floor scales with `lambda_max / lambda_1`, `lambda_max ~ h^-2` and
`lambda_1` fixed, so Tet10 80 x 4 x 4 should sit at `4 x 2.7e-9 = 1.1e-8`; `1e-7` leaves a margin of 9 there, 38 for Tet10 40
x 2 x 2 and 380 for Hex8. Effect on the checked quantities: the eigenvalue error of a converged pair is second order,
`<~ tau^2 lambda_i / (lambda_j - lambda_i)` relative (ADR 0283 D6): `<~ 1e-14 / 37` for the first pair, far below the
frequency limits (1e-2) and the split limit (1e-6); the split of a cluster resolved by Rayleigh-Ritz is of the same order;
M-orthonormality comes from the Rayleigh-Ritz step, not from `tau`; the Sturm check still runs. The first runs stay as
ignored records: `s1_symmetric_hex8_cantilever_bending_planes` and `t2_tet10_cantilever_covering` (`#[ignore]`, their result
the `NotConverged` above), and `t1_tet10_cantilever_heavy`, not run (by the same floor it would stop near 1.1e-8). The shared
helper `run_cantilever` keeps its behaviour (it calls the new `run_cantilever_with` with `ModalOptions::new(6)`). The library
default for users is a remaining question for the addendum (a slender model fails at `1e-10`).

## Amendment 2 (2026-11-12, written after the mutation runs and before any run of the added check)

**What happened.** After the corrected runs (all registered checks met, addendum) the mutations were run on a `git archive`
copy of the tree at 3338f958 (`f383-scratch/mutate.py`, `mutations.log`). M1 to M7 were caught. **M8** (Tet10 mass with
`tet_4`, degree 2) **survived** B4: a diagnostic run of the mutant (not a registered result) gave the Tet10 bar errors 1.11e-6,
8.84e-5, 6.56e-4 against 6.19e-7, 4.94e-5, 3.70e-4 with `tet_14`, all within 5e-3 and above the lower bound. The registration
expected the under-integrated mass to be singular; it is not (the assembled `M` stays positive definite), and its error is of
the order of the discretisation error, so no frequency limit of the acceptance can see it. C1 to C3 cannot either: rigid
motions need only degree 2. M9 (informative) also survived: `Shift::Auto` accepts `sigma = 0` for the bar and the cantilevers.

**Diagnosis.** No registered check tests the element mass matrix itself beyond its rigid-body moments; a mass rule of too low
a degree for the non-rigid part is invisible to the checks as registered. By the decision rule this is an uncaught mutation:
the result is **partial** whatever the added check gives.

**Added check C5** (new test `c5_element_mass_closed_forms`, run once after this amendment is committed, and M8 re-run once
against it):

- Tet4 and Tet10 on one straight-sided tetrahedron with vertices (0.1, 0.2, 0.0), (1.3, 0.1, 0.2), (0.2, 1.1, -0.1), (0.3, 0.4,
  0.9) (mid-edge nodes at the midpoints, node order of ADR 0271): `element_mass` against the exact `rho int N_a N_b dV`, written
  in barycentric coordinates (Tet4 `N_a = L_a`; Tet10 `L_i (2 L_i - 1)` at the corners and `4 L_i L_j` on the edges
  `TET_EDGES`), expanded into monomials and integrated by `int_T L_1^a L_2^b L_3^c L_4^d dV = 6 V a! b! c! d! / (a + b + c + d +
  3)!` (the reference-tetrahedron integral of `xi^a eta^b zeta^c (1 - xi - eta - zeta)^d` by three Beta integrals, times the
  Jacobian `6 V` of the affine map; derived, not recalled as a table);
- Hex8 on the rectangular element `[0.1, 0.5] x [-0.2, 0.1] x [0.3, 0.55]`: `M_ab = rho prod_k (h_k / 6)(2 if the reference
  signs of `a` and `b` agree in direction `k`, else 1)` (the tensor product of the 1-D linear mass);
- on the three diagonal component blocks, zero on the off-diagonal ones; limit `max |M_h - M| / max |M| <= 1e-13` (rounding of
  at most 27 terms per entry, `~1e-16`). Expected under M8: the degree-2 rule misses the degree-4 integrand by `O(1e-2)` of
  the entries (ADR 0273 N1: `tet_4` misses degree 3 by 3.6e-4 of a monomial; the quartic products of `L` are larger).

C5 is an extension check (it does not enter the done set), recorded with the mutation that motivated it.

## Addendum 1: results (2026-11-12)

Code: `crates/rcfd-fem/src/mass.rs` and `modal.rs` (the two stubs filled), `crates/rcfd-fem/tests/sb_modal.rs` (new); no other
file of the crate or the workspace changed (`git diff adede15c --stat`: these three and this ADR). Commits: 56a14dd6
(registration), 4e77518d (amendment 1), 98e08497 (code and tests as in run 1), 3338f958 (amended tests, `tolerance` builder,
rustfmt), ef712b1e (amendment 2), 20c905e3 (C5), and this addendum. Runs, each once, release, `--nocapture --test-threads=1`,
logs in `w49i-work/f383-scratch`: run 1, the fast tests (`run1.log`); run 2, the amended fast tests S1, T2 (`run2_amended_fast.log`);
run 3, the heavy T1 (`run3_t1_heavy.log`, 2.9 s); run 4, C5 (`run4_c5.log`). Confirmation runs of the whole crate test set
(`cargo test -p rcfd-fem --release`, `crate_tests.log` after amendment 1, `crate_tests_final.log` on the final tree): every
test passes (`sb_elements` 8, `sb_loads` 7, `sb_model` 11, `sb_static` 9 unedited; `sb_modal` 8 passed, 4 ignored); no
registered number changed between them.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| C1 translation block / m (Tet4, Hex8, Tet10) | `<= 1e-12` | 1.2e-16, 2.4e-16, 6.0e-16 | yes |
| C2 centre and translation-rotation block | `<= 1e-12` | 2.3e-16, 3.5e-16, 5.4e-16 | yes |
| C3 inertia about the origin; `mass_properties` mass, centre, inertia about the centre | `<= 1e-12` each | 4.2e-16, 3.1e-16, 7.3e-16; mass `<= 6.0e-16`, centre `<= 8.6e-16`, inertia 3.3e-15, 3.4e-15, 3.9e-15 | yes |
| C4 `max |M - M^T|`; 1 and 8 threads | `= 0`; equal | 0 (all kinds); bit for bit | yes |
| C5 (amendment 2) element mass against closed forms (Tet4, Tet10, Hex8) | `<= 1e-13` | 4.5e-16, 8.0e-16, 1.4e-15 | yes |
| N1 Tet4 with `tet_1`: C3 error | `> 1e-6` | 7.3e-3 | holds |
| F1 `max |f_1..6| / f_7` (Tet4, Hex8, Tet10); M-orthonormality | `<= 1e-6`; `<= 1e-9` | 3.2e-8, 3.4e-8, 6.5e-8 (`f_7` = 2171.7, 1492.5, 1354.4 Hz); 8.9e-16, 4.4e-16, 4.4e-16 | yes |
| F2 non-rigid part of modes 1 to 6 | `<= 1e-8` | 6.8e-16, 8.1e-16, 2.5e-15 | yes |
| F3 Hex8 `0 < sigma < 0.1 lambda_7` | as stated | `sigma / lambda_7 = 2.77e-3` (12 iterations; Tet4 16, Tet10 14; Sturm 7 = 7 in each) | yes |
| N2 Hex8 `sigma = 0` | `Err` | refused: pivot -7.6e-5 at row 14 fails the positive test | holds |
| B1 Hex8 32: `f_1, f_2, f_3`; M-orthonormality | `<= 5e-3`; `<= 1e-9` | 1.004e-4, 9.038e-4, 2.512e-3 (as arithmetic 4); 4.4e-16 | yes |
| B2 observed order of `f_1`: 16 -> 32; 8 -> 16 | `>= 1.8` | 2.0001; 2.0005 | yes |
| B3 Hex8 8, 16, 32 against the discrete closed form | `<= 1e-9` | `<= 2.2e-14` | yes |
| B4 Tet4 32; Tet10 8: errors; lower bound; `e(Tet4) - e(Hex8)` | `<= 5e-3`; `>= -1e-12`; `<= 1e-9` | Tet4 9.09e-5, 7.89e-4, 2.04e-3; Tet10 6.19e-7, 4.94e-5, 3.70e-4; all positive; -9.5e-6, -1.15e-4, -4.76e-4 | yes |
| B5 the first three modes axial (every mesh) | as stated | modes 1 to 4 all axial (`a_x >= 0.99988`) | yes |
| T1 Tet10 80 x 4 x 4 (amended, heavy record): first pair against `f_1^ref`; second pair against `f_2^ref`; M-orthonormality | `<= 1e-2`; `<= 2.5e-2`; `<= 1e-9` | 2.68e-3, 2.72e-3; 2.89e-3, 2.94e-3; 6.0e-15 (38 880 unknowns, 7 iterations, 2.7 s) | yes |
| T1b the same against `f_n^EB` | `<= 1e-2`; `<= 2.5e-2` | 6.8e-4, 7.2e-4; 1.076e-2, 1.071e-2 | yes |
| T2 Tet10 40 x 2 x 2 (amended): against `f^ref`; M-orthonormality | `<= 3e-2`; `<= 6e-2`; `<= 1e-9` | 3.60e-3, 3.81e-3; 3.91e-3, 4.21e-3; 4.2e-15 | yes |
| S1 Hex8 40 x 2 x 2 (amended): pair splits; M-orthonormality | `<= 1e-6`; `<= 1e-6`; `<= 1e-9` | 3.98e-11; 1.02e-12; 1.1e-15 | yes |
| S1, T2 first runs (records) | | `NotConverged` at the residual floor (amendment 1) | not reached |
| D1 Hex8 bar 16, 1 and 8 threads | bit for bit | equal (27 iterations each) | yes |
| X1 refusals | as stated | `modes` 0 and 10 000 of 20; non-zero `u_x` names node 0; density 0 names node 0 component 1 | yes |
| E1 existing tests | pass unedited | pass; no existing file edited | yes |

Records. Tet10 cantilever against `f^ref` (first pair, second pair, the larger member): 20 x 1 x 1: 8.65e-3, 1.09e-2; 40 x 2 x
2: 3.81e-3, 4.21e-3; 80 x 4 x 4: 2.72e-3, 2.94e-3. The error halves from 20 to 40 and then levels off near 0.25 percent: what
remains is the 3-D solid against Timoshenko's beam (clamp with Poisson restraint, section warping), estimated at `<~ 0.3`
percent in D6. The Kuhn split separates the bending pair by 1.1e-3, 2.1e-4, 4.1e-5 (first pair) as `h` halves; the modes are
the diagonal bending directions (`e_y = e_z = 0.306`, the EB fraction 0.613 shared), as D6 expected. Hex8 40 x 2 x 2 (full
integration) is 7.0 and 7.2 percent stiff against `f^ref` (shear locking; not limited, B-bar is ADR 0293). Measured
effective mass fractions of the bending modes (Tet10 80 x 4 x 4): 0.612, 0.189, 0.0655 against EB 0.613, 0.188, 0.0647. The
residual floor of the lowest cantilever pair (amendment 1) is confirmed by the amended runs: they stopped after 7 iterations
with residuals up to 2.1e-8 (Hex8), 4.6e-8 (Tet10 40) and 9.7e-8 (Tet10 80) below the tolerance 1e-7.

**Mutations** (`f383-scratch/mutate.py`, `mutations.log`; `git archive` copies of 3338f958 and, for the re-run after amendment
2, of 20c905e3, own target directory, each mutation alone by exact text replacement, file restored and compared by SHA-256):

| Id | Caught by |
| --- | --- |
| M1 weight `w` without `det J` | C1 (36.5 relative, Tet4), C2, C3 |
| M2 every component pair | C1 (1.0), C2, C3 |
| M3 `rho = 1` | C1 (0.9999), C2, C3 |
| M4 Hex8 mass with `gauss_2` | C3 Hex8 (3.3e-8 about the origin, 1.9e-7 about the centre) |
| M5 frequency without `2 pi` | B3, B1 (factor 2 pi) |
| M6 shift always `Value(0)` | F1: the free box's modal analysis refused (`unwrap` of the error) |
| M7 prescribed entries 1 | B1 M-orthonormality (56) |
| M8 Tet10 mass with `tet_4` | **survived** B4 (the registered test); caught by C5 after amendment 2 (2.7e-1) |
| M9 shift always `Auto` (informative) | survived B1 to B5, S1, T2: `Auto` accepts `sigma = 0` on these constrained models |

M1 and M3 were run a second time on the 20c905e3 copy together with M8 (same verdicts). The diagnostic run of the M8 mutant
quoted in amendment 2 is not a registered result. `cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem
--all-targets -- -D warnings`: clean.

**Deviations.** Amendment 1 (the cantilever tolerance, a set-up defect: the residual floor of the eigensolver was not estimated)
and amendment 2 (check C5 after the surviving mutation M8). The heavy T1 took 2.9 s, far below the two minutes of the house
rule, but stays `#[ignore]` as registered (T2 covers its code path). Nothing else was skipped. The whole workspace was not
tested (the lead runs the gate); no shared solver file was changed.

**Decision by the registered rule: partial.** Every clause of the acceptance is met (C1 to C3 for the three kinds, F1 for
the three kinds, B1, B2, T1 with T1b, S1, M-orthonormality on every run, E1), and M1 to M7 are caught; but M8 survived every
registered test and is caught only by C5, added by amendment 2 after the mutation run. The rule makes that partial (the same
situation as ADR 0275's M10). What would close it: the maintainer's acceptance of amendment 2's C5 as the check that sees the
mass rule.

**Remaining questions.**

1. Status. Options: (a) partial as the rule says, closed by accepting amendment 2; (b) done now, since the acceptance is fully
   met and C5 catches M8. Recommendation: (a), as for F-374.
2. The default residual tolerance of `ModalOptions` (1e-10) fails on slender models (amendment 1: the floor `c eps
   lambda_max / lambda_1`, 2.7e-9 for the 40 x 2 x 2 cantilever; a fan blade is slender). Options: (a) keep 1e-10 and the
   `tolerance` builder, documented; (b) make the modal default 1e-7, as the cantilever runs used; (c) a scale-aware test in
   `rcfd_linalg::eigen` that measures the residual against an estimate of `||A|| ||x||` instead of `||A x||`, so that the
   floor no longer depends on the spectrum (a follow-up item below). Recommendation: (c), with (a) until it exists; (b) changes
   the results of runs already registered here.
3. The heavy T1 runs in 3 s. Options: (a) keep it ignored as registered; (b) let the lead make it a regular test. Recommendation:
   (a): it allocates about 150 MB of envelope, and T2 covers the path.

## For the lead: roadmap entries

F-383: proposed status **partial**. Note:

> Structural solver, basic tier (plan ADR 0270). ADR 0284, 2026-11-12: rcfd_fem::mass::consistent (consistent mass by the mass rules of ADR 0273, tet_4, gauss_3, tet_14, through assemble_with; exactly symmetric, bit-identical for 1 and 8 threads), element_mass, rigid_motions and mass_properties from r^T M r; rcfd_fem::modal::modal_analysis (zero constraints by symmetric elimination, lowest_eigenpairs of ADR 0283 with sigma = 0 when the constraints hold every rigid motion and Shift::Auto for a free body; massless unknowns and non-zero constraints refused; modes M-orthonormal over the full DofMap). Box with moved interior nodes: mass, centre and inertia from rigid motions within 7.3e-16 (Tet4, Hex8, Tet10; limit 1e-12), about the centre within 3.9e-15. Free box: six rigid frequencies at most 6.5e-8 of the seventh (limit 1e-6). Fixed-free bar, nu = 0, Hex8 32 x 1 x 1: f_1..3 within 1.0e-4, 9.0e-4, 2.5e-3 (limit 5e-3), equal to the discrete 1-D chain within 2.2e-14, observed order of f_1 2.0001; Tet4 and Tet10 below their 1-D chain bounds. Cantilever L/b = 20, Tet10 80 x 4 x 4: first and second bending within 2.7e-3 and 2.9e-3 of Euler-Bernoulli less the exact Timoshenko correction (0.199 and 1.361 percent, kappa 5/6; the draft's 0.15 and 0.9 used beta_n^2 for the cantilever's J_n), within 7.2e-4 and 1.08e-2 of pure Euler-Bernoulli (limits 1 and 2.5 percent); symmetric Hex8 mesh: bending planes equal within 4.0e-11 (limit 1e-6); mode shapes M-orthonormal within 6.0e-15 (limit 1e-9). Amendment 1: slender models need ModalOptions::tolerance(1e-7) (residual floor of the eigensolver, c eps lambda_max/lambda_1); amendment 2: check C5 of the element mass matrices against closed forms added after mutation M8 (Tet10 mass with a degree-2 rule) survived the registered checks; eight of nine mutations caught by the registered tests, M8 by C5. Partial by the rule until the maintainer accepts amendment 2.

F-387 (ADR 0288), sentence to append to its note:

> ADR 0284 (2026-11-12): rcfd_fem::modal::modal_analysis gives the modes at rest; a slender blade needs ModalOptions::new(m).tolerance(1e-7) or so (the residual floor of lowest_eigenpairs is about c eps lambda_max/lambda_1, 2.7e-9 for a 40 x 2 x 2 Tet10 cantilever of L/b = 20), and modes are best identified by effective mass fractions from mass::rigid_motions, as sb_modal does.

Follow-up item (new; number by the lead): title "Scale-aware convergence test of the subspace eigensolver", deps F-382, F-383. Acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd_linalg::eigen::lowest_eigenpairs gains a residual test relative to an estimate of ||A|| ||x|| (for example from the factorisation or a few power steps), selectable without changing the existing default; the 40 x 2 x 2 Tet10 cantilever of ADR 0284 then converges with the scale-aware tolerance 1e-12 in at most 20 iterations, its first two bending frequencies agree with the runs of ADR 0284 amendment 1 within 1e-10 relative, and every check of ADR 0283 still passes with the default unchanged.

## The planned stub, as written

> # ADR 0284: Mass matrix and linear modal analysis of a solid model
>
> Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
> registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
> the acceptance draft that the registration turns into checks.
>
> Work package SB-14 of the structural solver, basic tier. Size M.
>
> ## Goal
>
> Consistent mass matrix of the three elements from the density, assembled like the stiffness; the lowest natural frequencies and
> mass-normalised mode shapes of a constrained or free model with the eigensolver of ADR 0283. This is the modal analysis of fan
> breakdown item E5 for a blade at rest (no rotation effects).
>
> ## Dependencies
>
> ADR 0275, ADR 0283; existing roadmap items: none.
>
> ## Acceptance (draft, in the roadmap's form)
>
> Every limit not stated here is registered in the ADR before the run. Consistent mass matrix for Tet4, Hex8 and Tet10: for a box,
> the total mass, the centre of mass and the inertia tensor obtained from rigid motions r^T M r equal the closed forms within
> 1e-12. Modal analysis: a free box has six frequencies below 1e-6 of the seventh; a fixed-free bar with nu = 0 has f_n = (2n - 1)
> sqrt(E/rho)/(4 L), n = 1, 2, 3, within 0.5 percent on a registered mesh, and the error of f_1 falls at an observed order >= 1.8
> with Hex8; a cantilever of slenderness 20 and square section has its first bending frequency within 1 percent of (1.8751)^2
> sqrt(E I/(rho A L^4))/(2 pi) and its second (4.6941) within 2.5 percent with Tet10, the Euler-Bernoulli value less the shear and
> rotary-inertia correction estimated in the registration (0.15 and 0.9 percent); on a symmetric Hex8 mesh the two bending planes
> give the same frequency within 1e-6 relative; mode shapes are M-orthonormal within 1e-9.
>
> ## Reference to check against
>
> Closed forms: the wave equation of a bar (`f_n = (2n - 1) c/(4L)`, exact for a 3-D bar with nu = 0); Euler-Bernoulli cantilever,
> roots of `cos x cosh x + 1 = 0` (1.8751, 4.6941, 7.8548: to be recomputed in the registration, one line of arithmetic); the
> correction for shear and rotary inertia from Timoshenko beam theory (Timoshenko, Phil. Mag. 41 (1921) 744; from memory).
> Rigid-body inertia of a box.
>
> ## Files
>
> `crates/rcfd-fem/src/{mass.rs, modal.rs}`, `crates/rcfd-fem/tests/sb_modal.rs`.
>
> ## What could make it miss
>
> Size: 3-D solid elements on a slender beam need many unknowns for the second mode within the direct solver's reach. Torsional
> and bending modes interleave, so modes must be identified by shape, not by index. Rotating-blade effects (spin softening, stress
> stiffening, Campbell diagram) are intermediate.
