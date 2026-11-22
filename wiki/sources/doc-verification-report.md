---
type: source
tags: [validation, verification, reference-data]
sources: []
origin: docs/verification-report.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference-data verification report (2026-10-15)

**Origin:** [docs/verification-report.md](../../docs/verification-report.md) at origin/main 2e94e5ef
**Kind:** repo doc: per-item re-check (wave 13) of the constants and benchmark values marked unverified or from memory in [[doc-reference-data]].

## Summary

A table-by-table re-check against web sources of turbulence and wall constants, benchmarks, thermodynamics/EOS, radiation, chemistry and acoustics values, plus bibliographic details checked on Crossref. Raw sources (NASA NTRS PDFs, arXiv PDFs, NIST WebBook tables) were pulled as raw files or HTML; others were read through a page summariser. Discrepancies are ranked by their impact on registered test outcomes.

## Key points

- Counts: 57 rows, 44 confirmed, 4 differ (Realizable A0; Ar acentric factor; the N2 test constants; dry-air composition), 7 unverified, 2 not re-checked. After the 2026-10-27 relabel: 40 confirmed, 11 unverified.
- §1.5 lists bibliographic details checked on Crossref (2026-10-15).
- §1.6 licence observation: NIST WebBook is Standard Reference Data (SRD 69), copyrighted by the U.S. Secretary of Commerce; `realgas_nist.rs` and ADR 0007 copy about 60 numbers from the WebBook, while `reference-data.md` says the WebBook was deliberately not used.
- §2.1 Realizable k-epsilon A0 = 4.04 versus 4.0 ("differs; medium impact"): the known-miss F-042 Realizable result (6.402) could change; the re-run was NOT done.
- §2.2 scalable wall function y*_lam = 11.53 versus 11.06 / 11.225 under other log-law constants: "not an error; low" impact.
- §2.4 source conflicts: the adiabatic flame temperature 2224.25 K (CEA) vs 2156.25 K (GRI-Mech 3.0 / Cantera) in the same secondary source.
- §3 lists the items still unverified.

## Contradictions with the wiki

> [!warning] Contradiction
> §1.6 points out that [[doc-reference-data-material-library]] says "NIST WebBook data was deliberately not used" while [[doc-reference-data-real-gas-eos]] embeds WebBook values. Within the docs.

## Pages this source informs

- [[doc-reference-data]], [[unverified]], [[verification-vs-validation]], [[provenance]], [[known-miss]]
