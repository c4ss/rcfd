# ADR 0542: F-618 registration: the supernodal factor in the Newton driver, for the total Lagrangian contact body

Status: **REGISTERED 2026-11-15, written before any code of F-618 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text five diagnostic probes were run. They are disclosed below with every number used here. The probes
are uncommitted test files, `f618-scratch/probe_f618.rs` (ADR 0517's hub model, copied from
`tests/sa_contact_supernodal_f615.rs`, plus four probe functions) and `f618-scratch/probe_q5.rs` (a cantilever), each copied
into `crates/rcfd-fem/tests/` only while it ran. They call only the existing public API. No library code was written or
changed. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-618 (deps F-615), the follow-up proposed by
[ADR 0533](0533-f615-registration-supernodal-factorisation-in-the-linear-contact-driver.md) (its remaining question 3).
Number 0542 given by the lead. Branch `f618-work` from 498b5417, worktree `w49i-work/f618`, scratch `w49i-work/f618-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. The brief overrides rule 9 for this item: the F-618 entries of
`docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a separate last commit (the svgs are not regenerated).
Other agents edit F-601 (contact search and targets in `contact.rs`), F-425 (hyperelastic materials, possibly small hunks in
`nonlinear.rs`) and F-528 (the direct harmonic solve on the transformation path). This item keeps its `contact.rs` hunks to the
option check and the pass-through, its `nonlinear.rs` hunks to `NewtonOptions` and `factor_solve`, and does not touch
`sparse_chol.rs`, `harmonic.rs`, `solve.rs` or any material. Every choice the maintainer would normally be asked about is taken
on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-618 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. NewtonOptions gives nonlinear::solve_from the supernodal L D L^T as an opt-in factor: a total Lagrangian contact solve of the ADR 0517 hub at 16 cells in under a third of the present time, every reading within 1e-10 of the present driver, and without the option every result bit-identical.

Clauses: **c1** limits registered before the run (this document); **c2** `NewtonOptions` gives `nonlinear::solve_from` the
supernodal `L D L^T` as an opt-in factor; **c3** a total Lagrangian contact solve of the ADR 0517 hub at 16 cells in under a
third of the present time; **c4** every reading within 1e-10 of the present driver; **c5** without the option every result
bit-identical. The brief adds: (b1) lift F-615's refusal of the supernodal factor with the total Lagrangian body and register
how the contact driver passes the choice; (b2) the pivot handling of the Newton driver (refused pivots, halvings) behaves
identically with the new factor, with a registered check (equal Newton iteration and halving counts); (b3) register how the
time is measured; (b4) opt-in, the default-bits record first, before library code; `baseline_hashes` unchanged; no case-file
key; (b5) the mutations the checks catch; (b6) every aggregated test error finite.

## What was read (read only)

ADR 0533 (F-615: D1, D2, finding 3 on Sylvester's law and the pivot test, the time protocol, the checks, N2 and the mutation M6,
whose log `f615-scratch/M6.log` shows the total Lagrangian hub at `n = 4` failing with "singular tangent: pivot -1.68e10 at node
387 component 0"), ADR 0517 (D3 the model, D4 the readings), ADR 0503 (D1 the dissections, D3 the pivot test of the supernodal
factor), ADR 0509 (`DirectFactor` as the one name of the factor), ADR 0404 D5 and ADR 0479 D2 (the Newton driver, the attempt,
`solve_from`), ADR 0507 (D3 the contact driver, amendment 2 the convex switch of the linear body). Code:
`crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `Problem::attempt_with`, `factor_solve`, `controlled_correction`,
`solve_with` with its halving loop, `solve_from`, `solve_from_field`), `crates/rcfd-fem/src/contact.rs` (`ContactOptions` and
`check`, `solve_with`'s total Lagrangian branch), `crates/rcfd-fem/src/arclength.rs` (`ArcLengthOptions::check`),
`crates/rcfd-fem/src/nl_transient.rs` (its Newton calls `solve_from_field`), `crates/rcfd-fem/src/solve.rs` (`DirectFactor`,
`PIVOT_TOLERANCE = 1e-10`), `crates/rcfd-linalg/src/skyline.rs` (`SkylineLdlt::factor`: reverse Cuthill-McKee, positive
pivots, symmetry tolerance 1e-12), `crates/rcfd-linalg/src/sparse_chol.rs` (`SupernodalLdlt::factor`: nested dissection by
level structures, positive pivots, symmetry tolerance 1e-12, `order()`, `pivots()`), tests `sa_contact_supernodal_f615.rs`,
`sa_contact_f615_default_bits.rs`, `si_nonlinear.rs` (A7/C3: halvings). Nothing outside the repository; no algorithm is new
here (the supernodal multifrontal factor and its dissection are ADR 0503's, cited there; the Newton driver is ADR 0404's).

Findings that shape the design:

1. Every Newton attempt without a load-factor control (`solve`, `solve_with`, `solve_from*`, `solve_in_time*`, the Newton of
   `nl_transient`, the total Lagrangian contact body) solves the reduced tangent by `factor_solve`: `SkylineLdlt::factor`, then
   the post-test that refuses a pivot `d_i <= PIVOT_TOLERANCE a_ii` (or NaN). A refusal of either is a `Failure::Other("singular
   tangent: ...")`: `solve_with` halves the increment, `solve_from` returns `FemError::Solver { solver: "newton" }`. The
   arc-length correction (`controlled_correction`) factors with `PivotRule::NonZero` and reads the inertia; it is a separate
   path.
2. The total Lagrangian contact body calls `solve_from` with `options.newton` and the contact set as a contribution; there is
   no convex switch (ADR 0507 amendment 2 is in the linear body only), so an indefinite tangent ends the solve.
3. As ADR 0533 finding 3: both factors' sign tests refuse exactly when a pivot is at or below a rounding threshold, and by
   Sylvester's law the number of negative pivots does not depend on the order; so for a tangent whose smallest eigenvalue is not
   within rounding of zero both factors refuse the same tangents. The post-test `d_i <= 1e-10 a_ii` depends on the order (the
   pivots differ); it can disagree only on a tangent nearly singular at the level `1e-10`.
4. With ADR 0517's interference `d = 1e-3 a` the total Lagrangian hub is refused at the first tangent by the present driver
   (Q1 below): the contact tangent at `u = 0`, with the whole interference as penetration, has a curvature part that makes it
   indefinite. The total Lagrangian hub therefore needs a smaller interference to be solved at all (D4).

## The probes (disclosed; every number used)

All in release, on this machine (16 logical processors, rayon's default pool of 16), with other agents' builds running. Logs in
`f618-scratch` (`q1_n4.log`, `q1_n8.log`, `q1_n16.log`, `q2_n16.log`, `q3_n16.log`, `q4_n16.log`, `q5.log`).

**Q1** (`probe_q1_tl_interference`): ADR 0517 D3's hub with `Kinematics::TotalLagrangian`, ADR 0517's options (one load step,
penalty `100 E/h`, tolerance `1e-6 d`, 30 iterations), the present driver, interference `d` in `{1e-3, 1e-4, 1e-5, 1e-6} a`.
The Newton record is (iterations, convex iterations) per Newton solve.

| `n` | `d / a` | result |
|---|---|---|
| 4 | 1e-3 | fails in 0.13 s: "load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot -1.68e10 at node 387 component 0 (diagonal 5.15e11)" |
| 4 | 1e-4 | converges, 2.76 s, `[(3, 0), (2, 0), (2, 0)]`; the linear body 6.52 s, the same record; total Lagrangian over linear minus 1: `p_mid` 2.5e-5, `B_0.25` 2.2e-5, `P_f` 1.5e-5 |
| 4 | 1e-5 | converges, 4.03 s, `[(4, 0), (2, 0), (2, 0)]`; against the linear body 2.5e-6, 2.2e-6, 1.5e-6 |
| 4 | 1e-6 | fails, 12.6 s: not converged in 30 iterations (relative residual 9.3e-10 above the tolerance 1e-10: a rounding floor) |
| 8 | 1e-4 | converges, 8.40 s, `[(3, 0), (2, 0), (2, 0)]` |
| 8 | 1e-5 | fails, 36.8 s: not converged in 30 iterations (relative residual 2.0e-10) |
| 16 | 1e-4 | converges, **34.30 s**, `[(4, 0), (2, 0)]` (12 133 nodes, 10 320 elements) |

**Q2** (`probe_q2_factor_times`, `n = 16`, `d = 1e-3 a`): the reduced linear stiffness `K` (34 154 free unknowns, 2 443 816
entries): supernodal 0.79 s (17 149 754 entries of `L`), envelope 3.56 s (envelope 33 660 946); both positive definite.
`K + K_c(u = 0)`: both factors refuse (supernodal: pivot -4.2e10 at elimination position 33 417; envelope: -1.05e10 at 5 156).

**Q3** (`probe_q3_iteration_costs`, `n = 16`, `d = 1e-4 a`, the total Lagrangian tangent at `u = 0` plus the contact tangent,
twice): tangent assembly 0.22 and 0.13 s, internal force 0.04 s, contact set 0.006 s (51 995 triplets), `add_triplets` 0.08 and
0.07 s (2 648 961 to 2 665 145 entries), reduction 0.04 and 0.02 s, envelope factor 5.58 and 5.91 s (envelope 40 283 762),
supernodal factor 1.76 and 1.72 s (20 249 750 entries of `L`), solves 0.05 s each. This tangent is positive definite in both.

**Q4** (`probe_q4_order_cost`, the same matrix, three times): the nested dissection order 0.18, 0.19, 0.13 s; the supernodal
factor with that order given 1.07, 0.96, 1.04 s; `SupernodalLdlt::factor` (order inside) 1.31, 1.13, 1.15 s, its pivots
bit-equal to those with the given order; the envelope factor 5.32, 4.65, 5.27 s.

**Q5** (`probe_q5.rs`): a Hex8 cantilever `10 x 1 x 1` of `40 x 4 x 4` cells, `E = 2e11`, `nu = 0.3`, clamped at `x = 0`, a dead
traction on `x = 10` with `P L^2/(E I) = 1`, `NewtonOptions::new(2)`, total Lagrangian (`nonlinear::solve`). Plain: tip
deflection 2.9338 (0.29 of the length), record (load factor, halvings, iterations) `[(0.5, 0, 6), (1.0, 0, 7)]`. With the
device of D5 (a tangent-only contribution `-10 K_ii` at the tip's `u_y` once its increment in the attempt exceeds `tau`): `tau =
0.35` of the plain tip deflection gives `[(0.25, 1, 5), (0.5, 1, 5), (0.75, 1, 6), (1.0, 1, 6)]` and the same tip deflection to
the last digit; with `max_halvings = 0` it fails with "load step 1 of 2 (load factor 0e0 to 5e-1) after 0 halvings: singular
tangent: pivot -4.22e11 at node 532 component 1"; `tau = 0.7` never acts.

Reading: at `n = 16` the factor is about 0.9 of the present total Lagrangian solve (6 factorisations, `6 x 5.0 s` of 34.3 s;
the other costs of an iteration are about 0.3 s, Q3). The supernodal factor costs 1.0 to 1.8 s on the same tangent.

## Design

### D1. The option (`crates/rcfd-fem/src/nonlinear.rs`)

`NewtonOptions` gains `pub factor: DirectFactor` (`rcfd_fem::solve::DirectFactor`, the one name of the factor, ADR 0509 D4 and
ADR 0533 D1) and the builder `factor(DirectFactor)`. `NewtonOptions::new` sets `DirectFactor::Envelope`, the present factor.
`DirectFactor::SupernodalNestedDissection` selects the supernodal factor (c2). `NewtonOptions::check` refuses any other value
(`SparseMinimumDegree`) with `FemError::InvalidValue { key: "solver", .. }` (not asked for; not tested beyond the refusal). No
case-file key (b4).

### D2. The factor (`factor_solve`)

`factor_solve` takes the choice. The `Envelope` arm is the present code, unchanged in text and order. The
`SupernodalNestedDissection` arm is `SupernodalLdlt::factor(a)` (nested dissection by level structures of the reduced tangent's
pattern, ADR 0503 D1; positive pivots and the symmetry tolerance 1e-12, the rule and tolerance of the envelope factor), the same
mapping of `SkylineError::Pivot` to "singular tangent: pivot .. at node .. component .. (diagonal ..)" and of any other error
to "tangent factorisation: ..", then the same post-test over `order()` and `pivots()` (`d_i` NaN or `<= PIVOT_TOLERANCE a_ii`
refused with the same text), then `solve`. A refusal is therefore the same `Failure::Other` in both arms, and the driver's
response (a halving in `solve_with`, an error in `solve_from`) does not depend on the factor (b2; finding 3 for when both
refuse). The order is computed inside each factorisation (Q4: 0.13 to 0.19 s of about 1.2 s; caching it by pattern is not
done). The choice applies to every attempt of finding 1 without a control. The arc-length path keeps the envelope:
`ArcLengthOptions::check` refuses a `newton.factor` other than `Envelope` (key `"solver"`), since its correction needs the
indefinite factor and the inertia (not in scope).

**Bits (c5).** Without the option the code path is the present one: the same factor call on the same matrix, the same test and
solve. Recorded by B (below) before any library code.

### D3. The contact driver (`crates/rcfd-fem/src/contact.rs`, b1)

`ContactOptions::check` no longer refuses `DirectFactor::SupernodalNestedDissection` with `Kinematics::TotalLagrangian`. With
the total Lagrangian body, `solve_with` passes to `solve_from` the Newton options `options.newton` with `factor` set to
`SupernodalNestedDissection` when `ContactOptions::factor` is that, and `options.newton` unchanged otherwise (built once before
the steps). So both `ContactOptions::factor(SupernodalNestedDissection)` and `ContactOptions::newton(NewtonOptions::new(1)
.factor(SupernodalNestedDissection))` select it (check C1). With the linear body the factor is `ContactOptions::factor` (ADR
0533); a `newton.factor` other than `Envelope` is refused there with the key `"solver"` (it would be ignored otherwise). No
other line of the contact driver changes.

**The F-615 test.** `sa_contact_supernodal_f615.rs::f615_n2_refusals` asserts, in its second half, the refusal that b1 lifts.
House rule 2 says existing tests are not edited; the brief asks for the lift. Taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation): the one attribute line `#[ignore = "superseded by ADR 0542 ..."]` is added to that test,
its body unchanged as a record, and its first half (the envelope refused) is repeated in the new test file (N2a). This is the
only edit of an existing test file, disclosed here.

### D4. The model of the total Lagrangian hub (c3, c4)

ADR 0517 D3's hub at `n = 16` with `Kinematics::TotalLagrangian` and the interference **`d = 1e-4 a`**; everything else is ADR
0517's (`L = 4 (b - a)`, `e = 2 a`, `nu = 0.3`, `E = 2e11`, the offset 0.05, penalty `100 E/h`, one load step, tolerance `1e-6
d`, `NewtonOptions::new(1).max_iterations(30)`). Why not ADR 0517's `d = 1e-3 a`: the present driver refuses its first tangent
(finding 4, Q1), so there is no present time and no present reading to compare with. `1e-4 a` is the largest of the probed
powers of ten with which the present driver converges at `n = 4`, `8` and `16` (Q1); the readings are ratios to `p_0`, linear in
`d` to `2.5e-5` (Q1). ADR 0517's `d = 1e-3 a` stays in the checks as the refused-pivot case P (both factors must refuse it the
same way).

### D5. The halving device (b2)

The halving check needs a Newton attempt whose tangent is refused for a large increment and accepted for a smaller one. A
test-only contribution (`Spoiler`) adds no force and, at one equation `q`, the tangent entry `-10 K_qq` (`K` the linear
stiffness) whenever `|u_q - u_q,start| > tau`, where `u_q,start` is `u_q` at the first call with the attempt's load factor (the
iteration 0 of every attempt is at the committed state). The equilibrium is the device-free one; only the Newton path changes.
A tangent with `e_q^T K_t e_q < 0` is indefinite, so both factors refuse it (finding 3). On Q5's cantilever with `tau = 0.35`
of the plain tip deflection every load step halves once (Q5).

## How the time is measured (c3, b3)

The heavy record `f618_tl_hub_n16_supernodal_against_present_heavy` builds D4's model once and calls `contact::solve` six times
in one test process, alternating: present, supernodal, present, supernodal, present, supernodal (the supernodal by
`ContactOptions::factor(SupernodalNestedDissection)`). Each `t` is the wall time of that one call (`std::time::Instant` before
and after; the model build and the readings outside). All six run in rayon's global pool (its size printed; 16 here) with
`NewtonOptions::threads = None`. The ratio is **`median(t_s) / median(t_p)`** over the three of each, fixed now; the three pair
ratios are recorded. The run is made once, alone (no other test of this agent running), started when the free physical memory is
at least 4 GB (estimate of the peak: about 1.5 GB; the envelope factor is 0.32 GB of values, Q3). Other agents' work on the
machine is not controlled; that is a limit of the measurement, and the median of three is the registered defence against it.
`t_p` is also compared with Q1's 34.3 s (recorded, no limit).

## Checks (limits fixed now)

Readings (c4) are those of ADR 0517 D4, by its `read` (copied from `sa_contact_supernodal_f615.rs`): `p_mid`, `B_0.25`, `B_0.5`,
`P_f`, `q_0.25`, `q_0.5`, every row's `F_k/A_k`, the largest node pressure and its row, and E1's flag. The difference of a reading
is `abs(x_s/x_p - 1)` with `x_p` the present driver's. The aggregate over rows is `worst` (infinite if any value is not finite,
b6). A non-finite value is a miss.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| T1 | c3 | heavy, `n = 16`, `d = 1e-4 a`: `median(t_s) / median(t_p)` | `< 1/3` | Q1, Q3, Q4: present about `6 x 5.0 + 8 x 0.3 = 32` s (Q1: 34.3 s); supernodal about `6 x 1.2 + 8 x 0.3 = 9.6` s: **about 0.30**, between 0.26 and 0.37 over Q3's and Q4's spread of the supernodal factor; a real risk of a miss |
| V1 | c4 | heavy, `n = 16`, the first pair: each of `p_mid`, `B_0.25`, `B_0.5`, `P_f`, `q_0.25`, `q_0.5`, the largest node pressure, and the worst over the rows | `<= 1e-10` each | ADR 0533: `8.1e-12` at worst for the linear body at `n = 32` |
| V1e | c4 | heavy, `n = 16`: the row of the largest node pressure; E1's flag | equal | ADR 0533 |
| V2 | c4 | fast, `n = 4` and `n = 8`, `d = 1e-4 a`: as V1 and V1e | as V1 and V1e | ADR 0533: up to `2.4e-12` |
| V3 | b2 | `n = 4`, `8` (fast) and `16` (heavy): the Newton record (Newton solves, iterations of each, convex iterations), supernodal against present | equal | finding 3; Q1 records |
| U1 | c4 | `n = 4`, `8`, `16`: `max abs(u_s - u_p) / max abs(u_p)` | `<= 1e-10` | ADR 0533: `1.4e-14` |
| H1 | b2 | fast, Q5's cantilever with D5's device (`tau = 0.35` of the plain present tip deflection, the same `tau` for both): per converged increment (step, load factor bits, halvings, iterations), supernodal against present | equal | finding 3; D5 |
| H2 | b2 | the same: `max abs(u_s - u_p) / max abs(u_p)` | `<= 1e-10` | well-conditioned solid; ADR 0533 |
| H3 | b2 | the same with `max_halvings = 0`, both factors: `FemError::Solver { solver: "newton" }` whose reason contains "after 0 halvings: singular tangent: pivot" | holds for both | Q5 |
| P | b2 | fast, the total Lagrangian hub at `n = 4` with ADR 0517's `d = 1e-3 a`, both factors: `FemError::Solver { solver: "contact" }` whose reason starts with "load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot" | holds for both | Q1, Q2 (both refuse `K + K_c(0)` at `n = 16`) |
| C1 | b1 | fast, `n = 4`, `d = 1e-4 a`: the solve with `ContactOptions::factor(SupernodalNestedDissection)` and the one with `ContactOptions::newton(... .factor(SupernodalNestedDissection))`: the digest of B's `flat` | equal | D3 |
| D | D2 | heavy: the supernodal `n = 16` solve inside a rayon pool of 1 thread against the first supernodal solve (16 threads): B's `flat` | bit for bit | ADR 0503 D3, ADR 0533 D |
| R2 | b3 | heavy: the three present solves bit-equal to each other, and the three supernodal ones | equal | deterministic drivers |
| B | c5, b4 | `tests/sa_newton_f618_default_bits.rs`, run on 498b5417 plus this ADR plus the file, before any library code; its digests written into the file; after the change equal. Cases: **G1** Q5's cantilever, the plain solve and the solve with D5's device (displacement, reactions, per increment the step, load factor, halvings, residuals and energies); **G2** the total Lagrangian hub at `n = 4`, `d = 1e-4 a` (ADR 0533's `flat`: displacement, reactions, every contactor node's force, pressure, gap and local coordinates, every Newton residual, energy, penetration, convex count and active set); **G3** the error text of the total Lagrangian hub at `n = 4`, `d = 1e-3 a` (its bytes). Also `sa_contact_f615_default_bits.rs`, `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs` and every other test of `cargo test -p rcfd-fem --release` (the nonlinear default-bits records among them) pass unchanged, and `cargo test -p rcfd-api --release --test baseline_hashes` | equal; pass; pass | D2 |
| R | b3 | heavy: every `t_p` and `t_s`, the three pair ratios, the thread count, `t_p` against Q1's 34.3 s | recorded | |

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | V2: the supernodal displacement at `n = 4` and at `n = 8` is not bit-equal to the present one; and H1's supernodal displacement not bit-equal to the present one | holds (the option took effect) |
| NH | H1: the present run halves at least once | holds (the device acts) |
| N2 | (a) `contact::solve` with `ContactOptions::factor(DirectFactor::Envelope)`; (b) `nonlinear::solve` with `NewtonOptions::factor(DirectFactor::SparseMinimumDegree)`; (c) `contact::solve`, linear body, with `newton.factor(SupernodalNestedDissection)`; (d) `arclength::trace` with `newton.factor(SupernodalNestedDissection)`: each `FemError::InvalidValue` with key `"solver"` | holds |

### Planned mutations

On a `git archive` copy of the final tree in `f618-scratch/mut` with its own target directory `target-f618-mut`; each alone, by
exact text replacement; the fast test files (`sa_newton_supernodal_f618.rs` without the heavy record, and
`sa_newton_f618_default_bits.rs`) run once; the file restored.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | the supernodal arm of `factor_solve` builds `SkylineLdlt::factor` | N1 |
| M2 | the envelope arm builds `SupernodalLdlt::factor` | B (G1, G2) |
| M3 | the supernodal arm factors with `PivotRule::NonZero` (accepting an indefinite tangent) | H1 (the device's tangents accepted) |
| M4 | the supernodal arm maps a `SkylineError::Pivot` to the text of the other errors ("tangent factorisation: ..."), not "singular tangent" | H3 |
| M5 | the contact pass-through dropped (`solve_from` gets `options.newton` unchanged) | C1 |
| M6 | `ContactOptions::check` keeps F-615's refusal of the supernodal factor with the total Lagrangian body | V2 (the solve refused) |
| M7 | `NewtonOptions::check` accepts `SparseMinimumDegree` | N2 (b) |
| M8 | `ArcLengthOptions::check` accepts the supernodal factor | N2 (d) |
| M9 | `ContactOptions::check` accepts `newton.factor` other than `Envelope` with the linear body | N2 (c) |

## Tests (new files, and one attribute line)

- `crates/rcfd-fem/tests/sa_newton_f618_default_bits.rs`: B (G1 to G3), written and committed with its digests before any
  library code.
- `crates/rcfd-fem/tests/sa_newton_supernodal_f618.rs`: ADR 0517's model and `read` (copied); `f618_v2_tl_hub_n4_n8` (fast: V2,
  V3, U1, N1, C1), `f618_h_halvings_cantilever` (fast: H1 to H3, N1, NH), `f618_p_refused_pivot_tl_hub` (fast: P),
  `f618_n2_refusals` (fast: N2), and the heavy record `f618_tl_hub_n16_supernodal_against_present_heavy` (`#[ignore]`, about 3
  minutes, run once, alone: T1, V1, V1e, V3, U1, D, R2, R).
- `crates/rcfd-fem/tests/sa_contact_supernodal_f615.rs`: the attribute line of D3 on `f615_n2_refusals`.

## Decision rule

**Done** if every check meets its limit on its registered run (the heavy record included), every negative control holds, and
every mutation is caught by a check named for it on its first run. **Partial** if a check misses or is not run, a mutation
survives or is caught only by another check, or a check is met only after an amendment made after its run. **Todo** if the
code does not build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- ADR 0517's interference `1e-3 a` is not solved by the total Lagrangian body by either factor (finding 4); the timed hub uses
  `1e-4 a` (D4). A convex switch for the total Lagrangian body would change the present driver's results (an error becomes a
  solution): not in scope.
- The paths other than the contact body and `nonlinear::solve` (`solve_in_time*`, the nonlinear transient's Newton) take the
  option through `factor_solve` but are not checked beyond their existing default-bits records. The arc-length path refuses it.
- The order is recomputed at every factorisation, not cached by pattern (Q4: about 0.15 s of 1.2 s).
- The time is three pairs of runs on a shared machine.
- Whether the supernodal factor becomes the Newton driver's default changes default results: not taken, open. No case-file key.

## Open questions

1. **Case-file key.** Not added (it changes the schema golden). As ADR 0533 open question 1: one key `solver.factor` read by
   every analysis is recommended, after the maintainer approves the golden change. Not taken.
2. **`SparseMinimumDegree` in the Newton driver.** Refused here (not asked for). Taken on the maintainer's standing instruction
   of 2026-11-11 (take the recommendation): refuse rather than add an unchecked factor.
3. **Default.** Making the supernodal factor the Newton default changes default results: not taken, open.
