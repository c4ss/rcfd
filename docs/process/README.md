# Process

How work on rcfd is issued, recorded, checked and merged. [agent-rules.md](agent-rules.md) is the rule set every agent
follows, in both streams. [lead-process.md](lead-process.md) is the lead's side: issuing, merging, the single writer of each
shared file, questions to the maintainer and the full run. [tolerances.md](tolerances.md) is the draft table of allowed
limits by class of check. [Briefs](../adr/briefs/README.md) holds what each agent was told. `ledger.tsv` in this folder is
the record of merges into main and of full runs, written on main only; the `exceptions/` folder holds the lists of known
cases that the checks of the process report pass over, which only shrink.
[documentation-process-review.md](documentation-process-review.md) is the review these pages came from, with the
maintainer's decisions of 2026-11-18.
[framework-reconciliation.md](framework-reconciliation.md) says how a wider proposal of 2026-11-17, kept in the folder
`framework-2026-11-17/`, stands against these pages: what of it is adopted and what is not.

Commands, from the repository root:

    python scripts/process_report.py            print the report
    python scripts/process_report.py --write    write the generated pages ([the ADR index](../adr/index.md), the overview numbers, the area lists)
    python scripts/process_report.py --check    exit non-zero if a generated page is stale or a check fails (CI)
    python scripts/process_report.py --weekly   the weekly list for the maintainer
    python scripts/push_main.py                 the lead's checks, selected tests, ledger line and push after a local merge
