---
type: source
tags: [adr-thread, process, push-main, roadmap, lead, stop-conditions]
sources: [doc-process-lead-process, doc-process-agent-rules, doc-test-process]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the lead's tooling and measures (push script, process measures and stop conditions, roadmap proposal files; F-851, F-863, F-864)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda, the lead's answers, the maintainer's answers), 3 ADRs, all 2026-11-21.

Three items that changed how the [[lead]] merges and records: the push script after its first real runs (F-851, ADR 0752), measured process quantities and the stop conditions that limit how many agents run (F-863, ADR 0754), and the roadmap proposal file an agent writes beside its addendum (F-864, ADR 0755). The maintainer answered ADRs 0754, 0755 and 0756 together on 2026-11-21: "Yes, all four".

## Current state

- **Push script** (F-851 done, ADR 0752): [[push-main]] runs a failed check two further times and names a flaky test in the ledger line, resumes a stopped run, counts 1.8 s of build per selected binary, takes several merges as one diff (`--base`), takes its times from the newest gate line (`--gate-times`), names an ADR without a mutation patch, regenerates before the checks and runs the script tests. Merged "by the changed script itself, its first real run (4.2 minutes ...)". Follow-up F-860 (todo) carries the lead's answers to findings 10, 11 and 16.
- **Rule 7 change** (maintainer, 2026-11-21, ADR 0752 question 5): "Yes, add it". Agents run `python scripts/process_report.py --check` before their report from version 4 of `docs/process/agent-rules.md`; a generated file that is only not current is named and left to the lead.
- **Process measures** (F-863 done, ADR 0754): four measures recorded in ledger lines and printed by `process_report.py --weekly`: queue age (minutes from the agent's report to the merge), the lead's review minutes, whether the status the lead wrote differed from the status the agent proposed (`proposed=`, `written=`), and escaped defects (`--escaped COMMIT=BINARY::TEST`). The lead noted at the first merge that "a figure made up afterwards would be worse than none" and records report times from then on.
- **Stop conditions in force from 2026-11-21** (maintainer's answers to ADR 0754, recommended option each): "(1) no further agent while 3 or more reports wait unread; (2) none when, over the last 28 days, 2 or more commits brought a failure into a full gate and they are more than 1 in 10 of the merges; (3) while the newest gate line is red, no agent is started except on the fix of a test that failed in it; (4) at most 4 agents at work at once, put again after four weeks of the measures." Condition 3 applied at once (gate line 62a6fec3 red on `listing_budget` L1, F-866).
- **Roadmap proposal files** (F-864 partial, ADR 0755): an agent writes `docs/roadmap/proposals/<ADR>-<item>.json` of "form 1" beside the quoted paragraphs and checks it with `python scripts/roadmap_proposals.py check`; the lead applies it with the script and "still writes `docs/roadmap.json` alone; the agent's entry stands as written, and the lead adds what he knows as an entry of his own after it" (rule 9 from version 5). No cutover to one file per item now (Q2 (a)). First live use: ADR 0757's proposal (see [[roadmap-proposal]]).
- **Why F-864 stays partial:** "the stale check refuses an item to which the lead only added a note; a gap cannot name a follow-up that has no id", and the use on live merges (ADR 0755, answers).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0752](../../docs/adr/0752-f851-registration-push-main-after-its-first-real-runs.md) | 2026-11-21 | registration | F-851 | `scripts/push_main.py` after its first real runs: eight points plus findings | done; 62 unit tests, 13 of 13 mutations caught; F-860 follow-up | F-860 (todo) |
| [0754](../../docs/adr/0754-f863-registration-process-measures-and-stop-conditions-before-more-workers.md) | 2026-11-21 | registration | F-863 | Process measures and stop conditions before more workers | done by its rule; written partial at the merge, done when the maintainer's numbers were written | |
| [0755](../../docs/adr/0755-f864-registration-a-roadmap-proposal-as-a-file-validated-applied-and-replayed.md) | 2026-11-21 | registration | F-864 | A roadmap proposal as a file, validated, applied and replayed on six merged ADRs | proposed done, written partial (the lead's wording replaced the agent's in 6 of 6 notes, 4 of 4 gaps) | |

## Key numbers and limits

- **ADR 0752 lead's answers:** finding 10 option (b), "the start and the end commit in the left-out line"; finding 11 option (b), "the ADR's tests charged to the budget and never cut, their minutes printed in the dry run"; finding 16 option (a), "the binaries named after a changed source file first". "Not shown by any run yet: a further run of a failed check, a resumed run and a joint diff in a real merge."
- **ADR 0754 basis for the numbers:** "its mean push of 23.8 minutes puts three waiting reports at 71 minutes of pushes alone"; "By the roadmap notes 1 merge of 16 did so in November (6 %); the ledger names none"; "On 2026-11-20 and 2026-11-21 four gates in a row were red".
- **ADR 0755 replay:** A1 six of six addenda written as form-1 files without loss; five of five mutations caught. "In the replay the lead rewrote 6 of 6 notes and 4 of 4 gaps." Cost of a cutover by reading: under option (b) none of the readers (`render_roadmap.py`, `roadmap_progress.py`, `process_report.py`, `adr_test_index.py`, two Studio scripts) changes; "what is new is 770 item files".

## Open questions

- None of this thread is open with the maintainer. The cutover (ADR 0755 Q2) "is decided on the differences of the proposals in use".
- ADR 0754: "The optional follow-up (a file that records a report's arrival ...) is not entered" (the lead).

## Related

- [[lead]], [[push-main]], [[process-ledger]], [[process-report]], [[roadmap-proposal]], [[roadmap]], [[maintainer-questions]]
- [[adr-thread-daily-gate-and-edit-lock]], [[adr-thread-simulation-milestones-validation]]
- [[roadmap-quality-docs-and-validation]]
