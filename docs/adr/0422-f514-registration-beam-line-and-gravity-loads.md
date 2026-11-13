# ADR 0422: F-514 registration: distributed and body loads on beams (uniform line load on Beam2 elements with the consistent load of the interdependent Timoshenko shape functions, gravity on beams, section forces with the element load)

Status: **REGISTERED 2026-11-13, written before any code of F-514 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-514 was written, compiled or run: the files listed under "What was read" were read,
the arithmetic below was done by hand and evaluated with a short script of the closed forms and of a plain planar prototype of
the element of ADR 0417 with the consistent load derived here (`w49i-work/f514-scratch/estimate.py`, numpy, no rcfd code;
disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md) (F-391: six unknowns per node, `Load::NodalMoment`, the
two-node Timoshenko element Beam2 in `crates/rcfd-fem/src/beam.rs`; its limit L2 and remaining question 3 left distributed loads
and gravity on beams to a follow-up). Roadmap item F-514 (deps F-391). Size S. Number 0422 given by the lead. Branch
`f514-beam-distributed-loads`, worktree `w49i-work/f514`, scratch `w49i-work/f514-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room. The parallel item F-511 works in
`crates/rcfd-fem/src/rotating`; this item does not touch it.

## What F-514 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Uniform line load q on a cantilever: tip deflection
> q L^4/(8 E I) + q L^2/(2 kappa G A) within 1e-10 with one element (consistent load of the interdependent shape functions);
> gravity on a beam gives reactions rho A L g within 1e-12; a pinned-pinned beam under uniform load gives 5 q L^4/(384 E I) plus
> the shear term within 1e-10 at any element count.

Clauses: (a1) uniform line load on a one-element cantilever, tip deflection `q L^4 / (8 E I) + q L^2 / (2 kappa G A)` within
1e-10, the load being the consistent load of the interdependent shape functions; (a2) gravity on a beam, reactions `rho A L g`
within 1e-12; (a3) pinned-pinned beam under uniform load, midspan deflection `5 q L^4 / (384 E I)` plus the shear term within
1e-10 at any element count. The brief adds: (b1) solid-only models give the same bits (`si_scaffold_bits.rs` unedited); (b2)
every F-391 test passes unedited (a retired refusal row follows the standing decision).

Readings, registered:

- **(a1)** "a cantilever": clamped at node 0 (all six unknowns), free at node 1; one Beam2 element in a seeded orientation in
  space (the "any orientation" of ADR 0417 kept); the load a uniform force per unit length `q` given as a global vector whose
  local components are registered; the formula checked in each bending plane with its own `I` and `kappa` (x-y with `I_z`,
  `kappa_y`; x-z with `I_y`, `kappa_z`), relative to the closed form.
- **(a2)** "reactions rho A L g": the sum of the reaction forces of a beam model under `Load::Gravity` equals `-rho A L g`, `L`
  the total length of the beam elements, relative to `rho A L |g|`. Checked on three models (G1 below), one of them a solid
  block and a separate beam in one model, where the reference is `rho (V + A L) g`.
- **(a3)** "plus the shear term": `q L^2 / (8 kappa G A)` (arithmetic 3). "At any element count": the element counts
  `n = 1, 2, 3, 4, 7, 10, 20, 64` are run; the midspan is a node only for even `n`, so the acceptance's midspan value is checked
  for `n = 2, 4, 10, 20, 64`, and for every `n` the deflection and rotation at **every node** are checked against the exact
  Timoshenko solution (arithmetic 3, which at midspan is the acceptance's value). The claim behind "any element count" is that
  the nodal values are exact for every `n` (arithmetic 2), limited only by rounding; the largest `n = 64` is fixed from the
  rounding estimate.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0417 (whole, including its addendum and roadmap text).
- `crates/rcfd-fem/src/beam.rs` (whole), `loads.rs` (whole: `nodal_load_vector`, `add_force`, `add_moment`, `add_body_load`,
  `element_body_load`, `body_force_loads`), `model.rs` (`Load`, `Load::*` constructors, `add_load`, `need_set`,
  `check_elements*`, `check_supported*`), the `.loads()` users (`cyclic.rs` refuses any load, `nonlinear.rs` and `thermal.rs`
  look only for `Temperature`), `mesh.rs` (`element_set`, `element_kind`).
- Tests: `si_beam.rs` (whole: its `n2_n3_other_paths_and_invalid_data` has the row "gravity on a beam refused naming
  `beam::SOLID_ROUTINES`", which this item retires), the `0417` rows of `si_scaffold.rs` and `si_nonlinear.rs` (unchanged by
  this item: they concern the solid-element routines, which stay refused).
- **Not read:** any finite-element code, manual or documentation outside this repository. Cited from memory: P. Tong, "Exact
  solution of certain problems by finite-element method", AIAA Journal 7 (1969) 178-180 (nodal exactness when the shape
  functions solve the homogeneous equations); the textbook consistent load of a shear-flexible beam (Przemieniecki 1968). Both
  are re-derived below, and nothing is taken from them that is not derived here.

## Scaffold row retired (standing decision: ignored record citing this ADR plus a running `_after_landed` copy)

| file | test | row | why |
| --- | --- | --- | --- |
| `si_beam.rs` | `n2_n3_other_paths_and_invalid_data` | "N2 gravity on a beam": `solve_static` with `Load::gravity` refused with an `Element` error naming `beam::SOLID_ROUTINES` | gravity on a beam is now taken (D3) |

It gets `#[ignore = "record of ADR 0417 before ADR 0422 (F-514, loads on beams) retired the N2 gravity row; superseded by the
n2_n3_other_paths_and_invalid_data_after_landed copy (maintainer standing decision, 2026-11-13)"]` and nothing else; the copy
keeps every other row as it was and states the new behaviour of the gravity row (the solve succeeds and the reactions balance the
weight, check G1 model (i) below at the same limit) and adds that the centrifugal load still refuses naming
`beam::SOLID_ROUTINES` (N2 below). No other existing test is touched.

## Arithmetic done before the code (disclosed)

1. **Consistent load of a uniform line load.** In a bending plane of ADR 0417 arithmetic 2 the deflection is `d(xi) = sum_j b_j
   xi^j`, `b = C q_e`, `q_e = (d_0, L psi_0, d_1, L psi_1)`, `C = A^{-1}` (`plane_shape_coefficients`). The consistent load of a
   uniform transverse load `p` per unit length is `f_i = p int_0^L N_{d,i} dx` (the work `int p d dx` of the interpolated
   deflection; the section-rotation functions get nothing, since there is no distributed moment). With `A^{-1}` written out (the
   interdependent functions, `Phi = 12 E I / (kappa G A L^2)`):
   `N_{d,1} = (1 - 3 xi^2 + 2 xi^3 + Phi (1 - xi)) / (1 + Phi)`,
   `N_{d,2} / L = (xi - 2 xi^2 + xi^3 + (Phi / 2)(xi - xi^2)) / (1 + Phi)`,
   `N_{d,3} = (3 xi^2 - 2 xi^3 + Phi xi) / (1 + Phi)`,
   `N_{d,4} / L = (-xi^2 + xi^3 + (Phi / 2)(xi^2 - xi)) / (1 + Phi)`.
   Integrals over `xi in [0, 1]`: `(1 - 1 + 1/2 + Phi / 2) / (1 + Phi) = 1/2`; `(1/2 - 2/3 + 1/4 + Phi / 12) / (1 + Phi) = 1/12`;
   `1/2`; `-1/12`. So `f = p L (1/2, L/12, 1/2, -L/12)` in `(d_0, psi_0, d_1, psi_1)`, **independent of `Phi`**: the same as the
   Euler-Bernoulli (Hermite) load. Script: the 4-point Gauss integral of `plane_shape_coefficients(phi)` against this closed form
   for `phi = 0, 0.1, 1, 10`: at most `2.1e-16` relative. Axial: linear functions, `f = p_x L (1/2, 1/2)`. Torsion: nothing.
   **In global axes** (the two planes of ADR 0417 arithmetic 3: x-y with `psi = theta_z`, x-z with `psi = -theta_y`): for a
   uniform global `q`, node 0 gets the force `q L / 2` and the moment `(L^2 / 12) e_1 x q`, node 1 the force `q L / 2` and the
   moment `-(L^2 / 12) e_1 x q`. Check: `e_1 x (q_y e_2) = q_y e_3` gives `+q_y L^2 / 12` on `theta_z` at node 0 (x-y plane,
   `psi_0` row); `e_1 x (q_z e_3) = -q_z e_2` gives `-q_z L^2 / 12` on `theta_y` at node 0 (x-z plane, `psi = -theta_y`); the
   axial part has no moment (`e_1 x e_1 = 0`). The vector needs only the axis and the length, not the orientation vector `v`.
2. **Nodal exactness for any load and any element count (Tong's argument, derived).** Energy on an element
   `a_e(u, w) = int (E I psi_u' psi_w' + k_s gamma_u gamma_w) dx`, `gamma = d' - psi`. Integration by parts gives
   `a_e(u, w) = [E I psi_w' psi_u + k_s gamma_w d_u]_0^L - int psi_u (E I psi_w'' + k_s gamma_w) dx - int d_u (k_s gamma_w)' dx`.
   For `w` in the element space the shape functions solve the homogeneous equations (`(k_s gamma)' = 0`, `E I psi'' + k_s gamma
   = 0`, ADR 0417 arithmetic 2), so both integrals vanish and `a(u, w_h)` depends only on the **nodal values** of `u`:
   `a(u, w_h) = a(I u, w_h)` with `I u` the element-space function with the nodal values of `u`. With `u` the exact solution of
   the loaded beam and `u_h` the finite-element solution with the consistent load, Galerkin orthogonality `a(u - u_h, w_h) = 0`
   for every admissible `w_h` gives `a(I u - u_h, w_h) = 0`; `w_h = I u - u_h` (admissible: both satisfy the nodal constraints)
   and positive definiteness of the constrained energy give `I u = u_h`. **The nodal displacements and rotations are exact for any
   distributed load whose consistent load is integrated exactly, at any element count**; the same holds for the axial (linear
   functions solve `E A u'' = 0`) and torsional parts. The element end forces `K_e u_e - f_e` are then the exact end forces (the
   element's exact solution with the given end values and load is unique, and its end forces satisfy the same identity), so the
   section forces with the load subtracted are exact as well (D4). This is what "at any element count" relies on.
3. **Closed forms.** Cantilever, clamped at `x = 0`, uniform `p`: `E I psi' = M(x) = p (L - x)^2 / 2` (sign as in the plane),
   `psi(L) = p L^3 / (6 E I)`, `gamma = V / k_s = p (L - x) / k_s`, `d(L) = p L^4 / (8 E I) + p L^2 / (2 k_s)`, axial
   `u(L) = p_x L^2 / (2 E A)`. Pinned-pinned, `d(0) = d(L) = 0`, `M(0) = M(L) = 0`: `d(x) = p x (L^3 - 2 L x^2 + x^3) / (24 E I)
   + p x (L - x) / (2 k_s)`, `psi(x) = p (L^3 - 6 L x^2 + 4 x^3) / (24 E I)` (the derivative of the bending part; the shear part
   is `gamma`), midspan `5 p L^4 / (384 E I) + p L^2 / (8 k_s)`. Script: `d(L/2)` equals the midspan formula to 0 in double.
   **Section forces by statics** (ADR 0417 reading (a4): the force and moment the part at larger local `x` exerts on the other):
   at the section `x` they are the resultant of the external loads on `[x, L]` and their moment about the section point. For the
   cantilever `S = q (L - x)`, `M_S = ((L - x)^2 / 2) e_1 x q`: at the root `(N, V_y, V_z) = L q_l`, `(T, M_y, M_z) = (0, -q_z L^2
   / 2, q_y L^2 / 2)`, at the tip zero. For the pinned-pinned beam with `q` normal to the axis (reactions `-q L / 2` at each end)
   `S = q (L / 2 - x)`, `M_S = -(x (L - x) / 2) e_1 x q`.
4. **Rounding (the prototype).** One-element cantilever with the data of ADR 0417 arithmetic 6 and `q_l = (300, -800, 1100)
   N/m`: tip deflection `1.6e-15` (`v`), `7.5e-16` (`w`), tip rotations `2.1e-15`, `7.3e-16`. Shear share of the tip deflection
   `7.9e-3` (`v`), `5.4e-3` (`w`). A lumped load (forces `q L / 2` only) misses the tip deflection by `0.331` (N1). Pinned-pinned,
   data of ADR 0417 arithmetic 5 (`L = 1`, square of side `0.02 sqrt 12`, `kappa = 5/6`, `E = 2e11`, `nu = 0.3`), `p = -1000 N/m`:
   shear share of the midspan value `1.18e-2`; nodal deflection error relative to the midspan value and rotation error relative to
   `psi(0)`, by `n`: `1`: `0, 3.7e-16`; `2`: `2.0e-16`; `3`: `1.9e-15`; `4`: `2.0e-15`; `7`: `2.3e-14, 5.1e-14`; `10`: `1.6e-14,
   2.3e-14`; `20`: `4.3e-14, 7.7e-14`; `64`: `3.3e-12, 5.5e-12`; `200`: `1.2e-11, 1.7e-11` (condition of the reduced stiffness
   `1.5e6` at 64, `1.4e7` at 200). Section forces at the element ends against statics, shear relative to `|p| L / 2`, moment to
   `|p| L^2 / 8`: at most `1.9e-12`, `5.7e-12` at `n = 64`. With the registered rectangle of the checks (`b = 0.02 sqrt 12` along
   y, `h = 0.04` along z; `estimate2.py`): shear share of the midspan value `1.18e-2` (x-y, `I_z`) and `3.98e-3` (x-z, `I_y`);
   worst nodal errors at `n = 64`: x-y `4.2e-13` (deflection), `4.7e-13` (rotation); x-z `8.1e-12`, `1.34e-11`; at `n <= 20`
   below `6e-13`. The largest registered `n` is 64 (margin about 7 to 1e-10 on the worst quantity of the prototype, the x-z
   rotation; the 3-D model adds the transformation and a sparse direct solve, of the same order).
5. **Gravity.** For a Beam2 element `b = rho g` over the section gives the line load `q = rho A g` (the centroidal axis; no
   moment from gravity on a centroidal section). Its consistent load is arithmetic 1 with that `q`; its sum over the nodes is
   `rho A L g` exactly in exact arithmetic (the moments are no forces), so the reaction sum is rounding-limited. A solid element
   keeps its existing consistent body load (ADR 0276), whose force sum is `rho V g` (partition of unity).

## Design

### D1. `Load::LineLoad`

New variant `Load::LineLoad { set, force }` (`Load::line_load(set, force)`): a uniform force per unit length, N/m, a global
vector, on every element of an **element set**; every element of the set must be a Beam2 element. `add_load` refuses an unknown
element set (`UnknownSet`, as the other set loads), a non-finite vector (`InvalidValue { key: "force per length" }`) and a set
holding a non-Beam2 element (`InvalidValue { key: "line load" }`, naming the element and its kind). `Load` is
`#[non_exhaustive]`; no other crate matches on it.

### D2. `beam::line_load_vector`

`pub fn line_load_vector(coords: &[Vec3], q: Vec3) -> Result<[f64; 12]>`: the global consistent load of arithmetic 1 in the
element's unknown order (node 0: three forces, three moments; node 1 likewise). Refuses other than two coordinates and a zero or
non-finite length, as `frame`. Independent of the section and of the orientation vector.

### D3. Loads (`loads.rs`)

- `Load::LineLoad`: for each element of the set, in set order, the twelve values of D2 added to its nodes (forces with
  `add_force`, moments with `add_moment`).
- `Load::Gravity`: the element loop of `add_body_load` takes an optional beam rule: for a Beam2 element, `q = rho_e A g` (the
  element's density, its block's section) and D2; every other element exactly as before (`element_body_load`, same order). A
  model without Beam2 elements runs the former code path with the same operations in the same order: the same bits.
- `Load::Centrifugal`, `Load::BodyForceNodal`, `Load::Temperature` on a beam keep refusing through the solid-element routines,
  naming `beam::SOLID_ROUTINES` (limit L1 below); `body_force_loads` is unchanged.

### D4. Section forces with the element load

`beam::section_forces` becomes `f = K_l T u_e - T f_e`, `f_e` the element's consistent load of the model's line loads (every
`LineLoad` whose set holds the element) and gravity (`rho A g`), summed as one `q` per element; `start = -f_0`, `end = f_1` as
before. An element without such a load takes the former expression (no subtraction): the F-391 results are unchanged. By
arithmetic 2 the end forces are then exact.

### D5. Paths

Line loads and gravity on beams are taken where beams are taken (ADR 0417 D4: static and modal elimination paths; modal ignores
loads). Every other path refuses a beam model as before (`beam::FOLLOW_UP`); the cyclic path refuses any load as before.

## Checks (limits fixed now)

Common data: one-element case of ADR 0417 arithmetic 6 (`L = 0.8 m`, `A = 0.012 m^2`, `I_y = 3e-6`, `I_z = 5e-6`, `J = 4e-6 m^4`,
`kappa_y = 0.85`, `kappa_z = 0.75`, `E = 2.1e11 Pa`, `nu = 0.3`, `rho = 7850`), geometry from the seed `0x0422_0514` of the
xorshift64* generator of `si_beam.rs` drawn as there; line load in local axes `q_l = (300, -800, 1100) N/m` applied as a global
vector. Pinned-pinned case: `L = 1 m` along `e_x`, orientation `(0, 0, 1)`, `rectangle_section(b = 0.02 sqrt 12, h = 0.04,
torsion = 1e-6 m^4, shear = 5/6)` (so `I_z = h b^3 / 12` and `I_y = b h^3 / 12` differ and a plane
swap is seen), `E = 2e11`, `nu = 0.3`, `rho = 7850`, `q = (0, -1000, 600) N/m`; constraints: `u_x, u_y, u_z` at both end nodes,
`theta_x` at node 0.

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| L1 | a1 | one element, seeded: tip `v` (local) against `q_y (L^4 / (8 E I_z) + L^2 / (2 kappa_y G A))`, relative | `1e-10` | arithmetic 4: `1.6e-15` |
| L2 | a1 | tip `w` against `q_z (L^4 / (8 E I_y) + L^2 / (2 kappa_z G A))`, relative | `1e-10` | `7.5e-16` |
| L3 | (stronger, a1) | tip `theta_z` against `q_y L^3 / (6 E I_z)`, `theta_y` against `-q_z L^3 / (6 E I_y)`, `u` against `q_x L^2 / (2 E A)` (each relative), `|theta_x|` relative to the largest tip rotation | `1e-10` each | arithmetic 3, 4 |
| L4 | (a1, D4) | root section forces against `(L q_l, 0, -q_z L^2 / 2, q_y L^2 / 2)`, tip section forces against 0: forces relative to `L max |q_l|`, moments to `L^2 max |q_l|` | `1e-10` | arithmetic 2, 3 |
| L5 | (a1) | reactions: `|R_f + L q|` relative to `L |q|`, `|R_m + (L^2 / 2) e_1 x q|` relative to `L^2 |q|` | `1e-10` | rounding |
| L6 | (a1, the derivation) | `beam::line_load_vector` of the seeded element, rotated to local axes, against the 4-point Gauss integral of `plane_shape_coefficients(Phi)` in each plane with the element's `Phi_y`, `Phi_z` (and the axial `1/2, 1/2`), relative to its largest entry; the same for a stubby element `L = 0.05` (`Phi` about 25 and 35) | `1e-13` | arithmetic 1: `2.1e-16` |
| G1 | a2 | gravity `g = (1.3, -2.1, -9.81) m/s^2`, `|sum R_f + W|` relative to `|W|`, `W` the weight: (i) the space frame of ADR 0417 arithmetic 8 (three members, lengths 0.8, 0.6, 0.5, section of the one-element case, orientation `(1, 2, 3)`) clamped at node 0, `W = rho A L g`, `L = 1.9`; (ii) the pinned-pinned beam (data above) with 20 elements, `W = rho A L g`; (iii) a Hex8 box `[2, 1, 1]` (div `[2, 1, 1]`) clamped at `x = 0` and a separate one-element beam from `(3, 0, 0)` to `(4, 0, 0)` clamped at `(3, 0, 0)` in one model, `W = rho (V + A L) g`, `V = 2` | `1e-12` | arithmetic 5: rounding |
| G2 | (stronger, a2) | frame (i): reaction moment at node 0 against `-sum_e (c_e - x_0) x (rho A L_e g)`, `c_e` the element midpoints, relative to `|W| max_e |c_e - x_0|`; pinned-pinned (ii) each end's transverse reaction `-W / 2` relative to `|W|` | `1e-12` | rounding; symmetry |
| G3 | (a2, D3) | the load vector of gravity on the frame (i) against that of `Load::line_load("all", rho A g)` (same expression), relative to its largest entry | `1e-15` | the same arithmetic |
| P1 | a3 | pinned-pinned, `n = 2, 4, 10, 20, 64`: midspan `v` against `5 q_y L^4 / (384 E I_z) + q_y L^2 / (8 kappa G A)` and `w` against the same with `q_z`, `I_y`, relative | `1e-10` | arithmetic 4: `1e-16` to `8.1e-12` |
| P2 | a3 (any element count) | `n = 1, 2, 3, 4, 7, 10, 20, 64`: at every node `v`, `w` against `d(x)` of arithmetic 3 in each plane relative to the plane's midspan value, `theta_z` against `psi(x)` (x-y) and `theta_y` against `-psi(x)` (x-z) relative to the plane's `|psi(0)|` | `1e-10` each | arithmetic 4: worst `1.34e-11` at 64 |
| P3 | (a3, D4) | the same counts: section forces at both ends of every element against statics (arithmetic 3): `V_y, V_z` relative to `|q| L / 2`, `M_y, M_z` relative to `|q| L^2 / 8`; `N`, `T` relative to `|q| L / 2` (zero) | `1e-10` | arithmetic 4: `5.7e-12` at 64 |
| P4 | (a3) | the same counts: the reaction force at each end against `-q L / 2`, relative to `|q| L / 2` | `1e-10` | rounding |
| B1 | b1 | `si_scaffold_bits.rs` passes unedited | pass | D3 |
| B2 | b1, b2 | every existing `rcfd-fem` test passes, unedited except the one retired row (ignored) whose copy passes; in particular every F-391 test of `si_beam.rs` and the gravity tests of `sb_loads.rs` | pass | D3, D4 |
| B3 | b1 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |
| B4 | b1 | in model G1 (iii) the load entries of the box nodes equal, bit for bit, those of the box alone under the same gravity | exact | D3: the solid path unchanged |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | the one-element cantilever loaded by the lumped load (nodal forces `q L / 2` at both nodes, no moments) | tip `v` off by about `0.33` (arithmetic 4): the moments of the consistent load matter | `> 0.1` |
| N2 | the clamped one-element model with `Load::centrifugal`, with `Load::body_force_nodal`, with `Load::temperature` | each refused with an `Element` error whose source names `beam::SOLID_ROUTINES` | exact |
| N3 | `Load::line_load` on an unknown element set; with a non-finite vector; on the set of the G1 (iii) model holding the Hex8 elements | `UnknownSet`; `InvalidValue { key: "force per length" }`; `InvalidValue { key: "line load" }` naming element 0 | exact |
| N4 | L1, L2 and P1 against the closed forms without the shear term | `7.9e-3`, `5.4e-3`; P1 `1.18e-2` (`v`), `3.98e-3` (`w`): the checks see the shear | `> 1e-3` |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the moments of the consistent load dropped (lumped load) | L1, L2, L6, P2 |
| MU2 | the sign of the moment at node 1 flipped (`+ L^2 / 12`) | L1, L6, P2 |
| MU3 | `L^2 / 12` replaced by `L^2 / 8` | L1, L6, P2 |
| MU4 | `q x e_1` instead of `e_1 x q` | L1, L2, L6 |
| MU5 | gravity on a beam without the area (`q = rho g`) | G1, G3 |
| MU6 | section forces without the element load (D4 removed) | L4, P3 |
| MU7 | `add_load` accepts a set with a solid element | N3 |

## Decision rule

- **done:** every check of the acceptance (L1, L2, G1, P1, P2) and the brief (B1 to B4) met, the stronger checks L3 to L6, G2,
  G3, P3, P4 and N1 to N4 as stated, MU1 to MU7 each caught by a registered check.
- **partial:** a registered check missed (recorded as a `*_known_miss`) or not run, one of MU1 to MU7 not caught, or a check met
  only after an amendment written after its run.
- **todo:** the line load does not build.

## Limits (not covered)

- L1. Centrifugal load, nodal body force field and thermal load on beams stay refused (`beam::SOLID_ROUTINES`): the centrifugal
  load on a beam is a linearly varying line load plus distributed moments of the section's rotary inertia, a follow-up.
- L2. Only uniform line loads in global axes on whole elements: no linearly varying or partial loads, no loads in local axes
  (follower), no distributed moments, no point loads inside an element.
- L3. The paths that refuse beams (ADR 0417 D4) refuse them with line loads too; output of beams (`rcfd-post`) is the F-391
  follow-up.
- L4. No change to `rcfd-api` case files or schema: `Load::LineLoad` is a library variant (a case-file key would touch the
  schema golden).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The line load names an **element set** (not a node set or a surface): the elements are the carrier of a line load, and a set
   of beam elements is how a beam is named. Taken.
2. `beam::section_forces` subtracts the element load (D4) rather than a new function: without it the section forces of a loaded
   beam are wrong, and no existing caller has a loaded beam (gravity on beams was refused). Taken.
3. Gravity on a centroidal section gives no distributed moment (arithmetic 5); offsets are outside ADR 0417 L3. Taken.
