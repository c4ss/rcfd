---
type: entity
tags: [validation, data, provenance]
sources: [doc-validation-data, doc-reference-data-validation-pack, doc-test-process, doc-changelog, doc-fan-measurement-protocol, doc-reference-data-f878-fan01-lda-profiles, adr-thread-fan01-design-point, doc-reference-data-f883-fan01-wall-pressure]
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Validation data pack (F-125)

**What it is:** rcfd's pack of open benchmark data with fetch scripts, SHA-256 checksums and licence records ([[doc-validation-data]]).
**Where:** `validation-data/` (`manifest.json`, `fetch.py`, `reference/`, git-ignored `data/`); CLI `rcfd data list|verify|provenance`.

## Details

- `manifest.json` holds datasets, pinned upstream URLs, SHA-256 and size of every file, licence record, citation and the roadmap case each validates; `fetch.py` downloads into `validation-data/data/<id>/` and verifies every checksum ([[doc-validation-data]]).
- "CI and `cargo test` never use the network and never need the downloaded files"; GPL files are fetched on the user's machine and never vendored; "copies of datasets distributed with commercial software are never used" ([[doc-validation-data]]).
- Datasets include `cavity-ghia1982`, `heated-cavity-devahldavis1983`, `flat-plate-tmr`, `backward-step-tmr`, `naca0012-tmr`, `dambreak-openfoam`, `fluent-elbow`, `gri-mech-30`, `nasa-rotor37` and the unresolved `hannover-compressor` ([[doc-validation-data]]); the as-fetched findings of 2026-10-13 are in [[doc-reference-data-validation-pack]].
- Because the gate runs on a `git archive` copy without `validation-data/data`, 12 non-ignored data-gated tests always skip and count as passed ([[doc-test-process]]).
- The 0.1.0 changelog lists "a validation data pack with fetch scripts and checksums" ([[doc-changelog]]).
- Measured computer-fan data, under the protocol, "stay outside the repository" unless a licence is stated ([[doc-fan-measurement-protocol]]).
- The FAN-01 record's wall-pressure data (`mic_kulite.tar`, 460 728 320 bytes, checksummed) were read on 2026-11-22 for ADR 0769; the archive is not committed, only the transcription of the time means `docs/investigations/f883-fan01-wall-pressure/kulite_means.csv`. No uncertainty of the wall pressure is stated in the record ([[doc-reference-data-f883-fan01-wall-pressure]], [[doc-validation-data]]).

## Related

[[lid-driven-cavity]], [[differentially-heated-cavity]], [[flat-plate]], [[backward-facing-step]], [[naca0012]], [[fan-01]], [[provenance]], [[gate]]

## Open questions

- No open-licence upstream for the Hannover compressor cascade; GRI-Mech 3.0 legal status unverified ([[doc-validation-data]]).
- Whether data-gated tests get their data and a fail-on-skip switch in the gate remains open ([[doc-test-process]]).
- `validation-data/manifest.json` does not list the FAN-01 LDA transcription `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv` of ADR 0765; whether it becomes a file of the data pack "is the lead's" ([[adr-thread-fan01-design-point]], [[doc-reference-data-f878-fan01-lda-profiles]]).
