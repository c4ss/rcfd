---
type: source
tags: [adr-thread, compressible, wet-steam, condensation, nucleation, mutation-testing]
sources: [doc-reference-data-f221-condensing-steam]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: non-equilibrium condensing (wet) steam in a transonic nozzle

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registration and a follow-up of its surviving mutations), 2 ADRs, 2026-11-07 to 2026-11-08.

F-221 (ADR 0217) built non-equilibrium condensing steam in a transonic nozzle in the library: inviscid, with a perfect-gas vapour, compared against nozzle data (see [[doc-reference-data-f221-condensing-steam]]). The acceptance comparison V1 was met, but two mutations (MU3, MU6) survived, so by its rule F-221 stayed todo. F-320 (ADR 0226) registered eight checks that catch them, and applied ADR 0217's rule again as a recommendation: partial.

## Current state

- **F-320 done; F-221 todo on the roadmap** ([[roadmap-compressible-flow-and-acoustics]]). ADR 0226 recommends **F-221 partial** ("F-221's roadmap entry is not changed on this branch"), with the remaining part named: case keys (F-316), real-gas vapour (F-317), viscous terms (F-318), polydispersion (F-319) and the coarse-grid unsteadiness (ADR 0217 Q10). "Done stays the maintainer's call."
- **What holds** (ADR 0226 addendum): K' met; every mutation MU1 to MU6 caught by the fast checks of K'; F missed only in part (S1n on the coarse 230 x 10 run, `8.18e-6` at its cap); V1 (a) and (b) met, "worst -4.932 percent, onset +1.90 percent".
- **Limits of the model** (ADR 0217): library level, inviscid, perfect-gas vapour; V1 met "by a margin of 0.07 percentage points at the last station".

> [!warning] Contradiction
> `docs/roadmap.json` keeps F-221 at `todo` ([[doc-roadmap-json]]), while ADR 0226 (F-320 done) applies ADR 0217's rule again and recommends partial. Within the repo docs: the ADR leaves the status change to the lead, and the roadmap has not taken it.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0217](../../docs/adr/0217-f221-registration-non-equilibrium-condensing-steam-in-a-transonic-nozzle.md) | 2026-11-07 | registration (one amendment) | F-221 | Non-equilibrium condensing steam in a transonic nozzle | todo by the rule: MU3, MU6 survived, S1n missed on the coarse run; V1, V2 met | 0226 |
| [0226](../../docs/adr/0226-f320-registration-wet-steam-checks-for-mu3-and-mu6.md) | 2026-11-08 | registration (follow-up checks) | F-320 | Checks that catch the nucleated-mass term (MU3) and the wave-speed sound speed (MU6) | done; recommends F-221 partial | |

## Key numbers and limits

- ADR 0226: MU3 caught by R1 and L2, MU6 by A1, A2, U1 and U2; MU6a and MU6b each caught.
- ADR 0217 Q9 (a): a check that isolates the nucleated-mass term (the `rates()` decomposition at the M1 states) and a check of the HLLC wave-speed bounds against the frozen sound speed.

## Open questions

- ADR 0217 Q1: a `[wet_steam]` optional table under the compressible algorithm (after Q2).
- ADR 0217 Q2: real-gas vapour ([S4]-type virial or IF97 region-2 tables, F-220 `PtTable`) needs a general EOS path; "F-221 not done before it".
- ADR 0217 Q3: boundary layers, once a compressible Navier-Stokes path exists.
- ADR 0217 Q10: a study of the coarse-grid condensation front. ADR 0226 Q1: the reading of the unwritten case of ADR 0217's rule.

## Related

- [[doc-reference-data-f221-condensing-steam]], [[doc-reference-data-real-gas-eos]], [[doc-case-schema-compressible]], [[mutation-testing]]
- [[adr-thread-compressible-acoustic-boundaries]], [[roadmap-compressible-flow-and-acoustics]]
