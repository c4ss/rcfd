# ADR 0353: F-456 registration: `monitors.get` builds only what is asked for

Status: **REGISTERED 2026-11-12**, written before any code of F-456 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and nothing was timed: the code and documents under "What was read" were read,
the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the end; nothing
above the addendum is changed after a run.

Parents: [ADR 0350](0350-electron-studio-performance-spike-and-rcfd-studio-retirement.md) (P3, amendment 1 section A1.4,
addendum 1: P3 measured 126.6 ms for a 12.0 MB reply, question QA1 whose recommendation (a) is this item). Roadmap item F-456
(follow-up of F-447). Branch `f456-monitors-get-filter` from main 3313d3b0, worktree `w49i-work/f456`.

House rules (ADR 0130, 0213, 0219): limits fixed here and not changed after a run; a miss becomes an ignored `*_known_miss`
test with the limit intact and the number recorded, diagnosed and not tuned; each registered run is made once; existing tests,
ignored records, `baseline_hashes.rs`, the hash tables and the goldens are not edited; no new crate, no unsafe code, no
trailers, no push.

Clean-room. Nothing here needs a paper: the change is a different order of the same operations on a JSON document. The
formats used are the repository's (`rcfd-monitors` v1, `docs/service-protocol.md`) and the number formatting of `serde_json`
(shortest round-trip decimal for `f64`, the algorithm of R. Ryu by U. Adams, PLDI 2018, cited from memory; this work does not
re-implement it, it calls the same function the old path called).

## What F-456 asks

The roadmap acceptance, quoted and binding: "`monitors.get {names, since}` applies `names` and `since` before the document is
built, with replies equal to today's byte for byte for every existing test; P3 of ADR 0350 measured again as registered (4
series of 1e5 samples, request to parsed object in the renderer) with the 100 ms limit unchanged, and an increment of 100
samples at 1e5 stored in at most 2 ms; a miss is recorded, not tuned."

**Reading (registered here).**

1. "Applies `names` and `since` before the document is built": the store selects the series and the sample range first and
   only the selected samples are copied or formatted. The in-process `call("monitors.get")` still returns a `Value`; its
   `Value` holds the selected samples only.
2. "Replies equal to today's byte for byte": for every request, the `Value` returned by `Service::call`, and the line written
   by `Service::serve_stream` (the wire), are the bytes the unchanged code gives. "Every existing test" is read as: the
   existing tests that call `monitors.get` (service unit tests, `rcfd-mcp`, `rcfd-assistant`, `rcfd-gui` controller) pass
   unedited, and a new equality test (E1 to E3) compares the new replies with the old algorithm for a large set of requests.
3. "P3 measured again as registered": `bench/electron` check `p3` of F-447 unedited: a 4 x 4 x 4 box session stepped 1e5 times
   with one force report (four series), `monitors.get {names}` of the four series, the main process hands the reply line as a
   string to the renderer, which `JSON.parse`s it; from the renderer's request to the parsed object; 20 repeats, judged on the
   median; the same machine; limit 100 ms.
   Because the request names all four series and asks for the full history, `names` and `since` do not reduce what P3 asks
   for. The remaining saving comes from not building the intermediate `Value` tree (below). That P3 may stay missed is a
   possible outcome (prediction below).
4. "An increment of 100 samples at 1e5 stored in at most 2 ms" is ambiguous. Two readings are registered and both are checked:
   **I1** (the reading I judge): reading an increment: with 1e5 samples in each of the four series, `monitors.get {names,
   since}` that returns the last 100 samples of each, request to parsed object in the renderer, median at most 2 ms. **I2**:
   storing an increment: appending 100 samples to each of four series that already hold 1e5, `MonitorStore::push`, median at
   most 2 ms. Either way "stored" is the state of 1e5 samples held by the store.

## What was read (no run)

`docs/adr/0350-...md` (section 3 P3, amendment 1 A1.4, addendum 1 and its diagnosis: "the reply reaches Node as a string in a
median of 116 ms and `JSON.parse` takes 58 ms there"; "`monitors.get {since}` for the last 100 samples (a 12 kB reply) still
takes 25 ms, because the whole document is built before `since` is applied"); `crates/rcfd-api/src/service.rs` (the
`monitors.get` arm, `handle_request`, `serve_stream`, `Client`); `crates/rcfd-post/src/monitors.rs` (`MonitorStore`, `Series`,
`to_json`, `since`, `from_json`); `bench/electron/main.js`, `renderer/bench.js` (`p3`), `run.js`;
`crates/rcfd-api/examples/f447_bench.rs`; `docs/service-protocol.md`.

Facts used from the code:

- Today's arm calls `MonitorStore::to_json`, which copies every sample of every series into a `serde_json::Value`, then
  removes the unwanted series, then replaces the retained series by `since` slices (a second copy). `Value` is 32 bytes per
  number, so 8e5 numbers are about 26 MB of nodes plus the array vectors.
- `serve_stream` wraps the result in `json!({"jsonrpc","id","result"})` and calls `to_string()`: a third pass, the formatting.
- `serde_json` is built without `preserve_order` in this workspace (`Cargo.toml`: features `float_roundtrip` only), so object
  keys print in sorted order: `{"id":..,"jsonrpc":"2.0","result":..}`, `{"format","series","version","x_label"}`, and in a
  series `{"unit","x","y"}`.
- `Series` slices serialise with the same code whether they sit in a `Value` or are written directly: a finite `f64` by the
  shortest round-trip form, a non-finite one as `null`.
- `since` uses `partition_point(|v| v <= after)`, so it assumes a sorted abscissa but is defined for any input; the new code
  uses the same call.

## Arithmetic before this registration (disclosed, by hand)

- Reply size: 4 series x 1e5 samples x 2 arrays = 8e5 numbers; 12.0 MB measured, so 15 bytes per number (abscissa `1.0`
  to `100000.0`, ordinate 17 significant digits).
- Time budget of ADR 0350's addendum: the Electron median is 126.6 ms. Its diagnostic (a separate run in Node, not a
  decomposition of the Electron median) found the string in Node after a median of 116 ms and the parse at 58 ms. The two do
  not add to 126.6, so I claim no decomposition; I note only that the parse is not touched by this change.
- What the change removes: one `Value` tree of 8e5 nodes built twice (once by `to_json`, once by the `since` branch only when
  `since` is given) and walked once more by the formatter. A guess for the removal is 20 to 40 ms of the 126.6 (allocation of
  26 MB plus cache-missing traversal against a straight formatting loop at about 25 to 50 ns per number for 8e5 numbers, that
  is 20 to 40 ms of formatting that stays). Prediction for P3 after the change: median 85 to 110 ms. A meet is not assured.
- I1: the reply is 400 numbers, about 6 kB; the work is 4 binary searches, 800 copies or direct formats, one lock; the
  tcp hop and the renderer's parse of 6 kB are well under 1 ms. Prediction 0.2 to 0.8 ms.
- I2: `Vec::push` amortised; a push that reallocates a 1e5-element `f64` vector copies 0.8 MB (about 0.1 ms); 800 pushes at most
  a few doubling events. Prediction under 0.1 ms.

## Design

### G1. The store builds a filtered document

In `rcfd-post` (`MonitorStore`): a selection `names: Option<&[&str]>`, `since: Option<f64>`.

- `to_json_filtered(names, since) -> Value`: the document of `to_json` restricted to the series whose name is in `names`
  (all when `None`) and, when `since` is given, to the samples with `x > since` of each retained series
  (`partition_point(|v| v <= after)`), the unit kept. Built from slices of the stored vectors: only retained samples are
  copied. `to_json()` stays and is unchanged in output; it is `to_json_filtered(None, None)`.
- `write_json_filtered(out, names, since)`: the same document written straight into a byte buffer, without a `Value`, in the
  key order of the `Value` form given above.

The old semantics are kept exactly: `names` that is not an array, or a `since` that is not a number, are ignored; an entry of
`names` that is not a string is skipped; an empty `names` array retains no series (an empty `series` object); a name that is
not a series is ignored; `since` NaN cannot arrive from JSON.

### G2. The service

- The `monitors.get` arm of `Service::call` uses `to_json_filtered`, the same parameter parsing as today. Its output `Value`
  is equal to today's.
- The wire. `serve_stream` parses the request as today; for a valid request whose method is `monitors.get` it computes the
  whole response line from `write_json_filtered` plus the envelope and writes it, instead of building a `Value`. A request
  with no `id` is a notification: nothing is written, as today. An error (no session, no simulation) takes today's path
  (`json!({"jsonrpc","id","error"})`), so its bytes are today's. All other methods are untouched.
- `report.export` format json, `panel.snapshot` and the other callers of `to_json()` are untouched.

### G3. Why this is within the rules

Defaults are bit-identical by construction and by checks E1 to E3: no parameter, no reply format and no protocol member
changes, `service.info` and the schema are untouched, no golden is involved. No new crate: `serde_json::to_writer` is the
function `Value`'s printer uses.

### Questions settled by taking the recommendation

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

- Q1. A direct-to-bytes wire path for `monitors.get` only (recommendation) against building a `Value` with a cheaper
  document. Reason: P3 asks for the full history, so a cheaper document does not remove the formatting pass; the wire path
  removes the tree. The in-process path keeps returning a `Value`, which its callers (MCP, GUI, assistant) need.
- Q2. Carrying the series over `data.*` as `f64` sections (QA1 option (b)) is not done; ADR 0350 recommends it only if (a)
  misses.

## Registered checks

All limits fixed now. Release profile throughout. "Reference" in E1 to E3 is the old algorithm written out in the test file,
verbatim from the unchanged arm: `to_json()`, `retain` by `names`, then `since` slices.

| Id | Check | Limit | Estimate behind it |
|---|---|---|---|
| E1 | `to_json_filtered(names, since)` against the reference, 3000 seeded random (store, names, since) combinations; stores of 0 to 8 series of 0 to 300 samples with ties, an unsorted series, `NaN`, infinities, `-0.0`, subnormals, 1e308, an empty series, a series name that needs JSON escaping, a unit with quotes and non-ASCII text; compared as `Value` and as `to_string()` | 0 differing | same arithmetic on the same slices |
| E2 | the same on a service session: a store recorded by a seeded generator (5 series, 700 samples, ties, the special values above) is placed in the session's simulation; 600 requests (fixed list of 40 edge cases plus seeded random names/since/extra params, bad session id, no session); `Service::call` result against the reference | 0 differing bytes of `to_string()`; errors equal in code and message | as E1 |
| E3 | the same requests through `Service::serve_stream` (in-memory reader and writer), the line against the reference line wrapped by `json!({"jsonrpc","id","result"})` or the error envelope; ids of kinds integer, string, null, absent (notification: no output), fractional | 0 differing bytes | as E1 |
| E3h | FNV-1a 64 of the concatenated reference lines of E3 equals the constant recorded from the unchanged tree (the test is committed and run on the old code first) | equal | proves the reference is the old code |
| E3b | one request for the full history of 4 series of 1e5 samples (generator values) through the wire and through the reference | 0 differing bytes | as E1 |
| E4 | the unedited existing tests: `cargo test -p rcfd-api --release --lib service` (service unit tests), `--test baseline_hashes`, `--test cosim_service_f360`, `cargo test -p rcfd-post --release` (monitor tests), `cargo test -p rcfd-mcp --release --test default_f1`, `cargo test -p rcfd-assistant --release` (it calls `monitors.get`), `cargo test -p rcfd-gui --release` (controller) | all pass; none edited | unchanged outputs |
| P3 | `p3` of bench/electron, unedited, one run on the new server (20 repeats, median) | median <= 100 ms; also report max and `reply_chars` | 85 to 110 ms (see arithmetic); a meet is not assured |
| P3c | control: the same `p3` on the unchanged server (the F-447 binary, same data folder, same day) | reported; it reproduces the old number if it lies in 105 to 150 ms and `reply_chars` equals that of P3 | ADR 0350 measured 126.6 ms; load differs |
| I1 | check `p3i` (added to bench/electron, new, `p3` not edited): own session of the same 4 x 4 x 4 box with the force report, 1e5 steps; `since` = the abscissa of the 101st sample from the end (so exactly 100 samples remain per series), `names` the four series; 20 warm-up requests then 200 timed, request to parsed object in the renderer | median <= 2 ms; the replies hold 100 samples per series (checked in the renderer); max and p99 reported | 0.2 to 0.8 ms |
| I1c | control: `p3i` on the unchanged server | reported; ADR 0350 measured 25 ms for this reply | 15 to 40 ms |
| I2 | a Rust test (release, `#[test]`, fast): a store of 4 series x 1e5 samples; 200 repeats of: clone the store, push 100 samples to each series, time the pushes only (`Instant`); the median of the 200 | <= 2 ms | under 0.1 ms |
| I2s | recorded, no limit: wall time of `run.step {count: 100}` at 1e5 stored samples by the Electron bench, inside `p3i` after the timed requests | reported | solver cost, not storage |

### Negative controls

- N1. A reference that is wrong on purpose (the reference run with `since` applied as `x >= after`) must be reported as
  different by the same comparison in E1: the test asserts that at least one of the random combinations differs
  (self-test of the comparison).
- N2. The E3 comparison on a deliberately altered line (one byte flipped in a copy) reports a difference.
- N3. P3c and I1c reproduce the slow path on the unchanged server, so a gain is not a change of the set-up.

### Mutations (scratch copy `f456-scratch`, not committed; each must be caught by a named check)

- M1. `partition_point(|v| v < after)` (a sample at `x == since` is returned) in the filtered builder.
- M2. The wire writer ignores `names` when `since` is given.
- M3. The wire writer prints the ordinate slice from index 0 while the abscissa is sliced.
- M4. The wire writer swaps the key order in a series (`"x"` before `"unit"`).
- M5. The wire path answers an unknown session with a result instead of the error envelope.
- M6. The wire path writes `null` as `0` for a non-finite number (a hand formatter).
- M7. The wire path writes a reply to a notification (no `id`).
- M8. The in-process path forgets the unit in a `since` slice.

### Classes

Equality checks E1 to E3b are exact (0 differences); there is no tolerance anywhere. Timings: the median of the stated sample.

## Predictions (mine, before any run)

E1 to E3b pass at once, as the wire writer is simple; the likely surprise is a key order or a number format (`1.0` against
`1`), which E3h catches against the old tree. P3: 85 to 110 ms, so it is as likely to miss as to meet. I1: meet. I2: meet.

## Decision rule

- **done**: E1 to E3b, E3h and E4 pass, P3 median <= 100 ms, I1 median <= 2 ms and I2 median <= 2 ms.
- **partial**: the equality checks and E4 pass but P3 is above 100 ms, or I1 or I2 is above 2 ms. A miss is recorded as an
  ignored `*_known_miss` record with the limit intact and diagnosed; it is not tuned. Question QA1 (b) of ADR 0350 then goes
  to the maintainer.
- **todo**: any equality check fails and cannot be repaired without changing a reply.

## Limits of this registration

- P3 is measured by one registered run on a shared machine whose load is recorded with the run; the control run is also one
  run. A difference of a few milliseconds between them is within the load.
- The wire path is one method; other large replies (`field.slice`, ...) are not changed.
- `monitors.get` still returns the whole history unless the client passes `since`; the Studio's use of `since` is a separate
  matter (ADR 0350, the shell).
- I2 measures the store, not the solver's step.
- The renderer's `JSON.parse` of 12 MB is not touched.

## Amendment 1: E3h as the test file defines it (2026-11-12, written before any run of the new code)

Checks E1 to E3b are as registered. Clarification of E3h, made after writing the service test and before running it on the
changed code: the hash of E3h is taken over the lines the service wrote (not over a reference), for the 594 requests, three
error requests, one request with `jsonrpc` "1.0", and one two-request connection. Where a reference exists (all requests with
a result) the line equals the reference line, which E3 asserts; the error lines have no reference in the test, and the hash
covers them. The test file `crates/rcfd-api/src/service/monitors_f456.rs` was written against the unchanged tree and passed
its E2, E3 and E3b comparisons there; the constant `E3H` = 0xa918e7df5af7a64a was recorded from that run. The unchanged code
is the "before". Committed before any code of the change.

## Addendum 1: results (2026-11-12)

Code: `crates/rcfd-post/src/monitors.rs` (`MonitorStore::to_json_filtered`, `write_json_filtered`, private `selected`),
`crates/rcfd-api/src/service.rs` (the `monitors.get` arm uses `to_json_filtered`; `monitors_selection`; `monitors_get_line`
and its call in `serve_stream`), tests `crates/rcfd-api/src/service/monitors_f456.rs` (E2, E3, E3h, E3b, N1, N2) and
`crates/rcfd-post/tests/monitors_filtered_f456.rs` (E1, I2), `bench/electron/renderer/bench.js` (new check `p3i`; `p3`
unedited). The Electron runs were made once each (`electron bench/electron --checks=p3,p3i`, the data folder and the
Electron 45.1.0 of F-447, a server built from this tree for the new run and one built from the tree before the change for
the control), 1920 x 1080 at 60 Hz, after a smoke run of the new code (numbers not results). Raw results are in the scratch
folder (`out/new.json`, `out/old.json`, `out/load.txt`), not committed. Processor load (all cores, other agents' builds
included): 54 percent before and 63 after the new run, 36 and 44 around the control.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| E1 | 0 differing of 3000 | 0 (as `Value`, as `to_string()` and as the written text; the 3000 documents have more than 1000 distinct sizes) | yes |
| E2 | 0 differing bytes of 594 requests | 0 | yes |
| E3 | 0 differing bytes of 594 requests on the wire (ids integer, string, null, fractional) | 0; the notification gets no line | yes |
| E3h | FNV-1a equals the constant of the tree before | `0xa918e7df5af7a64a` on the old tree and on the new | yes |
| E3b | 0 differing bytes, 4 series x 1e5 samples, three requests | 0 | yes |
| E4 | existing tests pass unedited | `rcfd-api` lib `service` (14: 11 old and 3 new), `baseline_hashes` (3 passed, 1 ignored), `cosim_service_f360` (13), `data_service_f447` (6), `rcfd-post` all tests, `rcfd-mcp default_f1` (2), `rcfd-assistant` all tests, `rcfd-gui` all tests: all pass | yes |
| P3 | median <= 100 ms | median **82.6**, p99 = max 103.1, min 65.2 (20 repeats; 4 x 1e5 samples; reply 12,044,347 characters) | **yes** (the maximum is above 100; the limit is on the median) |
| P3c | control reproduces 105 to 150 ms and the same `reply_chars` | median **151.4** (max 193.2, min 127.3), 12,044,347 characters | `reply_chars` equal; the median is 1.4 ms above the registered band (see deviations) |
| I1 | median <= 2 ms | median **0.30** ms, p99 0.70, max 0.90 (200 timed after 20 warm-up; 100 samples returned for each of the four series; reply 12,335 characters) | yes |
| I1c | control reported (15 to 40 ms predicted) | median 21.1 ms, p99 26.5, max 27.5 | in the band |
| I2 | median <= 2 ms | median **1.17** ms, max 2.27 (800 pushes onto four series of 1e5) | yes (the prediction of under 0.1 ms was wrong, see below) |
| I2s | recorded | `run.step {count: 100}` at 1e5: 58.1 ms (new server), 53.8 ms (control): the solver's cost, not storage | recorded |
| N1, N2 | the comparison sees a difference | N1: a reference with `x >= since` differs on at least one request; N2: one flipped byte differs | yes |

### Mutations (scratch copy, not committed; the unmutated tree passes the same tests)

| Mutation | Caught by |
|---|---|
| M1 `since` as `x >= after` in the builder | E1, E2/E3, E3b |
| M2 the wire writer ignores `names` when `since` is given | E2/E3 (E1 does not see it: it concerns the wire writer's call, not the builder) |
| M3 the wire writer writes the whole ordinate vector | E1, E2/E3, E3b |
| M4 key order changed in a series | E1, E2/E3, E3b |
| M5 an unknown session answered with a result | E2/E3 (error request, hash) |
| M6 non-finite numbers written as 0 | E1, E2/E3, E3b |
| M7 a reply to a notification | E2/E3 |
| M8 the in-process path drops the unit under `since` | E1, E2/E3 |

All eight are caught. `cargo fmt -p rcfd-api --check` and `-p rcfd-post --check` clean; `cargo clippy -p rcfd-api -p rcfd-post
--all-targets -- -D warnings` clean (after two clean-ups in the new tests, and `map_or(true, ..)` in place of `is_none_or`
in `rcfd-post`, which the workspace's MSRV of 1.80 rejects there).

### Deviations and disclosures

- The control P3c median lies 1.4 ms above its registered band; reported, not re-run. The control ran at a lower load (36 to
  44 percent) than the new run (54 to 63) and was slower, so the set-up is not the cause of the gain; the ratio of the two
  runs (151.4 to 82.6 ms; ADR 0350's own run gave 126.6) is indicative. P3's limit is met by 17.4 ms on the median.
- I2's median is 1.17 ms against a prediction of under 0.1 ms. The test clones the store before each repeat, and a cloned
  `Vec` has capacity equal to its length, so the first push of each of the eight vectors reallocates and copies 0.8 MB (6.4 MB
  in all): by inspection, not measured separately. A live store grows by doubling and does not pay this each time. The limit
  (2 ms) is met on the median; the maximum (2.27 ms) is above it.
- E3h as defined by amendment 1 (the lines the service wrote, error lines included).
- The wire path serves `monitors.get` only; `rcfd-mcp`, the GUI controller and the assistant call `Service::call`, which
  returns a `Value` holding the selected samples only (they gain from `names` and `since`, not from the direct writer).
- The p3i check measures request to parsed object including the Electron hop, as P3 does.

### Not run

The full workspace suite (the lead runs the gate). No golden or hash table needed an edit and none was touched. `rcfd serve`
on stdio uses `serve_stream`, covered by E3, and was not run as a process.

### Decision by the registered rule

**Done.** E1 to E3b, E3h and E4 pass, P3 median 82.6 ms <= 100, I1 median 0.30 ms <= 2, I2 median 1.17 ms <= 2.

### Remaining questions (options and a recommendation each)

- **QF1, the shell's use of the history.** (a) a shell fetches the history once and then `since` increments (0.3 ms per
  poll); (b) refetches the whole history (83 ms for 4 x 1e5). Recommendation: (a), in the Studio design (F-448).
- **QF2, other large replies.** `field.slice` of 1e6 cells (62.6 MB in ADR 0350, R0) is still built as a `Value`. (a) a
  follow-up item that applies the same direct writer to `field.slice`; (b) leave it, since `data.*` carries bulk fields.
  Recommendation: (b): the opt-in `data.*` route already serves bulk fields.
- **QF3, I2's cloned-store set-up.** (a) leave the record; (b) a corrected test that clones with spare capacity.
  Recommendation: (a); the limit is met and a corrected test would not change the decision.

### For the lead: roadmap entries

Proposed status of F-456: done.

> F-456: ADR 0353, 2026-11-12. `monitors.get {names, since}` selects the series and the samples before the document is built (`MonitorStore::to_json_filtered`), and on the line protocol the reply is written straight into the reply line without a JSON tree (`write_json_filtered`, `serve_stream`); `Service::call` still returns a `Value`. Replies are equal to the unchanged code byte for byte: 0 differing of 3000 stores (E1) and of 594 requests through the call and through the wire (E2, E3), including NaN and infinities, escapes, ties, an unsorted series, error and notification cases, four series of 1e5 samples (E3b), and the hash of the old tree's lines reproduced; the existing tests of `rcfd-api`, `rcfd-post`, `rcfd-mcp`, `rcfd-assistant` and `rcfd-gui` pass unedited, no hash or golden moved. P3 of ADR 0350 measured again as registered (Electron 45.1.0, 4 series of 1e5 samples, 12.0 MB): median 82.6 ms against 100 (the control on the unchanged server, same day: 151.4 ms, 1.4 ms above the registered band of the control). An increment of 100 samples at 1e5: fetched in a median of 0.30 ms (control 21.1 ms), stored (800 pushes) in a median of 1.17 ms, limit 2 ms. Eight mutations caught.

Notes for other items: F-447 (partial, P3 missed): its note may add that P3 is met after F-456 (ADR 0353, 2026-11-12) at 82.6 ms. F-448 (Studio design): the history is fetched once, then `since` increments at 0.3 ms.
