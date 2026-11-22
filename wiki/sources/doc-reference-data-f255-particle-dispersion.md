---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-255 particle dispersion from turbulence-model fields and particle surface reactions (ADR 0208)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-255 particle dispersion from turbulence-model fields and particle surface reactions (ADR 0208)" (lines 1396-1407) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-255 particle dispersion and surface reaction references (ADR 0208).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing in F-255 is compared with measured data. No source was read for this work.

| Item | Status | What was read (source) |
|---|---|---|
| Mean-square displacement of a Stokes particle released at rest in a fluid velocity of autocorrelation `sigma^2 exp(-abs(s)/T)`: `sigma^2 (I0 - 2 I1 + I2)` (ADR 0208 D3), and its `tau -> 0` limit `2 sigma^2 T^2 (t/T - 1 + exp(-t/T))` | derived | from `X = int u (1 - exp(-(t - a)/tau)) da` and `<X^2> = int int g g R`; checked against the moment equations of the jump process (CF1, 7.6e-12); the limit is the form of Taylor's result for an exponential autocorrelation (Taylor 1921, "Diffusion by continuous movements", not read) |
| Long-time diffusivity `sigma^2 T` independent of `tau` | derived | the limit of the same closed form; consistent with the Tchen/Hinze result recalled in the wave 29C row (not read) |
| `epsilon = beta* k omega`, `beta* = 0.09` | standard; `beta*` verified (TMR, the k-omega SST row above) | the conversion `chemistry::mixing_vectors` already uses; not read in a source for this work |
| First-order kinetic surface reaction `omega = k_s rho_g Y_o / M_o` per unit surface, shrinking sphere at constant density `dd/dt = -2 nu_s M_s omega / rho_p` | registered here; the shrinkage derived | ADR 0208 D4 and Q7; no char or particle kinetics source read, no constants; the molar masses of the checks (0.012, 0.032, 0.044 kg/mol) are round test inputs, not data |
| Measured dispersion of particles in grid turbulence, char burnout data | not used | no comparison attempted |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
