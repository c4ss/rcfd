# ADR 0308: Command-line subcommand and MCP tools for the solid solver (needs the maintainer: two golden files change)

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks. **Blocked
until the maintainer approves two golden edits (decision N2 of ADR 0290).**

Work package SI-18 of the structural solver, intermediate tier. Size S.

## Goal

Expose the solid case file to users and to the assistant: a subcommand `rcfd solid <file>` (run, with `--check` to validate only)
and two MCP tools (validate a solid case, run it and return the JSON report). This is option (b) of decision M3 of ADR 0270. It
CANNOT be done under house rule 3 without the maintainer: a subcommand changes the CLI help golden and a tool changes the MCP
tool-list golden. It is kept apart from every library package so that none of them waits for it.

## Dependencies

ADR 0306; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-16, F-386.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. NEEDS THE MAINTAINER'S APPROVAL of two golden edits before it
starts. rcfd solid <file> runs a solid case file and writes the files and the report of the library entry with the same bytes;
--check validates without solving; exit codes and located errors follow the existing subcommands (an unknown key exits non-zero
with the key and the line). The MCP tools solid_check and solid_run return the validation result and the JSON report of the same
run. The CLI help golden and the MCP tool-list golden change by exactly the added subcommand and the two added tools (the diff is
listed in the addendum for the maintainer); every other golden, the hash tables and the flow-case behaviour are unchanged; the
coupled run of ADR 0307 is reachable only through the library until a further decision.

## Reference to check against

The existing subcommands and tools are the model (their tests in `crates/rcfd-cli/tests/cli.rs` and `crates/rcfd-mcp/tests`); the
numbers are those of the library, compared by bytes.

## Files

`crates/rcfd-cli/src` (the subcommand), `crates/rcfd-mcp/src` (the tools), their tests `si_solid_cli.rs` and `si_solid_tools.rs`,
and, by the maintainer's approval only, the CLI help golden and `crates/rcfd-mcp/tests/golden` (tool list).

## What could make it miss

No approval: the package does not start. Whether the assistant layer (`rcfd-assistant`) keeps its own list of tools that must
change with the MCP list was not read.
