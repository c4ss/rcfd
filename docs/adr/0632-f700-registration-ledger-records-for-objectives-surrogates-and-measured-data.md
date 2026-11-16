# ADR 0632: F-700 registration: ledger records for objectives, surrogates and multi-fidelity or measured data

Status: **REGISTERED 2026-11-16, written before any code of F-700 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-700 was written in the branch. Disclosed arithmetic done beforehand:
one Python script (`w49i-work/f700-scratch/fixture.py`, never committed) that lays out the fixtures below and lists the
expected pairs and the training matrix; it runs no Rust code and reads no ledger. Its output is quoted in "Set-ups". Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-700 (deps F-268). It implements proposals P1 to P8 of [ADR 0623](0623-f268-remaining-ledger-logging-and-reduced-order-bands.md)
(stub of 2026-11-15, commit 9cf95fc3, section "Ledger fields and queries for F-697, F-698, F-699"), for the consumers F-697
(ADR 0620), F-698 (ADR 0621), F-699 (ADR 0622) and F-701 (ADR 0624). Parents: [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md)
(the ledger), [ADR 0622](0622-f699-fidelity-management-screen-confirm-correct.md) D6 and D7 (the fidelity pair record and the
privacy split), [ADR 0136](0136-fidelity-tiers-xhigh-to-xlow.md) amendment of 2026-11-15 (points to ADR 0622). Number 0632
given by the lead. Branch `f700-work` from c4424c9b (tip of F-268's remaining scope, which adds `design/study_ledger.rs`),
worktree `w49i-work/f700`, scratch `w49i-work/f700-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run is made once;
existing tests are not edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of any golden; no
new dependency, no unsafe code; clean-room. Binding maintainer decisions of 2026-11-15 (ADR 0623 "Maintainer decisions",
`loop-drafts/decisions.md` D1 to D6): new fields are optional top-level keys of `rcfd-ledger-1`, omitted when unset (D5);
`Fidelity::Measured` in the library only, written only to private ledgers, MCP enums unchanged until F-702 (D2); strict privacy
split (D3); the pair and discrepancy record is ADR 0622's (D4); library only, no MCP tool, no case key, no golden edit (D1).
Every other choice is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked so.

## What was read

`crates/rcfd-api/src/design/ledger.rs` (all), `ledger_tests.rs`, `study_ledger.rs`, the ledger tools in `design/tools.rs`
(lines 1528 to 1705: `ledger_append`, `ledger_filter`, `ledger_query`, `ledger_export`, `ledger_rerun_check`), the three
`fidelity` enums of `crates/rcfd-mcp/src/lib.rs` (lines 437, 458, 483), ADR 0623 (current text and stub), ADR 0622 D6 to D8,
ADR 0136's amendment, ADRs 0620, 0621, 0624 where they name F-700, `loop-drafts/decisions.md`, F-700's roadmap entry.

Facts that shape the design (read in the source):

1. `Entry::to_json` writes every key, always (null for an unset `wall_s`, `seed`, `settings`, `provenance`), through
   `serde_json::json!`; serde_json is built without `preserve_order`, so keys are sorted and the text is deterministic.
2. `Entry::from_json` reads the keys it knows and ignores any other key. It rejects (the line is skipped) an unknown `verdict`
   or `fidelity` text. So a base reader keeps a line whose only novelty is new top-level keys, and skips a line with
   `"fidelity": "measured"`.
3. The MCP tools call `Fidelity::parse` for `design_ledger_append` and for the query filter, with the error texts
   "fidelity: '...' is not analytic, reduced_order or cfd" and "fidelity: '...' is unknown"; `ledger_query` returns the
   reader's skipped lines with their reasons; `indexed_json` (used by query and export) is `to_json` plus `index`.
4. Struct literals of `Entry` without `..Default::default()`: `design/tools.rs` 1551, `design/uq.rs` 552 and 760,
   `design/sensitivity.rs` 226 and 325. `Filter` is built field by field in `design/tools.rs` 1589.

## Decision

### D1. New fields (P1 to P7), optional top-level keys, omitted when unset

`Entry` gains six fields, each with an "unset" value that writes nothing:

| Key | Field | Unset | Content | Proposal |
|---|---|---|---|---|
| `parents` | `parents: Vec<EntryRef>` | empty | list of `{"index": n, "hash": "fnv1a64:<16 hex>"}`, hash optional | P1, P2 |
| `source` | `source: Option<Source>` | `None` | `{"id", "kind", "citation"?, "file_sha256"?}`, kind one of `computed`, `measured`, `literature` | P3 |
| `fidelity_level` | `fidelity_level: Option<String>` | `None` | a finer level inside the tier (an ADR 0136 tier name, a mesh level), free non-empty text | P4 |
| `uncertainty` | `uncertainty: Vec<Quantity>` | empty | standard uncertainty of named metrics, `{name, value, unit}`, same unit as the metric | P5 |
| `objective` | `objective: Option<Identity>` | `None` | `{"id", "version", "hash"}` of the objective that scored the entry | P7 |
| `fitted` | `fitted: Option<Identity>` | `None` | `{"id", "version", "hash"}` of a fitted surrogate or correction the entry records | P8 (record of a fit) |

`to_json` builds exactly the base object and then inserts only the keys that are set, so an entry with none of them set writes
the base text byte for byte (fact 1). An empty list is "unset" (registered: an explicitly empty `parents` cannot be written; it
means the same as none). `from_json` reads each key when present and refuses the line (located reason) when a present key is
malformed; a missing key is unset.

`validate` adds: each `parents[i].hash`, when given, is `fnv1a64:` and 16 lower-case hex digits; `source.id` non-empty,
`source.kind` one of the three words, `file_sha256` when given 64 lower-case hex digits (the caller computes it; no hash crate
is added and the ledger does not open the file); `fidelity_level` non-empty; each `uncertainty` name is a metric of the entry,
given once, in the metric's unit, with a finite non-negative value; `objective` and `fitted` have non-empty id, version and
hash.

**Entry reference (P1).** `EntryRef { index, hash }`. The index is the line position (ADR 0121); the hash is FNV-1a 64 of the
canonical text `Entry::to_json().to_string()`, which for any line written by `Ledger::append` is the line itself without its
newline (the reader returns entries, not raw lines; a line written by hand or by another tool may hash differently, and then
`resolve` reports a mismatch rather than guess). `entry_ref(i, &e)` builds one; `resolve(&entries, &r)` checks the index range
and, when the hash is given, that it matches, with located messages. FNV-1a 64 from its public-domain definition (Fowler, Noll,
Vo; offset basis `0xcbf29ce484222325`, prime `0x100000001b3`, quoted from memory and the same constants as
`design/fan_rom_eval.rs`); it is a change detector, not a security hash.

### D2. `Fidelity::Measured` and the privacy split (P4, D2, D3)

* `Fidelity::Measured`, text `measured`. `Fidelity::parse` keeps the three public words (so the MCP tools, which call it, keep
  their behaviour and their error texts, fact 3, and cannot append a measured entry); `Fidelity::parse_any` also accepts
  `measured`. Taken on the standing instruction (recommendation): the split of the parser is the smallest change that keeps every
  MCP answer as before.
* `Ledger::new(path)` is a public ledger, as today. `Ledger::private(path) -> Result<Ledger, String>` makes a private ledger
  and refuses, with a located error naming the ancestor, a path that lies inside a repository tree: any ancestor directory of the
  absolute path (the path joined to the current directory when relative; the nearest existing ancestor canonicalised) that
  holds `.git` or a `Cargo.toml` whose text contains `[workspace]` (ADR 0622 D7). The check is made at construction, before
  any write.
* A public ledger's `append` refuses, before writing anything, an entry that is "private content": fidelity `measured`, or a
  `source` of kind `measured`. The refusal names the entry position and the reason and contains no metric or parameter value.
* A public ledger's `read` (and `parse_text`) skips a `measured` line as the base reader does; the skip reason names the
  private ledger instead of "unknown". A private ledger's `read` (`parse_text_private`) returns it.
* Not in F-700 (it needs F-699's driver): the check that a public ledger path is not inside the private directory, the rounded
  public aggregates (3 significant digits, at least 5 pairs). They are the driver's, ADR 0622 D7.

### D3. Discrepancy record (P6, ADR 0622 D6)

`fidelity_pair(entries, low, high, metrics, floors, preds) -> Result<Entry, String>` builds the one record of ADR 0622:
kind `fidelity_pair`, evaluator `ledger.fidelity_pair` version `1`, verdict `ok`, `params` the low entry's (the two entries
must have bit-identical parameters, the pairing key of D4), per metric `m`: `m.low`, `m.high`, `m.diff = high - low` (the
metric's unit), `m.rel = diff / max(|high|, floor_m)` (unit `1`), and `m.pred` when `preds` is given; `uncertainty` carries
`m.low` and `m.high` when the source entry states one; `parents = [ref(low), ref(high)]` with hashes, so the record names both
entries; `settings` = `{"low": {evaluator, evaluator_version, fidelity, fidelity_level}, "high": {...}}`; `fidelity` is
`measured` when either side is measured, else the high side's, and `fidelity_level` the high side's; `source` the high side's
(else the low side's), so a pair with a measured source is private content and a public ledger refuses it (D2). Refused: either
entry not an `ok` evaluation, parameters not bit-identical, a metric missing on either side or in different units, a
non-finite metric, `floors` or `preds` of the wrong length, a floor not finite and positive. `provenance` is left to the caller
(ADR 0622 puts the driver, round and reason there).

### D4. Queries (P8)

* `Filter` gains `variant` and `condition` (`settings.condition` as written by the study sink of ADR 0623). Default `None`, so
  every existing filter matches as before. `group_by_variant(entries, idx)` returns `(variant, indices)` in first-seen order.
* `Level { fidelity, level: Option<String> }`. `pair_by_params(entries, idx, low, high) -> Result<Pairing, String>`: among
  `idx`, the `ok` `evaluation` entries at each level (fidelity equal and `fidelity_level` equal) are keyed by their parameters
  sorted by name: (name, unit, value bits). A low and a high entry pair when their keys are equal (same names, each value's
  64 bits equal; `0.0` and `-0.0` differ). The first entry of a key in ledger order on each side is used; later ones with the
  same key on the same side are listed in `Pairing::duplicates`. Pairs are returned in ascending low index. Refused with a
  located message: the two levels equal; a parameter name with two units among the selected entries (naming the parameter,
  both units and both entries); more than one (evaluator, evaluator_version) on one side.
* `training_set(entries, idx, params, metrics) -> Result<TrainingSet, String>`: among `idx`, the `evaluation` entries; the `ok`
  ones are rows (ledger order) with `x[r][c]` the parameter `params[c]` and `y[r][c]` the metric `metrics[c]`, in the order the
  caller gives; the non-`ok` ones are listed in `failed`. Refused with a located message: more than one evaluator, evaluator
  version, fidelity or level among them (naming both values and both entries); a parameter or metric with two units; an `ok`
  entry lacking a requested parameter or metric; empty `params` or `metrics`. `TrainingSet` carries the column names with
  units, the evaluator identity, and `refs(entries)` (references with hashes for a fit record's `parents`).
* The record of a fit (P8) is an entry with `fitted` set and `parents` = the training references; the record of a score (P7)
  is an entry of kind `objective_score` with `objective` set and `parents` = the raw entries. No new function beyond the fields
  and `refs`; F-697 and F-698 write them.

### D5. Defaults and exposure

No MCP tool, service argument or case key (D1). The edits outside `ledger.rs` are mechanical: `..Default::default()` added to
the five `Entry` literals and the `Filter` literal of fact 4, which leaves every value they build unchanged. `to_csv` is
unchanged and does not show the new keys (registered limit). `indexed_json` shows them when an entry has them (a ledger written
by the library with the new fields is then shown in full by `design_ledger_query`; no golden covers such a file).

## Set-ups (fixed now)

**F96, the pairing fixture** (96 entries, one `append`, fixed clock `1_700_000_000_000`, public ledger). Generation list (g =
0..95): L: 44 low entries `k = 0..43`, kind `evaluation`, `ok`, fidelity `reduced_order`, evaluator `rom.fan` version `1`,
params `x = (k - 23) * 0.125` [m] and `y = sqrt(k)` [1] (f64 square root), metric `q = x + y` [m3/s]. H: 24 high entries for
`k = 7 j mod 44`, `j = 0..23`, fidelity `cfd`, evaluator `cfd.fan` version `3`, the same parameter bits listed in the order
`y, x`, metric `q = x + y + 0.01` [m3/s]. N: 8 near misses (high, ok) for `k` = 2, 4, 6, 9 with `x` changed by one unit in the
last place (bits + 1) and `k` = 11, 13, 16, 18 with `y` changed likewise. Z: one high entry for `k = 23` with `x = -0.0` (the
low has `+0.0`), and three for `k` = 20, 25, 27 with an extra parameter `z = 1` [1]. D: four high `diverged` entries for `k` =
30, 32, 34, 36 (bit-identical parameters, `q` NaN, a reason). R: four high re-runs (ok, bit-identical) of `j = 0..3`. S: four
entries of kind `uq_summary`, fidelity `cfd`, with the parameters of `j = 4..7`. A: four `analytic` entries (evaluator
`analytic.fan` 1) with the parameters of `j = 8..11`. The ledger position of generation `g` is `37 g mod 96`.

Expected (from the script; registered): 24 pairs (low index, high index)
(0, 36), (2, 78), (8, 64), (9, 11), (15, 93), (17, 79), (18, 26), (24, 12), (31, 41), (37, 27), (38, 14), (40, 56), (46, 42),
(47, 85), (53, 71), (60, 4), (62, 86), (67, 33), (69, 19), (75, 5), (76, 48), (82, 34), (89, 63), (91, 49); duplicates (high
side) 51, 70, 73, 92.

**F96u, mixed units.** F96 with the unit of `x` of the H entry `j = 5` set to `mm`; and, separately, F96 with the unit of `y`
of the L entry `k = 1` set to `rad`.

**T14, the training fixture** (one append, positions 0..14). `k = 0..11`: `ok` evaluation, fidelity `reduced_order`,
evaluator `rom.fan` version `2`, params `a = 0.125 k` [m] and `b = 1 + (k mod 3)` [1], metrics `f = a a + b` [Pa] and
`g = 0.1 k` [W] (f64 products). `k = 12, 13`: same evaluator, `diverged`, params as the formula, `f` and `g` NaN. Position 14:
kind `uq_summary`, same evaluator. Query: params `["b", "a"]`, metrics `["g", "f"]`, `idx` all 15.

Registered matrix (rows = entries 0..11, columns b [1], a [m], g [W], f [Pa]; the decimal is the shortest round-trip text):

| row | b | a | g | f |
|---|---|---|---|---|
| 0 | 1.0 | 0.0 | 0.0 | 1.0 |
| 1 | 2.0 | 0.125 | 0.1 | 2.015625 |
| 2 | 3.0 | 0.25 | 0.2 | 3.0625 |
| 3 | 1.0 | 0.375 | 0.30000000000000004 | 1.140625 |
| 4 | 2.0 | 0.5 | 0.4 | 2.25 |
| 5 | 3.0 | 0.625 | 0.5 | 3.390625 |
| 6 | 1.0 | 0.75 | 0.6000000000000001 | 1.5625 |
| 7 | 2.0 | 0.875 | 0.7000000000000001 | 2.765625 |
| 8 | 3.0 | 1.0 | 0.8 | 4.0 |
| 9 | 1.0 | 1.125 | 0.9 | 2.265625 |
| 10 | 2.0 | 1.25 | 1.0 | 3.5625 |
| 11 | 3.0 | 1.375 | 1.1 | 4.890625 |

failed = [12, 13]. **T14v**: T14 plus position 15, an `ok` entry of version `3` (otherwise as `k = 0`). **T14u**: T14 with the
unit of `f` of entry 4 set to `kPa`.

**B, the default-bytes set**: the six entries of the existing check L1 (`ledger_tests.rs`, `sample_entry` with the diverged,
constraint-rejected, NaN metric and `wall_s` variants), and F96 and T14, each written by the new `Ledger` and by the base
`Ledger` to separate files with the fixed clock. **The base code** is a verbatim copy of c4424c9b's
`crates/rcfd-api/src/design/ledger.rs` lines 1 to 854 (all but its `#[cfg(test)] mod tests` declaration), committed as the test
fixture `design/ledger_base_c4424c9b.rs` and compiled as a test-only module; its identity with c4424c9b is checked with
`git show c4424c9b:... | head -n 854 | cmp` when it is copied and recorded in the addendum.

**X, lines with new fields**: entries with exactly one new field set, for each of the six fields (source of kind `computed`; fidelity `cfd` or
`reduced_order`), one entry with all six set, and one `fidelity_pair` record of F96 pair (0, 36): 8 lines, written by the new
`Ledger`.

**P, privacy**: a measured entry (fidelity `measured`, source `{id: "rig-synthetic", kind: measured}`, synthetic values), an
entry of fidelity `cfd` with a `measured` source, and the `fidelity_pair` of an L entry with a measured copy of itself; private
ledger under `std::env::temp_dir()`; repository path `CARGO_MANIFEST_DIR/target/f700_private/ledger.jsonl`.

## Checks (limits fixed now)

| Check | Clause | What | Limit |
|---|---|---|---|
| A1 | a1: no new field set gives the base bytes | set B, new `Ledger` against base `Ledger`, and per entry `to_json().to_string()` against the base's | 0 differing bytes in each of the 3 file pairs; every entry text equal |
| A2 | a2: base reader, 0 skipped lines | set X read by the base `parse_text` | 8 entries, 0 skipped; each base entry equal to the new entry's base fields (bits of every number) |
| A3 | a3: 24 pairs on 96 entries | `pair_by_params` on F96, levels (`reduced_order`, none) and (`cfd`, none) | exactly the registered 24 pairs in the registered order; duplicates exactly [51, 70, 73, 92] |
| A4 | a3: refuses mixed units | F96u, both variants | refused both times; the message names the parameter (`x` then `y`), both units and two entry indices |
| A5 | a4: discrepancy round trip, names both | `fidelity_pair` of F96 pair (2, 78) with metric `q`, floor 1e-3, a `pred`, the high entry given an uncertainty of `q` of 0.002 m3/s; appended after F96, read back | read entry equal to the written one, every number's bits equal; `parents` = [{2, h2}, {78, h78}] where `h_i` = FNV-1a 64 of line `i` of the file; `resolve` gives 2 and 78; `q.diff` and `q.rel` bit-equal to the formula of D3 computed in the test |
| A6 | a5: the registered matrix | `training_set` on T14 | rows [0..11]; columns `b [1], a [m]` and `g [W], f [Pa]`; every value's bits equal to the registered table (formula computed in the test); failed [12, 13] |
| A7 | a5: refuses mixed versions | `training_set` on T14v and T14u | T14v refused, message names versions `2` and `3` and entries 0 and 15; T14u refused, naming `f`, `Pa`, `kPa` |
| A8 | a6: measured only to a private ledger | set P | public `append` of each of the 3 entries refused, the file not created; `Ledger::private` under the temp dir accepts all 3, private `read` returns them equal; public `read` and the base `parse_text` of that file return 0 entries and 3 skipped lines; `Ledger::private` of the repository path refused with a message naming the ancestor; no value of the measured entry occurs in any refusal message |
| A9 | a7: MCP goldens unchanged | the rcfd-mcp tests `default_f1` and `project_list_f270` (tools-list goldens), and `git diff c4424c9b -- crates/rcfd-mcp` | pass; empty diff |
| A10 | defaults | existing tests: `design::` unit tests of rcfd-api (ledger, study_ledger, uq, sensitivity, study, store, tools) and the rcfd-mcp tests, release | all pass; no existing test file edited |
| A11 | D1 validation | six malformed entries (uncertainty name not a metric; uncertainty unit differs; negative uncertainty; parent hash `fnv1a64:xyz`; source kind `guess`; objective with empty hash), each by `validate` and, as a hand-written line, by the new reader | 6 of 6 refused by `validate` with a message naming the field; 6 of 6 lines skipped by the new reader |
| A12 | D4 filter | a 6-entry fixture (study sink style: 2 variants by 3 conditions, `settings.condition` `c0..c2`) | `variant` filter returns the 3 entries of the variant, `condition` filter the 2 of the condition, `group_by_variant` 2 groups in first-seen order, exact indices |
| A13 | D4 levels | 4 entries: two `cfd` at level `coarse`, two at `fine`, pairwise identical parameters; and a call with the two levels equal | 2 pairs; equal levels refused |

All checks are fast (milliseconds); none is ignored. Run in release: `cargo test -p rcfd-api --release --lib design::` and
`cargo test -p rcfd-mcp --release`.

Negative controls (inside the checks): N near misses and the signed zero must stay unpaired (A3), the D, S and A groups and the
duplicates must not pair (A3), the base reader must skip a measured line (A8).

Mutations (scratch copy of the worktree, one at a time, each must fail a registered check):

| | Mutation | Expected to fail |
|---|---|---|
| M1 | `to_json` writes `parents: []` when unset | A1 |
| M2 | pairing compares values with `==` instead of bits | A3 (the signed zero pairs: 25) |
| M3 | pairing does not check units | A4 |
| M4 | `training_set` ignores `evaluator_version` | A7 |
| M5 | public `append` does not refuse measured content | A8 |
| M6 | `Ledger::private` skips the repository check | A8 |
| M7 | `from_json` does not read `parents` | A5 |
| M8 | pairing includes non-`ok` entries | A3 |
| M9 | public reader accepts `measured` (`parse_any`) | A8 |
| M10 | `training_set` returns metric columns in ledger order instead of the caller's | A6 |

## Decision rule

**done** when A1 to A13 are met and every mutation fails a check; **partial** when A1, A2, A9 and A10 hold but another limit
is missed (an ignored `*_known_miss` record) or a mutation survives; **todo** (the lead does not take the code) when A1, A2, A9
or A10 fails.

## Limits (not covered)

- Library only: no MCP tool can write or filter by the new fields, and `measured` is not in the MCP enums (F-702).
- The entry hash is of the canonical text; a hand-edited line can hash differently and is then reported, not repaired.
- `file_sha256` is recorded as given; the ledger does not open or hash the file.
- The privacy check is a path rule, not access control (ADR 0622 risk 7): a private file copied into the repository is not
  detected; symbolic links are followed only as far as `canonicalize` of the nearest existing ancestor follows them.
- The public-path-inside-private-directory check and the rounded public aggregates belong to F-699's driver.
- `to_csv` does not show the new fields; old builds drop them on export (base `to_json`).
- Two writers, hand edits and ledger growth: as ADR 0121.

## References

- ADR 0121, ADR 0622 (D6, D7), ADR 0623 (P1 to P8, maintainer decisions), ADR 0136 (amendment).
- G. Fowler, L. C. Noll, K.-P. Vo, the FNV hash (public-domain definition; constants quoted from memory, as in ADR 0623).
- JCGM 100:2008 (standard uncertainty, the meaning of the `uncertainty` values).
- A. I. J. Forrester, A. Sobester, A. J. Keane (2007), Proc. R. Soc. A 463 (pairing cheap and costly evaluations; motivation
  only, nothing numeric is taken). Pages UNVERIFIED.

## Amendment 1 (2026-11-16, before any run of any check): the public reader and a `measured` source

Found while writing the A8 test, before any check ran: set P holds an entry of tier `cfd` with a source of kind `measured`.
D2 makes the public `append` refuse it, but D2's public reader skips only `measured` lines, and the base reader skips only an
unknown tier; so A8's "public `read` and the base `parse_text` of that file return 0 entries and 3 skipped lines" is wrong for
that entry under the text as registered (the base reader cannot know the `source` key). Corrected before any run:

- D2, public reader: a public `read` (and `parse_text`) skips every line that is private content (tier `measured` or a source
  of kind `measured`), with a reason naming the private ledger. This is the strict reading of decision D3.
- A8, corrected limit: public `read` of the private file returns 0 entries and 3 skipped lines; the base `parse_text` returns
  1 entry (the `cfd` entry with a measured source) and 2 skipped lines (the two `measured` lines). Every other clause of A8 is
  unchanged.
- Limits, added: a build older than F-700 reads an entry of a public tier with a measured source from a private file as an
  ordinary entry; the private file is outside any repository by construction, so this concerns only a user who points an old
  build at it.

## Addendum 1: results (2026-11-16)

Code: `design/ledger.rs` (D1 to D4), `design/ledger_f700_tests.rs` (A1 to A8, A11 to A13), the base copy
`design/ledger_base_c4424c9b.rs` (identity with c4424c9b's `ledger.rs` lines 1 to 854 checked with `cmp` when copied and again
after `cargo fmt`; SHA-256 `fa5786d77855bd2b88991e827bd08d5fc5541b5737ac913cb98124d5e046ccd9`), `design/tools.rs` (two
`..Default::default()`, lines 1580 and 1602, no behaviour change), `docs/design-loop.md` (one section). The registered run is
`cargo test -p rcfd-api --release --lib design::` (76 passed, 3 ignored pre-existing, 0 failed) and
`cargo test -p rcfd-mcp --release --no-fail-fast` (every test binary ok; `adopt_f288` first failed because the `rcfd` and
`rcfd-runner` binaries were not built in this target directory, its own precondition message; built with
`cargo build --release -p rcfd-cli -p rcfd-runner` and run again: 2 of 2 ok).

| Check | Limit | Measured | Met |
|---|---|---|---|
| A1 default bytes | 0 differing bytes in 3 file pairs; entry texts equal | L1 set 3704 bytes, F96 45630 bytes, T14 7283 bytes: 0 differing bytes each; every entry text equal | yes |
| A2 base reader | 8 entries, 0 skipped, base fields bit-equal | 8 entries, 0 skipped, equal; the new reader returns all six fields equal | yes |
| A3 pairing | the registered 24 pairs, duplicates [51, 70, 73, 92] | exactly the registered list and duplicates | yes |
| A4 mixed units | refused, parameter, both units, two entries named | `parameter 'x' has unit 'm' in entry 0 and 'mm' in entry 85`; `parameter 'y' has unit '1' in entry 0 and 'rad' in entry 37` | yes |
| A5 pair round trip | equal, bits equal, parents [{2, h2}, {78, h78}] with the line hashes, `resolve` 2 and 78, diff and rel by formula | all hold; hashes `fnv1a64:edd65df0c9e20f89`, `fnv1a64:e6a2fb93f88b6255` equal the test's own FNV of lines 2 and 78; a stale reference is refused | yes |
| A6 matrix | rows 0..11, columns, 48 values bit-equal to the table and the formula, failed [12, 13] | as registered, 48 of 48 | yes |
| A7 mixed versions | refused, versions 2 and 3, entries 0 and 15; units f Pa kPa | `evaluator_version is '2' in entry 0 and '3' in entry 15`; `'f' has unit 'Pa' in entry 0 and 'kPa' in entry 4` | yes |
| A8 privacy (amended) | 3 public refusals, no file; private read equal; public 0 entries 3 skipped; base 1 entry 2 skipped; repository path refused naming the ancestor; no value in messages | as registered; ancestor `\?\E:\projects\cfd\w49i-work\f700` (the canonical form on Windows) | yes |
| A9 goldens | `default_f1`, `project_list_f270` pass; empty diff in `crates/rcfd-mcp` | pass (2 and 1 tests); `git diff c4424c9b -- crates/rcfd-mcp` empty | yes |
| A10 existing tests | all pass, none edited | 76 `design::` tests and every rcfd-mcp test pass; no existing test file edited | yes |
| A11 validation | 6 of 6 by `validate`, 6 of 6 skipped by the reader | 6 of 6, 6 of 6, each message naming the field | yes |
| A12 filters | [0, 1, 3], [1, 4], two groups | as registered; `Filter::default()` matches all 6 | yes |
| A13 levels | 2 pairs; equal levels refused | [(0, 1), (2, 3)]; refused | yes |

Mutations (scratch copy `w49i-work/f700-scratch/mcopy`, `cargo test --release --lib design::ledger::f700_tests`, unmutated
copy 11 of 11 pass): M1 A1 (3213 differing bytes in the L1 set); M2 A3; M3 A4; M4 A7; M5 A8; M6 A8; M7 A5 (also A2, A8,
A11); M8 A3; M9 A8; M10 A6. Ten of ten caught. M1's first attempt did not apply (its search text predated `cargo fmt`; the
script refused and that run tested the unmutated code); its corrected text was applied and run once.

Deviations: after the registered run, `clippy -D warnings` asked for a type alias of the pairing key (`type_complexity`) and
`cargo fmt` reformatted `ledger.rs`; neither changes behaviour, and `design::ledger` (19 tests) was run once more after them to
confirm (19 of 19). `cargo fmt -p rcfd-api --check` and `cargo clippy -p rcfd-api --all-targets -- -D warnings` are clean. Not
run: `baseline_hashes` (no shared solver file changed), the rcfd-api integration tests (none constructs `Entry` or `Filter`
literally; the design unit tests cover every changed line), the whole workspace (the lead's gate).

Decision by the registered rule: **done** (A1 to A13 met, ten of ten mutations caught).

### Remaining questions

1. Repository marker. `Ledger::private` refuses an ancestor with `.git` or a `Cargo.toml` containing `[workspace]`. A user's
   project folder in another repository kind (Mercurial, a non-Rust git-less tree) is not detected. Options: (a) keep; (b) also
   refuse `.hg`, `.svn` and a project-mode marker of ADR 0185. Recommendation: (b) as a small follow-up when F-699 lands, since
   it decides where measured data lives.
2. Error text of the repository refusal shows Windows' verbatim prefix (`\?\E:\...`). Options: (a) keep; (b) strip the
   prefix for display. Recommendation: (a) now; it is exact and located.
3. MCP. `design_ledger_query` now shows the new keys of a library-written ledger (through `indexed_json`); nothing can write
   them through MCP. Options: (a) leave to F-702; (b) hide them until F-702. Recommendation: (a); no golden covers such a file
   and showing what is in the file is the reader's job.

## For the lead: roadmap entries

> Proposed status: done

> Split from F-268 by the maintainer on 2026-11-15 (ADR 0623 Q1 b). ADR 0632, 2026-11-16. DONE. Library only (no MCP tool, case key or golden edit): rcfd-ledger-1 gains optional top-level keys parents (references with an FNV-1a 64 hash of the line), source (computed, measured or literature, citation, file SHA-256), fidelity_level, uncertainty (per metric, same unit), objective and fitted ({id, version, hash}), omitted when unset; Fidelity::Measured and any entry with a measured source are written only to a private ledger (Ledger::private, refused inside a repository tree) and refused by a public one, whose reader skips them; the MCP tools keep the three public tiers. ledger::fidelity_pair writes the ADR 0622 pair and discrepancy record naming both entries; pair_by_params, training_set, group_by_variant and the variant and condition filters are the P8 queries. Checks: the default ledger is byte-identical to the base writer on three fixtures (0 differing bytes); 8 lines with the new fields read by the verbatim base reader with 0 skipped; exactly the registered 24 pairs and 4 duplicates on the 96-entry fixture, mixed units refused; the pair record round-trips bit for bit with both parents resolved; the 12 by 4 training matrix bit for bit with the failed points apart, mixed versions and units refused; measured content refused by a public ledger, kept by a private one outside the repository; MCP tools-list goldens unchanged. Ten of ten mutations caught. Not covered: the public-path-inside-private check and the rounded public aggregates (F-699's driver), to_csv does not show the new keys.

> Follow-up proposed: Private-ledger repository markers beyond git and Cargo. Deps F-700, F-699. Acceptance: Ledger::private refuses a path under a directory holding .hg, .svn or the project-mode marker of ADR 0185, with a located message, and every ADR 0632 check keeps its result.
