---
type: concept
tags: [numerics, linear-solver, multigrid, preconditioner]
sources: [adr-thread-amg-coarse-lu, doc-case-schema-coupled, doc-case-schema-multiphase, doc-gpu, doc-reference-data-wave28a-vof-wiring, doc-solid-case-file]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# AMG (algebraic multigrid)

A multigrid method built from the matrix alone: coarse levels are made by grouping unknowns (aggregation), a smoothed prolongator moves corrections between levels, and the coarsest level is solved directly. Used as a preconditioner for CG on pressure-type systems. rcfd's scalar AMG is smoothed aggregation (Vanek, Mandel and Brezina 1996, "structure recalled, not read") in `rcfd-linalg` ([[doc-reference-data-wave28a-vof-wiring]], [[adr-thread-amg-coarse-lu]]).

## How rcfd uses it

- **Pressure solves:** the VOF and multiphase pressure solve is "CG with smoothed-aggregation AMG above 2000 cells" ([[doc-case-schema-multiphase]]); the block-coupled solver applies "one aggregation-AMG V-cycle on the approximate pressure Schur complement" ([[doc-case-schema-coupled]]). AMG is not on the GPU: "Jacobi or Chebyshev replace DIC/AMG" there ([[doc-gpu]]).
- **Structural solver:** `amg` (ADR 0305, aggregation with rigid-body modes), `amg_quadratic`, `amg_quadratic_smoothed` are CG preconditioners of the solid case file ([[doc-solid-case-file]]).
- **A defect found and fixed (wave 28A):** a zero filtered diagonal, "proven by a reproducer in `rcfd-linalg/tests/amg_contrast.rs`" ([[doc-reference-data-wave28a-vof-wiring]]).
- **The coarsest dense LU order** (F-474, F-471): the dense LU of the coarsest level applied its row interchanges in the wrong order whenever a pivot interchange happened at a step `k >= 1`; the coarse residual was `0.213` instead of `5.3e-15`, the V-cycle became indefinite and AMG-CG broke down on the immersed DFG 2D-2 run on T at step 2951. The corrected order (row interchanges applied before the forward substitution) is the default since ADR 0743, approved by the maintainer on 2026-11-18; it moved one immersed fingerprint, eleven overset values and the store hash of `axial_fan_cgrid`, each approved one by one ([[adr-thread-amg-coarse-lu]]). The same defect had been fixed earlier in `amg_elastic.rs` (ADR 0305 amendment 1).
- **Opt-in setting:** `AmgSettings::coarse_lu_interchanges_first` (ADR 0369) ([[adr-thread-amg-coarse-lu]]).

## Related

- [[rcfd-linalg]], [[simple]], [[piso]], [[adr-thread-amg-coarse-lu]], [[adr-thread-immersed-solid]], [[structural-solver]]

## Open questions

- An `rz <= 0` guard inside `cg` (ADR 0369 Q4, ADR 0363 Q6): it changes default solves and is the maintainer's ([[adr-thread-amg-coarse-lu]]).
