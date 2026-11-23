---
type: entity
tags: [example-project, studio, lid-driven-cavity, heated-cavity]
sources: [doc-example-projects]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Example project: quick start

**What it is:** the small finished Studio example project "Quick start: lid-driven cavity and heated cavity" (ADR 0142): two closed-box 2D flows, each a case, a variant and completed runs of both. "Every run takes one or two seconds, so this is the project to try the Studio's own run controls on" ([[doc-example-projects]]).
**Where:** [example-projects/quick-start/](../../example-projects/quick-start/) (start with `notes/README.md`); a Linux-built twin in `example-projects-linux/quick-start/` ([[doc-example-projects]]).

## Details

- Layout: `rcfd-project.json` (created 2026-10-28T15:26:41Z); 4 cases; `meshes/` empty except a README (the cases use the generators `cavity_2d` and `box_2d`); 4 runs; `designs/variants.ledger.jsonl` (4 lines); one note. 4 250 540 bytes against a cap of 6 000 000 ([[doc-example-projects]]).
- **Lid-driven cavity** (`cavity-re100.toml`, variant `cavity-re400.toml`): lid at 1 m/s, 32 x 32; the tutorial `docs/tutorials/cavity.md` runs the same case. Results: lid_drag -0.004911 (121 steps, 0.6 s) and -0.001609 (158 steps, 0.7 s). At Re 400 "the vortex centre moves towards the middle and the lower corner vortices grow" ([[doc-example-projects]]). See [[lid-driven-cavity]].
- **Heated cavity** (`heated-cavity-ra1e3.toml`, variant `heated-cavity-ra1e4.toml`): hot left wall, cold right wall, Boussinesq, 40 x 40. Results: nu_hot 1.119, q_hot -0.02797 (Ra 1e3); nu_hot 2.257, q_hot -0.05643 (Ra 1e4). For orientation only, de Vahl Davis's 1.118 and 2.243 in `cases/heated_cavity/reference/devahldavis1983.csv`. `nu_hot` divides the heat flow by the cell depth 0.025 m of a 40 x 40 box of side 1 m; "a case with another cell count needs another divisor" ([[doc-example-projects]]). See [[differentially-heated-cavity]].
- Steady runs: the scrubber runs over iterations and stops at the stored snapshots (every 5 or 10); reports `lid_drag`, `q_hot`, `nu_hot` in Monitor series ([[doc-example-projects]]).
- "Coarse meshes ..., no mesh study: nothing here is a validation" ([[doc-example-projects]]).

## Related

- [[doc-example-projects]], [[example-project-wind-tunnel]], [[example-project-axial-fan]]
- [[studio]], [[rcfd-project]], [[run]]
- [[adr-thread-example-projects-and-linux-values]], [[design-ledger]]
- [[lid-driven-cavity]], [[differentially-heated-cavity]]

## Open questions

_None recorded._
