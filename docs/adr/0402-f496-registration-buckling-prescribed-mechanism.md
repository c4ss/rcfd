# ADR 0402: F-496 registration: refuse a mechanism that is not rigid in buckling on the prescribed path (the zero-shift pivot-ratio rule on K_FF)

Status: **REGISTERED 2026-11-13, written before any code of F-496 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-496 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand (disclosed). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: [ADR 0401](0401-f495-registration-buckling-mechanism-refusal.md) (F-495: the zero-shift pivot-ratio guard of
`buckling::factors` on the transformation path, `mpc::zero_shift_failing_pivot`), its observation O1 (the two-block hinge HP on
the prescribed path was refused only because the rounding pivot of the eigensolver's factor came out negative, "K + sigma M is
not positive definite for sigma = 0e0: pivot -9.5e-7 of row 13", with no name and not by the rule) and its remaining question 2,
option (a), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). The models whose factors
must keep their bits: [ADR 0390](0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md) (F-395, the beam B
and the column C of `si_geometric.rs`) and [ADR 0394](0394-f492-registration-knife-edge-buckling-closed-form.md) (F-492, the
knife-edge beams B20 and B40 of `si_knife_edge_closed_form.rs`). The rule: ADR 0384 / ADR 0283 D3 (pivot ratio `d_i / a_ii <
1e-8` at zero shift). Roadmap item F-496 (deps F-495). Size S. Number 0402 given by the lead. Branch
`f496-buckling-prescribed-mechanism` from main 3f2f0b9b, worktree `w49i-work/f496`, scratch `w49i-work/f496-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (a model
that passes the new test runs exactly the lines it ran before, so its factors and modes keep their bits); no edit of
`baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe code; clean-room. `harmonic.rs` (a
parallel package, F-491) is not touched.

## What F-496 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. buckling::factors on the prescribed-displacement path
> refuses a model whose reduced stiffness K_FF has a zero-shift pivot ratio below 1e-8 (the rule of ADR 0401) with FemError
> naming the unknown of the failing pivot, checked on ADR 0401's two-block Hex8 hinge with its face held by Displacement rows
> (which passes the rigid-motion test); the factors of ADR 0390's and ADR 0394's models keep their values bit for bit.

Clauses: (c1) on the prescribed path a `K_FF` with a zero-shift pivot ratio below `1e-8` is refused by the rule of ADR 0401;
(c2) the refusal is a `FemError` naming the unknown of the failing pivot; (c3) checked on ADR 0401's hinge HP (face `a0` held
by `Displacement` rows, no support, no linear row), which passes the rigid-motion test; (c4) the factors of ADR 0390's and
ADR 0394's models keep their values bit for bit.

Reading of the clauses, registered:

- **(c1)** "the rule of ADR 0401": the first pivot, in the elimination order of the factor `SkylineLdlt::factor_with(K_FF,
  &SkylineOptions::default())` (reverse Cuthill-McKee, positive pivots, symmetry tolerance `1e-12`), with `d_i / a_ii < 1e-8`
  or a NaN ratio, or the pivot this factorisation refuses (`SkylineError::Pivot`), as `mpc::zero_shift_failing_pivot` returns
  it. `K_FF` is the matrix `factors` already builds (`constraint::reduce` of the assembled `K` by the prescribed equations).
- **(c2)** The error is `FemError::Solver { solver: "lowest_buckling_factors", reason }`: the same variant and solver name as
  HP's refusal today (ADR 0401 O1), so that the error kind of every model refused today is kept; `reason` starts with `"node N
  component C: "` (the equation `free()[row]` of the failing row, by `DofMap::node_component`) and contains "pivot ratio".
  Checked two ways as ADR 0401: the name equals the one computed in the test from public functions (identity), and the named
  unknown moves in the hinge rotation (meaning).
- **(c3)** "passes the rigid-motion test": `constraint::check_rigid_body_motions(HP, prescribed_equations(HP))` returns `Ok`,
  and `free_motions::free_rigid_motions(HP).count == 0` (recorded as a companion).
- **(c4)** "keep their values bit for bit": factors **and modes** (the brief), measured two ways: (i) a 64-bit FNV-1a hash of
  the bits of every factor and every mode value of each model, printed by a new test, run once on the code commit and once on a
  scratch copy of the same commit with `buckling.rs` restored to its base text (3f2f0b9b); the hashes equal; (ii) the printed
  output of the existing tests that solve these models (`si_geometric.rs`, `si_knife_edge_closed_form.rs`; they print the
  factors with `{:?}`, the shortest text that reads back to the same `f64`), run once on this registration's commit and once
  after the code, every line equal except the harness's timing lines. ADR 0390's models solved by `factors`: B (Hex20, pin,
  `s = -1e6`, threads 1 and 8), the tension beam (`s = +1e6`, no factor below the exhausted bound), the Hex8 beam, the column C;
  ADR 0394's: B20 (`L = 2`, 40 x 2 x 1) and B40 (`L = 4`, 80 x 2 x 1). The models of (i): B, the tension beam, the Hex8 beam,
  C, B20, B40 (the pre-stresses as in those tests: the static solve at the reference load).

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entry F-496; ADR 0401 (whole), the set-ups of ADR 0390 (`si_geometric.rs`:
  `beam`, `column`, `box_mesh`, the calls of `factors` and their stresses) and ADR 0394 (`si_knife_edge_closed_form.rs`:
  `beam`, `buckle`).
- Code: `crates/rcfd-fem/src/buckling.rs` (whole), `mpc.rs` (`zero_shift_failing_pivot`, `unknown_name`, `zero_shift_refused`),
  `constraint.rs` (`prescribed_equations`, `check_rigid_body_motions`, `ReducedSystem`, `reduce`, all public),
  `crates/rcfd-linalg/src/eigen.rs` (`lowest_buckling_factors`: its zero-shift factor uses `SkylineOptions { ordering:
  opts.ordering (default Rcm), pivots: Positive, symmetry_tolerance: SYMMETRY_TOLERANCE = 1e-12 }`, the same options as
  `SkylineOptions::default()`, and maps a refusal to `EigenError::ShiftNotPositiveDefinite { shift: 0.0 }`),
  `crates/rcfd-linalg/src/skyline.rs` (`SkylineOptions::default`).
- Tests: `si_buckling_mechanism.rs` (whole; the hinge set-up and O1), `si_geometric.rs` (the refusal tests `c7_refusals`
  (ignored record) and `c7_refusals_after_transformation_landed`), `si_knife_edge_closed_form.rs`. Searched: no crate other than
  `rcfd-fem` calls `buckling::factors`; no test in the repository asserts the text "not positive definite" of a buckling
  refusal (grep of `crates/*/tests` and the crates' sources for that text and for `lowest_buckling_factors"`: only
  `si_buckling_mechanism.rs` matches the solver name, on the transformation path).
- **Not read:** any finite-element code or manual outside this repository.

## Design

### D1. The test and why it is the right one here

On the prescribed path `factors` refuses a free motion today only by `check_rigid_body_motions` (ADR 0275 D5: the rigid motions
of each connected part against the prescribed equations). A mechanism that is not rigid (HP: block B turns about the edge it
shares with block A, every rigid motion of the connected part held by the face `a0`) passes it, and `K_FF` is singular. What
happens then is decided by rounding: the eigensolver's factor of `K_FF` either refuses a negative rounding pivot (HP, with an
unnamed row of `K_FF`) or accepts a positive one of size `eps` and returns a meaningless factor of the order of that pivot (ADR
0398's BM on the other path returned `1.8e-8`).

The rule of ADR 0401 applies unchanged (its D1: `K_FF` is symmetric positive semidefinite, a principal submatrix of the assembled
`K`; the first leading block in the elimination order that is singular has a zero exact pivot at its last row, a rounding residue
far below `1e-8` in floating point, and that row moves in a free motion). It sees every null direction of `K_FF`, rigid or not.
It is placed **after** every existing refusal of the prescribed path (validation, non-zero prescribed values,
`check_rigid_body_motions`, the geometric stiffness's checks such as the pre-stress length, the count of factors), so a model
refused today by one of them keeps its error, and **before** the eigensolver.

### D2. Code

`buckling.rs`, `factors`, prescribed path, after the check of `options.factors` and before the eigensolver options:

```text
if let Ok(Some((row, ratio))) = crate::mpc::zero_shift_failing_pivot(&kr.matrix) {
    let (node, component) = dofs.node_component(free[row]).expect("free equation");
    return Err(FemError::Solver { solver: "lowest_buckling_factors", reason: format!(
        "node {node} component {component}: pivot ratio {ratio:e} of K_FF at zero shift, below 1e-8 (the rule of \
         Shift::Auto, ADR 0384): the prescribed displacements leave a motion free that is not a rigid motion of a part \
         (a mechanism); a buckling analysis needs every motion held (ADR 0402)") });
}
```

An `Err` of `zero_shift_failing_pivot` (a factorisation refused for another reason than a pivot: not symmetric, not finite) is
**not** returned by the guard: the code falls through to the eigensolver, whose identical factor refuses the same matrix with
today's error and text. The doc of `factors` and of the module say so. `mpc.rs` is not changed; the transformation path and
`prestressed_modes` are not changed.

**Why the bits of a solved model are kept** (c4): the guard reads `kr.matrix` and returns nothing on a model that passes; the
lines after it are the lines of today with the same inputs. It costs one extra factorisation of `K_FF` per call.

### D3. Which existing error texts change

Only models of the prescribed path that pass every earlier refusal and whose `K_FF` fails the rule:

1. those refused today by the eigensolver's zero-shift factor (`Solver { lowest_buckling_factors }`, reason "K + sigma M is not
   positive definite for sigma = 0e0: pivot ... of row ..."), such as HP: the **kind stays** (`Solver { lowest_buckling_factors
   }`), the reason text changes to the named one above;
2. those solved today with a pivot ratio below `1e-8` (a rounding pivot accepted): refused now (a meaningless factor becomes an
   error). No such model is expected among the tests (arithmetic 2); S1 and the logs of (c4) would show one.

No existing test asserts either text (What was read). ADR 0401's `c3_n2_o1_hinge` prints HP's result as its record O1 without
asserting it; its printed line changes (C4 below). This package retires no refusal row asserted by a test, so no existing
test is marked ignored or edited.

### D4. Files

`crates/rcfd-fem/src/buckling.rs` (D2 and the docs), new test file `crates/rcfd-fem/tests/si_buckling_prescribed_mechanism.rs`
(the hinge copied from `si_buckling_mechanism.rs`, the beams and the column from `si_geometric.rs` and
`si_knife_edge_closed_form.rs`; those files are not edited). Nothing else in `crates`.

### D5. The set-ups

- **HP**: ADR 0401's hinge without its spring: Hex8 blocks `A = [0, a]^3`, `B = [a, 2a] x [a, 2a] x [0, a]`, `a = 0.1 m`, 14
  nodes, `E = 2e11`, `nu = 0`, `rho = 7850`; `a0` (the four nodes of `A` at `x = 0`) held by `Displacement` rows in all three
  components; `Prestress::uniform([-1e6, 0, 0, 0, 0, 0])`; `BucklingOptions::new(2)`.
- **HD** (negative control): HP plus `Constraint::displacement("bfar", 1, 0.0)` (block B's far edge `x = y = 2a`, which the
  hinge rotation moves by `theta a` in `y`): still the prescribed path, the hinge held.
- **HZ** (order of refusals): HP with `a0` held in components 0 and 1 only (the parts translate freely in `z`): a rigid motion
  free.
- ADR 0390's B, tension beam, Hex8 beam, C and ADR 0394's B20, B40, copied with their pre-stresses (D7 of ADR 0390, D2 of ADR
  0394).

## Arithmetic before this registration (disclosed, by hand)

1. **HP's failing pivot.** `K_FF` of HP is the matrix ADR 0401 factored for H (H's spring acts on prescribed equations only and
   adds nothing; the free unknowns are the same 30, ascending in both reductions), so the factor is the same: refused at row 13
   with pivot `-9.5e-7` (ADR 0401 O1 and C3: H named node 8 component 1, ratio `-2.1e-16`). Expected name "node 8 component 1"
   (node 8 = `(2a, a, 0)`, a node of B off the hinge, `u_y` moves `a`, the largest value of the hinge field: meaning 1.0) and
   ratio about `-2.1e-16`. Not a limit (the identity check computes the name independently).
2. **The solved models' ratios.** For the last pivot of a leading block, `d_j = 1 / (K_j^{-1})_jj >= 1 / (K^{-1})_jj`, so the
   smallest ratio is at least `1 / (max_j a_jj x max_j (K_FF^{-1})_jj)`. The diagonal of a Hex20 node of these meshes: at most
   about `8e10 N/m` (ADR 0401 arithmetic 2). The largest flexibility: the mid-span lateral deflection under a unit load,
   `L^3 / (48 E I)` with `E I = 2e11 x 8.33e-6 = 1.67e6 N m^2`: B and B20 (`L = 2`) `1.0e-7 m/N`, B40 (`L = 4`) `8.0e-7`; C
   (clamped-free, `L = 2`) `L^3 / (3 E I) = 1.6e-6`. Smallest ratios at least `1.25e-4` (B), `1.6e-5` (B40), `7.8e-6` (C):
   above `1e-8` by more than 700. The Hex8 beam is stiffer (Hex8 in bending): at least as B. The tension beam is B. None is
   expected to be refused; the guard falls through on each.
3. **HD.** The only null direction of HP's `K_FF` is the hinge rotation; HD's row holds it (`u_y = theta a` at `bfar`), so
   `K_FF(HD)` is positive definite; its smallest pivot ratio is of the order of the Hex8 shape (no soft spring): estimate above
   `1e-3`, solved.
4. **Run time.** The six models of (c4) are those of the existing tests (each a few seconds in release); the hinge models are 14
   nodes. Estimate under 60 s in release for the new file.

## Registered checks

All in `crates/rcfd-fem/tests/si_buckling_prescribed_mechanism.rs`, release profile, run once: `CARGO_BUILD_JOBS=4 cargo test
-p rcfd-fem --release --test si_buckling_prescribed_mechanism -- --nocapture --test-threads=1`. "Refused as registered" means
`Err(FemError::Solver { solver: "lowest_buckling_factors", reason })` with `reason` starting with `"{name}: "` and containing
"pivot ratio", where **identity**: `name` is computed in the test: `prescribed_equations(m, &[])`, `assemble::stiffness(m)`,
`constraint::reduce(&k, &zero, &fixed)`, the factor `SkylineLdlt::factor_with(&kr.matrix, &SkylineOptions::default())`, the
first pivot in `order()` with `d_i / a_ii < 1e-8` (or the refused row), mapped by `kr.free()` and `DofMap::node_component`;
and **meaning**: `|z(named)| / max |z| >= 1e-3`, `z` the hinge field (`u = (-(y - a), x - a, 0)` on the nodes of B off `A`,
zero on `A`).

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | c1, c2, c3 | HP: `factors` refused as registered (identity and meaning); the smallest pivot ratio (or the refused pivot's ratio) of `K_FF` computed in the test | as stated; `< 1e-8` | refused, node 8 component 1; about `-2.1e-16` (arith. 1) |
| A2 | c3 | HP: `check_rigid_body_motions(HP, prescribed_equations(HP))` | `Ok` | `Ok` (ADR 0401 O1 reached the eigensolver) |
| A3 | c4 (i) | the FNV-1a hash of the bits of the factors and modes of B (threads 1 and 8), the tension beam, the Hex8 beam, C, B20, B40, printed by `a3_c5_bit_for_bit_hashes` on the code commit and on the scratch copy with `buckling.rs` of 3f2f0b9b | 7 hashes equal of 7 | equal (D2) |
| A4 | c4 (ii) | the active tests of `si_geometric.rs` and `si_knife_edge_closed_form.rs` run with `--nocapture --test-threads=1` on the registration commit and after the code | 0 lines differ (timing lines excepted); every test passing both times | equal |

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | HP: `free_rigid_motions(HP).count` | `== 0` | 0 (ADR 0401 C3 on H, the same rows) |
| C2 | order of refusals: HP with `BucklingOptions::new(0)` gives `InvalidValue { key: "factors" }`; HP with the pre-stress of another mesh (ADR 0390's B, the wrong length) gives `FieldLength { key: "prestress" }`; HZ gives `RigidBodyMotionFree` | each as stated | as stated |
| C3 | the six solved models of A3: each `Ok`, and the smallest pivot ratio of its `K_FF` computed in the test | `> 1e-8` | `>= 7.8e-6` (arith. 2) |
| C4 | the active tests of `si_buckling_mechanism.rs` (ADR 0401) and `si_prestress_mpc.rs` (ADR 0398) run as A4 before and after: every line equal except the line `O1 HP (prescribed path): ...` of `c3_n2_o1_hinge`, which changes from the eigensolver's text to the named refusal of A1; every test passing both times | exactly that one line differs | as stated (D3) |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | HD (the hinge held by a `Displacement` row on block B): `factors` | `Ok`, so HP's refusal is the mechanism |
| N2 | HP: the failing row of `K_FF` taken as a mesh equation (without `kr.free()`) | names another unknown than the reported one (`a0` contains node 0, so `free()[r] >= r + 3`): the identity check sees the mapping |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --no-fail-fast` | 0 failed |
| S2 | `cargo check -p rcfd-post --tests` | builds |
| S3 | `git diff 3f2f0b9b --stat -- crates`: only `buckling.rs` and the new test file | as stated |

`rcfd-api`'s `baseline_hashes` is not run: no `rcfd-api` path calls `buckling::factors` (brief: tests of `rcfd-fem` only).

### Mutations (scratch copy `w49i-work/f496-scratch/copy`, `git archive` of the code commit, own target directory)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the guard removed | A1 (the eigensolver's unnamed text) |
| Mu2 | the guard's row named as a mesh equation (`row` in place of `free[row]`) | A1 identity, N2 |
| Mu3 | `ZERO_SHIFT_PIVOT_RATIO` in `zero_shift_failing_pivot` replaced by `1e-1` (too strict) | C3, N1, A4 (solved models refused) |
| Mu4 | the guard placed before `check_rigid_body_motions` | C2 (HZ refused by the guard), the existing `c7_refusals_after_transformation_landed` |
| Mu5 | the guard placed before the check of `options.factors` | C2 (`factors = 0` on HP refused by the guard) |

## Decision rule

- **done**: A1 to A4 met, every C and S check met, the negative controls as expected, every mutation caught.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed, a negative
  control not as expected, or a mutation surviving.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- The resolution and the margin of the rule (ADR 0401's limits and its remaining questions 1 and 3) apply unchanged: a model held
  only by a very soft part could be refused; a rounding residue above `1e-8` would pass. Not checked here beyond ADR 0401.
- The name is one unknown of a free motion (the last of its support in the elimination order), not the motion.
- `prestressed_modes` on the prescribed path is not changed (its Auto shift already moves a free motion; ADR 0390).
- One extra factorisation per buckling call on the prescribed path.
- The workspace suite and `rcfd-api` are not run (the lead's gate; the brief).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The variant `FemError::Solver { solver: "lowest_buckling_factors" }` (HP's kind today and ADR 0401's), not
   `RigidBodyMotionFree` (a struct about rigid motions of a part) or `SingularStiffness`. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).
2. A factorisation error other than a pivot falls through to the eigensolver (today's error and text kept) rather than being
   returned as `Solver { "skyline-ldlt" }` as on the transformation path. Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
3. The reading of (c4) as factors and modes, measured by hashes against the base library and by the printed logs. Taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
