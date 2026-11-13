# ADR 0390: F-395 registration: geometric (initial-stress) stiffness of solid elements, pre-stressed modes and linear buckling

Status: **REGISTERED 2026-11-13, written before any code of F-395 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-395 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done by hand and with a short scratch script of the closed forms (`w49i-work/f395-scratch/arith.py`,
disclosed under "Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: [ADR 0298](0298-geometric-stiffness-prestressed-modal-and-linear-buckling.md) (SI-8, the stub with the acceptance
draft), plan [ADR 0290](0290-structural-solver-intermediate-tier-plan.md) (section 2.3: `geometric::prestressed_modes`,
`buckling::factors`, the `Element::geometric_stiffness(stress)` entry point and the generic assembly hook; section 2.4 item 4: one
function appended to `rcfd-linalg` `eigen.rs`). Roadmap item F-395 (deps F-388, F-374, F-376, F-382, F-383). Size M. Branch
`f395-geometric-stiffness` from main ae92ee27, worktree `w49i-work/f395`, scratch `w49i-work/f395-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
only edit of an existing test is the one the maintainer's standing decision of 2026-11-13 permits (D9). `modal.rs`, `mpc.rs` and
`assemble.rs` (another package, F-399, works in parallel) are not edited; their public functions are called, read only.

## What F-395 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. K_sigma is symmetric within 1e-13 of its norm, gives no
> force for a rigid translation within 1e-12, and for a uniform uniaxial stress s and a linear displacement field w gives x^T
> K_sigma x = s V |grad w . e|^2 within 1e-12. rcfd-linalg: for K = tridiagonal (2, -1), n = 200, and K_sigma = -I the ten lowest
> buckling factors equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10; a K_sigma with no negative direction returns no factor and says
> so. Pre-stressed modes: a beam on knife-edge supports at its neutral axis (slenderness 20, nu = 0) under an axial tension stress
> s has (omega_n^2 - omega_n,0^2) = (n pi/L)^2 s/rho within 1 percent for n = 1 and 2, and under compression the frequency falls
> by the same amount. Buckling: the same beam in compression has the critical load pi^2 E I/L^2 reduced by the shear term (0.6
> percent at this slenderness, registered) within 1 percent, the second factor four times the first within 2 percent; a
> clamped-free column pi^2 E I/(4 L^2) within 1 percent (Tet10 or Hex20).

Clauses: (a1) `K_sigma` symmetric within 1e-13 of its norm; (a2) no force for a rigid translation within 1e-12; (a3) `x^T K_sigma
x = s V |grad w . e|^2` within 1e-12 for a uniform uniaxial stress and a linear field; (a4) linalg: tridiagonal (2, -1), `n = 200`,
`K_sigma = -I`, ten lowest factors `4 sin^2(j pi/(2 (n + 1)))` within 1e-10; (a5) a `K_sigma` with no negative direction returns
no factor and says so; (a6) knife-edge beam, slenderness 20, `nu = 0`, tension `s`: `omega_n^2 - omega_n,0^2 = (n pi/L)^2 s/rho`
within 1 percent for `n = 1, 2`; (a7) under compression the frequency falls by the same amount; (a8) the same beam in compression:
critical load `pi^2 E I/L^2` reduced by the shear term, the term registered here, within 1 percent; (a9) second factor four times
the first within 2 percent; (a10) clamped-free column `pi^2 E I/(4 L^2)` within 1 percent, Tet10 or Hex20.

Reading of the clauses, registered:

- "slenderness 20" is `L/h = 20` (span over depth), the reading under which the shear term is the half percent the draft quotes
  (arithmetic 1: 0.491 percent with `kappa = 5/6`); with `L/r = 20` it would be 5.6 percent. The draft's "0.6 percent" was not
  derived; the registered value is the derived **0.4911 percent** (arithmetic 1), stated here once, before the run.
- (a1) and (a2) are checked for the element matrices of all four solid kinds and for the assembled matrix of the beam; "its norm"
  is `max_ij |K_ij|`; the translation measure is `||K_sigma t||_inf / (||K_sigma||_inf ||t||_inf)`.
- (a3): `V` is the element volume of its own stiffness rule, `sum_q w_q det J_q`, which equals the geometric volume for the
  straight-sided elements used (affine tetrahedra, trilinear hexahedra); the relative difference is measured.
- (a4) "within 1e-10" is registered as relative, `|lambda_j / lambda_j,ref - 1| <= 1e-10`, which implies the absolute form here
  (every `lambda_j <= 0.0244`).
- (a7) "falls by the same amount": `(omega_n,0^2 - omega_n,c^2)` within 1 percent of the same closed form `(n pi/L)^2 s/rho`;
  the difference of the two amounts is a further check (C1).
- (a10): Hex20, the same section and length as the beam.

## What was read (no run)

- ADR 0298 (whole), ADR 0290 sections 2.3 to 2.5 and the SI-8 entry, ADR 0389 (the style of a registration), ADR 0385 (the
  Hex20 cantilever at `L/h = 20` with one element through the thickness and ten along: 0.04 percent from Timoshenko; the full
  clamp with `nu = 0` restrains the shear warping, 0.999037 of Timoshenko in ADR 0292).
- `crates/rcfd-fem/src/element.rs` (the trait; `geometric_stiffness(coords, stress)` refuses by default with "ADR 0298";
  `integration_points`, `stiffness_with_rule`, `b_matrix`; the impls of Tet4, Hex8, Tet10; `Pending`; `element_for`),
  `hex20.rs` (`Hex20`, `gauss_3` for stiffness and mass, `upgrade_to_hex20` extends node sets by the mid-edge nodes of their
  edges and keeps surface sets), `assemble.rs` (`assemble_with`: one dense element matrix per element on the pattern of
  `stiffness_with`, scattered in element order, bit-identical for any thread count), `modal.rs` (`modal_analysis_with`, the
  reduction by `constraint::reduce`, the shift rule, `ModalResult::from_parts` is `pub(crate)`), `mass.rs` (`consistent_with`),
  `recover.rs` (`IntegrationPointField::compute`: stress `D (eps - eps_th)` at the points of each element's stiffness rule, in
  element order), `constraint.rs` (`prescribed_equations`, `check_rigid_body_motions`, `reduce`), `mpc.rs` lines 1 to 60
  (`uses_transformation`), `solve.rs` (`solve_static`), `model.rs` (`Load::traction`, `Constraint::displacement`), `builders.rs`
  (`hex_box`, `BOX_FACES`), `loads.rs` (Load::Traction on Hex20 faces through the model path, ADR 0383), `error.rs`.
- `crates/rcfd-linalg/src/eigen.rs` (whole: `lowest_eigenpairs`, `factor_shifted`, `count_unchecked`, `generalized_unchecked`,
  `ResidualTest`, `SturmCheck`, `EigenError`), `skyline.rs` (`SkylineLdlt::factor_with`, `PivotRule`, `inertia`), the test
  `tests/sb_eigen.rs` (helpers `tridiag`, `diag`).
- `crates/rcfd-fem/tests/si_scaffold.rs`: X2 asserts that `geometric_stiffness` of Tet4, Hex8 and Tet10 refuses naming ADR 0298;
  this package retires that row (D9). No other test under `crates/` calls `geometric_stiffness` (searched).

## Design

### D1. The geometric stiffness (first principles)

Green-Lagrange strain `E_kl = (u_k,l + u_l,k + u_m,k u_m,l)/2`. At a state of stress `S` in equilibrium with its loads, the
energy of an added displacement `u` at fixed `S` (the total Lagrangian form; Bathe, Finite Element Procedures, and Zienkiewicz
and Taylor vol. 2, both from memory; derived here) is `int (eps:D:eps/2 + S:E) dV - work of the loads`; the part of `S:E` linear
in `u` cancels against the loads by equilibrium, and what is left beyond the linear stiffness is the quadratic part

    int S_kl u_m,k u_m,l / 2 dV = x^T K_sigma x / 2.

With `u_m = sum_a N_a x_am`: `K_sigma[(a, i), (b, j)] = delta_ij int N_a,k S_kl N_b,l dV`, the same 3 x 3 block-diagonal scalar
`g_a . S g_b` for the three components. Evaluated with the element's stiffness rule (the points at which the static stress is
recovered, ADR 0277): `K_sigma = sum_q w_q det J_q (g_a,q . S_q g_b,q) delta_ij`, both triangles computed (as the stiffness). `S`
is the Voigt stress `[xx, yy, zz, xy, yz, zx]` with tensor shear (ADR 0270 section 2.2).

Properties used by the checks, exact in exact arithmetic:

1. Symmetric (`S` symmetric).
2. A rigid translation `x_am = t_m` gives `sum_b N_b,l t_m = 0` (the gradients of a partition of unity sum to zero), so `K_sigma t
   = 0`.
3. A linear field `u = G x + c` is reproduced exactly by an isoparametric element, `u_m,k = G_mk` at every point, so `x^T K_sigma x
   = sum_q w_q det J_q G_mk S_kl G_ml = V tr(G S G^T)`; for `S = s e e^T` this is `s V |G e|^2`.
4. Tension (`s > 0`) stiffens, compression softens: `x^T K_sigma x = s int |du/de|^2 dV`.

`element.rs` gains one overriding method in each of the impls of Tet4, Hex8 and Tet10, and `hex20.rs` one in the impl of Hex20,
each a one-line call of `geometric::element_geometric_stiffness(self, coords, stress)` (the function in `geometric.rs`). The
trait default (refusal) is not changed: Beam2, Shell4 and the `Pending` placeholder keep it. A stress list whose length is not the
number of rule points is refused (`FemError::FieldLength`, key `"stress"`); a non-finite stress is refused (`InvalidValue`, key
`"stress"`).

### D2. The pre-stress

`geometric::Prestress`: per element, one Voigt stress per point of its stiffness rule (offsets as `IntegrationPointField`).
Constructors: `Prestress::from_field(&IntegrationPointField)`, `Prestress::from_static(model, &SolidResult)` (the stress of
`IntegrationPointField::from_result`, which includes `D eps_th` of a temperature load), `Prestress::uniform(model, [f64; 6])`, and
`scaled(f)`. A pre-stress whose element count or point counts do not fit the model is refused (`FieldLength`, key `"prestress"`).

### D3. The assembled matrix

`geometric::geometric_stiffness(model, &prestress, threads) -> CsrMatrix` through `assemble::assemble_with` (the generic hook of
ADR 0291): the pattern is that of `stiffness_with`, so `K + K_sigma` is a value-by-value sum on one pattern, and the result is
bit-identical for any thread count. Elements of a kind or formulation not supported are refused before the assembly
(`check_elements_supported`).

### D4. Pre-stressed modes

`geometric::prestressed_modes(model, &prestress, &ModalOptions) -> ModalResult`: `(K + K_sigma) phi = omega^2 M phi`, the same
steps as `modal::modal_analysis_with` (zero prescribed values only, the reduction by `constraint::reduce`, the shift rule, the
subspace iteration with its Sturm check), with `K + K_sigma` in place of `K`. With a zero pre-stress `K + 0 = K` value by value,
so the result has the bits of `modal_analysis` (check C4). A model with a linear constraint, rigid set or support (the
transformation path) is refused, key `"constraint"` (a follow-up). Beyond the buckling load `K + K_sigma` is not positive definite
and the factorisation at shift 0 refuses; the refusal is returned (`FemError::Solver`), never a negative eigenvalue (C7).

### D5. The buckling eigensolver (`rcfd-linalg`, one function appended to `eigen.rs`)

`(K + lambda K_sigma) x = 0`, `K` symmetric positive definite, `K_sigma` symmetric and indefinite in general: the lowest positive
factors. With `A = -K_sigma` and `theta = 1/lambda` it is `A x = theta K x`, a symmetric-definite pencil (`K` positive definite):
every `theta` is real, the vectors are `K`-orthogonal, and the wanted factors are the **largest positive** `theta`. The operator
`K^{-1} A` is self-adjoint in the `K` inner product, so subspace iteration with a Rayleigh-Ritz step (Bathe and Wilson, from
memory; it is the iteration of ADR 0283 with the roles of the matrices exchanged) applies:

1. `Y = A X`; `Xb = K^{-1} Y` (the envelope factor of `K`, `PivotRule::Positive`; a refusal is `ShiftNotPositiveDefinite` with
   shift 0); columns scaled to unit `K`-norm `Xb_j . Y_j`.
2. Reduced pair `A_r = Xb^T (A Xb)`, `K_r = Xb^T Y` (positive definite while the subspace has full rank, else `RankDeficient`),
   dense solve by Cholesky reduction and Jacobi (`generalized_unchecked`, D2 of ADR 0283); `X = Xb Z`.
3. Residual of a pair: `||A x - theta K x|| / (|theta| ||K x||)` (`ResidualTest::Relative`) or the backward error `||A x - theta K
   x|| / ((||A||_inf + |theta| ||K||_inf) ||x||)` (`ResidualTest::Backward`, ADR 0354).
4. Positive cut: a Ritz value counts as a factor only if `theta > POSITIVE_CUT max_j |theta_j|`, `POSITIVE_CUT = 1e-8`, so that
   rounding about a null direction of `K_sigma` is never reported as a factor; the bound is `S = 1/(POSITIVE_CUT max |theta|)`.
   The wanted set `W` is the `count` largest positive Ritz values (fewer if fewer pass the cut), extended by the cluster rule of
   ADR 0283 D6 (relative gap 1e-6); the settled set `D` is the `count` Ritz values of largest `|theta|`. Converged when every pair
   of `W` and of `D` meets the tolerance.
5. Sturm check (Sylvester's law of inertia): `K + s K_sigma` is congruent through `K^{1/2}` to `I - s C`, `C` with the eigenvalues
   `theta`, so its number of negative pivots is `#{theta_j > 1/s} = #{lambda_j in (0, s)}`. It is counted at `s` between the last
   factor of `W` and the next positive Ritz factor (fractions 0.5, 0.25, 0.75 as ADR 0283 D8), or at `s = S` when there is no
   next positive Ritz value or fewer than `count` were found; a count different from `|W|` is `SturmMismatch` (a factor was
   missed: the subspace should be larger).
6. Result `BucklingEigen`: factors `lambda_j = 1/theta_j` ascending, vectors `K`-normalised (`x^T K x = 1`), residuals,
   iterations, subspace size, the Sturm record, and `exhausted: Option<f64>`: `Some(S)` when fewer than `count` factors were found,
   meaning every factor below `S` is returned and the Sturm count at `S` confirms it; with no factor at all the vectors are empty
   and `exhausted` says below which value there is none (clause a5).

Options `BucklingEigenOptions`: tolerance (default 1e-10), residual test (default Relative), iteration cap (200), subspace size
(default `min(n, max(2 count, count + 8))`), ordering (RCM), start vectors (default: the pseudo-random columns of ADR 0283 D4, all
`q` of them; the first column of ADR 0283, `M 1`, has no analogue here since `A 1 = 0` for a translation). No new `EigenError`
variant (the type is not `non_exhaustive`, so a new variant could break a match elsewhere). Sequential, fixed order.

### D6. Buckling of a model

`buckling::factors(model, &prestress, &BucklingOptions) -> BucklingResult`: validate, `check_supported`, the transformation path
refused (key `"constraint"`), zero prescribed values only (key `"constraint"`), the rigid motions must be held
(`check_rigid_body_motions`, its refusal returned: `K` must be positive definite), `K` and `K_sigma` reduced by the same
elimination, D5, the modes expanded to the model's equations (0 at prescribed ones). `BucklingOptions::new(count)`: tolerance
1e-10 with `ResidualTest::Backward` (the relative measure has a floor near `eps lambda_max/lambda_1`, about 1e-9 for the beam;
ADR 0354), cap 200, subspace `None`, threads `None`. `BucklingResult`: `factors`, `modes`, `residuals`, `iterations`, `sturm`,
`unknowns`, `exhausted` (as D5) and `dofs()`.

### D7. The set-ups

Steel-like material, `E = 2e11 Pa`, `nu = 0`, `rho = 7850 kg/m^3`, no expansion. Section `h = b = 0.1 m`, `L = 2 m` (`L/h =
20`), `A = 0.01 m^2`, `I = b h^3/12 = 8.3333e-6 m^4`, `G = E/2`, `kappa = 5/6` (Cowper's `10 (1 + nu)/(12 + 11 nu)` at `nu = 0`,
from memory; the value of ADR 0292 and 0385).

- **Beam B (knife edges).** `hex_box([0, 0, 0], [L, h, b], [40, 2, 1])` upgraded by `upgrade_to_hex20`. Node sets added: `ke0` =
  nodes with `x = 0` and `y = h/2` (three: two corners and the mid-edge node between them), `ke1` = the same at `x = L`, `every` =
  all nodes. Constraints: `u_x = u_y = 0` on `ke0` (pin), `u_y = 0` on `ke1` (roller), `u_z = 0` on `every`. Why `u_z = 0`
  everywhere: with `nu = 0` the in-plane (`x-y`) bending of the bar has `u_z = 0` exactly (no Poisson coupling; the plane-stress
  and plane-strain solutions coincide, and the shear stress of a rectangle needs no correction at `nu = 0`), so the constraint
  does not change the bending in `y`; it removes the `z` bending and the torsion, so that the lowest two modes are the bending
  modes `n = 1, 2` in `y` without a degenerate partner (arithmetic 3: the first axial mode is above the third bending mode).
  Rigid motions: `t_x` (pin), `t_y` and `r_z` (two lines), `t_z`, `r_x`, `r_y` (`u_z = 0` everywhere). The knife edges sit on
  the neutral axis `y = h/2` (corner nodes there because `ny = 2` is even).
- **Pre-stress of B** from a static solve of B (direct solver) with `Load::traction` on the surface sets `xmax` `(s, 0, 0)` and
  `xmin` `(-s, 0, 0)` (tension for `s > 0`), balanced, so the support reactions are zero; with `nu = 0` the exact solution `u_x =
  s x/E + c`, `u_y = u_z = 0` satisfies every constraint and is reproduced exactly by the elements, so the recovered stress is
  `sigma_xx = s` at every point to rounding (C5). Modes: `s = +1e8 Pa` (tension) and `s = -1e8 Pa` (compression), against `s = 0`
  (plain `modal_analysis` of B). Buckling: `s = -1e6 Pa` (reference compression), factors `lambda`, critical stress `lambda 1e6`.
- **Column C (clamped-free).** `hex_box([0, 0, 0], [L, h, b], [40, 2, 2])` upgraded to Hex20, `u_x = u_y = u_z = 0` on `xmin`
  (the clamp; with `nu = 0` consistent with the uniform compression), `Load::traction` `(-1e6, 0, 0)` on `xmax`. Two factors
  wanted (the `y` and `z` modes are equal by symmetry). Fully three-dimensional (no `u_z` constraint).
- **Elements E (clauses a1 to a3).** One element of each kind, alone: Tet4 with vertices `(0,0,0)`, `(1.1,0.1,-0.05)`,
  `(0.2,0.9,0.1)`, `(-0.1,0.15,1.2)`; Tet10 on the same vertices with mid-edge nodes at the midpoints; Hex8 with the corners of
  the unit cube in the order of ADR 0271, each moved by `0.08 (sin(1.3 a + 0.2 i), cos(0.7 a + i), sin(2.1 a - i))` for corner
  `a` and coordinate `i` (trilinear, straight edges); Hex20 on the same corners with mid-edge nodes at the edge midpoints.
  Stresses: a general one `S_g = [3.1, -1.7, 2.3, 0.9, -1.1, 0.6] 1e6` (all six components, the same at every point) and the
  uniaxial `S_u = s e e^T`, `s = 2.5e6`, `e = (1, 2, 2)/3`. Linear field `u = G x + c`, `G = [[0.3, -0.7, 0.2], [0.5, 0.1,
  -0.4], [-0.6, 0.8, 0.9]] 1e-3`, `c = (1, -2, 3) 1e-3`.

### D8. Measures of identification

The mode `n` of B is identified by the correlation `|sum u_y(x_i) sin(n pi x_i/L)| / sqrt(sum u_y^2 sum sin^2)` over the nodes
of the neutral line `y = h/2, z = 0`; the buckling mode of C by the same correlation of the lateral displacement along the centre
line `y = z = h/2` projected on its principal direction (the unit vector of the larger eigenvalue of the 2 x 2 matrix `sum v v^T`)
with `1 - cos(pi x/(2 L))`.

### D9. The retired scaffold row

X2 of `si_scaffold.rs` asserts that `geometric_stiffness` refuses for Tet4, Hex8 and Tet10 with "ADR 0298"; after this package
it returns the matrix. By the maintainer's standing decision of 2026-11-13, `x2_entry_points_refuse_by_default` is marked
`#[ignore = "record of ADR 0291 before ADR 0390 (F-395, geometric stiffness) retired the geometric_stiffness row; superseded by
the _after_geometric_landed copy below (maintainer, 2026-11-13)"]`, body untouched, and a copy
`x2_entry_points_refuse_by_default_after_geometric_landed` with the `internal_force` and `tangent` rows only is added at the end of
the file. The refusal of Beam2 and Shell4 (`Pending`) is not part of X2 and stays.

### Arithmetic before this registration (disclosed)

By hand and with `arith.py` (closed forms only):

1. `P_E = pi^2 E I/L^2 = 4.112335e6 N` (`sigma_E = 4.112335e8 Pa`); `kappa G A = 8.333333e8 N`; `a = P_E/(kappa G A) =
   4.934802e-3`. Engesser (Timoshenko and Gere, from memory; derived: `w = w_b + w_s`, `E I w_b'' = -P w`, `kappa G A w_s' = P w'`
   give `w'' (1 - P/(kappa G A)) = -(P/(E I)) w`, so `P = P_E/(1 + P_E/(kappa G A))`): `P_1 = 4.092141e6 N`, a reduction of
   `a/(1 + a) = 0.4911 percent` (the registered shear term); `lambda_1 = 409.2141` for `s = -1e6 Pa`. The Haringx form differs by
   `O(a^2) = 2.4e-5`, below every limit here.
2. `n = 2`: `P_2 = 4 P_E/(1 + 4 a) = 1.613093e7 N`, `lambda_2 = 1613.093`; `lambda_2/(4 lambda_1) = (1 + a)/(1 + 4 a) =
   0.985482`, 1.452 percent below 4 (inside the 2 percent of a9 with 0.55 percent to spare).
3. Modes of B, Euler-Bernoulli: `omega_n,0^2 = (n pi/L)^4 E I/(rho A)`: 1.29258e5 and 2.06813e6 rad^2/s^2. With `c =
   sqrt(E/rho)`: bending `omega h/c = (n pi h/L)^2/sqrt(12) = 0.00712, 0.0285, 0.0641` for `n = 1, 2, 3`; the first axial mode
   of B (held in `x` at one end only) at `omega = (pi/(2L)) c`, `omega h/c = 0.0785`: the lowest two modes are the bending modes. `Delta_n,ref = (n pi/L)^2 s/rho` at `s = 1e8`: 3.14319e4 and 1.25727e5, i.e. 24.3 and 6.08 percent
   of `omega_n,0^2`; compression `s = -1e8` is 24 percent of the buckling stress, so `K + K_sigma` stays positive definite.
4. Why the difference cancels the discretisation and the shear and rotary inertia: for a field separable in `x` (each component
   `f(y, z) sin(kx)` or `f(y, z) cos(kx)`), `x^T K_sigma x = s int |du/dx|^2 = s k^2 int |u|^2` and `x^T M x = rho int |u|^2`, so
   the Rayleigh quotient of `K_sigma` over `M` is `s k^2/rho` exactly, whatever the section deformation (shear, rotary inertia);
   in the continuum `K_sigma u = -s u_xx = s k^2 u` is proportional to the mass operator, so the mode is unchanged by the stress
   and the shift is exact at every order. What is left: the knife-edge end region (the end face is free except on the line, so
   the shear stress of the separable mode, about the shear share of the energy, 0.5 percent for `n = 1` and 2 percent for `n =
   2`, is relaxed locally) and the discrete `k_h^2` of quadratic elements along `x` (`(k h_e)^4/720` order, below 1e-6 at 40
   elements). Estimate `|Delta/Delta_ref - 1| <= 3e-3` for `n = 1`, `<= 5e-3` for `n = 2`.
5. Second order in `s`: tension and compression differ by `alpha^2 lambda''` with `lambda''` from the coupling of the mode to
   others through `K_sigma`, which needs the non-separable end part (relative size `epsilon ~ 1e-3`): `|Delta_t - Delta_c|/
   Delta_ref ~ epsilon^2 ~ 1e-6`; limit 1e-3.
6. Column C: `P_c = pi^2 E I/(4 L^2) = 1.028084e6 N`, `lambda = 102.8084`; Engesser with `a/4 = 1.2337e-3` lowers it by 0.123
   percent; the full clamp at `nu = 0` stiffens the root a little (ADR 0292: 0.1 percent in the static deflection). Estimate
   `|P/P_c - 1| <= 3e-3`.
7. Hex20 buckling discretisation of B at 40 elements along (80 per half-wave for `n = 1`, 40 for `n = 2`): from ADR 0385 (10
   elements along, one through the depth, 0.04 percent of Timoshenko in static bending) the bending error here is far below 1e-3.
   Estimate for a8 `|P_1/P_Eng - 1| <= 2e-3`; for C3 (`n = 2` against `4 P_E/(1 + 4 a)`) `<= 3e-3`.
8. Linalg (a4): `lambda_j = 4 sin^2(j pi/402)`: `2.4429e-4` (j = 1) to `2.4379e-2` (j = 10), `lambda_21 = 0.10677`; the slowest
   wanted pair converges at `lambda_10/lambda_21 = 0.228` per iteration with the default `q = 20`. Relative residual floor of the
   lowest pair about `eps ||K|| theta ||x|| / (theta ||K x||) ~ 7e-12`, below the default 1e-10; the Ritz values carry the error
   of the factor solve, about `eps cond(K) = 2e-12` relative. Estimate `<= 1e-11`.
9. Element checks (a1 to a3): sums of a few hundred products of numbers of order 1 to 1e7: rounding about `1e-15` relative.
10. Hex8 locking (negative control N1): a fully integrated Hex8 in bending with square elements carries parasitic shear (the
    element cannot bend without shear strain); its bending stiffness is too high by tens of percent at this aspect ratio
    (Cook et al., from memory, `1 + (a/b)^2/2` order at `nu = 0`): `lambda_1` of B on Hex8 above `P_Eng` by more than 1 percent.

## Registered checks

All in `crates/rcfd-fem/tests/si_geometric.rs` (A1 to A3, A5b to A10, C, N) and `crates/rcfd-linalg/tests/si_buckling_eigen.rs`
(A4, A5a, L), release profile, each run once (`CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --test si_geometric --
--nocapture` and `CARGO_BUILD_JOBS=4 cargo test -p rcfd-linalg --release --test si_buckling_eigen -- --nocapture`). Relative
differences are `|x - y|/|y|`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1 | E, each of Tet4, Hex8, Tet10, Hex20, stresses `S_g` and `S_u`: `max_ij |K_ij - K_ji| / max_ij |K_ij|`; and the assembled `K_sigma` of B with the tension pre-stress | each <= 1e-13 | ~1e-16 |
| A2 | a2 | E, each kind, `S_g`: `||K_sigma t||_inf / (||K_sigma||_inf ||t||_inf)` for `t` = the three unit translations and `(0.3, -1.2, 0.7)`; and the assembled `K_sigma` of B | each <= 1e-12 | ~1e-16 |
| A3 | a3 | E, each kind, `S_u`, `u = G x + c`: `|x^T K_sigma x / (s V |G e|^2) - 1|`, `V = sum_q w_q det J_q`; and `|V/V_geom - 1|` (exact volume of the tetrahedron; of the trilinear hexahedron by the 2 x 2 x 2 rule, exact for it) | each <= 1e-12 | ~1e-15 |
| A4 | a4 | `K` = tridiagonal (2, -1), `n = 200`, `K_sigma = -I`, 10 factors, default options: `max_j |lambda_j / (4 sin^2(j pi/402)) - 1|` | <= 1e-10 | <= 1e-11 (arith. 8) |
| A5a | a5 | the same `K`, `K_sigma = +I`, and `K_sigma = diag(0, 1, 0, 1, ...)` (positive semidefinite, singular): `Ok` with no factor, `exhausted = Some(S)`, the Sturm record counted 0 at `S` | as stated | D5 |
| A5b | a5 | B with the tension pre-stress `s = +1e8`: `buckling::factors` returns no factor and `exhausted = Some(S)`, Sturm counted 0 | as stated | D1 property 4 |
| A6 | a6 | B, `s = +1e8`: `|(omega_n^2 - omega_n,0^2)/Delta_n,ref - 1|`, `n = 1, 2` | each <= 0.01 | 3e-3, 5e-3 (arith. 4) |
| A7 | a7 | B, `s = -1e8`: `|(omega_n,0^2 - omega_n,c^2)/Delta_n,ref - 1|`, `n = 1, 2` | each <= 0.01 | 3e-3, 5e-3 |
| A8 | a8 | B, `s = -1e6`, 2 factors: `|lambda_1 1e6 A / P_1 - 1|`, `P_1 = 4.092141e6 N` (shear term 0.4911 percent registered) | <= 0.01 | 2e-3 (arith. 7) |
| A9 | a9 | the same run: `|lambda_2/(4 lambda_1) - 1|` | <= 0.02 | 0.0145 (arith. 2) |
| A10 | a10 | C, 2 factors: `|lambda_1 1e6 A / P_c - 1|`, `P_c = 1.028084e6 N` | <= 0.01 | 3e-3 (arith. 6) |

Modal options of A6, A7, C1, C4: `ModalOptions::new(2).residual_test(ResidualTest::Backward)` with the default tolerance 1e-10
(the relative measure's floor is above 1e-10 for B, ADR 0354). Buckling options: `BucklingOptions::new(2)` (D6 defaults).

### C. Companions and further checks

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | B: `|(omega_n,t^2 - omega_n,0^2) - (omega_n,0^2 - omega_n,c^2)| / Delta_n,ref`, `n = 1, 2` | each <= 1e-3 | ~1e-6 (arith. 5) |
| C2 | identification (D8): the 6 modes of A6/A7 (`n = 1, 2` at `s = 0, +1e8, -1e8`) and the 2 buckling modes of A8 against `sin(n pi x/L)`; the first buckling mode of C against `1 - cos(pi x/(2L))` | each >= 0.99 | > 0.999 |
| C3 | A8's run: `|lambda_2 1e6 A / (4 P_E/(1 + 4 a)) - 1|` | <= 0.01 | 3e-3 |
| C4 | B with `Prestress::uniform(model, [0; 6])`: `prestressed_modes` against `modal_analysis` | eigenvalues and modes bit-equal | equal (D4) |
| C5 | B, the static pre-stress of `s = +1e8`: `max |sigma_xx/s - 1|` and `max |sigma_other|/s` over all points; and `prestressed_modes` with it against `Prestress::uniform(model, [s, 0, 0, 0, 0, 0])` (relative eigenvalue difference) | each <= 1e-6 | ~1e-9 |
| C6 | B: `geometric_stiffness` with `threads = 1` and `8`, and `buckling::factors` with `threads(1)` and `threads(8)` | bit-equal | equal (D3) |
| C7 | refusals: a pre-stress of B used on C (`FieldLength`, key `"prestress"`); element entry with one stress too few (`FieldLength`, key `"stress"`) and with a NaN (`InvalidValue`, key `"stress"`); `buckling::factors` on B without its pin (`u_x` free: `RigidBodyMotionFree`); on B with an added nodal spring (`InvalidValue`, key `"constraint"`); `prestressed_modes` of B at `s = -5e8` (beyond the buckling stress 4.09e8: `FemError::Solver`) | each refused as stated | D1 to D6 |
| C8 | A1 to A3 also through the trait (`element_for(kind).geometric_stiffness`) against the function: bit-equal; Beam2 and Shell4 still refuse | as stated | D1 |
| C9 | E, each kind, `S_g`, `u = G x + c`: `|x^T K_sigma x / (V tr(G S_g G^T)) - 1|` (every stress component) | each <= 1e-12 | ~1e-15 |
| C10 | C, the factor pair: `|lambda_2/lambda_1 - 1|` (symmetry of the square section) | <= 1e-6 | ~1e-10 |
| L1 | A4's run: Sturm record `counted = expected = 10` | equal | D5 |
| L2 | `n = 200`, `K` = tridiagonal (2, -1), `K_sigma = diag(-1 for i < 100, +1 otherwise)`, 5 factors, `subspace_size = 30`, against the dense reference (`generalized_eigen_dense(-K_sigma, K)`, the 5 largest positive `theta`, `lambda = 1/theta`) | each relative <= 1e-9 | ~1e-12 |
| L3 | `n = 10`, `K = I`, `K_sigma = diag(-1, 100, 200, ..., 900)`, 1 factor, `subspace_size = 3`: the positive `theta = 1` is behind nine larger negative ones and not in the subspace | `SturmMismatch` (counted 1, expected 0) | D5 step 5 |
| L4 | L3 with `subspace_size = 10`: the factor `lambda = 1` | relative <= 1e-12 | ~1e-16 |
| L5 | refusals: `K` = minus the tridiagonal (not positive definite: `ShiftNotPositiveDefinite`); `count = 0` (`BadCount`); sizes 200 and 199 (`DimensionMismatch`); an unsymmetric `K_sigma` (`Unsymmetric`) | each as stated | D5 |
| L6 | A4 run twice | bit-equal | sequential |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A8's measure on B built of Hex8 (the same `[40, 2, 1]` box, not upgraded) | `> 0.01` (shear locking, arith. 10: the check discriminates) |
| N2 | A6's measure with the pre-stress negated (`scaled(-1)` of the tension pre-stress, used as if it were tension) | `> 0.01` (about 2: the sign is seen) |
| N3 | A4's measure with `K_sigma = -2 I` | `> 1e-10` (about 1: the factors halve) |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `cargo test -p rcfd-fem --release` (every test of the crate; the X2 record ignored, its copy passing) | 0 failed |
| S2 | `cargo test -p rcfd-linalg --release --test sb_eigen --test sb_eigen_scale --test si_buckling_eigen` and the crate's unit tests (`--lib`) | 0 failed |
| S3 | `cargo test -p rcfd-api --release --test baseline_hashes` (shared solver files `element.rs`, `hex20.rs`, `eigen.rs` touched) | 0 failed |
| S4 | `cargo check -p rcfd-post --tests` | builds |
| S5 | `git diff ae92ee27 -- crates` touches only: `rcfd-fem/src/{geometric.rs, buckling.rs}` (filled), `element.rs` and `hex20.rs` (the overriding methods only), `rcfd-linalg/src/eigen.rs` (appended), the two new test files, `si_scaffold.rs` (D9 only) | as stated |

### Mutations (scratch copy, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the shear stresses dropped from `S` (diagonal only) | C9 |
| Mu2 | `g_a . S g_b` replaced by `(g_a . g_b) tr(S)/3` | A3, C9 |
| Mu3 | the block put on component 0 only (`delta_ij` lost) | A3, A6 |
| Mu4 | the Sturm comparison removed from the buckling eigensolver | L3 (returns no factor instead of refusing) |
| Mu5 | the positive cut removed (every Ritz value reported) | A5a, A5b |
| Mu6 | `prestressed_modes` with `K - K_sigma` | A6, A7 |
| Mu7 | the factor reported as `theta` instead of `1/theta` | A4 |
| Mu8 | `Prestress::from_static` taking the strain instead of the stress | A6 (the shift scaled by `1/E`) |

## Decision rule

- **done**: every A check met, every C, L and S check met, the negative controls as expected.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C, L or S check missed.
- **todo**: the checks could not be run.

## Limits of this registration

- Solid elements only (Tet4, Hex8, Tet10, Hex20); no geometric stiffness of beams and shells (Beam2 and Shell4 are placeholders and
  keep the refusal, whose text still names ADR 0298).
- No transformation path: models with linear constraints, rigid sets or supports are refused for pre-stressed modes and buckling
  (key `"constraint"`); a follow-up. Zero prescribed values only.
- No pre-load: the buckling load is one pattern scaled by `lambda`; a fixed part plus a scaled part is not covered.
- No follower loads (pressure on the deformed surface), no load stiffness, no spin softening (ADR 0299).
- The buckling eigensolver finds the largest positive `theta` among the dominant ones of the subspace; positive factors hidden
  behind many larger negative `theta` (a `K_sigma` mostly in tension with a small compressed region) need a larger subspace, and
  the Sturm check refuses rather than miss them (L3). No shift-invert in `lambda`.
- One mesh per set-up; no convergence study. The knife edges carry no reaction in the static solve (balanced loads), so the local
  concentration of a knife edge under a reaction is not exercised.
