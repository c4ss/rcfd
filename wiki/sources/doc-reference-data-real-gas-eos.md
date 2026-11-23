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

# Reference data: Real-gas equation of state (F-061, ADR 0007)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Real-gas equation of state (F-061, ADR 0007)" (lines 465-495) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Real-gas equation of state (F-061, ADR 0007): NIST isotherm comparisons and the density and enthalpy known misses.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


The isotherm values come from the NIST Chemistry WebBook, fluid properties, isothermal tables (webbook.nist.gov/cgi/fluid.cgi, CO2 ID C124389, N2 ID C7727379, default reference state), retrieved 2026-10-14 through a page fetcher whose output passes through a language-model summariser. The numbers (density, enthalpy, entropy, cp, sound speed at the listed T and p) are copied into `crates/rcfd-fv/tests/realgas_nist.rs` as returned. They were NOT re-read from the raw page, so they are marked "retrieved, not independently re-read". As a sanity check, CO2 350 K 0.1 MPa 1.5167 kg/m^3 and N2 300 K 0.1 MPa 1.1233 kg/m^3 agree with the ideal gas to 0.3 percent and 0.0 percent. The tables list some rows (8.1 MPa, 5.1 MPa, 6.1 MPa) three times with different phase labels and identical values.

Wave 13 update: the isotherm values were re-read from the raw WebBook HTML (no summariser) and all match. The critical constants below were confirmed against the NIST fluid pages (only Ar omega differs). See `docs/verification-report.md`.

Originally UNVERIFIED (written from memory of the literature, not looked up): critical constants and acentric factors in `rcfd-core/src/eos_data.rs` (CO2 304.1282 K, 7.3773 MPa, 0.22394; N2 126.192 K, 3.3958 MPa, 0.0372; O2, Ar, CH4, H2, H2O likewise). The Peng-Robinson (1976) and Redlich-Kwong (1949) constants were already listed as unverified under F-111. The `rcfd-fv/src/realgas.rs` tests checked the thermodynamic consistency of the departure functions numerically instead (Maxwell relations `dh/dp|T = v - T dv/dT|p` and `ds/dp|T = -dv/dT|p`).

The nozzle tests compute their real-gas isentropic reference inside the test from the same EOS (an ODE in p with `dT/dp|s = T (dv/dT)_p / cp`). This reference checks the solver against the thermodynamics, not the EOS against nature. The (R1)-(R3) tests cover EOS accuracy against NIST: density within 1 percent for |Z-1| <= 0.05 etc. The table below lists the misses.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-061 PR density vs NIST | within 1 percent where \|Z_NIST - 1\| <= 0.05 | N2 300 K: 5.1 MPa +1.03, 10.1 MPa +1.68, 15.1 MPa +2.24 percent | `rcfd-fv/tests/realgas_nist.rs::r1_peng_robinson_density_vs_nist_known_miss` |
| F-061 PR enthalpy difference vs NIST | within 10 percent | N2 300 K: +18.8 (5.1 MPa), +15.7 (10.1), +12.3 (20.1) percent; CO2 350 K +9.3/+8.5/+7.8, 310 K +2.6/+1.2 pass | `r2_departure_enthalpy_and_entropy_differences_vs_nist_known_miss` |
| F-061 ideal-gas limit at 1 Pa | h_dep <= 1e-8 cp0 T and s_dep <= 1e-8 cp0 | CO2 PR 3.1e-8 / 2.1e-8, RK 2.6e-8 / 1.7e-8 (departures scale with p; limit was mis-estimated) | `r3_ideal_gas_limit_at_one_pascal_known_miss` |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
