---
type: entity
tags: [process, ledger]
sources: [doc-process-lead-process, doc-process-documentation-review, doc-process-handover-2026-11-19, doc-adr-briefs-readme]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Process ledger (docs/process/ledger.tsv)

**What it is:** the tab-separated record of merges into main and of full runs; "what is merged is the ledger" ([[doc-process-lead-process]]). Written by [[push-main]] and the [[lead]], on main only.
**Where:** [docs/process/ledger.tsv](../../docs/process/ledger.tsv). Header row at 2e94e5ef: `date kind commit branch items adrs result minutes left_out left_out_minutes detail`.

## Details

- **Merge lines** are written by `scripts/push_main.py` with the generated files: branch, items, merge commit, commands, exit codes ([[doc-process-documentation-review]], I2); the item field comes from the branch's brief or the head of the branch name; one line can name every branch of a joint run ([[doc-process-lead-process]]).
- A flaky test is named in the detail (`flaky=`); left-out tests and their minutes are recorded so a later CI failure of one is "an expected kind of event, not a surprise" ([[doc-process-lead-process]], [[doc-process-documentation-review]]).
- **Gate lines** (kind `gate`) are added by the lead when a full run finishes: commit, date, passed, failed, ignored, and for each failed test its primary ADR and item (`scripts/gate_attribute.py`); on green the tag `verified` moves to that commit ([[doc-process-lead-process]]).
- Process measures since F-863 (2026-11-21): `--reported`, `--review-minutes`, `--proposed`, `--written`, and on a red gate `--escaped <commit>=<binary::test>` or `--escaped none`; "a value that is not given is absent from the line, not zero" ([[doc-process-lead-process]]).
- Shows the number of merges on main since the newest known-green full run ([[doc-process-documentation-review]]).
- A brief whose branch no ledger line names, and that carries no "Closed without merge" line, is work issued and not merged ([[doc-adr-briefs-readme]]).
- Early lines: green local gate at 49fbf1a3 (2026-11-12); red gate at `205889bf` and green at `39269587` (2026-11-18, added by PR 27) ([[doc-process-handover-2026-11-19]]); the snapshot's first merge line is 2026-11-19, `f848-site-record-final`, F-848, ADR 0739 (read from the file at 2e94e5ef).
- The lines it appends during a left-out run do not make the tree unclean for the next push ([[doc-process-lead-process]]).

## Related

- [[process-report]]
- [[gate]]
- [[brief]]
