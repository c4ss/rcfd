---
type: source
tags: [adr-thread, runner, run-records, run-sites, remote, provenance, studio, mcp]
sources: [doc-run-control, doc-mcp-tools, doc-service-protocol, adr-thread-wave48-egui-studio]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the runner's budgets, run records, run sites and the order of its final writes (F-837, F-828, F-848, F-262)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: three engine proposals for the Electron Studio, accepted and built on 2026-11-17 (one with a diagnosis of 2026-11-18), and one diagnosis registration of 2026-11-19; 4 ADRs. The run service itself is ADR 0123 (wave 47B), on the records thread of job p3-records-a.

[[rcfd-runner]] owns runs (G7). These ADRs move three things the Studio used to do or could not do into the runner: writing its own budgets file (ADR 0733), recording how a run was launched so it can be reproduced and rerun (ADR 0734), and sending a run to another machine (ADR 0739, "run sites"). ADR 0742 then fixes an ordering fault in how a run's last records are written, found by an intermittent failure of check S7 of ADR 0123.

## Current state

- **Budgets written by their owner** (ADR 0733, built 2026-11-17): `Config::state` / `Config::set` in `rcfd-runner`, `rcfd-runner config [set ...]` (exit 0, 1 refused, 4 conflict), MCP `runner_config` (read) and `set_runner_config` (`write_files`, with `base_hash` and a journalled `reason`). The Studio "no longer writes into `.rcfd/`"; a unit check fails if code under `src/main` names `runner-config.json`. Changes apply at the next runner start (`applies: "next_start"`).
- **Launch records and rerun** (ADR 0734, built 2026-11-17): `runs/<id>/launch.json` (then `launch-<n>.json`), immutable, written before `spawn`, with `argv`, `cwd`, `solver.path`, `solver.version` and `build {exe_hash, git_rev}`; `rcfd version --json`; `time_unit: "s"` in `run_status`, `list_runs` and `data.open`; `rcfd-runner start --continue-from --max-steps`; `rcfd-runner rerun <id>` and MCP `start_run {from_run}`; MCP `hash_project_file` and `rcfd project hash`; `mesh_hash` in `run.json` for mesh-file cases. The Studio's How this ran and Rerun as it ran use them. F-828 stays partial for Studio numbers without provenance ([[adr-thread-studio-engine-entry-points]]).
- **Run sites, phase 1** (ADR 0739, F-848; the engine and the Studio built 2026-11-17): a user-level `sites.toml`; `rcfd site list|check [--confirm]`; the agent `rcfd-runner site --stdio` reached over SSH; content-addressed upload; `rcfd-runner start|rerun --site <name>`; live mirroring of events, residuals, log and status; pause, resume and cancel forwarded; outputs and fields downloaded at the end; MCP `list_sites` and `start_run {site}`; the Studio's Where choice, badge and Sites section (ADR 0801 Addendum 24). The roadmap still lists F-848 as `todo`. `rcfd serve` refuses a non-loopback `--addr` unless `--allow-unauthenticated-remote` (Q9). Phases 2 (studies on a site, resumable downloads, `rcfd site gc`), 3 (Slurm) and later are not built.
- **S1 of the sites tests** failed once in the full gate (2026-11-18: `site.json` not final at the terminal status); a change was registered (the terminal `status.json` only after the final `site.json` was written) and the cause "is still inferred". Rate on the gate's runner: "one failure in two runs".
- **The order of a run's final writes** (ADR 0742, 2026-11-19): S7 of ADR 0123 failed in 6 of 30 runs on the held run because the terminal `status.json` was written before the last state event and before the request files were removed. The hypothesis was confirmed without load (D1 failed in run A as predicted); "the terminal status is now the last write of a run", S7 passed 20 of 20 after. F-262 stays partial: socket control and runs started from the application are not done, S5 is an ignored known miss.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0733](../../docs/adr/0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md) | 2026-11-17 | proposal, accepted, built | F-837 | The runner writes its own budgets file | built; `runner_config` joined the first allow-list (Q answered (b), 16 tools) | the allow-list went with the panel (ADR 0801 Addendum 23) |
| [0734](../../docs/adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md) | 2026-11-17 | proposal, accepted, built | F-828 | Launch argv and build id, time unit, rerun options, mesh-file hash | built; L1: the recorded argv rerun by hand gives byte-identical `store.rts`, `residuals.jsonl` and `output/` | |
| [0739](../../docs/adr/0739-f848-proposal-run-sites-a-remote-simulation-service-for-the-studio-cli-and-mcp.md) | 2026-11-17 | proposal (plan, 10 questions), accepted, phase 1 built; diagnosis 1 2026-11-18 | F-848 | Remote run sites for the Studio, the CLI and the MCP server | phase 1 built; S1 failed once in the gate, change registered | |
| [0742](../../docs/adr/0742-f262-diagnosis-the-terminal-status-of-a-run-is-written-before-its-last-state-event.md) | 2026-11-19 | diagnosis registration (rule 13) | F-262 | Why S7 of ADR 0123 fails intermittently on the held run | hypothesis confirmed; remedy done, every row met in run B; F-262 partial | |

## Key numbers and limits

- **`set_runner_config` errors** (ADR 0733): 1003 with `data.key` (refused), 1200 with `data.current_hash` (conflict), 1201, -32602 for a value that is not a number or null, a missing `base_hash` or an empty `set`; 1100 under `--read-only`. "a current file the runner cannot read is refused unless the call passes `replace: true` (CLI `--replace`) with every key given."
- **Tool counts at ADR 0733 and 0734** (restated, no golden edited): `tool_list_f275.rs` C5-L "42 tools, 25 project tools" (ADR 0733), then "43 tools, 26 project tools" (ADR 0734). The Studio's first allow-list grew to 16 tools (`runner_config`) and 17 (`hash_project_file`), both on the maintainer's (b) of 2026-11-17.
- **Mesh-file hash limits** (ADR 0734 Q5 (b)): "hashing is capped at 4 GiB and 100 000 files, the limit named in the error". A folder hash is `fnv64:` over the sorted `<relative path>\0<file hash>\n` lines, symlinks refused.
- **Run sites** (ADR 0739): ssh is given an argument list `-o BatchMode=yes -o ServerAliveInterval=15 -T -e none -- <host> <engine>/rcfd-runner site --stdio --work <work>` and "never a local shell"; "rcfd stores no key and no secret"; objects stored as `<fnv64>-<bytes>` and verified on arrival; a lost link is "retried after 100 ms, with the wait doubling to 5 s, and then every 30 s"; work root mode 0700, objects 0600. Q6: "Claude Code and other MCP clients may name only a site with `assistant = true` (default false)". Q10: "Another protocol major is refused; another engine version is allowed after the user's yes for that site and version, recorded."
- **Why the Studio does not use `start_run {site}`** (ADR 0739, ADR 0801 Addendum 24): "`start_run {site}` always requires `assistant = true`, whoever calls it"; the Studio starts a remote run with `rcfd-runner start ... --site <name> --no-runner`, then `ensure`, using the store plan of a `start_run` dry run.
- **Sites diagnosis** (ADR 0739, 2026-11-18): "60 runs one after another and 120 runs ten at a time ... none failed"; candidate (a) a failed final write (`atomic_write` retries the rename "for about 0.63 s"), candidate (b) two followers of one run.
- **S7 of ADR 0123** (ADR 0742): failed "in 3 of 20 runs at main 10ba1a34 and in 3 of 10 with one more merge ... 6 of 30, a rate of 0.2"; after the remedy E2 passed 20 of 20, "the registered 0.012 chance of a false pass at the lead's rate"; D1 to D4 are exact. M1 is committed as `crates/rcfd-runner/src/runner/tests/f262.mutation.patch`.

## Open questions

- ADR 0734 "Decisions made in building (for the maintainer)": a missing or unhashable mesh file at submit is recorded without `mesh_hash` and the submit is not refused; a rerun does not copy the study, variant and condition labels; Rerun is offered on a run in any state.
- ADR 0739 left: Where is not on home Run, Run again and Rerun; the Studio cannot create a sites file; `site.jsonl` lines are not feed events; R6 on the reference Linux (no ssh server); phases 2 and 3 (S13 to S15).
- ADR 0739 S1: whether the cause is (a) or (b) is not known; "item 4 is what will show it at the next failure".
- ADR 0742 gap: "Non-terminal state changes (paused, resumed) still write status.json before their state event, and a runner killed inside the final writes leaves a second terminal state event after adoption."

## Records without an item: the run-record contract, how the runner is started, and its liveness (ADRs 0130, 0143, 0146, 0229; added by job p3-records-b)

Before the Electron Studio, wave 48 and one later fix shaped how a client starts the runner and how the runner knows what is alive. ADR 0130 (48A) gave the runner, `rcfd-project` and the Studio one run-record contract (the runner on `rcfd-project` types, tolerant readers, `--store-dir`, opt-in `--status-monitors`); the rest of that ADR is the wave-48 overview on [[adr-thread-wave48-egui-studio]].

- **Started natively** (ADR 0143, 48I): every client starts `rcfd-runner` directly ("the PowerShell launcher is removed"), reaps the runners a process started, and treats process state Z as dead. N1 "measures 0.389 s against the 1.5 s limit (5.341 s on the code before the change)"; Z1 on Linux 0.0025 s against 2.0 s ("before the change, the killed runner still read as alive after 10.011 s"). "every start hands the runner its starter's inheritable handles, so S5 stays an ignored known miss". C3 and P1 became Linux known misses with their limits intact (from ADR 0137, on [[adr-thread-daily-gate-and-edit-lock]]).
- **Output and early exits** (ADR 0146, 48L): a started runner's stdout and stderr go to `<project>/.rcfd/runner.log` by default, with `--log-file`, `--stdout` (alias `--foreground`) and `--quiet`; an early exit is reported at once with its exit code: "0.042 s on Windows and 0.021 s on Linux against 2.0 s, where the code before the change took 10.072 s and 10.015 s". On Windows C1 and C2 (a capturing caller released within 3.0 s) are known misses: "a std-spawned runner holds the caller's pipes whatever its standard handles are, and that only turning inheritance off releases the caller, which also drops the standard handles given to the runner". Question 1 recommends (a), accept and document.
- **Liveness, retries and the shutdown race** (ADR 0229, 2026-11-11, "ACCEPTED 2026-11-11 by the maintainer, in the session that wrote it"): four ways in which what the runner writes could disagree with what happened, each fixed and checked: the lock probe that "collides with itself" (`share_mode(0)`, two probes in the same instant), a shutting-down runner that "still looks alive", an adopted solver "known by its pid and its executable name only" (now identified by a held file), and failed `status.json` writes and stream appends that "are only logged" (now retried; cursors move after the appends). Addendum 1: P1 to W3 all met; runner crate 56 passed, 0 failed, 5 ignored. "No mutation runs were made." The second gate of main 49fbf1a3: "release tests 3548 passed, 0 failed, 1023 ignored (2 h 28 min)"; "The Ubuntu gate has not run."

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0130](../../docs/adr/0130-wave48a-overview-and-run-record-contract.md) | 2026-10-28 | plan + registration | F-274, F-287 | One run-record contract for runner, Studio and plugin | accepted; A1 to A8 passed; `serve --project <relative path>` defect not fixed | |
| [0143](../../docs/adr/0143-job48i-native-runner-start-unix-zombies-linux-known-misses.md) | 2026-10-28 | registration | | Native runner start, dead Unix runners read as dead, two Linux known misses | N1, Z1, S4L met; S5 stays a known miss on Windows | 0146 (Q6, Q1) |
| [0146](../../docs/adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md) | 2026-10-28 | registration + investigation | | Runner output handled explicitly, early exits at once, the capturing caller on Windows | early-exit limits met on both platforms; C1, C2 known misses on Windows; no safe route in product code | |
| [0229](../../docs/adr/0229-runner-liveness-identity-status-and-append-retries-and-the-shutdown-start-race.md) | 2026-11-11 | registration (accepted) | | Solver identity by a held file, retried writes, the shutdown-start race | every check met; Windows debug profile only; gates of f88a18cd (2 timing failures) and 49fbf1a3 (clean) | 0742 (order of the final writes) |

Open from this section: ADR 0146 Q1 (Windows inherited handles: accept and document); ADR 0229 notes that the Studio suite in the debug profile "fails 91 tests on this branch, each with egui's debug-only assertion" (not compared with main), and S2 was narrowed to Windows.

## Related

- Predecessors (added by p3-records-a): `rcfd-runner` was created by ADR 0123 (wave 47B, files in the project folder as queue and transport, no socket) and fixed by the audit of ADR 0128; see [[adr-thread-studio-foundation-crates]].
- [[rcfd-runner]], [[run]], [[run-site]], [[provenance]], [[rcfd-mcp]], [[rcfd-service-protocol]], [[studio]], [[studio-guardrails]] (G7, G10)
- Threads: [[adr-thread-studio-engine-entry-points]], [[adr-thread-workspace-layer]], [[adr-thread-mcp-tools]], [[adr-thread-studio-electron]]
- Areas: [[roadmap-studio-desktop-app]], [[roadmap-claude-and-the-mcp-server]]
