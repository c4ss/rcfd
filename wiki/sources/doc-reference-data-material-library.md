---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Material property library (F-111)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Material property library (F-111)" (lines 311-378) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Material property library (F-111): IAPWS-IF97, NASA polynomials, air viscosity, Sutherland, atomic weights, dry air, cubic EOS constants.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Verified against primary sources in this session (fetched 2026-10-13):

- IAPWS-IF97 (IAPWS R7-97(2012), `iapws.org`): region 1 and region 2 coefficient tables, the B23 and saturation-pressure equations, and the property relations were read from the release itself. The program-verification values of Tables 5, 15 and 35 (all three state points each) are reproduced to 1e-8 relative in `crates/rcfd-core/src/iapws.rs`, which also validates the transcription of the coefficients. IAPWS R12-08 (viscosity, Tables 1, 2, verification Table 4) and R15-11 (thermal conductivity, Tables 1, 2, verification Table 4) likewise; their tables carry 7-9 digits, so the test tolerance there is 1e-7 relative (set at 1e-8 first, widened to 1e-7 after the 433.15 K, 1 kg/m^3 viscosity point missed it by 3.4e-8, which is the rounding of its last printed digit: a post-hoc change, disclosed). Not implemented: region 3, region 5, backward equations, critical enhancements.
- NASA polynomials (NASA TM-4513, McBride, Gordon and Reno 1993): coefficients for N2, O2, Ar, H2O, CO2, CH4, H2 copied from Cantera's `data/nasa_gas.yaml` (BSD-3-Clause, the file cites TM-4513); the Cantera licence requires keeping its notice, which is in the source header comment of `fluids.rs`. Consistency check: the two temperature ranges agree at 1000 K.
- Air viscosity: M. L. Huber, "Viscosity of Gases", CRC Handbook of Chemistry and Physics (NIST publication page, pub_id 926439), air at 100 kPa: 13.3, 18.5, 23.1, 27.1, 30.8 uPa s at 200-600 K (read from the rendered table; 3 digits).
- Air conductivity: Huber, "Thermal Conductivity of Gases" (pub_id 926438): 18.5, 26.4, 33.5, 39.9, 46.0 mW/(m K).
- Air specific heat: NASA TN D-5452 Table I (Poferl, Svehla, Lewandowski 1969), 1.005, 1.013, 1.030, 1.051 J/(g K) at 300-600 K (OCR of the scanned table, values consistent with the NASA polynomials to 0.12 %).
- Sutherland constants for air: 1.716e-5 Pa s at 273.15 K with S = 110.4 K, and 0.0241 W/(m K) with S = 194 K, are the usual textbook values written from memory (the viscosity pair also is the US Standard Atmosphere 1976 pair); only their agreement with the Huber tables is checked. Wave 13: confirmed (secondary) against CFD-Online "Sutherland's law" and the 1976 standard-atmosphere constants (C1 = 1.458e-6, S = 110.4 K gives 1.7161e-5 at 273.15 K), and arXiv:2210.01621 for the conductivity pair (2.41e-2 W/(m K), T0 273 K, S 194 K).

Sutherland's law result, pre-registered at 1 % for 300-500 K:

| Property | 300 K | 400 K | 500 K | Also 200 K | Also 600 K |
|---|---|---|---|---|---|
| Viscosity | -0.22 % | **-1.08 %** | **-1.46 %** | -0.11 % | -2.07 % |
| Conductivity | -0.64 % | +0.26 % | +0.69 % | -3.23 % | +0.35 % |

The 1 % criterion is NOT met by Sutherland's law for the viscosity above about 350 K; for the conductivity it is met. The viscosity assert for Sutherland air is a regression guard of 1.5 % set after the measurement; the 1 % criterion itself is recorded as missed.

The cubic fits `air_viscosity_fit_law`/`air_conductivity_fit_law` were fitted to the same five table rows (in-sample residual at most 0.11 % and 0.08 %; leave-one-out prediction of the 400 K row -0.22 % and -0.15 %). The 1 % assert on the cubic fits is in-sample.

Unverified (from memory, not checked against a primary source in this session; the wave 13 outcome is appended to each item):

- Standard atomic weights used for the molar masses (N 14.007, O 15.999, Ar 39.948, H 1.008, C 12.011). Wave 13: all inside the CIAAW intervals (ciaaw.org), confirmed.
- Dry-air mole fractions 0.78084 / 0.20946 / 0.00934 (N2/O2/Ar). Wave 13: NIST CIPM-2007 gives 0.780848 / 0.209390 / 0.009332; code differed by up to 0.09 % (immaterial). Post-verification fix: `GasMixture::dry_air` now uses the CIPM-2007 values 0.780848 / 0.209390 / 0.009332 (renormalised over the three species); its only user is the air cp/transport unit test, whose cp values moved by about 0.01 J/kg/K, all assertions unchanged and passing.
- Peng-Robinson constants 0.45724, 0.07780, kappa = 0.37464 + 1.54226 omega - 0.26992 omega^2 and Redlich-Kwong constants 0.42748, 0.08664, and the Wilke pair weight, as published by Peng and Robinson (Ind. Eng. Chem. Fundam. 15, 1976), Redlich and Kwong (Chem. Rev. 44, 1949) and Wilke (J. Chem. Phys. 18, 1950). The tests use only properties that follow from the forms themselves (critical-point conditions, ideal limit, root consistency, thermodynamic consistency of fugacity and residual enthalpy, the acentric-factor definition of the vapour pressure at reduced temperature 0.7 within 5 %: measured +0.38, -0.35, -0.23 %). The critical constants of N2 used in the tests were illustrative (126.2 K, 3.39 MPa); they are now the NIST values 126.192 K, 3.3958 MPa (omega 0.0372), tests unchanged and passing. Wave 13: PR 0.45724/0.07780/kappa polynomial confirmed (Wikipedia citing Peng and Robinson 1976; exact 0.457236, 0.077796 computed), RK 0.42748/0.08664 confirmed (Wikipedia and closed form); the Wilke weight remains unverified. The N2 test constants differed from NIST (126.192 K, 3.3958 MPa) as stated; fixed afterwards, see above.
- NIST WebBook data was deliberately not used (its terms were not reviewed); no REFPROP or other licensed file is read or shipped. (See the F-061 section below: later waves did embed WebBook values; wave 13 notes that NIST SRD 69 is copyrighted, see `docs/verification-report.md` section 1.6.)
- Critical constants and acentric factors in `rcfd-core/src/eos_data.rs` (the header comment now points at `docs/verification-report.md`): wave 13 confirmed all seven fluids against the NIST WebBook fluid pages; only the Ar acentric factor differed (code -0.002, NIST -0.00219); fixed to -0.00219 afterwards (no test uses Ar PR against NIST; workspace tests unchanged).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
