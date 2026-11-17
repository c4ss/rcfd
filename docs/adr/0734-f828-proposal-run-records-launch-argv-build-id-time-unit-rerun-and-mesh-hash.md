# ADR 0734: F-828 proposal: run records, the launch argv and build id, the time unit, rerun options and a mesh-file hash

Status: **ACCEPTED 2026-11-17.** Proposed 2026-11-17; the maintainer accepted it the same day, taking every open
question on its recommendation (answers below). Built the same day; see "As built".

Roadmap item F-828 (Electron Studio: provenance inspector and reproduce-from-CLI), ADR 0801 "Engine follow-ups of the
Studio items", rows E-828a, E-828b, E-828d and E-828e. E-828c (a service session's record) is
[ADR 0735](0735-f828-f814-proposal-session-journals-on-disk-and-claude-code-session-discovery.md). Parents:
[ADR 0123](0123-wave47b-run-service.md) (the run service),
[ADR 0200](0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md) S1 to S3
(`continue_from`, `max_steps`), guardrails G2, G4, G16 and G17. Number 0734: 0733 is the highest ADR on main, 0722 is
held by branch `f840-work`, and 08xx is the Studio range.

## Context

What a run records today:

- `run.json` is written once, at submit, before the run is launched: `rcfd_runner::store::FsStore::create_run_with`
  (`crates/rcfd-runner/src/store.rs:100-146`) fills `rcfd_project::records::RunRecord`
  (`crates/rcfd-project/src/records.rs:67-101`) and writes it with `write_json_new` (immutable, `atomic.rs:155`).
- `solver_version` is the text of `rcfd --version` next to the runner (`crates/rcfd-runner/src/client.rs:255-264`),
  which prints `rcfd <package version>` only (`crates/rcfd-cli/src/main.rs:204-206`). Two builds of one release record
  the same text. The design ledger has a version with the git revision when the build sets `RCFD_GIT_REV`
  (`crates/rcfd-api/src/design/ledger.rs:30-36`), but `rcfd --version` does not use it.
- The solver's command line is built at launch: `solver_cmd` (`crates/rcfd-runner/src/runner.rs:651-665`) and `launch`
  (`runner.rs:731-763`: `--out`, `--status-file`, `--status-every 1 --quiet --threads n`, `--stop-file`, `--pause-file`,
  `--store-dir`, `--status-monitors`, then `--continue-from` and `--max-steps` when set). It is not written anywhere.
  The `launched` event records `pid`, `launch_seq`, `threads` and `mem_mb` only (`runner.rs:780-790`).
- `status.json` `time` (`records.rs:160-164`) is `Simulation::time` (`crates/rcfd-api/src/sim.rs:2998-3006`): simulated
  seconds, 0 for a steady run. The engine stores every quantity in coherent SI units
  (`crates/rcfd-core/src/units.rs:1-3`). Neither the file nor `run_status` names the unit. A store snapshot's time
  (`crates/rcfd-tsstore/src/lib.rs:148-163`) is the same number, also without a unit; `data.open` returns `{step, time}`
  (`crates/rcfd-api/src/data_service.rs:221-243`).
- `continue_from` and `max_steps` are launch settings of `run.json` `settings`
  (`crates/rcfd-runner/src/records.rs:54-94`), reachable from `Client::submit_with` (`client.rs:266-290`) and the design
  study, but `rcfd-runner start` has no option for either (`crates/rcfd-runner/src/main.rs:108-133`) and MCP `start_run`
  has none (`crates/rcfd-workspace/src/runs.rs:208-298`, schema `crates/rcfd-mcp/src/project.rs:302-319`). Nothing
  reruns a run from its own `case.snapshot`.
- `RunRecord` already has `mesh` and `mesh_hash` (`records.rs:74-79`), filled by `rcfd_project::Project::create_run`
  (`crates/rcfd-project/src/project.rs:254-291`) but left `None` by the runner (`store.rs:122-123`). A case's mesh file
  is `mesh.file` (`crates/rcfd-api/src/sim.rs:694-708`), a `.msh` file or an OpenFOAM folder. `read_project_file` hashes
  what it reads (`crates/rcfd-workspace/src/queries.rs:608-643`) but refuses a folder and any file that is not UTF-8
  text.

What the Studio does instead (ADR 0801 Addendum 8 D1 and Addendum 18): it builds the reproduce command from the runner's
start options, says `continue_from`, `max_steps` and a mesh file as gaps in words, captions time "Time, s" in its own
words, and says the snapshot time's unit is not recorded.

## Proposal

### P1. The launch record (E-828a)

- The runner writes `runs/<id>/launch.json` at the launch, immutable (`launch-<launch_seq>.json` for a later launch of
  the same run; Q2): `{launch_seq, argv: [..], cwd, solver: {path, version, build}, started}`.
  `argv` is the exact list `Command` was given, program first; paths absolute as passed. Written before `spawn`; a write
  failure fails the launch as a failed `launched` event does today (`runner.rs:791-806`).
- `build`: `{exe_hash, git_rev}`. `exe_hash` is `fnv64:` of the solver executable's bytes (`records.rs:387`), cached per
  `(path, size, modified)` for the runner's life. `git_rev` is `RCFD_GIT_REV` of the build, or null.
- `rcfd version --json` (new, additive): `{name: "rcfd", version, git_rev, profile, target}`. `rcfd --version` prints
  its text unchanged.
- MCP `run_status` gains `launch` (the latest `launch*.json`, or null) in its reply. CLI: `rcfd-runner status <id>`
  prints it the same way.

### P2. Units of run and snapshot times (E-828b)

- Constants `rcfd_project::records::TIME_UNIT = "s"` and `rcfd_tsstore::TIME_UNIT = "s"`, documented as "simulated
  seconds; 0 for a steady run, whose `step` is the iteration".
- Replies gain members, files do not change: `run_status` and `list_runs` gain `time_unit: "s"`; `data.open` gains
  `time_unit: "s"` (and `length_unit`, ADR 0738 P5). `status.json` and `store.rts` keep their bytes.

### P3. Rerun and launch options (E-828d)

- `rcfd-runner start <case> ... [--continue-from <run-id>] [--max-steps <n>]` through `Client::submit_with`, with its
  checks (`client.rs:276-284`: a run of this project; at least 1). Exit 1 with that message when refused.
- `rcfd-runner rerun <run-id> [--priority n] [--threads n] [--mem-mb n] [--no-runner]`: a new run from
  `runs/<run-id>/case.snapshot` with the record's `settings` (its `set`, `continue_from`, `max_steps`) and `case_dir`,
  so its relative paths resolve as the original's did. The new `run.json` records `case` as the original's and a new key
  `rerun_of: <run-id>` (written only on a rerun). Prints the new id. Options given override the record's.
- MCP `start_run` gains `continue_from` and `max_steps` (as `submit_with`) and `from_run` (instead of `case` or
  `new_case`: the rerun above). Errors: -32602 for `from_run` with `case` or `new_case`; 1202 no such run; 1003 the
  record's `settings` do not read (`LaunchSettings::of`, `records.rs:68-70`).

### P4. A mesh-file hash (E-828e)

- MCP `hash_project_file {path}`, class `read`: `{path, kind: "file" | "folder", hash, bytes, files}`. A file: `fnv64:`
  of its bytes. A folder (an OpenFOAM mesh): `fnv64:` over the sorted list of `<relative path>\0<file hash>\n` of every
  file below it, symlinks refused. Errors 1004 path outside the project, 1202 missing, 1003 over a size limit (Q5). CLI:
  `rcfd project hash <path>` (the `rcfd project` family of ADR 0731).
- At submit, the runner fills `run.json` `mesh` and `mesh_hash` for a case with `mesh.source = "file"` by the same rule
  (Q4). A generator case keeps `mesh` and `mesh_hash` absent, so its `run.json` keeps its bytes.

### P5. The Studio

The reproduce command is `rcfd-runner rerun <id>` when the project is the one open, else `rcfd-runner start ...` with
`--continue-from` and `--max-steps`; the gaps for those go. The provenance shows `launch.json`'s argv and build, says
"changed" when `hash_project_file` of the mesh differs from `run.json` `mesh_hash`, and takes time units from the
replies.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver path, scheme or default changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded.
- `run.json` of a generator-mesh run without the new options is byte for byte as before (`store.rs:119-133` unchanged
  for it). `status.json`, `events.jsonl`, `store.rts` and `index.json` keep their bytes. `launch.json` is a new file.
- `rcfd --version`, `rcfd-runner start` without the new options and `start_run` without the new arguments behave as now.
- New reply members (`launch`, `time_unit`) and the new tool `hash_project_file` change the project tool list and reply
  shapes. The tool list is a registered restatement of `crates/rcfd-mcp/tests/project_list_f270.rs` and
  `tool_list_f275.rs` (the golden plus exactly this tool at a stated place), as ADR 0733 did; no golden is edited.

## Alternatives

1. **Argv in `run.json`.** `run.json` is written before the launch and is immutable; the argv is not known then.
   Recommendation: no.
2. **Argv in the `launched` event.** One file fewer, but it changes the bytes of every run's `events.jsonl`.
   Recommendation: no; a new file changes nothing that exists.
3. **Build id from `RCFD_GIT_REV` only.** A `git archive` build has none (`ledger.rs:30`). Recommendation: the
   executable hash, with the revision when known.
4. **A `time_unit` key in `status.json` and in each store snapshot.** Changes every run's files and the store's bytes.
   Recommendation: no; the unit is fixed by the engine, so a constant and reply members say it.
5. **Hash the mesh in the Studio.** The Studio would carry the folder-hash rule (G2). Recommendation: no.

## Open questions for the maintainer

1. **Where the argv goes.** Recommendation: (a). (a) a new file in the run folder (Q2 says which); (b) the `launched`
   event; (c) only in `log.txt`.
2. **More than one launch.** Recommendation: (a). (a) `launch.json`, then `launch-<launch_seq>.json` for a later launch
   (one immutable file per launch, as `run.json`); (b) one `launches.jsonl`, appended.
3. **Build id.** Recommendation: (a). (a) `fnv64:` of the executable plus `git_rev` when set; (b) `git_rev` only;
   (c) a build script that stamps a build id into the binary.
4. **The runner fills `mesh_hash` at submit.** Recommendation: (a). (a) yes, for file-mesh cases only (their `run.json`
   gains the two keys `RunRecord` already has); (b) no, only `hash_project_file`, so the Studio can say what the hash is
   now but not what it was.
5. **Size limit of `hash_project_file`.** Recommendation: (b), with the limit in the error. (a) none, it streams;
   (b) 4 GiB and 100 000 files.
6. **`rerun` scope.** Recommendation: (a): a rerun needs no case file, so a run whose case changed since still reruns.
   (a) `rerun` and the two `start` options; (b) only the two options.
7. **Time units in replies.** Recommendation: (a). (a) constant `"s"` reply members (`time_unit`), files unchanged;
   (b) also a `time_unit` key in `status.json` and the store's snapshots (changes their bytes); (c) documentation only.

## The maintainer's answers (2026-11-17)

1. Q1 (a): the argv goes in a new file in the run folder, not in the `launched` event.
2. Q2 (a): one immutable `launch-<n>.json` per launch (`launch.json` for the first).
3. Q3 (a): the build id is `fnv64:` of the solver executable, plus the git revision when known.
4. Q4 (a): the runner fills `run.json` `mesh_hash` at submit, for mesh-file cases only.
5. Q5 (b): hashing is capped at 4 GiB and 100 000 files, the limit named in the error.
6. Q6 (a): both `rcfd-runner rerun <id>` and the `--continue-from` / `--max-steps` start options.
7. Q7 (a): replies say `time_unit: "s"`; `status.json` and `store.rts` keep their bytes.

## Tests that would prove it

- **L1** a launch writes the argv; spawning that argv by hand in a copy of the run folder gives byte-identical
  `fields/store.rts`, `residuals.jsonl` and `output/` (G17's comparison).
- **L2** `rcfd version --json` parses; `rcfd --version` prints the old text.
- **L3** `rcfd-runner start --continue-from <id> --max-steps 3` gives the same `run.json` `settings` as
  `Client::submit_with`; a bad id or 0 is refused with nothing written.
- **L4** `rerun` of a finished run gives equal stored fields and residuals, `rerun_of` set, `case` and `case_dir` equal.
- **L5** `hash_project_file` of a `.msh` equals `hash_bytes` of its bytes; of an OpenFOAM folder is stable under file
  order and changes when one byte changes; outside the project 1004.
- **L6** a generator run's `run.json`, `status.json` and `events.jsonl` equal the base commit's byte for byte (times and
  ids masked as in G17); `baseline_hashes.rs` and the runner tests pass unedited.
- **L7** the project tool list equals the restated golden; `run_status` of an old run (no launch file) gives
  `launch: null`.

## Unblocks

F-828 rows E-828a, E-828b, E-828d and E-828e: the reproduce command without gaps for launch options, the build in the
provenance, a "changed" mesh, and time units from the engine.

## As built (2026-11-17)

- **P1, the launch record.** `crates/rcfd-runner/src/runner.rs` `launch`: before `spawn`, the runner writes
  `{launch_seq, argv, cwd, solver: {path, version, build: {exe_hash, git_rev}}, started}` through the new
  `RunStore::write_launch` (default method, `write_json_new`, so immutable) to `runs/<id>/launch.json`, or
  `launch-<launch_seq>.json` when `launch.json` exists (`store::launch_file`). `argv` is the `Command`'s program and
  arguments as given. A write failure fails the launch with "cannot record the launch in <dir>: ...; the solver was not
  started" and the solver is not spawned. `crates/rcfd-runner/src/build.rs` `SolverFacts` caches the solver's facts per
  `(path, size, modified)` for the runner's life: `exe_hash` by `filehash::hash_path`, `version` from `rcfd --version`,
  `git_rev` from the solver's own `rcfd version --json` (null when the solver has no such command or no revision).
  `store::latest_launch` picks the record with the latest `started`.
- **`rcfd version [--json]`** (`crates/rcfd-cli/src/main.rs`): `{name, version, git_rev, profile, target}`; `git_rev`
  is `RCFD_GIT_REV` at build time or null; `target` is `<arch>-<os>` from `std::env::consts` (no build script); `rcfd
  --version` is unchanged. `rcfd version` alone prints `rcfd <version>`.
- **`run_status`** gains `launch` (the latest launch record, or null) and `time_unit`; `list_runs` gains a top-level
  `time_unit`; `rcfd-runner status <id>` prints its JSON with a `launch` member.
- **P2.** `rcfd_project::records::TIME_UNIT` and `rcfd_tsstore::TIME_UNIT` (`"s"`); `data.open` replies `time_unit`.
  `status.json`, `events.jsonl` and `store.rts` keep their bytes. `length_unit` is ADR 0738's and was not added here.
- **P3.** `rcfd-runner start ... --continue-from <id> --max-steps <n>` goes through `Client::submit_with` and its checks
  (now one `check_launch`). `rcfd-runner rerun <id> [--priority n] [--threads n] [--mem-mb n] [--no-runner]`:
  `Client::rerun` and `FsStore::create_rerun` copy the original's `case.snapshot`, `case`, `case_source`, `case_dir` and
  `settings` (overridden by the options given), set `rerun_of`, and print the new id. `RunRecord.rerun_of` is skipped
  when absent, so other records keep their bytes. MCP `start_run` gains `continue_from` (string), `max_steps` (integer,
  minimum 1) and `from_run` (string); `from_run` with `case`, `new_case`, `set`, `store`, `continue_from` or `max_steps`
  is -32602; no such run 1202; unreadable settings 1003; reply `{run, case, rerun_of, state}`. The project journal's
  `run_submitted` line carries `continue_from`, `max_steps` and `rerun_of` when set.
- **P4.** `crates/rcfd-project/src/filehash.rs` `hash_path` / `hash_path_limited` (re-exported as
  `rcfd_workspace::filehash`): a file is `fnv64:` of its bytes, streamed; a folder is `fnv64:` over the sorted
  `<relative path>\0<file hash>\n` lines, symlinks refused; limits `MAX_BYTES` 4 GiB and `MAX_FILES` 100 000, named in
  the error (`max_bytes`, `max_files`). MCP `hash_project_file {path}` (read class, after `read_project_file`):
  `{path, kind, hash, bytes, files}`; 1004 outside the project, 1202 missing, 1003 over a limit. `rcfd project hash
  <path>` prints the same object. At submit, `crates/rcfd-runner/src/mesh.rs` `case_mesh` reads `mesh.source` and
  `mesh.file` of the case (TOML or JSON, after `--set`) and, for `source = "file"`, the runner fills `mesh` (project
  relative when inside, else absolute) and `mesh_hash`. A generator case keeps both absent.
- **P5, the Studio** (`apps/studio-web`). The selected run of Runs gains two icons (no words at rest, G22): **How this
  ran** (`src/renderer/runs/HowItRan.tsx`, in the row of Log and events) shows the launch record's command line, folder,
  solver, build and start time, the mesh with its recorded hash and "unchanged" or "changed: <hash> now"
  (`hash_project_file` against `mesh_hash`), `rerun_of`, and the rerun command with Copy; **Rerun as it ran**
  (`project.rerun_run`, `start_run {from_run}` on the run-control server; CLI twin `rcfd-runner rerun <id>`). The
  provenance's reproduce command carries `--continue-from` and `--max-steps` (those gaps are gone); a recorded mesh is a
  gap only when it changed, has no hash, cannot be hashed or is outside the project. The unit of a run's and a
  snapshot's time comes from `run_status` `time_unit` (the Runs caption and the provenance of a time).
  `hash_project_file` joins the Studio's read-only project tools. The capability map gains the new tool and commands.

### Tests run (2026-11-17, release, headless)

- New: `crates/rcfd-runner/tests/run_records_adr0734.rs` (L1: the recorded argv run by hand in a copy of the run folder
  gives byte-identical `fields/store.rts`, `residuals.jsonl` and `output/`, and the runner's exit code; L3: the start
  options and their refusals with nothing written; L4: rerun through the CLI; L6: a generator run's `run.json` keys are
  main's; P4: `mesh_hash` of a `.msh`, of a folder by `--set`, and a missing file recorded without a hash),
  `crates/rcfd-cli/tests/version_adr0734.rs` (L2), `crates/rcfd-mcp/tests/run_records_adr0734.rs` (L5: the tool and the
  CLI give the same hash, a folder stable and changed by one byte, 1004 and 1202; L7: `launch: null` and `time_unit`;
  P3 through `start_run`), and unit tests of `filehash` and `case_mesh`.
- Restated, no golden edited: `project_list_f270.rs` (`hash_project_file` and the three `start_run` properties taken
  out at their place before ADR 0733's block), `tool_list_f275.rs` (43 tools, 26 project tools), `project_w48f.rs` F3
  (17 + 26) and F2 (the first allow-list is the read-class project tools except `hash_project_file`, not decided then; decided
  2026-11-17 below: no exception),
  `project_runs_w48f.rs` (43 tools; `run_status`'s `launch` and `time_unit` and `list_runs`'s `time_unit` taken out
  of the shape before comparing).
- `crates/rcfd-api/tests/baseline_hashes.rs` is not edited; no solver path, scheme or default changed.
- Studio: typecheck, build, `package:dir`; unit `provenance`, `run-control`, `pictograms`, `capabilities`, `runs`,
  `f837`, `claude`, `project-home`, `compare`; e2e `provenance` (with How this ran and Rerun as it ran), `runs`,
  `run-control`, `f837` and `word-budget`, all within their budgets (none raised).

### Decisions made in building (for the maintainer)

- A mesh file missing or unhashable at submit is recorded (`mesh`) without `mesh_hash`; the submit is not refused.
- A rerun does not copy the original's study, variant and condition labels (it is not a member of the study).
- `start_run {from_run}` refuses `set`, `store`, `continue_from` and `max_steps` beside it; the CLI `rerun` takes only
  `--priority`, `--threads`, `--mem-mb` and `--no-runner`, as P3 lists.
- `rcfd project hash` takes any path (it has no project to stay inside); `hash_project_file` stays inside the project.
- Rerun is offered on a run in any state; the engine decides.
- Was not decided: whether `hash_project_file` joins the Studio bridge's first allow-list (`PROJECT_READ_TOOLS`, ADR 0195
  P5), as ADR 0733 asked for `runner_config`. (a) leave the list as it is (built; F2 restated with this one exception);
  (b) add it, which restates C5-A and F2 again. Recommendation: (b), it only reads.
- **Decided 2026-11-17: joins the first list** (the maintainer, (b)). `hash_project_file` is added after `runner_config`:
  `PROJECT_READ_TOOLS` in `apps/studio-web/src/main/claude/allow.ts` and its Rust twin in
  `crates/rcfd-mcp/tests/claude_allow/mod.rs` (17 tools). Restated, no golden re-recorded: `tool_list_f275.rs` C5-A (main's
  11, then `read_feed`, `check_case_mesh`, `preview_mesh`, `list_meshes`, `runner_config`, `hash_project_file`),
  `project_w48f.rs` F2 (exactly the 17 read-class project tools, no exception left), the Studio unit test
  (`claude.spec.ts`, 17) and the e2e checks (`17 allowed, everything else denied`, 17 list items, 17 tool flags). An
  existing allow-list file is not changed. The README's list and `docs/mcp-tools.md` say 17.
