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

# Reference data: Wave 32B: aeroacoustics for fan noise (ADR 0066, docs/acoustics.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 32B: aeroacoustics for fan noise (ADR 0066, docs/acoustics.md)" (lines 1341-1357) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 32B: aeroacoustics references for fan noise (ADR 0066).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No measured fan or airfoil noise data were read or used. The broadband proxies (Proudman, Curle, trailing edge) are NOT validated. Validation would need measured or scale-resolving wall-pressure and far-field data for a known operating point, calibration of the constants on one data set and a held-out comparison on another.

| Item | What was read (source) | Result |
|---|---|---|
| FW-H formulation 1A (Farassat, Brentner) | not read in this wave; formulas as in ADR 0013 (from memory of the papers) | UNVERIFIED as transcription; the implementation is verified only against closed-form fields (pulsating sphere, compact dipole, rotor point force, moving observer in flow) |
| Gutin rotor harmonics | Gutin (1936) and Lowson (1965) NOT read | the closed form was derived in ADR 0066 from the compact-dipole far field and checked against the FW-H solver (<= 4e-5); no claim to match the published form |
| Curle (1955), Proudman (1952) | NOT read | surface-dipole and isotropic-turbulence scalings re-derived or taken as scalings (U^6 dipole power, U^8 quadrupole) |
| Proudman constant alpha = 0.1 and M_t = sqrt(2k)/c0 | secondary search results only (see [provenance](../../docs/provenance.md)); the Sarkar and Hussaini report (a scanned PDF) could not be read | UNVERIFIED, secondary; UNVERIFIED against a published source, replace before relying on it; default only, not calibrated |
| Ffowcs Williams and Hall (1970) trailing-edge U^5 law | a review snippet; the paper was not read | only the exponent is used; the constant K_TE has no value from a source |
| A weighting | IEC 61672-1 NOT read; formula constants (20.6, 107.7, 737.9, 12194 Hz) from Wikipedia and metricgate.com | formula implemented; the tabulated A-weights matched the formula to 0.049 dB at exact base-10 frequencies when compared from memory of the usual table (table itself unverified) |
| 1/3-octave band definition | IEC 61260-1 NOT read | base-10 bands (ratio 10^(1/10)), as commonly stated; nominal labels from the usual rounded list |
| Reference pressure 20 uPa | common standard value | used as the SPL reference |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
