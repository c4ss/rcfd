# ADR 0739: F-848 proposal: run sites, a remote simulation service for the Studio, the CLI and the MCP server

Status: **Accepted 2026-11-17.** The maintainer answered all ten open questions with option (a) on 2026-11-17 (see
"Answers" after the open questions). Q9 and the engine part of phase 1 are built; see "Built" at the end.

Roadmap item F-848 (Electron Studio and engine: runs on a remote site). Parents:
[ADR 0123](0123-wave47b-run-service.md) (the run service),
[ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (the workspace layer),
[ADR 0734](0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md) (the launch record),
[ADR 0350](0350-electron-studio-performance-spike-and-rcfd-studio-retirement.md) (the `data.*` channel),
[ADR 0058](0058-wave29d-distributed-parallel.md) (the distributed-memory layer),
ADR 0801 Addendum 21 (the engine found without setup), guardrails G2, G4, G7, G10, G12, G16, G17 and G22. Number 0739:
0738 is the highest ADR on main and no branch holds a 0739 (`git log --all -- 'docs/adr/0739*'` is empty).

Clean room: the design below uses public, general knowledge of SSH, TLS, batch schedulers and content-addressed
storage. No other program's source, binary or documentation of internals was read or is to be read for this work.

## The maintainer's direction (2026-11-17)

> we should open the studio and if no external simulation service is specified it spins it up and supplies it,
> otherwise we can use an external one that is in the cloud to run high intensity or sharded or distributed simulations

and, in ADR 0801 Addendum 21 (`docs/adr/0801-studio-web-dependencies-and-layout.md:3135-3137`): "of course simulations
should be able to be run in the cloud or on clusters".

## Context (checked against main 21fa5c34)

### The three engine processes the Studio uses

1. **The session service, `rcfd serve`.** JSON-RPC 2.0, one object per line, over TCP or standard streams
   (`docs/service-protocol.md:22-25`; `Service::serve_tcp`, `crates/rcfd-api/src/service.rs:1947-1972`, one thread per
   connection; `Service::serve_stream`, `service.rs:1912-1944`). `--root` confines every client path
   (`docs/service-protocol.md:27-30`; `ServiceConfig::root`, `service.rs:134-147`). `cmd_serve`
   (`crates/rcfd-cli/src/main.rs:957-1008`) binds whatever `--addr` says, `127.0.0.1:7878` by default (`main.rs:998`).
   There is no authentication, and **the engine itself does not refuse a non-loopback address**: `rcfd serve --addr
   0.0.0.0:7878` listens on every interface today. The loopback rule of G10 is enforced only by the Studio
   (`parseLoopbackAddress`, `apps/studio-web/src/main/service/config.ts:33-46`), which starts the service with
   `--addr 127.0.0.1:0` (`apps/studio-web/src/main/service/spawn.ts:3-7`). `service.info` reports `protocol`
   (`rcfd-service/1`) and `protocol_version` (`"1.1"`; `service.rs:28-32`, `service.rs:484-493`).
2. **The project server, `rcfd-mcp --project <folder>`**, spoken to over its standard streams. Everything the Studio
   shows of a project goes through it: the lists, the change feed (`read_feed`), residuals (`read_residuals`), the
   provenance (`read_project_file`, `hash_project_file`) and run control, on a second instance without `--read-only`
   (`apps/studio-web/src/main/project/server.ts:1-6`, `project/control.ts:1-5`, `project/session.ts:1-4`,
   `run/series.ts:1-4`, `provenance/record.ts:1-7`). `--project` implies `--root <project>`
   (`crates/rcfd-mcp/src/main.rs:58-76`).
3. **The run service, `rcfd-runner`**, one detached process per project folder. "Clients and the runner talk through
   files only" (`crates/rcfd-runner/src/lib.rs:3-5`): the project folder is the queue, each run is an `rcfd run` child
   whose output goes to files, so the solver outlives the runner and a later runner adopts it; pause and cancel are
   request files the solver watches (`crates/rcfd-runner/src/runner.rs:1-7`). The Studio starts it with `rcfd-runner
   ensure --project <folder>` (`project/control.ts:337`). Its budgets are `max_runs`, `cpu_threads`, `mem_budget_mb`
   and five more (`crates/rcfd-runner/src/config.rs:18-27`; defaults `runner.rs:54-60`: 2 runs, half the cores, 4096
   MB), checked at `runner.rs:619-640` and `runner.rs:685-690`. Branch `runner-lifetime` adds that the service and the
   runner close with the Studio unless the user keeps them.

The runner reaches the run records only through the trait `RunStore` (`crates/rcfd-runner/src/store.rs:31-66`), whose
one implementation `FsStore` is the project folder. `Runner::launch` (`runner.rs:734-810`) builds the `rcfd run`
command (`--out`, `--status-file`, `--threads`, `--stop-file`, `--pause-file`, `--store-dir`; `runner.rs:745-760`).

### The run folder

`docs/ui-restart-plan.md:98-118`: `runs/<id>/` holds the immutable `run.json`, `case.snapshot`, the atomically replaced
`status.json`, the append-only `events.jsonl` and `residuals.jsonl`, `log.txt`, `fields/` (the time-series store) and
`metrics.json`. The runner adds `pause`, `pause.ack`, `cancel`, `solver-status.jsonl`, `output/` and `check.json`
(`store.rs:12-29`) and the launch record `launch.json` / `launch-<n>.json` (`store.rs:236-262`), written before the
spawn with `{launch_seq, argv, cwd, solver: {path, version, build: {exe_hash, git_rev}}, started}`
(`runner.rs:776-784`; `crates/rcfd-runner/src/build.rs:1-4`). No launch record names the machine. `run.json` names the
mesh by project-relative path and `mesh_hash` (`crates/rcfd-project/src/records.rs:71-108`); the mesh itself is not
copied into the run folder. Run states: `queued running paused finished failed cancelled` (`records.rs:39-52`).

### The project folder and the change feed

`rcfd-project` reads and writes the folder (`crates/rcfd-project/src/lib.rs:1-6`). The watcher is poll-based with
change tokens, content-hashing files up to 1 MiB (`crates/rcfd-project/src/watch.rs:1-9`). The feed merges the project
journal (`.rcfd/edits.jsonl`), the runs' `events.jsonl` and the watcher's changes; "the files stay the truth; a reader
polls them" (`crates/rcfd-workspace/src/feed.rs:1-16`). Hashes are `fnv64:` throughout; a mesh file or an OpenFOAM
folder has a defined hash (`crates/rcfd-project/src/filehash.rs:1-8`). FNV-64 is not a cryptographic hash.

### The field data the viewport reads

Two routes (`apps/studio-web/src/main/data/studio-data.ts:1-3`). R2 reads `store.rts` of a project on this machine in a
worker thread, only inside the data roots (`data/local.ts:1-3`, `data/roots.ts:1-8`, the reader `data/tsstore.ts:1-12`,
whose frames each carry an `fnv64` of their payload). R1 asks the connected service's `data.*` methods and fetches the
bytes on a second TCP connection to the port `data.info` names (`data/remote.ts:1-9`;
`docs/service-protocol.md:138-159`). `rcfd serve` does not switch `data.*` on (`docs/service-protocol.md:17-20`;
`cmd_serve` never calls `Service::enable_data`, `crates/rcfd-api/src/data_service.rs:81`). The data port has no
authentication and its tickets are consecutive integers (`data_service.rs:149`): safe on loopback, not on a network.
`data.field` already reads one field of one snapshot, which is the natural unit of a range read.

### Parallel work the engine has

- **Threads.** Shared memory with `rayon`, deterministic chunking (ADR 0003). `rcfd run --threads n`; "results are
  bit-identical for any thread count (measured)" (`docs/run-control.md:56`, `:153`). The runner passes `--threads`
  (`runner.rs:751-752`).
- **One GPU.** `linear_solver.device` puts the segregated pressure solve on a device (`docs/gpu.md:17`); the GPU
  backend of F-101 is single-device (F-100 note in `docs/roadmap.json`).
- **Distributed memory: a library, not a solver.** `rcfd-dist` has a transport-agnostic `Communicator`, halo exchange,
  a distributed CSR matrix and distributed CG and BiCGStab, with only the serial and the in-process thread transports;
  "an MPI or socket backend ... is NOT part of this crate" (`crates/rcfd-dist/src/lib.rs:1-12`). ADR 0058 (`:6-16`):
  no MPI or socket transport, `rcfd run` not wired, distributed SIMPLE/PISO not attempted, no cluster scaling claim.
  The partitioners and `Decomposition` are in `crates/rcfd-mesh/src/partition.rs:24-207`. The solid solver's
  distributed CG is library only, "nothing here is reached from a case file, the CLI or the default static solve"
  (`crates/rcfd-api/src/solid_parallel.rs:1-12`). `rcfd run --hostfile` is rejected with exit 2, "needs F-100"
  (`docs/ROADMAP.md:2840`). F-100 is `todo`.
- **Work already split into runs.** With `--project`, `design_run_study` and `design_evaluate` make each candidate at
  each operating condition one runner run, tagged `study`, `variant`, `condition`, and a matching run is adopted rather
  than run again (`docs/mcp-tools.md:100-110`; ADR 0189, ADR 0201). `workers` candidates are in flight at once and the
  runner's `max_runs` bounds how many run (`docs/design-loop.md:275`). A chained condition continues from the run of
  the condition before (`settings.continue_from`), and an operating map runs each point from the converged point before
  (ADR 0200), so chains are serial inside and parallel across. `rcfd study --parallel n` runs a CLI study in one process
  (`docs/studies.md:28`). Co-simulation over the service uses leases (ADR 0345, `docs/cosimulation-protocol.md`).

### What G10 says

`docs/studio-guardrails.md:85-89`: "`contextIsolation` on, `nodeIntegration` off, the renderer sandboxed, a strict
Content Security Policy, no remote content loaded, and an allow-list of IPC calls through the preload script. The
service listens on loopback only unless the user says otherwise. Electron stays on a release its maintainers still
support." The Studio's attach (`--service host:port`, `RCFD_STUDIO_SERVICE`) accepts loopback hosts only
(`config.ts:3-4`, `:55-69`); its comment already says "The attach is where a service on another machine (cloud,
cluster) will plug in later" (`config.ts:15`).

## Proposal

### P1. The idea: a run site

A **run site** is where the runner launches a run's solver. There is always the site `local`: today's code path, the
runner the Studio starts (Addendum 21). A **remote site** is a machine (a workstation, a cluster login node, a cloud
VM) that has an rcfd engine installed and that the user can reach. The user names remote sites in one file; when no
site is named, everything is as today. That is the maintainer's "spins it up and supplies it, otherwise we can use an
external one".

The project folder stays on the user's machine and stays the truth (G12). A run on a remote site has its folder
`runs/<id>/` created locally exactly as now; the local runner sends its inputs to the site, the site runs it, and the
local runner keeps the local run folder up to date from the site. So the Studio, Claude Code, the CLI and the MCP
server keep reading one folder through the code they read it with today, and a remote run is in the same Runs list,
feed and run workspace as a local one.

The interactive session service (`rcfd serve`, case editing, `mesh.preview`, `case.facts`) stays local in every phase
before P8: editing a case needs no cluster.

### P2. Topology

| | what runs where | for | cost |
|---|---|---|---|
| T1 | a remote `rcfd-runner` per project on the site, over a mirror folder, reached by a stdio agent | one machine or a login node with enough cores; sharded studies | small: the runner, unchanged, runs there |
| T2 | T1 with a scheduler adapter behind the remote runner (Slurm, then PBS): each run, or a pilot job holding the runner, is a batch job | clusters where login nodes must not compute | the adapter and its job states |
| T3 | cloud batch: inputs to object storage, one container job per run, results back to object storage | elastic capacity without a cluster | accounts, storage, images, billing; its own ADR |
| T4 | a remote `rcfd serve` the Studio attaches to | live sessions on big hardware, remote co-simulation | the session protocol over a network, the second data port |

**Recommended: T1 first**, with T2 and T3 as later adapters behind one interface in `rcfd-runner`:

```text
trait Site {                                   // crates/rcfd-runner, beside RunStore
    fn hello(&mut self) -> SiteFacts;          // protocol, engine version, build id, os, arch, cores, budgets
    fn missing(&mut self, blobs: &[BlobRef]) -> Vec<BlobRef>;
    fn put(&mut self, blob: &BlobRef, from: u64, bytes: &[u8]);
    fn submit(&mut self, run: &RemoteRun) -> RemoteId;    // the run folder appears complete or not at all
    fn tail(&mut self, run: &RemoteId, file: &str, from: u64) -> Tail;  // append-only files by byte offset
    fn read(&mut self, run: &RemoteId, file: &str, from: u64, len: u64) -> Bytes;  // status.json, outputs, fields
    fn request(&mut self, run: &RemoteId, what: Request);  // pause, resume, cancel
    fn list(&mut self) -> Vec<RemoteState>;
}
```

`Local` is not a `Site` implementation: the local path keeps its code, so it cannot change (P9). T4 is P8 below.

### P3. Transport and authentication

| | how | listener | secrets kept by rcfd |
|---|---|---|---|
| A1 | `ssh <host> <engine>/rcfd-runner site --stdio --work <dir>`: the agent speaks line JSON-RPC (`rcfd-site/1`) on its standard streams | none on either machine | none: OpenSSH's keys, agent, `known_hosts` and `~/.ssh/config` |
| A2 | an SSH local forward to a remote loopback port of an agent or `rcfd serve` | loopback on the site | none |
| A3 | TLS listener with bearer tokens | public port on the site | a token per site, in the OS keychain |
| A4 | mutual TLS | public port | a client key and certificate per user |

**Recommended: A1 first.** Nothing listens anywhere, so the loopback sentence of G10 holds without change. The user's
SSH set-up (keys, jump hosts, agents, cluster two-factor) works as it is; the Studio stores no secret; the agent lives
only as long as the SSH session. A2 needs a forward per port (the data channel is a second port whose number `data.info`
reports, so a forward needs a port map), and on a shared login node any user of that node can connect to a loopback
port. A3 and A4 belong to a hosted multi-user service (T3) and need their own ADR on accounts. OpenSSH is part of
Windows 10 and later, macOS and Linux; the engine calls the system `ssh` with an argument list, never a shell.

**Naming a site.** One user-level file, `<config>/rcfd/sites.toml` (`%APPDATA%\rcfd\sites.toml` on Windows,
`~/.config/rcfd/sites.toml` elsewhere); projects never hold hosts or credentials (Q8):

```toml
[site.lab]
kind = "ssh"                       # later: "slurm", "pbs" (T2); "batch" (T3)
host = "me@hpc.example.org"        # given to ssh as one argument; ~/.ssh/config applies
engine = "/opt/rcfd/0.9.4/bin"     # the site's rcfd, rcfd-runner (and rcfd-mcp)
work = "/scratch/me/rcfd"          # the site's work root, created 0700
max_runs = 4                       # written to the mirror's runner config (ADR 0733)
cpu_threads = 64
mem_budget_mb = 250000
assistant = false                  # Q6: whether MCP clients may name this site
```

Named and used by: the CLI, `rcfd site list | check <name> | gc <name>` and `rcfd-runner start <case> --site <name>`;
the MCP server, the tool `list_sites` (class `read`) and an optional `site` on `start_run`, `rerun_run`, and the
design tools with `--project` (class `execute`, as now); the Studio, a Sites page in settings (add, check, remove; it
writes the same file through `rcfd site`, G16), a "Where" choice in the run dialog, and `--site <name>` /
`RCFD_STUDIO_SITE` for the default of a session. `--service` and `RCFD_STUDIO_SERVICE` stay loopback only.

**How G10 changes (Q3).** The listen sentence stays. One sentence is added: "The Studio reaches another machine only
through a run site the user named in their sites file and confirmed once (the host key fingerprint and the engine
version shown); project data goes there only for a run the user started there." Confirmation is per site, stored in
the sites file (`confirmed = "<fingerprint>"`); a changed host key is refused by OpenSSH itself.

### P4. Data movement

**Where things live.** Project local and primary; the site holds a mirror (Q1):

```text
<work>/objects/<16 hex fnv64>-<bytes>          content-addressed inputs, shared by the user's projects on the site
<work>/projects/<key>/rcfd-project.json        a copy; <key> = <name>-<fnv64 of the local absolute path and created>
<work>/projects/<key>/runs/<id>/               the same run id as the local run; written by the site's runner
```

The site's runner is the ordinary `rcfd-runner` over that mirror project, with the site's budgets in its
`.rcfd/runner-config.json`. Local budgets do not count remote runs; the site's runner queues what does not fit.

**Inputs.** A run's inputs are `case.snapshot`, the mesh `run.json` names (`mesh_hash`, ADR 0734 P4), the parent's
`output/restart.rcfd` for `continue_from`, and every other file the case reads (user-function `.wasm` modules,
`docs/udf-abi.md`; tables and profiles). The engine has no list of the files a case reads; this needs one new
read-only entry point (`rcfd check --inputs --json`, the same rule in `rcfd_api`), each path with its `filehash` hash.
Upload: `missing` with the hashes and sizes, then `put` in pieces from an offset (resumable); the agent writes
`objects/tmp-*`, checks hash and length, and renames. A blob already there costs nothing, so the second run on one
mesh uploads only its case. The run is submitted only when every input is present; on the site the agent builds
`runs/<id>/` with the inputs at the paths the case expects (hard links into `objects/` where the file system allows,
copies otherwise) and the site's runner creates it with `create_run`, which "appears complete or not at all"
(`store.rs:41-42`).

**Streaming.** The local runner polls the agent every `poll_ms` (ADR 0123) and copies into the local run folder:
`status.json` (replaced atomically, as now), and the new bytes of `events.jsonl`, `residuals.jsonl`, `log.txt` and
`pause.ack` from the byte offset it has (the files are append-only, so a copy is exact and survives reconnection). The
local copies are byte copies; what only the link knows goes to two new local files, `runs/<id>/site.json` (site name,
remote folder, the site's facts at submit, last contact) and `runs/<id>/site.jsonl` (link events: connected, lost,
uploaded, downloaded). The feed reads `site.jsonl` lines as run events (additive). Nothing in the Studio polls the site;
it keeps polling the feed.

**Fields and outputs (Q4).** Recommended: at `finished`, `failed` or `cancelled` the local runner downloads `output/`,
`metrics.json`, `launch*.json` and `fields/` by ranged reads, resumable, each `store.rts` frame checked by its own
`fnv64`; then the viewport reads them through R2 unchanged. While a run is running, the residuals, monitors and status
are live and the fields are not. Later (P7) a "fields on first open" setting and remote `data.*` for stores too large to
copy.

**Bandwidth and caching.** No SSH compression by default (stored fields are already deflated); the run dialog says the
bytes to upload before the first run on a site, and the Runs list shows download progress. Objects stay on the site
until `rcfd site gc <name>` removes those no run folder of the site references; nothing is deleted on its own.

### P5. Scale-out

- **High intensity: one big run.** Many threads on one machine: the site's `cpu_threads` and the run's `--threads`,
  with the solver's own guarantee that the result does not depend on the thread count for one build
  (`docs/run-control.md:56`). One GPU when the site has a device and the case sets `linear_solver.device`. This is
  the first deliverable value of T1.
- **Sharded: many runs.** Studies, operating maps and reruns already make independent runner runs (Context). With
  `site`, each goes to the site and the site's `max_runs` sets the width; a study's `workers` should match it. A chain
  (`continue_from`) stays on one site: the restart file is an input, uploaded once if the parent ran elsewhere.
  Adoption (ADR 0201) compares the solver version, so a site with another engine version does not adopt local runs.
  Several sites at once for one study (a pool) is P6.
- **Distributed: one run across nodes.** Not possible today, and out of scope here: it is F-100, a solver project. It
  needs a socket or MPI `Communicator` for `rcfd-dist`; distributed assembly of the momentum, pressure, energy and
  turbulence equations and of the boundary conditions (ADR 0058 did one scalar convection-diffusion problem only);
  partitioned mesh input and a field store written per rank or gathered; per-rank checkpoints; a launcher (`--hostfile`
  is rejected today); and a scaling measurement against F-100's acceptance. When it exists, `settings.ranks` and T2's
  multi-node allocation are where this design meets it; nothing here blocks it.

### P6. How remote runs show

- `run.json` gains `site` (the site's name) for a remote run only; a local run's `run.json` is byte for byte as now.
  `solver_version` is the site's engine version, taken from `hello` at submit.
- The launch record is the site runner's own, copied as is (its `argv` and `cwd` are the site's paths, its `solver`
  the site's build id). `site.json` adds `host` (the name the site reports), `os`, `arch`, `cores` and the agent's
  `protocol` and version, so ADR 0734's provenance answers "where, with which build". The reproduce command of F-828
  for a remote run is `rcfd-runner rerun <id> --site <name>`, and the site's argv is shown beside it.
- The Runs list: a site badge on remote runs (`lab`), the link state (connected, lost since 14:02, results
  downloading 40 %), and the same pause, resume and cancel. A request is a local file as today (`pause`, `cancel`); the
  local runner forwards it and the run shows "sent to lab" until the mirrored `pause.ack` or state change confirms it.
- MCP `run_status` and `list_runs` gain `site` (additive); CLI `rcfd-runner list` and `status` print it.

### P7. Failure modes

| what | what happens |
|---|---|
| the link is lost | the site's runner and solvers go on (G7 holds across the link). The local run keeps its last mirrored state, `site.jsonl` records `lost`, the Runs list says so. The local runner reconnects with backoff (100 ms doubling to 5 s, then every 30 s) and resumes every copy from its offsets. No state is invented locally |
| the local runner or Studio closes (branch runner-lifetime) | remote runs go on; the next local runner reads `site.json` of every unfinished remote run and catches up |
| the site restarts | its runner, on restart, adopts or fails runs as it does locally (ADR 0123); the mirror copies the result. A run lost with the machine is `failed` with the site's reason |
| the site runner dies with the SSH session | on Unix the runner starts detached in its own process group but not a new session (`crates/rcfd-runner/src/sys.rs:123-126`); a login manager that kills a user's processes at logout would stop it. Test R6 checks this on the reference Linux; the fix, if needed, is a new session for the detached runner on Unix or a scheduler adapter (T2) |
| an upload stops half way | the blob stays `tmp-*` and resumes from its length; nothing is submitted until every input is checked |
| a download stops half way | resumes from the local length; each `store.rts` frame is checked; a run whose fields are not all here says so and the viewport refuses the missing snapshots with the reason |
| version skew | `hello` returns `{protocol: "rcfd-site/1", engine: {version, git_rev, exe_hash}, os, arch}`. Another major protocol is refused with a located message. Another engine version is shown and needs the user's yes per site (Q10); it is recorded per run |
| clocks differ | times are as the site wrote them; `site.json` records the clock offset measured at `hello`; the feed orders by its own reading order, as now |
| the site's disk or quota is full | the agent's error names the folder and the bytes needed; the run fails before launch |
| a site removed from the file | its runs keep their name and data; the Runs list says "site lab is not configured"; nothing is sent |

### P8. Later: a remote session service (T4)

For live sessions on big hardware and remote co-simulation, the agent can open `rcfd serve --stdio --root <mirror>`
on the site and carry it over the same SSH session; the `data.*` bytes go as tagged frames on the agent's stream
instead of a second port, which also removes the unauthenticated ticket port from the network question. Not in the
first phases.

### P9. Defaults stay bit-identical

- No site named anywhere: no new process starts, no file is written, the sites file is not read except to list it;
  `Runner::launch` and every local code path are unchanged. A local run's `run.json`, `launch.json`, `status.json`,
  `events.jsonl` and the feed are byte for byte as now (test S8).
- The solver is not touched; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.
- Engine interfaces change only by addition (G4): the `site` members are optional and absent for local runs; the new
  CLI commands, the agent subcommand and the tool `list_sites` are new names. The MCP tool list gains one tool and one
  optional parameter on five tools; those goldens are restated, not re-recorded, in the same commit.
- The Studio's attach and spawn rules (`config.ts`) are unchanged.

### P10. Phasing

| phase | delivers | proves |
|---|---|---|
| 1 (first useful slice) | `sites.toml`, `rcfd site list/check`; the agent `rcfd-runner site --stdio`; the `Site` seam and the SSH site; `rcfd check --inputs`; content-addressed upload; `start --site`, `rerun --site`; mirrored status, events, residuals, log; pause, resume, cancel forwarded; download of outputs and fields at the end; `run.json` `site`, `site.json`, `site.jsonl`; MCP `list_sites` and `start_run {site}`; the Studio's Where choice, badge and link state | one big run on a workstation or login node from the Studio, the CLI and Claude Code, viewed as a local run afterwards |
| 2 | studies and operating maps with `site`; resumable large downloads with progress; fields on first open; `rcfd site gc` | sharded work on one site |
| 3 | Slurm adapter (T2), then PBS: one job per run or a pilot job; job id and scheduler state in `site.jsonl` | clusters |
| 4 | a pool of sites for one study | several machines at once |
| 5 | T4, the session service over the agent | live remote sessions |
| 6 | T3 cloud batch and A3 tokens, after an ADR on accounts and storage | the cloud |
| (F-100) | distributed solver, separate | one run on many nodes |

### P11. Security

**Threat model.** What is protected: the project data (often proprietary geometry and results), the user's machine,
the user's account on the site. Who could attack: an observer of the network (SSH encrypts and authenticates); another
user of a shared site (the work root is created 0700 and objects 0600; A1 opens no port they could reach, which a
loopback forward on a shared login node would); a compromised site (it can read what it was sent and return wrong
results; it cannot reach the user's machine, because the local side only writes the files it asked for into
`runs/<id>/` under names it checks, and never runs what the site returns); a hostile project or case (it can name a
site only by name, which must be in the user's file; a host, a path or an `engine` from a project is never used); a
hostile site entry (a host beginning with `-` is refused; `ssh` gets an argument list, the remote command is the fixed
agent call with quoted paths).

**Secrets.** For SSH none are stored: OpenSSH and its agent hold them. A later token site keeps its token in the OS
keychain (Electron's `safeStorage` in the Studio; the platform store for the CLI), never in the sites file or a
project; the file holds a reference only.

**Nothing leaves unasked.** Data goes to a site only for a run the user started there (by choosing the site in the
run dialog, `--site`, or a session default they set with `--site` / `RCFD_STUDIO_SITE`). There is no background sync,
no prefetch to a site, no telemetry. The first run of a project on a site lists the files and bytes that will go and
asks once (G22: implicit by default, explicit on exception). MCP clients may name only sites marked `assistant = true`
(Q6).

**The engine's own listener (Q9).** `rcfd serve --addr` with a non-loopback address listens unauthenticated today.
Recommended: refuse it unless `--allow-unauthenticated-remote` is given, with a message naming SSH as the way; the
default address is loopback, so the default does not change.

## Alternatives

- **The project on the site, the Studio a thin client** (`ssh host rcfd-mcp --project <remote folder>` over stdio,
  which the project server's stdio transport would allow almost as is). Rejected as the first step: Claude Code, the
  editor and the CLI on the user's machine would no longer see the folder (G11, G12), the viewport's local reader (R2)
  would not work, and every list would wait on the network. It stays possible later for users whose data must not
  leave the cluster (Q1 (b)).
- **Two-way sync of the whole project.** Conflicts between two writers of `cases/` and the journal; the folder
  contract has no merge rule. Rejected.
- **A cloud service first (T3, A3).** Needs accounts, storage, images, billing and a hosting decision before anything
  is useful; SSH reaches workstations and clusters the user already has.
- **MPI now.** The solver cannot use it (Context); F-100 is the place.

## Open questions for the maintainer

Each question stands alone; the first option is the recommendation.

1. **Where a project lives when its runs go to another machine.** Today a project is one folder on the user's machine,
   and the Studio, Claude Code and the CLI all read that folder. (a) The project stays local and is the truth; the site
   keeps only a mirror of the runs sent to it, and their status, residuals and results are copied back into the local
   `runs/<id>/`, so everything reads one folder as now. (b) The project lives on the remote machine and the Studio
   becomes a thin client over SSH; nothing is copied back unless asked, which suits data that must not leave a
   cluster, but Claude Code and the editor on the user's machine no longer see the files. (c) Two-way sync of the whole
   project; needs a merge rule the folder contract does not have. Recommendation: (a).

2. **How the engine reaches a remote machine first.** (a) SSH running a small agent (`rcfd-runner site --stdio`) on the
   remote machine: no port is opened anywhere, the user's existing SSH keys and config are used, rcfd stores no secret.
   (b) An SSH tunnel to a port the remote service opens on its loopback; two ports to forward, and other users of a
   shared login node can reach that port. (c) A TLS listener with tokens; needs certificates and token storage, suits a
   hosted multi-user service. (d) Mutual TLS; strongest, heaviest for users. Recommendation: (a), with (c) only later
   for a hosted cloud service.

3. **The wording of guardrail G10.** G10 says "The service listens on loopback only unless the user says otherwise."
   (a) Keep that sentence and add: "The Studio reaches another machine only through a run site the user named in their
   sites file and confirmed once (host key fingerprint and engine version shown); project data goes there only for a
   run the user started there." (b) Also let `--service` attach to a non-loopback host over TLS. (c) No change; remote
   runs only from the CLI. Recommendation: (a).

4. **What is copied back from a remote run, and when.** Residuals, monitors and status are copied live in every option.
   (a) At the end of the run, everything: outputs and the field store, so the field view works as for a local run.
   (b) Only small files at the end; the field store on first open in the field view. (c) A per-site size threshold
   choosing between (a) and (b). Recommendation: (a) now, with (b) as a setting in phase 2.

5. **The hash that names uploaded files.** The project uses FNV-64 (`fnv64:`) everywhere; it catches accidents, not a
   deliberate forgery. (a) FNV-64 plus the byte length as the object name, checked on arrival, for sites used by one
   user (phase 1 to 5). (b) Add SHA-256 to the object name now, so a shared site is safe later. (c) SHA-256 for
   uploads only, FNV-64 everywhere else. Recommendation: (a), revisited before a shared cloud site (phase 6).

6. **Whether Claude Code (or any MCP client) may start runs on a remote site.** Starting a run is already an `execute`
   tool that needs the user's approval in Claude Code. (a) Only on sites the user marked `assistant = true` in the sites
   file (default false). (b) On any configured site. (c) Never; remote runs only from the Studio and the CLI.
   Recommendation: (a).

7. **The first batch scheduler adapter (phase 3).** (a) Slurm, the most common on university and lab clusters. (b) PBS
   or OpenPBS. (c) A cloud batch service first. Recommendation: (a).

8. **Where sites are defined.** (a) One user-level file (`%APPDATA%\rcfd\sites.toml`, `~/.config/rcfd/sites.toml`);
   the Studio remembers the last site per project in its own state, not in the project. (b) The user file, and a
   project may name a default site by name in `rcfd-project.json` (never a host). (c) Full site entries in the project.
   Recommendation: (a).

9. **`rcfd serve` on a network address.** Today `rcfd serve --addr 0.0.0.0:7878` listens on every interface with no
   authentication; only the Studio refuses non-loopback addresses. (a) The engine refuses a non-loopback `--addr`
   unless `--allow-unauthenticated-remote` is given, and the message points to SSH; the default address is loopback,
   so defaults do not change. (b) A warning on standard error only. (c) Leave it. Recommendation: (a).

10. **Engine version differences between the user's machine and a site.** Results are bit-identical across thread
    counts for one build, not across builds or platforms. (a) Refuse another major site protocol; allow another engine
    version after the user's yes per site, recorded in every run's `site.json` and `solver_version`. (b) Require the
    same `git_rev` on both sides. (c) Allow anything with a warning. Recommendation: (a).

## Answers (maintainer, 2026-11-17)

All ten with option (a):

1. The project stays local; the site holds a mirror.
2. SSH, running the stdio agent.
3. G10 keeps its sentence and gains the one in (a), added in `docs/studio-guardrails.md` with this date.
4. Everything is copied back at the end of a run; a per-site setting may come later.
5. fnv64 and the length name an object; it is verified on arrival.
6. Claude Code and other MCP clients may name only a site with `assistant = true` (default false).
7. Slurm is the first scheduler adapter (phase 3).
8. A user-level `sites.toml`; the Studio remembers the last site per project.
9. `rcfd serve` refuses a non-loopback `--addr` unless `--allow-unauthenticated-remote` is given.
10. Another protocol major is refused; another engine version is allowed after the user's yes for that site and
    version, recorded.

## Tests that would prove it

The fake remote for CI: a site of `kind = "exec"` (tests only, refused outside a test build) whose command is the
agent itself, `rcfd-runner site --stdio --work <temp dir>`, spawned directly instead of through `ssh`; a second
folder on the same machine is the remote. One ignored test does the same through `ssh localhost` when
`RCFD_TEST_SSH=1`. Registered before any code:

| id | phase | what | pass |
|---|---|---|---|
| S1 | 1 | a generator case and a mesh-file case run on the fake site and locally, same build | `solution.vtu`, `residuals.jsonl` and `metrics.json` byte-identical; `store.rts` snapshots value-identical |
| S2 | 1 | a second run on the same mesh | 0 mesh bytes uploaded (agent's byte counter) |
| S3 | 1 | upload cut at half (agent killed), then again | resumes from the length; the object's hash and length match; a corrupted piece is refused and nothing is submitted |
| S4 | 1 | the agent killed mid-run, restarted 5 s later | the solver ran on; the local `events.jsonl`, `residuals.jsonl` and `log.txt` equal the site's byte for byte, no line twice; `site.jsonl` has `lost` then `connected` |
| S5 | 1 | pause, resume, cancel of a remote run | the site's run reaches the state within 3 `poll_ms`; the local status follows |
| S6 | 1 | a fake agent answering `rcfd-site/2`; one answering another `git_rev` | the first refused with a located message; the second needs the yes, then `site.json` records it |
| S7 | 1 | the site's runner killed and started again during a run | the run is adopted and finishes; the mirror copies it |
| S8 | 1 | a local run before and after the change, no site named | `run.json`, `launch.json` (paths and times masked), `status.json`, `events.jsonl` and the feed byte-identical; the agent is never spawned; `baseline_hashes` pass |
| S9 | 1 | a project naming an unknown site; a host `-oProxyCommand=x`; a site with `assistant = false` from MCP | each refused before any process starts |
| S10 | 1 | `rcfd check --inputs` on every shipped case | each listed file exists and its hash is `filehash`'s; a case with a `.wasm` user function lists it |
| S11 | 1 | Studio e2e with the fake site | the badge, the link state, cancel across the link, the field view of the downloaded store |
| S12 | 1 | the work root and objects on Unix | modes 0700 and 0600 |
| R6 | 1 | the reference Linux (docker `linux-test`): agent over `ssh localhost`, the SSH session closed during a run | the site's runner and solver are still alive and the run finishes (else the detach is fixed before phase 1 closes) |
| S13 | 2 | a study, `workers = 4`, on a fake site with `max_runs = 4` | 4 runs at once; the ledger equal to the local study's |
| S14 | 2 | a 2 GB store downloaded with the link cut twice | complete; every frame checked |
| S15 | 3 | a fake `sbatch`/`squeue`/`scancel` on the fake site's path | job ids recorded; cancel reaches `scancel`; a job the scheduler kills is `failed` with its reason |

## Unblocks

F-848's phases; T2 and T3 for the cluster and cloud uses of the maintainer's direction; F-100 later meets it through
`settings.ranks` and T2.

## Built

### Q9 (commit ebb57300)

`rcfd serve` refuses an `--addr` that resolves to a non-loopback address unless `--allow-unauthenticated-remote` is
given. The refusal names SSH and the flag and exits 2. The rule is in `docs/service-protocol.md`, and
`crates/rcfd-cli/tests/serve_loopback_adr0739.rs` tests it with 4 tests. No test binds `0.0.0.0`.

### Phase 1, the engine

- **The `Site` seam** (`crates/rcfd-runner/src/site/mod.rs`): `hello`, `missing`, `put`, `submit`, `read`, `files`,
  `request`, `list` and `ensure`. The protocol is `rcfd-site/1`: JSON-RPC 2.0, one object per line, on the agent's
  standard streams. Local runs never use the seam. In `runner.rs`, `scan` hands a run whose `run.json` has `site`
  to `site::remote::RemoteRuns`, which keeps one worker thread per site. `shutdown` stops it. No other line of the
  local path changed.
- **Sites** (`site/sites.rs`): a user-level `sites.toml` (`RCFD_SITES_FILE`, else `%APPDATA%\rcfd\sites.toml` or
  `~/.config/rcfd/sites.toml`) with `[site.<name>]` tables. The fields are `kind` (`ssh`, or `exec` for tests),
  `host`, `engine`, `work`, `max_runs`, `cpu_threads`, `mem_budget_mb`, `assistant` (default false), `confirmed` and
  `accept_engines`. Unknown keys are refused, and every message names the file and the table. A host that starts
  with `-` is refused, and so are quotes in `engine` or `work`.
- **The link** (`site/link.rs`): ssh gets an argument list (`-o BatchMode=yes -o ServerAliveInterval=15 -T -e none --
  <host> <engine>/rcfd-runner site --stdio --work <work>`) and never a local shell. OpenSSH checks the host key, and
  rcfd stores no key and no secret. The agent's output is read on a thread, and its exit is seen by `try_wait`:
  on Windows, the runner that the agent starts inherits its handles (ADR 0146).
- **The agent** (`site/agent.rs`, `rcfd-runner site --stdio --work <dir>`) does the following:
  - It creates the work root with mode 0700, and objects with mode 0600.
  - It stores objects by content as `<fnv64>-<bytes>` (Q5). An upload resumes from the length of the partial
    object already on the site, and the object is verified before it is renamed into place. Every received piece is
    appended to `received.jsonl`.
  - It mirrors a project as `<work>/projects/<name>-<hash>`, with the budgets written to the mirror's runner
    config. Inputs are hard-linked or copied from the objects into the run's place.
  - It creates the run folder through a temporary folder and a rename, then starts or reuses the site's own runner.
- **What is sent** is `rcfd check <case> --inputs --json` (P4): every file the case reads, with its `filehash` hash
  and size. The list comes from `crates/rcfd-api/src/inputs.rs`. The runner runs that command rather than linking
  rcfd-api, because rcfd-api depends on rcfd-runner.
- **What comes back**:
  - `events.jsonl`, `residuals.jsonl` and `log.txt` are copied live by byte offset. The jsonl files are copied only
    up to their last newline.
  - The status is copied live.
  - When the site's run ends, `output/` and `fields/` are downloaded. The downloads go to `.part` files, resume, and
    are checked against their hash. The final `status.json` is written last (Q4).
  - When the link is lost, it is retried after 100 ms, with the wait doubling to 5 s, and then every 30 s.
- **Pause, resume and cancel** are written locally as usual, forwarded to the site, and cleared when the site's run
  ends.
- **The records**: `run.json` gains an optional `site`, which a local run never has. `site.json` records where the
  run went: the site, the host, the remote folder, the engine, and whether that engine was accepted. `site.jsonl`
  logs one line per event, with these kinds: `connected`, `lost`, `uploaded`, `submitted`, `request_sent`,
  `downloaded`, `refused`, `error`, `unconfigured` and `diverged`.
- **Versions (Q10)**: another protocol major is refused with a located message. A site engine whose
  `version+git_rev` differs from this machine's is refused until `rcfd site check <name> --confirm` records it in
  `accept_engines`. `site.json` records it for every run.
- **Confirmation**: `rcfd site check <name> --confirm` (or `rcfd-runner site check`) starts the agent once. It shows
  the host key fingerprints that OpenSSH holds (from `ssh -G`, then `ssh-keygen -l -F`) and the engine beside this
  machine's. It writes `confirmed = "<fingerprints>"` into the file, keeping the file's comments, and replaces the
  file atomically. A site without `confirmed` takes no run.
- **The CLI**:
  - `rcfd-runner start ... --site <name>` and `rcfd-runner rerun <id> --site <name>` start remote runs.
  - `rcfd site list [--json]` and `rcfd site check <name> [--confirm]` read and confirm sites.
  - `rcfd check <case> --inputs [--json] [--case-dir <dir>]` lists a case's inputs.
  - `rcfd site gc` says that it comes in phase 2.
- **MCP (Q6)**: `list_sites` is a read tool that comes right after `refine_case_mesh`. `start_run` gains `site`.
  A site without `assistant = true`, an unknown site, or an unconfirmed one is refused (1002) before a case is made,
  a file is written, or an agent starts. `list_sites` is not on the first Claude allow-list, because no answer adds
  it. The tool-list tests are restated, not re-recorded: `project_list_f270.rs`, `tool_list_f275.rs`,
  `project_w48f.rs` and `project_runs_w48f.rs`. The Studio's capability inventory is regenerated, with F-848
  exemptions for the site commands.

Decisions taken while building:

- `exec` sites, and the agent's test overrides of its protocol and `git_rev`, work only when
  `RCFD_TEST_EXEC_SITES=1` is set. The ADR's "refused outside a test build" could not be a cargo feature, because
  the integration tests run the release binaries.
- `start_run {site}` always requires `assistant = true`, whoever calls it. The Studio talks to the same MCP server,
  so its remote runs are to go through `rcfd-runner start --site`, which is a direct user action.

### Tests

| file | count | covers |
|---|---|---|
| `crates/rcfd-runner/tests/sites_adr0739.rs` | 8 (9 on Unix) | S1, S2, S3, S4, S5, S6, S7, S8, S9, S12 (Unix) and an ssh localhost run with R6, skipped unless `ssh -o BatchMode=yes localhost` answers |
| `crates/rcfd-cli/tests/check_inputs_adr0739.rs` | 3 | S10: the 11 shipped cases, a table, a `.wasm` user function and a mesh file, and a missing input (exit 3) |
| `crates/rcfd-mcp/tests/sites_adr0739.rs` | 1 | S9 from MCP (`assistant = false`, unknown site, `new_case` with a closed site), and a finished run on an open site |
| `crates/rcfd-cli/tests/serve_loopback_adr0739.rs` | 4 | Q9 |

S1 compares `metrics.json` without the solver's wall-clock time. S8 checks the local run's `run.json` keys, and
that there is no `site.json`, no `site.jsonl` and no agent. It does not mask and byte-compare the files; the
unchanged `baseline_hashes` covers the defaults.

### Left

- **The Studio's part of phase 1 and S11**: the Where choice at Start, remembered per project (Q8); the site badge
  and link state on remote runs; and a Sites page or sites.toml editor. Until they are built, the coverage map
  exempts the site commands under F-848.
- **R6 on the reference Linux**: there is no ssh server here (see the report of this build). The test is written
  for Unix and skips here.
- **Feed**: lines of `site.jsonl` are not yet events in the feed. A local run has no `site.jsonl`, so the feed of
  local runs is unchanged.
- **Phases 2 and 3**: S13, S14 and S15, `rcfd site gc`, studies on a site, download progress, and the Slurm adapter.
