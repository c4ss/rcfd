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

# Reference data: Wave 18B combustion constants (ADR 0027)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 18B combustion constants (ADR 0027)" (lines 793-806) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 18B combustion constants (ADR 0027), EDC constants from a secondary source.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


None of the constants below could be checked against a primary source in this environment. The checks of ADR 0027 test the implementation against the formulas as written here, not against published flame data.

| Quantity | Source | Status |
| --- | --- | --- |
| Eddy-dissipation rate `A rho (eps/k) min(Y_R/(nu' W_R), B Y_P/sum nu'' W)`, A = 4.0, B = 0.5 | Magnussen and Hjertager (1977) | UNVERIFIED: values from secondary lecture notes and forum posts; the primary paper was not accessed |
| EDC fine-structure fraction `xi* = C_xi (nu eps/k^2)^(1/4)`, residence time `tau* = C_tau (nu/eps)^(1/2)`, C_xi = 2.1377, C_tau = 0.4082 | Magnussen (1981, 1989); Gran and Magnussen (1996) | secondary: values stated in arXiv:2101.08764, which cites Gran and Magnussen; primary papers not accessed. The relations `C_xi = (3 C_D2 / (4 C_D1^2))^(1/4)`, `C_tau = sqrt(C_D2/3)` with C_D1 = 0.134, C_D2 = 0.5 reproduce both values; C_D1 and C_D2 are UNVERIFIED (from memory) |
| EDC reacting-fraction exchange rate `m = xi*^2 / (tau* (1 - xi*^3))` | same | read in arXiv:2101.08764 (eq. 7, reacting fraction taken as 1); secondary |
| Extended Zeldovich closed form `d[NO]/dt = 2(k1f k2f [O][N2][O2] - k1r k2r [NO]^2 [O]) / (k1r [NO] + k2f [O2])` | Zeldovich (1946); Lavoie, Heywood and Keck (1970) | derived in ADR 0027 K10 under the steady-state-N approximation; no rate constants bundled |
| Zeldovich rate constants (O + N2 = NO + N etc.) | Hanson and Salimian evaluation | UNVERIFIED: the primary evaluation was not accessed and no published confirmation was found; the tests use invented constants |
| Nine-coefficient polynomial `cp/R = a1 T^-2 + a2 T^-1 + a3 + a4 T + a5 T^2 + a6 T^3 + a7 T^4`, with `h/RT` and `s/R` obtained by integration | NASA Glenn thermodynamic polynomial form | functional form as commonly stated, UNVERIFIED against the report; the integrated forms are verified by derivative consistency (K12a). The text file layout of the public database is NOT implemented |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
