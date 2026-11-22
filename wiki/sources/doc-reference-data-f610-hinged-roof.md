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

# Reference data: F-610 the hinged cylindrical roof limit load against the published value (ADR 0716)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-610 the hinged cylindrical roof limit load against the published value (ADR 0716)" (lines 2128-2142) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-610 hinged cylindrical roof limit load against the published value (ADR 0716).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


The paper was read (not a secondary source): K. Y. Sze, X. H. Liu and S. H. Lo, "Popular benchmark problems for geometric nonlinear analysis of shells", Finite Elem. Anal. Des. 40 (2004) 1551-1569, the authors' copy at `https://hub.hku.hk/bitstream/10722/54269/2/92945.pdf` (HKU Scholars Hub, record `https://hub.hku.hk/handle/10722/54269`), read 2026-11-17 (its text; the drawings not seen; SHA-256 of the PDF in ADR 0716). The paper's reference solutions are themselves finite-element solutions (ABAQUS S4R, Riks method), so the published limit load carries an unknown discretisation error.

| Item | Status | Source |
|---|---|---|
| Hinged roof, Figure 9a: $R = 2540$, $L = 254$ (two $L$ labels; read as the half-length, straight edges $2L = 508$, as ADR 0522), $\theta = 0.1$ rad, $h = 12.7$ (or `6.35`), $P_{\mathrm{max}} = 3000$, isotropic $E = 3102.75$, $\nu = 0.3$, central point load, straight edges hinged, curved edges free; consistent units (read as N, mm) | cited | Sze, Liu and Lo (2004), Figure 9a: read (text only; the $2L = 508$ reading also in arXiv:2107.12483, Section 3.3, secondary) |
| **Published limit load, $t = 12.7$, isotropic**: the largest tabulated `P/Pmax` before the snap, `0.7421` at $-W_C = 11.293$, so $P_{\mathrm{lim}} = 0.7421 \times 3000 = 2226.3$ N (whole roof); neighbours `(0.7412, 10.358)` and `(0.7286, 12.141)` | cited | Sze, Liu and Lo (2004), Table 9a ("12.7 unit thick isotropic hinged cylindrical shell", 16 x 16 S4R, Riks): read |
| Rising branch of Table 9a, $(P/P_{\mathrm{max}}, -W_C)$: `(0.1980, 1.638)`, `(0.3473, 3.087)`, `(0.4686, 4.477)`, `(0.5647, 5.802)`, `(0.6381, 7.057)`, `(0.6908, 8.237)`, `(0.7246, 9.339)` | cited | Sze, Liu and Lo (2004), Table 9a: read |
| Vertex of the parabola through the three tabulated points around the limit: $W_C = 10.876$, $P/P_{\mathrm{max}} = 0.74374$, `2231.2` N (`2.2e-3` above the tabulated value; a record, not the reference) | computed | ADR 0716 arithmetic 1 |
| The Hex20 record of ADR 0522 against the published value: point load `2213.9, 2184.5, 2167.5` N at $n = 4, 8, 16$ (`-5.6e-3, -1.88e-2, -2.64e-2`); the solid falls away from the published value as it is refined (the singular mid-line hinge of ADR 0522's diagnosis; kept as a record, not rerun) | computed | ADR 0522 addendum 1; ADR 0716 arithmetic 3 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
