---
type: source
tags: [adr-thread, structural, fem, creep, thermal, temperature-field, finite-strain, shells]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, creep at finite strain and temperature on the nonlinear paths

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 11 ADRs, 2026-11-14 to 2026-11-15. Job p3-structural-b. Power-law creep, viscoelasticity, temperature-tabulated materials and the B-bar/F-bar Hex8 creep cylinder of ADR 0506 (C4 missed) are on [[adr-thread-structural-materials-nonlinear]]; shell elements on [[adr-thread-structural-beams-shells]].

Two strands in [[rcfd-fem]]: (1) the thick-walled creep cylinder whose growth bound ADR 0506 missed, traced to a finite-strain pressure drift of the F-bar Hex8 (F-597, F-580, F-636, F-637); (2) temperature on the nonlinear paths: a uniform history, a nodal field with thermal strain, a thermal predictor, an Arrhenius creep factor, and through-thickness temperature gradients on shells (F-592, F-613, F-617, F-581, F-651, F-527, F-687).

## Current state

- **Creep cylinder.** Re-registered at `p/10`, the B-bar creep cylinder meets its growth bound (`C4' 1.06e-4` against `2e-3`) (F-597, ADR 0528, partial: three run-1 witnesses missed because the locking shows as a pressure oscillation). Against a one-dimensional finite-strain reference written in the test, the growth `F3` is `1.51e-3` within `2e-3`, where ADR 0506 had missed against the small-strain closed form at `6.02e-3` (F-580, [ADR 0549](../../docs/adr/0549-f580-registration-creep-on-the-b-bar-hex8-against-a-finite-strain-reference.md), partial: MU3, MU4 not caught). A bore-zone witness (runs at $p$ and `p/100`) catches those mutations; growth `4.34e-3` within `6e-3` (F-636, ADR 0563, done).
- **Diagnosis (F-637, ADR 0568, partial).** The growth is a pressure error of the F-bar Hex8 that is finite-strain in origin and `O(h)` in size: the law's pressure $\kappa \operatorname{tr} \bar{E}$ contains $\kappa \sum e'^2$, which the continuum absorbs in $J$ while the F-bar element has one $J$. An opt-in $J$-based volumetric variant of the creep material was approved on 2026-11-18 under the additive-golden-edit rule (ADR 0568 answers); not built at the snapshot.
- **Temperature on the nonlinear paths.** The quasi-static driver and the nonlinear transient integrator take a uniform temperature history, with the Rayleigh stiffness tangent at the temperature (F-592, ADR 0526, done), a nodal temperature field against time with the multiplicative thermal stretch $F_{\mathrm{th}} = (1 + \alpha dT) I$ (F-613, ADR 0534, partial; refused with shells, BBar blocks, the energy-momentum step and for the strain energy), and an opt-in thermal predictor that starts each attempt at the free thermal expansion of the field change (F-617, ADR 0539, done).
- **Arrhenius creep.** `Material::creep_arrhenius(base, a, n, m, Q)`, rate $A \exp(-Q/(R T)) q^n t^m$, `activation: None` keeps the bits (F-581, ADR 0558, partial: MU5 caught only by a check added after the run); every non-positive temperature is refused before any step, including in the nonlinear transient integrator (F-651, ADR 0578, done).
- **Shell temperature gradients.** `Load::ShellTemperatureGradient` (K/m along the normal, one value per node) on Shell4 (F-527, ADR 0612, partial: free-plate rows miss by solve rounding growing as $(h/t)^2$) and on Shell3 and Layup Shell4 with classical lamination theory (F-687, ADR 0626, partial: MU12 equivalent, an amendment after the run).
- No case-file keys: a temperature history key (ADR 0526 Q1), creep keys (ADR 0558 Q2) and a gradient key (ADR 0612 Q2) are recommended for later and need a schema decision.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0526](../../docs/adr/0526-f592-registration-temperature-history-on-the-quasi-static-driver-and-the-nonlinear-transient-integrator.md) | 2026-11-14 | registration | F-592 (done) | Uniform temperature history on the quasi-static driver and nonlinear transient | done | ADR 0534 |
| [ADR 0528](../../docs/adr/0528-f597-registration-finite-strain-creep-cylinder-growth-bound-and-full-solve-bbar-recovery-control.md) | 2026-11-14 | registration | F-597 (partial) | B-bar creep cylinder growth bound re-registered at p/10; Full-solve locking control | partial: LC-u5, LC-u20, LC-e20 missed in run 1 | ADR 0549 |
| [ADR 0534](../../docs/adr/0534-f613-registration-nodal-temperature-field-with-thermal-strain-on-the-nonlinear-paths.md) | 2026-11-14 | registration | F-613 (partial) | Nodal temperature field against time on the nonlinear paths, with thermal strain | partial | ADR 0539 |
| [ADR 0539](../../docs/adr/0539-f617-registration-thermal-predictor-for-the-attempts-of-the-nonlinear-paths-with-a-temperature-field.md) | 2026-11-15 | registration | F-617 (done) | Thermal predictor for the attempts of nonlinear paths with a temperature field | done (opt-in) | — |
| [ADR 0549](../../docs/adr/0549-f580-registration-creep-on-the-b-bar-hex8-against-a-finite-strain-reference.md) | 2026-11-15 | registration | F-580 (partial) | Creep on the B-bar Hex8 against a finite-strain reference | partial: F3 `1.51e-3` vs `2e-3` met; MU3, MU4 not caught | ADR 0563, ADR 0568 |
| [ADR 0558](../../docs/adr/0558-f581-registration-arrhenius-temperature-factor-for-creep-on-the-time-stepping-driver.md) | 2026-11-15 | registration | F-581 (partial) | Arrhenius temperature factor for power-law creep | partial: MU5 caught only by NA2b added after the run | ADR 0578 |
| [ADR 0563](../../docs/adr/0563-f636-registration-bore-zone-witness-for-the-finite-strain-growth-of-the-b-bar-creep-cylinder.md) | 2026-11-15 | registration | F-636 (done) | Bore-zone witness for finite-strain growth of the B-bar creep cylinder | done: growth `4.34e-3` vs `6e-3` | — |
| [ADR 0568](../../docs/adr/0568-f637-registration-pressure-drift-of-the-f-bar-hex8-under-creep-at-finite-strain.md) | 2026-11-15 | registration | F-637 (partial) | Pressure drift of the F-bar Hex8 under creep at finite strain | partial: J4 `1.30e-4` vs `1e-5` | — |
| [ADR 0578](../../docs/adr/0578-f651-registration-arrhenius-creep-factor-non-positive-temperature-control-and-the-nonlinear-transient-integrator.md) | 2026-11-15 | registration | F-651 (done) | Arrhenius factor: control of every non-positive temperature; nonlinear transient | done | — |
| [ADR 0612](../../docs/adr/0612-f527-registration-through-thickness-temperature-gradient-on-shell4.md) | 2026-11-15 | registration | F-527 (partial) | Through-thickness temperature gradient on Shell4 | partial: rounding $\sim (h/t)^2$ misses | ADR 0626 |
| [ADR 0626](../../docs/adr/0626-f687-registration-through-thickness-temperature-gradient-on-shell3-and-layup-shells.md) | 2026-11-15 | registration | F-687 (partial) | Through-thickness temperature gradient on Shell3 and Layup shells | partial: MU12, amendment | — |

## Key numbers and limits

- B-bar cylinder at `p/10`: `C1' 5.4e-3`, `C2' 9.5e-3`, `C3' 9.6e-3` (limits `1e-2`, `2e-2`, `2e-2`), growth `C4' 1.06e-4` against `2e-3` (ADR 0528).
- At the registered $p$ ($p/E = 10^{-4}$, largest creep strain `9.7e-3`): point-wise against the stationary field `9.5e-3` at `5 t*`, `1.6e-2` at `20 t*` (limit `2e-2`); `F3 = 1.51e-3` within `2e-3` (ADR 0549).
- Bore zone (2304 points with $r < 1.1 a$): `5.96e-3` at `5 t*`, `1.03e-2` at `20 t*` (limit `2e-2`); growth `4.34e-3` (bound `6e-3`, proxy estimate `4.33e-3`); mutations MU3 `0.190`, MU4 `0.730` against `5e-2` (ADR 0563).
- F-bar drift: J4 `1.30e-4` against `1e-5` (the control's estimate was second order, the agreement is first order); finite-strain part scales by `0.0914` against the linear `0.0909` (limit 5 percent); worst point $r = 1.0545$ (ADR 0568).
- Shell4 gradient: free-plate accuracy `~3e-12` at `h/t ~ 5`, `~2.6e-10` at `h/t ~ 50`; the exact discrete field reproduced to `4e-16` (ADR 0612).

## Open questions

- Re-register the bore-zone growth on $\sigma_{\theta} - \sigma_r$ once the drift is diagnosed (ADR 0563 Q1, recommendation (b) within F-637).
- The strain energy with a temperature field (ADR 0534 Q3) and shells, F-bar Hex8 and the energy-momentum step with a field (ADR 0534 Q4): follow-ups when a case needs them.
- A non-positive nodal temperature reaches the caller as a step error (ADR 0578 Q1): recommendation leave now.
- Case-file keys for temperature histories, creep, and shell gradients (ADR 0526 Q1, ADR 0558 Q2, ADR 0578 Q3, ADR 0612 Q2): the maintainer's call.

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-materials-nonlinear]], [[adr-thread-structural-beams-shells]], [[adr-thread-structural-incompatible-hex8-rule-l]] (the warm start with creep, ADR 0668)
- [[known-miss]], [[registered-limit]]
