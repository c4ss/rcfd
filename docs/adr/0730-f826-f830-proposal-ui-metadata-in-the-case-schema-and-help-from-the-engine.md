# ADR 0730: F-826 and F-830 proposal: UI metadata in the case schema, and help and an error catalogue from the engine

Status: **PROPOSED 2026-11-17.** Not accepted by the maintainer. Nothing is built: no Rust, TypeScript or test code was
written for this ADR, and nothing was run. The maintainer approves the commands below before any code.

Roadmap items F-826 (Engine: UI metadata in the case schema) and F-830 (Electron Studio: explain in place from the engine's
docs). F-830 depends on F-826 (`docs/roadmap.json`, F-830 `deps`), and both need the same thing from the engine: text and
flags the Studio shows but must not own. So they share this ADR. Parents: [ADR 0800](0800-electron-studio-roadmap.md),
guardrails G2, G4, G15, G16 and G19 (`docs/studio-guardrails.md`). Number 0730: 0729 is the highest ADR on main (d56933a8)
and on every branch; 08xx is the Studio range.

## Context

What the engine gives today:

- The case schema is built in code by `rcfd_api::schema::case_schema` (`crates/rcfd-api/src/schema.rs:66`). Each node is
  a `rcfd_core::settings::Schema` (`crates/rcfd-core/src/settings.rs:82`) with a kind, a `doc` string, an `optional` flag,
  a display `unit` (`settings.rs:231`, exported as `x-rcfd-unit`) and `hash_omit_default`. Floats carry a hard range and an
  SI dimension (`settings.rs:37`, exported as `x-rcfd-si-dimension`).
- The export is `Schema::to_json_schema` (`settings.rs:271`, node writer `settings.rs:279`). There is no label, no
  recommended range, no "advanced" flag and no short help. `doc` is often a paragraph: `mesh.generator`'s is over 3000
  characters (`schema.rs:75-80`).
- The service returns that export from `settings.schema` (`crates/rcfd-api/src/service.rs:429-432`), which ignores its
  params. `rcfd schema` prints the same export (`crates/rcfd-cli/src/main.rs:301-319`).
- Error codes are spread over several places: the service's `code` module (`service.rs:35-58`) and its table in
  `docs/service-protocol.md:32-46`; the workspace codes 1200-1202 and 1002-1004 (`crates/rcfd-workspace/src/lib.rs:55-66`)
  and their list in `docs/mcp-tools.md:145-149`; the MCP policy refusal 1100 (`docs/mcp-tools.md:18`); the runner's exit
  codes (`crates/rcfd-runner/src/runner.rs:26`, `crates/rcfd-runner/src/main.rs`). No record gives a recovery for a code.
- The glossary is `docs/glossary.md` (25 lines). No engine call returns it.
- Opt-in method families already exist: `data.*` is off unless the embedder calls `enable_data`, and only then does
  `service.info` report protocol 1.2 and the extra methods (`crates/rcfd-api/src/data_service.rs:30-34`, `:109-118`).

What the Studio does today:

- F-825 lays out the 948 case keys from `settings.schema` (`apps/studio-web/src/renderer/settings/schema.ts:109`, `:121`).
  With no importance metadata it guesses tiers from structure: the first 8 own keys of a section are shown, the rest are one
  step away (`apps/studio-web/src/renderer/settings/disclosure.ts:4-20`). It shows the raw `description` as a tooltip
  (`apps/studio-web/src/renderer/settings/KeyRow.tsx:74`, `:85`).
- The first path keeps its own explanatory words and cites a doc by name
  (`apps/studio-web/src/renderer/first/words.ts:47-55`). G19 and F-830 ask for no help text that lives only in the Studio.
- The Studio calls only allow-listed service methods (`apps/studio-web/src/shared/service.ts:21-36`), each through a named
  action (`apps/studio-web/src/shared/actions/registry.ts:85`, `:127-132`).

## Proposal

### P1. Metadata on schema nodes (F-826)

A schema node gets optional UI metadata. None of it is used by validation, parsing, unit conversion or hashing.

| Field | JSON key (export) | Meaning |
|---|---|---|
| label | `x-rcfd-label` | A short human name, for example "Cells in x". |
| advanced | `x-rcfd-advanced` | `true`: one step away in a learner-first layout. Absent means common. |
| recommended | `x-rcfd-recommended` `{minimum, maximum}` | A usual working range, inside the hard range. Advice only; never refused. |
| help | `x-rcfd-help` | One line, at most 160 characters, no newline. |
| see | `x-rcfd-see` | The docs page and heading that explain the key, for example `case-schema.md#mesh-generators-and-boundary-layer-inflation`. |
| unit | `x-rcfd-unit` | Only for a node that has neither `x-rcfd-unit` nor `x-rcfd-si-dimension` today (a count, a ratio), and only in the `ui: true` export. Floats with a dimension already export its text (`settings.rs:314-316`); those are not repeated. |

Storage (see Q1): a data file in `rcfd-api` keyed by dotted schema path (`mesh.nx`, `boundaries.*.velocity` with `*` for a
named-object segment), read with `include_str!` and applied when the schema is exported. `schema.rs` is not edited.

### P2. Delivery (F-826)

- Service: `settings.schema {ui: true}` returns the same reply with the P1 keys added to the nodes that have them.
  `settings.schema` with no params, or `ui: false`, returns today's reply byte for byte. An unknown param type is -32602.
- CLI: `rcfd schema --ui [--out <file>]`. `rcfd schema` without `--ui` prints today's text byte for byte.
- MCP: no new tool. Claude Code gets the same text with `rcfd schema --ui` (G16).

### P3. Help and the error catalogue (F-830)

Two read-only service methods, in an opt-in family `help.*`:

- `help.glossary {}` returns `{source: "docs/glossary.md", terms: [{term, meaning, page}]}`, parsed from the table in
  `docs/glossary.md` at build time.
- `help.errors {}` returns `{errors: [{surface, code, name, meaning, recovery}]}`. `surface` is one of `service`,
  `workspace`, `mcp`, `runner_exit`, `cli_exit`. `meaning` is the text of the existing tables; `recovery` is one new line
  per code that says what the user can do next ("Open the case and fix the setting named in `data.diagnostics`").
- Errors of these methods: -32602 for any params other than `{}`; nothing else can fail.

The catalogue is a Rust constant (`rcfd_api::errors::CATALOGUE`). Messages that the engine already returns do not change.
The Studio shows an error as the engine's message (the problem) followed by the catalogue's `recovery` for its code. That
is how "every error message names the problem and the recovery" is met without editing any reply.

CLI: `rcfd help glossary` and `rcfd help errors`, each printing the same JSON (G16).

### P4. Opt-in and protocol version

The `ui` param and the `help.*` methods are on only when the server is started with `rcfd serve --ui`. Then, as
`data_service::extend_info` does for 1.2, `service.info` adds `"ui": "rcfd-ui/1"` and the two method names to `methods`.
Without `--ui`, `service.info` and every method answer as now, and `help.*` is -32601. The Studio's spawn adds `--ui`
(`apps/studio-web/src/main/service/spawn.ts:90`) and the three names join its allow-list. See Q2 for the version number.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver, mesher, parser or default value is touched. Metadata lives outside `Kind` and outside the data, and
  `SettingsTree::content_hash` hashes the data only (`settings.rs:561-567`), so every case hash stays the same.
- The default reply of `settings.schema`, the default `rcfd schema` text, `service.info` without `--ui` and the MCP tool
  lists do not change. Tests T1 and T2 hold them to the bytes recorded at the base commit.
- `crates/rcfd-api/tests/baseline_hashes.rs` and every golden are not edited or re-recorded. No existing test is edited.

## Alternatives

1. **Metadata inline in `schema.rs` (`.label(..).advanced()`) vs a side data file keyed by path.** Inline keeps the label
   next to the key, so whoever adds a key sees it. The side file leaves the 2530-line `schema.rs` alone and can be reviewed
   as data, with a test that every path in it names a real node. Recommendation: side file (Q1).
2. **Add the keys to the default `settings.schema` reply vs opt-in.** Adding is simpler for clients, but it changes an
   existing reply, which G4 forbids without the maintainer. Recommendation: opt-in (`ui: true` behind `--ui`).
3. **A separate method `settings.ui` returning a path-to-metadata map.** It keeps the schema reply clean, but the Studio
   would have to merge two trees and keep the merge in step with named-object paths. Recommendation: no; one tree with
   `ui: true`.
4. **Short help taken automatically from the first sentence of `doc`.** No new text to write, but many first sentences
   are not good help ("A list of three numbers; ...") and some docs open with a clause. Recommendation: hand-written
   `help` where it exists, with the first sentence of `doc` as the fallback the Studio shows (Q4).
5. **The Studio reads `docs/*.md` from disk.** Free on the engine side, but it puts parsing of engine docs in the Studio
   (G2) and breaks when the docs are not installed. Recommendation: no; the engine serves them.
6. **Recovery text inside each error reply (`data.recovery`).** Changes every reply shape. Recommendation: no; by code
   from the catalogue.

## Open questions for the maintainer

1. **Where the metadata lives.** (a) a side data file in `rcfd-api` keyed by path; (b) inline builder calls in
   `schema.rs`; (c) both, inline winning. Recommendation: (a).
2. **Opt-in and version.** (a) `rcfd serve --ui`, a `ui` key in `service.info`, `protocol_version` unchanged at 1.1 (1.2
   with `data.*`); (b) the same flag with `protocol_version` 1.3 when on; (c) always on, adding to `service.info` (needs
   your approval under G4). Recommendation: (a), so versions stay a property of the method families.
3. **Who decides "advanced".** (a) the lead proposes a list from the docs, you approve it once, then each new key's
   author sets it; (b) a rule (every key outside the quick-start cases is advanced); (c) no flag, only the order.
   Recommendation: (a).
4. **Help text source.** (a) hand-written one-liners, fallback to the first sentence of `doc`; (b) only the first
   sentence of `doc`, no new text; (c) hand-written for every leaf before F-826 is done. Recommendation: (a), with a
   coverage count reported by a test but not gated.
5. **Recommended ranges.** (a) only where `docs/case-schema.md` or an ADR already states one, cited in the data file;
   (b) the lead proposes ranges from experience; (c) none in F-826. Recommendation: (a); no number without a source.
6. **Docs embedded in the binary.** (a) `include_str!` of `docs/glossary.md` (and later headings of
   `docs/case-schema.md` for `see`); (b) read from an install folder at run time. Recommendation: (a); a test fails when
   the file changes shape.
7. **Error catalogue scope.** (a) every code of the service, workspace, MCP policy and the runner and CLI exit codes;
   (b) service and workspace codes only. Recommendation: (a).
8. **Should `help.*` and `rcfd help` also be MCP tools?** (a) no, `rcfd help` covers Claude Code; (b) yes, two read
   tools (changes the tool list goldens, needs a registered restatement). Recommendation: (a).

## Tests that would prove it

- **T1** `settings.schema` with no params and with `{ui: false}` equals, byte for byte, the reply saved from the base
  commit into a new fixture before any code. `rcfd schema` the same.
- **T2** `service.info` of a service without `--ui` equals the base commit's reply; `help.glossary` is -32601 there.
- **T3** every path in the metadata file names a node of `case_schema()`; a path that names nothing fails with its line.
- **T4** every `x-rcfd-recommended` lies inside the node's hard `minimum` and `maximum`, and only on numeric nodes.
- **T5** every `x-rcfd-help` is one line of at most 160 characters; every `x-rcfd-see` names an existing docs file and
  heading.
- **T6** the export with `ui: true`, with every `x-rcfd-*` key added by this ADR removed, equals the default export.
- **T7** `help.glossary` returns every row of `docs/glossary.md`; a changed table shape fails the test.
- **T8** the catalogue holds every constant of `service::code` and every workspace code, each with a non-empty recovery,
  and agrees with the tables in `docs/service-protocol.md` and `docs/mcp-tools.md`.
- **T9** the existing case and run hash tests (`baseline_hashes.rs`, `case_hashes_w48u.rs`, `w48v_case_hashes.rs`) pass
  unedited.
- **Studio (F-825, F-830)** the disclosure reads `x-rcfd-advanced` instead of `COMMON_KEYS` when present; a test adds a
  key with metadata to a test schema and sees it placed with no Studio change; an e2e test calls every catalogue code's
  error and checks the shown text names the recovery; a static check that `first/words.ts` keeps no help text of its own;
  a view-and-report step (G19).
