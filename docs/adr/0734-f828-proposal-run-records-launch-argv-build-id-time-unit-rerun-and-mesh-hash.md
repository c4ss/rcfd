# ADR 0734: F-828 proposal: run records, the launch argv and build id, the time unit, rerun options and a mesh-file hash

Status: **PROPOSED 2026-11-17.** Not accepted by the maintainer. Nothing is built: no Rust, TypeScript or test code was
written for this ADR, and nothing was run. The maintainer approves the commands below before any code.

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
