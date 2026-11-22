---
type: source
tags: [provenance, clean-room, process]
sources: []
origin: docs/provenance.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Provenance record

**Origin:** [docs/provenance.md](../../docs/provenance.md) at origin/main 2e94e5ef
**Kind:** repo doc: record of where rcfd's knowledge comes from, and every recorded case in which vendor documentation was consulted.

## Summary

States the clean-room rule and lists all exceptions on record, so users and contributors can judge the clean-room claim themselves. The rule: "Algorithms, model constants, file formats and benchmark data are to come from published literature (papers, textbooks, public reports, public format specifications) and from first principles." No vendor source code, binaries or install contents were used, and no vendor documentation text was copied into the code. "No decompilation, disassembly or other inspection of vendor binaries was ever done; an audit of the full history on 2026-10-27 found no decompiler output and no reverse-engineering artefact in any commit."

## Key points

The six consulted cases:

1. Vendor product pages (public), 2026-10-17: to list capabilities for the parity matrix (S1, S2) and the product-suites backlog (P1 to P20); paraphrased, no text copied.
2. Vendor online help (normally behind a login), 2026-10-17: mostly contents pages (parity matrix S3 to S10, product suites P21), read through a web-fetch tool; used only to name and group capabilities. Whether any short vendor phrase survives in the paraphrases is UNVERIFIED.
3. Vendor documentation as a secondary check of model constants, 2026-10-13 to 2026-10-18; no longer counted as sources. Values marked UNVERIFIED until a published source confirms them:
   - Realizable k-epsilon `A0 = 4.04` ("the published report prints 4.0; 4.04 stays the default, ADR 0017");
   - log-law pair `kappa = 0.4187`, `E = 9.793` (rounded to 0.41 and 9.8 in the code);
   - two-layer constants `A_mu = 70`, `A_eps = 2 c_l`, `Re_y* = 200` and the tanh blend form;
   - the Zeldovich NO rate constants (none bundled; tests use invented values);
   - the Proudman constant `alpha = 0.1` with `M_t = sqrt(2k)/c0`. An earlier verification report quoted about 45 characters of a vendor guide, with attribution; it was removed.
4. One experiment, ADR 0063 (2026-10-18): standard wall-function forms taken from an unofficial web copy of an older vendor theory guide, implemented opt-in; it missed its registered limits and its code was removed in the same wave.
5. Early roadmap wording, 2026-10-13 to 2026-10-22: gap notes for about 20 items named settings and modules of commercial products; per the project's audit of 2026-10-14 those names came from file listings of a commercial product installed on the author's machine. Only names were used; the notes were reworded with generic names.
6. Vendor material on the author's machine: the project rule forbids using it; apart from item 5's file names, the records state nothing from it was read.

File formats:

- Fluent `.msh` / `.cas` reader (2026-10-13): from memory of an unnamed public description, so its source is UNVERIFIED; checked only as a black box against the public `elbow.msh` sample (GPL, fetched, never committed).
- EnSight Gold export: from the vendor's public format description (ADR 0048).
- Nastran-style bulk data export: from memory of the public Nastran manuals (ADR 0051).
- OBJ and glTF 2.0 surface readers (job J4, ADR 0184): from memory of the public specifications; selected glTF rules checked against the Khronos page on 2026-11-04, "the rest is unverified".

Third-party data licences: `NOTICE` and `validation-data/manifest.json`; Rust package licences: `THIRD_PARTY_LICENSES.md`.

## Contradictions with the wiki

None found.

## Pages this source informs

- [[provenance]], [[clean-room]], [[unverified]], [[doc-parity-matrix]], [[doc-reference-data]], [[doc-verification-report]], [[doc-designs-m-fan-b]]
