# ADR 0441: F-523 registration: deck pressure loads on non-parallelogram faces

Status: **REGISTERED 2026-11-14, written before any code of F-523 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-523 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done with one disclosed python script (no library
code run). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0436](0436-f384-registration-one-way-deck-loads.md) (F-384; its remaining question 1, option (a), and its MU4 not
caught). Roadmap item F-523 (depends on F-384). Size XS, test-only expected. Branch `f523-deck-nonparallelogram` from main
65d82f02, worktree `w49i-work/f523`, scratch `w49i-work/f523-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical: no library
file is edited (test-only), no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file, no existing
test edited, no new dependency, no unsafe code; clean-room (first principles, derived below).

## What F-523 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. On a trapezoidal and on a warped Hex8 face the nodal
> loads of deck_pressure_loads equal those of Load::pressure node by node within 1e-14 relative, and the lumped-load mutation
> is caught.

Clauses: (c0) every limit not stated is registered here, before the run; (c1) a trapezoidal Hex8 face: nodal loads of
`deck_pressure_loads` equal those of `Load::pressure` node by node within 1e-14 relative; (c2) the same on a warped Hex8 face;
(c3) the lumped-load mutation (MU4 of ADR 0436) is caught.

"Relative" is registered precisely: for a face set, `e = max_n |f_deck,n - f_ref,n| / max_n |f_ref,n|`, where `f_deck,n` and
`f_ref,n` are the 3-vectors of node `n` (all mesh nodes; nodes off the face carry zero in both), `|.|` the Euclidean norm, and
the maximum in the denominator runs over the nodes of the reference. Node by node: the numerator is the largest single node's
difference, so no node hides in an average.

## What was read (main 65d82f02, read only)

ADR 0436 (all: D3, K3, N2, N2a, MU4 and its diagnosis, remaining question 1), `crates/rcfd-post/src/solid_loads.rs` (all:
`deck_pressure_loads`, `face_pressure_load`), `crates/rcfd-post/tests/sb_deck_loads.rs` (all: `Checks`, K3,
`n2a_warped_asymmetric_face`, `resultant_errors`), `crates/rcfd-fem/src/loads.rs` (`nodal_load_vector`, signature and the
`Load::pressure` route). Scaffold rows: none for F-523.

## Arithmetic before this registration (disclosed)

Script `w49i-work/f523-scratch/pre_arith.py` (numpy, 3 x 3 Gauss on the bilinear face, no repository code). For a face with
corner positions `x_a`, the consistent load is `f_a = -p int N_a (x_u x x_v) du dv` and the lumped load is `-p A / 4` at every
corner, `A = (x_2 - x_0) x (x_3 - x_1) / 2`. Measure `max_n |f_cons - f_lump| / max_n |f_cons|`:

| Face (corners in the cycle (0,0), (1,0), (1,1), (0,1) of the face) | difference |
| --- | --- |
| parallelogram `(0,0,1), (1,0,1), (1.3,1,1), (0.3,1,1)` (control: what ADR 0436's checks saw) | 1.5e-16 |
| trapezoid `(0,0,1), (1,0,1), (0.75,1,1), (0.25,1,1)` | 0.1000 |
| warped `(0,0,1), (1,0,1), (1.3,1,1.01), (0,1,1)` | 0.0417 |

So on the two registered faces the lumped and the consistent loads differ by 4e-2 to 1e-1, about 12 orders above the
registered limit 1e-14: the mutation is visible if the check compares node by node. On the parallelogram the difference is
rounding, which is why K3 of ADR 0436 missed MU4. The warped face is the asymmetric corner of N2a (the N2 lesson of ADR 0436:
a warp symmetric about a diagonal makes the centroid formula exact); the table shows that for this face the lumped and the
consistent loads differ, so it discriminates.

## Design

Test-only: a new file `crates/rcfd-post/tests/sb_deck_loads_np.rs` (the existing test file is not edited). No library change is
expected: `deck_pressure_loads` and `Load::pressure` use the same arithmetic (`face_rule`, `face_shape_functions`), so identity
is expected; a library change would be a finding.

Faces (a single Hex8 `hex_box([0, 0, 1], [1, 1, 0.2], [1, 1, 1])` with nodes of the `zmin` face moved; the `zmin` surface set
copied to the new mesh as in N2a; the top face stays):

- **trapezoid T:** nodes `(0,1,1) -> (0.25,1,1)` and `(1,1,1) -> (0.75,1,1)`: a planar isosceles trapezoid, parallel sides 1
  and 0.5;
- **warped W:** node `(1,1,1) -> (1.3,1,1.01)` (the N2a face, asymmetric);
- **strip S:** `hex_box([0, 0, 1], [1, 1, 0.2], [2, 1, 1])`, the three nodes at `y = 1` of the `zmin` face moved from `x = 0,
  0.5, 1` to `x = 0.25, 0.5, 0.75`: two trapezoids sharing the nodes of the edge `x = 0.5` (checks the accumulation at shared
  nodes).

Route: `shell_from_surface_set(mesh, "zmin", Plane { axes: [0,1] })`, `shell_deck(shell, &[P0; n_quads], 1)`, text,
`Deck::parse`, `deck_pressure_loads(mesh, "zmin", deck, 1, 1e-9)`; `P0 = 1e5`. Reference: `nodal_load_vector` of a `SolidModel`
with `Load::pressure("zmin", P0)` (no constraint, no material: a surface pressure needs neither); deck side: `nodal_load_vector`
of a model without the load and the forces as `extra` (as K3 of ADR 0436). Per-node 3-vectors by `dof_map().node_component`.

## Registered checks

All in `crates/rcfd-post/tests/sb_deck_loads_np.rs`, release, run once (`cargo test -p rcfd-post --release --test
sb_deck_loads_np -- --nocapture --test-threads=1`).

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| B1 | c1 | face T: `e` (definition above) | <= 1e-14 | 0 (same arithmetic) |
| B2 | c2 | face W: `e` | <= 1e-14 | 0 |
| B3 | c1 | strip S: `e` (shared nodes accumulate) | <= 1e-14 | 0 to 2e-16 (summation order) |
| D1 | c0, c3 | face T: `max_n abs(f_ref - f_lump) / max_n abs(f_ref)`, lumped `-p A/4` computed in the test from the deck's corner coordinates | > 1e-2 | 0.100 |
| D2 | c0, c3 | face W: the same | > 1e-2 | 0.0417 |
| D3 | c0, c3 | strip S: the same | > 1e-2 | about 0.1 (not computed beforehand; a check, not an acceptance) |
| R1 | | face T: `abs(F_n - F_d) / S_d` and `abs(M_n - M_d) / (S_d L_d)` against `deck_force_moment` (planar face: the moment is exact) | each <= 1e-12 | 1e-16 |

D1 to D3 are the negative control in the run: the same comparison made against the lumped loads must fail by a wide margin,
which shows the faces and the measure can see MU4.

### Mutations (scratch copy `w49i-work/f523-scratch/tree` from `git archive` of the final commit, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| MU4 | `face_pressure_load` returns the lumped `-p A/4` at each corner | B1, B2, B3 (expected 0.1, 0.042, about 0.1) |
| MU1 | `face_pressure_load` with `+p` instead of `-p` (sanity: the check sees the sign) | B1, B2, B3, R1 |

## Decision rule

- **done**: B1, B2, B3, D1 to D3, R1 met; MU4 caught by each of B1, B2 and B3; no library file changed.
- **partial**: a registered check missed (kept as a `*_known_miss` record with the limit intact and the measured number), or a
  mutation not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- Hex8 4-node faces only; uniform pressures only (as ADR 0436). The equality is against `Load::pressure` of the same crate, not
  an independent quadrature: the independent value is ADR 0436's D3 derivation and D1 to D3's consistent-versus-lumped
  difference. A common error of the two routes would not be seen here.
- The resultant moment on the warped face is not checked here (it differs from `deck_force_moment` by design: ADR 0436 N2a).
- No hash table, golden file or default is touched.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. "Relative" read as relative to the largest nodal load magnitude of the reference (definition above): taken; a per-node ratio
   is undefined at nodes with zero load and meaningless where one component is zero.
2. A new test file rather than an edit of `sb_deck_loads.rs` (existing tests are not edited): taken.

## Addendum 1: results (2026-11-14)

The registered run: commit of the test file (after the registration commit 97e97162), once, release, `cargo test -p rcfd-post
--release --test sb_deck_loads_np -- --nocapture --test-threads=1` (log `w49i-work/f523-scratch/run1.log`): 1 test passed.
Afterwards only `rustfmt` was applied to the test file (line wrapping, no change of any number or limit). No library file was
changed (test-only, as expected).

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| B1 trapezoid, deck vs `Load::pressure`, node by node | <= 1e-14 | 0 (bit-identical) | yes |
| B2 warped, same | <= 1e-14 | 0 (bit-identical) | yes |
| B3 strip of two trapezoids (shared nodes), same | <= 1e-14 | 0 (bit-identical) | yes |
| D1 trapezoid, lumped vs `Load::pressure` | > 1e-2 | 1.0000e-1 (estimate 0.100) | yes |
| D2 warped, lumped | > 1e-2 | 4.1712e-2 (estimate 0.0417) | yes |
| D3 strip, lumped | > 1e-2 | 1.0000e-1 (estimate about 0.1) | yes |
| R1 trapezoid `abs(F_n - F_d)/S_d`; `abs(M_n - M_d)/(S_d L_d)` | each <= 1e-12 | 0; 1.58e-16 | yes |

Mutations (scratch copy `w49i-work/f523-scratch/tree` from `git archive` of the test commit, own target directory; logs
`base.log`, `mu4.log`, `mu1.log`; the unmutated copy passed first; each mutation applied alone):

| Id | Mutation | Caught by |
| --- | --- | --- |
| MU4 | `face_pressure_load` returns the lumped `-p A/4` at each corner | B1 (1.0e-1), B2 (4.17e-2), B3 (1.0e-1), and R1's moment (7.86e-2) |
| MU1 | `+p` instead of `-p` | B1, B2, B3 (each 2.0), R1 force (2.0) and moment (0.157) |

Both mutations are caught by every one of B1 to B3. `cargo fmt -p rcfd-post --check`: clean after rustfmt of the new file.
`cargo clippy -p rcfd-post --all-targets -- -D warnings`: clean. Only the new test file was run (release); no existing line was
changed, so no existing test and no baseline-hash run is owed; the whole workspace suite was not run (the lead's gate).

Deviations: none. B1 to B3 are bit-identical, as expected from the shared arithmetic; the limits were not touched.

**Decision by the registered rule: done.** Every check met on its single run, both mutations caught by B1, B2 and B3, no
amendment, no library change.

### Remaining questions

1. The comparison is against `Load::pressure` of the same crate (same `face_rule`, `face_shape_functions`), so a common error
   would not show; D1 to D3 and ADR 0436 D3 are the independent support. Options: (a) leave; (b) add a closed-form check of
   `int N_a dA` on the trapezoid (a polynomial, exact by hand). Recommendation: (a), the face is covered by ADR 0436's derivation
   and the lumped control.

## For the lead: roadmap entries

Proposed status of F-523: **done**.

> ADR 0441, 2026-11-14: deck_pressure_loads equals Load::pressure node by node (bit-identical, 0 against the limit 1e-14 relative to the largest nodal load) on a planar trapezoidal Hex8 face, on an asymmetric warped face (corner (1.3, 1, 1.01), not symmetric about a diagonal) and on a strip of two trapezoids sharing nodes; the lumped load -p A/4 differs from the consistent one by 1.0e-1, 4.2e-2 and 1.0e-1 on the same faces, so the faces discriminate; resultant of the trapezoid against deck_force_moment 1.6e-16. The lumped-load mutation (MU4 of ADR 0436) is caught by all three checks and by the moment of the resultant check; the sign mutation too. Test-only (crates/rcfd-post/tests/sb_deck_loads_np.rs), no library change.

Note for F-384: its note may add "its MU4 is now caught by ADR 0441 (F-523)":

> ADR 0441, 2026-11-14: the lumped-load mutation of F-384 (ADR 0436 MU4) not caught on parallelogram faces is caught by F-523's node-by-node comparison on trapezoidal and warped faces; F-384 stays partial by its own rule (the N2 amendment).
