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

# Reference data: Backward-facing step (`cases/backward_step/case.json`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Backward-facing step (`cases/backward_step/case.json`)" (lines 209-217) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Driver and Seegmiller (1985) turbulent backward-facing step reattachment length and conditions.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Value | Source | Result |
|---|---|---|
| Reattachment x/H = 6.26 +/- 0.10 (Driver and Seegmiller, AIAA J. 23 (1985) 163) | NASA TMR backstep validation page (tmbwg.github.io/turbmodels/backstep_val.html); OpenFOAM docs (doc.openfoam.com 2306 backward-facing-step) | verified |
| Re_H about 36000 | same pages | verified |
| Expansion ratio 1.125, inlet channel 8h | not stated on either fetched page; figure only. Wave 13: NASA GRC WIND page (grc.nasa.gov/WWW/wind/valid/backstep/backstep01/backstep01.html) gives H = 0.5 in and step:exit height 1:9, i.e. exit 9H, inlet 8H, ratio 9/8; ERCOFTAC case 30 snippet agrees (1.27 cm step, 10.1 cm inlet) | confirmed (secondary; ERCOFTAC page itself not fetched) |
| "5.5 to 7 step heights for RANS" scatter | no source found (wave 13 searched again) | unverifiable |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[backward-facing-step]]
