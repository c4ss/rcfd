---
type: concept
tags: [provenance]
sources: [doc-glossary, doc-readme, doc-provenance, doc-validation-data, doc-reference-data, doc-verification-report, doc-design-loop]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Provenance

The record of where a piece of knowledge came from, including every case in which vendor documentation was consulted (`docs/provenance.md`) ([[doc-glossary]]).

## How rcfd uses it

- Vendor docs were consulted in a few recorded cases: mostly contents pages to list capabilities, and some theory pages ([[doc-readme]]).
- In the Studio, every number shows its provenance and unit (guardrail G17) ([[doc-product]]).
- `docs/provenance.md` lists six recorded cases of vendor material consulted ([[doc-provenance]]):
  1. public product pages
  2. online-help contents pages
  3. constants found only in vendor docs, now marked UNVERIFIED
  4. the ADR 0063 experiment, whose code was removed
  5. early roadmap names taken from file listings of a commercial install
  6. vendor material on the author's machine, which the rule forbids using

  "an audit of the full history on 2026-10-27 found no decompiler output and no reverse-engineering artefact in any commit" ([[doc-provenance]]).
- File formats have their own provenance lines ([[doc-provenance]]):
  - Fluent reader: from memory, source UNVERIFIED.
  - EnSight: from the vendor's public description.
  - Nastran: from memory.
  - OBJ/glTF: partly checked against the Khronos page on 2026-11-04.
- Data provenance: `validation-data/manifest.json` records upstream URL, SHA-256, licence and citation per file. `rcfd data provenance` prints the table ([[doc-validation-data]]).
- Value provenance: [[doc-reference-data]] records, per benchmark value or constant, where it comes from and how well it was checked, with labels verified / secondary / UNVERIFIED / NOT read / data not obtained ([[doc-reference-data]], [[doc-verification-report]]).
- Design provenance: each [[design-ledger]] entry carries evaluator and solver versions, a geometry hash, settings, seed and free provenance, and optionally a `source` with citation and file hash (ADR 0632) ([[doc-design-loop]]).

## Related

- [[clean-room]]
- [[unverified]]
- [[doc-provenance]], [[validation-data-pack]], [[design-ledger]], [[verification-vs-validation]]
