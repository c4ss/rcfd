# Reference data audit (wave 6a)

This page is a lookup record for anyone who needs to know where a benchmark value or constant in a test, case or data
file comes from and how well it was checked. For the downloadable datasets, their checksums and licences, read
[validation data](validation-data.md) instead. The [glossary](glossary.md) explains project terms such as wave, ADR,
F-xxx and known miss.

Wave 6a checked every benchmark value or published constant that earlier waves typed from memory against a citable
public source. Counted by table row, the result is about 34 verified, 3 mismatched, about 8 unverifiable or not
re-checked (wave 6a counts; see the wave 13 note below and `docs/verification-report.md` for the later re-check). Nothing
was loosened: no tolerance was changed, and no benchmark number in a data file or assertion turned out to be wrong. The
mismatches are one sign error in a test helper (Taylor-Green initial pressure) and two citation or label errors
(Darwish-Moukalled journal; flat-plate "Prandtl-Schlichting" label); all were corrected in comments, notes or the test
helper, and every affected test still passes.

"Verified" means the value in the repo agrees with the source; "secondary" means the source is a reputable paper or
repository that reproduces the primary table, not the original printed table (the original journal papers were not
accessible online).

Wave 13 re-checked against web sources items that were marked unverified or from memory: turbulence-model and wall
constants, EOS critical data, Peng-Robinson/Redlich-Kwong constants, NIST isotherm values, Sutherland, atomic weights,
backward-facing-step and laminar-step values, Milne, Robertson, flame temperature, Formulation 1A, rheology constants.
The per-item values, URLs and verdicts are in `docs/verification-report.md`. One code constant differs from the original
(Realizable A0 4.04 vs 4.0). The unverified/from-memory markers below are updated in place where re-checked.

Later waves added their own sections at the end, in the order the work was done. Sources that could not be reached
online, and what still rests on secondary copies, are in "Not verifiable online"; registered criteria the code does not
meet, with their ignored known-miss tests, are in "Known misses".

On this page:

- Benchmarks and model constants: Lid-driven cavity; Differentially heated cavity; Exact Riemann solver, Sod problem and oblique
  shock; Taylor-Green vortex; Turbulence model constants; k-omega SST and wall treatment; Channel DNS; Cylinder wake;
  Backward-facing step; Laminar backward-facing step; Flat plate; the NACA four-digit thickness check.
- Literature and data: Other cited literature; Not verifiable online; Public validation-data pack.
- Models and formats, by feature: mesh-file format details; Viscosity laws; Material property library; Variable-density
  validation; Transient variable density; Known misses; Compressible boundary conditions; Real-gas equation of state; MRF zones;
  Interchange formats, registers and GLB validation; Lumped zones and radiation; reactors and FW-H acoustics; Spalart-Allmaras;
  Turbulence finishing.
- Wave sections, one per wave part, in the order the work was done, ending with the fan sections; among them "Wave 25B reference
  verification" (a re-check of earlier values) and "Wave 31C: reference reading" (sources read for the validation cases).

## Lid-driven cavity (Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411)

Used in `cases/cavity/reference/*.csv`, `crates/rcfd-fv/tests/cavity.rs`, `rcfd-io` cases test.

| Value | Source | Result |
|---|---|---|
| u centreline, Re 100/400/1000, 16 rows y = 0.0547 ... 0.9688 (48 numbers) | (a) a student course repository, github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 `data/validation/ghia/ghia_Re{100,400,1000}_u_centerline.csv` (this file omits y = 0.9766); (b) Uppsala bachelor thesis diva2:1668016, Table 2, which prints all 17 rows for Re 100/400/1000 (its y labels are shifted by one row by the PDF text extraction, but the value columns match our CSV row for row in order); (c) for Re 1000 also arXiv:1807.04896 Table A-1, "Ghia, Table 1, p. 398" | verified, two independent secondary copies, exact match of every number |
| u at y = 0.9766: 0.84123 / 0.75837 / 0.65928 | thesis (b) and, for Re 1000, arXiv:1807.04896 | verified (Re 1000 twice, Re 100/400 once; wave 31C: a third copy, gist ivan-pi/3e9326d18a366ffe6a8e5bfda6353219 (Table I typed by a third party), has all three, so Re 1000 three times and Re 100/400 twice) |
| v centreline Re 100, 17 rows | course repository `ghia_Re100_v_centerline.csv`; thesis (b) v-table (same shifted-label extraction; values in order -0.05906 ... 0.09233 match, including 0.05454, 0.17527, -0.24533) | verified, two secondary copies, exact match |
| Only Re 100 v is stored; Re 400/1000 v are not stored | n/a | not applicable. Wave 31C: the Re 400 and 1000 v tables (and u for all three Re, including y = 0.9766) were read from three secondary copies and stored in `validation-data/reference/w31c_ghia1982_{u,v}_centerline_read.csv`; see the wave 31C section and its discrepancy list |

Both secondary copies carry the known printed misprint of the Re=400 v table at x = 0.9063 (-0.23827; the thesis marks
it in red). rcfd does not use Re 400 v.

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

Wave 19A re-check (`crates/rcfd-fv/tests/coupled_open.rs` B3, Ra = 1e3 and 1e4 values 1.118 / 3.649 / 3.697 and
2.243 / 16.178 / 19.617): the two arXiv papers were downloaded and read as text. arXiv:1412.2866 Table 2 prints Nu 1.118
and 2.243, u_max 3.649 and 16.178, v_max 3.697 and 19.617 for the benchmark row. arXiv:physics/0305049 attributes
Nu 2.243, 4.519 and 8.800 to de Vahl Davis. The Cardinal V&V page answered with HTTP 403 in this wave and was not
re-read. The 1983 printed tables remain unreachable, so the values stay *verified through secondary sources* only. The
coupled-solver tests claim agreement with these numbers within 2 percent (measured 0.02 to 0.74 percent), nothing more.

Wave 30C (ADR 0061) re-checked the cavity table against the primary source. The 1983 paper itself was read as a page
scan (https://elearning.univ-bejaia.dz/mod/resource/view.php?id=71697, Table V on page 256), and all eleven quantities
at the four Rayleigh numbers were transcribed (`cases/heated_cavity/reference/devahldavis1983_tableV.csv`):

| Quantity | Values at the four Rayleigh numbers |
|---|---|
| Nu_avg | 1.118, 2.243, 4.519, 8.800 |
| Nu_max | 1.505, 3.528, 7.717, 17.925 at z = 0.092, 0.143, 0.081, 0.0378 |
| u_max | 3.649, 16.178, 34.73, 64.63 at z = 0.813, 0.823, 0.855, 0.850 |
| v_max | 3.697, 19.617, 68.59, 219.36 at x = 0.178, 0.119, 0.066, 0.0379 |

Every value matches the secondary sources above, so Nu_avg, u_max, v_max and Nu_max are now *verified, primary* and the
locations of the maxima are read for the first time. The paper states the accuracy as better than 1 percent at the
highest Rayleigh number and down to one tenth of that at the lowest. Tables I to IV (the other Prandtl numbers and the
reference-mesh listings) were not used. The transcription from the scan is by eye; the agreement with the independently
published copies is the check on it.

Other values read in wave 30C (all numeric tables of public pages, used by the ADR 0061 tests):

| Value | Source | Status |
|---|---|---|
| Sod star state p* 0.30313, u* 0.92745, rho*L 0.42632, rho*R 0.26557, shock speed 1.75216, fan head -1.18322 and tail -0.07027 | Marzouk, Sci. Afr. 10 (2020) e00573 = arXiv:2108.01709 Table 2 (read as text); the shock speed also from the Rankine-Hugoniot condition | verified, secondary (paper read; Sod 1978 and Toro not read) |
| Fully developed laminar tube Nu_D 3.66 (constant wall T) and 4.36 (constant flux); entry length x_fd,t = 0.05 Re Pr D; Hausen Nu_m = 3.66 + 0.0668 Gz / (1 + 0.04 Gz^(2/3)) | BYU ME340 table 8.4, https://www.et.byu.edu/~vps/ME340/TABLES/8.4.pdf | verified, secondary (Hausen's paper, Shah and London, Incropera not read). The 0.05 Re Pr D rule is the constant-T rule; constant flux needs a longer pipe (ADR 0061 addendum 3) |
| Parallel-plate Graetz tables (theta_b, local Nu for constant wall T and constant flux, limits 7.5407 and 8.2353) | NPTEL ME-662 lecture 19, https://archive.nptel.ac.in/content/storage2/courses/112101002/downloads/l19.pdf, slides 11 to 19; since 2026-10-27 the stored tables are recomputed by `validation-data/generate_graetz.py` (they agree with the transcription except constant-flux Nu at x+ = 1e-4: 32.1558 against 32.153 printed) | verified, secondary; the tube Graetz local-Nu table was not obtained (data not obtained) |
| Flat plate Cf = 1 / (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012) (Karman-Schoenherr), Coles-law u+(y+) at Re_theta = 10000 | NASA TMR https://turbmodels.larc.nasa.gov/flatplate_val.html (page text) and its `cf_K-S.dat`, `u+y+.dat` (formula agrees with the data file to 1e-6; 71 of the 2685 rows stored) | verified, primary data of the TMR; the page draws 5 percent error bars on Cf, which is the only stated uncertainty |

Not obtained in wave 30C: de Vahl Davis Tables I to IV, Shah and London, Hausen (1943), Incropera and Kays, Sod (1978), any NACA 0012 surface data (case not attempted).

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
| RNG: Cmu 0.0845, C1 1.42, C2 1.68, eta0 4.38, beta 0.012, C2* = C2 + Cmu eta^3 (1 - eta/eta0)/(1 + beta eta^3); sigma = 0.7194 (= 1/1.3929) | arXiv:2501.00037; Yakhot, Orszag, Thangam, Gatski and Speziale, Phys. Fluids A 4(7) (1992) 1510-1520 (not read). | verified (arXiv) |
| Realizable: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(eta+5)), Cmu = 1/(A0 + As U* k/eps), As = sqrt(6) cos(phi) | Shih et al. NASA TM-106721 (see below); OpenFOAM `realizableKE` class docs (default A0 4.0, C2 1.9, sigma 1.0/1.2) | verified. Note A0 = 4.04: UNVERIFIED against a published source, replace before relying on it; see the note under this table |
| Wall function kappa = 0.4187, E = 9.793; rounded to 0.41 / 9.8 by rcfd | Launder and Spalding, Comput. Methods Appl. Mech. Eng. 3 (1974) 269-289 (not read; attribution not checked) | UNVERIFIED against a published source, replace before relying on it (no published source for the pair was reached, see [provenance](provenance.md); rounding disclosed in code) |
| Log law kappa = 0.41, B = 5.2 (Pope 2000) | problem set based on Pope (perso.ens-lyon.fr), Abe et al. 2001; Wikipedia gives kappa 0.41, C+ 5.0 | verified, within the usual 5 percent spread |
| Reichardt (1951): u+ = ln(1 + 0.41 y+)/0.41 + 7.8[1 - e^{-y+/11} - (y+/11) e^{-y+/3}] | CFD-Online wiki "Reichardt profile" (via search result); numerical cross-check against MKM DNS | verified, and within 0.7, 0.8, 2.3, 2.3, 1.4 percent of DNS at y+ = 20, 30, 50, 100, 150 |
| Inlet relations k = 1.5 (I U)^2, eps = Cmu^0.75 k^1.5 / l, l = 0.07 L (`rcfd-api/src/turb.rs`) | CFD-Online/CFX forum guidance; l factor 0.07 for mixing-length codes | verified (k and l); eps = Cmu k^2/(nu * ratio) is the definition nu_t = Cmu k^2/eps, no external source needed |

Realizable A0 = 4.04 (used by rcfd, `kepsilon.rs`): UNVERIFIED against a published source (see
[provenance](provenance.md)), replace before relying on it; but 4.0 in OpenFOAM and, per one forum post, in Shih et al.;
the original paper's printed value could not be fetched. Flagged in the source comment. **Wave 13 verification: Shih et
al., NASA TM-106721 (https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf) prints A0 = 4.0 and
Table 2 C2 1.9, sigma 1.0/1.2, C1 = max(0.43, eta/(5+eta)); rcfd's 4.04 differs from the original (see
`docs/verification-report.md`).** ADR 0017: `KEpsilon::a0` / `turbulence.realizable_a0` selects 4.0 (original report) or
4.04 (a commercial-code variant, still the default); BFS-C medium x_r/h 6.349 with 4.0 against 6.413 with 4.04.

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

Profile samples:

| y+ | U+ |
|---|---|
| 5 | 4.82 |
| 10 | 8.44 |
| 15 | 10.657 (interpolated from the same chan395.means file for the SST test, verified from the file) |
| 20 | 12.007 |
| 30 | 13.493 |
| 50 | 14.856 |
| 100 | 16.534 |
| 150 | 17.646 |
| 300 | 19.745 |

## Cylinder wake (`crates/rcfd-fv/tests/cylinder.rs`)

| Value | Source | Result |
|---|---|---|
| St = 0.165 at Re = 100 | Williamson, Annu. Rev. Fluid Mech. 28 (1996), reported as 0.165 in the Re = 100 table of arXiv:2303.09262; same table: Kravchenko et al. 0.164, Posdziech and Grundmann 0.164, Sivakumar et al. 0.164, Oliveira 0.167; a vendor validation manual quotes the same 0.165 (not used as a source) | verified within 1 percent |
| "Williamson 1989 about 0.164" and "Henderson 1995 about 0.165" | wave 31C: Williamson, J. Fluid Mech. 206 (1989) 579-627, eq. (6), read in the author copy https://authors.library.caltech.edu/records/wavw6-f7k56/files/WILjfm89a.pdf: S = A/Re + B + C Re with A = -3.3265, B = 0.1816, C = 1.600e-4 (parallel shedding, study range Re 49 to 178), which gives St(100) = 0.164335 | Williamson 1989 about 0.164: verified (primary, computed from the printed fit; the paper prints no value at exactly Re = 100). Henderson 1995 about 0.165: still unverifiable. The CSV row "Williamson (experiment) 0.165" is the 1996 review value as quoted in arXiv:2303.09262; it differs from the 1989 fit by 0.4 % |

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
| Armaly et al., J. Fluid Mech. 127 (1983) 473: reattachment lengths versus Re | no numeric table accessible here (the abstract gives the Re range 70-8000 only); the experiment is 3-D with expansion ratio 1.942 and differs from the 2-D laminar computations above beyond Re about 400 | unverifiable here; no assertion uses it. Wave 31C: full text read (https://courses.washington.edu/me431/handouts/armaly-jfm-83.pdf): expansion ratio 1:1.94, Re = V D / nu with D = 2 h of the inlet channel, laminar regime Re < 1200, extra separated regions above Re about 400; numeric reattachment lengths only in figures: data not obtained |
| "Gartling Re 800 about 12.2, Armaly about 12" (F-106 note, from memory) | superseded by the Gartling value above | the Armaly value stays unverified. Wave 31C: Gartling 12.20 also seen as 6.10 channel heights (H = 1, step 0.5) in the MFIX-Exa tutorial quoting Gartling's Tables II and III (secondary); the Armaly paper was read in full, its reattachment lengths exist only as figures 4 and 14: data not obtained. See the wave 31C section (one conflicting secondary row 11.9 / 9.5 / 20.6) |

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
  Ghia Re 100/400 values at y = 0.9766 rest on two secondary sources since wave 31C (thesis, gist); the de Vahl Davis
  values were checked against the primary table in wave 30C.
- The "5.5 to 7" scatter statement (the expansion ratio 1.125 was confirmed in wave 13).
- Henderson 1995 St values (wave 31C: Williamson's 1989 fit coefficients and the paper's fit range were read in the primary paper).
- Toro Table 4.3 / NACA 1135 attributions.
- (Resolved in wave 13: Shih et al. print A0 = 4.0; rcfd uses 4.04.)
- Choi 1999 bibliographic details (Roe, Harten, Liou, Shu-Osher, Issa were confirmed via Crossref in wave 13).

## Public validation-data pack (F-125, fetched 2026-10-13)

Files are listed with checksums and licences in `validation-data/manifest.json` (see `docs/validation-data.md`).

- Flat plate Cf at x = 0.97: the pinned TMR file `FlatPlate/SST/cf_plate_sstv.dat` (CFL3D, SST) gives about 2.691e-3
  (2.6949e-3 at x = 0.9601, 2.6909e-3 at x = 0.9701). `crates/rcfd-turb/tests/flat_plate_sst.rs` uses 0.00268299
  (273 x 193 grid value from the TMR page). rcfd measures 0.002663: -0.7 % vs the page value, -1.0 % vs the file. Test
  tolerance (5 %) unchanged.
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

- the Carreau-Yasuda exponent convention `(n-1)/a`;
- the Cross form `1/(1+(lambda g)^m)`;
- the regularised yield term `tau0 (1 - exp(-m g))/g`;
- the WLF constants 17.44 and 51.6 K (the "universal" values, relative to the glass transition temperature, offered only
  as defaults).

Wave 13: the Carreau-Yasuda `(n-1)/a` convention and the WLF constants 17.44 and 51.6 K are now confirmed, secondary:
arXiv:1810.02292, IUPAC Gold Book goldbook.iupac.org/terms/view/12821; Cross and Papanastasiou remain unverified.

The checks that do not depend on those forms are analytic:

- the power-law plane channel profile and pressure gradient (`u = U (2n+1)/(n+1) [1 - |1-2y/H|^((n+1)/n)]`, derived by
  integrating the momentum balance);
- the Buckingham-Reiner plane solution of a Bingham fluid (`U = G h^2/(3 mu) (1 - 1.5 xi + 0.5 xi^3)`, `xi = tau0/(G h)`,
  mean speed verified to 1e-6 by quadrature of the profile in the test).

## Material property library (F-111)

Verified against primary sources in this session (fetched 2026-10-13):

- IAPWS-IF97 (IAPWS R7-97(2012), `iapws.org`): region 1 and region 2 coefficient tables, the B23 and saturation-pressure
  equations, and the property relations were read from the release itself. The program-verification values of Tables 5,
  15 and 35 (all three state points each) are reproduced to 1e-8 relative in `crates/rcfd-core/src/iapws.rs`, which also
  validates the transcription of the coefficients. IAPWS R12-08 (viscosity, Tables 1, 2, verification Table 4) and R15-11
  (thermal conductivity, Tables 1, 2, verification Table 4) likewise; their tables carry 7-9 digits, so the test
  tolerance there is 1e-7 relative (set at 1e-8 first, widened to 1e-7 after the 433.15 K, 1 kg/m^3 viscosity point
  missed it by 3.4e-8, which is the rounding of its last printed digit: a post-hoc change, disclosed).
  Not implemented: region 3, region 5, backward equations, critical enhancements.
- NASA polynomials (NASA TM-4513, McBride, Gordon and Reno 1993): coefficients for N2, O2, Ar, H2O, CO2, CH4, H2 copied from
  Cantera's `data/nasa_gas.yaml` (BSD-3-Clause, the file cites TM-4513); the Cantera licence requires keeping its notice,
  which is in the source header comment of `fluids.rs`. Consistency check: the two temperature ranges agree at 1000 K.
- Air viscosity: M. L. Huber, "Viscosity of Gases", CRC Handbook of Chemistry and Physics (NIST publication page, pub_id
  926439), air at 100 kPa: 13.3, 18.5, 23.1, 27.1, 30.8 uPa s at 200-600 K (read from the rendered table; 3 digits).
- Air conductivity: Huber, "Thermal Conductivity of Gases" (pub_id 926438): 18.5, 26.4, 33.5, 39.9, 46.0 mW/(m K).
- Air specific heat: NASA TN D-5452 Table I (Poferl, Svehla, Lewandowski 1969), 1.005, 1.013, 1.030, 1.051 J/(g K) at
  300-600 K (OCR of the scanned table, values consistent with the NASA polynomials to 0.12 %).
- Sutherland constants for air: 1.716e-5 Pa s at 273.15 K with S = 110.4 K, and 0.0241 W/(m K) with S = 194 K, are the
  usual textbook values written from memory (the viscosity pair also is the US Standard Atmosphere 1976 pair); only their
  agreement with the Huber tables is checked. Wave 13: confirmed (secondary) against CFD-Online "Sutherland's law" and the
  1976 standard-atmosphere constants (C1 = 1.458e-6, S = 110.4 K gives 1.7161e-5 at 273.15 K), and arXiv:2210.01621 for
  the conductivity pair (2.41e-2 W/(m K), T0 273 K, S 194 K).

Sutherland's law result, pre-registered at 1 % for 300-500 K:

| Property | 300 K | 400 K | 500 K | Also 200 K | Also 600 K |
|---|---|---|---|---|---|
| Viscosity | -0.22 % | **-1.08 %** | **-1.46 %** | -0.11 % | -2.07 % |
| Conductivity | -0.64 % | +0.26 % | +0.69 % | -3.23 % | +0.35 % |

The 1 % criterion is NOT met by Sutherland's law for the viscosity above about 350 K; for the conductivity it is met. The
viscosity assert for Sutherland air is a regression guard of 1.5 % set after the measurement; the 1 % criterion itself
is recorded as missed.

The cubic fits `air_viscosity_fit_law`/`air_conductivity_fit_law` were fitted to the same five table rows (in-sample
residual at most 0.11 % and 0.08 %; leave-one-out prediction of the 400 K row -0.22 % and -0.15 %). The 1 % assert on the
cubic fits is in-sample.

Unverified (from memory, not checked against a primary source in this session; the wave 13 outcome is appended to each
item):

- Standard atomic weights used for the molar masses (N 14.007, O 15.999, Ar 39.948, H 1.008, C 12.011). Wave 13: all
  inside the CIAAW intervals (ciaaw.org), confirmed.
- Dry-air mole fractions 0.78084 / 0.20946 / 0.00934 (N2/O2/Ar). Wave 13: NIST CIPM-2007 gives 0.780848 / 0.209390 /
  0.009332; code differed by up to 0.09 % (immaterial). Post-verification fix: `GasMixture::dry_air` now uses the
  CIPM-2007 values 0.780848 / 0.209390 / 0.009332 (renormalised over the three species); its only user is the air
  cp/transport unit test, whose cp values moved by about 0.01 J/kg/K, all assertions unchanged and passing.
- Peng-Robinson constants 0.45724, 0.07780, kappa = 0.37464 + 1.54226 omega - 0.26992 omega^2 and Redlich-Kwong constants
  0.42748, 0.08664, and the Wilke pair weight, as published by Peng and Robinson (Ind. Eng. Chem. Fundam. 15, 1976),
  Redlich and Kwong (Chem. Rev. 44, 1949) and Wilke (J. Chem. Phys. 18, 1950). The tests use only properties that follow
  from the forms themselves (critical-point conditions, ideal limit, root consistency, thermodynamic consistency of
  fugacity and residual enthalpy, the acentric-factor definition of the vapour pressure at reduced temperature 0.7 within
  5 %: measured +0.38, -0.35, -0.23 %). The critical constants of N2 used in the tests were illustrative (126.2 K,
  3.39 MPa); they are now the NIST values 126.192 K, 3.3958 MPa (omega 0.0372), tests unchanged and passing. Wave 13: PR
  0.45724/0.07780/kappa polynomial confirmed (Wikipedia citing Peng and Robinson 1976; exact 0.457236, 0.077796 computed),
  RK 0.42748/0.08664 confirmed (Wikipedia and closed form); the Wilke weight remains unverified. The N2 test constants
  differed from NIST (126.192 K, 3.3958 MPa) as stated; fixed afterwards, see above.
- NIST WebBook data was deliberately not used (its terms were not reviewed); no REFPROP or other licensed file is read or shipped.
  (See the F-061 section below: later waves did embed WebBook values; wave 13 notes that NIST SRD 69 is copyrighted, see
  `docs/verification-report.md` section 1.6.)
- Critical constants and acentric factors in `rcfd-core/src/eos_data.rs` (the header comment now points at
  `docs/verification-report.md`): wave 13 confirmed all seven fluids against the NIST WebBook fluid pages; only the Ar
  acentric factor differed (code -0.002, NIST -0.00219); fixed to -0.00219 afterwards (no test uses Ar PR against NIST;
  workspace tests unchanged).

## Variable-density validation (wave 10, ADR 0005)

Verified against the primary source:

- Le Quere, Weisman, Paillere, Vierendeels, Dick, Becker, Braack and Locke, "Modelling of natural convection flows with
  large temperature differences: a benchmark problem for low Mach number solvers. Part 1. Reference solutions", ESAIM: M2AN
  39(3) 609-616 (2005), DOI 10.1051/m2an:2005027; full text read from the author's postprint
  https://wrap.warwick.ac.uk/id/eprint/3824/1/WRAP_Locke_modelling_natural_convection.pdf. Its Table 1, Sutherland law and
  thermodynamic pressure follow this list.
- Quasi-1-D isentropic relations and the area-Mach relation (A/A*)(M) = (1/M) [2/(gamma+1) (1 + (gamma-1)/2 M^2)]^((gamma+1)/(2(gamma-1))),
  choked mass flow A* p0 sqrt(gamma/(R T0)) (2/(gamma+1))^((gamma+1)/(2(gamma-1))): standard textbook formulae (for example
  Anderson, Modern Compressible Flow), not looked up in this session; the code of the checks uses them as written here.
- Hydrostatic ideal-gas columns: analytic atmospheres dp/dy = -rho g with p = rho R T (isothermal exponential; linear T
  polytropic with exponent g/(R a)), derived in the test headers.

Le Quere et al. Table 1 (square cavity, adiabatic horizontal walls, no slip, P0 = 101325 Pa, T0 = 600 K, R = 287,
Pr = 0.71, gamma = 1.4, g = 9.81, epsilon = (Th-Tc)/(Th+Tc) = 0.6, Ra = Pr g rho0^2 (Th-Tc) L^3 / (T0 mu0^2) with a
single g):

| Case | Ra | Properties | Nu | P/P0 |
|---|---|---|---|---|
| T1 | 1e6 | constant properties | 8.85978 | 0.856338 |
| T2 | 1e6 | Sutherland | 8.6866 | 0.924487 |
| T3 (not used) | 1e7 | Sutherland | 16.2410 | 0.92263 |

Their Sutherland law: mu = mu* (T/T*)^1.5 (T*+S)/(T+S), T* = 273 K, S = 110.5 K, mu* = 1.68e-5 Pa s,
k = mu gamma R / ((gamma-1) Pr). Thermodynamic pressure P(t) = P0 integral(1/T0 dv) / integral(1/T dv).

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

Each row is a registered criterion that the code does not meet. The threshold in each test is the registered one, not
widened. Each test is `#[ignore]`d and its name ends in `known_miss`, so it fails by design when run with `--ignored` and
will start passing if the solver improves. The values were measured in release with `CARGO_TARGET_DIR=target-fix2`; no
solver parameter changed.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-042 k-epsilon BFS, Realizable | x_r/h in 6.26 +- 0.10 (Driver and Seegmiller) | 6.402 (0.042 outside) | `rcfd-turb/tests/step_kepsilon.rs::step_reattachment_realizable_known_miss` |
| F-042 k-epsilon BFS, RNG | same | 6.373 (0.013 outside) | `step_reattachment_rng_known_miss` |
| F-042 k-epsilon BFS, Standard | same | 5.359 (0.80 below) | `step_reattachment_standard_known_miss` |
| F-061 nozzle case file | tolerance_u = tolerance_p = 1e-8 within 3000 iterations | res_u 2.62e-5, res_p 6.07e-13, not converged | `rcfd-api/tests/variable_density.rs::subsonic_nozzle_residual_target_1e8_known_miss` |
| F-043 SST flat plate vs White | Cf(0.97) within 5 % of 0.455/ln^2(0.06 Re_x) | 0.002663 vs 0.002875, -7.36 % (-6.6 to -8.5 % along the plate) | `rcfd-turb/tests/flat_plate_sst.rs::sst_flat_plate_cf_within_5pct_of_white_known_miss` |
| F-111 Sutherland air viscosity | within 1 % of Huber table, 300-500 K | -0.22, -1.08, -1.46 % | `rcfd-core/src/fluids.rs::sutherland_air_viscosity_within_1pct_300_to_500_k_known_miss` |

Met and now asserted, in `sst_flat_plate_cf` (ignored, heavy, about 6 minutes): F-043 Cf(0.97) within 5 % of the TMR page
value 0.00268299 (-0.7 %) and of the TMR file value 0.0026909 (-1.0 %).

The air test that asserted 1.5 % for Sutherland viscosity was renamed
`air_cp_conductivity_fit_laws_within_1pct_and_sutherland_viscosity_within_1p5pct_300_to_500_k`. The 1.5 % is a regression
guard; the 1 % fit-law check is in-sample.


## Compressible boundary conditions (F-062, ADR 0006)

| Value | Source | Result |
|---|---|---|
| Characteristic (LODI) boundary treatment, pressure-relaxation outlet `K (p - p_ref)` | Thompson, J. Comput. Phys. 68 (1987) 1-24; Poinsot and Lele, J. Comput. Phys. 101 (1992) 104-129 | bibliographic details confirmed via Crossref (wave 13); formulas re-derived in ADR 0006, not checked against the papers, unverified |
| Riemann-invariant far field, total-pressure inlet | Blazek, *Computational Fluid Dynamics: Principles and Applications*, ch. 8 | textbook formulas re-derived; book not accessed here, unverified |
| Isentropic and area-Mach relations (nozzle check) | Anderson, *Modern Compressible Flow* | standard textbook formulae, same as F-061 |
| Vassberg and Jameson, J. Aircraft 47(4) (2010) 1152-1166 (Euler NACA 0012 grid convergence) | listed in the reference list of arXiv:2407.03746 (fetched) | citation verified; its Cl, Cd and Cp values were NOT accessed |
| AGARD AR-211 NACA 0012 transonic Cp scatter | not accessed | no numbers available; the F-062 airfoil criterion is therefore not attempted (wave 31C: not attempted again; the incompressible NASA TMR experiment subsets were read instead, see the wave 31C section) |
| NACA 0012 M = 0.8, alpha = 1.25 degrees upper/lower shock positions (about x/c 0.62 / 0.34 in Euler solutions) | recollection only | UNVERIFIED, not used |


## Real-gas equation of state (F-061, ADR 0007)

The isotherm values come from the NIST Chemistry WebBook, fluid properties, isothermal tables
(webbook.nist.gov/cgi/fluid.cgi, CO2 ID C124389, N2 ID C7727379, default reference state), retrieved 2026-10-14 through a
page fetcher whose output passes through a language-model summariser. The numbers (density, enthalpy, entropy, cp, sound
speed at the listed T and p) are copied into `crates/rcfd-fv/tests/realgas_nist.rs` as returned. They were NOT re-read from
the raw page, so they are marked "retrieved, not independently re-read". As a sanity check, CO2 350 K 0.1 MPa 1.5167 kg/m^3
and N2 300 K 0.1 MPa 1.1233 kg/m^3 agree with the ideal gas to 0.3 percent and 0.0 percent. The tables list some rows
(8.1 MPa, 5.1 MPa, 6.1 MPa) three times with different phase labels and identical values.

Wave 13 update: the isotherm values were re-read from the raw WebBook HTML (no summariser) and all match. The critical
constants below were confirmed against the NIST fluid pages (only Ar omega differs). See `docs/verification-report.md`.

Originally UNVERIFIED (written from memory of the literature, not looked up): critical constants and acentric factors in
`rcfd-core/src/eos_data.rs` (CO2 304.1282 K, 7.3773 MPa, 0.22394; N2 126.192 K, 3.3958 MPa, 0.0372; O2, Ar, CH4, H2, H2O
likewise). The Peng-Robinson (1976) and Redlich-Kwong (1949) constants were already listed as unverified under F-111. The
`rcfd-fv/src/realgas.rs` tests checked the thermodynamic consistency of the departure functions numerically instead
(Maxwell relations `dh/dp|T = v - T dv/dT|p` and `ds/dp|T = -dv/dT|p`).

The nozzle tests compute their real-gas isentropic reference inside the test from the same EOS (an ODE in p with
`dT/dp|s = T (dv/dT)_p / cp`). This reference checks the solver against the thermodynamics, not the EOS against nature.
The (R1)-(R3) tests cover EOS accuracy against NIST: density within 1 percent for |Z-1| <= 0.05 etc. The table below lists
the misses.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-061 PR density vs NIST | within 1 percent where \|Z_NIST - 1\| <= 0.05 | N2 300 K: 5.1 MPa +1.03, 10.1 MPa +1.68, 15.1 MPa +2.24 percent | `rcfd-fv/tests/realgas_nist.rs::r1_peng_robinson_density_vs_nist_known_miss` |
| F-061 PR enthalpy difference vs NIST | within 10 percent | N2 300 K: +18.8 (5.1 MPa), +15.7 (10.1), +12.3 (20.1) percent; CO2 350 K +9.3/+8.5/+7.8, 310 K +2.6/+1.2 pass | `r2_departure_enthalpy_and_entropy_differences_vs_nist_known_miss` |
| F-061 ideal-gas limit at 1 Pa | h_dep <= 1e-8 cp0 T and s_dep <= 1e-8 cp0 | CO2 PR 3.1e-8 / 2.1e-8, RK 2.6e-8 / 1.7e-8 (departures scale with p; limit was mis-estimated) | `r3_ideal_gas_limit_at_one_pascal_known_miss` |


## MRF zones (F-031, F-114, ADR 0008, `crates/rcfd-api/tests/mrf.rs`)

All reference values are analytic (solid-body rotation, Taylor-Couette flow `u_theta = A r + B/r`, torque
`4 pi rho nu W_i r_i^2 r_o^2/(r_o^2 - r_i^2)` per unit height) and derived in the ADR. No literature data and no pump or
turbine benchmark are used or claimed. The set-up is r_i 0.5, r_o 1, nu 0.1, 48 cells around. Limits were registered
before the runs; misses are ignored `*_known_miss` tests.

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

ADR 0012 (addendum) records the measurements against these references. Registered misses: Z3 network pressure 8.5e-4 vs
1e-4 (`z3_network_pressure_known_miss`), Z4 effectiveness duty vs nominal formula 1.45e-6 vs 1e-6
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

The rotating dipole is realised as a pair of opposite monopoles on a circular orbit. A rotating point force with an exact
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
| Cebeci-Bradshaw roughness shift (transitional range exponent, ks+ = 2.25 and 90, C_s) | Cebeci and Bradshaw (1977) | wave 13: constants 2.25, 90, 87.75, 0.4258, 0.811 and the fully rough 1 + C_s ks+ confirmed in OpenFOAM v6 `nutkRoughWallFunction` source; the book was not read; the rounded constants 0.4258 and 0.811 replaced by their exact forms (equal to 5 digits) and the transitional shift clamped at 0 (deviation, ADR 0015); Nikuradse intercept 8.5 confirmed, the "textbook C_s = 0.5" remark not found |
| Scalable wall function form (y* limited below the viscous/log intersection) | Grotjans and Menter (1998) | wave 13: concept from secondary descriptions only, UNVERIFIED. The limit is the root of y* = ln(E y*)/kappa: 11.06 for kappa 0.41, B 5.2 (E = e^(kappa B); Pope, Turbulent Flows (2000), sec. 7.1, not re-read), 11.225 for kappa 0.4187, E 9.793, and rcfd's y*_lam = 11.53 for its rounded 0.41 / 9.8; paper formulas not read |
| Two-layer constants A_mu = 70, A_eps = 2 c_l, blend Re_y* = 200 | Chen and Patel (1988), Jongen (1998) | wave 13: UNVERIFIED against a published source, replace before relying on it (constants and tanh blend form not found in a published source, see [provenance](provenance.md); Wolfshtein, Int. J. Heat Mass Transfer 12 (1969) 301-318, is the usual origin of the two-layer length scales, not read); Chen and Patel citation confirmed (Crossref) but the constants not seen in the paper; blend width 0.1 Re_y* is an rcfd choice within the quoted 5 to 20 percent |
| Kader blended law (Gamma = -0.01 y+^4/(1 + 5 y+)) | Kader (1981) | wave 13: confirmed (secondary) in a heat-transfer paper writing the equivalent positive-Gamma form (Xia et al., see `docs/verification-report.md`); the 1981 paper was not read |
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

Wave 16W introduces no new literature value. The expected values of the registered checks are closed forms or equalities
with the library call the wiring surfaces: the linear velocity field `u = x + 2y`, `v = 3x - y` (vorticity 1, Q = -7), the
Couette wall shear `mu U/h` with `U = 3`, `h = 0.5`, and the y+ definition `y+ = y u_tau / nu` with
`u_tau = sqrt(tau/rho)`. The references already marked unverified under the wave 15B heading (partitioning, reverse
Cuthill-McKee, isentropic total pressure) are unchanged and still unverified (memory).

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

Wave 17C introduces no physical reference value. The checks use these conventions, all from memory and unverified as
citations:

- Rec. 709 luma weights (0.2126, 0.7152, 0.0722) for the colour-map luminance test.
- The "1-2-5" family (with 2.5) of tick steps.
- Min/max bucket decimation for plot data.

The OpenAI-compatible wire format (`POST /v1/chat/completions`, `GET /v1/models`, SSE `data:` events with `[DONE]`,
`Authorization: Bearer`) and the default local ports (Ollama 11434, LM Studio 1234, llama.cpp 8080, vLLM 8000) are recalled
from memory and unverified against current documentation. They were checked only against the loopback mock server written
for the tests. That server shares the author's understanding of the format, so agreement is not independent evidence about
any real server.

The lid-driven cavity logic check (top-row mean ux, return flow) is a sanity bound, not a benchmark comparison.

Wave 17A (ADR 0023): convergence floor diagnosis and pseudo-transient control. Known misses added (limits as registered,
never widened; ignored tests):

| Item | Limit | Measured | Test |
| --- | --- | --- | --- |
| F-106 `ser` controller, cavity Re 1000 / step Re_h 400 | residual 1e-6 | not converged (cavity 0.18 after 6000 its, scale at cap) | `rcfd-fv/tests/pseudo_control.rs::*_known_miss` |
| F-106 `ser2` controller (symmetric), same cases | residual 1e-6 | cavity 1.6e-2, step 7.7e-5 | `pseudo_control.rs::*_ser2_known_miss` |
| F-106 controller on C2/D1/D2 (local cfl 1, physical 0.2) | converge, registered accuracy | residual 0.21 or NaN | `rcfd-api/tests/convergence_wave17a.rs::f?_*_known_miss`, `f?b_*_known_miss` |
| F-209 error-controlled stepping, linear decay (relative and absolute scale) | steps <= 0.8 x uniform | 81 vs 76; 46 vs 36 | `rcfd-fv/src/ddt.rs::e1*_known_miss` |

Findings:

- The F-200 C2/D1/D2/E2 residual targets are met with `solver.residual_norm = scaled` (new passing tests F1c, F2c, F3c,
  F4); the reference normalisation stalled by construction.
- The F-061 nozzle 1e-8 residual floor (2.619e-5) stays undiagnosed.
- Wave-13 D5b n_r = 128 is slow convergence.

No external reference data were used in wave 17A. The Taylor-Green and flat-plate acceptance items of F-209 were not run.

## Wave 17B conjugate heat transfer (F-051, ADR 0024)

Every reference value used by the registered checks is derived in the ADR or is closed-form arithmetic. Every source
attribution below is from memory and unverified as a citation:

| Quantity | Source | Status |
| --- | --- | --- |
| 1-D steady conduction through layers in series with a convective end, `q = dT / (sum L_i/k_i + 1/h)` | standard heat transfer | derived (series resistances); unverified (memory) as a citation |
| Fin of uniform section, tip convection: `Q = sqrt(h P k A) theta_b (sinh mL + (h/mk) cosh mL) / (cosh mL + (h/mk) sinh mL)` | standard heat transfer | recalled from memory; wave 31C: the formula (convective tip, `m^2 = h P / (k A_c)`) is printed in the Wikipedia article "Fin (extended surface)" (secondary, formulas from the page's LaTeX alt text): verified (secondary); the 1% bound is an engineering limit and the measured 3.3e-4 compares the 2-D solve with this 1-D formula |
| Lumped slab `T = T_inf + (T0 - T_inf) exp(-t/tau)` (small Biot number) | standard | derived; unverified (memory); the 1% bound is an engineering limit (Biot number of the set-up is the ADR's) |
| Semi-infinite contact of two solids, `T_i = (e1 T1 + e2 T2)/(e1 + e2)`, `e = sqrt(k rho c)`, `T = T_i -/+ (..) erfc(x / (2 sqrt(alpha t)))` | Carslaw and Jaeger; Incropera | unverified (memory); the `erfc` series and continued fraction are implemented in the test file and were not compared with a table |
| Stefan-Boltzmann constant | CODATA exact SI values `sigma = 2 pi^5 k_B^4 / (15 h^3 c^2)` | computed in code from exact SI constants and unit-tested against 5.670374419e-8; the value 5.670374419e-8 is verified in wave 31C: the NIST CODATA 2022 page physics.nist.gov/cgi-bin/cuu/Value?sigma prints "5.670 374 419... x 10^-8 W m^-2 K^-4", standard uncertainty (exact) |
| Fully developed laminar flow between parallel plates with uniform wall heat flux, `Nu = 140/17 = 8.2353` (`D_h = 4H`, half channel `H`) | derived in ADR 0024 section D from the energy equation with Poiseuille velocity | derived; the textbook value 8.235 (uniform flux, plates) was recalled from memory and is verified since wave 30C (NPTEL ME-662 lecture 19, limit 8.2353); the derivation, not the recollection, is the reference |
| Contact resistance `R''` in series, `1/k_eff = 1/k + R''/Delta` | standard | derived; unverified (memory) |

Known deviations of the checks from the ADR text (details in the ADR addendum):

- The duct tests use central convection, since upwind is not exact for the linear axial profile at the Dirichlet
  inlet face.
- The steady stop criterion of the library tests is 1e-11, not 1e-13.

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
| Zeldovich rate constants (O + N2 = NO + N etc.) | Hanson and Salimian evaluation | UNVERIFIED: the primary evaluation was not accessed and no published confirmation was found; the tests use invented constants |
| Nine-coefficient polynomial `cp/R = a1 T^-2 + a2 T^-1 + a3 + a4 T + a5 T^2 + a6 T^3 + a7 T^4`, with `h/RT` and `s/R` obtained by integration | NASA Glenn thermodynamic polynomial form | functional form as commonly stated, UNVERIFIED against the report; the integrated forms are verified by derivative consistency (K12a). The text file layout of the public database is NOT implemented |

## Wave 19B meshing references (ADR 0030)

No published source for the following was accessed in this environment. The algorithms were written from memory and are
tested against closed-form or exact-arithmetic checks only.

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

ADR 0035 derives the closed forms below from conservation and Fourier/Ohm laws, and each is checked by a registered test.
None is a data claim. Web search was attempted for the Stefan solution and the enthalpy-porosity paper; primary texts were
not retrievable.

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
| Constant-kernel exponential solution `n(v,t) = N0/(1+tau)^2 exp(-v/(1+tau))`, `tau = K N0 t/2`; moments `m_k = k! N0 (1+tau)^(k-1)` | Smoluchowski (1917); W. T. Scott (1968), J. Atmos. Sci. 25, 54-65 (author initials CORRECTED in wave 25B, see the wave 25B verification section) | citation record verified in wave 25B (Crossref); the solution is DERIVED in ADR 0041 (substitution into the moment equation, every `k`) |
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

No web page and no paper was consulted in this wave. Every formula below is reproduced from knowledge and is checked in
the tests only against closed forms, invariants and internal consistency, NOT against the original publication. Entries
are therefore UNVERIFIED as citations unless marked "derived".

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
| Crossflow, both streams unmixed, curve fit `1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)]` | Kays and London type correlation | RETIRED in wave 25B (ADR 0044): replaced by the exact series; the fit itself was never located in a source. Historical note: it is a fit: measured deviation from the exact (cell recursion) solution up to 3.3 percent (H3 known miss) and a wrong small-N slope (H2 known miss). The series solution IS implemented since wave 25B |
| Shell and tube, one shell pass, 2n tube passes closed form | textbook | recalled from memory, UNVERIFIED; checked only for `Cr = 0`, limits, ordering and monotonicity (H2); NO numeric agreement with a published table is claimed |
| Pressure-loss closure `dp = K rho v^2/2 + r1 v`, vent `dp = K rho v^2/2` | standard hydraulics | derived/standard; hand values (H4) and exact imposed drop in the CFD (X2) |
| Porous jump `dp = (mu/alpha) v t + C2 t rho v^2/2`, fan curve (piecewise linear) | ADR 0012 | prior evidence Z1, Z2 (not re-registered); used as analytic operating point in X4 |

## Wave 25B reference verification (ADR 0044 D5)

This verification changed no registered limit, and no formula disagreement required a code change. It found these
discrepancies:

- Scott author initials
- Lee year and title
- Jacobsen blend attribution (doubtful)
- Zwart bubble radius (1e-6 vs 2e-6 m in sources)
- Neumann equation is a special case

Protocol: this section credits only pages that were actually fetched or returned as search results in wave 25B.
"Record" means authors, title, journal, volume, pages and year were found in a bibliographic record (Crossref API, https://api.crossref.org/works, or a publisher or archive page). "Formula"
means the formula itself was read in a source or re-derived from a source. No vendor documentation was used as
authority. Page content was treated as data. Anything not listed here keeps the status in the wave sections above
(UNVERIFIED).

| Item | Status | Evidence |
|---|---|---|
| Kim and Moin (1985) J. Comput. Phys. 59, 308-323 | record VERIFIED | Crossref, DOI 10.1016/0021-9991(85)90148-2 (resolves the volume/pages note) |
| Zang, Street, Koseff (1994) J. Comput. Phys. 114, 18-33 | record VERIFIED; formula not checked | Crossref, DOI 10.1006/jcph.1994.1146 |
| Timmermans, Minev, van de Vosse (1996) Int. J. Numer. Methods Fluids 22, 673-688 | record VERIFIED; rotational form not checked | Crossref, DOI 10.1002/(SICI)1097-0363(19960415)22:7<673::AID-FLD373>3.0.CO;2-O |
| Kumar and Ramkrishna (1996) Chem. Eng. Sci. 51, 1311-1332 | record VERIFIED; scheme not checked | Crossref, DOI 10.1016/0009-2509(96)88489-2 |
| McGraw (1997) Aerosol Sci. Technol. 27, 255-265 | record VERIFIED | Crossref, DOI 10.1080/02786829708965471 |
| Saffman and Turner (1956) J. Fluid Mech. 1, 16-30 | record VERIFIED; constant sqrt(8 pi/15) STILL UNVERIFIED | Crossref, DOI 10.1017/s0022112056000020; search results confirm the structure (eps/nu)^(1/2)(r_i+r_j)^3 but no retrieved source stated the prefactor |
| Hill and Ng (1995) AIChE J. 41, 1204-1216 | record VERIFIED; symmetric-beta form not checked | Crossref, DOI 10.1002/aic.690410516 (Hill and Ng 1996, AIChE J. 42, 1600, is a different paper) |
| Scott (1968) J. Atmos. Sci. 25, 54-65 | CORRECTED: author is William T. Scott (not P. J. Scott); record VERIFIED | Crossref, DOI 10.1175/1520-0469(1968)025<0054:asocdc>2.0.co;2, "Analytic Studies of Cloud Droplet Coalescence I" |
| Gordon (1968) J. Math. Phys. 9, 655-663 | record VERIFIED; product-difference attribution not checked | Crossref, DOI 10.1063/1.1664624 |
| Wheeler (1974) Rocky Mountain J. Math. 4 | record VERIFIED for authors, year, journal, volume; start page 287 inferred from the DOI only; algorithm not checked | Crossref, DOI 10.1216/rmj-1974-4-2-287 |
| Smoluchowski (1917) Z. phys. Chem. 92, 129-168 | record VERIFIED (search results, reference lists, no page opened); kernels not checked | web search |
| Jacobsen, Fuhrman, Fredsoe, Int. J. Numer. Meth. Fluids 70(9), 1073-1088, year 2012 | record VERIFIED (Crossref DOI 10.1002/fld.2726 and DTU Orbit page; Crossref "issued 2011" is the online date) | attribution of the exponential blend `1 - (exp(chi^P) - 1)/(e - 1)` is DOUBTFUL: a search summary of arXiv:1810.03492 attributes the form to Fuhrman, Madsen and Bingham (2006). The PDF was not read. Formula, default exponent 3.5 and attribution STILL UNVERIFIED |
| Hartmann (1937), Kgl. Danske Vid. Selsk., Mat.-fys. Medd. 15 no. 6 (single author J. Hartmann) | record VERIFIED from a search result (scan at gymarkiv.sdu.dk, not opened); Part II (no. 7) is Hartmann and Lazarus | profile form `cosh(Ha) - cosh(Ha y)` seen at ar5iv 1502.07683 Eq. (1); the normalisation and the mean ratio `3(Ha - tanh Ha)/Ha^3` were re-derived by hand, not found in a fetched source |
| Lee model | formula form seen only in search summaries derived from code documentation, which is not authority: STILL UNVERIFIED. Report title and year CORRECTED-candidate: W. H. Lee, "A Pressure Iteration Scheme for Two-Phase Flow Modeling", Los Alamos Scientific Laboratory LA-UR 79-975, 1979 (the 1980 in the wave 23B entry is probably the later book chapter) | no fetched page confirmed it |
| Zwart, Gerber, Belamri (2004), 5th Int. Conf. Multiphase Flow, Yokohama | STILL UNVERIFIED (probable): only reference-list snippets in search results; default F_vap = 50, F_cond = 0.01, alpha_nuc = 5e-4 reported by summaries; R_B appears as 1e-6 m in some and 2e-6 m in other sources, a discrepancy that is a finding (docs use 1e-6; the value is a Default only) | no page fetched |
| Hasselmann et al. (1973), Deutsche Hydrographische Z. Reihe A (8 degrees) No. 12, 1-95 | record PARTIAL (secondary: scientimate page fetched, AWI EPIC listing seen in a search summary only; the scan PDF was unreadable) | JONSWAP shape factors s = 0.07 (w <= wp) and 0.09 (w > wp), gamma = 3.3 and the `gamma^r` structure VERIFIED (secondary) at https://en.wikipedia.org/wiki/Wave_spectrum (ITTC form, `944/T1^4 = 1.25 wp^4` with wp = 5.24/T1) |
| Pierson-Moskowitz Hs-wp form | formula VERIFIED by derivation from the ITTC form on the same Wikipedia page (peak condition gives exponent `1.25 (wp/w)^4` and prefactor `5/16 Hs^2 wp^4 w^-5` exactly); Bretschneider/PM attribution not checked | as above |
| Effectiveness-NTU: counterflow, parallel flow, Cmin-mixed, Cmax-mixed | formulas VERIFIED (secondary) at https://en.wikipedia.org/wiki/NTU_method; all match the code; Kays and London book record not checked | page fetched |
| Crossflow both unmixed exact series | VERIFIED (secondary + numerical): the page above gives `eps = 1 - e^-N - e^-(1+Cr)N sum Cr^n P_n(N)`; evaluated independently it agrees with the implemented Poisson-tail series to 9 digits at five (N, Cr) points, e.g. 0.476222388 at (1, 1) | implemented in wave 25B (ADR 0044 D1, tests h5_x1..x5); the earlier curve fit is RETIRED and was never found in a source |
| Shell and tube, one shell pass, 2n tube passes | STILL UNVERIFIED (no legible page fetched); consistency note: Cr = 1, N to infinity gives 2/(2 + sqrt 2) = 0.586 | |
| Neumann two-phase Stefan solution (wave 21A / ADR 0035) | CORRECTED-note: the equation in ADR 0035 (single exponent `e^-l^2`, one diffusivity) is the equal-diffusivity special case of the general classical Neumann condition (Roscani and Tarzia, https://ar5iv.labs.arxiv.org/html/1405.5928, eq. 3.31, which carries the diffusivity ratio through `lambda = lambda1/lambda2`); it reduces to ADR 0035 at lambda = 1. ADR 0035 states equal properties, so no code change. One-phase limit `beta lambda = e^-lambda^2/(sqrt(pi) erf lambda)` VERIFIED (secondary) at https://en.wikipedia.org/wiki/Stefan_problem | Carslaw and Jaeger, Alexiades and Solomon, Crank not fetched |
| Nusselt (1916), Schrage (1953), Rohsenow correction, Dean and Dalrymple second-order Stokes, Andrade-form name, Wheeler algorithm, Kumar-Ramkrishna formulas, Brownian and laminar-shear kernels, Smoluchowski constant-kernel solution (as cited) | STILL UNVERIFIED: not fetched in wave 25B | the numerical checks in the respective tests remain closed-form or derived, as before |

## Wave 25C transient momentum sources (F-214, F-217, F-218, ADR 0045)

No literature source was consulted in wave 25C. Every reference value below is derived from first principles in this
section or reused from the earlier waves named in ADR 0045. Nothing is credited to a book or paper.

**Impulsively accelerated plane Poiseuille flow (check T2).** A uniform body acceleration `G` along `x` acts from `t = 0`
on a fluid at rest between no-slip plates at `y = 0` and `y = h`: `u_t = G + nu u_yy`, `u(0,t) = u(h,t) = 0`,
`u(y,0) = 0`. The steady solution is `u_s = G y (h - y) / (2 nu)`. The difference `v = u - u_s` satisfies the homogeneous
heat equation with `v(y,0) = -u_s(y)`, so `v = sum_n b_n sin(n pi y / h) exp(-n^2 pi^2 nu t / h^2)` with
`b_n = -(2/h) integral_0^h u_s sin(n pi y / h) dy = -(G / (nu h)) integral_0^h y (h - y) sin(n pi y / h) dy`. Two
integrations by parts give `integral_0^h y (h - y) sin(n pi y / h) dy = 2 h^3 (1 - cos n pi) / (n pi)^3`, which is
`4 h^3 / (n pi)^3` for odd `n` and zero for even `n`. Hence

`u(y,t) = G y (h - y) / (2 nu) - (4 G h^2 / (nu pi^3)) sum_{n odd} sin(n pi y / h) exp(-n^2 pi^2 nu t / h^2) / n^3`.

The derivation was checked by substitution (each term solves the heat equation and vanishes at both plates) and
numerically. For `G = 8`, `nu = h = 1` with the 100 odd terms used by the test, the numerical check gave
`max |u(y_c, 0)| = 5.0e-7` over the 40 cell centres of the T2 mesh (the truncation of the series at `t = 0`) and
`u(h/2, 5) = 1.000000 = G h^2 / (8 nu)`.

**Uniform acceleration with a linear sink (check T1b).** The equation `du/dt = a - s u` has the solution
`u = a/s + (u_0 - a/s) exp(-s t)`, and the implicit Euler recursion is `u_{n+1} = (u_n + a dt) / (1 + s dt)`. Both are
exact algebra, no data.

**Hartmann limits (T3).** The short-circuit profile and flow rate of `rcfd_fv::lorentz::hartmann` are reused from
wave 23A (docs/reference-data.md, F-214 section).

## Wave 26C post-object catalogue and EnSight export (F-238, F-103, ADR 0048)

No literature value is used. Every expected number is a definition or a closed form derived here.

**Verified by derivation (first principles).**

- Sphere of radius `r`: area `4 pi r^2`; outward flux of `F = x - c` is `r * 4 pi r^2 = 4 pi r^3` (divergence theorem,
  `3 V`); a linear field `z - c_z` on the sphere has mean 0 and variance `r^2 / 3` (equal-area slices, `z - c_z` is
  uniform on `[-r, r]` by Archimedes' hat-box theorem).
- Ellipsoid: flux of `x - c` is `3 V = 4 pi a b c`.
- Unit cube: surface area 6; outward flux of `(x,y,z)` is `3`; area-weighted mean of `x` over the six faces is
  `(0 + 1 + 4 * 1/2)/6 = 1/2`; mean of `x^2` is `(0 + 1 + 4/3)/6 = 7/18`, so the variance is `7/18 - 1/4 = 5/36` and the
  standard deviation `sqrt(5)/6`.
- Plane sections of the unit cube: `x = const` area 1; `x + y = 1` area `sqrt 2`; `x + y + z = 3/2` the regular hexagon
  of side `sqrt 2 / 2`, area `3 sqrt 3 / 4`.
- Cylinder: lateral area `2 pi r h`.
- Circular cone: lateral area `pi R l` with `R = h tan a`, `l = h / cos a`.
- Weighted variance of `1e9 + {0..6}` is 4 (variance of the integers 0..6 is `(49 - 1)/12`).
- Momentum flux of a uniform velocity `u` through a patch of area vector `S` is `(u . S) u`.

**Unverified / not read.** Layouts of the EnSight Gold files were taken from the vendor's public user-manual page (case
file sections, geometry and variable file layouts, `nfaced`, 80-character binary records, wildcard expansion). The page
text read did not state the `hexa8` node numbering (figure), the `nfaced` face orientation or the binary integer / float
widths. The hexahedron convention (bottom face counter-clockwise seen from the top), outward `nfaced` faces and 4-byte
little-endian integers and floats are from memory. HDF5 and CGNS layouts were not read or used.

## Wave 26B sweep and Cartesian-core meshing (F-235, F-234, ADR 0047)

| Item | Status | Note |
|------|--------|------|
| Poiseuille pipe `u_c = G R^2/(4 nu)`, `U_b = G R^2/(8 nu)` | derived | from the axisymmetric momentum balance; no source read |
| Pappus volume of revolution, inscribed N-gon area `(N/2) sin(2 pi/N) R^2` | derived | first principles |
| Ghia et al. 1982 cavity centreline Re 100 | verified | two secondary copies (see wave 25B) |
| Ray-cast parity point-in-solid, 2:1 octree balance, hanging-node polyhedra | unverified | from memory of the general literature; not checked against a source |
| Layer-height rules (uniform, aspect ratio, last ratio) | derived | ADR 0047 |

## Wave 27B robustness and adaptive time stepping (F-209, F-235, ADR 0050)

| Item | Status | Note |
|------|--------|------|
| Poiseuille pipe `u_c = 2.5`, `U_b = 1.25` for `G = 1`, `nu = 0.1`, `R = 1` | derived | axisymmetric momentum balance, as wave 26B |
| Variable-step BDF2 coefficients `c0 = (1+2r)/(1+r)`, `c1 = 1+r`, `c2 = r^2/(1+r)` | derived | from exactness for `1, t, t^2`; tested to 1e-12; no source read |
| Step-doubling local error estimate, Courant RMS/MAX criteria, safety/growth/shrink factors | derived | first principles (implicit Euler local error `dt^2 y''/2`); no source read |
| Aitken delta-squared relaxation (omega_{k+1} = -omega_k (r_{k-1} . dr) / (dr . dr), dr = r_k - r_{k-1}) | unverified | recalled from the fixed-point acceleration literature; checked only on a linear test iteration |
| Taylor-Green decay `exp(-2 nu t)` | derived | analytic solution of the 2-D Navier-Stokes equations |
| SIMPLEC compensation `rAtU = V/(aP - sum a_nb)` | derived | first principles; the measured `alpha_u` threshold (diverged 0.9, converged 0.8) is a result of this work, not a reference value |

## Wave 27C viscoelasticity and structural export (F-135, F-237, ADR 0051)

| Item | Status | Note |
|------|--------|------|
| Oldroyd-B start-up `tau_xy = eta_p gdot (1 - e^{-t/lambda})`, N1 start-up | derived | from the constitutive ODE |
| Uniaxial extension `C_zz = 1/(1 - 2 Wi_e)`, linear growth at `Wi_e = 0.5` | derived | |
| Steady shear: PTT `Y^2 (Y-1) = 2 eps Wi^2`, FENE-P `L2 f^2 (f-1) = 2 Wi^2`, Giesekus implicit form in `f = -A_yy` | derived | checked against the ODE to 1e-12 |
| Giesekus closed form as recalled from the literature (chi form) | unverified, found wrong | 5.1e-3 relative error at Wi = 0.1; kept only as an ignored `known_miss` test |
| FENE-P normalisation `f = (L2-3)/(L2-tr C)`, tau = G(fC - I) | unverified | recalled |
| Log-conformation (matrix logarithm) transport, divided-difference form | derived | chain rule of the matrix logarithm |
| Poiseuille start-up modal series (Oldroyd-B) | derived | cross-checked against an independent 1-D PDE solve in the test |
| PLOAD4, TEMP field layouts | verified | pyNastran documentation pages |
| GRID, element node orders, CHEXA continuation, free-field syntax, PLOAD4 sign | unverified | recalled |
| Third-party solver acceptance of the deck | not tested | no solver available |

## Wave 27A volume of fluid and surface tension (F-055, ADR 0049)

| Item | Status | Note |
|------|--------|------|
| Martin and Moyce dam-break non-dimensional average front velocity `v / sqrt(g H)` = 1.48 (H = 57 mm), 1.69 (H = 114 mm), averaging window t* > 1 | verified, secondary | Table 1 of a 2014 journal paper (arXiv:1308.0115), page read; its extraction method from the original is not stated; the primary Martin-Moyce 1952 tables were not obtained and the original's time normalisation was not resolved, so no pointwise front table is used |
| Ritter front velocity `2 sqrt(g H)` (upper bound) | derived | frictionless dam-break characteristic solution |
| Laplace jump `dp = sigma / R` (2-D, cylinder) | derived | |
| Zalesak slotted-disk set-up (disk R = 15 at (50,75), slot width 5, 628 steps) | unverified | recalled from the literature; the test uses the registered numbers, not a published table |
| Zalesak flux-corrected-transport limiter (1979), Boris-Book FCT (1973) | unverified | implemented from memory of the published algorithm and validated by boundedness and exact-conservation tests |
| Interface-compression flux (Rusche 2002 thesis, Weller 2008 report form) | unverified | structure recalled, not read |
| CSF surface tension (Brackbill, Kothe, Zemach 1992) | unverified | the method is standard; the formula is validated by the Laplace-jump test |
| Capillary time-step constant `dt_cap = sqrt((rho_1 + rho_2) dx^3 / (8 pi sigma))` | unverified | from memory; the most restrictive reading is used |
| LeVeque (1996) single-vortex deformation field | unverified | recalled; used with the registered period T = 2 |
| Hysing et al. (2009) rising bubble case 1: `y_c(3)` = 1.0813, `v_max` = 0.2417 at t = 0.9213 | verified (wave 39E, ADR 0096: Int. J. Numer. Meth. Fluids 60 (2009) 1259-1288, Table V, TP2D 1/h = 320; Table XI MooNMD 0.2417 / 1.0817; the earlier time 0.9317 was a misrecall) | originally quoted from memory; the registered limits are loose (0.12 in height, 25 percent in velocity) because the 40 x 80 algebraic scheme is far from the references |

## Wave 28C adaptation and turbomachinery post-processing (F-102, F-123, ADR 0054)

No source was read in this wave. Everything is first-principles or recalled.

| Item | Status | Note |
|------|--------|------|
| Linear reconstruction exact for linear fields; volume-weighted restriction conserves the integral | derived | tested to 6.7e-16 |
| Hanging-node face loops with all lattice vertices on the boundary edges (closed, two-cell faces) | derived | checked by closedness and face-count tests |
| Zienkiewicz-Zhu error indicator (patch-recovered gradient minus cell gradient) | unverified | the recovery is the volume-weighted patch mean, a simplified form; the original SPR/ZZ papers were not read |
| Gradient and Hessian indicators (least squares) | derived | |
| Polyhedral dual of a tet mesh (centroid rings, edge midpoints) | derived | volume and patch areas exact by construction, tested to 1e-15 |
| Kuhn six-tet split of the cube used for the dual test | derived | |
| Manufactured tanh-front diffusion solution | derived | |
| Pump head `dp0/(rho g)`, hydraulic efficiency `rho g H Q/(T omega)` | derived | definitions as used here; conventions of a specific standard not checked |
| Compressor `eta_is = (PR^k-1)/(tau-1)`, `eta_p = k ln PR/ln tau`, `k = (gamma-1)/gamma` | derived | perfect gas, adiabatic; recalled to agree with textbook forms, no book consulted |
| Head coefficient `gH/u_t^2`, flow coefficient `Q/(u_t A)` | unverified | normalisations vary between sources; these are the ones coded |
| Mass-flow and area-weighted averaging formulas | derived | |
| Published pump or compressor performance data (rotor 37, NASA, etc.) | not used | no comparison made; roadmap literature acceptance is not met |

## Wave 28A VOF wiring: AMG, mass transfer, wave inlet and beach (F-055, F-226, F-227, ADR 0052)

No source was read in this wave. Everything below is first-principles or recalled. The earlier entries for the same
references (waves 23B, 25B) keep their status.

| Item | Status | Note |
|------|--------|------|
| Smoothed-aggregation AMG (Vanek, Mandel, Brezina 1996; Stuben 2001 survey): strength filter, weak couplings lumped to the diagonal, damped-Jacobi smoothed prolongator | unverified | structure recalled, not read; the defect found (zero filtered diagonal) is proven by a reproducer in `rcfd-linalg/tests/amg_contrast.rs`, and the fix is validated by convergence, not by a citation |
| Variable-density pressure equation `div(gamma grad p) = div(phiHbyA)` with `gamma = V/a_p`, 1000:1 jump | derived | |
| Lee model `mdot = c alpha rho (T - T_sat)/T_sat`, its exponential-integrator form for a frozen temperature | derived / unverified | the closed form `alpha(t) = alpha_0 exp(-c theta t)` is derived and checked to 1e-12; the attribution and the model form remain as listed in wave 25B (still UNVERIFIED) |
| Volume source `mdot (1/rho_v - 1/rho_l)` and the mass balance `rho_l S_alpha = -mdot` | derived | continuity residual checked to 1e-6 |
| Hertz-Knudsen flux, Schrage factor, Clausius-Clapeyron `p_sat(T)` with constant `h_fg` | derived / unverified | one-way flux derived from kinetic theory (checked 1e-9 in the M tests); the Schrage factor and the Clausius-Clapeyron constant-`h_fg` form are recalled, not read |
| Airy / second-order Stokes inlet values, ramp, exact liquid-fraction helper | as in wave 23B | the library formulas are those of `rcfd-fv/src/waves.rs`; the third-order residual scaling was checked there |
| Relaxation zone `w computed + (1 - w) target` with the exponential weights (Jacobsen, Fuhrman, Fredsoe 2012) | unverified | the form is the one of wave 23B; its attribution is DOUBTFUL (wave 25B); the PDF was not read |
| Goda and Suzuki (1976) two-gauge incident / reflected separation | unverified | the 2 x 2 complex system is DERIVED here (`B_i = A_I e^{ikx_i} + A_R e^{-ikx_i}`) and solved directly; the paper was not read; the gauge spacing is 0.25 wavelength, away from the singularities at multiples of half a wavelength |
| Damping estimates: implicit Euler `omega^2 dt/2`, first-order upwind `2 (|u| dx/2) k^2` | derived | used in the ADR to state the wave-height tolerances before the runs |
| Wave-tank results (heights, periods, reflection) | measured | numerical results of this code on a coarse mesh (lambda/32, 10 and 20 periods), no comparison with published tank data |

## Wave 28B chemistry acceleration and combustion extensions (F-223, F-129, ADR 0053)

No source was read in this wave. Every reference below is recalled from memory and is unverified.

| Item | Status | Note |
|------|--------|------|
| Pope (1997), in-situ adaptive tabulation: ellipsoid of accuracy, retrieve / grow / add | unverified | the algorithm as coded is DERIVED from the recalled outline (Sherman-Morrison growth, linear error test at the query); the paper was not read |
| Lu and Law (2005), directed relation graph reduction | unverified | interaction coefficients and graph search coded from the recalled outline; no published reduced-mechanism size is claimed |
| Clustering of similar cells with increment transfer | derived | leader clustering in the scaled max-norm; no source |
| Mixture fraction (Bilger element definition), unity-Lewis linear enthalpy mixing | unverified | derived here from element conservation |
| Beta PDF of the mixture fraction, moments and parameters | derived | quadrature of the table is exact; the incomplete beta by continued fraction (modified Lentz), lnGamma by Stirling plus recurrence, tested to 4e-15 |
| Burke and Schumann (1928) infinitely fast chemistry | unverified | the closed form is derived for a one-step toy mechanism with constant cp (Z_st 3/19, T_st 2194.7 K) |
| Peters (1984) steady flamelet equations, chi(Z) from erfc^-1 | unverified | equations derived with unity Lewis number; the scalar-dissipation profile is the recalled form |
| GRI CH4/air maximum equilibrium temperature about 2225 K | unverified | from memory, reported by test B7 only; not a check |
| Fisher / KPP (1937) front speed 2 sqrt(Gamma k) for the Zeldovich cubic | unverified | the closed form m = sqrt(Gamma k/2) of the cubic was derived; the paper was not read |
| Sandia flames D to F, any published flame | not used | no comparison attempted |
| Published ignition-delay measurements | not used | the 5 percent point is met against the same mechanism integrated directly only |

## Wave 29D distributed-memory layer (F-100, F-016, ADR 0058)

No source was read in this wave. Every reference below is recalled from memory and is unverified. No MPI library, no
vendor code or binary was used or installed.

| Item | Status | Note |
|------|--------|------|
| Saad, "Iterative Methods for Sparse Linear Systems": preconditioned CG and BiCGStab, block-Jacobi and additive-Schwarz preconditioning | unverified | the algorithms are the ones already in `rcfd-linalg`, copied with global inner products; the book was not read in this wave |
| Higham, "Accuracy and Stability of Numerical Algorithms": `gamma_k = k u / (1 - k u)` bounds for dot products and matrix-vector products | unverified | the bound used in B4/B5 is DERIVED from the standard model of floating-point arithmetic; the text was not read |
| Condition number of the 2-D Dirichlet five-point Laplacian, `kappa = cot^2(pi / (2 (n+1)))` | derived | from the known eigenvalues `4 - 2 cos` sums; used to state the solution-difference limit 2e-7 (kappa 973 for 48 x 48) |
| Rank-ordered sequential reduction (deterministic, O(P) messages) | derived | a design choice, not taken from an MPI implementation; MPI collectives do not guarantee a fixed order, which is why the order is fixed here |
| Green-Gauss gradient with linear face interpolation, upwind convection with deferred linear-upwind correction | unverified | standard finite-volume practice recalled from memory; verified here only by exactness on linear fields and by distributed-versus-serial agreement |
| Strong-scaling timings of in-process threads | measured | one 16-logical-core host, shared memory bandwidth and rayon pool; NOT a cluster or network measurement and not compared with any published scaling |

## Wave 29C Lagrangian extensions (F-128, F-056, ADR 0057)

No source was read in this wave. Every reference below is recalled from memory and is unverified unless marked derived.
No vendor binary, install or text was used.

| Item | Status | Note |
|------|--------|------|
| Gosman and Ioannides (1983) eddy-interaction (discrete random walk) model | unverified | the structure (Gaussian velocity fluctuation held for min(eddy lifetime, crossing time)) is recalled; the paper was not read |
| Constants `C_L = 0.15` (integral time `T_L = C_L k/eps`) and `C_mu = 0.09` (eddy length `C_mu^(3/4) k^(3/2)/eps`) | unverified | values recalled from common practice; `C_L` is a case key (`dispersion_cl`), not a validated constant |
| Variance of a Stokes particle in a fluid velocity with autocorrelation `sigma^2 exp(-s/T_L)`: `sigma_p^2 = sigma^2 T_L/(T_L + tau_p)` | derived | double integral `int int exp(-(a+b)/tau) exp(-abs(a-b)/T) = tau^2 T/(T+tau)` worked out in the ADR; consistent with the Tchen/Hinze ratio `1/(1 + tau/T_L)` recalled from memory (not read) |
| Exponential eddy lifetimes with independent Gaussian redraws give autocorrelation exactly `exp(-s/T_L)` | derived | probability that no redraw occurred in the lag |
| Eddy crossing time `-tau ln(1 - L_e/(tau abs(u - v)))` | derived | from Stokes relative motion `tau w (1 - exp(-t/tau))` set equal to `L_e` (the recalled published form is the same) |
| Ranz and Marshall (1952) `Nu = 2 + 0.6 Re^(1/2) Pr^(1/3)` | unverified | coefficients recalled from memory (secondary sources earlier in this file for related correlations); lumped-sphere relaxation time is derived |
| Rosin-Rammler mass CDF `1 - exp(-(d/d_bar)^n)` | verified in two secondary sources in wave 18C (ADR 0028) | sampling by inversion and the equal-mass parcel weighting are derived |
| Kolmogorov-Smirnov critical value 1.95/sqrt(N) (99.9 percent) | unverified | recalled from memory; used only to set a statistical bound, the constant 1.949 is the usual two-sided tail value |
| Momentum and total-energy exchange definitions (drag impulse, `-(Delta KE - W_g + m c_p Delta T)`) | derived | conservative by construction; no source |
| Particle-laden jet, spray penetration, falling-film thickness (Nusselt), any published experiment | not used | no comparison attempted |

## Wave 29A rotor interfaces and guarded secant (F-126, F-200, ADR 0055)

No source was read in this wave. Every reference below is recalled from memory and is unverified. No vendor code, binary
or install was used. No published turbomachine (NASA Rotor 37, Hannover compressor, any pump or fan) was run or compared:
NOT ATTEMPTED.

| Item | Status | Note |
|------|--------|------|
| Frame velocity `Omega x r`, relative/absolute velocity `w = u - Omega x r` | derived | elementary kinematics; the transformation checks are exact vector identities (R1) |
| Frozen-rotor (fixed relative position) interface as the steady limit of a rotor-stator interface | unverified | the usual textbook description recalled from memory; the solver form is the absolute-velocity MRF of ADR 0008 |
| Mixing-plane / stage interface by circumferential averaging in radial bands, conserving mass, momentum and energy flux | unverified | the idea is recalled from the turbomachinery CFD literature (for example Denton's work on mixing); the formulas used here (u_n = Q/A, u_theta = Gamma/r, mixed-out pressure from normal momentum, flux-weighted scalar) are DERIVED in ADR 0055 D3 and checked against closed forms, not taken from a paper |
| Euler turbine equation (torque from the change of angular momentum flux) | unverified | recalled; used only to justify conserving angular momentum flux |
| Mixing loss `(A/2)(U sigma^2 + m3) >= 0` for uniform pressure and non-negative normal velocity | derived | from `<d^2 (d + U)> >= 0`; the two-jet closed form `(A/2) a(1-a)(u1-u2)^2 (U + (1-2a)(u1-u2))` was checked (3.3e-15) |
| 3-point Gauss-Legendre nodes and weights (exact to degree 5) | unverified | standard values recalled from memory; verified indirectly by the exact band integrals of X1/X4 (1e-15) |
| Free-vortex swirl `u_theta = Gamma_0/r` and solid-body swirl `Omega r` | derived | closed-form angular momentum fluxes integrated here |
| Taylor-Couette profile `A r + B/r` | derived | as in ADR 0008 |
| Guarded secant rule thresholds (`|dm| <= 0.5 |e|`, slope within 0.1 to 10 of the dynamic-head slope) | derived | chosen from the diagnosed cause of the plain-secant miss (spin-up contaminated slope), not tuned to the result; one case (F1) exercises it |

## Wave 29B mixture and cavitation (F-127, ADR 0056)

No source was read in this wave. Every reference below is recalled from memory and is UNVERIFIED. No vendor code, binary
or install was used. No published cavitation data (cavitating venturi, hydrofoil sheet length, nozzle) and no
bubble-column void-fraction profile was compared: NOT ATTEMPTED. Full multi-fluid Euler-Euler (per-phase momentum, lift,
virtual mass, turbulent dispersion) is NOT delivered.

| Item | Status | Note |
|------|--------|------|
| Mixture (algebraic-slip) model: slip flux `alpha (1-alpha) v_12`, drift mass flux and drift stress in the mixture momentum | unverified | the model class (Ishii drift flux, Manninen et al. 1996 mixture model) is recalled from memory; the equations used are DERIVED in ADR 0056 from the volume-averaged mixture balances and checked against analytic divergences (M3 order 1.99) |
| Manninen 1996 relative velocity with Stokes factor `f = 1`; Schiller-Naumann `f = 1 + 0.15 Re^0.687` (1935) | unverified | the closure form and the 0.15 and 0.687 constants are recalled from memory; Stokes terminal velocity itself is derived (D2 1.5e-16, D3 balance 6.4e-16) |
| Kynch batch-settling front (kinematic wave) | unverified | recalled (Kynch 1952); the front speed used in D4/D5 is the DERIVED flux `alpha v` characteristic; D4_corrected front 0.234 dx, D5_corrected 0.432 dx |
| Schnerr-Sauer cavitation model, `alpha_v = x/(1+x)`, `x = n (4/3) pi R^3`, `mdot = rho_v rho_l/rho_m alpha(1-alpha)(3/R) R'` | unverified | the model is recalled from Schnerr and Sauer (2001); the algebra is checked here as an identity `D alpha_v/Dt = mdot rho_m/(rho_v rho_l)` (C1 exact); the bubble-number density `n = 1e11` and nucleus radius `1e-5 m` are set by the case, not taken from a source |
| Rayleigh bubble growth `R' = sqrt(2 (p_sat - p)/(3 rho_l))` (Rayleigh 1917; Plesset 1949 adds viscosity and surface tension) | unverified | recalled; the inertial limit is derived from the energy balance of the liquid shell; the Rayleigh-Plesset integral comparison C4c (first integral 3.95e-10; `R_SS - R_RP` within `[0.56, 0.57] R_0`) is against the equation without viscosity or surface tension |
| Saturation pressure of water, `p_sat(293 K) = 2339 Pa` | unverified | recalled from memory (steam-table value); used as a case input |
| Bernoulli pressure drop in the venturi throat, `p = p_in - 0.5 rho (u_t^2 - u_in^2)` | derived | N1: throat pressure 99.3 Pa against Bernoulli-from-inlet 488.9 Pa (limit 1250 Pa); minimum throat-region pressure -1349.9 Pa < `p_sat`, so inception is expected |
| Mixture mass and volume conservation | derived | D1_corrected volume 1.7e-15, mass 2.4e-16; M4b per-cell mass balance 7.9e-15 |
| Cavitating venturi (2-D, 80 x 20): bounded vapour fraction | derived | N2: `alpha_v` in `[4.19e-4, 0.99993]` over 6000 steps; periodic cavity growth and collapse with a water-hammer-like pulse (ADR 0056 addendum) |
| Cavitating venturi N3 end of run, N4, N4', N5 | MISS | known-miss tests with the registered limits intact: end-of-run phase (cavity collapsed), advection into `p >= p_sat` cells (31763 events), end-of-step source sign (16 events), global balance 1.96e-5 against 1e-6 |
| Void-fraction profile of a bubble column, sheet-cavity length of a hydrofoil | not used | no comparison attempted |

## Wave 30A two-way particle coupling and mixing plane in the solver loops (F-128, F-126, ADR 0059)

No source was read in this wave and no web page was fetched. Every reference below is derived from first principles or
recalled from memory (unverified). No vendor code, binary or install was used. No published particle-laden jet, no
spray, no published turbomachine (Rotor 37, Hannover compressor, any pump or fan) was run or compared: NOT ATTEMPTED.

| Item | Status | Note |
|------|--------|------|
| Momentum given to the carrier is minus the momentum the parcels receive from it; the sum is conserved | derived | Newton's third law on the drag impulse; the record is conservative to round-off (J2 5.7e-16) |
| Steady plug: force on the carrier per unit area balances the pressure drop, `F = mdot_p (v_L - v0)`, with `v = U - (U - v0) e^{-t/tau}`, `x = x0 + U t - (U - v0) tau (1 - e^{-t/tau})` | derived | uniform stream cannot change by continuity; PLUG-B measured F 1.8e-5 and pressure drop 7.6e-4 off it (limits 1e-6 miss, 1e-3 met) |
| Free-vortex plus uniform axial flow `u = W z + (Gamma/r) theta`, `p = p0 - Gamma^2/(2 r^2)` as an exact steady Navier-Stokes solution | derived | curl u = 0 so the viscous term vanishes; convection balances the radial pressure gradient; the MRF terms cancel for any axisymmetric field, so the field is exact for any rotor speed |
| Mass flux and angular momentum flux conservation across the plane per band | derived | `u_n = Q/A'` and `u_theta = Gamma/r` by construction (identity checks, 1e-14), so MP2 and MP3 test the wiring, not the physics |
| Mixing loss non-negative for uniform pressure and non-negative normal velocity | derived | as ADR 0055; measured 11.8 percent of the energy flux for a 30 percent cosine axial distortion |
| Ranz and Marshall (1952) correlation, Rosin-Rammler distribution, mixing-plane literature (Denton) | unverified | as in the wave 29A and 29C sections; not re-read |
| Pressure error 6.85e-2 P at the hub wall against the exact centripetal gradient | measured | the plane-free control gives 1.13e-1, 6.62e-2, 4.24e-2 P for 4, 8, 16 radial layers (roughly first order); attributed, as an unisolated order-of-magnitude estimate, to the zero-gradient wall pressure; not compared with any other code |

## Wave 30B: incompressible validation data actually read (ADR 0060, retrieved 2026-10-17)

Wave 30B read every row below as raw text (`curl` plus `pdftotext -layout`, or the raw data file). The numbers are stored
in `validation-data/reference/` and registered in `validation-data/manifest.json`. Nothing here is typed from memory.

The measured results of wave 30B against the registered limits are in the addendum of ADR 0060. Known misses are the
ignored `*_known_miss` tests in `crates/rcfd-api/tests/validation_*.rs`.

| Value | Source and what was read | Result |
|---|---|---|
| Cavity Re 1000, u along x = 0.5, 15 rows, Ghia next to Botella and Peyret spectral values (largest difference 0.0063) | arXiv:1807.04896 Table A-1 ("Ghia, Table 1, p. 398" and "Botella, Table 9, p. 426") | verified (secondary copy of both); used in `validation_cavity_ghia.rs` |
| Cavity Re 100 and 400 u, 16 rows each | raw CSV in github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3, equal to the in-repo table to the last digit | verified (secondary, student repository); the Ghia 1982 original, and Ghia v at Re 400 and 1000, were NOT obtained |
| Backward step ER 2, Re_D = 800: ten published x_r/h values 11.48 to 12.20 | arXiv:2507.16509 Table 2 re-read in full (Rogers and Kwak, Barton, Erturk, Kim and Moin, Lee and Mateescu, Guj and Stella, Grigoriev and Dargush, Keskar and Lyn, Gartling, Gresho et al.) | verified as a quotation in a 2025 paper; the originals including Gartling (1990) and Erturk (2008) were NOT read. Armaly et al. (1983): data not obtained (host timed out, Cambridge paywalled), no check |
| Blasius f''(0) = 0.33205733621519629893718, f'(eta) at 1..5, delta* = 1.72078765752 sqrt(nu x/U) | arXiv:2204.02215 (Lal and Martin 2022) Table 1 and benchmark tables, 32-digit Taylor series | verified; Cf = 0.664115/sqrt(Re_x) derived from f''(0) |
| Channel DNS Re_tau = 178.12: U_c+ 18.301, U_b+ 15.6787, U+ at y+ = 5, 10, 20, 30, 50, 100, 150 | `chan180.means` of Moser, Kim and Mansour at torroja.dmt.upm.es/turbdata/agard/chapter5/PCH10/chan180/profiles/ (65 points, read and integrated); only the sampled numbers are stored (redistribution terms not stated) | verified (file read) |
| Cylinder Re 100: C_D 1.314 to 1.370, St 0.164 to 0.167, experiment St 0.165 | arXiv:2303.09262 Table 1 (Newtonian rows) and arXiv:1401.6513 Table 3 | verified as quotations; the original papers were NOT read |
| Moody, Colebrook, Dean friction correlations; Gartling and Ghia originals; Armaly | not obtained | still unverified or data not obtained; no wave 30B check uses them |

## Wave 31C: reference reading (ADR 0064, retrieved 2026-10-18)

The discrepancies between a repo or source value and the text read are listed after the table; nothing was silently
changed. Wave 31C downloaded public pages and PDFs as raw bytes (`curl`) and converted them with `pdftotext -layout`,
or read them through the page fetcher. The fetcher's output is LLM-condensed, so potentially lossy; the numbers used below
were checked against raw text wherever the raw file was available. Nothing was filled from memory. Small tables are stored
as `validation-data/reference/w31c_*.csv` with manifest entries. The checks are in
`crates/rcfd-api/tests/validation_refs_w31c.rs`.

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

### Discrepancies between a repo or source value and the text read (wave 31C; nothing was silently changed)

1. Ghia v, Re 400, x = 0.5: 0.05186 (gist, student repository) against 0.05188 (Uppsala thesis Table 1). Stored 0.05186
   (two of three). No rcfd test uses it.
2. Ghia v, Re 1000, x = 0.9063: -0.5155 (student repository; thesis -0.51550) against -0.51500 (gist). Stored -0.5155. No
   rcfd test uses it.
3. Ghia v, Re 400, x = 0.9063 = -0.23827 is a misprint of the original table (noted by the gist and the thesis), kept as
   printed. No rcfd test uses the Re 400 v table.
4. Gartling (1990) Re 800: 12.20 / 9.70 / 20.96 step heights (MFIX-Exa quoting Gartling; arXiv:2507.16509 for 12.20)
   against 11.9 / 9.5 / 20.6 (AMCA 2013 row labelled Gartling). The in-repo 12.20 agrees with two of three secondary
   sources. The original was not read.
5. St at Re 100: the in-repo CSV row "Williamson (experiment) 0.165" (1996 review via arXiv:2303.09262) against the 1989
   fit 0.164335 (0.4 % apart). Both lie inside the 1 % window of the cylinder test.
6. Dean table row Re_tau 100 prints U_b/u_tau 14.4 against 14.13 from the paper's own Eq. (A1).
7. `validation_channel_re180.rs` takes the DNS Cf = 0.008136 (U_b+ 15.6787 at Re_tau 178.12). Dean's relation with
   Re_b = 15.6787 x 178.12 = 2792.7 gives 0.008446, 3.8 % higher. The source itself states the correlation is inaccurate
   at low Re_tau, so the repo's choice to use the DNS value instead is consistent with it.

## Wave 31B: turbulence wall diagnosis (ADR 0063)

| Item | What was read (source) | Result |
|---|---|---|
| SST model definition and wall conditions | https://tmbwg.github.io/turbmodels/sst.html (NASA TMR), fetched 2026-10-18 | read: equations, F1, F2, cross-diffusion floor, production limiter, nu_t with S and F2, constants, k_w = 0 and omega_w = 10 * 6 nu / (beta1 d1^2). Compared term by term with `crates/rcfd-turb/src/sst.rs`: no discrepancy found in the published terms. Menter (1994) and Menter, Kuntz, Langtry (2003) were NOT read (paywalled; only the TMR transcription is claimed) |
| SST flat-plate reference | https://tmbwg.github.io/turbmodels/flatplate_sst.html (NASA TMR 2DZP) | read: CFL3D SST Cf(x = 0.97) = 0.00268299 on 273 x 193 (GCI 0.28 %, p = 1.21), plate x in [0, 2] behind a symmetry section. rcfd's Richardson-extrapolated Cf(0.97) is +2.6 ... +3.6 % above it; not resolved (virtual origin untested) |
| Standard wall function (k-epsilon) | a secondary description, not a published source (recorded in [provenance](provenance.md)); two neighbouring pages fetched, scalable-function text seen only through the fetch summary | UNVERIFIED against a published source, replace before relying on it. Read: y* definition, log law for y* > 11.225 and U* = y* below, k and epsilon of the wall-adjacent cell from equilibrium forms, zero normal gradient of k. rcfd differs for y* < y*_lam (G = 0, eps = 2 nu k/y^2); an opt-in test with the equilibrium forms removed the bump but not the fall and was removed (ADR 0063 Addendum 2) |
| Launder and Spalding (1974), Grotjans and Menter (1998) | not read | secondary descriptions only (search results, the description above); no claim about the original formulations |

## Wave 31A: solver defects found by the wave 30 validation (ADR 0062)

No new external source was read in this wave. All limits were taken unchanged from ADR 0060 and ADR 0061. The reference
values used are the ones already in the repository from waves 30B and 30C:

- de Vahl Davis 1983 Table V, from the page scan.
- The cylinder literature rows of `validation-data/reference/cylinder_re100_literature.csv`, St 0.164 to 0.167 and
  C_D 1.314 to 1.370.
- The Sod exact values of Marzouk.

Results of this wave computed by rcfd (not literature, not validation data):

- The Ra 1e6 cavity diagnosis: converged 32 x 32 value v_max 220.48, Nu 8.873 at alpha_u 0.5.
- The cylinder time-step and mesh study: St extrapolated to zero time step and mesh 0.1669, C_D 1.357 by a fitted model
  with first-order time and second-order space terms, versus the unchanged literature windows.

## Wave 32B: aeroacoustics for fan noise (ADR 0066, docs/acoustics.md)

No measured fan or airfoil noise data were read or used. The broadband proxies (Proudman, Curle, trailing edge) are
NOT validated. Validation would need measured or scale-resolving wall-pressure and far-field data for a known operating
point, calibration of the constants on one data set and a held-out comparison on another.

| Item | What was read (source) | Result |
|---|---|---|
| FW-H formulation 1A (Farassat, Brentner) | not read in this wave; formulas as in ADR 0013 (from memory of the papers) | UNVERIFIED as transcription; the implementation is verified only against closed-form fields (pulsating sphere, compact dipole, rotor point force, moving observer in flow) |
| Gutin rotor harmonics | Gutin (1936) and Lowson (1965) NOT read | the closed form was derived in ADR 0066 from the compact-dipole far field and checked against the FW-H solver (<= 4e-5); no claim to match the published form |
| Curle (1955), Proudman (1952) | NOT read | surface-dipole and isotropic-turbulence scalings re-derived or taken as scalings (U^6 dipole power, U^8 quadrupole) |
| Proudman constant alpha = 0.1 and M_t = sqrt(2k)/c0 | secondary search results only (see [provenance](provenance.md)); the Sarkar and Hussaini report (a scanned PDF) could not be read | UNVERIFIED, secondary; UNVERIFIED against a published source, replace before relying on it; default only, not calibrated |
| Ffowcs Williams and Hall (1970) trailing-edge U^5 law | a review snippet; the paper was not read | only the exponent is used; the constant K_TE has no value from a source |
| A weighting | IEC 61672-1 NOT read; formula constants (20.6, 107.7, 737.9, 12194 Hz) from Wikipedia and metricgate.com | formula implemented; the tabulated A-weights matched the formula to 0.049 dB at exact base-10 frequencies when compared from memory of the usual table (table itself unverified) |
| 1/3-octave band definition | IEC 61260-1 NOT read | base-10 bands (ratio 10^(1/10)), as commonly stated; nominal labels from the usual rounded list |
| Reference pressure 20 uPa | common standard value | used as the SPL reference |

## Wave 32C: design-loop infrastructure (ADR 0067, docs/design-loop.md)

No measured data and no vendor material were read or used in this wave.

| Item | What was read (source) | Result |
|---|---|---|
| Nelder-Mead simplex | Nelder and Mead (1965) NOT read; standard coefficients 1, 2, 0.5, 0.5 and the 5 % initial simplex from general knowledge | implemented and checked on Rosenbrock (20 of 20 seeds, f < 1e-18); no claim about the original text |
| Differential evolution | Storn and Price (1997) NOT read; DE/rand/1/bin with F = 0.5, CR = 0.9 from general knowledge | checked on Rastrigin d = 4 (20 of 20 seeds, f = 0) |
| Deb's feasibility rules and fast non-dominated sorting | Deb (2000) and Deb et al. (2002) NOT read; rules as commonly stated | checked on analytic fronts (ZDT1-like, quarter circle, plane) |
| Latin hypercube | McKay, Beckman, Conover (1979) NOT read | stratification property tested exactly |
| Sobol sequence, Joe-Kuo direction numbers | Sobol (1967), Antonov and Saleev (1979), Joe and Kuo (2008) NOT read; the direction numbers for dimensions 2 to 10 were recalled from memory | UNVERIFIED as a transcription; verified only by the net properties (first 8 points of dimensions 1 and 2 exact, power-of-two stratification in all 10 dimensions, (0, m, 2)-net of the first two dimensions, L2-star discrepancy 0.0051 against a random mean of 0.0131) |
| Warnock's L2-star discrepancy formula | from memory | used only as a relative comparison between Sobol and random points |
| xoshiro256** and SplitMix64 | Blackman and Vigna generators recalled from memory | used for seeded reproducibility only; no statistical quality claim beyond the stratification and determinism tests |
| Contraction channel (E1) | no reference data; the 40 % claim of the ADR is an estimate from a lubrication argument | computed by rcfd: pressure drop 9.94 to 5.07 (49 %) with the outlet velocity ratio at the constraint 1.3 |

## Wave 32A: fan geometry and sector mesh (ADR 0065)

Nothing in this wave is a validation against measured data.

| Item | What was read (source) | Result |
|---|---|---|
| NACA 4-digit thickness and camber formulas | the Wikipedia page "NACA airfoil" fetched 2026-10-18, which cites Moran, "Fundamentals of Aerodynamics" (2003) p. 7; the originals (Jacobs, Ward, Pinkerton, NACA TR-460, 1933) NOT read | coefficients 0.2969, -0.1260, -0.3516, 0.2843 and the closed-trailing-edge -0.1036 as on that page; the code uses -0.1036; the NACA 00xx closed-form area matches the quadrature to 1e-15 |
| Double circular arc section | constructed here from elementary geometry; no source | formulas stated in docs/meshing.md and checked by quadrature |
| Flat-plate friction `Cf = 0.0576 Re^(-1/5)` for the first-cell estimate | the power-law form used earlier in this repository (wave 7); not re-read | used only to size the first cell; the y+ of a converged solution was not measured |
| Published fan or blade data | none read | no comparison with a real fan was made; geometry and mesh are checked only against analytic formulas and conservation identities |

## F-253 particle wall film (ADR 0192)

Nothing in F-253 is compared with measured data. The film model is derived; two published thresholds set its range.

| Item | Status | What was read (source) |
|---|---|---|
| Nusselt film profile `u = (rho g sin(theta)/mu)(h y - y^2/2)`, `u_bar = rho g sin(theta) h^2/(3 mu)`, `Gamma = rho^2 g sin(theta) h^3/(3 mu)` and the thickness `h = (3 mu Gamma/(rho^2 g sin(theta)))^(1/3)` | derived | momentum balance of a steady laminar film with no slip at the wall and no shear at the surface, worked out in ADR 0192 M1 |
| Laminar wave-free film for `Re = 4 Gamma/mu_l` below about 30 (wavy laminar to about 1800, turbulent above) | verified (secondary) | two university lecture notes on film condensation found by a web search on 2026-11-05 (the original texts they draw on not accessed) |
| Long-wave instability of a film on a plane inclined at `theta` above `R_c = (5/4) cot(theta)`, `R = rho h U_s/mu`, `U_s` the Nusselt surface speed (`(5/6) cot(theta)` for a Reynolds number on the mean velocity) | verified (secondary) | Thompson, Tseluiko and Papageorgiou, "Falling liquid films with blowing and suction", J. Fluid Mech. 2015, arXiv:1506.01579, read 2026-11-05; a second search result gives the mean-velocity form; Benjamin (1957) and Yih (1963) not accessed. In `Re_f = 4 Gamma/mu` the threshold is `(10/3) cot(theta)`, 5.774 at 30 degrees |
| Donor-cell transport, Courant limit on the kinematic wave speed `3 u_bar`, steady state exact below the feed | derived | ADR 0192 M5 to M7 |
| Falling-film thickness against a measured data set | not used | no comparison attempted |

## F-255 particle dispersion from turbulence-model fields and particle surface reactions (ADR 0208)

Nothing in F-255 is compared with measured data. No source was read for this work.

| Item | Status | What was read (source) |
|---|---|---|
| Mean-square displacement of a Stokes particle released at rest in a fluid velocity of autocorrelation `sigma^2 exp(-abs(s)/T)`: `sigma^2 (I0 - 2 I1 + I2)` (ADR 0208 D3), and its `tau -> 0` limit `2 sigma^2 T^2 (t/T - 1 + exp(-t/T))` | derived | from `X = int u (1 - exp(-(t - a)/tau)) da` and `<X^2> = int int g g R`; checked against the moment equations of the jump process (CF1, 7.6e-12); the limit is the form of Taylor's result for an exponential autocorrelation (Taylor 1921, "Diffusion by continuous movements", not read) |
| Long-time diffusivity `sigma^2 T` independent of `tau` | derived | the limit of the same closed form; consistent with the Tchen/Hinze result recalled in the wave 29C row (not read) |
| `epsilon = beta* k omega`, `beta* = 0.09` | standard; `beta*` verified (TMR, the k-omega SST row above) | the conversion `chemistry::mixing_vectors` already uses; not read in a source for this work |
| First-order kinetic surface reaction `omega = k_s rho_g Y_o / M_o` per unit surface, shrinking sphere at constant density `dd/dt = -2 nu_s M_s omega / rho_p` | registered here; the shrinkage derived | ADR 0208 D4 and Q7; no char or particle kinetics source read, no constants; the molar masses of the checks (0.012, 0.032, 0.044 kg/mol) are round test inputs, not data |
| Measured dispersion of particles in grid turbulence, char burnout data | not used | no comparison attempted |

## F-254 particle wall interaction (ADR 0194)

Nothing in F-254 is compared with measured data. The erosion correlation is read from its source; the rough-wall model is
taken from secondary sources, and its realizability rule and rebound-angle distribution are derived.

| Item | Status | What was read (source) |
|---|---|---|
| Erosion `E = m_p K U_p^n F(alpha)`, `F(alpha) = sum_{i=1..8} (-1)^(i+1) A_i (alpha pi/180)^i`, `A_1..A_8` = 9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170; `alpha` the angle between the particle path and the surface; depth rate `E/(rho_t A_t)` | read | DNV RP O501 "Erosive wear in piping systems", Revision 4.2-2007 (amended January 2011), eq. 7.1 to 7.3, Table 7-1, Figure 7-1, eq. 8.1, read 2026-11-06 from a third-party copy (wiki.pengtools.com); the binding dnv.com version and the later DNVGL-RP-O501 editions not read; Haugen et al. (1994), the source DNV cites for F, not read |
| `K`, `n` and density of Table 7-2: steel grades 2.0e-9 (m/s)^-n, 2.6, 7800 kg/m^3; titanium 2.0e-9, 2.6, 4500; GRP/epoxy 0.3e-9, 3.6, 1800; GRP/vinyl ester 0.6e-9, 3.6, 1800 | read | the same pages; quoted in the docs, not used as defaults. The steel row is the opt-in preset `particles.erosion.material = "dnv_o501_2007_steel"` (ADR 0197); the other rows are quoted only. The Table 7-2 values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate |
| Rough wall: impact angle `theta' = theta + Delta_gamma xi`, `xi` standard normal; a virtual wall tilted by the roughness angle; the shadow effect described in words | verified (secondary) | Tian, Inthavong, Tu and Yeoh, ANZIAM J. 48 (CTAC2006) C509-C526 (2007), eq. 5 and Figure 2; Lain and Sommerfeld, slides of the 14th Workshop on Two-Phase Flow Predictions (Halle, 2015); Radenkovic, doctoral dissertation, University of Belgrade (2019), eq. 7.21 and 7.27; Sommerfeld (1992) and Sommerfeld and Huber (1999) not accessed |
| Realizability rule (redraw until the parcel reaches the virtual wall from the fluid side and rebounds away from the real wall) and the resulting rebound-angle distribution | derived | ADR 0194 W4 and W5; not the effective distribution of Sommerfeld and Huber (1999), which was not read |
| One-sample Kolmogorov-Smirnov test, Kolmogorov distribution `1 - 2 sum (-1)^(k-1) exp(-2 k^2 x^2)`, `K_0.01 = 1.6276` | verified (secondary) | Wikipedia "Kolmogorov-Smirnov test", read 2026-11-06; the originals not accessed |
| Measured erosion rates or rebound distributions | not used | no comparison attempted |

## F-249 parametric studies: NSGA-II, Nelder-Mead options, extended warm start (ADR 0209, docs/design-loop.md)

Nothing in F-249 is compared with measured data. Every check is against a closed form, an analytic test function or a cold run of
the same case.

| Item | Status | What was read (source) |
|---|---|---|
| NSGA-II: fast non-dominated sorting, crowding distance, crowded binary tournament, elitist survivor selection, constrained domination; crossover probability 0.9, distribution indices 20, mutation probability 1/d | from memory | Deb, Pratap, Agarwal and Meyarivan, IEEE Trans. Evol. Comput. 6 (2002) 182-197, NOT read for ADR 0209; the constants are those recalled from its experiments (ADR 0209 question 5) |
| Simulated binary crossover `beta = (2u)^(1/(eta+1))` or `(1/(2(1-u)))^(1/(eta+1))`, children `0.5((1 +- beta) p1 + (1 -+ beta) p2)` | from memory | Deb and Agrawal (1995), NOT read; applied to every variable of a crossed pair, without exchanging the children's values (ADR 0209 D1; the MO1z miss and its diagnosis, amendment 1) |
| Polynomial mutation `delta = (2u)^(1/(eta+1)) - 1` or `1 - (2(1-u))^(1/(eta+1))`, `x += delta (hi - lo)` | from memory | Deb and Goyal (1996), the simple form, NOT read |
| ZDT1 (`f1 = x1`, `g = 1 + 9 sum x_i/(d-1)`, `f2 = g (1 - sqrt(f1/g))`, front `f2 = 1 - sqrt(f1)`) and FON (`f_{1,2} = 1 - exp(-sum (x_i -+ 1/sqrt d)^2)`, x in [-4, 4]^3) | from memory | Zitzler, Deb and Thiele (2000) and Fonseca and Fleming (1995), NOT read; the definitions are checked by their own closed forms (ADR 0209 MO3c) |
| Gao-Han adaptive Nelder-Mead coefficients `alpha = 1, beta = 1 + 2/n, gamma = 0.75 - 1/2n, delta = 1 - 1/n` | verified (secondary) | the Optim.jl documentation, "Nelder-Mead" page (julianlsolvers.github.io/Optim.jl/stable/algo/nelder_mead/), read 2026-11-07; Gao and Han, Comput. Optim. Appl. 51 (2012) 259-277, NOT read (the University of Idaho copy was not reachable) |
| `fminsearch` initial simplex: "adding 5% of each component x0(i) to x0 ... uses 0.00025 as component i if x0(i) = 0" | read | MathWorks documentation, "Optimizing Nonlinear Functions" (the fminsearch algorithm), read 2026-11-07; no MathWorks code was opened. The `fminsearch` reference page's Rosenbrock example (start (-1.2, 1), final value 8.17766e-10 in its figure title) was read for context only; it gives no value at 120 evaluations |
| Warm start of temperature and k-epsilon / k-omega SST fields | derived | no source; the checks compare a warm and a cold run of the same discrete equations (ADR 0209 W1 to W4) |

## F-219 immersed solid with modified forcing (ADR 0218)

| Item | Status | What was read (source) |
|---|---|---|
| DFG 2D-1 set-up (channel `[0, 2.2] x [0, 0.41]`, cylinder centre `(0.2, 0.2)`, radius 0.05, `rho = 1`, `nu = 1e-3`, inflow `4 U y (0.41 - y)/0.41^2`, `U = 0.3`, `U_mean = 0.2`, Re 20, `C_D,L = 2 F_D,L/(U_mean^2 L)`, `L = 0.1`, pressure difference between `(0.15, 0.2)` and `(0.25, 0.2)`, do-nothing outflow) and reference values `C_D = 5.57953523384`, `C_L = 0.010618948146`, `p_diff = 0.11752016697` | read | FeatFlow benchmark page "DFG flow around cylinder benchmark 2D-1, laminar case (Re=20)", `wwwold.mathematik.tu-dortmund.de/~featflow/en/benchmarks/cfdbenchmarking/flow/dfg_benchmark1_re20.html`, read 2026-11-07 ("computed with high order spectral methods") |
| DFG 2D-2 set-up (`U = 1.5`, `U_mean = 1`, Re 100, `St = L f/U_mean`) and the level-6 values at `k = 1/200`: `St = 0.30188`, `1/f = 0.33125`, max/min/mean `C_D` 3.2200/3.1569/3.1884, max/min `C_L` 0.9859/-1.0206 | read | FeatFlow benchmark page "DFG flow around cylinder benchmark 2D-2, time-periodic case (Re=100)", `.../dfg_benchmark2_re100.html`, read 2026-11-07 |
| The lower and upper bounds of Schaefer and Turek (1996), Notes on Numerical Fluid Mechanics 52, 547-566 | not obtained | the paper was not read; the bounds are not used |
| Volume penalisation (Angot, Bruneau, Fabrie 1999), direct forcing (Mohd-Yusof 1997; Fadlun et al. 2000), the linear Dirichlet extrapolation to the interface crossing (Gibou, Fedkiw, Cheng, Kang 2002) | from memory | NOT read; the algebra of ADR 0218 D2 is derived on rcfd's discrete equations |
| Circular Couette flow `u_theta = A r + B/r`, `A = -omega R1^2/(R2^2 - R1^2)`, `B = omega R1^2 R2^2/(R2^2 - R1^2)`, torque `4 pi mu omega R1^2 R2^2/(R2^2 - R1^2)` | derived | textbook solution of the axisymmetric Navier-Stokes equations, derived here |

## F-309 second-order immersed forcing: the face flux at the surface crossing (ADR 0234)

Opt-in (`ImmersedForcing::build_second_order`); the forcing of ADR 0218 (`build`) is unchanged and stays what a caller gets.

| Item | Status | What was read (source) |
|---|---|---|
| The ghost-cell (ghost-fluid) closure of a first fluid cell with the extension of the fluid profile through the no-slip value at the crossing (Fedkiw, Aslam, Merriman, Osher 1999; Gibou, Fedkiw, Cheng, Kang 2002; Tseng and Ferziger 2003; Mittal et al. 2008) | from memory | NOT read; the face flux `(HbyA_c + (t_f/theta)(u_b(x_G) - HbyA_c)) . S_f` of ADR 0234 D1 is derived on rcfd's pressure equation |
| A mass source in the cells of the body so that the flow inside the body does not constrain the fluid (Kim, Kim, Choi, J. Comput. Phys. 171 (2001) 132-150) | from memory | NOT read; the source of ADR 0234 D2 (the solid cells' net outflow, passed as the volume source of ADR 0052) and the net-flux correction of D3 are derived there |
| Circular Couette flow as the measure of the order (the set-up CC of ADR 0218) | derived | as the F-219 row above; measured `e` 6.12e-3, 1.61e-3, 4.24e-4, 1.03e-4 at N 24, 48, 96, 192 (orders 1.93, 1.93, 2.04) against 8.29e-3, 3.20e-3, 1.58e-3 of the forcing of ADR 0218 |

## F-337 immersed solid: second-order moment of the forcing (ADR 0239)

Opt-in (`ImmersedForcing::force_moment_second_order`); `force_moment` is unchanged and stays what a caller gets.

| Item | Status | What was read (source) |
|---|---|---|
| The force and moment on a body from the momentum and angular-momentum balance of a control volume (Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2), and the force on an immersed boundary from its forcing and the momentum of the forced cells (Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719; Balaras, Comput. Fluids 33 (2004) 375-404) | from memory | NOT read; the statement of ADR 0239 D1 (the moment of the forcing with every term at the centre of its cell is the moment of the full traction and of the momentum flux on the regular fluid faces around the forced cells) is derived there on rcfd's discrete momentum equation |
| The identity `integral x x (grad u^T n) dS = -integral n x u dS` for a divergence-free field on a closed surface | derived | ADR 0239 D1 (Stokes' theorem on a closed surface) |
| Circular Couette torque `4 pi mu omega R1^2 R2^2/(R2^2 - R1^2) = 4.18879` as the measure of the order of the moment | derived | as the F-219 row above; measured error 0.396, 0.107, 0.027 percent at N 24, 48, 96 (orders 1.89, 1.98) against 3.46, 1.60, 0.74 percent of `force_moment` |
| DFG 2D-1 reference values | read | as the F-219 row above; the force is that of `force_moment` (M2, second-order forcing: `C_D` -0.306 percent, `C_L` +2.534 percent) |

## F-346 immersed solid: force and moment in unsteady flow (ADR 0249)

Opt-in (`ImmersedForcing::force_moment_unsteady`, `immersed::velocity_rate`); `force_moment` and `force_moment_second_order`
are unchanged.

| Item | Status | What was read (source) |
|---|---|---|
| The force on a body from the momentum balance of a fixed control volume (Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2), and the hydrodynamic force on a body represented by a volume forcing as the reaction of the forcing plus the rate of change of the momentum of the fictitious fluid inside it (Uhlmann, J. Comput. Phys. 209 (2005) 448-476; Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719; Balaras, Comput. Fluids 33 (2004) 375-404) | from memory | NOT read; ADR 0249 D1 derives on rcfd's discrete momentum equation that the storage to add back is that of the solid cells and that the first fluid cells do not enter |
| Inner cylinder accelerated with `Omega = alpha t` in a fixed outer one: `u_theta = alpha t (A r + B/r) + w(r)`, `w = (alpha/nu)(A r^3/8 + (B/2) r ln r) + C r + D/r`, moment on the inner cylinder per unit depth `-4 pi mu B alpha t + 2 pi R1^2 mu [(alpha/nu)(A R1^2/4 + B/2) - 2 D/R1^2]` (`-4.188790 t - 0.123934` for `R1 = 0.5`, `R2 = 1`, `alpha = nu = rho = 1`) | derived | ADR 0249 amendment 1 (X1b); measured error 0.167 and 0.078 percent at N 48 and 96, the reaction alone 4.3 and 4.4 percent |
| Plate accelerated with `U = a t` under a wall at rest at distance `d`: `u = a t (1 - eta/d) + (a/nu)(eta^2/2 - eta^3/(6 d) - d eta/3)`, force on the plate per area `-(mu a t/d + rho a d/3)` | derived | ADR 0249 (X2, X2b); measured error 0.056 and 0.014 percent at N 16 and 32, the reaction alone 42.8 percent |
| DFG 2D-2 level-6 values | read | as the F-219 row above (max `C_D` 3.2200, max `C_L` 0.9859); measured values in ADR 0249 addendum 1 |

## F-355 second-order immersed forcing in transient PISO: bounded weights (ADR 0254)

Opt-in (`ImmersedForcing::build_second_order_bounded`); `build` and `build_second_order` are unchanged.

| Item | Status | What was read (source) |
|---|---|---|
| Ghost-cell extrapolation near a boundary point close to a grid node has large weights of alternating sign and can destabilise the update; remedy: a stencil with bounded weights through an image point in the fluid (Tseng and Ferziger, J. Comput. Phys. 192 (2003) 593-623; Mittal et al., J. Comput. Phys. 227 (2008) 4825-4852) | cited from memory, not read for this work | the formula used (the wall value reflected through the value at the image point of the face, `2 u_G - ((1 - a) HbyA_c + a HbyA_d)`, error `xi_f^2 u''` plus the linear interpolation) is derived in ADR 0254 amendment 1; measured on circular Couette: order 1.947, face-flux reconstruction error 7.99e-4 at N 96 with ratio 3.86 |
| DFG 2D-2 level-6 values | read | as the F-219 row above (`St` 0.30188, max `C_D` 3.2200, max `C_L` 0.9859); measured on M2 with the bounded form in ADR 0254 addendum 1: 0.30112, 3.20930, 0.97593 |

## F-354 immersed solid: DFG 2D-2 force extrema under refinement (ADR 0268)

Measurement only; no library code.

| Item | Status | What was read (source) |
|---|---|---|
| DFG 2D-2 level-6 values | read | as the F-219 row above (max `C_D` 3.2200, max `C_L` 0.9859, `St` 0.30188); measured on the D/80 meshes in ADR 0268 addendum 1: T2 `build` 3.22876, 0.98911, 0.30158; T2 bounded 3.22439, 0.98762, 0.30175; M3 `build` 3.22460, 0.98311, 0.30151; M3 bounded 3.22024, 0.98167, 0.30168 |
| Upstream decay of an outlet disturbance in a channel, about `exp(-pi x/H)` | derived | used only to read the outlet measurement P5 of ADR 0268 (no change to six digits when the outlet moves from 2.2 to 4.4) |

## F-297 NSGA-II crossover: the operator read against the papers (ADR 0220, docs/design-loop.md)

Read 2026-11-07 for the registration of ADR 0220, before any code. No source listing of any optimisation code was opened (the web
searches also listed implementation source pages of other NSGA-II and SBX codes; none was opened). The rows of F-249 above stay as
they were written; this section says what the reading found.

| Item | Status | What was read (source) |
|---|---|---|
| NSGA-II main loop, crowded binary tournament, survivor selection; real-coded constants: crossover probability 0.9, mutation probability 1/n, distribution indices 20 and 20; population 100, 25 000 evaluations (250 generations); "the variablewise SBX operator used here" | **read**: equal to ADR 0209 D1 | Deb, Pratap, Agarwal and Meyarivan, IEEE Trans. Evol. Comput. 6 (2002) 182-197, sections III-C and IV-A (copy at cse.unr.edu/~sushil/class/gas/papers/nsga2.pdf). Its Table II gives the convergence metric of real-coded NSGA-II on ZDT1 as 0.033482 (mean of 10 runs), variance 0.004750, and on FON 0.001931 (a mean distance of the final population to 500 front points, not ADR 0209's C) |
| ZDT1 and FON definitions (n, bounds, objectives, Pareto-optimal solutions) | **read**: equal to ADR 0209 | the same paper, Table I |
| SBX: spread factor `beta = abs((c1 - c2)/(p1 - p2))`, children equidistant from the parents' mid-point; densities `0.5 (n + 1) beta^n` (`beta <= 1`) and `0.5 (n + 1) / beta^(n + 2)` (`beta > 1`); multi-variable problems: "we choose to perform SBX in each variable with probability 0.5" (1.0 for a single variable), by analogy with single-point crossover (which swaps the variables beyond the cross site between the parents) and uniform crossover; children unbounded, a bounded distribution suggested as an extension | **read** | Deb and Agrawal, Complex Systems 9 (1995) 115-148, eqs. (6), (19), (20), sections 2, 3.1 and 6 (the publisher's PDF). ADR 0209's `beta(u)` is this distribution sampled by inversion. Differences from rcfd: rcfd crosses every variable of a crossed pair (ADR 0220 question 2) and clamps into the box |
| SBX procedure with bounds (`beta_q` from `u` and `alpha = 2 - beta^-(eta+1)`), children `c1 = 0.5 [(y1 + y2) - beta_q abs(y2 - y1)]`, `c2 = 0.5 [(y1 + y2) + beta_q abs(y2 - y1)]` for `y1 < y2` ("a simple modification ... for y1 > y2"); "each variable is chosen with a probability 0.5"; polynomial mutation `delta_q` with the bound term `(1 - delta)^(eta_m + 1)`, `c = y + delta_q (y_u - y_l)`, citing Deb and Goyal (1996) | **read** | Deb and Agrawal (S.), "A niched-penalty approach for constraint handling in genetic algorithms", ICANNGA 1999, Appendix A (copy at homes.cs.washington.edu/~sagarwal/icannga.pdf). ADR 0209's SBX and mutation are these formulas without the bound terms (`alpha = 2`, `delta = 1`). The text does not state which child receives which value when `y1 > y2`, and states no exchange between the children |
| Polynomial mutation `p' = p + delta_L (p - x_L)` (`u <= 0.5`) or `p + delta_R (x_U - p)`, `delta_L = (2u)^(1/(1 + eta_m)) - 1`, `delta_R = 1 - (2(1 - u))^(1/(1 + eta_m))` | **read** (another bounded form) | Deb and Deb, KanGAL report 2012016 (Int. J. Artif. Intell. Soft Comput. 4 (2014) 1-28), section 2 (copy at egr.msu.edu/~kdeb/papers/k2012016.pdf); it credits the operator to Deb and Agrawal (1999) |
| The per-variable exchange of the children's values (`Crossover::Exchange`) | **not in the papers read** | the lead's operator from ADR 0209 diagnostic DG1; its link to the papers is the analogy of Deb and Agrawal (1995) section 3.1 |
| Deb and Goyal, Computer Science and Informatics 26 (1996) 30-45 (the original polynomial mutation) | **not obtained** | no open copy found; the mutation was read in the 1999 and 2012 texts above (ADR 0220 question 1) |

## F-260 CMA-ES: the algorithm read in Hansen's tutorial and the restart paper (ADR 0222, docs/design-loop.md)

Read 2026-11-08 for the registration of ADR 0222, before any code. No source listing of any CMA-ES or other optimisation code
was opened: the tutorial's Appendix C (MATLAB source code, pages 36 and 37) was not opened or rendered, its link to a Python
implementation of the weights (page 32) was not followed, and the MATLAB code version named in a footnote of Auger and Hansen
(2005) was not looked at. Nothing in F-260 is compared with measured data.

| Item | Status | What was read (source) |
|---|---|---|
| The (mu/mu_W, lambda)-CMA-ES: sampling `z ~ N(0, I)`, `y = B D z`, `x = m + sigma y`; recombination `<y>_w`; mean update with `c_m`; step-size path with `C^{-1/2} = B D^-1 B^T` and the step-size update with `d_sigma` and `E norm(N(0, I))`; `h_sigma` with `(1.4 + 2/(n+1)) E norm(N(0, I))`; covariance path and update with `c_1`, `c_mu`, `delta(h_sigma)` and the negative weights `w_i n / norm(C^{-1/2} y_i)^2`; `E norm(N(0, I)) ~ sqrt n (1 - 1/(4n) + 1/(21 n^2))` | **read** | N. Hansen, "The CMA Evolution Strategy: A Tutorial", arXiv:1604.00772v2 (2023), Appendix A, Fig. 6, eqs. (38) to (47), page 28 (the arXiv PDF, pages read as rendered images) |
| Default parameters: `lambda = 4 + floor(3 ln n)`, `w'_i = ln((lambda + 1)/2) - ln i`, `mu = floor(lambda/2)`, `mu_eff`, `mu_eff^-`, `alpha_mu^- = 1 + c_1/c_mu`, `alpha_mu_eff^- = 1 + 2 mu_eff^-/(mu_eff + 2)`, `alpha_posdef^- = (1 - c_1 - c_mu)/(n c_mu)`, the weights (53), `c_m = 1`, `c_sigma = (mu_eff + 2)/(n + mu_eff + 5)`, `d_sigma = 1 + 2 max(0, sqrt((mu_eff - 1)/(n + 1)) - 1) + c_sigma`, `c_c = (4 + mu_eff/n)/(n + 4 + 2 mu_eff/n)`, `c_1 = 2/((n + 1.3)^2 + mu_eff)`, `c_mu = min(1 - c_1, 2 (1/4 + mu_eff + 1/mu_eff - 2)/((n + 2)^2 + mu_eff))` | **read** | the same, Table 1 (eqs. 48 to 58), page 31; the generalised `c_c` (61) on page 32 was read and not used |
| Initial values: `p_sigma = p_c = 0`, `C = I`; "If the optimum is expected to be in the initial search interval [a, b]^n we may choose ... m uniformly randomly in [a, b]^n, and sigma = 0.3(b - a)"; different intervals by `c_ii` or by a scaling of the variables | **read** | the same, Fig. 6 and its footnote, page 29 |
| Termination criteria NoEffectAxis (0.1 standard deviation along an axis, `i = (g mod n) + 1`), NoEffectCoord (0.2 standard deviations in a coordinate), ConditionCov (`10^14`), EqualFunValues and TolFun (`10 + ceil(30 n/lambda)` generations, `10^-12`), Stagnation (20 %, at least `120 + 30 n/lambda`, at most 20 000 iterations, medians of the first and last 30 %), TolXUp (`10^4`), TolX (`10^-12` times the initial `sigma`) | **read** | the same, Appendix B.3, pages 33 and 34 |
| Box handling: `f_max + norm(x - x_feasible)` (62) with `x_feasible` "preferably in the middle of the feasible domain", or resampling, when the optimum is not too close to the boundary; repair with the penalty `alpha norm(x - x_repaired)^2` (63) | **read** | the same, Appendix B.5, pages 34 and 35 (rcfd uses (62) and does not evaluate the infeasible point) |
| Recomputing `B` and `D` only every `max(1, floor(1/(10 n (c_1 + c_mu))))` generations | **read**, not used | the same, Appendix B.2 (rcfd decomposes every generation) |
| Restarts with the population doubled ("increased by a factor of 2"); stopping criteria Tolfun `10^-12`, TolX `10^-12 sigma0`, noeffectaxis, noeffectcoord, conditioncov `10^14`; initial point uniform in `[A, B]^n` and `sigma0 = (B - A)/2` per restart; budget `n x 10^4`, target error `10^-8`; Table 2 (n = 10): shifted Rastrigin success rate 0.76 (25 runs, error `10^-2` within `n x 10^4`), median 7.85e4 evaluations | **read** | A. Auger, N. Hansen, "A Restart CMA Evolution Strategy With Increasing Population Size", Proc. IEEE CEC 2005, 1769-1776, pages 1 to 4 (copy in the CEC 2005 contributions at sci2s.ugr.es). rcfd keeps the given start point for every restart (ADR 0222 question 5) and the tutorial's `0.3 (b - a)` |
| Box-Muller normal draws `sqrt(-2 ln u1) cos(2 pi u2)` | from memory | G. E. P. Box, M. E. Muller, Ann. Math. Statist. 29 (1958) 610-611, NOT read; checked by ADR 0222 K3 (moments and the distribution function against Simpson's rule) |
| Cyclic Jacobi eigenvalue method for symmetric matrices | from memory | the textbook method (for example Golub and Van Loan, Matrix Computations), NOT read for ADR 0222; checked by ADR 0222 K2 (reconstruction and orthogonality) |
| Rastrigin (`10 d + sum (x_i^2 - 10 cos 2 pi x_i)`), sphere, ellipsoid `sum 10^(6 (i-1)/(d-1)) x_i^2`, Rosenbrock | from memory | standard test functions; checked by their closed forms (ADR 0222 K0) |
| Hansen and Kern (2004), "Evaluating the CMA Evolution Strategy on Multimodal Test Functions", PPSN VIII | not obtained | no open copy found |

## F-221 non-equilibrium condensing steam (ADR 0217)

The equations and the nozzle data were read from the sources below (page images or extracted text; the PDFs are kept outside
the repository). SHA-256 of the files read: GT2016-57598 accepted manuscript
`172644ad36a846a67a01d0dc1a94aae8f62ceabc05b40a9f242f2d4ff23c4ae3`, GT2011-45672 `f77c3faa910dac090e693eeda16cb63b89ad836c2a46ba6df7e07311ab61fa32`,
Afzalifar et al. 2016 `bcbaa6e78b8f441fc62cb118138dd6c3eb45eed76303aa548687939ea67f597f`, Dykas et al. 2013
`c70aff5280c5a098184e9fb9a0e630593c1592497d312ba40c647f5a02e02c94`, IAPWS R1-76(2014)
`77d07ae4c6d473806b73f982d4c031b927db02a5718bd3c25cd7a03c3321587d`.

| Item | Status | What was read (source) |
|---|---|---|
| Nucleation rate per unit mass of mixture `J = q_c/(1+phi) sqrt(2 sigma/(pi m^3)) rho_g^2/(rho rho_f) exp(-4 pi sigma r*^2/(3 k_B T_g))`, `q_c = 1`; Young's growth `dr/dt = lambda_g/(rho_f L r) (T_s - T_g)(1 - r*/r)/(1 + 3.78 (1 - nu) Kn/Pr)`, `nu = R T_s/L (alpha - 0.5 - (2 - q_c)/(2 q_c) ((gamma + 1)/(2 gamma)) (c_p T_s/L))`; `alpha = 11` | read | Starzmann, Hughes, White, Grübel, Vogt, ASME GT2016-57598, Eqs. 1-3 and text (accepted manuscript, University of Cambridge Apollo repository, handle 1810/256656) |
| Moses and Stein nozzle: depth 10 mm, throat height 10 mm, wall arcs R = 53 mm and R = 686 mm (Fig. 1); case 203 `p_t1 = 35.87 kPa`, `T_t1 = 368.3 K`; 41 centreline pressures and 17 Sauter radii (Fig. 2, data taken by the authors from Young 1982) | read (digitized) | the same paper, Figs. 1 and 2 digitized from their vector paths with the figures' own tick marks (ADR 0217); the values are tabulated in ADR 0217 and in `crates/rcfd-fv/tests/wetsteam.rs`. Moses and Stein (1978) and Young (1982) were not read |
| The Moses and Stein nozzle's transonic and supersonic sections are one circular arc; throat height 0.01 m | read | Afzalifar, Turunen-Saaresti, Grönman, ISROMAC 2016 (HAL hal-01891315), section 4 |
| Critical radius `r* = 2 sigma/(rho_d R T_v ln S)`, `S = p_v/p_s`; single-fluid mixture `rho_m = rho_v/(1 - Y)`, `E_m = Y E_d + (1 - Y) E_v` | read | the same paper, Eqs. 1, 5-8 |
| Kantrowitz correction `C = [1 + 2 (gamma-1)/(gamma+1) (h_v - h_l)/(R T_v) ((h_v - h_l)/(R T_v) - 1/2)]^-1`; `h_m = (1 - y) h_v + y h_l` | read | Dykas, Majkut, Smołka, Strozik, J. Power Technologies 93(5) (2013) 288-294, Eqs. 2 and 5 (open access) |
| Knudsen number `Kn = l/(2r)`, mean free path `l = 1.5 mu sqrt(R T_g)/p`, `Pr = mu c_p/lambda` | read | Starzmann, Schatz, Casey, Mayer, Sieverding, ASME GT2011-45672, Eqs. 6-9 |
| Surface tension `sigma = 235.8 mN/m tau^1.256 (1 - 0.625 tau)`, `tau = 1 - T/647.096 K`; Table 1 column 4; extrapolation to -25 °C | read | IAPWS R1-76(2014), pages 3-4 |
| Boltzmann constant `1.380 649 x 10^-23 J/K`, exact | read | NIST CODATA, physics.nist.gov/cgi-bin/cuu/Value?k, 2026-11-07 |
| Saturation line, liquid density, latent heat, `c_p`, vapour viscosity and conductivity | existing | IAPWS-IF97 and the IAPWS 2008/2011 transport correlations as implemented and verified in `rcfd-core::iapws` (F-111); the saturation equation and region 1 are extrapolated below 273.15 K only in transients (ADR 0217 M3) |
| Moore nozzle B geometry | not read | stated in GT2016-57598 to be "available in the IWSMP collaboration"; not printed in any source read |
| F-320 checks (ADR 0226): rates decomposition, dry nucleation box, frozen sound speed, HLLC upwinding | no new data | the reference values are ADR 0217's M1 table (the F-221 prototype) and first principles (isentropic derivative of the M2 equation of state, the upwind branch of HLLC, Toro ch. 10); no new source was read |

## F-245 droplet evaporation and TAB breakup (ADR 0224)

Read 2026-11-08 for the registration of ADR 0224, before any code. No source listing of any spray or CFD code was opened (web
searches listed source pages of open-source and commercial implementations of the TAB model and theory pages of a commercial code;
none was opened). Nothing in F-245 is compared with measured data; the liquid and gas values of the checks are round test inputs.
SHA-256 of the files read: LA-UR-87-2105-Rev. scan `ccc109b4b1f663149641924b4f67fa78ec43dac5b5af5cb4050ab965af4f9bef`,
arXiv:2201.02818v1 `52c7b9dd554ccd34d2150a68fbdb5717710a5bf25ada3ed1495283993c1a86c4`, arXiv:2510.13033v2
`7f4dc3615150dd83a1e8a1d99435f7173c619ebe08a89732fc4b04b655d64cd3`.

| Item | Status | What was read (source) |
|---|---|---|
| TAB equation `ydd = (C_F/C_b)(rho_g/rho_l)(u^2/r^2) - (C_k sigma/(rho_l r^3)) y - (C_d mu_l/(rho_l r^2)) yd`, breakup at `y = 1`; `We = rho_g u^2 r/sigma`, `1/t_d = (C_d/2) mu_l/(rho_l r^2)`, `omega^2 = C_k sigma/(rho_l r^3) - 1/t_d^2`; constants `C_k = 8` (Lamb, fundamental frequency), `C_d = 5` (Lamb, damping), `C_b = 1/2`, `C_F = 1/3`, `We_crit = 6` (shock experiments), `C_k C_b/C_F = 12`; the numerical implementation (eqs. 25a, 25b, the amplitude test `We/12 + A <= 1`, the breakup time of eq. 26 "assuming that the drop oscillation is undamped for its first period", `N^(n+1) = N^n (r^n/r^(n+1))^3`, `y = yd = 0` after breakup) | read | P. J. O'Rourke, A. A. Amsden, "The TAB method for numerical calculation of spray droplet breakup", Los Alamos report LA-UR-87-2105-Rev. (SAE paper 872089, 1987), eqs. 1 to 10, 25, 26 and the section "Numerical implementation"; scan at the UNT Digital Library (`digital.library.unt.edu/ark:/67531/metadc1106123/m2/1/high_res_d/6118786.pdf`). The first line of the printed eq. 4 is too faint in the scan; it was derived from eq. 3 and agrees with eqs. 25a, 25b |
| Product drops: `r/r_32 = 1 + 8K/20 + (rho_l r^3/sigma) yd^2 (6K - 5)/120` (eq. 22), `K = 10/3`, bag regime `r/r_32 = 7/3`, large We `rho_g u^2 r_32/sigma = 6` (eq. 23); normal velocity `V_perp = C_v C_b r yd` (eq. 14), "C_v is approximately unity", taken as 1; `tan(theta/2) = C_v (sqrt 3/3) sqrt(rho_g/rho_l)` (eq. 16); breakup times `sqrt 3 sqrt(rho_l/rho_g) r/u` for large We (eq. 12a) and `pi sqrt(rho_l r^3/(8 sigma))` near critical (eq. 13); product radii drawn from a chi-squared distribution | read | the same report, eqs. 12a to 23 and the text after eq. 22 (rcfd gives every product drop the radius `r_32`: ADR 0224 Q4) |
| TAB constants `C_k = 8`, `C_f = 1/3`, `C_d = 5`, `C_b = 1/2`, `K = 10/3` and eq. 22 | verified (secondary) | P. John, H. Chandrasekhar, S. Saha, O. Owoyele, arXiv:2510.13033v2, section 2.2.1 (equal to the report; its breakup test reads "A + We_g > 1.0" in the text extraction where the report has `We/12 + A > 1`) |
| Breakup time `arccos(1 - 1/W)/omega` from rest, `r/r_32 = 4/3 + We/6` without viscosity, `We_child = 6 We/(8 + We) < 6` | derived | ADR 0224 DB from eqs. 22 and 26 |
| Quasi-steady droplet evaporation with Bird's correction (model B): `dm_p/dt = -pi d_p rho_m D_m Sh_m,0 ln(1 + B_M)`, `dT_p/dt = pi d_p lambda_m Nu_m,0/(m_p c_p,p) (T_s - T_p) ln(1 + B_T)/B_T + (L_v/(m_p c_p,p)) dm_p/dt`, `1 + B_T = (1 + B_M)^phi_m`, `phi_m = (c_p,vap,m/c_p,m)(1/Le_m)(Sh_m,0/Nu_m,0)`, `B_M = (Y_f,i - Y_f,s)/(1 - Y_f,i)`, `Nu_m,0 = 2 + 0.6 Re^(1/2) Pr^(1/3)` (Ranz-Marshall), `Sh_m,0 = 2 + 0.6 Re^(1/2) Sc^(1/3)`, Raoult's law and the frozen-chemistry surface mass fraction; the psychrometric wet bulb `c_p,vap,m (T_s - T_p) = B_T L_v`; the boiling point of n-heptane 371.5 K | read | A. Both, D. Mira, O. Lehmkuhl, "Evaporation of volatile droplets subjected to flame-like conditions", arXiv:2201.02818v1 (2022), sections 2.1, 2.2, eqs. 14 to 25, Tables 3 and 4. The constant properties of rcfd (no 1/3 law, no Daubert-Danner functions inside the library) are a simplification of ADR 0224 |
| The d-squared law `d^2 = d0^2 - K t`, `K = 8 rho_m D_m ln(1 + B_M(T_wb))/rho_l = 8 lambda_m ln(1 + B_T)/(rho_l c_p,vap)`, independent of `d` at `Re = 0` | derived | ADR 0224 DK from the model above; arXiv:2510.13033 section 3.1 names "the classical D2-law" (its source not read) |
| Clausius-Clapeyron `dP/dT = P L/(R T^2)` for an ideal-gas vapour with negligible liquid volume, integrated for a constant `L`: `ln(P2/P1) = -(L/R)(1/T2 - 1/T1)` | verified (secondary) | Wikipedia, "Clausius-Clapeyron relation", read 2026-11-08; used through the boiling point, `X = (p_b/p) exp((L_v W_f/R)(1/T_b - 1/T))` |
| Molar gas constant `R = 8.314 462 618... J/(mol K)` (exact), Avogadro constant `6.022 140 76 x 10^23 /mol` (exact) | read | NIST CODATA 2022, physics.nist.gov/cgi-bin/cuu/Value?r and ?na, read 2026-11-08; rcfd uses `R = N_A k = 8.31446261815324` with `k` of the F-221 row |
| Abramzon-Sirignano film correction, Langmuir-Knudsen non-equilibrium, Miller, Harstad and Bellan (1998), Godsave and Spalding (1953), Taylor (1963), Reitz and Diwakar (1987) | not read | named in the sources above; not used (ADR 0224 Q6) |
| Measured droplet lifetimes, product drop sizes or breakup times | not used | no comparison attempted (F-252) |

## F-306 the folded edge removal of the tetrahedral mesher (ADR 0228)

No source was read for F-306. The cause and the fix are derived on the code of `crates/rcfd-mesh/src/tetmesh.rs`.

| Item | Status | What was read (source) |
|---|---|---|
| Edge removal: a ring of cells around an interior edge `(d, e)` replaced by the cells `(a, b, c, d)` and `(a, b, c, e)` of each triangle of a triangulation of the ring polygon; it is valid when every triangle, taken in the ring's turning order, has `d` on the side `-s` and `e` on the side `s` of its plane (`s = orient3d(d, e, poly[0], poly[1])`), and the old test (`d`, `e` on opposite sides) also passes triangles folded across a reflex corner | derived | ADR 0228 "The cause"; the same rule as F-247's `cand_removal` (ADR 0215 M3, M4) |
| Edge removal and its dynamic-programming triangulation (Shewchuk's topological improvement of tetrahedral meshes; Klincsek's polygon triangulation) | from memory | NOT read; the existing code's choice of triangulation is unchanged |
## F-302 periodic seam pair diffusivity: the property and user-function viscosities (ADR 0227)

Nothing was read for ADR 0227. The rule is the flux balance of a pair of finite-volume faces, derived in ADR 0205 D1, applied to the
face formulas the code already uses on internal faces (`crates/rcfd-api/src/properties.rs`, the user-function face loop of F-122).

| Item | Status | Source |
|---|---|---|
| Both faces of a periodic pair take one diffusivity, the internal-face formula with the pair's weight `w = dn_g / (dn_f + dn_g)`: `mu(w T_P + (1 - w) T_Q) / rho` for a property law, the module at the centre of the pair's face of lower index with `w T_P + (1 - w) T_Q` and `w abs(u_P) + (1 - w) abs(u_Q)` for a user function | derived | ADR 0205 D1, ADR 0212 D1, ADR 0227 D1 (the face of lower index is a choice of ADR 0227, question 1) |
| The property viscosity is already one value per pair by default, up to rounding (the periodic face temperature is the same interpolation) | derived | ADR 0227 item 2 (code read), checked by F2p, G2p and T4p |

## F-300 transient RANS follow-ups (ADR 0225)

| Item | Status | What was read (source) |
|---|---|---|
| Spalding (1961) law of the wall `y+ = u+ + (1/E)[exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6]`, used for the k-based wall eddy viscosity `nu (y*/u+ - 1)` of the opt-in `wall_nut_law = spalding` (`WallConstants::nut_wall_from_k_spalding`) and its inverse `SpaldingLaw::u_plus` | existing | the row "Spalding (1961) law of the wall" above (standard cubic form, two secondary sources, original paper not read); no new source was read for F-300 |
| The standard k-based wall eddy viscosity `nu (y* kappa/ln(E y*) - 1)` above `y*_lam` and 0 below, written as `nu (y*/u+ - 1)` with `u+ = min(y*, ln(E y*)/kappa)`; its slope jump `0.0684 nu` at `y*_lam = 11.53` | derived | from the existing `WallConstants::nut_wall_from_k` (Launder and Spalding 1974 as recorded above, not read) |

## F-431 sparse L D L^T with a fill-reducing ordering (ADR 0447)

No source was read for F-431; every method is cited from memory and checked by the residual, the fill count, the closed-form
spectrum of the tridiagonal (2, -1) and the envelope solver of ADR 0274.

| Item | Status | Source |
|---|---|---|
| Approximate minimum degree on the quotient graph: elements and variables, element absorption (and aggressive absorption, `|L_e \ L_p| = 0`), the bound `|A_i| + |L_p \ i| + sum_e |L_e \ L_p|` of the external degree, supervariables found by a hash of `A_i` and `E_i` | from memory | Amestoy, Davis and Duff, "An approximate minimum degree ordering algorithm", SIAM J. Matrix Anal. Appl. 17 (1996) 886-905: NOT read; derived in ADR 0447 D1 (the old-degree bound and the dense-row rule left out) |
| Minimum degree, mass elimination and indistinguishable nodes | from memory | George and Liu, "The evolution of the minimum degree ordering algorithm", SIAM Review 31 (1989) 1-19; Liu, "Modification of the minimum-degree algorithm by multiple elimination", ACM Trans. Math. Softw. 11 (1985) 141-153: NOT read |
| Elimination tree (`parent(j)` = least `k > j` with `l_kj != 0`) and the row structure of `L` as the row subtree | from memory | Liu, ACM Trans. Math. Softw. 12 (1986) 127-148; Liu, SIAM J. Matrix Anal. Appl. 11 (1990) 134-172: NOT read; ADR 0447 D2 |
| Up-looking `L D L^T` by rows with the tree reach as the pattern of row `k` | from memory | Davis, "Algorithm 849: a concise sparse Cholesky factorization package", ACM Trans. Math. Softw. 31 (2005) 587-591: NOT read; derived in ADR 0447 D3 |
| Nested dissection of a grid (used only for an order-of-magnitude fill estimate before the registration) | from memory | George, SIAM J. Numer. Anal. 10 (1973) 345-363: NOT read; `w49i-work/f431-scratch/nd_estimate.py` |
| Eigenvalues of the tridiagonal (2, -1) of order n: `4 sin^2(j pi/(2 (n + 1)))`; `lambda_j + lambda_{n+1-j} = 4` | derived | closed form (as F-382, ADR 0283); the symmetry made the registered middle shift of ADR 0447 A5 exactly 2 (amendment 1) |
| Sylvester's law of inertia: the negative pivots of `C = L D L^T` count the negative eigenvalues of `C` | derived | congruence (as ADR 0283 D8) |

## F-414 general real eigensolver, quadratic eigenproblem, damped modes (ADR 0450)

No source was read for F-414; every method is cited from memory and checked by residuals and closed forms (the companion matrix
of a polynomial with registered roots, the Jacobi values of F-382, Jordan blocks, the oscillator, the gyroscopic state solver of
F-505 as an independent cross-check).

| Item | Status | Source |
|---|---|---|
| Balancing by powers of 2 (scaling part only, the 0.95 acceptance test) | from memory | Parlett and Reinsch, "Balancing a matrix for calculation of eigenvalues and eigenvectors", Numer. Math. 13 (1969) 293-304: NOT read; ADR 0450 D1.1 |
| Householder reduction to Hessenberg form with the orthogonal factor accumulated | from memory | Golub and Van Loan, Matrix Computations, section 7.4: NOT read; ADR 0450 D1.2 |
| Francis implicit double-shift QR, bulge chase by 3-element reflections, deflation test `|h_{k,k-1}| <= eps (|h_{k-1,k-1}| + |h_kk|)` | from memory | Francis, "The QR transformation", Computer J. 4 (1961-62); Golub and Van Loan section 7.5: NOT read; ADR 0450 D1.3 |
| Exceptional shifts at the 10th and 20th iteration (sum `1.5 s`, product `s^2`, `s = |h_{hi,hi-1}| + |h_{hi-1,hi-2}|`); cap 30 iterations per eigenvalue | from memory | Wilkinson and Reinsch, Handbook for Automatic Computation II (1971), procedure hqr: NOT read; ADR 0450 D1.3; the cyclic permutation of order 6 needs them (check G6, mutation MU1) |
| Eigenvectors by back-substitution on the quasi-triangular Schur factor, small divisors replaced by `eps ||T||_F` | from memory | Golub and Van Loan section 7.6; the device of hqr2 (Handbook II): NOT read; ADR 0450 D1.5 |
| Damped oscillator `lambda = -zeta w +/- i w sqrt(1 - zeta^2)`; proportional damping `zeta_j = (a/w_j + b w_j)/2`; skew-coupled pair `w^4 - (k1 + k2 + g^2) w^2 + k1 k2 = 0` | derived | closed forms, ADR 0450 |
| Bar with a tip dashpot: `tanh(lambda L/c0) = -A sqrt(E rho)/c`; for `c < Z` `lambda_j = (c0/L)(-atanh(c/Z) + i (2j - 1) pi/2)`, for `c > Z` `lambda_j = (c0/L)(-atanh(Z/c) + i j pi)`, `j = 0` real (the roadmap's draft said no oscillating root above `Z`: corrected) | derived | wave equation with `E A u_x = -c u_t`, ADR 0450; truncation to `N` fixed-free modes estimated from the partial fractions `tanh s = sum 2 s/(s^2 + ((2k-1) pi/2)^2)` |
| Constant loss factor in the eigenproblem: `lambda = +/- i w sqrt(1 + i eta)`, the root with `Im > 0` decaying | derived | complex stiffness `(1 + i eta) K` (as F-412), ADR 0450 |
| Eigenvalues of a complex matrix from the real embedding `[[A_r, -A_i], [A_i, A_r]]`, the half with `u + i v != 0` | derived | ADR 0450 D2 |

## F-413 high-cycle fatigue: stress-life curves, mean stress, rainflow, Miner, narrow band (ADR 0449)

No source was read for F-413; every definition is cited from memory, and every number checked comes from a closed form derived
in ADR 0449 or from the library's own finite-element models (Kirsch and the beam formula as the references). The material values
used (`sigma_f = 900` MPa, `b = -0.09`, `sigma_u = 600` MPa, `sigma_y = 400` MPa) are illustrative, not data of a material.

| Item | Status | Source |
|---|---|---|
| Basquin `sigma_a = sigma_f (2 N)^b`, so `N = (sigma_a/sigma_f)^(1/b)/2` (the factor 2 on the reversals); power-law form `m = -1/b`, `C = sigma_f^m/2` | derived | Basquin, "The exponential law of endurance tests", Proc. ASTM 10 (1910) 625-630, in the reversal notation of Morrow: NOT read; ADR 0449 closed forms |
| Goodman `sigma_a/(1 - sigma_m/sigma_u)`, Gerber `sigma_a/(1 - (sigma_m/sigma_u)^2)`, Soderberg `sigma_a/(1 - sigma_m/sigma_y)`; no credit for a compressive mean by default | from memory | Goodman (1899), Gerber (1874), Soderberg, Trans. ASME 52 (1930); standard texts (e.g. Dowling, Mechanical Behavior of Materials): NOT read; derived as the lines (parabola) through the strength in ADR 0449 |
| Von Mises amplitude of `Re((A + i B) e^{i w t})`: `sqrt((q_A + q_B)/2 + sqrt(((q_A - q_B)/2)^2 + p_AB^2))` | derived | first principles, ADR 0449; checked against `max(s, sqrt 3 t)` and 200 sampled cases |
| Rainflow counting: four-point closing of hysteresis loops; periodic history rotated to its turning point of largest modulus | from memory | Matsuishi and Endo (1968); Downing and Socie, Int. J. Fatigue 4 (1982) 31-40; ASTM E1049-85 (2017) section 5.4.4: NOT read (the standard's worked example not used); properties derived and checked in ADR 0449 (`f413-scratch/rainflow.py`) |
| Palmgren-Miner linear damage `D = sum n_i/N_i` | from memory | Palmgren (1924); Miner, "Cumulative damage in fatigue", J. Appl. Mech. 12 (1945) A159-A164: NOT read |
| Narrow-band damage rate `nu0 (sqrt 2 s)^m Gamma(1 + m/2)/C`; `nu0 = sqrt(m2/m0)`, `s = sqrt(m0)` | derived | Rice, Bell Syst. Tech. J. 23 (1944) and 24 (1945); Miles, J. Aeronaut. Sci. 21 (1954) 753-762; Bendat (1964): NOT read; the Rayleigh integral evaluated in ADR 0449 |
| `ln Gamma` by Stirling's series with `B_2 .. B_14` for `x >= 15` and the recurrence below | from memory | Abramowitz and Stegun, Handbook of Mathematical Functions, 6.1.40: NOT read; Bernoulli numbers exact rationals; checked against factorials and half-integer values (B1) |
| Kirsch: hoop stress `3 sigma_0` at the edge of a circular hole, 90 degrees from remote tension | from memory | Kirsch, Z. VDI 42 (1898) 797-807; Timoshenko and Goodier, Theory of Elasticity: NOT read; measured 1.0123 on the quarter plate of ADR 0449 (`r/W = 0.05`) |
| Finite-width factor of a hole, `K_tn = 3 - 3.14 (d/W) + 3.667 (d/W)^2 - 1.527 (d/W)^3` (gross 3.023 at `d/W = 0.1`) | from memory | Peterson's Stress Concentration Factors after Howland (1930): NOT read; used only for the estimate of K1 in ADR 0449, not for a limit |
| Cantilever root stress `6 P L/(b h^2)` | derived | Euler-Bernoulli beam; the support of beam theory of ADR 0433 D2 |

## F-409 orthotropic elasticity, material axes, orthotropic expansion, temperature tables (ADR 0452)

| Item | Status | Source |
|---|---|---|
| Orthotropic compliance from nine engineering constants, `S_12 = -nu_12/E_1` etc., reciprocity `nu_ji/E_j = nu_ij/E_i`; the stiffness its inverse, isotropic constants give `lambda + 2 mu`, `lambda`, `mu` | derived | first principles (symmetry of the strain energy), ADR 0452 F1, F3; Jones, Mechanics of Composite Materials: NOT read |
| Positive definiteness: `nu_ij^2 < E_i/E_j` and `1 - nu12 nu21 - nu13 nu31 - nu23 nu32 - 2 nu21 nu32 nu13 > 0` | derived | Sylvester's criterion on the normal block, ADR 0452 F2; the restrictions of Lempriere as quoted in Jones: NOT read |
| Rotation of the engineering-strain Voigt vector `eps' = T eps`, `D = T^T C T` by energy conjugacy | derived | tensor transformation, ADR 0452 F4 |
| Off-axis lamina `1/E_x = c^4/E1 + s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2`, `S16 = (2 S11 - 2 S12 - S66) s c^3 - (2 S22 - 2 S12 - S66) s^3 c` (axes turned by `+theta`, rows `(c, s, 0)`, `(-s, c, 0)`) | derived | ADR 0452 F5, checked against a numerical rotation (`f409-scratch/py/verify.py`); Jones' `S-bar` from memory, sign convention of this ADR |
| Polar-orthotropic ring under internal pressure: `sigma_r = A r^(k-1) + B r^(-k-1)`, `k = sqrt(E_theta/E_r)`, bore hoop `k p (a^(2k) + b^(2k))/(b^(2k) - a^(2k))` (`34 p/15` for `b/a = 2`, `k = 2`) | derived | equilibrium and compatibility, ADR 0452 F7; Lekhnitskii, Theory of Elasticity of an Anisotropic Body: NOT read |
| Bar with a linear modulus: `delta = F L ln(E1/E0)/(A (E1 - E0))`; midpoint-rule error of 20 Hex8 `-1.126e-4` | derived | one integral, ADR 0452 F8 |
| Thermal strain of a tabulated coefficient `int_Tref^T alpha dT`, for `alpha = a0 + a1 T` `a0 (T - Tref) + a1 (T^2 - Tref^2)/2` | derived | ADR 0312 goal (instantaneous coefficient), ADR 0452 F9 |
| Timoshenko cantilever `P L^3/(3 E1 I) + P L/(kappa G13 A)`, `kappa = 5/6` | from memory | Timoshenko, Strength of Materials: NOT read; the support of beam theory of ADR 0385 D1 |

## F-419 three-node flat shell Shell3 (ADR 0454)

No source was read for F-419; every item is cited from memory and re-derived in ADR 0454 (arithmetic 1 to 8, the prototype
`f419-scratch/proto.py`). The one number taken from a source is the stabilisation factor `alpha = 0.1`, taken once and not
varied.

| Item | Status | Source |
|---|---|---|
| Constant-strain membrane of a triangle, `B` from the shape gradients `(b_i, c_i)/(2A)`; exact for a linear displacement | derived | first principles, ADR 0454 arithmetic 2 |
| MITC3 transverse shear: the assumed shear in the lowest-order edge space `gamma = (a1 - b Y, a2 + b X)`, tied to the tangential shear at the edge mid-points, `g_e = (w_j - w_i)/l_e + (beta_i + beta_j)/2 . t_e` | derived | P.-S. Lee and K.-J. Bathe, Comput. Struct. 82 (2004) 945-962; J. C. Nedelec, Numer. Math. 35 (1980) 315: NOT read; the tying re-derived in ADR 0454 arithmetic 4 |
| Shear stabilisation `H t^2/(t^2 + alpha h^2)`, `alpha = 0.1`, `h` the longest edge | from memory | M. Lyly, R. Stenberg and T. Vihinen, Comput. Methods Appl. Mech. Engrg. 110 (1993) 343-357; K.-U. Bletzinger, M. Bischoff and E. Ramm (DSG3), Comput. Struct. 75 (2000) 321-334: NOT read; plain MITC3 measured 0.8988 of Navier at `t/a = 0.001` (N4) |
| Drilling penalty tying `theta_z` to the membrane rotation `omega = (dv/dx - du/dy)/2` | from memory | Hughes and Brezzi, as ADR 0432: NOT read; Shell4's factor `DRILLING_FACTOR` reused |
| Navier series of the simply supported square plate under uniform pressure, `w_c = c p a^4/D`, `c = 0.004062` | derived | Timoshenko and Woinowsky-Krieger, Theory of Plates and Shells, as ADR 0432: NOT read; the series summed in ADR 0454 (SS4) |
| Consistent triangle mass `rho t A (1 + delta_ij)/12`, rotary inertia `rho t^3/12` on the three rotations | derived | the integral of `N_i N_j` over the triangle, ADR 0454 arithmetic 6 |
| Thin cylinder `u_r = p R^2/(E t)`, cantilever tip `P L^3/(3 E I)`, pinched ring diameter change | derived | membrane equilibrium and beam theory, as ADR 0432 |

## F-426 tied non-matching meshes, beam and shell joints to solids, cyclic extensions (ADR 0456)

| Item | Status | Source |
|---|---|---|
| One-pass node-to-surface tie fails the patch test on non-matching meshes; at 2:3 the slave trapezoid weights give a master corner `5/18` of the face width instead of `1/4`, the middle node `4/9` instead of `1/2` | derived | ADR 0456 arithmetic 1; Taylor and Papadopoulos 1991 (patch test for contact): NOT read, from memory; El-Abbasi and Bathe 2001: NOT read |
| NTS bar of two Hex8 parts (2:3, 4:6, 3:5, 6:10), far-end error and interface stress errors (`2.8797762239e-3` ... `5.6804678473e-2`) | computed | independent numpy prototype `f426-scratch/proto_tie.py`, ADR 0456 P; the Rust tie agrees within `4e-11` |
| Mortar tie `D u_s = M u_m` with the slave face functions as multipliers, segment integration on the clipped slave faces; passes the flat patch test exactly | derived | ADR 0456 arithmetic 2, 3; Puso and Laursen 2004 (segment-to-segment mortar), Wohlmuth 2000 (mortar finite elements): NOT read, from memory |
| Sutherland-Hodgman polygon clipping | from memory | Sutherland and Hodgman 1974: NOT read; the algorithm restated in `tie::clip_polygon` |
| Distributing coupling of a beam or shell node: the least-squares rigid motion of a node set, `A (t, theta) = sum B_i^T u_i` | derived | first principles, ADR 0456 arithmetic 4 |
| Cyclic sector, node on the axis: held unless `k = 0` (axial free) or `k = 1` doubled (`b = n x a` in-plane free) or `N = 2`, `2k = N` | derived | ADR 0456 arithmetic 5; Thomas 1979 (dynamics of rotationally periodic structures): NOT read, from memory |
| Rotating solid disc in plane stress, centre `(3 + nu) rho Omega^2 b^2 / 8`, field `sigma_r = (3 + nu) rho Omega^2 (b^2 - r^2)/8`, `sigma_theta = rho Omega^2 ((3 + nu) b^2 - (1 + 3 nu) r^2)/8` | derived, from memory | Timoshenko and Goodier, Theory of Elasticity: NOT read; equilibrium and compatibility checked, ADR 0456 arithmetic 6 |
| Regular polygon section `A = (M/2) a^2 sin(2 pi/M)`, `I = A a^2 (2 + cos(2 pi/M))/12` | derived | ADR 0456 arithmetic 7 |
| Timoshenko cantilever with `kappa = 10 (1 + nu)/(12 + 11 nu)` (rectangle), `6 (1 + nu)/(7 + 6 nu)` (circle) | from memory | Cowper 1966: NOT read |

## F-539 drilling-unknown membranes for Shell3: ANDES OPT and Allman's triangle (ADR 0466)

No source was read for F-539; every item is cited from memory. Allman's edge field and the basic stiffness are derived in ADR
0466 (arithmetic 1, 4); the higher-order stiffness of the ANDES element and its parameters `alpha_b = 3/2`, `beta_0 = max((1 -
4 nu^2)/2, 0.01)`, `beta_1 .. beta_9 = 1, 2, 1, 0, 1, -1, -1, -1, -2` are taken as recalled, once, and verified by the property
they were designed for: exact in-plane pure bending of a rectangle cut into two triangles for any aspect ratio (check BE1,
measured `3.7e-14`).

| Item | Status | Source |
|---|---|---|
| Allman's triangle: the edge-normal displacement quadratic with the mid-edge excess `l (theta_j - theta_i)/8`; the spurious mode of equal vertex rotations | derived | D. J. Allman, Comput. Struct. 19 (1984) 1-8: NOT read; ADR 0466 arithmetic 1, 2 |
| Basic stiffness of the free formulation: a constant stress lumped on the edge field with the excess times `alpha_b`, `K_b = A Bbar(alpha_b)^T A_m Bbar(alpha_b)` | derived | P. G. Bergan and C. A. Felippa, Comput. Methods Appl. Mech. Engrg. 50 (1985) 25-69: NOT read; ADR 0466 arithmetic 4 |
| ANDES higher-order stiffness `(3/4) beta_0 A sum_mid Q^T T_e^T A_m T_e Q` on the deviatoric corner rotations, the natural-strain matrices `Q_1, Q_2, Q_3` of `beta_1 .. beta_9`; the OPT parameters | from memory | C. A. Felippa and C. Militello, Finite Elem. Anal. Des. 12 (1992) 189-201; C. A. Felippa, Comput. Methods Appl. Mech. Engrg. 192 (2003) 2125-2168: NOT read; verified by BE1 (the factor `A/3` in place of `3A/4` gives 0.861) |
| Timoshenko cantilever tip `P L^3/(3 E I) + P L/(kappa G A)`, `kappa = 5/6`; end couple `M L^2/(2 E I)` | derived | beam theory; the clamped two-dimensional strip within `1.5e-4` of it by refinement (ADR 0466 arithmetic 6) |

## F-549 a drilling membrane for Shell4 matching the ANDES edge (ADR 0470)

No source was read for F-549; every item is cited from memory. The element is the mean of the two diagonal splits of the
quadrilateral into ANDES OPT triangles (the Shell3 membrane of ADR 0466, unchanged); that its edges carry the ANDES edge field
with weight one, so that it shares edges with the ANDES Shell3 and passes the mixed membrane patch test, is derived in ADR 0470
arithmetic 1 and measured by MP1 (`2.1e-14`). Nothing numerical is taken from the sources below.

| Item | Status | Source |
|---|---|---|
| A quadrilateral with drilling rotations made of ANDES triangles | from memory | K. Alvin, H. M. de la Fuente, B. Haugen and C. A. Felippa, Finite Elem. Anal. Des. 12 (1992) 163-187; B. Haugen, PhD thesis, University of Colorado, 1994: NOT read |
| A quadrilateral as the mean of its two diagonal splits (overlapping triangles) | from memory | the early membrane literature, not attributed: NOT read; ADR 0470 arithmetic 1 (edge field), 4 (pure bending exact) |
| 8 x 2 in-plane cantilever strip on quadrilaterals: `u_y / delta = 0.9957912994` against the Timoshenko beam of the F-539 row above | derived | ADR 0470 arithmetic 5, check CQ1 |
## F-418 radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral (ADR 0468)

No source was read for F-418; every item is cited from memory and derived in ADR 0468 (closed forms 1 to 6; numpy checks
`f418-scratch/closed_forms.py` and `estimate.py`). No measured radiation data.

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

## F-551 radiation reactance and surface pressure of a baffled plane (ADR 0488)

No source was read for F-551; every item is cited from memory and derived in ADR 0488 (derivations 1 to 6; numpy checks
`f551-scratch/estimate.py` and `estimate2.py`). The impedance closed form with J1 and H1 is the one verified in ADR 0468. No
measured data.

| Item | Status | Source |
|---|---|---|
| In-plane `int_polygon dS/R = sum_edges d ln((R+ + s+)/(R- + s-))` (signed distance `d` to the edge line, tangent coordinates `s-`, `s+`) | derived | polar integration about the point; the edge-sum form of D. R. Wilton, S. M. Rao, A. W. Glisson, D. H. Schaubert, O. M. Al-Bundak and C. M. Butler, IEEE Trans. Antennas Propag. AP-32 (1984) 276-281: NOT read |
| Square of side `L`: centre `4 L ln(1 + sqrt 2)`, corner `2 L ln(1 + sqrt 2)`; rectangle `a x b` at a corner `a ln((b + sqrt(a^2+b^2))/a) + b ln((a + sqrt(a^2+b^2))/b)`; right triangle of legs `L` at the right angle `sqrt 2 L ln(1 + sqrt 2)` | derived | from the edge sum by hand; numpy and the A1 check to 4.4e-16 |
| Potential of a uniform disk at an in-plane point, `int_disk dS/R = 4 a E(r/a)` | from memory | checked at `r = 0` (`2 pi a`) and `r = a` (`4 a`) by hand |
| Added mass of a baffled piston `8 rho a^3/3` (low-frequency limit of `rho c S H1(2 k a)/(k a)/omega`) | derived | `H1(x) = 2 x^2/(3 pi) + O(x^4)`; Kinsler, Frey, Coppens and Sanders, and Morse and Ingard, chapter 7: NOT read |
| Surface continuity `P(x + h n) = P(x) - i omega rho V_n h + O(h^2)` | derived | the Euler equation at the plane |

## F-430 substructuring: static condensation and the Craig-Bampton reduction (ADR 0469)

No source was read for F-430; the two papers are cited from memory for the names and the published form. Every formula used is
derived in ADR 0469 (block elimination and the Rayleigh-Ritz method); the reference of every check is the full model computed
by the existing solver paths (`solve_static`, `ModalBasis::all_modes`, `modal_analysis`, `DirectHarmonic`).

| Item | Status | Source |
|---|---|---|
| Static condensation `Psi = -K_ii^{-1} K_ib`, `K_hat = K_bb + K_bi Psi`, `f_hat = f_b + Psi^T f_i`, exact recovery `u_i = Psi u_b + K_ii^{-1} f_i` | derived | block Gaussian elimination (Schur complement), ADR 0469 design; R. J. Guyan, AIAA J. 3 (1965) 380: NOT read, from memory |
| Fixed-interface reduction `T_cb = [[I, 0], [Psi, Phi]]`, `K_r = diag(K_hat, Lambda)`, `M_r` with `M_hat = M_bb + M_bi Psi + Psi^T M_ib + Psi^T M_ii Psi`, `M_bq = (M_bi + Psi^T M_ii) Phi` | derived | Rayleigh-Ritz projection on `T_cb`, ADR 0469 design; R. R. Craig and M. C. C. Bampton, AIAA J. 6 (1968) 1313: NOT read, from memory |
| Reduced frequencies bound the full ones from above; nested bases give non-increasing eigenvalues; every fixed-interface mode kept gives every frequency exactly; rigid motions of a free part carried by the constraint modes | derived | min-max principle; congruence by a square nonsingular `T_cb`; `K_q r = 0` gives `r_i = Psi r_b` (ADR 0469 design) |
| Truncation estimate of the bar (16 x 2 x 2 Hex8, two halves, five modes each): first three frequency errors 2.8e-6, 2.8e-6, 3.4e-4, axial 0.10; harmonic error above 1e-2 from 1894 rad/s, 0.85 at the axial resonance | computed | numpy 1-D family chains fitted to the full model's first frequency per family, `f430-scratch/est/estimate.py`, `band.py` (ADR 0469 arithmetic 2, 3); measured 4.5e-6, 5.5e-6, 5.6e-4, 0.0999; 1978 rad/s and 0.94 |
## F-415 the Coriolis term on the modal subspace: split Campbell table and stability (ADR 0472)

No source was read for F-415; every item is cited from memory and derived in ADR 0472 (set-ups and arithmetic 1 to 6; numpy checks
`f415-scratch/pm_probe.py`, `lag_estimate.py`, `flap_estimate.py`). The references of the checks are closed forms and the existing
F-396 solver path (`RotatingPart::modes`, `campbell`).

| Item | Status | Source |
|---|---|---|
| Free vibration in the rotating frame `M u_tt + 2 Omega C u_t + K_eff u = 0`, `C` skew | derived | the relative acceleration `2 Omega a x u_t` in the kinetic energy (ADR 0411 D1, ADR 0472 D1) |
| Spectrum of a gyroscopic problem symmetric about both axes; a root off the imaginary axis implies one with positive real part | derived | `det(lambda^2 M + lambda G + K)` invariant under `lambda -> -lambda` for `G^T = -G`; L. Meirovitch, Methods of Analytical Dynamics (1970): NOT read |
| Point mass on isotropic springs in the rotating frame: `|w0 - Omega|`, `w0 + Omega` | derived | `z = x + i y`: `s^2 + 2 Omega s - (w0^2 - Omega^2) = 0` (ADR 0411 set-up PM) |
| Point mass on unequal springs: unstable exactly for `(w_x^2 - Omega^2)(w_y^2 - Omega^2) < 0`, growth `sqrt(mu+)` of `mu^2 + (w_x^2 + w_y^2 + 2 Omega^2) mu + (w_x^2 - Omega^2)(w_y^2 - Omega^2) = 0` | derived | the characteristic polynomial (ADR 0472 set-up PMA); G. Genta, Dynamics of Rotating Systems (2005), for the name only: NOT read |
| Thin ring spinning about its axis: travelling waves of in-plane mode `n` split by `4 n Omega/(n^2 + 1)` in the rotating frame (whirl numbers `+/- 2 n/(n^2 + 1)`) | derived | inextensional kinetic energy in the rotating frame, Lagrange's equations (ADR 0472 set-up R); G. H. Bryan, Proc. Camb. Phil. Soc. 7 (1890) 101: NOT read |
| Inextensional ring frequency `w_n^2 = (E I/(rho A R^4)) n^2 (n^2 - 1)^2/(n^2 + 1)` | from memory | classical thin-ring result, used only for the speed scale and the foundation (set-up check `w3/w2` within 5 percent) |
| Second-order shift of a gyroscopically coupled mode `omega ~ w_i (1 - 1/2 sum_j G_ij^2/(w_j^2 - w_i^2))` | derived | perturbation of the quadratic problem in modal coordinates (ADR 0472 arithmetic 4) |
| Lag shift of the radial beam by the axial modes `Delta omega = -2 Omega^2 S omega_lag`, `S = 1.4772e-8 s^2` (L = 1 m, 0.05 m square steel); flap shift by torsion `-9.5e-8` at `0.2 w_0` | computed | Euler-Bernoulli cantilever (`beta_1 L = 1.875104068711961`, root of `cos x cosh x = -1`) and fixed-free rod modes, quadrature (ADR 0472 arithmetic 4) |

## F-555 the split Campbell table of a cyclic sector on the modal subspace (ADR 0481)

No source was read for F-555; every item is cited from memory and derived in ADR 0481 (D1, D5). The references of the checks are
the whole-sector whirl solve of ADR 0411 (`RotatingSector::whirl_modes`) and the F-396 sector paths (`RotatingSector::modes`,
`campbell`).

| Item | Status | Source |
|---|---|---|
| A full-rotor matrix commuting with the sector shift enters index `k` as `T_k^T blockdiag(A_s, A_s) T_k`; for the Coriolis matrix the doubled `blockdiag(C_s, C_s)` | derived | ADR 0407 D1, ADR 0411 D5 |
| Projection on expanded harmonic modes with doubled matrices equals the reduced projection: `x^T A_2 y = q^T T_k^T A_2 T_k q'` | derived | ADR 0481 D1 |
| Ritz error of a gyroscopic mode on a subspace, first order: `eta = |e^H Q(omega) e| / (omega |s^H (2 omega M - i G) s|)`, `s` the M-projection of the exact mode, `e = phi - s`, `s^H Q s = e^H Q e` exactly | derived | the Rayleigh functional of the Hermitian family `Q(w) = K - w^2 M + i w G`; P. Lancaster, Lambda-Matrices and Vibrating Systems (Pergamon 1966), K. P. Hadeler, Arch. Rational Mech. Anal. 27 (1968) 306, B. N. Parlett, The Symmetric Eigenvalue Problem (Prentice-Hall 1980): NOT read, from memory (ADR 0481 D5) |
| Leading order of `eta` in modal coordinates: `sum_out 2 Omega^2 |C_ja|^2/(w_j^2 - omega^2)` | derived | ADR 0481 D5, ADR 0472 arithmetic 4 |

## F-569 static correction vectors for the split modes of a sector index (ADR 0482)

No source was read for F-569; every item is cited from memory and derived in ADR 0482 (D1, D2). The references of the checks are
the whole-sector whirl solve of ADR 0411 (`RotatingSector::whirl_modes`) and the F-555 sector split path without the option.

| Item | Status | Source |
|---|---|---|
| Static correction vector `K^-1 G phi_a` of a kept mode carries the leading (static) part of every out-of-basis error component `b_j = -i omega G_ja/(w_j^2 - omega^2)`; the rest is smaller by `omega^2/w_j^2` | derived | modal expansion of `K^-1` (ADR 0482 D1); J. M. Dickens, J. M. Nakagawa, M. J. Wittbrodt, Computers and Structures 62 (1997) 985 (modal truncation augmentation): NOT read, from memory |
| Ritz error from the split mode alone: `eta = |t . b| / (theta |2 theta psi^H M psi + 2 psi_r^T G psi_i|)`, `t` the solution of the complement-projected correction equation `(I - P)^T Q(theta) (I - P) t = -(I - P)^T Q(theta) psi` | derived | Schur complement of the projected problem, first order (ADR 0482 D2, addendum); G. L. G. Sleijpen, H. A. van der Vorst, SIAM J. Matrix Anal. Appl. 17 (1996) 401 (correction equation): NOT read, from memory |
| MINRES with a positive definite preconditioner | from memory | C. C. Paige, M. A. Saunders, SIAM J. Numer. Anal. 12 (1975) 617: NOT read; the recurrence written out in `gyro.rs` |
| On a basis that is not invariant under `K^-1 M` (modes plus correction vectors) the first-order estimate from the Ritz vector falls below the measured error, and ADR 0481's `eta` from the exact mode above it, by up to a factor 2.6 each (DS B4); the ratio of the squared error to their product lies in `[0.898, 1.001]` over the 34 matched modes with `eta > 1e-9` (an observation, not derived) | measured | ADR 0482 addendum 1 (diagnostic `f569-scratch/diag1.log`) |

## F-416 a flexible rotor on bearings in the stationary frame (ADR 0476)

No source was read for F-416; every item is cited from memory and derived in ADR 0476 (set-ups and arithmetic 1 to 5; numpy
checks `f416-scratch/py/estimates.py`, `r_set.py`, `j_set.py`, `o_set.py`, `a_set.py`, `k_set.py`, `u_set.py`; the zero-speed
probe `f416-scratch/probe`). The references of the checks are closed forms, the F-391 beam model (`modal_analysis`) and the
rigid rotor of ADR 0120 (`rotordyn`).

| Item | Status | Source |
|---|---|---|
| Gyroscopic moments of a spinning section `m_x = rho I_p Omega theta_y,t`, `m_y = -rho I_p Omega theta_x,t`; the element matrix `G = [[0, P], [-P, 0]]`, `P = int rho I_p N_psi^T N_psi` | derived | the rate of the angular momentum `rho I_p Omega (theta_y, -theta_x, 1)` (ADR 0476 D2); H. D. Nelson, J. M. McVaugh, J. Eng. Ind. 98 (1976) 593 and M. Lalanne, G. Ferraris, Rotordynamics Prediction in Engineering (1998), for the names only: NOT read |
| Forward and backward by the sense of the orbit: `Im(X conj Y) > 0` forward; the mass-weighted whirl number in `[-1, 1]` | derived | the averaged `x y_t - y x_t` (ADR 0476 D5) |
| Undamped critical speeds `K z = Omega^2 (M - i G) z`, `M - i G` Hermitian | derived | the synchronous root `lambda = i Omega` (ADR 0476 D6) |
| Jeffcott rotor: critical speed `sqrt(k/m)`, unbalance amplitude `e r^2/sqrt((1 - r^2)^2 + (2 zeta r)^2)` | derived | `m w_tt + c w_t + k w = m e Omega^2 e^{i Omega t}` in `w = x + i y`; G. Genta, Dynamics of Rotating Systems (2005), for the name only: NOT read |
| Timoshenko shaft under a central load: `k_s = 1/(L^3/(48 E I) + L/(4 kappa G A))` | derived | bending and shear compliance of a simply supported beam (ADR 0476 set-up J) |
| Cross-coupled stiffness on a damped disc: threshold `k_c = c w_n` exactly | derived | `m s^2 + c s + k - i k_c = 0` at `s = i w` (ADR 0476 set-up X); D. W. Childs, Turbomachinery Rotordynamics (1993), for the name only: NOT read |
| Spinning Rayleigh beam on pins: `E I k^4 - rho A w^2 - rho I k^2 w^2 +/- 2 rho I k^2 Omega w = 0`, `k = n pi/L` | derived | `E I w'''' + rho A w_tt - rho I w_tt'' + i rho I_p Omega w_t'' = 0` (ADR 0476 set-up R) |
| Overhung disc on a cantilever, the two-unknown model: `det [[k11 - m lam^2, k12], [k12, k22 - I_t lam^2 + I_p Omega lam]] = 0` with the Timoshenko tip flexibility `a11 = L^3/(3 E I) + L/(kappa G A)`, `a12 = L^2/(2 E I)`, `a22 = L/(E I)` | derived | ADR 0476 set-up O; M. I. Friswell et al., Dynamics of Rotating Machines (2010), for the name only: NOT read |
| Fujiwara's bound on polynomial roots `2 max |a_{n-k}/a_n|^{1/k}` (with `|a_0/(2 a_n)|^{1/n}`) | from memory | used only for the scan range of the quartic's roots (ADR 0476 amendment 1) |
| Cyclic Jacobi rotation `t = sgn(tau)/(|tau| + sqrt(1 + tau^2))` and its Hermitian form by a phase | from memory | G. H. Golub, C. F. Van Loan, Matrix Computations: NOT read; checked through the closed forms of the checks |

## F-420 stress stiffening of beams and shells (ADR 0473)

No source was read for F-420; the books and the paper below are cited from memory for the names and the published form. Every
matrix is derived in ADR 0473 (arithmetic 1 to 5, D1 to D5) from the second variation of the strain energy under a constant pre-stress
and from the kinetic energy of the motion perpendicular to the axis; the references of the checks are closed forms or the
solid model of the existing solver (Hex20 for E1 to E3).

| Item | Status | Source |
|---|---|---|
| Euler loads `pi^2 E I/L^2` (pinned) and `pi^2 E I/(4 L^2)` (clamped-free) with the shear correction `P = P_E/(1 + P_E/(kappa G A))` (Engesser) | cited | S. P. Timoshenko and J. M. Gere, Theory of Elastic Stability, 2nd ed., McGraw-Hill 1961, ch. 2: NOT read, from memory |
| Taut-beam frequencies `w_n^2 = ((n pi/L)^4 E I + (n pi/L)^2 T)/(rho A)` of a pinned beam under tension `T` | derived | separation of variables of `E I w'''' - T w'' + rho A w_tt = 0` (ADR 0473 arithmetic 3) |
| Beam geometric stiffness `N/(30 L)[36, 3L, -36, 3L; ...]` per bending plane, `N/L` axial and `N I_p/(A L)` torsion terms, with the shear-flexible (Timoshenko) interpolation | derived | second variation of the axial strain under `N` (ADR 0473 arithmetic 1, D1); W. McGuire, R. H. Gallagher and R. D. Ziemian, Matrix Structural Analysis, 2nd ed., Wiley 2000, ch. 9: NOT read, from memory |
| Plate buckling `N_cr = 4 pi^2 D/a^2` (uniaxial) and `2 pi^2 D/a^2` (equal biaxial) of a simply supported square plate, `D = E t^3/(12 (1 - nu^2))` | cited | Timoshenko and Gere, op. cit., ch. 9: NOT read, from memory |
| Shell geometric stiffness `int N_ab (dN_i/dx_a)(dN_j/dx_b) dA` on the three translations, `N = t Lambda S Lambda^T` | derived | ADR 0473 D2 (second variation of the membrane strain) |
| Spin-softening mass `int rho (P u) . (P u) dV`, `P = I - a a^T`, beam section and shell rotary terms; the flap-lag identity `w_flap^2 - w_lag^2 = Omega^2 (1 - f_rot)` | derived | ADR 0473 arithmetic 4, D3 |
| Centrifugal load of a beam: the consistent nodal load of `rho A Omega^2 P (x - p)` and the section moment `-rho Omega^2 (I_z a_2 e_2 x a + I_y a_3 e_3 x a)` | derived | ADR 0473 arithmetic 5, amendment 1 (U6) |
| Stiffening slope of a rotating radial cantilever (Southwell coefficient) `Q = 1.193336` of the first flap mode | computed | numpy Rayleigh quotient, `f420-scratch/probe_rot.py` (ADR 0473 arithmetic 4, after ADR 0403); measured 1.19395 (R3) |

## F-552 residual vectors of the interior load and kept modes by a cutoff (ADR 0474)

No source was read for F-552; the texts below are cited from memory for the names and the published form only. Every formula
used is derived in ADR 0474 (the modal expansion of `K_ii^{-1} f_i` and the Rayleigh-Ritz method); the reference of every check
is the full model computed by the existing solver paths (`solve_static`, `modal_analysis`, `DirectHarmonic`).

| Item | Status | Source |
|---|---|---|
| Residual flexibility of the interior load `r = K_ii^{-1} f_i - Phi Phi^T M_ii K_ii^{-1} f_i = sum_{k > m} phi_k phi_k^T f_i / omega_k^2`, M- and K-orthogonal to the kept modes; with `[Psi, Phi, R]` the static solution of an interior load lies in the trial space (exact statics) | derived | ADR 0474 design; R. R. Craig and A. J. Kurdila, Fundamentals of Structural Dynamics (2006); S. Rubin, AIAA J. 13 (1975) 995; R. H. MacNeal, Computers and Structures 1 (1971) 581: NOT read, from memory |
| M-orthonormalisation by classical Gram-Schmidt made twice, drop below `1e-8` of the M-norm | shared | ADR 0464 D1 (`damped::m_orthonormalise`); B. N. Parlett, The Symmetric Eigenvalue Problem ("twice is enough"): NOT read, from memory |
| Keeping the fixed-interface modes up to about 1.5 to 2 times the top frequency of interest (rule of thumb; ADR 0474 takes 4 times) | cited | Craig and Kurdila (2006): NOT read, from memory; no number taken |
| Response error at a resonance `2 abs(delta) / eta` for a relative frequency error `delta` and a loss factor `eta` | derived | `1 / (omega^2 (1 + 2 delta)(1 + i eta) - omega^2)` against `1 / (i eta omega^2)` (ADR 0474 summary); measured 1.77e-2 at 2921 rad/s for `delta` 1.77e-4 |
| Probe numbers of the bar (cutoff 32964 rad/s, A 5 and B 9 modes, the vector of B): H1 largest 0.494 at 15107 rad/s; to 2000 rad/s 5.4e-4; statics 7.5e-12 | computed | `f552-scratch/probe` on the API of 4abff5fe (ADR 0474 arithmetic 2); measured 0.494, 5.39e-4, 1.5e-12 |
## F-422 J2 plasticity with isotropic and kinematic hardening, radial return and consistent tangent (ADR 0475)

No source was read for F-422; the books are cited from memory for the names and the published form. Every formula used is
derived in ADR 0475 (arithmetic 1 to 6) and checked by a plain numpy prototype (`f422-scratch/probe.py`): the tangent against
central differences, the uniaxial bar, the closed forms of the sphere and the beam.

| Item | Status | Source |
|---|---|---|
| Radial return for von Mises plasticity with linear isotropic and kinematic hardening, `dg = f / (2 mu + (2/3)(H_iso + H_kin))` (exact for linear hardening) | derived | consistency of the yield condition after the step, ADR 0475 arithmetic 1; J. C. Simo and R. L. Taylor, Comput. Meth. Appl. Mech. Engng 48 (1985) 101; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998, chapter 3; E. A. de Souza Neto, D. Peric and D. R. J. Owen, Computational Methods for Plasticity, Wiley, 2008: NOT read, from memory |
| Consistent tangent `C = kappa 1 (x) 1 + 2 mu theta I_dev - 2 mu theta_bar n (x) n`, `theta = 1 - 2 mu dg/|xi_tr|`, `theta_bar = 1/(1 + H/(3 mu)) - (1 - theta)` | derived | linearisation of the return map, ADR 0475 arithmetic 2; prototype against central differences `1.2e-11 .. 6.9e-10`; same books: NOT read |
| Bilinear uniaxial response `sigma_y + (E H/(E + H))(eps - sigma_y/E)`, `H = H_iso + H_kin`; re-yield at `-sigma_max` (isotropic) and `sigma_max - 2 sigma_y` (kinematic) | derived | uniaxial reduction of the yield condition, ADR 0475 arithmetic 2 |
| Partly plastic thick sphere: `p = 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3)`, limit `2 sigma_y ln(b/a)`, elastic shell `sigma_theta - sigma_r = sigma_y (c/r)^3`; `b/a = 2`: `p(1.25 a) = 0.950193 sigma_y`, `p(1.5 a) = 1.196347 sigma_y` | derived | equilibrium, Tresca = von Mises under spherical symmetry, Lame shell, ADR 0475 arithmetic 3; R. Hill, The Mathematical Theory of Plasticity, Oxford, 1950, chapter 5: NOT read, from memory |
| Residual hoop stress at the bore after elastic unloading from `p(1.25 a)`, `b/a = 2`: `sigma_y - (12/7) p = -0.628903 sigma_y` (elastic unloading for `p < (7/6) sigma_y`) | derived | superposition of the Lame field of `-p`, ADR 0475 arithmetic 4 |
| Elastic-perfectly plastic rectangle in pure bending, `M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2` | derived | uniaxial fibres, elastic core `y_e = (h/2) kappa_y/kappa`, ADR 0475 arithmetic 5 |
## F-417 random vibration by modal superposition (ADR 0477)

No source was read for F-417; the books and papers below are cited from memory for the names and the published forms. Every
formula is derived in ADR 0477 (closed forms; arithmetic 1 to 6) and the references of the checks are closed forms or integrals
computed by plain-Python quadrature in `f417-scratch` (`refs.py`, `refs2.py`).

| Item | Status | Source |
|---|---|---|
| Output density `|Y|^2 G` of a linear system under a stationary input, cross-densities `S_ij = sum_ab G_ab Re(Y_ia conj Y_jb)` | cited | S. H. Crandall and W. D. Mark, Random Vibration in Mechanical Systems (Academic Press 1963); D. E. Newland, An Introduction to Random Vibrations, Spectral and Wavelet Analysis (3rd ed., Longman 1993), ch. 7: NOT read, from memory |
| One mode under a flat force spectrum: `m_0 = (pi/2) f_n Q G/k^2`, `Q = 1/(2 zeta)`; `int_0^inf r^2/D dr = int_0^inf dr/D = pi/(4 zeta)`, `nu_0 = f_n` | derived | residues of `D = (r^2 + 2 s r + 1)(r^2 - 2 s r + 1)`, `s = sqrt(1 - zeta^2)`, and the substitution `r = 1/s` (ADR 0477); J. W. Miles, J. Aeronaut. Sci. 21 (1954) 753, for the name only: NOT read |
| One mode under a flat base acceleration: absolute acceleration `m_0 = (pi/2) f_n G (Q + 1/Q)` | derived | the transmissibility `(1 + 2 i zeta r)/(1 - r^2 + 2 i zeta r)` (ADR 0477) |
| Finite-band integrals at `zeta = 0.02`: `I_0(10) = 39.26957079652609`, `I_1 = 38.77258066850795`, `I_2 = 39.16923599186199`, `I_4 = 49.006230409610886`, `I_0 + 4 zeta^2 I_2 = 39.33224157411308` | computed | 4-point Gauss-Legendre on split intervals, `f417-scratch/refs.py`, `refs2.py` |
| Two modes: CQC cross term `rho = 8 zeta^2 (1 + b) b^1.5/((1 - b^2)^2 + 4 zeta^2 b (1 + b)^2)` | derived | the same residues (ADR 0477); P. H. Wirsching, T. L. Paez and K. Ortiz, Random Vibrations: Theory and Practice (Wiley 1995): NOT read, from memory |
| Zero up-crossing rate `sqrt(m_2/m_0)`, peak rate `sqrt(m_4/m_2)` | cited | S. O. Rice, Bell Syst. Tech. J. 23 (1944) 282 and 24 (1945) 46: NOT read, from memory (as F-413) |
| Root-mean-square von Mises stress `sqrt(tr(A C))`, `A` the quadratic form of `sigma_vm^2`, `C` the stress covariance | derived | first principles (ADR 0477 D5); D. J. Segalman, C. W. G. Fulcher, G. M. Reese and R. V. Field, J. Vib. Acoust. 122 (2000) 42, for the definition: NOT read, from memory |
## F-432 stress recovery by element patches and the energy-norm error estimate (ADR 0478)

No source was read for F-432; every method is cited from memory and derived in ADR 0478 as a discrete least-squares fit and an
energy integral. The checks use closed forms (polynomial fields), the manufactured solution of F-378 (true error known) and
Kirsch's hole as recorded above (F-413).

| Item | Status | Source |
|---|---|---|
| Superconvergent patch recovery: least-squares fit of a complete polynomial to the stresses at the sampling points of the elements around a vertex node, nodal values from the fit; boundary nodes from an interior patch | from memory | Zienkiewicz and Zhu, "The superconvergent patch recovery and a posteriori error estimates. Part 1: The recovery technique", Int. J. Numer. Meth. Engng 33 (1992) 1331-1364: NOT read; ADR 0478 D1 to D3 (the nearest-centre rule of D3 is this repository's) |
| Error estimate `eta_e^2 = int (sigma* - sigma_h) . D^{-1} (sigma* - sigma_h) dV`, relative estimate `eta/(||u_h||^2 + eta^2)^{1/2}` | from memory | Zienkiewicz and Zhu, "A simple error estimator and adaptive procedure for practical engineering analysis", Int. J. Numer. Meth. Engng 24 (1987) 337-357; Part 2 of the 1992 paper, ibid. 1365-1382: NOT read; ADR 0478 D4 |
| Effectivity index `theta = eta/||u - u_h||_E`, asymptotic exactness when the recovered stress converges faster than the raw one | from memory | Ainsworth and Oden, A Posteriori Error Estimation in Finite Element Analysis, Wiley (2000), the chapter on recovery-based estimators: NOT read; measured in ADR 0478 (M1, M2) |
| Optimal stress points: the centroid of the trilinear hexahedron, the 2 x 2 x 2 Gauss points of the quadratic hexahedron | from memory | Barlow, "Optimal stress locations in finite element models", Int. J. Numer. Meth. Engng 10 (1976) 243-251: NOT read; the 1-D argument of ADR 0478 arithmetic 3 |
| One-dimensional effectivities of the rule (linear `1.080, 1.025, 1.007` and quadratic `0.854, 0.971, 0.995` at `n = 4, 8, 16`) | computed | numpy, `f432-scratch/probe_1d.py`, `probe_1d_quad.py` (ADR 0478 arithmetic 1, 2) |
## F-423 nonlinear transient dynamics, Newton inside the Newmark and generalised-alpha step (ADR 0479)

No source was read for F-423; the books and papers below are cited from memory for the names and the published forms. Every
formula used is derived in ADR 0479 (arithmetic 1 to 8, D1) and the reference values are closed forms evaluated by plain
Python in `f423-scratch` (`reg_arith.py`, log `reg_arith.log`); the probes of the registration are two-node truss prototypes
(`probe_truss.py`, `probe2.py` to `probe5.py`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Step with the internal force averaged at its end values, `M a_m + C v_f + (1 - alpha_f) f_int(u_{n+1}) + alpha_f f_int,n = (1 - alpha_f) f_{n+1} + alpha_f f_n`, unknown `u_{n+1}`, `a_{n+1} = (u_{n+1} - u~)/(beta h^2)`; `rho_inf = 1` is the trapezoidal rule | derived | ADR 0479 D1 and arithmetic 7; J. Chung and G. M. Hulbert, J. Appl. Mech. 60 (1993) 371, N. M. Newmark, J. Eng. Mech. Div. ASCE 85 (1959) 67 (as ADR 0389); D. Kuhl and M. A. Crisfield, IJNME 45 (1999) 569, S. Erlicher, L. Bonaventura and O. S. Bursi, Comput. Mech. 28 (2002) 83, for the averaged form: NOT read, from memory |
| Compound pendulum released from the horizontal: `T = 2 pi sqrt(2 L/(3 g)) (2/pi) K(sin 45 degrees)`, `K(1/sqrt 2) = 1.854074677301372`, factor `1.180340599016096`; `L = 1 m`, `g = 9.81`: `T = 1.933334854373 s` | derived, computed | energy integral of `I = m L^2/3`, torque `m g (L/2) sin theta`; `K` by the arithmetic-geometric mean and a 20000-point midpoint quadrature (`reg_arith.log`); L. D. Landau and E. M. Lifshitz, Mechanics, section 11, for the elliptic form: NOT read, from memory |
| Large axial oscillation of one St. Venant-Kirchhoff bar element, `nu = 0`, from `lambda = 1.2`: `T/T_lin = (1/pi) int_{-pi/2}^{pi/2} dphi/sqrt(1 + 0.44 sin phi) = 1.0407443288903`, `T_lin = 2 pi sqrt(rho L^2/(3 E))`, `lambda_min = sqrt(0.56)` | derived, computed | energy `(rho A L/6) qdot^2 + (E A L/8)(lambda^2 - 1)^2`, the substitution `lambda^2 - 1 = 0.44 sin phi`; midpoint rule (64 to 4096 points) and the binomial series agree to `3e-15` (`reg_arith.log`) |
| Conservation of `P = sum p_a` and `J = sum x_a x p_a`, `p = M v`, for the consistent mass and an objective internal force; the trapezoidal rule keeps `P` and, in general, not `J` | derived, cited | ADR 0479 arithmetic 3; J. C. Simo and N. Tarnow, ZAMP 43 (1992) 757, J. C. Simo, N. Tarnow and K. K. Wong, CMAME 100 (1992) 63, for the non-conservation and the instability of the trapezoidal rule in nonlinear elastodynamics: NOT read, from memory |
| Probes (truss): pendulum `dt = 0.005 s`, `E = 2e9`: `T/T_0 = 1.180504`, energy `2.76e-4` of `m g L/2`; spinning truss from the steady spin, Newmark: `|dJ|/|J| = 7.2e-12`, unstretched start, generalised-alpha 0.9: `1.6e-3` | computed | `probe4.log`, `probe5.log` (ADR 0479 arithmetic 2, 4) |
## F-424 follower pressure, arc-length continuation and modes about a nonlinear state (ADR 0480)

No source was read for F-424; the books and papers below are cited from memory for the names and the published forms. Every
formula used is derived in ADR 0480 (arithmetic 1 to 7); the references of the checks are closed forms, a thick-ring ODE
integrated in plain Python (`f424-scratch/ring_ode.py`) and a probe of the solid two-bar truss (`f424-scratch/zz_probe_f424.rs`,
`probe.log`).

| Item | Status | Source |
|---|---|---|
| Follower pressure on the current face `f_a = -p int N_a (x_u x x_v) du dv` and its load stiffness `K_p = -p int N_a [N_b,u (e_j x x_v) + N_b,v (x_u x e_j)]`; the antisymmetric part an edge term of the loaded surface | derived | linearisation of the pressure's virtual work, ADR 0480 arithmetic 1 and 3; H. D. Hibbitt, "Some follower forces and load stiffness", Int. J. Numer. Meth. Engng 14 (1979) 937 (the reference itself uncertain); J. Bonet and R. D. Wood, Nonlinear Continuum Mechanics for Finite Element Analysis, Cambridge 1997, chapter 6: NOT read, from memory |
| Thin ring under follower pressure, SVK, `nu = 0`: `p = E t (lambda^2 - 1)/(2 R)`; dead pressure `p = lambda E t (lambda^2 - 1)/(2 R)` | derived | hoop equilibrium on the deformed radius, ADR 0480 arithmetic 2 |
| Thick ring, plane, SVK, `nu = 0`: `d(R P_rR)/dR = P_tt`, bore traction `-p r_i/R_i` (follower) or `-p` (dead); thin-law departure `+1.01e-2` at `lambda -> 1`, `+0.67e-2` at `lambda = 1.5`, `t/R = 0.02` | computed | RK4, 2000 steps, `f424-scratch/ring_ode.py` (ADR 0480 arithmetic 2) |
| Ring buckling under external pressure: `p_cr = (n^2 - 1) E I/R^3` (hydrostatic, follower), `n^2 E I/R^3` (fixed direction); `3` and `4 E I/R^3` at `n = 2` | derived | inextensional ring energy to second order, ADR 0480 arithmetic 3; S. P. Timoshenko and J. M. Gere, Theory of Elastic Stability, McGraw-Hill 1961, chapter 7: NOT read, from memory |
| Two-bar truss of Green-Lagrange bars with SVK stress, exact at any depth: `P = E A (h - w)(2 h w - w^2)/L0^3`, limit points `w = h (1 -+ 1/sqrt 3)`, `P_max = 2 E A h^3/(3 sqrt 3 L0^3)` | derived | ADR 0480 arithmetic 4; M. A. Crisfield, Non-linear Finite Element Analysis of Solids and Structures, vol. 1, Wiley 1991, chapter 1: NOT read, from memory |
| Bipyramidal Tet4 bar: the homogeneous state is an exact FE equilibrium with effective area `A = s^2/3`; probe residuals `2.1e-12 .. 7.2e-12 P_max` at `w/h = 0.05, 0.4226, 1, 1.5, 2.2` | derived, computed | ADR 0480 arithmetic 5; `f424-scratch/zz_probe_f424.rs`, `probe.log` |
| Spherical arc-length constraint `|Du|^2 + c Dlambda^2 = Ds^2`, the quadratic for the load-factor correction, the root closer in angle to the previous increment | cited | M. A. Crisfield, "A fast incremental/iterative solution procedure that handles snap-through", Comput. Struct. 13 (1981) 55; E. Riks, "An incremental approach to the solution of snapping and buckling problems", Int. J. Solids Struct. 15 (1979) 529: NOT read, from memory (ADR 0480 D4) |

## F-556 the Shell3 rest stiffness of the twisted strip against Hex20 (ADR 0483)

No source was read for F-556; the papers below are cited from memory for the names. The torsion of a pretwisted strip is
derived in ADR 0483 (arithmetic 2); the reference of every comparison is the Hex20 strip of ADR 0473 run by the existing
solver, and the base numbers come from the F-420 run log.

| Item | Status | Source |
|---|---|---|
| Torsional stiffness of a pretwisted thin strip `G c t^3/3 + E t c^5 theta'^2/180` (the fibre strain `theta' phi' s^2`, free mean) | derived | ADR 0483 arithmetic 2; A. Rosen, "Structural and dynamic behavior of pretwisted rods and beams", Appl. Mech. Rev. 44 (1991) 483: NOT read, from memory |
| The base rest frequencies of the third track: Hex20 442.797470 Hz, Shell4 445.161813 Hz, Shell3 460.730207 Hz | computed | ADR 0473 E1/E2 run log (`f420-scratch/run_e1.txt`); reproduced to 8.3e-10 (C1) |
| Constant-strain triangle: parasitic in-plane shear when the strain varies across the element (in-plane bending) | cited | C. A. Felippa, "A study of optimal membrane triangles with drilling freedoms", Comput. Methods Appl. Mech. Engrg. 192 (2003) 2125: NOT read, from memory (as F-539) |
| ANDES optimal membrane triangle, Allman's triangle | cited | Felippa 2003, op. cit.; D. J. Allman, Comput. Struct. 19 (1984) 1: NOT read, from memory (implemented by ADR 0466) |
| MITC3 transverse shear, the stabilisation `t^2/(t^2 + alpha h^2)` | cited | P. S. Lee and K. J. Bathe, Comput. Struct. 82 (2004) 945; M. Lyly, R. Stenberg and T. Vihinen, Comput. Mech. 13 (1993) 92: NOT read, from memory (implemented by ADR 0454) |
| Measured: the CST membrane-energy excess `e_m = 9.16e-2` at 12 x 36 on the coupled torsion field, `O(h^2)` (ratios 0.257, 0.264 per halving); the ANDES Shell3 within 3.3e-4 of Hex20 at rest and 2.98e-3 over the Campbell table | computed | ADR 0483 addenda 1 and 2 |

## F-410 linear viscoelasticity: Prony series, complex modulus, time-temperature shift (ADR 0485)

No source was read for F-410; the books and papers below are cited from memory for the names and the published forms. Every
formula used is derived in ADR 0485 (arithmetic 1 to 8) and checked by a plain numpy prototype of the uniaxial law
(`f410-scratch/probe.py`, log `probe.log`), no rcfd code. No polymer data is used: every modulus, relaxation time and shift
constant of the checks is an input (decision A1 of ADR 0310).

| Item | Status | Source |
|---|---|---|
| Generalised Maxwell solid on an isotropic base, relative shear and bulk weights per branch; strain step gives `G(t) = G_0 (g_inf + sum g_i exp(-t/tau_i))`, `K(t)` alike, and for equal weights `E(t) = E_inf + sum E_i exp(-t/tau_i)` | derived | hereditary integral of the branch stresses, ADR 0485 arithmetic 1; J. D. Ferry, Viscoelastic Properties of Polymers, 3rd ed., Wiley, 1980; N. W. Tschoegl, The Phenomenological Theory of Linear Viscoelastic Behavior, Springer, 1989: NOT read, from memory |
| Recursive update `h^{n+1} = e h^n + gamma P (sigma_0^{n+1} - sigma_0^n)`, `e = exp(-dxi/tau)`, `gamma = (1 - e) tau/dxi`, exact for a strain linear in the step; tangent `2 G_t I_dev + K_t 1 (x) 1`, `G_t = G_0 (1 - sum g_i (1 - gamma_i))` | derived | ADR 0485 arithmetic 1; R. L. Taylor, K. S. Pister and G. L. Goudreau, Int. J. Numer. Meth. Engng 2 (1970) 45; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998, chapter 10 (the chapter number not checked): NOT read, from memory |
| Creep of the standard solid under constant stress, `eps(t) = sigma0 (1/E_inf - (1/E_inf - 1/E_0) exp(-t/tau_c))`, `tau_c = tau E_0/E_inf` | derived | the ordinary differential equation of the standard solid; probe: the recursion `1.208e-5` at 20 steps per `tau`, order `2.0` |
| Complex modulus `G*(omega) = G_0 (g_inf + sum g_i i omega tau_i/(1 + i omega tau_i))`, `E* = 9 K* G*/(3 K* + G*)`; dissipation per cycle and volume `pi eps0^2 E''` | derived | steady state of the hereditary law, ADR 0485 arithmetic 2 |
| Logarithmic decrement of a mode of the standard solid, `delta = pi eta/(1 - (omega/2)(dE'/d omega)/E')` to first order in the loss (`pi E''/E'` without the dispersion term); at `omega tau = 1`, `w = 2/21`: `eta = 0.05`, `delta/(pi eta) - 1 = 2.56e-2` (first order), `2.525e-2` (exact root) | derived, computed | ADR 0485 arithmetic 3; root of `s^2 + lambda (w_inf + w s tau/(1 + s tau)) = 0` by Newton (probe and test) |
| Time-temperature shift: WLF `log10 a_T = -c1 (T - T_ref)/(c2 + T - T_ref)`, Arrhenius `ln a_T = (E_a/R)(1/T - 1/T_ref)`, `R = 8.314462618 J/(mol K)` (CODATA 2018, exact); the "universal" WLF constants `c1 = 17.44`, `c2 = 51.6 K` used as inputs only | cited | M. L. Williams, R. F. Landel and J. D. Ferry, J. Am. Chem. Soc. 77 (1955) 3701; Ferry (1980), chapter 11 (the chapter number not checked): NOT read, from memory; the CODATA 2018 value of `R` from memory |
## F-577 the viscoelastic material in the direct harmonic solve (ADR 0491)

No source was read for F-577. Every formula used is derived in ADR 0491 (arithmetic 1 to 9) or re-used from ADR 0485 and ADR
0448; the references of the checks are closed forms and the 1-D chain of linear elements solved in the test. The material
constants are inputs (the values of ADR 0485 V1), not data.

| Item | Status | Source |
|---|---|---|
| Steady harmonic stress of the isotropic viscoelastic solid `sigma* = 2 G* dev eps* + K* tr eps* 1`, so the element stiffness is `(G*/G_0) K^G + (K*/K_0) K^K` with `K^G`, `K^K` the stiffness of `2 G_0 I_dev` and `K_0 1 (x) 1` (exact, the stiffness being linear in the elasticity matrix); for equal weights `E*/E_0` times the base stiffness | derived | the correspondence principle of linear viscoelasticity, from the steady state of the hereditary law (ADR 0485 arithmetic 2, ADR 0491 arithmetic 1); textbook statement (Tschoegl 1989, Ferry 1980): NOT read, from memory |
| Clamped rod with an end force `F`: `u(L) = F tan(kL)/(A Mod* k)`, `k = omega sqrt(rho/Mod*)`; the rod with the lateral motion held is in uniaxial strain, modulus `K* + 4 G*/3` (`= E*` for `nu = 0` and equal weights), the shear rod `G*` | derived | ADR 0491 arithmetic 2 |
| Discretisation error of the linear element with consistent mass near the first resonance, `2 (kh)^2/(24 eta)` at the peak (`kh = (pi/2)/n`); `2.47e-3` (rod A), `2.65e-3` (B), `1.60e-3` (C) at 40 elements, second order | derived, computed | ADR 0491 arithmetic 4; probe `f577-scratch/probe.py` (numpy, no rcfd code), reproduced by the test |

## F-573 follower pressure on shells (ADR 0487)

No source was read for F-573. Every formula used is derived in ADR 0487 (arithmetic 1 to 8) or re-used from ADR 0480 (cited
there from memory, NOT read); the references of the checks are closed forms and the dead `ShellPressure` of the repository.

| Item | Status | Source |
|---|---|---|
| Follower shell pressure on the interpolated current mid-surface `f_a = +p int N_a (x_u x x_v) du dv` (translations, no moments) and its load stiffness `K_(ai)(bj) = p int N_a [N_b,u (e_j x x_v) + N_b,v (x_u x e_j)]_i`; the solid face arithmetic of ADR 0480 with the pressure negated | derived | virtual work of the pressure on the current surface, ADR 0487 arithmetic 1 and 2 (the linearisation as in ADR 0480 arithmetic 1) |
| Vector area of a bilinear quad `(1/2)(x_2 - x_0) x (x_3 - x_1)`, equal to that of its projected facet: the resultants of the follower and the dead shell pressure agree on a warped Shell4 | derived | ADR 0487 arithmetic 5 |
| Cylinder of `N` facets at the stretch `lambda`: `sum f . e_r = p N lambda R L sin(2 pi/N)` (follower), `p N R L sin(2 pi/N)` (dead); the hoop tension `p lambda R cos(pi/N)` and, with an SVK membrane of `nu = 0`, `p = E t (lambda^2 - 1)/(2 R cos(pi/N))` | derived | node balance of the facet polygon, ADR 0487 arithmetic 6 |
| Ring buckling of shells under external pressure: `3 E I/R^3` with the load stiffness (hydrostatic), `4 E I/R^3` without (fixed-direction forces), `n = 2` | derived | ADR 0480 arithmetic 3 re-used (ADR 0487 arithmetic 7) |
