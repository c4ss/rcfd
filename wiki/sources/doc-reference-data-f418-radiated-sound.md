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

# Reference data: F-418 radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral (ADR 0468)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-418 radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral (ADR 0468)" (lines 1728-1743) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-418 radiated sound of a vibrating surface (ADR 0468).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-418; every item is cited from memory and derived in ADR 0468 (closed forms 1 to 6; numpy checks `f418-scratch/closed_forms.py` and `estimate.py`). No measured radiation data.

| Item | Status | Source |
|---|---|---|
| Rayleigh integral of a baffled plane, `P = (i omega rho/(2 pi)) int V_n e^{-i k R}/R dS` (`e^{i omega t}`) | derived | the free monopole and the image in the rigid baffle; Rayleigh, The Theory of Sound, vol. 2, section 278 and following: NOT read |
| On-axis pressure of a baffled circular piston, `|P| = 2 rho c V |sin(k (sqrt(z^2 + a^2) - z)/2)|` | derived | radial integral, checked by quadrature to 6e-16; Kinsler, Frey, Coppens and Sanders, Fundamentals of Acoustics, radiation chapter: NOT read |
| Far-field directivity `|2 J1(k a sin theta)/(k a sin theta)|` | derived | the disk integral of `e^{i k s x}`, checked by quadrature to 1.3e-15; Pierce, Acoustics, chapter 5: NOT read |
| Radiation impedance `Z = rho c S (1 - J1(2 k a)/(k a) + i H1(2 k a)/(k a))`, `H1` the Struve function | derived | the chord-length density of the disk, `p(s) = (4 s/(pi a^2)) (arccos(s/2a) - (s/2a) sqrt(1 - s^2/4a^2))`, integrated numerically: equal to the closed form to 3e-15 (`k a = 0.1`) .. 3.4e-13 (`k a = 10`); Morse and Ingard, Theoretical Acoustics, chapter 7: NOT read |
| Radiated power `W = (rho c k/(4 pi)) int int Re(V conj V) sin(k R)/R`; baffled monopole `rho c k^2 |Q|^2/(4 pi)`; baffled pair `(|Q1|^2 + |Q2|^2 + 2 Re(Q1 conj Q2) sinc(k d))` | derived | first principles, ADR 0468 closed form 5; the elemental-radiator form as in Elliott and Johnson, J. Acoust. Soc. Am. 94 (1993): NOT read |
| ERP `(rho c/2) int |V_n|^2 dS` (peak), `rho c int v_rms^2 dS` (RMS); plate mode `rho c A v0^2/8` | derived | definition (radiation efficiency one) |
| J1 by `(1/pi) int_0^pi cos(t - x sin t) dt`, Struve `H1(x) = (2x/pi) int_0^1 sqrt(1 - t^2) sin(x t) dt` | from memory | integral representations, each checked against its power series (1e-15, 6e-15) |
| Sound power reference 1e-12 W | common standard value | the power-level reference; third-octave numbering of ADR 0066 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
