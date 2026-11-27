# ADR 0833: F-998 registration: the refused case-file replace, with the project outside the temporary folder and the case written plainly

Status: **REGISTERED 2026-11-27, written before any code of F-998 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and copying ADR 0832's scratch tools into this work's scratch folder.
Results go in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5
(`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f998-replace-outside-temp.md` ([the file](briefs/f998-replace-outside-temp.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f998-replace-outside-temp` from
091c5ff1, scratch `w49i-work/f998-replace-outside-temp-scratch`. It answers Q1 (a) of
[ADR 0832](0832-f978-registration-the-hold-of-a-refused-file.md) as the lead took it.

## What was read

`AGENTS.md`, the rules (version 5), the brief, ADR 0832 and `crates/rcfd-mcp/tests/replace_hold_f978.rs`, whole;
`crates/rcfd-mcp/tests/common/mod.rs` to `binaries`; `crates/rcfd-project/src/atomic.rs` lines 70 to 180 (`write_temp`,
`atomic_write_with`, `write_new`); `crates/rcfd-workspace/src/cases.rs` lines 125 to 195 (a new case is written by
`write_new`); ADR 0832's `trace3.ps1`, `run_x3.sh`, `analyse3.py`; F-998's acceptance (by a script). `%TEMP%` is
`C:\Users\rober\AppData\Local\Temp`; the scratch folder is on `E:`. Bytes read: about 45 000.

Arithmetic beforehand. ADR 0832's H gave 15 refusals of 900 calls. Taking 15 as a Poisson mean, P(5 or more) = 1 −
e^-15 (1 + 15 + 112.5 + 562.5 + 2109.4) = 1 − 0.00086; P(0) = 3 × 10^-7. At ADR 0830's lower rate, 7.5 per 900, P(0) =
0.00055 and P(5 or more) = 0.87.

## Design

Four arms, each ADR 0832's arm H exactly (no `--canvas` server, one spinning thread per logical processor, 16 here,
300 rounds, a marker on each refusal, CPU recorded) except as named, each run once with the trace X4 beside it:

| Arm | Project folder | Case created by |
|---|---|---|
| H | `%TEMP%\rcfd-mcp-w48f-f998-hold-h-<pid>\quick-start` (as ADR 0832) | `new_case` (`write_new`: temp file, `sync_all`, hard link, temp removed) |
| HO | `<root>\rcfd-mcp-w48f-f998-hold-ho-<pid>\quick-start`, `<root>` = `RCFD_F998_OUTSIDE` = the scratch folder's `outside` | `new_case` |
| HP | as H | a plain write by the test process |
| HOP | as HO | a plain write |

A **plain write** is `OpenOptions::new().write(true).create_new(true).open(case)` then `write_all` of the bytes of
`cases/cavity-re100.toml` (the bytes `new_case` with `copy` writes), then close: no temp file, no hard link, no sync. HP
differs from H in three things together (no hard link, no sync, the creating process is the test, not the server); its
reading names the creation path as a whole. HO differs in the folder and the volume (`C:` to `E:`) together. Each round
is: create `cases/arm-<i>.toml`, then B1's two `edit_case` (each after a `read_project_file` for the hash). The measure,
per arm: **r = refusals of the 900 calls** (300 creations and 600 `edit_case`), the `edit_case` refusals alone, the holder
of each refused file and N = FREE − t0 by `analyse3.py`.

**X4, the trace**: ADR 0832's X3 (`trace3.ps1`) copied to the scratch folder as `trace4.ps1` with two changes: the folder
pattern is `rcfd-mcp-w48f-f998-*`, and it scans `-Roots` (the outside root) besides `%TEMP%`. **TC4**, its control: ADR
0832's TC3 in the outside root (a marked file `arm-0.toml`, not the newest, held by the test for 800 ms).

**The rule that reads a cause.** The run decides only if H (the control) gives r ≥ 5. An arm is *free* with r = 0 and
*held* with r ≥ 5; r of 1 to 4 does not decide that arm. Then:

- HO free and HP held: the place (the temporary folder or its volume). Remedy option (a): a project's default place,
  with tests' projects outside `%TEMP%`.
- HP free and HO held: the creation path. Remedy option (b): a plain `create_new` write in `write_new` for case files.
- HOP free, HO and HP held: both together; options (a) and (b) together.
- HO, HP and HOP all held: neither. Remedy option (c): an `edit_case` refusal reported as transient, for a client-side retry.
- Anything else: does not decide; reported with the numbers.

No remedy is made here: each changes where projects are made, how a file is written or what a client is told, which
the brief leaves to the maintainer. No library file and no default is changed; only the test file of ADR 0832 gains arms.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` in the scratch folder, `CARGO_BUILD_JOBS` 8. Tests in
`crates/rcfd-mcp/tests/replace_hold_f978.rs`: S1 (fast, not ignored); TC4, H, HO, HP, HOP (ignored, each run once).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| S1 | the arms' set-up: HO's project under `RCFD_F998_OUTSIDE` (or `CARGO_TARGET_TMPDIR`) and not under `temp_dir()`, H's under `temp_dir()`; a plain write leaves the source's bytes and no `.tmp` file; one unloaded round of HOP: creation and both `edit_case` ok | all hold | the brief (the arms as named) | holds | exact | code verification |
| TC4 | X4 sees the test's 800 ms hold in the outside root | the test's pid seen on `arm-0.toml`; one FREE line, FREE − release in 0 to 720 ms | ADR 0832 addendum 1 (TC3 measured 479 ms at 100 % CPU) × 1.5 | seen; 480 ms | 1.5 | code verification |
| H | the control | r ≥ 5 of 900 | ADR 0832 addendum 1 (15 of 900); arithmetic above | 15 | 3 | code verification |
| HO, HP, HOP | each arm | r reported; free (0), held (≥ 5) or not deciding | the brief (the arms reported with their numbers) | — | rule | code verification |
| C | the cause | the reading of the rule above | the brief | — | rule | code verification |
| A | F-998's acceptance: 0 refusals of 900 `edit_case` by a registered change | 0 | F-998's acceptance | not met: no change made | exact | code verification |
| L1 | `cargo test -p rcfd-mcp --release --lib`; `--test replace_hold_f978` (S1) | pass | rule 7 | pass | exact | unchanged default output |
| L2 | `cargo fmt -p rcfd-mcp --check`; `cargo clippy -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `docs_site`; `roadmap_proposals.py check` of the proposal; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

Order: S1, TC4, M2, H, HO, HP, HOP, M1, M3, L1 to L3. A refusal of a file by the operating system in S1 is recorded,
not run again.

### Negative controls and mutations (rule 16)

- **N1** is H: the unchanged arm must still be held (r ≥ 5), or nothing is read.
- **M1** (committed as `crates/rcfd-mcp/tests/replace_hold_f978.mutation.patch`): the place ignores the arm's
  outside flag, so every project is made in `%TEMP%`. Must fail: S1. First line `# expect-fail: cargo test --release -p
  rcfd-mcp --test replace_hold_f978 --target-dir target/mutation s1_`.
- **M2** (scratch): X3 (`trace3.ps1`, without the outside root and with the old pattern) beside TC4. Must fail: TC4.
- **M3** (scratch): the plain write drops the last byte. Must fail: S1.

M1 and M3 are run on a scratch copy of the final commit with their own target folder.

## Size

Size (agent): partial; 1; 4; 2; 60000

(Expected partial, since no remedy is made here; one further registration, for the remedy; four heavy runs: H, HO, HP,
HOP; two questions: the remedy and the scope; about 60 000 bytes read.)

## Decision rule

**done** for F-998 if an arm meets "0 refusals of 900 edits" by a change that is made and checked and each mutation is
caught; **partial** otherwise, with the arms' numbers. No product change is made in this registration, so the expected
outcome is partial whatever the arms give; the cause is read by the rule above. A missed limit is recorded with its
number (rule 2).

## Not covered

- Which driver or service behind the System process holds the file; the machine's antivirus and indexer settings,
  which are not changed.
- HO does not tell the folder from the volume, and HP does not tell the hard link from the sync or from the creating
  process.
- Linux and macOS, where an open file does not block a rename.
