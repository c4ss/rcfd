# For an agent entering this repository

This file points to the rules; it does not restate them. If a line here and a page it names disagree, the page applies.

- **Your rules:** [docs/process/agent-rules.md](docs/process/agent-rules.md). Read it in full before any work, and cite it
  by path and version in your registration. Your task is a brief under [docs/adr/briefs/](docs/adr/briefs/README.md).
- **Clean-room:** every algorithm comes from published literature or first principles, from sources that were read.
  Vendor documentation is never a source. See rule 4 and [CONTRIBUTING.md](CONTRIBUTING.md).
- **Registration before code:** the ADR that registers your checks and limits is your first commit, alone. ADRs are
  append-only.
- **You do not merge, push or write the roadmap.** The lead does, by
  [docs/process/lead-process.md](docs/process/lead-process.md). Hand back the report your brief asks for.
- **Checks:** the ones your registration names and rule 7 lists, then `python scripts/process_report.py --check`. What is
  checked per merge and in the full run is in [docs/test-process.md](docs/test-process.md), section 7.4.
- **Commits:** plain messages, no trailers, no Co-Authored-By.
- **Studio work** also follows [docs/studio-guardrails.md](docs/studio-guardrails.md).
- **Where state lives:** items in `docs/roadmap.json` (the pages are generated), decisions and evidence in `docs/adr/`,
  merges and full runs in `docs/process/ledger.tsv`. Session memory is not a record.

A proposal of 2026-11-17 for a wider framework is in the repository as a proposal;
[docs/process/framework-reconciliation.md](docs/process/framework-reconciliation.md) says what of it is in force.
