# ADR 0546: F-598 registration: a warping degree of freedom for Beam2 (the 14-DOF element) and the I-section lateral-torsional buckling under uniform moment

Status: **REGISTERED 2026-11-15, written before any code of F-598 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-598 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with two plain numpy scripts in the scratch folder
(`w49i-work/f598-scratch/arith.py`, log `arith.log`: the section, the closed forms and a cubic-Hermite Euler-Vlasov finite
element model of the lateral-torsional problem to estimate the discretisation error; `arith2.py`, log `arith2.log`: the
non-uniform torsion of a cantilever on the same element; no rcfd code; disclosed, their numbers quoted below). The bit record B0
of the default paths is a test file written and run on the base after this commit and before any library code (D9). Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-598 (deps F-558), structural solver, advanced tier, the follow-up named by
[ADR 0508](0508-f558-registration-beam-moment-shear-wagner-and-shell-rotation-geometric-terms.md) (its question Q1 and the
recorded miss `ltb_i_section_known_miss`, -19.2 percent). Number 0546 given by the lead. Branch `f598-work` from f69eb128,
worktree `w49i-work/f598`, scratch `w49i-work/f598-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-598 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A warping degree of freedom for Beam2 (a 14-DOF element),
> opt-in, with per-section Wagner coefficients from the section properties: the lateral-torsional buckling moment of a simply
> supported I beam under uniform moment equals (pi/L) sqrt(E I_z G J) (1 + (pi^2 E C_w)/(G J L^2))^(1/2) within 1 percent at 16
> elements (the clause of F-558 missed without warping); every result without the option bit-identical.

The brief adds: register the numbering of the seventh nodal unknown and how it coexists with six-unknown nodes (opt-in per block;
mixed models, constraints and supports; refuse what is not supported), the warping stiffness `E C_w`, the boundary conditions of
"simply supported" for warping (fork supports, warping free), the Wagner coefficient form of a doubly symmetric section and its
source (cited from memory in `docs/reference-data.md`, flagged), the shape functions (cubic Hermite for the twist); a
Saint-Venant-only negative control (`C_w = 0` recovers the F-558 result); mutations; defaults bit-identical with the
default-bits record first; `baseline_hashes` unchanged; no case-file key (open question); every aggregated test error required
finite. Other agents edit `nonlinear.rs` (F-618), `nl_transient` (F-611) and the buckling refusals for Shell3 (F-541): this item
does not edit `buckling.rs`, `geometric.rs`, `nonlinear.rs` or `nl_transient*` (D8).

Clauses: **a0** every limit registered here; **a1** a warping unknown for Beam2, a 14-unknown element, opt-in; **a2** per-section
Wagner coefficients from the section properties; **a3** the LTB moment of a simply supported I beam under uniform moment within
`1e-2` of the closed form at 16 elements; **a4** every result without the option bit-identical.

### Readings, registered

- **"Opt-in"**: a separate entry, `beam_warping::buckling_factors(model, prestress, warping, options)`, with a `Warping` value
  that names the Beam2 blocks that take the warping unknown and their `WarpingSection`. Nothing else in the library reads it; no
  existing function changes behaviour; no case-file key. The pre-stressed modes, the static solve, the rotating part and the
  nonlinear paths do not take the warping unknown (Limits).
- **"Per-section Wagner coefficients from the section properties"**: each warping block has its own `WarpingSection { c_w,
  wagner = (beta_y, beta_z) }`; the warping path uses these per block instead of the one model-wide pair of
  `Prestress::with_beam_resultants` (ADR 0508 Q3). The constructor `WarpingSection::doubly_symmetric_i(b_f, t_f, h_w, t_w,
  shear)` computes the `BeamSection` and `C_w`, `beta` from the plate dimensions (D1); for this section `beta_y = beta_z = 0` by
  symmetry. A general `WarpingSection::new(c_w, beta_y, beta_z)` takes caller values in the ADR 0508 convention (centroidal
  integrals; the element has no shear-centre offset, Limits).
- **"Simply supported"**: fork supports, as ADR 0508: `v = w = phi = 0` at both ends, `u = 0` at node 0, the bending rotations
  free, **the warping unknown free at both ends** (no constraint on it: the bimoment is zero there, the natural condition
  `phi'' = 0`). The closed form is that of these conditions (arithmetic 1).
- **"Uniform moment"**: equal and opposite end moments `M_ref` about the strong axis `e_2` by nodal moments, the pre-stress the
  static solve of the six-unknown model (ADR 0508 D3) with `Prestress::with_beam_resultants` (D6: torsion-free, so the warping
  unknown does not change the pre-stress).
- **"I beam"**: the section of ADR 0508 arithmetic 2 (flanges `b_f = 0.1`, `t_f = 0.01`, web `h_w = 0.18`, `t_w = 0.006`), `L =
  3`, steel `E = 2e11`, `nu = 0.3`, shear factor `5/6` both directions, `C_w` of D1.
- **"Bit-identical"**: the default-bits record B0 (D9) on the base and after; and no existing source file changes behaviour
  (only visibility of private items of `beam.rs` and one `pub mod` line, D8).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0508 (whole: D1 the second-order strains, D3 the end forces, the I1 record, Q1 to Q3); ADR
  0473 D1 (the axial-force geometric stiffness, Wagner's `N I_p/A` term).
- `crates/rcfd-fem/src`: `beam.rs` (whole), `geometric.rs` (`Prestress`, `masked`, `assemble_geometric`), `geometric_struct.rs`
  (the pre-stress of beams), `buckling.rs` (whole: `factors_with`, the transformation branch, the zero-shift rule), `model.rs`
  (`DofMap`), `mesh.rs` (`BeamSection`, `BlockProperty`), `constraint.rs` (`reduce`, `prescribed_equations`), `assemble.rs`
  (`element_dofs`), `rcfd-linalg` `CsrMatrix::from_triplets`, `BucklingEigenOptions`.
- Tests: `sa_geometric_struct_f558.rs` (the LTB set-up, `ltb_i_section_known_miss`), `sa_geometric_struct_f558_bits.rs` (the
  digest pattern).

## Design

### D1. Section: warping constant and Wagner coefficients (cited from memory, derived)

Vlasov's thin-walled theory (V. Z. Vlasov, Thin-Walled Elastic Beams, 1940/1961; S. P. Timoshenko and J. M. Gere, Theory of
Elastic Stability, 2nd ed., 1961, ch. 5 and 6; N. S. Trahair, Flexural-Torsional Buckling of Structures, 1993, ch. 2 and 6:
NOT read, from memory): the section twisting by `phi(x)` about its shear centre warps out of its plane by `u_w = -omega phi'`,
`omega` the sectorial coordinate; the warping normal strain `-omega phi''` stores `(1/2) E C_w phi''^2` per length, `C_w = int
omega^2 dA`. For a doubly symmetric I with flanges `b_f x t_f` at the distance `h_c = h_w + t_f` between flange mid-planes, the
web's sectorial coordinate is zero and each flange's is `omega = y h_c/2` (up to sign), so `C_w = 2 (h_c/2)^2 t_f b_f^3/12 = t_f
b_f^3 h_c^2/24` (= `I_f h_c^2/2`, `I_f` the second moment of one flange about the web axis; the usual `I_z h^2/4` with the web's
`h_w t_w^3/12` neglected). The other constants of the constructor, thin-walled with the flange and web rectangles: `A = 2 b_f t_f +
h_w t_w`, `I_y = 2 (b_f t_f^3/12 + b_f t_f (h_c/2)^2) + t_w h_w^3/12` (strong, about `e_2`), `I_z = 2 t_f b_f^3/12 + h_w t_w^3/12`
(weak), `J = (2 b_f t_f^3 + h_w t_w^3)/3`; these are exactly the expressions of ADR 0508's I1 record, so the `BeamSection` is the
same (the record's `C_w = I_z h_c^2/4` differs from D1's by the web term, `-0.19` percent, arithmetic 1).

Wagner coefficients (ADR 0508 D1 convention, Trahair op. cit. for the monosymmetric `beta_x`, from memory): `beta_y = int z (y^2
+ z^2) dA/I_y`, `beta_z = int y (y^2 + z^2) dA/I_z`. For the doubly symmetric I the integrands are odd in `z` (resp. `y`) over a
section symmetric under `z -> -z` (resp. `y -> -y`): `beta_y = beta_z = 0` exactly; the constructor returns `[0.0, 0.0]`, not a
quadrature. Refused: a non-positive or non-finite dimension (key the dimension's name); `h_w <= 0`.

### D2. The 14-unknown element (first principles)

Local unknowns, node 0 then node 1: `(u, v, w, phi, theta_y, theta_z, chi)`, `chi = phi'` the warping unknown (the rate of twist,
whose nodal continuity carries the bimoment between elements). Index map from the 12-unknown order: `i -> i` for `i < 6`, `i -> i
+ 1` for `i >= 6`; `chi_0` is 6, `chi_1` is 13.

- **Twist**: cubic Hermite, `phi(xi) = H_1 phi_0 + H_2 chi_0 + H_3 phi_1 + H_4 chi_1`, `xi = x/L`, `H_1 = 1 - 3 xi^2 + 2 xi^3`, `H_2 =
  L (xi - 2 xi^2 + xi^3)`, `H_3 = 3 xi^2 - 2 xi^3`, `H_4 = L (xi^3 - xi^2)`.
- **Stiffness**: the 12-unknown `local_stiffness` of ADR 0417 embedded by the index map, with its torsion pair `G J/L` on `(phi_0,
  phi_1)` replaced by the Hermite energy `(1/2) int (G J phi'^2 + E C_w phi''^2) dx` on `(phi_0, chi_0, phi_1, chi_1)`:
  `G J/(30 L) [36, 3L, -36, 3L; 3L, 4L^2, -3L, -L^2; -36, -3L, 36, -3L; 3L, -L^2, -3L, 4L^2] + E C_w/L^3 [12, 6L, -12, 6L; 6L, 4L^2,
  -6L, 2L^2; -12, -6L, 12, -6L; 6L, 2L^2, -6L, 4L^2]` (closed form of the integrals, arithmetic 3). The axial and bending entries
  are the 12-unknown ones bit for bit (the ADR 0417 element has no twist-bending coupling: the shear centre is the centroid).
- **Geometric stiffness**: the terms of ADR 0473 D1 and ADR 0508 D1 with the twist field `phi` and its slope `phi'` of the Hermite
  functions in place of the linear ones: the axial and plane terms of `local_geometric_stiffness` embedded (they do not read the
  twist); Wagner's `N (I_y + I_z)/A int phi'^2` on the Hermite (`N I_p/(30 A L)` times the first matrix above); the moment and
  shear-force terms of ADR 0508 D1 with the rows of `phi`, `phi'` replaced by the Hermite rows (the other rows embedded); the
  per-section Wagner terms `(1/2) int (beta_y M_y - beta_z M_z) phi'^2 dx` with the block's `beta`. Four-point Gauss (the
  integrands have degree at most 5 with the linear resultants: exact).
- **Global matrix**: `T^T K_l T`, `T = diag(R, R, 1, R, R, 1)`, `R` the rows `e_1, e_2, e_3` of the frame; the warping unknown is a
  scalar (a rate of twist about the element axis).

### D3. Numbering of the seventh unknown (registered)

The six-unknown `DofMap` of the model is unchanged (`model.dof_map()`, `n` equations). The warping unknowns are appended after
it: the nodes of the elements of the warping blocks, in ascending node order, get the equations `n, n + 1, ...`
(`WarpingDofs::equation(node)`). The element's 14 global unknowns are `(eq(n_0, 0..6), w(n_0), eq(n_1, 0..6), w(n_1))`. A warping
unknown is shared by every warping element at its node (continuity of `phi'`, hence of the bimoment's work).

### D4. Mixed models, constraints, supports (registered; refusals with `FemError::InvalidValue` and the key)

Supported: warping blocks beside any other blocks (beams without warping, shells, solids) that share **no node** with a warping
element; the model's fork constraints on the six unknowns (zero values); warping free by default; `Warping::restrain(node_set)`
holds `chi = 0` at the set's nodes. Refused:

| id | refusal | key |
|---|---|---|
| RF1 | a block named twice, a block that is not Beam2 or has no Beam property, an empty block list | `"warping"` |
| RF2 | a node of a warping element that is also a node of an element outside the warping blocks | `"warping"` |
| RF3 | a node shared by two warping elements whose frames differ (`max |e_k - e_k'| > 1e-9` for `k = 1, 2`: a kink, a reversed element, a rotated section; the warping unknown would not be the same quantity) | `"warping"` |
| RF4 | the transformation path (linear constraints, rigid sets, cyclic constraints, elastic supports) | `"warping"` |
| RF5 | a nonzero prescribed value (as the base path) | `"constraint"` |
| RF6 | a restrained node set that does not exist (`UnknownSet`) or holds a node without a warping unknown | `"warping"` |
| RF7 | `c_w` negative or not finite, `beta` not finite | `"c_w"`, `"wagner"` |
| RF8 | a pre-stress with end forces (`with_beam_resultants`) whose torque on a warping element exceeds `1e-9` of the element's scale `max(|M|, |V| L, |N| sqrt(I_p/A))` (the pre-stress is that of the six-unknown, Saint-Venant static solve; with torsion it would differ from the warping model's) | `"warping"` |
| RF9 | `coordinate_order` set in the options (not offered on this path) | `"factorisation"` |

### D5. Assembly

`K_aug = embed(K) + sum_e T^T (K14_l - embed(K12_l)) T` (the difference is the torsion block only: the other entries subtract to
an exact zero), `K` the model's `point_stiffness`; `K_sigma,aug = embed(assemble_geometric(model, prestress.masked(not a warping
element))) + sum_e T^T K_sigma14_l T`. Both are built from triplets on the union of their patterns (zero entries kept so the
patterns are equal). Then as `buckling::factors_with`: the prescribed equations of the model plus the restrained warping
equations are eliminated (`reduce`), the zero-shift pivot rule of ADR 0402 (`FemError::Solver` naming the unknown, the warping
one as "warping unknown of node k"), `lowest_buckling_factors` with the options. `matrices(model, prestress, warping, threads)`
returns `K_aug`, `K_sigma,aug` and the numbering (for the checks).

### D6. Pre-stress

The `Prestress` of the six-unknown static solve, as ADR 0508: per element `N` from its stress point and, when set, the twelve local
end forces. Torsion-free reference loads only (RF8); under uniform moment the twist of the static solution is zero, so it is the
pre-stress of the warping model too.

### D7. API (new module `rcfd_fem::beam_warping`)

`WarpingSection` (`new`, `saint_venant`, `doubly_symmetric_i`, `c_w`, `wagner`); `Warping` (`new`, `block`, `restrain`);
`WarpingDofs`; `local_stiffness`, `local_geometric_stiffness` (14 x 14, local), `element_stiffness`, `element_geometric_stiffness`
(global); `matrices`; `buckling_factors` returning `WarpingBucklingResult` (factors, modes of `n + n_w` values, residuals,
iterations, Sturm check, unknowns, exhausted, the numbering).

### D8. Files

New: `src/beam_warping.rs`, tests `tests/sa_beam_warping_f598.rs` and `tests/sa_beam_warping_f598_bits.rs`. Changed: `src/lib.rs`
(one `pub mod` line), `src/beam.rs` (private helpers made `pub(crate)`: `moduli`, `place_pair`'s neighbours as needed,
`second_order_rows` and its struct, `end_resultants`, `frame_of`, the Gauss constants; no body changed). Not touched:
`buckling.rs`, `geometric.rs`, `geometric_struct.rs`, `nonlinear.rs`, `nl_transient*`, `shell3.rs`. Docs:
`docs/reference-data.md` (an F-598 section), `docs/structural-solver.md` (a paragraph) if it has a beam section.

### D9. The default-bits record

`tests/sa_beam_warping_f598_bits.rs` is written and run on the base (f69eb128 plus this ADR, no library line changed): digests
(FNV-1a 64 of ADR 0291) of the I-section Beam2 element's local and global stiffness (axis along `(1, 1, 0)/sqrt 2`), its global
axial and resultant geometric stiffness for a set of end forces, the static displacement of the I-beam LTB model, its assembled
geometric stiffness with the resultants, the buckling factors and modes of the F-558 path for it (the `ltb_i_section_known_miss`
set-up: the number -19.2 percent of ADR 0508 stays) and of the rectangle model. It runs unchanged after the change (check B0).

## Arithmetic done before the code (disclosed)

1. **Section and closed forms** (`arith.log`): `A = 3.08e-3`, `I_z = 1.66991e-6`, `I_y = 2.09827e-5`, `J = 7.96267e-8`, `h_c = 0.19`,
   `C_w = 1.504167e-8` (ADR 0508's `1.507091e-8`: ratio 0.99806). `M_cr(L) = (pi/L) sqrt(E I_z G J (1 + pi^2 E C_w/(G J L^2)))`:
   `L = 1.5`: `M_0 = 94727.8`, `M_cr = 168242.7` (warping term 2.154); `L = 3`: `M_0 = 47363.9`, `M_cr = 58750.35` (0.5386, factor
   1.2404); `L = 6`: `M_0 = 23681.9`, `M_cr = 25226.0` (0.1347). Derivation: `v = a sin kx`, `phi = b sin kx`, `k = pi/L` (fork
   supports, warping free: `v = v'' = phi = phi'' = 0` at the ends), energy `(1/2)[E I_z a^2 k^4 + (G J k^2 + E C_w k^4) b^2] - M k^2
   a b`, `det = 0` gives `M^2 = E I_z k^2 (G J + E C_w k^2)`. Fixed-fixed bound (lateral and warping both fixed, `v = v' = phi =
   phi' = 0`, mode `1 - cos(2 pi x/L)`): `M_ff = (2 pi/L) sqrt(E I_z (G J + 4 pi^2 E C_w/L^2))` = `168242.7` at `L = 3`. Rectangle of
   ADR 0508 (`b = 0.01`, `h = 0.2`): `M_cr = 4266.115`.
2. **Discretisation, Euler-Vlasov cubic Hermite model** (`arith.py`: `v` and `phi` cubic Hermite, the coupling `-(1/2) M (v' phi' -
   phi v'')`): error against the closed form `2.2e-5` (8 elements) and `1.4e-6` (16) at `L = 3`; `2.8e-5`, `1.7e-6` at `L = 1.5`;
   `1.8e-5`, `1.2e-6` at `L = 6`; `C_w = 0`: `1.0e-6`. Warping restrained at both ends (lateral pinned), 16 elements: `94746.4`
   (1.613 of the free value, below `M_ff`). The rcfd element differs by its shear-flexible lateral plane: the effective `E I_z` is
   lowered by `E I_z k^2/(kappa G A)` = `1.86e-3` at `L = 3` (`7.4e-3` at 1.5, `4.6e-4` at 6), so `M` by about half:
   **estimates `-1e-3` (L = 3), `-4e-3` (1.5), `-3e-4` (6)**. F-558's six-unknown value of the same beam was `+2.2e-3` of `M_0`
   (linear twist, stiffer), so the `C_w = 0` warping-path value is estimated `-3e-3` from it.
3. **Hand element matrices**: the two torsion matrices of D2 (closed integrals of `H_i' H_j'` and `H_i'' H_j''`); Wagner `N I_p/A`
   and `beta M` uniform: `(N I_p/A or beta M)/(30 L)` times the first matrix.
4. **Cantilever under end torque `T`, root `phi = 0`** (`arith2.log`, 16 Hermite elements, `L = 3`, the I section): warping held at
   the root, `phi(L) = (T/(G J))(L - tanh(kL)/k)`, `k = sqrt(G J/(E C_w))`, `kL = 4.2807`: error `-1.07e-6`; warping free:
   `T L/(G J)` exact to `4e-13`.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss).

| id | check | limit | estimate |
|---|---|---|---|
| B0 | default-bits record of D9, base and after | equal (bit) | exact |
| A1 | **acceptance**: I beam, `L = 3`, 16 elements, fork supports, warping free, uniform moment: `|M_FE/M_cr - 1|` | `<= 1e-2` | `1e-3` |
| A2 | the same at 8 elements | `<= 2e-2` | `1.5e-3` |
| A3 | the same at `L = 1.5` and `L = 6` (the `L` dependence of the warping term), 16 elements | `<= 1e-2` each | `4e-3`, `3e-4` |
| A4 | `-M_ref`: `|f(+M)/f(-M) - 1|` | `<= 1e-8` | `1e-12` |
| A5 | the same beam along global `y` (orientation `z`, the permuted supports of ADR 0508 amendment 1): `|M/M(axis x) - 1|` | `<= 1e-6` | `1e-12` |
| A6 | `3 M_ref`: `lambda M_ref` unchanged | `<= 1e-9` | `1e-12` |
| N1 | **negative control, Saint-Venant only**: `WarpingSection::saint_venant()` (`C_w = 0`, `beta = 0`) on the A1 beam: against the `C_w = 0` closed form `M_0` and against the F-558 six-unknown value of the same model (run in the same test) | `<= 1e-2` each | `1e-3`, `3e-3` |
| N2 | the rectangle of ADR 0508 L1 with `C_w = b^3 h^3/144` on the warping path: against the closed form (`4266.115`) and against the F-558 value | `<= 1e-2`; `<= 5e-3` | `5e-4`; `1e-3` |
| N3 | warping restrained at both ends (`restrain`): `M_free (1 + 0.2) < M_restrained < M_ff (1 + 1e-2)` | strict | `1.61 M_free` |
| N4 | the A1 value is not the F-558 value: `M_FE(warping)/M_FE(F-558) - 1 > 0.15` | strict | `+0.236` |
| E1 | element torsion block of `local_stiffness` (rows/columns `3, 6, 10, 13`) against the two hand matrices of D2, `L = 0.7`, the I section | `max|diff|/max|hand| <= 1e-12` | `1e-15` |
| E2 | the other entries of the 14 x 14 local stiffness against the 12 x 12 `beam::local_stiffness` by the index map | exact (bit) | exact |
| E3 | rigid twist `phi_0 = phi_1 = 1`, `chi = 0`: `|K14 x|` and the constant rate (`phi = x`, `chi = 1`): energy `G J L` | `<= 1e-12 max|K|`; `|E/(G J L) - 1| <= 1e-12` | `1e-15` |
| E4 | geometric torsion block, `N = -5e3`, no end forces: against `N I_p/(30 A L)` times the first matrix; the non-twist entries bit-equal to the embedded `beam::local_geometric_stiffness` | `<= 1e-12`; exact | `1e-15` |
| E5 | frame invariance: the global `element_stiffness` and `element_geometric_stiffness` (with end forces) of an element along `(1, 1, 0)/sqrt 2` (orientation rotated with it) against `Q K Q^T` of the axis-`x` element (`Q` acting on the four vectors, the warping unknowns fixed) | `<= 1e-12 max|K|` | `1e-15` |
| E6 | cantilever under end torque on the 14-unknown element (`L = 3`, 16 elements, the I section; solved densely in the test on the element's torsion rows): root warping held, against arithmetic 4; root warping free, against `T L/(G J)` | `<= 1e-3`; `<= 1e-10` | `1.1e-6`; `1e-13` |
| E7 | per-section Wagner terms: uniform `M_y = 3`, `M_z = 2` (end forces of an element in equilibrium), `beta = (0.7, -0.4)`: the twist block of `local_geometric_stiffness(beta) - local_geometric_stiffness(0)` against `(beta_y M_y - beta_z M_z)/(30 L)` times the first matrix | `<= 1e-12` relative | `1e-15` |
| E8 | `doubly_symmetric_i` gives the `BeamSection` of ADR 0508's I1 (bit-equal `A, I_y, I_z, J`), `C_w = t_f b_f^3 h_c^2/24` (bit) and `wagner = [0, 0]` | exact | exact |
| P1 | model level, per-section: a two-block beam (8 elements, blocks of 4, the A1 set-up, `beta` `(0.7, -0.4)` on block 0 and `(0, 0)` on block 1, the pre-stress pair `(0, 0)`): `K_sigma,aug(beta) - K_sigma,aug(0)` against the hand Wagner matrices of block 0's elements only | `<= 1e-8 max|hand|` | `1e-13` |
| M1 | mixed model: the A1 I beam (warping) and a disconnected rectangle beam of ADR 0508 (no warping, end moments `M_ref r` with `r = 4266.115/(30 M_ref)` so its factor is near 30), 16 elements each: the two lowest factors equal the rectangle's F-558 factor (model alone) and the I beam's warping factor (model alone) | `<= 1e-9` relative each | `1e-12` |
| R1 to R9 | each refusal of D4 gives the error with its key | exact | |

Negative controls: N1 (Saint-Venant only recovers F-558), N2 (the rectangle, `C_w` negligible), N4 (the warping term acts), A4
(sign), N3 (direction of the warping restraint), M1 (a non-warping block in the same model keeps its F-558 value).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| X1 | drop the `E C_w` term | A1, A3, N4, E1, E6 |
| X2 | `E C_w` times 2 | A1, A3, E1, E6 |
| X3 | the Hermite `H_4` sign flipped in the twist rows of the geometric terms | A1, E4, E7 |
| X4 | per-section Wagner ignored (the pre-stress pair used instead) | E7, P1 |
| X5 | the warping unknown not shared (a new equation per element end) | A1, A2 |
| X6 | the restraint ignored | N3 |
| X7 | the six-unknown torsion pair `G J/L` kept in `K_aug` (the difference not subtracted) | N1, A1 |
| X8 | the masked pre-stress not masked (the six-unknown `K_sigma` of the warping elements kept) | A1, N1 |
| X9 | RF2 removed (shared node accepted) | R2 |
| X10 | the global transform drops the warping unknowns (`T` zero there) | E5, A1 |
| X11 | Wagner `N I_p/A` torsion term kept linear (not on the Hermite rows) | E4 |

## Decision rule

**done**: B0, A1 to A6, N1 to N4, E1 to E8, P1, M1, R1 to R9 met. **partial**: everything met except one of A2, A3, N3, M1 or N2
recorded as a known miss with a diagnosed cause. **todo**: A1 or B0 missed, or any E check missed without a diagnosed set-up
defect (amendment).

## Limits (not covered)

- Buckling only: the static solve, the modes, the pre-stressed modes, the rotating part, the harmonic and transient paths and the
  nonlinear drivers do not take the warping unknown; no consistent mass of the warping unknown.
- The pre-stress is that of the six-unknown static solve; torsion in the reference load is refused (RF8); no bimoment pre-stress.
- No shear-centre offset: sections with one axis of symmetry (a channel, a monosymmetric I) need the shear centre and the
  `beta` about it; `WarpingSection::new` takes caller values in the centroidal convention of ADR 0508 and is exact only when the
  shear centre is the centroid.
- No warping continuity through a kink or a joint of differently oriented members (RF3); no warping unknown on the
  transformation path (RF4).
- Load height (a load above the shear centre) is not modelled (moments only, as ADR 0508).
- The pre-buckling-curvature factor `(1 - I_z/I_y)^(-1/2)` of the exact theory is not modelled (ADR 0508 D1); the closed form of
  the acceptance does not have it either.

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. Case-file key.** Options: (a) none (this item); (b) `[[structural.blocks]] warping = { c_w = ..., beta = [..] }` or
  `section = "i"` with the plate dimensions, on the buckling block only. Recommendation (b) as a separate item with the
  `geometric_terms` key of ADR 0508 Q2 (the schema golden changes: maintainer's approval).
- **Q2. The warping unknown on the other paths** (static, modes, pre-stressed modes) and torsion in the pre-stress. Recommendation:
  a separate item; the static solve first (it would lift RF8).
- **Q3. Monosymmetric sections** (shear-centre offset and `beta_x` about it). Recommendation: a separate item.
