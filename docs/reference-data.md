# Reference data audit (wave 6a)

Every benchmark value or published constant that earlier waves typed from memory, checked against a
citable public source. "Verified" means the value in the repo agrees with the source; "secondary" means
the source is a reputable paper, vendor guide or repository that reproduces the primary table, not the
original printed table (the original journal papers were not accessible online). Nothing was loosened:
no tolerance was changed, and no benchmark number in a data file or assertion turned out to be wrong.

Summary (counted by table row): about 34 verified, 3 mismatched, about 8 unverifiable or not re-checked.
The mismatches are one sign error in a test helper (Taylor-Green initial pressure) and two citation or
label errors (Darwish-Moukalled journal; flat-plate "Prandtl-Schlichting" label); all were corrected in
comments, notes or the test helper, and every affected test still passes.

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
| Reference list entries Roe 1981 JCP 43, Harten 1983 JCP 49, Liou 1996 JCP 129, Shu-Osher 1988 JCP 77, Toro et al. 1994 | bibliographic recollection, not re-fetched | not re-checked |

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
| Realizable: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(eta+5)), Cmu = 1/(A0 + As U* k/eps), As = sqrt(6) cos(phi) | Fluent theory guide an unofficial copy (not linked); OpenFOAM `realizableKE` class docs (default A0 4.0, C2 1.9, sigma 1.0/1.2) | verified. Note A0 = 4.04 in Fluent (used by rcfd, `kepsilon.rs`) but 4.0 in OpenFOAM and, per one forum post, in Shih et al.; the original paper's printed value could not be fetched. Flagged in the source comment. |
| Wall function kappa = 0.4187, E = 9.793; rounded to 0.41 / 9.8 by rcfd | ANSYS Fluent docs (Launder-Spalding standard wall functions) | verified (rounding disclosed in code) |
| Log law kappa = 0.41, B = 5.2 (Pope 2000) | problem set based on Pope (perso.ens-lyon.fr), Abe et al. 2001; Wikipedia gives kappa 0.41, C+ 5.0 | verified, within the usual 5 percent spread |
| Reichardt (1951): u+ = ln(1 + 0.41 y+)/0.41 + 7.8[1 - e^{-y+/11} - (y+/11) e^{-y+/3}] | CFD-Online wiki "Reichardt profile" (via search result); numerical cross-check against MKM DNS | verified, and within 0.7, 0.8, 2.3, 2.3, 1.4 percent of DNS at y+ = 20, 30, 50, 100, 150 |
| Inlet relations k = 1.5 (I U)^2, eps = Cmu^0.75 k^1.5 / l, l = 0.07 L (`rcfd-api/src/turb.rs`) | CFD-Online/CFX forum guidance; l factor 0.07 for mixing-length codes | verified (k and l); eps = Cmu k^2/(nu * ratio) is the definition nu_t = Cmu k^2/eps, no external source needed |

## k-omega SST and wall treatment (`crates/rcfd-turb/src/sst.rs`, `wall.rs`)

| Value | Source | Result |
|---|---|---|
| beta* 0.09, a1 0.31, sigma_k1 0.85, sigma_w1 0.5, beta1 0.075, gamma1 5/9; sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828, gamma2 0.44; P limited to 10 beta* rho omega k; CDkw = max(2 rho sigma_w2/omega dk.dw, 1e-10); F1 = tanh(arg1^4), F2 = tanh(arg2^2); nu_t = a1 k/max(a1 omega, S F2), S = sqrt(2 SijSij); wall omega = 10 * 6 nu/(beta1 d1^2) | NASA Turbulence Modeling Resource, "The Menter Shear Stress Transport Turbulence Model" (tmbwg.github.io/turbmodels/sst.html; the turbmodels.larc.nasa.gov address redirects); original: Menter, Kuntz and Langtry (2003) | verified against the TMR page by web fetch (2003 formulation, with the TMR production limiter) |
| TMR flat plate set-up: Re_L = 5e6, M = 0.2, k_inf = 9e-9 a^2 (= 2.25e-7 U^2), omega_inf = 125 U/L (nu_t/nu = 0.009); CFL3D SST Cf(x = 0.97) = 0.00268299 (273 x 193 grid; the finest-grid value is not printed on the page) | tmbwg.github.io/turbmodels/flatplate_sst.html | verified; used in `tests/flat_plate_sst.rs`; rcfd gives 0.002663 (-0.7 percent) |
| Spalding (1961) law of the wall: y+ = u+ + exp(-kappa B)[exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6] | Spalding, J. Appl. Mech. 28 (1961) 455-458, from memory of the standard form (checked by the `wall.rs` unit test against the log law) | from memory, not re-fetched |
| Compound viscous/log epsilon of the smooth k-epsilon wall treatment, eps = sqrt(eps_vis^2 + eps_log^2) | Popovac and Hanjalic, Flow Turb. Combust. 78 (2007) 177-202, from memory; the blended omega of the SST wall cell follows Menter and Esch (2001) in spirit | from memory, not re-fetched (the formulas are rcfd's own implementation of the idea) |
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
| Expansion ratio 1.125, inlet channel 8h | not stated on either fetched page; figure only | unverifiable online here |
| "5.5 to 7 step heights for RANS" scatter | no source found | unverifiable |

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
| Issa 1986 JCP 62; Choi 1999 Numer. Heat Transfer A 36; Ericson RTCD 5.1.5 | not re-checked |

## Not verifiable online

- Original printed tables of Ghia 1982 and de Vahl Davis 1983 (only secondary reproductions were reachable); the
  Ghia Re 100/400 values at y = 0.9766 rest on a single source.
- Backward-step geometry (expansion ratio 1.125) and the "5.5 to 7" scatter statement.
- Williamson 1989 / Henderson 1995 St values and the Williamson fit coefficients.
- Toro Table 4.3 / NACA 1135 attributions.
- Original value of A0 in Shih et al. (4.04 vs 4.0).
- Roe/Harten/Liou/Shu-Osher/Issa/Choi bibliographic details (not re-fetched).

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
