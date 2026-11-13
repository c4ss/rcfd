# ADR 0395: F-489 registration: cyclic symmetry, harmonic loads and rigid sets at a non-zero harmonic index

Status: **REGISTERED 2026-11-13, written before any code of F-489 and before any run of its checks**. Not yet accepted by
the maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0386](0386-f394-registration-cyclic-symmetry-sector-statics-and-harmonic-modes.md) (F-394: the sector, the
real cos/sin pair of rows, `mpc::harmonic_transformation`, `cyclic::modal_analysis_harmonic`, `full_rotor`; its remaining
question 2 proposed this item). Builds on [ADR 0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md)
(the transformation `x = T q + g`, rigid sets with six reference unknowns) and [ADR
0384](0384-f486-registration-modal-analysis-on-the-transformation.md) (modal analysis on the transformation). Number 0395
given by the lead. Branch `f489-cyclic-harmonic-loads`, worktree `w49i-work/f489`, base a12b7e1c.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model and call that does not ask for a harmonic load or a rigid set at `k > 0` runs exactly the code it ran before); no edit
of `baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; no existing test edited except as
the maintainer's decision of 2026-11-13 permits (D5); clean-room. A parallel package (F-490, transient on the transformation
path) may touch `mpc.rs`: the edits there are additive and small (D2), their line ranges are reported in the addendum.

## What F-489 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A static load given with a harmonic index k (cosine
> and sine parts per sector) is solved on the sector by the real pair of rows of ADR 0386 and equals the full 360-degree
> model's solution under the expanded load within 1e-8 relative for N = 12 and k = 0 to 6; rigid sets are accepted at k > 0
> (their reference unknowns doubled) and the frequencies of a sector with a rigid set equal the full model's within 1e-8.

Clauses: (c1) harmonic static load, cosine and sine parts per sector, solved on the sector by the rows of ADR 0386, equal to
the full model under the expanded load within 1e-8 relative, N = 12, every k = 0 .. 6; (c2) rigid sets accepted at `k > 0`
with their reference unknowns doubled; (c3) the frequencies of a sector with a rigid set equal the full model's within 1e-8.

## What was read (base a12b7e1c, read only)

1. `AGENT_RULES.md`; ADR 0386 (all, with its addendum and amendment); ADR 0296 (rigid-set rows, the reference unknowns, the
   elimination); ADR 0384 (modal analysis on the transformation).
2. Code: `crates/rcfd-fem/src/cyclic.rs` (all), `crates/rcfd-fem/src/mpc.rs` (all), `loads.rs` (`nodal_load_vector`,
   `NodalForceAt`), `model.rs` (`Load`, `Constraint::rigid_set`), `solve.rs` (`StaticOptions`, `StaticSolver`,
   `PIVOT_TOLERANCE`). Tests: `si_cyclic.rs` (all; its `r6_rigid_set_and_non_zero_value_refused_at_k_2` asserts the refusal
   this package retires), `si_scaffold.rs` (the `_after_landed` pattern).
3. **Not read:** any finite-element code or manual outside this repository. No literature beyond what ADR 0386 cites; every
   formula below is derived here.

Arithmetic done beforehand: none beyond the derivation below (no script, nothing compiled or run).

## Method and derivation

Notation of ADR 0386: `N` sectors, `alpha = 2 pi / N`, `theta = k alpha`, `R = R(alpha)`; the displacement of sector `j` in
sector-local components `v_j = R^(-j) u_j`; `K`, `M` the sector stiffness and mass (every element of the rotor in exactly one
sector); for a harmonic index `k` the sector amplitude `w = a + i b`, `v_j = Re(w e^(i j theta)) = c_j a - s_j b` with `c_j =
cos(j theta)`, `s_j = sin(j theta)`; the rows `a_H = R(cos theta a_L - sin theta b_L)`, `b_H = R(sin theta a_L + cos theta
b_L)` (single, `a_H = +/- R a_L`, for `k = 0` and `2k = N`).

### D0. Harmonic static load (clause c1)

**The load.** A harmonic load is given by two sector load vectors over the sector's `DofMap`, the cosine part `f_a` and the
sine part `f_b`; the load of sector `j` in its local components is `f_j = c_j f_a - s_j f_b` (the real part of `(f_a + i
f_b) e^(i j theta)`), i.e. in global components `R^j f_j` at the nodes of sector `j`. The **expanded load** on the full rotor
is the assembly of these per-sector vectors: at a node shared by sectors `j` (its high node) and `j + 1` (its low node) both
contributions are added. This is the assembly the full model itself makes of an element-wise load (a pressure or traction on
faces, a body force over elements): each element's contribution lies in exactly one sector. For a nodal force the user
decides on which sector node (low or high) it sits; either way the expansion is the sum. For `k = 0` and `2k = N`, `s_j = 0`
for every `j`, so a sine part has no expanded load: it is **refused** when non-zero (D1), not silently ignored.

**The sector problem.** Full potential energy for a full displacement of harmonic form, `Pi = sum_j [ (1/2) v_j^T K v_j - f_j^T
v_j ]` (the expanded load's work equals the sum of the per-sector works because the shared node has one displacement and the
loads add). With `sum_j c_j^2 = sum_j s_j^2 = N/2`, `sum_j c_j s_j = 0` for `0 < k < N/2`:

```
Pi = (N/2) [ (1/2)(a^T K a + b^T K b) - f_a^T a - f_b^T b ]      (0 < 2k < N)
Pi =  N    [ (1/2) a^T K a - f_a^T a ]                             (k = 0, 2k = N)
```

The full stiffness is block-circulant (ADR 0386), its discrete Fourier transform block-diagonal; the expanded load has only
the harmonics `+/- k`, so the full solution (the full model being non-singular) lies in the harmonic-`k` subspace, which is
exactly the image of the admissible `(a, b)` under `v_j = c_j a - s_j b` (an injective map: the factor `N/2` or `N` is
non-zero). Minimising `Pi` over that subspace is therefore the full problem, and the constant factor drops out: **the sector
problem is `blockdiag(K, K)` over `(a, b)` with the load `(f_a, f_b)`, on the transformation of the rows of ADR 0386, and the
full solution is `u_j = R^j (c_j a - s_j b)` with scale 1** (unlike the modes of ADR 0386, whose scale `sqrt(2/N)` is a
normalisation). For `k = 0` this is exactly the `k = 0` static solve of ADR 0386 (`mpc::transformation`) with the load
`f_a`.

**Prescribed values and the model's own loads.** At `k = 0` the prescribed values may be non-zero (the transformation of ADR
0296 carries `g`); at `k > 0` the rows of `harmonic_transformation` take zero values only (as in ADR 0386; a harmonic
prescribed displacement is a limit, below). The model's own load list (`SolidModel::loads`) has no harmonic index: a model
with loads is **refused** by the harmonic static solve, and the parts are given as vectors (`loads::nodal_load_vector` of a
model that carries the loads builds them).

### D0b. Rigid sets at a harmonic index (clauses c2, c3)

A rigid set of the sector (node set `S`, reference point `x_R`) means one rigid set in every sector of the full rotor: the
copy in sector `j` has the nodes of `S` of that sector and the reference point `o + R^j (x_R - o)`. Its rows in sector `j`, in
global components, `u_p = t^(j) + w^(j) x (x_p^(j) - x_R^(j))`; with `u_p = R^j v_p`, `x_p^(j) - x_R^(j) = R^j d_p` (`d_p = x_p
- x_R` in sector 0) and, for a proper rotation, `R^j w x R^j d = R^j (w x d)`, they read in local components `v_p = tau_j +
omega_j x d_p` with `tau_j = R^(-j) t^(j)`, `omega_j = R^(-j) w^(j)` (the small rotation is an axial vector; a proper rotation
maps it as a vector). The rows are linear and the same in every sector, so the harmonic substitution `tau_j = c_j tau_a - s_j
tau_b`, `omega_j = c_j omega_a - s_j omega_b` turns the `N` sets of rows into **the rows of ADR 0296 applied to the cosine
part with reference unknowns `(tau_a, omega_a)` and to the sine part with `(tau_b, omega_b)`** (the real and imaginary parts of
the complex row): six reference unknowns per part, twelve per rigid set for `0 < 2k < N`, six for `k = 0` and `2k = N`. The
energy argument of D0 applies unchanged (the reference unknowns carry no stiffness or mass of their own; ADR 0384), and the
map `(a, b) -> (-b, a)` (with the references) still maps the admissible set into itself, so each eigenvalue of the doubled
real problem is double as in ADR 0386.

A rigid set whose nodes include low or high nodes is allowed: in the full model the copies in sectors `j` and `j + 1` then
share nodes, and the sector rows of the set and the cyclic rows describe that sharing exactly (the elimination of ADR 0296
takes the cyclic row on an already dependent high unknown with another pivot). This case is checked (H3, H6).

A reference load on a rigid set has a cosine part `(F_a, M_a)` and a sine part `(F_b, M_b)`; the copy in sector `j` carries
`R^j (c_j F_a - s_j F_b)` and `R^j (c_j M_a - s_j M_b)` at its reference point (the same argument: force and moment rotate with
the sector). The reference motion of copy `j` is `R^j (c_j tau_a - s_j tau_b)` and `R^j (c_j omega_a - s_j omega_b)`.

## Design

### D1. `cyclic.rs` (this package's file)

- `HarmonicLoad { harmonic, cos: Vec<f64>, sin: Vec<f64>, reference_cos: Vec<ReferenceLoad>, reference_sin:
  Vec<ReferenceLoad> }` (`#[non_exhaustive]`, `HarmonicLoad::new(harmonic, cos, sin)`, `.references(cos, sin)`).
- `solve_static_harmonic(model, extra, load, options) -> Result<HarmonicStatic>`: `validate`, `check_supported`, exactly one
  Cyclic constraint; refuses a harmonic above `N/2` (key `harmonic`), a model with its own loads (key `loads`), a part of
  the wrong length (`FieldLength`), a non-zero sine part or sine reference load at `k = 0` or `2k = N` (key `harmonic load`),
  a non-finite value (key `harmonic load`), a reference load on a constraint that is not a rigid set (key `reference load`),
  a solver other than `Direct` (key `solver`). `k = 0`: `mpc::transformation(model, extra)`; `k > 0`:
  `mpc::harmonic_transformation(model, extra, k)`; `K` with the supports (`mpc::add_supports`), doubled block-diagonally for
  `0 < 2k < N`; the load over all unknowns `(f_a, f_b, references)`; `Transformation::reduce`; the skyline `L D L^T` with the
  pivot test of the direct static solve (`d <= PIVOT_TOLERANCE a_ii` refused, naming the unknown: a mesh unknown with "(sine
  part)" where it applies, or "the reference translation x of constraint 3 (RigidSet, sine part)"). Result `HarmonicStatic {
  harmonic, sectors, cos, sin, reference_cos: Vec<ReferenceMotion>, reference_sin, relative_residual, unknowns,
  transformation, reduced_matrix }` (`sin` and `reference_sin` zero for single harmonics).
- `FullRotor::expand_field(sector_dofs, full_dofs, harmonic, cos, sin) -> Vec<f64>`: `u_j(p) = R^j (c_j a_p - s_j b_p)`,
  scale 1, each full node written once from the sector in which it is not a high node (as `expand`). `FullRotor::expand_load(
  sector_dofs, full_dofs, harmonic, cos, sin) -> Vec<f64>`: the sum over every sector node (high nodes included) of `R^j (c_j
  f_a - s_j f_b)` at `node(j, p)`. `FullRotor::expand` (the modes) is not changed.
- `modal_analysis_harmonic`: no change of the code path except the name of an unknown in its "no mass" refusal (a reference
  unknown is named as such instead of by `i % n`); it accepts rigid sets because `harmonic_transformation` does (D2).

### D2. `mpc.rs` (shared file, additions only)

- `harmonic_transformation`: the refusal of a `RigidSet` is replaced by its rows on each part: reference unknowns after the
  (doubled) mesh unknowns, per rigid set in constraint order six for the cosine part, then (doubled harmonic) six for the sine
  part; `Transformation::references()` lists the rigid set once per part, cosine first (so `reference_offset(c)` is the cosine
  part, and the sine part is at `+ 6`). The rows are those of `transformation` (same terms, same order, `RowOrigin::RigidSet`),
  built by a new private helper `rigid_set_rows(model, constraint, set, reference, mesh_offset, reference_offset)`;
  `transformation` itself is not touched.
- `Elimination::name`: in the sine case a reference unknown is counted from `2 n` (a message only; the non-sine branch is
  unchanged).
- Nothing else; a model without a rigid set builds exactly the same rows in the same order in both functions.

### D3. Files

`crates/rcfd-fem/src/cyclic.rs` (D1), `crates/rcfd-fem/src/mpc.rs` (D2), a new test file
`crates/rcfd-fem/tests/si_cyclic_harmonic.rs`, the permitted edit of `si_cyclic.rs` (D5), and this ADR. `rcfd-post` is checked
with `cargo check -p rcfd-post --tests`. `rcfd-api` does not depend on `rcfd-fem` (ADR 0386 D3), so `baseline_hashes` cannot
change and is not run.

### D4. Models of the checks

Material `E = 2e11` Pa, `rho = 7850` kg/m^3, `nu = 0.3`; `N = 12`, axis `z` through the origin; the Hex20 polar sector of ADR
0386 (`hex_box` in `(r, phi, z)` upgraded by `upgrade_to_hex20_mapped` with the polar map; `ymin` low, `ymax` high).

- **D (clamped disc)**: `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [3, 2, 1])`, inner face `xmin` clamped (three
  `Displacement`).
- **DR (disc with a rim rigid set)**: D plus `Constraint::rigid_set("rim", x_R)`, `rim` the nodes of the outer face (`r = 1`)
  with `0 < phi < alpha` strictly (7 nodes: `phi = alpha/4, 3 alpha/4` at `z = +/-0.1`, `phi = alpha/2` at `z = -0.1, 0, 0.1`;
  not collinear), `x_R = (cos(alpha/2), sin(alpha/2), 0)`.
- **DSH (disc with a shroud rigid set)**: D plus `Constraint::rigid_set("shroud", x_R)`, `shroud` every node of the outer face
  (low and high nodes included, shared with the neighbours' sets in the full model).
- The full models: `full_rotor` of the sector mesh, material on `all`, `xmin` clamped (invariant under the rotation); for DR
  and DSH one rigid set per sector, node set `rim_j` (or `shroud_j`) `= { node(j, p) : p in rim }`, reference `R^j x_R`,
  added in the order `j = 0 .. N-1` after the clamp.
- **Loads**: cosine part `f_a` = `nodal_load_vector` of the sector model with `Load::pressure("xmax", 1e6)` plus nodal forces
  `(1e4, -2e4, 3e4)` N at the first node of `ymin` (ascending) and `(-5e3, 1e4, 2e4)` N at the first node of `ymax`; sine part
  `f_b` = `nodal_load_vector` of the sector model with `Load::traction("zmax", [2e5, -1e5, 5e5])` plus `(3e4, 1e4, -1e4)` N at
  the node nearest `(0.6 cos(alpha/2), 0.6 sin(alpha/2), 0.1)` (an interior node). Zero sine part at `k = 0` and `6`. For DR
  the reference loads: cosine `F = (1e4, 2e4, -3e4)` N, `M = (1e3, -2e3, 5e2)` N m; sine `F = (-2e4, 1e4, 1e4)` N, `M =
  (-5e2, 1e3, 2e3)` N m (sine parts only for `k = 1 .. 5`).
- The full load: `FullRotor::expand_load` of `(f_a, f_b)` as nodal forces of the full model (`NodalForceAt` per node) and the
  reference loads of copy `j` as above; solved by `mpc::solve_static_mpc` (Direct).

Static comparison metric: `e_u = max |expand_field(a, b) - u_full|_inf / max |u_full|_inf` over every full node; for DR and
DSH also `e_t = max_j |R^j (c_j tau_a - s_j tau_b) - t^(j)|_inf / max |u_full|_inf` and `e_w = max_j |R^j (c_j omega_a - s_j
omega_b) - w^(j)|_inf / max_j |w^(j)|_inf`.

Modal: the procedure of ADR 0386 E1/E2 (sector `ModalOptions::new(4)`, tolerance `1e-9`, cap 400, every `k = 0 .. 6`; cut-off
`lambda_c = 0.999 min_k lambda_(4),k`; full model `n_c + 8` modes, same tolerance and cap; the set-up guards: the full model's
largest computed eigenvalue above `lambda_c` and no eigenvalue within `1e-7` of `lambda_c`, else a defective set-up, an
amendment).

## Registered checks

All in `crates/rcfd-fem/tests/si_cyclic_harmonic.rs`, release, run once (`cargo test -p rcfd-fem --release --test
si_cyclic_harmonic -- --test-threads=1 --nocapture`).

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| H1 | c1 | D, every `k = 0 .. 6`: `e_u` | `<= 1e-8` | the same discrete problem (D0); rounding of the element matrices in rotated coordinates and of the two factorisations `~ eps kappa`, `kappa ~ 2e5` (ADR 0386), below 1e-10 |
| H2 | c1, c2 | DR, every `k = 0 .. 6`, with the reference loads: `e_u`, `e_t`, `e_w` | each `<= 1e-8` | as H1; the rigid-set rows have coefficients of order 1 |
| H3 | c1, c2 | DSH, every `k = 0 .. 6`: `e_u`, `e_t`, `e_w` (no reference loads) | each `<= 1e-8` | as H1 |
| H4 | (expanded load) | `k = 0`: `expand_load` of the sector pressure vector of `xmax` against `nodal_load_vector` of the full model with `Load::pressure("xmax", 1e6)`: max abs difference / max abs entry | `<= 1e-12` | the same face integrals in rotated coordinates, rounding ~1e-15 |
| H5 | c2, c3 | DR modal: count of the full eigenvalues below `lambda_c` equals the sector count `n_c`; `max |f_s/f_f - 1|` over the sorted lists; pair splits of `k = 1 .. 5` | equal; `<= 1e-8`; `<= 1e-8` | as ADR 0386 E2 (measured there 1.8e-13) |
| H6 | c2, c3 | DSH modal: as H5 | as H5 | as H5 |
| H7 | c2 | DR at `k = 2`: the transformation has `2 n + 12` unknowns, `references()` lists the rim twice with offsets `2 n` and `2 n + 6`; at `k = 6`: `n + 6` unknowns, once; `k = 0`: `n + 6`, once | as stated | D2 |
| H8 | (k = 0 path) | D at `k = 0`: `solve_static_harmonic` against `mpc::solve_static_mpc` of the sector model carrying the pressure load and the two nodal forces (the cosine part): max abs difference / max abs | `<= 1e-14` | the same transformation, reduction and factorisation; equal bits expected |
| H9 | (defaults) | the existing tests of `rcfd-fem` (among them every `si_cyclic` test but the record of D5) pass unchanged | all pass | D2 builds the same rows without a rigid set |

**Negative controls (each must hold):**

| Id | Control | Expected |
| --- | --- | --- |
| N1 | D with a non-zero sine part at `k = 0` and at `k = 6`; a non-zero sine reference load on DR at `k = 6` | `InvalidValue { key: "harmonic load" }` |
| N2 | D carrying `Load::pressure("xmax", 1e6)` in its load list | `InvalidValue { key: "loads" }` |
| N3 | a cosine part of length `n - 1` | `FieldLength` |
| N4 | `k = 7` | `InvalidValue { key: "harmonic" }` |
| N5 | D with a prescribed value `1e-3` (extra) at `k = 2` refused ("zero values only"); the same at `k = 0` solved (non-zero values at `k = 0`) | as stated |
| N6 | a reference load on constraint 0 of D (a `Displacement`) | `InvalidValue { key: "reference load" }` |
| N7 | the `ConjugateGradient` solver | `InvalidValue { key: "solver" }` |
| N8 | D plus a rigid set on two nodes (collinear) at `k = 2`, static | refused as a singular reduced stiffness at a reference rotation (the message names "reference rotation" and "RigidSet") |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f489-scratch`, own target directory; each applied
alone by exact text replacement, the test file run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the sine-part rigid-set rows use the cosine reference unknowns | H2, H3, H5 or H6 |
| MU2 | `expand_load` with `c f_a + s f_b` | H1 |
| MU3 | `expand_load` overwrites instead of adding (shared nodes) | H1 or H4 |
| MU4 | the sine reference load put on the cosine reference unknowns | H2 |
| MU5 | the rigid-set rows built on the cosine part only | H2, H3, H5 or H6 |
| MU6 | the rotation term of the harmonic rigid-set rows with the wrong sign | H2, H3, H5 or H6 |
| MU7 | the refusal of a non-zero sine part at `k = 0`, `N/2` removed | N1 |
| MU8 | the pivot test of the harmonic static solve removed | N8 |

## Decision rule

**Done** if every check meets its limit, every negative control holds, MU1 to MU8 are caught on the first run, and no
existing test fails other than the record made under D5; **partial** if a check misses or is not run, a negative control
fails, a mutation survives, or another existing test fails; **todo** if the harmonic static problem cannot be built or a model
without a harmonic load or rigid set at `k > 0` changes its result.

## D5. Permitted edit of an existing test (maintainer decision 2026-11-13)

`si_cyclic.rs` `r6_rigid_set_and_non_zero_value_refused_at_k_2` asserts the refusal of a RigidSet at `k = 2` (retired here)
and of a non-zero value at `k = 2` (still valid). It is marked `#[ignore]` with a reason naming ADR 0395 as a record (body
untouched), and a copy `r6_non_zero_value_refused_at_k_2_after_landed` keeps the valid row.

## Limits (what is not covered)

- Harmonic prescribed displacements (`k > 0` with non-zero values) are refused; the harmonic load is given as two vectors (no
  harmonic form of the model's own load list); only the Direct solver.
- A rigid set of the sector is one rigid set per sector in the full model; a single rigid body spanning several sectors (a
  hub as one rigid set of the whole rotor) is not expressible on the sector. Mode shapes of the full model with rigid sets
  (expansion of the reference unknowns, eigen-equation residual) are not checked here; only frequencies and counts (c3).
- No harmonic response (forced vibration in time), no pre-stress, Coriolis or spin softening (ADR 0386 limits stand).
- A load that is not of a single harmonic is the caller's decomposition (one solve per harmonic, superposed).

## Open questions

1. The API carries the load as vectors rather than as a harmonic tag on `Load`. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation): vectors, because `Load` is shared with the case schema and a new
   field there would change goldens.

## Amendment 1 (2026-11-13, after the library code and before the test file was written or any check run)

1. **MU8 could not be caught as registered.** The rigid set of N8 (two nodes on a line along the axis) leaves the reference
   rotation about that line in no row at all: its row and column of `K_r` are empty, the pivot is exactly zero and the skyline
   factor's own positive-pivot rule (`d <= (w + 1) eps (|a_ii| + sum)`) refuses it before the relative test `d <=
   PIVOT_TOLERANCE a_ii` is reached. Removing that relative test (MU8) therefore leaves N8 unchanged. Replaced by **MU8'**:
   the mapping of the factor's pivot refusal to the named refusal (`SkylineError::Pivot` -> "singular reduced stiffness at the
   reference rotation ... (RigidSet)") replaced by the generic message of the factor; must fail N8 (whose check of the message
   names "reference rotation" and "RigidSet"). The relative pivot test stays in the code (as in `mpc::solve_static_mpc`) and
   is listed under the limits as not exercised by a check of this package.
2. `StaticOptions::default()` (the Direct solver) is the option of every static solve of the checks, sector and full.
