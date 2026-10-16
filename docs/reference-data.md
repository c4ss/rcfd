# Reference data audit (wave 6a)

Every benchmark value or published constant that earlier waves typed from memory, checked against a
citable public source. "Verified" means the value in the repo agrees with the source; "secondary" means
the source is a reputable paper, vendor guide or repository that reproduces the primary table, not the
original printed table (the original journal papers were not accessible online). Nothing was loosened:
no tolerance was changed, and no benchmark number in a data file or assertion turned out to be wrong.

Summary (counted by table row): about 34 verified, 3 mismatched, about 8 unverifiable or not re-checked (wave 6a counts; see the wave 13 note below and `docs/verification-report.md` for the later re-check).
The mismatches are one sign error in a test helper (Taylor-Green initial pressure) and two citation or
label errors (Darwish-Moukalled journal; flat-plate "Prandtl-Schlichting" label); all were corrected in
comments, notes or the test helper, and every affected test still passes.

Wave 13 follow-up (the unverified/from-memory markers below are updated in place where re-checked): items that were marked unverified or from memory (turbulence-model and wall constants,
EOS critical data, Peng-Robinson/Redlich-Kwong constants, NIST isotherm values, Sutherland, atomic weights,
backward-facing-step and laminar-step values, Milne, Robertson, flame temperature, Formulation 1A, rheology
constants) were re-checked against web sources; per-item values, URLs and verdicts are in
`docs/verification-report.md`. One code constant differs from the original (Realizable A0 4.04 vs 4.0).

## Lid-driven cavity (Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411)

Used in `cases/cavity/reference/*.csv`, `crates/rcfd-fv/tests/cavity.rs`, `rcfd-io` cases test.

| Value | Source | Result |
|---|---|---|
| u centreline, Re 100/400/1000, 16 rows y = 0.0547 ... 0.9688 (48 numbers) | (a) a student course repository, github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 `data/validation/ghia/ghia_Re{100,400,1000}_u_centerline.csv` (this file omits y = 0.9766); (b) Uppsala bachelor thesis diva2:1668016, Table 2, which prints all 17 rows for Re 100/400/1000 (its y labels are shifted by one row by the PDF text extraction, but the value columns match our CSV row for row in order); (c) for Re 1000 also arXiv:1807.04896 Table A-1, "Ghia, Table 1, p. 398" | verified, two independent secondary copies, exact match of every number |
| u at y = 0.9766: 0.84123 / 0.75837 / 0.65928 | thesis (b) and, for Re 1000, arXiv:1807.04896 | verified (Re 1000 twice, Re 100/400 once) |
| v centreline Re 100, 17 rows | course repository `ghia_Re100_v_centerline.csv`; thesis (b) v-table (same shifted-label extraction; values in order -0.05906 ... 0.09233 match, including 0.05454, 0.17527, -0.24533) | verified, two secondary copies, exact match |
| Only Re 100 v is stored; Re 400/1000 v are not stored | n/a | not applicable |

Side note: both secondary copies carry the known printed misprint of the Re=400 v table at x = 0.9063
(-0.23827; the thesis marks it in red). rcfd does not use Re 400 v.

## Differentially heated cavity (de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264)

Used in `cases/heated_cavity/reference/devahldavis1983.csv`, `crates/rcfd-fv/tests/natural_convection.rs`
(REFERENCE table), `crates/rcfd-api/src/sim.rs` (Nu = 1.118 comment).

| Value | Source | Result |
|---|---|---|
| Nu_avg 1.118, 2.243, 4.519, 8.800 | MOOSE/Cardinal V&V page (cardinal.cels.anl.gov/vv/square_cavity_natural_convection.html) "Benchmark" column; arXiv:physics/0305049 Table 1 (Ra 1e4..1e6); arXiv:1412.2866 Table 2 | verified, secondary |
| u_max 3.649, 16.178, 34.73, 64.63 | Cardinal V&V page; arXiv:1412.2866 Table 2 | verified, secondary |
| v_max 3.697, 19.617, 68.59, 219.36 | Cardinal V&V page; arXiv:1412.2866 Table 2 | verified, secondary |
| Nu_max 1.505, 3.528, 7.717, 17.925 | arXiv:1412.2866 Table 2 (benchmark row, which prints 3.328 for Ra=1e4, an evident typo: its own quoted error of 0.094 percent against 3.5313 implies 3.528) | verified, secondary |
| Pr = 0.71, velocity scale alpha/L | Cardinal V&V page (Davis and Jones 1983 set-up) | verified |

Locations of the maxima (not used by tests) were not copied.

Wave 19A re-check (`crates/rcfd-fv/tests/coupled_open.rs` B3, Ra = 1e3 and 1e4 values 1.118 / 3.649 / 3.697 and 2.243 / 16.178 / 19.617): the
two arXiv papers were downloaded and read as text; arXiv:1412.2866 Table 2 prints Nu 1.118 and 2.243, u_max 3.649 and 16.178, v_max 3.697
and 19.617 for the benchmark row, and arXiv:physics/0305049 attributes Nu 2.243, 4.519 and 8.800 to de Vahl Davis. The Cardinal V&V page
answered with HTTP 403 in this wave and was not re-read. The 1983 printed tables remain unreachable, so the values stay *verified through
secondary sources* only. The coupled-solver tests claim agreement with these numbers within 2 percent (measured 0.02 to 0.74 percent),
nothing more.

## Exact Riemann solver, Sod problem and oblique shock

Used in `crates/rcfd-fv/src/riemann.rs` tests, `compressible.rs` wedge tests.

| Value | Source | Result |
|---|---|---|
| Sod (rho,u,p) = (1,0,1) / (0.125,0,0.1): p* = 0.30313, u* = 0.92745 (also rho*L 0.42632, rho*R 0.26557) | arXiv:2108.01709 (analytic Sod solution table) | verified, secondary |
| Attribution "Toro (2009) Table 4.3 test 1" | Toro's test-suite Test 1 is the *modified* Sod with uL = 0.75 (Toro lab handout, edanya.uma.es) | table number unverifiable, removed from the comment |
| theta-beta-M relation tan(theta) = 2 cot(beta)(M^2 sin^2 beta - 1)/(M^2(gamma + cos 2beta) + 2) | Wikipedia "Oblique shock" (same form as Anderson ch. 4) | verified |
| M = 2, theta = 10 deg, gamma 1.4: weak beta = 39.31 deg | independent bisection solve (39.314 deg; strong 83.70, theta_max 22.97 deg, p2/p1 = 1.7066); an online oblique-shock calculator quotes 39.3 deg and p2/p1 about 1.71 by normal-shock relations | verified (computed + corroborated) |
| theta = 30 deg at M = 2 detaches | theta_max = 22.97 deg from the solve | verified |
| NACA 1135 chart attribution | not accessed | unverifiable, removed from the comment |
| Reference list entries Roe 1981 JCP 43, Harten 1983 JCP 49, Liou 1996 JCP 129, Shu-Osher 1988 JCP 77, Toro et al. 1994 | bibliographic recollection | wave 13: Roe, Harten, Liou, Shu-Osher confirmed via Crossref; Toro 1994 not re-checked |

Corrected: `compressible.rs` cited "Darwish and Moukalled, Numer. Heat Transfer B 44 (2003)"; the TVD paper
is Int. J. Heat Mass Transfer 46(4) (2003) 599-611 (confirmed by multiple citing sources). Fixed the doc comment.

## Taylor-Green vortex (`crates/rcfd-fv/src/pimple.rs` tests)

| Value | Source | Result |
|---|---|---|
| u = sin x cos y e^{-2 nu t}, v = -cos x sin y e^{-2 nu t}; kinetic energy decays as e^{-4 nu t} | arXiv:2603.18222 and Wikipedia "Taylor-Green vortex" | verified |
| p = +(1/4)(cos 2x + cos 2y) e^{-4 nu t} for that velocity form | same sources; also derived: u.grad u = (1/2) sin 2x so p_x = -(1/2) sin 2x | **mismatch (sign)**: the test helper initialised p with a minus sign |

Corrected the initial pressure sign in the test helper (`taylor_green`, in the `#[cfg(test)]` module of
`crates/rcfd-fv/src/pimple.rs`). All 5 pimple tests pass with unchanged tolerances. Measured effect on
`taylor_green_decay_below_one_percent` (release build, 64x64, central, dt 0.02, t = 2):

| initial p sign | KE error | field amplitude error |
|---|---|---|
| old (-) | 0.2700 percent | 0.2175 percent |
| corrected (+) | 0.2991 percent | 0.2259 percent |

Both are far inside the 1 percent gate. The F-029 roadmap note quotes 0.38 percent, which matches neither
(it was recorded on an earlier build), so the recorded figure is stale in either case.

## Turbulence model constants (`crates/rcfd-turb/src/kepsilon.rs`, `wall.rs`)

| Value | Source | Result |
|---|---|---|
| Standard: Cmu 0.09, C1 1.44, C2 1.92, sigma_k 1.0, sigma_eps 1.3 (Launder and Spalding, Comput. Methods Appl. Mech. Eng. 3 (1974) 269-289) | Wikipedia "K-epsilon turbulence model" | verified |
| RNG: Cmu 0.0845, C1 1.42, C2 1.68, eta0 4.38, beta 0.012, C2* = C2 + Cmu eta^3 (1 - eta/eta0)/(1 + beta eta^3); sigma = 0.7194 (= 1/1.3929) | ANSYS Fluent 12 theory guide an unofficial copy (not linked) ((unofficial copy, not linked) mirror); arXiv:2501.00037 | verified |
| Realizable: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(eta+5)), Cmu = 1/(A0 + As U* k/eps), As = sqrt(6) cos(phi) | Fluent theory guide an unofficial copy (not linked); OpenFOAM `realizableKE` class docs (default A0 4.0, C2 1.9, sigma 1.0/1.2) | verified. Note A0 = 4.04 in Fluent (used by rcfd, `kepsilon.rs`) but 4.0 in OpenFOAM and, per one forum post, in Shih et al.; the original paper's printed value could not be fetched. Flagged in the source comment. **Wave 13 verification: Shih et al., NASA TM-106721 (https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf) prints A0 = 4.0 and Table 2 C2 1.9, sigma 1.0/1.2, C1 = max(0.43, eta/(5+eta)); rcfd's 4.04 differs from the original (see `docs/verification-report.md`).** ADR 0017: `KEpsilon::a0` / `turbulence.realizable_a0` selects 4.0 (original report) or 4.04 (a commercial-code variant, still the default); BFS-C medium x_r/h 6.349 with 4.0 against 6.413 with 4.04. |
| Wall function kappa = 0.4187, E = 9.793; rounded to 0.41 / 9.8 by rcfd | ANSYS Fluent docs (Launder-Spalding standard wall functions) | verified (rounding disclosed in code) |
| Log law kappa = 0.41, B = 5.2 (Pope 2000) | problem set based on Pope (perso.ens-lyon.fr), Abe et al. 2001; Wikipedia gives kappa 0.41, C+ 5.0 | verified, within the usual 5 percent spread |
| Reichardt (1951): u+ = ln(1 + 0.41 y+)/0.41 + 7.8[1 - e^{-y+/11} - (y+/11) e^{-y+/3}] | CFD-Online wiki "Reichardt profile" (via search result); numerical cross-check against MKM DNS | verified, and within 0.7, 0.8, 2.3, 2.3, 1.4 percent of DNS at y+ = 20, 30, 50, 100, 150 |
| Inlet relations k = 1.5 (I U)^2, eps = Cmu^0.75 k^1.5 / l, l = 0.07 L (`rcfd-api/src/turb.rs`) | CFD-Online/CFX forum guidance; l factor 0.07 for mixing-length codes | verified (k and l); eps = Cmu k^2/(nu * ratio) is the definition nu_t = Cmu k^2/eps, no external source needed |

## k-omega SST and wall treatment (`crates/rcfd-turb/src/sst.rs`, `wall.rs`)

| Value | Source | Result |
|---|---|---|
| beta* 0.09, a1 0.31, sigma_k1 0.85, sigma_w1 0.5, beta1 0.075, gamma1 5/9; sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828, gamma2 0.44; P limited to 10 beta* rho omega k; CDkw = max(2 rho sigma_w2/omega dk.dw, 1e-10); F1 = tanh(arg1^4), F2 = tanh(arg2^2); nu_t = a1 k/max(a1 omega, S F2), S = sqrt(2 SijSij); wall omega = 10 * 6 nu/(beta1 d1^2) | NASA Turbulence Modeling Resource, "The Menter Shear Stress Transport Turbulence Model" (tmbwg.github.io/turbmodels/sst.html; the turbmodels.larc.nasa.gov address redirects); original: Menter, Kuntz and Langtry (2003) | verified against the TMR page by web fetch (2003 formulation, with the TMR production limiter) |
| TMR flat plate set-up: Re_L = 5e6, M = 0.2, k_inf = 9e-9 a^2 (= 2.25e-7 U^2), omega_inf = 125 U/L (nu_t/nu = 0.009); CFL3D SST Cf(x = 0.97) = 0.00268299 (273 x 193 grid; the finest-grid value is not printed on the page) | tmbwg.github.io/turbmodels/flatplate_sst.html | verified; used in `tests/flat_plate_sst.rs`; rcfd gives 0.002663 (-0.7 percent) |
| Spalding (1961) law of the wall: y+ = u+ + exp(-kappa B)[exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6] | Spalding, J. Appl. Mech. 28 (1961) 455-458, from memory of the standard form (checked by the `wall.rs` unit test against the log law) | confirmed (secondary) in wave 13: same cubic form in libWMLES docs (libwmles.readthedocs.io) and arXiv:2104.09054 (which prints one more term); citation confirmed via Crossref; original paper not read |
| Compound viscous/log epsilon of the smooth k-epsilon wall treatment, eps = sqrt(eps_vis^2 + eps_log^2) | Popovac and Hanjalic, Flow Turb. Combust. 78 (2007) 177-202, from memory; the blended omega of the SST wall cell follows Menter and Esch (2001) in spirit | citation confirmed via Crossref (wave 13); formulas remain unverified (rcfd's own implementation of the idea) |
| White flat-plate friction Cf = 0.455/ln^2(0.06 Re_x), used for y+-targeted first cell height | see Flat plate below | verified |

## Channel DNS (Moser, Kim and Mansour, Phys. Fluids 11 (1999) 943)

Used in `crates/rcfd-turb/tests/channel_kepsilon.rs`, `cases/channel/case.json`.

Source: the published statistics file `https://turbulence.oden.utexas.edu/data/MKM/chan395/profiles/chan395.means`
(129 half-channel points, parsed and integrated with the trapezoid rule).

| Value in repo | Computed from MKM data | Result |
|---|---|---|
| U_c+ = 20.1 | 20.133 | verified |
| U_b+ = 17.4, Re_b = 2 h U_b/nu = 13750 | U_b+ = 17.545 (relative to the true u_tau, Re_tau = 392.24), Re_b = 13764 | verified. 17.4 equals Re_b/(2 x 395), which is the consistent value for the nominal Re_tau = 395 imposed in the test (nu = 1/395); 17.54 applies at 392.24. Left unchanged. |
| "Re_tau = 395" | actual 392.24 (nominal 395) | note added to the test and the case citation |

Profile samples (y+: U+): 5: 4.82, 10: 8.44, 15: 10.657 (interpolated from the same chan395.means file for the SST test, verified from the file), 20: 12.007, 30: 13.493, 50: 14.856, 100: 16.534, 150: 17.646, 300: 19.745.

## Cylinder wake (`crates/rcfd-fv/tests/cylinder.rs`)

| Value | Source | Result |
|---|---|---|
| St = 0.165 at Re = 100 | Williamson, Annu. Rev. Fluid Mech. 28 (1996), reported as 0.165 in the Re = 100 table of arXiv:2303.09262; same table: Kravchenko et al. 0.164, Posdziech and Grundmann 0.164, Sivakumar et al. 0.164, Oliveira 0.167; ANSYS validation manual target 0.165 | verified within 1 percent |
| "Williamson 1989 about 0.164" and "Henderson 1995 about 0.165" | no accessible source gives these particular values | unverifiable; comment reworded to the verified attribution |

The 5 percent tolerance is unchanged; the recorded run of 0.1588 is -3.8 percent from 0.165 (-3.2 percent from 0.164).

## Backward-facing step (`cases/backward_step/case.json`)

| Value | Source | Result |
|---|---|---|
| Reattachment x/H = 6.26 +/- 0.10 (Driver and Seegmiller, AIAA J. 23 (1985) 163) | NASA TMR backstep validation page (tmbwg.github.io/turbmodels/backstep_val.html); OpenFOAM docs (doc.openfoam.com 2306 backward-facing-step) | verified |
| Re_H about 36000 | same pages | verified |
| Expansion ratio 1.125, inlet channel 8h | not stated on either fetched page; figure only. Wave 13: NASA GRC WIND page (grc.nasa.gov/WWW/wind/valid/backstep/backstep01/backstep01.html) gives H = 0.5 in and step:exit height 1:9, i.e. exit 9H, inlet 8H, ratio 9/8; ERCOFTAC case 30 snippet agrees (1.27 cm step, 10.1 cm inlet) | confirmed (secondary; ERCOFTAC page itself not fetched) |
| "5.5 to 7 step heights for RANS" scatter | no source found (wave 13 searched again) | unverifiable |

## Laminar backward-facing step, expansion ratio 2 (`crates/rcfd-fv/tests/step_laminar.rs`)

Convention: the tests use Re_h = U_mean h / nu (h = step height = inlet channel height); the literature below uses
Re = U_mean D_h / nu with D_h = 2h, so Re_literature = 2 Re_h.

| Value | Source | Result |
|---|---|---|
| Re = 800 (Re_h = 400): x_r/h = 11.834 (Erturk, Comput. Fluids 37 (2008) 633) and 12.20 (Gartling, Int. J. Numer. Methods Fluids 11 (1990) 953) | Table 2 of arXiv:2507.16509 (a 2025 MHD backward-step paper that quotes both), fetched 2026-10-14 through a page fetcher with an LLM summariser; the original papers were not accessed | secondary; wave 13 re-read Table 2 of https://arxiv.org/pdf/2507.16509 as raw text (Erturk 11.834, Gartling 12.20, Gresho 12.20) and confirmed the citations via Crossref; originals still not accessed. Used by `step_reattachment_re_h_400_vs_erturk_and_gartling` (5 % windows); measured 11.672 (-1.4 % / -4.3 %), pass |
| Re = 200 and 400 (Re_h = 100, 200): x_r/h = 4.95 and 8.10 | Table 1 of the same paper: the finite-volume results of its authors (ER 2, 2-D), not a published benchmark | secondary, unverified against Armaly/Gartling/Erturk; used only as an independent-code consistency check (5 % windows); measured 4.949 and 8.182 (+1.0 %), pass |
| Armaly et al., J. Fluid Mech. 127 (1983) 473: reattachment lengths versus Re | no numeric table accessible here (the abstract gives the Re range 70-8000 only); the experiment is 3-D with expansion ratio 1.942 and differs from the 2-D laminar computations above beyond Re about 400 | unverifiable here; no assertion uses it |
| "Gartling Re 800 about 12.2, Armaly about 12" (F-106 note, from memory) | superseded by the Gartling value above | the Armaly value stays unverified |

## Flat plate (`cases/flat_plate/reference/correlations.csv`)

| Value | Source | Result |
|---|---|---|
| Blasius Cf = 0.664/sqrt(Re_x) | Jaffer, "Skin-friction and forced convection from rough and smooth plates" (people.csail.mit.edu/jaffer/convect/rough.pdf) citing Schlichting; several search results | verified |
| Cf = 0.0576 Re_x^-0.2, 5e5 < Re_x < 1e7 | CFD-Online wiki "Skin friction coefficient" (via search result) | verified; other textbooks use 0.0592 (noted in the CSV) |
| Cf = 0.455/ln^2(0.06 Re_x) | Jaffer (White's local formula; 0.523 is the plate average) | verified; the CSV mislabelled it "Prandtl-Schlichting", which is 0.455/log10(Re)^2.58 (average). Label corrected |

## NACA 0012 (`cases/naca0012/reference/naca4_thickness.txt`)

| Value | Source | Result |
|---|---|---|
| y_t = 5t(0.2969 sqrt x - 0.1260 x - 0.3516 x^2 + 0.2843 x^3 - 0.1036 x^4) (closed TE; open TE uses -0.1015) | Wikipedia "NACA airfoil" | verified |
| Thin-airfoil C_L = 2 pi alpha, 0.110 per degree | standard; 2 pi pi/180 = 0.1097 | verified (the 0.11/deg is an indicative target for real sections, which fall slightly below it) |

## Other cited literature (bibliographic details only)

| Citation | Result |
|---|---|
| Patankar, Liu and Sparrow, J. Heat Transfer 99(2) (1977) 180-186 (`simple.rs`) | verified |
| Darwish and Moukalled, IJHMT 46(4) (2003) 599-611 | verified, comment corrected (see above) |
| Issa 1986 JCP 62; Choi 1999 Numer. Heat Transfer A 36; Ericson RTCD 5.1.5 | Issa confirmed via Crossref (wave 13); Choi and Ericson not confirmed |

## Not verifiable online

- Original printed tables of Ghia 1982 and de Vahl Davis 1983 (only secondary reproductions were reachable); the
  Ghia Re 100/400 values at y = 0.9766 rest on a single source.
- The "5.5 to 7" scatter statement (the expansion ratio 1.125 was confirmed in wave 13).
- Williamson 1989 / Henderson 1995 St values and the Williamson fit coefficients.
- Toro Table 4.3 / NACA 1135 attributions.
- (Resolved in wave 13: Shih et al. print A0 = 4.0; rcfd uses 4.04.)
- Choi 1999 bibliographic details (Roe, Harten, Liou, Shu-Osher, Issa were confirmed via Crossref in wave 13).

## Public validation-data pack (F-125, fetched 2026-10-13)

Files are listed with checksums and licences in `validation-data/manifest.json` (see `docs/validation-data.md`).

- Flat plate Cf at x = 0.97: the pinned TMR file `FlatPlate/SST/cf_plate_sstv.dat` (CFL3D, SST) gives about 2.691e-3
  (2.6949e-3 at x = 0.9601, 2.6909e-3 at x = 0.9701). `crates/rcfd-turb/tests/flat_plate_sst.rs` uses 0.00268299
  (273 x 193 grid value from the TMR page). rcfd measures 0.002663: -0.7 % vs the page value, -1.0 % vs the file. Test tolerance (5 %) unchanged.
- Rotor 37 values from NASA TP-1337: only the report is stored; no numbers are transcribed yet. Unverified until they are.
- GRI-Mech 3.0 (Berkeley copy): licence status unverified (no licence text on the page). Cantera BSD copy preferred.
- OpenFOAM damBreak set-up files: GPL-3.0-only, fetched only, never vendored.
- Hannover compressor: no open upstream found; unresolved.

## Fluent mesh format details (F-118)

All from memory of the public format description and not checked against a primary source in this session, except
where a real file confirmed them. Treat as unverified.

- Boundary type ids: 2 interior, 3 wall, 4 pressure-inlet, 5 pressure-outlet, 7 symmetry, 8 periodic-shadow, 10 velocity-inlet,
  12 periodic, 20 mass-flow-inlet, 31 parent (hanging node), 36 outflow, 37 axis. The names in section 45 take precedence. The real file
  `elbow.msh` confirmed 2 (interior), 3 (wall), 5 (pressure-outlet), 10 (velocity-inlet) with names and the 2D layout
  (`(10 (zone first last type nd)`, face records `n0 n1 right-cell left-cell`, hex numbers in headers).
- Section 18 as periodic shadow faces; the binary section ids 2010/3010 (nodes single/double), 2012/3012, 2013/3013 and the
  `End of Binary Section` terminator; little-endian 32-bit integers in binary face data. No real binary file was read.

## Viscosity laws (F-110)

The law forms in `crates/rcfd-api/src/rheology.rs` are textbook expressions written from memory of the literature
(Bird, Armstrong and Hassager 1987; Carreau 1972; Yasuda 1981; Cross 1965; Herschel and Bulkley 1926; Papanastasiou
1987; Williams, Landel and Ferry 1955) and were not checked against the papers in this session. Unverified:
(wave 13: the Carreau-Yasuda `(n-1)/a` convention and the WLF constants 17.44 and 51.6 K are now confirmed, secondary: arXiv:1810.02292, IUPAC Gold Book goldbook.iupac.org/terms/view/12821; Cross and Papanastasiou remain unverified) the Carreau-Yasuda exponent convention `(n-1)/a`, the Cross form `1/(1+(lambda g)^m)`, the regularised yield term
`tau0 (1 - exp(-m g))/g`, and the WLF constants 17.44 and 51.6 K (the "universal" values, relative to the glass
transition temperature, offered only as defaults). The checks that do not depend on those forms are analytic:
the power-law plane channel profile and pressure gradient (`u = U (2n+1)/(n+1) [1 - |1-2y/H|^((n+1)/n)]`, derived by
integrating the momentum balance), and the Buckingham-Reiner plane solution of a Bingham fluid
(`U = G h^2/(3 mu) (1 - 1.5 xi + 0.5 xi^3)`, `xi = tau0/(G h)`, mean speed verified to 1e-6 by quadrature of the
profile in the test).

## Material property library (F-111)

Verified against primary sources in this session (fetched 2026-10-13):

- IAPWS-IF97 (IAPWS R7-97(2012), `iapws.org`): region 1 and region 2 coefficient tables, the B23 and saturation-pressure
  equations, and the property relations were read from the release itself; the program-verification values of Tables 5, 15 and 35
  (all three state points each) are reproduced to 1e-8 relative in `crates/rcfd-core/src/iapws.rs`, which also validates the
  transcription of the coefficients. IAPWS R12-08 (viscosity, Tables 1, 2, verification Table 4) and R15-11 (thermal conductivity,
  Tables 1, 2, verification Table 4) likewise; their tables carry 7-9 digits, so the test tolerance there is 1e-7 relative
  (set at 1e-8 first, widened to 1e-7 after the 433.15 K, 1 kg/m^3 viscosity point missed it by 3.4e-8, which is the rounding of its
  last printed digit: a post-hoc change, disclosed). The viscosity assert for Sutherland air is a regression guard of 1.5 % set after
  the measurement; the 1 % criterion itself is recorded as missed. The 1 % assert on the cubic fits is in-sample.
  Not implemented: region 3, region 5, backward equations, critical enhancements.
- NASA polynomials (NASA TM-4513, McBride, Gordon and Reno 1993): coefficients for N2, O2, Ar, H2O, CO2, CH4, H2 copied from
  Cantera's `data/nasa_gas.yaml` (BSD-3-Clause, the file cites TM-4513); the Cantera licence requires keeping its notice,
  which is in the source header comment of `fluids.rs`. Consistency check: the two temperature ranges agree at 1000 K.
- Air viscosity: M. L. Huber, "Viscosity of Gases", CRC Handbook of Chemistry and Physics (NIST publication page, pub_id 926439), air at
  100 kPa: 13.3, 18.5, 23.1, 27.1, 30.8 uPa s at 200-600 K (read from the rendered table; 3 digits). Air conductivity: Huber, "Thermal
  Conductivity of Gases" (pub_id 926438): 18.5, 26.4, 33.5, 39.9, 46.0 mW/(m K). Air specific heat: NASA TN D-5452 Table I
  (Poferl, Svehla, Lewandowski 1969), 1.005, 1.013, 1.030, 1.051 J/(g K) at 300-600 K (OCR of the scanned table, values
  consistent with the NASA polynomials to 0.12 %).
- Sutherland constants for air: 1.716e-5 Pa s at 273.15 K with S = 110.4 K, and 0.0241 W/(m K) with S = 194 K, are the
  usual textbook values written from memory (the viscosity pair also is the US Standard Atmosphere 1976 pair); only their
  agreement with the Huber tables is checked. Wave 13: confirmed (secondary) against CFD-Online "Sutherland's law" and the
  1976 standard-atmosphere constants (C1 = 1.458e-6, S = 110.4 K gives 1.7161e-5 at 273.15 K), and arXiv:2210.01621 for the
  conductivity pair (2.41e-2 W/(m K), T0 273 K, S 194 K). Result, pre-registered at 1 % for 300-500 K: viscosity -0.22 % (300 K),
  **-1.08 % (400 K), -1.46 % (500 K)** (also -0.11 % at 200 K, -2.07 % at 600 K): the 1 % criterion is NOT met by
  Sutherland's law for the viscosity above about 350 K; conductivity -0.64, +0.26, +0.69 % (also -3.23 % at 200 K, +0.35 % at
  600 K) is met. The cubic fits `air_viscosity_fit_law`/`air_conductivity_fit_law` were fitted to the same five table rows
  (in-sample residual at most 0.11 % and 0.08 %; leave-one-out prediction of the 400 K row -0.22 % and -0.15 %).

Unverified (from memory, not checked against a primary source in this session; the wave 13 outcome is appended to each item):

- Standard atomic weights used for the molar masses (N 14.007, O 15.999, Ar 39.948, H 1.008, C 12.011). Wave 13: all inside the CIAAW intervals (ciaaw.org), confirmed.
- Dry-air mole fractions 0.78084 / 0.20946 / 0.00934 (N2/O2/Ar). Wave 13: NIST CIPM-2007 gives 0.780848 / 0.209390 / 0.009332; code differed by up to 0.09 % (immaterial).
  Post-verification fix: `GasMixture::dry_air` now uses the CIPM-2007 values 0.780848 / 0.209390 / 0.009332 (renormalised over the three species); its only user is the air cp/transport unit test, whose cp values moved by about 0.01 J/kg/K, all assertions unchanged and passing.
- Peng-Robinson constants 0.45724, 0.07780, kappa = 0.37464 + 1.54226 omega - 0.26992 omega^2 and Redlich-Kwong constants
  0.42748, 0.08664, and the Wilke pair weight, as published by Peng and Robinson (Ind. Eng. Chem. Fundam. 15, 1976), Redlich and Kwong
  (Chem. Rev. 44, 1949) and Wilke (J. Chem. Phys. 18, 1950); the tests use only properties that follow from the forms
  themselves (critical-point conditions, ideal limit, root consistency, thermodynamic consistency of fugacity and residual enthalpy,
  the acentric-factor definition of the vapour pressure at reduced temperature 0.7 within 5 %: measured +0.38, -0.35, -0.23 %). The
  critical constants of N2 used in the tests were illustrative (126.2 K, 3.39 MPa); they are now the NIST values 126.192 K, 3.3958 MPa (omega 0.0372), tests unchanged and passing.
  Wave 13: PR 0.45724/0.07780/kappa polynomial confirmed (Wikipedia citing Peng and Robinson 1976; exact 0.457236, 0.077796
  computed), RK 0.42748/0.08664 confirmed (Wikipedia and closed form); the Wilke weight remains unverified. The N2 test constants
  differed from NIST (126.192 K, 3.3958 MPa) as stated; fixed afterwards, see above.
- NIST WebBook data was deliberately not used (its terms were not reviewed); no REFPROP or other licensed file is read or shipped.
  (See the F-061 section below: later waves did embed WebBook values; wave 13 notes that NIST SRD 69 is copyrighted, see
  `docs/verification-report.md` section 1.6.)
- Critical constants and acentric factors in `rcfd-core/src/eos_data.rs` (the header comment now points at `docs/verification-report.md`): wave 13
  confirmed all seven fluids against the NIST WebBook fluid pages; only the Ar acentric factor differed (code -0.002, NIST -0.00219); fixed to -0.00219 afterwards (no test uses Ar PR against NIST; workspace tests unchanged).


## Variable-density validation (wave 10, ADR 0005)

Verified against the primary source:

- Le Quere, Weisman, Paillere, Vierendeels, Dick, Becker, Braack and Locke, "Modelling of natural convection flows with
  large temperature differences: a benchmark problem for low Mach number solvers. Part 1. Reference solutions", ESAIM: M2AN
  39(3) 609-616 (2005), DOI 10.1051/m2an:2005027; full text read from the author's postprint
  https://wrap.warwick.ac.uk/id/eprint/3824/1/WRAP_Locke_modelling_natural_convection.pdf. Table 1 (square cavity, adiabatic
  horizontal walls, no slip, P0 = 101325 Pa, T0 = 600 K, R = 287, Pr = 0.71, gamma = 1.4, g = 9.81, epsilon = (Th-Tc)/(Th+Tc) = 0.6,
  Ra = Pr g rho0^2 (Th-Tc) L^3 / (T0 mu0^2) with a single g): T1 (Ra 1e6, constant properties) Nu 8.85978, P/P0 0.856338;
  T2 (Ra 1e6, Sutherland) Nu 8.6866, P/P0 0.924487; T3 (Ra 1e7, Sutherland) Nu 16.2410, P/P0 0.92263 (T3 not used).
  Sutherland: mu = mu* (T/T*)^1.5 (T*+S)/(T+S), T* = 273 K, S = 110.5 K, mu* = 1.68e-5 Pa s, k = mu gamma R / ((gamma-1) Pr).
  Thermodynamic pressure P(t) = P0 integral(1/T0 dv) / integral(1/T dv).
- Quasi-1-D isentropic relations and the area-Mach relation (A/A*)(M) = (1/M) [2/(gamma+1) (1 + (gamma-1)/2 M^2)]^((gamma+1)/(2(gamma-1))),
  choked mass flow A* p0 sqrt(gamma/(R T0)) (2/(gamma+1))^((gamma+1)/(2(gamma-1))): standard textbook formulae (for example
  Anderson, Modern Compressible Flow), not looked up in this session; the code of the checks uses them as written here.
- Hydrostatic ideal-gas columns: analytic atmospheres dp/dy = -rho g with p = rho R T (isothermal exponential; linear T
  polytropic with exponent g/(R a)), derived in the test headers.

## Transient variable density (ADR 0009)

Reference values used by `crates/rcfd-fv/tests/varden_transient*.rs` and `crates/rcfd-api/tests/transient_variable_density.rs`:

- Differentially heated cavity (T4): the steady Le Quere et al. 2005 values of the section above (Nu 8.85978, P/P0
  0.856338, verified from the author's postprint). The transient approach is checked against them; no transient
  reference data exist or are claimed.
- Closed-box heating (T1, T1r), temporal order (T2): closed-form solutions derived in the ADR (uniform heating of a
  closed ideal gas: `p = rho0 R T`, `T = T0 + q t/(rho cv)`; sinusoidal source `T - T0 = q0/(rho0 cv w) (1 - cos w t)`).
  The real-gas energy balance uses the Peng-Robinson internal energy of the solver's own EOS (self-consistency, not an
  independent data set).
- Standing acoustic wave (T3): linear acoustics, `f = a/(2L)` of a tube closed at both ends and the Stokes-Kirchhoff
  decay estimate; textbook results (not looked up in this session; the code uses the formulae as written in the ADR).
  UNVERIFIED against a primary source here.
- Couette flow with viscous heating (T6): `k T'' + mu (U/H)^2 = 0`, profiles `T - T_w = mu U^2/(2k) (y/H)(1 - y/H)` and
  `(y/H)(2 - y/H)`. ADR 0009 cites Bird, Stewart and Lightfoot, Transport Phenomena 2nd ed., secs. 10.4-10.5, and
  Schlichting and Gersten, Boundary-Layer Theory 9th ed., sec. 13.3; neither was accessed in this session. The
  profiles follow from the stated equation by two integrations and the code agrees with them to 1.6e-4 of the
  scale. UNVERIFIED section numbers.
- Patterson and Imberger (1980), J. Fluid Mech. 100, 65: only the order of the boundary-layer filling time
  `L^2/(kappa Ra^(1/4))` is used to choose the 20 s run length of T4; not accessed, no number is compared with it.

## Known misses (assertions added after the 2026-10-14 audit)

Each row is a registered criterion that the code does not meet. The threshold in the test is the registered one, not
widened; the test is `#[ignore]`d and its name ends in `known_miss`, so it fails by design when run with `--ignored`
and will start passing if the solver improves. Measured in release with `CARGO_TARGET_DIR=target-fix2`, no solver
parameter changed.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-042 k-epsilon BFS, Realizable | x_r/h in 6.26 +- 0.10 (Driver and Seegmiller) | 6.402 (0.042 outside) | `rcfd-turb/tests/step_kepsilon.rs::step_reattachment_realizable_known_miss` |
| F-042 k-epsilon BFS, RNG | same | 6.373 (0.013 outside) | `step_reattachment_rng_known_miss` |
| F-042 k-epsilon BFS, Standard | same | 5.359 (0.80 below) | `step_reattachment_standard_known_miss` |
| F-061 nozzle case file | tolerance_u = tolerance_p = 1e-8 within 3000 iterations | res_u 2.62e-5, res_p 6.07e-13, not converged | `rcfd-api/tests/variable_density.rs::subsonic_nozzle_residual_target_1e8_known_miss` |
| F-043 SST flat plate vs White | Cf(0.97) within 5 % of 0.455/ln^2(0.06 Re_x) | 0.002663 vs 0.002875, -7.36 % (-6.6 to -8.5 % along the plate) | `rcfd-turb/tests/flat_plate_sst.rs::sst_flat_plate_cf_within_5pct_of_white_known_miss` |
| F-111 Sutherland air viscosity | within 1 % of Huber table, 300-500 K | -0.22, -1.08, -1.46 % | `rcfd-core/src/fluids.rs::sutherland_air_viscosity_within_1pct_300_to_500_k_known_miss` |

Met and now asserted: F-043 Cf(0.97) within 5 % of the TMR page value 0.00268299 (-0.7 %) and of the TMR file value
0.0026909 (-1.0 %), in `sst_flat_plate_cf` (ignored, heavy, about 6 minutes). The air test that asserted 1.5 % for
Sutherland viscosity was renamed `air_cp_conductivity_fit_laws_within_1pct_and_sutherland_viscosity_within_1p5pct_300_to_500_k`
(the 1.5 % is a regression guard, the 1 % fit-law check is in-sample).


## Compressible boundary conditions (F-062, ADR 0006)

| Value | Source | Result |
|---|---|---|
| Characteristic (LODI) boundary treatment, pressure-relaxation outlet `K (p - p_ref)` | Thompson, J. Comput. Phys. 68 (1987) 1-24; Poinsot and Lele, J. Comput. Phys. 101 (1992) 104-129 | bibliographic details confirmed via Crossref (wave 13); formulas re-derived in ADR 0006, not checked against the papers, unverified |
| Riemann-invariant far field, total-pressure inlet | Blazek, *Computational Fluid Dynamics: Principles and Applications*, ch. 8 | textbook formulas re-derived; book not accessed here, unverified |
| Isentropic and area-Mach relations (nozzle check) | Anderson, *Modern Compressible Flow* | standard textbook formulae, same as F-061 |
| Vassberg and Jameson, J. Aircraft 47(4) (2010) 1152-1166 (Euler NACA 0012 grid convergence) | listed in the reference list of arXiv:2407.03746 (fetched) | citation verified; its Cl, Cd and Cp values were NOT accessed |
| AGARD AR-211 NACA 0012 transonic Cp scatter | not accessed | no numbers available; the F-062 airfoil criterion is therefore not attempted |
| NACA 0012 M = 0.8, alpha = 1.25 degrees upper/lower shock positions (about x/c 0.62 / 0.34 in Euler solutions) | recollection only | UNVERIFIED, not used |


## Real-gas equation of state (F-061, ADR 0007)

NIST Chemistry WebBook, fluid properties, isothermal tables (webbook.nist.gov/cgi/fluid.cgi, CO2 ID C124389, N2 ID C7727379,
default reference state), retrieved 2026-10-14 through a page fetcher whose output passes through a language-model
summariser. The numbers (density, enthalpy, entropy, cp, sound speed at the listed T and p) are copied into
`crates/rcfd-fv/tests/realgas_nist.rs` as returned; they were NOT re-read from the raw page, so they are marked
"retrieved, not independently re-read" (sanity: CO2 350 K 0.1 MPa 1.5167 kg/m^3 and N2 300 K 0.1 MPa 1.1233 kg/m^3 agree with
the ideal gas to 0.3 percent and 0.0 percent). The tables list some rows (8.1 MPa, 5.1 MPa, 6.1 MPa) three times with
different phase labels and identical values.

Wave 13 update: the isotherm values were re-read from the raw WebBook HTML (no summariser) and all match; the critical
constants below were confirmed against the NIST fluid pages (only Ar omega differs); see `docs/verification-report.md`.

Originally UNVERIFIED (written from memory of the literature, not looked up): critical constants and acentric factors in
`rcfd-core/src/eos_data.rs` (CO2 304.1282 K, 7.3773 MPa, 0.22394; N2 126.192 K, 3.3958 MPa, 0.0372; O2, Ar, CH4, H2, H2O
likewise); the Peng-Robinson (1976) and Redlich-Kwong (1949) constants were already listed as unverified under F-111. The
thermodynamic consistency of the departure functions was checked numerically instead (Maxwell relations
`dh/dp|T = v - T dv/dT|p` and `ds/dp|T = -dv/dT|p`, `rcfd-fv/src/realgas.rs` tests).

The real-gas isentropic reference of the nozzle tests is computed in the test from the same EOS (an ODE in p with
`dT/dp|s = T (dv/dT)_p / cp`); it checks the solver against the thermodynamics, not the EOS against nature. EOS accuracy
against NIST is the (R1)-(R3) tests: density within 1 percent for |Z-1| <= 0.05 etc. Misses are in the table below.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-061 PR density vs NIST | within 1 percent where \|Z_NIST - 1\| <= 0.05 | N2 300 K: 5.1 MPa +1.03, 10.1 MPa +1.68, 15.1 MPa +2.24 percent | `rcfd-fv/tests/realgas_nist.rs::r1_peng_robinson_density_vs_nist_known_miss` |
| F-061 PR enthalpy difference vs NIST | within 10 percent | N2 300 K: +18.8 (5.1 MPa), +15.7 (10.1), +12.3 (20.1) percent; CO2 350 K +9.3/+8.5/+7.8, 310 K +2.6/+1.2 pass | `r2_departure_enthalpy_and_entropy_differences_vs_nist_known_miss` |
| F-061 ideal-gas limit at 1 Pa | h_dep <= 1e-8 cp0 T and s_dep <= 1e-8 cp0 | CO2 PR 3.1e-8 / 2.1e-8, RK 2.6e-8 / 1.7e-8 (departures scale with p; limit was mis-estimated) | `r3_ideal_gas_limit_at_one_pascal_known_miss` |


## MRF zones (F-031, F-114, ADR 0008, `crates/rcfd-api/tests/mrf.rs`)

All reference values are analytic (solid-body rotation, Taylor-Couette flow `u_theta = A r + B/r`, torque
`4 pi rho nu W_i r_i^2 r_o^2/(r_o^2 - r_i^2)` per unit height) and derived in the ADR; no literature data and no pump or
turbine benchmark are used or claimed. Set-up r_i 0.5, r_o 1, nu 0.1, 48 cells around. Limits were registered before the
runs; misses are ignored `*_known_miss` tests.

| Check | Measured | Limit | Result |
|---|---|---|---|
| M1 solid body, velocity | 7.84e-3 W r_o | 2e-2 | pass |
| M1 pressure | 7.55e-2 of range | 5e-2 | MISS |
| M2 single MRF zone, velocity (symmetry ends) | 6.76e-3 V_i | 1.5e-2 | pass |
| M2 torque of the `moment` report vs full-stress value | -37.8 % | 3 % | MISS (report omits the transpose term) |
| M2b open ends, velocity / torque vs report value | 6.61e-3 V_i / -0.70 % | 1.5e-2 / 3 % | pass |
| M3 frames 0, 0.5, 1, 2 W_i, velocity; difference to stationary | 6.76e-3 V_i; 4.3e-4, 8.8e-4, 1.7e-3 V_i | 1.5e-2; 1e-2 | pass |
| M3(a) registered stationary frame with no source: velocity / spurious / torque / difference to MRF frames | 3.06e-2 V_i / 1.39e-3 V_i / -27.3 % / 2.57e-2 V_i | 1.5e-2 / 1e-3 / 3 % / 1e-2 | MISS (kernel without H correction) |
| M3 spurious speed | 1.41e-3 (Omega 0), 9.7e-4, 5.3e-4, 3.2e-4 V_i | 1e-3 | MISS at Omega 0 only |
| M3b open ends: velocity, torque, difference; spurious | 6.6e-3, -0.70 %, <= 1.8e-3 V_i; 8.2e-3 to 8.6e-3 V_i | 1.5e-2, 3 %, 1e-2; 1e-3 | pass; spurious MISS |
| M4 across the non-conformal interface, velocity / torque | 7.92e-3 V_i / -37.8 % | 2e-2 / 3 % | pass / MISS (report) |
| M4b open ends, velocity / torque | 7.59e-3 V_i / -0.87 % | 2e-2 / 3 % | pass |
| M5 continuity / net flux / sum abs flux (symmetry ends) | 5.2e-12 / 3.3e-14 / 8.9e-4 | 1e-8 / 1e-8 / 1e-3 | pass |
| M5 in M4b (open ends) net flux / sum abs flux | 3.2e-5 / 1.0003e-3 | 1e-8 / 1e-3 | MISS |
| M6 through-flow: in/out, interface flux, swirl, axial deviation | exact to 1e-10; 2.0e-4; 9.8e-3 | 1e-9; 2e-2; 2e-2 | pass |

The first runs (before the H-operator component-diagonal correction) missed M1-M5 by large margins; see the ADR
disclosures. The correction (`SimpleSolver::component_diag_correction`) is enabled only for cases with an MRF source.


## Interchange formats, registers and GLB validation (F-119, F-124, F-117, ADR 0011)

Limits were registered in the ADR before any code ran. "VTK" is VTK 9.6.2 installed with pip; its readers are the
independent oracle. No commercial viewer was available.

| Check | Measured | Limit | Result |
|---|---|---|---|
| S1 STL ASCII vertex round trip | max abs diff 0 | 0 | pass |
| S2 STL binary round trip, relative error | 6.0e-8 or less | 6.0e-8 | pass |
| S3 STL area / enclosed volume, ASCII | within 1e-12 | 1e-12 | pass |
| S5 welded 20^3 box boundary | 2402 vertices, 4800 triangles, 7200 edges, Euler 2, 0 boundary, 0 non-manifold edges | same | pass |
| STL in VTK `vtkSTLReader` (binary and ASCII; hex, tet, prism meshes) | enclosed volume 2.0 on the test mesh, mixed patch 22 triangles | equal to written | pass |
| T1/T2 Tecplot round trip (gmsh tet, hex, prism samples and a hand-built mixed mesh) | bit exact, volume match 1e-13 | 0 / 1e-13 | pass |
| Tecplot in VTK `vtkTecplotReader` | cell counts 64 / 1139 / 336 and 4 (mixed) as written, volume sums exact; mixed mesh volume 1.515000004917 vs 1.515 (VTK stores float32 coordinates) | equal | pass (tolerance 1e-7 in the script for the float32 reader) |
| P1 Plot3D round trips | bit exact for binary double (both byte orders) and ASCII; single precision 6.0e-8 or less | 0 / 6.0e-8 | pass |
| P3 `block_to_mesh` graded 9x5x4 box | 96 cells, volume and area within 1e-12 | 1e-12 | pass |
| Plot3D sample files in VTK `vtkMultiBlockPLOT3DReader` | blocks (5,4,3), (2,2,2), (7,3,4) in binary and ASCII | equal sizes | pass |
| P5 public sample `bluntfinxyz.bin` (https://raw.githubusercontent.com/pyvista/vtk-data/master/Data/bluntfinxyz.bin, SHA-256 b0748b066152c7001d2979245e729da32b44eb6f171b0c49cf6ed0eb84fe0e6a, 491532 bytes, repository licence Apache-2.0, per-file NASA licence not found) | one block 40x32x32; bounds x [-7.815747261047363, 14.362203598022461], y [0, 8.327558517456055], z [0, 5.7242512702941895], equal to VTK's | equal | pass |
| `block_to_mesh` on the blunt fin | refused: degenerate face (collapsed edge) | n/a | recorded limitation, not a registered criterion |
| X1-X3 XDMF consistency, X4 VTK `vtkXdmfReader` | counts, volume 2.0, positive cell volumes, cell data p = 10..13 read | equal | pass after reversing the wedge triangle winding in the writer (first run gave negative wedge volumes) |
| H1 fuzz (all prefixes + 3000 mutations) STL, Tecplot, Plot3D, GLB | 0 panics | 0 | pass |
| H2 header claiming `u32::MAX` / `i32::MAX` items | `Err` in under 1 s | under 1 s | pass |
| H4 located errors | `line 7`, `byte offset`, JSON pointers found in messages | present | pass |
| G1-G8 GLB rules, one negative test each, fuzz, hand-built GLB | all pass | 0 issues on writer output | pass |
| Khronos gltf-validator 2.0.0-dev.3.10 on `contour_zmax.glb`, `contour_all.glb`, `plain.glb` (20^3 box) | errors 0, warnings 0, infos 0, hints 0 | errors 0 | pass |
| `vtkGLTFReader` on `contour_zmax.glb` | 1600 points, 800 triangles, arrays NORMAL, COLOR_0, _SCALAR | loads | pass; no viewer colour check, "viewer with correct colours" NOT demonstrated |
| R1 box on 20^3 | 1000 cells, volume rel err 8.9e-16 | 1e-12 | pass |
| R2 threshold x >= 0.5 | 4000 cells, mean 0.750000000000062, integral over cube 0.499999999999995, min 0.525, max 0.975 | 1e-12 | pass |
| R3 sphere r = 0.3 on 40^3 | volume 0.112625 vs 0.113097, rel err 4.2e-3 | 3e-2 | pass |
| R4 boundary adjacent xmin layers 1 / 2, all six patches | 400 / 800 / 2168 cells; interface area 0.999999999999995 | exact; 1e-12 | pass |
| R5 algebra, point-neighbour grow / shrink of 10^3 block | 1728 / 1000 | 1728 / 1000 | pass |
| R5 face-neighbour grow of the 10^3 block | 1600 | 1728 | MISS (wrong arithmetic in the ADR, see errata); `r5_face_grow_known_miss` |
| R6 patch changes (settings layer, 20x20 mesh) | exactly 100 cells, others bit identical | register count | pass |
| R7 porous sink on register (20x20 mesh) | 100 non-zero cells = register count, bit-exact zero elsewhere | equal | pass |
| R8 step field tau 0.5 / linear field | 800 cells (volume fraction 0.1) / 0 | 800 / 0 | pass |
| R9, R10 statistics and located errors | empty register `None`; every error names the setting path | as registered | pass |

## Lumped zones and radiation (F-113, F-130, ADR 0012, `crates/rcfd-api/tests/lumped.rs`, `radiation.rs`)

| Item | Reference | Source status |
| --- | --- | --- |
| Porous-jump law `dp = (mu/alpha) t v + C2 t rho v^2/2` | analytic Darcy-Forchheimer sheet | definition used by the code itself; not checked against vendor documentation (clean-room) |
| Fan operating point | intersection of fan curve and system curve by bisection | standard fan-engineering definition; no published benchmark case compared |
| Effectiveness `Q = eps C_min (T_sec - T_in)` | Kays and London, Compact Heat Exchangers | formula from memory of the standard definition; the book was not accessed |
| P1 / Marshak slab `q = sigma (T1^4 - T2^4) / (1/e1 + 1/e2 - 1 + 3 tau/4)` | Modest, Radiative Heat Transfer (P1 approximation, Marshak condition `q.n = -(e/(2(2-e)))(E_b - G)`) | derived from the equations here; the book was not accessed |
| Milne extrapolation length 0.7104 and P1 value 2/3 | secondary source excerpt (ANS letter on extrapolation distances, found through a web search) | NOT verified against a primary source; that radiative equilibrium between plates reduces to the Milne problem is my own derivation. R2 (0.41 % P1 vs Milne asymptote) is therefore only as good as these two statements. Wave 13: the value 0.7104 is confirmed (Hopf constant 0.7104460895, arXiv:1704.05940 eq. 62; ANS letter wx1.ans.org/pubs/journals/download/a_26506 gives 0.7104 Milne vs 0.6667 Marshak/P1); the slab mapping Psi = 4/(3 tau + 6 x 0.7104) was re-derived by the verifier, not found in a source |
| Rosseland diffusion `k_r = 16 sigma n^2 T^3 / (3 beta)` | Modest (diffusion approximation) | standard formula; book not accessed. Wave 13: form confirmed (secondary) against the COMSOL Rosseland model description |
| WSGG coefficients | none | not implemented: no coefficient table could be verified from an accessible published source |

Measured against these in ADR 0012 (addendum). Registered misses: Z3 network pressure 8.5e-4 vs 1e-4
(`z3_network_pressure_known_miss`), Z4 effectiveness duty vs nominal formula 1.45e-6 vs 1e-6
(`z4_effectiveness_nominal_formula_known_miss`).

## 0D reactors and FW-H acoustics (F-112, F-136, ADR 0013, `crates/rcfd-reactor/tests/reactor.rs`, `crates/rcfd-post/tests/fwh.rs`)

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

Rotating dipole is realised as a pair of opposite monopoles on a circular orbit; a rotating point force with an exact
non-compact velocity field is not tested. Registered misses: R3 NASA continuity (above).

## Spalart-Allmaras (F-041, ADR 0014)

| Quantity | Source | Status |
| --- | --- | --- |
| SA-noft2 equations, sigma 2/3, cb1 0.1355, cb2 0.622, kappa 0.41, cw2 0.3, cw3 2, cv1 7.1, S~ limiter (c2 0.7, c3 0.9), SA-neg (cn1 16, fn, far-field 3-5 nu) | NASA TMR https://tmbwg.github.io/turbmodels/spalart.html | verified by web fetch 2026-10-15; the original Spalart and Allmaras 1994 paper and Allmaras et al. 2012 were not re-fetched |
| Cf(x = 0.97) = 2.70565e-3 (CFL3D SA) | TMR `FlatPlate/SA/cf_cfl3d.dat` (pinned commit 20a39f54..., SHA-256 2298419a...5c6c), interpolated between rows 0.96005368 (2.70961295e-3) and 0.97008407 (2.70562153e-3) | value verified; the grid is not stated in the file ("finest 545x385" is an inference from the page, UNVERIFIED) |
| Theory curves at x = 0.97: 2.72379e-3, 2.88302e-3, 2.87459e-3 | TMR `cf_as_function_of_x.dat` read off the pinned file | read from file; their exact definition is not documented there |
| White 0.455/ln^2(0.06 Re_x), power law 0.0576 Re_x^-0.2 | White, Viscous Fluid Flow, flat-plate correlations | from memory, as for the SST entry; wave 13: White's form confirmed (Jaffer arXiv:1810.05743v12 eq. 8), the 0.0576 power law by CFD-Online |
| S5 tolerance 10 % and the y+ band [20, 60] | rcfd choice (ADR 0014) | not a literature value |


## Turbulence finishing (F-042, F-043, F-203, ADR 0015)

Known misses added by ADR 0015 (limits as registered, never widened; ignored `*_known_miss` tests):

| Item | Limit | Measured | Test |
| --- | --- | --- | --- |
| F-042 BFS-C Standard (corrected inlet, medium mesh) | x_r/h in 6.16 to 6.36 | 5.350 (fine 5.452) | `rcfd-turb/tests/step_kepsilon_corrected.rs::step_c_reattachment_standard_known_miss` |
| F-042 BFS-C Realizable | same | 6.413 (fine 6.520, not grid-converged) | `step_c_reattachment_realizable_known_miss` |
| F-043 SST vs 6-112a / 6-121 / 6-134 | 5 % at x = 0.5, 0.97, 1.5 | worst -5.41 / -7.67 / -8.17 % | `flat_plate_bsl_sst.rs::b2_*_known_miss` |
| F-043 Cf(0.97) spread over y+ 2, 1, 0.5 | 3 % | SST 3.45, BSL 3.45, SST low_re 3.03 % | `b4_*_known_miss` |
| F-043 BFS-C SST / BSL wall-resolved | converged, window, spread | diverged (first set-up) or not converged (B5b) | `step_sst.rs::*_known_miss` |
| F-203 SST automatic y+ ~ 1 vs Prandtl-Schlichting | 5 % | -8.44 / -7.76 % | `near_wall_catalogue.rs::c1_sst_smooth_y1_known_miss` |
| F-203 k-epsilon Standard y+ ~ 30 | 5 % | -6.15 / -4.42 % | `c2_kepsilon_standard_y30_known_miss` |
| F-203 Enhanced y+ ~ 1 and ~ 30 | converged, 5 % | not converged | `c2_kepsilon_enhanced_y{1,30}_known_miss` |
| F-203 SST + roughness | 10 % | -22 to -17 %, field stationary (max update 4.4e-9 vs registered 1e-9) | `c3_rough_plate_sst_known_miss` |
| F-203 C5 tree equals library run | 1e-12 | wave 13: max abs difference 0 in all seven option runs (ADR 0016 results) | `rcfd-api/tests/wall_options.rs` |

Reference values and sources:

| Item | Source | Status |
| --- | --- | --- |
| BFS: Re_H about 36000, delta about 1.5 H, x_r/H 6.26 +- 0.10, Re_theta 5000 | NASA TMR 2DBFS page (fetched 2026-10-15) | verified; free-stream turbulence level not stated |
| Measured inlet profile at x/H = -4 (u, u'u', v'v') | `validation-data/data/backward-step-tmr/profiles.exp.dat` (F-125 pack) | file values used; v'v' = w'w' assumed for k, top-wall profile assumed to mirror the floor, epsilon from l = min(kappa d, 0.135): assumptions, unverified |
| TMR CFL3D SST-V Cf and theory curves 6-112a, 6-121, 6-134 | TMR flat-plate data files `cf_plate_sstv.dat`, `cf_as_function_of_x.dat` | values interpolated at x = 0.5, 0.97, 1.5; theory 1 is 0.0592 Re_x^-0.2 |
| SST wall omega 10 * 6 nu/(beta1 d^2), k_wall = 0, SST-2003 gamma 5/9 and 0.44 | TMR SST page | verified |
| BSL sigma_k1 = 0.5 and nu_t = k/omega; gamma from the consistency relation (0.5532, 0.4403) | Menter 1994, from memory; the TMR page lists BSL only as differing in sigma_k1 and nu_t | wave 13: confirmed against Menter AIAA 93-2906 (NTRS https://ntrs.nasa.gov/api/citations/19960044572/downloads/19960044572.pdf: Set 1 sigma_k1 0.5, sigma_w1 0.5, beta1 0.0750; Set 2 1.0, 0.856, 0.0828); gamma 0.5532, 0.4403 recomputed from the relation |
| Prandtl-Schlichting C_F = 0.455/(log10 Re_L)^2.58 | Schlichting and Gersten, Boundary-Layer Theory | wave 13: confirmed (secondary) as a plate-average drag coefficient, Re_L up to 1e9 (Univ. Iowa notes); CFD-Online lists it as local, sources disagree; the book itself not read |
| Fully rough local Cf = (2.87 + 1.58 log10(x/ks))^-2.5 | Schlichting, Boundary-Layer Theory | wave 13: confirmed (secondary), Jaffer arXiv:1810.05743v12 eq. 3 citing Prandtl and Schlichting |
| Cebeci-Bradshaw roughness shift (transitional range exponent, ks+ = 2.25 and 90, C_s) | Cebeci and Bradshaw (1977) | wave 13: constants 2.25, 90, 87.75, 0.4258, 0.811 and the fully rough 1 + C_s ks+ confirmed in OpenFOAM v6 `nutkRoughWallFunction` source and attributed to Cebeci-Bradshaw by the Fluent guide; the book was not read; the rounded constants 0.4258 and 0.811 replaced by their exact forms (equal to 5 digits) and the transitional shift clamped at 0 (deviation, ADR 0015); Nikuradse intercept 8.5 confirmed, the "textbook C_s = 0.5" remark not found |
| Scalable wall function form (y* limited below the viscous/log intersection) | Grotjans and Menter (1998) | wave 13: concept confirmed (CFX theory guide cites the paper, limit y* = 11.06; Fluent 11.225); rcfd's y*_lam = 11.53 follows from its rounded constants; paper formulas not read |
| Two-layer constants A_mu = 70, A_eps = 2 c_l, blend Re_y* = 200 | Chen and Patel (1988), Jongen (1998) | wave 13: constants and tanh blend form confirmed in the Fluent theory guide (secondary, an unofficial copy (not linked)); Chen and Patel citation confirmed (Crossref) but the constants not seen in the paper; blend width 0.1 Re_y* is an rcfd choice within the quoted 5 to 20 percent |
| Kader blended law (Gamma = -0.01 y+^4/(1 + 5 y+)) | Kader (1981) | wave 13: confirmed (secondary) in the Fluent theory guide (a = 0.01, b = 5, same form) and in a heat-transfer paper writing the equivalent positive-Gamma form; the 1981 paper was not read |
| Standard k-epsilon under-predicts BFS reattachment by 10 to 20 percent | general literature statement | not tied to a specific source here; the measured 14.5 percent is consistent |
| Non-equilibrium wall functions (Kim and Choudhury) | not used | not attempted: formulas not verifiable |

## Wave 13 numerics and wall motion (F-211, F-208, F-204, ADR 0016)

Every literature statement below was written from memory while building; none was checked against a source in this wave (no web
access to the papers). They are marked unverified and none of the registered checks depends on them: the checks use exact solutions
(manufactured, linear profiles, the Taylor-Couette solution derived in the ADR) or equivalences between options.

| Item | Source | Status |
| --- | --- | --- |
| Gamma scheme (normalised-variable blend, beta_m range 0.1 to 0.5) | Jasak, Weller and Gosman (1999), Int. J. Numer. Meth. Fluids 31:431 | unverified (memory); bounds measured in W1, not assumed |
| Compressive family psi_k(r) = max(0, min(k r, 1), min(r, k)); k = 1 minmod, k = 2 superbee | Roe (1985), Sweby (1984) | unverified (memory); the extension to k in [0, 1) is project-defined |
| Convection boundedness criterion | Gaskell and Lau (1988) | unverified (memory); not used as a pass criterion |
| Bounded central differencing equals minmod in normalised variables | Jasak (1996) thesis | unverified (memory); equivalence to `Tvd(Minmod)` is tested bitwise (B3), the literature identification is not claimed |
| `Bdf2Bounded` time scheme | project-defined | no source; accuracy measured (T1) and boundedness measured (T2) |
| Face-gradient correction (compact difference replaces the component along the cell-centre line) | Jasak (1996) thesis; Mathur and Murthy (1997) | unverified (memory); effect measured in M1 to M4 |
| Maxwell slip law and the slip length l = lambda (1 + s)/(1 - s) | Maxwell (1879) | unverified (memory); D3 checks the discrete law against its own closed form, not the slip law |
| Thermocapillary layer u = (tau h/(4 mu)) (3 xi^2 - 2 xi) | derived in ADR 0016 (parallel flow, zero net flux) | derived, checked by hand in the ADR; no external source |
| Taylor-Couette u_theta = A r + B/r and torque 4 pi mu B | derived in ADR 0016 (standard solution) | derived; D5b (rigid rotation) separately confirms the torque evaluation has no spurious transpose term |

## Wave 14 mesh quality and editing (F-233, F-231, ADR 0018)

Every literature statement below was written from memory; none was checked against a source. None of the registered checks depends on
them: every expected value is a closed form derived in the ADR from the project definitions.

| Item | Source | Status |
| --- | --- | --- |
| Face warp as the angle between the normals of the two triangles of a split quadrilateral | common finite-element pre-processing practice | unverified (memory); the maximum over both diagonals and the polygon form for 5 or more vertices are project choices |
| Orthogonal quality as the minimum cosine between the face normal and the centre-to-face and centre-to-neighbour vectors | common finite-volume practice | unverified (memory); ortho-skew (intra-cell part only) and size change (neighbour-to-cell volume ratio) are project definitions |
| Default poor-mesh limits (warp 15 deg, face angle 20 and 160 deg, inverse orthogonal quality 0.9, ortho-skew 0.85, face edge-length ratio 100) | rules of thumb | unverified (memory); not guarantees, adjustable through `Thresholds` |
| Closed forms for the right-corner tetrahedron, sheared and warped hexahedra | derived in ADR 0018 A3 | derived; the decimal expansion printed for the tetrahedron in A3 has a typo, the formula `1 - 3/sqrt(11)` is correct |

## Wave 15B partitioning, reordering, inflation and derived fields (F-236, F-108, F-072, ADR 0020)

Every literature statement below was written from memory; none was checked against a source. None of the registered checks depends on
them: every expected value is a closed form or invariant derived in the ADR.

| Item | Source | Status |
| --- | --- | --- |
| Boundary refinement by single-cell moves with positive gain under a balance cap (Kernighan-Lin / Fiduccia-Mattheyses family) | Kernighan and Lin (1970); Fiduccia and Mattheyses (1982) | unverified (memory); only the idea of gain-driven moves is used, the implementation is a plain greedy variant, not a published algorithm |
| Recursive coordinate bisection and inertial bisection | Berger and Bokhari (1987); Taylor and Nour-Omid (1994) | unverified (memory); cuts checked against closed forms in the ADR |
| Reverse Cuthill-McKee ordering with a pseudo-peripheral start | Cuthill and McKee (1969); George and Liu (1981) | unverified (memory); the bandwidth bound `2 n2 n3 - 1` for grids is derived in ADR 0020 A2 from the level-set structure |
| Isentropic total-pressure relation `p0/p = (1 + (gamma-1)/2 M^2)^(gamma/(gamma-1))` | standard gas dynamics | derived series expansion to `M^4` in ADR 0020 C2 (binomial); the closed form itself is textbook, unverified (memory) |
| Edge-isoperimetric minimum bisection width of a grid (cross-section of the longest axis) | Bollobas and Leader (1991) | unverified (memory); NOT used: only the upper bound given by the slab is registered |

## Wave 16W wiring (F-236, F-072, F-108, ADR 0021)

No new literature value is introduced. The expected values of the registered checks are closed forms or equalities with the library
call the wiring surfaces: the linear velocity field `u = x + 2y`, `v = 3x - y` (vorticity 1, Q = -7), the Couette wall shear `mu U/h`
with `U = 3`, `h = 0.5`, and the y+ definition `y+ = y u_tau / nu` with `u_tau = sqrt(tau/rho)`. The references already marked
unverified under the wave 15B heading (partitioning, reverse Cuthill-McKee, isentropic total pressure) are unchanged and still
unverified (memory).

## Wave 16B boundary-condition catalogue (F-200, ADR 0022)

Every closed form below is a textbook relation written from memory and not checked against a source in this session; each was
re-derived in the ADR or checked numerically as stated. No vendor data is used.

| Item | Source | Status |
| --- | --- | --- |
| Plane Poiseuille flow: `-dp/dx = 12 mu U / H^2`, profile `6 U (y/H)(1 - y/H)` | standard fluid mechanics | derived from the momentum balance (parabolic profile, mean `U`); unverified (memory) as a citation. The measured 0.498 % deficit is the cell-centred wall-gradient error `1.5 (h/gap)^2` predicted in the ADR |
| Annular Poiseuille flow: `Q = pi G / (8 mu) [R2^4 - R1^4 - (R2^2 - R1^2)^2 / ln(R2/R1)]` | Lamb, Hydrodynamics; Bird, Stewart and Lightfoot | unverified (memory); the closed form is evaluated with the equal-area radii of the 48-gon (0.499286, 0.998572), and the 0.398 % agreement with the solver is a consistency check, not a verification of the formula |
| Radial equilibrium `dp/dr = rho w^2 / r` of swirling flow, rigid rotation `p = p_a + rho Omega^2 (r^2 - <r^2>)/2` | standard (the F-200 roadmap text states the relation) | derived from the radial momentum balance; unverified (memory) as a citation |
| Total-pressure inlet `p = p0 - |u|^2/2`, Bernoulli for the frictionless duct | standard | textbook; unverified (memory) |
| Vent loss law `dp = K rho u |u| / 2` | common engineering practice | unverified (memory); the quadratic law with the dynamic head `rho u^2/2` as the unit is a project definition |
| Inlet turbulence `k = 1.5 (I U)^2`, `eps = Cmu^(3/4) k^(3/2) / l`, `eps = Cmu k^2 / (nu ratio)`, `omega = eps / (beta* k)` | standard | pre-existing helpers (F-044); the registered values 3.75e-3, 5.390521e-4, 1.597191, 1.265625e-6 are arithmetic of these formulas; unverified (memory) |
| Secant and dynamic-head sensitivity updates of the target-mass-flow pressure | generic root finding | project design; the secant option is a documented known miss (ADR 0022 addendum) |

## Wave 17C GUI, assistant and renderer (F-143, F-144, F-149, F-150, F-073, ADR 0025)

No physical reference value is introduced. Conventions used by the checks, all from memory and unverified as citations: Rec. 709 luma
weights (0.2126, 0.7152, 0.0722) for the colour-map luminance test; the "1-2-5" family (with 2.5) of tick steps; min/max bucket
decimation for plot data. The OpenAI-compatible wire format (`POST /v1/chat/completions`, `GET /v1/models`, SSE `data:` events with
`[DONE]`, `Authorization: Bearer`) and the default local ports (Ollama 11434, LM Studio 1234, llama.cpp 8080, vLLM 8000) are
recalled from memory and unverified against current documentation; they were checked only against the loopback mock server written
for the tests, which shares the author's understanding of the format, so agreement is not independent evidence about any real server.
The lid-driven cavity logic check (top-row mean ux, return flow) is a sanity bound, not a benchmark comparison.

Wave 17A (ADR 0023): convergence floor diagnosis and pseudo-transient control. Known misses added (limits as registered, never widened; ignored tests):

| Item | Limit | Measured | Test |
| --- | --- | --- | --- |
| F-106 `ser` controller, cavity Re 1000 / step Re_h 400 | residual 1e-6 | not converged (cavity 0.18 after 6000 its, scale at cap) | `rcfd-fv/tests/pseudo_control.rs::*_known_miss` |
| F-106 `ser2` controller (symmetric), same cases | residual 1e-6 | cavity 1.6e-2, step 7.7e-5 | `pseudo_control.rs::*_ser2_known_miss` |
| F-106 controller on C2/D1/D2 (local cfl 1, physical 0.2) | converge, registered accuracy | residual 0.21 or NaN | `rcfd-api/tests/convergence_wave17a.rs::f?_*_known_miss`, `f?b_*_known_miss` |
| F-209 error-controlled stepping, linear decay (relative and absolute scale) | steps <= 0.8 x uniform | 81 vs 76; 46 vs 36 | `rcfd-fv/src/ddt.rs::e1*_known_miss` |

Findings: the F-200 C2/D1/D2/E2 residual targets are met with `solver.residual_norm = scaled` (new passing tests F1c, F2c, F3c, F4), the reference
normalisation stalled by construction; the F-061 nozzle 1e-8 residual floor (2.619e-5) stays undiagnosed; wave-13 D5b n_r = 128 is slow convergence. No external reference data
were used in wave 17A; the Taylor-Green and flat-plate acceptance items of F-209 were not run.

## Wave 17B conjugate heat transfer (F-051, ADR 0024)

Every reference value used by the registered checks is derived in the ADR or is closed-form arithmetic, and every source
attribution below is from memory and unverified as a citation:

| Quantity | Source | Status |
| --- | --- | --- |
| 1-D steady conduction through layers in series with a convective end, `q = dT / (sum L_i/k_i + 1/h)` | standard heat transfer | derived (series resistances); unverified (memory) as a citation |
| Fin of uniform section, tip convection: `Q = sqrt(h P k A) theta_b (sinh mL + (h/mk) cosh mL) / (cosh mL + (h/mk) sinh mL)` | standard heat transfer | recalled from memory, unverified; the 1% bound is an engineering limit and the measured 3.3e-4 compares the 2-D solve with this 1-D formula |
| Lumped slab `T = T_inf + (T0 - T_inf) exp(-t/tau)` (small Biot number) | standard | derived; unverified (memory); the 1% bound is an engineering limit (Biot number of the set-up is the ADR's) |
| Semi-infinite contact of two solids, `T_i = (e1 T1 + e2 T2)/(e1 + e2)`, `e = sqrt(k rho c)`, `T = T_i -/+ (..) erfc(x / (2 sqrt(alpha t)))` | Carslaw and Jaeger; Incropera | unverified (memory); the `erfc` series and continued fraction are implemented in the test file and were not compared with a table |
| Stefan-Boltzmann constant | CODATA exact SI values `sigma = 2 pi^5 k_B^4 / (15 h^3 c^2)` | computed in code from exact SI constants and unit-tested against 5.670374419e-8; the value is recalled from memory, unverified |
| Fully developed laminar flow between parallel plates with uniform wall heat flux, `Nu = 140/17 = 8.2353` (`D_h = 4H`, half channel `H`) | derived in ADR 0024 section D from the energy equation with Poiseuille velocity | derived; the textbook value 8.235 (uniform flux, plates) is recalled from memory and is UNVERIFIED; the derivation, not the recollection, is the reference |
| Contact resistance `R''` in series, `1/k_eff = 1/k + R''/Delta` | standard | derived; unverified (memory) |

Known deviations of the checks from the ADR text (details in the ADR addendum): the duct tests use central convection, since upwind is not
exact for the linear axial profile at the Dirichlet inlet face; the steady stop criterion of the library tests is 1e-11, not 1e-13.

## Wave 18C particle tracking (F-056, ADR 0028)

Every reference value of the registered checks is a closed form derived in the ADR (terminal velocity, exponential relaxation,
Couette and solid-body-rotation particle motion, ballistic bounce and landing). Published correlations used by the code:

| Quantity | Source | Status |
| --- | --- | --- |
| Stokes drag `C_D = 24/Re`, `tau_p = rho_p d^2/(18 mu)`, terminal speed `d^2 (rho_p - rho_f) g/(18 mu)` | standard | derived |
| Schiller-Naumann drag factor `1 + 0.15 Re^0.687` (and its Stokes limit) | arXiv:1506.00478 and a published molecular-dynamics code's documentation, found by web search during wave 18C; original Schiller and Naumann (1933/1935) not accessed | coefficients verified (secondary); validity limit quoted as Re 800 by some sources and 1000 by others, the switch to `C_D = 0.44` at 1000 is UNVERIFIED |
| Rosin-Rammler oversize mass fraction `Y = exp(-(d/d_bar)^n)`, `d_bar` at `Y = 1/e` | two independent published references (a CFD manual and an engineering reference page) found by web search; Rosin and Rammler (1933) not accessed | verified (secondary) |
| Inversion sampler `d = d_bar (-ln u)^(1/n)` | derived from `Y(d) = u` | derived |
| SplitMix64 constants | Steele, Lea and Flood 2014, from memory | unverified; irrelevant to correctness (any 64-bit generator with the same interface works), only reproducibility is claimed |

## Algebraic subgrid and mixing-length models (F-045, F-206, ADR 0026)

All entries below are written from memory of the published literature; none could be checked against the source text in this
environment (marked UNVERIFIED). The closed-form checks of ADR 0026 test the implementation against the formulas as written
here, not against the papers.

| Quantity | Source | Status |
| --- | --- | --- |
| Smagorinsky model `nu_t = (Cs Delta)^2 sqrt(2 S_ij S_ij)`, Cs = 0.17 (range 0 to 0.5), `Delta = V^(1/3)` | Smagorinsky (1963); Lilly (1967) for the constant | UNVERIFIED, from memory; Cs is flow dependent (0.1 to 0.2 in the literature) |
| WALE operator, Cw = 0.325, exponents 3/2, 5/2, 5/4 | Nicoud and Ducros (1999) | UNVERIFIED, from memory; the zero for pure shear and the `y^3` wall scaling are derived in the tests, but the nonzero value for rigid rotation (2.86e-3 at w = 3) follows from the formula as written |
| van Driest damping `1 - exp(-y+/A+)`, A+ = 25 (Smagorinsky), 26 (mixing length) | van Driest (1956) | UNVERIFIED, from memory |
| Mixing length `l = kappa d` capped at 0.09 delta | Prandtl; common textbook form | UNVERIFIED, from memory |
| Nikuradse pipe profile `l/R = 0.14 - 0.08 (1 - d/R)^2 - 0.06 (1 - d/R)^4` | Nikuradse (1932) as quoted in textbooks | UNVERIFIED, from memory; no Nikuradse data are in the tree and none was compared |
| Channel Re_tau = 395 mean velocity (U+ at y+ = 30, 50, 100, 150, 300 = 13.493, 14.856, 16.534, 17.646, 19.745; centreline 20.133; bulk 17.545) | Moser, Kim and Mansour (1999) DNS | UNVERIFIED, from memory; used as the ML4/ML5 targets with a 10 percent limit |
| Taylor-Green vortex decay `exp(-2 nu t)` and the velocity field `(sin x cos y, -cos x sin y)` | analytic solution of the Navier-Stokes equations | derived |
| Decaying isotropic turbulence slope | none | not verified, not run, no claim made |

## Wave 18B combustion constants (ADR 0027)

None of the constants below could be checked against a primary source in this environment. The checks of ADR 0027 test the
implementation against the formulas as written here, not against published flame data.

| Quantity | Source | Status |
| --- | --- | --- |
| Eddy-dissipation rate `A rho (eps/k) min(Y_R/(nu' W_R), B Y_P/sum nu'' W)`, A = 4.0, B = 0.5 | Magnussen and Hjertager (1977) | UNVERIFIED: values from secondary lecture notes and forum posts; the primary paper was not accessed |
| EDC fine-structure fraction `xi* = C_xi (nu eps/k^2)^(1/4)`, residence time `tau* = C_tau (nu/eps)^(1/2)`, C_xi = 2.1377, C_tau = 0.4082 | Magnussen (1981, 1989); Gran and Magnussen (1996) | secondary: values stated in arXiv:2101.08764, which cites Gran and Magnussen; primary papers not accessed. The relations `C_xi = (3 C_D2 / (4 C_D1^2))^(1/4)`, `C_tau = sqrt(C_D2/3)` with C_D1 = 0.134, C_D2 = 0.5 reproduce both values; C_D1 and C_D2 are UNVERIFIED (from memory) |
| EDC reacting-fraction exchange rate `m = xi*^2 / (tau* (1 - xi*^3))` | same | read in arXiv:2101.08764 (eq. 7, reacting fraction taken as 1); secondary |
| Extended Zeldovich closed form `d[NO]/dt = 2(k1f k2f [O][N2][O2] - k1r k2r [NO]^2 [O]) / (k1r [NO] + k2f [O2])` | Zeldovich (1946); Lavoie, Heywood and Keck (1970) | derived in ADR 0027 K10 under the steady-state-N approximation; no rate constants bundled |
| Zeldovich rate constants (O + N2 = NO + N etc.) | Hanson and Salimian evaluation | UNVERIFIED: the primary evaluation was not accessed, the only confirmation found was in vendor documentation, which is excluded here; the tests use invented constants |
| Nine-coefficient polynomial `cp/R = a1 T^-2 + a2 T^-1 + a3 + a4 T + a5 T^2 + a6 T^3 + a7 T^4`, with `h/RT` and `s/R` obtained by integration | NASA Glenn thermodynamic polynomial form | functional form as commonly stated, UNVERIFIED against the report; the integrated forms are verified by derivative consistency (K12a). The text file layout of the public database is NOT implemented |

## Wave 19B meshing references (ADR 0030)

No published source for the following was accessed in this environment; the algorithms were written from memory and are tested
against closed-form or exact-arithmetic checks only.

| Quantity | Source | Status |
| --- | --- | --- |
| Incremental Delaunay insertion (cavity of circumspheres), boundary recovery by Steiner points | Bowyer (1981), Watson (1981) | UNVERIFIED: from memory, primary papers not accessed |
| Exact orient3d / insphere with floating-point filter and expansion arithmetic | Shewchuk (1997) | UNVERIFIED: technique from memory; correctness checked against exact i128 arithmetic (T9), not against the paper's code or constants |
| Radius-edge bound 2.0 for Delaunay refinement | Ruppert / Shewchuk refinement literature | UNVERIFIED: value taken as the commonly stated bound, from memory; the bound is not proven for this implementation (boundary splitting off) |
| Incremental isotropic surface remeshing (split 4/3 h, collapse 4/5 h, valence flip, tangential smoothing) | Botsch and Kobbelt (2004) | UNVERIFIED: ratios from memory |
| Curvature from vertex normals, size = theta / kappa | project definition | no source; checked on sphere and cylinder closed forms |


## Wave 20C study references (ADR 0034)

| Quantity | Source | Status |
| --- | --- | --- |
| Fully developed laminar plane-channel pressure drop `dp = 12 mu U L / h^2` (mean velocity U, gap h) | textbook result, derived from the parabolic profile `u = 6 U (y/h)(1 - y/h)` | derived here from the momentum balance; no external source was consulted (UNVERIFIED against a printed source). Check S7 misses its 1 percent limit at 10 cells across the gap (1.9 percent) and meets it at 20 (0.29 percent) |
| Steady 1-D conduction `q = k A dT / L` | first principles | exact for the linear profile; the depth of the `box_2d` mesh (`lx / nx`) was measured from the mesh volume, not looked up |

## Wave 20A transposed stress, SA-based DES and Reynolds stress model references (F-045, F-207, F-046, ADR 0032)

Web pages were consulted for the entries marked "secondary" or "TMR"; none of the original papers was read. Vendor documentation
was not consulted. The checks of ADR 0032 test the implementation against the formulas as written here.

| Quantity | Source | Status |
| --- | --- | --- |
| Full viscous stress `div(nu_eff (grad u + grad u^T))` of the incompressible momentum equation; the transposed part `div(nu_eff grad u^T)` equals `grad u . grad nu_eff` for divergence-free velocity (zero for constant viscosity) | continuum mechanics | derived; the operator is verified exactly on linear fields (A1) |
| SSG pressure-strain constants C1 1.7, C1* 0.9, C2 1.05, C3 0.8, C3* 0.65, C4 0.625, C5 0.2, and the GGDH diffusion constant 0.22 | Speziale, Sarkar and Gatski (1991); values read on the NASA Turbulence Modeling Resource page https://tmbwg.github.io/turbmodels/rsm-ssglrr.html (outer-region set) | verified against the TMR page; the 1991 paper itself was not read. The sign convention of the C5 (rotation) term follows the TMR definition of the rotation tensor `W_ij = (dU_i/dx_j - dU_j/dx_i)/2` and was not tested against a physical case |
| LRR-IP constant C1 = 1.8 | Launder, Reece and Rodi (1975) via the same TMR page | consistent with the TMR page; the 1975 paper was not read |
| LRR-IP constant C2 = 0.6 | Launder, Reece and Rodi (1975) | UNVERIFIED, from memory |
| Epsilon-equation constants Ce1 = 1.44, Ce2 = 1.83 (SSG) / 1.92 (LRR-IP), dissipation diffusion constant C_eps = 0.18, eddy viscosity `C_mu k^2/eps` with C_mu = 0.09 | common practice for stress-transport models | UNVERIFIED: values from memory (Ce2 1.83 is the value associated with SSG in the literature, 1.92 with LRR-IP) |
| Homogeneous-shear growth and isotropic decay exponent `1/(Ce2 - 1)` | model ODE | derived from the model; no comparison with experiment (the decay exponent of grid turbulence, the equilibrium shear parameter `S k/eps`) was made: no claim |
| DES length scale `d~ = min(d, C_DES Delta)`, `Delta = max(dx, dy, dz)` | Spalart et al. (1997) | verified secondary: statement on https://wmles.umd.edu/hybrid-les-rans-models/des/; the 1997 paper was not read |
| C_DES = 0.65 | Shur et al. (1999) / Spalart (2001) calibration | secondary only: stated in an arXiv summary (2512.18609) and an encyclopedia entry; the calibration papers were not read. Treated as an adjustable default (range 0.1 to 2) |
| DDES `f_d = 1 - tanh[(8 r_d)^3]`, `r_d = (nu + nu_t)/(max(sqrt(U_ij U_ij), 1e-10) kappa^2 d^2)`, `d~ = d - f_d max(0, d - C_DES Delta)` | Spalart et al. (2006) | verified secondary: wmles.umd.edu and arXiv:1606.05114; the 2006 paper was not read |
| SA equation with `d~` in place of `d` in `S~`, `r` and the destruction term | Spalart et al. (1997) | consistent with the secondary pages above; the equations are those of the SA-noft2 model of ADR 0014 |

## Wave 20B dynamic mesh and six-degree-of-freedom references (F-063, F-229, ADR 0033)

No external source was consulted in this wave; every entry below is from memory and UNVERIFIED unless it says derived.

| Quantity | Source | Status |
| --- | --- | --- |
| Geometric conservation law: cell volume change equals the sum of the face swept volumes | Thomas and Lombard (1979); Demirdzic and Peric (1988) | UNVERIFIED citation; the identity is checked to round-off by G1 to G3 |
| Time accuracy and the GCL of implicit schemes on moving meshes | Farhat, Geuzaine and Grandmont (2001) | UNVERIFIED citation, not used quantitatively |
| Variable-diffusivity Laplacian mesh motion (inverse distance) | Jasak and Tukovic (2006/2007) | UNVERIFIED citation; the diffusivity `1/l^k` is implemented as described in the ADR |
| Stokes second problem `u = U0 exp(-eta) cos(omega t - eta)`, `eta = y sqrt(omega/(2 nu))` | Schlichting, Boundary-Layer Theory | UNVERIFIED citation; the profile was checked by substitution into the diffusion equation |
| Newmark average acceleration (beta 1/4, gamma 1/2) | Newmark (1959) | UNVERIFIED citation; second-order convergence measured (ratio 3.99) |
| Euler's equations, axisymmetric torque-free precession `e^{+i Omega t}` with `Omega = (I3 - I1) w3 / I1` | rigid-body dynamics | derived here; the registered sign (`e^{-i Omega t}`) of the ADR was wrong |
| Piston pushing a fluid column, `v^2/2 = -(F/(rho A)) ln(1 - rho A x/m0)` | unsteady Bernoulli for plug flow | derived here (ADR 0033), no external source |
| Added-mass instability of explicit partitioned coupling | Causin, Gerbeau and Nobile (2005) | UNVERIFIED citation; not reproduced, the checks use a fluid-to-body mass ratio of 0.2 |

## Wave 21B tabulated properties, actuator disk and regions references (F-220, F-218, F-216, ADR 0036)

Web pages were consulted for the entries marked "verified"; no vendor documentation was consulted.

| Quantity | Source | Status |
| --- | --- | --- |
| Tait equation of the compressible liquid, `rho(p) = rho_ref / (1 - C ln((B + p)/(B + p_ref)))` (equivalent to the inverse used in `rcfd_core::tabular::Tait`) | NIST Thermodynamics Data Engine, Tait equation page (https://trc.nist.gov/TDE/TDE_Help_4_0/Eqns-Pure-DensityL/TaitEquation.htm) and the Wikipedia article "Tait equation" | verified secondary: the form of the relation; the water coefficients used in the tests are test inputs, not data claims |
| Vogel-Fulcher-Tammann viscosity `mu = a exp(b/(T - c))` | Wikipedia "Vogel-Fulcher-Tammann equation" (https://en.wikipedia.org/wiki/Vogel%E2%80%93Fulcher%E2%80%93Tammann_equation) | verified secondary: the equation; the name "Andrade form" for `c = 0` is UNVERIFIED; coefficients in tests are inputs |
| Steam and water values for the P-T table (saturation line, phase selection) | the IAPWS-IF97 module of this repository (`rcfd_core::iapws`), verified in wave 7 | verified as before; no new table data was introduced |
| Betz limit 16/27 = 0.593 at induction factor 1/3 | https://www3.nd.edu/~tcorke/w.WindTurbineCourse/Aerodynamics_Presentation.pdf (lecture notes) | verified secondary |
| Axial momentum theory: `T = 2 rho A u_d (u_d - u0)`, hover `v_h = sqrt(T/(2 rho A))`, wake velocity `2 v_h`, propulsive efficiency `2 u0/(u0 + u_w)`, figure of merit `T^{3/2}/sqrt(2 rho A)/P` | actuator-disk / momentum theory | DERIVED here from mass, momentum and energy conservation across the disk; not checked against a rotorcraft text; UNVERIFIED citation |
| Pressure jump across a disk in a duct `Delta p = T/A` | one-dimensional momentum balance | derived here |
| Kirchhoff transform for temperature-dependent conductivity, `K(T) = int k dT` linear in x | heat-conduction theory | derived here (ADR 0036 P2); no external source |
| Per-region laminar assignment as a face-viscosity mask | design decision of ADR 0036 | no external source; the equality with a laminar run (R1) is an implementation check |


## Wave 21C run-loop wiring and fractional-step references (F-056, F-054, F-210, ADR 0037)

Web searches were run for the entries marked "verified" (publisher or society records, abstracts). No paper was read in full,
so the numerical content of every entry below is either derived in the ADR or checked by the registered tests, not taken from
the papers.

| Quantity | Source | Status |
| --- | --- | --- |
| Projection method, one Poisson solve per step, non-incremental pressure | A. J. Chorin (1968), "Numerical solution of the Navier-Stokes equations", Math. Comp. 22, 745-762 | verified citation (AMS record); paper not read |
| Fractional step with boundary conditions for the intermediate velocity that remove the splitting error at the wall (`u* = u_b + dt grad(q)_tangential` for the lagged pressure term) | J. Kim and P. Moin (1985), "Application of a fractional-step method to incompressible Navier-Stokes equations", J. Comput. Phys. 59, 308-323 | abstract and the NASA TM-85898 precursor found; volume and page numbers not confirmed against a publisher record (UNVERIFIED); the lagged-increment form implemented here is the author's reading, not copied |
| Incremental pressure-correction schemes, order of the splitting error of the non-incremental scheme | J.-L. Guermond, P. Minev and J. Shen (2006), "An overview of projection methods for incompressible flows", Comput. Methods Appl. Mech. Engrg. 195, 6011-6045; J.-L. Guermond and L. Quartapelle (1997) | verified citation and abstract; theorems not re-derived (the measured F8 result is consistent with a first-order non-incremental steady state, see the ADR addendum) |
| Collocated-grid fractional step (pressure-smoothing face flux) | Zang, Street and Koseff (1994), J. Comput. Phys. 114, 18-33 | UNVERIFIED, from memory, not used quantitatively |
| Rotational pressure-correction form (named gap, wave 25A) | Timmermans, Minev and van de Vosse (1996), Int. J. Numer. Methods Fluids 22, 673-688 | UNVERIFIED, from memory, not read; NOT implemented and not used |
| Wave 25A (ADR 0043) BDF2 extrapolation of the periodic partner value; line search on the convex enthalpy potential; exact 1-D discrete Brinkman solution; compensated column sums | derived in ADR 0043 from first principles | derived; no external data; the Voller-Swaminathan linearised-source update (1991) was considered from memory (UNVERIFIED) and not implemented |
| Taylor-Green vortex decay `exp(-2 nu t)` | wave 7 entry above | verified earlier |
| Lid-driven cavity centreline data, Re = 100 | Ghia, Ghia and Shin (1982); two secondary copies | verified earlier (the CSV files in `cases/cavity/reference`) |
| Particle tracking and chemistry closed forms used by the wiring checks | ADR 0027, ADR 0028 entries above | unchanged; the wiring checks compare the run path with the library, so they introduce no new reference values |


## Wave 21A thermal extras references (F-215, F-213, F-212, ADR 0035)

Closed forms below are derived in ADR 0035 from conservation and Fourier/Ohm laws, and each is checked by a registered test; none
is a data claim. Web search was attempted for the Stefan solution and the enthalpy-porosity paper; primary texts were not
retrievable.

| Quantity | Source | Status |
| --- | --- | --- |
| Series thermal resistance, composite wall with contact resistance, `q = dT / (sum t_i/k_i + R_c)` | heat-conduction theory | derived here (ADR 0035 D1); no external source needed |
| Ohm bar, Joule parabola `T = T0 + (J^2 rho/ (2 k)) x (L - x)`, `sigma(T) = sigma_0/(1 + a (T - T_ref))` bar solution and lumped Joule ODE | conduction theory | derived here (ADR 0035 D2); checked by H1 to H6 |
| Two-phase Neumann/Stefan solution `St_l e^{-l^2}/(sqrt(pi) erf l) - St_s e^{-l^2}/(sqrt(pi) erfc l) = l`, front `s = 2 l sqrt(alpha t)` (one-phase limit `St_s = 0`) | textbook form (H. S. Carslaw and J. C. Jaeger 1959, Conduction of Heat in Solids; V. Alexiades and A. D. Solomon 1993; J. Crank 1984) | UNVERIFIED against a primary text: a search tool returned the same form but not from the textbook; the root is checked in the tests through the residual (P7) and the numerical front converges to it |
| Enthalpy-porosity model, Carman-Kozeny damping `A(f) = C (1 - f)^2 / (f^3 + b)` | V. R. Voller and C. Prakash (1987), Int. J. Heat Mass Transfer 30(8), 1709-1719 | verified citation (bibliographic record by search); paper not read. `C` (mushy constant) and `b` are case dependent: no universal value is claimed, the tests choose `C` from the target permeability `kappa` |
| Brinkman channel `u = (G/kappa)(1 - cosh(m(y - H))/cosh(mH))`, `m = sqrt(kappa/nu)` | momentum balance with a linear sink | derived here (ADR 0035, K2) |
| Gallium-melting cavity experiment (published front shape) | not obtained | NOT used: not delivered, named gap of F-212 |

## Wave 23C population balance references (F-228, ADR 0041)

No paper was read and no web check was made in this wave: every literature entry below is a citation written from memory and is
marked UNVERIFIED. None of them supplies a number used by a test. Every acceptance value is a closed form derived in ADR 0041 from
the moment equations (and checked by the registered tests), or an identity of the code.

| Quantity | Source | Status |
| --- | --- | --- |
| Constant-kernel exponential solution `n(v,t) = N0/(1+tau)^2 exp(-v/(1+tau))`, `tau = K N0 t/2`; moments `m_k = k! N0 (1+tau)^(k-1)` | Smoluchowski (1917); P. J. Scott (1968), J. Atmos. Sci. 25, 54-65 | citation UNVERIFIED (memory); the solution is DERIVED in ADR 0041 (substitution into the moment equation, every `k`) |
| Discrete (Flory) constant-kernel distribution `N_k = N0 tau^(k-1)/(1+tau)^(k+1)` | discrete Smoluchowski system on the integer grid | DERIVED; reproduced by the integrated bin solution (test B1, 1.2e-11) |
| Sum-kernel `m_0 = N0 e^(-B m_1 t)`, `m_2 = m_2(0) e^(2 B m_1 t)`; product-kernel `m_0 = m_0(0) - C m_1^2 t/2`, `m_2 = m_2(0)/(1 - C m_2(0) t)`, `m_3 = m_3(0)(1 - C m_2(0) t)^-3`, gel time `1/(C m_2(0))` | moment equations | DERIVED (ADR 0041); not compared to a paper |
| Breakup moment laws (constant rate: uniform `exp(Gamma t (1-k)/(1+k))`, binary `exp(Gamma t (2^(1-k) - 1))`; linear rate `m_0 = m_0(0) + m_1 t`) | moment equations | DERIVED (ADR 0041) |
| Fixed-pivot discretisation | S. Kumar and D. Ramkrishna (1996), Chem. Eng. Sci. 51, 1311-1332 | citation UNVERIFIED (memory); scheme restated and its conservation properties derived in ADR 0041 |
| QMOM | R. McGraw (1997), Aerosol Sci. Technol. 27, 255-265 | citation UNVERIFIED (memory) |
| Product-difference algorithm; Wheeler's algorithm | R. G. Gordon (1968), J. Math. Phys. 9, 655-663; J. C. Wheeler (1974), Rocky Mountain J. Math. 4, 287-296 | citations UNVERIFIED (memory); the Wheeler recurrence was reconstructed from the modified Chebyshev algorithm and is validated by exact recovery of finite distributions (Q1) and Gauss-Laguerre abscissae `3 +- sqrt(3)` (Q2) |
| Brownian kernel `(2kT/3mu)(u^(1/3)+v^(1/3))(u^(-1/3)+v^(-1/3))` and laminar shear kernel `(4/3) gdot (r_u+r_v)^3` | Smoluchowski (1917) | citation UNVERIFIED (memory); only equal-size values and symmetry are tested |
| Turbulent shear kernel `sqrt(8 pi/15) sqrt(eps/nu) (r_u+r_v)^3` | Saffman and Turner (1956), J. Fluid Mech. 1, 16-30 | citation and constant UNVERIFIED (memory); only the identity with the laminar-shear kernel is tested, NOT validated against a turbulent case |
| Symmetric-beta daughter distribution (integer order) | Hill and Ng (1995), AIChE J. 41, 1204-1216 | citation UNVERIFIED (memory); the form is defined in ADR 0041 and its moments are verified by quadrature (B10) |

## Wave 23B interfacial mass transfer and wave references (F-226, F-227, ADR 0040)

No web page and no paper was consulted in this wave. Every formula below is reproduced from knowledge and is checked in the tests only against closed forms, invariants and internal consistency, NOT against the original publication. Entries are therefore UNVERIFIED as citations unless marked "derived".

| Quantity | Source | Status |
| --- | --- | --- |
| Lee temperature-driven evaporation-condensation `mdot = r alpha rho (T - Tsat)/Tsat` | attributed to W. H. Lee (1980); title not recalled with confidence | UNVERIFIED citation; the relaxation frequencies are inputs, no default is claimed; hand values and the 0-D analytic relaxation checked (M1, M8) |
| Hertz-Knudsen flux and Schrage correction `2 sigma/(2 - sigma)` | kinetic theory of evaporation (Hertz, Knudsen, Schrage) | UNVERIFIED citation; the one-way flux `p sqrt(M/(2 pi R T))` is DERIVED here from `n m vbar/4`, `vbar = sqrt(8RT/(pi M))`, and matched to 1e-13 (M3); the Schrage factor is not independently checked |
| Pressure-driven mass transfer `3 alpha rho_v/R_B sqrt((2/3)abs(dp)/rho_l)` with `F_vap = 50`, `F_cond = 0.01`, `alpha_nuc = 5e-4`, `R_B = 1e-6 m` | attributed to Zwart, Gerber, Belamri (2004); the structure is DERIVED here from the Rayleigh-Plesset growth rate (ADR 0040 D1) | structure derived and checked (M4); the numerical factor values are UNVERIFIED and only the `Default` |
| Raoult's law, bubble and dew pressures; Henry's law with van 't Hoff dependence `H(T) = H0 exp(-C(1/T - 1/T0))` | standard thermodynamics | derived/standard; checked by sum-to-one and round-trip identities (M5, M6); no constants shipped |
| Nusselt laminar film: `delta = [4 mu k dT x/(g rho (rho - rho_v) h_fg)]^(1/4)`, mean `h = 0.943[...]^(1/4)`, `Gamma = rho (rho - rho_v) g delta^3/(3 mu)` | Nusselt (1916) film condensation theory | derived here from the film momentum and energy balance; the coefficient `(4/3)(1/4)^(1/4) = 0.9428` is computed; the Rohsenow correction `h_fg + 0.68 c_p dT` is UNVERIFIED; the water property values in the test are round test inputs, not data claims |
| Dispersion `omega^2 = g k tanh(kh)`, phase and group velocity, Airy kinematics, energy density `rho g a^2/2`, mean flux `E/c` | linear wave theory | derived/standard; checked against deep and shallow limits, kinetic-energy equipartition, continuity, Laplace (W1, W2) |
| Second-order Stokes: `eta2 = (k a^2/4) cosh kh (2 + cosh 2kh)/sinh^3 kh`, `u2`, `w2`, `phi2 = (3/8) a^2 omega cosh 2k(z+h)/sinh^4 kh sin 2 theta` | textbook Stokes expansion (Dean and Dalrymple type) | UNVERIFIED citation; the formulas are VALIDATED here by satisfying the kinematic and dynamic free-surface conditions through second order (the residual is third order, W3b ratios 8.00) and by the deep and shallow limits |
| Pierson-Moskowitz in `Hs`-`wp` form `S = (5/16) Hs^2 wp^4 w^-5 exp(-1.25 (wp/w)^4)`; moments `m_n = (5/64) Hs^2 wp^n 1.25^((n-4)/4) Gamma(1 - n/4)` | form attributed to Bretschneider / Pierson-Moskowitz | `m0 = Hs^2/16` and the moments are DERIVED here (substitution `s = 1.25 (wp/w)^4`) and checked numerically (W5); the attribution is UNVERIFIED |
| JONSWAP shape `gamma^r`, `r = exp(-(w - wp)^2/(2 s^2 wp^2))`, `s = 0.07/0.09`, `gamma = 3.3` | Hasselmann et al. (1973) | UNVERIFIED citation; the constant `A` is replaced by a numerical normalisation to `Hs` |
| Relaxation-zone exponential blending `1 - (exp(chi^3.5) - 1)/(e - 1)` | attributed to Jacobsen, Fuhrman, Fredsoe (2012) | UNVERIFIED citation; endpoint exactness and monotonicity checked (W6); the reflection result is a toy-model measurement (W7), not a validation |
| Linear shallow-water leapfrog tank | first principles | derived; the incident amplitude and the rigid-wall reflection `R = 1.00000` are internal consistency checks (W7) |

## Wave 23A MHD Lorentz force and heat-exchanger references (F-214, F-217, ADR 0039)

No web page and no paper was consulted in this wave. Every formula is derived from first principles in ADR 0039 or reproduced from
knowledge, and is checked only against closed forms, independent numerical solutions and invariants, NOT against a published table or
experiment. Entries are UNVERIFIED as citations unless marked "derived".

| Reference value | Source | Status |
| --- | --- | --- |
| Hartmann flow, short-circuited walls `u = G/(sigma B0^2) (1 - cosh(Ha y/a)/cosh Ha)`, mean `G/(sigma B0^2)(1 - tanh Ha/Ha)`, ratio to Poiseuille `3(Ha - tanh Ha)/Ha^3` | J. Hartmann (1937); textbook MHD (Davidson) | citation UNVERIFIED (memory); the profile is DERIVED in ADR 0039 from `0 = G + mu u'' - sigma B0^2 u` and substituted into the momentum equation numerically (consistency test); the solver is compared with it (A1) |
| Hartmann flow, insulating walls, `u/U = (1 - cosh(Ha y/a)/cosh Ha)/(1 - tanh Ha/Ha)` and `G = sigma B0^2 U (tanh Ha/Ha)/(1 - tanh Ha/Ha)` | same | citation UNVERIFIED; DERIVED in ADR 0039 (zero net current); compared in A2 |
| Lorentz force identities (work `J.E` split, global `integral J x B dV` balance, electrode current `sigma B Ubar`) | electromagnetism | derived/standard; checked at round-off (M1, M2, A3) |
| Effectiveness-NTU, counterflow `(1 - e^{-N(1-Cr)})/(1 - Cr e^{-N(1-Cr)})` (`N/(1+N)` at `Cr = 1`), parallel flow `(1 - e^{-N(1+Cr)})/(1+Cr)`, `Cr = 0`: `1 - e^{-N}` | textbook heat-exchanger theory (Kays and London type) | citation UNVERIFIED (memory); DERIVED in ADR 0039 and compared with an independent marched two-stream integration, worst relative error 4.99e-13 (H1) |
| Crossflow, one stream mixed (`Cmax` mixed: `(1 - exp(-Cr(1 - e^{-N})))/Cr`; `Cmin` mixed: `1 - exp(-(1 - e^{-N Cr})/Cr)`) | same | DERIVED in ADR 0039; compared with cell-exact row recursions, relative error at most 4.4e-9 (H3) |
| Crossflow, both streams unmixed, curve fit `1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)]` | Kays and London type correlation | recalled from memory, UNVERIFIED; it is a fit: measured deviation from the exact (cell recursion) solution up to 3.3 percent (H3 known miss) and a wrong small-N slope (H2 known miss). The series solution is not implemented |
| Shell and tube, one shell pass, 2n tube passes closed form | textbook | recalled from memory, UNVERIFIED; checked only for `Cr = 0`, limits, ordering and monotonicity (H2); NO numeric agreement with a published table is claimed |
| Pressure-loss closure `dp = K rho v^2/2 + r1 v`, vent `dp = K rho v^2/2` | standard hydraulics | derived/standard; hand values (H4) and exact imposed drop in the CFD (X2) |
| Porous jump `dp = (mu/alpha) v t + C2 t rho v^2/2`, fan curve (piecewise linear) | ADR 0012 | prior evidence Z1, Z2 (not re-registered); used as analytic operating point in X4 |
