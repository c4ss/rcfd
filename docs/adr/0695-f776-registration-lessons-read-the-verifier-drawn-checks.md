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
