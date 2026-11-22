---
type: entity
tags: [validation, benchmark, incompressible, laminar]
sources: [doc-reference-data-lid-driven-cavity, doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31c-reference-reading, doc-validation-data, doc-design-loop]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Lid-driven cavity (Ghia, Ghia and Shin 1982)

**What it is:** the square lid-driven cavity benchmark, validated against the centreline velocities of Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411 ([[doc-reference-data-lid-driven-cavity]]).
**Where:** `cases/cavity/reference/*.csv`, `crates/rcfd-fv/tests/cavity.rs`, `crates/rcfd-api/tests/validation_cavity_ghia.rs`; dataset `cavity-ghia1982` of the pack ([[doc-validation-data]]).

## Details

- Reference values: u centreline at Re 100 / 400 / 1000, 16 rows from y = 0.0547 to 0.9688 (48 numbers), "verified, two independent secondary copies, exact match of every number"; u at y = 0.9766: 0.84123 / 0.75837 / 0.65928; v centreline at Re 100, 17 rows (-0.05906 ... 0.09233) ([[doc-reference-data-lid-driven-cavity]]).
- The original 1982 paper was never read; all values are from secondary copies (a student course repository, an Uppsala thesis, arXiv:1807.04896, and a third-party gist in wave 31C) ([[doc-reference-data-wave31c-reference-reading]]).
- Both secondary copies carry "the known printed misprint of the Re=400 v table at x = 0.9063 (-0.23827 ...)"; rcfd does not use Re 400 v ([[doc-reference-data-lid-driven-cavity]]).
- Wave 30B: Re 1000 u along x = 0.5, 15 rows, Ghia next to Botella and Peyret spectral values, "largest difference 0.0063" ([[doc-reference-data-wave30b-incompressible-validation]]).
- Wave 31C stored the Re 400 and 1000 v tables from three copies and listed two cell disagreements (Re 400 x = 0.5: 0.05186 vs 0.05188; Re 1000 x = 0.9063: -0.5155 vs -0.51500); no rcfd test uses them ([[doc-reference-data-wave31c-reference-reading]]).
- Also used as a cheap evaluation case: the design-loop runner check ran the 32 x 32 cavity at three viscosities and runner records equalled in-process ones bit for bit ([[doc-design-loop]]).

## Related

[[validation-data-pack]], [[verification-vs-validation]], [[doc-reference-data]], [[benchmark-descriptor]]

## Open questions

- The Ghia 1982 original and its printed tables remain unread (listed as unverified in [[doc-verification-report]]).
