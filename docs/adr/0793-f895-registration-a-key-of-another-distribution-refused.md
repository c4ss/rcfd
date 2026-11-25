# ADR 0793: F-895 registration: `design_uq` and `design_sobol` refuse a key of another distribution in `dist`

Status: **REGISTERED 2026-11-25, written before any code of F-895 and before any run of its checks.** Before this text no
test or library code was written, no file of the repository was changed and nothing was built or run. What was done:
reading (below) and searches of the source tree; after the text was written, the link check `docs_site` the brief asks for,
on this text. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f895-dist-foreign-keys.md` ([the file](briefs/f895-dist-foreign-keys.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f895-dist-foreign-keys` from main bc6ddd16,
scratch `w49i-work/f895-dist-foreign-keys-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-895 in `docs/roadmap.json`: "Every limit not stated here is registered in the ADR before the run. In
params[i].dist of design_uq and design_sobol a key that the named type does not have (mean, sd, a or b of a uniform; a or b
of a normal; mean or sd of a beta), with any value, null among them, is refused with the invalid-arguments code, the message
naming the key by its path and the keys of the type, before any sample, and no file is written; every form Dist::to_json
writes and every call without such a key gives results bit-identical to main; each of the eight wrong acceptances is caught
by a registered test under a mutation; no golden and no tool-list text changes."

The maintainer's word of 2026-11-25 and the lead's decisions are in the brief: the key is refused whatever its value, a key
of no distribution keeps its present message, a type that is not known is refused first, as now. The sentence this work must
be able to write: each of the eight foreign keys is refused by name in both tools with nothing written, and every other call
gives main's bytes.

## What was read

`docs/process/agent-rules.md` (version 5, whole); `AGENTS.md`; the brief (whole);
[ADR 0791](0791-f339-registration-arguments-of-design-uq-and-design-sobol.md) (whole);
[ADR 0792](0792-f894-registration-a-normal-distribution-open-on-one-side.md) (to the end of addendum 2); the item F-895 (by a
script); `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs` and `f894_one_open_side_tools.rs` (whole), the refusals of
`design_uq_sobol_f339.rs`, `crates/rcfd-api/tests/f894_bit_identity.rs` (whole), `crates/rcfd-mcp/tests/common/mod.rs`; of
`crates/rcfd-api/src/design/tools.rs` the helpers, `parse_robust`, `parse_method`, `parse_study`, `parse_axes`, and from
`object_at` to the end; `Dist` in `uq.rs`; of `crates/rcfd-mcp/src/lib.rs` the two descriptors, `obj`, `validate_args` and the
two arms of `dispatch`; the section on the two tools of [docs/mcp-tools.md](../mcp-tools.md); the
[tolerance table](../process/tolerances.md).

## Findings of the reading (before any run)

1. **What reaches the handler.** `params` is `{"type": "array", "items": {"type": "object"}}` in both descriptors, so
   `validate_args` looks at no key of an input: every key and value of `dist` reaches `study_params` unchanged.
   `study_params` is called by `study_call`, the first call of both handlers: before `stop`, `n`, the check of the spec, the
   limit, the reading of the ledger and the driver.
2. **No existing test sends a foreign key.** Every `dist` literal of `crates/rcfd-mcp/tests` and the added keys of B2 and B4
   of ADR 0791 were read: none. Nothing else in the tree sends one (`dist` is read in `tools.rs` only).
3. **The map of `serde_json` is sorted** (`preserve_order` is not a feature of the workspace, `Cargo.lock`), so `object_at`
   names the alphabetically first unknown key. The rule for two foreign keys below does not rest on it.
4. **The other readers of a tagged object in `tools.rs`** (reported, not changed). `dist` is read in `study_params` alone.
   `fan_rom_model` reads no tagged object (`family` changes no key). The same weakness, a known key of another kind taken and
   ignored, is in three readers of the `design.*` service methods, which do not use `object_at` and so ignore an unknown key
   too: `parse_robust` (`k` beside a `robust` that is not `mean_plus_k_sigma`, `weights` beside one that is not `weighted`);
   `parse_method` (`budget`, `restarts`, `pop`, `levels`, `n` and `seed` with a method that does not read them; four keys are
   refused with another method, ADR 0209, 0220 and 0222, but not when `null`); `parse_axes` (`start`, `stop`, `count` beside
   `values`; `axes` and `serpentine` beside `points`). At the top level the server refuses an argument of the other tool
   (`additionalProperties: false`); the handlers called directly do not.
5. **The documentation** gives the keys of each type and does not say that another is refused; the sentence on errors names a
   setting that is "missing, unknown or invalid".

## Design (registered)

In `study_params`, after `object_at` has refused a key of no distribution (its call, list and message unchanged) and before a
number of the distribution is read:

- the keys of the named type, in the order of the documentation: uniform `type, lo, hi`; normal `type, mean, sd, lo, hi`;
  beta `type, a, b, lo, hi`;
- of the list `type, lo, hi, mean, sd, a, b` (the present list, in this order) the first key that is in the object and not one
  of the type is refused with -32602 and the message
  `params[<i>].dist.<key>: not a key of a <type> distribution (<keys of the type, separated by ", ">)`;
- a type that is not known has no keys of its own to compare with and falls to the present refusal, unchanged.

A key is there if the object has it, whatever its value. What is fixed by this text: for two foreign keys of one input the
first of `mean, sd, a, b` is named, whatever the order in the call and the kind of the map; for foreign keys in two inputs
the first input's; a key of no distribution comes before a foreign key and before an unknown type (as now); a type that is
not known comes before a foreign key; a foreign key comes before a missing or malformed number of its own type (on main the
number was named: such a call was and is refused). One sentence of `docs/mcp-tools.md` is made to say it. The tool-list text
is a golden and is not changed.

## Set-up of the checks (fixed here, not searched for)

`crates/rcfd-mcp/tests/design_dist_keys_f895.rs`, in-process `McpServer` ("server") and `uq_study` / `sobol_study` directly
("handler"), over a sandbox root. Fan F(`naca4`), model Mo and condition of ADR 0791; seed 895001; `design_uq` with `batch 5`,
`max_samples 10`, `design_sobol` with `n 3`, study name `s`. Inputs: U, `tip_gap_delta` (m) uniform `[-1.5e-4, 2.5e-4]`; N,
`stagger_common_deg` (deg) normal (0.1, 0.3) on `[-0.2, 0.25]`; B, `chord_rel` (1) beta (2, 3.5) on `[-0.012, 0.018]`. Pairs
of two inputs: a uniform beside N, a normal beside B, a beta beside N. Guard of every accepted study: at least two usable
samples or rows; a guard that fails is a defect of the set-up.

"Equal" is as in ADR 0791: the reply's `result` as text is the library's `result_json` as text for the hand-built spec, and
the ledger file is the library's line by line with the stamp set to 0. "Main" is the record of the same tests made on the
unchanged library of bc6ddd16 before the change and committed with them, as a second commit: for each call the FNV-1a hash
of the text of `result` and the ledger lines (stamp 0), and their number; for refusals the hash of code and message. The
record is of one mathematical library, so there is one per platform, as ADR 0792, amendment 1, has it: `.tsv` (Windows
x86_64) and `.linux.tsv` (Linux x86_64, in the image of
[ADR 0150](0150-job48p-linux-example-stores.md), offline, from an export of the second commit, in two runs that must be
equal); each is asserted on its platform. On another platform the names and counts and the refusals (no call of the
mathematical library) are asserted, and "equal" holds the rest. If the container cannot build, no Linux record is
registered, it is said, and it is not a miss.

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| K1 | foreign key | `mean` in a uniform: number (0.25), `null`, string (`"0.5"`), `false`, `[]`, `{}`; both tools; server and handler; in the first and in the second of two inputs (48 calls) | each refused: -32602, the whole message as registered, the sandbox empty after it | acceptance of F-895 | refused | exact | code verification |
| K2 | foreign key | `sd` in a uniform, the same 48 calls | as K1 | acceptance of F-895 | refused | exact | code verification |
| K3 | foreign key | `a` in a uniform, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| K4 | foreign key | `b` in a uniform, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| K5 | foreign key | `a` in a normal, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| K6 | foreign key | `b` in a normal, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| K7 | foreign key | `mean` in a beta, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| K8 | foreign key | `sd` in a beta, the same | as K1 | acceptance of F-895 | refused | exact | code verification |
| C | control of K | the six ordered pairs without the key, both tools (12 calls) | accepted, equal to the library | acceptance of F-895 | equal | exact | code verification |
| T | which is named | two foreign keys: a uniform with each pair of `mean, sd, a, b` (6) and with all four, a normal with `a, b`, a beta with `mean, sd`, the keys in the call in either order of the text; foreign keys in both inputs; a foreign key beside a missing and a malformed number of the type (`lo` absent, `lo: "x"`, `sd` of a normal absent, `a` of a beta `null`); both tools, server and handler | the registered key and whole message, -32602, nothing written | acceptance of F-895; the design above (the brief: fixed by the rule) | as stated | exact | code verification |
| U | what keeps its message | a key of no distribution (`zz`, `Mean`, `min`) in each type, alone and beside a foreign key, and beside an unknown type; a type that is not known (absent, `null`, 3, `"Uniform"`, `"triangular"`, `""`), alone and beside each of `mean, sd, a, b`; `dist` absent, `null`, a string, a list; `lo` a string in each type and absent in a uniform and a beta; both tools | code and whole message as main's literal text and as the record of main, nothing written | acceptance of F-895 (bit-identical to main); the brief | equal | exact | unchanged default output |
| N1 | before any sample | a service with `max_design_evaluations` 1: each of the eight keys (`null`), both tools, refused as in K, not with 1004; control: without the key 1004 | as stated, nothing written | acceptance of F-895 (before any sample): the limit is checked before the driver is called | as stated | exact | code verification |
| N2 | no file written | a ledger that already holds the study `s`, written by an accepted call: each of the eight keys (a number), both tools; the ledger's bytes and the list of files unchanged; a ledger in a folder that does not exist: no folder made; control: without the key the `name:` refusal | as stated | acceptance of F-895 (no file is written) | as stated | exact | code verification |
| F1 | forms of `to_json` | the six forms (uniform, beta, normal with two finite, an open lower, an open upper and two open bounds) as `uq::result_json` writes them in `spec.params`, sent back; both tools (12 calls) | equal to the library, and to main | acceptance of F-895 (bit-identical to main); tolerances, bit-identity | equal | exact | unchanged default output |
| F2 | forms of the open sides | N with `lo` in {absent, `null`, `"-inf"`, -0.2} and `hi` in {absent, `null`, `"inf"`, 0.25}, both tools (32 calls) | equal to the library, and to main | acceptance of F-895; tolerances, bit-identity | equal | exact | unchanged default output |
| F3 | both drivers | U, N and B in one study: `design_uq` by Monte Carlo and by `lhs` (`min_batches 2`), `design_sobol` (3 calls) | equal to the library, and to main | acceptance of F-895; tolerances, bit-identity | equal | exact | unchanged default output |
| L | main on Linux x86_64 | the file on an export of the last code commit in the container | two records of main equal; the file passes; the dumps equal main's byte for byte | acceptance of F-895 (bit-identical to main); tolerances, bit-identity | equal | exact | unchanged default output |
| X | rule 3 | unedited: `design_uq_sobol_args_f339`, `design_uq_sobol_f339`, `f894_one_open_side_tools`, `cargo test -p rcfd-mcp --release` (the tool-list goldens among them), `cargo test -p rcfd-api --release --lib` | pass | rule 3; rule 7 | pass | exact | unchanged default output |

No limit on time is set. No check is of kind validation: the acceptance claims no agreement with a measured case.

### Mutations (rule 16)

Each alone in `tools.rs` on a scratch copy of the final code commit with a target folder of its own, the new test file run
once, the file restored; the unchanged copy is run too. A mutation the compiler refuses is replaced by an amendment.

| Id | Change | Must fail |
|---|---|---|
| M1 | `mean` of a uniform accepted again | K1 |
| M2 | `sd` of a uniform accepted again | K2 |
| M3 | `a` of a uniform accepted again | K3 |
| M4 | `b` of a uniform accepted again | K4 |
| M5 | `a` of a normal accepted again | K5 |
| M6 | `b` of a normal accepted again | K6 |
| M7 | `mean` of a beta accepted again | K7 |
| M8 | `sd` of a beta accepted again | K8 |
| M9 | the check of `design_uq` after the study (`uq::run` has written the ledger) | K1 to K8, N1, N2 |
| M10 | the check of `design_sobol` after the study | K1 to K8, N1, N2 |
| M11 | the message without the path (`<key>: not a key of ...`) | K1 to K8 |
| M12 | the message without the keys of the type | K1 to K8 |
| M13 | a foreign key with `null` accepted | K1 to K8 |
| M14 | a foreign key refused only with a number | K1 to K8 |
| M15 | the check of the first input only | K1 to K8 |
| M16 | the last of two foreign keys named | T |
| M17 | the alphabetically first of two foreign keys named | T |
| M18 | a type that is not known has no key but `type`, so a foreign key is named first | U |
| M19 | the key of no distribution refused by the list of the type (another message) | U |
| M20 | the refusal with the code 1004 | K1 to K8 |
| M21 | `lo` and `hi` not keys of a normal | F1, F2 |
| M22 | the check between the limit and the driver (both tools) | N1 |
| M23 | the check after the numbers of the type are read | T |

The committed one is M1, `crates/rcfd-mcp/tests/design_dist_keys_f895.mutation.patch`, whose first line is
`# expect-fail: cargo test --release -p rcfd-mcp --test design_dist_keys_f895`.

Size (agent): done; 0; 0; 0; 300000

## Decision rule

**done** if every registered row holds, each mutation is caught by its row, the named existing tests pass unedited and the
results that must not move are bit-identical to main; **partial** otherwise. (The brief's rule. Where a mutation names several
rows, it is caught if each of them fails.)

## What is not covered

- The readers of finding 4; a change of `object_at`, of `Dist`, of the server's argument check or of the tool-list text.
- The stdio transport (rows K to F go through `McpServer::handle`, which the binary calls with each line).
- A key given twice in the JSON text: the parser keeps the last, and the handler cannot see the first.
- The accuracy of the drivers and of the fan, which are unchanged.

## Amendment 1 (2026-11-25, before the second run of row X)

Written after the registered run of the new file (at 153674f0) and after the first run of the part
`cargo test -p rcfd-mcp --release` of row X (at 980455f7), and committed before its second run. No limit, row, test or
mutation is changed.

**The first run of that part is void, by its set-up.** The target folder of this work was new, and its `release` folder held
`rcfd-mcp` and not the binaries `rcfd` and `rcfd-runner` that the project tests of the crate need beside it. Two tests of the
second test binary stopped at the check of `common/mod.rs:147` that says so ("missing: the project run tools need it next to
rcfd-mcp (cargo build -p rcfd-cli -p rcfd-runner)") before they called anything, and cargo ran no further test binary. It
says nothing of the change, and it ran 18 of the crate's tests. The set-up is corrected as that message says:
`cargo build --release -p rcfd-cli -p rcfd-runner` in the same target folder, then the part is run once, whole, with
`--no-fail-fast` so that one failure cannot hide another. What fails in that run is recorded as it comes out. The other parts of row X
were run and are not run again for it: `design_uq_sobol_args_f339`, `design_uq_sobol_f339`, `f894_one_open_side_tools`,
`rcfd-api --lib`.

**Rows L and M.** The mutations are run on an export of 980455f7, which adds only the patch file to 153674f0; row L too.

## Addendum 1: results (2026-11-25)

Commits: registration 326c8980; the record of main on Windows 1596c4a2, its row U made whole added868, the record of main on
Linux 48f05450 (`tools.rs` is main's in these three, by its blob); the change and the rows 153674f0; the committed mutation
980455f7; amendment 1 4481ec66. The registered run was made once, at 153674f0:
`cargo test -p rcfd-mcp --release --test design_dist_keys_f895 -- --nocapture --test-threads 1`, 16 passed, 0 failed, 27.9 s
in series.

| Id | Limit | Measured | Met |
|---|---|---|---|
| K1 | refused, whole message, nothing written | `mean` in a uniform: 48 calls, -32602, the registered message, the sandbox empty after each | yes |
| K2 | as K1 | `sd` in a uniform: 48 calls | yes |
| K3 | as K1 | `a` in a uniform: 48 calls | yes |
| K4 | as K1 | `b` in a uniform: 48 calls | yes |
| K5 | as K1 | `a` in a normal: 48 calls | yes |
| K6 | as K1 | `b` in a normal: 48 calls | yes |
| K7 | as K1 | `mean` in a beta: 48 calls | yes |
| K8 | as K1 | `sd` in a beta: 48 calls | yes |
| C | accepted, equal to the library | 12 calls equal, result and ledger | yes |
| T | the registered key and message | 168 calls | yes |
| U | main's code and message | 102 inputs, 408 calls as main's literal text; 204 groups equal to the record of main | yes |
| N1 | refused, not 1004 | 32 calls refused before the limit of 1 evaluation; without the key 1004; nothing written | yes |
| N2 | nothing written | 64 calls left two ledgers byte for byte and made no folder; without the key the `name:` refusal | yes |
| F1 | equal to the library and to main | 12 calls, 0 differ | yes |
| F2 | equal to the library and to main | 32 calls, 0 differ | yes |
| F3 | equal to the library and to main | 3 calls, 0 differ | yes |
| L | two records equal, the file passes, dumps equal | Linux x86_64 (the image of ADR 0150, glibc 2.39, cargo 1.99.0, no network): the two records of main equal; at 980455f7 16 passed and the eight dump files equal main's byte for byte | yes |
| X | pass, unedited | `design_uq_sobol_args_f339` 16 passed, 1 ignored (K1 of ADR 0791); `design_uq_sobol_f339` 11 passed; `f894_one_open_side_tools` 4 passed; `cargo test -p rcfd-mcp --release --no-fail-fast` 164 passed, 0 failed, 11 ignored over 38 test results, the tool-list golden among them (second run, amendment 1); `rcfd-api --lib` 160 passed, 4 ignored; no existing test and no golden edited | yes |

On Windows too the eight dump files of main (made twice, equal) and of the changed library are equal byte for byte
(`main_win1`, `fix_win` of the scratch folder). All 47 hashes of F differ between the two platforms' records; row U is the
same text on both. `tools.rs` gains 20 lines and loses 3, all in `study_params` and its doc comment; `docs/mcp-tools.md` by one sentence.

### Mutations

On a copy of 980455f7 with a target folder of its own, each alone, the file restored after (compared with the branch's: equal);
the unchanged copy passes (16). All 23 are caught by the rows registered for them (`mutate.py`, `mutations.tsv` and a log each
in the scratch folder).

| Id | Registered to fail | Failed |
|---|---|---|
| M1 | K1 | K1, K2, K3, K4, N1, N2, T |
| M2 | K2 | K1, K2, K3, K4, N1, N2, T |
| M3 | K3 | K1, K2, K3, K4, N1, N2, T |
| M4 | K4 | K1, K2, K3, K4, N1, N2, T |
| M5 | K5 | K5, K6, N1, N2, T |
| M6 | K6 | K5, K6, N1, N2, T |
| M7 | K7 | K7, K8, N1, N2, T |
| M8 | K8 | K7, K8, N1, N2, T |
| M9 | K1 to K8, N1, N2 | those, T, U |
| M10 | K1 to K8, N1, N2 | those, T, U |
| M11 | K1 to K8 | those, N1, N2, T |
| M12 | K1 to K8 | those, N1, N2, T |
| M13 | K1 to K8 | those, N1 |
| M14 | K1 to K8 | those, N1 |
| M15 | K1 to K8 | those |
| M16 | T | T |
| M17 | T | T |
| M18 | U | U |
| M19 | U | U, K1 to K8, N1, N2, T |
| M20 | K1 to K8 | those, N1, N2, T |
| M21 | F1, F2 | all 16 |
| M22 | N1 | N1, N2, T |
| M23 | T | T |

Read in the logs: in M1 to M8 the row of the key fails because the call is accepted (its log holds the study's reply, 11
entries); the neighbouring keys of the type fail by the list of keys in the message. In M9 and M10 the K rows fail because the
refused call left `l.jsonl` (seen for K1 and K8 of each), N1 by 1004 and N2 by the `name:` refusal; U fails in them because
the mutation's check in the other tool comes before `object_at`. The committed mutation is M1,
`crates/rcfd-mcp/tests/design_dist_keys_f895.mutation.patch`; `git apply --check` accepts it.

### Other runs, deviations, what was not run

- `cargo clippy -p rcfd-api -p rcfd-mcp --all-targets -- -D warnings` clean. `cargo fmt -p rcfd-api -- --check` and
  `cargo fmt -p rcfd-mcp -- --check` clean on Windows; the two crates in one command do not run there ("The filename or
  extension is too long", os error 206: the list of files on one command line), in the worktree and in a shorter folder; on
  Linux, in the container, the one command is clean (exit 0, on the export of 980455f7).
- `docs_site`: 2 passed before the registration's commit; the run with this addendum is in the agent's report.
- Deviations: (1) amendment 1, the set-up of one part of X. Its text says `rcfd` and `rcfd-runner` were missing and "the second
  test binary": only `rcfd-runner` was (`rcfd` came from `docs_site`), and it was the third binary (the first integration test).
  (2) The first record (1596c4a2) had row U with fewer combinations than registered (an unknown type beside each key); it
  was made whole and recorded again on main's library before the change (added868), the 47 groups of F unchanged; a first
  container run, stopped before it had built, is void for the same reason. (3) clippy refused two comparisons of the test file
  (`cmp_owned`); rewritten before the registered run. (4) The runs that made the records fail by design (no record to compare
  with). (5) The folders of the first, incomplete record were removed from the scratch folder.
- Finding 4 (other readers of a tagged object) stands as reported; nothing of it was changed. It is proposed to the lead below
  as an observation, not as a follow-up.
- The gate runs the new file's 16 tests (about 28 s in series, less with the default threads). No heavy record. Not run: the
  whole workspace, the integration tests of any crate but `rcfd-mcp`, `baseline_hashes` (no solver file touched), the Studio's tests.

Size (agent), actual: done; 0; 0; 0; 340000

### Decision

**done** by the registered rule: every registered row holds, M1 to M23 are each caught by the registered rows, the named
existing tests pass unedited, and the results that must not move are bit-identical to main on Windows x86_64 and on Linux
x86_64. The sentence asked for holds: each of the eight foreign keys is refused by name in both tools with nothing written,
and every other call registered here gives main's bytes.

## Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed: done. No follow-up is proposed; finding 4 is for the lead to weigh: `parse_robust`, `parse_method` and
`parse_axes` of the `design.*` methods take a known key of another kind, and an unknown key, and ignore it.

> ADR 0793, 2026-11-25, done: in params[i].dist of design_uq and design_sobol a key that the named type does not have (mean, sd, a or b of a uniform; a or b of a normal; mean or sd of a beta) is refused with -32602, whatever its value, the message naming the key by its path and the keys of the type, before the limit and the driver and with nothing written: 384 calls over the eight keys (number, null, string and three other values, both tools, through the server and the handler, in the first and the second input); of several the first of mean, sd, a, b is named; a key of no distribution, a type that is not known and a malformed number keep code and message; the six forms of Dist::to_json, the sixteen forms of the open sides and a study of three inputs give the library's and main's bytes on Windows x86_64 and Linux x86_64; 23 registered mutations, the eight wrong acceptances among them, are each caught by the registered rows, one is committed; no golden, tool-list text or existing test is changed.

The same entry as `docs/roadmap/proposals/0793-F-895.json`.

## The lead's reading (2026-11-25), at the merge

Merged at 0fea72e9 on the maintainer's answer "Yes, merge it", since the work ran in a session of its own, and pushed at
c5570767 (the report read at 17:19 UTC, 12 minutes of reading, proposed done, written done). Checked by the lead before the
merge: the registration is the first commit and alone; the record of main is made before the change, in three commits in
which `tools.rs` is main's; amendment 1 comes before the second run of one part of row X and states the set-up that voided the
first; of the existing files only `study_params` and one sentence of `docs/mcp-tools.md` are changed, and no golden or
existing test, so no reader was issued. A call with a foreign key was accepted and is now refused, which is what the item
asks. On the merged tree the lead ran the new file (16 passed), `design_uq_sobol_args_f339` (16) and
`f894_one_open_side_tools` (4). The Linux record and the 23 mutations are the session's; the lead did not repeat them beyond
the three patches of the push run, which were caught. 127 binaries were left out for a left-out run.

The session was started with the brief's `/goal` line, which was not set as a command; it took 74 turns, past the line's 60,
which nothing enforced. The line also asked for the named tests among the final outputs, so the new file ran again at the
end, after its registered run (deviation 4 of the report): a goal line asks from now on for the output of the registered
run and not for another.

Status: **done**, as proposed. Finding 4 (`parse_robust`, `parse_method` and `parse_axes` ignore a known key of another kind
and an unknown key) is a question of scope for the maintainer and is not entered.
