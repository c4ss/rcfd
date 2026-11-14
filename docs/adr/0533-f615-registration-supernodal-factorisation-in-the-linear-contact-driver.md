# ADR 0533: F-615 registration: the supernodal factorisation in the linear contact driver

Status: **REGISTERED 2026-11-14, written before any code of F-615 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text two diagnostic probes were run. They are disclosed below with every number used here. The probe
is an uncommitted test file, `f615-scratch/probe_f615.rs` (ADR 0517's hub model, copied, plus two probe functions), copied into
`crates/rcfd-fem/tests/` only while it ran. It calls only the existing public API (`contact::solve`, `contact::solve_with`,
`ContactSet`, `assemble::stiffness`, `constraint::reduce`, `nonlinear::add_triplets`, `SparseLdlt`, `SupernodalLdlt`). No
library code was written or changed. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-615 (deps F-427, F-594), the follow-up proposed by
[ADR 0517](0517-f604-registration-press-fit-of-a-finite-hub-on-a-longer-shaft.md) (its remaining question 4). Number 0533
given by the lead. Branch `f615-work` from 679081a1, worktree `w49i-work/f615`, scratch `w49i-work/f615-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. The brief overrides rule 9 for this item: the F-615 entries of
`docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a separate last commit (the svgs are not regenerated).
Other agents edit F-595 (the eigensolvers' use of the supernodal factor), F-603 (Craig-Bampton) and F-613 (temperature in the
nonlinear paths). This item does not edit `sparse_chol.rs`, `eigen.rs`, `nonlinear.rs`, `nl_transient.rs`, `cms.rs` or any of
their tests: the supernodal factor is only called.

## What F-615 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. contact::linear_solve with an opt-in supernodal L D L^T: the ADR 0517 hub fit at 32 cells through b - a in under a third of the present time, every reading within 1e-10 of the present driver, and without the option every result bit-identical.

Clauses: **c1** limits registered before the run (this document); **c2** `contact::linear_solve` with an opt-in supernodal
`L D L^T`; **c3** the ADR 0517 hub fit at 32 cells through `b - a` in under a third of the present time; **c4** every reading
within 1e-10 of the present driver; **c5** without the option every result bit-identical. The brief adds: (b1) register how the
time is measured; (b2) "every reading" means the registered readings of ADR 0517 (D4); (b3) the default-bits record first,
before library code; `baseline_hashes` unchanged; (b4) no case-file key (an open question, consistent with `solver.factor` of
ADR 0509); (b5) register the mutations the checks catch; (b6) every aggregated test error finite.

## What was read (read only)

ADR 0517 (D3 the model, D4 the readings, addendum 1: A1-H's `n = 32` solve 1336 s, 2 Newton solves of 5 and 2 iterations;
remaining question 4), ADR 0509 (D1 to D5, the `DirectFactor` reuse in the harmonic solve, open question 1 on `solver.factor`),
ADR 0503 (D1 the two dissections, D3 the pivot test and the parallel later-column update; the bits for 1 and 8 threads).
Code: `crates/rcfd-fem/src/contact.rs` (whole; `factor_cached`, `linear_solve`, `solve_with`, `ContactOptions`, which is
`#[non_exhaustive]` and built only in `contact.rs`), `crates/rcfd-linalg/src/sparse_chol.rs` (`SparseOptions`,
`SparseLdlt::factor`, `SupernodalLdlt::factor` and `factor_with`, `nested_dissection_order`,
`nested_dissection_order_with_points`), `crates/rcfd-fem/src/solve.rs` (`DirectFactor`, `supernodal_direct`),
`crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions::new`: residual and energy tolerances 1e-10; `add_triplets`; `Contribution`),
tests `sa_contact_hub_f604.rs` (the model and `read`), `sa_contact_project_f604_default_bits.rs` (B1: digests of the blocks, the
ring and the hub at `n = 4`), `sa_contact_default_bits.rs`. Nothing outside the repository; no algorithm is new here (the
supernodal multifrontal factor is ADR 0503's, cited there).

Findings that shape the design:

1. `linear_solve` builds one factor per Newton iteration whose reduced tangent changed bits (`factor_cached`), always
   `SparseLdlt::factor` (approximate minimum degree, positive pivots, symmetry tolerance 1e-12). A pivot refusal
   (`SkylineError::Pivot`) switches that iteration to the tangent without the curvature part (ADR 0507 amendment 2).
2. A1-H's `n = 32` solve was 2 Newton solves of 5 and 2 iterations, so 7 factorisations in 1336 s.
3. The positive pivot test of both factors refuses exactly when a pivot is at or below a rounding threshold. By Sylvester's law
   of inertia the number of negative pivots of `L D L^T` does not depend on the order, so for a tangent whose smallest
   eigenvalue is not within rounding of zero, both factors refuse the same tangents, and the convex switch happens at the same
   iterations.

## The probes (disclosed; every number used)

All in release, on this machine (16 logical processors, rayon's default pool), with other agents' builds running.

**P1** (`probe_p1_factor_times`, log `f615-scratch/probe1b.log`). The hub of ADR 0517 D3 at `n = 16` and `32`; the reduced
first-iteration tangent `K + K_c(u = 0)` (the contact tangent of `ContactSet::add` at `u = 0`, added by `add_triplets`, reduced
by `reduce` on the prescribed equations), factored with `PivotRule::NonZero` (the tangent is indefinite: a first run with
positive pivots was refused at position 4225, pivot `-1.64e11`, `f615-scratch/probe1.log`).

| `n` | free unknowns | nnz(`K_t`) | `SparseLdlt` (min. degree): order + factor, nnz(L) | supernodal, coordinate dissection: order, factor, nnz(L), largest front | supernodal, level structures: order + factor, nnz(L), largest front |
|---|---|---|---|---|---|
| 16 | 34 154 | 2 457 746 | 26.53 s, 27 243 331 | 0.13 s, 2.16 s, 24 804 385, 2 775 | 2.23 s, 20 255 054, 2 551 |
| 32 | 96 046 | 7 094 744 | 186.45 s, 112 838 626 | 0.61 s, 15.64 s, 119 446 673, 5 664 | 6.24 s, 79 715 546, 4 041 |

At `n = 16` the inertia of both factors is `(265, 33 889)`, equal. The solution of the first tangent by the supernodal factor
differs from `SparseLdlt`'s by `1.2e-11` (coordinates) and `6.1e-12` (levels) relative in the largest component, with residuals
`1.6e-14`, `1.1e-15` and `2.1e-15`. At `n = 32` the inertia is `(422, 95 624)` for both, the differences are `3.2e-7`
(coordinates) and `8.8e-10` (levels), with residuals `2.7e-11`, `7.6e-16` and `7.8e-13`. This first tangent is indefinite and
factored without pivoting, so these differences measure its conditioning; the driver does not solve it (finding 1: the
positive test refuses it and the iteration takes the tangent without the curvature part), and each later Newton iteration
corrects the error of the one before (P2). The other per-iteration costs at `n = 32`: the stiffness 0.27 s (once per
solve), the contact set's evaluation 0.01 s, the copy, triplets and reduction 0.22 s, a solve 0.33 s.

**P2** (`probe_p2_tangent_sensitivity`, log `f615-scratch/probe2.log`). The readings' sensitivity to a perturbation of the
tangent, a proxy for another factor's rounding: `contact::solve_with` with an extra contribution that adds `delta` times the
stiffness diagonal to the tangent and nothing to the force (so the converged state is the same, the Newton path not quite),
against the plain solve, at `n = 4` and `8`, `delta = 1e-14` and `1e-12`. Every Newton solve took the same iterations and
convex iterations as the plain one (`[(3, 1), (2, 0), (2, 0)]`: three solves, the first with one convex iteration). The largest
relative change of the readings of ADR 0517 D4 (`p_mid`, `B_0.25`, `B_0.5`, `P_f`, `q_0.25`, `q_0.5`, every row's `F_k/A_k`)
was `9.7e-13` at `n = 4` and `2.0e-12` at `n = 8` (rows), and of the displacement `7.2e-15`.

Reading: the factor is about 0.98 of the present solve time (7 x 186.45 s = 1305 s of A1-H's 1336 s; the remaining costs are below 1 s per
iteration). The level-structure dissection stores 0.67 of the coordinate dissection's entries at `n = 32` and factors in 0.40 of
its time, so the driver orders by level structures (D2). The readings of two Newton paths that differ at the rounding level of
a factor agree to about `2e-12`.

## Design

### D1. The option (`crates/rcfd-fem/src/contact.rs`)

`ContactOptions` gains `pub factor: DirectFactor` (`rcfd_fem::solve::DirectFactor`, the enum that names the factor everywhere,
as ADR 0509 D4 did for the harmonic solve) and the builder `factor(DirectFactor)`. `ContactOptions::new` sets
`DirectFactor::SparseMinimumDegree`, the present driver. `DirectFactor::SupernodalNestedDissection` selects the supernodal
factor (c2). `check` refuses with `FemError::InvalidValue { key: "solver", .. }`: `DirectFactor::Envelope` (the contact driver
has no envelope factor) and `SupernodalNestedDissection` with `Kinematics::TotalLagrangian` (that body's tangent is factored by
`nonlinear::solve_from`, which this item does not change). No case-file key (b4).

### D2. The factor

`factor_cached` takes the choice. Its cache holds `(CsrMatrix, Factored)`, with `enum Factored { Sparse(SparseLdlt),
Supernodal(SupernodalLdlt) }` and one `solve` that calls the held factor's `solve`. The test of reuse (the same pattern and the
same bits) is unchanged. The `SparseMinimumDegree` arm is `SparseLdlt::factor(a)?`, the present call. The
`SupernodalNestedDissection` arm is `SupernodalLdlt::factor(a)?`: nested dissection by level structures of the reduced
tangent's pattern (ADR 0503 D1), positive pivots and the symmetry tolerance 1e-12, the same rule and tolerance as the present
factor's. A `SkylineError::Pivot` from either arm takes the existing convex switch. The order is computed inside each
factorisation (P1: included in the 6.24 s). No other line of `linear_solve` or `solve_with` changes.

**Bits (c5).** Without the option the code path is the present one: the same factor call on the same matrix and the same
solve. Recorded by B (below) before any library code.

**Threads.** `SparseLdlt` is sequential. The supernodal factor's only parallel step, the later-column update of large fronts,
gives the same bits for any pool size (ADR 0503 D3, ADR 0509 L3), so the supernodal driver's results do not depend on the
thread count (check D).

## How the time is measured (c3, b1)

The heavy record `f615_hub_n32_supernodal_against_present_heavy` builds ADR 0517 D3's model at `n = 32`, `L = 4 (b - a)`, with
ADR 0517's options (`ContactOptions::new(1, 1e-6 d)`, `NewtonOptions::new(1).max_iterations(30)`, penalty `100 E/h`), and calls
`contact::solve` twice in the same test process, one after the other: first with the default options (the present driver),
then with `.factor(DirectFactor::SupernodalNestedDissection)`. Each time `t_p`, `t_s` is the wall time of that one call
(`std::time::Instant` before and after; the model build and the readings are outside). Both calls run in the same process with
rayon's global pool (its size, `rayon::current_num_threads()`, is printed; 16 on this machine) and `NewtonOptions::threads =
None`, so the thread count is the same. The ratio is `t_s / t_p` from that single pair of runs. The run is made once, alone (no
other test of this agent running), started only when the free physical memory is at least 6 GB (estimate of the peak: about 4.5 GB, ADR 0517's 3.9 GB for the present driver plus the supernodal factor's 0.64 GB of values and its fronts), by a PowerShell script
(`f615-scratch/heavy.ps1`, ADR 0509's script with the names changed) that also records the process's peak working set polled
once a second. Other agents' work on the machine is not controlled; it is a limit of the measurement. `t_p` is also compared
with A1-H's 1336 s (recorded, no limit).

## Checks (limits fixed now)

Readings (b2) are those of ADR 0517 D4, by its `read`: `p_mid`, `B_0.25`, `B_0.5`, `P_f`, `q_0.25`, `q_0.5`, every row's
resultant `F_k/A_k`, the largest node pressure and its row, and E1's flag (the face row has the largest row resultant and holds
the largest node pressure). The difference of a reading is `abs(x_s/x_p - 1)` with `x_p` the present driver's. The aggregate
over rows is `worst` of ADR 0517 (infinite if any value is not finite, b6). A non-finite value is a miss.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| T1 | c3 | heavy, `n = 32`: `t_s / t_p` | `< 1/3` | P1: present about 7 x 186 s plus about 10 s (A1-H: 1336 s); supernodal about 7 x 6.2 s plus about 10 s: ratio about 0.04 |
| V1 | c4 | heavy, `n = 32`: each of `p_mid`, `B_0.25`, `B_0.5`, `P_f`, `q_0.25`, `q_0.5`, the largest node pressure, and the worst over the rows | `<= 1e-10` each | P2: up to `2.0e-12`; P1 (8.8e-10 on the indefinite first tangent, not solved by the driver) is the risk |
| V1e | c4 | heavy, `n = 32`: the row of the largest node pressure, and E1's flag | equal | P2 |
| V2 | c4 | fast, `n = 4` and `n = 8`: as V1 and V1e | as V1 and V1e | P2: `9.7e-13`, `2.0e-12` |
| V3 | finding 3 | `n = 4`, `8` (fast) and `32` (heavy): the number of Newton solves (augmentations), the iterations of each and its convex iterations, supernodal against present | equal | finding 3; P1 inertia equal; P2 |
| U1 | c4 | `n = 4`, `8`, `32`: `max abs(u_s - u_p) / max abs(u_p)` over the displacement | `<= 1e-10` | P2: `7.2e-15` |
| B | c5, b3 | `tests/sa_contact_f615_default_bits.rs`, run on 679081a1 plus this ADR plus the file, before any library code; its digests written into the file; after the change equal. Cases: **G1** the hub at `n = 8` (ADR 0517 D3; displacement, reactions, every contactor node's force, pressure, gap and local coordinates, every Newton residual, energy, penetration, convex count and active set); **G2** ADR 0507 B1's two blocks pressed, 2 load steps, linear body, with `max_augmentations(0)` and an extra contribution that adds nothing (the pure penalty and the `extra` path); **G3** the same blocks with `Kinematics::TotalLagrangian`. Also `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs` (B1) and `sa_contact.rs` run unchanged, and `cargo test -p rcfd-api --release --test baseline_hashes` | equal; pass; pass | D2 |
| D | D2 | fast, `n = 16`, the supernodal driver inside rayon pools of 1 and 4 threads: displacement, node forces and pressures, every Newton residual | bit for bit | ADR 0503 D3; P1: 306 fronts split at `n = 16`, so the parallel path runs |
| R | b1 | heavy: `t_p`, `t_s`, the thread count, the peak working set, `t_p` against 1336 s | recorded | |

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | V2: the supernodal displacement at `n = 4` and at `n = 8` is not bit-equal to the present one | holds (the option took effect) |
| N2 | `contact::solve` with `.factor(DirectFactor::Envelope)` and with the supernodal factor and `Kinematics::TotalLagrangian`: each `FemError::InvalidValue` with key `"solver"` | holds |

### Planned mutations

On a `git archive` copy of the final tree in `f615-scratch/mut` with its own target directory `target-f615-mut`; each alone,
by exact text replacement in `contact.rs`; the fast test files (`sa_contact_supernodal_f615.rs` without the heavy record, and
`sa_contact_f615_default_bits.rs`) run once; the file restored.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | the supernodal arm builds `SparseLdlt::factor` | N1 |
| M2 | the minimum-degree arm builds `SupernodalLdlt::factor` | B (G1, G2) |
| M3 | the supernodal arm factors with `PivotRule::NonZero` (accepting the indefinite exact tangent) | V3 (P2: one convex iteration in the first solve) |
| M4 | the supernodal arm reuses the cached factor whenever the pattern is the same (values not compared) | V3 |
| M5 | `check` accepts `DirectFactor::Envelope` (and the driver then uses the minimum-degree factor) | N2 |
| M6 | `check` accepts the supernodal factor with `Kinematics::TotalLagrangian` | N2 |

## Tests (new files only)

- `crates/rcfd-fem/tests/sa_contact_f615_default_bits.rs`: B (G1 to G3), written and committed with its digests before any
  library code.
- `crates/rcfd-fem/tests/sa_contact_supernodal_f615.rs`: ADR 0517's model and `read` (copied); `f615_v2_hub_n4_n8_supernodal_
  against_present` (fast: V2, V3, U1, N1 at `n = 4`, `8`), `f615_d_threads_n16` (fast: D), `f615_n2_refusals` (fast: N2), and
  the heavy record `f615_hub_n32_supernodal_against_present_heavy` (`#[ignore]`, about 25 minutes, run once, alone: T1, V1,
  V1e, V3, U1, R).

## Decision rule

**Done** if every check meets its limit on its registered run (the heavy record included), both negative controls hold, and
every mutation is caught by a check named for it on its first run. **Partial** if a check misses or is not run, a mutation
survives or is caught only by another check, or a check is met only after an amendment made after its run. **Todo** if the
code does not build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- The total Lagrangian body (its factor is `nonlinear::solve_from`'s); refused, not changed.
- The coordinate dissection is not used by the contact driver (P1: more fill on this model). The order is recomputed at every
  factorisation, not cached by pattern.
- Whether the supernodal factor becomes the contact driver's default changes default results: not taken, open.
- The time is one pair of runs on a shared machine; no repetition (each registered run is made once).
- No case-file key (b4).

## Open questions

1. **Case-file key.** Not added (it changes the schema golden). Options: (a) one key `solver.factor = "envelope" |
   "supernodal"` read by every analysis, the contact driver included (ADR 0509's open question 1); (b) a contact-only key;
   (c) none. Recommendation: (a), after the maintainer approves the golden change. Not taken (it changes a golden).
2. **The enum.** Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the option reuses
   `DirectFactor` (one enum names the factor everywhere) and refuses `Envelope`; the contact driver's supernodal order is by
   level structures, while the static and harmonic solves order the same variant by coordinates. The variant's name says
   "nested dissection", which holds for both.
3. **Default.** Making the supernodal factor the contact default changes default results: not taken, open.
