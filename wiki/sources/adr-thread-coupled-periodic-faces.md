---
type: source
tags: [adr-thread, periodic, coupled-faces, sector, rhie-chow, fractional-step]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: coupled periodic faces (implicit partner, non-orthogonal and ddt corrections, fractional step)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 7 ADRs, 2026-11-16 to 2026-11-17.

Opened by F-725 (ADR 0661, [[adr-thread-sliding-mesh-rotor-stator]]): a sector run differs from the full annulus because the solver treats a periodic (seam or wrap) pair differently from an internal face. Each item adds an opt-in that makes the coupled face behave as the internal face between the owner and the partner's image: the implicit rotated partner in the momentum matrix (F-743), the non-orthogonal correction (F-744, F-752, F-753), the transient Rhie-Chow ddt correction (F-754, F-757) and, on the way, the body force in the fractional step (F-771). All are `PimpleSettings` flags, default false, defaults bit-identical.

## Current state

- **Built (opt-in):** `rotational_periodic_implicit` (F-743, done), `coupled_non_orth` (F-744 partial, pressure part checked by F-752 and F-753, both done), `coupled_ddt_correction` (F-754 done; fractional step and BDF2 by F-757), `fractional_periodic_implicit` and `periodic_partner_extrapolated` (F-757), `fractional_body_force` (F-771).
- **Effect on the sector lockstep of ADR 0661:** with the implicit pair, ADR 0641's step-1 difference falls from `3.494e-2` to `6.74e-4` (ADR 0664 J1); with the implicit pair and `coupled_non_orth`, one outer iteration, the largest difference from the full annulus is `1.104e-4` (ADR 0667 N2, limit `4.5e-4`, from `6.0e-3`).
- **Statuses:** F-743, F-752, F-753, F-754 done; F-744 partial (MN1 and MN4 not caught: the pressure part "has no shown effect" on the lockstep, below its resolution); F-757 and F-771 proposed done on the amended rule, partial in the roadmap (a check met only after amendment 1).
- **Defaults are the open thread.** Every ADR recommends one change that makes the coupled face the internal face by default (F-746 "Implicit rotational periodic pairs by default and as a case key", todo; F-775 "Fractional-step coupled-face opt-ins as defaults", todo). The maintainer's answer of 2026-11-18 on F-775 (in ADR 0691): "Held. The default change is not approved now: it moves existing results, and it waits until the fan path needs it. Not to be issued."
- **Case keys:** none for these opt-ins (ADR 0664 Q2, ADR 0667 Q3 open).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0664](../../docs/adr/0664-f743-registration-implicit-momentum-coupling-of-periodic-pairs.md) | 2026-11-16 | registration | F-743 | Implicit momentum coupling of rotational periodic pairs | done; J1 `6.74e-4` (default `3.494e-2`, limit `5e-3`); K1s `2.36e-4` against `1.5e-3`; translational pairs implicit since ADR 0046 | default: F-746 |
| [0667](../../docs/adr/0667-f744-registration-non-orthogonal-correction-on-coupled-faces.md) | 2026-11-16 | registration | F-744 | Non-orthogonal correction on coupled faces (`CoupledNonOrth`) | partial; N1 `3.80e-5` (limit `3e-4`), N2 `1.104e-4` (limit `4.5e-4`) met; MN1, MN4 not caught | 0672, 0674 |
| [0672](../../docs/adr/0672-f752-registration-pressure-part-of-the-non-orthogonal-correction-on-coupled-faces.md) | 2026-11-16 | registration | F-752 | Pressure part on a tilted periodic channel (`abs(k)/abs(S)` 0.298) against its unrolled twin | done | |
| [0674](../../docs/adr/0674-f753-registration-pressure-part-of-the-coupled-non-orthogonal-correction-on-rotational-pairs.md) | 2026-11-17 | registration | F-753 | Pressure part on rotational pairs (60 deg sector, tilt up to 0.363, body force $\phi \sim r^6 \cos 6\theta$) | done | |
| [0675](../../docs/adr/0675-f754-registration-rhie-chow-ddt-correction-on-coupled-faces.md) | 2026-11-17 | registration | F-754 | Rhie-Chow ddt correction on coupled faces | done; periodic run within `5.6e-15 U_ref` of its twin on 200 steps (limit `1e-8`) | 0687 |
| [0687](../../docs/adr/0687-f757-registration-rhie-chow-ddt-correction-on-coupled-faces-in-the-fractional-step-and-with-bdf2.md) | 2026-11-17 | registration | F-757 | The same in the fractional step and with BDF2 | done on the amended rule (P1b of the first set-up missed, diagnosed); roadmap partial | |
| [0691](../../docs/adr/0691-f771-registration-body-force-in-the-fractional-step.md) | 2026-11-17 | registration | F-771 | Body force in the fractional step (`fractional_body_force`, incremental form; Chorin refused) | done on the amended rule (S2a missed first); roadmap partial; default held 2026-11-18 | |

## Key numbers and limits

- ADR 0664 D0: default step-1 difference `3.494134721e-2` (limit `3.494135e-2` within `1e-8`); D1, D2: setting on against off bit for bit on translational pairs and on the annulus.
- ADR 0667 D0h: largest $G_{(6,0)}$ `1.483922e-4`; largest $G_*$ `5.890778e-3` (step 102).
- ADR 0687 question 2: a default BDF2 periodic PISO run differs from its twin by `4.2e-4 U_ref`; the plain fractional step's lagged partner gives `1.9e-2 U_ref`.
- ADR 0691 question 3: the lagged periodic partner of the plain fractional step costs `5.8e-2` of the start-up channel's mean at step 1 at the cut.

## Open questions

- ADR 0664 Q1 / Q2: implicit rotational pairs as default and as a case key (F-746, todo).
- ADR 0667 Q2, Q3; ADR 0675 Q3 (`coupled_ddt_correction` with `ddt_correction` as default); ADR 0687 questions 2 and 3: one combined default change, for the maintainer. ADR 0691: held on 2026-11-18.
- ADR 0664 question 3: the pressure-lag estimate and adaptive corrector count still on the lagged rotational coupling.
- ADR 0674 question 2: the pressure part on the sliding seam pairs not checked separately (recommendation leave).

## Related

- [[sliding-mesh]], [[piso]], [[component-diagonal]]
- [[adr-thread-sliding-mesh-rotor-stator]]
- [[roadmap-moving-meshes-rotors-and-coupling]], [[roadmap-numerics-and-solvers]]
