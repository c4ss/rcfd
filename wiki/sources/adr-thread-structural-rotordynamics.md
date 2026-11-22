---
type: source
tags: [adr-thread, structural, fem, rotordynamics, campbell, whirl, coriolis]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, rotating parts (stress stiffening, Coriolis, whirl, Campbell, rotors on bearings)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 20 ADRs, 2026-11-13 to 2026-11-14. Later shell and beam whirl ADRs (F-647, F-654, F-657, F-675, F-679) and the split-mode Schur follow-ups (F-584, F-612) are in the other half of the category.

Modes of a rotating part in [[rcfd-fem]]: stress stiffening by the centrifugal pre-stress and spin softening (F-396), then the Coriolis (gyroscopic) coupling, whirl modes and their stability, the [[campbell-diagram]] by harmonic family on a [[cyclic-symmetry]] sector, sparse whirl solvers for large parts, and a flexible rotor of Beam2 shafts on bearings in the stationary frame (F-416).

## Current state

- **Stress stiffening and spin softening:** F-396 (ADR 0403) is **partial**: the flap-lag identity misses its limit on the beam; on a 3-D solid the difference is `(1 - 2 f_x) Omega^2`, met only in that restated form (1.1e-8). F-498 (ADR 0408) restates the acceptance (done). Rotating modes of a cyclic sector by index (F-500, ADR 0407) are partial by a tracking check met only after amendment 1.
- **Coriolis and whirl:** the gyroscopic matrix and whirl modes by a symmetric linearisation (F-499, ADR 0411, done); above the loss of stiffness by the state matrix with Hessenberg reduction and Francis double-shift QR (F-505, ADR 0416, partial by a corrected set-up); stationary-frame frequencies of a cyclic whirl table (F-507, ADR 0419, done); whirl stability of a cyclic sector (F-511, ADR 0420, done); odd-index instability on the model path (F-510, ADR 0424, done).
- **Large parts:** shift-invert skew Lanczos (F-506, ADR 0427, done); a shift-invert Arnoldi state solver above the loss of stiffness (F-519, ADR 0428, partial: far-mode vectors 8.5e-8 against 1e-8) refined by inverse iteration (F-520, ADR 0442, done).
- **On the modal subspace:** the Coriolis term by the general eigensolver of ADR 0450 and the split Campbell table (F-415, ADR 0472, partial: zero-speed frequencies miss 1e-10); the per-mode Rayleigh-quotient report (F-554, ADR 0484, done); the split Campbell table of a cyclic sector (F-555, ADR 0481, partial); its static correction vectors (F-569, ADR 0482, partial) and the second-order Ritz error (F-575, ADR 0492, done).
- **Flexible rotor on bearings** (`rcfd_fem::rotor`): F-416 (ADR 0476) partial (label check L1 met only with the tilt orbit after amendment 2); speed-dependent bearing tables (F-563, ADR 0498, done); damped critical speeds as unbalance-response peaks and checked Campbell crossings (F-564, ADR 0501, done); damped massless unknowns as first-order unknowns (F-562, ADR 0504, partial: tighter limits missed).
- There is no fan-rotor demonstration yet: F-406 (Campbell diagram of the blade, bladed disc, blade-passing forcing) is todo ([[roadmap-structural-solver]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0403](../../docs/adr/0403-f396-registration-rotating-part-modes-and-campbell-table.md) | 2026-11-13 | registration (hub) | F-396 | Stress stiffening, spin softening, mode tracking, Campbell table | partial; A3 flap-lag 1.9e-3 against 1e-4 Omega^2 | 0408 |
| [0407](../../docs/adr/0407-f500-registration-rotating-modes-of-a-cyclic-sector.md) | 2026-11-13 | registration | F-500 | Rotating modes of a cyclic sector by harmonic index, Campbell table by family | partial; P2 tracking 0.895 against 0.9 | |
| [0408](../../docs/adr/0408-f498-registration-flap-lag-identity-of-a-rotating-solid-beam.md) | 2026-11-13 | restatement | F-498 | Flap-lag identity of a rotating solid beam, acceptance restated (1 - 2 f_x) | done (numbers known from ADR 0403, disclosed) | |
| [0411](../../docs/adr/0411-f499-registration-coriolis-gyroscopic-whirl-modes.md) | 2026-11-13 | registration | F-499 | Gyroscopic matrix, whirl modes by symmetric linearisation, forward/backward whirl | done | |
| [0416](../../docs/adr/0416-f505-registration-gyroscopic-stabilisation.md) | 2026-11-13 | registration | F-505 | Whirl above the loss of stiffness (state matrix, Hessenberg + Francis QR), stability flag | partial; odd-index check met on a corrected set-up | |
| [0419](../../docs/adr/0419-f507-registration-stationary-frame-whirl.md) | 2026-11-13 | registration | F-507 | Stationary-frame frequencies `omega + tau k Omega` of a cyclic whirl table | done | |
| [0420](../../docs/adr/0420-f511-registration-sector-whirl-stability.md) | 2026-11-13 | registration | F-511 | Whirl stability of a cyclic sector above the loss of stiffness | done; eight mutations caught | |
| [0424](../../docs/adr/0424-f510-registration-odd-index-instability.md) | 2026-11-13 | registration | F-510 | Odd-index instability on the model path (cube on springs, closed-form divergence) | done | |
| [0427](../../docs/adr/0427-f506-registration-sparse-whirl-lanczos.md) | 2026-11-14 | registration | F-506 | Sparse whirl solver: shift-invert skew Lanczos, one sparse factor of `K_eff` per speed | done | |
| [0428](../../docs/adr/0428-f519-registration-sparse-whirl-state.md) | 2026-11-14 | registration | F-519 | Sparse state solver above the loss (shift-invert Arnoldi) | partial; Q1 8.5e-8 against 1e-8 on far mode vectors | 0442 |
| [0442](../../docs/adr/0442-f520-registration-far-mode-vectors.md) | 2026-11-14 | registration | F-520 | Far-mode vectors refined by inverse iteration on `Q(s)` | done (opt-in) | |
| [0472](../../docs/adr/0472-f415-registration-coriolis-split-campbell-modal-subspace.md) | 2026-11-14 | registration | F-415 | Coriolis on the modal subspace, split Campbell table, stability | partial; ring 4.6e-10, beam 1.0e-10 against 1e-10 | 0484 |
| [0476](../../docs/adr/0476-f416-registration-flexible-rotor-on-bearings-stationary-frame.md) | 2026-11-14 | registration (hub) | F-416 | Flexible rotor of Beam2 shafts, discs, bearings with cross-coupling; whirl, critical speeds | partial; L1 met after amendment 2 | |
| [0481](../../docs/adr/0481-f555-registration-split-campbell-of-a-cyclic-sector-on-the-modal-subspace.md) | 2026-11-14 | registration | F-555 | Split Campbell table of a cyclic sector on the modal subspace | partial; one mutation not caught | |
| [0482](../../docs/adr/0482-f569-registration-static-correction-vectors-for-sector-split-modes.md) | 2026-11-14 | registration | F-569 | Static correction vectors for sector split modes, Ritz error eta | partial; err/eta 2.62 on k = 1 to 3 | 0492 |
| [0484](../../docs/adr/0484-f554-registration-zero-speed-split-rayleigh-quotients.md) | 2026-11-14 | registration | F-554 | Zero-speed split frequencies against Rayleigh quotients of the F-396 basis | done | |
| [0492](../../docs/adr/0492-f575-registration-schur-ritz-error-of-corrected-sector-split-modes.md) | 2026-11-14 | registration | F-575 | Second-order Ritz error (Schur-complemented projected problem) | done; schur_eta within 3.4e-4 of the measured error | |
| [0498](../../docs/adr/0498-f563-registration-speed-dependent-bearing-coefficients.md) | 2026-11-14 | registration | F-563 | Speed-dependent bearing tables k(Omega), c(Omega) | done | |
| [0501](../../docs/adr/0501-f564-registration-damped-critical-speeds-peaks-and-checked-campbell-crossings.md) | 2026-11-14 | registration | F-564 | Damped critical speeds as unbalance-response peaks, checked Campbell crossings | done | |
| [0504](../../docs/adr/0504-f562-registration-damped-massless-unknowns-first-order.md) | 2026-11-14 | registration | F-562 | Damped massless rotor unknowns as first-order unknowns | partial; A2, B1r, N3, A3 tighter limits missed | |

## Key numbers and limits

- F-396 gap: "The flap-lag identity misses its limit: 1.9e-3 against 1e-4 Omega^2 on the beam. On a solid it is 1 - 2 f_x, which is met only in that restated form (1.1e-8)" ([[roadmap-structural-solver]]).
- F-500 gap: "sector frequencies equal the full rotor's within 8.8e-14. The registered tracking check missed (0.895 against 0.9, degenerate pairs of k = N/2) and was met only after amendment 1 (0.99998)".
- F-505 (ADR 0416): the isotropic point mass "within 3.3e-15 relative and flagged stable, the anisotropic band with edges within 2.2e-15".
- F-415 gap: "Zero-speed frequencies against F-396 miss the 1e-10 limit on the ring (4.6e-10) and the beam (1.0e-10), because the F-396 rest eigenvalues differ from their vectors' Rayleigh quotients by up to 9e-10".
- F-555 gap: "223 to 233 of 246 modes inside the basis within 8.3e-8 and 9.9e-8".
- F-569 gap: "err/eta reaches 2.62 on k = 1 to 3 against the 10 percent agreement"; F-575 "schur_eta within 3.4e-4 of the measured error".
- F-562 gap: "against the existing whirl on the 1e-9 model the difference is 3.7e-7 and 2.0e-6, and whirl itself still refuses a massless damped unknown".

## Open questions

- ADR 0411 Q1 (speeds above the loss of stiffness) was answered by the follow-up F-505.
- ADR 0419 Q1: aliased lines `omega + (k + mN) Omega` and standing waves, options (a) leave out, (b) per-mode list with energy weights.
- ADR 0424 Q1: a negative support as a model support (e.g. a magnetic bearing's negative axial stiffness in a case file).
- F-500, F-505, F-555: "The maintainer decides done" (roadmap gaps).

## Second half of the category (ADRs 0527 to 0602, job p3-structural-b)

The cyclic constraint with shells and beams, `full_rotor` and the free-sector centrifugal pre-stress are added to [[adr-thread-structural-modal-buckling-cyclic]].

### Current state (second half)

- **Split-mode Schur error.** Opt-in `RotatingOptions::corrected_frequency` and `SplitSchur::Perturbation` (one complement solve per split mode, second-order Rayleigh-Schroedinger sum) (F-584, ADR 0527, partial: G1, G2, G4 missed, a3 `0.38` against `0.01`); `SplitSchur::PerturbationChecked` with the series ratio `|delta_2/delta_1|` and a dense fallback above `0.08` (F-612, ADR 0535, partial: the cost clause b4 `0.83` against `0.5`).
- **Shell whirl.** With `CyclicOptions::shells` the rotating sector's modes, pre-stressed and spin-softened modes and Campbell table take Shell4 and Shell3; the whirl paths take them through a new Coriolis matrix of the shell director (`coriolis_matrix_with_shells`, `RotatingOptions::shells`) (F-647, ADR 0561, partial: CO1 missed in run 1, a library defect). Independent references: the free thin ring as a cylindrical band against the closed form of the extensible thin ring in the rotating frame (F-654, ADR 0569, partial: a sign in the reference and a negative control's rounding limit); the spinning annular disc of shells, the first independent reference of the rotary block `m2 W^T [a x] W` (F-657, ADR 0573, done).
- **Beam whirl.** A Coriolis matrix of the Beam2 section kinematics `u + theta x s` for the whirl paths on a cyclic beam sector (F-675, ADR 0598, partial by an amended check); an independent reference, the free spinning Beam2 ring at nodal diameters 2 and 3 against the exact rotating-frame solution (F-679, ADR 0602, done).
- **Latest decision:** the whirl paths and `full_rotor` take a beam sector without `CyclicOptions::beams` and `full_rotor_beams`; approved by the maintainer on 2026-11-21 ("Yes, both", ADR 0744 answers), to be made "in one further registration under F-672"; not made at the snapshot.

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0527](../../docs/adr/0527-f584-registration-corrected-frequency-and-perturbation-schur-error.md) | 2026-11-14 | registration | F-584 (partial) | Corrected frequency and second-order perturbation Schur Ritz error of split modes | partial: G1, G2, G4 missed (a3 0.38 vs 0.01) | ADR 0535 |
| [ADR 0535](../../docs/adr/0535-f612-registration-accuracy-indicator-and-dense-fallback-of-the-perturbation-schur-error.md) | 2026-11-14 | registration | F-612 (partial) | Series-ratio indicator of the perturbation Schur error and dense fallback | partial: J6 cost 0.83 vs 0.5 | — |
| [ADR 0561](../../docs/adr/0561-f647-registration-shells-on-the-rotating-cyclic-paths-and-the-full-rotor.md) | 2026-11-15 | registration | F-647 (partial) | Shells on rotating cyclic paths and full rotor (shell-director Coriolis) | partial: CO1 missed run 1 (library defect) | ADR 0569 |
| [ADR 0569](../../docs/adr/0569-f654-registration-independent-whirl-reference-for-spinning-shells.md) | 2026-11-15 | registration | F-654 (partial) | Independent whirl reference for spinning shells (free thin ring) | partial: KA sign in reference, NC1 rounding | ADR 0573 |
| [ADR 0573](../../docs/adr/0573-f657-registration-whirl-of-a-spinning-annular-disc-of-shells.md) | 2026-11-15 | registration | F-657 (done) | Whirl of a spinning annular disc of shells (rotary block of shell Coriolis) | done | — |
| [ADR 0598](../../docs/adr/0598-f675-registration-rotating-paths-on-a-cyclic-sector-of-beams.md) | 2026-11-15 | registration | F-675 (partial) | Rotating paths on a cyclic sector of beams (Beam2 Coriolis) | partial | ADR 0602 |
| [ADR 0602](../../docs/adr/0602-f679-registration-an-independent-whirl-reference-for-spinning-beams.md) | 2026-11-15 | registration | F-679 (done) | Independent whirl reference for spinning beams (free Beam2 ring) | done | ADR 0606 |

### Key numbers and limits (second half)

- Perturbation threshold of `PerturbationChecked`: `0.08` (the truncation miss is about the square of the ratio, so 1 percent is crossed at `0.1`); b3 `6.3e-3` against `1e-2` (ADR 0535).
- Free thin ring (`R = 0.1 m`, `t = 0.5 mm`, `L = 5 mm`, `nu = 0`, 32 sectors): frequencies within `0.25 (k Delta)^2` (Shell4) and `0.5 (k Delta)^2` (Shell3) of the closed form; inextensional split `4 n Omega / (n^2 + 1)` (ADR 0569).

### Open questions (second half)

- The public `coriolis_matrix` and `gyro::gyroscopic_matrix` stay solid-only (ADR 0561 Q2, ADR 0598 Q2): recommendation leave.
- Rotary spin softening and `t^2/12` shell geometric terms are below the resolution of the checks (ADR 0573 Q2): recommendation leave.
- `SplitSchur::Perturbation` cost (ADR 0535 Q1): recommendation keep as built.

## Related

- [[campbell-diagram]], [[cyclic-symmetry]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-beams-shells]] (Beam2 shafts, shell stress stiffening)
- [[fan-01]] (the fan target that F-406 and F-436 would demonstrate on)
