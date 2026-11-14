# ADR 0493: F-390 registration: Hex8 without locking, the mean-dilatation (B-bar) and the incompatible-mode formulations, opt-in per element block

Status: **REGISTERED 2026-11-14, written before any code of F-390 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-390 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f390-scratch`: `proto.py` (the three element formulations and a dense assembly), `est1.py` (patch tests, single
elements, cantilevers), `est2.py` (the cylinder), `est3.py` (pure bending and the recovery identities), `mut_proto.py` (two
planned mutations), logs `est1.log`, `est2.log`, `est3.log`, `mut_proto.log`; dense matrices, no rcfd code; disclosed, its
numbers quoted below). The bit record B1 of the default paths is a test file written and run on the base after this commit and
before any library code (D7). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-390 (deps F-388, F-372, F-374, F-376, F-379), stub [ADR 0293](0293-locking-free-hex8-b-bar-and-incompatible-modes.md)
(SI-3 of [ADR 0290](0290-structural-solver-intermediate-tier-plan.md)), the scaffold of [ADR 0291](0291-intermediate-scaffold-modules-and-model-extensions.md).
It unblocks F-580 (the creep of the thick cylinder point-wise, after [ADR 0489](0489-f411-registration-power-law-creep-time-hardening.md)
found that fully integrated elements lock under volume-preserving creep flow). Number 0493 given by the lead. Branch `f390-work`
from local main aa0e4d5b, worktree `w49i-work/f390`, scratch `w49i-work/f390-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited except the records-and-copies step of the scaffold (D8); clean-room.

## What F-390 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Two opt-in formulations of Hex8, default unchanged bit for bit. Both pass the patch test of F-374 on distorted meshes within 1e-10 and a single element of either has exactly six zero eigenvalues (the seventh above 1e-6 of the largest) for aspect ratios 1, 10 and 100. Cantilever of F-379 (L/h = 20, tip shear force) with ONE element through the thickness and 20 along: incompatible modes within 1 percent of the closed form at nu = 0 and within 3 percent at nu = 0.3, while full integration on the same mesh is the registered locking control (below 80 percent); at L/h = 100 with elements of aspect ratio 5 incompatible modes stay within 2 percent. Thick cylinder b/a = 2 in plane strain with nu = 0.4999: B-bar gives the bore displacement within 1 percent of Lame on the mesh on which full integration is below 90 percent (control). Recovered stress on the patch test equals D eps within 1e-10; the condensed internal modes are recovered for the stress.

The roadmap note adds (ADR 0291): the formulation is `ElementBlock::formulation()`, set by `SolidMesh::set_block_property` with
`BlockProperty::solid`; `hex8x::PENDING` gates it; the stiffness and the recovery do not delegate to `hex8x`, so this package
edits `assemble.rs` and `recover.rs`. The brief adds: (b1) the incompatible element passes the patch test on distorted meshes
only with the Taylor correction (the Jacobian at the centre), registered; (b2) follow the per-block option pattern of F-549
(`BlockProperty::shell4_with_membrane`); (b3) linear elasticity is enough, B-bar in the nonlinear driver is a follow-up; (b4)
defaults keep their bits, a default-bits test at the base before any library change; (b5) one-ulp probes before `1e-12`-class
limits, every aggregated error required finite (an `f64::max` over a NaN hides it); (b6) the scaffold records of `si_scaffold.rs`
kept as ignored records citing ADR 0291 with `_after_f390` copies, the edit minimal.

Clauses: (a0) every limit registered here; (a1) two opt-in formulations, the default bit for bit; (a2) both pass the patch test of
F-374 on distorted meshes within `1e-10`; (a3) a single element of either has exactly six zero eigenvalues, the seventh above
`1e-6` of the largest, for aspect ratios 1, 10, 100; (a4) the F-379 cantilever at L/h = 20, one element through the thickness and
20 along: incompatible modes within 1 percent at `nu = 0` and 3 percent at `nu = 0.3`, full integration below 80 percent
(control); (a5) L/h = 100, elements of aspect ratio 5: incompatible modes within 2 percent; (a6) the cylinder b/a = 2, plane
strain, `nu = 0.4999`: B-bar's bore displacement within 1 percent of Lame on a mesh on which full integration is below 90
percent; (a7) the recovered stress on the patch test equals `D eps` within `1e-10`, and the condensed internal modes are
recovered for the stress.

Readings, registered:

- **(a1) "opt-in"**: `BlockProperty::solid(SolidFormulation::BBar)` and `BlockProperty::solid(SolidFormulation::Incompatible)`
  on a Hex8 block (the property data of ADR 0291, already in `mesh.rs`); the default (`None` property, or `Solid { Full }`) is the
  basic-tier Hex8. The scaffold's `SolidFormulation` already has the two variants, so no new constructor is needed (b2: the
  F-549 pattern is a property chosen per block and refused on a block of another kind, D2).
- **(a2) "the patch test of F-374 on distorted meshes"**: the patch of `sb_static.rs` (ADR 0275): the box at `(0.1, -0.2, 0.3)`
  of lengths `(1.2, 1.0, 0.8)`, `4 x 3 x 3` Hex8, every interior node moved by `0.1 h_k (2U - 1)` per coordinate with the
  SplitMix64 of seed `0xF374_0275` and tag 11 (the Hex8 tag of `sb_static.rs`), the boundary nodes held at the linear field of
  `sb_static.rs` (all six strains non-zero), `nu = 0.3`, `E = 2e11`, through `solve_static_with`; the measure is
  `max |u_h - u|` over the interior unknowns relative to the largest field component. "Distorted meshes" (plural) is read as that
  patch and a second, stronger one: the same box with the interior nodes moved by `0.25 h_k (2U - 1)` (tag 1001), at `nu = 0.3`
  and at `nu = 0.4999`. The faces of these elements are not planar: general trilinear maps.
- **(a3) "aspect ratios 1, 10 and 100"**: one element, the box `[0, AR] x [0, 1] x [0, 1]` in the Hex8 node order, `nu = 0.3`,
  its 24 x 24 stiffness through `hex8x::element_stiffness`, all eigenvalues by `rcfd_linalg::eigen::symmetric_eigen` (cyclic
  Jacobi); "zero" means `|lambda| <= 1e-10 lambda_max`, the F-549 SE1 criterion; exactly six such, and `lambda_7 / lambda_max >
  1e-6`.
- **(a4) "the cantilever of F-379 with ONE element through the thickness and 20 along"**: `L = 2`, `h = 0.1` square section,
  `E = 2e11`, the tip shear `P = 1000 N` as the uniform traction `-P / h^2` on `xmax` (`Load::traction`, as `sb_closed_forms.rs`),
  `hex_box [20, 1, 1]`, the measure of ADR 0433 D3 `delta = sum_i f_i u_i / P` against `delta_T = P L^3 / (3 E I) (1 + 3 E I /
  (kappa G A L^2))`, `kappa = 5/6`. **The support**: F-379's beam support (ADR 0433 D2) holds `u_z` at the two root nodes on the
  neutral line `z = h/2`, which this mesh does not have; read here as `u_x = 0` and `u_z = 0` on every root node and `u_y = 0` at
  the root node `(0, 0, 0)` (the root section plane and the mean deflection of the root zero; the lateral Poisson contraction at
  the root free). The full clamp (all three components on `xmin`) is printed beside it as a record (O1).
  "Within 1 percent" is `|delta / delta_T - 1| <= 0.01`; "below 80 percent" is `delta / delta_T < 0.8`.
- **(a5)**: the same with `L = 10` (L/h = 100), `hex_box [20, 1, 1]` (each element `0.5 x 0.1 x 0.1`, aspect ratio 5), `nu =
  0.3` (F-379's), the same support and load; within 2 percent is `|delta / delta_T - 1| <= 0.02`.
- **(a6)**: the cylinder of `sb_closed_forms.rs` (ADR 0433 D6 to D8): `a = 1`, `b = 2`, one axial layer `H = 0.1`, the quarter
  model with the rollers `u_y = 0` on `ymin`, `u_x = 0` on `ymax`, plane strain by `u_z = 0` at every node, `Load::pressure("xmin",
  1e6)`, `hex_box [4k, 8k, 1]` mapped by the radially graded polar map (`beta = 2`); `E = 2e11`, `nu = 0.4999`. **The mesh: `k =
  2` (8 x 16)**, fixed from the prototype (arithmetic 6) before any rcfd run. "The bore displacement within 1 percent" is
  `max_bore |u_r / u_r^Lame(a) - 1| <= 0.01` (ADR 0433's measure); "full integration below 90 percent" is `max_bore u_r /
  u_r^Lame(a) < 0.9` with the default Hex8 on the same mesh.
- **(a7)**: "recovered stress" is `IntegrationPointField::compute` (F-376): at every point of every element of the patches of
  (a2), `max |sigma_h - D eps| / max |D eps|` with `eps` the field's exact strain; "the condensed internal modes are recovered
  for the stress" is read as: the stress of an Incompatible element is `D (B u + G alpha)` with `alpha` recovered from the
  element's condensation (D1), checked where `alpha` is not zero (RS2, RS3).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0293 (the stub), ADR 0291 (the scaffold: section D6, the refusal table, the note for 0293 and
  its open question on `assemble.rs` / `recover.rs`), ADR 0290 (section 2.3, the 0293 entry), ADR 0470 (F-549, the per-block
  option and the style of the registration), ADR 0433 (the F-379 benchmarks), ADR 0489's summary in the roadmap (the locking that
  F-580 waits on); the roadmap entries F-390, F-372, F-374, F-376, F-379, F-388, F-549, F-580.
- `crates/rcfd-fem/src/hex8x.rs`, `element.rs` (whole), `assemble.rs` (whole), `recover.rs` (lines 1 to 420), `mesh.rs` (the
  formulation, the property, `set_block_property`, `ElementBlock::formulation`), `model.rs` (`check_elements`), `thermal.rs`
  (lines 1 to 140), `nonlinear.rs` (`check_model`), `geometric.rs` (`assemble_geometric`), `damped.rs` and `harmonic_direct.rs`
  (the per-material element stiffness closures), `estimate.rs` (`element_samples`, `recover`, `energy_error`), the `grep` of every
  `element_for`, `b_matrix`, `strain_at`, `stiffness_with*`, `point_stiffness` caller in `crates` (only `rcfd-fem`).
- Tests: `sb_static.rs` (the patch, its seed and RNG), `sb_closed_forms.rs` (the cantilever and the cylinder), `si_scaffold.rs`
  (the R4, R4b rows and the records-and-copies scheme), `sa_shell4_membrane_bits.rs` (the digest pattern).
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Cited from memory
  (marked NOT read in `docs/reference-data.md`): T. J. R. Hughes, "Generalization of selective integration procedures to
  anisotropic and nonlinear media", Int. J. Numer. Meth. Engng 15 (1980) 1413-1418 (the B-bar method, the mean dilatation);
  J. C. Nagtegaal, D. M. Parks and J. R. Rice, "On numerically accurate finite element solutions in the fully plastic range",
  Comput. Methods Appl. Mech. Engng 4 (1974) 153-177 (mean dilatation, the volumetric locking of incompressible flow); E. L.
  Wilson, R. L. Taylor, W. P. Doherty and J. Ghaboussi, "Incompatible displacement models", in Numerical and Computer Methods in
  Structural Mechanics (S. J. Fenves et al., eds.), Academic Press, 1973, 43-57 (the modes `1 - xi^2`); R. L. Taylor, P. J.
  Beresford and E. L. Wilson, "A non-conforming element for stress analysis", Int. J. Numer. Meth. Engng 10 (1976) 1211-1219 (the
  Jacobian at the centre and the factor `det J0 / det J` that make the element pass the patch test); J. C. Simo and M. S. Rifai,
  "A class of mixed assumed strain methods and the method of incompatible modes", Int. J. Numer. Meth. Engng 29 (1990) 1595-1638
  (the enhanced strain view: the enhanced field orthogonal to constant stress). The volumes, pages and years are as I recall them
  and were not checked. What is used is **derived** below (arithmetic 1 to 4) and verified by the prototype; no number is taken
  from these sources.

## Arithmetic done before the code (disclosed)

Hex8 of the basic tier (ADR 0273): node order and reference coordinates of `mesh.rs`, the 2 x 2 x 2 Gauss rule `gauss_2` (points
`q`, weights `w_q = 1`), `B_q = b_matrix(grad N(xi_q))`, `K = sum_q w_q det J_q B_q^T D B_q`, Voigt order xx, yy, zz, xy, yz, zx
with engineering shear.

1. **B-bar, mean dilatation (derived).** Split `B = B_dev + B_vol` with `B_vol = (1/3) m m^T B`, `m = (1, 1, 1, 0, 0, 0)`: for
   node `a`, component `c`, the dilatation row is `grad_c N_a`, so `B_vol` has `grad_c N_a / 3` in the three normal rows. Replace
   the dilatation by its element mean, `bbar_a = (sum_q w_q det J_q grad N_a(q)) / V`, `V = sum_q w_q det J_q`: `Bbar_q = B_q +
   (1/3) m (bbar - grad N(q))^T`, i.e. `Bbar[i][3a + c] = B[i][3a + c] + (bbar_a[c] - grad_c N_a(q)) / 3` for the rows `i = 0, 1,
   2`. `K = sum_q w_q det J_q Bbar_q^T D Bbar_q` (any `D`; Hughes's form), the strain recovered at the points is `Bbar_q u`. **The
   patch test:** for a linear field `u = c + A x`, `grad N(q) . u_a` summed gives `A` at every point (isoparametric completeness),
   so the dilatation of `u` is the same at every point and equals its mean: `Bbar_q u = B_q u` exactly, and the element force of the
   linear field is `sum_q w det J Bbar_q^T D eps`; summed over the patch, the interior rows are `sum_q w det J (Bbar_q)^T sigma`
   with a constant `sigma`, and `sum_q w det J Bbar_q^T = sum_q w det J B_q^T + (1/3)(V bbar - sum_q w det J grad N(q)) m^T = sum_q
   w det J B_q^T` (the correction integrates to zero by the definition of `bbar`): the interior equations of the standard element,
   which pass. **Zero modes:** the deviatoric part is integrated fully and the volumetric part keeps the element dilatation, so the
   rigid motions are the only zero modes (a mode with no deviatoric strain at the eight points is affine).
2. **Incompatible modes with the Taylor correction (derived).** Nine internal unknowns `alpha_{k c}`, the modes `P_k = 1 - xi_k^2`
   (`k = 0, 1, 2`) for each displacement component `c`. Wilson's element differentiates `P_k` with the Jacobian at the point:
   `grad P_k(q) = J_q^{-T} dP_k/dxi`, and the enhanced strain does not integrate to zero over a distorted element, so a constant
   stress does work on the modes and the patch test fails (prototype: `3.8e-3`, arithmetic 5). Taylor, Beresford and Wilson's
   correction uses the Jacobian at the centre and the ratio of determinants: `G_k(q) = (det J_0 / det J_q) J_0^{-T} dP_k/dxi(q)`,
   with `dP_k/dxi_j = -2 xi_k delta_{kj}`, so that `sum_q w_q det J_q G_k(q) = det J_0 J_0^{-T} sum_q w_q dP_k/dxi(q) = 0` (the
   Gauss points are symmetric, `sum_q w_q xi_k = 0`): the enhanced strain `B_alpha alpha`, `B_alpha = b_matrix(G_0, G_1, G_2)` (6 x
   9, column `3k + c`), is orthogonal to every constant stress (Simo and Rifai's condition). Element matrices `K_uu = sum w det J
   B^T D B`, `K_ua = sum w det J B^T D B_alpha`, `K_aa = sum w det J B_alpha^T D B_alpha` (9 x 9, positive definite for a positive
   `D`), the condensed stiffness `K = K_uu - K_ua K_aa^{-1} K_au` (static condensation: no internal load), the internal modes
   `alpha = -K_aa^{-1} K_au u_e`, the strain at a point `B_q u_e + B_alpha,q alpha`. **The patch test:** for a linear field
   `K_au u_e = sum w det J B_alpha^T D eps = (sum w det J B_alpha)^T D eps = 0`, so `alpha = 0` and the element force is that of
   the standard element (which passes), on any distortion. **Zero modes:** the condensed energy is `min_alpha` of the energy of
   `(u, alpha)`; a rigid motion has zero energy with `alpha = 0`; a non-rigid `u` would need `B u + B_alpha alpha = 0` at all
   eight points; prototype: six zeros (arithmetic 5).
3. **Exactness in pure bending (derived).** On rectangular elements (`J` constant, `G_k` the true gradients of `P_k`) the 3D field
   of pure bending about `y`, `u_x = -kappa x z'`, `u_y = -nu kappa y' z'`, `u_z = (kappa / 2)(x^2 + nu (z'^2 - y'^2))` (`y', z'`
   from the section centre), lies in the space: `x z'`, `y' z'` are trilinear, `x^2`, `y'^2`, `z'^2` are nodal interpolants plus
   the modes `P_k`. Its stress is `sigma_xx = E kappa z'` only, the only modes driven by it are those of `u_x` in `P_0` whose strain
   `-2 xi alpha` integrates against a `sigma_xx` odd in `z` to zero, so the field satisfies the condensed equations: the element is
   exact for an end couple at any `nu`, and the recovered stress must be `M z' / I` at every point. Without the modes (`B u` only)
   the shear strain `gamma_xz = -kappa x + kappa x_P` is wrong by the `x^2` part (the parasitic shear): prototype below.
4. **Locking of full integration (control estimate).** A fully integrated trilinear element of length `l` and height `h` in pure
   bending carries the parasitic shear `gamma_xz = -kappa x'` (`x'` from the element centre) at its Gauss points, whose energy
   adds to the bending energy in the ratio `G (l/2)^2 (1/3) / (E (h/2)^2 (1/3))`, that is `(l / h)^2 / (2 (1 + nu))`; for `l = h`
   and `nu = 0` the element over-estimates the bending stiffness by one half (ADR 0293), so the deflection ratio is about `2/3`:
   prototype `0.6665` (arithmetic 5), the control limit `0.8`. At `l / h = 5` (a5) the ratio falls
   to about `0.09` (prototype), printed as control C4b.
5. **Prototype numbers (`est1.log`, `est3.log`).** Patch (a2), `0.1 h` distortion, interior error: Full `6.1e-16`, BBar `7.0e-16`,
   Incompatible `5.2e-16`, Wilson's uncorrected `3.76e-3`; stress against `D eps`: `4.5e-15`, `4.1e-15`, `4.1e-15`, `5.8e-2`.
   `0.25 h`: `5.2e-16`, `5.2e-16`, `3.5e-16`, uncorrected `1.27e-2`; stress `3.5e-15` to `3.7e-15`, uncorrected `0.38`. `0.25 h` at
   `nu = 0.4999`: Full `1.1e-15`, BBar `1.16e-13`, Incompatible `5.2e-15`; stress `8.8e-15`, `1.1e-14`, `7.3e-15`. Single
   elements `[AR, 1, 1]`, `nu = 0.3`, all three formulations: six zeros (largest `|lambda_6| / lambda_max <= 1.9e-16`),
   `lambda_7 / lambda_max = 5.128e-2` (AR 1), `6.655e-4` (AR 10), `6.667e-6` (AR 100); symmetric within `3.7e-16`; at `nu =
   0.4999` (observation only) `2.2e-5`, `3.3e-7`, `3.3e-9`; the distorted element of `est1.py` six zeros, `lambda_7 / lambda_max =
   4.46e-2`. Cantilever (a4), the support of reading (a4): Incompatible `nu = 0` **`0.999126`**, `nu = 0.3` **`0.998938`**; Full
   **`0.666500`**, `0.649891`; BBar `0.932530` (`nu = 0`), `1.211744` (`nu = 0.3`: the mean dilatation removes the bending
   dilatation the one-layer element cannot relax, observation O2); full clamp: Incompatible `0.999126`, `0.993516`, Full
   `0.666500`, `0.646354`; Incompatible at 40 and 80 elements along `0.999475`, `0.999610`. L/h = 100, AR 5 (a5): Incompatible
   **`0.999268`** (`nu = 0.3`), `0.999365` (`nu = 0`); Full `0.092856`, BBar `0.099448`. Pure bending by an end couple `M = 10 N m`
   (nodal forces `+- M / (2 h)` in `x` at the four tip nodes, the consistent load of the linear `sigma_xx`), the same support:
   Incompatible tip `u_z / (M L^2 / (2 E I)) = 1.000000000` (`nu = 0`, `0.3`), recovered stress against `M z' / I` relative to
   `M (h/2) / I`: `3.2e-12` (`nu = 0`), `1.7e-11` (`nu = 0.3`); the compatible strain alone `0.289`, `0.233`; Full `0.6667`, BBar
   `0.9333`, `1.2133`. Element identities on the `nu = 0.3` tip-shear cantilever, Incompatible, every element: `|sum_q w det J
   B^T sigma_q - K u_e| / max |K u_e| <= 2.97e-12` (with the compatible strain only `0.544`); `|sum_q w det J B_alpha^T sigma_q| /
   max (sum_q w det J |B_alpha|^T |sigma_q|) <= 6.7e-13`.
6. **The cylinder (`est2.log`).** Bore `max |u_r / u_r^Lame(a) - 1|`, the pressure as consistent nodal forces on the flat bore
   faces: `nu = 0.4999`, k = 1, 2, 4: Full `0.932`, `0.775`, `0.463` (ratios `0.068`, `0.225`, `0.537`); **BBar `6.85e-3`,
   `1.72e-3`, `4.31e-4`**; Incompatible `1.09e-2`, `2.75e-3`, `6.90e-4`. `nu = 0.3` (validation of the prototype against ADR 0433):
   Full `1.15e-2`, `2.92e-3`, `7.31e-4` (second order; ADR 0433 measured `1.8e-4` at k = 8 with rcfd, the prototype's k = 4 value
   divided by four is `1.83e-4`). The registered mesh is k = 2: BBar `1.72e-3` against the limit `1e-2`, Full `0.225` against the
   control `0.9`; k = 1 is too close to the limit for BBar (`6.9e-3`), k = 4 also meets both.
7. **Mutations prototyped (`mut_proto.log`).** MU2 (the factor `det J_0 / det J_q` dropped): patch `1.68e-3`, stress `4.2e-2`.
   MU4 (the B-bar correction `1/2` in place of `1/3`): the patch is untouched (the correction vanishes on a linear field) and the
   cylinder k = 2 bore error is `0.463`.
8. **One-ulp probes and the class of the limits (b5).** The patch and stress limits of `1e-10` are the acceptance's; the
   prototype's errors are `1e-16` to `1.2e-13` (the largest at `nu = 0.4999` with BBar, the condition number of the nearly
   incompressible patch). The symmetry and rigid-motion limits are registered at `1e-13` and `1e-11` relative (F-372's classes),
   against `3.7e-16` measured; they are sums of `8 x 6 x 6` products per entry plus, for Incompatible, a 9 x 9 solve. The
   bending-stress and identity limits are registered at `1e-8` and `1e-9` (not `1e-12` class): the bending solve of a 20-element
   strip is ill-conditioned (`3.2e-12` to `1.7e-11` in the dense prototype; rcfd's direct solver accumulates differently), and the
   identities cancel (`K u_e` is a small resultant of large terms). Every aggregated error in the tests is required finite before
   it is compared (a NaN makes the check fail, never pass).

## Design

### D1. The element (`hex8x.rs`)

`hex8x::PENDING = None`; `hex8x::FOLLOW_UP = "a follow-up of ADR 0493"` (the package named by the refusals of D5).

- `pub fn element_stiffness(formulation: SolidFormulation, coords: &[Vec3], d: &Matrix6) -> Result<ElementMatrix>`: `Full`
  returns `element_for(Hex8).stiffness(coords, d)` (the same call as the assembly, the same bits); `BBar` arithmetic 1; and
  `Incompatible` arithmetic 2. Eight coordinates required (`FieldLength`), every point through `Element::point` (the same `det J
  <= 0` refusal). BBar and the `K_uu` of Incompatible are summed point by point in the loop order of
  `Element::stiffness_with_rule` (both triangles). The condensed part `S = K_ua X`, `X = K_aa^{-1} K_au` by a Cholesky
  factorisation of `K_aa` (refused as `InvalidValue { key: "internal modes" }` if a pivot is not positive and finite), and `K_ij =
  K_uu,ij - (S_ij + S_ji) / 2` (symmetric bit for bit).
- `pub fn point_strains(formulation, coords, d, u_e) -> Result<Vec<[f64; 6]>>`: the strain at the eight points of `gauss_2` in
  the rule's order: `Full` `Element::strain_at` at each point (the recovery's call), `BBar` `Bbar_q u_e`, `Incompatible` `B_q u_e +
  B_alpha,q alpha`.
- `pub fn internal_modes(coords, d, u_e) -> Result<[f64; 9]>`: `alpha = -K_aa^{-1} K_au u_e`, ordered `3 k + c` (mode `k`,
  component `c`).
- `pub(crate) fn opted(mesh, e) -> Option<SolidFormulation>`: the block's formulation of a Hex8 element when it is not `Full`,
  `None` otherwise (every default element: the former path).

### D2. The opt-in (`mesh.rs`)

`set_block_property`: a `Solid` property whose formulation is not `Full` fits a Hex8 block only, refused otherwise with
`UnsupportedKind { operation: "a Hex8 formulation" }` (F-549's pattern; `Solid { Full }` keeps its arm and its message). No other
change: `BlockProperty::solid(f)` exists since ADR 0291.

### D3. The stiffness and the recovery (`assemble.rs`, `recover.rs`)

`assemble_stiffness` (behind `stiffness`, `stiffness_with`, `stiffness_with_materials` and `point_stiffness`, so behind the
static solve on both paths, the modal analysis, the CMS, the cyclic, transient, rotating and harmonic paths): first
`hex8x::check_model(model)?` (D5), then, in the element closure after the structural kinds, an element with `opted(..) =
Some(f)` takes `hex8x::element_stiffness(f, coords, D)` with its material's elasticity matrix. `IntegrationPointField::compute_with`
(behind every recovery: `from_result`, `attach`, nodal averages, the pre-stress of a static result): `hex8x::check_model(model)?`,
then for an opted element the strains of `point_strains` at the rule's points in place of `strain_at` (positions, weights and the
stress call unchanged).

### D4. Which paths take the formulations

By construction through D3: the linear static solve (direct and PCG, elimination and transformation), `assemble::stiffness*`,
the modal analysis, CMS, the cyclic and rotating modal paths without pre-stress, transient and modal harmonic dynamics, and every
recovery. Verified here: the static solve, the stiffness and the recovery (the checks) and the modal analysis being taken
(RF5). Mass, body, pressure and traction loads use the standard Hex8 functions (the stub: "Mass uses the standard Hex8 mass").

### D5. Refusals (`hex8x.rs`, one line each in `thermal.rs`, `geometric.rs`, `damped.rs`, `harmonic_direct.rs`, `estimate.rs`)

`hex8x::check_model(model)` returns at once if no block is opted (every default model); otherwise refuses, with
`NotYetSupported { item: "formulation", variant: f.name(), location: "block {b}, ..." , package: hex8x::FOLLOW_UP }`: a
temperature load (the thermal load and thermal stress would need `Bbar` and the internal load of the modes) and an opted block
whose element set has a per-point material (orthotropic or tabulated, `mat_ortho::is_point_material`). `thermal::add_thermal_load`
calls it (so `nodal_load_vector` refuses as well). `hex8x::refuse_formulations(model, path)` refuses any opted block, naming the
path, in: `geometric::assemble_geometric` (the geometric stiffness, the pre-stressed modes, buckling, the rotating stress
stiffening; the standard `G` matrix with the stress of the formulation is not verified), `damped::damped_modes` and
`DirectHarmonic::new` (their per-material element stiffness closures call `element_for(kind).stiffness`; and the viscoelastic
split into shear and bulk parts is not linear in `D` for the condensed element), `estimate::prepare` (the patch recovery samples
`strain_at`). The nonlinear paths keep their own refusal (`nonlinear::check_model`, key `"nonlinear"`, "solid elements of the Full
formulation only", ADR 0404), now reached instead of the scaffold's "pending" refusal.

### D6. Names in the topic documentation

`docs/reference-data.md` gains an F-390 section (the sources above, NOT read; what is derived).

### D7. The bit record of the default (B1)

`crates/rcfd-fem/tests/si_hex8x_bits.rs`, using only the API of the base aa0e4d5b, run once on the base after this commit and
before any library code; its digests (FNV-1a of the bits) written into `EXPECTED` from that run (`f390-scratch/b1-base.log`):
the distorted Hex8 patch (stiffness, solution, integration-point stress); the 20 x 1 x 1 cantilever with the default property
and with `BlockProperty::solid(Full)` (stiffness, loads, solution, integration-point stress, nodal average; the explicit `Full`
must give the default's digests); the cylinder k = 2 at `nu = 0.4999` (solution); a Hex8 cantilever 8 x 2 x 2 with a temperature
load (loads, solution, stress); its four lowest frequencies; the orthotropic Hex8 stiffness (the point-material path of
`assemble_stiffness`); the geometric stiffness of the cantilever's static pre-stress; `estimate::energy_error` of the cantilever;
the Tet4 and Tet10 patch stiffnesses. The damped and direct harmonic paths keep their own bit records (`sa_damped_*_default_bits`,
`sa_harmonic_visco_default_bits`), run unedited (B2).

### D8. The scaffold records (`si_scaffold.rs`)

The active tests that assert the formulations pending — `x1_the_scaffold_exists_after_shell_landed`,
`r1_to_r9_static_solve_refuses_each_new_variant_after_shell_landed`, `r10_refused_on_every_static_path_after_shell_landed`
(their only rows are R4, R4b) and `r11_modal_and_stiffness_refuse_after_modal_landed` (R4) — get an `#[ignore = "record of ADR
0291 before ADR 0493 (F-390) ..."]` attribute and nothing else; two copies are added: `x1_the_scaffold_exists_after_f390`
(`hex8x::PENDING` and `BBar.pending()` `None`, every other line as the last copy) and `r4_formulations_taken_after_f390` (the
R4 and R4b models solve on the four static paths of R10, the modal analysis and `stiffness`, each `Ok`).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| PT1 | a2 | the patch of reading (a2), `0.1 h` (tag 11), BBar and Incompatible, `solve_static_with`: interior error | `1e-10` | arithmetic 5: `7.0e-16`, `5.2e-16` |
| PT2 | a2 | the same, `0.25 h` (tag 1001), `nu = 0.3` and `nu = 0.4999`, both | `1e-10` | `5.2e-16`, `3.5e-16`; `1.16e-13`, `5.2e-15` |
| RS1 | a7 | the recovered stress of PT1 and PT2 at every point against `D eps` relative to `max |D eps|`, both | `1e-10` | `4.1e-15`; at `nu = 0.4999` `1.1e-14` |
| EV1 | a3 | one element `[AR, 1, 1]`, AR 1, 10, 100, `nu = 0.3`, both: count of `|lambda| <= 1e-10 lambda_max`; `lambda_7 / lambda_max` | `= 6`; `> 1e-6` | 6; `5.13e-2`, `6.66e-4`, `6.67e-6` |
| EV2 | a3 | the distorted element of `est1.py`, both: the same | `= 6`; `> 1e-6` | 6; `4.46e-2` |
| EV3 | a3 | the elements of EV1 and EV2: `max |K - K^T| / max |K|`; the six rigid motions `max |K r| / (max |K| max |r|)` | `1e-13`; `1e-11` | `3.7e-16`; rounding |
| C1 | a4 | cantilever reading (a4), `nu = 0`, Incompatible: `|delta / delta_T - 1|` | `0.01` | `8.7e-4` |
| C2 | a4 | the same, `nu = 0.3` | `0.03` | `1.06e-3` |
| C3 | a4 | the same mesh, default Hex8, `nu = 0` and `nu = 0.3`: `delta / delta_T` (locking control) | `< 0.8` | `0.6665`, `0.6499` |
| C4 | a5 | L/h = 100, aspect ratio 5, `nu = 0.3`, Incompatible: `|delta / delta_T - 1|` | `0.02` | `7.3e-4` |
| C4b | a5 | the same mesh, default Hex8 (control) | `< 0.8` | `0.0929` |
| T1 | a6 | cylinder reading (a6), k = 2, `nu = 0.4999`, BBar: `max_bore |u_r / u_r^Lame(a) - 1|` | `0.01` | `1.72e-3` |
| T2 | a6 | the same mesh, default Hex8: `max_bore u_r / u_r^Lame(a)` (control) | `< 0.9` | `0.225` |
| RS2 | a7 | the end couple of arithmetic 5 on the mesh of C1, Incompatible, `nu = 0` and `0.3`: recovered stress against `(M z'/I, 0, ...)` at every point relative to `M (h/2) / I`; tip `mean u_z / (M L^2/(2 E I))` minus 1 | `1e-8`; `1e-8` | `3.2e-12`, `1.7e-11`; `< 1e-9` |
| RS3 | a7 | C2's solution, every element: `|sum_q w det J B_q^T sigma_q - K_e u_e| / max |K_e u_e|` with the field's stress and `K_e = element_stiffness(Incompatible)`; `|sum_q w det J G_q^T sigma_q| / max(sum_q w det J |G_q|^T |sigma_q|)` with `G` built in the test from arithmetic 2 | `1e-9`; `1e-9` | `3.0e-12`; `6.7e-13` |
| X1 | D1 | the test's own Incompatible element (arithmetic 2 from `Element::point`, dense Gauss elimination) against `element_stiffness(Incompatible)` on EV2's element and on PT1's elements, relative to `max |K|`; `internal_modes` against the test's `alpha` on C2's elements relative to `max |alpha|` | `1e-11`; `1e-9` | rounding |
| D1 | D3 | a single-element model: `stiffness(model)` equals `element_stiffness(f, ..)` bit for bit, both; the C2 model `stiffness_with` 1 and 8 threads bit for bit | exact | D3 |
| D2 | D3 | the C2 model: `IntegrationPointField::compute` strains of each element equal `point_strains(Incompatible, ..)` bit for bit; the same for PT1 with BBar | exact | D3 |
| RF1 | D2 | `BlockProperty::solid(BBar)` and `(Incompatible)` on a Tet4, a Tet10 and a Hex20 block refused (`UnsupportedKind`, "a Hex8 formulation"); `solid(Full)` on a Tet4 block taken | exact | D2 |
| RF2 | D5 | the C2 model (both formulations) with a temperature load: `solve_static`, `nodal_load_vector` and `IntegrationPointField::compute` refused (`NotYetSupported`, item "formulation", the variant, package `FOLLOW_UP`); a BBar block of an orthotropic material refused by `solve_static` the same way | exact | D5 |
| RF3 | D5 | the C2 model with BBar: `geometric_stiffness`, `damped_modes`, `DirectHarmonic::new`, `estimate::recover` refused (`NotYetSupported`, item "formulation"); `nonlinear::internal_force` and `nonlinear::tangent` refused with `InvalidValue { key: "nonlinear" }` | exact | D5 |
| RF4 | D4 | the C2 model with each formulation: `modal_analysis` with 2 modes `Ok`, finite frequencies | exact | D4 |
| B1 | a1 | the digests of `si_hex8x_bits` equal the base run; the explicit `Full` digests equal the default's | exact | D7 |
| B2 | a1 | `cargo test -p rcfd-fem --release`: every test passes, the four scaffold tests of D8 ignored as records and their copies passing; no other existing test edited | pass | D8 |
| B3 | a1 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |

### Negative controls

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | Wilson's incompatible element without the correction (the Jacobian at each point, test-side through `assemble_with`) on PT1's patch | the patch test fails | interior error `> 1e-6` (`3.76e-3`) |
| N2 | C3, C4b, T2 above (the locking controls of the acceptance) | locked | as registered |
| N3 | RS2 with the compatible strain only (`Element::strain_at` of the default Hex8 on the Incompatible solution) | the modes matter | stress error `> 0.1` (`0.289`, `0.233`) |
| N4 | RS3 with the compatible strain only | the modes matter | first identity `> 0.1` (`0.544`) |
| O1 | C1, C2 with the full clamp, printed | `0.9991`, `0.9935` | observation |
| O2 | BBar on the meshes of C1, C2, C4, printed | `0.9325`, `1.2117`, `0.0994` | observation (B-bar does not relieve the parasitic shear) |
| O3 | Incompatible on T1's mesh, printed | `2.75e-3` | observation |
| O4 | EV1 at `nu = 0.4999`, printed | `lambda_7 / lambda_max` `2.2e-5` to `3.3e-9` | observation |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | Incompatible without the Taylor correction (`J_q^{-T}` in place of `(det J_0 / det J_q) J_0^{-T}`) | PT1, PT2, RS1, X1 |
| MU2 | the factor `det J_0 / det J_q` dropped (arithmetic 7) | PT1, PT2, RS1, X1 |
| MU3 | the B-bar correction zero (BBar becomes Full) | T1 |
| MU4 | the B-bar correction `1/2` in place of `1/3` (arithmetic 7) | T1 |
| MU5 | the recovery of Incompatible without `alpha` (`B_q u_e` only) | RS2, RS3, D2 |
| MU6 | the condensation with the wrong sign (`K_uu + K_ua K_aa^{-1} K_au`) | C1, C2, C4, EV1, X1 |
| MU7 | `assemble_stiffness` ignores the formulation (every Hex8 Full) | C1, C2, C4, T1, D1 |
| MU8 | `compute_with` ignores the formulation | RS2, RS3, D2 |
| MU9 | `check_model` does not refuse the temperature load | RF2 |
| MU10 | `set_block_property` accepts a formulation on a Tet4 block | RF1 |
| MU11 | a default Hex8 element takes the BBar branch | B1, the existing Hex8 tests |
| MU12 | `refuse_formulations` returns `Ok` | RF3 |

## Decision rule

- **done:** PT1, PT2, RS1, EV1 to EV3, C1 to C4b, T1, T2, RS2, RS3, X1, D1, D2, RF1 to RF4, B1 to B3 met, the negative controls as
  stated, every mutation caught by a registered check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the formulations do not build.

## Limits (not covered)

- L1. Linear elasticity only. The nonlinear driver (J2, creep, viscoelastic, geometric nonlinearity) refuses both formulations
  (ADR 0404's refusal, unchanged). F-580 needs B-bar (or F-bar) in that driver: proposed as a follow-up.
- L2. A temperature load, per-point materials (orthotropic, tabulated), the geometric stiffness (pre-stressed modes, buckling,
  rotating stress stiffening), the damped modes, the direct harmonic solve and the patch recovery refuse the formulations (D5).
- L3. The two formulations are not combined in one element (the stub). Hex8 only: Tet4, Tet10, Hex20 refuse them (D2).
- L4. Mass, body, pressure and traction loads use the standard Hex8 functions; the modal analysis is taken by construction, not
  checked against a closed form here.
- L5. B-bar keeps the shear locking of the trilinear element in bending (O2): it is for near incompressibility, not for thin
  bending on one layer.
- L6. The internal modes are recomputed from the displacement at recovery (no stored `alpha`), consistent because the condensation
  has no internal load.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The cantilever support with one element through the thickness (reading (a4)): `u_x`, `u_z` on the root face and `u_y` at one
   root node (taken) rather than the full clamp (printed as O1; it adds the lateral Poisson restraint at the root, `0.9935` at
   `nu = 0.3`, still within the limit) or a neutral-line support that the mesh does not have.
2. The cylinder mesh k = 2 (taken, arithmetic 6) rather than k = 1 (B-bar `6.9e-3`, close to the limit) or k = 4 (both clauses
   also met, a weaker control).
3. The paths of D5 refuse the formulations (taken) rather than taking them unverified; each is a follow-up when needed.

## Addendum 1: results (2026-11-14)

The registered run is commit b10dfe26 (library 74b97d03, the checks and the scaffold step b10dfe26), `f390-scratch/run1.log`,
the three files `si_hex8x`, `si_hex8x_bits`, `si_scaffold`, made once. Before it, B1 had been run once on the uncommitted
library code identical to 74b97d03 (`b1-run.log`, pass); its only earlier run is the base recording (`b1-base.log`, commit
1744a1f2). After the run, `cargo fmt -p rcfd-fem` changed the layout of the F-390 lines only (3f2013f4); the three files passed
again with identical printed numbers (`after-fmt.log`). No amendment was written; no limit was changed.

Deviations, disclosed: (1) the content of B1 (D7) was adjusted while it was recorded on the base, before any library code:
`stiffness()` refuses the orthotropic material (the static solve's solution is recorded instead) and the one-layer cantilever has
no interior node for the patch recovery (the energy-norm estimate is recorded on the clamped 8 x 2 x 2 box instead); 26 digests.
(2) One setup defect was corrected before the registered run, while the test file was being compiled and before it was ever run:
the end couple of RS2 bends the tip downwards, so the tip check compares `-mean u_z` with `M L^2 / (2 E I)` (the prototype printed
`-1.000000000`, arithmetic 5).

| id | limit | measured | met |
| --- | --- | --- | --- |
| PT1 | `1e-10` | BBar `5.2e-16`, Incompatible `4.4e-16` | yes |
| PT2 | `1e-10` | `nu = 0.3`: `5.2e-16`, `4.4e-16`; `nu = 0.4999`: `8.2e-14`, `4.8e-15` | yes |
| RS1 | `1e-10` | largest `1.3e-14` (Incompatible, `nu = 0.4999`); `3.3e-15` to `4.1e-15` at `nu = 0.3` | yes |
| EV1 | 6; `> 1e-6` | both formulations 6 zeros (`|lambda_6| / lambda_max <= 1.4e-16`); `lambda_7 / lambda_max` `5.128e-2` (AR 1), `6.655e-4` (AR 10), `6.667e-6` (AR 100) | yes |
| EV2 | 6; `> 1e-6` | 6; `4.459e-2` (BBar), `4.455e-2` (Incompatible) | yes |
| EV3 | `1e-13`; `1e-11` | symmetry `<= 3.0e-16`; rigid motions `<= 6.4e-16` | yes |
| C1 | `0.01` | `8.74e-4` (`delta / delta_T = 0.999126`) | yes |
| C2 | `0.03` | `1.06e-3` (`0.998938`) | yes |
| C3 | `< 0.8` | `0.666500` (`nu = 0`), `0.649891` (`nu = 0.3`) | yes |
| C4 | `0.02` | `7.32e-4` (`0.999268`) | yes |
| C4b | `< 0.8` | `0.092856` | yes |
| T1 | `0.01` | `1.721e-3` | yes |
| T2 | `< 0.9` | `0.224875` | yes |
| RS2 | `1e-8`; `1e-8` | stress `1.7e-11` (`nu = 0`), `2.7e-12` (`nu = 0.3`); tip `1.7e-11`, `2.9e-12` | yes |
| RS3 | `1e-9`; `1e-9` | `4.8e-12`; `9.5e-13` | yes |
| X1 | `1e-11`; `1e-9` | `2.6e-16`; `1.3e-12` | yes |
| D1 | exact | bit for bit (single element, both; 1 and 8 threads, both) | yes |
| D2 | exact | bit for bit (C2 both, PT1 BBar) | yes |
| RF1 | exact | refused on Tet4, Tet10, Hex20 with "a Hex8 formulation"; Full on Tet4 taken | yes |
| RF2 | exact | temperature: `solve_static`, `nodal_load_vector`, `IntegrationPointField::compute` refused ("block 0, load 1 (a temperature load)"); orthotropic refused ("the per-point material Orthotropic of element 0") | yes |
| RF3 | exact | geometric stiffness, damped modes, direct harmonic, patch recovery refused naming the path; `nonlinear::internal_force`, `tangent` refused with key `"nonlinear"` | yes |
| RF4 | exact | modal taken: BBar `18.48`, `18.53` Hz; Incompatible `20.36`, `20.43` Hz | yes |
| B1 | exact | 26 digests equal the base; explicit `Full` equal to the default | yes |
| B2 | pass | `cargo test -p rcfd-fem --release` (3f2013f4): 686 passed, 0 failed, 174 ignored (`b2-fem.log`); the four scaffold tests of D8 ignored as records, `x1_the_scaffold_exists_after_f390` and `r4_formulations_taken_after_f390` pass | yes |
| B3 | pass | `baseline_hashes`: 3 passed, 1 ignored (`b3.log`) | yes |

Negative controls: N1 `3.75e-3` (`> 1e-6`); C3, C4b, T2 above; N3 `0.289` (`nu = 0`), `0.233` (`nu = 0.3`) (`> 0.1`); N4 `0.544`
(`> 0.1`). Observations: O1 full clamp `0.999126`, `0.993516`; O2 BBar `0.932530`, `1.211744`, `0.099448`; O3 Incompatible on T1's
mesh `2.75e-3`; O4 `lambda_7 / lambda_max` at `nu = 0.4999` `2.22e-5`, `3.32e-7`, `3.33e-9` (six zeros each). Every measured
number agrees with the numpy prototype of arithmetic 5 and 6 to its printed digits.

Mutations (archive copy of 3f2013f4 with its own target, `f390-scratch/mutate.py`, `mutlogs/`), each caught:

| id | caught by |
| --- | --- |
| MU1 | PT1, PT2, RS1, X1 |
| MU2 | PT1, PT2, RS1, X1 |
| MU3 | T1 |
| MU4 | T1 |
| MU5 | RS2, RS3 (D2 does not see it: it compares the field with `point_strains`, which carries the mutation; registered as a catcher by mistake) |
| MU6 | C1, C2, C4, RS2, RS3, X1 (EV1 does not see it: `K_au r = 0` for a rigid motion, so the wrong sign keeps six zeros) |
| MU7 | C1, C2, C4, T1, D1, RS2 |
| MU8 | RS2, RS3, D2 |
| MU9 | RF2 |
| MU10 | RF1 |
| MU11 | B1, ADR 0291's `si_scaffold_bits` B1 to B4, C3, T2 (run with `--no-fail-fast`, `MU11-nofailfast.log`; `sb_static`, `sb_closed_forms`, `sb_recover`, `sb_elements` pass with B-bar in place of the default) |
| MU12 | RF3 |

fmt (`cargo fmt -p rcfd-fem --check`) and clippy (`cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`): clean.
`rcfd-cli`'s `docs_site` test (for `docs/reference-data.md`): 2 passed (`docs-site.log`). Not run: the whole workspace suite (the lead's
gate).

Shared solver files changed (`git diff aa0e4d5b`, all in `crates/rcfd-fem/src`): `mesh.rs` (one guarded arm in
`set_block_property`), `assemble.rs` (the `check_model` call and the dispatch in `assemble_stiffness`), `recover.rs` (the
`check_model` call and the strains of an opted element in `compute_with`), `thermal.rs`, `geometric.rs`, `damped.rs`,
`harmonic_direct.rs`, `estimate.rs` (one refusal call each); `hex8x.rs` is this package's module.

**Decision: done**, by the rule: every registered check met in the single run, every negative control as stated, all twelve
mutations caught by registered checks (two predicted catchers did not see their mutation, disclosed above; each mutation was
caught by others), no amendment. The acceptance's clauses: two opt-in formulations with the default bit for bit (B1 to B3); the
F-374 patch on two distortions within `8.2e-14` (PT1, PT2); six zero eigenvalues, the seventh at least `6.7e-6` of the largest at
aspect ratios 1, 10, 100 (EV1); the one-layer cantilever within `8.7e-4` (`nu = 0`) and `1.1e-3` (`nu = 0.3`), full integration
`0.667` (C1 to C3); L/h = 100 at aspect ratio 5 within `7.3e-4` (C4); the cylinder at `nu = 0.4999` with B-bar within `1.7e-3`
on the 8 x 16 mesh where full integration gives `0.225` (T1, T2); the recovered stress on the patch equal to `D eps` within
`1.3e-14` and the condensed modes recovered for the stress (RS1 to RS3).

Remaining questions (options, recommendation):

1. B-bar for the nonlinear driver (F-580 needs it): (a) a follow-up giving the small-strain driver (J2, creep, viscoelastic) the
   `Bbar` strain and the tangent `sum w Bbar^T C_t Bbar`, and the total Lagrangian driver F-bar; (b) fold it into F-425 (which
   asks for a mean-dilatation element for hyperelasticity). Recommend (a), deps F-390, F-411, F-397, with F-425 building on it.
2. The refused linear paths (D5: temperature, per-point materials, geometric stiffness, damped and direct harmonic, patch
   recovery): (a) one follow-up lifting them with registered checks; (b) leave them refused until a model needs them. Recommend
   (a) at a low priority, the temperature load first (thermal stress in a nearly incompressible part).
3. The default formulation stays `Full` (changing it would change every Hex8 result and the hash tables; the maintainer's call).

## For the lead: roadmap entries

Proposed status of F-390: **done**.

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0293. ADR 0493, 2026-11-14: two opt-in Hex8 formulations through `BlockProperty::solid(SolidFormulation::BBar | Incompatible)` (refused on other kinds), filled in rcfd_fem::hex8x (element_stiffness, point_strains, internal_modes) and dispatched by assemble_stiffness and IntegrationPointField: the mean-dilatation B-bar (Hughes) and nine incompatible modes (Wilson et al.) with the Taylor, Beresford and Wilson correction (Jacobian at the centre, det J0/det J), condensed and recovered for the stress. Met: the F-374 patch on two distortions within 8.2e-14 (Wilson's uncorrected element 3.7e-3, the control); six zero eigenvalues, the seventh 5.1e-2, 6.7e-4, 6.7e-6 of the largest at aspect ratios 1, 10, 100; the one-layer cantilever L/h = 20 at 0.99913 (nu = 0) and 0.99894 (nu = 0.3) of the closed form, full integration 0.667 (control); L/h = 100 at aspect ratio 5 0.99927; Lame cylinder nu = 0.4999 on 8 x 16 with B-bar within 1.7e-3, full integration 0.225 (control); recovered stress on the patch within 1.3e-14 of D eps, pure bending exact to 1.7e-11 with the recovered modes (0.29 without). Static, modal and every recovery take them; temperature loads, per-point materials, the geometric stiffness, damped and direct harmonic solves, the patch recovery and the nonlinear driver refuse them. Defaults bit-identical (26 digests), baseline_hashes unchanged; twelve mutations caught.

Follow-up 1:

> Title: B-bar and F-bar Hex8 in the nonlinear driver (J2, creep, viscoelastic, geometric nonlinearity). Deps: F-390, F-411, F-397.

> Every limit not stated here is registered in the ADR before the run. The BBar formulation of F-390 taken by the nonlinear driver: at small strain the strain Bbar u at the points and the tangent sum w Bbar^T C_t Bbar, with geometric nonlinearity the F-bar deformation gradient (det F_0 / det F)^(1/3) F; the tangent against central differences within 1e-6; the F-390 patch test within 1e-10 through the driver; the J2 thick cylinder in plane strain and the creep cylinder of ADR 0489 point-wise within limits registered there, full integration the registered locking control; the Full results of ADR 0404, 0475, 0485 and 0489 bit-identical.

Follow-up 2:

> Title: the Hex8 formulations on the remaining linear paths (temperature load, per-point materials, geometric stiffness, damped and direct harmonic solves, patch recovery). Deps: F-390.

> Every limit not stated here is registered in the ADR before the run. The refusals of ADR 0493 D5 lifted with consistent arithmetic: the free thermal expansion of the distorted F-374 patch stress-free within 1e-10 with both formulations; an orthotropic patch within 1e-10; Euler buckling of a one-layer Incompatible strip within registered limits; the damped modes and the direct harmonic solve with the formulations' stiffness (the viscoelastic shear and bulk split refused for Incompatible, whose condensed stiffness is not linear in D); the patch recovery with the formulations' strain; defaults bit-identical.

Note for F-425 (to append):

> ADR 0493 (2026-11-14): the mean-dilatation B-bar Hex8 exists for the linear paths (BlockProperty::solid(SolidFormulation::BBar)); the nonlinear driver refuses it; its F-bar form for finite strain is proposed as follow-up 1 of ADR 0493.
