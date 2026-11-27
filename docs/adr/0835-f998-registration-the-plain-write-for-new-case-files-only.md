# ADR 0835: F-998 registration: the plain write for new case files only, and the arms H and P interleaved as the measure

Status: **REGISTERED 2026-11-27, written before any code of F-998 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and Poisson arithmetic. Results go in addenda at the end; nothing above
them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f998-plain-write-cases-only.md` ([the file](briefs/f998-plain-write-cases-only.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f998-plain-write-cases-only`
from 36e85a20 (ADR 0834's head 5cc8ff79 merged with origin/main c1d9e642), scratch
`w49i-work/f998-plain-write-cases-only-scratch`. It carries out the lead's answers to Q1 (b) and Q2 (a) of
[ADR 0834](0834-f998-registration-the-holds-of-a-new-case-file-and-the-plain-write.md), taken on the maintainer's
standing instruction of 2026-11-26.

## What was read

`AGENTS.md`, the rules (version 5), the brief, ADR 0834 whole, `crates/rcfd-project/src/atomic.rs` and
`crates/rcfd-workspace/src/cases.rs` whole, `crates/rcfd-mcp/tests/replace_hold_f978.rs` and
`crates/rcfd-project/tests/write_new_f998.rs` (with its patch) whole; the callers of `write_new` and `write_json_new` by
search; `create.rs` (`write_case`, `new_case`), `facts.rs` (`case_facts`), `project.rs` (`cases`), the heads of
`new_case_f271.rs` and `crates/rcfd-mcp/tests/common/mod.rs` (`scratch`, `binaries`); ADR 0834's scratch scripts
`run_x5.sh`, `runs.sh`, `analyse5.py`, `trace5.ps1`, `mutations.sh`; F-998's acceptance (by a script). The MCP server
makes a new case only through `rcfd-workspace` (`create.rs` 223 calls `create_by`), so `crates/rcfd-mcp/src` is not
changed. Bytes read: about 110 000.

Arithmetic beforehand (Poisson, h per arm). At ADR 0833's rate (18 holds in 300 rounds) H's 2000 rounds have a mean of
120, P(h(H) ≤ 7) = 6 × 10^-42; at the rate of ADR 0833 and 0834 pooled (18 in 750) a mean of 48, P(h(H) ≤ 7) = 2 ×
10^-13; at ADR 0834's 95 % upper rate (3 in 450) a mean of 13.3, P(h(H) ≤ 7) = 0.046. The rates moved between days, so
the run may not decide; the interleaving gives H and P the same machine state.

## Design

**The split.** In `crates/rcfd-project/src/atomic.rs`:

- `write_new` is put back to origin/main's text (e0c235ec, lines 159 to 179 there, its doc comment included): a temp
  file, synced, hard-linked to the name, the temp removed. Every caller keeps it: `rcfd-project` `project.rs` 129
  (manifest), 275 (`case.snapshot`), 293 (`run.json`), `trash.rs` 307; `rcfd-runner` `store.rs` 64 (launch record), 239,
  240; `rcfd-workspace` `meshes.rs` 676, 707, and `cases.rs` 180 (the data files of a new case, which are not case files
  and which `edit_case` does not replace). `rcfd-runner/src/site/agent.rs` has its own `write_new` and is not a caller.
- **`write_new_case(path, bytes)`**, new: on Windows ADR 0834's plain write unchanged (`create_new` with `share_mode(0)`,
  `write_all`, `sync_all`, close; an error of the write removes the file; `AlreadyExists` gives `ProjectError::Exists`);
  on other systems it calls `write_new` (ADR 0834 kept them unchanged, and an open file there does not refuse a rename).
- Its callers, the new case files only: `rcfd-workspace` `cases.rs` 142 (a case without data files) and 187 (a case with
  data files, written last), and the doc comment at 73. `create_with`, `create_by` and through them `new_case` and
  `start_run` with `new_case` (`create.rs` 223) reach it.

**What a writer killed mid-write leaves** (Windows): the name `cases/<name>.toml` with a prefix of its bytes (0 or more),
since the file is created under its final name and the bytes written before the kill stay; no temp file. The old path
left at most `.<name>.<pid>.<n>.tmp`. While the writer lives no reader opens it (share mode 0). After the kill, a reader
of the project treats it as any malformed case file: it is listed, `case_facts` refuses it with an error that names it,
the other cases read as before, and a `new_case` of that name is refused as existing with the bytes left unchanged. K1
checks this on the state the kill leaves (a prefix written under the name: no process is killed in the test).

**ADR 0834's W1 to W3 and N3** move to `write_new_case` (the file `write_new_f998.rs` is this work's, by the brief):
`write_new` on the old path gives no sharing violation, so W1's lower limit would refuse it. Their limits are kept.

**The arms, interleaved.** A new ignored test `f998c_block` in `replace_hold_f978.rs` runs ADR 0833's `arm_f998` with
50 rounds, the project in `%TEMP%`, `new_case` then B1's two `edit_case` per round, 16 spinning threads, its tag from
`RCFD_F998_BLOCK` (`h00` to `h39`, `p00` to `p39`). **H**: its test binary built at the commit that adds the split with
`cases.rs` still on `write_new` (origin/main's path for a new case), target folder `target-h`. **P**: built at the commit
that moves `cases.rs` to `write_new_case`, target folder `target`. Each `rcfd-mcp.exe`'s sha256 is logged. A scratch
script runs 40 pairs, pair k as H then P for even k and P then H for odd k; **before each block TC4**
(`f998_tc4_trace_control`, unchanged, run from P's binary, in `RCFD_F998_OUTSIDE` = the scratch folder's `outside`) and
the CPU (`LoadPercentage`, the average of the processors) are recorded; one trace X5 (ADR 0834's `trace5.ps1`, copied
unchanged) runs beside the whole run. The analysis `analyse6.py` gives per block: h = distinct `arm-<i>.toml` of that
block's folder with a SEEN line whose holders include `4:System`, the files looked at, the refusals; per TC4 (matched by
its pid): met or missed by ADR 0834's TC4 limit. h(H) and h(P) are the sums over the arm's 40 blocks. Each arm is run
once; a block's failure is recorded, not run again.

**The rule** (ADR 0834's, with its floor and fraction): the run decides only if **h(H) ≥ 8**; then **P passes if
h(P) ≤ 0.1 × h(H)**, and fails otherwise.

## Checks (rule 14)

Release profile; `CARGO_TARGET_DIR` under the scratch folder spelled with backslashes; `CARGO_BUILD_JOBS` 8. New fast
tests K1 and K3 in `crates/rcfd-workspace/tests/new_case_file_f998.rs` (K3 and W1, N3, N4 on Windows only).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| U1 | `git diff --numstat e0c235ec HEAD` over `crates/*/src`: files and removed lines; `write_new` text against e0c235ec's | only `atomic.rs` (0 lines removed, one added block: `write_new_case`) and `cases.rs` (at most 3 removed: lines 73, 142, 187); `write_new` equal | the brief (bit-identical to origin/main's `write_new`; every other caller on the old path) | holds | exact | unchanged default output |
| W1 | ADR 0834 W1 against `write_new_case` | partial reads = 0; sharing violations ≥ 5 | ADR 0834 W1 | 0; thousands | exact; > 100 | code verification |
| N3 | ADR 0834 N3 | partial reads ≥ 1 | ADR 0834 N3 | 6 | rule | code verification |
| W2 | ADR 0834 W2 against `write_new_case` | read back equal; no `.tmp` | ADR 0834 W2 | holds | exact | code verification |
| W3 | ADR 0834 W3 against `write_new_case` | `Exists`; bytes unchanged | ADR 0834 W3 | holds | exact | code verification |
| K1 | a prefix (the first 40 % of `cavity`'s bytes, cut mid-line) left as `cases/k1.toml` in a project: `project_info`, `case_facts` of it and of an intact case, `new_case` of `k1` | listed (cases count includes it); `case_facts` an error whose text names `cases/k1.toml`; the intact case answers; `new_case` refused, `k1.toml`'s bytes unchanged | the brief (how a reader treats what a killed writer leaves) | holds | exact | code verification |
| K3 | a lister thread reads `cases/` without pause while `create_with` makes 40 new cases of 2 MiB, the odd ones with a data file `cases/k-<i>.dat` of 2 MiB | temp names of a case file (`.k-<i>.toml.`) seen = 0; temp names of a data file seen ≥ 1 (the data files stay on the old path); listings ≥ 40 | the brief (the split); ADR 0834 W1's overlap minimum | 0; tens | exact; > 10 | code verification |
| N4 | K3's lister while `write_new` makes 20 files of 2 MiB | temp names seen ≥ 1 | the negative control of K3 | tens | rule | code verification |
| TC4 | before each of the 80 blocks | each: the test's pid seen on `arm-0.toml`; one FREE, 0 to 720 ms after release | ADR 0834 TC4 | 80 of 80 | exact | code verification |
| H | the control (N1), 2000 rounds | h(H) ≥ 8 | ADR 0834's floor (the lead's Q2 (a)) | 48 | 6 | code verification |
| P | the rule, 2000 rounds | h(P) ≤ 0.1 × h(H) | ADR 0834's fraction (the lead's Q2 (a)) | 0 | rule | code verification |
| A | F-998's acceptance: the first 900 `edit_case` of P (blocks `p00` to `p08`) | 0 refused (P's 4000 recorded) | F-998's acceptance | 0 | exact | code verification |
| L1 | `rcfd-project` (`--features test-hooks`) `--lib`, `write_new_f998`, `hammer`, `records`, `replace_refused_f978`, `edit_lock_f755`, `edit_lock_f855`; `rcfd-workspace` `--lib`, `new_case_file_f998`, `new_case_f271`, `mesh_f272`; `rcfd-runner` `--lib`, `run_records_adr0734` (target folder with backslashes, ADR 0834 amendment 2); `rcfd-mcp` `--lib`, `replace_hold_f978` (S1), `new_simulations_f271`, `edit_case_refused_f978` | pass (ignored tests stay ignored) | rule 7; F-998's acceptance (every existing replace test and caller unchanged) | pass | exact | unchanged default output |
| L2 | `cargo fmt --check` of `rcfd-project`, `rcfd-workspace`, `rcfd-mcp`; clippy of the three `--release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `docs_site`; `roadmap_proposals.py check` of `docs/roadmap/proposals/0835-F-998.json`; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

Order: U1, the fast tests (`write_new_f998`, `new_case_file_f998`), the interleaved run (TC4, H, P, A), the mutations,
L1 to L3. A refusal of a file by the operating system in a fast test is recorded, not run again.

### Negative controls and mutations (rule 16)

- **N1** is H (h(H) ≥ 8), **N2** is TC4, **N3** as in ADR 0834, **N4** above.
- **MP1** (committed as `crates/rcfd-project/tests/write_new_f998.mutation.patch`, replacing ADR 0834's): `write_new_case`
  without `share_mode(0)`. Must fail: W1. First line `# expect-fail: cargo test --release -p rcfd-project --test
  write_new_f998 --target-dir target/mutation w1_`.
- **MP2** (scratch): `write_new_case` writes all bytes but the last. Must fail: W2.
- **MP3** (scratch): `create(true).truncate(true)` for `create_new(true)` in `write_new_case`. Must fail: W3.
- **MC1** (scratch): `cases.rs` 142 back to `write_new`. Must fail: K3 (case temp names).
- **MC2** (scratch): `cases.rs` 187 back to `write_new`. Must fail: K3.
- **MC3** (scratch): `cases.rs` 180 (data files) on `write_new_case`. Must fail: K3 (no data-file temp name).

Each changes one line of a scratch copy of the final commit, built in its own target folder, run once.

## Size

Size (agent): partial; 1; 1; 0; 110000

(Expected partial, since the holds of the old path were 0 a day after 18, so the rule may not decide; one further
registration; one heavy run, the interleaved arms, about 15 minutes at full load; no question expected; about 110 000
bytes read.)

## Decision rule

**done** for F-998 if the rule decides for P (h(H) ≥ 8 and h(P) ≤ 0.1 × h(H)), the split's checks hold (U1, W1 to W3,
N3, K1, K3, N4), TC4 holds in every block, A gives 0 refusals of the 900, each of MP1 to MP3 and MC1 to MC3 is caught,
and L1 to L3 hold; **partial** otherwise, with the numbers. A run that does not decide (h(H) < 8) is partial. A missed
limit is recorded with its number (rule 2).

## Not covered

- Which driver behind the System process holds a file. The machine's antivirus and indexer settings, not changed.
- A real kill of a writer mid-write (K1 checks the state it leaves, not the kill); the removal of a half-written file on
  a write error (no fault can be injected into `write_all` without a hook).
- Linux and macOS, where `write_new_case` is `write_new`. Readers in other processes that do not retry a sharing
  violation while a new case is written.
- The data files of a new case stay on the old path; whether the System process holds them is not measured.
