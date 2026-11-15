# ADR 0615: F-684 registration: the line search on the other total Lagrangian contact cases (ADR 0542 P, ADR 0551 HS20t and HS10t)

Status: **REGISTERED 2026-11-15, written before any code of F-684 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-684 was written or run; no probe and no diagnostic run was made. The arithmetic below
was done by hand from the code and the earlier ADRs (no rcfd run, no script). Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-684 (deps F-608, F-639), the follow-up proposed by
[ADR 0607](0607-f608-registration-total-lagrangian-contact-with-a-lifted-contactor.md) (its remaining question 1). Number 0615
given by the lead. Branch `f684-work` from `a9efc22e` (main), worktree `w49i-work/f684`, scratch `w49i-work/f684-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or
ignored record edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and
`docs/roadmap.json` not edited. Every choice the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-684 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The total Lagrangian contact cases ADR 0542 P (hub, d =
> 1e-3 a) and ADR 0551 HS20t and HS10t are run with NewtonOptions::line_search with and without curvature_fallback; each run
> converges or its failure is diagnosed, and the fallback iterations and step lengths are recorded (limits registered).

Clauses: **c1** every other limit registered here; **c2** six runs: {P, HS20t, HS10t} x {line search alone (`-L`), line search
and fallback (`-LF`)}; **c3** each run converges, or its failure is diagnosed; **c4** the fallback iterations and the step
lengths of each run are recorded, with registered limits.

This is a measurement item. No library change is planned; one is made only if a run shows a defect that needs it (then by an
amendment committed before the corrected run, opt-in, defaults bit-identical).

## What was read

ADR 0607 (D1 the line search: `s(alpha) = -d . r(u + alpha d)`, `eta = 0.5`, Illinois regula falsi with 8 further trials, the
condition "only an iteration without a prescribed increment is searched"; D2 `Contribution::add_trial_force`; D3
`NewtonSolve::step_lengths`; its remaining question 1, which names this item), ADR 0570 (F-639: `ContactOptions::
curvature_fallback`, the fallback in `Problem::attempt_with`, `NewtonSolve::convex_iterations`; P-f `[(3, 1), (2, 0), (2, 0)]`;
HS20f and HS10f: one fallback iteration each in the first solve, Newton iterations `[10, 3, 3, 3, 3, 3, 3, 2, 2, 2]` and `[9, 3,
3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2]`, `F_fe` 1.4088e-3 and 1.4159e-3, `a_M` 0.10118 and 0.09944; arithmetic 1, which left open at
which iteration ADR 0551's pivot failed), ADR 0542 (P: the total Lagrangian hub at `n = 4` with ADR 0517's `d = 1e-3 a`
refused at the first tangent by both factors, supernodal "pivot -9.0e8 at node 2418"), ADR 0551 (HS20t "pivot -0.148 at node
1129 component 1", HS10t "pivot -0.0336 at node 931 component 1", both at augmentation 0). Code: `crates/rcfd-fem/src/
nonlinear.rs` (`Problem::attempt_with` 1663 to 1862, `trial_slope`, `step_length`, `contribute_split`, `solve_from_field_reusing`
2600 to 2650), `crates/rcfd-fem/src/contact.rs` (`solve_with` 2272 to 2450: the start `u` carries the step's prescribed values
before each Newton solve; `extra` contributions after the contact set and once more for the reactions). Tests:
`sa_contact_tl_fallback_f639.rs` (P-f, `hertz_setup`, `hertz_run`, `moment_radius`, NF-HS), `sa_newton_supernodal_f618.rs`
(`f618_p_refused_pivot_tl_hub`), `f601_common`, `f623_common`, `f624_common` (`fit_model`, `options`, `Route`, `BASE`).

## Arithmetic done beforehand (disclosed; from the code, no run)

1. **Every iteration of the contact driver's Newton solves is searched.** `solve_with` sets `u[eq] = lf v` at the prescribed
   equations before each solve, and `solve_from_field_reusing` calls `attempt(start, 1.0, 1.0, ..)`: `lc = lt = 1`, so the
   prescribed increment `(lt - lc) v` of iteration 0 is `0` and ADR 0607 D1's condition holds at every iteration, the first
   included. (ADR 0607's limit "iteration 0 of a step with non-zero prescribed values takes the full step" does not arise on
   this path.) The Hertz cases are driven by prescribed far-face values; their first iteration starts from the far faces
   displaced and the rest at zero, and it is searched.
2. **P with the line search alone is refused at the same pivot as the default.** P has no prescribed value and no load: the
   start is `u = 0` with the interference `d` as initial penetration. ADR 0542 records the refusal "at the first tangent": the
   tangent of iteration 0 at the start, before any correction. The line search acts on corrections only; the tangent and its
   factorisation at iteration 0 are the same calls on the same data. So P-L ends with the same error, byte for byte, as the
   default P on the same route; the diagnosis is the one of ADR 0542 and ADR 0570 (the contact curvature part at the start's
   forces `eps d`), which only the fallback addresses.
3. **P with both options keeps P-f's bits.** Iteration 0 takes the fallback matrix `K_f = K_e + eps G G^T` because the exact
   tangent fails. The omitted part is the curvature term `-t d2g/dq2`. Against a convex target (the shaft's cylinder, the
   contactor outside it) `g` is a convex function of the contactor's position and `-t d2g` is negative semi-definite, so
   `K_f >= K_exact` in the quadratic model: `s(1) = -d . K_f d + d . K_exact d <= 0` (an undershoot), and D1 accepts `alpha =
   1` for every `s(1) <= eta |s_0|`, in particular every negative one. The remaining nonlinearity at the strain `d/a = 1e-3` is
   of relative size `1e-3`. Later iterations are Newton's on a converging iteration (P-f: 3, 2, 2 iterations), where `s_1/s_0`
   is of the order of `r_{k+1}/r_k`, far below `eta` (ADR 0607 arithmetic 3). With every `alpha = 1` the step is `u_i += d_i`
   exactly as without the option and the trial evaluations change no state (ADR 0607 D2, LS0), so P-LF reproduces the run with
   the fallback alone bit for bit: the record `[(3, 1), (2, 0), (2, 0)]`, every step length `1`. The faceted target's nodes
   also move, so convexity in all variables is not proven: **likely, not certain.**
4. **The default HS refusal is at iteration 1 of the first solve.** At the start the top surface is at zero displacement: the
   gap is `r^2/(2R) >= 0`, zero only at the origin, the multipliers zero, so every contact force `t = max(0, lambda - eps g)` is
   `0` and the curvature part vanishes; the tangent of iteration 0 is the element tangent of the start, which has no
   compressed or inverted element (the boundary layer of the far faces strains by about `delta/h_far`, `h_far` the coarse
   graded cells, a few percent). It factors. Iteration 0's full correction lifts the surface by about `delta = 0.01`
   (ADR 0570 arithmetic 1), the iterate penetrates the sphere over `r < 0.14`, and the tangent of iteration 1 is the one ADR
   0551 saw refused. Estimate: the refusal at iteration 1 (the iteration index is read by D2's recorder below).
5. **The first step length of HS** (both `-L` and `-LF`; their iteration 0 is the same: same start, same exact tangent, which
   factors by 4). A model along the first correction `d` (a lift of about `delta`): the elastic part `s_e(alpha) = -|s_0| (1 -
   alpha)` with the minimum at `alpha = 1`; the penalty part over the quarter, with the penetration `p(alpha) - r^2/2` (`p =
   delta alpha`, `R = 1`) and the penalty per area `eps = 100/h` (`h = a/4`: 4000): `W_c = (eps/2)(pi/2) int_0^p (p - w)^2 dw =
   eps pi p^3/12 = 1.05e-3 alpha^3`, `W_c' = 3.1e-3 alpha^2`. `|s_0|` is twice the elastic energy of the start relative to the
   lifted state: a boundary layer of three far faces of `0.25 x 0.25` (the quarter of `5a = 0.5`) and thickness about `0.1`
   with the strain about `0.1` gives `E eps^2/2 x V = 0.5 x 1e-2 x 7.5e-2 = 3.8e-4`, `|s_0|` about `7.5e-4` (order of magnitude
   only). `s(1) = 3.1e-3 > eta |s_0|`, so the search runs; the root of `3.1e-3 alpha^2 + 7.5e-4 alpha - 7.5e-4` is `alpha =
   0.38`. The accepted `alpha` is where `|s| <= 0.5 |s_0|`: about `0.25 to 0.45`. A factor 3 on `|s_0|` moves the root between
   `0.25` and `0.6`. Limit `alpha_0 <= 0.9` (the full step is not taken).
6. **HS-L (line search alone): outcome not decidable before the run.** At the damped first iterate the penetration is about
   `alpha_0 delta` and the nodal forces about `alpha_0` times those of the full-step iterate (`t = eps_a pen`), so the curvature
   part is about `0.4` times that of the iterate ADR 0551 saw refused. Whether that tangent factors depends on the ratio of
   the curvature part to the element's tangential stiffness at the refusing node, which neither ADR recorded (ADR 0570
   arithmetic 1: its own estimate was smaller than the measured pivot). Two outcomes are possible: a refused pivot at a later
   iterate (the curvature part, which the line search does not change: it shortens steps, the tangent at the next iterate is
   still the exact one), or convergence once every iterate's forces stay low enough. Either is the acceptance's outcome if it
   is diagnosed; no limit on which.
7. **HS-LF (both): converges.** Where the exact tangent fails the fallback factors `K_e + eps G G^T` (positive definite by the
   pivot test), a descent direction; by 3 a fallback step is an undershoot against the sphere (convex, rigid: `-t d2g` negative
   semi-definite, proven for a rigid sphere) and is taken in full. The line search damps only exact-tangent steps that
   overshoot (new active nodes, the penalty kink). ADR 0570 converged without the damping; the damping keeps the iterates
   nearer the line minimum. The risks: an element tangent indefinite at an iterate ("without the curvature part", ADR 0570 D2),
   30 iterations exhausted in the first solve by short steps (HS20f needed 10 full ones), or D6's cycle count. Estimate:
   converges, 10 to 20 augmentations as HS20f and HS10f.
8. **HS-LF reaches the same state as HS20f and HS10f.** The converged state solves the same discrete equations (the fallback
   and the line search change iterates, not residuals) with the gap tolerance `1e-8`; the augmentations may differ in number.
   The difference of two states both within `1e-8` of the constraint against the approach `delta = 1e-2` is of relative order
   `1e-6` in `F_fe` and `a_M`; ADR 0570 printed them to 5 digits (rounding up to `3.6e-5` of `F_fe = 1.4088e-3`, `5e-6` of
   `a_M`). Limit `1e-3` relative for both against ADR 0570's printed values. The Hertz limits of ADR 0570 (radius `0.15`, peak
   `0.20`, approach `0.10`) are repeated.
9. **Fallback iterations of HS-LF.** HS20f and HS10f took one each, at the full-step iterate. The damped iterates of HS-LF have
   forces about `alpha_0` times those, so fewer refusals are expected, not more; limit: at most `2` fallback iterations over
   all solves of each run (one more than ADR 0570's, for an iterate that the damping makes different). P-LF: exactly `1`
   (arithmetic 3).
10. **The trial count.** D1 makes at most `1 + 8` trial evaluations per correction. Limit `<= 9` (the recorder's count).
11. **The recorder's step lengths.** The recorder reconstructs `alpha_k = (u_{k+1} - u_k) . d/(d . d)` with `d = x_1 - u_k`,
    `x_1` the first trial state of iteration `k` (D1 always tries `alpha = 1` first). Rounding: `|u|/|d|` times `1e-16`; for the
    corrections with `|d|_inf >= 1e-6 |u_k|_inf` the error is below `1e-9`. Limit `1e-6` relative against `NewtonSolve::
    step_lengths` on those corrections.

## Design

### D1. No library change

The runs use the existing opt-ins: `ContactOptions::newton(NewtonOptions::new(1).max_iterations(30).line_search(true))`
(ADR 0607) and `ContactOptions::curvature_fallback(true)` (ADR 0570), with the total Lagrangian body. Each case's other options
are its record's:

- **P** (ADR 0542, as ADR 0570's P-f): `f624_common::fit_model(Setup { n: 4, ..BASE })`, `options(&s, Route::Contact)` (the
  supernodal factor through `ContactOptions::factor`, `NewtonOptions::new(1).max_iterations(30)`, gap tolerance `1e-6 d`, the
  penalty `100 E/h`), its newton options replaced by the same ones with `.line_search(true)`.
- **HS20t, HS10t** (ADR 0551 D6, as ADR 0570's `hertz_setup` with `tl = true`): the meshes `hs20_mesh()`, `hs10_mesh()`, the
  sphere pair with `100/h` (`h = a/4`, `a/3`), `ContactOptions::new(1, 1e-8)`, `NewtonOptions::new(1).max_iterations(30)` with
  `.line_search(true)`, the mortar contactor, the total Lagrangian body, the supernodal factor.

### D2. The recorder (test only)

A test `Contribution` passed through `contact::solve_with`'s `extra`: name `"recorder"`, `add` writes nothing into the force
or the tangent and records the evaluation `u`; `add_trial_force` writes nothing and records a trial `u`; no fallback tangent,
not in the residual reference. It keeps only the last evaluation and the first trial after it, and per correction the pair
(trial count, reconstructed `alpha`, `|d|_inf/|u|_inf`). An evaluation at the same `u` (bit for bit) with no trial since the
last one starts a new segment (a new Newton solve starts at the converged `u` of the previous; the reactions add one more
evaluation after the accepted solve). So the recorder gives, also for a failed run, the iteration at which the last solve
ended. Being passive, it changes no bit: the force is not touched (`+ 0.0` is not even executed), it pushes no triplet, and
`contribute_split` and `contribute_with` then give the same sums; checked by DH and PL below (the error texts with the
recorder against those without it, byte for byte).

### D3. The runs (the test file `crates/rcfd-fem/tests/sa_contact_line_search_f684.rs`)

| run | case | options | test |
|---|---|---|---|
| P-0 | P | default (as `f618_p_refused_pivot_tl_hub`, `Route::Contact`), `solve` | fast |
| P-F | P | fallback alone (ADR 0570 P-f), `solve` | fast |
| P-L | P | line search alone, `solve_with` + recorder | fast |
| P-LF | P | both, `solve_with` + recorder | fast |
| HS20-0, HS10-0 | HS | default, `solve_with` + recorder | heavy records |
| HS20-L, HS10-L | HS | line search alone, `solve_with` + recorder | heavy records |
| HS20-LF, HS10-LF | HS | both, `solve_with` + recorder | heavy records |

The heavy records: one ignored test per mesh (HS20: about 20 s for a converging run, HS10: about 60 s, ADR 0570, times three
runs), each run once. P-0 and P-F are references in the same test (P-0 is the existing record's configuration, P-F ADR 0570's
P-f configuration), not re-runs of a registered result.

## Checks (limits fixed now)

Every aggregated quantity is the maximum of finite values (each asserted finite first); "bit for bit" compares `f64::to_bits`;
"text" compares the `Display` of the error, byte for byte.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| PL | c2, c3 | P-L: `Err`, text equal to P-0's; P-0's text starts with "load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot"; the recorder: one segment, 1 evaluation, 0 corrections (the refusal at iteration 0) | all | arithmetic 2 |
| PLF1 | c2, c3 | P-LF: `Ok` | converges | arithmetic 3 |
| PLF2 | c4 | P-LF: the record (iterations, fallback iterations) per solve equal to P-F's; `convex_iterations` of the first solve `1`, total `1` | equal; 1 | arithmetic 3 |
| PLF3 | c4 | P-LF: every solve `step_lengths.len() == energies.len()`; every length `1.0` | all; all | arithmetic 3 |
| PLF4 | c2 | P-LF against P-F: displacement, the residuals and energies of every solve | bit for bit | arithmetic 3 |
| DH | c3 | HS20-0, HS10-0: `Err` with "load step 1 of 1, augmentation 0", "singular tangent: pivot" and "at node 1129 component 1" (HS20) / "at node 931 component 1" (HS10); the recorder: the refusal at iteration 1 of the first solve | all; 1 | arithmetic 4, ADR 0551 |
| HK0 | c4 | HS20-L, HS20-LF, HS10-L, HS10-LF: the first step length `alpha_0` (recorder; `NewtonSolve::step_lengths[0]` where the run converges) | `<= 0.9` each | arithmetic 5 |
| HL | c2, c3 | HS20-L, HS10-L: converges; or fails, with the failure located (the segment and the iteration from the recorder) and its reason recorded | one of the two | arithmetic 6 |
| HLc | c3 | HS20-L, HS10-L if they converge: radius (moments) `0.15`, peak `0.20`, approach `0.10`; `F_fe` and `a_M` against ADR 0570's within `1e-3` relative | the limits | arithmetic 8 |
| HLF1 | c2, c3 | HS20-LF, HS10-LF: `Ok` with the gap tolerance `1e-8` | converges | arithmetic 7 |
| HLF2 | c3 | HS20-LF, HS10-LF: radius (moments) `0.15`, peak `0.20`, approach `0.10` (ADR 0570's readings) | the limits | ADR 0570 |
| HLF3 | c3 | HS20-LF, HS10-LF: `F_fe` against 1.4088e-3 / 1.4159e-3 and `a_M` against 0.10118 / 0.09944, relative | `<= 1e-3` each | arithmetic 8 |
| HLF4 | c4 | HS20-LF, HS10-LF: the sum of `convex_iterations` over all solves | `<= 2` | arithmetic 9 |
| SL | c4 | every converged run (P-LF, HS-LF, HS-L if it converges): per solve `step_lengths.len() == energies.len()`, every length in `(0, 1]` | all | ADR 0607 D1 |
| RT | c4 | every run with the line search: the recorder's trials per correction | `<= 9` | arithmetic 10 |
| RA | c4 | every converged run with the line search: the recorder's `alpha` against `step_lengths` on the corrections with `\|d\|_inf >= 1e-6 \|u\|_inf`, relative; the recorder's corrections per segment equal to `energies.len()` per solve | `<= 1e-6`; equal | arithmetic 11 |
| B | defaults | no library line changed: `git diff a9efc22e -- crates/*/src` empty; `cargo test -p rcfd-fem --release --test sa_contact_tl_fallback_f639 --test sa_newton_supernodal_f618 --test sa_contact_lifted_f608` pass | empty; pass | D1 |

Recorded without a limit: per run and per solve the Newton iterations, the fallback iterations, the step lengths, the trial
counts, the penetration, the augmentations, the run time; the active contactor nodes at the end; for HS-L and HS-LF the
Hertz readings (also where no limit applies); for a failed run the error text, the segment and iteration of the failure and the
step lengths before it.

Negative controls: P-0 and HS20-0, HS10-0 (the defaults keep their refusals; DH, PL); P-L (the line search without the fallback
cannot act on a tangent refused before the first correction).

### Planned mutations (on a `git archive` copy of the final tree in `f684-scratch/mut`, target `f684-scratch/target-mut`, each alone by exact text replacement; the fast test run once, the HS20 heavy record where named; the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| MA | `contact.rs`: the total Lagrangian path reports `step_lengths: Vec::new()` | PLF3 (lengths count), SL |
| MB | `contact.rs`: `ContactSet::has_fallback_tangent` returns `false` | PLF1 (P-LF refused) |
| MC | `nonlinear.rs`: the line search skips iteration 0 (`&& k >= 1` on the search condition) | HK0 on the HS20 heavy record (`alpha_0 = 1`); not by the fast test (P's `alpha` are all 1) |

## Decision rule

- **done**: every check met on its registered run (the heavy records included), every planned mutation caught by a check named
  for it, fmt and clippy clean. HL is met by either outcome (the acceptance's "converges or its failure is diagnosed").
- **partial**: B met and any other check missed (recorded as a `*_known_miss` test with its limit and the measured number, and
  diagnosed), or a mutation not caught, or a run not made.
- **todo**: otherwise, in particular any change of a library line without an amendment, or any change of the default bits.

## D4. Files

The new test file `crates/rcfd-fem/tests/sa_contact_line_search_f684.rs` (it uses the existing helpers of `f601_common`,
`f623_common`, `f624_common`, unchanged). No library file, no existing test or record edited.

## Limits (not covered)

- P only on the supernodal route (`Route::Contact`, as ADR 0570's P-f); the present route and `Route::Newton` are not run with
  the line search.
- One load step for every case (the records' setting); more load steps, which would shorten the first step's overshoot by load
  control, are not run.
- The convexity argument of arithmetic 3 is proven for a rigid convex target (the HS sphere), not for the deformable faceted
  shaft of P.
- The defaults stay as they are: making the line search or the fallback the default of the total Lagrangian contact body is
  the maintainer's question (ADR 0570 remaining question 1, ADR 0607 remaining question 1); this item gives the measurements
  for it.
- The cycle detector, friction, several pairs and the linear body are not exercised here.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The route of P.** Options: (a) `Route::Contact` (supernodal, as ADR 0570's P-f, so P-F is a known reference); (b) both
   routes. Taken (a): the acceptance names the case, not the route, and (a) keeps a measured reference.
2. **How to locate a failure.** Options: (a) a passive recording contribution through `solve_with`'s `extra` (test only, D2);
   (b) a library field in the error (changes error texts, a default output). Taken (a).
3. **Library changes.** Taken: none unless a run diagnoses a defect (then by an amendment).

## Addendum 1: results (2026-11-15)

The test file was committed (f8ae47a9) after the registration (803be07d). The fast test ran once before that commit and the heavy
records once after it, with nothing in between that changes a run. Logs in `w49i-work/f684-scratch`: `run-fast.log` (P),
`run-heavy.log` (HS10 and HS20, one test thread, 172 s, free memory 7.8 GB at the start), `mut-MA.log`, `mut-MB.log`,
`mut-MC.log`, `run-mut.log`, `run-mut2.log`, `clippy.log`, `run-B.log`. No limit was changed, no amendment was made and no
diagnostic run followed the registered ones. No library line was changed.

| id | limit | measured | met |
|---|---|---|---|
| PL P-0 refused at the first tangent | the text | "load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot -9.009926067085571e8 at node 2418 component 0 (diagonal 9.939e10)" | yes |
| PL P-L: P-0's text; recorder 1 evaluation, 0 corrections | equal; holds | equal byte for byte; one segment, 1 evaluation, 0 corrections | yes |
| PLF1 P-LF converges | converges | converges (1.65 s) | yes |
| PLF2 record equal to P-F's; fallback 1 in the first solve, 1 in all | equal; 1; 1 | `[(3, 1), (2, 0), (2, 0)]` both; 1; 1 | yes |
| PLF3 one length per correction, all 1 | all; all | `[[1, 1, 1], [1, 1], [1, 1]]` | yes |
| PLF4 P-LF against P-F | bit for bit | bit for bit (displacement, residuals, energies of the 3 solves) | yes |
| DH HS20-0: ADR 0551's text; refusal at iteration 1 | holds; 1 | "pivot -1.4835732797044943e-1 at node 1129 component 1 (diagonal 2.036e-2)"; 2 evaluations, 1 correction | yes |
| DH HS10-0: ADR 0551's text; refusal at iteration 1 | holds; 1 | "pivot -3.363022575389211e-2 at node 931 component 1 (diagonal 4.723e-2)"; 2 evaluations, 1 correction | yes |
| HK0 `alpha_0`: HS20-L, HS20-LF, HS10-L, HS10-LF | `<= 0.9` each | 0.26123, 0.26123, 0.24875, 0.24875 (L and LF equal bit for bit) | yes |
| HL HS20-L | converges or a located failure | converges (25.9 s), 10 solves, Newton iterations `[9, 3, 3, 3, 3, 3, 3, 2, 2, 2]` | yes |
| HL HS10-L | converges or a located failure | converges (52.3 s), 13 solves, `[7, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2]` | yes |
| HLc HS20-L: radius, peak, approach; `F_fe`, `a_M` against ADR 0570 | `0.15`, `0.20`, `0.10`; `1e-3` | 2.51e-2, 1.23e-1, 2.65e-2; 8.3e-6, 3.8e-5 | yes |
| HLc HS10-L: the same | the same | 5.78e-3, 1.23e-1, 2.31e-2; 1.2e-5, 3.5e-5 | yes |
| HLF1 HS20-LF, HS10-LF converge | converges | both converge (19.9 s, 71.8 s); the same records as HS20-L and HS10-L | yes |
| HLF2 HS20-LF: radius, peak, approach | `0.15`, `0.20`, `0.10` | 2.51e-2, 1.23e-1, 2.65e-2 | yes |
| HLF2 HS10-LF | the same | 5.78e-3, 1.23e-1, 2.31e-2 | yes |
| HLF3 HS20-LF `F_fe`, `a_M` against 1.4088e-3, 0.10118 | `<= 1e-3` | 8.3e-6 (`F_fe` 1.408812e-3), 3.8e-5 (`a_M` 0.1011762) | yes |
| HLF3 HS10-LF against 1.4159e-3, 0.09944 | `<= 1e-3` | 1.2e-5 (1.415883e-3), 3.5e-5 (0.09943648) | yes |
| HLF4 fallback iterations, all solves | `<= 2` | 0 and 0 | yes |
| SL converged runs with the line search: one length per correction, in `(0, 1]` | all | all (P-LF, HS20-L, HS20-LF, HS10-L, HS10-LF) | yes |
| RT trials per correction | `<= 9` | P-LF 1, HS20 4, HS10 3 | yes |
| RA recorder corrections per solve; `alpha` against `step_lengths` | equal; `<= 1e-6` | equal on all five; 0, 1.0e-14, 1.0e-14, 5.8e-15, 5.8e-15 | yes |
| B no library line changed; the three existing test files | empty; pass | `git diff a9efc22e -- crates/*/src` empty; `sa_contact_tl_fallback_f639` 4 passed, `sa_newton_supernodal_f618` 4 passed, `sa_contact_lifted_f608` 7 passed | yes |

Recorded (no limit). Step lengths below 1: HS20-L and HS20-LF first solve `[0.2612, 1, 1, 0.6605, 1, 1, 1, 1, 1]`, every later
solve all 1; HS10-L and HS10-LF first solve `[0.2488, 1, 1, 1, 1, 1, 1]`, the eighth solve (augmentation 7) `[1, 1, 0.6056]`,
every other all 1. Trials of the damped corrections: HS20 4 and 2, HS10 3 and 2. Final state: HS20 41 of 280 contactor nodes
active, penetration 8.1e-9, 9 augmentations; HS10 30 of 289, 9.7e-9, 12 augmentations (ADR 0570: 9 and 12 augmentations, the
same active counts). Penetrations at the end of the first solve 1.6e-5 (HS20) and 2.6e-5 (HS10).

### Notes on the results

1. **The line search alone solves both Hertz cases; the fallback is never taken.** The `-L` and `-LF` runs give the same
   records, readings and step lengths (`alpha_0` equal bit for bit): with the line search every exact tangent factors, so
   `curvature_fallback` stays unused, as it did on ADR 0607's T2 punch. Arithmetic 1 was what this turned on. The contact
   driver's solves start with the prescribed values already in `u` (`lc = lt`), so the first correction is searched, and its
   length 0.26 (HS20) or 0.25 (HS10), at the low end of arithmetic 5's 0.25 to 0.45, keeps the iterate from going 0.01 into the
   sphere. Without the cut the next tangent is the one ADR 0551 saw refused. DH places that refusal at iteration 1 for both
   meshes, which settles the question ADR 0570 arithmetic 1 left open (the iteration was not recorded). The cause is the
   curvature part at the full-step iterate. The element tangent is not the cause, since the damped iterate factors.
2. **P is the opposite case.** Its exact tangent is refused at the start, before any correction, so the line search cannot act
   there and P-L ends with the default's text byte for byte (arithmetic 2). With the fallback, every step passes the test at
   `alpha = 1` and the run is P-f's bit for bit (arithmetic 3). On this case only the fallback matters.
3. HS-LF reaches ADR 0570's HS20f and HS10f state to 1e-5 in `F_fe` and 4e-5 in `a_M`. That is the 5-digit rounding of ADR 0570's
   printed values, so the line search changes the path and not the solution. The Newton iterations of the first solve drop
   from 10 to 9 (HS20) and from 9 to 7 (HS10). The later solves are the same as ADR 0570's.
4. MC (below) shows the same mechanism from the other side: with iteration 0 not searched, HS20-L is refused at iteration 1
   with ADR 0551's text, and HS20-LF goes back to ADR 0570's HS20f record `[(10, 1), ..]`, one fallback iteration.

### Mutations

Each alone on a `git archive` copy of f8ae47a9 (`f684-scratch/mut`, target `target-mut`, `mutate.py`). The fast test was run
once each; for MC the HS20 heavy record was also run. After each run the copy was checked identical to the worktree up to line
endings (`diff -r --strip-trailing-cr`). On the first attempt the restore of MA found its replacement text twice (an existing
`step_lengths: Vec::new(),` on the linear path). The script was stopped before MB's test ran, the source files were extracted
again from the archive, and MB and MC were run on the clean copy (`run-mut2.log`). MA's own run was on a clean copy and stands.

| id | mutation | caught by |
|---|---|---|
| MA | the total Lagrangian path reports `step_lengths: Vec::new()` | PLF3 (`[[], [], []]`), SL |
| MB | `ContactSet::has_fallback_tangent` returns `false` | PLF1 (P-LF refused with P-0's text) |
| MC | the line search skips iteration 0 | HK0 on HS20 (`alpha_0 = 1` in both runs; HS20-L then fails at iteration 1 with ADR 0551's text, HS20-LF takes one fallback iteration); the fast test passes, as registered |

Three of three were caught by the checks named for them.

### Crate runs

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. Release
runs: the new test file (fast test, and the two heavy records run once), and the existing `sa_contact_tl_fallback_f639`,
`sa_newton_supernodal_f618` and `sa_contact_lifted_f608` (B). The `rcfd-fem` suite and `baseline_hashes` were not run because
no library file changed. The workspace suite was not run (the lead's gate).

Files: the new test file `crates/rcfd-fem/tests/sa_contact_line_search_f684.rs` and this ADR. No library file, existing test
or record was edited.

### Deviations

None from the registration. The mutation restore incident above does not touch a registered run.

### Decision

**done** by the registered rule. Every check was met on its registered run, the heavy records included. Three of three planned
mutations were caught by checks named for them, and fmt and clippy are clean. On the acceptance: all six runs were made. Four
converge (HS20-L, HS20-LF, HS10-L, HS10-LF, the last two identical to the first two) and P-LF converges. P-L fails, and the
failure is diagnosed: a tangent refused at the start, before any correction. The fallback iterations and step lengths of each
run are recorded with registered limits.

### Remaining questions

1. **Defaults** (ADR 0570 question 1, ADR 0607 question 1). The cases now cover the line search: it solves the T2 punch (ADR 0607)
   and both Hertz cases on its own, and the fallback solves P on its own. No case needs both, and no case gets worse with both:
   every run here with both options matches the better single-option run. Options: (a) keep both opt-in; (b) make both the
   default of the total Lagrangian contact body by a superseding ADR, which changes the outcomes of ADR 0542 P, ADR 0551
   HS20t/HS10t and ADR 0519 probe 2 from refusals to solutions, and any default run with `alpha < 1`. Recommendation: (b), as a
   maintainer decision. It changes default results and records, so it is not taken here.
2. **A case that needs both options together.** None is known. Options: (a) leave it; (b) look for one, e.g. a hub with a
   prescribed push-in. Recommendation: (a). No follow-up is proposed.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-608 and F-639 (ADR 0607, ADR 0570). DONE. ADR 0615, 2026-11-15: NewtonOptions::line_search run on the total Lagrangian contact cases with and without curvature_fallback, no library change. ADR 0542 P (hub, n = 4, d = 1e-3 a, supernodal): with the line search alone it is refused with the default's text byte for byte (the exact tangent fails at the start, before any correction the search could shorten); with both options it is ADR 0570's P-f bit for bit, record [(3, 1), (2, 0), (2, 0)], every step length 1. ADR 0551 HS20t and HS10t: the default refusal is at Newton iteration 1 (located by a passive recording contribution), after the full first correction; the contact driver's solves start with the prescribed values in u, so the first correction is searched and cut to 0.26 (HS20) and 0.25 (HS10). Every later tangent factors, and both cases converge with the line search alone (fallback iterations 0, identical with the fallback on): radius 2.5e-2 and 5.8e-3, peak 1.23e-1, approach 2.7e-2 and 2.3e-2, F_fe and a_M within 4e-5 of ADR 0570's HS20f/HS10f; other step lengths below 1: one each (0.66, 0.61). Trials per correction at most 4. 3 of 3 mutations caught. Making the line search and the fallback the default is the maintainer's call (it turns the refusals of ADR 0542 P and ADR 0551 into solutions).

> Text F-608's note should gain: "Follow-up F-684 done (ADR 0615, 2026-11-15): the line search alone solves ADR 0551 HS20t and HS10t; ADR 0542 P needs the fallback (refused at the start)."
