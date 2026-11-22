---
type: source
tags: [adr-thread, numerics, linear-solver, amg, dense-lu, fingerprints]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: AMG definiteness and the order of the coarsest dense LU

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (one diagnosis, one default switch, two registrations of moved fingerprints), 4 ADRs, 2026-11-12 to 2026-11-20.

The immersed DFG 2D-2 run on the mesh T stopped at step 2951 inside the AMG-preconditioned pressure CG, with $r^T z < 0$ from iteration 1 (F-466, ADR 0363, see [[adr-thread-immersed-solid]]). F-474 (ADR 0369) located the cause in the scalar AMG of `rcfd_linalg::amg`: the dense LU of the coarsest level applied its row interchanges in the wrong order whenever a pivot interchange happened at a step $k \ge 1$, so the coarse solve was wrong, the V-cycle became non-symmetric and indefinite, and CG broke down. The same defect had been found and fixed in `amg_elastic.rs` by ADR 0305 amendment 1 (F-402); its question 1 became F-471. See [[amg]].

## Current state

- **F-471 is done** (roadmap): the corrected order ("row interchanges applied before the forward substitution") is the default of the scalar AMG since ADR 0743, after the maintainer's answer of 2026-11-18 ("The default change is approved", written into ADR 0369).
- **The switch moved registered values, each approved one by one.** ADR 0743 stopped at FP4 of ADR 0374 N0 (registered `0x279c4c4ed4cf89be`, new `0xda894fae1d52a3ad`); the maintainer answered "Approve the move" on 2026-11-19 (option (a); option (c), an approval in advance of further fingerprints, "was not given"). ADR 0747 restated that test. ADR 0748 restated nine overset tests ("Approve the moves", 2026-11-20), then stopped at two further values and the store hash of `cases/axial_fan_cgrid/axial_fan_cgrid.toml`; the maintainer answered "Approve all three" (2026-11-20). ADR 0748 addendum 2 (written by the lead) restated them; decision "**done** for this registration".
- **A way back exists:** the old order reproduces every old value bit for bit (ADR 0748 W1 to W9, S3 through `simple_solver_mut`); ADR 0748 adds a way back to the old order for the overset solver.
- **F-474 is partial** (roadmap): "The default is not switched: AMG-CG converges on the stopping system only with the opt-in AmgSettings::coarse_lu_interchanges_first". See the contradiction below.

> [!warning] Contradiction
> The roadmap gap of F-474 (`docs/roadmap.json`, [[doc-roadmap-json]]) still says "The default is not switched ... Open: switching the default (maintainer; recommended)". ADR 0743 (F-471, done) switched the default of the coarsest dense LU to the corrected order after the maintainer's approval of 2026-11-18, which is the repair F-474 asked for. Within the repo docs: the F-474 gap text is older than ADR 0743.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0369](../../docs/adr/0369-f474-registration-amg-definiteness-and-coarse-lu-order.md) | 2026-11-12 | diagnosis + opt-in | F-474, F-471 | Why AMG-CG loses definiteness on T; is the coarsest dense LU order wrong | partial (both): cause located (interchanges at steps 13, 16, 17; coarse residual `0.213`); opt-in repair converges in 10 iterations; L4 missed (`l4_..._known_miss`) | 0743 |
| [0743](../../docs/adr/0743-f471-registration-the-corrected-order-of-the-coarsest-dense-lu-as-the-default.md) | 2026-11-19 | registration (default switch) | F-471 | The corrected order as the default | partial, work stopped at FP4 of ADR 0374 N0 | 0747, 0748 |
| [0747](../../docs/adr/0747-f471-registration-the-moved-fingerprint-fp4-of-adr-0374-under-the-new-default.md) | 2026-11-19 | registration (moved fingerprint) | F-471 | Restate FP4 under the new default | met; no other registered value moved; four long files pass | |
| [0748](../../docs/adr/0748-f471-registration-the-moved-fingerprints-of-the-overset-solver-under-the-new-default.md) | 2026-11-20 | registration (moved fingerprints) | F-471 | Restate nine overset fingerprints | addendum 1 partial (two further values, N7 and N9); addendum 2 done after "Approve all three" | |

## Key numbers and limits

- ADR 0369 H0: the stop at step 2951; default `rz` after iteration 1 `-2.006e-9` (start `+5.52e-9`; `rz <= 0` at 2616 of 3001 iterations).
- ADR 0369 H1: coarsest level 24 unknowns, pivots `9.4e-10` to `2.3e-5`; interchanges at steps 13, 16, 17 (`S`), 14, 16, 17, 20, 21 (`G`, step 2950).
- ADR 0369 H2: coarse-solve relative residual, opt-in at most `5.3e-15`, default `0.213` on the received vector (`0.083` to `0.86` on the ten others).
- ADR 0369 H3: default cycle has a negative minimum quotient on level 4 (`-7.0e8`) and level 0 (`-3.4e7`), asymmetry `4.2e-3` to `0.13`; opt-in all quotients positive, asymmetry at most `1.2e-15`. H4: operators, prolongators and the coarsest Cholesky pivots are sound.
- ADR 0369 L4 (missed): the 40 x 40 Laplacian interchanges at step `[0]`, so "no interchange" was not reached; L4b (amendment 1, "subset of `{0}`") met.
- ADR 0743 A1: on the baseline set, 119 factorisations in 12 case runs, none with an interchange at any step; A2: the 20 x 20 tridiagonal solves to `8.863e-19` (old order `4.071e-3`).
- ADR 0748 store hash of `axial_fan_cgrid`: `0xe4539468a88f2746` to `0x03d18f072e9caca5`.

## Open questions

- ADR 0369 Q4: the `rz <= 0` guard in `cg` (option (a) taken on the standing instruction: only by an amendment; (b) "changes default solves (ADR 0363 Q6) and is the maintainer's").
- The F-474 roadmap gap still lists the default switch as open (see the contradiction above).

## Related

- [[amg]], [[rcfd-linalg]], [[bit-identical]], [[adr-thread-immersed-solid]], [[adr-thread-overset]]
- [[roadmap-numerics-and-solvers]]
