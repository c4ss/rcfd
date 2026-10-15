# Reference-data verification report (2026-10-15)

Scope: every constant or reference value in `docs/reference-data.md`, `docs/validation-data.md` and ADRs 0005 to 0015
that was marked unverified, from memory, or "retrieved, not independently re-read". No code, test, tolerance or ADR
criterion was changed. Method: web search and page fetch; where a source was a PDF it was downloaded and read as raw text
(`pdftotext`), and the NIST WebBook tables were pulled as raw HTML, not through a page summariser. Where only a
search-engine snippet or a vendor page supported a value this is stated. "Confirmed" means the number in the code or
docs agrees with the cited source; it does not mean the original printed paper was read unless the source column says so.
"Unverified" means no source reached stated the value.

Provenance of URLs: all URLs below were seen in this pass either as fetched pages or as search-result links; for the
ones marked "via search" only the search summary/snippet was read, not the page. Raw reads (pdftotext or raw HTML via
curl): NASA NTRS PDFs, arXiv PDFs, Bari ROBER report, NIST WebBook tables. Others were read through a page summariser.

Verdict key: confirmed / differs / unverified. `secondary` = a reputable page or paper reproducing the primary value.

## 1. Results table

### 1.1 Turbulence models and wall treatment

| Item | Where used | Code value | Authoritative value | Source URL | Verdict |
|---|---|---|---|---|---|
| Kader blend, Gamma = -0.01 y+^4/(1+5 y+), u+ = e^Gamma u+_lam + e^(1/Gamma) u+_turb | `rcfd-turb/src/wall.rs:305-325`, ADR 0015 | a = 0.01, b = 5, negative Gamma | Fluent theory guide: Gamma = -a y+^4/(1+b y+), a = 0.01, b = 5, same exponents. Kader original (Xia et al.) writes the positive Gamma = 1e-2 (Pr y+)^4/(1+5 Pr^3 y+) with e^-Gamma and e^-1/Gamma: same function with the sign moved into the exponent (Pr = 1). | (unofficial copy, not linked) ; https://www.secs.oakland.edu/~wang/doc/publication/Xia-Castillo-Guillmo-ASME%20J%20of%20Heat%20Transfer.pdf (via search) | confirmed (secondary; IJHMT 24(9) 1541-1544 citation confirmed via Crossref) |
| Two-layer A_mu | `rcfd-turb/src/kepsilon.rs:93` | 70.0 | A_mu = 70 | an unofficial copy (not linked) (above) | confirmed (secondary) |
| Two-layer A_eps = 2 c_l, c_l = kappa Cmu^-3/4 | `kepsilon.rs:101-102` | 2 c_l | A_eps = 2 C_l*, C_l* = kappa Cmu^-3/4 | an unofficial copy (not linked) | confirmed (secondary) |
| Two-layer switch Re_y* | `kepsilon.rs:92` | 200 | Re_y* = 200 | an unofficial copy (not linked) | confirmed (secondary) |
| Blend width, lambda = (1+tanh((Re_y-Re_y*)/A))/2, A = dRe/atanh(0.98) | `kepsilon.rs:94,111` | dRe = 0.1 Re_y*, atanh(0.98) | form and atanh(0.98) confirmed; "dRe between 5 and 20 percent of Re_y*", so 0.1 is a choice inside the stated range | an unofficial copy (not linked) | confirmed (form); 0.1 is an rcfd choice |
| Attribution to Chen and Patel 1988 / Jongen 1998 | comments, ADR 0015 | n/a | Chen and Patel, AIAA J. 26(6) 641-648 (1988) exists (Crossref). The constants were seen only on the Fluent page, not in the paper. | Crossref REST API bibliographic query (exact query URL not recorded) | unverified (attribution of the constants to the original paper) |
| Scalable wall function (Grotjans and Menter 1998) | `wall.rs:246-262`, ADR 0015 | y* limited to the viscous/log intersection, y_lam = 11.53 (kappa 0.41, E 9.8) | Concept confirmed: CFX theory guide cites Grotjans and Menter and limits y* at 11.06 (where the linear and log laws meet); Fluent limits at 11.225. The paper's own equations (epsilon boundary condition, shear) were not reached. | vendor online help (login required; not linked) ; an unofficial copy (not linked) | partly confirmed (concept); details unverified; numeric limit is a property of rcfd's own constants, see 2.2 |
| BSL sigma_k1 = 0.5, sigma_w1 0.5, beta1 0.0750, sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828, beta* 0.09, kappa 0.41 | `sst.rs:216`, ADR 0015 | [0.5, 1.0], TMR values | Menter 1993 (AIAA 93-2906), "Set 1 (Wilcox)": sigma_k1 0.5, sigma_w1 0.5, beta1 0.0750, gamma1 = beta1/beta* - sigma_w1 kappa^2/sqrt(beta*); Set 2: sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828. | https://ntrs.nasa.gov/api/citations/19960044572/downloads/19960044572.pdf (read as text) | confirmed (primary, NASA copy of the paper) |
| BSL gamma1, gamma2 | `sst.rs:73-75,461-462` | 0.5532, 0.4403 | computed from the Menter relation: 0.55317, 0.44035 | same, plus own arithmetic | confirmed |
| BSL eddy viscosity nu_t = k/omega | `sst.rs:224` | k/omega | BSL has no limiter; limiter introduced for SST in the same paper | same | confirmed |
| SST (1993/1994) sigma_k1 = 0.85, a1 = 0.31 | `model.rs:270`, `sst.rs` | 0.85, 0.31 | Menter 1993 Set 1 (SST inner): sigma_k1 0.85, sigma_w1 0.5, beta1 0.0750, a1 0.31 | same | confirmed |
| SST-2003 gamma 5/9, 0.44; TMR form | `sst.rs` | 5/9, 0.44 | TMR SST page lists gamma1 = 5/9, gamma2 = 0.44, sigma_k1 0.85 | https://tmbwg.github.io/turbmodels/sst.html | confirmed |
| SA constants: sigma 2/3, cb1 0.1355, cb2 0.622, kappa 0.41, cw2 0.3, cw3 2, cv1 7.1, ct3 1.2, ct4 0.5, cn1 16, c2 0.7, c3 0.9 | `rcfd-turb/src/sa.rs:63-72` | same | same, as printed; Allmaras, Johnson, Spalart ICCFD7-1902 (2012) reference confirmed | https://tmbwg.github.io/turbmodels/spalart.html | confirmed (TMR; the 1994 paper itself not read) |
| Realizable k-epsilon A0 | `kepsilon.rs:448` (and header comment :23) | 4.04 | A0 = 4.0 (Shih et al., NASA TM-106721, text "This leads to A0 = 4.0" and Table 2). Also confirmed there: C2 1.9, sigma_k 1.0, sigma_eps 1.2, C1 = max(0.43, eta/(5+eta)). | https://ntrs.nasa.gov/api/citations/19950005029/downloads/19950005029.pdf | differs |
| Spalding (1961) law of the wall, cubic truncation, E = exp(kappa B) | `wall.rs` `SpaldingLaw` | y+ = u+ + exp(-kappa B)[e^(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6] | Same form with cubic truncation in libWMLES / OpenFOAM; arXiv review of single-formula laws prints one more (quartic) term. Spalding paper itself (J. Appl. Mech. 28, 455-458, 1961) citation confirmed by Crossref. | https://libwmles.readthedocs.io/en/develop/api/class_a00247.html ; https://arxiv.org/pdf/2104.09054 | confirmed (secondary); truncation order of the original not checked |
| Compound wall treatment of Popovac and Hanjalic | `wall.rs` (rcfd's own implementation of the idea) | eps = sqrt(eps_vis^2 + eps_log^2) | Paper exists: Flow Turb. Combust. 78(2), 2007 (Crossref). Formula not seen. | Crossref | unverified (formula); bibliography confirmed |
| Cebeci-Bradshaw transitional shift: thresholds 2.25 and 90, 87.75, 0.4258, 0.811, fully rough ln(1+Cs ks+)/kappa | `wall.rs:172-218` | exact forms (pi/2)/ln 40 = 0.42582 and ln 2.25 = 0.81093 replace rounded 0.4258, 0.811; transitional shift clamped at 0 | OpenFOAM v6 `nutkRoughWallFunction::fnRough`: KsPlus < 90: pow((KsPlus-2.25)/87.75 + Cs KsPlus, sin(0.4258 (ln KsPlus - 0.811))); else 1 + Cs KsPlus; applied as E' = E/fnRough for KsPlus > 2.25, so Delta B = ln(fn)/kappa, which is the rcfd form. Fluent theory guide says the three regimes follow Cebeci and Bradshaw (equations are images). | https://cpp.openfoam.org/v6/nutkRoughWallFunctionFvPatchScalarField_8C_source.html ; vendor online help (login required; not linked) | confirmed (constants, secondary); the 1977 book was not read; the clamp at 0 and the exact-constant substitution are disclosed rcfd deviations |
| Nikuradse fully rough intercept 8.5 and C_s = exp(-kappa(8.5-B)) | `wall.rs:190`, `wall_options.rs`, schema text | 8.5; derived C_s = 0.3004 (kappa 0.41, E 9.8) | Fully rough intercept B_FR = 8.5 (several recent papers; one fit gives 8.48); the derivation of C_s is rcfd's own; the "textbook C_s = 0.5" statement in ADR 0015 was not found in any fetched source | https://arxiv.org/pdf/1812.04141 ; https://arxiv.org/pdf/2603.24288 (via search) | confirmed (8.5); unverified ("textbook 0.5") |
| Prandtl-Schlichting C_F = 0.455/(log10 Re_L)^2.58 | `tests/common/plate.rs:257` | as written, used as the plate-average coefficient | Same expression, "friction drag coefficient", valid to Re_L about 1e9 (Univ. Iowa notes). CFD-Online lists it among local coefficients, so sources disagree on local versus average; rcfd uses it as an average, consistent with the Iowa text | https://user.engineering.uiowa.edu/~me_160/2023/Lecture_notes/Chapter7/Chapter%207.3%20Turbulent%20BL.pdf ; https://cfd-online.com/Wiki/Skin_friction_coefficient | confirmed (secondary); local/average ambiguity in the literature |
| Fully rough local Cf = (2.87 + 1.58 log10(x/ks))^-2.5 | `plate.rs:262`, near_wall_catalogue C3 | as written | Jaffer eq. (3), attributed to Prandtl and Schlichting, Boundary-layer theory; companion average (1.89 + 1.62 log10(L/ks))^-2.5 for 1e2 < L/ks < 1e6 | https://arxiv.org/pdf/1810.05743v12 (read as text) | confirmed (secondary; the book not read) |
| White Cf = 0.455/ln^2(0.06 Re_x) | flat-plate tests | as written | Jaffer eq. (8) | same | confirmed (secondary) |
| Launder-Spalding wall constants, RNG, Realizable (except A0) | `kepsilon.rs` | as listed in reference-data.md | not re-checked in this pass (verified in wave 6a) | n/a | not re-checked |

### 1.2 Benchmarks

| Item | Where used | Code value | Authoritative value | Source URL | Verdict |
|---|---|---|---|---|---|
| Driver and Seegmiller reattachment | `cases/backward_step/case.json`, step tests | 6.26 +- 0.10 | x_r/H = 6.26 +- 0.10 | https://tmbwg.github.io/turbmodels/backstep_val.html ; https://www.grc.nasa.gov/WWW/wind/valid/backstep/backstep01/backstep01.html | confirmed |
| Re_theta 5000, Re_H about 36000, delta about 1.5 H, profiles at x/H = -4 | ADR 0015 BFS-C | same | TMR backstep page states the same (page notes an earlier 50000 was corrected) | backstep_val | confirmed |
| Expansion ratio 1.125, inlet 8 H, H = 0.5 in (12.7 mm) | ADR 0015, `cases/backward_step` | 1.125, 8 H | NASA GRC WIND page: H = 0.5 in, step height to exit height 1:9, so exit 9 H and inlet 8 H, ratio 9/8. ERCOFTAC case 30 (via search snippet only; direct fetch of the page failed): inlet duct 10.1 cm, step 1.27 cm, ratio 1.125 | https://www.grc.nasa.gov/WWW/wind/valid/backstep/backstep01/backstep01.html ; http://cfd.mace.manchester.ac.uk/ercoftac/doku.php?id=cases:case030 | confirmed (secondary, GRC page by inference; ERCOFTAC snippet only) |
| "5.5 to 7 step heights for RANS" scatter | reference-data.md | n/a | no authoritative statement found (a simulation paper snippet gave 5.04 and 5.56 for two RANS grids) | n/a | unverified |
| Laminar BFS ER 2, Re = 800: Erturk 11.834, Gartling 12.20 | `step_laminar.rs` | 11.834, 12.20 | Table 2 of arXiv:2507.16509, read as raw text: Erturk 11.834, Gartling 12.20, Gresho et al. 12.20, spread over authors 11.48 to 12.20. Re = u_a D/nu, D = 2h (same paper), so Re_h = 400 | https://arxiv.org/pdf/2507.16509 | confirmed (secondary, raw text read; Erturk and Gartling originals not read; bibliographies confirmed by Crossref) |
| Laminar BFS Re 200, 400: 4.95, 8.10 | `step_laminar.rs` | 4.95, 8.10 | Table 1 of the same paper (its own finite-volume results, not a published benchmark) | same | confirmed as quoted; still not an independent benchmark |
| Armaly 1983 reattachment table | none (no assertion) | n/a | not reached | n/a | unverified (unused) |

### 1.3 Thermodynamics, transport and EOS

| Item | Where used | Code value | Authoritative value | Source URL | Verdict |
|---|---|---|---|---|---|
| N2 Tc, pc, acentric factor | `rcfd-core/src/eos_data.rs:30` | 126.192 K, 3.3958 MPa, 0.0372 | NIST WebBook fluid page: 126.192 K, 3.3958 MPa, 0.0372 | https://webbook.nist.gov/cgi/fluid.cgi?ID=C7727379 | confirmed |
| CO2 Tc, pc, omega | `eos_data.rs:33` | 304.1282 K, 7.3773 MPa, 0.22394 | NIST: 304.1282 K, 7.3773 MPa, 0.22394 | https://webbook.nist.gov/cgi/fluid.cgi?ID=C124389 | confirmed |
| O2 | `eos_data.rs:31` | 154.581, 5.0430, 0.0222 | NIST: 154.581 K, 5.0430 MPa, 0.0222 | ...?ID=C7782447 | confirmed |
| Ar | `eos_data.rs:32` | 150.687, 4.8630, -0.002 | NIST: 150.687 K, 4.8630 MPa, -0.00219 | ...?ID=C7440371 | differs (omega rounded, 1.9e-4; no test uses Ar PR vs NIST) |
| CH4 | `eos_data.rs:34` | 190.564, 4.5992, 0.01142 | NIST: same | ...?ID=C74828 | confirmed |
| H2 | `eos_data.rs:35` | 33.145, 1.2964, -0.219 | NIST: same | ...?ID=C1333740 | confirmed |
| H2O | `eos_data.rs:36` | 647.096, 22.064, 0.3443 | NIST: 647.096 K, 22.0640 MPa, 0.3443 | ...?ID=C7732185 | confirmed |
| N2 critical constants used in unit tests | `rcfd-core/src/fluids.rs:829` | 126.2 K, 3.39 MPa | NIST 126.192 K, 3.3958 MPa | NIST N2 page | differs (illustrative; pc low by 0.17 percent; unit tests check forms, not values) |
| NIST isothermal tables: CO2 350 K and 310 K, N2 300 K and 140 K; density, h, s, cp, sound speed | `rcfd-fv/tests/realgas_nist.rs`, ADR 0007 R1/R2 | 26 densities, 11 enthalpies, 11 entropies, 5 sound speeds, cp0 899.03 / 862.07 / 1041.3 | Every number matches the raw NIST table (e.g. N2 300 K, 5.1 MPa: 57.468 kg/m^3, 300.76 kJ/kg, 5.6457 J/g/K; CO2 310 K, 9.1 MPa: 625.60; N2 140 K, 6.1 MPa: 349.60) | https://webbook.nist.gov/cgi/fluid.cgi?Action=Load&ID=C7727379&Type=IsoTherm&Digits=5&PLow=0.1&PHigh=20.1&PInc=5&T=300&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm (and the CO2 and 140 K analogues) | confirmed (raw HTML, no summariser) |
| Peng-Robinson 0.45724, 0.07780, kappa = 0.37464 + 1.54226 w - 0.26992 w^2 | `fluids.rs:521-523` | same | Wikipedia (citing Peng and Robinson 1976, I&EC Fund. 15, 59-64): same constants, Omega_a = (8+40 eta_c)/(49-37 eta_c) = 0.457236, Omega_b = eta_c/(3+eta_c) = 0.077796 (own arithmetic); arXiv:2105.04845 uses the same | https://en.wikipedia.org/wiki/Peng%E2%80%93Robinson_equation_of_state | confirmed (secondary; 1976 paper not read) |
| Redlich-Kwong 0.42748, 0.08664 | `fluids.rs:506,517` | same | Wikipedia; also analytic: 1/(9(2^(1/3)-1)) = 0.427480, (2^(1/3)-1)/3 = 0.086640 | https://en.wikipedia.org/wiki/Redlich%E2%80%93Kwong_equation_of_state | confirmed |
| Wilke pair weight | `fluids.rs` | as implemented | Paper exists (J. Chem. Phys. 18(4), 517-519, 1950, Crossref); formula not seen | Crossref | unverified (formula) |
| Standard atomic weights N 14.007, O 15.999, Ar 39.948, H 1.008, C 12.011 | `species` table | as listed | CIAAW intervals: N [14.00643, 14.00728], O [15.99903, 15.99977], H [1.00784, 1.00811], C [12.0096, 12.0116]; Ar 39.948 (CIAAW recommended 1979, interval 39.792 to 39.963). All inside | https://ciaaw.org/nitrogen.htm etc. | confirmed |
| Dry air 0.78084 / 0.20946 / 0.00934 | `fluids.rs:373` | same | CIPM-2007 (NIST): N2 0.780848, O2 0.209390, Ar 0.009332 (with CO2 0.0004). Code is within 0.03 percent (O2) and 0.09 percent (Ar). Matches older standard-atmosphere style values, not checked against that table. | https://www.nist.gov/sites/default/files/documents/calibrations/CIPM-2007.pdf | differs (immaterial, under 0.1 percent) |
| Sutherland air viscosity 1.716e-5 Pa s at 273.15 K, S = 110.4 K | `fluids.rs:404-412` | as written | C1 = 1.458e-6, S = 110.4 K (US Standard Atmosphere 1976 code); C1 T^1.5/(T+S) at 273.15 K = 1.7161e-5 (own arithmetic). CFD-Online table: 1.716e-5 at 273.15 K, S 110.4 | https://www.pdas.com/programs/atmos76.f90 ; https://www.cfd-online.com/Wiki/Sutherland's_law | confirmed (secondary) |
| Sutherland air conductivity 0.0241 W/(m K), S = 194 K | `fluids.rs:415-421` | at 273.15 K | kappa0 = 2.41e-2 W/(m K), T0 = 273 K, S = 194 K (valid 160 to 2000 K) | https://arxiv.org/pdf/2210.01621 (via search) | confirmed (secondary; White not read; reference temperature 273 versus 273.15 K is immaterial) |
| IAPWS-IF97 Table 5 first row (T 300 K, p 3 MPa: v 0.100215168e-2, h 0.115331273e3) | `rcfd-core/src/iapws.rs:446-447` | same | spot check only, third-party validation sheet; earlier wave records a full read of the release | https://vegargje.folk.ntnu.no/master/model/CoolProp/externals/IF97/wrapper/Mathcad/if97_validation.pdf (via search) | confirmed (spot check); full tables not re-read in this pass |
| Le Quere et al. 2005 values | `varden` tests | Nu 8.85978 etc. | read from primary in wave 10 | n/a | not re-checked |
| Viscosity-law forms (Carreau-Yasuda, Cross, Papanastasiou, Herschel-Bulkley) | `rheology.rs` | `(n-1)/a`; `1/(1+(lambda g)^m)`; `tau0(1-exp(-m g))/g` | Carreau-Yasuda `[1+(lambda g)^a]^((n-1)/a)` confirmed by arXiv:1810.02292 (Bird et al. five-parameter form); Cross and Papanastasiou forms not reached. All four papers exist with the stated journals and pages (Crossref) | https://arxiv.org/pdf/1810.02292 | confirmed (Carreau-Yasuda); unverified (Cross, Papanastasiou forms) |
| WLF constants 17.44 and 51.6 K | `rheology.rs` | same | universal values with reference temperature = Tg, valid Tg to about Tg + 50 K | https://goldbook.iupac.org/terms/view/12821 ; https://en.wikipedia.org/wiki/Williams%E2%80%93Landel%E2%80%93Ferry_equation | confirmed (secondary) |
| Rosseland k_r = 16 sigma n^2 T^3/(3 beta) | ADR 0012 | same | COMSOL Rosseland model: k_R = 16 n^2 sigma T^3/(3 beta_R) | https://www.comsol.com/blogs/4-methods-to-account-for-radiation-in-participating-media (via search) | confirmed (secondary) |

### 1.4 Radiation, chemistry, acoustics

| Item | Where used | Code value | Authoritative value | Source URL | Verdict |
|---|---|---|---|---|---|
| Milne extrapolation length 0.7104 | `rcfd-api/tests/radiation.rs:153` | 0.7104 | Hopf constant 0.7104460895 (arXiv:1704.05940 eq. 62); ANS letter: exact Milne value 0.7104 mean free paths versus 0.6667 from Marshak/P1 | https://arxiv.org/pdf/1704.05940 ; https://wx1.ans.org/pubs/journals/download/a_26506 | confirmed |
| Slab law Psi = 4/(3 tau + 6 x 0.7104) from the Milne value | same | as written | verifier's own derivation: perturbation about the wall-temperature equilibrium has pi B' = -(3/4) F (tau + q), q(inf) = 0.7104 for each black wall, so sigma(T1^4 - T2^4) = (3/4) F (tau + 2 x 0.7104); gives the code's formula. Not found stated in a source. | derived | confirmed by independent derivation, not by literature |
| Robertson at t = 40: 0.7158270687, 9.185534764e-6, 0.2841637457 | `rcfd-reactor/tests/reactor.rs` | 10 digits | Bari ROBER report gives the reference only at t = 1e11 (y1 = 0.2083340149701255e-7), not t = 40. A journal table (J. Math. Chem. 61, 75, Table 5) shows 7.158E-01, 9.185E-06, 2.841E-01 (4 digits, garbled extraction); a student repository quotes 0.7158271, 9.1855e-6, 0.2841637 as a cross-check. Digits 5 to 10 not confirmed. | https://archimede.uniba.it/~testset/report/rober.pdf ; https://link.springer.com/content/pdf/10.1007/s10910-022-01402-2.pdf | unverified beyond 4 digits (leading digits corroborated) |
| CH4/air phi = 1 constant-pressure adiabatic flame temperature about 2225 K | `reactor.rs` R7, ADR 0013 | 2225 K, 1 percent window, measured 2225.5 K | NASA CEA (CEARUN), 298.15 K, 1 atm, equilibrium: 2224.25 K (Marzouk 2023). The same paper lists a Cantera/GRI-Mech 3.0 value of 2156.25 K (3.06 percent lower), which is far from every other value I know and probably a set-up fault, but I did not run it. Cantera's own guide prints no number. | https://arxiv.org/pdf/2503.11826 ; https://cantera.org/3.2/userguide/flame-temperature.html | confirmed against CEA (secondary); conflicting GRI-Mech figure in the same source noted |
| GRI-Mech 3.0 licence | validation-data.md, ADR 0013 | "citation requested, terms unverified" | Berkeley page contains only a "HOW TO CITE" block, no licence text (checked by raw fetch). Cantera repository `License.txt` is BSD-3-Clause for Cantera; `gri30.yaml` header only refers to the GRI README for a disclaimer. | http://combustion.berkeley.edu/gri-mech/version30/text30.html ; https://raw.githubusercontent.com/Cantera/cantera/main/License.txt | confirmed that no licence text exists on the page; legal status still unverified |
| Formulation 1A thickness and loading terms, including (M_r - M^2) and r dM_r/dt terms | `rcfd-post/src/fwh.rs:344-362` | as implemented | Farassat NASA/TM-2007-214853 gives the derivation and states that the combination M_r - M^2 in eqs. (67),(68) is correct; equation text extracts garbled, term structure (1-M_r)^2 and ^3 denominators and L_r - L_M readable; code matches the standard form. A 2026 preprint claims a missing surface-normal term; not evaluated. | https://ntrs.nasa.gov/api/citations/20070010579/downloads/20070010579.pdf ; https://arxiv.org/pdf/2606.07081 | confirmed (structure and key combination; full equation images not machine-readable) |
| Permeable terms U_i = (1 - rho/rho0) v_i + rho u_i/rho0, L_i = p' n_i + rho u_i (u_n - v_n) | `fwh.rs:8-9` | as written | NASA paper AIAA 2004-2805 eq. (2): L_i = P_ij n_j + rho u_i (u_n - v_n), B = rho0 v_n + rho (u_n - v_n); B = rho0 U_n gives the U_i form | https://ntrs.nasa.gov/api/citations/20040073459/downloads/20040073459.pdf | confirmed |

### 1.5 Bibliographic details (Crossref, 2026-10-15)

Confirmed against Crossref records (journal, volume, pages, year): Thompson JCP 68(1) 1-24 (1987); Poinsot and Lele JCP 101(1) 104-129
(1992); Spalding J. Appl. Mech. 28(3) 455-458 (1961); Chen and Patel AIAA J. 26(6) 641-648 (1988); Kader IJHMT 24(9) 1541-1544 (1981);
Launder and Spalding CMAME 3(2) 269-289 (1974); Cross J. Colloid Sci. 20(5) 417-437 (1965); Papanastasiou J. Rheol. 31(5) 385-404
(1987); Williams, Landel, Ferry JACS 77(14) 3701-3707 (1955); Wilke JCP 18(4) 517-519 (1950); Patterson and Imberger JFM 100(1)
65-86 (1980); Roe JCP 43(2) 357-372 (1981); Harten JCP 49(3) 357-393 (1983); Liou JCP 129(2) 364-382 (1996); Shu and Osher JCP
77(2) 439-471 (1988); Issa JCP 62(1) 40-65 (1986); Armaly et al. JFM 127 473-496 (1983); Gartling IJNMF 11(7) 953-967 (1990);
Erturk C&F 37(6) 633-655 (2008); Di Francescantonio JSV 202(4) 491-509 (1997); Brentner and Farassat AIAA J. 36(8) 1379-1386
(1998) and Prog. Aerosp. Sci. 39(2-3) 83-120 (2003); Williamson ARFM 28 477-539 (1996); Driver and Seegmiller AIAA J. 23(2)
163-171 (1985); Carreau Trans. Soc. Rheol. 16(1) 99-127 (1972); Herschel and Bulkley Kolloid-Z. 39(4) 291-300 (1926).
Not confirmed by this search: Choi 1999 (Numer. Heat Transfer A 36; the top Crossref hit was a different paper), Vassberg and
Jameson J. Aircraft 47(4) 1152-1166 (2010) (Crossref returned the 2009 AIAA conference version only), Menter, Kuntz and Langtry 2003
(Crossref returned a different title), Cebeci and Bradshaw 1977 book (only a 1979 review), Spalart and Allmaras (Crossref
returned the AIAA 92-0439 version; the La Recherche Aerospatiale 1994 version was not looked up).

### 1.6 Licence observation (not a numeric value)

NIST WebBook is Standard Reference Data (SRD 69). NIST states that SRD is copyrighted by the U.S. Secretary of Commerce, "all
rights reserved", and may not be reproduced without prior permission (https://www.nist.gov/srd/public-law ; fair-use and licensing
page https://www.nist.gov/open/copyright-fair-use-and-licensing-statements-srd-data-software-and-technical-series-publications).
`realgas_nist.rs` and ADR 0007 copy about 60 numbers from the WebBook. `reference-data.md` says "NIST WebBook data was
deliberately not used (its terms were not reviewed)", which contradicts the F-061 section of the same file. Whether a handful of
validation points is permitted use is a legal question this pass did not resolve; the terms exist and should be reviewed.

## 2. Discrepancies, ranked by impact on registered test outcomes

No fetched authoritative source contradicts a benchmark number or a registered tolerance. Two code constants differ from the
sources, and several minor differences are listed for completeness.

### 2.1 Realizable k-epsilon A0 = 4.04 versus 4.0 (differs; medium impact)

- Where: `crates/rcfd-turb/src/kepsilon.rs:448` (`1.0 / (4.04 + a_s * u_star * k / eps)`), comment at `:23`; also the note in
  `docs/reference-data.md` line 85 and ADR 0015.
- Code 4.04, source 4.0: Shih, Liou, Shabbir, Yang and Zhu, NASA TM-106721 (the report version of Comput. Fluids 24(3) 227-238,
  1995), text "This leads to A0 = 4.0 which corresponds to C_mu = 0.09 in the inertial sublayer" and Table 2. 4.04 is the value
  of the Fluent theory guide. The change is 1 percent of the A0 term (C_mu = 1/(A0 + A_s U* k/eps)), so C_mu moves by well under 1 percent
  where U* k/eps is of order one or more.
- Tests touched: F-042 Realizable BFS known misses (`step_reattachment_realizable_known_miss`, measured 6.402 against the
  6.26 +- 0.10 window; BFS-C Realizable 6.413); the ordinary Realizable channel and flat-plate tests. A sub-1-percent change of
  C_mu in the recirculation zone cannot be predicted to move x_r by more than a few hundredths, so the 6.402 (0.042 outside)
  could flip to a pass or stay a miss; this was NOT run (no cargo in this pass). Recommendation for the code owner: set A0 to 4.0
  or document 4.04 as the Fluent variant.

### 2.2 Scalable wall function limit y*_lam = 11.53 versus 11.06 (CFX) / 11.225 (Fluent) (not an error; low impact)

- Where: `wall.rs:45-50` (`y_plus_lam` from the rcfd rounded log-law constants kappa 0.41, E 9.8, giving 11.53) used by
  `nu_eff_wall_scalable` (`wall.rs:249-262`).
- Sources: CFX theory guide 11.06 (intersection for its log-law constants), Fluent 11.225 (kappa 0.4187, E 9.793; own arithmetic
  gives 11.2247). 11.53 is the correct linear/log intersection for rcfd's own disclosed kappa 0.41, E 9.8, so this is consistent with the
  definition, not a wrong constant; it is listed because the scalable behaviour
  depends on it by about 3 to 4 percent in the limited y*. Tests touched: `c2_kepsilon_scalable_y30`, `..._y1` (currently
  passing within 5 percent) and the Standard-versus-Scalable comparison in the C2 known miss.
- The exact Grotjans-Menter epsilon boundary treatment (`y_eff`) in ADR 0015 could not be checked, so the scalable results
  rest on an unverified reconstruction.

### 2.3 Minor numeric differences (no test impact found)

- Argon acentric factor: code -0.002, NIST -0.00219 (`eos_data.rs:32`).
- `fluids.rs:829` test constants N2 126.2 K, 3.39 MPa versus NIST 126.192 K, 3.3958 MPa (pc 0.17 percent low; unit tests of the
  EOS forms only).
- Dry-air mole fractions O2 0.20946 and Ar 0.00934 versus CIPM-2007 0.209390 and 0.009332 (0.03 and 0.09 percent); the
  Sutherland-versus-Huber comparison is unaffected at the quoted precision.
- Spalding series truncation (cubic, as in OpenFOAM and libWMLES; one review prints a quartic term): not checked against the
  1961 paper.

### 2.4 Source conflicts and caveats to keep in mind

- Flame temperature: CEA 2224.25 K supports the 2225 K gate, but the same secondary paper's GRI-Mech/Cantera figure is 2156.25 K.
  The test passes at 2225.5 K inside a 1 percent window; if the Cantera 2156 K were correct the gate would be 3 percent wrong.
  I do not believe it, but the check of the thermochemistry itself (running Cantera) was outside this pass.
- Prandtl-Schlichting: sources disagree on local versus plate-average. rcfd uses it as an average over [0, L] against an
  average Cf, which matches the Iowa/Schlichting reading; the CFD-Online listing as a local law is probably loose.
- Robertson digits beyond the fourth are still memory plus the integrator's own agreement.
- The "textbook C_s = 0.5" remark in ADR 0015 was not found in any fetched page.

## 3. Items still unverified

- Grotjans-Menter scalable details (epsilon boundary condition, shear form); Chen-Patel original constants.
- Cebeci-Bradshaw 1977 original text; Spalart-Allmaras 1994 paper itself (TMR values confirmed); Kader 1981 paper itself.
- Robertson t = 40 digits 5 to 10.
- Cross and Papanastasiou law forms; Wilke formula; Kays and London effectiveness definition; Modest P1/Marshak slab (derived here
  only for the black-wall Milne mapping); Bird, Stewart and Lightfoot section numbers; Stokes-Kirchhoff decay.
- Thompson/Poinsot-Lele LODI formulas, Blazek chapter 8 (bibliography confirmed only).
- "5.5 to 7 step heights for RANS"; Williamson 1989 and Henderson 1995 St values; Armaly table; Toro Table 4.3 attribution.
- Ghia and de Vahl Davis original printed tables (secondary copies agree, as recorded in wave 6a).
- Fluent mesh format section ids (F-118); Rotor 37 values; GRI-Mech legal terms.

## 4. Counts

Table rows in section 1 (57 data rows; a row may bundle several constants): 44 confirmed (most secondary; 3 of these carry an
explicit unverified sub-part), 4 differs (Realizable A0; Ar acentric factor; N2 test constants; dry-air composition), 7 unverified
or partly verified, 2 not re-checked. The scalable intersection limit is not counted as a difference (see 2.2). Bibliographic
confirmations in 1.5 are not counted.
