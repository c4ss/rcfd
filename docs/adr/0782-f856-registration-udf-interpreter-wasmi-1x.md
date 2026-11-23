# ADR 0782: F-856 registration: the UDF interpreter from wasmi 0.31 to wasmi 1.1.0 (the minimum Rust read first, the port of `udf.rs`, and the checks of the fuel limit, the memory limit and the refusal of imports)

Status: **REGISTERED 2026-11-23, written before any code of F-856 and before any run of its checks.** Before this text no
library or test code of F-856 was written and no file of the repository was changed. What was done: reading (below), one
query of the crates.io index, and a throwaway project in the scratch folder that resolves `wasmi = "1"` so that its
source could be read. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f856-wasmi-1x.md` ([the file](briefs/f856-wasmi-1x.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f856-wasmi-1x` from main e2fe571b, scratch
`w49i-work/f856-wasmi-1x-scratch`.

## What is asked

Roadmap item F-856 (no deps), the binding acceptance:

> Every limit not stated here is registered in the ADR before the run. The workspace depends on wasmi 1.x (the newest 1.x release at the time of the registration) and crates/rcfd-api/src/udf.rs is ported to its interface; cargo clippy --all-targets with warnings as errors, the type-check with the README's minimum Rust and the unit tests pass; every existing UDF test of rcfd-api (udf, udf_hooks_w38a) and rcfd-cli check_inputs_adr0739 passes unedited, and baseline_hashes is unchanged; the fuel or step limit, the memory limit and the refusal of a module that imports anything not offered behave as before, each shown by an existing or a new test; cargo deny passes. If the minimum Rust of wasmi 1.x is above the README's minimum, that is reported to the lead before any code and not decided by the agent.

Clauses: **c1** the workspace depends on the newest wasmi 1.x and `udf.rs` is ported; **c2** clippy with warnings as
errors; **c3** the type-check with Rust 1.90; **c4** the unit tests; **c5** the three existing test files pass unedited;
**c6** `baseline_hashes` unchanged; **c7** the fuel limit behaves as before; **c8** the memory limit behaves as before;
**c9** a module that imports anything is refused as before; **c10** `cargo deny` passes; **c11** the minimum Rust is read
before any code.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); `crates/rcfd-api/src/udf.rs` (whole);
`crates/rcfd-api/tests/udf.rs` (the header and the tests of isolation, runaway modules and failing runs);
`crates/rcfd-api/tests/udf_hooks_w38a.rs` (the list of its tests); `crates/rcfd-cli/tests/check_inputs_adr0739.rs` (its
uses of a `.wasm` file); `deny.toml`; `Cargo.toml`, `crates/rcfd-api/Cargo.toml` and the wasmi entries of `Cargo.lock`;
the README's section Build and test; the jobs `min-toolchain`, `deny` and the licence list step of
`.github/workflows/ci.yml`; the limits section of [docs/udf-abi.md](../udf-abi.md); item F-856 of `docs/roadmap.json`
(by a script).

Of wasmi, as the crate's own source shows it (a dependency's interface, not a source of an algorithm), in the cargo
registry copy of the published crates: wasmi 1.1.0 `Cargo.toml`, `src/lib.rs` (the exported names),
`src/engine/config.rs`, `src/store/mod.rs` (`limiter`, `get_fuel`, `set_fuel`), `src/limits.rs`, `src/linker.rs`
(`instantiate_and_start`), `src/func/into_func.rs` (the types a typed function takes), `src/engine/code_map.rs` (fuel
charged for translation on first use); wasmi_core 1.1.0 `src/memory/mod.rs` (where the limiter is asked); wasmi 0.31.2
`src/engine/config.rs` (the defaults of the configuration and of the fuel costs), `src/limits.rs` and
`src/memory/mod.rs`.

## Step 1: the minimum Rust (c11)

The crates.io index (`https://crates.io/api/v1/crates/wasmi/versions`, read 2026-11-23) lists as the newest release of
major version 1: **wasmi 1.1.0** (2026-06-12, not yanked, licence `MIT/Apache-2.0`), with `rust-version` **1.86**. Every
1.x release from 1.0.0 declares 1.86. The newest release of all is 2.0.2 (2026-11-19, also 1.86); the item names 1.x and
this work stays there.

The closure of `wasmi = "1"`, resolved in the scratch project (`cargo metadata`): wasmi, wasmi_core, wasmi_collections
and wasmi_ir 1.1.0 declare 1.86; wasmparser 0.239.0 declares 1.76; spin, libm and bitflags are lower. With wasmi's
default features the crate `wat` (1.88) becomes a normal dependency; this work switches the default features off (D1), so
the highest minimum that the port adds to the normal dependencies is **1.86**.

1.86 is not above the README's minimum for the type-check (1.90). The work is not stopped. One consequence lies outside
the files this work owns and is handed to the lead in the addendum: `crates/rcfd-api/Cargo.toml` states
`rust-version = "1.85"` and the README says the crates' own minimums run from 1.75 to 1.85; with wasmi 1.1.0 the
closure of rcfd-api needs 1.86.

## Design

D1. `Cargo.toml`: `wasmi = { version = "1.1", default-features = false, features = ["std"] }`. The feature `wat` is
left out because with it `Module::new` also accepts the text format, which 0.31 never did: a `.wasm` file holding text
would be loaded where it is refused today.

D2. The engine's configuration keeps the set of accepted modules of 0.31.2. wasmi 0.31.2 defaults
(`src/engine/config.rs`, `impl Default for Config`): mutable-global, sign-extension, saturating-float-to-int,
multi-value, bulk-memory, reference-types and floats on; tail-call and extended-const off; it has no switch for
multi-memory or memory64. wasmi 1.1.0 defaults (`Config::default_features`) switch on multi-memory, memory64, tail-call
and extended-const in addition. The port switches these four off (`wasm_multi_memory`, `wasm_memory64`,
`wasm_tail_call`, `wasm_extended_const`, each `false`). Custom page sizes and wide arithmetic are off by default and
SIMD needs a crate feature that is not enabled. Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation): a change of interpreter is not a change of ABI 1.

D3. Translation is eager (`CompilationMode::Eager`). The default of 1.1.0 is lazy translation, which translates a
function at its first call and charges that call's fuel for it (`src/engine/code_map.rs`); 0.31 translated the whole
module at load. Eager keeps the cost of a call independent of whether it is the first.

D4. Fuel. 0.31 counted the fuel consumed, and `udf.rs` topped the store up so that the remaining fuel was
`fuel_per_call` before every call: `left = added - consumed` never exceeds the cap, so after the top-up the remaining
fuel is exactly the cap. 1.1.0 holds the remaining fuel (`Store::get_fuel`, `Store::set_fuel`). The port sets the
remaining fuel to `fuel_per_call` at the same places (before instantiation, before every call or group of three
component calls) and records as the cost `fuel_per_call - get_fuel()` after the call. The field `fuel_added` goes.
The unit of fuel is the interpreter's own; the numbers of one call under the two versions are measured and recorded
(F1 below), with no limit on them.

D5. Limits of the store: the same builder calls with the same values (memory 16 MiB, one instance, one memory, one
table of 10 000 elements, trap on a failed growth). `StoreLimits::memory_growing` has the same rule in both versions
(refuse when the desired size exceeds the limit).

D6. Imports: the same refusal before instantiation, by `Module::imports()`, with the same message. Instantiation by
`Linker::instantiate_and_start` with an empty linker. Typed functions take `f64` directly; the wrapper type goes.

Nothing of the solver is touched. No case of `baseline_hashes` is expected to change.

## Checks

New test file `crates/rcfd-api/tests/udf_limits_adr0782.rs`, written against the public interface of `rcfd_api::udf`
alone. It is first run once on the unported tree (wasmi 0.31.2) as a control, to show the behaviour "before", and then
once on the port. The limits bind the run on the port; a test that differs on 0.31.2 is reported with its output.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- | --- |
| V1 | c1, c11 | `Cargo.lock` after the port | wasmi 1.1.0 and no wasmi 0.31 | acceptance of F-856 | 1.1.0 | exact | code verification |
| Q1 | c2 | `cargo clippy -p rcfd-api --all-targets -- -D warnings` and `cargo fmt -p rcfd-api --check` | no finding | acceptance of F-856 | none | exact | code verification |
| Q2 | c3 | `cargo check --workspace --all-targets --locked` with Rust 1.90, if that toolchain is installed | passes | acceptance of F-856 | passes | exact | code verification |
| Q3 | c4 | `cargo test -p rcfd-api --release --lib` | passes | acceptance of F-856 | passes | exact | code verification |
| E1 | c5 | `udf` and `udf_hooks_w38a` of rcfd-api and `check_inputs_adr0739` of rcfd-cli, unedited | every test passes | acceptance of F-856 | pass | exact | unchanged default output |
| B1 | c6 | `baseline_hashes` | passes unedited | acceptance of F-856; rule 3 | pass | exact | unchanged default output |
| L1 | c7 | a counted loop in `kinematic_viscosity`: 100 turns with the default fuel; 10 000 000 turns with the default fuel; then 10 turns on the same instance; 1000 turns with a fuel of 20 | succeeds; fails with "kinematic_viscosity failed"; succeeds; fails | acceptance of F-856 | as stated | exact | code verification |
| L2 | c7 | the recorded cost: `max_fuel_per_call` after 10 turns, then after 100 turns | positive, larger after 100 turns, at most the fuel per call | acceptance of F-856 | as stated | exact | code verification |
| L3 | c7 | a module whose start function loops for ever | refused at load with "cannot instantiate" | acceptance of F-856 | refused | exact | code verification |
| M1 | c8 | a module with one page that grows by a given number of pages, fuel 100 000 000: by 255 pages (16 MiB in all); by 256 pages | returns 256 pages; fails with "kinematic_viscosity failed" | acceptance of F-856; `MAX_MEMORY_BYTES` | as stated | exact | code verification |
| M2 | c8 | a module that declares 256 pages; one that declares 257 | loads; refused with "cannot instantiate" | acceptance of F-856 | as stated | exact | code verification |
| I1 | c9 | a module that imports a function; a memory; a global; a table | each refused, the message has "imports" and the import's name | acceptance of F-856 | refused | exact | code verification |
| A1 | c9 | modules that need two memories, a 64-bit memory, a tail call, an extended constant expression (D2) | each refused with "not a valid WebAssembly module" | acceptance of F-856 (behave as before) | refused | exact | code verification |
| A2 | c9 | the text of a valid module given as the file's bytes (D1) | refused with "not a valid WebAssembly module" | acceptance of F-856 (behave as before) | refused | exact | code verification |
| D1 | c10 | `cargo deny check` | passes | acceptance of F-856 | passes | exact | code verification |

F1, a record without a limit: the `max_fuel_per_call` that L2 prints for 10 and for 100 turns, on 0.31.2 and on 1.1.0.

The existing test of the memory cap (`runaway_modules_are_stopped` in `tests/udf.rs`) grows by 1024 pages with a fuel of
10 000; if growth is charged fuel by the byte, it cannot tell the cap from the fuel. M1 uses a fuel that the growth to
16 MiB fits (at 64 bytes per unit, the default of 0.31.2, 16 MiB costs 262 144) and puts the two sides of the cap one
page apart.

No limit of this table is the agent's own estimate; none is a limit on wall-clock time. No heavy record: every new test
is expected to take well under a second.

## Negative controls and planned mutations

On a scratch copy of the ported tree, each with the command that must fail:

* MU1 (the committed one, `crates/rcfd-api/tests/udf_limits_adr0782.mutation.patch`): the store limits without
  `memory_size`. Must fail M1 and M2.
* MU2: `kinematic_viscosity` without the renewal of the fuel. Must fail L1 (the call after the failed one) or the
  existing test of 20 000 calls.
* MU3: the refusal of imports removed. Must fail I1 (the message), whatever the empty linker then says.
* MU4: the four switches of D2 removed. Must fail A1.
* MU5: `Cargo.toml` with wasmi's default features. Must fail A2.

The control run on 0.31.2 is the negative control of the claim "as before": the same assertions on the old interpreter.

Size (agent): done; 0; 0; 0; 110000

## Decision rule

Done if every check of the table holds with wasmi 1.1.0. Partial if one does not, with its output; a check that could
not be run because a tool is not installed (Q2 without Rust 1.90, D1 without cargo-deny) is named as not run and makes
the proposal partial unless the lead runs it. Todo if the minimum Rust had stopped the work at step 1 (it did not).

## What is not covered

wasmi 2.x. Any change of ABI 1 or of the hook catalogue. The fixes of translation in 1.0.5 to 1.0.9 are the reason
for the port and are not re-tested here. A comparison of run time between the two interpreters. The files outside this
work's ownership that the new version touches (`crates/rcfd-api/Cargo.toml` `rust-version`, the README's range of
minimums, the generated `THIRD_PARTY_LICENSES.md`): reported to the lead, not edited.

## Addendum 1: results (2026-11-23)

Commits: the registration 608ec768 alone; the port and the new test file bcb0d2a7; the mutation patch, this addendum,
the proposal file and the topic page after them. Checks in the release profile (clippy in the dev profile), Rust 1.99.0,
Windows, target folder under the scratch folder. Each registered run was made once; after `rustfmt` had rewrapped lines
of `udf.rs` (the first `cargo fmt --check` found five places, none a change of code), the three UDF test files, clippy
and the format check were run again and passed.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| V1 | wasmi 1.1.0 and no wasmi 0.31 in `Cargo.lock` | wasmi, wasmi_core, wasmi_collections, wasmi_ir 1.1.0; wasmi 0.31.2, wasmi_arena and wasmparser-nostd gone | yes |
| Q1 | clippy with `-D warnings` and the format check: no finding | clippy none; format none after the rewrap | yes |
| Q2 | the type-check with Rust 1.90 passes | not run: the toolchain 1.90.0 is not installed, and none was installed | not decided |
| Q3 | the unit tests of rcfd-api pass | 159 passed, 0 failed, 4 ignored (as before) | yes |
| E1 | the three existing files pass unedited | `udf` 8 of 8; `udf_hooks_w38a` 9 passed, 1 ignored (its record of before); `check_inputs_adr0739` 3 of 3 | yes |
| B1 | `baseline_hashes` passes unedited | 3 passed, 1 ignored (as before) | yes |
| L1 | succeeds; fails; succeeds; fails | as registered | yes |
| L2 | positive, larger after 100 turns, at most the fuel per call | 47 after 10 turns, 407 after 100, of 1 000 000 | yes |
| L3 | refused with "cannot instantiate" | refused | yes |
| M1 | 256 pages returned; the call with one page more fails | as registered | yes |
| M2 | 256 pages load; 257 refused | as registered | yes |
| I1 | four kinds of import refused with the name | as registered | yes |
| A1 | four proposals refused as not valid | as registered | yes |
| A2 | the text format refused as not valid | as registered | yes |
| D1 | `cargo deny check` passes | "advisories ok, bans ok, licenses ok, sources ok" (cargo-deny 0.22.0); the subset of the CI job also | yes |

F1 (no limit): `max_fuel_per_call` for 10 and 100 turns of the loop was 105 and 915 on wasmi 0.31.2 and is 47 and 407 on
1.1.0. The same budget buys about 2.2 times as many turns of that loop; other code was not measured. A budget that was
enough for a module stays enough as far as this sample shows; the time until a runaway module is stopped grows.

The control: the new test file on the unported tree (wasmi 0.31.2), before the port: 8 of 8 passed. So the behaviour
the new tests fix is the behaviour of before.

Mutations, on a scratch copy of bcb0d2a7 (`udf_limits_adr0782` and `udf` run on each; the unmutated copy passed both):

| Id | Mutation | Caught by | Not caught by |
| --- | --- | --- | --- |
| MU1 (committed) | the store limits without `memory_size` | M1 and M2 | all 8 tests of `udf`: the existing test of the memory cap passes without the cap, as the registration expected |
| MU2 | no renewal of the fuel in `kinematic_viscosity` | L1; `runaway_modules_are_stopped` | |
| MU3 | the refusal of imports removed | I1; `module_isolation_and_abi_checks` | |
| MU4 | the four switches of D2 removed | A1 | `udf` |
| MU5 | wasmi with its default features | A2 | `udf` |

Deviations: none from the design. cargo-deny was not installed on the machine; it was built into the scratch folder
(`cargo install --root`), nothing outside it was changed. The gate runs the eight new tests (0.02 s) with the existing
files; there is no heavy record.

Not run: Q2. The whole workspace suite and the other integration tests of rcfd-api (rule 7). `npm test` (not touched).

Size (agent), actual: partial; 0; 0; 1; 130000

### Decision

By the registered rule: **partial**. Every check that was run holds with wasmi 1.1.0; Q2 could not be run for want of
the toolchain. If the lead's run of `cargo check --workspace --all-targets --locked` with Rust 1.90 passes (the CI job
`min-toolchain`), the rule gives done.

### For the lead, outside the files of this work

* `crates/rcfd-api/Cargo.toml` states `rust-version = "1.85"`; its closure now needs 1.86 (wasmi 1.1.0). The README's
  sentence on the crates' own minimums ("1.75 to 1.85") changes with it. Neither file was edited.
* `python scripts/third_party_licenses.py --check` fails after the port (the wasmi packages changed). It also fails on
  main e2fe571b, where the list still names wat 1.262.0 and `Cargo.lock` has 1.263.0. The list was not regenerated.
* `python scripts/process_report.py --check`: one failure, "generated file is not current: docs/adr/index.md".

### Open questions for the maintainer

1. wasmi 2.0.2 was published on 2026-11-19 (2.0.0 and 2.0.1 are yanked), with the same minimum Rust. Options: (a) stay
   on 1.1.0 until a fix or a need names 2.x; (b) a new item for 2.x now. Recommendation: (a); the item named 1.x, the
   2.x line is four days old with two yanked releases, and the tests of this ADR make a later move cheap to check.

## For the lead: roadmap entries

Status proposed: partial.

> ADR 0782, 2026-11-23, partial: the workspace uses wasmi 1.1.0 (the newest 1.x; minimum Rust 1.86, below the README's 1.90) without its default feature wat, and udf.rs is ported with the accepted set of modules, the eager translation and the limits of before; clippy with warnings as errors, the 159 unit tests, udf, udf_hooks_w38a, check_inputs_adr0739 and baseline_hashes pass unedited, cargo deny check passes, and eight new tests of the fuel, the 16 MiB cap one page apart and the refusals pass on wasmi 0.31.2 and on 1.1.0 alike, each of five mutations caught; the type-check with Rust 1.90 was not run because that toolchain is not installed on the machine.

> Missing for done: cargo check --workspace --all-targets --locked with Rust 1.90 (not installed where the work was done; the CI job min-toolchain runs it). For the lead outside the item's files: rust-version of crates/rcfd-api/Cargo.toml and the README's range (1.85 to 1.86), and THIRD_PARTY_LICENSES.md regenerated.

No follow-up item. The same entries are in `docs/roadmap/proposals/0782-F-856.json`.

## The lead's reading (2026-11-23), at the merge

Merged at 695a6892 and pushed at 47ade6bf (the report read at 04:20 UTC, 8 minutes of reading, proposed partial,
written partial). Checked by the lead before the merge: the registration is the first commit and alone, the new tests
are committed before the port, and every existing UDF test passes unedited. At the merge the lead regenerated
`THIRD_PARTY_LICENSES.md` for wasmi 1.1.0. The first push run stopped on access denied to `target/release/rcfd.exe`,
held by `rcfd serve` processes left behind by Studio end-to-end tests; the run was repeated in a separate target
folder and passed.

Status: **done** by the lead's reading. The one check missing for done, `cargo check --workspace --all-targets --locked`
with Rust 1.90, passed in CI on main at 47ade6bf (run 45131392392, job "Type-checks with Rust 1.90"). wasmi stays at
1.1.0; a move to 2.x is not taken. The `rust-version` fields of the crates already lagged their dependency closure
before this item (1.88 is needed by rcfd-api, rcfd-cli, rcfd-mcp and rcfd-workspace), so the lead did not change them
here; the README's minimum of 1.90 is the one the CI job holds.
