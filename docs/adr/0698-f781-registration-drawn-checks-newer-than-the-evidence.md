# ADR 0698: F-781 registration: drawn checks must be newer than the evidence they check

Status: **REGISTERED 2026-11-17, written before any code of F-781 and before any run of its checks.** Written by the
implementing agent on branch `f781-work` (from 5573d296, which holds F-745 / ADR 0693 and F-776 / ADR 0695), worktree
`w49i-work/f781`, scratch `w49i-work/f781-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer
would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is
marked "(standing instruction)".

Roadmap item F-781 (deps F-776), the follow-up proposed by ADR 0695 (remaining question 3). The binding acceptance:

> Every limit not stated here is registered in the ADR before the run. lessons_drawn counts a drawn record for a row only when no evidence entry of the row is newer than the record (entries_seen covers every evidence entry of the row), and reports the records it ignored for that reason separately; on synthetic ledgers an impostor that appends a second evidence set after a passing drawn record is dropped or not reported, and an honest ledger keeps its row; ledgers without drawn records give the lessons of ADR 0624 bit for bit; every ADR 0624, ADR 0662, ADR 0693 and ADR 0695 check keeps its result.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run, each registered run made once,
existing tests not edited, defaults bit-identical, no golden / hash table / `baseline_hashes.rs` edit, no new dependency, no
unsafe code, clean-room.

## What was read

`AGENT_RULES.md`; the brief; ADR 0695 (D1, D2, the rows, addendum 1, amendment 1 and its A0b lesson); ADR 0693 (the drawn record:
`entries_seen`, parents = registration plus every re-run entry); `design/hypothesis.rs` (`lessons`, `verify` / `check_member`,
`is_evidence_of`, `lessons_drawn`, `drawn_entry`, `drawn_record_problems`, `LessonQuery::trust_stored`);
`tests/f776_lessons_drawn.rs` (builders copied, file not edited).

No arithmetic beforehand. One reading, disclosed because it shapes the checks: in `verify`, every evidence entry of a hypothesis
beyond the first complete set gives a problem ("second evidence ..."), and `lessons` then flags the row and does not return it.
So in the default mode a second evidence set already removes the row from `lessons`; the new rule can change a result only
where the row is still reported although evidence is newer than the record: `LessonQuery::trust_stored` (ADR 0624 NC5, a check
switch that reads the last stored result without recomputation), and any caller that holds the rows of a longer ledger. The checks
therefore run the impostor in both modes.

## Design (fixed now; names are the implementation's)

### D1. Newer evidence

For a drawn record at index `k` with `entries_seen = s` (read from its settings; a record without it is already inconsistent in
the sense of ADR 0695 and never reaches this step), an **evidence entry of the row** is an entry for which `is_evidence_of(e, id)`
holds (kind `evaluation`, `provenance.driver` `hypothesis`, `provenance.hypothesis` = the row's id), at any index. It is **newer
than the record** when its index is `>= s` (it is not among the `s` entries the check saw; an entry written between the check and
its record has index `s` and counts as newer). The record is **stale** when the row has such an entry. Evidence of other
hypotheses never makes a record stale. Whether the evidence entry is valid (right pair, right hash) is not asked: any entry
that claims to be evidence of the row counts, because the point is that the check did not see it.

New public helper `drawn_newer_evidence(entries, index) -> Option<usize>`: the first evidence entry of the record's hypothesis
with index `>= entries_seen`, `None` when there is none or the record has no readable `entries_seen`.

### D2. What `lessons_drawn` does with a stale record

Order of the steps per candidate record of ADR 0695 D2 (unchanged up to step 3): (1) no model among `rerun`: `drawn_unchecked`,
as before; (2) inconsistent (problems or another hash): `drawn_ignored`, as before; (3) **new:** consistent but stale: ignored,
counted in the new field `drawn_stale`, never used to drop or to keep, named in `drawn_flags` ("drawn record ignored: stale ...",
with the first newer entry) and listed in the new `DrawnLessons::stale: Vec<DrawnStale { id, record, entries_seen, newer }>`;
(4) otherwise as before. So `drawn_checks`, `drawn_failed` and `drawn_latest` count and use only non-stale consistent records. A
stale record that did not reproduce does not drop the row either (standing instruction: a record is believed only for the
evidence it saw; the row then has no valid drawn check, which `drawn_checks` 0 and `drawn_stale` show; see limits).

The row stays in `rows` if `lessons` returned it: the impostor of the acceptance is "dropped or not reported" in the sense that
(a) in the default mode `lessons` itself does not return it (second evidence), and (b) with `trust_stored` its passing record is
not counted (`drawn_checks` 0, `drawn_latest` `None`, `drawn_stale` 1, listed). No new way to drop a row is added.

The count field is additive (`DrawnRow::drawn_stale`, `DrawnLessons::stale`); both are only built inside the library.
`lessons`, `Lesson`, `Lessons`, the drawn functions of ADR 0693 and the records are untouched, and a ledger without drawn
records goes through no new code (the loop of ADR 0695 finds no candidate).

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f781_drawn_newer.rs`, release profile; W5, the D1 evidence and the builders copied from
`f776_lessons_drawn.rs` (not edited). World w = 0 to 39, seed `781_000 + w`, verifier seed `fnv64("f781:verifier:<w>")`, 64 pairs,
rtol = atol = 0. The impostor's second set is built in memory on the entries of the ledger file: a copy of the 128 evidence
entries with the high-arm `y` raised by 0.3 (so it differs from the honest set), then a copy of the result entry whose parents
point at the new copies. Queries: "default" = the query of f776 (`current` = the model); "stored" = the same with `trust_stored`
true.

| Id | Clause | What | Limit |
|---|---|---|---|
| B0 | (3) | worlds, one-hypothesis and two-hypothesis honest ledgers without drawn records, queries default and stored: `lessons_drawn(..).plain() == lessons(..)` (struct and Debug text), all counts and `drawn_stale` 0, `stale`, `dropped`, `drawn_flags` empty | 40 x 2 x 2 = 160 of 160 |
| B1 | (3) | worlds, honest ledger, `verify_drawn` k = 4: the row is kept, equals the row of `lessons`, `drawn_checks` 1, `drawn_failed` 0, `drawn_stale` 0, `drawn_latest` = the record, `stale` empty | 40 of 40 |
| B2 | (2) | worlds, honest ledger with its passing record, then the second set (above) in memory: default query: 0 rows, `plain()` equals `lessons` (which flags the row); stored query: the row is returned, `drawn_checks` 0, `drawn_stale` 1, `drawn_latest` `None`, `stale` = [the record, `newer` = the first copy], a `drawn_flags` entry containing "stale", no drop | each of the two modes 40 of 40 |
| B3 | (2) | worlds, D1 evaluator, a failing k = 4 record (verifier seed: the first of `fnv64("f781:seed:<i>")` whose check fails), then the second set: stored query: the row is not dropped, `drawn_stale` 1, `drawn_checks` 0, `dropped` empty; control, the same ledger without the second set: dropped | 40 of 40 and 40 of 40 |
| B4 | (1)(3) | worlds, **two hypotheses** H-1 (seed) and H-2 (seed + 1), the real model, `verify_drawn` k = 4 each; (a) run H-1, check H-1, run H-2: H-1 `drawn_checks` 1 and `drawn_stale` 0 although H-2's evidence is newer; H-2 0 checks; (b) then check H-2: both 1 check, 0 stale; (c) then the second set of H-1 only, stored query: H-1 `drawn_checks` 0, `drawn_stale` 1, H-2 `drawn_checks` 1, `drawn_stale` 0; in (a), (b), (c) the rows are H-1 then H-2 | each of (a), (b), (c) 40 of 40, order 120 of 120 |
| B5 | (2) | boundary: an evidence copy inserted at index `entries_seen` exactly (between the last entry the check saw and its record), stored query: the record is stale | 40 of 40 |
| B6 | (2) | helper boundary: ledger of an honest run cut after the last evidence entry (no result entry), `verify_drawn` k = 4: `drawn_newer_evidence` is `None` (newest evidence is `entries_seen - 1`); with one evidence copy inserted at index `entries_seen`: `Some(entries_seen)` | 40 of 40 |
| C1 | (4) | `--test f776_lessons_drawn` unedited | 6 of 6 pass with the numbers of ADR 0695 addendum 1 (A0 600, A0b 200 and 40, A1 40 and 40, A2 40, A3 to A6) |
| C2 | (4) | `--test f745_drawn_sample`, `f701_hypothesis`, `f727_rerun_sample`, `f728_coupled_held`, `f749_floor_sample`, `--lib` (fast tests, no `--ignored`) | pass: 6, 19, 8, 10, 8 and 141 (4 ignored) |
| C3 | - | `cargo test -p rcfd-cli --release --test docs_site` (the `docs/design-loop.md` edit) | pass |
| R | - | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

If an ADR 0695 test would change its result, that is registered here as the reason for **partial**; none is expected: no
candidate record of those tests has evidence of its row at an index `>= entries_seen` (A3 and A4 add records only at the end of
the ledger). The heavy `--ignored` records of ADR 0693 / 0662 / 0624 are not re-run: the code of `lessons`, `verify`,
`check_drawn` is untouched and the heavy tests do not call `lessons_drawn` (disclosed).

## Negative controls and mutations (registered now)

Negative controls: B0 (nothing changes without records), B1 (the honest row is never marked stale), B4 (a multi-hypothesis
ledger from the start: evidence of another hypothesis, newer than the record, does not make it stale), B3 control (the same
failing record without a second set still drops).

Mutations (scratch copy of the final code, one at a time; each must fail a registered test; the unmodified control M0 must pass):

| Id | Mutation | Expected to fail |
|---|---|---|
| M41 | the stale step is skipped (as ADR 0695) | B2 (stored), B3, B4c, B5 |
| M42 | newer means index `> entries_seen` | B5, B6 |
| M43 | newer means index `>= entries_seen - 1` | B1, B6 |
| M44 | any entry of the ledger after `entries_seen` makes the record stale (hypothesis id not compared) | B4a, B4b |
| M45 | a stale record is counted in `drawn_checks` and also in `drawn_stale` | B2, B3, B4c |
| M46 | a stale failing record still drops the row | B3 |
| M47 | the `stale` list and the flag are not written | B2 |
| M48 | rows reversed when any record is stale | B4c (order) |

## Decision rule

* **done:** every check met and M41 to M48 all caught.
* **partial:** B0, B1, B2, C1, C2 met and another check missed (ignored `*_known_miss` record with the limit intact), or a
  mutation survives, or an ADR 0695 result changes (with the reason registered above).
* **todo:** B0, B1, B2, C1 or C2 missed.

## Limits (not covered)

* In the default mode a second evidence set is already stopped by `verify`; the new rule matters for `trust_stored` and for
  callers that read the rows of a ledger some other way. No MCP tool or CLI switch (library only, as ADR 0695).
* A stale failing record is not used to drop the row: an evaluator that appends evidence after a failed check launders the
  failure for `lessons_drawn` (the row reports `drawn_checks` 0, `drawn_stale` 1 and a flag; it is not shown as checked). Option
  not taken: a stale failure still drops (the check saw real evidence that failed). Listed as a question.
* Evidence that does not claim to be evidence of the row (wrong or missing `provenance.hypothesis`) is not seen by the rule,
  nor by `verify`.
* A new drawn check made after the second set (with the new `entries_seen`) is not stale by this rule; whether the second set is
  valid is decided by `verify`.
* Synthetic evaluators labelled cfd; no CFD run.

## References

ADR 0624, ADR 0662, ADR 0693, ADR 0695; first principles.
