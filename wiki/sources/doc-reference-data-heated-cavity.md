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

# Reference data: Differentially heated cavity (de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Differentially heated cavity (de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264)" (lines 57-106) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

de Vahl Davis (1983) benchmark values for the differentially heated cavity at Ra 1e3 to 1e6, read in the primary Table V scan in wave 30C, plus other values read in that wave.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Used in `cases/heated_cavity/reference/devahldavis1983.csv`, `crates/rcfd-fv/tests/natural_convection.rs` (REFERENCE table), `crates/rcfd-api/src/sim.rs` (Nu = 1.118 comment).

| Value | Source | Result |
|---|---|---|
| Nu_avg 1.118, 2.243, 4.519, 8.800 | MOOSE/Cardinal V&V page (cardinal.cels.anl.gov/vv/square_cavity_natural_convection.html) "Benchmark" column; arXiv:physics/0305049 Table 1 (Ra 1e4..1e6); arXiv:1412.2866 Table 2 | verified, secondary |
| u_max 3.649, 16.178, 34.73, 64.63 | Cardinal V&V page; arXiv:1412.2866 Table 2 | verified, secondary |
| v_max 3.697, 19.617, 68.59, 219.36 | Cardinal V&V page; arXiv:1412.2866 Table 2 | verified, secondary |
| Nu_max 1.505, 3.528, 7.717, 17.925 | arXiv:1412.2866 Table 2 (benchmark row, which prints 3.328 for Ra=1e4, an evident typo: its own quoted error of 0.094 percent against 3.5313 implies 3.528) | verified, secondary |
| Pr = 0.71, velocity scale alpha/L | Cardinal V&V page (Davis and Jones 1983 set-up) | verified |

Locations of the maxima (not used by tests) were not copied.

Wave 19A re-check (`crates/rcfd-fv/tests/coupled_open.rs` B3, Ra = 1e3 and 1e4 values 1.118 / 3.649 / 3.697 and 2.243 / 16.178 / 19.617): the two arXiv papers were downloaded and read as text. arXiv:1412.2866 Table 2 prints Nu 1.118 and 2.243, u_max 3.649 and 16.178, v_max 3.697 and 19.617 for the benchmark row. arXiv:physics/0305049 attributes Nu 2.243, 4.519 and 8.800 to de Vahl Davis. The Cardinal V&V page answered with HTTP 403 in this wave and was not re-read. The 1983 printed tables remain unreachable, so the values stay *verified through secondary sources* only. The coupled-solver tests claim agreement with these numbers within 2 percent (measured 0.02 to 0.74 percent), nothing more.

Wave 30C (ADR 0061) re-checked the cavity table against the primary source. The 1983 paper itself was read as a page scan (https://elearning.univ-bejaia.dz/mod/resource/view.php?id=71697, Table V on page 256), and all eleven quantities at the four Rayleigh numbers were transcribed (`cases/heated_cavity/reference/devahldavis1983_tableV.csv`):

| Quantity | Values at the four Rayleigh numbers |
|---|---|
| Nu_avg | 1.118, 2.243, 4.519, 8.800 |
| Nu_max | 1.505, 3.528, 7.717, 17.925 at z = 0.092, 0.143, 0.081, 0.0378 |
| u_max | 3.649, 16.178, 34.73, 64.63 at z = 0.813, 0.823, 0.855, 0.850 |
| v_max | 3.697, 19.617, 68.59, 219.36 at x = 0.178, 0.119, 0.066, 0.0379 |

Every value matches the secondary sources above, so Nu_avg, u_max, v_max and Nu_max are now *verified, primary* and the locations of the maxima are read for the first time. The paper states the accuracy as better than 1 percent at the highest Rayleigh number and down to one tenth of that at the lowest. Tables I to IV (the other Prandtl numbers and the reference-mesh listings) were not used. The transcription from the scan is by eye; the agreement with the independently published copies is the check on it.

Other values read in wave 30C (all numeric tables of public pages, used by the ADR 0061 tests):

| Value | Source | Status |
|---|---|---|
| Sod star state p* 0.30313, u* 0.92745, rho*L 0.42632, rho*R 0.26557, shock speed 1.75216, fan head -1.18322 and tail -0.07027 | Marzouk, Sci. Afr. 10 (2020) e00573 = arXiv:2108.01709 Table 2 (read as text); the shock speed also from the Rankine-Hugoniot condition | verified, secondary (paper read; Sod 1978 and Toro not read) |
| Fully developed laminar tube Nu_D 3.66 (constant wall T) and 4.36 (constant flux); entry length x_fd,t = 0.05 Re Pr D; Hausen Nu_m = 3.66 + 0.0668 Gz / (1 + 0.04 Gz^(2/3)) | BYU ME340 table 8.4, https://www.et.byu.edu/~vps/ME340/TABLES/8.4.pdf | verified, secondary (Hausen's paper, Shah and London, Incropera not read). The 0.05 Re Pr D rule is the constant-T rule; constant flux needs a longer pipe (ADR 0061 addendum 3) |
| Parallel-plate Graetz tables (theta_b, local Nu for constant wall T and constant flux, limits 7.5407 and 8.2353) | NPTEL ME-662 lecture 19, https://archive.nptel.ac.in/content/storage2/courses/112101002/downloads/l19.pdf, slides 11 to 19; since 2026-10-27 the stored tables are recomputed by `validation-data/generate_graetz.py` (they agree with the transcription except constant-flux Nu at x+ = 1e-4: 32.1558 against 32.153 printed) | verified, secondary; the tube Graetz local-Nu table was not obtained (data not obtained) |
| Flat plate Cf = 1 / (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012) (Karman-Schoenherr), Coles-law u+(y+) at Re_theta = 10000 | NASA TMR https://turbmodels.larc.nasa.gov/flatplate_val.html (page text) and its `cf_K-S.dat`, `u+y+.dat` (formula agrees with the data file to 1e-6; 71 of the 2685 rows stored) | verified, primary data of the TMR; the page draws 5 percent error bars on Cf, which is the only stated uncertainty |

Not obtained in wave 30C: de Vahl Davis Tables I to IV, Shah and London, Hausen (1943), Incropera and Kays, Sod (1978), any NACA 0012 surface data (case not attempted).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[differentially-heated-cavity]]
