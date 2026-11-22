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

# Reference data: Wave 31C: reference reading (ADR 0064, retrieved 2026-10-18)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 31C: reference reading (ADR 0064, retrieved 2026-10-18)" (lines 1274-1315) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 31C: sources read on 2026-10-18 for the validation cases (ADR 0064), with a discrepancy list.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


The discrepancies between a repo or source value and the text read are listed after the table; nothing was silently changed. Wave 31C downloaded public pages and PDFs as raw bytes (`curl`) and converted them with `pdftotext -layout`, or read them through the page fetcher. The fetcher's output is LLM-condensed, so potentially lossy; the numbers used below were checked against raw text wherever the raw file was available. Nothing was filled from memory. Small tables are stored as `validation-data/reference/w31c_*.csv` with manifest entries. The checks are in `crates/rcfd-api/tests/validation_refs_w31c.rs`.

| Item | What was read (source) | Result |
|---|---|---|
| Ghia, Ghia and Shin (1982) Table I (u) and Table II (v), Re 100/400/1000 | Gists ivan-pi/3e9326d18a366ffe6a8e5bfda6353219 (Table I) and ivan-pi/caa6c6737d36a9140fbcf2ea59c78b3c (Table II) of the paper, typed by a third party for all seven Re; raw CSV files of github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 `data/validation/ghia/`; Uppsala thesis diva2:1668016 Tables 1 and 2 (its y and x labels are shifted by one row in the text extraction; shifted back and compared by script). The original paper was NOT read | u (Re 100/400/1000, all 17 rows): gist, student repository and thesis agree cell for cell, and equal the in-repo `cases/cavity/reference/ghia1982_u_centerline.csv` and the Ghia column of `cavity_re1000_ghia_botella_u.csv`: verified (three secondary copies). v Re 100: equal to the in-repo file (three copies). v Re 400 and 1000: three copies with two cell disagreements (list below); stored in `w31c_ghia1982_v_centerline_read.csv`. The misprint Re 400 v(0.9063) = -0.23827 is flagged by the gist and the thesis |
| Backward-facing step, Armaly et al. (1983) | Full text of J. Fluid Mech. 127:473 (courses.washington.edu/me431/handouts/armaly-jfm-83.pdf) | text facts verified (expansion 1:1.94, Re = V D / nu, D = 2 h of the inlet channel, V the mean inlet velocity, laminar regime Re < 1200, extra separation regions above Re about 400); numeric x1/S: data not obtained (figures 4 and 14 only; not read off the plot) |
| Backward-facing step, Gartling (1990), Re 800 | MFIX-Exa tutorial (mfix.netl.doe.gov, `steady-backward-facing-step`) quoting Gartling's tables: L1 = 6.10, upper-wall separation 4.85, reattachment 10.48, Lu = 5.63, in channel heights H = 1 (step height 0.5; inlet profile 24 y (0.5 - y), Re = rho u H / mu = 800 with mean inlet velocity 1, so Re_h(inlet) = 400 = the repo's Re_h). In step heights: 12.20, 9.70, 20.96, 11.26 | verified (secondary); 12.20 equals the 12.20 of arXiv:2507.16509 Table 2. Gartling's original was not read. Conflict: Teruel and Fogliatto, Mecanica Computacional XXXII (2013) 3265 (amcaonline.org.ar) print a row labelled Gartling (1990) of 11.9 / 9.5 / 20.6 (and their own finest grid 12.20 / 9.70 / 20.96): unresolved |
| Cylinder Re 100, Williamson (1989) | Primary paper, equation (6) and figure 14 caption: S = A/Re + B + C Re, A = -3.3265, B = 0.1816, C = 1.600e-4, rms error 0.000089; laminar parallel shedding 49 < Re < 178 in the study | verified (primary); St(100) = 0.164335. The rows of `cylinder_re100_literature.csv` were re-checked against arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3 (Ding 1.325 / 0.164): all verified as quotations; no C_D value from a primary paper |
| Pipe friction: Colebrook-White, Prandtl, Blasius, Haaland, Swamee-Jain | Wikipedia "Darcy friction factor formulae" (raw HTML) and "Darcy-Weisbach equation" (fetcher): Colebrook-White 1/sqrt f = -2 log10( eps/(3.7 D) + 2.51/(Re sqrt f) ) for Re > 4000 in a full circular conduit; equivalent forms (14.8 R_h; 1.1364... and 9.287); Karman-Prandtl smooth form with adjusted constants 1/sqrt f = 1.930 log10(Re sqrt f) - 0.537 (data quoted for 4000 < Re < 1e8); Blasius f = 0.3164 Re^-1/4, valid up to Re 1e5; Haaland 1/sqrt f = -1.8 log10[(eps/D/3.7)^1.11 + 6.9/Re]; Swamee-Jain f = 0.25/[log10(eps/D/3.7 + 5.74/Re^0.9)]^2; Moody accuracy +-5 % smooth and +-10 % rough; laminar f = 64/Re; transition 2300 to 4000 | verified (secondary, formulas as printed on the pages; the Moody, Colebrook and Prandtl originals were not read). The tests are algebraic equivalences and the stated +-5 % bound. No numeric Moody table and no published Colebrook check values were obtained; the classical Prandtl smooth form with constants 2.0 and 0.8 was not read: data not obtained |
| Dean (1978) channel friction | arXiv:1405.1750 Appendix A: Cf = tau_w/(rho U_b^2/2) = alpha Re_b^-1/4 with alpha = 0.0614, Re_tau = sqrt(alpha/2) Re_b^(7/8) = 0.1752 Re_b^(7/8), Re_b = U_b h / nu with h the channel half-width (laminar Cf = 6/Re_b), Table 1 (Re_tau 100 to 600 against Re_b 1413 to 10954); the paper notes the inaccuracy of the correlation at low Re_tau (its DNS: Re_tau 199.7, Re_b 3176, against the table row 200 / 3121). Dean's abstract (search snippet only, not read in full) gives Cf = 0.073 Re^-1/4 with the duct-height Reynolds number; 0.073 / 2^(1/4) = 0.0614 is my arithmetic | verified (secondary; Dean's paper not read). Table 1 row Re_tau 100 prints U_b/u_tau 14.4 whereas Eq. (A1) gives 14.13 (other rows agree to 0.3 %): printed inconsistency or extraction artefact |
| Tube Graetz local Nusselt table (Shah and London, Hausen, Sellars-Tribus-Klein) | Clarkson CH490 notes (lin-web.clarkson.edu/projects/subramanian/ch490/notes/Graetz%20Problem.pdf): circular-tube constant-wall-temperature eigenvalues 2.7044, 6.6790, 10.673, 14.671, 18.670 (squares 7.3136, 44.609, 113.92, 215.24, 348.57), Nu_inf = 3.656 | eigenvalues and Nu_inf verified (secondary; the independent series solver in the test reproduces all five to 2e-4). Local Nu versus x+: data not obtained (Shah and London, Hausen 1943, Sellars-Tribus-Klein not accessible). The Hausen mean-Nu correlation 3.66 + 0.0668 Gz/(1 + 0.04 Gz^(2/3)) was seen in secondary search results only; one source (Sandia Aria manual) prints 0.4 instead of 0.04 |
| NACA 0012 force and Cp data | NASA TMR page https://tmbwg.github.io/turbmodels/naca0012_val.html: Re = 6e6 per chord, run M = 0.15; Ladson tripped force data (NASA TM 4074, 1988) `CLCD_Ladson_expdata.dat` (M 0.15, Re 6e6, 80/120/180 grit); Gregory and O'Reilly NASA R&M 3726 (1970) `CP_Gregory_expdata.dat` (file header Re 2.88e6, page text 3 million; digitised from a photocopy, only approximate; upper surface only; alpha 0, 10, 15); Ladson, Hill, Johnson NASA TM 100526 `CP_Ladson.dat` (M 0.3, aspect ratio 1.333, may not be sufficiently 2-D). The files were already in the F-125 pack (CC0, pinned commit 20a39f54...) | read; stored subsets: Ladson 80 grit (17 rows) and Gregory alpha = 0 (25 rows). Observations from the numbers: Cl slope about 0.108 per degree below 6.1 degrees (thin airfoil 0.1097), Cd min 0.00800 (80 grit), Cl max 1.6116 at 17.13 degrees, Cp stagnation 1, suction peak about -0.42 near x/c 0.1. Not read: AGARD AR-211, Vassberg and Jameson Cl/Cd/Cp, the transonic shock positions of the item above, any compressible NACA 0012 data |
| Stefan-Boltzmann constant | https://physics.nist.gov/cgi-bin/cuu/Value?sigma (CODATA 2022) | verified: 5.670 374 419... x 10^-8 W m^-2 K^-4, exact |
| Fin with convective tip | en.wikipedia.org/wiki/Fin_(extended_surface) (formulas from the page's alt text) | verified (secondary) |
| Schiller-Naumann switch to C_D = 0.44 at Re 1000 and its validity range | Wikipedia "Drag (physics)" does not give it. An OpenFOAM source listing was seen that shows the threshold; implementation code is not accepted as a literature source and is not used | still UNVERIFIED |
| Robertson digits, Carreau/Cross, TVD limiters, partitioning, Wilke weights, GRI-Mech licence, Peng-Robinson constants and the other unverified items of the wave 15 to 25 lists | not attempted in wave 31C (originals paywalled; not cheap) | unchanged |

#### Discrepancies between a repo or source value and the text read (wave 31C; nothing was silently changed)

1. Ghia v, Re 400, x = 0.5: 0.05186 (gist, student repository) against 0.05188 (Uppsala thesis Table 1). Stored 0.05186 (two of three). No rcfd test uses it.
2. Ghia v, Re 1000, x = 0.9063: -0.5155 (student repository; thesis -0.51550) against -0.51500 (gist). Stored -0.5155. No rcfd test uses it.
3. Ghia v, Re 400, x = 0.9063 = -0.23827 is a misprint of the original table (noted by the gist and the thesis), kept as printed. No rcfd test uses the Re 400 v table.
4. Gartling (1990) Re 800: 12.20 / 9.70 / 20.96 step heights (MFIX-Exa quoting Gartling; arXiv:2507.16509 for 12.20) against 11.9 / 9.5 / 20.6 (AMCA 2013 row labelled Gartling). The in-repo 12.20 agrees with two of three secondary sources. The original was not read.
5. St at Re 100: the in-repo CSV row "Williamson (experiment) 0.165" (1996 review via arXiv:2303.09262) against the 1989 fit 0.164335 (0.4 % apart). Both lie inside the 1 % window of the cylinder test.
6. Dean table row Re_tau 100 prints U_b/u_tau 14.4 against 14.13 from the paper's own Eq. (A1).
7. `validation_channel_re180.rs` takes the DNS Cf = 0.008136 (U_b+ 15.6787 at Re_tau 178.12). Dean's relation with Re_b = 15.6787 x 178.12 = 2792.7 gives 0.008446, 3.8 % higher. The source itself states the correlation is inaccurate at low Re_tau, so the repo's choice to use the DNS value instead is consistent with it.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[lid-driven-cavity]]
- [[backward-facing-step]]
- [[cylinder-re100]]
- [[naca0012]]
