# ADR 0124: Wave 47C, the time-series field store (`rcfd-tsstore`, Studio plan L3) and its opt-in run-path hook

Status: **ACCEPTED 2026-10-26** (limits registered before the registered runs). Written at main f307f59 on branch `w47c`, target
`<target dir>`. House rules: limits fixed before the registered runs; a missed limit becomes an ignored
`*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected run; defaults are
bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Clean-room: first principles only (IEEE 754 rounding,
the FNV-1a construction, RFC 1951 deflate through an existing crate). Parents: `docs/ui-restart-plan.md` (L3, section 8 contract),
F-015 (the native container, whose mesh encoding is reused), F-116 (output policy), ADR 0050 (`output.write_time_interval`).

## Context: what exists

* `rcfd_io::container` (F-015): a versioned, sectioned, FNV-64-checksummed file written whole (temp file then rename). It holds a mesh
  (`mesh_to_bytes` / `mesh_from_bytes`, topology exact, geometry recomputed) and named f64 arrays. Restart files (`restart.rcfd`) use
  it. It has no time axis, no partial reads and cannot be appended to; a crash during a write leaves the old file or nothing.
* `rcfd_io::xdmf` writes XML plus raw binary heavy data; `rcfd_io::vtk`, `ensight`, `tecplot`: one file set per time. The run path
  writes `snapshots/snapshot_<step>.vtu` (`output.write_interval`, F-116) and `snapshot_t<time>.vtu` (`output.write_times`, ADR 0050).
  None of them is indexed, checksummed per chunk or readable field by field.
* HDF5 and CGNS: no reader or writer exists (F-014, F-237 notes; ADR 0048 D7). The open decisions are not touched: this store does not
  depend on HDF5 and does not imitate its format.
* The project folder contract (plan section 8) lists `runs/<run-id>/fields/` as "time-series store (index.json, mesh once, one
  snapshot per saved time)".

## Decisions

* **D1 One append-only file per run, `fields/store.rts`, plus an advisory `fields/index.json`.** The task asks for a footer index with
  offsets and a scan when it is missing; the contract names `index.json`. Reading: the binary store is authoritative (footer index);
  `index.json` (format `rcfd-tsstore-index`, steps and times, mesh hash, counts) is a mirror written by `Writer::finish` through a
  temp file and rename, and its failure is not an error (a reader may hold it open on Windows). Readers never need it. This is an
  additive reading of the contract, reported to the lead.
* **D2 Layout** (little-endian): header `RCFDTSS\0 | u32 version | u64 fnv64(previous 12)`; frames `kind[4] | u64 len | u64
  fnv64(payload) | u64 fnv64(previous 20) | payload` with kinds MESH (mesh hash, cell and point counts, `mesh_to_bytes`), DATA (one
  block of one field), SNAP (JSON directory of one snapshot: step, time, per field name, unit, location cell or point, n, precision,
  codec, block size, block offsets), FOOT (JSON index, written by `finish` only); trailer `u64 FOOT offset | "RTSE" | u64 fnv64(previous
  12)`. Fields are scalar; a vector is stored as components (ux, uy, uz). Version: files with version 0 or above this build's are
  refused.
* **D3 Checksum and codec.** Checksum: `rcfd_core::hash::fnv64` (already in the workspace). FNV-1a over a fixed-length region detects
  every change confined to one byte with certainty: the state update `h -> (h xor b) * p` is a bijection of the 64-bit state for each
  byte (xor with a byte is a bijection, multiplication by the odd prime p is a bijection modulo 2^64), so two inputs that differ in one
  byte reach different states at that byte and stay different. The frame head carries its own checksum so a damaged length is not
  mistaken for a truncated tail. Lossless codec `shuffle_deflate`: bytes regrouped by significance (all first bytes of a block, then all
  second bytes, ...) and deflated at level 1 by `miniz_oxide =0.8.9` (MIT OR Zlib OR Apache-2.0), which png 0.18 already pulls into
  `rcfd-post`; it adds no crate to the build graph (checked with `cargo tree -i miniz_oxide@0.8.9`). Default codec `raw`.
* **D4 Lossy mode** `Precision::F32`: each value rounded to the nearest IEEE single. Registered bound per value:
  `|stored - x| <= 2^-24 |x| + 2^-150` (half an ulp relative in the f32 normal range; half the subnormal spacing 2^-149 below it).
  Finite values that round to infinity are refused (the writer returns an error). NaN stays NaN (payload not kept), infinities stay.
* **D5 Crash safety.** A snapshot's DATA frames are written before its SNAP frame; the snapshot exists once the SNAP frame is complete
  and the buffer is flushed to the OS (every commit). A reader with no valid footer scans frame heads from the mesh frame on, stops at the
  first frame that runs past the end of the file (truncated tail) and reports `IndexSource::Scanned { reason, tail_bytes }`. A complete
  frame head with a wrong checksum is an error, not a tail. `Writer::open_append` scans, cuts everything after the last complete
  snapshot (interrupted snapshot, old footer) and continues. Assumption: an interrupted write leaves a prefix of the bytes written (true
  for a killed process: the OS page cache keeps what was handed over). Power loss and `fsync` are not covered and not tested.
* **D6 Access.** Seek-based: memory mapping needs `unsafe`, which the workspace forbids (`unsafe_code = "forbid"`). The reader keeps
  only the snapshot index (step, time, offsets); directories and blocks are read per call. Blocks of `block_values` values (default
  65 536 = 512 KiB of f64) are the unit of partial reads: `read_subset` reads only the blocks holding the requested cells. Lookup by
  step or time, `Pick::Nearest` (earlier one on a tie) or `Pick::Floor` (equal times: the latest step); `Mode::Linear` interpolates
  `a + w (b - a)`, `w = (t - t0)/(t1 - t0)`, flagged `interpolated`, and returns the stored values unflagged at a stored time. A live
  reader picks up new snapshots with `refresh()`.
* **D7 Retention** `retain_every(src, dst, k)`: a new store with snapshots 0, k, 2k, ... and the last one, DATA frames copied verbatim
  after their checksums and decoding are checked.
* **D8 Run-path hook** (rcfd-api). Keys `output.store_interval` (int, 0 = off), `output.store_time_interval` (float s, 0 = off),
  `output.store_fields` (list, empty = ux uy uz p). The hook lives in `runctl::drive`, the one loop that drives steady iterations and
  transient steps for `rcfd run`, `run.headless`, chains and studies; it is the narrowest place that sees both drivers, the output
  directory and the end of the run. `Simulation::step` is not touched. It writes to `<out_dir>/fields/` (the run folder of the
  contract, when the runner passes it as the output directory): after each step when `index % N == 0` or when the step's time passes the
  next multiple of `dt` (relative slack 1e-9; the time step is not changed, unlike `output.write_time_interval`), and the final state at
  the end of every run outcome unless that step was just stored. With both keys 0, `Hook::open` returns `None` after reading the two
  keys; nothing else runs. Units: `m/s` for velocity components and speed, `K` for `T`, `unknown` otherwise (incompressible `p` is
  kinematic, compressible `p` is Pa; no unit table exists). Located errors: `store_time_interval` with `simple`, `simplec` or `coupled`;
  either key with `dynamic_mesh.enabled` (the mesh is stored once). A steady engine selected otherwise (steady compressible, steady
  variable density) gets a warning and no store. An unknown field name: a warning, no store, run unchanged. A fresh run replaces a store
  in the same output directory (as `solution.vtu` is replaced); a continued run appends when the mesh hash matches and the store ends
  before the continued run's first step. Library callers that step a `Simulation` themselves are not covered.

## What I already knew when registering (rules c, d; disclosed)

The crate, the hook and every test were written before this ADR so that the rule-d smoke could run; they are committed after it. The
smoke runs (debug, plus one release run of C7/C8 to set the time and memory limits) are recorded below; the registered runs are the
release gate runs after this commit.

Smoke results (debug unless stated):

* C1 passed (both codecs). C2 worst error / bound 0.9996 (both codecs). C3 ratio 0.647. C4 file 8439 bytes, mesh frame ends at 4239,
  snapshots at 5594 / 6951 / 8309, 4201 truncations reopened and appended, all correct. C5 3 snapshots recovered, tail 8224 bytes. C5b
  rounds recovered 5 / 12 / 20 snapshots, tails 8078 / 0 / 8090 bytes. C6 raw 8248 bytes and deflate 8439 bytes, two masks, 0
  undetected, 204 flagged per codec (the 82-byte footer payload and the 20-byte trailer, times two masks). C9 dyadic error 0, random
  worst error / bound 0.055; negative controls violated in 196/196 and 200/200 queries. C10, C11, C12 passed.
* H1 set-up defect found in the smoke: the steady `simple` cavity (nx = ny = 12, default relaxation) diverged at iteration 14 (steady
  divergence guard). Corrected before registration: `alpha_u = 0.7`, `alpha_p = 0.3` for the steady arm; H1-H4 then passed.
* C7/C8 release smoke (one run; the first attempt failed in the test's own parsing of the child output, the harness prints the test
  name on the same line; fixed before the counted run): set-up 1M-cell mesh 5.8 s, write 1.3 s, store 674 MB. Medians: open with
  footer 0.0001 s, open by scan 0.0026 s, nearest field 0.0094 s, linear field 0.0199 s, 1000-cell subset 0.0091 s, 2000-snapshot
  open with footer 0.0002 s, by scan 0.0123 s, random lookup plus field read 0.00004 s. Peak working set: scrub 28.7 MB, whole-snapshot
  reader 158.4 MB. The 1000 subset cells are spread over all 16 blocks, so the subset read costs as much as a whole field (stated
  as measured; a localized subset reads fewer blocks).

## Registered checks (tests in `crates/rcfd-tsstore/tests/store.rs`, `scale.rs`, `crates/rcfd-api/tests/fieldstore_w47c.rs`)

| Id | Check | Limit (reason) | Negative control |
|---|---|---|---|
| C1 | Lossless round trip, raw and shuffle_deflate, block size 5 (partial last blocks), 4 snapshots, cell and point fields, random bit patterns incl. NaN payloads, infinities, subnormals, -0 | bit-exact: every `to_bits` equal; mesh topology and content hash equal; `read_subset` (unsorted, repeats, across blocks) equals the full read | another snapshot's data differs; the f32 field is not bit-exact |
| C2 | Lossy F32 on 4000 values over 1e-46 .. 3.3e37 with both signs, 0, NaN, inf, an f32 subnormal | max of `|err| / (2^-24|x| + 2^-150)` <= 1 (D4, IEEE round to nearest) | the ratio exceeds 0.5 somewhere (the bound is not loose by more than 2x); a value 1e39 is refused |
| C3 | shuffle_deflate on a smooth 16 000-cell pressure field | snapshot bytes <= 0.9 x raw (the codec must earn its CPU time); round trip bit-exact | (ratio stated) |
| C4 | Truncation at every byte length of a 3-snapshot deflate store | length < end of mesh frame: open fails; otherwise opens, holds exactly the snapshots whose SNAP frame ends within the length, all verify and are bit-exact; footer used only at full length; `open_append` then adds one snapshot, reopens from the footer, all bit-exact | the footer is not used for any truncated length |
| C5 | Child process writes 3 snapshots and one field of the fourth, flushes, prints a marker and is killed (`Child::kill`) | reopen: exactly 3 snapshots, verify passes, bit-exact; `open_append` writes snapshot 4, footer present, all 4 bit-exact | no footer (`Scanned`) and tail bytes > 0 |
| C5b | Child writes deflate snapshots in a loop, killed after >= 5 / 12 / 19 snapshots (three rounds) | every recovered snapshot verifies and is bit-exact; at least the observed count recovered | (tail bytes reported, no limit) |
| C6 | Flip every byte (xor 0xff and xor 0x01) of a 3-snapshot store, raw and deflate | every flip is rejected (open or `verify` fails) or flagged (`Scanned`) with data still bit-exact; unchecked area: none (`UNCHECKED = []`) | the intact file opens from its footer and verifies |
| C6b | Version 2 header with a valid checksum; damaged magic | refused with "unsupported store version 2" / "magic" | |
| C7 | Release, 1M-cell box (100^3) with 20 f64 fields x 3 snapshots, medians of 5: open with footer; open by scan (trailer cut); one field, nearest snapshot; one field, linear in time; 1000-cell subset; and a 2000-snapshot store (1000 cells): open with footer, open by scan, random lookup by step or time plus a field read | open with footer <= 0.05 s, open by scan <= 0.5 s, nearest field <= 0.1 s, linear field <= 0.2 s, subset <= 0.1 s; 2000 snapshots: open with footer <= 0.05 s, by scan <= 0.5 s, lookup plus read <= 0.005 s per access. Reason: scrubbing at 10 frames per second or better on a shared machine; the limits sit 5-40x above the smoke medians for a gate with three test threads | values checked bit-exact inside each timed call |
| C8 | Peak working set (Windows, `PeakWorkingSet64`; Linux `VmHWM`) of a child process that scrubs one field of the 1M-cell store at 33 times, linear in time | peak <= 64 MiB (40 % of one 160 MB snapshot; three 8 MB field vectors plus the process) | a child that holds one whole snapshot (20 fields) exceeds the cap |
| C9 | `Mode::Linear` on fields linear in time: (A) dyadic coefficients and times, (B) random reals, irregular times; 200 queries each | A: error exactly 0; B: `|interp - (a + b t)| <= 8 eps (|a| + |b t| + |v0| + |v1|)` (a few roundings of each operand); at stored times the stored values bit-exact and not flagged | a field quadratic in time and a nearest-snapshot read violate the bound at every interpolated query |
| C10 | Retention k = 3 on 10 and 11 snapshots, k = 1 on 4 | kept steps [10,40,70,100], [10,40,70,100,110], all; bit-exact against the source | the dropped step 20 resolves to 10; k = 0 refused |
| C11 | Lookups and writer preconditions | the table in the test (ties, before/after range, equal times, NaN) | non-increasing step, decreasing or NaN time, wrong length, wrong location, duplicate name, replacing a store: refused |
| C12 | Live reader `refresh()` | picks up exactly the one new snapshot, then 0 | |
| H1 | Hook off vs on, transient `piso` cavity and steady `simple` cavity, `store_interval = 7` | off: no `fields/`; on and off: ux, uy, uz, p bit-identical, same iteration count and event count; stored steps = multiples of 7 plus the last; stored final snapshot = `Simulation::field` bit-exact | the previous stored snapshot differs from the final state |
| H2 | `store_time_interval = 0.05`, dt 0.01, end 0.3, `store_fields = [p, speed]` | 6 snapshots at times within 1e-6 relative of k x 0.05; run bit-identical to the plain run; final `speed` bit-exact | |
| H3 | Located errors and warnings | `store_time_interval` on `simple` and either key with `dynamic_mesh.enabled` are located errors; unknown field: warning, no store, run bit-identical | the same key on `piso` is accepted |
| H4 | Fresh run into a directory with a store | replaced: steps [15, 30], final bit-exact | |

Every existing test unchanged; the default path is covered by the unchanged `baseline_hashes.rs` and the full gate.

## Amendment 1 (2026-10-26, before the corrected run): C8 memory probe

The first gate run (archive of e344301) hung in `c7_c8_scrubbing_a_1m_cell_store_with_20_fields`: the scrubbing child process called
`powershell -NoProfile -Command (Get-Process -Id <pid>).PeakWorkingSet64`, and that powershell process never returned. On this machine
many `powershell.exe` processes were hung at that time (tasklist status "Unknown"), and a CIM query from my own shell hung too. The
set-up is defective: the probe depends on a shell that can hang, without a timeout. What I knew when writing this amendment: the C7
set-up had completed in that run (the 674 MB store was written and deleted is not known; the parent was blocked in the first child),
and `typeperf "\Process(scale-*)\Working Set Peak"` returned within seconds from my shell, showing the hung child at 30 089 216 bytes.

Correction (same limits, same set-up otherwise): the memory probe reads `\Process(<test exe>*)\Working Set Peak` and `\ID Process`
with `typeperf -sc 1` (the PDH performance counters, no PowerShell, no WMI) and takes the instance whose ID is the child's PID; every
probe runs under a 60 s timeout and is killed after it, so a hung probe fails the test instead of hanging the gate. The original test is
kept as `c7_c8_scrubbing_a_1m_cell_store_with_20_fields_known_miss` (ignored, limits intact, also with the timeout); the corrected one is
`..._typeperf`. Smoke of the corrected probe (debug build, before registration of this amendment): scrub peak 29.3 MB, whole-snapshot
reader 159.0 MB.

The first gate run also failed `crates/rcfd-cli/tests/workspace_graph.rs::layering_matches_architecture_doc`: the test classifies every
workspace crate and did not know `rcfd-tsstore`. Fixed in b64a412 by placing `rcfd-tsstore` in layer 4 (with `turb` and `post`; it
depends on core, mesh and io, and api depends on it) in the test and in `docs/architecture.md`. Other wave-47 crates need the same
one-line addition.
