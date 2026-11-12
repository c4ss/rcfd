# ADR 0365: F-458 registration: the Tet4 traction-case L2 order on the pair 16 -> 32 as a registered check

Status: **REGISTERED 2026-11-12, written before any code of F-458 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-458 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Roadmap item F-458 (deps F-378), from [ADR 0279](0279-manufactured-solution-and-convergence-orders.md) addendum 1, remaining
question 1, option (a). Branch `f458-tet4-traction-order` from main 862d7e02, worktree `w49i-work/f458`, scratch
`w49i-work/f458-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here and not changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact and the number recorded; each registered run is made once;
- tests only: one new file `crates/rcfd-fem/tests/sb_mms_tet4_traction.rs`. `sb_mms.rs` (ADR 0279) is not edited; no library
  file, golden or hash table is touched, so every default result keeps its bits by construction;
- no new dependency, no unsafe code; clean-room (finite element a-priori estimates and the manufactured field of ADR 0279).

## What F-458 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run, which discloses that ADR 0279 ran this pair once as a
> diagnostic (L2 order 1.9176). The manufactured solution and case T of ADR 0279 with Tet4 on 8, 16, 32 cells per edge (98 304
> unknowns, direct solver, an ignored heavy record): the L2 order over the pair 16 -> 32 is >= 1.8 and the energy order >= 0.9,
> both errors fall monotonically.

Clauses: (c1) limits not stated are registered here, with the disclosure of the diagnostic; (c2) the manufactured solution and
case T of ADR 0279, Tet4, 8, 16, 32 cells per edge, 98 304 unknowns at the finest, direct solver, an ignored heavy record;
(c3) L2 order of the pair 16 -> 32 `>= 1.8`; (c4) energy order of the same pair `>= 0.9`; (c5) both errors fall monotonically
over the three levels.

## Disclosure: this pair was run before

ADR 0279 ran Tet4 case T on 16 and 32 once, as the diagnostic G2 (`g2_tet4_case_t_one_level_finer_diagnostic`, amendment 1,
no limit): **L2 order 1.9176, energy order 0.9912**, 98 304 free unknowns, 150 s. It also measured Tet4 case T on 4, 8, 16: L2
orders 1.6699 (4 -> 8) and 1.7925 (8 -> 16), errors 8.690e-2, 2.731e-2, 7.883e-3 (L2), and the energy order 0.9750 on 8 -> 16.
The limits below are therefore registered **with knowledge of the outcome of the pair 16 -> 32**; they are the limits of the
acceptance text (written by the lead after that diagnostic) and are not tightened or loosened on the strength of it. This is
a re-measurement of a value already seen, a regression record, not an independent prediction. The same code (copied) and the
same inputs are used, so the run is expected to reproduce the diagnostic; an unexpected difference would be a defect of the
copy, which check R1 is there to expose.

## What was read

1. `AGENT_RULES.md`; `docs/roadmap.json` F-458; ADR 0279 in full (registration, amendment 1, remaining questions, G1 to G3).
2. `crates/rcfd-fem/tests/sb_mms.rs` in full: the field `Trig`, the body force, `add_traction`, `solve_case`, `errors`,
   `error_rule`, `run_sequence`, the diagnostics G1 to G3.
3. `crates/rcfd-fem/src/solve.rs` for `StaticSolver` (direct is the default; `PreconditionedCg` of ADR 0357 exists).
4. Not read: any paper or code outside the repository. The a-priori estimates are cited from memory for orientation (Ciarlet,
   The Finite Element Method for Elliptic Problems, 1978; Strang and Fix) and used only as in ADR 0279.

## Arithmetic before this registration (disclosed, by hand)

1. **Sizes.** Case T on a Tet4 `n^3` box prescribes `xmin`, `ymin`, `zmin`; the free nodes are those with all coordinates
   index `>= 1`, `n^3` of them, three unknowns each: `n = 8`: 1 536; `n = 16`: 12 288; `n = 32`: **98 304** (equals the
   acceptance figure, which is a check, S1). Elements `6 n^3`: 3 072; 24 576; 196 608.
2. **Time and memory of the direct solve.** ADR 0279 measured 5.5 s at 12 288 unknowns (the complete run of the level, mesh,
   solve and error) and 150 s for the pair 16 + 32 in G2, so the level 32 takes about 144 s. Scaling `8^(7/3) = 128` times
   5.5 s gives 700 s as the nominal estimate of ADR 0279 (the measurement beat it by a factor of 5, nested dissection or
   fill better than the exponent). Estimate for this run: **150 to 200 s** total (the level 8 adds about 0.3 s); budget
   limit 1 800 s (the budget of H1). Memory: the factor of a 3-D box problem of `N = 98 304` unknowns with nested-dissection-like
   ordering has about `30 N^(4/3)` entries = 4.4e7 doubles = 0.35 GB; ADR 0279 estimated 2.5 GB for the whole process (matrix,
   factor, working copies). Estimate **0.5 to 2.5 GB**. Free RAM read before this registration: 21.4 GB of 93.6 GB, other agents
   building. The run needs well under that.
3. **Solver choice (stated before the run).** The **direct solver** (`StaticOptions::default()`), as the acceptance says and as
   the diagnostic G2 ran. The iterative solver of ADR 0357 is not used: it would change the quantity checked (a tolerance
   enters the error) and the direct solve is affordable.
4. **Expected orders.** From G2 and the Tet4 trend of ADR 0279 (1.67, 1.79, 1.92 over 4->8, 8->16, 16->32 with the asymptotic
   order 2): L2 order of the pair 16 -> 32 **1.92** (the diagnostic), margin 0.12 over the limit 1.8; energy order **0.99**
   (the diagnostic), margin 0.09 over 0.9. The run is deterministic apart from the summation order of the solver; the
   difference to the diagnostic is expected below 1e-4 in the order.
5. **Monotone fall.** The errors at 8, 16, 32 were 2.731e-2, 7.883e-3 and (from the order 1.9176) about 2.09e-3 in L2; a fall
   by a factor 3.5 to 3.9 per level; the energy error falls by a factor near 2 per level. Monotone with a wide margin.
6. **Copy fidelity.** The errors at `n = 8` and `n = 16` printed in ADR 0279 are 2.731e-2 and 7.883e-3 (4 digits, L2). A correct
   copy reproduces them to the printed digits: relative tolerance 5e-3 (the digits give 2e-3 rounding, the solver 1e-12).

## Design

### D1. The test file `crates/rcfd-fem/tests/sb_mms_tet4_traction.rs`

Copied from `sb_mms.rs` (not imported: integration tests cannot share a file without editing `sb_mms.rs`), reduced to what
case T on Tet4 needs: the field `Trig` of ADR 0279 arithmetic 1 with its gradient and Hessian, the body force
`b = -[(lambda + mu) grad div u + mu Laplace u]`, the Gauss-Legendre nodes by Newton's method, the 125-point tetrahedron
error rule (degree 7), `add_traction` (consistent nodal load of `t = sigma n` on `xmax`, `ymax`, `zmax` by the library face
rules), the prescribed nodes of `xmin`, `ymin`, `zmin` at the exact nodal value, the solve by `solve_static_with` with the
default (direct) options, the L2 and energy errors, the observed order `log2(e(h)/e(h/2))`. Constants that the mutations
change are named (`TRACTION_SCALE`, `LAMBDA_SIGN`, `LOADED`, `LEVELS`) and have the values of ADR 0279 in the committed file.

### D2. Tests

| Test | Content |
|---|---|
| `h2_tet4_case_t_order_16_to_32` | `#[ignore]` heavy record: Tet4 case T on `LEVELS = [8, 16, 32]`, checks S1, S2, R1, A1 to A3, T1 below; run once in release |
| `f2_tet4_case_t_fast_pipeline` | fast, same code path: Tet4 case T on 2, 4, 8, checks F1 to F3 below |

Every measured value is printed with its limit; the test fails at the end if any limit is missed.

## Checks

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| S1 | c2 | free unknowns of the three levels (`result.stats().unknowns()`) | exactly 1 536; 12 288; 98 304 | exact (arithmetic 1) |
| S2 | c2 | the solver used | the default options (direct); recorded in the output | by construction |
| R1 | c2 | L2 error at `n = 8`, `n = 16` against ADR 0279 (copy fidelity) | relative difference `<= 5e-3` to 2.731e-2 and 7.883e-3 | `<= 2e-3` |
| A1 | c3 | L2 order of the pair 16 -> 32 | `>= 1.8` | 1.92 (diagnostic 1.9176) |
| A2 | c4 | energy order of the pair 16 -> 32 | `>= 0.9` | 0.99 (diagnostic 0.9912) |
| A3 | c5 | L2 and energy errors at 8, 16, 32 | each strictly smaller than at the previous level | factors about 3.7 and 2 |
| T1 | - | wall time of the whole test (budget, recorded) | `<= 1 800 s` | 150 to 200 s |
| F1 | c5 | fast: L2 and energy errors on 2, 4, 8 fall monotonically | strict | factors 2.5 to 3.5 and 1.9 |
| F2 | c3 | fast: L2 order of the pair 4 -> 8 | `>= 1.5` | 1.67 (ADR 0279, 4 -> 8) |
| F3 | c4 | fast: energy order of the pair 4 -> 8 | `>= 0.85` | about 0.95 (not measured at 4 -> 8; 8 -> 16 was 0.975) |

The orders of the pair 8 -> 16 are printed (no limit beyond R1; it is the pair that missed 1.8 in ADR 0279, 1.7925).

## Negative controls and planned mutations

Scratch copy `w49i-work/f458-scratch/mut` of the committed test commit; each mutation changes a named constant of the
committed file, the heavy test is run once per mutation in release, the file restored after each (the machine runs several
agents, so MU1 to MU3 take about 3 minutes each).

| Id | Change | Expected: caught by |
|---|---|---|
| MU1 | `TRACTION_SCALE = 1.1` (the tractions 10 percent too large) | A1 and A2 (a consistency error of order 0.1 of the load does not shrink: orders near 0). Estimate of the orders: below 0.3 |
| MU2 | `LAMBDA_SIGN = -1.0` (ADR 0279 negative control: the lambda term of the body force reversed) | A1 and A2, orders below 0.5 as N1 of ADR 0279 (measured there -0.0014 and 0.0000 on case D) |
| MU3 | the traction on `zmax` omitted (`LOADED` of two faces) | A1 and A2 (the missing load does not shrink with h), orders below 0.5 |
| MU4 | `LEVELS = [4, 8, 16]` (the pair that missed in ADR 0279; the check applied to the previous pair) | A1 only: L2 order 1.7925 `<` 1.8; A2 (0.975) and A3 met. R1 would be checked on a different pair and is not applied to this mutation |

MU4 is not a code mutation: it shows that the limit 1.8 separates the pair 16 -> 32 from the pair 8 -> 16, i.e. that the check
reports the pre-asymptotic miss ADR 0279 recorded. The mutations MU1 to MU3 are caught by the heavy test only; the fast test
is a coverage record of the same path, not a registered catcher.

## Decision rule

**done**: S1, R1, A1 to A3 and T1 met in the heavy run, F1 to F3 met, and every mutation MU1 to MU4 caught by the check
registered for it, without an amendment after a run. **partial** (Rule I): a registered check missed, or a registered mutation
not caught by the check registered for it, or caught only after a post-run amendment. **todo**: the file does not build.

## Limits (what is not covered)

- Tet4 only, case T only, the one field and one box of ADR 0279; the pair 16 -> 32 is one pair, not an asymptotic fit; Hex8
  and Tet10 stay as in `sb_mms.rs`.
- The order 1.92 is below 2: the check shows a trend toward 2 (1.67, 1.79, 1.92) and a margin over 1.8, not that the
  asymptotic order is exactly 2.
- The registration is made after G2 (disclosed above); the limits are the lead's acceptance text.
- Direct solver only; the iterative solver of ADR 0357 is not exercised here.
- Peak memory is estimated, not measured, unless the addendum says otherwise.
