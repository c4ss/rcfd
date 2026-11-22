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

# Reference data: F-580 creep on the B-bar Hex8 against a finite-strain reference of the thick cylinder (ADR 0549)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-580 creep on the B-bar Hex8 against a finite-strain reference of the thick cylinder (ADR 0549)" (lines 2340-2354) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-580 creep on the B-bar Hex8 against a thick cylinder (ADR 0549).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-580. The books below are cited from memory for the names and the published forms. The stationary field is derived in ADR 0549 arithmetic 1; the finite-strain reference is a one-dimensional solve written in the test (`si_fbar.rs`, `reference_with`), first prototyped by plain numpy scripts (`f580-scratch/probe_ref1d.py`, `probe_fbar1d.py` and their logs), no rcfd code in the probes.

| Item | Status | Source |
|---|---|---|
| Stationary Norton creep field of the plane-strain thick tube, `sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1)`, `sigma_theta = p ((2 - n)/n (b/r)^(2/n) + 1)/((b/a)^(2/n) - 1)` | derived, cited | ADR 0549 arithmetic 1, ADR 0489 arithmetic 6; F. K. G. Odqvist, Mathematical Theory of Creep and Creep Rupture, 1966; H. Kraus, Creep Analysis, Wiley, 1980; J. T. Boyle and J. Spence, Stress Analysis for Creep, Butterworths, 1983: NOT read, from memory |
| Axisymmetric total Lagrangian reference: `F = diag(rho', rho/R, 1)`, `P_rR = rho' S_RR`, `P_thTh = (rho/R) S_ThTh`, `int (P_rR du' + P_thTh du/R) R dR = p a du(a)` (dead pressure) | derived | ADR 0549 arithmetic 2 |
| Backward Euler radial return of the von Mises power law | derived, cited | ADR 0549 arithmetic 2, ADR 0489 D1; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998: NOT read, from memory |
| Measured: the reference at `p/1000` against Lame `2.9e-6`, against the stationary field at `20 t*` `8.5e-6`; `N = 400` against `N = 800` `2.2e-6`; at `p` its own drift from the small-strain stationary field `2.3e-3` (`5 t*`) to `8.6e-3` (`20 t*`) | computed | ADR 0549 addendum 1 |
| Measured: the BBar Hex8 cylinder (`48 x 24`) against the reference, `9.55e-3` at `5 t*`, `1.11e-2` at `20 t*`, growth `1.51e-3` (bound `2e-3`); the point-wise difference grows by `4.4e-3` at `r = 1.05` (reported) | computed | ADR 0549 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
