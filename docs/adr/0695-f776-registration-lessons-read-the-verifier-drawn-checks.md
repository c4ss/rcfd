# ADR 0695: F-776 registration: lessons read the verifier-drawn checks

Status: **REGISTERED 2026-11-17, written before any code of F-776 and before any run of its checks.** Written by the
implementing agent on branch `f776-work` (from acd83b83, which holds F-701 / ADR 0624, F-727 / ADR 0662 and F-745 / ADR 0693),
worktree `w49i-work/f776`, scratch `w49i-work/f776-scratch`. Not yet accepted by the maintainer. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

Roadmap item F-776 (deps F-745), the follow-up proposed by ADR 0693 (limits: "`lessons` does not read drawn records"). The
binding acceptance:

> Every limit not stated here is registered in the ADR before the run. lessons drops a hypothesis whose latest drawn record, redone consistently by drawn_record_problems with the caller's model, did not reproduce, and reports the number of drawn checks per hypothesis; on synthetic ledgers an impostor caught by a drawn check gives 0 rows and an honest one keeps its row; ledgers without drawn records give the lessons of ADR 0624 bit for bit; every ADR 0624, ADR 0662 and ADR 0693 check keeps its result.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run, each registered run made once,
existing tests not edited, defaults bit-identical, no golden / hash table / `baseline_hashes.rs` edit, no new dependency, no
unsafe code, clean-room.

## What was read

`AGENT_RULES.md`; the brief; ADR 0693 (D2, D3, the record, `drawn_record_problems`, its limits and addendum 1); ADR 0624 (lessons,
`verify`, the row); ADR 0662 (record-format rule); `design/hypothesis.rs` (`lessons`, `Lesson`, `Lessons`, `Flag`, `LessonQuery`,
`check_drawn`, `drawn_entry`, `drawn_record_problems`, `KIND_DRAWN`); `tests/f745_drawn_sample.rs` (W5, the file-aware impostor, `build`).

No arithmetic beforehand except the miss probabilities of ADR 0693 (disclosed there): for the D1 impostor (n = 64, honest on the m = 13
pairs of the F-727 sample) the miss probability of a drawn check is `C(13, k) / C(64, k)`: k = 4 gives 0.001125. Over 40 worlds
`P(missed >= 3) = 1.4e-5` (binomial, estimate `C(40,3) 0.001125^3`).

## Design (fixed now; names are the implementation's)

### D1. A new function; the old one is not touched

`lessons(entries, q, rerun)` already takes the caller's models (`rerun: &[&dyn Model]`), so no new input is needed. Its output type
`Lessons` / `Lesson` is `PartialEq` and built by struct literal in tests; adding a field would change both. Therefore (standing
instruction, "new function rather than changing the old one"): `lessons` keeps its code and bytes, and

* `lessons_drawn(entries, q, rerun) -> DrawnLessons`: calls `lessons` unchanged, then reads the drawn records and returns the rows
  wrapped, with the counts added.
* `DrawnRow { lesson: Lesson, drawn_checks, drawn_failed, drawn_ignored, drawn_unchecked, drawn_latest: Option<usize> }`.
* `DrawnLessons { rows: Vec<DrawnRow>, flagged: Vec<Flag> (exactly those of lessons), drawn_flags: Vec<Flag>, dropped: Vec<DrawnDrop> }`
  with `DrawnDrop { id, registration, record, mismatched }`, and `plain() -> Lessons` (the surviving rows' `lesson`s and
  `flagged`).

### D2. Which drawn records belong to a row, what "consistent" and "latest" mean

For a row of registration index `r` (record hash `h`, evaluator id and version `(i, v)`), the candidates are the entries of kind
`hypothesis_drawn_check` whose variant is the row's id and whose first parent index is `r` ("of the row").

* **Model.** The caller's model with id `i` and version `v` among `rerun`. If there is none, no candidate can be redone: each is
  counted in `drawn_unchecked`, none is used, the row stays (a record that cannot be redone proves nothing either way).
* **Consistent** (registered, standing instruction): `drawn_record_problems(entries, k, model)` is empty **and** the record's
  `settings.record_hash` equals `h`. Anything else is **inconsistent**: ignored (never used to drop or to keep), counted in
  `drawn_ignored`, and named in `drawn_flags` with the problems. So a forged "reproduced" on a check that does not redo as
  reproduced is not believed, and a forged "failed" is not believed either.
* **Latest** means ledger order: the consistent record with the largest entry index. (Drawn records carry no clock that is checked.)
* **Drop** (the acceptance): the row is removed when the latest consistent record has `reproduced = false`. Because the record is
  consistent, that stored value equals the redo's. The drop is listed in `dropped` and in `drawn_flags` with the mismatched entries.
  A latest consistent record with `reproduced = true` keeps the row, even if an earlier consistent one failed (the earlier failure
  stays visible in `drawn_failed`).
* **Counts.** `drawn_checks` = consistent records of the row (reproduced or not); `drawn_failed` = those not reproduced;
  `drawn_ignored`; `drawn_unchecked`; `drawn_latest` = index of the latest consistent record. Counted for the rows `lessons`
  returns (filters and staleness applied by `lessons` first). Dropped rows are not in `rows`; their record and mismatched entries
  are in `dropped`.
* Records are read only from the file; nothing is appended; `trust_stored` is not consulted for drawn records.

Ledgers without drawn records: every `drawn_*` count is 0, no row dropped, `plain()` equals `lessons(..)` (struct equality and Debug
text).

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f776_lessons_drawn.rs`, release profile, W5 and the impostor copied from `f745_drawn_sample.rs`
(that file is not edited). World w = 0 to 39, seed `776_000 + w`, verifier seed `fnv64("f776:verifier:<w>")`, 64 pairs, rtol = atol = 0.

| Id | Clause | What | Limit |
|---|---|---|---|
| A0 | (3) | worlds, D1 impostor, D2 impostor (16 dishonest pairs) and honest ledgers without drawn records: `lessons_drawn(..).plain() == lessons(..)` (struct equality and Debug text), all counts 0, `dropped` and `drawn_flags` empty; each under 5 queries: default, `include_stale`, an empty `rerun`, `min_tier` cfd, a non-matching `factor` | 40 x 3 x 5 = 600 of 600 equal |
| A1 | (2) | worlds, D1 ledger, `verify_drawn` k = 4 `Given(s_w)`: when the check found a problem, `lessons_drawn` has 0 rows and `dropped` = [that record]; when it did not, 1 row | the two cases hold in 40 of 40; **caught-and-dropped at least 38 of 40** |
| A2 | (2) | worlds, honest ledger, `verify_drawn` k = 4: the row is kept, equals the row of `lessons` bit for bit, `drawn_checks` 1, `drawn_failed` 0, `drawn_latest` = the record | 40 of 40 |
| A3 | (1) | world 776_900, D1: a failing record then a passing record (seeds: the first of `fnv64("f776:seed:<i>")`, i = 0.. whose check gives the wanted outcome): row kept, `drawn_checks` 2, `drawn_failed` 1; the order passing then failing: row dropped; each exactly | all |
| A4 | (1) | forgeries on world 776_901 (D1 with a failing record F): (a) F edited to `reproduced` true: ignored, `drawn_ignored` 1, `drawn_checks` 0, row kept, named in `drawn_flags`; (b) a genuine failing F followed by an appended forged passing record (a copy with `reproduced` true and `mismatched` empty): the forged one is ignored, the row is dropped; (c) the honest ledger with its record edited to `reproduced` false: ignored, row kept; (d) a record whose `record_hash` is edited: ignored; (e) a record of the honest ledger whose seed is edited by +1: ignored | each exactly |
| A5 | (2) | no model among `rerun` (empty slice; a model of version 2): ledger D1 with a failing record: row kept, `drawn_unchecked` 1, `drawn_checks` 0, `drawn_flags` empty | exactly |
| A6 | (3) | on the file of A1 (D1, world 776_902), `lessons(..)` on the entries after the record equals `lessons(..)` on the entries before it (ADR 0693's claim kept), and `lessons_drawn` before the record has no drop | equal |
| C1 | (3) | `--test f701_hypothesis` (19 fast), once `-- --ignored` | 19 pass; heavy numbers of ADR 0693 addendum 1 (S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118) |
| C2 | (3) | `--test f727_rerun_sample` (8 fast), once `-- --ignored` | 8 pass; A1 394, A2 0, NC6 0, NC7 116, A3 60 to 107 |
| C3 | (3) | `--test f745_drawn_sample` (6 fast), once `-- --ignored` | 6 pass; heavy D1 399, D2 394, NC1 0 of 800 |
| C4 | (3) | `--test f728_coupled_held`, `--test f749_floor_sample`, `--lib` | pass |
| C5 | — | `cargo test -p rcfd-cli --release --test docs_site` (the `docs/design-loop.md` edit) | pass |
| R | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

A1 runs the D1 worlds once. The 38 of 40 limit is from the 1.4e-5 miss probability above (a correct implementation fails it with
probability about 1e-5).

## Negative controls and mutations (registered now)

Negative controls: A0 (no records: nothing may change), A2 (the honest row is never dropped), A4 (a forged record is believed
neither way), A5 (a record that cannot be redone does not drop).

Mutations (scratch copy of the final code, one at a time; each must fail a registered test):

| Id | Mutation | Expected to fail |
|---|---|---|
| M33 | `lessons_drawn` never drops | A1 |
| M34 | the stored `reproduced` is used without `drawn_record_problems` | A4 (a), (b), (c), (e) |
| M35 | latest = first consistent record | A3 |
| M36 | inconsistent records counted in `drawn_checks` | A4 |
| M37 | an inconsistent record drops the row | A4 (c) |
| M38 | a row without a model is dropped when it has any drawn record | A5 |
| M39 | the `record_hash` comparison removed | A4 (d) |
| M40 | `lessons_drawn` reverses the row order when no drawn record exists | A0 |

## Decision rule

* **done:** every check met and M33 to M40 all caught.
* **partial:** A0, A1, A2, C1 to C3 met and another check missed (ignored `*_known_miss` record with the limit intact), or a
  mutation survives.
* **todo:** A0, A1, A2 or C1 to C3 missed.

## Limits (not covered)

* A drawn check is only as trustworthy as the ledger writer: an evaluator that also appends records can add genuine-looking
  passing records after a failure; the latest consistent record then wins and the row returns. `drawn_failed` and `drawn_checks`
  show this; they do not stop it. (Option: a row with any failed consistent record is dropped; not taken, because a later genuine
  pass of a repaired evaluator would then never be read. Listed as a question.)
* Only records whose redo is consistent are used, and the redo uses the entries the record saw (`entries_seen`): evidence entries
  added after the record are not re-checked by it.
* A row whose evaluator model the caller does not hold is never dropped by drawn records.
* A seed that the evaluator could have known when writing evidence is not distinguished from a verifier-drawn one: the guard of
  ADR 0693 (order) is the only one.
* Library only; no MCP tool; synthetic evaluators labelled cfd; no CFD run.

## References

ADR 0624, ADR 0662, ADR 0693 (`check_drawn`, `drawn_record_problems`); first principles.

## Amendment 1 (2026-11-17, written after the registered run of A0 to A6 and mutations M33 to M40, before the corrected run)

The registered run met A0 to A6 (see addendum 1) and M33 to M39 were caught, but **M40 survived** (all five tests passed): every A0
ledger holds one hypothesis, so reversing the row order is a no-op. This is a defect of the set-up, not of the code: the clause
"bit for bit" includes the row order, which a one-row ledger cannot test. The registered tests stay as they are (the old tests are
the record). Added before its run:

| Id | What | Limit |
|---|---|---|
| A0b | worlds w = 0 to 39, seed `776_000 + w`: one ledger with two hypotheses, H-1 (W5 claim, seed) and H-2 (W5 claim, seed + 1), both run by the real model, no drawn records; under the five queries of A0 (the third with an empty `rerun`): `lessons_drawn(..).plain() == lessons(..)` (struct equality and Debug text), and under the default query the rows are H-1 then H-2 | 200 of 200 equal; 40 of 40 in order |

M40 is re-run against the test file with A0b added (mutation made once, on a fresh archive of the amended code); it must fail
`a0b_two_hypotheses_order`. The decision rule is applied to the amended set: done if everything else is met and M40 is caught by A0b;
otherwise partial.

## Addendum 1: results (2026-11-17)

Code: `crates/rcfd-api/src/design/hypothesis.rs`: additions only (module doc bullet; `DrawnRow`, `DrawnDrop`, `DrawnLessons`,
`DrawnLessons::plain`, `lessons_drawn`, inserted before the explore section); `lessons`, `Lesson`, `Lessons` and every drawn
function of ADR 0693 are untouched. New `crates/rcfd-api/tests/f776_lessons_drawn.rs` (6 tests, all fast, about 13 s in release;
A0b added by amendment 1). `docs/design-loop.md`: one bullet and one changed sentence. No shared solver file, golden, hash table,
case key or MCP tool. Commits: registration 352fa459; code and tests; amendment 1 with the docs edit; the A0b test.

Runs (release, `CARGO_TARGET_DIR` in the scratch folder): registered run of A0 to A6 once (`run1.log`, 5 tests, 15.5 s), then the
corrected run with A0b (`run2.log`, 6 tests, 12.7 s); C1 to C3 fast and once `-- --ignored` (`c_*.log`, `h_*.log`); C4 `--lib`;
C5 docs_site; fmt and clippy. Mutations on a `git archive` copy with its own target directory (`mutate.py`, `mutate_run.log`,
`mut_M*.log`; control M0 passes). Machine shared with the other agents; no run was slow beyond the heavy records.

| Check | Limit | Measured | Met |
|---|---|---|---|
| A0 | 600 of 600 equal, counts 0 | 600 of 600 (struct and Debug equality, no drop, no drawn flag) | yes |
| A0b (amendment) | 200 of 200 equal; 40 of 40 in order | 200 of 200; 40 of 40 | yes |
| A1 | cases 40 of 40; caught and dropped at least 38 of 40 | 40 of 40; **40 of 40** (k = 4, D1 impostor) | yes |
| A2 | 40 of 40 honest rows kept, equal to the row of `lessons`, 1 check, 0 failed | 40 of 40 | yes |
| A3 | fail then pass: kept, 2 checks, 1 failed, latest = the pass; pass then fail: dropped | as stated (records 131; dropped record 131 with mismatched [72]) | yes |
| A4 | forged records ignored (a) to (e) | (a) rows 1, ignored 1, checks 0; (b) forged pass ignored (flag at 131), row dropped by the genuine record 130; (c), (d), (e) row kept, checks 0, ignored 1 | yes |
| A5 | no model / version 2: row kept, unchecked 1, no flag | as stated for both | yes |
| A6 | lessons equal before and after the record; no drop before it | equal | yes |
| C1 | 19 fast; S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118 | 19 passed (29.7 s); S1 1, S2 1592, NC1 400, NC2 61, S4 118 of 4000 (heavy 334.8 s) | yes |
| C2 | 8 fast; A1 394, A2 0, NC6 0, NC7 116, A3 60 to 107 | 8 passed; A1 394, A2 0, NC6 0, NC7 116, A3 60..107 (heavy 56.9 s) | yes |
| C3 | 6 fast; D1 399, D2 394, NC1 0 of 800 | 6 passed; D1 399, D2 394, NC1 0 of 800, H1 0 (heavy 41.3 s) | yes |
| C4 | pass | f728 10 passed, f749 8 passed, `--lib` 141 passed 4 ignored | yes |
| C5 | pass | docs_site 2 passed | yes |
| R | clean | fmt `--check` clean; clippy `-D warnings` clean | yes |

Mutations (caught = at least one test fails):

| Id | Failing tests | Caught |
|---|---|---|
| M33 never drops | a1_a2, a3, a4 | yes |
| M34 stored `reproduced`, no redo | a4 | yes |
| M35 latest = first | a3 | yes |
| M36 inconsistent counted | a4 | yes |
| M37 inconsistent drops | a4 | yes |
| M38 no model drops | a5_a6 | yes |
| M39 hash comparison removed | a4 | yes |
| M40 rows reversed without records | none in the registered run (survived); **a0b after amendment 1** | yes, by A0b only |

Deviations: M40 survived the registered A0 because every ledger there has one hypothesis; amendment 1 was written and committed
before A0b and the second M40 run, as the house rules allow for a defective set-up. No limit was changed. Not run: the whole
workspace and whole integration suites (daily gate), `baseline_hashes` (no shared solver file), rcfd-mcp tests (not touched).

Decision by the rule (amended): done (every check met; M40 caught by A0b).

Remaining questions:

1. A later genuine pass after a genuine failure restores the row (latest wins). Options: (a) as built; (b) any consistent failure
   drops the row for good. Recommendation (a), as registered, with `drawn_failed` visible so a laundering ledger writer is seen;
   (b) needs a way to retract a failure, which does not exist.
2. `lessons_drawn` is library only. Options: (a) leave; (b) an MCP/CLI switch (F-702 territory, golden lists change). Recommendation
   (a) until F-702.
3. Evidence entries appended after a drawn record are not re-checked by it. Recommendation: require a drawn record newer than the
   last evidence entry of the row (a later item; it changes which records count).

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-745 (ADR 0693 limit). ADR 0695, 2026-11-17. DONE. Library only, opt-in: design::hypothesis::lessons_drawn(entries, query, rerun) is lessons (unchanged, with its output type) plus the verifier-drawn checks: a drawn record counts for a row only when drawn_record_problems with the caller's model is empty and it names the row's registration hash; the latest such record in ledger order decides, a row whose latest consistent drawn record did not reproduce is dropped (listed in dropped), and every row reports drawn_checks, drawn_failed, drawn_ignored (inconsistent records, believed neither way) and drawn_unchecked (no model of the evaluator held). Checks on 40 synthetic worlds: the D1 impostor caught by a k = 4 drawn check gives 0 rows in 40 of 40 (limit 38), the honest evaluator keeps its row, equal to the lessons row, in 40 of 40; a genuine failure followed by a forged pass still drops the row, forged edits are ignored; ledgers without drawn records give the lessons of ADR 0624 bit for bit in 600 of 600 (one hypothesis) and 200 of 200 (two, row order); every ADR 0624, 0662 and 0693 check keeps its result; 8 of 8 mutations caught (M40, the row order, by the check added in amendment 1 after the one-row ledgers missed it).

> Note for F-745 (append): ADR 0695 (F-776, 2026-11-17): lessons_drawn reads the drawn records, so the verifier-drawn check now decides whether a hypothesis is reported.

> Follow-up proposed: Drawn checks must be newer than the evidence they check. Deps F-776. Acceptance: Every limit not stated here is registered in the ADR before the run. lessons_drawn counts a drawn record for a row only when no evidence entry of the row is newer than the record (entries_seen covers every evidence entry of the row), and reports the records it ignored for that reason separately; on synthetic ledgers an impostor that appends a second evidence set after a passing drawn record is dropped or not reported, and an honest ledger keeps its row; ledgers without drawn records give the lessons of ADR 0624 bit for bit; every ADR 0624, ADR 0662, ADR 0693 and ADR 0695 check keeps its result.
