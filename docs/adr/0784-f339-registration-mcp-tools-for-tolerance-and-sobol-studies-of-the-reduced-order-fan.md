# ADR 0784: F-339 registration: the MCP tools `design_uq` and `design_sobol` for tolerance and Sobol studies of a named evaluator, the reduced-order fan first

Status: **REGISTERED 2026-11-23, written before any code of F-339 and before any run of its checks.** Before this text no
library, server or test code of F-339 was written and no file of the repository was changed. What was done: reading
(below), searches of the source tree, one run of `python scripts/process_report.py --check` on the unchanged tree
(0 failures) and a build of the existing link test `docs_site`, to check the links of this file. Results go in addenda at
the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f339-mcp-uq-sobol-tools.md` ([the file](briefs/f339-mcp-uq-sobol-tools.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f339-mcp-uq-sobol-tools` from main
fcc84a84, scratch `w49i-work/f339-mcp-uq-sobol-tools-scratch`.

## What is asked

Roadmap item F-339 (deps F-268), the binding acceptance:

> design_uq and design_sobol tools run uq::run and sensitivity::run on a named evaluator (the reduced-order fan evaluator first) inside the sandbox, with the ledger, and the MCP tools-list goldens are updated by a registered edit.

Clauses: **a1** the two tools run `uq::run` and `sensitivity::run` on a named evaluator, the reduced-order fan evaluator
first; **a2** inside the sandbox; **a3** with the ledger; **a4** the tools-list goldens are updated by a registered edit.

The item comes from question 2 of [ADR 0240](0240-f268-registration-sobol-sensitivity-indices.md). The maintainer's answer
of 2026-11-18 (written into ADR 0240 by the lead) puts the golden edit under the class rule of rule 3: an edit that only
adds entries is reported by the agent as an "additive golden edit" with its diff and re-recorded by the lead
([docs/process/lead-process.md](../process/lead-process.md), "Golden files"). This branch edits no golden.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); ADR 0240 (whole); the item F-339 of
`docs/roadmap.json` (by a script); `crates/rcfd-api/src/design/uq.rs`, `sensitivity.rs`, `tools.rs` and `fan_rom_eval.rs`
(whole); of `crates/rcfd-api/src/design/ledger.rs` the ledger file (`Ledger`, `append`, the stamps), `Model`, `Outcome`
and `entry_of`; of `crates/rcfd-api/src/service.rs` the error codes, `sandbox_path`, `max_design_evaluations` and the list
`METHODS`; `study::parallel_map`; of `crates/rcfd-post/src/fan_rom.rs` the types `RomSection`, `RomFan` (with `validate`),
`RomModel`, `Operating` and `FanRom::new`; `SectionFamily` of `crates/rcfd-mesh/src/fan.rs`;
`crates/rcfd-mcp/src/lib.rs` (the tool table, the argument check, `all_tools`, `call_tool`, `dispatch`) and
`design_tests.rs` (T1, T5, M1); `crates/rcfd-mcp/Cargo.toml`; the tests that hold the tool lists, `default_f1.rs`,
`tool_list_f275.rs`, `tool_list_adr0730.rs`, `project_list_f270.rs` and `tests/common/mod.rs`; the head of
`crates/rcfd-api/tests/fan_rom_bands_f268.rs` (the fan of set-up C and the tolerances T3); the design sections of
[docs/mcp-tools.md](../mcp-tools.md); the lines of [docs/design-loop.md](../design-loop.md) on the two drivers;
`scripts/process_report.py` (the checks of a registration) and `scripts/push_main.py` (mutation patches);
[ADR 0780](0780-f842-registration-inertia-and-retry-in-the-solid-case-output.md) for the form; of the Studio,
`apps/studio-web/capabilities/coverage.json` and `inventory.json` (the rows of the design tools) and the head of
`tests/unit/capabilities.spec.ts`.

## Findings of the reading (before any code)

1. **The drivers and the evaluator exist.** `uq::run(spec, model, ledger)` and `sensitivity::run(spec, model, ledger)` take
   any `ledger::Model`. `fan_rom_eval::FanRomModel`
   ([ADR 0623](0623-f268-remaining-ledger-logging-and-reduced-order-bands.md) D2, on the reduced-order fan of
   [ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md)) is such a model with the id `rcfd.fan_rom`
   (`FAN_ROM_MODEL_ID`) and the three parameters of `FAN_ROM_PARAMS`: `stagger_common_deg` [deg], `chord_rel` [1],
   `tip_gap_delta` [m]. It is built from a blade row (`RomFan`), a section and loss model (`RomModel`) and a condition
   (`RomCondition`). None of the three has a JSON form yet: the tools need one.
2. **No source of any solver is touched.** The tools parse arguments, build the model and the spec, and call the two
   drivers. `uq.rs`, `sensitivity.rs`, `fan_rom_eval.rs`, `ledger.rs` and `rcfd-post` are not changed.
3. **The service's method list is pinned.** A `design.*` method is reached through `Service::call` and is listed in
   `METHODS` of `service.rs`, which `service.info` prints; `crates/rcfd-api/tests/cosim_service_f360.rs` holds its length
   at 69, and the Studio's capability inventory lists every method. `service.rs` is not a file of this item. So the two
   tools are **not** service methods: the MCP server calls two new public functions of `design/tools.rs` directly, and
   `design::tools::call`, `METHODS` and `service.info` are unchanged. Whether they should also become service methods
   is a question for the maintainer (it changes a pinned list); it is asked in the addendum.
4. **The tool lists are held by three tests and two goldens, with counts in the tests' source.** `default_f1.rs` holds
   the list without `--project` as `golden/tools_list_default.json` (33 tools) plus `create_project` (34 live);
   `tool_list_f275.rs` holds the project list as the 17 kept session tools of that golden and 36 project tools (53);
   `project_list_f270.rs` takes the project list apart down to `golden/tools_list_project_f270.json` (50 tools: 33 session
   tools and 17 project tools). Two new session tools make all three fail on this branch until the goldens gain the two
   entries **and** the counts in the three tests rise by two. That is the lead's re-recording; this branch reports the
   exact edit and checks it on a scratch copy (G3). Existing tests are not edited here (rule 2).
5. **The Studio's capability inventory lists every MCP tool.** `apps/studio-web/capabilities/inventory.json` is
   regenerated from the binaries (`npm run capabilities`) and `capabilities/coverage.json`, kept by hand, maps every
   capability to a UI path or an exemption; `tests/unit/capabilities.spec.ts` compares them with the engine when the
   binaries are found. Two new tools need two inventory rows and two coverage rows. Those files are not this item's;
   the rows are given to the lead in the addendum (the lead regenerates the inventory after merges, as commit 166624a3).
6. **Numbers cross MCP exactly.** The workspace builds `serde_json` with `float_roundtrip`, so a number read from the
   request text is the nearest double, and a double is written in its shortest form that reads back to the same bits.

## Design (registered)

### Where

- `crates/rcfd-api/src/design/tools.rs`, added at the end, nothing above changed: `pub fn uq_study(svc, p) -> Rpc` and
  `pub fn sobol_study(svc, p) -> Rpc`, the constant `STUDY_EVALUATORS` (the accepted names) and private parsers.
- `crates/rcfd-mcp/src/lib.rs`: two entries of the tool table, right after `design_ledger_rerun_check`, in the order
  `design_uq`, `design_sobol`, both of class `write_files` (they write the ledger file and start no case run), and their
  two arms in `dispatch`, which call the two functions. Nothing else of the server changes: no other descriptor, no
  class, no text of `initialize`. With `--project` the two tools are listed and callable like the ledger tools (they are
  not session-only), with the same class.
- `docs/mcp-tools.md`: the two rows of the tool list, a section on the two tools, and the count of kept session tools
  in project mode (17 becomes 19). `docs/design-loop.md`: one sentence under each driver that names its tool.
- New test file `crates/rcfd-mcp/tests/design_uq_sobol_f339.rs` and its mutation patch.

### Arguments

Both tools (`additionalProperties: false` at the top level; nested objects are checked by the handler, which refuses an
unknown key with its place):

| Argument | Type | Meaning |
|---|---|---|
| `ledger` | string, required | ledger file inside the sandbox (created on first use), through `Service::sandbox_path` |
| `name` | string, required | study name, the `study` of the entries; the name rule of the design tools (`name_ok`) |
| `evaluator` | string, required | a name of `STUDY_EVALUATORS`; now only `rcfd.fan_rom` |
| `evaluator_settings` | object, required | what builds the evaluator; for `rcfd.fan_rom` the three objects below |
| `params` | array of objects, required | uncertain inputs `{name, unit, dist}` |
| `seed` | integer, at least 0, default 1 | the seed of the driver (the default of the study methods' seed) |
| `workers` | integer, at least 1, default 1 | concurrent evaluations; the result does not depend on it |

`evaluator_settings` of `rcfd.fan_rom`, every number in the unit of the Rust field of the same name:

- `fan` (required): `n_blades` (integer), `r_hub`, `r_tip`, `tip_gap` [m], `family` (`naca4` or `circular_arc`, the
  serialised names of `SectionFamily`), `sections`: a list of `{radius [m], chord [m], stagger [rad], camber, camber_pos}`,
  all five required. This is `RomFan`.
- `model` (optional object): any of `n_strips`, `cl_max`, `cd_normal`, `friction`, `annulus_drag`, `secondary_drag`,
  `tip_cd`; a key that is absent keeps the value of `RomModel::default()` (the registered values of ADR 0119).
- `condition` (required): `rpm`, `rho`, `nu`, `system_k`, `system_l`, `c0`, `observer_r`, `observer_theta` [rad],
  `n_scan` (integer, at least 1), all nine required. This is `RomCondition` with `Operating` and `SystemCurve`.

The model is `FanRomModel { fan, model, cond }`; `FanRom::new(fan, model)` is called once before the study so that an
invalid blade row or model is refused with the library's located text, not reported sample by sample.

`params[i]`: `name` and `unit` must be a pair of `FAN_ROM_PARAMS` (the evaluator's own check would otherwise fail every
sample); `dist` is the form `uq::Dist::to_json` writes: `{type: "uniform", lo, hi}`, `{type: "normal", mean, sd, lo, hi}`
with `lo` and `hi` optional (absent, `null`, or the strings `-inf` and `inf`: no truncation on that side), or
`{type: "beta", a, b, lo, hi}`.

`design_uq` only: `sampler` (`monte_carlo`, the default, or `lhs`); `stop` (object, required): `batch` and `max_samples`
required, `min_batches` (default 1 for `monte_carlo`, 2 for `lhs`, the least `UqSpec::validate` accepts), `se_abs` and
`se_rel` (default 0), `metrics` (list of names, default empty: all); `quantiles` (default `[0.05, 0.5, 0.95]`) and
`confidence` (default 0.95), the values of `fan_rom_band_spec` (ADR 0623 D3); `requirements` (list of `{metric, cmp,
limit}`, `cmp` one of the two comparison strings of the study constraints, default empty).

`design_sobol` only: `n` (integer, required): the rows of each of the matrices `A` and `B`.

The spec handed to the driver is exactly these values: `UqSpec { name, params, sampler, seed, stop, quantiles,
confidence, requirements, workers }` and `SobolSpec { name, params, n, seed, workers }`. The tools add no sampling, no
statistic and no rounding of their own.

### Replies

`design_uq`: `{ledger, evaluator, evaluator_version, fidelity, entries_appended, result}` with `ledger` the resolved
path, `evaluator_version` and `fidelity` those of the model, `entries_appended = n_samples + 1` and `result` the value of
`uq::result_json(spec, res)`, unchanged. `design_sobol`: the same keys, `entries_appended = n_evaluations + 1`, `result`
the value of `sensitivity::result_json(spec, res)`.

### The sandbox and the ledger

- The ledger path goes through `Service::sandbox_path`; a path that leaves the root is error 1004 and nothing is written.
- One call evaluates at most `max_design_evaluations` variants (the service limit of every design tool,
  `--max-design-evaluations`, default 1000): `stop.max_samples` for `design_uq` (the driver never evaluates more) and
  `n (d + 2)` for `design_sobol`. Above it the call is error 1004 before anything is evaluated or written.
- The driver gets `Some(&Ledger::new(path))`: every sample or evaluation is an entry and one `uq_summary` or
  `sobol_summary` closes the study, as the drivers write them. The tool writes nothing else to the ledger.
- The drivers find their entries by study name (`name#k`, `name#summary`). A ledger that already holds an entry of the
  study `name` is refused (-32602, naming `name`) before anything is evaluated, so a repeated call cannot mix two
  studies under one name. The tools are not idempotent: a new study needs a new name or a new ledger.
- The class `write_files` puts both tools under the permission policy: a read-only server denies them.

### Errors

Arguments that do not match the `inputSchema` (a missing required argument, a wrong type, an unknown top-level argument)
are JSON-RPC errors -32602 from the server's argument check, as for every tool. The handler's refusals are tool results
with `isError: true`:

| Case | Code | The message names |
|---|---|---|
| `evaluator` not in `STUDY_EVALUATORS` | -32602 | `evaluator` and the accepted names |
| `evaluator_settings`: a missing or mistyped key, an unknown key, a blade row or model that `FanRom::new` refuses | -32602 | the place (`evaluator_settings.fan.sections[1].chord`, ...) |
| `name` against the name rule | -32602 | `name` |
| `params`: empty, a name or unit that is not the evaluator's, a malformed `dist`, anything `validate` of the spec refuses | -32602 | the parameter or key (the library's text for `validate`) |
| `stop`, `quantiles`, `confidence`, `requirements`, `sampler`, `n`: malformed, or refused by `validate` | -32602 | the key |
| the ledger already holds the study `name` | -32602 | `name` and the ledger |
| the ledger path leaves the sandbox | 1004 | the path (the text of `sandbox_path`) |
| more evaluations than `max_design_evaluations` | 1004 | the count and the limit |
| the ledger cannot be read or written, or the driver returns an error after its validation | -32603 | the driver's text |
| the policy denies the call | the server's permission error | the tool |

In every refused case no ledger file is created and an existing one keeps its bytes.

## Arithmetic done beforehand

Counts only; no number of a result was computed. The set-up is that of `fan_rom_bands_f268.rs`: the fan of set-up C of
ADR 0119 (7 blades, hub 0.030 m, tip 0.058 m, tip gap 0.001 m, NACA sections of chord 0.030 m and camber 0.04 at 0.5,
staggered by `free_vortex_design` for `Q_D = 0.045` m3/s at 3000 rev/min, density 1.2, viscosity 1.5e-5), the default
model, the system curve through the design point (`k = dp_s(Q_D) / Q_D^2`, `l = 0`), `c0 = 343`, observer at 1 m and
`pi / 2`, `n_scan = 100`. The inputs span the ranges of T3 of ADR 0623: `stagger_common_deg` normal (0, 0.5) on
`[-1, 1]` deg, `chord_rel` beta (2, 2) on `[-0.015, 0.02]`, `tip_gap_delta` uniform on `[-2e-4, 2e-4]` m, so that all
three kinds of distribution cross the parser.

- U1: Monte Carlo, `batch 64`, `min_batches 2`, `max_samples 256`, targets 0: the rule can only be met by a standard
  error of exactly 0, so 4 batches run: 256 samples, 257 entries. Seed 339001.
- U2: Latin hypercube, `batch 32`, `max_samples 128`, `se_rel 0.05` on `q`, the stagger untruncated (`lo`, `hi` absent),
  the requirement `q >= 0.045`: between 2 and 4 replicates, `n + 1` entries. Seed 339002.
- S1: `n = 64`, `d = 3`: `64 * 5 = 320` evaluations, 321 entries, 2 workers. Seed 339003.
- B2 with `max_design_evaluations = 100`: `max_samples 101` is above the limit and `100` is at it (`batch 50`);
  `n = 21` gives `21 * 5 = 105`, above, and `n = 20` gives 100, at it. Seed 339004.

Why equal text is equal bits: `result_json` writes every finite double through `serde_json` in the shortest form that
reads back to the same double (finding 6), which differs for two different doubles and for the two zeros, and every
non-finite one as its string. So two results whose serialised texts are equal agree in every float by bits.

## Checks

All new checks are in `crates/rcfd-mcp/tests/design_uq_sobol_f339.rs`, on an in-process `McpServer` over a sandbox root,
run in the release profile (`cargo test -p rcfd-mcp --release --test design_uq_sobol_f339`). The reference of U1, U2, S1,
L1 and L2 is built in the test by hand from the Rust types (the model from `RomFan`, `RomModel::default()` and
`RomCondition`; the spec as a struct literal) and run by the library; it shares no code with the tools' parsers. The
arguments are JSON values made from the same doubles.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| U1 | a1 | `design_uq`, the Monte Carlo set-up: the reply's `result`, serialised | the bytes of `uq::result_json` of `uq::run` on the hand-built model and spec; `evaluator` `rcfd.fan_rom`, version and tier the model's | acceptance of F-339; the brief's decision rule (bit for bit) | equal | exact | code verification |
| U2 | a1 | `design_uq`, the Latin hypercube set-up with a stopping rule and a requirement | the same equality | acceptance of F-339; the brief's decision rule | equal | exact | code verification |
| S1 | a1 | `design_sobol`, `n = 64`, 2 workers: the reply's `result`, serialised | the bytes of `sensitivity::result_json` of `sensitivity::run` on the hand-built model and spec | acceptance of F-339; the brief's decision rule | equal | exact | code verification |
| L1 | a3 | U1's ledger file against the file `uq::run` writes with a ledger of its own | 257 lines each; line by line equal as text once `recorded_at_ms` is set to 0 in both; the tool's stamps above 0; `entries_appended` 257; the last entry of kind `uq_summary` | acceptance of F-339 (with the ledger) | equal | exact | code verification |
| L2 | a3 | S1's ledger file against the file `sensitivity::run` writes | 321 lines each, equal in the same way; `entries_appended` 321; the last entry of kind `sobol_summary` | acceptance of F-339 | equal | exact | code verification |
| L3 | a3 | the ledger is the design ledger: `design_ledger_query` on U1's file; a second `design_uq` with the same `name`; a `design_sobol` with another name on the same file | `total` 257 and one `uq_summary` of the study; the repeat is -32602 naming `name` and the file keeps its bytes; the other study appends and the earlier bytes are a prefix of the file | acceptance of F-339; this design (the sandbox and the ledger) | as stated | exact | code verification |
| B1 | a2 | paths: `ledger` with `..` and an absolute path outside the root, both tools; a `name` with a slash | 1004 for the paths and -32602 for the name; no file at the target, none in the root | acceptance of F-339 (inside the sandbox); the code of `sandbox_path` | refused | exact | code verification |
| B2 | a2 | the limit, a server with `max_design_evaluations = 100`: 101 and 100 samples, `n = 21` and `n = 20` | above: 1004 naming the limit, no ledger file; at the limit: accepted, 101 entries each | acceptance of F-339; the service limit of the design tools | as stated | exact | code verification |
| B3 | a2 | permissions and descriptors: a read-only server; `tools/list` | both tools denied (the server's permission error), no file; both of class `write_files`, `rcfd/needsApproval` true, `readOnlyHint` false, `additionalProperties` false, required arguments as designed | acceptance of F-339; the permission model of the server (F-091) | as stated | exact | code verification |
| P1 | a2 | a project server (`--project` mode, in process): both tools listed with the descriptors of the list without a project, and one `design_sobol` (`n = 2`, 10 evaluations) into a ledger inside the project folder | byte-identical descriptors; the call accepted, 11 entries, the reply's `result` the library's bytes | acceptance of F-339; rule 3 (nothing else changes) | equal | exact | code verification |
| E1 | a1 | each handler refusal of the table of errors but the last two rows, at least one case per row | the registered code, the registered key in the message, no ledger file | this design, from the acceptance of F-339 (a named evaluator) | refused | exact | code verification |
| G1 | a4 | the list without a project, with `design_uq`, `design_sobol` and `create_project` taken out, against `golden/tools_list_default.json` with the two taken out if present | byte for byte, entry by entry, in order; the two sit right after `design_ledger_rerun_check`, in the order `design_uq`, `design_sobol` | rule 3 (additive class); acceptance of F-339 | equal | exact | unchanged default output |
| G2 | a4 | the project list: the two entries and the kept session tools | the two byte-identical to the list without a project and right after `design_ledger_rerun_check`; with the two taken out of both, the kept session tools are the golden's, byte for byte, but the class `execute` of the three tools of ADR 0196 | rule 3 (additive class) | equal | exact | unchanged default output |
| G3 | a4 | scratch copy of the final commit with the additive edit applied (two entries added to each golden, the counts of finding 4 raised by two): `default_f1`, `tool_list_f275`, `project_list_f270`; and a script on the goldens before and after | the three pass; the script finds every earlier entry byte-identical and in order, two added, none changed or removed, in each golden | rule 3 (additive class); the lead's process, "Golden files" | pass | exact | unchanged default output |
| X1 | a1 | existing tests, unedited: `cargo test -p rcfd-mcp --release --lib`; `--test tool_list_adr0730`; `cargo test -p rcfd-api --release --lib` | pass | rule 3; rule 7 | pass | exact | unchanged default output |

On this branch, before the lead's re-recording, `default_f1::f1_tools_list_without_project_is_the_golden_plus_create_project`,
`tool_list_f275::c5l_project_list_has_no_session_only_tool` and
`project_list_f270::a2_tools_list_with_project_is_the_golden_with_the_f271_changes` fail by design (finding 4). They are
run once and reported with their messages; they are not edited, ignored or skipped.

### Negative controls

- NC1: U1's reference run again with seed 339002 in place of 339001: its text differs from the tool's (the equality of
  U1 can fail).
- NC2: in B2 the calls at the limit are accepted (the refusal is not unconditional).
- NC3: in L1 the two files differ as raw bytes when the reference ledger has a fixed clock of 1 (the stamp is the only
  difference, and the comparison sees it before it is set to 0).

### Planned mutations (rule 16)

Each on a scratch copy of the final commit, the new test file run once, the file restored.

| Id | Mutation | Must fail |
|---|---|---|
| M1 | `uq_study` hands the driver no ledger (`None`) | L1, L3, B2 |
| M2 | the section parser reads `stagger` in degrees (`to_radians`) | U1, U2, S1, P1 (or E1 by a refused row) |
| M3 | the truncation bounds of a normal `dist` are dropped | U1, S1 |
| M4 | the limit on the evaluations is not applied | B2 |
| M5 | the ledger path does not go through `sandbox_path` | B1 |
| M6 | the refusal of a study name the ledger already holds is removed | L3 |
| M7 | `design_sobol` has the class `read` | B3 |
| M8 | `sobol_study` passes `seed + 1` to the driver | S1, L2, P1 |

The committed one is M1, `crates/rcfd-mcp/tests/design_uq_sobol_f339.mutation.patch`, whose first line is
`# expect-fail: cargo test --release -p rcfd-mcp --test design_uq_sobol_f339`.

Size (agent): done; 0; 0; 1; 420000

## Decision rule

**done** if U1, U2 and S1 hold (both tools reproduce the library bit for bit on `rcfd.fan_rom`), B1, B2, B3 and P1 hold
(inside the sandbox), L1, L2 and L3 hold (with the ledger), G1, G2 and G3 hold (the golden diff is additive), E1 and X1
hold and the committed mutation makes its test fail. **partial** otherwise, with the check that missed. This is the
brief's rule; the three pinned list tests that fail until the re-recording (finding 4) are the registered edit itself and
do not make the outcome partial, provided G3 shows that they pass with it.

## What is not covered

- Service methods for the two studies (finding 3); a CLI command; a Studio view.
- Any evaluator but `rcfd.fan_rom`; the case solver as an evaluator of these studies.
- The standardised inputs and the tolerance bands of ADR 0623 D3 (`fan_rom_bands`) through MCP; a `Tolerances` argument.
- A limit on `workers`; resuming a study; a stopping rule for the Sobol indices (ADR 0240, question 5).
- The accuracy of the drivers and of the reduced-order fan: they are those of ADR 0121, ADR 0240, ADR 0119 and
  ADR 0623, unchanged; no reference value is added and no check of kind validation is claimed (the acceptance claims
  no agreement with a measured case).
- The run through the `rcfd-mcp` binary on stdio is not a check of its own: the three pinned list tests run the binary,
  and G3 runs them with the edit.

## Amendment 1 (2026-11-23, before any code and before any run)

The table of errors gives -32603 for a ledger that cannot be read or written and for a driver error after validation.
That number was written without reading the constant: the service's internal error is `code::INTERNAL` of
`crates/rcfd-api/src/service.rs`, which is -32000, and the design tools use it for exactly these cases
(`ledger_append`, `read_ledger`). The two tools use `code::INTERNAL`, so the row reads -32000. No check is changed:
E1 leaves that row out, because neither case can be provoked inside the sandbox without a fault of the file system.
Nothing else of the registration changes.

## Addendum 1: results (2026-11-23)

Code at commit c7443193 (after the registration 6a139451 and amendment 1, 8261f2e1). The registered run was made once:
`cargo test -p rcfd-mcp --release --test design_uq_sobol_f339 -- --nocapture --test-threads 1`, 11 passed, 0 failed,
229.18 s in series (about 105 s with the default threads).

| Id | Limit | Measured | Met |
|---|---|---|---|
| U1 | the bytes of the library's result | equal; 256 samples, all ok; 7228 bytes of result; evaluator, version and tier the model's | yes |
| U2 | the same equality | equal; 2 replicates of 32, converged, the requirement's probability 0.5 (32 of 64) | yes |
| S1 | the bytes of the library's result | equal; 320 evaluations, 64 of 64 rows used | yes |
| L1 | 257 lines, equal but for the stamp | 257 and 257, equal line by line; stamps above 0; last entry `uq_summary` | yes |
| L2 | 321 lines, equal in the same way | 321 and 321, equal; last entry `sobol_summary` | yes |
| L3 | query total 257; repeat refused; another study appends | total 257, one summary; the repeat -32602 naming `name`, bytes kept; the Sobol study appended (268 lines), earlier bytes a prefix | yes |
| B1 | 1004 for paths, -32602 for names, nothing written | 6 paths 1004, 2 names -32602, no file | yes |
| B2 | above the limit 1004, at it accepted | 101 samples and 105 evaluations refused with 1004 at the limit 100; 100 and 100 accepted, 101 entries each | yes |
| B3 | denied by a read-only server; class and schema as designed | as registered | yes |
| P1 | descriptors byte-identical; the call accepted, 11 entries | identical; 10 evaluations, 11 entries, the library's bytes | yes |
| E1 | registered code and key, no file | 21 handler refusals with -32602 and their key, 3 schema refusals, nothing written; a control call accepted | yes |
| G1 | the golden, byte for byte, the two after `design_ledger_rerun_check` | 36 live tools: the 33 of the golden byte-identical and in order, `design_uq`, `design_sobol`, `create_project` | yes |
| G2 | the two byte-identical, the kept session tools the golden's | 55 tools with a project: 17 kept session tools as the golden, the two new ones, 36 project tools | yes |
| G3 | the three list tests pass with the edit; the script finds the edit additive | they pass, with the wider edit of the next sections; each golden: every earlier entry byte-identical and in order, 2 added, 0 changed, 0 removed (33 to 35 and 50 to 52 entries) | yes, with more count edits than registered |
| X1 | pass | `tool_list_adr0730` 2 passed; `rcfd-api --lib` 159 passed, 4 ignored; `rcfd-mcp --lib` 17 passed, **1 failed** | **no** |

Negative controls: NC1, NC2 and NC3 hold (inside U1, B2 and L1).

### X1 and the count pins the registration did not find

`tests::stdio_loop_handles_a_scripted_session` (`crates/rcfd-mcp/src/tests.rs`, line 295) holds the length of the list
without a project at 34 and found 36. Finding 4 named three tests; a search for the counts and one run of every other
test file of `rcfd-mcp` (unedited) found seven tests in six files that hold a count of the tool list:

| File | Test | Count held, found |
|---|---|---|
| `tests/default_f1.rs` | `f1_tools_list_without_project_is_the_golden_plus_create_project` | 34, 36 |
| `tests/tool_list_f275.rs` | `c5l_project_list_has_no_session_only_tool` | 53, 55 |
| `tests/project_list_f270.rs` | `a2_tools_list_with_project_is_the_golden_with_the_f271_changes` | 40, 42 |
| `tests/project_runs_w48f.rs` | `f3_f11_project_tools_on_runner_made_runs`, `f13_server_start` | 53, 55 |
| `tests/project_w48f.rs` | `f3_access_classes_read_only_and_schema` | 53, 55 |
| `src/tests.rs` | `stdio_loop_handles_a_scripted_session` | 34, 36 |

Each fails on its length assertion only. Every other test of the 31 other test files of `rcfd-mcp` passes (the first
run of twelve of them stopped on the missing binaries `rcfd` and `rcfd-runner` in the scratch target; they were built
and those twelve files run again). None of these tests was edited, ignored or skipped.

### The additive golden edit, for the lead (rule 3)

Kind: additive golden edit. In `crates/rcfd-mcp/tests/golden/tools_list_default.json` and
`tools_list_project_f270.json` the two entries `design_uq` (2779 bytes) and `design_sobol` (2210 bytes), as the server
prints them, go right after `design_ledger_rerun_check`; nothing else of either file changes (27188 to 32179 bytes and
38978 to 43969 bytes). With them, sixteen count literals rise by two: `default_f1.rs` 33, 34, the text "the 33 tools"
and `live[33]`; `tool_list_f275.rs` `kept.len()` 17 (line 128), 53 and `live[17..]`; `project_list_f270.rs` 50, 40, 33,
56, 55; `project_runs_w48f.rs` 53 twice; `project_w48f.rs` `17 + 36`; `src/tests.rs` 34. The hand list of `c5s` in
`tool_list_f275.rs` (17 kept tools called with arguments, line 262) does not read the list and passes unchanged; it does
not call the two new tools.

G3 was run on a scratch copy of c7443193 with exactly this edit, made by a script from the binary's own `tools/list`:
`--lib` (18 passed), `default_f1` (2), `tool_list_f275` (8), `project_list_f270` (1), `project_runs_w48f` (3),
`project_w48f` (10), `tool_list_adr0730` (2) and the new file (11) all pass. A first G3 run was void: the copy's two
source files had been restored with an older file time after mutation M8, the build did not see them, and the new file
failed as under M8; after the times were set and the copy rebuilt, the run above was made. The edit as a patch that
applies to c7443193 is in the scratch folder, `g3_additive_edit.patch` (not in the repository; it holds the goldens).

The Studio's capability files also need two rows each (finding 5): `apps/studio-web/capabilities/inventory.json` gains
`mcp:design_uq` and `mcp:design_sobol`, kind `mcp_tool`, access `write_files` in both modes (regenerated by
`npm run capabilities`), and `coverage.json` two rows; proposed: the exemption of the other design tools (item F-807,
unplanned). Those files were not touched and the Studio's tests were not run.

### Mutations

Each on the scratch copy, the new test file run once, the file restored.

| Id | Registered to fail | Failed |
|---|---|---|
| M1 | L1, L3, B2 | `u1_l1_l3`, `b2`, and `u2` (it reads its ledger) |
| M2 | U1, U2, S1, P1 | `u1_l1_l3`, `u2`, `s1_l2`, `p1`, and the extra test |
| M3 | U1, S1 | `u1_l1_l3`, `s1_l2`, `p1`, and the extra test |
| M4 | B2 | `b2` |
| M5 | B1 | `b1`, and `b2`, `p1`, `u1_l1_l3`, `u2`, `s1_l2` (their ledgers left the root) |
| M6 | L3 | `u1_l1_l3` |
| M7 | B3 | `b3` |
| M8 | S1, L2, P1 | `s1_l2`, `p1` |

All eight were caught by the registered checks. M1 is committed as
`crates/rcfd-mcp/tests/design_uq_sobol_f339.mutation.patch`.

### Other runs, deviations, what was not run

- `cargo clippy -p rcfd-api -p rcfd-mcp --all-targets -- -D warnings`: clean. Format: `cargo fmt -p rcfd-mcp -- --check`
  clean; `cargo fmt -p rcfd-api` fails on this machine with "The filename or extension is too long (os error 206)", so
  `rustfmt --edition 2021 --check crates/rcfd-api/src/design/tools.rs` was run in its place: clean.
- `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `python scripts/process_report.py --check`: in the
  agent's report (the generated `docs/adr/index.md` is the lead's).
- Deviations: (1) X1 as above; (2) the new file holds one test that was not registered,
  `extra_model_and_quantiles_reach_the_library` (the optional `model` keys, `quantiles`, `confidence` and string bounds
  against the library, the same equality); it passed and decides nothing; (3) the first G3 run was void, as above;
  (4) amendment 1.
- The gate runs the new test file (about 105 s for its eleven tests with the default threads, 229 s in series). No
  heavy record. Not run: the Studio's tests; the whole workspace; any crate but `rcfd-api`, `rcfd-mcp` and the link
  test of `rcfd-cli`.

Size (agent), actual: partial; 0; 0; 1; 600000

### Decision

**partial** by the registered rule, and the check that missed is X1: one existing library test of `rcfd-mcp` holds the
length of the tool list and fails until it is raised by two. Every other registered check holds, the eight mutations
are caught and the committed one fails its test. The miss is a further place of the registered edit itself (a count of
the list), not a behaviour: with the edit of the section above, X1 passes on the scratch copy (18 passed). The rule's
exception names "the three pinned list tests", so the agent does not extend it; whether the item reads done once the
lead has re-recorded is the lead's reading.

## Open questions for the maintainer

1. **Should the two studies also be service methods (`design.uq`, `design.sobol`)?** Not taken (finding 3: it changes
   the pinned method list of the service, 69, and the Studio's inventory, and `service.rs` is not a file of the item).
   Options: (a) MCP tools only, as built; (b) add the two methods in a new item, with the method list, its test and the
   Studio's inventory re-recorded, and the MCP tools then call them through the service; (c) as (b) and a CLI command.
   Recommendation: **(a)** until a second caller exists; the handlers are public functions of `design/tools.rs`, so (b)
   is a small item when it is wanted.

## For the lead: roadmap entries

Status proposed: partial.

> ADR 0784, 2026-11-23, partial: the MCP tools design_uq and design_sobol run uq::run and sensitivity::run on the named evaluator rcfd.fan_rom with the ledger, inside the sandbox and under the limit max_design_evaluations (class write_files, also in project mode); the replies equal the library's result bit for bit (256 Monte Carlo samples, a Latin hypercube study with a stopping rule, 320 Sobol evaluations) and the ledger files equal the drivers' but for the stamp; all registered checks hold but X1: seven existing tests in six files hold a count of the tool list and fail until the additive golden edit is made (two entries added to each of the two tools-list goldens, sixteen count literals raised by two), with which they pass on a scratch copy; the edit and the Studio's two inventory and coverage rows are the lead's; eight mutations caught; service methods for the two studies are open question 1 (recommended: tools only).

No follow-up item is proposed in the file.
