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

## Amendment 1 (2026-11-12, after run 1 of the registered checks and before any other run)

Run 1 (commit 6e8561fb, every registered check once) is reported in the addendum. It showed something no registered check
covers: **the transport is slow**. The out-of-process run of 650 steps took 147.3 s; it makes about 11 400 requests (per
step one `checkpoint` and, per iteration, one `advance` and from the second on one `restore`: `650 (2 x 8.77 - 1) + 650`),
and the in-process flow of ADR 0256's whole test file took 31 s. So a request costs several milliseconds, where two short
lines over the loopback interface should cost well under a tenth of one. No limit was missed (none was set), and no result
depends on it; but a test of more than two minutes would have to be an ignored record (rule 7), and for a user the cost per
call matters.

Cause, not yet measured (a hypothesis from reading the code): `Service::serve_stream` writes a reply with
`writeln!(writer, "{r}")` and the proxy writes a request the same way; the formatter then hands the JSON text to the
unbuffered socket in many small pieces, each its own `send`, and on the service's accepted sockets Nagle's algorithm holds
the later pieces back until the first is acknowledged.

**New check T1 (registered now).** A `SprungWall` hosted in a service of the test process, a `RemoteStructure` connected
to it over TCP on the loopback interface: 2000 calls of `query_time()` (`cosim.time`); the mean wall-clock time per call.
Limit `<= 1 ms`. Estimate: 0.03 to 0.1 ms (two system calls and a thread wake-up on each side). T1 is first run once on
the code of run 1, where it is expected to miss (the control that T1 sees the defect; if it does not miss there, the
hypothesis is wrong and the cause is looked for elsewhere before anything is changed).

**The change** (after that control run): (i) the proxy formats each request into one buffer and writes it with one call;
(ii) `Service::serve_stream` does the same for each reply; (iii) `Service::serve_tcp` sets `TCP_NODELAY` on the accepted
connections. None of the three changes a byte on the wire, so no registered result can change; (ii) and (iii) are in the
path of every method of the service, which is why the suites of B1 are run after the change, not before.

**Runs after the change**: T1 once; the whole new test file once more (run 2), as the confirmation that the changed code
still meets every check. Run 1 stays the registered run of S0 to V and NC1, NC2; run 2 is reported beside it, whatever it
gives. The reference run prints its wall-clock time from now on (a print). If the out-of-process run of 650 steps still
takes more than about two minutes, `a1r`, `a2r` and `d` become ignored heavy records and D2 (100 steps, both participants
remote) with F1 stays as the fast test of the same path. Mutations and B1 are made on the changed code. The decision
rule gains: T1 met, for **done**.

## Amendment 2 (2026-11-12, after run 2 and a timing diagnosis, before any further run)

T1's control on the code of run 1 missed as expected (1.99 ms per call; it was started twice because the first output was
cut off by the filter of the command, about 2.1 ms from the total time), the change of amendment 1 was made (commit
828566d1), and T1 then gave 0.048 ms. Run 2 met every check with the same numbers as run 1. But the out-of-process run
still took 154.3 s, and the in-process reference, timed for the first time, 66.8 s.

**Diagnosis** (a scratch test, not committed: 60 steps of CWs, timed, in the order in-process, structure in a child
process, structure in a thread of the test process behind TCP, in-process; twice): 17.59, 15.53, 16.76, 17.83 s and
17.34, 14.39, 19.46, 29.28 s. So after the change a remote structure costs nothing measurable (60 steps make about 1050
requests, 0.05 s at T1's rate), and the 0.29 s per step is the flow itself on a machine that other builds share: ADR
0256's whole file, three such runs and more, took 31 s. The 154 s against 66.8 s of run 2 is the load at the two moments,
not the transport.

By the rule written in amendment 1 the 650-step out-of-process run took more than two minutes, so:

- `a1r`, `a2r`, `d` and `s0` (which needs the 650-step reference) become **ignored heavy records**, their limits and code
  unchanged. They were run in run 1 and in run 2 (the library is not changed after run 2) and are not run again, except
  where a mutation names them.
- the fast tests of the same path: D2 (100 steps, both participants remote, against the reference bit for bit), NC1, F1
  to F5. So that these do not need the 650-step reference, the test file gets a second reference of 100 steps (the same
  code; the first 100 steps of the long one, as D2 of runs 1 and 2 showed: 0 values differ), and D2, NC1 and the "next
  step" comparisons of F1, F3 and F4 read that one. No limit changes.
- **Run 3**: the file without the ignored records, once, as the confirmation of the changed test file.
- Mutations: each is run on the file without the ignored records; MU3, MU4 and MU7, which name A1r or D, also with the
  ignored records.

This is a rule about test duration applied as written; whether the three acceptance tests should run in the gate on a
machine where they take some ten seconds is left to the lead (addendum, questions).

## Addendum 1: results (2026-11-12)

### What was built

- `crates/rcfd-api/src/cosim_service.rs` (new): the hosting (`Service::host_fluid`, `Service::host_structure`,
  `Service::unhost`; one thread per hosted participant, the lease kept on that thread), the eight methods `cosim.list`,
  `cosim.open`, `cosim.describe`, `cosim.time`, `cosim.checkpoint`, `cosim.restore`, `cosim.forces`, `cosim.advance`, and
  the proxies `RemoteStructure` and `RemoteFluid` with `RemoteSettings`. `lib.rs` gains `pub mod cosim_service;`.
- `crates/rcfd-api/src/service.rs` (lines of the final file): 3-4 (module text), 30-33 (`PROTOCOL_VERSION = "1.1"`), 52-55
  (codes 1005, 1006), 266-267 and 316 (the field of `Service`), 404-405 (`protocol_version` and `cosim` in `service.info`),
  1071 (the `cosim.*` arm of `Service::call`), 1761-1764 (`serve_stream` writes a reply with one call), 1783-1784
  (`serve_tcp` sets `TCP_NODELAY`), 1903-1911 (eight names appended to `METHODS`).
- `crates/rcfd-api/tests/cosim_service_f360.rs` (new): 18 tests (13 run by default in release, 4 ignored heavy records,
  1 ignored helper that is the participant process).
- `docs/cosimulation-protocol.md` (the section "Over the solver service", the version line and the section "Versions")
  and `docs/service-protocol.md` (version 1.1, the codes, the methods).
- Not edited: anything in `rcfd-fv`, the case schema, the CLI, `rcfd-mcp`, any existing test, any hash table or golden.
  No new crate; `Cargo.lock` unchanged; no unsafe code.

### Checks

Run 1: commit 6e8561fb, `cargo test --release -p rcfd-api --test cosim_service_f360 -- --test-threads=1 --nocapture`, every
registered check once, 16 passed, 280 s. Run 2: commit 828566d1 (after the change of amendment 1), the same command, 17
passed, 255 s, every number below identical to run 1 except the times and the ports. Run 3: commit 62ea46e0 (the test
file of amendment 2), 13 passed and 5 ignored, 90 s, the numbers of the tests it runs identical again.

| Check | Limit | Measured (run 1) | Verdict |
| --- | --- | --- | --- |
| S0 (set-up) the reference is the run of ADR 0256 | within `2e-6` of 0.998618 and 1.011453; 10; 13, 210, 343, 84 | `0.998618428`, `1.011452964`; 10; 13, 210, 343, 84 | held |
| P1 (c1) version and methods | `rcfd-service/1`; `1.1`; `rcfd-cosim/1`; 69 methods, the first 61 unchanged, the last eight of D3; `cosim.list` | as registered; both participants listed with kind and name | met |
| P2 (c1) the six calls over TCP against the in-process participant, a structure and a fluid | 0 bits differ; the second `advance` equals the first | 0 of 291 values | met |
| P3 (c1) refusals | -32601, 1006, -32602, 1002 (0 bits), 1002, 1005 (the participant's message), -32602, -32602, 1005, 1005 | the same codes; "sprung wall: 5 forces for 6 nodes" and "sprung wall: restore without a checkpoint" unchanged; 0 bits of the structure differ after the `advance` of the superseded lease | met |
| A1r (c2) the structure in another process, 650 steps | all converged; `<= 16`, `<= 12`; `<= 1e-8`; another process | all 650; 10 and `8.766` (7: 13, 8: 210, 9: 343, 10: 84); `9.995e-9`; child 112584, parent 65640 | met |
| A2r (c2) frequencies | `abs(. - 1) <= 0.005`; `<= 0.03`; `>= 6`, `>= 11` | `0.998618`; `1.011453`; 6 and 12 | met |
| D (c3) displacements against the in-process run, 650 steps, six nodes | `<= 1e-12 X`; 0 steps with another count | exactly 0; 0; also 0 of the 650 last test quantities differ in their bits | met |
| D2 (c3) both participants remote, 100 steps | `<= 1e-12 X`; 0 | exactly 0; 0 | met |
| F1 (c4) an error of the out-of-process structure | `structure`, time 0, iteration 2, "injected failure"; 0 bits; 0; the next step is that of the reference | as registered; 0 bits of the fluid differ; the time of the structure 0; the next step converged in 9 iterations with 0 bits differing | met |
| F2 (c4) an error of the hosted fluid | `fluid`, time 0, iteration 2, the message; 0; 0 | as registered ("injected failure of the mesh motion"); 0 and 0 bits | met |
| F3 (c5) connection dropped in the step, process alive: (a) cut before, (b) cut after, (c) the fluid, cut after | the participant, time 0, iteration 2, "connection"; 0 bits both; 1 reconnection; the next step is that of the reference | all three: "the connection to 127.0.0.1:port was lost during cosim.advance: the service closed the connection"; 0 and 0 bits; 1 reconnection (lease 2); the next step 9 iterations, 0 bits differ | met |
| F4 (c5) connection dropped between two steps | `structure`, iteration 0, "connection"; 0 bits; after `reconnect()` the second step is that of the reference | "... was lost during cosim.checkpoint ..." at `t = 0.01`, iteration 0; 0 and 0 bits; the second step 9 iterations, 0 bits differ | met |
| F5 (c5) the process of the participant killed | `structure`, time 0, iteration 2, "connection" and "restore failed"; 0 bits; `<= 30 s` | "... forcibly closed by the remote host (os error 10054); and the restore failed: could not connect to ...: ... actively refused it (os error 10061)"; 0 bits of the fluid differ; `2.09 s` | met |
| V (c6) the documents | version and methods named | yes | met |
| NC1 (control of D) springs times `1 + 1e-9` | `>= 1e-10 X` | `4.15e-9 X` (`8.3e-12` m) | held |
| NC2 (control of F3) connections the relay accepted | 2 each | 2 and 2 | held |
| T1 (amendment 1) 2000 `cosim.time` calls, time per call | `<= 1 ms` | on the code of run 1: `1.99 ms` (the control: missed, as expected); after the change `0.048 ms` (run 2), `0.053 ms` (run 3) | met |
| B1 (c7) existing suites, release, unedited, on the final library | all pass; the named files | `service` unit tests 11; `service_tcp` 2; `service_base_dir_w48j` 1; `sv301_service_w48z` 5; `physics_service` 1; `workflow` 5; `particle_run` 9; `baseline_hashes` 3 (1 ignored); `case_hashes_w48u` 1; `w48v_case_hashes` 0 (its one test is ignored in the tree); `rcfd-cli` `cli` 8; `rcfd-mcp` library 18 (1 ignored); 0 failed anywhere. The diff against b8b98cbf touches `service.rs`, `lib.rs` (+1), `cosim_service.rs`, the test file, this ADR and the two protocol pages | met |

Against the predictions: D and D2 exactly 0, as predicted; F5 2.1 s (predicted two to five); the histogram of S0 is
reproduced by this build. Nothing missed.

### Deviations and disclosures

- **Amendment 1** (after run 1): the first transport wrote each line in pieces and cost 2 ms per call. One write per line
  and `TCP_NODELAY` on the accepted sockets of the service brought it to 0.05 ms. Two of the three edits are in the path
  of every service method (`serve_stream`, `serve_tcp`); they change no byte on the wire, and B1 was run after them.
  `service::Client` (used by `rcfd call` and the existing tests) still writes its request in pieces; not touched.
- The control of T1 on the old code was started twice (the first output was cut off by the filter of the command); both
  missed.
- **Amendment 2**: the 650-step runs take two to three minutes on this machine today because of its load (the in-process
  run 67 s to 190 s where ADR 0256 measured about 10 s; the diagnosis shows no cost of the transport). By the rule of
  amendment 1 `a1r`, `a2r`, `d` and `s0` are ignored heavy records, run twice by me (runs 1 and 2, identical). The fast
  tests read a 100-step reference. A scratch timing test was run for the diagnosis and not committed.
- The mutations were applied to a scratch copy of the tree (`git archive` of 62ea46e0, its own target directory), as the
  house rules ask, not to the worktree as the registration said. MU5 replaces the message by a text that still contains
  its length (to compile without a warning); it no longer contains the words of the participant.
- Not in the design list of the registration and added: `Service::unhost`, and the accessors `reconnects`, `lease`,
  `is_connected` of the proxies.
- F3 (b) and (c) cut the connection after the service has finished the `advance`; a cut while the `advance` is still
  being computed (the `restore` waiting behind it, the stale request refused by the lease) is covered by the design and by
  the superseded lease of P3, not by a test of its own.
- The check that the first 61 names of `METHODS` are unchanged looks at the count, the first and the 61st name.

### Mutations

Each applied alone to the scratch copy by an exact replacement that occurred once; the test file run once without the
ignored records, and for MU3, MU4 and MU7 also the named record; the file restored and compared byte for byte (identical
in all seven).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 `restore` does not reconnect | F3 | F3 (a, b) and F3 (c) failed (nothing else) |
| MU2 the lease is not compared | P3 | P3 failed (nothing else) |
| MU3 `cosim.restore` does not restore | P2, A1r or D | P2 and A1r failed; also D2, F1 to F4, NC1, P3 |
| MU4 the time of the `advance` reply is not taken | A1r | A1r failed; also D2, F1 to F5, NC1 |
| MU5 the message of the participant replaced | F1, P3 | F1 and P3 failed; also F2 |
| MU6 `cosim.open` issues no new lease | P3 | P3 failed (nothing else) |
| MU7 forces sent in reverse node order | D | D failed; also D2, F1, F3, F4 |

`scripts/limit_audit.py` on the new test file: 0 hits.

### Not run

The whole-workspace suite and the gate (the lead runs them). `crates/rcfd-fv` is untouched, so its suites were not run. A
call that outlasts the time-out; a connection cut while the `advance` is being computed; a participant written with
another JSON library; more than one coupler; a network other than the loopback interface.

### After the tests

`cargo fmt -p rcfd-api --check`: clean. `cargo clippy -p rcfd-api --all-targets --release -- -D warnings`: clean (before
run 1, after the change of amendment 1, and on the final tree).

### Decision by the registered rule

P1 to P3, A1r, A2r, D, D2, F1 to F5, V, B1 and T1 are met, S0 holds, NC1 and NC2 hold, and every mutation failed its named
tests: **done** by the rule, proposed as such; the acceptance stays with the maintainer. What "done" covers: the calls
served for participants that a program hosts, the proxies, the mock structure in a second process on the loopback
interface. The clause "under a new protocol version" is met as version 1.1 with the identifier `rcfd-service/1` kept
(question 1). Three of the acceptance tests are ignored records (question 2).

### Open questions (each with options and a recommendation)

- **Q1. The identifier.** Options: (a) as built: `rcfd-service/1` kept, `protocol_version` 1.1, `rcfd-cosim/1` for the
  calls; (b) `rcfd-service/2`: edits the assertions of `service_tcp.rs` line 32 and `cli.rs` lines 202 and 220 and the
  comment of `clients/claude-panel/hooks/register.tsx`, and tells every existing client that something incompatible
  happened, which is not so. Recommendation (a). Option (b) needs the maintainer: it edits existing tests.
- **Q2. The ignored acceptance records** (`a1r`, `a2r`, `d`, `s0`). Options: (a) leave them ignored: D2 (100 steps, both
  remote, bit for bit) and the in-process A1 and A2 of `cosim_f139` stay in the gate; (b) run them in the gate: about 20 s
  more on a free machine, two to five minutes on a loaded one. Recommendation (a).
- **Q3. Participants from a case, hosted by `rcfd serve`.** Not built: a case key (schema golden) and a CLI change.
  Options: (a) a follow-up item (below); (b) leave hosting to embedding programs. Recommendation (a).
- **Q4. `service::Client` writes its request in pieces** (2 ms per call measured for the same pattern in the proxy).
  Options: (a) one write, as the proxy (three lines; it changes the timing of `rcfd call`, no byte); (b) leave.
  Recommendation (a), with the item of Q3.
- **Q5. The elements of the structure in `describe`** (the builders of ADR 0263 take them; the `Structure` trait has
  none). Options: (a) with the item of Q3, an optional `elements` member and a trait method with a default; (b) leave.
  Recommendation (a).

### For the lead: roadmap entries

Proposed status of F-360: **done** (`[x]`), acceptance still with the maintainer.

Note text for F-360:

> DONE pending acceptance (ADR 0345, 2026-11-12; registered before the code, two amendments). The calls of docs/cosimulation-protocol.md are served by the JSON-RPC service for participants that the embedding program hosts (rcfd_api::cosim_service: Service::host_fluid, host_structure, each participant on its own thread so it need not be Send; rcfd serve hosts none; no case key, CLI or MCP change): methods cosim.list, cosim.open, cosim.describe, cosim.time, cosim.checkpoint, cosim.restore, cosim.forces, cosim.advance, plain JSON numbers that cross bit for bit (serde_json float_roundtrip), error codes 1005 (the participant error, its own message) and 1006 (no such participant). Service protocol 1.1: the identifier rcfd-service/1 is kept (the pinned tests are unedited), service.info gains protocol_version 1.1 and cosim rcfd-cosim/1; both protocol pages are versioned. The coupler is the unchanged cosim::coupled_step driven with the proxies RemoteStructure and RemoteFluid (TCP). A lease per participant (cosim.open) refuses requests left over from an old connection; a proxy whose connection is lost fails at once except restore, which reconnects once and restores the participant, so a dropped connection ends the step with the located error and both sides at the start of the step. Measured on the model problem of ADR 0256 with the mock structure in a second process: every one of 650 steps converged, 7 to 10 iterations (mean 8.766), omega_m/omega 0.998618 and 1.011453, and the displacements of every node at every step equal to the in-process run bit for bit (limit 1e-12 X), also with both participants remote (100 steps); a participant error (structure in the other process, fluid hosted) and a dropped connection (cut before the request, after the reply, between steps, fluid or structure) each give an error naming participant, time and iteration with 0 bits of either state changed and the next step equal to the reference; a killed participant process gives the located error in 2.1 s with the fluid restored and the failed reconnection reported; refusals with their codes; a control with springs stiffer by 1e-9 is seen (4e-9 X). Amendment 1: the first transport cost 2 ms per call (lines written in pieces), now 0.05 ms (one write per line, TCP_NODELAY on accepted connections; no byte changes). Amendment 2: the 650-step out-of-process tests are ignored heavy records (minutes on the loaded machine; run twice, identical), the gate runs the 100-step both-remote comparison and the failure tests. Existing service, CLI and MCP suites and the case hashes pass unedited; seven mutations each failed their tests. Not delivered: participants built from a case or hosted by rcfd serve, structural elements in describe, authentication, a test of a call outlasting the time-out. Open: ADR 0345 Q1 to Q5.

Follow-up item (number for the lead to assign). Title: Co-simulation participants from the case, hosted by rcfd serve. Deps: F-360. Acceptance:

> A case key names a moving wall patch as a co-simulation fluid participant and rcfd serve hosts it under protocol rcfd-cosim/1; a structural participant in its own process (the mock of ADR 0345) and the coupler of rcfd_fv::cosim drive it over TCP and reproduce ADR 0256 A1 and A2 with the displacements of the in-process run to 1e-12; describe of a structure may carry its elements so that the map builders of ADR 0263 run on the side of the coupler; service::Client writes a request with one call; without the key every case hash is unchanged and the schema, CLI help and MCP goldens change only by the approved additions.
