---
type: source
tags: [audit, roadmap, partials, history]
sources: []
origin: docs/audit/partials-group-*.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Partials audit, groups 1-10 (2026-10-21)

**Origin:** [docs/audit/](../../docs/audit/) at `origin/main` 2e94e5ef (`partials-group-1.md` to `partials-group-10.md`)
**Kind:** read-only audit records, one per group of ten partial roadmap items

## Summary

Ten read-only audits, each grading ten roadmap items that had status `partial` on 2026-10-21 (HEADs 272eb89, 20ad0d3, d4e2033, e20b7fc, 43c052d, 3558616), 100 items in all. Each item is graded "against its `acceptance` field in docs/roadmap.json only"; wider scope from the title or a "SCOPE (merged review)" addition is listed separately as "scope beyond acceptance" and does not lower the grade. Grades: **DONE** (acceptance as written met by a passing non-ignored test, or a reproducible script, with numeric limits); **PARTIAL** (a real tested slice exists, unmet clause named); **TODO** (nothing usable); **OVERSTATED** (the note claims more than the tests show). Tests were re-run in release mode with `--include-ignored` in a private target directory on Windows 11, on a machine shared with other agents' cargo jobs ("wall times and speed-ups are noisy"); no full workspace run, no Linux (except F-149 on WSL2), no hosted CI. The auditors were agents; the documents do not say a human reviewed them.

## Key points

| Group | DONE | PARTIAL | Notes |
|---|---|---|---|
| 1 | F-044, F-119, F-201 | F-001, F-063, F-102, F-131, F-211, F-233 | OVERSTATED F-223: "speed-up 118 for cell ISAT" is "a speed-up on a wrong answer" |
| 2 | F-072, F-121, F-202 | F-002, F-045, F-103, F-132, F-212, F-224, F-234 | F-132 and F-121 notes stale the other way (claim less than exists); F-002: no evidence of a green hosted CI run |
| 3 | F-203, F-213, F-235 | F-004, F-046, F-073, F-106, F-123, F-133, F-225 | F-133 note omits the FAN-01 C-grid waves 34A-37D; F-235's default still gives 3.33 % (s10 known miss) |
| 4 | F-050, F-204, F-214, F-236 | F-014, F-083, F-108, F-124, F-135, F-226 | OVERSTATED sub-claim F-108 (y+ "within 10 percent" has no assertion); F-204's 1 % pass from a run reporting `converged false`; F-236 held partial by user decision |
| 5 | F-051, F-136, F-215 | F-027, F-092, F-111, F-125, F-205, F-227, F-237 | OVERSTATED sub-claim F-027: stand-alone V-cycles at 1M take 24 to 1e-6 and 32 to 1e-8, not under 15 (from an auditor scratch crate) |
| 6 | F-031, F-228, F-238 | F-052, F-093, F-112, F-126, F-143, F-206, F-216 | F-126 note understated: ADR 0075 attempted the acceptance on FAN-01 and missed |
| 7 | F-032, F-094, F-113, F-229 | F-054, F-127, F-144, F-207, F-217, F-239 | F-207 meets 0 of 3 clauses; F-032 compared at unequal relaxation; F-229 passes without a fluid though ADR 0033 reads two clauses as flow-coupled |
| 8 | F-055, F-095, F-115, F-128, F-149, F-208, F-230 | F-041, F-218, F-240 | only F-149 can flip now; F-055 pending a Hysing reference check; F-115's residual check "vacuous" |
| 9 | F-220, F-231, F-241 | F-042, F-056, F-096, F-117, F-129, F-150, F-209 | OVERSTATED sub-claim F-209 (fixed-step error never measured); F-231's known miss now passes non-ignored |
| 10 | F-101, F-210 | F-043, F-062, F-118, F-130, F-200, F-222, F-232, F-243 | F-222 and F-243 meet 0 of 2 clauses; F-101 asserts only GPU < CPU (1x), not 3x, and skips without a device |

- Totals across the ten groups: 35 DONE, 64 PARTIAL, 1 OVERSTATED (F-223), 0 TODO; plus three OVERSTATED sub-claims inside PARTIAL items (F-027, F-108, F-209). Summed by the wiki from the per-group counts.
- Every `#[ignore]`d known-miss or superseded test run under `--include-ignored` failed as its note says; no default test failed (groups 8, 9 say so explicitly) ([[known-miss]]).
- Each summary row also gives residual gap, size (S/M/L), blockers, whether one wave suffices, and whether a user decision is needed.
- Group 8 lists the unverified references its items depend on ([[unverified]]).

## Contradictions with the wiki

_None found at ingest._ These grades are of 2026-10-21; later status lives on the [[roadmap]].

## Pages this source informs

- [[roadmap-status]]
- [[known-miss]]
- [[roadmap]]
- [[fan-01]]
