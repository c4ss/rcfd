---
type: source
tags: [adr-thread, structural, fem, contact, interference-fit]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, contact and interference fits

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 3 ADRs in this half, all 2026-11-14. The contact follow-ups (F-600 Newton order on short solves, F-601 bucket search and rigid targets, F-608 lifted contactor and line search, F-615, F-618 the supernodal factor in the contact drivers, F-623 mortar contactor, F-638 pressure recovery, F-639, F-684) are in the other half of the category and belong on this page.

Frictionless contact in [[rcfd-fem]]: node-to-surface contact with an augmented Lagrangian on rigid and deformable targets and interference (press) fits (F-427), with two re-registrations of the interference-fit clauses (F-599, F-604).

## Current state

- `contact.rs` (F-427, ADR 0507): frictionless node-to-surface contact on bilinear Hex8 target faces or a rigid plane, closest-point projection with the exact gap gradient and second derivative, a per-node augmented Lagrangian force $\max(0, \lambda - \varepsilon A g)$ with Uzawa updates, a linear-body Newton driver (sparse $L D L^T$, $\varepsilon G G^T$ on a refused pivot) and a total Lagrangian body. **Partial**: the uniform shaft stress of the interference fit missed (7.0e-3 against 1e-6) and the punch residual order missed (least 0.17 against 1.8).
- F-599 (ADR 0513) explains the through-thickness variation of the interference-fit contact pressure and re-registers I3 as met (done). F-604 (ADR 0517, press fit of a finite hub on a longer shaft, edge pressure peak, bore-cell rule) is partial: the plane-strain clause misses at 8.6e-2 against 2e-2, as derived before the run.
- "No [contact] case-file table exists, and the maintainer must decide on it" (F-427 gap, [[roadmap-structural-solver]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0507](../../docs/adr/0507-f427-registration-frictionless-contact-augmented-lagrangian.md) | 2026-11-14 | registration (hub) | F-427 | Frictionless node-to-surface contact, augmented Lagrangian, rigid and deformable targets, interference fits | partial; shaft stress 7.0e-3 against 1e-6, punch order 0.17 against 1.8 | 0513 (I3); F-600 (order) |
| [0513](../../docs/adr/0513-f599-registration-interference-fit-through-thickness-variation.md) | 2026-11-14 | re-registration | F-599 | Through-thickness variation of the interference-fit pressure, I3 re-registered | done | |
| [0517](../../docs/adr/0517-f604-registration-press-fit-of-a-finite-hub-on-a-longer-shaft.md) | 2026-11-14 | registration | F-604 | Press fit of a finite hub on a longer shaft, edge pressure peak, bore-cell rule | partial; plane-strain clause 8.6e-2 against 2e-2 (predicted) | |

## Key numbers and limits

- F-427 gap: "Uniform shaft stress in the interference fit missed (7.0e-3 against 1e-6); the note says F-599 explains and re-registers it as met. The total Lagrangian punch residual order missed (least 0.17 against 1.8); F-600 reads it as a bound, met" ([[roadmap-structural-solver]]).
- F-604 gap: "The plane-strain clause misses at 8.6e-2 against 2e-2, as derived before the run: the hub and shaft each have zero axial resultant, so the mid-length pressure follows the free-ended closed form (2.1e-3)".

## Open questions

- F-427: a `[contact]` case-file table (maintainer).
- F-604: "restate clause (a1) against the free-ended form, which would make it done" (maintainer).

## Second half of the category (ADRs 0519 to 0615, job p3-structural-b)

### Current state (second half)

- **Newton order on short contact solves** (F-600, ADR 0519, done): on the total Lagrangian punch of ADR 0507 T2 every pair of consecutive residuals of one Newton solve whose correction did not change the active set satisfies $r_{k+1} \le \max(10 r_k^2, 10^{-12})$; T2 stays an ignored record.
- **Search, targets and quadratic faces** (F-601, ADR 0540, partial): a bucket search, rigid cylinder and sphere targets, Hex20 and Tet faces; the Hertz checks HS20 and HS10 with quadratic contactors on curved targets are known misses (over-constraint of point-wise gaps).
- **Mortar contactor** for Hex20 and Tet10 faces, `ContactOptions::quadratic_contactor(QuadraticContactor::Mortar)` (F-623, ADR 0551, partial): patch tests PT10 and PT20 within `6e-14`; HS20m/HS10m (contact radius) and HS20t/HS10t (total Lagrangian solve) known misses. The pressure recovery `ContactSet::recovered_tractions` meets HS20r and HS10r (F-638, ADR 0571, done). The opt-in curvature-free fallback tangent of the total Lagrangian body (`ContactOptions::curvature_fallback`, F-639, ADR 0570, done).
- **Lifted contactor and line search** (F-608, ADR 0607, done): the refused pivot is diagnosed (the first correction throws the lifted half 33 to 71 gaps into the plate); "the convergence comes from the line search; the convexified tangent of the title (ADR 0570) is switched on in L1 but never taken". On the other total Lagrangian cases (F-684, ADR 0615, done) the line search solves the T2 punch and both Hertz cases on its own, the fallback solves the ADR 0542 hub P on its own; no case needs both and none gets worse with both.
- **The supernodal factor** in the linear contact driver (F-615, ADR 0533) and in the Newton driver for the total Lagrangian contact body (F-618, ADR 0542; cached analysis F-624, ADR 0566) are on [[adr-thread-structural-linear-solvers]].
- **Open at the snapshot:** making the line search and the curvature fallback the default of the total Lagrangian contact body (ADR 0615 Q1, recommendation (b) by a superseding ADR, a maintainer decision because refusals become solutions); a `[[contact]]` case-file key with `target = { sphere | cylinder }` (ADR 0540 Q1, changes the schema golden).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0519](../../docs/adr/0519-f600-registration-contact-newton-order-reading-for-short-solves.md) | 2026-11-14 | registration | F-600 (done) | Contact Newton order reading for short solves on the TL punch of ADR 0507 T2 | done: $r_{k+1} \le \max(10 r_k^2, 10^{-12})$ met on every solve; T2 stays an ignored record | — |
| [ADR 0540](../../docs/adr/0540-f601-registration-contact-search-rigid-cylinder-and-sphere-targets-hex20-and-tet-faces.md) | 2026-11-15 | registration | F-601 (partial) | Bucket search, rigid cylinder/sphere targets, Hex20 and Tet faces | partial: HS20, HS10 known misses | ADR 0551 |
| [ADR 0551](../../docs/adr/0551-f623-registration-mortar-contactor-for-quadratic-faces.md) | 2026-11-15 | registration | F-623 (partial) | Mortar contactor for Hex20 and Tet10 faces | partial: HS20m/HS10m, HS20t/HS10t known misses | ADR 0570, ADR 0571 |
| [ADR 0570](../../docs/adr/0570-f639-registration-curvature-free-fallback-of-the-total-lagrangian-contact-body.md) | 2026-11-15 | registration | F-639 (done) | Curvature-free fallback tangent of the TL contact body | done (opt-in) | ADR 0607, ADR 0615 |
| [ADR 0571](../../docs/adr/0571-f638-registration-pressure-recovery-of-the-mortar-contactor.md) | 2026-11-15 | registration | F-638 (done) | Pressure recovery of the mortar contactor | done (HS20r, HS10r met) | — |
| [ADR 0607](../../docs/adr/0607-f608-registration-total-lagrangian-contact-with-a-lifted-contactor.md) | 2026-11-15 | registration | F-608 (done) | TL contact with a lifted contactor: convexified tangent, line search on the energy | done (convergence from the line search) | ADR 0615 |
| [ADR 0615](../../docs/adr/0615-f684-registration-line-search-on-the-other-total-lagrangian-contact-cases.md) | 2026-11-15 | registration | F-684 (done) | Line search on the other TL contact cases (ADR 0542 P, ADR 0551 HS20t/HS10t) | done | — |

### Key numbers and limits (second half)

- Short-solve order bound: $r_{k+1} \le \max(C r_k^2, F)$, $C = 10$, $F = 10^{-12}$ (ADR 0519).
- Mortar patch tests PT10, PT20 within `6e-14` (ADR 0551).

### Open questions (second half)

- Defaults of the total Lagrangian contact body: line search and curvature fallback (ADR 0607 Q1, ADR 0615 Q1); the maintainer's decision.
- `ContactNode::pressure` for mortar nodes from the recovered traction (ADR 0571 Q1): recommendation keep `t_a/A_a` until the mortar contactor becomes a default (ADR 0551 open question 3, the maintainer's call).
- The line search on the transient and quasi-static drivers is reached but unchecked (ADR 0607 Q3).
- Contact case-file key (ADR 0540 Q1).

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-materials-nonlinear]] (the Newton driver the contact body runs on)
- [[adr-thread-structural-linear-solvers]] (sparse $L D L^T$)
