---
type: source
tags: [adr-thread, structural, fem, beams, warping, lateral-torsional-buckling, thin-walled]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# ADR thread: structural solver, the warping Beam2 (14 unknowns), lateral-torsional buckling and load height

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 09647baa (first written at 2e94e5ef; ADR 0778 added at 09647baa, the earlier ADRs unchanged since).
**Kind:** ADR thread (registrations with result addenda), 11 ADRs, all 2026-11-15. Job p3-structural-b. The six-unknown Beam2 (F-391) and the earlier I-section lateral-torsional buckling without warping (F-558, -19.2 percent) are on [[adr-thread-structural-beams-shells]].

[[beam-warping]] in [[rcfd-fem]]: an opt-in warping unknown for Beam2 (the 14-unknown element, module `rcfd_fem::beam_warping`), first on buckling (F-598), then on the static solve, modes and pre-stressed modes (F-626), monosymmetric sections (F-627), the harmonic and transient paths (F-635), the pre-stress on those paths (F-661, F-667), a negative-pivot count (F-668), and the height at which a load acts (F-643, F-663, F-670, F-673).

## Current state

- **Element.** Cubic Hermite twist with $E C_w$, the warping unknowns numbered after the six-unknown DofMap, per-section Wagner coefficients (F-598, [ADR 0546](../../docs/adr/0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md), done): I-beam lateral-torsional buckling under uniform moment within `5.9e-4` at 16 elements.
- **Paths.** Static solve, modes (14-unknown consistent mass, no warping inertia) and pre-stressed modes, with `WarpingPrestress::from_static` carrying the total torque (F-626, ADR 0548, done). `WarpingHarmonic` and `WarpingTransient` (F-635, ADR 0574, partial: H3, HW missed in supporting checks; acceptance met). Pre-stress on the harmonic (F-661, ADR 0585) and transient (F-667, ADR 0591) paths, $K_{\sigma}$ also in the Rayleigh stiffness (the convention of ADR 0543). `negative_eigenvalues` by a Sturm count of the reduced $K + K_{\sigma}$ (F-668, ADR 0592, done).
- **Monosymmetric sections** (F-627, ADR 0553): shear-centre offset, $\beta_x = \int z (y^2 + z^2) dA/I_y - 2 z_0$, bimoment load; partial by the registered rule, done by the status rule of 2026-11-18 (roadmap note).
- **Load height.** `Warping::load_point` adds $(1/2) (F \cdot (p - s)) \phi^2$ of a dead force at a section point (F-643, ADR 0582, done); `Warping::line_load_point` the consistent distributed term (F-663, ADR 0588, done); both reach the pre-stressed modes and harmonic path through $K_{\sigma}$ (F-670, ADR 0594, done, audit only). On a monosymmetric I the pre-stressed modes need a coupled offset mass (F-673, ADR 0597, partial: M0 known miss).
- **Latest decision:** the consistent mass of warping sections with a shear-centre offset (F-678) was **approved** by the maintainer on 2026-11-18 as a default change (ADR 0597 answers); F-678 is todo at the snapshot. The case-file key for the warping unknown with `geometric_terms` (F-625) was approved on 2026-11-18 under the additive-golden-edit class rule (ADR 0546 answers); F-625 is todo and its roadmap title still says "NEEDS THE USER: schema golden" ([[roadmap-structural-solver]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0546](../../docs/adr/0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md) | 2026-11-15 | registration | F-598 (done) | Warping DOF for Beam2 (14-DOF element), I-section lateral-torsional buckling | done: A1 `5.9e-4` at 16 elements | ADR 0548 |
| [ADR 0548](../../docs/adr/0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md) | 2026-11-15 | registration | F-626 (done) | Warping unknown on the static solve, modes and pre-stressed modes | done: twist `1.07e-6` of closed form | ADR 0574 |
| [ADR 0553](../../docs/adr/0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md) | 2026-11-15 | registration | F-627 (done) | Monosymmetric thin-walled sections (shear-centre offset, beta_x, bimoment) | partial by rule (amendments); item done by the status rule of 2026-11-18 | — |
| [ADR 0574](../../docs/adr/0574-f635-registration-warping-unknown-on-the-harmonic-and-transient-paths.md) | 2026-11-15 | registration | F-635 (partial) | Warping unknown on harmonic and transient paths | partial: H3, HW missed; resonance `3.6e-7` | ADR 0585 |
| [ADR 0582](../../docs/adr/0582-f643-registration-load-height-and-non-uniform-moment-for-monosymmetric-warping-beams.md) | 2026-11-15 | registration | F-643 (done) | Load height and non-uniform moment for monosymmetric warping beams | done: $\le 1.42 \times 10^{-3}$ vs `2e-2` | ADR 0588 |
| [ADR 0585](../../docs/adr/0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md) | 2026-11-15 | registration | F-661 (done) | Pre-stress on the warping harmonic path | done | ADR 0591 |
| [ADR 0588](../../docs/adr/0588-f663-registration-distributed-loads-at-a-height-on-warping-beams.md) | 2026-11-15 | registration | F-663 (done) | Distributed loads at a height on warping beams | done: $\le 1.72 \times 10^{-5}$ | ADR 0594 |
| [ADR 0591](../../docs/adr/0591-f667-registration-pre-stress-on-the-warping-transient-path.md) | 2026-11-15 | registration | F-667 (done) | Pre-stress on the warping transient path | done | ADR 0592 |
| [ADR 0592](../../docs/adr/0592-f668-registration-negative-pivot-count-of-the-pre-stressed-warping-solves.md) | 2026-11-15 | registration | F-668 (done) | Negative-pivot count of pre-stressed warping harmonic and transient solves | done | — |
| [ADR 0594](../../docs/adr/0594-f670-registration-load-height-on-the-pre-stressed-warping-modes-and-harmonic-path.md) | 2026-11-15 | registration | F-670 (done) | Load height on pre-stressed warping modes and harmonic path | done (audit; no code change) | ADR 0597 |
| [ADR 0597](../../docs/adr/0597-f673-registration-load-height-on-the-pre-stressed-modes-of-a-monosymmetric-warping-beam.md) | 2026-11-15 | registration | F-673 (partial) | Load height on pre-stressed modes of a monosymmetric warping beam | partial: M0 known miss; consistent offset mass approved 2026-11-18 | — |

## Key numbers and limits

- Lateral-torsional buckling, doubly symmetric I, uniform moment: `5.9e-4` at 16 elements; M1' met at `1.8e-15` (ADR 0546).
- Static twist of the cantilever `1.07e-6` of the closed form; LTB factor with torsion in the reference load equals the warping-torque split to `5.4e-8`, the Saint-Venant split `2.6e-2` away (ADR 0548).
- Monosymmetric I under uniform moment: `1.3e-3` (larger flange in compression), `3.9e-4` (tension) at 16 elements (ADR 0553).
- First torsional resonance `3.6e-7` from $\sqrt{(G J k^2 + E C_w k^4)/(\rho I_p)}$ at 16 elements; step-torque transient `5.4e-3` of the static twist, energy conserved to `3e-13` (ADR 0574).
- Load height, point load: `5.3e-4`, `3.5e-4`, `7.2e-4`, `1.42e-3`, `6.9e-4` against Rayleigh-Ritz references converged to `3.2e-6`, limit `2e-2` (ADR 0582); distributed: `1.72e-5`, `1.70e-5`, `7.4e-6` (ADR 0588); on pre-stressed paths $\le 8.3 \times 10^{-4}$ against `1e-2` (ADR 0594); monosymmetric $\le 2.26 \times 10^{-3}$ against `1e-2`, offset mass `5e-4` at 16 elements (emulated consistent form `7e-7`) (ADR 0597).

## Open questions

- Warping inertia $\rho C_w$ (ADR 0548 Q1): recommendation none until a check needs it (`7e-4` of the first torsional frequency).
- The element moment under a line load (ADR 0588 Q2): the parabolic moment would change existing results, left open for the maintainer; recommendation keep the linear $L^2$ projection.
- Tie the entry's force to the pre-stress (ADR 0594 Q1): restated now, read from the static model's line load with a case-file key (F-625).
- The transient path with a load height (ADR 0594): a small extension of F-667 if a case needs it.

## Four supporting checks of reader trial 1 (ADR 0778, F-850; added 2026-11-23)

[ADR 0778](../../docs/adr/0778-f850-registration-four-supporting-checks-of-reader-trial-1.md) (registered and run 2026-11-23, merged 10fea82e, **done**) is filed here because two of its four clauses hold records of this thread (ADRs 0553 and 0585); the other two hold ADR 0570 ([[adr-thread-structural-contact]]) and ADR 0453 ([[adr-thread-structural-fatigue-and-radiated-sound]]). Its source is "the check of the 16 agreed findings" of reader trial 1 (`docs/process/trials/reader-trial-1/registration.md`); "none changes a result or a status". Test-only, in new test files: "no existing test or record is edited, no library line changes".

| Clause | Record held | Limit | Measured |
|---|---|---|---|
| (1) A1'', A2'' | the 16-element factors of ADR 0553 A1' and A2' against the closed form | each $\le 2\times10^{-2}$ | 1.2869e-3 and 3.8913e-4 (factors 1.78684213e5 against 1.78454564e5; 5.39797922e4 against 5.39587954e4) |
| (2) HB- | the check HB- of ADR 0585 must catch a mutation of the warping geometric stiffness | $\le 10^{-10}$ | 0 unmutated; the committed mutation (the axial force in the slot of $M_y$) gives 0.899 and is caught |
| (3) HS20m-M', HS10m-M' | the moment-radius readings of ADR 0570 | 0.08 | 1.352480e-2 and 3.401690e-2 (heavy records, 54.3 s and 83.7 s) |
| (3) NC | a uniform pressure on the Hertz disc, which the limit must refuse | each > 0.08 | 0.1301 and 0.1388 (continuum 0.118): refused by 0.08, "both would pass the 0.15 of ADR 0570" |
| (4) G2= | the Gerber message of ADR 0453 G2 | equality | the three reasons equal the formatted and the literal text |

- 4 of 4 mutations caught. The lead notes that mutation M4 "is caught only by the comparison with ADR 0553's printed digits, because the test's own closed form shares the mutated function".
- The two moment-radius readings are ignored heavy records; "if the lead wants them in the gate, that is a new test file of a later item, since a committed test is not edited". No open question for the maintainer. See [[beam-warping]], [[contact-mechanics]], [[fatigue]], [[mutation-testing]].

## Related

- [[beam-warping]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-beams-shells]], [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-dynamics-harmonic]]
