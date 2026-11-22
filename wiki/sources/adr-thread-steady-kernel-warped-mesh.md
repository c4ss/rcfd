---
type: source
tags: [adr-thread, numerics, simple, pressure-interpolation, non-orthogonal, warped-mesh]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: pressure-interpolation variants and the steady kernel on the warped mesh W

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registration, code-change record, diagnosis with an opt-in fix), 3 ADRs, 2026-11-06.

F-246 (ADR 0193) added two pressure-interpolation variants to the steady SIMPLE kernel, `skew_corrected` (V1) and `body_force_weighted` (V2), under the case key `solver.pressure_interpolation` (default `linear`). Its warped-mesh clause failed for a reason outside the variants: the default kernel and both variants diverge on F-208's warped mesh W from `n = 32`. F-285 (ADR 0198) diagnosed that divergence in the deferred-correction pressure step and built an opt-in fix, `SimpleSolver::implicit_non_orth_pressure`. ADR 0196 records one decided code change of F-246 (its R2 record). See [[simple]] and [[mesh-quality]].

## Current state

- **F-285 done**, **F-246 partial** ([[roadmap-numerics-and-solvers]]). F-246 gap: "the default and both variants diverge on F-208's warped mesh W from n = 32, so the warped-mesh clause is not met ... body_force_weighted raises near-orthogonal velocity error 1.342x (G) and 1.150x (G3) vs limit 1.10. Open ADR questions Q1 to Q7 remain."
- **Decision record 2026-11-06** (ADR 0193): Q1 to Q7 "Not asked"; Q8 answered (a): W stays the binding warped mesh, W2 is supplementary; "F-246 stays partial: M1 on W and M2 for `body_force_weighted` are not met"; Q9 answered (b): the new item F-285, after which M1 on W (`m1_w_v1_known_miss`, `m1_w_v2_known_miss`) is run again unchanged.
- **The fix (ADR 0198 amendment 2):** I-R, "the pressure equation with its non-orthogonal part solved implicitly, with the existing `face_flux` cell correction", behind the public field `SimpleSolver::implicit_non_orth_pressure: bool` (default `false`). It keeps the fixed point but changes every iterate, so it stays opt-in (Q3); library only, no case key (Q2 (a)).
- **Mechanism:** on W at `n = 32` the deferred-correction step `X_p` of the pressure equation has an eigenvalue of about `-1.18` (`rho` `1.1781`, `c` `-1`), and "A real eigenvalue below -1 cannot be damped by any relaxation of that iteration". The modes sit at interior points where the map compresses cells to `0.37` of their size along `(1, 1, 1)`, not at the edges (H3 falsified).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0193](../../docs/adr/0193-f246-registration-pressure-interpolation-variants.md) | 2026-11-06 | registration + decision record | F-246 | Pressure-interpolation variants V1, V2 of the steady kernel | partial; M1 on W and M2 for V2 known misses; Q9 opened F-285 | 0198 |
| [0196](../../docs/adr/0196-three-code-changes-decided-2026-11-06-f246-r2-record-design-tool-class-sv202-text.md) | 2026-11-06 | code-change record | F-246 (and two tool/setup-rule changes) | Build three changes decided on 2026-11-06: the F-246 R2 record, the class of two design tools in project mode, SV202's text per solver | A, B, C built as registered; no golden edited | |
| [0198](../../docs/adr/0198-f285-registration-default-steady-kernel-on-the-warped-mesh-w.md) | 2026-11-06 | diagnosis + opt-in fix | F-285 | Why the default steady kernel diverges on W, and a fix | done; fix I-R converges W at `n = 16, 32, 64` | |

## Key numbers and limits

- ADR 0198 A1: with the fix, W converges in 218 and 716 iterations at `n = 16, 32` (`e_u` `2.3792e-3`, `6.1113e-4`), and in 2489 iterations at `n = 64` (5241 s; `e_u` `1.5342e-4`, `e_p` `5.2401e-4`).
- ADR 0198 V0 orders with the fix (reported): velocity `1.961` / `1.994`, pressure `1.632` / `1.785`.
- ADR 0198 A2: where the default converges, the fix leaves the converged solution unchanged to `3e-12` (limit `1e-8` on cell differences).
- ADR 0198 H2: `rho(Pi_GG, W32)` `1.1991`, `1.3200` at `n = 64`; on G, K, W2 at `n = 32` and W at `n = 16` it is `1.0000`.
- ADR 0198 Q5: the momentum's deferred step `X_u` is `1.0525` on W at `n = 32`, "above 1, without stopping convergence here".
- ADR 0193: V2 velocity error `1.342x` (G) and `1.150x` (G3) against the `1.10x` limit.

## Open questions

- ADR 0193 Q1 to Q7: not asked, standing on the reversible choices (one enum key; steady kernel only; V2 refused with `wall_pressure_trapezoid` sources; readings of the acceptance).
- ADR 0198 Q3: the fix as the default (needs the bit-identity rule lifted). Q4: I-R or I-GG (recommendation keep I-R). Q5: the momentum's deferred step.

## Related

- [[simple]], [[mesh-quality]], [[doc-case-schema-pressure-interpolation]], [[doc-case-schema-rhie-chow]], [[bit-identical]]
- [[adr-thread-gradient-options]], [[roadmap-numerics-and-solvers]]
