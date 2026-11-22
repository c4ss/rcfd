---
type: source
tags: [numerics, simple, piso, symmetry, decision-analysis]
sources: []
origin: docs/component-diagonal-default.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# `solver.component_diagonal = "corrected"` as the default: what would change

**Origin:** [docs/component-diagonal-default.md](../../docs/component-diagonal-default.md) at `origin/main` 2e94e5ef
**Kind:** analysis record for a maintainer decision (218 lines), with updates 2026-10-19, 2026-11-14, 2026-11-15, 2026-11-16

## Summary

What would change if the corrected component diagonal became the default for the segregated kernels. "Nothing here changes the default: `averaged` stays the default and no baseline hash was re-recorded." The decision is the user's (ADR 0074 part C, wave 34C).

## Key points

- Mechanism: `a_P` is the mean of the component diagonals; where a condition acts on one component only (`symmetry`, `shear`, `partial_slip`, `marangoni`) the converged tangential velocity carries an extra sink `(a_P - a_t) u_t`, "like a partial no-slip wall". `corrected` adds `(a_P - a_k) u_k` to `H_k` (ADR 0010), removing it exactly.
- Kernel table: SIMPLE/SIMPLEC averaged by default (correction only where diagonals are not bit-equal); PISO/PIMPLE averaged (key reaches it since ADR 0074 W1, before "silently ignored although documented"; no bit-equality guard); variable density averaged; **MRF zones always corrected**; **VOF corrected by default since ADR 0114** (own key `multiphase.component_diagonal`); coupled and density-based not affected.
- Exact-solution evidence (ADR 0074 part A): laminar half-channel Poiseuille u_max 12.75 / 15.57 vs 50 averaged, exact to 1e-10 corrected; SST half channel Re_tau 395 bulk velocity -73 percent averaged; SST plate with symmetry run-up not converged in 14000 iterations averaged vs 594 / 1863 corrected; symmetry-top-only plate changes by about 1e-6.
- Baseline hashes: exactly one moves, `packed_bed` (60, 19e53df373637262) -> (16, fb46cdfee5d0d781). Every other shipped case bit-identical.
- Flipped-default test run: 1777 passed, 6 failed, 422 ignored.
- Options: (1) keep `averaged` (failure is silent), (2) make `corrected` default (re-register `packed_bed`, reword tests), (3) keep `averaged` with a run-time warning (not implemented).
- **Update 2026-10-19 (ADR 0077 part D):** the heat-exchanger "convergence regression" was a relative stationarity criterion on a round-off pressure field (max|p| about 6e-14); corrected reaches the exact plug to round-off. The z4 tests would need an absolute criterion.
- **Update 2026-11-14 (F-531, ADR 0455):** in the transient kernel, Stokes' second problem impedance +32 percent real / -51 percent imaginary averaged; corrected within 2.5e-4.
- **Update 2026-11-15 (F-538, ADR 0613):** relaxed PIMPLE cancels the defect "by accident"; `sigma(alpha_u)` can change sign, so a relaxed transient result next to a `Symmetry` plane "can err in either direction".
- **Update 2026-11-16 (F-712, ADR 0650):** one-cell-deep annuli with `symmetry` planes deviate `2.4e-2` of `|U|` after one PISO step averaged; corrected `8e-16` (`8e-15` through `[mesh.sliding]`). Re-measured flipped default: only `packed_bed` moves; 11 `rcfd-api` tests change outcome.

## Contradictions with the wiki

- Within the doc: the "Test suite with the default flipped" table calls the heat-exchanger result "a genuine convergence regression of `corrected`"; the 2026-10-19 update withdraws that ("not a solver defect"). The doc marks the revision itself.

## Pages this source informs

- [[component-diagonal]]
- [[simple]]
- [[piso]]
- [[mrf]]
- [[bit-identical]]
- [[sliding-mesh]]
