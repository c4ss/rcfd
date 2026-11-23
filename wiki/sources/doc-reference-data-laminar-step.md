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

# Reference data: Laminar backward-facing step, expansion ratio 2 (`crates/rcfd-fv/tests/step_laminar.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Laminar backward-facing step, expansion ratio 2 (`crates/rcfd-fv/tests/step_laminar.rs`)" (lines 218-229) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Laminar backward-facing step with expansion ratio 2: Erturk and Gartling reattachment lengths and the measured rcfd values.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Convention: the tests use Re_h = U_mean h / nu (h = step height = inlet channel height); the literature below uses Re = U_mean D_h / nu with D_h = 2h, so Re_literature = 2 Re_h.

| Value | Source | Result |
|---|---|---|
| Re = 800 (Re_h = 400): x_r/h = 11.834 (Erturk, Comput. Fluids 37 (2008) 633) and 12.20 (Gartling, Int. J. Numer. Methods Fluids 11 (1990) 953) | Table 2 of arXiv:2507.16509 (a 2025 MHD backward-step paper that quotes both), fetched 2026-10-14 through a page fetcher with an LLM summariser; the original papers were not accessed | secondary; wave 13 re-read Table 2 of https://arxiv.org/pdf/2507.16509 as raw text (Erturk 11.834, Gartling 12.20, Gresho 12.20) and confirmed the citations via Crossref; originals still not accessed. Used by `step_reattachment_re_h_400_vs_erturk_and_gartling` (5 % windows); measured 11.672 (-1.4 % / -4.3 %), pass |
| Re = 200 and 400 (Re_h = 100, 200): x_r/h = 4.95 and 8.10 | Table 1 of the same paper: the finite-volume results of its authors (ER 2, 2-D), not a published benchmark | secondary, unverified against Armaly/Gartling/Erturk; used only as an independent-code consistency check (5 % windows); measured 4.949 and 8.182 (+1.0 %), pass |
| Armaly et al., J. Fluid Mech. 127 (1983) 473: reattachment lengths versus Re | no numeric table accessible here (the abstract gives the Re range 70-8000 only); the experiment is 3-D with expansion ratio 1.942 and differs from the 2-D laminar computations above beyond Re about 400 | unverifiable here; no assertion uses it. Wave 31C: full text read (https://courses.washington.edu/me431/handouts/armaly-jfm-83.pdf): expansion ratio 1:1.94, Re = V D / nu with D = 2 h of the inlet channel, laminar regime Re < 1200, extra separated regions above Re about 400; numeric reattachment lengths only in figures: data not obtained |
| "Gartling Re 800 about 12.2, Armaly about 12" (F-106 note, from memory) | superseded by the Gartling value above | the Armaly value stays unverified. Wave 31C: Gartling 12.20 also seen as 6.10 channel heights (H = 1, step 0.5) in the MFIX-Exa tutorial quoting Gartling's Tables II and III (secondary); the Armaly paper was read in full, its reattachment lengths exist only as figures 4 and 14: data not obtained. See the wave 31C section (one conflicting secondary row 11.9 / 9.5 / 20.6) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[backward-facing-step]]
