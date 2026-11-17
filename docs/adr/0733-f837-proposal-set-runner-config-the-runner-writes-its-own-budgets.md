# ADR 0733: F-837 proposal: `set_runner_config`, the runner writes its own budgets file

Status: **PROPOSED 2026-11-17.** The maintainer approved an engine `set_runner_config` command for F-837 (the runner writes
its own budgets file instead of the Studio writing it). This ADR proposes its details; they are not yet accepted. Nothing
is built and nothing was run.

Roadmap item F-837 (run setup, logs and field-view extras), ADR 0801 addendum 13, "Needs the engine": "`rcfd-runner config
set` or an MCP `set_runner_config`, so the budgets are written by their owner, atomically". Parents:
[ADR 0123](0123-wave47b-run-service.md) section 6 (the runner's config), guardrails G2, G4, G16 and G18. Number 0733
follows ADR 0732.

## Context

- The runner's budgets are `rcfd_runner::runner::Config` (`crates/rcfd-runner/src/runner.rs:28-63`): eight keys, every one
  optional, `deny_unknown_fields`. The file is `<project>/.rcfd/runner-config.json` (`runner.rs:74-77`). `Config::load`
  refuses a file with an unknown key or a zero `max_runs`, `cpu_threads` or `poll_ms` (`runner.rs:79-95`).
- The runner reads the file once, when `serve` starts, and records what it uses in `.rcfd/runner.json` (`config`,
  `config_source`; `runner.rs:178-190`). A change takes effect at the next runner start.
- `rcfd-runner` has no command that reads or writes the file (`crates/rcfd-runner/src/main.rs:9-36`, `:97-147`). Tests
  write it with `std::fs::write` (for example `crates/rcfd-runner/tests/runner_wfix.rs:40`).
- The Studio writes it today, from its main process: `RunnerBudgetsFile.write` re-implements the runner's checks
  (`budgetProblem`), compares a sha256 of the file with the base the renderer read, and calls `writeFile`
  (`apps/studio-web/src/main/run/extras.ts:155-264`); `undo` writes the previous text back (`extras.ts:274-283`). The run
  port accepts `setBudgets` and `undoBudgets` (`apps/studio-web/src/main/run/bridge.ts:82-99`, `:197-198`). The command
  registry says so: "the main process reads and writes .rcfd/runner-config.json" (`apps/studio-web/src/shared/commands/studio.ts:275`).
  That is a copy of engine validation in the Studio (G2) and a non-atomic write by a second writer.
- The project already has a compare-and-replace write under an edit lock that journals every accepted write:
  `rcfd_project::atomic::replace_if_hash` (`crates/rcfd-project/src/atomic.rs:319`), with `fnv64:` hashes
  (`crates/rcfd-project/src/records.rs:387`) and conflict error 1200 with `data.current_hash`
  (`crates/rcfd-workspace/src/lib.rs:55`).

## Proposal

### P1. Library (`rcfd-runner`)

`Config::set(project, changes, base) -> Result<ConfigState, ConfigError>`:

- `changes`: a map of key to number or null (null removes the key, so the runner's default applies).
- Reads the current file, applies the changes, and checks the result by deserialising it into `Config` and running the
  same checks as `Config::load`. One check, in one place.
- Writes with `replace_if_hash` (`base` null means "there must be no file yet"), so the write is atomic and journalled in
  `.rcfd/edits.jsonl`.
- `ConfigState`: `{path, exists, hash, values, defaults, in_use: {values, source, started} | null, applies: "next_start"}`.
  `defaults` are `Config::default()` on this machine; `in_use` is read from `.rcfd/runner.json`.
- `Config::state(project)` returns the same object without writing.

Errors: an unknown key or a bad value is refused with the runner's own message and nothing is written; a base mismatch is a
conflict naming the current hash; a current file the runner cannot read (bad JSON, unknown key) is refused unless the
changes replace it whole (Q4).

### P2. CLI (`rcfd-runner`)

- `rcfd-runner config [--project <dir>]`: prints `ConfigState` as JSON.
- `rcfd-runner config set <key>=<number>... [--unset <key>]... [--base <hash>] [--project <dir>]`: prints the new state.
  Exit 0; 1 refused (message names the key and the rule); 4 conflict (Q3). `--base` absent means "whatever is there now".
- The usage text gains these two lines.

### P3. MCP project tools (`rcfd-workspace` and `rcfd-mcp`)

- `runner_config {}`, class `read`: `ConfigState`.
- `set_runner_config {set: {key: number | null}, base_hash: string | null, reason?: string}`, class `write_files`:
  `ConfigState` after the write. Errors 1200 (conflict, `data.current_hash`), 1003 (refused, located at the key), 1100
  under `--read-only`, -32602 for a bad argument shape. One project journal line, with the reason (ADR 0188).

### P4. The Studio

The budgets panel reads with `runner_config` and writes with `set_runner_config` through the project server that run
control already starts without `--read-only`. Undo is a second `set_runner_config` with the previous values and the new
hash as base. `RunnerBudgetsFile.write`, `undo` and `budgetProblem` are removed from `extras.ts`; the Studio no longer
writes into `.rcfd/`.

### How defaults and `baseline_hashes.rs` stay unchanged

- `Config`, `Config::default`, `Config::load` and `serve` do not change; a project with no file runs as before, and a file
  written by this command is read by the same `load`.
- No solver path is touched, so no result changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited.
- The project tool list changes by two tools. That is a registered restatement of `crates/rcfd-mcp/tests/project_list_f270.rs`
  (the golden plus exactly these two tools at a stated place), as ADRs 0186 to 0188 did; the golden is not edited.
  The default list (no `--project`) is unchanged. The runner's help is checked by `contains` (`runner_w47b.rs:53-58`).

## Alternatives

1. **Keep the Studio writer.** Works today, but duplicates the runner's checks in TypeScript and is a second writer
   without the edit lock. Recommendation: no (the maintainer already chose the engine writer).
2. **CLI only.** Enough for the Studio (it can spawn `rcfd-runner`) and scripts, but Claude Code in project mode would
   need a shell. Recommendation: CLI and MCP.
3. **The runner reloads the file while it runs.** Changes apply at once, but the scheduler's budgets would change under
   running runs, which ADR 0123 does not cover. Recommendation: no; say "next start" in the reply.
4. **sha256 base hash, as the Studio uses now.** Keeps the Studio unchanged, but the project's writes use `fnv64:`.
   Recommendation: `fnv64:`, one hash for every project write.

## Open questions for the maintainer

1. **A read tool beside the write tool.** (a) add `runner_config` (read); (b) only `set_runner_config`, the Studio reads
   the file with `read_project_file`. Recommendation: (a), so reading also goes through the owner.
2. **Undo.** (a) the client sends the previous values back (no engine undo); (b) an engine `undo` keyed by the last edit.
   Recommendation: (a).
3. **CLI exit code for a conflict.** (a) 4, distinct from refusals (1) and from `serve`'s 3; (b) 1 for every failure, as
   the runner's other commands do. Recommendation: (a), so scripts can retry on a conflict only.
4. **A current file the runner cannot read.** (a) refuse, unless the call sets `replace: true` and gives a full set of
   keys; (b) keep its unknown keys, as the Studio does now (the runner would still refuse the file); (c) overwrite.
   Recommendation: (a).
5. **Restart hint.** (a) the reply says `applies: "next_start"` and nothing else; (b) also offer to restart the runner
   when no run is active. Recommendation: (a); a restart is a separate action.

## Tests that would prove it

- **R1** `config set` then `Config::load` gives the values set; `--unset` gives the default for that key.
- **R2** each refused value (zero `max_runs`, `cpu_threads` or `poll_ms`, a negative or non-integer count, an unknown key)
  leaves the file byte-identical and gives the runner's message.
- **R3** a stale `base` is a conflict (exit 4, MCP 1200 with `data.current_hash`) and writes nothing.
- **R4** a runner started after `config set` records the written values in `runner.json` (`config`, `config_source`).
- **R5** an accepted write leaves one line in `.rcfd/edits.jsonl` and one journal line with the reason.
- **R6** `set_runner_config` under `--read-only` gives 1100; the project tool list equals the restated golden; the default
  list is unchanged.
- **R7** `baseline_hashes.rs` and every existing runner test pass unedited.
- **Studio** e2e: the budgets panel sets, conflicts and undoes through the engine; a static check that nothing under
  `apps/studio-web/src/main` writes `runner-config.json`.
