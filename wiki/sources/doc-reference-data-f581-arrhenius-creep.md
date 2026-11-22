---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-581 an Arrhenius temperature factor for power-law creep on the time-stepping driver (ADR 0558)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-581 an Arrhenius temperature factor for power-law creep on the time-stepping driver (ADR 0558)" (lines 2355-2368) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-581 Arrhenius temperature factor for creep (ADR 0558).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-581. The names below are cited from memory; the closed forms are those of ADR 0489 with $A$ replaced by `A exp(-Q/(R T))` (ADR 0558 arithmetic 1), evaluated by one plain python script (`f581-scratch/probe.py`, no rcfd code).

| Item | Status | Source |
|---|---|---|
| Thermally activated power-law creep, $d \bar{\varepsilon}/dt = A \exp(-Q/(R T)) q^n t^m$ | stated by the acceptance, cited | F. H. Norton, The Creep of Steel at High Temperatures, McGraw-Hill, 1929; H. J. Frost and M. F. Ashby, Deformation-Mechanism Maps, Pergamon, 1982: NOT read, from memory |
| $R = 8.314462618\ \mathrm{J/(mol\,K)}$ (`mat_visco::GAS_CONSTANT`, the 2019 SI value $N_A k_B$) | constant of the repository | from memory |
| Uniaxial constant stress at $T$: $\varepsilon_c = A \exp(-Q/(R T)) \sigma^n t^{m+1}/(m+1)$; a temperature step $T_{1} \to T_{2}$ at $t_s$ ($m = 0$): $\varepsilon_c = A \sigma^n (f_{1} \min(t, t_s) + f_{2} \max(t - t_s, 0))$ | derived | ADR 0558 arithmetic 1, 4 |
| Relaxation at fixed strain at $T$: $\sigma(t) = (\sigma_{0}^{1-n} + (n - 1) E A \exp(-Q/(R T)) t)^{1/(1-n)}$ | derived | ADR 0558 arithmetic 1, ADR 0489 arithmetic 3 |
| $Q = 2 \times 10^{5}\ \mathrm{J/mol}$: `exp(-Q/(R T))` = `1.19154e-15` (700 K), `1.17770e-14` (750 K), ratio `9.8838` | computed | `f581-scratch/probe.log` |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
