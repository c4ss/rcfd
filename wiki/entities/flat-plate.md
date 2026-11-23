---
type: entity
tags: [validation, benchmark, turbulence, boundary-layer]
sources: [doc-reference-data-flat-plate, doc-reference-data-sst-wall, doc-reference-data-spalart-allmaras, doc-reference-data-known-misses, doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31b-turbulence-wall, doc-validation-data, doc-investigation-job49a-sa-drag, doc-investigation-job49e-sa-wall, ref-nasa-tmr, doc-milestones-json]
builds_on: [boundary-layer, blasius-solution, skin-friction-coefficient, friction-velocity-and-y-plus, law-of-the-wall, k-omega-sst, spalart-allmaras]
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Flat plate (zero pressure gradient)

**What it is:** the zero-pressure-gradient flat plate, checked against skin-friction correlations, the Blasius solution and the NASA TMR 2DZP turbulent case ([[doc-reference-data-flat-plate]], [[doc-reference-data-sst-wall]]).
**Where:** `cases/flat_plate/reference/correlations.csv`, `tests/flat_plate_sst.rs`; dataset `flat-plate-tmr` (CC0, pinned commit) ([[doc-validation-data]]).

## Details

- Correlations: Blasius Cf = 0.664/sqrt(Re_x); Cf = 0.0576 Re_x^-0.2 for 5e5 < Re_x < 1e7 ("other textbooks use 0.0592"); Cf = 0.455/ln^2(0.06 Re_x) (White's local formula). The CSV "mislabelled it "Prandtl-Schlichting", which is 0.455/log10(Re)^2.58 (average). Label corrected" ([[doc-reference-data-flat-plate]]).
- Blasius to 32 digits (wave 30B, arXiv:2204.02215): f''(0) = 0.33205733621519629893718, delta* = 1.72078765752 sqrt(nu x/U) ([[doc-reference-data-wave30b-incompressible-validation]]).
- TMR SST: CFL3D SST Cf(x = 0.97) = 0.00268299 on 273 x 193; rcfd 0.002663 (-0.7 %) ([[doc-reference-data-sst-wall]]). The pinned finest-grid file gives about 2.691e-3 (2.6909e-3 interpolated), so rcfd is -1.0 % against it; "The test and its 5 % tolerance are unchanged" ([[doc-validation-data]]).
- TMR grid convergence: GCI 0.28 %, p = 1.21; "rcfd's Richardson-extrapolated Cf(0.97) is +2.6 ... +3.6 % above it; not resolved (virtual origin untested)" ([[doc-reference-data-wave31b-turbulence-wall]]).
- Spalart-Allmaras: TMR Cf(0.97) 2.70565e-3 (CFL3D SA) ([[doc-reference-data-spalart-allmaras]]).
- Known miss F-043: SST vs White 0.002663 vs 0.002875 (-7.36 %) against a 5 % criterion; met: SST vs the TMR page value (-0.7 %) and file value (-1.0 %) ([[doc-reference-data-known-misses]]).
- SA plate Cf(0.97) against TMR 2.70565e-3 by wall y+ (jobs 49A, 49E): default `auto` +3.1 % at y+ 0.99, +19.9 % at 5.31, +4.3 % at 39.6; opt-in `auto_sa_law` (amendment 1) +2.3 % to +4.4 % at every y+ from 1 to 60 ([[doc-investigation-job49a-sa-drag]], [[doc-investigation-job49e-sa-wall]]).
- Since 2026-11-23 the turbulent plate is a simulation milestone, `flat-plate` (ADR 0775): headline F-041, F-043 (both partial) and F-203 (done), 18 member items, 86 %, headline not met. The Blasius plate passes its comparison (ADR 0060) under no item; a headline item for it is proposed and with the maintainer ([[doc-milestones-json]], [[simulation-milestones]]).

## Related

[[gci-richardson]], [[backward-facing-step]], [[known-miss]], [[doc-reference-data]], [[simulation-milestones]]
- [[doc-investigation-job49a-sa-drag]], [[doc-investigation-job49e-sa-wall]]

## Builds on

*(added by p6-turbulence)*
- [[boundary-layer]]: the zero-pressure-gradient layer the case tests; on the TMR plate the maximum thickness is "about 0.03 L" ([[ref-nasa-tmr]] `flatplate.html`).
- [[blasius-solution]]: the laminar check values ($C_f = 0.664/\sqrt{Re_x}$, $\delta^*$).
- [[skin-friction-coefficient]]: $c_f = \tau_w/(\tfrac{1}{2} \rho U^2)$ and the turbulent correlations; the TMR validation page uses Karman-Schoenherr in $Re_\theta$ and warns the correlations "are not necessarily perfect" ([[ref-nasa-tmr]] `flatplate_val.html`).
- [[friction-velocity-and-y-plus]] and [[law-of-the-wall]]: the $u^+$ vs $y^+$ comparison at $Re_\theta = 10\,000$ (Coles profile) and the first-cell $y^+$ sensitivity of omega-based models ([[ref-nasa-tmr]] `flatplate_val_ypluseffect.html`).
- [[k-omega-sst]], [[spalart-allmaras]]: the turbulence models verified on it; the TMR purpose of the verification case is to show a model "has been implemented correctly", not its "goodness" against experiment ([[ref-nasa-tmr]] `flatplate.html`).

## Open questions

- The Richardson-extrapolated rcfd Cf above the TMR value is "not resolved (virtual origin untested)" ([[doc-reference-data-wave31b-turbulence-wall]]).
