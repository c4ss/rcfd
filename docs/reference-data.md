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
| Realizable: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(eta+5)), Cmu = 1/(A0 + As U* k/eps), As = sqrt(6) cos(phi) | Fluent theory guide an unofficial copy (not linked); OpenFOAM `realizableKE` class docs (default A0 4.0, C2 1.9, sigma 1.0/1.2) | verified. Note A0 = 4.04 in Fluent (used by rcfd, `kepsilon.rs`) but 4.0 in OpenFOAM and, per one forum post, in Shih et al.; the original paper's printed value could not be fetched. Flagged in the source comment. **Wave 13 verification: Shih et al., NASA TM-106721 (https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf) prints A0 = 4.0 and Table 2 C2 1.9, sigma 1.0/1.2, C1 = max(0.43, eta/(5+eta)); rcfd's 4.04 differs from the original (see `docs/verification-report.md`).** |
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
- Dry-air mole fractions 0.78084 / 0.20946 / 0.00934 (N2/O2/Ar). Wave 13: NIST CIPM-2007 gives 0.780848 / 0.209390 / 0.009332; code differs by up to 0.09 % (immaterial).
- Peng-Robinson constants 0.45724, 0.07780, kappa = 0.37464 + 1.54226 omega - 0.26992 omega^2 and Redlich-Kwong constants
  0.42748, 0.08664, and the Wilke pair weight, as published by Peng and Robinson (Ind. Eng. Chem. Fundam. 15, 1976), Redlich and Kwong
  (Chem. Rev. 44, 1949) and Wilke (J. Chem. Phys. 18, 1950); the tests use only properties that follow from the forms
  themselves (critical-point conditions, ideal limit, root consistency, thermodynamic consistency of fugacity and residual enthalpy,
  the acentric-factor definition of the vapour pressure at reduced temperature 0.7 within 5 %: measured +0.38, -0.35, -0.23 %). The
  critical constants of N2 used in the tests (126.2 K, 3.39 MPa, omega 0.0372) are illustrative, from memory.
  Wave 13: PR 0.45724/0.07780/kappa polynomial confirmed (Wikipedia citing Peng and Robinson 1976; exact 0.457236, 0.077796
  computed), RK 0.42748/0.08664 confirmed (Wikipedia and closed form); the Wilke weight remains unverified. The N2 test constants
  differ from NIST (126.192 K, 3.3958 MPa) as stated.
- NIST WebBook data was deliberately not used (its terms were not reviewed); no REFPROP or other licensed file is read or shipped.
  (See the F-061 section below: later waves did embed WebBook values; wave 13 notes that NIST SRD 69 is copyrighted, see
  `docs/verification-report.md` section 1.6.)
- Critical constants and acentric factors in `rcfd-core/src/eos_data.rs` (header comment still says "from memory"): wave 13
  confirmed all seven fluids against the NIST WebBook fluid pages; only the Ar acentric factor differs (code -0.002, NIST -0.00219).


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
| F-203 C5 tree equals library run | 1e-12 | not run: build path hook missing | `rcfd-api/tests/wall_options.rs` (see ADR 0015 results) |

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
