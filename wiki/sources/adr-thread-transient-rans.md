---
type: source
tags: [adr-thread, turbulence, rans, urans, k-omega-sst, k-epsilon, spalart-allmaras, pimple, wall-functions]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: transient RANS on PIMPLE, and the F-040 wall-function channel

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (three registrations with result addenda, one retrospective test record), 4 ADRs, 2026-11-06 to 2026-11-17.

Before F-286 the RANS transport models were steady only. ADR 0199 put k-omega SST on the outer iterations of `pimple`: at the start of each outer iteration the model makes one sweep of its steady update, with the implicit under-relaxation replaced by the time derivative of the momentum's time scheme (Euler, BDF2, or bounded BDF2) and no `relax_nut`. ADR 0210 did the same for k-epsilon and Spalart-Allmaras without DES (F-294); ADR 0225 (F-300) followed up the time-order misses and made `rcfd_turb::supports_transient` answer true for the transported models. ADR 0707 records, after the fact, the limits of the F-040 wall-function channel test. See [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]].

## Current state

- **F-286 done, F-294 and F-300 partial, F-040 done** ([[roadmap-turbulence]]).
- **SST time order is met** from a start that meets the walls: ADR 0199's S3 BDF2 was a known miss above the band (`p` `2.6694` for `ux`, band `[1.7, 2.3]`); ADR 0199 amendment 4 (written by F-300) registered S3', met: `p` `2.0450` (`ux`), `2.0394` (k), `2.0306` (omega); Euler `1.0030`, `0.9800`, `1.0028` (band `[0.8, 1.2]`).
- **The k-epsilon BDF2 order is still a known miss** with an unknown cause. F-300 gap: "p 0.8031 / 1.4814 / 1.4287 with the smooth Spalding wall law (standard law 0.7975 / 1.4910 / 1.5303), so the kink is not the cause and the real cause is not found." The Spalding wall eddy viscosity is library-only (`KEpsilonModel::set_option("wall_nut_law", ..)`; ADR 0225 Q3 recommendation (a)).
- **`piso` is refused** for the transported models at `turbulence.model` (ADR 0199 Q1, ADR 0210 Q1, recommendation (a)); `pimple` with `n_outer = 1` gives the lagged update. SA-DES and the Reynolds stress model stay on the once-per-step path (ADR 0199 D6, Q3).
- **Answers of 2026-11-18** (in each of ADRs 0199, 0210, 0225): the lead closes each question on its written recommendation, "except a question that would change a default result, a golden or a hash table, or that needs a source the repository lacks; those stay open and are named in the item's roadmap note". The F-294 gap still names "ADR 0210 Q1 to Q7 are also open" and F-300 "ADR 0225 Q1 to Q6 are open".
- **F-040 channel** (ADR 0707): the test file predates the ADR (`crates/rcfd-turb/tests/channel_wall_function.rs`, first import `f71f6e2c`, 2026-10-13); the log law is recovered with a mixing-length interior, "not yet with k-epsilon (F-042)". The ADR "is not a decision taken before the test".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0199](../../docs/adr/0199-f286-registration-transient-k-omega-sst-with-pimple.md) | 2026-11-06 | registration (four amendments) | F-286 | Transient k-omega SST on `pimple`, static mesh | done; S3 BDF2 known miss above the band, met as S3' by amendment 4 (ADR 0225) | 0210, 0225 |
| [0210](../../docs/adr/0210-f294-registration-transient-k-epsilon-and-spalart-allmaras-with-pimple.md) | 2026-11-07 | registration (three amendments) | F-294 | Transient k-epsilon and SA (no DES) on `pimple` | partial; both run with `pimple` and the rotating frame, defaults bit-identical; k-epsilon BDF2 order missed (`p` `0.80` to `1.53` at T/80 to T/320) | 0225 |
| [0225](../../docs/adr/0225-f300-registration-transient-rans-follow-ups.md) | 2026-11-08 | registration (follow-ups) | F-300 | `supports_transient`, SST order from a start meeting the walls, a smooth wall eddy viscosity for the k-epsilon miss | partial; every check met except S3s `bdf2` (`p` `0.8031`, `1.4814`, `1.4287`) | |
| [0707](../../docs/adr/0707-f769-registration-the-f040-wall-function-channel.md) | 2026-11-17 | retrospective test record (F-769 group 2 of 5; row corrections by ADR 0717) | F-040 | What the F-040 channel test checks | limits W1 to W8 recorded; results in ADR 0705 | |

## Key numbers and limits

- ADR 0199 D1 BDF2 weights: `((1 + 2 r)/(1 + r), 1 + r, r^2/(1 + r))` with `r = dt / dt_old`; Euler and the first step of either BDF2 `(1, 1, 0)`.
- ADR 0199 S3 set-up: channel CH on `pimple`, `n_outer = 20`, `n_correctors = 2`, tolerances `1e-12`, to `T = 2`, `dt = T/80, T/160, T/320`.
- F-294 gap: k-epsilon BDF2 observed `p` `0.80` to `1.53` at T/80 to T/320, "rising to 1.81 to 1.89 at T/320 to T/1280".
- ADR 0225 S3s: wall-cell `y*` of the T/320 run in `[8.8101, 13.4234]`.
- ADR 0707: `WallConstants` kappa `0.41`, E `9.8`, Cmu `0.09`, sublayer switch at y+ `11.5`; channel `Re_tau = 395`; first-cell y+ `49.4`, `32.9`, `24.7` (each above `11.6`); wall shear within `2e-3`; interior profile within `0.03` of the mixing-length balance (recorded `2.1 %`, `2.3 %`, `2.4 %` for `ny` 8, 12, 16); `h_from_solution` opt-in "about 6 times faster".

## Open questions

- F-294 and F-300 gaps: ADR 0210 Q1 to Q7 and ADR 0225 Q1 to Q6 named open in the roadmap notes.
- The cause of the k-epsilon BDF2 order miss (not the wall-law kink, ADR 0225).
- ADR 0225 Q3: a case key `turbulence.wall_nut_law` (needs a registered schema change).
- ADR 0707: no DNS comparison, no grid convergence of W7, mixing-length closure not selectable from the case schema.

## Related

- [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]], [[piso]], [[rcfd-turb]]
- [[adr-thread-periodic-seam-diffusivity]], [[doc-case-schema-turbulence]], [[doc-reference-data-f300-transient-rans]], [[doc-reference-data-sst-wall]]
- [[roadmap-turbulence]]
