---
type: source
tags: [adr-thread, structural, fem, transient, harmonic, random-vibration, damped-modes, substructuring]
sources: [doc-reference-data-f414-eigensolver]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, linear dynamics (transient, harmonic, damped modes, random vibration, substructuring)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 14 ADRs, 2026-11-13 to 2026-11-14. Later follow-ups (F-528, F-553, F-603, F-619, the viscoelastic damped modes F-586, F-609) are in the other half of the category.

Linear dynamics of [[rcfd-fem]]: Newmark and generalised-alpha time integration with Rayleigh damping and load tables (F-398), harmonic response by mode superposition (F-399) and by a direct complex solve (F-412), damped and complex modes from a dense general eigensolver (F-414), random vibration by modal superposition (F-417) with cross-spectra (F-567), and substructuring by static condensation and Craig-Bampton component mode synthesis (F-430, F-552, F-559).

## Current state

- **Transient:** `transient::Integrator` (Newmark, generalised-alpha) is in; F-398 (ADR 0389) stays **partial** on clause A8 (the generalised-alpha decay at `omega dt = 1e3`, ADR 0389 Q1 open). On the transformation and with prescribed motion histories (F-490, ADR 0393) done; the isotropic equivalence with a limit from a rounding probe (F-545, ADR 0463) done.
- **Harmonic:** mode superposition with modal damping, static correction, effective mass and force harmonics (F-399, ADR 0391, done); static correction for free bodies by inertia relief (F-491, ADR 0400, done); direct complex symmetric envelope `L D L^T` with loss factors, dashpots and base motion (F-412, ADR 0448, **partial**: S1 missed, `HarmonicResponse::stress` lacks the loss part; `DirectHarmonic::stress` added).
- **Damped modes:** dense general real eigensolver and the quadratic problem on a modal subspace (F-414, ADR 0450, done; reference data on [[doc-reference-data-f414-eigensolver]]); static residual vectors for boundary dampers (F-532, ADR 0464, done); a flag for pseudo-mode roots carried by the residual vectors (F-548, ADR 0467, partial: D3 met only after amendment).
- **Random vibration:** force and base-acceleration spectra, spectral moments, RMS displacement, stress and von Mises (F-417, ADR 0477, done); complex cross-spectral matrices and frequency-dependent coherence (F-567, ADR 0510, done).
- **Substructuring:** static condensation and fixed-interface CMS (F-430, ADR 0469, partial: H1 missed as predicted), residual vectors of interior loads with modes by cutoff (F-552, ADR 0474, partial), residual vectors of caller load patterns (F-559, ADR 0511, partial: two clauses hold only with the interface enrichment of F-603).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0389](../../docs/adr/0389-f398-registration-linear-transient-newmark-generalised-alpha.md) | 2026-11-13 | registration (hub) | F-398 | Newmark and generalised-alpha, Rayleigh damping, load tables | ADR done after two amendments; item partial (A8 known miss) | |
| [0391](../../docs/adr/0391-f399-registration-harmonic-response-by-mode-superposition.md) | 2026-11-13 | registration (hub) | F-399 | Harmonic response by mode superposition, damping, static correction, effective mass | done | |
| [0393](../../docs/adr/0393-f490-registration-transient-transformation-and-prescribed-motion.md) | 2026-11-13 | registration | F-490 | Transient integrator on the transformation and with prescribed motion | done | |
| [0400](../../docs/adr/0400-f491-registration-harmonic-static-correction-free-bodies-and-transformation.md) | 2026-11-13 | registration | F-491 | Static correction for free bodies (inertia relief), transformation, pre-stressed basis | done | |
| [0448](../../docs/adr/0448-f412-registration-direct-harmonic-complex-ldlt.md) | 2026-11-14 | registration | F-412 | Direct harmonic solve, loss factors, dashpots, base motion | partial; S1 2.0e-2 missed, R8 after amendment 2 | (F-528, other half) |
| [0450](../../docs/adr/0450-f414-registration-damped-modes-general-eigensolver.md) | 2026-11-14 | registration | F-414 | Dense general eigensolver, quadratic eigenproblem, damped and complex modes | done | |
| [0463](../../docs/adr/0463-f545-registration-transient-isotropic-equivalence-probed-limit.md) | 2026-11-14 | registration | F-545 | Transient isotropic equivalence with a limit from a rounding probe | done; 9.25e-12 against 5e-11 | |
| [0464](../../docs/adr/0464-f532-registration-damped-modes-residual-vectors.md) | 2026-11-14 | registration | F-532 | Static residual vectors for damped modes (dampers near the impedance) | done | 0467 |
| [0467](../../docs/adr/0467-f548-registration-damped-modes-pseudo-mode-flag.md) | 2026-11-14 | registration | F-548 | Flag the damped-mode roots carried by residual vectors | partial; D3 missed, D3b after amendment | |
| [0469](../../docs/adr/0469-f430-registration-substructuring-craig-bampton.md) | 2026-11-14 | registration (hub) | F-430 | Static condensation and fixed-interface CMS (Craig-Bampton) | partial; H1 0.997 against 1 percent at 8241 rad/s | 0474 |
| [0474](../../docs/adr/0474-f552-registration-craig-bampton-residual-vectors-cutoff.md) | 2026-11-14 | registration | F-552 | Residual vectors of the interior loads, kept modes by cutoff | partial; H1 0.494 at 15107 rad/s | 0511 |
| [0477](../../docs/adr/0477-f417-registration-random-vibration-by-modal-superposition.md) | 2026-11-14 | registration | F-417 | Random vibration by modal superposition | done | |
| [0510](../../docs/adr/0510-f567-registration-complex-cross-spectra-and-frequency-dependent-coherence-of-random-loads.md) | 2026-11-14 | registration | F-567 | Complex cross-spectral matrices, frequency-dependent coherence | done; M7 equivalent | |
| [0511](../../docs/adr/0511-f559-registration-craig-bampton-residual-vectors-of-load-patterns.md) | 2026-11-14 | registration | F-559 | CB residual vectors of caller load patterns, harmonic acceptance at resonance | partial; 5.0e-3 and 0.104 missed as predicted | (F-603, other half) |

## Key numbers and limits

- F-398 (ADR 0389): bar of 20 Hex8, amplitude after 100 periods 9e-15, energy 2.5e-14. Gap: "at omega dt = 1e3 the generalised-alpha mode decays by 0.50785 and 0.80772, not by rho_inf within 1e-3 (it does at 1e6, 7.8e-5)" ([[roadmap-structural-solver]]).
- F-412 gap: "Stress check S1 missed (2.0e-2) ... DirectHarmonic::stress was added (7.2e-6)".
- F-430 gap: "0.997 against 1 percent limit at 8241 rad/s, with no axial mode kept and no residual flexibility of the interior load".
- F-552 gap: "harmonic error 0.494 at 15107 rad/s with the cutoff ... 1.04 at 8241 with five modes".
- F-559 gap: "frequencies below half the lowest omitted frequency within 1e-3 (5.0e-3 at 15027 rad/s) and off-resonance response within 1 percent (0.104 at 15778 rad/s). They hold only when the interface enrichment of F-603 (ADR 0532) is added".

## Open questions

- ADR 0389 Q1: re-word A8 to the closed-form spectral radius (maintainer).
- F-559: "restate the clauses or take F-603" (roadmap gap, maintainer).
- F-548: "The maintainer decides done".

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-fatigue-and-radiated-sound]] (random stress moments feed broadband fatigue)
- [[adr-thread-structural-materials-nonlinear]] (viscoelastic material in the harmonic solve and damped modes)
