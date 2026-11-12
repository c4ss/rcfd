# ADR 0345: F-360 registration: the co-simulation protocol over the solver service

Status: **REGISTERED 2026-11-12**, written before any code of F-360 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing was compiled or run: the code and the documents listed under "What was read" were read,
and no arithmetic was done beyond what is written out below. Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Parents: [ADR 0256](0256-f139-registration-cosimulation-participant.md) (F-139: `rcfd_fv::cosim`, the protocol, the model
problem CWs, its checks A1 and A2, question 1 of its addendum, from which this item comes; the recommendation was taken on
the maintainer's standing instruction of 2026-11-11), `docs/cosimulation-protocol.md`, `docs/service-protocol.md` (F-085, the
JSON-RPC service). Roadmap item F-360 (depends on F-139 and F-085). Number 0345 given by the lead. Branch
`f360-cosim-over-service` from main b8b98cbf, worktree `w49i-work/f360`.

House rules (as ADR 0256):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment committed before the corrected run, the old one stays as an ignored record; each registered
  run is made once;
- defaults stay bit-identical: a request that names no `cosim.*` method is answered as before, apart from two added members
  of the `service.info` reply (below); no case key; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the
  goldens (schema, CLI help, MCP tools list) are not edited; no existing test is edited;
- `crates/rcfd-fv` is not edited at all (so not `pimple.rs`, `pimple/*`, `overset.rs`, `cosim.rs`);
- no new third-party crate (`std::net`, the workspace's `serde_json`), no unsafe code, the crate graph unchanged, no commit
  trailers, no push.

Clean-room. The wire format, the lease and the reconnection rule are designed in this text from the needs of the calls of
ADR 0256 and of a line-delimited request and reply transport. JSON-RPC 2.0 is the public specification the service already
follows (F-085); nothing else was consulted: no coupling library, no vendor document, no paper.

## What F-360 asks

The acceptance text of `docs/roadmap.json`, binding:

> The calls of docs/cosimulation-protocol.md (describe, time, checkpoint, restore, forces, advance) are served by the
> JSON-RPC service under a new protocol version; an out-of-process mock structural participant driven over TCP reproduces
> ADR 0256 A1 and A2 (every step converged, omega_m/omega within 0.5 percent and 3 percent) and the same displacements as
> the in-process run to 1e-12; a dropped connection or a participant error stops the step with a located error and both
> sides restored; the protocol document is versioned; existing service tests and case hashes are unchanged.

Clauses: (c1) the six calls are served by the JSON-RPC service, under a new protocol version; (c2) an out-of-process mock
structural participant driven over TCP reproduces A1 and A2; (c3) the same displacements as the in-process run to `1e-12`;
(c4) a participant error stops the step with a located error and both sides restored; (c5) a dropped connection does the
same; (c6) the protocol document is versioned; (c7) existing service tests and case hashes are unchanged.

## What was read (no run)

1. ADR 0256 in full (registration, amendment 1, addendum) and `docs/cosimulation-protocol.md`.
2. `crates/rcfd-fv/src/cosim.rs`: the traits `Participant` (`name(&self) -> &str`, `time(&self) -> f64`, `checkpoint`,
   `restore`), `Structure` (`node_coordinates(&self)`, `advance`), `Fluid` (`point_coordinates`, `face_points`,
   `forces(&self)`, `advance`); `coupled_step`: it calls `time`, the three describing calls and `checkpoint` before the
   iteration (an error there returns without a `restore`), and after an error inside the iteration it calls `restore` on
   the fluid and then on the structure, appending "; and the restore failed: ..." to the message when one fails. `FlowWall`
   holds a `Box<dyn FnMut>` (the mesh motion), so it is not `Send`. `SprungWall` is plain data.
3. `crates/rcfd-fv/tests/cosim_f139.rs`: the set-up CWs (the channel, the smoothing, the six nodes, the settings C3), the
   crossings, the bit comparisons of R1 and the faulty structure of E1.
4. `crates/rcfd-api/src/service.rs`: `PROTOCOL = "rcfd-service/1"`; `Service::call` (a `match` on the method name, the
   `design.*` methods handed to their own module), `handle_request`, `serve_stream` (one JSON object per line),
   `serve_tcp` (one thread per connection, connections share the service), `METHODS` (61 names, returned by
   `service.info`), `Client` (a blocking line client with a read time-out of 120 s), the error codes 1001 to 1004.
5. The tests that pin the identifier: `crates/rcfd-api/tests/service_tcp.rs` line 32 and `crates/rcfd-cli/tests/cli.rs`
   lines 202 and 220 assert `protocol == "rcfd-service/1"`. No test compares the whole `service.info` reply or the whole
   `METHODS` list (`particle_run.rs` and `workflow.rs` look for single names); `rcfd-mcp` does not read `METHODS`.
6. The workspace `Cargo.toml`: `serde_json` is built with `float_roundtrip`.

## Design

### D1. Who serves what

The fluid participant of this repository cannot be built from a case (ADR 0256: no case key), and a case key would change
the schema golden. So the service does not build participants; **the embedding program hands them to it**:

    service.host_fluid("fluid", || Ok(Box::new(flow_wall)))            // a closure, run on the participant's thread
    service.host_structure("structure", || Ok(Box::new(sprung_wall)))

Each hosted participant lives on its own thread, built there by the closure (so a participant that is not `Send`, such as
`FlowWall`, can be hosted) and called there, one call at a time, in the order the requests reach it. The connection threads
of `serve_tcp` and `serve_stream`, and `Service::call` in-process, pass each `cosim.*` request to that thread and wait for
its reply. `rcfd serve` hosts none (no CLI change), so there every `cosim.*` call answers "unknown participant".

The coupler (`rcfd_fv::cosim::coupled_step`, unchanged) is driven with **proxies**: `RemoteStructure` and `RemoteFluid`
implement the traits `Structure` and `Fluid` by sending each call as one request over a TCP connection to a service that
hosts the participant. So any of the three arrangements runs with the same coupler: both in-process (ADR 0256), one
remote, both remote.

New module `crates/rcfd-api/src/cosim_service.rs` (the hosting, the methods, the proxies), one line in `lib.rs`. In
`service.rs`: one field of `Service` (the hosted participants), one arm of `Service::call` (`cosim.*`, as `design.*`),
eight names appended to `METHODS`, two members added to the `service.info` reply, two error codes. `rcfd-api` already
depends on `rcfd-fv`.

### D2. The version

The existing tests pin `service.info.protocol == "rcfd-service/1"`, and they are not edited. The new methods are an
addition: every request of version 1 is answered as before. So:

- the identifier stays `rcfd-service/1` (it names the major version: any `1.x` service answers a `1.0` client);
- the service protocol becomes **version 1.1**: `service.info` gains `"protocol_version": "1.1"` and `"cosim":
  "rcfd-cosim/1"`; `docs/service-protocol.md` is retitled "version 1.1" with a list of what 1.1 added;
- the co-simulation calls carry their own identifier **`rcfd-cosim/1`** (returned by `cosim.list` and `cosim.open`), and
  `docs/cosimulation-protocol.md` states it, with the wire format and a list of versions.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). The other reading of "a new
protocol version", the identifier `rcfd-service/2`, needs the three pinned assertions edited and is left as an open
question (it is not taken: it edits existing tests).

### D3. The methods

One JSON object per line, JSON-RPC 2.0, as every other method. `participant` is the name the participant was hosted
under; `lease` is the number `cosim.open` returned (D4).

| Method | Parameters | Result |
| --- | --- | --- |
| `cosim.list` | | `{protocol, participants: [{participant, kind, name}]}`, sorted by `participant` |
| `cosim.open` | `participant` | `{protocol, lease, kind, name, time}`; every earlier lease of this participant stops being valid |
| `cosim.describe` | `participant`, `lease` | structure: `{kind: "structure", name, nodes}`; fluid: `{kind: "fluid", name, points, faces}` |
| `cosim.time` | `participant`, `lease` | `{time}` |
| `cosim.checkpoint` | `participant`, `lease` | `{time}` |
| `cosim.restore` | `participant`, `lease` | `{time}` |
| `cosim.forces` | `participant`, `lease` (a fluid) | `{forces, time}` |
| `cosim.advance` | `participant`, `lease`, `dt`; a structure: `forces`; a fluid: `displacements` | structure: `{displacements, time}`; fluid: `{forces, time}` |

`nodes`, `points`, `forces`, `displacements` are arrays of `[x, y, z]`; `faces` is an array of arrays of point indices;
`time` and `dt` are numbers; SI units, the conventions of `docs/cosimulation-protocol.md`. `kind` is `fluid` or
`structure`; `name` is the participant's own name (`Participant::name`), the one a coupling error carries.

**Numbers.** A real number travels as a JSON number. The service writes the shortest decimal text that reads back as the
same binary64 value and reads with a correctly rounding parser (`serde_json` with `float_roundtrip`, which the workspace
sets), so a finite value crosses the wire bit for bit, the sign of zero included. From this: the remote run must give the
*same bits* as the in-process run, not merely `1e-12` (check D below; the estimate of the difference is 0). JSON has no
text for a non-finite number, so a participant that returns one is answered with the error 1005 naming the array; the
coupler would have refused it anyway (ADR 0256, protocol 7). A participant written with another JSON library keeps this
property only if its library round-trips binary64 (stated in the document, not tested here).

**Errors.** `-32602`: a missing or malformed parameter (no `participant`, no `lease`, `dt` not a number, a vector that is
not three numbers). **1006** (new): no participant of that name. `1002`: the lease is not the current one, or the call
does not belong to the participant's kind (`cosim.forces` to a structure). **1005** (new): the participant returned an
error; `error.message` is the participant's message, unchanged. `-32000`: the participant's thread has stopped.

After a failed `advance` the participant may be in any state (the protocol); the service does not restore it on its own.
The coupler does.

### D4. The lease, and what a dropped connection means

The participant's state (and its checkpoint) lives in the service, not in the connection, so a connection can be dropped
and a new one opened without losing either. What must not happen is a request of the old connection, still in a buffer,
being applied after the coupler has restored the participant over the new connection. Hence the lease: `cosim.open`
returns a number that grows by one with every `open` of that participant; every other call carries it and is refused
(1002) unless it is the current one. The lease is compared on the participant's thread, where the calls are applied in
order, so a call is either applied before the `open` that supersedes it or refused. It also keeps a second coupler from
driving a participant unnoticed.

The proxy (`RemoteStructure`, `RemoteFluid`):

- `connect(address, participant, settings)`: connects (with `connect_timeout`, default 5 s), `cosim.open`,
  `cosim.describe`; keeps the name, the interface mesh and the time. `name`, `time`, `node_coordinates`,
  `point_coordinates` and `face_points` answer from what it keeps (the traits give them no way to fail); the time is
  refreshed from the `time` member of every reply. `query_time()` sends `cosim.time`.
- every call is one request line and one reply line, read with `call_timeout` (default 120 s). An input-output error, an
  end of stream, a time-out or a reply that is not the answer to the request marks the connection as **lost** and closes
  it; the call returns an error text that names the method and the address ("the connection to 127.0.0.1:port was lost
  during cosim.advance: ..."). An error reply returns the participant's message and leaves the connection in use.
- on a lost connection every call but `restore` fails at once ("not connected"); **`restore` reconnects once** (connect,
  `cosim.open` for a new lease, then `cosim.restore`) and fails if that fails. `reconnect()` does the first two for a
  caller whose step failed before the iteration (where the coupler sends no `restore`).

Why this gives clause c5. `coupled_step` answers a failed call inside the iteration with `restore` on both participants.
For a remote participant whose connection dropped, that `restore` reconnects and restores the participant in the service
to its checkpoint, the start of the step: whether the dropped `advance` was never received, was received and applied, or
is still being computed (the `restore` waits behind it on the participant's thread), the participant ends at its
checkpoint, and the request that was lost cannot be applied later (its lease is superseded). The other participant is
restored as in ADR 0256. The step's error names the participant, the time `t_n`, the iteration, and the lost connection.
If the participant's process is gone, nothing can restore it: the reconnection fails, the error says so (the "restore
failed" suffix of `coupled_step`), and the participant that is still there is restored.

A call that outlasts `call_timeout` is treated as a lost connection (the reply may still arrive, so the connection cannot
be used again); the `restore` that follows waits behind the unfinished call for at most `call_timeout`. Not tested.

## Set-up

**CWs**: the model problem of ADR 0256 with amendment 1, copied without change from `cosim_f139.rs` (that file is not
edited): `box_mesh([20, 8, 2], [1, 0.4, 0.1])`, `Symmetry` on `ymin` and `ymax`, six nodes, `mu_0 = 4`, `X = 0.002`,
`dt = T_0/100`, 650 steps, the settings C3 (Aitken from 1/3, tolerance `1e-8`, cap 20, stop at the cap).

**The reference** is the in-process run of CWs made in the same test binary (the same code, the same build), recording the
displacement of every node after every step.

**The out-of-process structure.** The test binary starts itself as a second process (`std::env::current_exe()` with the
name of an ignored helper test and an environment variable that selects the role; without the variable the helper returns
at once). The child builds a `Service`, hosts the `SprungWall` of CWs as `structure`, listens on `127.0.0.1:0`, prints its
address and its process id, and ends when its standard input closes (the parent holds it), when it is killed, or after
600 s. The parent reads the address with a time-out of 60 s and kills the child when the test ends. Roles: `structure`;
`structure-fails-2` (the faulty structure of ADR 0256 E1: an error at its second `advance`); `structure-stiff` (every
spring multiplied by `1 + 1e-9`, the negative control).

**The relay** (dropped connections with the participant's process alive). A thread in the test accepts connections on
`127.0.0.1:0` and passes request and reply lines between the proxy and a service; at the registered request it closes
both sockets, either before passing the request on (**cut-before**) or after the service has answered it, discarding the
answer (**cut-after**). Later connections pass through untouched. The hosted participant is wrapped so that the test can
read its state: a structure behind `Arc<Mutex<..>>`; for the fluid (not `Send`) the wrapper writes the state's bits to a
shared vector after every call.

Every socket of the tests is bound to `127.0.0.1` port 0; every read has a time-out; `connect_timeout` 5 s and
`call_timeout` 60 s in the tests.

## Registered checks

New file `crates/rcfd-api/tests/cosim_service_f360.rs`, release, each run once; the heavy ones are ignored in a debug
build. "Bits differ" counts values whose 64 bits differ.

### Acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| S0 (set-up) | the in-process reference is ADR 0256's run: `omega_m/omega` of `Q_0` and `Q_1`; largest iteration count; histogram of the counts 7 to 10 | within `2e-6` of 0.998618 and 1.011453; 10; 13, 210, 343, 84 | ADR 0256 addendum (the numbers were printed to six decimals); the same code |
| P1 (c1, version) | `service.info`: `protocol`; `protocol_version`; `cosim`; `METHODS`: length, the first 61 names end with `design.ledger_rerun_check`, the last eight are the methods of D3; `cosim.list` on a service with two hosted participants | `rcfd-service/1`; `1.1`; `rcfd-cosim/1`; 69; yes; yes; both listed with kind and name, `protocol` `rcfd-cosim/1` | D2, D3 |
| P2 (c1, the six calls) | over TCP against a hosted `SprungWall` and a hosted `FlowWall` of CWs, each beside the same participant called in-process: `describe` (nodes; points and faces), `time`, `checkpoint`, `forces`, `advance` (a structure with the predictor's forces; the fluid with the displacement that follows), `time` after it, `restore`, `time` after it, `advance` again: values whose bits differ | 0 of all values; the second `advance` equals the first | D3: the service calls the participant and the numbers cross bit for bit |
| P3 (c1, refusals) | `cosim.nope`; an unknown participant; a call without `lease`; with a superseded lease (and the participant's state after it); `cosim.forces` to a structure; `advance` with one force too few; `advance` without `dt`; a vector of two numbers; `restore` before any checkpoint; a participant returning NaN | codes -32601, 1006, -32602, 1002 (0 bits differ), 1002, 1005 (the participant's message), -32602, -32602, 1005, 1005 | D3, D4 |
| A1r (c2) | CWs, the structure out of process (role `structure`), the fluid in-process, 650 steps: every step converged; iterations per step, largest and mean; the largest last test quantity; the child's process id | all 650; `<= 16` and `<= 12`; `<= 1e-8`; not the parent's | the reference: 10, 8.766, `9.995e-9` |
| A2r (c2) | the same run: `omega_m/omega_0` of `Q_0`, `omega_m/omega_1` of `Q_1`; upward crossings | `abs(. - 1) <= 0.005`; `<= 0.03`; `>= 6` and `>= 11` | 0.998618, 1.011453; 6 and 12 |
| D (c3) | the same run against the reference: `max` over steps and nodes of `abs(q_remote - q_in_process)`; steps whose iteration count differs; values of the last test quantity of every step whose bits differ | `<= 1e-12 X` (`2e-15` m); 0; printed | 0 exactly: the same operations on the same bits (D3) |
| D2 (c3, both remote) | CWs, 100 steps, the fluid hosted in a service of the test process and driven over TCP, the structure out of process: the same two numbers | `<= 1e-12 X`; 0 | 0 exactly |
| F1 (c4, structure) | CWs first step, role `structure-fails-2`: the error; the fluid's state against before the call; `query_time()` of the structure; then a clean step against the first step of the reference (node displacements, iteration count) | names `structure`, time 0, iteration 2, contains "injected failure"; 0 bits differ; 0; converged, 0 bits differ, the same count | ADR 0256 E1 (a); a restored structure repeats the reference's first step |
| F2 (c4, fluid) | CWs first step, the fluid hosted (test process, over TCP) with a mesh function that fails at its second call, the structure in-process: the error; the hosted fluid's state after the step against before; the structure's | names `fluid`, time 0, iteration 2, contains the mesh function's message; 0 bits differ; 0 bits differ | ADR 0256 E1 (b) |
| F3 (c5, connection dropped, process alive) | CWs first step, the structure hosted behind the relay, (a) cut-before and (b) cut-after the second `cosim.advance`: the error; the fluid's and the hosted structure's state against before the call; reconnections of the proxy; then a clean step against the reference's first step; (c) the same for the fluid hosted behind the relay, cut-after its second `cosim.advance` (the hosted fluid's state, the in-process structure's) | names the participant (`structure`, `structure`, `fluid`), time 0, iteration 2, contains "connection"; 0 bits differ, both; 1; converged, 0 bits differ in the node displacements, the same count | D4 |
| F4 (c5, between steps) | CWs: one clean step with the structure behind the relay, then the connection is cut before the next `cosim.checkpoint`: the error of the second step; both states against after the first step; after `reconnect()` the second step against the reference's second step | names `structure`, iteration 0, contains "connection"; 0 bits differ; converged, 0 bits differ | D4: before the iteration the coupler restores nothing and nothing was advanced |
| F5 (c5, the process is gone) | CWs first step, role `structure`; the parent kills the child before the second `advance`: the error; the fluid's state; the wall-clock time of the failed step | names `structure`, time 0, iteration 2, contains "connection" and "restore failed"; 0 bits differ; `<= 30 s` | D4; the reconnection is refused or times out after 5 s |
| V (c6) | `docs/cosimulation-protocol.md` and `docs/service-protocol.md` (read by the test): the first names `rcfd-cosim/1` and has a section of versions; the second says "version 1.1"; both name every method of D3 | yes | written with the code |
| B1 (c7) | `rcfd-api`, release, unedited: the unit tests of `service`, the suites `service_tcp`, `service_base_dir_w48j`, `sv301_service_w48z`, `physics_service`, `workflow`, `particle_run`, `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes`; `rcfd-cli` suite `cli`; the library tests of `rcfd-mcp` (the tools list); `git diff main` touches only `service.rs`, `lib.rs` (one line), the new module, the new test file, this ADR and documentation | all pass; exactly those files | no solver file is touched; the service change is additive |

### Negative controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| NC1 (D can fail) | CWs, 100 steps, role `structure-stiff` (springs times `1 + 1e-9`), against the reference: `max abs(q_remote - q_in_process)` | `>= 1e-10 X` | the frequencies change by `5e-10` relative (less for mode 0, whose inertia is mostly fluid: `omega^2 = k/(m + rho L)` still scales with `k`), a phase of `omega t x 5e-10 = 3e-9` after one period, so about `3e-9 X` where the wall moves fastest |
| NC2 (the relay cuts) | F3 (a) and (b): the number of connections the relay accepted | 2 each | one cut, one reconnection |

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement that must occur once; the new test file is run once and
the named tests must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the proxy's `restore` does not reconnect on a lost connection | F3 |
| MU2 | the service does not compare the lease | P3 |
| MU3 | `cosim.restore` answers without calling the participant's `restore` | P2, A1r or D |
| MU4 | the proxy does not take the time from the reply of `advance` | A1r |
| MU5 | the service replaces the participant's error message by a fixed text | F1, P3 |
| MU6 | `cosim.open` does not issue a new lease (the number stays) | P3 |
| MU7 | the proxy's structure `advance` sends the forces in reverse node order | D |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test --release -p rcfd-api --test cosim_service_f360 -- --test-threads=1 --nocapture`, the suites of B1,
`cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings`. The whole-workspace suite is not
run.

## Predictions (mine, before any run)

D and D2 exactly 0 and no step with another count; A1r and A2r the reference's numbers; F5 two to five seconds. The check I
trust least is F5: how Windows reports a peer that was killed (a reset or an end of stream) and how long a connection to a
port nobody listens on takes to be refused. Second: S0's histogram, which assumes this build reproduces the bits of the
build of ADR 0256's record (it is a set-up check, not an acceptance check: the reference of D is whatever this build gives).

## Decision rule

- **done**: P1 to P3, A1r, A2r, D, D2, F1 to F5, V and B1 are met, S0 holds, NC1 and NC2 hold, and every mutation fails its
  tests. The acceptance stays the maintainer's.
- **partial**: the six calls are served (P1, P2) and the out-of-process run completes, and one of the other checks is
  missed (kept as `*_known_miss`, diagnosed) and not shown by an amendment to be a defective set-up; or a mutation is not
  caught.
- **todo**: the calls are not served, or the out-of-process run does not complete.

A set-up check (S0) or a negative control that does not hold means the check is not shown to be able to fail: reported as
a defective set-up, corrected by an amendment before any re-run, never counted as a pass. S0's histogram alone failing
(another build, other rounding) with the frequencies inside `2e-6` is reported and does not block: D compares against the
reference of this build.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default result, a
golden or a hash table.

- **T1. Version 1.1 with the identifier `rcfd-service/1` kept, and `rcfd-cosim/1` for the calls** (D2). Option: the
  identifier `rcfd-service/2`: left open, it edits three assertions of existing tests.
- **T2. Participants are hosted by the embedding program**, not built from a case (D1). Option: a case key and `rcfd
  serve` building the participant: a schema golden and a CLI change; left to a later item.
- **T3. The coupler stays where it is** (`coupled_step`, in the process of whoever couples), driven with proxies. Option:
  a `cosim.step` method that runs the coupled step inside the service: not in the acceptance.
- **T4. Numbers as JSON numbers** (D3). Option: the bits as text; not needed, the workspace's JSON library round-trips.
- **T5. A lease per participant and reconnection by the proxy's `restore`** (D4). Option: the service restores a
  participant when its connection closes: it cannot tell a finished step from an unfinished one.
- **T6. The out-of-process participant of the test is the test binary started a second time.** Option: a new binary or a
  CLI subcommand (`rcfd cosim-mock`): a CLI help golden.

## Limits of this registration

One coupler per participant (the lease). No authentication and no encryption: as the rest of the service, meant for the
loopback interface or a trusted network. The service does not build participants from a case; `rcfd serve` hosts none. No
`cosim.step`; no event of the event log for co-simulation calls. The structure's `describe` carries nodes only (the
elements that ADR 0263's map builders take are not part of the `Structure` trait). A call that outlasts the time-out is
treated as a lost connection and is not tested. A participant process that died is not restarted or restored. The
bit-for-bit transfer rests on a round-tripping JSON library on both sides. No real structural solver; the mock and the
model problem of ADR 0256, with its limits.
