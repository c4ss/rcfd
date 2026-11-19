# Brief: the weekly citation item of week 47

Branch: citations-2026-w47
Items: none
ADR: 0745
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 0; 0; 26035

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/citations-2026-w47` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/citations-2026-w47-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/process/lead-process.md 10303
- docs/process/citation-batches.md 3829

## Task

The weekly citation item of `docs/process/lead-process.md` ("Weekly", and "Citations from memory: the Sources read
addendum"). Record: `docs/adr/0745-citations-2026-w47-audit-five-citations-drawn-by-seed.md`, registered before any reading.

- Registration: the population (every ADR under `docs/adr` that contains the words "from memory" and has no
  `## Sources read` heading; print its size), the seed 20261119, and the draw: Python's `random.Random(20261119)` over the
  sorted file names picks five ADRs, and in each the first citation from memory in file order. Commit the registration with
  the five named, then read.
- For each of the five, re-read the citation against its source and append a `## Sources read (2026-11-19)` addendum to that
  ADR in the form lead-process.md gives (citation, status, where, finding). `read` only for a source you opened in this
  task and found the place in. A second-hand statement is `read` of that second source, named as such.
- Vendor documentation is never a source and is not opened. A publisher's page that refuses the fetch is `not obtainable`
  with the addresses tried; do not retry with another client name and do not download files through a browser dialog.
- Nothing above an addendum is edited. Differences are listed at the end of each addendum under "Differences for the lead",
  with the code that uses the value. No code changes.
- Addendum of the record: the five rows, the count of ADRs that cite from memory before and after (the report of
  `python scripts/process_report.py --weekly`; it may not rise), and a line for `docs/process/citation-batches.md`, which
  the lead adds.

The maintainer has not yet decided how far the citation work goes beyond this weekly item: do these five and no more.
