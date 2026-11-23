---
type: source
tags: [adr-thread, structural, fem, transient, harmonic, random-vibration, damped-modes, substructuring]
sources: [doc-reference-data-f414-eigensolver]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# ADR thread: structural solver, linear dynamics (transient, harmonic, damped modes, random vibration, substructuring)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 14 ADRs, 2026-11-13 to 2026-11-14. Later follow-ups (F-528, F-553, F-603, F-619, the viscoelastic damped modes F-586, F-609) are in the other half of the category.

Linear dynamics of [[rcfd-fem]]: Newmark and generalised-alpha time integration with Rayleigh damping and load tables (F-398), harmonic response by mode superposition (F-399) and by a direct complex solve (F-412), damped and complex modes from a dense general eigensolver (F-414), random vibration by modal superposition (F-417) with cross-spectra (F-567), and substructuring by static condensation and Craig-Bampton component mode synthesis (F-430, F-552, F-559).

## Current state

- **Transient:** `transient::Integrator` (Newmark, generalised-alpha) is in; F-398 (ADR 0389) stays **partial** on clause A8 (the generalised-alpha decay at $\omega \, dt = 10^{3}$, ADR 0389 Q1 open). On the transformation and with prescribed motion histories (F-490, ADR 0393) done; the isotropic equivalence with a limit from a rounding probe (F-545, ADR 0463) done.
- **Harmonic:** mode superposition with modal damping, static correction, effective mass and force harmonics (F-399, ADR 0391, done); static correction for free bodies by inertia relief (F-491, ADR 0400, done); direct complex symmetric envelope $L D L^T$ with loss factors, dashpots and base motion (F-412, ADR 0448, **partial**: S1 missed, `HarmonicResponse::stress` lacks the loss part; `DirectHarmonic::stress` added).
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
- The reduction of ADR 0469, the interface enrichment of ADR 0532 and the superelements of ADR 0536 are reachable from a solid case file since ADR 0785 (F-616, done 2026-11-23): static solve and modal analysis only; "the harmonic solve of a combined model from a case file belongs to the item that gives the solid case file a harmonic analysis" (maintainer, 2026-11-23). See [[adr-thread-structural-case-file-and-fan-blade]].

## Second half of the category (ADRs 0520 to 0565, job p3-structural-b)

The mutual radiation impedance (F-579, ADR 0538) is added to [[adr-thread-structural-fatigue-and-radiated-sound]]; the warping-beam harmonic and transient paths to [[adr-thread-structural-beam-warping]]; buckling and harmonic response about a nonlinear state to [[adr-thread-structural-hyperelastic-and-follower]].

### Current state (second half)

- **Viscoelastic damped modes at each root's own frequency** (F-586, ADR 0520, partial): `DampedOptions::viscoelastic_own_frequency(OwnFrequency::new(roots))`, update $w \leftarrow \operatorname{Im} \lambda$ of the nearest root, converged at $\lvert dw \rvert \le 10^{-12} w$, refused after 50 iterations; rod A's first root equals the chain root within `1.1e-13` (limit `1e-8`). Pseudo-modes in that iteration are skipped by the opt-in `OwnFrequency::skip_pseudo` (F-609, ADR 0529, done).
- **Craig-Bampton interface enrichment** by Krylov blocks started from the constraint modes (F-603, ADR 0532, done): default 0 blocks (bit-identical); one block (the residual flexibility of the constraint modes) gave `3e-6` frequencies and `1e-3` response to half the lowest omitted frequency on the bar, two blocks `1e-10` and `3e-5`.
- **Reduced parts as superelements** in an ordinary model (F-553, ADR 0536, done). The case-file keys `superelements` and `interface_enrichment` were **approved** on 2026-11-18 under the additive-golden-edit class rule (ADR 0536 answers); F-616 is todo at the snapshot.
- **Direct harmonic on the transformation path** (constraints, rigid sets, elastic supports, $x = T q$ of ADR 0296) and with a pre-stress (F-528, ADR 0543, partial by the rule, recommended to accept as done): bar on a tip spring within `3.9e-13` of the 1-D chain (limit `1e-8`). Base motion and the six complex reference unknowns of each rigid set by `response_with_references` (F-619, ADR 0565, done).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0520](../../docs/adr/0520-f586-registration-viscoelastic-damped-modes-at-each-roots-own-frequency.md) | 2026-11-14 | registration | F-586 (partial) | Viscoelastic damped modes, each root iterated to its own frequency | partial (known misses RV1) | ADR 0529 (pseudo-modes) |
| [ADR 0529](../../docs/adr/0529-f609-registration-pseudo-modes-in-the-viscoelastic-own-frequency-iteration.md) | 2026-11-14 | registration | F-609 (done) | Pseudo-modes in the viscoelastic own-frequency iteration | done (`OwnFrequency::skip_pseudo`, opt-in) | — |
| [ADR 0532](../../docs/adr/0532-f603-registration-craig-bampton-interface-enrichment.md) | 2026-11-14 | registration | F-603 (done) | Craig-Bampton interface enrichment by Krylov blocks from the constraint modes | done | — |
| [ADR 0536](../../docs/adr/0536-f553-registration-reduced-parts-as-superelements.md) | 2026-11-14 | registration | F-553 (done) | Reduced parts as superelements in an ordinary model | done; case-file keys approved 2026-11-18 | — |
| [ADR 0543](../../docs/adr/0543-f528-registration-direct-harmonic-solve-on-the-transformation-path-and-with-pre-stress.md) | 2026-11-15 | registration | F-528 (partial) | Direct harmonic solve on the transformation path and with pre-stress | partial by rule (recommended done) | ADR 0565 |
| [ADR 0565](../../docs/adr/0565-f619-registration-direct-harmonic-base-motion-and-reference-response-on-the-transformation-path.md) | 2026-11-15 | registration | F-619 (done) | Direct harmonic base motion and reference response on the transformation path | done | — |

### Open questions (second half)

- Default number of enrichment blocks (ADR 0532 Q2): recommendation keep 0; large interfaces may need an interface reduction first (Q3).
- Harmonic damping of superelements beyond loss factors, and a solid part at shell or beam host nodes (ADR 0536 Q2, Q3).
- Whether F-528 is accepted as done (ADR 0543 remaining question 1); the roadmap lists it partial.

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-fatigue-and-radiated-sound]] (random stress moments feed broadband fatigue)
- [[adr-thread-structural-materials-nonlinear]] (viscoelastic material in the harmonic solve and damped modes)
