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

# Reference data: 0D reactors and FW-H acoustics (F-112, F-136, ADR 0013, `crates/rcfd-reactor/tests/reactor.rs`, `crates/rcfd-post/tests/fwh.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "0D reactors and FW-H acoustics (F-112, F-136, ADR 0013, `crates/rcfd-reactor/tests/reactor.rs`, `crates/rcfd-post/tests/fwh.rs`)" (lines 577-592) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

0D reactors and FW-H acoustics (F-112, F-136, ADR 0013): Robertson digits, flame temperature conflict, Formulation 1A.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Reference | Source status |
| --- | --- | --- |
| Robertson problem at t = 40: y = (0.7158270687, 9.185534764e-6, 0.2841637457) | stiff test-set value (Mazzia and Magherini, Bari IVP test set; Hairer and Wanner) | digits from memory; the Bari PDF was password protected and not readable. The integrator reproduces them to 1e-10 relative, which corroborates the memory but is not a primary-source check. Wave 13: the Bari ROBER report (archimede.uniba.it/~testset/report/rober.pdf) is readable but lists a reference only at t = 1e11; a J. Math. Chem. 61 (2023) table shows 7.158E-01, 9.185E-06, 2.841E-01 and a student repository 0.7158271, 9.1855e-6, 0.2841637, so only the leading 4 to 7 digits are corroborated; digits beyond remain unverified |
| Prothero-Robinson `y' = -lambda (y - sin t) + cos t`, `y(0) = 0` | exact solution `y = sin t` | analytic, derived here |
| Toy A2 <=> 2A equilibrium and relaxation | closed forms from the constant-cp thermo written in the test | analytic, derived here |
| CH4/air phi = 1 constant-pressure adiabatic flame temperature about 2225 K (registered with 1 percent window; measured 2225.5 K) | commonly quoted value | figure from memory, NOT verified against a primary source. Wave 13: NASA CEA (CEARUN) equilibrium 2224.25 K at 298.15 K, 1 atm (Marzouk, arXiv:2503.11826 Table VII), supporting 2225 K; the same paper lists 2156.25 K for GRI-Mech 3.0/Cantera, an unexplained conflict (secondary source) |
| GRI-Mech 3.0 H2/air and CH4/air ignition delay values | published GRI-based results (F-112 acceptance text) | NOT available: no number could be verified from an accessible source, so no such comparison is claimed. The ignition delays printed by the tests are this code's own results |
| GRI-Mech 3.0 input files (Berkeley mechanism, thermo, transport) | `validation-data/data/gri-mech-30/` (git-ignored fetch, F-125) | licence "citation requested", terms unverified (wave 13: the Berkeley page was re-checked and has only a "how to cite" block, no licence text; Cantera's own License.txt is BSD-3-Clause for Cantera code); never committed. The thermo polynomials of the file are discontinuous at 1000 K by 5e-3 (h) and 5e-4 (s) relative: registered limit 1e-3 missed (`r3_gri_nasa_continuity_known_miss`) |
| Formulation 1A (Brentner and Farassat 1998) and permeable terms (Di Francescantonio 1997; Brentner and Farassat 2003) | published papers | formulas from memory (wave 13: structure and the M_r - M^2 combination confirmed against Farassat NASA/TM-2007-214853, permeable L_i and B terms against AIAA 2004-2805 eq. 2; equation images not machine-readable); validated only through the analytic monopole, dipole, rotating monopole and rotating monopole-pair checks (errors 4e-6 to 1e-5) |
| Rotating-source references | exact Lienard-Wiechert potential of a moving monopole, velocity by 8th-order central differences | derived here from the wave equation |

The rotating dipole is realised as a pair of opposite monopoles on a circular orbit. A rotating point force with an exact non-compact velocity field is not tested. Registered misses: R3 NASA continuity (above).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
