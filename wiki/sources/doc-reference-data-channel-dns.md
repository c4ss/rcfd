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

# Reference data: Channel DNS (Moser, Kim and Mansour, Phys. Fluids 11 (1999) 943)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Channel DNS (Moser, Kim and Mansour, Phys. Fluids 11 (1999) 943)" (lines 173-199) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Moser, Kim and Mansour (1999) channel DNS at Re_tau 395 values read from chan395.means.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Used in `crates/rcfd-turb/tests/channel_kepsilon.rs`, `cases/channel/case.json`.

Source: the published statistics file `https://turbulence.oden.utexas.edu/data/MKM/chan395/profiles/chan395.means` (129 half-channel points, parsed and integrated with the trapezoid rule).

| Value in repo | Computed from MKM data | Result |
|---|---|---|
| U_c+ = 20.1 | 20.133 | verified |
| U_b+ = 17.4, Re_b = 2 h U_b/nu = 13750 | U_b+ = 17.545 (relative to the true u_tau, Re_tau = 392.24), Re_b = 13764 | verified. 17.4 equals Re_b/(2 x 395), which is the consistent value for the nominal Re_tau = 395 imposed in the test (nu = 1/395); 17.54 applies at 392.24. Left unchanged. |
| "Re_tau = 395" | actual 392.24 (nominal 395) | note added to the test and the case citation |

Profile samples:

| y+ | U+ |
|---|---|
| 5 | 4.82 |
| 10 | 8.44 |
| 15 | 10.657 (interpolated from the same chan395.means file for the SST test, verified from the file) |
| 20 | 12.007 |
| 30 | 13.493 |
| 50 | 14.856 |
| 100 | 16.534 |
| 150 | 17.646 |
| 300 | 19.745 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
