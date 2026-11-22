---
type: source
tags: [reference-data, benchmark]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Turbulence model constants (`crates/rcfd-turb/src/kepsilon.rs`, `wall.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Turbulence model constants (`crates/rcfd-turb/src/kepsilon.rs`, `wall.rs`)" (lines 143-162) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

k-epsilon (standard, RNG, Realizable) and wall-function constants with their verification state, including the Realizable A0 4.04 vs 4.0 difference.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Value | Source | Result |
|---|---|---|
| Standard: Cmu 0.09, C1 1.44, C2 1.92, sigma_k 1.0, sigma_eps 1.3 (Launder and Spalding, Comput. Methods Appl. Mech. Eng. 3 (1974) 269-289) | Wikipedia "K-epsilon turbulence model" | verified |
| RNG: Cmu 0.0845, C1 1.42, C2 1.68, eta0 4.38, beta 0.012, C2* = C2 + Cmu eta^3 (1 - eta/eta0)/(1 + beta eta^3); sigma = 0.7194 (= 1/1.3929) | arXiv:2501.00037; Yakhot, Orszag, Thangam, Gatski and Speziale, Phys. Fluids A 4(7) (1992) 1510-1520 (not read). | verified (arXiv) |
| Realizable: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(eta+5)), Cmu = 1/(A0 + As U* k/eps), As = sqrt(6) cos(phi) | Shih et al. NASA TM-106721 (see below); OpenFOAM `realizableKE` class docs (default A0 4.0, C2 1.9, sigma 1.0/1.2) | verified. Note A0 = 4.04: UNVERIFIED against a published source, replace before relying on it; see the note under this table |
| Wall function kappa = 0.4187, E = 9.793; rounded to 0.41 / 9.8 by rcfd | Launder and Spalding, Comput. Methods Appl. Mech. Eng. 3 (1974) 269-289 (not read; attribution not checked) | UNVERIFIED against a published source, replace before relying on it (no published source for the pair was reached, see [provenance](../../docs/provenance.md); rounding disclosed in code) |
| Log law kappa = 0.41, B = 5.2 (Pope 2000) | problem set based on Pope (perso.ens-lyon.fr), Abe et al. 2001; Wikipedia gives kappa 0.41, C+ 5.0 | verified, within the usual 5 percent spread |
| Reichardt (1951): u+ = ln(1 + 0.41 y+)/0.41 + 7.8[1 - e^{-y+/11} - (y+/11) e^{-y+/3}] | CFD-Online wiki "Reichardt profile" (via search result); numerical cross-check against MKM DNS | verified, and within 0.7, 0.8, 2.3, 2.3, 1.4 percent of DNS at y+ = 20, 30, 50, 100, 150 |
| Inlet relations k = 1.5 (I U)^2, eps = Cmu^0.75 k^1.5 / l, l = 0.07 L (`rcfd-api/src/turb.rs`) | CFD-Online/CFX forum guidance; l factor 0.07 for mixing-length codes | verified (k and l); eps = Cmu k^2/(nu * ratio) is the definition nu_t = Cmu k^2/eps, no external source needed |

Realizable A0 = 4.04 (used by rcfd, `kepsilon.rs`): UNVERIFIED against a published source (see [provenance](../../docs/provenance.md)), replace before relying on it; but 4.0 in OpenFOAM and, per one forum post, in Shih et al.; the original paper's printed value could not be fetched. Flagged in the source comment. **Wave 13 verification: Shih et al., NASA TM-106721 (https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf) prints A0 = 4.0 and Table 2 C2 1.9, sigma 1.0/1.2, C1 = max(0.43, eta/(5+eta)); rcfd's 4.04 differs from the original (see `docs/verification-report.md`).** ADR 0017: `KEpsilon::a0` / `turbulence.realizable_a0` selects 4.0 (original report) or 4.04 (a commercial-code variant, still the default); BFS-C medium x_r/h 6.349 with 4.0 against 6.413 with 4.04.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[backward-facing-step]]
