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
