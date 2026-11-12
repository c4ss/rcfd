# ADR 0362: F-464 registration: the moved-mesh check of the Hex8 patch test measures the stored mesh

Status: **REGISTERED 2026-11-12, written before any code of F-464 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-464 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Roadmap item F-464 (deps F-461), from [ADR 0358](0358-f461-registration-hex8-patch-test-on-moved-mesh.md) addendum 1, remaining
question 1, option (a). Branch `f464-patch-moved-mesh-check` from main a4ca1401, worktree `w49i-work/f464`, scratch
`w49i-work/f464-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here and not changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact and the number recorded; each registered run is made once;
- tests only: one new file `crates/rcfd-fem/tests/sb_patch_moved_stored.rs`. `sb_patch_moved.rs` (ADR 0358) is not edited; no
  library file, golden or hash table is touched, so every default result keeps its bits by construction;
- no new dependency, no unsafe code; clean-room (the argument is arithmetic on coordinates, no paper is used).

## What F-464 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. In a new test file, for the moved boxes of ADR 0358 (n =
> 3, 4, 6; alpha 0.15 and 0.05; three draws), the rms of the offsets read from the stored node coordinates of the moved mesh
> against the regular builder mesh (interior nodes only, relative to h) is >= 0.3 alpha and the boundary nodes are unchanged bit
> for bit; the same test on an unmoved mesh (alpha = 0 passed through the same code) fails these checks; the existing tests are
> not edited.

Clauses: (c1) limits not stated are registered here; (c2) new test file, the 18 moved boxes of ADR 0358; (c3) rms of the
interior offsets read from the stored coordinates against the regular builder mesh, relative to h, `>= 0.3 alpha`; (c4)
boundary nodes unchanged bit for bit; (c5) the same test on an unmoved mesh (alpha = 0 through the same code) fails these
checks; (c6) existing tests not edited.

## What was read

1. `AGENT_RULES.md`; ADR 0358 in full, in particular addendum 1 (deviation of M3, remaining question 1).
2. `crates/rcfd-fem/tests/sb_patch_moved.rs` in full: V2 there takes `c.rms_offset`, a number the generator `moved` returns
   from its own draws, not read from the mesh it stores; its limit `0.3 alpha` is 0 at `alpha = 0`, so an unmoved mesh meets it.
3. `crates/rcfd-fem/src/mesh.rs`: `SolidMesh::nodes()` (stored coordinates), `blocks()`, `ElementBlock::connectivity()`;
   `builders::BOX_FACES`, `hex_box`.
4. Not read: any paper, any code outside this repository.

## Arithmetic before this registration (disclosed, by hand)

1. **Why V2 was blind.** In ADR 0358, V2 compares the generator's own rms to `0.3 alpha`. Two defects: (i) the quantity is the
   generator's, so a mesh built from the unmoved nodes after the draws would still pass; (ii) the limit scales with the
   `alpha` passed, so `alpha = 0` gives `0 >= 0`. The fix is to read the offsets from `SolidMesh::nodes()` of the mesh that is
   handed to the solver, against a freshly built `hex_box` of the same `n`, and to compare against the registered amplitude of
   the configuration, plus absolute floors that do not depend on any amplitude.
2. **Rounding of the read-back offset.** The stored coordinate is `x + d` rounded; the offset is read as `(x_stored - x) / h`.
   The rounding is at most `1.1e-16 / h` (coordinates in [0, 1]), i.e. `<= 7e-16` at `n = 6`, against offsets of order `alpha`
   (0.05): relative 1.4e-14. Negligible against the limits below.
3. **The offsets themselves.** The same generator and seed as ADR 0358 (`SEED = 0xF461_0358`, tag `1000 s + n`, 3 uniforms per
   interior node in ascending node order, `alpha h (2 r - 1)`), so the unit offsets are the ones ADR 0358 measured: rms
   0.534 alpha to 0.634 alpha (ADR 0358 addendum 1, V2), largest `<= alpha`. The read-back rms equals those to rounding, so the
   estimate for S2 is **0.53 to 0.64 alpha**; in absolute terms 0.534 x 0.05 = 0.0267 is the smallest and 0.634 x 0.15 = 0.095
   the largest.
4. **Floors without amplitude.** Absolute S2b: rms `>= 1e-3` (units of h), a factor 27 below the smallest estimate 0.0267 and far
   above rounding (1e-14). Absolute S4 (every interior node moved): for a node, the largest of three offset components
   (each `|alpha h (2r-1)|`, uniform in `[0, alpha h]` in size) is below `t` with probability `(t / (alpha h))^3`. For
   `t = 1e-5 h` and `alpha = 0.05` this is `(2e-4)^3 = 8e-12` per node; over at most 18 x 125 nodes the probability of a false
   failure is below 1e-8. The typical smallest node-maximum on one mesh with `N` interior nodes is about `alpha N^(-1/3)`: 0.5
   alpha at `N = 8`, 0.2 alpha at `N = 125`; estimate for S4 **0.1 alpha to 0.5 alpha** (5e-3 to 0.07), limit **1e-5**.
5. **Unmoved mesh.** Built with `alpha = 0`, `x + 0 = x` exactly, so the read-back offsets are exactly 0: S2 against the
   registered `alpha` (0.3 x 0.05 = 0.015 or 0.045) fails by the whole value; S2b (`>= 1e-3`) and S4 (`>= 1e-5`) fail by their
   whole value, also when the expected amplitude passed to the checker is 0 (control C2). The checks S0, S1, S3, S5 are met on
   an unmoved mesh (nothing in them depends on the move), which is registered as the sanity of the control.

## Design

### D1. The checker

`fn stored_checks(stored: &SolidMesh, n: usize, expected_alpha: f64) -> Vec<Measure>`: it builds the regular mesh itself
(`hex_box([0;3], [1;3], [n;3])`, the builder, not the generator's copy), classifies as boundary the nodes in any of the
`BOX_FACES` node sets of the regular mesh and as interior all others, and returns the measured values read from
`stored.nodes()`. No value of the generator (`max_offset`, `rms_offset`) is used. The generator `moved` (copied from ADR 0358)
takes the built amplitude; the checker takes the amplitude the configuration is registered for: for a real configuration the
two agree, in the controls they differ or are both 0.

### D2. Configurations

The 18 meshes of ADR 0358 D2: `n` in {3, 4, 6}; `alpha` in {0.15, 0.05}; draws 0, 1, 2; tag `1000 s + n`, same `SEED`.

### D3. Tests in `crates/rcfd-fem/tests/sb_patch_moved_stored.rs`

| Test | Content |
|---|---|
| `s_stored_mesh_of_the_moved_boxes` | S0 to S5 on the 18 moved meshes (checker with the registered amplitude) |
| `c1_unmoved_mesh_fails_the_stored_checks` | each of the 18 meshes built with `alpha = 0` (the same `moved` and `cube` code), checked against the registered amplitude: asserts S2, S2b, S4 report a miss and S0, S1, S3, S5 are met |
| `c2_unmoved_mesh_with_zero_expected_amplitude` | built with 0 and checked with expected 0: asserts S2b and S4 report a miss (S2 is met trivially, 0 >= 0, recorded) |

Every measured value is printed with its limit; the test fails at the end if any limit is missed.

## Checks

| Check | Clause | Quantity (read from `stored.nodes()` against the regular builder mesh) | Limit | Estimate |
|---|---|---|---|---|
| S0 | c2 | node count equal to the regular mesh; number of interior nodes | equal; `(n - 1)^3` | exact |
| S1 | c4 | number of boundary nodes (nodes of the six face sets of the regular mesh) whose coordinates differ in their bits from the regular mesh | `== 0` | 0 |
| S2 | c3 | rms of the offset components of the interior nodes / h | `>= 0.3 alpha` (alpha the registered one) | 0.53 to 0.64 alpha |
| S2b | c3, c5 | the same rms, absolute | `>= 1e-3` | 0.027 to 0.095 |
| S3 | c3 | largest offset component / h over interior nodes | `<= alpha (1 + 1e-9)` | `<= alpha`, near 0.9 alpha |
| S4 | c3, c5 | minimum over interior nodes of the largest offset component of that node / h | `>= 1e-5` | 5e-3 to 0.07 |
| S5 | c2 | connectivity of every block of the stored mesh equal to that of the regular mesh | equal | equal |
| C1 | c5 | unmoved mesh against the registered amplitude: S2, S2b, S4 report a miss; S0, S1, S3, S5 met | all 18 | by arithmetic 5 |
| C2 | c5 | unmoved mesh, expected amplitude 0: S2b and S4 report a miss | all 18 | by arithmetic 5 |

## Planned mutations

Scratch copy `w49i-work/f464-scratch/mut` of the committed test commit; each run once; the test file restored after each.

| Id | Change | Expected: caught by |
|---|---|---|
| MU1 | `cube` builds every moved mesh with built amplitude 0 (the unmoved mesh through the real test) | `s_stored_mesh_of_the_moved_boxes`: S2, S2b and S4 (this is clause c5 itself) |
| MU2 | `moved` stores the unmoved nodes but still returns the draws' statistics (the blind spot of ADR 0358 V2) | S2, S2b, S4 |
| MU3 | `moved` also moves the boundary nodes | S1 |
| MU4 | one boundary node (index 0) moved by +1e-12 in x | S1 (bit comparison) |
| MU5 | all offsets scaled by 1e-4 | S2 and S2b (S4 also expected) |
| MU6 | every second interior node left unmoved | **S4** (registered for it); S2 expected to survive on most meshes (rms falls by 1/sqrt 2 to about 0.4 alpha), may fail on some |
| MU7 | offsets scaled by 1.1 | S3 |
| MU8 | stored block connectivity rotated by one element (`rotate_left(8)`) | S5 |
| MU9 | the control C1 builds the meshes with the registered amplitude instead of 0 | C1 (its assertion that the checks miss fails) |

## Decision rule

**done**: S0 to S5 met on all 18 meshes, C1 and C2 as registered on all 18, and every mutation MU1 to MU9 caught by the check
registered for it, without an amendment after a run. **partial** (Rule I): a registered check missed, or a registered mutation
not caught by the check registered for it, or caught only after a post-run amendment. **todo**: the file does not build.

## Limits (what is not covered)

- Box `[0,1]^3` Hex8 only, the 18 meshes of ADR 0358, one seed; no check of the solves (those are ADR 0358's P1, P2, M1).
- The regular mesh is the builder's; a bug common to the builder and the checker's use of it would not be seen.
- Moves of boundary nodes are only checked as unchanged; no check that the face node sets are preserved in the stored mesh.
- The checker reads coordinates; it does not read element Jacobians (ADR 0358 V1 does).

## Addendum 1: results (2026-11-12)

Runs, each once, release profile, on the shared machine, at commit 566df5f7 (the test file as committed before any run): the
file `cargo test -p rcfd-fem --release --test sb_patch_moved_stored -- --nocapture --test-threads=1` (0.02 s, log
`w49i-work/f464-scratch/run1.log`, 3 passed); the mutations MU1 to MU9 on the archive copy `w49i-work/f464-scratch/mut` of
566df5f7 by `mutate.py` (each applied, the file run once, the file restored; logs `MU1.log` to `MU9.log`). No amendment was
made; nothing above this addendum was changed after a run. Only the new test file was added; no library file, golden or hash
table was touched and `sb_patch_moved.rs` is unedited.

### Checks

| Check | Limit | Measured (18 meshes) | Met |
|---|---|---|---|
| S0 node count equal; interior count | equal; `(n-1)^3` | 64 to 343 nodes equal; 8 to 125 interior, all equal | yes |
| S1 boundary nodes differing in bits | `== 0` | 0 on all 18 | yes |
| S2 rms interior offset / h | `>= 0.3 alpha` | 0.534 alpha to 0.634 alpha (absolute 0.0267 to 0.0951) | yes |
| S2b the same, absolute | `>= 1e-3` | 0.0267 to 0.0951 | yes |
| S3 largest offset / h | `<= alpha (1 + 1e-9)` | 0.0471 to 0.1499 (largest 0.99907 alpha) | yes |
| S4 min over nodes of the node's largest offset component / h | `>= 1e-5` | 3.92e-3 to 8.38e-2 | yes |
| S5 differing block connectivities | `== 0` | 0 on all 18 | yes |
| C1 unmoved mesh against registered alpha: S2, S2b, S4 miss; S0, S1, S3, S5 met | all 18 | S2, S2b, S4 read 0 and miss on all 18; the other four met | yes |
| C2 unmoved, expected 0: S2b and S4 miss | all 18 | both read 0 and miss on all 18; S2 met trivially (0 >= 0) | yes |

Estimates against measured: S2 0.534 to 0.634 alpha, as in ADR 0358 V2 (estimate 0.53 to 0.64); S2b inside; S4 3.9e-3 against
the estimated 5e-3 to 0.07 (just below the estimated range, 390 times above the limit).

### Mutations (scratch copy of 566df5f7, each run once)

| Id | Result | Caught by (misses in `s_stored_mesh_of_the_moved_boxes`, 18 meshes each) |
|---|---|---|
| MU1 | caught as registered | S2, S2b, S4 on all 18 meshes (clause c5 through the real test) |
| MU2 | caught as registered | S2, S2b, S4 on all 18 |
| MU3 | caught as registered | S1 on all 18 (only S1) |
| MU4 | caught as registered | S1 on all 18 (only S1; the controls C1, C2 also fail because they share the code) |
| MU5 | caught as registered | S2, S2b and S4 on all 18 |
| MU6 | caught as registered by S4 | S4 on all 18; S2 survived on all 18 (rms 0.346 to 0.419 alpha, above 0.3 alpha), which is the registered expectation of survival |
| MU7 | caught as registered | S3 on all 18 (only S3) |
| MU8 | caught as registered | S5 on all 18 (only S5; the controls also fail) |
| MU9 | caught as registered | C1 fails (its assertion): S2, S2b, S4 met on a moved mesh |

### Deviations and what was not run

- None in the checks or mutations. MU4 and MU8 also fail the two control tests, because the controls use the same builder code;
  not a deviation, recorded.
- The probe of arithmetic 4 for S4 was too optimistic at its low end (3.9e-3 against 5e-3); the limit 1e-5 is untouched.
- Not run: tetrahedra, moves of boundary nodes, other seeds or amplitudes, solves on the moved meshes (those are ADR 0358).
- `cargo fmt -p rcfd-fem --check` passed; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` passed. The whole test file
  was the only test run (new tests; no line of an existing file changed).

### Decision by the registered rule

**done**: S0 to S5 met on all 18 meshes, C1 and C2 as registered on all 18, and MU1 to MU9 caught by the check registered for
each, without an amendment.

### Remaining questions

1. **The solves of ADR 0358 do not consume this check.** `sb_patch_moved.rs` still uses its own V2; the stored-mesh check is a
   separate file and does not prove the P1/P2 solves ran on the meshes it checks (same generator, same seed, same code, but not
   the same objects). Options: (a) leave (the two files copy the same `moved` and tag); (b) a follow-up that runs one solve
   inside the new file on the checked mesh object. Recommendation: (a); the first-order evidence already exists (M1 against M6
   of ADR 0358).
2. **F-461 / F-459 notes.** ADR 0358 remaining question 1 is closed by this item; recommendation: one sentence in the F-461
   note.

### For the lead: roadmap entries

Proposed status of F-464: done pending acceptance.

> F-464 (ADR 0362, 2026-11-12): done pending acceptance. New test file sb_patch_moved_stored.rs reads the node coordinates stored in the 18 moved Hex8 boxes of ADR 0358 against the regular builder mesh: interior rms offset 0.534 to 0.634 alpha (limit 0.3 alpha, and an absolute 1e-3), every interior node moved (min node maximum 3.9e-3 h against 1e-5), largest offset at most alpha, boundary nodes unchanged bit for bit (0 differing), connectivity unchanged. The same code on an unmoved mesh (alpha = 0) fails the rms and node-moved checks on all 18 meshes, also with the expected amplitude set to 0 (absolute floors). Nine mutations (unmoved mesh through the real test, stored nodes unmoved, boundary moved, one boundary node moved by 1e-12, offsets scaled 1e-4 and 1.1, every second node unmoved, connectivity rotated, control built moved) each caught by the check registered for it. Tests only, ADR 0358 tests unedited.

> F-461 note addition: the remaining question 1 of ADR 0358 (V2 cannot see an unmoved mesh) is closed by F-464 (ADR 0362, 2026-11-12).
