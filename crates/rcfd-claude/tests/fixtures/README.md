# rcfd-claude test fixtures

Output of the Claude Code command-line tool, version 2.1.310 (`v2.1.310/`), recorded 2026-10-26 and scrubbed: paths
are replaced by `C:\proj\demo` and `C:\proj\other`, account and environment data are removed, and every identifier is
synthetic. The structure of each line is kept.

Synthetic identifiers (the only ones in these files and in the tests that read them):

* UUIDs (session ids, message `uuid` / `parentUuid`, prompt ids and the like): `00000000-0000-4000-8000-XXXXXXXXXXXX`
  with a sequence number in the last group. `...-000000000001` and `...-000000000002` are the two session ids the tests
  name (`S1`, `S2`); `...-000000000003` is the third session in `agents.json`; the rest start at `...-000000000101`.
* API ids: `msg_01Synthetic...`, `req_01Synthetic...` and `toolu_01Synthetic...`, each with a sequence number.
* `tests/driver_fake.rs` also uses `11111111-2222-4333-8444-555555555555` and `aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee`.

The `signature` fields of thinking blocks are opaque values produced by the service and are kept as recorded.
