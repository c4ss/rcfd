---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 31B: turbulence wall diagnosis (ADR 0063)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 31B: turbulence wall diagnosis (ADR 0063)" (lines 1316-1324) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 31B: turbulence wall diagnosis (ADR 0063) and the TMR flat-plate grid convergence.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | What was read (source) | Result |
|---|---|---|
| SST model definition and wall conditions | https://tmbwg.github.io/turbmodels/sst.html (NASA TMR), fetched 2026-10-18 | read: equations, F1, F2, cross-diffusion floor, production limiter, nu_t with S and F2, constants, k_w = 0 and omega_w = 10 * 6 nu / (beta1 d1^2). Compared term by term with `crates/rcfd-turb/src/sst.rs`: no discrepancy found in the published terms. Menter (1994) and Menter, Kuntz, Langtry (2003) were NOT read (paywalled; only the TMR transcription is claimed) |
| SST flat-plate reference | https://tmbwg.github.io/turbmodels/flatplate_sst.html (NASA TMR 2DZP) | read: CFL3D SST Cf(x = 0.97) = 0.00268299 on 273 x 193 (GCI 0.28 %, p = 1.21), plate x in [0, 2] behind a symmetry section. rcfd's Richardson-extrapolated Cf(0.97) is +2.6 ... +3.6 % above it; not resolved (virtual origin untested) |
| Standard wall function (k-epsilon) | a secondary description, not a published source (recorded in [provenance](../../docs/provenance.md)); two neighbouring pages fetched, scalable-function text seen only through the fetch summary | UNVERIFIED against a published source, replace before relying on it. Read: y* definition, log law for y* > 11.225 and U* = y* below, k and epsilon of the wall-adjacent cell from equilibrium forms, zero normal gradient of k. rcfd differs for y* < y*_lam (G = 0, eps = 2 nu k/y^2); an opt-in test with the equilibrium forms removed the bump but not the fall and was removed (ADR 0063 Addendum 2) |
| Launder and Spalding (1974), Grotjans and Menter (1998) | not read | secondary descriptions only (search results, the description above); no claim about the original formulations |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[flat-plate]]
- [[gci-richardson]]
