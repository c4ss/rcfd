---
type: source
tags: [adr-thread, turbulence, spalart-allmaras, naca0012, wall-shear, far-field, wall-function, linear-upwind, aoa-sweep]
sources: [adr-thread-simulation-milestones-validation, adr-thread-example-projects-and-linux-values]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the NACA 0012 angle-of-attack sweep and why the Spalart-Allmaras drag was wrong (jobs 48Y, 49A to 49H)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, diagnoses, default changes made on the maintainer's answers), 8 ADRs, 2026-10-29 to 2026-10-30. The sweeps (ADRs 0159, 0167) carry F-868 in the index; the other file names carry no item.

ADR 0159 (48Y) swept the [[naca0012]] at Re 6e6 from 0 to 24 degrees through `rcfd run` and compared with Ladson 1988. The lift slope met its limit, but "SA's drag is half the measured value at 0 degrees and 1.4 to 4.6 times it from 4 degrees up". ADR 0164 (49A) found two independent causes, "neither is the SA model, its coefficients or its wall law": the default force report took the molecular viscosity, so on a wall-function mesh the reported friction was "0.319 of" the shear the solver applies (y+ about 42), and "Nothing fixes nuTilda on a `freestream` far field" (median outer nuTilda/nu 6.02e4 at 10 degrees). On the maintainer's answers both fixes became defaults (ADR 0166 for SA, ADR 0169 for SST and k-epsilon, ADR 0170 for moment reports), the sweep was repeated on a 500-chord far field (ADR 0167), the buffer-layer error of SA's wall treatment got an opt-in (ADR 0168), and the trailing-edge divergence at fine wall spacing was traced to unlimited linear-upwind on sliver cells (ADR 0171). That last mechanism was then tested on FAN-01 (ADR 0174, on [[adr-thread-fan01-rotating-solve]]) and became the static warning SV302 (ADR 0172, on [[adr-thread-setup-warnings-and-time-step]]).

## Current state

- **Defaults changed** (each bit-identical where not touched, old behaviour spellable): `force` and `force_coefficient` reports with component `total` or `viscous` take "the momentum equation's face viscosity when a turbulence model is active" (ADR 0166 D1; old values `total_molecular` / `viscous_molecular`); a `velocity = "freestream"` patch gets the `freestream` turbulence role under `turbulence = "auto"` for SA (ADR 0166 D2) and for k-omega SST and k-epsilon (ADR 0169); RSM stays `zero_gradient`; `moment` reports follow the force default (ADR 0170, superseding ADR 0166 D3). No `baseline_hashes.rs` case changed in any of them.
- **The repository's NACA 0012 curve** is ADR 0167's steady SA arm on M1w-far500, which ADR 0760 (F-868) reproduces bit for bit and judges against Ladson: ~~partial, 17 degrees a known miss~~ done since 2026-11-22: at 17 degrees a time-accurate SA run on the same mesh (ADR 0764) settles to a steady attached flow with $c_l$ 1.5516 inside the band, and the maintainer accepted it (ADR 0764 answer (a)); the steady iteration's non-convergence at 17 degrees is F-884 ([[adr-thread-simulation-milestones-validation]]). ADR 0167: lift slope 0.986 x 2 pi, cl within 0.013 of Ladson's from 2 to 15 degrees, peak 1.542 at 16 degrees, "4.3 % below Ladson's 1.612 at 17.13 and about 1 degree early"; "From 17 degrees up steady SIMPLEC does not converge, so those angles enter the curve as flagged iteration means, which are not flow values".
- **Drag is still high at lift and not attributed:** against TMR's SA values cd is "+2.5 % at 0 degrees, +9.8 % at 10 and +26.0 % at 15" (ADR 0167, LT15 a known miss); ADR 0164's C10 on 500 chords "gives cd +10 %, and that remainder is not attributed".
- **SA wall mode** default stays `auto`; the opt-in `turbulence.sa.wall_mode = "auto_sa_law"` meets every limit after amendment 1 (plate +2.3 % to +4.4 % against TMR at all 10 y+; aerofoil cd +2.0 %, +1.3 %, +2.3 % at median wall y+ 5, 15, 42). "the addendum recommends `auto` now and `auto_sa_law` after one more job" (ADR 0168).
- **Trailing-edge divergence:** fixed by an existing opt-in, `solver.linear_upwind_limiter = "barth_jespersen"`, which "makes all five 300-iteration runs clean" and costs +3.0 % of `cd_eff` at 0 degrees; "question 1 recommends against making it the default for now" (ADR 0171). H-LU, the mechanism, "is supported but not proved by a registered run".
- **FAN-01 torque under the effective moment** (ADR 0170): F-126 attempt 3 stays a known miss at -37.1 % and -44.6 %; ADR 0086's bladed studies B-M and B-P stay VALID with torque p falling from 1.51 to 1.39.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0159](../../docs/adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md) | 2026-10-29 | registration | F-868 | NACA 0012 sweep 0 to 24 degrees, steady SA and SST, SA-DDES where steady fails, against Ladson | every limit met (L3 only with the extension to 24 degrees); M1 diverged at the trailing edge, M1w (y+ about 40) used; drag wrong, not investigated | 0167 |
| [0164](../../docs/adr/0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md) | 2026-10-29 | diagnosis | | Why steady SA's drag is wrong (ADR 0159 Q4) | both causes found and confirmed; LA4 (+25.95 %, band 25 %), LA6, LP1 at y+ 5 (+19.9 %) known misses; Q1, 2, 4, 5 answered with the recommendations | 0166 |
| [0166](../../docs/adr/0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md) | 2026-10-29 | default change | | Effective wall shear and the SA far-field role as defaults | every limit met, no known miss; ADR 0159's case text now gives cl 1.055, cd 0.01548 at 10 degrees (before 0.982, 0.0396) | 0169 (D2 scope), 0170 (D3) |
| [0167](../../docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md) | 2026-10-29 | registration | F-868 | The sweep again with both fixes, far field at 500 chords | every limit met except LT15 (cd +26.0 % at 15 degrees, band 25 %) and LD (drift metric); "neither fix alone is right at lift; both are needed" | ADR 0760 reproduces it |
| [0168](../../docs/adr/0168-job49e-sa-wall-treatment-in-the-buffer-layer.md) | 2026-10-29 | diagnosis + opt-in | | SA wall treatment with the wall cell in the buffer layer (ADR 0164 Q8) | cause confirmed (linear face interpolation of nut in `effective_viscosity`); opt-in `auto_sa_law` meets every limit after amendment 1; as registered LP1, LP2, LP4 at y+ 5 known misses | |
| [0169](../../docs/adr/0169-job49f-sst-and-k-epsilon-far-field-default-and-the-naca-example-runs.md) | 2026-10-29 | default change | | `freestream` far-field default for SST and k-epsilon; regenerate the 8 stored NACA runs | F1 to F7, R1, G1 met; the regenerated runs converge, so ADR 0142 G1, ADR 0155 E2, ADR 0148 N9 aerofoil parts become known misses | |
| [0170](../../docs/adr/0170-job49g-moment-reports-follow-the-effective-wall-shear-default.md) | 2026-10-29 | default change | | Moment reports follow the effective wall-shear default (ADR 0166 Q2 (a)) | every limit met; the effective moment adds 0.027 N m (design) and 0.032 N m (off design) to FAN-01 | |
| [0171](../../docs/adr/0171-job49h-the-naca0012-trailing-edge-divergence-at-fine-wall-spacing.md) | 2026-10-30 | diagnosis | | Trailing-edge divergence at fine wall spacing (ADR 0159 Q5) | reproduced; H-084 rejected; H-LU supported, not proved; the Barth-Jespersen opt-in removes it; LL2 (+7.1 %, limit 6 %) and LL7 (+12.7 %, limit 10 %) missed | 0172 (SV302), 0174 (on FAN-01) |

## Key numbers and limits

- **ADR 0159:** lift slope 0.920 x 2 pi, "0.02 above the lower edge"; SA's cl_max 1.528 at 20 degrees, SST's 1.397 at 14 degrees, against Ladson's 1.612 at 17.1 degrees: "the two RANS models bracket the measured stall angle by about 3 degrees on either side". SA-DDES at 20 degrees "is not settled (half-window drift -0.21 in cl)".
- **ADR 0164:** with `total_effective`, A00 gives cd +3.6 % against TMR at 0 degrees; with the `freestream` role as well, F10 gives cd +26 % and cl -3.3 % at 10 degrees on 50 chords. "Arm B, the registered far-field set-up, was itself defective (`inlet` traps nuTilda)".
- **ADR 0166:** ADR 0159's SA cases "reproduce bit for bit under the old spellings over prefixes of 1099 and 876 iterations, not their full length".
- **ADR 0168:** in `auto`, "plate Cf(0.97) is +19.9 % against TMR at wall y+ 5 and +19.7 % at y+ 8, and the NACA 0012 at 0 degrees gives friction +17.4 % and cd +17.8 % on the mesh with median wall y+ 5". The plate set-up has a baseline of "+2 % to +3 %" at y+ 1 in every mode.
- **ADR 0169:** at 4 degrees `cd` moves from the stored 0.02536 to 0.02820: the effective wall shear "+0.00278 (+11 %)" and the far field "+0.00006 (+0.2 %)". Far-field values come from `turbulence.initial_intensity` and `turbulence.initial_viscosity_ratio` when the patch gives no inlet specification.
- **ADR 0170:** "ADR 0086's 'balance closes to 0.04 % on F1' holds for the molecular moment only".
- **ADR 0171:** M1 (first cell 1.57e-5 chord) at 6 degrees "hits the guard at iteration 15 in the trailing-edge tip cell 0 (SA; SST at 16)"; LL4 "passes by 0.01 %".

## Open questions

- ADR 0159 Q2 (a 2D C-grid generator), Q3 (URANS without DES), Q6 (which curve is "the" result), Q7 (SA-DDES at 20 degrees), Q8 (`CHANGELOG.md`).
- ADR 0164 Q3 (re-run the sweep; done as ADR 0167), Q6 (LA4), Q7 (LA6's reading rule); Q8 became ADR 0168.
- ADR 0166 Q4: laminar variable viscosity (`rheology`, user-function viscosity) still takes `physics.nu` in `total`; recommendation (a), its own job.
- ADR 0167 Q1 (SST far-field values with the `freestream` role), Q2 (stall from 17 degrees), Q3 (the remaining drag excess at lift), Q4 (the drift metric), Q5 (which curve is the reference; answered in effect by F-868, ADR 0760).
- ADR 0168 Q1 (default SA wall mode), Q2 (the `auto` documentation says it "works from y+ about 1 into the log layer"), Q3 (the same face interpolation in SST and k-epsilon), Q4 (steady stop at y+ <= 3).
- ADR 0169 Q1 (cases that set `k_initial` / `omega_initial`), Q3 (the now false case comment on `max_iterations`), Q4, Q5.
- ADR 0170 Q1 (should `component` select a moment's part), Q3 (records with molecular torques not re-run; recommendation one line in `docs/milestone-fan.md`), Q4 (the report against the solver's wall flux, its own job).
- ADR 0171 Q1: the limiter as default (recommendation no, for now).

## Related

- Predecessor (added by p3-records-a): the Spalart-Allmaras model (SA-noft2) and the turbulence-model interface are ADR 0014 (F-041 partial), and the NACA 0012 check B7 of ADR 0015 "was not attempted"; see [[adr-thread-early-turbulence]].
- [[naca0012]], [[spalart-allmaras]], [[k-omega-sst]], [[k-epsilon]], [[flat-plate]], [[report]], [[setup-rules]], [[known-miss]], [[fan-01]]
- Threads: [[adr-thread-simulation-milestones-validation]] (F-868), [[adr-thread-setup-warnings-and-time-step]] (SV302), [[adr-thread-fan01-rotating-solve]] (ADR 0174, torques), [[adr-thread-example-projects-and-linux-values]] (the stored aerofoil runs), [[adr-thread-transient-rans]]
- Areas: [[roadmap-turbulence]], [[roadmap-quality-docs-and-validation]]
- Investigation folders: [[doc-investigation-job48y-aoa-sweep]] (ADR 0159), [[doc-investigation-job49a-sa-drag]] (ADR 0164), [[doc-investigation-job49d-aoa-sweep-fixed]] (ADR 0167), [[doc-investigation-job49e-sa-wall]] (ADR 0168), [[doc-investigation-job49h-te-divergence]] (ADR 0171), [[doc-investigation-f868-naca0012-lift-curve]] (ADR 0760)
