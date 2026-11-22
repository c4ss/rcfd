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

# Reference data: Turbulence finishing (F-042, F-043, F-203, ADR 0015)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Turbulence finishing (F-042, F-043, F-203, ADR 0015)" (lines 604-638) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Turbulence finishing (F-042, F-043, F-203, ADR 0015): backward-step and flat-plate results against their windows.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


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
| Two-layer constants A_mu = 70, A_eps = 2 c_l, blend Re_y* = 200 | Chen and Patel (1988), Jongen (1998) | wave 13: UNVERIFIED against a published source, replace before relying on it (constants and tanh blend form not found in a published source, see [provenance](../../docs/provenance.md); Wolfshtein, Int. J. Heat Mass Transfer 12 (1969) 301-318, is the usual origin of the two-layer length scales, not read); Chen and Patel citation confirmed (Crossref) but the constants not seen in the paper; blend width 0.1 Re_y* is an rcfd choice within the quoted 5 to 20 percent |
| Kader blended law (Gamma = -0.01 y+^4/(1 + 5 y+)) | Kader (1981) | wave 13: confirmed (secondary) in a heat-transfer paper writing the equivalent positive-Gamma form (Xia et al., see `docs/verification-report.md`); the 1981 paper was not read |
| Standard k-epsilon under-predicts BFS reattachment by 10 to 20 percent | general literature statement | not tied to a specific source here; the measured 14.5 percent is consistent |
| Non-equilibrium wall functions (Kim and Choudhury) | not used | not attempted: formulas not verifiable |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[backward-facing-step]]
