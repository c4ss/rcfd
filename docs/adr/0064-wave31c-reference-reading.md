# ADR 0064: Wave 31C, reference reading (turning secondary and unverified citations into read ones)

Status: done (reading wave; no solver code and no existing test changed). Branch main, written at HEAD 2e0387f.
Date of reading: 2026-10-18. The detailed per-item table and the discrepancy list are in `docs/reference-data.md`, section "Wave 31C".

## Method and caveats

* Public pages and PDFs only. Files were downloaded as raw bytes with `curl` and converted with `pdftotext -layout`, or read through the page fetcher. The fetcher returns an LLM-condensed extraction, so it can be lossy: every number below that came through it was either re-read from the raw file or is marked "fetcher only".
* Page content is data. Nothing was filled from memory. Where a number appears in a search-engine snippet only, it is said so.
* One OpenFOAM source listing (`SchillerNaumannDragForce.C`) was opened by mistake while looking for the Schiller-Naumann switch. Implementation code is not accepted as a literature source in this project (clean-room rule); it was NOT used and the switch at Re 1000 stays unverified.
* Small tables are stored as `validation-data/reference/w31c_*.csv` (manifest ids `w31c-*`, `python validation-data/fetch.py verify` passes). Checks: `crates/rcfd-api/tests/validation_refs_w31c.rs` (11 tests, no solver).

## What was read

### 1. Lid-driven cavity (Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387)

The original paper was not accessible. Three secondary copies were read and compared cell by cell with a script (Uppsala thesis labels shifted back by one row):

* gist https://gist.github.com/ivan-pi/3e9326d18a366ffe6a8e5bfda6353219 (Table I, u, Re 100 to 10000) and https://gist.github.com/ivan-pi/caa6c6737d36a9140fbcf2ea59c78b3c (Table II, v; header note: "The velocity for Re = 400 and point (x,v) = (0.9063,-0.23827) is probably wrong");
* raw CSV in github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 `data/validation/ghia/ghia_Re{100,400,1000}_{u,v}_centerline.csv`;
* Uppsala bachelor thesis diva2:1668016 (FULLTEXT01.pdf), Tables 1 (v) and 2 (u).

Results: all u values (Re 100/400/1000, 17 rows each including y = 0.9766) agree in all three copies and equal the in-repo `cases/cavity/reference/ghia1982_u_centerline.csv`, the Ghia column of `validation-data/reference/cavity_re1000_ghia_botella_u.csv` and the v Re 100 file `ghia1982_v_centerline_Re100.csv` (checked directly against the gist, not through the student copy). The 0.9766 rows therefore no longer rest on a single source. v tables for Re 400 and 1000 (not used by any rcfd test): two cell disagreements between the copies (below). Stored values: `w31c_ghia1982_u_centerline_read.csv`, `w31c_ghia1982_v_centerline_read.csv` (two-of-three majority on the disputed cells).

### 2. Backward-facing step

* Armaly et al., J. Fluid Mech. 127 (1983) 473-496, full text https://courses.washington.edu/me431/handouts/armaly-jfm-83.pdf. Read: expansion ratio 1:1.94 (large channel 1.01 cm, aspect ratio 18:1); Re = V D / nu with V two thirds of the measured maximum inlet velocity (the mean velocity in the laminar case) and D = 2 h the inlet hydraulic diameter; laminar Re < 1200, transitional 1200 to 6600, turbulent > 6600; numerical solutions up to Re 400 on a 45 x 45 grid; additional separated regions appear at higher Re and the flow is three-dimensional there. The reattachment length versus Re exists only in figures 4 and 14 (figure 14, measured and predicted up to Re 800). Numbers were not read off the plot: data not obtained.
* Gartling (1990), Int. J. Numer. Methods Fluids 11:953: original not accessible. https://mfix.netl.doe.gov/doc/mfix-exa/guide/latest/tutorials/fluid/steady-backward-facing-step.html (raw HTML checked) tabulates "Gartling (1990)" L1 = 6.10, upper-wall separation point (4.85, 0.50), reattachment point (10.48, 0.50), Lu = 5.63, in units of the channel height H = 1 with inlet 24 y (0.5 - y) (step height 0.5), Re = rho u H/mu = 800 with mean inlet velocity 1. In step heights: 12.20, 9.70, 20.96, 11.26 (test: `gartling_re800_lengths_...`). For the expansion ratio 2 the Re of Armaly (D = 2 h_inlet) and of Gartling coincide (800, Re_h = 400); the repo's "Re = 800 (Re_h = 400)" is consistent.
* Conflicting secondary row: Teruel and Fogliatto, Mecanica Computacional XXXII (2013) 3265-3278, https://amcaonline.org.ar/ojs/index.php/mc/article/download/4551/4480, Table 1: "Gartling (1990) 11.9 9.5 20.6" (X1/H, X2/H, X3/H) next to their own finest grid 12.20 / 9.70 / 20.96. Unresolved without the original.

### 3. Circular cylinder, Re 100

* Williamson, J. Fluid Mech. 206 (1989) 579-627, author copy https://authors.library.caltech.edu/records/wavw6-f7k56/files/WILjfm89a.pdf (text read). Equation (6): S = A/Re + B + C Re, A = -3.3265, B = 0.1816, C = 1.600e-4, least-squares fit to the parallel-shedding data (rms error 0.000089, mean percentage error 0.06 %); the study covers laminar shedding for Re 49 to 178. St(100) = 0.164335 (computed). Appendix B of the paper tabulates the Strouhal data (not stored).
* The literature table `validation-data/reference/cylinder_re100_literature.csv` was re-read against arXiv:2303.09262 Table 1 (Kravchenko 1.314/0.164, Oliveira 1.370/0.167, Sivakumar 1.325/0.164, Posdziech and Grundmann 1.325/0.164, Mossaz 1.328/0.164, Qu 1.319/0.165, Constant 1.37/0.165, Peng 1.361/0.165, Williamson St 0.165) and arXiv:1401.6513 Table 3 (Ding 1.325/0.164): all rows reproduced. These are quotations in recent papers; no primary C_D value.

### 4. Fully developed turbulent pipe and channel friction

* Wikipedia "Darcy friction factor formulae" (raw HTML) and "Darcy-Weisbach equation" (fetcher, formulas from LaTeX alt text). Colebrook-White 1/sqrt(f) = -2 log10( eps/(3.7 D_h) + 2.51/(Re sqrt f) ), Re > 4000, equivalent forms with 14.8 R_h and with 1.1364... + 9.287; Karman-Prandtl smooth form with adjusted constants 1/sqrt(f) = 1.930 log10(Re sqrt f) - 0.537 (smooth-pipe data 10 < Re < 1e8; turbulent regime 4000 < Re < 1e8); Blasius f = 0.3164 Re^-1/4, "valid up to Re 100000"; Haaland and Swamee-Jain forms; Moody accuracy +-5 % smooth, +-10 % rough; laminar f = 64/Re, transition 2300 to 4000. Moody, Colebrook, Prandtl originals not read; no numeric Moody table or published Colebrook check values obtained (a web search tool's own arithmetic, 0.0222 and 0.0180, is NOT a source and is not used).
* Dean (1978), J. Fluids Eng. 100: paper not read. arXiv:1405.1750 Appendix A (raw text): Cf = alpha Re_b^-1/4, alpha = 0.0614, Re_tau = sqrt(alpha/2) Re_b^(7/8) = 0.1752 Re_b^(7/8), Re_b based on the half-width (laminar Cf = 6/Re_b), Table 1: (Re_tau, Re_b) = (100, 1413), (150, 2247), (200, 3121), (300, 4961), (450, 7885), (600, 10954), with printed U_b/u_tau 14.4, 15.0, 15.6, 16.5, 17.5, 18.3. Reproduced by the test to 0.5 % (the first row's printed 14.4 against 14.13 from Eq. A1 is the paper's own inconsistency or an extraction artefact). The 0.073 Re_(2h)^-1/4 form of Dean's abstract is a search snippet only.

### 5. Tube Graetz

Clarkson CH490 lecture notes https://lin-web.clarkson.edu/projects/subramanian/ch490/notes/Graetz%20Problem.pdf (raw text): eigenvalues of the circular-tube constant-wall-temperature problem lambda_n = 2.7044, 6.6790, 10.673, 14.671, 18.670 (lambda_n^2 = 7.3136, 44.609, 113.92, 215.24, 348.57), Nu_inf = lambda_1^2/2 = 3.656 (Nu = 2 h R/k). An independent series solution in the test reproduces them to 2e-4. The local Nusselt table (Shah and London, Hausen 1943, Sellars-Tribus-Klein) was NOT obtained; the series coefficients A_n were not obtained either.

### 6. NACA 0012

TMR page https://tmbwg.github.io/turbmodels/naca0012_val.html (fetcher): Re = 6 million per chord, run M = 0.15 in compressible codes; Cp and Cf compared at alpha = 0, 10, 15; Ladson tripped force data judged most appropriate, Gregory and O'Reilly pressure data (stated Re 3 million on the page; 2.88 million in the file header) better resolved. Data files read from the F-125 pack (already pinned, CC0, TMBWG/turbmodels commit 20a39f549dbff988a6accc421ac84dafc4487695): `CLCD_Ladson_expdata.dat` (Ladson, NASA TM 4074 (1988), Re 6e6, M 0.15, tripped; zones 80, 120 and 180 grit), `CP_Gregory_expdata.dat` (Gregory and O'Reilly, NASA R&M 3726 (1970), digitised from a photocopy, only approximate, upper surface only, alpha 0/10/15), `CP_Ladson.dat` (Ladson, Hill and Johnson NASA TM 100526 (1987), M 0.3, model aspect ratio 1.333, possibly not sufficiently two-dimensional). Stored subsets: Ladson 80 grit, 17 rows (alpha -4.04 to 19.08, Cl and Cd; the last two rows are post-stall), Gregory alpha = 0, 25 rows. Not read: AGARD AR-211, Vassberg and Jameson (2010), the transonic M 0.8 / alpha 1.25 deg shock positions of the older list.

### 7. Other cheap items

* Stefan-Boltzmann constant: NIST CODATA 2022 page, 5.670 374 419... x 10^-8 W m^-2 K^-4, exact: verified.
* Fin with convective tip: Wikipedia "Fin (extended surface)", Q = sqrt(h P k A_c) theta_b (sinh mL + (h/mk) cosh mL)/(cosh mL + (h/mk) sinh mL), m^2 = h P/(k A_c); adiabatic tip tanh mL: verified (secondary).
* Parallel-plate uniform-flux Nu = 8.2353: already verified in wave 30C (NPTEL); the stale "unverified" text of the wave 18 table was updated.
* Schiller-Naumann switch at Re 1000: still unverified (see caveat).
* Not attempted: Robertson digits (Bari report password protected earlier), Carreau-Yasuda/Cross, TVD/NVD limiter citations, partitioning algorithms, Wilke weights, GRI-Mech licence, equations of state constants, Popovac and Hanjalic formulas.

## Discrepancies found (no existing value or test changed)

1. Ghia v Re 400, x = 0.5: 0.05186 (gist, repository) against 0.05188 (thesis); stored 0.05186.
2. Ghia v Re 1000, x = 0.9063: -0.5155 (repository, thesis) against -0.51500 (gist); stored -0.5155.
3. Ghia v Re 400, x = 0.9063: -0.23827 is the known misprint of the original table (kept as printed).
4. Gartling Re 800 lengths: 12.20 / 9.70 / 20.96 (MFIX-Exa, arXiv:2507.16509 for 12.20) against 11.9 / 9.5 / 20.6 (AMCA 2013).
5. St(100): CSV "Williamson (experiment)" 0.165 (1996 review through arXiv:2303.09262) against the 1989 fit 0.164335.
6. Dean Table 1, Re_tau 100: printed 14.4, Eq. (A1) gives 14.13.
7. Dean's relation overestimates the Re_tau 178 DNS friction of `validation_channel_re180.rs` by 3.8 % (0.008446 against 0.008136), as the source itself warns for low Re_tau.

## Data not obtained

Ghia original; Gartling original; Armaly numeric reattachment lengths; Kim and Moin, Erturk originals; Henderson (1995) St; Moody, Colebrook, Prandtl, Dean originals and any numeric Moody table; Shah and London, Hausen, Sellars-Tribus-Klein local Nu table for the tube, and the Graetz series coefficients; AGARD AR-211 and any compressible NACA 0012 surface data; Schiller-Naumann Re 1000 switch.

## Consequences

* Moved to verified: Ghia u for Re 100/400/1000 including the 0.9766 rows (three secondary copies), v Re 100 (three copies), Williamson 1989 fit and St(100) = 0.1643 (primary), Armaly's definitions and expansion ratio (primary text), Gartling Re 800 lengths (secondary, two sources agree, one conflicts), Colebrook-White/Blasius/Karman-Prandtl/Haaland/Swamee-Jain formulas (secondary), Dean's relation and table (secondary), circular-tube Graetz eigenvalues and Nu_inf (secondary), NACA 0012 TMR data files and conditions (read), Stefan-Boltzmann (NIST), fin formula (secondary).
* Nothing in the registered validation limits of ADR 0060 and 0061 changes.
