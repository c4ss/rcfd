# Brief: F-883, a mean static pressure behind the rotor from the wall-pressure data of the record

Branch: f883-fan01-wall-pressure
Items: F-883
ADR: 0769
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 1; 128563

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f883-fan01-wall-pressure`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f883-fan01-wall-pressure-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0765-f878-registration-fan01-lda-profiles-euler-work-and-the-division-of-the-missing-pressure-rise.md 65998 (the record, the identity and the division; read the sections on the source and the division)
- docs/reference-data.d/0765-F-878.part 5045 (the record's citation and checksums)
- docs/validation-data.md 44529 (how FAN-01's data are recorded)

Also in the repository, as needed: `validation-data/manifest.json`.

## What the maintainer decided (2026-11-22)

ADR 0765 could not divide the missing pressure rise of FAN-01 because the record's velocity data give no pressure behind the rotor. The same public record (Zenodo doi:10.5281/zenodo.23943494) holds wall-pressure data that were not fetched. The maintainer chose a short reading of them beside the rotor loss budget (F-882). The sentence this work must be able to write: the record gives (or does not give) a time-mean static pressure behind the rotor at the design point, and with it the rotor's share of the missing pressure rise is so much with this band.

## Task

One registration, `docs/adr/0769-f883-registration-<slug>.md`, the first commit of this work, alone.

1. **Fetch and checksum** the wall-pressure data of the record (the file ADR 0765 names), and record the
   source as a fragment under `docs/reference-data.d/`.
2. **Read** them for a time-mean static pressure on casing or hub behind the rotor at the design point: the station,
   the uncertainty as the record states it, the file and the place in it. Register before the reading what counts as
   the design point and the mean.
3. **If there is one**, evaluate the identity of ADR 0765 with it and state the rotor's share of the missing pressure
   rise with its band; **if there is none**, record that as the result.
4. **Hold it**: a fast test over the committed transcription.

Decision rule to register: done if the reading gives the pressure with its uncertainty and the share is stated, or if the record is read and holds no such pressure (that is a result); partial if the data cannot be read as registered.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: a new test file, a transcription under `docs/investigations/f883-fan01-wall-pressure/`, the reference fragment, and the manifest entry for the transcription. Another agent runs the rotor's loss budget (F-882) and owns its own files.

## Common to this work

The acceptance of F-883 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. This item runs no solve. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0769-F-883.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
