---
type: source
tags: [adr-thread, structural, fem, modal, buckling, cyclic-symmetry, mpc]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, modes, buckling, the transformation path and cyclic symmetry

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 12 ADRs, 2026-11-12 to 2026-11-14. Later buckling-eigensolver and shell/beam cyclic ADRs (F-628, F-642, F-650, F-672, F-680) are in the other half of the category.

Modal analysis and linear buckling of [[rcfd-fem]] beyond the basic tier: the convergence test of the subspace eigensolver, the geometric (initial-stress) stiffness, pre-stressed modes and buckling, everything on the [[mpc-transformation]] $x = T q + g$ (linear constraints, rigid sets, elastic supports, F-393), the refusal of mechanisms, and [[cyclic-symmetry]]: one sector of a rotor for statics and modes by harmonic index.

## Current state

- **Cyclic symmetry** works for statics and modes by harmonic index (F-394, ADR 0386, done: eight of eight mutations caught), with harmonic loads and rigid sets at a non-zero index (F-489, ADR 0395, done), cylindrical material axes (F-544, ADR 0462, done) and pre-stressed modes with a cyclic constraint and per-point materials (F-547, ADR 0465, done).
- **On the transformation:** modal analysis (F-486, ADR 0384, partial by the A1 zero-mode ratio 2.07e-6 against 1e-6, which F-488 re-measured at 2.1e-13 sigma), zero modes and the count of free rigid motions (F-488, ADR 0399, done), pre-stressed modes and buckling (F-493, ADR 0398, partial: a mechanism not refused), and the zero-shift pivot-ratio rule that refuses a mechanism in buckling on the transformation (F-495, ADR 0401, done) and on the prescribed path (F-496, ADR 0402, done).
- **Buckling:** F-395 (ADR 0390) is **partial**: the second factor misses by 2.0135e-2 against 2e-2 (the solid's K_sigma carries axial stress on rotating sections); the knife-edge column against the solid's closed form (F-492, ADR 0394) is done.
- **Eigensolver:** scale-aware convergence of the subspace iteration (F-457, ADR 0354) done, with a caveat on C2.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0354](../../docs/adr/0354-f457-registration-scale-aware-eigensolver-convergence.md) | 2026-11-12 | registration | F-457 | Scale-aware convergence test of the subspace eigensolver | done; MS1 to MS7 caught, caveat on C2 | |
| [0384](../../docs/adr/0384-f486-registration-modal-analysis-on-the-transformation.md) | 2026-11-13 | registration | F-486 | Modal analysis on the transformation (constraints, rigid sets, supports) | partial; A1 zero-mode ratio 2.07e-6 against 1e-6; `si_mpc::n4` needs the maintainer | 0399 |
| [0386](../../docs/adr/0386-f394-registration-cyclic-symmetry-sector-statics-and-harmonic-modes.md) | 2026-11-13 | registration (hub) | F-394 | One sector of a rotor for statics and for modes by harmonic index | done; frequencies within 3.96e-14 of the full model | |
| [0390](../../docs/adr/0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md) | 2026-11-13 | registration | F-395 | Geometric stiffness, pre-stressed modes, linear buckling | partial; A9 2.0135e-2 against 2e-2 (known miss) | 0394 |
| [0394](../../docs/adr/0394-f492-registration-knife-edge-buckling-closed-form.md) | 2026-11-13 | registration | F-492 | Knife-edge column factors against the solid's closed form | done | |
| [0395](../../docs/adr/0395-f489-registration-cyclic-harmonic-loads-and-rigid-sets.md) | 2026-11-13 | registration | F-489 | Harmonic loads and rigid sets at a non-zero harmonic index | done (H3 via corrected H3b, set-up defect) | |
| [0398](../../docs/adr/0398-f493-registration-prestressed-modes-and-buckling-on-the-transformation.md) | 2026-11-13 | registration | F-493 | Pre-stressed modes and buckling on the transformation | partial; C13 mechanism row returns a rounding factor instead of a refusal | 0401 |
| [0399](../../docs/adr/0399-f488-registration-zero-modes-on-the-transformation.md) | 2026-11-13 | registration | F-488 | Zero modes on the transformation (measure against the shift, free rigid motions count) | done; seven mutations caught | |
| [0401](../../docs/adr/0401-f495-registration-buckling-mechanism-refusal.md) | 2026-11-13 | registration | F-495 | Refuse a mechanism in buckling on the transformation (zero-shift pivot ratio) | done; rounding ratio 2.5e-11, 400 times below 1e-8 | |
| [0402](../../docs/adr/0402-f496-registration-buckling-prescribed-mechanism.md) | 2026-11-13 | registration | F-496 | Refuse a non-rigid mechanism in buckling on the prescribed path (pivot ratio on K_FF) | done | |
| [0462](../../docs/adr/0462-f544-registration-cyclic-symmetry-with-cylindrical-material-axes.md) | 2026-11-14 | registration | F-544 | Cyclic symmetry with cylindrical material axes | done; (a4) with a 45-fold margin | |
| [0465](../../docs/adr/0465-f547-registration-prestressed-modes-cyclic-per-point-materials.md) | 2026-11-14 | registration | F-547 | Pre-stressed modes with a cyclic constraint and per-point materials | done; a behaviour change flagged for the maintainer | |

## Key numbers and limits

- F-394 (ADR 0386): sector against full model $\max \lvert f_s/f_f - 1 \rvert$ 3.96e-14 (limit 1e-8); thin-ring n = 2 at k = 2 4.40e-4 and n = 3 at k = 3 1.81e-3 (limit 0.02); spinning-disc rim $u_r$ 8.89e-5 (limit 0.01); matching tolerance `MATCH_TOLERANCE 1e-8` of the largest radius.
- F-395 gap: "The second buckling factor is 2.0135e-2 from four times the first against the limit 2e-2 ... a follow-up settled this against a derived closed form (1.7e-4) and at L/h = 40 (0.52 percent), but not against the acceptance as written" ([[roadmap-structural-solver]]).
- F-486 gap: "the three zero modes of a bar held only by axial springs are at 2.07e-6 of the first flexible frequency against the registered 1e-6; F-488 measured them at 2.1e-13 sigma (limit 1e-9 sigma)".
- F-495: "BM's rounding ratio is 2.5e-11, 400 times below `1e-8`; the margin may shrink on larger" models (ADR 0401 Q1).

## Open questions

- ADR 0390 Q1: re-word clause a9 (F-395) to the derived form, "the second factor within 1 percent of the root of the" derived equation (maintainer).
- ADR 0386 Q1 was taken on the standing instruction of 2026-11-11 (option (a): `modal_analysis` of a cyclic model gives $k = 0$, documented); Q2 became F-489.
- F-486: "The maintainer decides done" (roadmap gap).

## Second half of the category (ADRs 0556 to 0744, job p3-structural-b)

Shell and beam whirl on a cyclic sector are added to [[adr-thread-structural-rotordynamics]]; hyperelastic structural elements on the cyclic path to [[adr-thread-structural-hyperelastic-and-follower]].

### Current state (second half)

- **Buckling eigensolver** (`rcfd-linalg` `lowest_buckling_factors`): a Sturm count just above the wanted set when the subspace holds no further positive Ritz value (F-642, ADR 0559, done) and one restart sized by Sturm counts when it holds too few (F-650, ADR 0564, done). Only inputs that were refused can change.
- **Shells in the cyclic constraint**, opt-in `CyclicOptions::shells`: the rotation unknowns of Shell4 and Shell3 nodes rotated by the same $R$ as the translations (F-628, ADR 0556, done); interpolated boundaries and axis nodes with `CyclicOptions::shell_extensions` (F-648, ADR 0575, partial: the one-sided 2:3 boundary I2b `4.17e-2` against `2e-2`); two-sided shell edge ties for non-matching boundaries remove that soft line (F-662, ADR 0583, done).
- **Beam2 in the cyclic constraint**, first opt-in `CyclicOptions::beams` (F-672, ADR 0595), `full_rotor` and expansion with the qualifier `full_rotor_beams` (F-674, ADR 0601, done). **Default changed:** the maintainer approved on 2026-11-18 "accepting Beam2 elements in the cyclic constraint by default"; ADR 0744 (2026-11-19, done) made it: a cyclic constraint with the shells option takes Beam2 without `beams`, the full ring within `8.3e-13` (limit `1e-10`), every model accepted before keeps its bits, NC3 of ADR 0556, A-B of ADR 0593 and NC3 of ADR 0595 retired as ignored records with restated copies. On 2026-11-21 the maintainer approved lifting the two remaining opt-ins (whirl paths and `full_rotor`) in one further registration (ADR 0744 answers); F-672 stays partial until that is made.
- **Free-sector centrifugal pre-stress.** The cyclic constraint leaves two rigid motions at index 0 (axial translation and rotation about the axis); two reaction-free gauge constraints at one interior node give the free stress (F-680, ADR 0606, partial: M9 not caught); on solid and shell sectors (F-683, ADR 0611, partial) and on Tet10, layered-shell and orthotropic sectors (F-686, ADR 0633, done). ADR 0633 found that the layup centrifugal load of ADR 0502 drops the force term $m_{1} \Omega^2 r_{\mathrm{perp}}(e_3)$: a spurious torque of relative size $\sim m_{1} / (m_{0} \rho)$ on a layup with a first mass moment; an opt-in `SolidModel::set_layup_first_moment_force` adds it (F-709, ADR 0635, done). A fan wheel is mounted, so the free-sector opt-in is "not urgent" (ADR 0602 Q1, ADR 0606 Q2).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0556](../../docs/adr/0556-f628-registration-shells-in-the-cyclic-constraint.md) | 2026-11-15 | registration | F-628 (done) | Shells in the cyclic constraint (rotation unknowns rotated) | done (`CyclicOptions::shells`) | ADR 0561, ADR 0575 |
| [ADR 0559](../../docs/adr/0559-f642-registration-buckling-eigensolver-on-sign-asymmetric-spectra.md) | 2026-11-15 | registration | F-642 (done) | Buckling eigensolver on sign-asymmetric spectra (Sturm count above the wanted set) | done: `1.2e-15` vs `1e-9` | ADR 0564 |
| [ADR 0564](../../docs/adr/0564-f650-registration-buckling-eigensolver-restart-when-the-subspace-holds-too-few-positive-ritz-values.md) | 2026-11-15 | registration | F-650 (done) | Buckling eigensolver restart when too few positive Ritz values | done | — |
| [ADR 0575](../../docs/adr/0575-f648-registration-shells-with-interpolated-cyclic-boundaries-and-axis-nodes.md) | 2026-11-15 | registration | F-648 (partial) | Shells with interpolated cyclic boundaries and axis nodes | partial: I2b `4.17e-2` vs `2e-2` | ADR 0583 |
| [ADR 0583](../../docs/adr/0583-f662-registration-two-sided-shell-edge-ties-for-non-matching-cyclic-boundaries.md) | 2026-11-15 | registration | F-662 (done) | Two-sided shell edge ties for non-matching cyclic boundaries | done | — |
| [ADR 0595](../../docs/adr/0595-f672-registration-beam2-elements-in-the-cyclic-constraint.md) | 2026-11-15 | registration | F-672 (partial) | Beam2 elements in the cyclic constraint (opt-in `beams`) | done; default lift approved 2026-11-18 | ADR 0744 |
| [ADR 0601](../../docs/adr/0601-f674-registration-full-rotor-and-expansion-of-a-cyclic-sector-with-beam2.md) | 2026-11-15 | registration | F-674 (done) | `full_rotor` and expansion of a cyclic sector with Beam2 | done (qualifier `full_rotor_beams`) | ADR 0744 |
| [ADR 0606](../../docs/adr/0606-f680-registration-the-centrifugal-pre-stress-of-a-free-cyclic-sector.md) | 2026-11-15 | registration | F-680 (partial) | Centrifugal pre-stress of a free cyclic sector (reaction-free gauge) | partial: M9 not caught | ADR 0611 |
| [ADR 0611](../../docs/adr/0611-f683-registration-the-free-sector-prestress-on-solid-and-shell-sectors.md) | 2026-11-15 | registration | F-683 (partial) | Free-sector pre-stress on solid and shell sectors | partial (amendment) | ADR 0633 |
| [ADR 0633](../../docs/adr/0633-f686-registration-the-free-sector-prestress-on-tetrahedral-layered-shell-and-orthotropic-sectors.md) | 2026-11-16 | registration | F-686 (done) | Free-sector pre-stress on Tet, layered-shell and orthotropic sectors | done; layup load finding | ADR 0635 |
| [ADR 0635](../../docs/adr/0635-f709-registration-the-torque-free-centrifugal-load-of-a-layup-with-a-first-mass-moment.md) | 2026-11-16 | registration | F-709 (done) | Torque-free centrifugal load of a layup with a first mass moment | done (opt-in) | — |
| [ADR 0744](../../docs/adr/0744-f672-registration-beam2-in-the-cyclic-constraint-by-default.md) | 2026-11-19 | amendment (default change) | F-672 (partial) | Beam2 in the cyclic constraint by default | done: full ring within `8.3e-13`; further lift approved 2026-11-21 | — |

### Key numbers and limits (second half)

- Monosymmetric I buckling under `-M_ref` with default options: `178.684212800573` at 16 elements, `1.2e-15` from the amended run of ADR 0553 (ADR 0559); at $L = 1.5$: `610.71097991747`, `2333.79172623103`, `5256.57773727524` (ADR 0564).
- Beam2 wheel sector with shells alone: `6.6e-13` (static $k = 0$), `6.1e-13` (harmonic static), `8.3e-13` (eigenvalues of $k = 0, \dots, 3$), limit `1e-10` (ADR 0744).
- Layup spurious torque: about `7e-3` of the force times the radius for `m1 = 4e-3 m`, `m0 rho ~ 0.6 m` (ADR 0633).

### Open questions (second half)

- The default for the torque-free layup load (ADR 0635 Q1): recommendation flip after the maintainer's approval, since it changes centrifugal results of every layup with $m_{1} \ne 0$; not taken.
- Supported or constrained sectors with the free-sector opt-in (ADR 0606 Q2): keep the refusal.
- Non-nested non-matching boundaries (ADR 0583 Q3): follow-up only when one appears; docs keep recommending the finer side as the high side.
- `mp = 0` in the buckling restart (ADR 0564 Q1): leave refused.

## Related

- [[cyclic-symmetry]], [[mpc-transformation]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-rotordynamics]] (rotating modes of a cyclic sector, Campbell tables)
- [[known-miss]]
