# ADR 0358: F-461 registration: the constant-strain patch test of Hex8 on a moved mesh

Status: **REGISTERED 2026-11-12, written before any code of F-461 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-461 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script, no prototype and no library
code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-461 (deps F-459), follow-up 1 of [ADR 0355](0355-f459-registration-manufactured-solution-on-distorted-meshes.md)
(its remaining question 1, option (b)). Branch `f461-hex8-moved-patch` from main 4745b90c, worktree `w49i-work/f461`, scratch
`w49i-work/f461-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run; each registered run is made once;
- tests only: one new file `crates/rcfd-fem/tests/sb_patch_moved.rs`; `sb_mms_distorted.rs` and every other test are not
  edited (the node-moving code, the field type, the error rule and the error norms are copied from it); no library file, no
  shared solver file, no golden, no hash table is touched, so every default result keeps its bits by construction; a library
  defect found is reported, not patched;
- no new dependency, no unsafe code, the crate graph stays acyclic;
- clean-room: the argument is derived below from the isoparametric map and the degree of the stiffness integrand; the
  validity bound is the one derived in ADR 0355 (Gershgorin and Ostrowski). No paper is used.

## What F-461 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. A linear displacement field on a box of Hex8 whose
> interior nodes are moved by a registered seed and an amplitude below the validity bound of ADR 0355, all boundary nodes
> prescribed, no body force: the relative L2 and energy errors are <= 1e-9, and the same test with the stiffness rule mutated to
> one point gives a relative energy error >= 1e-5.

Clauses: (c1) a linear displacement field; (c2) a box of Hex8; (c3) interior nodes moved by a registered seed and an amplitude
below the validity bound of ADR 0355 (`alpha < 1/6`, element valid at every node); (c4) all boundary nodes prescribed; (c5) no
body force; (c6) relative L2 error `<= 1e-9`; (c7) relative energy error `<= 1e-9`; (c8) with the Hex8 stiffness rule mutated to
one point the relative energy error is `>= 1e-5`; (c9) every other limit registered here.

## What was read (main 4745b90c, read only)

1. `AGENT_RULES.md`; ADR 0355 in full (registration, addendum 1, its arithmetic 3 for the validity bound, arithmetic 5 for the
   patch-test argument of the one-point Hex8, its check X3, its remaining question 1 which is this item).
2. `crates/rcfd-fem/tests/sb_mms_distorted.rs` in full (the node-moving code `moved`, the field type `Poly`, `error_rule`,
   `solve_case_d`, `errors`, `Checks`, the SplitMix64 `Rng`).
3. `crates/rcfd-fem/src/element.rs`: the Hex8 stiffness rule is `quadrature::gauss_2()` at line 310 (mass rule `gauss_3` at
   line 313); `quadrature.rs`: `gauss_1`, `gauss_2`, `gauss_3` exist.
4. **Not read:** any paper; any finite-element code outside this repository.

## Arithmetic before this registration (disclosed, by hand)

1. **Why the full rule passes (derivation of ADR 0355 arithmetic 5, restated).** For a constant stress `sigma` the internal
   force at node `a` of an element is `F_a = sigma int cof(J) grad_xi N_a dxi`. With the trilinear map every column of
   `cof(J)` has degree at most 2 per reference variable and `dN_a/dxi` has degree at most 1 in the other two, so each
   integrand has degree at most 2 per variable: the 2 x 2 x 2 rule (exact to degree 3) integrates it exactly. The nodal forces
   of the assembled element sum, at an interior node, to the force of a constant stress on a closed patch, which is zero
   (the element forces of a constant stress `sigma` over a closed surface cancel exactly, `int sigma n dS = 0`). A field whose
   interpolation is exact for a linear field (the map and the field use the same shape functions, so `u = G x + a` is
   interpolated exactly) has constant `sigma` and therefore zero residual: the discrete solution with the boundary values of
   the linear field is that field up to the rounding of the solve. This is the patch test of the isoparametric element on
   any valid mesh. The one-point rule does not integrate the `eta^2`-type term (ADR 0355 arithmetic 5) and leaves a nodal
   force of order `sigma h^2 alpha` per element, random in sign.
2. **Rounding estimate.** The solver works in double precision on a stiffness matrix of condition number growing like `n^2`
   (n up to 6) and with moved nodes modestly worse; relative errors of the solution 1e-15 to 1e-13. ADR 0355 measured
   1.8e-16 to 4.0e-16 (L2) and 1.1e-15 to 3.9e-15 (energy) for the same kind of test on a Hex8 mesh with `n = 4`, `alpha =
   0.15`. Estimate 1e-16 to 1e-13; limit **1e-9** (c6, c7), four to five orders above the estimate.
3. **The one-point rule's error.** ADR 0355 measured, on `n = 4`, `alpha = 0.15`, relative L2 1.9e-4 and relative energy
   1.7e-3 for the linear field. The error is first order in the distortion (arithmetic 1: nodal force error `~ sigma h^2
   alpha`), so the estimate at `alpha = 0.05` is a third of that: energy about 5.7e-4, L2 about 6e-5; at the coarsest mesh
   `n = 3` the number of random contributions is smaller (8 interior nodes), the relative spread of the error larger, say a
   factor 2 either way. Estimate for every configuration below: relative energy error of the mutated element **2e-4 to 5e-3**,
   limit **1e-5** (c8), a factor of about 20 below the lowest estimate. The mutated test also fails (c6, c7) in every
   configuration.
4. **Validity (c3).** From ADR 0355 arithmetic 3: with uniform offsets in `[-alpha h, alpha h]` per component the Hex8 corner
   Jacobian satisfies `det J / det J_regular >= (1 - 6 alpha)^3` for `alpha < 1/6`: 1.0e-3 at `alpha = 0.15`, 0.343 at `alpha =
   0.05`. ADR 0355 measured minima 0.37 to 0.62 at `alpha = 0.15` for the seed there; estimate here 0.3 to 0.7 (`alpha =
   0.15`) and 0.8 to 0.95 (`alpha = 0.05`). Limit: the derived bound `(1 - 6 alpha)^3` (strictly the check that the code does
   what is registered; also enforced by `add_block`, which refuses `det J <= 0`).
5. **That the mesh is really moved.** Each offset component is uniform on `[-alpha h, alpha h]`, rms `alpha h / sqrt(3) =
   0.577 alpha h`. For `n = 3` there are 8 interior nodes, 24 samples, the relative spread of an rms of 24 uniform samples is
   about `1/sqrt(2 x 24)` times 0.9, i.e. 0.14 to 0.2; the lowest plausible value is 0.577 x 0.6 = 0.35 alpha. Limit for the
   rms of the offset components of the interior nodes (relative to `h`): **>= 0.3 alpha**.
6. **Negative control (field not in the element space).** The quadratic field of ADR 0279's X3 (`Poly::quadratic`, constant
   body force `-div sigma`, loaded by `body_force_loads`) on the same moved meshes is not reproduced by the trilinear element:
   the interpolation error is `~ h^2 |Q|` in L2 against `|u| ~ 0.4`, and `~ h |Q|` relative `|G|` in energy. With `|Q| ~ 0.2`,
   `|G| ~ 0.5`, `h = 1/3 .. 1/6`: relative energy error 0.02 to 0.2, L2 0.003 to 0.03. Limit **>= 1e-3** for both norms, in
   every configuration (a factor 3 or more below the lowest estimate), showing that the 1e-9 above is not the effect of a
   test that cannot see an error.

## Design

### D1. Copied code (from `sb_mms_distorted.rs`, unchanged in substance)

`Rng` (SplitMix64), `Checks`, `lame`, `d_matrix`, `voigt_strain`, `body_force`, the `Field` trait, `Poly` (the linear field
and the quadratic field; a second linear field is added, below), `gauss_legendre`, `error_rule` (Hex8 part), `moved`, `model_of`,
`solve_case_d` (case D: every node of the six face node sets prescribed at the exact field, body force by `body_force_loads`,
`solve_static_with` with `StaticOptions::default()`), `errors`. Trig, the tetrahedra, Tet10 and the order study are not copied.
The seed constant is this ADR's: `SEED = 0xF461_0358`.

### D2. Configurations (registered now, fixed)

- Elements: Hex8 only, `hex_box([0;3], [1;3], [n;3])`, box `[0, 1]^3`.
- Cells per edge `n` in {3, 4, 6}; amplitude `alpha` in {0.15, 0.05} (both below 1/6; 0.15 is the amplitude of ADR 0355 for
  Hex8, 0.05 a mild distortion); `s` in {0, 1, 2} draws; the generator is `Rng::new(1000 s + n)` (tag `1000 s + n`, the same
  for both amplitudes of a pair, so the unit offsets are identical and only the scale differs), the nodes in ascending index,
  the interior nodes (in no face node set) moved by `alpha h (2 r - 1)` in each component, three uniforms per interior node.
  Boundary nodes are not moved: the domain stays the unit cube.
- Fields (both linear, `b = 0` exactly because the second derivatives are zero): **A**, `Poly::linear()` of ADR 0355 (`a =
  (0.1, -0.2, 0.3)`, `G = [[0.5, 0.2, -0.1], [0.3, -0.4, 0.25], [-0.15, 0.35, 0.6]]`); **B**, `a = (-0.05, 0.4, 0.1)`, `G =
  [[-0.3, 0.6, 0.2], [0.1, 0.45, -0.5], [0.7, -0.25, 0.15]]` (a different strain state, so that no cancellation special to
  one field is mistaken for success).
- 3 `n` x 2 `alpha` x 3 draws x 2 fields = **36 solves** (largest: `n = 6`, 125 interior nodes, 375 free unknowns): seconds.
- Material `E = 1`, `nu = 0.3`; the error norms of ADR 0355 (125-point rule per element, the exact field evaluated at the
  physical point of the moved element); the relative errors are the L2 and energy norms of the error divided by those of the
  exact field.

### D3. Tests in `crates/rcfd-fem/tests/sb_patch_moved.rs`

| Test | Content | Ignored |
|---|---|---|
| `v1_moved_boxes_valid_and_really_moved` | V1 (validity), V2 (rms offset) on the 18 meshes | no |
| `p1_constant_strain_patch_test_on_moved_hex8` | P1, P2 on the 36 solves; prints min and max of every measured error | no |
| `n1_quadratic_field_not_reproduced_on_moved_hex8` | N1 on the 18 meshes with the quadratic field | no |

Every measured value is printed with its limit; a test fails at its end if any of its limits is missed.

## Checks

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| P1 | c1 to c6 | relative L2 error of the solution, each of the 36 solves | `<= 1e-9` | 1e-16 to 1e-13 |
| P2 | c7 | relative energy error, each of the 36 solves | `<= 1e-9` | 1e-16 to 1e-13 |
| V1 | c3 | per mesh: min over elements and nodes of `det J / (h^3/8)`; largest offset / h | `>= (1 - 6 alpha)^3` (1.0e-3 at 0.15, 0.343 at 0.05); offset `<= alpha` | 0.3 to 0.7; 0.8 to 0.95; `alpha` |
| V2 | c3 | per mesh: rms of the offset components of the interior nodes / h | `>= 0.3 alpha` | 0.45 to 0.7 alpha |
| M1 | c8 | with the Hex8 stiffness rule `gauss_2` replaced by `gauss_1` (library mutation, below), the relative energy error of every one of the 36 solves | `>= 1e-5` | 2e-4 to 5e-3 |
| N1 | c9 | quadratic field with its body force, same 18 meshes: relative L2 and energy errors | `>= 1e-3` both | L2 0.003 to 0.03, energy 0.02 to 0.2 |

(The check M1 is read from the printed output of the mutated run: under the mutation P1 and P2 fail by their `<= 1e-9`
limits, and the printed relative energy errors are the measured quantity of M1. The unmutated tests do not contain M1; the
one-point rule is a library change, not a test option.) Wall times are recorded, not limited.

## Planned mutations (scratch copy `w49i-work/f461-scratch/mut` of the committed test commit; each run once, the fast tests
only; the file restored after each)

| Id | Where | Change | Expected |
|---|---|---|---|
| M1 | library `element.rs` line 310 | Hex8 stiffness rule `gauss_2` -> `gauss_1` | **caught** by P1 and P2 (clause c8); printed relative energy error >= 1e-5 in all 36 solves; N1 unchanged (still >= 1e-3) |
| M2 | library `element.rs` line 310 | Hex8 stiffness rule `gauss_2` -> `gauss_3` (over-integration) | **survives**: both rules are exact for the integrand (arithmetic 1); P1, P2, V1, V2, N1 met |
| M3 | test | `ALPHA` set to 0 for both amplitudes (mesh not moved) | caught by V2 (rms offset 0 < 0.3 alpha); P1 and P2 still met (they hold on the regular mesh too) |
| M4 | test | amplitude 0.15 -> 0.4 (above the bound 1/6) | caught by V1 (`add_block` refuses an inverted element, which counts as caught by V1, or the minimum is below the bound) |
| M5 | test | the prescribed value of one boundary node, component 0, raised by 1e-6 | caught by P1 and P2 (relative error of order 1e-6/|u| in the neighbourhood, above 1e-9) |
| M6 | library `element.rs` line 310 plus test | `gauss_1` and `ALPHA` 0 (the one-point rule on a regular mesh) | P1, P2 met (the rule passes the patch test on parallelepipeds, ADR 0279 M5/ADR 0355), V2 fails: shows the M1 error comes from the moves |
| M7 | test | the quadratic field replaced by the linear field A in N1 | caught by N1 (errors rounding level < 1e-3): shows N1 can fail |

M1 and M2 are library lines on the scratch copy only; the branch contains no library change.

## Decision rule

**done**: P1, P2, V1, V2, N1 all met on every configuration, and M1 caught with the printed relative energy error `>= 1e-5` in
every one of the 36 solves, without an amendment after a run. M2 surviving and M3 to M7 caught as registered are expected;
a deviation among M2 to M7 is recorded and diagnosed and does not alone block done unless its diagnosis shows the acceptance
checks rest on a defect. **partial** (Rule I): a missed registered check among P1, P2, V1, V2, N1, M1; M1 caught only after a
post-run amendment. **todo**: the test file does not build.

## Limits (what is not covered)

- Hex8 only on the unit cube; no Tet4/Tet10 patch test here (the tetrahedra are affine; ADR 0355 X3 covers them).
- Linear fields with constant strain; the rigid-body and the constant-strain-per-component fields are special cases of the two
  fields A and B; the quadratic field is only the negative control.
- Interior nodes only; the boundary nodes stay on the faces (a mesh with moved boundary nodes is not tested).
- Random uniform offsets of bounded amplitude, three seeds, two amplitudes, three meshes: not a proof for every mesh; the
  derivation in arithmetic 1 is the general argument.
- The one-point rule is run as a library mutation on a scratch copy; reduced-integration with hourglass control is not part of
  the library and not tested.
- One material, `nu = 0.3`.

## The roadmap entry, as it stood before this registration

F-461, title "Element tests: the constant-strain patch test of Hex8 on a moved mesh", deps F-459; the acceptance quoted under
"What F-461 asks".

## Addendum 1: results (2026-11-12)

Runs, each once, release profile, on the shared machine, at commit 358152f1 (the test file as committed before any run): the
file `cargo test -p rcfd-fem --release --test sb_patch_moved -- --nocapture --test-threads=1` (0.28 s, log
`w49i-work/f461-scratch/run1.log`); the mutations M1 to M7 on the archive copy `w49i-work/f461-scratch/mut` of 358152f1 by
`runmut.sh` (each applied, the file run once, the file restored; logs `M1.log` to `M7.log`). One correction of the test file was
made before the first run and is in the committed file: the copied body-force helper indexes the Hessian as `q[k][i][k]` (as in
ADR 0355); it is zero for the linear fields, so it matters only for N1. No amendment was needed; nothing above this addendum
was changed after a run.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| P1 relative L2 error, 36 solves (n 3, 4, 6; alpha 0.15, 0.05; 3 draws; fields A, B) | `<= 1e-9` | maximum 2.96e-16 | yes |
| P2 relative energy error, same 36 solves | `<= 1e-9` | minimum 5.5e-16, maximum 2.36e-15 | yes |
| V1 min `det J / regular`, 18 meshes | `>= (1 - 6 alpha)^3` (1.0e-3, 0.343) | alpha 0.15: 0.389 to 0.69; alpha 0.05: 0.782 to 0.95 | yes |
| V1 largest offset / h | `<= alpha` | met on all 18 | yes |
| V2 rms interior offset component / h | `>= 0.3 alpha` | 0.534 alpha to 0.634 alpha | yes |
| N1 quadratic field, relative L2 / energy, 18 meshes | `>= 1e-3` | L2 2.26e-3 to 9.14e-3; energy 3.30e-2 to 6.69e-2 | yes |
| M1 one-point Hex8 rule (library mutation), relative energy error, 36 solves | `>= 1e-5` | 6.05e-5 to 6.71e-3 (alpha 0.15: 5.45e-4 to 6.71e-3; alpha 0.05: 6.05e-5 to 7.36e-4) | yes |

Estimates against measured: P1/P2 inside the estimate; V1 inside; V2 0.53 to 0.63 alpha against 0.45 to 0.7; N1 L2 low end
2.3e-3 against estimated 3e-3 to 3e-2 (just below the estimated range, 2.3 times above the limit); M1 low end **6.05e-5 against
the estimated 2e-4 to 5e-3** (below the estimated range at the smallest amplitude and coarsest mesh, field B, 6 times above the
limit; the spread of the error with the random draw was underestimated for `n = 3`). The one-point relative L2 error was
1.09e-5 to 4.24e-4 (no limit).

### Mutations (scratch copy of 358152f1, each run once)

| Id | Change | Result | Caught by |
|---|---|---|---|
| M1 | Hex8 stiffness `gauss_2` -> `gauss_1` | **caught** | P1 and P2 fail in all 36 solves; measured relative energy error 6.05e-5 to 6.71e-3 (clause c8, all `>= 1e-5`); V1, V2, N1 unaffected |
| M2 | Hex8 stiffness `gauss_2` -> `gauss_3` | survived (as registered) | all tests pass (P1 about 2e-16) |
| M3 | `ALPHAS` = 0 (mesh not moved) | caught, **but not by V2 as registered** | V1 at `alpha = 0` fails by rounding (`min det J / regular` prints 1.0000 against a bound computed as 1.0, misses at `n = 6`); V2's limit `0.3 alpha` is 0 at `alpha = 0`, so V2 is met (0 >= 0). P1, P2, N1 met |
| M4 | amplitude 0.15 -> 0.4 | caught | `add_block` refuses the inverted element (unwrap panic at the mesh build, all three tests) |
| M5 | prescribed value of node 0, component 0, +1e-6 | caught | P1 (1.5e-8 to 5.3e-8) and P2 (1.8e-7 to 2.8e-7) fail in all 36 solves |
| M6 | `gauss_1` and `ALPHAS` = 0 | as registered in the P checks | P1, P2 met (about 2e-16): the one-point rule passes the patch test on a regular mesh, so the M1 error comes from the moves; the run still fails V1 by the same rounding as M3 |
| M7 | N1 field replaced by the linear field A | caught | N1 fails (relative error 1.7e-16 against `>= 1e-3`) |

### Deviations and what was not run

- **M3 deviates from its registration.** It was registered as caught by V2 (rms offset 0 < 0.3 alpha). V2's limit is relative to
  `alpha`, so at `alpha = 0` it is 0 and cannot fail; the run is failed only by a floating-point artifact of V1 at `alpha = 0`
  (not a real detection). Diagnosis: a flaw of V2 as written (an absolute floor would have been needed, and V2 measures the
  generator's draws, not the stored mesh), not of the acceptance checks P1, P2 (c6, c7) or M1 (c8). That the real runs use
  moved meshes is shown independently by M1 versus M6: the same mutated rule fails by 6e-5 to 7e-3 on the moved meshes and
  passes at 2e-16 on the regular ones. No amendment was made; a corrected V2 is a follow-up (below).
- The M1 low end (6.05e-5) is below its estimated range (arithmetic 3); the limit was met with a factor 6.
- Not run: tetrahedra, boundary-node moves, other seeds or amplitudes, a box that is not the unit cube.
- No library file was edited; no golden or hash table touched; the files of the branch are the ADR and the new test file.
- `cargo fmt -p rcfd-fem --check` passed; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` passed.

### Decision by the registered rule

**done**: P1, P2, V1, V2, N1 met on every configuration, and M1 caught with the relative energy error `>= 1e-5` in all 36
solves, without an amendment. M2 survived and M4, M5, M7 were caught as registered; M3 and M6 deviate (V2 cannot detect an
unmoved mesh, recorded and diagnosed above), which by the rule does not alone block done.

### Remaining questions

1. **V2's relative limit cannot see an unmoved mesh (M3).** Options: (a) a follow-up that adds an absolute floor and measures
   the stored mesh nodes instead of the draws (a new test; the registered limits stay); (b) leave it, since M1 versus M6
   already show the meshes are moved. Recommendation: (a), small, as a follow-up item, not applied here.
2. **The mutation is a scratch-copy library edit, not an in-test switch.** The one-point rule cannot be selected from a test
   without a library change. Options: (a) leave (the record is in this ADR); (b) an in-test assembler of the Hex8 stiffness with
   a selectable rule through `Element::point`. Recommendation: (a).
3. **F-459's note** can say that its remaining question 1(b) is closed by this item; recommendation: yes, one sentence.

### For the lead: roadmap entries

F-461: proposed status **done**. Note:

> Structural solver, basic tier. ADR 0358, 2026-11-12: the constant-strain patch test of Hex8 on a unit box with interior nodes moved by seed 0xF461_0358, uniform offsets of at most 0.15 h and 0.05 h per component (validity bound 1/6 of ADR 0355; minimum det J at the nodes 0.389 to 0.95 of the regular value), n = 3, 4, 6 cells per edge, three draws, two linear fields, all boundary nodes prescribed, no body force; tests crates/rcfd-fem/tests/sb_patch_moved.rs only. Over the 36 solves the relative L2 error is at most 3.0e-16 and the relative energy error at most 2.4e-15 (limit 1e-9); the quadratic field is not reproduced (L2 2.3e-3 to 9.1e-3, energy 3.3e-2 to 6.7e-2, so the test can see an error). With the Hex8 stiffness rule mutated to one point (scratch copy) the relative energy error is 6.1e-5 to 6.7e-3 in every solve (limit 1e-5) and the relative L2 error 1.1e-5 to 4.2e-4; the same one-point rule on an unmoved mesh passes at 2e-16, so the error comes from the moves. The 3-point over-integration survives as expected; a 1e-6 offset of one prescribed value and an amplitude above the bound are caught. One check is weak: the check that the mesh is moved has a limit relative to the amplitude and cannot see an unmoved mesh (that mutation was caught only by a rounding artifact of the validity check); the move itself is evidenced by the contrast between the one-point rule on the moved and on the regular mesh. Follow-up proposed below.

F-459: suggested addition to its note (one sentence at the end):

> The moved-mesh patch test of Hex8 that detects the one-point stiffness directly is ADR 0358 (F-461).

Follow-up item (id by the lead):

1. Title: "Element tests: the moved-mesh check of the Hex8 patch test measures the stored mesh". Deps: F-461.

> Every limit not stated here is registered in the ADR before the run. In a new test file, for the moved boxes of ADR 0358 (n = 3, 4, 6; alpha 0.15 and 0.05; three draws), the rms of the offsets read from the stored node coordinates of the moved mesh against the regular builder mesh (interior nodes only, relative to h) is >= 0.3 alpha and the boundary nodes are unchanged bit for bit; the same test on an unmoved mesh (alpha = 0 passed through the same code) fails these checks; the existing tests are not edited.
