# ADR 0385: F-485 registration: Hex20 cantilever with the beam-theory support and the bore stress on a refined mesh

Status: **REGISTERED 2026-11-13, written before any code of F-485 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-485 was written, compiled or run: the code listed under "What was read" was read and
the arithmetic below was done by hand. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0292](0292-quadratic-hexahedron-hex20.md) (F-389, the Hex20 element; its known misses C1 and T1b and its
remaining questions 2 and 3, option (a) of each, from which this item comes). Roadmap item F-485 (depends on F-389). Size S.
Branch `f485-hex20-beam-support-bore` from main 4b8f3b59, worktree `w49i-work/f485`, scratch `w49i-work/f485-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (this item
adds a test file only, no library code); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file;
no existing test edited (`si_hex20.rs` and its ignored records `c1_amended_hex20_cantilever_known_miss` and
`t1_thick_cylinder_plane_strain_known_miss` stay as they are); no new dependency, no unsafe code; clean-room.

## What F-485 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Cantilever L/h = 20, nu = 0.3, one Hex20 through the
> thickness and 10 along, with the support of beam theory at the root (u_x = 0 on the root face, u_y and u_z fixed on its neutral
> line), registered before the run: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)), the full clamp
> recorded beside it. Thick cylinder b/a = 2, plane strain, Hex20 with the radial element count derived in the registration from
> the second-order rate of ADR 0292's diagnostic: bore hoop stress within 1 percent and bore displacement within 0.2 percent of
> Lame.

Clauses: (a1) the support, derived and registered here (D1); (a2) tip deflection within 1 percent of `delta_T`; (a3) the full
clamp recorded beside it; (a4) the radial count derived here from the second-order rate (D2); (a5) bore hoop stress within 1
percent; (a6) bore displacement within 0.2 percent.

## What was read (no run)

- ADR 0292 (whole): set-ups C1 and T1 (arithmetic 4 and 5), amendment 1 (the sign of the deflection measure), the addendum
  table (C1 ratio 0.981949, T1a 7.3e-5, T1b 1.168e-2), the "Diagnosis of the two misses" (numbers quoted below), the remaining
  questions 2 and 3.
- `crates/rcfd-fem/tests/si_hex20.rs`: `Checks`, `material`, `model_of`, `cantilever_ratio_amended` (mesh `hex_box([0; 3], [2,
  0.1, 0.1], [10, 1, 1])` upgraded, traction `(0, 0, -P / h^2)` on `xmax` by `traction_loads`, `delta = sum_a f_az u_az / P`,
  `delta_T` with `kappa = 5/6`, `G = E / (2 (1 + nu))`), `t1_amended_met_clauses` (the quarter cylinder, its constraints, the hoop
  stress of `nodal_average(None)`).
- `crates/rcfd-fem/src/solve.rs` (`solve_static_report`: `check_rigid_body_motions` before the stiffness, reactions `K u - f` per
  unknown stored in the result), `crates/rcfd-fem/src/constraint.rs` (`check_rigid_body_motions`: the 6 x 6 Gram matrix of the
  rigid motions restricted to the prescribed unknowns, refused with `FemError::RigidBodyMotionFree` when an eigenvalue is below
  `RIGID_TOLERANCE` times the largest), `crates/rcfd-fem/src/model.rs` (`SolidResult::reactions`, `DofMap::equation`),
  `crates/rcfd-fem/src/mesh.rs` (`add_node_set`), ADR 0383 (Load::Pressure / Traction on Hex20 faces through the model path; not
  needed here: the `hex20::{pressure_loads, traction_loads}` forces of ADR 0292 are used as before, so the set-ups are those of
  the ADR 0292 records).

## Design

No library code. A new test file `crates/rcfd-fem/tests/si_hex20_support_bore.rs` with the checks below.

### D1. The support of beam theory at the root (clause a1)

Mesh: `hex_box([0, 0, 0], [L, h, h], [10, 1, 1])`, `L = 2`, `h = 0.1` (`L/h = 20`), upgraded by `upgrade_to_hex20`. The root face
`x = 0` (node set `xmin`) has 8 nodes: the corners `(y, z) in {0, h}^2` and the mid-edge nodes `(h/2, 0)`, `(h/2, h)`, `(0,
h/2)`, `(h, h/2)`. The load `P` acts in `-z`; the neutral surface of bending about `y` is `z = h/2` (the centroid of the square
section), and the neutral line of the root face is `{x = 0, z = h/2, 0 <= y <= h}`. On it lie exactly two nodes, the mid-edge
nodes `n0 = (0, 0, h/2)` and `n1 = (0, h, h/2)` of the two vertical root edges (Hex20 has no face-centre node).

The support, registered:

1. `u_x = 0` at all 8 nodes of the root face (node set `xmin`, component 0).
2. `u_z = 0` at `n0` and `n1` (node set `root_neutral`, component 2).
3. `u_y = 0` at `n0` only (node set `root_neutral_y0`, component 1).

Every other root unknown (`u_y` at 7 nodes, `u_z` at 6) is free.

Why this is the support of beam theory and not a clamp (first principles):

- Item 1 makes `x = 0` a plane of symmetry: with zero tangential traction there (`u_y`, `u_z` free), the solution is the half of
  a beam of length `2L` loaded by `P` at both ends, mirrored about `x = 0`. The section at the root stays plane and normal to the
  axis (the rotation `psi(0) = 0` of the Timoshenko beam), while the lateral Poisson strains of the root section are free; this is
  the boundary condition under which the Timoshenko cantilever gives `w(L) = P L^3 / (3 E I) + P L / (kappa G A)` (rotation of the
  section zero, slope of the axis at the root equal to the shear angle `V / (kappa G A)`), which is `delta_T`.
- Rigid motions: item 1 removes the translation `t_x` and the rotations `r_y` (`u_x = r_y (z - z_0)`) and `r_z` (`u_x = -r_z (y -
  y_0)`) over the 8 points, which span the face. Item 2 removes `t_z` and `r_x` (`u_z = t_z + r_x y`, zero at `y = 0` and `y = h`).
  Item 3 removes `t_y` (`u_y = t_y - r_x (z - h/2)`, `r_x` already zero). Six rigid motions, six independent conditions beyond the
  symmetry plane: in the `y-z` plane the support is statically determinate (three reactions, `R_z(n0)`, `R_z(n1)`, `R_y(n0)`, for
  three equilibrium equations: forces in `y` and `z`, moment about `x`). It adds no restraint to the deformation of the root
  section in its own plane, so it adds no clamp.
- Consistency with beam theory of the values fixed: on the neutral line `sigma_x = 0`, so `eps_y = eps_z = 0` there (no Poisson
  strain); the anticlastic term of Saint-Venant bending, `u_z ~ nu kappa_b (y - h/2)^2 / 2`, is equal at `y = 0` and `y = h`, so
  fixing `u_z` at both nodes removes only `t_z` and `r_x`. Fixing `u_y` at both nodes (a variant, check R2) would add one redundant
  condition, consistent with `eps_y = 0` on the neutral line of beam theory but not needed; the registered support fixes `u_y` at
  `n0` only, the minimal one.
- Equilibrium consequences, closed form: `sum R_z = P`, `R_z(n0) = R_z(n1) = P / 2` (symmetry in `y`), `R_y(n0) = 0` (no load in
  `y`), `sum R_x = 0`, and the bending moment carried by the `u_x` reactions alone: `-sum_a R_x,a (z_a - h/2) = P L` (moment about
  the `y` axis through the neutral line of the root: the tip load contributes `-x F_z = L P`, the `R_z` act at `x = 0`).

The deflection measure is that of ADR 0292 amendment 1: `delta = sum_a f_az u_az / P` over the tip nodes, the deflection relative
to the root neutral line (where `u_z = 0`), exact for the consistent load.

The full clamp (clause a3): `xmin` fixed in all three components, the set-up of `c1_amended_hex20_cantilever_known_miss`, run
again in the new file and printed beside, with its reproduction of the recorded ratio checked (R1).

### D2. The radial element count of the cylinder (clause a4)

ADR 0292's registered run on `4 x 8` (radial x circumferential) gave the bore hoop-stress error `e(4) = 1.168e-2`. The
second-order rate: `e(n_r) = e(4) (4 / n_r)^2`. Required: the predicted error at most one third of the 1 percent limit, the margin
3 covering a rate below 2 (the diagnostic of ADR 0292 showed 1.64 between 4 and 8 and 1.86 between 8 and 16, disclosed; with rate
1.5 a halving of `h` still gives `e(4) / 2^1.5 = 4.1e-3`, below the limit):

`n_r >= 4 sqrt(3 e(4) / 0.01) = 4 sqrt(3.504) = 7.49`, so **`n_r = 8`**.

The circumferential count stays 8 (the registered T1 mesh; ADR 0292's diagnostic: 4 x 16 against 4 x 8 changes the error by 1.4
percent of itself, the circumferential resolution does not matter). Mesh: `hex_box([1, 0, 0], [1, pi/2, 0.2], [8, 8, 1])` mapped
by `(r, theta, z) -> (r cos theta, r sin theta, z)` with `upgrade_to_hex20_mapped`; constraints, load (`p = 1e8` on `xmin` by
`pressure_loads`), material and measures exactly those of ADR 0292 T1.

Disclosure: ADR 0292's unregistered diagnostic measured this mesh (8 x 8) once, `3.75e-3`; the result is therefore expected,
not unknown. This registered run is made once, as any other.

### Arithmetic before this registration (disclosed)

Cantilever (ADR 0292 arithmetic 4): `delta_T = 1.6031e-3 m`, the shear term `0.00195`. The ADR 0292 diagnostic gave the full
clamp with `nu = 0` on this mesh `0.999037`: the element bends correctly with one element through the thickness; the 1.8 percent
of the clamp with `nu = 0.3` is the restrained Poisson contraction at the root. The support of D1 frees that contraction.
Remaining error sources: (i) discretisation as with `nu = 0`, about 0.1 percent; (ii) the restraint of the shear warping at the
symmetry plane (`u_x = 0` over the face, also present in the `nu = 0` clamp, inside the 0.1 percent above); (iii) the local
compliance of the line reaction `2P` (both halves) at the neutral line: the 2-D Kelvin solution has a logarithmic displacement,
`(2P/h) (3 - 4 nu)(1 + nu) / (4 pi (1 - nu) E) ln(L/h_e)` against the beam deflection `4 P L^3 / (E h^4)` per width: relative
`2 x 0.27 x ln 20 / (4 x 8000) ~ 5e-5`. Expected ratio 0.998 to 0.9995, against the 1 percent limit.

Poisson strain at the root (check S2): `M = P L = 2000 N m`, `I = h^4 / 12 = 8.333e-6 m^4`, outer fibre stress `M (h/2) / I =
1.2e7 Pa` (top `z = h` in tension for the load in `-z`, bottom in compression), lateral strain `-/+ nu 1.2e7 / E = -/+ 1.8e-5`;
the width change of the root face along its top edge `u_y(0, h, h) - u_y(0, 0, h) = -1.8e-6 m`, bottom `+1.8e-6 m`. The
concentrated reaction disturbs the stress at the root fibres (Flamant-type `2 F / (pi r)` with `F = 2P/h = 2e4 N/m`, `r = h/2`:
`2.5e5 Pa`, 2 percent of the bending stress), and the moment is linear in `x`, which the element represents; expected within 3
percent, limit 10 percent. Under the full clamp both are exactly 0 (all prescribed).

Cylinder (ADR 0292 arithmetic 5): `u_r(a) = 1.9067 p / E`, `s_L(a) = 5p/3`. Expected on 8 x 8: T2b about `3.75e-3` (the
diagnostic), T2a well below 0.2 percent (`7.3e-5` on 4 x 8).

## Registered checks

All in `crates/rcfd-fem/tests/si_hex20_support_bore.rs`, release profile, run once (`cargo test -p rcfd-fem --release --test
si_hex20_support_bore -- --nocapture`), `E = 2e11`, `nu = 0.3`, `rho = 7850`, `P = 1000 N`, `kappa = 5/6`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| C2 | a1, a2 | the D1 support: `abs(delta / delta_T - 1)` | <= 0.01 | 1e-3 to 2e-3 |
| R1 | a3 | the full clamp, printed beside C2: `abs(ratio_clamp - 0.981949)` (reproduces ADR 0292's recorded ratio to its 6 printed digits); and `abs(ratio_clamp - 1) > 0.01` (the clamp still misses: the support, not the mesh, changed) | <= 1e-6; > 0.01 | same code path as the record |
| T2a | a4, a6 | 8 x 8 cylinder, `max abs(u_r / u_r(a) - 1)` over the bore nodes | <= 0.002 | below 1e-4 |
| T2b | a4, a5 | 8 x 8 cylinder, hoop stress `t^T sigma t` of `nodal_average(None)` at every bore node, `max abs(s / s_L(a) - 1)` | <= 0.01 | 3.75e-3 |

### K. The support itself (D1)

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | `xmin` has 8 nodes; exactly 2 of them have `abs(z - h/2) <= 1e-12`, at `y = 0` and at `y = h` (each within 1e-12) | as stated | D1 |
| K2 | the solve is accepted by `check_rigid_body_motions` (no `RigidBodyMotionFree`), i.e. the six rigid motions are removed | as stated | D1 |
| K3 | reactions: `abs(sum R_z / P - 1)`, `abs(R_z(n_i) / (P/2) - 1)` for both, `abs(R_y(n0)) / P`, `abs(sum R_x) / P`, `abs(-sum R_x (z - h/2) / (P L) - 1)` | each <= 1e-8 | roundoff of `K u - f`, about 1e-11 |
| S2 | Poisson strain free at the root: top and bottom edge width changes against `-/+ nu (P L h / (2 I)) h / E`, `abs(ratio - 1)` | each <= 0.1 | about 0.03 |
| R2 | variant with `u_y = 0` at both `n0` and `n1`: `abs(ratio_both - ratio)` | <= 1e-3 | below 1e-4 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | D1 with `u_z` fixed at `n0` only (item 2 incomplete) | refused with `FemError::RigidBodyMotionFree` (the rotation `r_x` about the line through `n0` is free) |
| N2 | the full clamp (R1): S2's width changes | both exactly 0 (all prescribed) |
| N3 | the cylinder on ADR 0292's 4 x 8 mesh in the same code | T2b measure `> 0.01` (the known miss, 1.168e-2, reproduced) |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff main -- crates` lists only the new test file; `cargo test -p rcfd-fem --release --test si_hex20` | 0 failed (no library code, so no baseline-hash run is owed) |

### Mutations (scratch copy of the new test file, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the "neutral line" at `z = 0` instead of `h/2` | K1 (3 nodes); also K3 moment arm |
| Mu2 | the support replaced by `u_y`, `u_z` fixed on the whole root face (a clamp) | C2, S2 |
| Mu3 | the cylinder radial count 4 instead of 8 | T2b |
| Mu4 | the cylinder material with `nu = 0` (Lame with 0.3) | T2a |
| Mu5 | `u_x` fixed at the 4 corner nodes of the root face only | K-rows or C2 (recorded whichever catches; may survive: the mid-edge `u_x` carries part of the moment) |

## Decision rule

- **done**: C2, R1, T2a and T2b met, every K, S and R check met, every negative control as expected.
- **partial**: any acceptance check (C2, T2a, T2b) missed with a diagnosed cause, or a K/S/R check missed; the miss kept as a
  `*_known_miss` record.
- **todo**: the checks could not be run.

## Limits of this registration

- The support is checked on the registered mesh only (10 x 1 x 1); no mesh study is registered.
- The 1 percent limit cannot see the shear term (0.2 percent of `delta_T`) nor the kappa of a square section: C2 confirms the
  Timoshenko deflection only to the precision of the acceptance.
- No stress check of the cantilever (none is in the acceptance).
- The cylinder is checked on 8 x 8 only; the rate is not re-measured (ADR 0292's diagnostic is quoted, not repeated).
