# ADR 0401: F-495 registration: refuse a mechanism in buckling on the transformation path (the zero-shift pivot-ratio rule)

Status: **REGISTERED 2026-11-13, written before any code of F-495 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-495 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand (disclosed). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: [ADR 0398](0398-f493-registration-prestressed-modes-and-buckling-on-the-transformation.md) (F-493: `buckling::factors`
on the transformation `x = T q`), its known miss C13 (BM, the knife-edge beam without its roller rows, returned the factor
`1.8176e-8` instead of a refusal; kept as the ignored record `si_prestress_mpc::c9_c13_zero_prestress_and_refusals_known_miss`,
which this ADR does not touch) and its remaining question 1, option (a), taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation). The rule: ADR 0384 / ADR 0283 D3 (`Shift::Auto`, pivot ratio `d_i / a_ii < 1e-8` at zero
shift; `mpc::zero_shift_refused`). The count of free rigid motions: [ADR 0399](0399-f488-registration-zero-modes-on-the-transformation.md)
(`free_motions::free_rigid_motions`). Roadmap item F-495 (deps F-493). Size S. Number 0401 given by the lead. Branch
`f495-buckling-mechanism-refusal` from main b9d6c7a1, worktree `w49i-work/f495`, scratch `w49i-work/f495-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the
prescribed path of `buckling::factors` is not touched; on the transformation path a model that passes the new test runs exactly
the lines it ran before, so its factors and modes keep their bits); no edit of `baseline_hashes.rs`, of the hash tables or of a
golden file; no new dependency, no unsafe code; clean-room. No existing test is edited: this package retires no refusal row (it
adds one). `harmonic.rs` (a parallel package, F-491) is not touched.

## What F-495 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. buckling::factors of a model on the transformation path
> whose constraints and supports leave a motion free (ADR 0398's knife-edge beam without its roller rows, and without its roller
> and spring) is refused with FemError naming the unknown of the failing pivot, by the zero-shift pivot-ratio rule of ADR 0384
> (ratio below 1e-8); a model held by a soft spring whose ratio is above the rule is still solved and its first factor equals k L
> within 1 percent; every check of ADR 0398 keeps its value bit for bit.

Clauses: (b1) BM (the knife-edge beam with its pin rows as `Linear` rows and no roller) is refused; (b2) "without its roller and
spring" is refused; (b3) the refusal is a `FemError` naming the unknown of the failing pivot, decided by the zero-shift
pivot-ratio rule (ratio below `1e-8`); (b4) a model held by a soft spring whose ratio is above the rule is solved and its first
factor is `k L` within 1 percent; (b5) every check of ADR 0398 keeps its value bit for bit.

Reading of the clauses, registered:

- **(b2)** ADR 0398's BS(k) without its spring is BF, which has no support and no linear row: it is on the prescribed path and
  already refused there (`RigidBodyMotionFree`, ADR 0398 A3), not on the transformation path that the entry is about. The
  reading registered: the same beam (pin, no roller, no lateral spring) **kept on the transformation path by a support that
  holds nothing of the free motion**, in two forms: **BZ**, a nodal spring on the `ke1` nodes in `z` (every `u_z` is
  prescribed, so the spring acts on eliminated equations only and adds nothing, ADR 0398 D1 "spring beside the roller"), and
  **BX**, an axial nodal spring on the `ke1` nodes in `x` (the free rigid rotation about the pin line has `u_x = -theta (y -
  h/2) = 0` on the `ke1` line, so the spring is not engaged). BF itself is checked to stay refused as before (`RigidBodyMotionFree`).
  Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **(b3)** "naming the unknown of the failing pivot": the error is `FemError::Solver { solver: "lowest_buckling_factors", reason }`
  (the variant ADR 0398 D2 step 3 registered for this refusal, so that its known-miss record describes the refusal now given),
  whose reason contains the name of the unknown (`mpc::unknown_name`: "node N component C", or a reference unknown of a rigid
  set) and the ratio. "The failing pivot" is the first pivot, in the elimination order of the zero-shift factor (`SkylineOptions::
  default()`: reverse Cuthill-McKee, positive pivots), whose ratio `d_i / a_ii` is below `1e-8`, or the pivot the factorisation
  itself refuses. Checked two ways: the name equals the one computed in the test from public functions (identity), and the
  named unknown moves in the free motion (meaning, D1).
- **(b4)** "a soft spring whose ratio is above the rule": ADR 0398's BS(k) (the roller replaced by a lateral spring of total
  stiffness `k` on the `ke1` nodes) with `k = 1e4 N/m`, a hundred times softer than ADR 0398's `1e6` and with the lowest
  pivot ratio estimated in arithmetic 2 above `1e-8` by at least a factor 12. "Its ratio" is measured in the test (the smallest
  `d_i / a_ii` of the same zero-shift factor) and must be above `1e-8`; "equals `k L` within 1 percent" is `|lambda_1 |s_ref| A /
  (k L) - 1| <= 0.01`, as ADR 0398 A3.
- **(b5)** "every check of ADR 0398 keeps its value bit for bit": the active tests of `si_prestress_mpc.rs` print every measured
  value with `{:?}` (the shortest representation that reads back to the same `f64`, so equal text is equal bits) and assert
  bit-equality to the reduction in the test (A1). The registered measure: the log of those tests run once on this registration's
  commit (before any code) and once after the code, compared line by line except the timing lines of the harness; every line
  equal, and every test passing.

## What was read (no run)

- `AGENT_RULES.md`; the brief (the reading of (b2) left to the ADR; "a mechanism need not be rigid": register the test used, why,
  and what it misses); `docs/roadmap.json` entry F-495 (in the main checkout's working copy; not present on this branch's base);
  ADR 0398 (whole: D1 to D6, the set-ups, C13, amendment 2, addendum 1, the diagnosis and remaining question 1); ADR 0399 (the
  counting rule, `COUNT_TOLERANCE`, the observation that `zero_shift_refused` and the Auto rule are the same computation, the
  limits: mechanisms are not counted, cyclic constraints and rotational unknowns refused).
- Code: `crates/rcfd-fem/src/buckling.rs` (whole), `mpc.rs` (`Transformation` and its accessors, `reduce`, `expand`, `singular`
  and `direct` of the static solve, `ZERO_SHIFT_PIVOT_RATIO`, `unknown_name`, `zero_shift_refused`, `modal_analysis_mpc`,
  `refuse_nonzero_values`), `free_motions.rs` (the module doc and `free_rigid_motions`, `FreeRigidMotions`),
  `crates/rcfd-linalg/src/skyline.rs` (`SkylineOrdering`, `PivotRule`, `SkylineOptions::default`, `SkylineError::Pivot` with
  the original row, `order`, `pivots`), `eigen.rs` (`lowest_buckling_factors`: its factor of `K` with `PivotRule::Positive`),
  `solve.rs` (`PIVOT_TOLERANCE`), `model.rs` (`ElasticSupport::nodal_spring`, stiffness positive and finite), `mesh.rs`
  (`SolidMesh::new`, `add_block`, `add_node_set`), `builders.rs` (`hex_box`, the Hex8 node order), `geometric.rs`
  (`Prestress::uniform`).
- Tests: `crates/rcfd-fem/tests/si_prestress_mpc.rs` (whole: the set-ups `beam(kind, s)`, `Kind`, the helpers, the records).
  Searched: no crate other than `rcfd-fem` calls `buckling::factors`; in `rcfd-fem` only `si_geometric.rs`,
  `si_knife_edge_closed_form.rs` (both prescribed path) and `si_prestress_mpc.rs` do.
- **Not read:** any finite-element code or manual outside this repository.

## Design

### D1. Which test, why, and what it misses

`K_r = T^T K_ext T` is symmetric positive semidefinite (an assembled elastic stiffness plus springs, congruent by `T`). A motion
the constraints and supports leave free is a vector `z != 0` with `K_r z = 0`, rigid or not. Two tests are available in the
repository:

1. **The count of free rigid motions** (ADR 0399, `free_rigid_motions`): the rank of the rows on the six rigid fields. It sees
   rigid motions only. A **mechanism that is not rigid** (two parts joined by a hinge line, a part held by fewer rows than its
   own rigid motions while the rest is held) has `K_r z = 0` with a count of zero: it would pass. It also refuses models with
   cyclic constraints or rotational unknowns.
2. **The zero-shift pivot ratio** (ADR 0384, ADR 0283 D3): factor `K_r = L D L^T` in the default order and refuse if any `d_i /
   a_ii < 1e-8`. For a positive semidefinite matrix the leading block `{1..j}` (elimination order) is singular exactly when a
   null vector of `K_r` is supported on `{1..j}`; the first such `j` has a null vector with a non-zero component at `j`, and the
   exact pivot `d_j` is zero there. In floating point `d_j` is a rounding residue, of relative size about `eps` times the growth
   of the elimination, far below `1e-8` (ADR 0398 measured BM's consequence: the factor `1.8e-8`, which with `z^T K_sigma z ~
   |s| theta^2 V` means a pivot of about `1e-4 N/m` against a diagonal of about `1e10`, a ratio of order `1e-14`). The test sees
   **every** null direction, rigid or not, including the reference unknowns of rigid sets, and it names an unknown: the row of
   the failing pivot, which by the argument above moves in a free motion.

**Registered: the pivot-ratio test** (clause (b3) names it; it covers non-rigid mechanisms, which the count misses, check C3).
What it misses or gets wrong, registered as limits and partly checked:

- **Resolution.** The test is relative to the diagonal: a model held only by a spring so soft that its pivot ratio is below
  `1e-8` is refused although it is physically held (a false refusal; check C2 with `k = 1e-4 N/m`). The boundary in `k` depends
  on the mesh and the elimination order (arithmetic 2: for this beam between about `3e-3` and `8e2 N/m`).
- **The name is one unknown of the free motion**, the last one of its support in the elimination order, not a description of
  the motion. Another ordering names another unknown.
- **A rounding residue above `1e-8`** (a very ill-conditioned model whose elimination growth reaches `1e8`) would pass. Not
  expected for meshes of the size in the repository; not checked.
- The test costs one more factorisation of `K_r` per call (the eigensolver factors it again; the factor is not shared so that
  the eigensolver's lines stay those of ADR 0398).

### D2. Code

`mpc.rs`, additive only (no existing line changed): a new function

```text
pub(crate) fn zero_shift_failing_pivot(k: &CsrMatrix) -> Result<Option<(usize, f64)>>
```

that factors `k` with `SkylineOptions::default()` (the factor of `zero_shift_refused`) and returns, for the first pivot in the
elimination order with `d_i / a_ii < ZERO_SHIFT_PIVOT_RATIO` or a NaN ratio, its row of `k` (`order[i]`) and the ratio; for a
factorisation refused by `SkylineError::Pivot { row, pivot, .. }` the row and `pivot / a_row,row`; `None` if every pivot
passes; any other factorisation error (non-finite, unsymmetric) as `FemError::Solver { solver: "skyline-ldlt" }`.

`buckling.rs`, `factors_on_transformation`: after the factor-count check and before `lowest_buckling_factors`,

```text
if let Some((row, ratio)) = mpc::zero_shift_failing_pivot(&kr)? {
    return Err(FemError::Solver { solver: "lowest_buckling_factors", reason: format!(
        "{name}: pivot ratio {ratio:e} of T^T K_ext T at zero shift, below 1e-8 (the rule of Shift::Auto, ADR 0384): \
         the constraints and supports leave a motion free (a mechanism); a buckling analysis needs every motion held \
         (ADR 0401)") });
}
```

with `name = mpc::unknown_name(&tr, model, tr.independent()[row])`. The docs of `factors` and of the module say so. The
prescribed path of `factors` and `prestressed_modes` are not changed (the modes on the transformation path keep ADR 0398's Auto
shift and `rigid_free`).

### D3. Files

`crates/rcfd-fem/src/buckling.rs` (D2), `crates/rcfd-fem/src/mpc.rs` (one new function), new test file
`crates/rcfd-fem/tests/si_buckling_mechanism.rs` (the set-ups copied from `si_prestress_mpc.rs`, which is not edited). Nothing
else.

### D4. The set-ups

ADR 0398 D5: `E = 2e11 Pa`, `nu = 0`, `rho = 7850`, `h = b = 0.1 m`, `L = 2 m`, `A = 0.01 m^2`, Hex20 `hex_box([0,0,0], [L, h,
b], [40, 2, 1])` upgraded, node sets `ke0`, `ke1`, `every`, tractions `(+-s, 0, 0)` on `xmax`/`xmin`, `s_ref = -1e6 Pa`.
`BucklingOptions::new(2)`.

- **B**: ADR 0398's B (prescribed knife edges). Its static pre-stress `P_B` is used for every model that is refused (the mesh is
  the same; the refusal comes before the eigensolver).
- **BM**: ADR 0398's BM (pin as `Linear` rows on `ke0`, no roller, `u_z = 0` everywhere).
- **BZ**: B without the roller row, plus `nodal_spring("ke1", 2, 1e6/3)`. **BX**: B without the roller row, plus
  `nodal_spring("ke1", 0, 1e6/3)`. **BF**: B without the roller (old path).
- **BS(k)**: ADR 0398's BS (roller replaced by `nodal_spring("ke1", 1, k/3)`), `k = 1e4 N/m` with its own static pre-stress
  (the static solve on the transformation path, `PIVOT_TOLERANCE = 1e-10` below the ratio of arithmetic 2), and `k = 1e-4 N/m`
  with `P_B`.
- **H** (a hinge, a mechanism that is not rigid): Hex8 blocks `A = [0, a]^3` and `B = [a, 2a] x [a, 2a] x [0, a]`, `a = 0.1 m`,
  sharing the edge `x = y = a` (two nodes), 14 nodes, the node order of `hex_box`; node set `a0` (the four nodes of `A` at `x =
  0`) prescribed in all three components; `nodal_spring("a0", 0, 1e6)` (on prescribed equations: it adds nothing and puts the
  model on the transformation path); `Prestress::uniform([-1e6, 0, 0, 0, 0, 0])`. Block `B` turns freely about the hinge line
  (`u = theta (-(y - a), x - a, 0)` on `B`, zero on `A`): a mechanism with every rigid motion held. **HN**: H plus
  `nodal_spring("bfar", 1, 1e6)` on the two nodes of `B` at `x = y = 2a` (engaged by the hinge rotation: `u_y = theta a`).
  **HP**: H without its spring (prescribed path), recorded only.

## Arithmetic before this registration (disclosed, by hand)

1. **BM's ratio.** From ADR 0398's measured factor (D1): ratio of order `1e-14`, more than five decades below `1e-8`. BZ and BX
   have the same `K_r` null direction (the springs do not touch it): same order.
2. **The pivot of a spring-held sway.** For `K_r` with a near-null direction `z` (the rigid rotation about the pin, `u_y = theta
   x`, `u_x = -theta (y - h/2)`), the pivot at the end of its support is `d_j ~ z^T K z / z_j^2` (the Schur complement in the
   direction of `z`), `z^T K z = k theta^2 L^2` (three nodes of stiffness `k/3`, each moved `theta L`), `z_j = theta r_j` with
   `r_j` the lever of the unknown (`x_j` for `u_y`, `|y_j - h/2|` for `u_x`; on this mesh `0.025 <= r_j <= 2`). The diagonal of a
   Hex20 node: `c E V_e / h_e^2`, `V_e = 2.5e-4 m^3`, `h_e = 0.05 m`, `E V_e/h_e^2 = 2e10 N/m`, with `c` from 0.1 to 4 (shape
   factor and up to four elements per node): `a_jj` from `2e9` to `8e10`. Ratio `k L^2 / (r_j^2 a_jj)` from `k x 1.25e-11` (`r = 2`,
   `a = 8e10`) to `k x 3.2e-6` (`r = 0.025`, `a = 2e9`).
   - BS(1e4): ratio between `1.25e-7` and `3.2e-2`: above `1e-8` by at least 12.
   - BS(1e6), BS(1.5e6) of ADR 0398: at least `1.25e-5`; BS(1e11) and the models without a near-null direction (B-like): far
     above. ADR 0399 observed that the Auto rule's zero-shift decision is the same computation as `zero_shift_refused`, and ADR
     0398 C12 measured shift 0 on BS(1e6)'s and R's modal runs at `s = 0` and on BL, so no ADR 0398 buckling model is expected to
     be refused (b5).
   - BS(1e-4): ratio at most `3.2e-10`, below `1e-8` by at least 30 (refused, C2). The boundary `k` where the ratio is `1e-8`:
     between `3e-3` and `8e2 N/m` (D1, resolution).
3. **BS(1e4)'s factor.** `k L = 2e4 N`, `lambda_1 = k L / (|s_ref| A) = 2`; deficit `c_0 k/(G b)` (ADR 0398 D1) `= 1e-6 c_0`,
   `c_0 <= 5`: `<= 5e-6` against `0.01`. Its second factor is the bending one, ADR 0398 measured `lambda_1(B) = 408.40047`;
   with the spring this soft the roller reaction of the knife-edge mode is not resisted: as ADR 0398 C4 (BS(1e6) `7.1e-12`),
   estimate `<= 1e-6`, limit `1e-3`. The Rayleigh bound `lambda_1 <= 2` holds exactly (ADR 0398 D1).
4. **H.** Unknowns `14 x 3 - 12 = 30`; the hinge rotation is the only null direction of `K_r` (block `B` keeps two points of a
   line, one rotation about it left); the rigid count is 0 (block `A`'s face holds the six rigid motions). HN: the spring holds
   the rotation with `z^T K z = 1e6 theta^2 a^2` against a Hex8 diagonal of about `E a / 4 ~ 5e9`: ratio of order `1e-4`,
   solved. The named unknown of H moves in the hinge rotation: `u_x` or `u_y` of a node of `B` not on the hinge.
5. **Run time.** About eight static solves and three buckling solves of 2319 unknowns plus a few factorisations; ADR 0398's file
   ran in seconds: estimate under 10 s in release.

## Registered checks

All in `crates/rcfd-fem/tests/si_buckling_mechanism.rs`, release profile, run once: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem
--release --test si_buckling_mechanism -- --nocapture --test-threads=1`. "Refused as registered" means `Err(FemError::Solver {
solver: "lowest_buckling_factors", reason })` with `reason` containing "pivot ratio" and the name of an unknown, and both
naming conditions below. **Identity**: the name in `reason` equals `"{name}:"` computed in the test: the reduction
`T^T K_ext T` from public functions (`mpc::transformation`, `assemble::stiffness`, `mpc::add_supports`, `Transformation::reduce`),
its factor with `SkylineLdlt::factor_with(&kr, &SkylineOptions::default())`, the first pivot with `d_i / a_ii < 1e-8` (or the
refused row), mapped by `tr.independent()` to node and component. **Meaning**: the named unknown moves in the free motion:
`|z(named)| / max |z| >= 1e-3`, `z` the free field of the model written in the test (BM, BZ, BX: `u_x = -(y - h/2)`, `u_y = x`,
`u_z = 0`; H: `u_x = -(y - a)`, `u_y = x - a`, `u_z = 0` on the nodes of block `B`, zero on `A`).

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | b1, b3 | BM with `P_B`: refused as registered (identity and meaning); the smallest pivot ratio of its zero-shift factor computed in the test | as stated; `< 1e-8` | refused; ~1e-14 (arith. 1) |
| A2 | b2, b3 | BZ and BX with `P_B`: each refused as registered (identity and meaning), smallest ratio `< 1e-8`; BF still `RigidBodyMotionFree` | as stated | as A1 |
| A3 | b4 | BS(1e4) with its own pre-stress: solved (Ok, two factors, Sturm counted = expected = 2); its smallest pivot ratio computed in the test; `|lambda_1 |s_ref| A / (k L) - 1|` | `> 1e-8`; `<= 0.01` | `1.25e-7` to `3.2e-2`; `<= 5e-6` |
| A4 | b5 | the active tests of `si_prestress_mpc.rs` (`a1_c12_...`, `a2_..._corrected`, `a3_...`, `c6_..._rigid_end`, `c9_c13_..._met_rows`) run with `--nocapture --test-threads=1` on the registration commit and after the code: every printed line equal (the harness's timing lines excepted), every test passing both times | 0 lines differ; 5 passed | equal (D2: a model that passes runs the old lines) |

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | BS(1e4): `lambda_1 <= 2 (1 + 1e-9)` (Rayleigh); `|lambda_2 / 408.40047... - 1|` against B's factor computed in the same test | holds; `<= 1e-3` | exact; `<= 1e-6` |
| C2 | BS(1e-4) with `P_B` (resolution, a false refusal): refused as registered (identity; meaning with the sway field); smallest ratio | as stated; `< 1e-8` | `<= 3.2e-10` |
| C3 | H (a mechanism that is not rigid): `free_rigid_motions(H).count == 0`, and `factors` refused as registered (identity and meaning with the hinge field) | as stated | 0; refused |
| C4 | BM: `free_rigid_motions(BM).count == 1` (the two tests agree on a rigid mechanism) | `== 1` | 1 (ADR 0399 counts the pin rows: rank 5 of 6) |
| C5 | BS(1e4): `factors` bit-equal (factors and every mode value) to `lowest_buckling_factors` on the reduction in the test (the guard does not change a solved model) | bit-equal | equal |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | for BM, the name obtained by taking the failing row of the reduced matrix as a mesh equation (without `tr.independent()`) | differs from the reported name (the identity check sees the mapping) |
| N2 | HN (the hinge held by a spring on block `B`): `factors` | Ok (solved), so H's refusal is the mechanism |

### O. Recorded, no limit

| Id | What |
| --- | --- |
| O1 | HP (the hinge on the prescribed path): the result of `factors` (`Ok` factors or the error), printed. The prescribed path is not changed here; if it returns a factor, that is a mechanism the geometric rigid-motion test of the prescribed path does not see, reported as a remaining question. |
| O2 | ADR 0398's ignored record `c9_c13_zero_prestress_and_refusals_known_miss` run once after the code with `--ignored --exact` (not edited): pass or fail, printed. |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --no-fail-fast` | 0 failed |
| S2 | `cargo check -p rcfd-post --tests` | builds |
| S3 | `git diff b9d6c7a1 --stat -- crates`: only `buckling.rs`, `mpc.rs` (one added function) and the new test file | as stated |

`rcfd-api`'s `baseline_hashes` is not run: no `rcfd-api` path calls `buckling::factors`, and the edit of `mpc.rs` adds a function
no existing path calls (brief: tests of `rcfd-fem` only). Recorded.

### Mutations (scratch copy `w49i-work/f495-scratch/copy`, `git archive` of the code commit, own target directory)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the guard removed | A1, A2, C2, C3 (factors returned) |
| Mu2 | the rule's ratio `1e-1` in place of `1e-8` in `zero_shift_failing_pivot` (too strict) | A3 (BS(1e4) refused: its ratio is at most `3.2e-2`, arith. 2) |
| Mu3 | the failing row named as a mesh equation (without `tr.independent()`) | A1, A2 identity |
| Mu4 | the guard applied to the unreduced `K_ext` (singular on every model) | A3, A4, C5, N2 |
| Mu5 | the guard only when the model has an elastic support | A1 (BM has linear rows only) |

## Decision rule

- **done**: A1 to A4 met, every C and S check met, the negative controls as expected, every mutation caught.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed, a negative
  control not as expected, or a mutation surviving.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- The resolution of the rule (D1): springs giving a ratio below `1e-8` are refused although held (C2); the boundary is mesh and
  order dependent.
- The name is one unknown of a free motion, not the motion; for several free motions only the first failing pivot is named.
- `prestressed_modes` and the prescribed path of `factors` are not changed (O1 records the prescribed path on a hinge).
- One extra factorisation per buckling call on the transformation path.
- Cyclic constraints (on the transformation path) are covered by the same code but not checked.
- The workspace suite and `rcfd-api` are not run (the lead's gate; the brief).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The reading of (b2) as the beam without roller and lateral spring held on the transformation path by a spring that holds
   nothing of the free motion (BZ, BX), BF staying on the prescribed path. Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. The error variant `FemError::Solver { solver: "lowest_buckling_factors" }` (ADR 0398 D2's), not `SingularStiffness` (which
   cannot name a reference unknown and whose text speaks of the direct solver). Taken on the maintainer's standing instruction
   of 2026-11-11 (take the recommendation).
3. The pivot-ratio test, not the rigid count (D1). Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
