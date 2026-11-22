---
type: source
tags: [adr-thread, fan, computer-fan, reduced-order, mesh, c-grid, design-loop]
sources: [doc-meshing, doc-fan-project-breakdown]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: a reduced-order reference fan in the schema and C-grid constructions for strongly staggered blades (F-713, F-722, F-726, F-777, F-782, F-784, F-791)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (diagnosis, registrations with result addenda), 7 ADRs, 2026-11-16 to 2026-11-17.

ADR 0620 found that `ComputerFan::default()` stalls in the reduced-order model. ADR 0639 diagnosed why (F-713) and ADR 0643 added a named reference fan `ComputerFan::rom_reference()` beside the default (F-722). Its strongly staggered outer sections (75 to 78 deg) fold the `camber_seam_h` C-grid, so five ADRs build successive opt-in constructions S, S', S'', S''' and S'''' until the reference and a second strongly staggered blade mesh within 84.5 deg / 4.0 / 1000 (F-791, ADR 0713). Every construction is library only, with no case value; `camber_seam_h` and every earlier construction stay unchanged in bits. See [[fan-passage-mesh]].

## Current state

- **The schema default is not changed** (F-713 partial, ADR 0639): "A ROM-usable default needs a hub zero-lift angle of about 58 deg or more, which the ADR 0118 axial fit and root off hub variants do not allow together." The maintainer's choice "replace the default or put a ROM fan beside it" was recommended as "beside it" and became F-722.
- **Reference fan** (F-722 partial, ADR 0643): `ComputerFan::rom_reference()`, a 120 mm 7-blade fan, camber 0 at every station, stagger 69.8 / 75.4 / 77.6 deg, chord 0.016 / 0.029 / 0.036 m; in `RomModel::default()` at 3000 rpm "no stalled strip at any of 601 flows on [0.4, 1] q_f (largest incidence 8.21 deg against 10.03), stall from 0.31 q_f". The C-grid clause is missed: "camber_seam_h folds its leading-edge fan at every in-range front_gap and n_nose".
- **C-grid chain:** S (F-726 partial, ADR 0692; 86.97 deg and skewness 7.43 missed) -> S' (F-777 partial, ADR 0696; second blade skewness 4.0112 against 4.0) -> S'' (F-782 done, ADR 0699) -> S''' (F-784 partial, ADR 0702; one blade-line face 84.688 deg against 84.5) -> S'''' (F-791 done, ADR 0713; both blades within 84.5 deg / 4.0 / 1000, scan 12 of 12).
- **Not yet reachable from a case:** F-778 ("case value camber_seam_s for mesh.fan.c_grid_construction") is todo. On FAN-01, construction S "gives 88.0 deg and 4.30 ... worse" (ADR 0762, ranked gap rank 3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0639](../../docs/adr/0639-f713-registration-schema-default-fan-in-the-reduced-order-model.md) | 2026-11-16 | diagnosis | F-713 | Why the schema default fan stalls in the ROM, and what a usable default costs | partial; default unchanged; two known-miss records | 0643 |
| [0643](../../docs/adr/0643-f722-registration-reduced-order-reference-fan-in-the-schema.md) | 2026-11-16 | registration | F-722 | A named reduced-order reference fan beside the default | partial; C-grid clause missed (fold at outer staggers 75 to 78 deg) | 0692 |
| [0692](../../docs/adr/0692-f726-registration-camber-seam-c-grid-on-strongly-staggered-blades.md) | 2026-11-17 | registration | F-726 | Construction S, opt-in beside `camber_seam_h` | partial; 86.97 deg against 85, skewness 7.43 against 4.0 | 0696 |
| [0696](../../docs/adr/0696-f777-registration-construction-s-without-the-first-cell-stretch-on-strongly-staggered-blades.md) | 2026-11-17 | registration | F-777 | S' without the first-cell stretch | partial; second blade skewness 4.0112 against 4.0 | 0699 |
| [0699](../../docs/adr/0699-f782-registration-wake-stacks-of-construction-s-prime-at-the-thin-end-wall-layers.md) | 2026-11-17 | registration | F-782 | S'' wake-stack remap at the thin end-wall layers | done; 84.95 / 84.47 deg, skewness 2.76 / 2.83 | 0702 |
| [0702](../../docs/adr/0702-f784-registration-ramp-length-of-the-wake-stack-remap-from-the-wake-line-spacing.md) | 2026-11-17 | registration | F-784 | S''' ramp length from the wake line spacing | partial; 84.688 deg against 84.5 (predicted in the registration) | 0713 |
| [0713](../../docs/adr/0713-f791-registration-one-growth-ratio-of-the-free-rows-across-the-span-layers.md) | 2026-11-17 | registration | F-791 | S'''' one growth ratio of the free rows across span layers | done; 84.122 / 2.538 / 525.6 and 83.887 / 2.720 / 551.7 | |

## Key numbers and limits

- **ADR 0639 diagnosis:** at 3000 rpm the default's free delivery is 0.0765 m^3/s and "83 percent of dp_t is the exit dynamic pressure"; "every strip sits at 2.1 to 6.0 deg of the 10.03 deg attached window (cl_max 1.1 / 2 pi)"; "the outer strips stall from 0.79 q_f". Negative controls: "stagger -8 deg stalls from 0.555 q_f, cl_max 0.8 from 0.451".
- **ADR 0643:** free delivery 0.02826 m^3/s; static characteristic peak 26.83 Pa; "q / q_f 0.915 / 0.847 / 0.705 / 0.563 at k 1e4 / 2e4 / 5e4 / 1e5".
- **ADR 0692 (S):** 12841 cells, "no folded cell (all 100096 corner Jacobians ... of one sign, min volume 2.7e-13 m^3)", aspect 525.6 (limit 1000); h_slit on the reference "85.37 deg and 16.7 missed, 827 met". "Four of six mutations caught (the nose-region straight extension and the first-cell stretch are not seen by a check)."
- **ADR 0696 (S'):** reference 84.69 deg / 3.24 / 525.6 within 85 / 4.0 / 1000 ("numbers seen before as ADR 0692 mutation MU4, disclosed"); second blade outer stagger 76.6 deg, 84.22 deg, aspect 551.7, skewness 4.0112.
- **ADR 0699 (S''):** "the drift per thin layer falls from 3.3e-5 m to 7e-7 m".
- **ADR 0702 (S'''):** `L = 1.5 D / tan(1 deg)`; L = 7.0e-3 / 7.7e-3 m; first ramp face 84.397 / 83.564 deg against S'' 84.953 / 84.472.
- **ADR 0713 (S''''):** the 84.688 deg face was "a radial offset of the cell centroids (unit centroid vector radial 0.71)"; after the change the former worst face is 82.803 deg; "a front_gap 0.15 to 1.0 x n_nose 4 / 6 / 9 scan builds 12 of 12 on each blade (limit 9 of 12)"; six of six mutations caught.

## Open questions

- F-713: whether a ROM-usable schema fan "replaces the default ... or sits beside it (recommended)" (ADR 0639); F-722 put it beside it and the default is unchanged.
- F-778 (case value for the new construction) is todo; whether S'''' helps FAN-01's mesh gates is not shown in these ADRs.

## Related

- [[fan-passage-mesh]], [[computer-fan-target]], [[mesh-quality]], [[design-loop]], [[fan-01]]
- [[adr-thread-design-surrogate-and-fidelity]], [[adr-thread-fan01-design-point]]
- [[roadmap-design-studies-and-optimisation]], [[roadmap-meshing-and-mesh-formats]]
