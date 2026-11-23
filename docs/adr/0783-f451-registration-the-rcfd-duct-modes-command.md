# ADR 0783: F-451 registration: the `rcfd duct-modes` command (its arguments and output, the bit-for-bit check against the library on the X1 field of ADR 0351, the two located errors with their texts, the mutations)

Status: **REGISTERED 2026-11-23, written before any code of F-451 and before any run of its checks.** Before this text no
library, CLI or test code of F-451 was written and no file of the repository was changed. What was done: reading (below),
searches of the source tree, three lines of floating-point arithmetic (below), and a compilation of the existing link
test `docs_site` to check the links of this file. Results go in addenda at the end; nothing above them is changed
afterwards.

It answers the brief `docs/adr/briefs/f451-duct-modes-command.md` ([the file](briefs/f451-duct-modes-command.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f451-duct-modes-command` from main
fcc84a84, scratch `w49i-work/f451-duct-modes-command-scratch`.

## What is asked

Roadmap item F-451 (deps F-333), the binding acceptance:

> rcfd duct-modes reads a saved pressure time series on a duct section and writes the rcfd-duct-modes export; on the ADR 0351 X1 field it reproduces the library's export bit for bit; located errors for a section outside the duct and a non-integer number of periods; the CLI help golden updated with the maintainer's approval.

Clauses: **c1** the command reads a saved pressure time series on a duct section and writes the `rcfd-duct-modes` export;
**c2** on the X1 field of ADR 0351 the export equals the library's bit for bit; **c3** a located error for a section
outside the duct; **c4** a located error for a non-integer number of periods; **c5** the CLI help golden updated with the
maintainer's approval (given on 2026-11-18 under the class rule of rule 3; the re-recording is the lead's step).

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); the item F-451 of `docs/roadmap.json` (by a script);
[ADR 0351](0351-f333-registration-acoustic-wave-solver-and-modal-export.md) (D3, D5, DR7, DR8, the set-up X1, the table
of checks, the mutations of addendum 1, remaining question 1, the follow-up acceptance, the maintainer's answer on
F-451); `crates/rcfd-post/src/duct_modes.rs` (whole); `crates/rcfd-post/tests/duct_modes_f333.rs` (whole; the test
`x1_rectangular_chain_export_and_recovery` is the X1 chain); `crates/rcfd-cli/src/cmd_fwh.rs` (whole);
`crates/rcfd-cli/tests/fwh_cli.rs` (the head); `crates/rcfd-cli/src/main.rs` (the help text and the command table);
[docs/acoustics.md](../acoustics.md) (the sections on the wave solver and duct modes, on the drive from a sampled
surface, and the reference section); `crates/rcfd-post/src/surface_series.rs` (`SeriesSpec`, `validate`, `to_json`,
`from_json`); `crates/rcfd-post/src/wave_drive.rs` (`DuctFrame`, `SurfaceDrive::from_series`);
[ADR 0372](0372-f452-registration-wave-solver-driven-by-a-sampled-cfd-surface.md) (D3 and D5: the refusals of a sampled
section and their tolerances); `apps/studio-web/tests/support/capabilities.ts` (`helpCapabilities`, `commandNames`) and
`apps/studio-web/scripts/write-capabilities.mjs` (the head); `scripts/process_report.py` (the checks of a registration);
`scripts/roadmap_proposals.py` (the form); [docs/process/tolerances.md](../process/tolerances.md) (the table).

## Findings of the reading

1. **The saved series has a format already.** `rcfd-surface-series` version 1 (`surface_series.rs`; documented in
   `docs/acoustics.md`, "Surface-series input") holds, per face, an area, a position, a normal and a pressure series on
   a uniform clock `t0 + n dt`, with `rho0` and `c0`. `wave_drive.rs` (ADR 0372) already reads it as "the pressure on a
   duct section". The command reads this format; no new input format is made.
2. **X1 is an axial export over seven sections.** The library's X1 export is `ModalExport::axial` from seven stations.
   The command therefore takes one file per section: one file gives the export of kind `section`, two or more the
   export of kind `axial`. Both are checked on the X1 field.
3. **The library needs no change.** Every function of the chain (`ModeBasis::below`, `HarmonicDft`, `decompose_section`,
   `axial_wavenumber`, `split_axial`, `ModalExport`) is public. `crates/rcfd-post/src/duct_modes.rs` is not edited.
4. **JSON numbers are exact.** The workspace builds `serde_json` with `float_roundtrip` (`Cargo.toml`, line 34), and
   `to_json` writes the shortest representation that reads back to the same `f64`; a series written by
   `SeriesSpec::to_json` is read back bit for bit.
5. **Which file is the CLI help golden.** No file under `crates/rcfd-cli/tests` holds the help text. The file that
   records the help is `apps/studio-web/capabilities/inventory.json` (generated from `rcfd --help` by
   `npm run capabilities`; `tests/unit/capabilities.spec.ts` fails when it differs from the engine), with the hand-kept
   map `apps/studio-web/capabilities/coverage.json` and the generated report `docs/studio-coverage.md`. None of the
   three is edited here (rule 3); the additive diff is reported in the addendum.

## Arithmetic done beforehand

In IEEE double precision (Python, the same operations the code will do): with `w = 60.0 * 340.0 = 20400.0` and
`f = w / (2 pi) = 3246.760839074665`, `2 pi f == 20400.0` is true and `(2 pi f) / 340.0 == 60.0` is true. So a command
that is given `f` and forms `omega = 2 pi f`, `k = omega / c0` has the `w` and the `k` of the X1 test bit for bit. The
X1 sample times are `n * period / 64.0`; a division by 64 is exact, so they equal `0.0 + n * (period / 64.0)`, the clock
`t0 + n dt` of a series with `t0 = 0`, `dt = period / 64`.

## Design (registered)

**D1. The command.** A new module `crates/rcfd-cli/src/cmd_duct_modes.rs`, one line `mod`, one line in the command table
and one help entry in `main.rs`. Nothing else of the CLI changes; no solver or library source changes.

    rcfd duct-modes <series.json> [<series.json> ...] --duct <shape> --frequency <hz> [--k-max <k>] [--x-ref <x>]
        [--origin y,z] [--out <file.json>]

- `<series.json>`: one `rcfd-surface-series` document per section, inline `p` (a document that names `pressure_csv` is
  refused by the reader's own error). The duct axis is `+x`; the section coordinates of a face are
  `(x[1] - oy, x[2] - oz)` with `--origin oy,oz` (default `0,0`): for a rectangular duct the corner, for a circular
  duct the axis. The station of a file is `x[0]` of its first face.
- `--duct rect:<a>,<b>` or `--duct circ:<radius>` (metres), required. `--frequency` in Hz, required.
- `--k-max`: the modes fitted are `ModeBasis::below(shape, k_max)`; default `k = 2 pi f / c0` (the propagating modes).
- `--x-ref`: reference position of the axial waves; default the station of the first file. With one file it is a usage
  error.
- `--out`: the file receives exactly the bytes of `ModalExport::to_json_string`. Without it, standard output receives
  those bytes and one line feed. With `--out`, standard error receives `wrote <file>`.
- Exit codes as `rcfd fwh`: 0 success, 1 a file cannot be read or written, 2 usage (no file, a missing or malformed
  option, an unknown `--` option), 3 invalid input.

**D2. The chain** (DR7 and DR8 of ADR 0351, the calls of the X1 test in the same order). `rho0` and `c0` are those of
the first file. `omega = 2 pi f`, `k = omega / c0`, `basis = ModeBasis::below(shape, k_max)`. Per file, in the order
given: a `HarmonicDft::new(omega, faces)` fed with the rows of all faces at `t0 + n dt` for `n = 0 .. N-1`;
`decompose_section(basis, points, areas, amplitudes)`. One file: `ModalExport::section(basis, f, c0, rho0, fit)`. Two
or more: per mode `split_axial(stations, amplitudes, axial_wavenumber(k, k_cut), x_ref)`, then
`ModalExport::axial(basis, f, c0, rho0, x_ref, pairs, residual)` with `residual` the largest section residual
(`0.0` folded with `f64::max` over the files in order).

**D3. Refusals (exit 3, one line on standard error, `<path>: invalid value at '<JSON path>': <reason>`; nothing is
written).** Per file, in this order: the reader's and `validate`'s errors (`<path>: <message>`, as `rcfd fwh`); a
uniform flow (`$.flow`); a rotating surface (`$.motion`); `rho0` or `c0` differing from the first file's by more than
`1e-9` relative (`$.rho0`, `$.c0`); the periods (T2); a face normal with `|n_x| < 1 - 1e-6` (`$.faces[k].n`); a face
whose `x[0]` differs from the station by more than `1e-6 L` (`$.faces[k].x`); a face outside the duct (T1); more modes
than faces (`$.faces`). `L` is `max(a, b)` or the radius. The two texts of the acceptance:

- **T1, a section outside the duct.** A face with section coordinates `(y, z)` is outside when, with `tol = 1e-6 L`,
  `y < -tol`, `y > a + tol`, `z < -tol` or `z > b + tol` (rectangular), or `hypot(y, z) > radius + tol` (circular). The
  first such face `k` of the file is named:
  `<path>: invalid value at '$.faces[<k>].x': section point (<y>, <z>) lies outside the duct`
- **T2, a non-integer number of periods.** With `N` samples and `r = N dt f`, refused when `round(r) < 1` or
  `|r - round(r)| > 1e-6 max(1, r)`:
  `<path>: invalid value at '$.faces[0].p': <N> samples of dt = <dt> s cover <r> periods of <f> Hz, not a whole number of periods`

Numbers are printed with Rust's `{}` (shortest round trip). The tolerances and both reasons are those of
`SurfaceDrive::from_series` (ADR 0372 D3 and D5, where they are derived: a file written with shortest round-trip
numbers has `|r - P|` of order `N eps`, a missing sample is off by `P/N`; a cell centre is inside by half a cell). The
JSON path locates the error in the file; the file is named first.

**D4. Help text** (two usage lines and four lines of description, added after the entry of `fwh`; no other line of the
help changes):

        duct-modes <series.json> [<series.json> ...] --duct <shape> --frequency <hz> [--k-max <k>] [--x-ref <x>]
            [--origin y,z] [--out <file.json>]
                                   modal amplitudes of a hard-walled duct at one frequency from pressure time series
                                   on duct sections (rcfd-surface-series, one file per section; <shape> is rect:a,b
                                   or circ:r): one section gives the amplitudes, two or more the waves towards +x
                                   and -x with their powers (rcfd-duct-modes format, docs/acoustics.md)

The inventory of finding 5 is expected to gain the command `cli:rcfd duct-modes` and its six options (`--duct`,
`--frequency`, `--k-max`, `--origin`, `--out`, `--x-ref`), and to lose or change nothing.

**D5. The reference of the bit-for-bit check.** The test file builds the X1 field and the library's export text by the
statements of `x1_rectangular_chain_export_and_recovery` (`duct_modes_f333.rs`, lines 393 to 466), copied: the test of
another crate cannot be called. As a guard that the copy is the X1 chain, the test asserts ADR 0351's own X1 limit on
the reference text (every amplitude read back within `1e-2`; ADR 0351 measured `4.17e-15`); this is ADR 0351's check
on the reference and is not a check of this item. The seven sections are written as seven series files with
`SeriesSpec::to_json` (`t0 = 0`, `dt = period / 64`, 128 samples, static, no flow; per face the area
`0.12 / 24 * 0.08 / 16`, the position `[x_s, y, z]`, the normal `[1, 0, 0]`, the real series
`Re(field e^{i w t})`). The command is run as
`rcfd duct-modes s0.json .. s6.json --duct rect:0.12,0.08 --frequency <f> --k-max 80 --out <file>` with `<f>` the `{}`
print of `w / (2 pi)`.

## Checks

Every row is a test of the new file `crates/rcfd-cli/tests/duct_modes_f451.rs` unless it says otherwise. "Differing
bytes" is the number of positions at which two byte strings differ, plus the difference of their lengths.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | c1, c2 | the seven X1 files (D5), `--out`: the file against the library's X1 export text | no differing byte | acceptance of F-451 | none differs (arithmetic above) | exact | code verification |
| A1s | c1, c2 | the same run without `--out`: standard output against the text and one line feed | no differing byte | acceptance of F-451 | none differs | exact | code verification |
| A1x | c1, c2 | the same files with `--x-ref 0.1`: against the library's export with the axial split at `x_ref = 0.1` | no differing byte | acceptance of F-451 | none differs | exact | code verification |
| A2 | c1, c2 | the X1 file of station `x = 0` alone: against `ModalExport::section` of the library's fit of that station | no differing byte | acceptance of F-451 | none differs | exact | code verification |
| E1 | c3 | the X1 file of station 0 with face 5 moved to `z = 0.09` (`b = 0.08`) | exit 3; standard error is T1 with `k = 5` and the point printed by the test; no output file | acceptance of F-451; ADR 0372 D5 | refused | exact | code verification |
| E1c | c3 | the unchanged X1 file of station 0 with `--duct circ:0.06` | exit 3; standard error is T1 naming the first face with `hypot(y, z) > 0.06 + 6e-8`, found by the test | acceptance of F-451; ADR 0372 D5 | refused | exact | code verification |
| E2 | c4 | the X1 file of station 0 cut to 127 samples and to 96 samples | exit 3 each; standard error is T2 with the numbers printed by the test (`r` about 1.984 and 1.5); no output file | acceptance of F-451; ADR 0372 D3 and its row R1 | refused | exact | code verification |
| E2n | c4 | negative control: the same file cut to 64 samples (one period) | exit 0 and an export that `ModalExport::from_json_str` reads | ADR 0372 D3 | accepted | exact | code verification |
| U1 | c1 | usage: no file; no `--duct`; no `--frequency`; `--duct oval:1`; `--x-ref` with one file; an unknown option | exit 2 each | D1; the exit codes of `rcfd fwh` in `docs/acoustics.md` | exit 2 | exact | code verification |
| H1 | c5 | `rcfd --help` holds the two usage lines of D4 | both present | D4; acceptance of F-451 | present | exact | code verification |
| H2 | c5 | `git diff fcc84a84 -- crates/rcfd-cli/src/main.rs` (not a test) | no removed line; added: one `mod`, one table line, the six help lines | rule 3, the additive class | none removed | exact | unchanged default output |
| B1 | c5 | `baseline_hashes`; the existing `cli`, `fwh_cli` and `docs_site` test files of `rcfd-cli` (not edited) | pass unedited | rule 3; the brief | pass | exact | unchanged default output |

## Negative controls and planned mutations (rule 16)

Negative controls: E2n (a whole number of periods is accepted); A2 (the unchanged section is accepted with the
rectangular duct, where E1c refuses it with the circular one).

Mutations of `cmd_duct_modes.rs`, each once on a scratch copy:

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| MU1 | `--k-max` ignored: the basis from `k` (five modes instead of eight) | A1, A1s, A1x |
| MU2 | the sample time `t0 + n dt` replaced by `t0 + (n + 1) dt` | A1, A1s, A1x, A2 |
| MU3 | the test of T1 removed (every face taken as inside) | E1, E1c |
| MU4 | the tolerance of T2 widened from `1e-6` to `0.6` | E2 |
| MU5 | the default of `--x-ref` taken from the last file | A1, A1s |

The committed one is MU1, as `crates/rcfd-cli/tests/duct_modes_f451.mutation.patch` with the first line
`# expect-fail: cargo test -p rcfd-cli --release --test duct_modes_f451`.

Size (agent): done; 0; 0; 0; 260000

## Decision rule

**done** if A1, A1s, A1x and A2 show no differing byte and E1, E1c and E2 give exit 3 with the registered texts (the
brief's rule: the export equals the library's bit for bit on X1 and both errors are located as registered).
**partial** otherwise, with the failing row kept as the repository keeps a known miss. E2n, U1, H1, H2 and B1 support
the decision: a failure of one of them is reported and diagnosed, and done is not proposed while it stands. Clause c5
is met by the lead's re-recording of the inventory at the merge (rule 3, the additive class); this work reports the
diff and edits none of the three files.

## What is not covered

A duct axis other than `+x` (a section must be normal to `x`); a pressure table in a CSV file (`pressure_csv`); more
than one frequency per run; a fitted axial wavenumber (`fit_axial`; the export uses the closed form, as X1); a medium
with flow; sections of a moving surface; whether the sampled field lies in the span of the fitted modes (the residual
is written, not judged); an MCP tool or a Studio control for the command; a case key. No solve is run: the X1 field is
a closed-form field.

## Open questions for the maintainer

None.

## Addendum 1: results (2026-11-23; the registered text above is unchanged)

### What was built

`crates/rcfd-cli/src/cmd_duct_modes.rs` (new, the command of D1 to D3); `crates/rcfd-cli/src/main.rs` (eight added
lines: `mod cmd_duct_modes;`, the table line and the six help lines of D4); `crates/rcfd-cli/tests/duct_modes_f451.rs`
(new, five tests) with `duct_modes_f451.mutation.patch` (MU1); the section "The `rcfd duct-modes` command" in
[docs/acoustics.md](../acoustics.md). No file of `rcfd-post`, no solver source, no golden and no hash table was
changed. The registered tests were run once, in the release profile, after the code
(`cargo test -p rcfd-cli --release --test duct_modes_f451`); all five passed.

### Checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 | no differing byte | 0 of 3337 bytes | yes |
| A1s | no differing byte | 0 of 3338 bytes | yes |
| A1x | no differing byte | 0 of 3336 bytes (the two references differ from each other) | yes |
| A2 | no differing byte | 0 of 2091 bytes | yes |
| E1 | exit 3, T1 with face 5, no output file | exit 3; `<path>: invalid value at '$.faces[5].x': section point (0.0025, 0.09) lies outside the duct`; no file | yes |
| E1c | exit 3, T1 with the first face outside the circle | exit 3; T1 with face 12, `section point (0.0025, 0.0625)`; no file | yes |
| E2 | exit 3, T2, no output file (127 and 96 samples) | exit 3 each; `127 samples of dt = 0.000004812488746307894 s cover 1.9843749999999998 periods of 3246.760839074665 Hz, not a whole number of periods`, and the same with `96` and `1.4999999999999998`; no file | yes |
| E2n | exit 0, a readable export | exit 0; eight modes read back | yes |
| U1 | exit 2 each | exit 2 in the six cases | yes |
| H1 | both usage lines in the help | both present | yes |
| H2 | no removed line of `main.rs` | 8 lines added, 0 removed | yes |
| B1 | pass unedited | `baseline_hashes` 3 passed (1 ignored, as on main); `cli` 8 passed; `fwh_cli` 2 passed; `docs_site` 2 passed | yes |

Also run: `cargo test -p rcfd-cli --release --bins` (no unit test in the crate); `cargo fmt -p rcfd-cli --check`
clean; `cargo clippy -p rcfd-cli --all-targets -- -D warnings` clean.

### Mutations (scratch copy `mut/` of the tree at the code commit, its own target folder, each once)

| Id | Caught by | Numbers |
| --- | --- | --- |
| MU1 (committed) | A1, A1s, A1x, A2, E2n | 1197, 1198, 1198 and 1033 differing bytes; E2n reads 5 modes, not 8 |
| MU2 | A1, A1s, A1x, A2 | 1977, 1978, 2028 and 793 differing bytes |
| MU3 | E1 | exit 0 and a file written; E1c stands after E1 in the same test and was not reached |
| MU4 | E2 | the 127-sample file accepted (exit 0); the 96-sample case stands after it and was not reached |
| MU5 | A1, A1s | 2370 and 2371 differing bytes; A1x 0, as expected (it gives `--x-ref`) |

Five of five caught. MU1 is also caught by A2 and E2n, which the registration did not list (both pass `--k-max 80`).

### The additive golden edit (rule 3; not made here, for the lead to re-record)

The help gains the six lines of D4 and loses or changes none (H2). By the repository's own parser
(`helpCapabilities` of `apps/studio-web/tests/support/capabilities.ts`, run on the new `rcfd --help` from a scratch
script and compared with the committed `apps/studio-web/capabilities/inventory.json`): 210 entries of `rcfd --help`
before, 217 now; 7 added, 0 removed, 0 changed. The added entries:

    {"id":"cli:rcfd duct-modes","kind":"cli_command","source":"help","usage":"duct-modes <series.json> [<series.json> ...] --duct <shape> --frequency <hz> [--k-max <k>] [--x-ref <x>]"}
    {"id":"cli:rcfd duct-modes --duct","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}
    {"id":"cli:rcfd duct-modes --frequency","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}
    {"id":"cli:rcfd duct-modes --k-max","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}
    {"id":"cli:rcfd duct-modes --origin","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}
    {"id":"cli:rcfd duct-modes --out","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}
    {"id":"cli:rcfd duct-modes --x-ref","kind":"cli_option","source":"help","parent":"cli:rcfd duct-modes"}

With them the totals in the `counts` member of the inventory rise (commands by 1, options by 6). The hand-kept map
`apps/studio-web/capabilities/coverage.json` needs an entry for the id `cli:rcfd duct-modes` and one for the prefix
`cli:rcfd duct-modes --` (as `cli:rcfd fwh` has), and `docs/studio-coverage.md` is regenerated from both
(`npm run capabilities`). Until then `tests/unit/capabilities.spec.ts` of the Studio differs from the engine.

### Deviations and disclosures

- The registration's finding 5 stands: the "CLI help golden" of the acceptance is the Studio capability inventory;
  there is no help text file under `crates/rcfd-cli/tests`.
- Before the registered run the code was compiled and linted (`cargo clippy --release`), which runs no test.
- E1c and the 96-sample case of E2 share a test with E1 and the 127-sample case, so a mutation that fails the first
  does not reach the second (MU3, MU4).

### Not run

`npm test` in `apps/studio-web` (no file of it was touched; its capability test is expected to fail until the
inventory is re-recorded); `npm run capabilities` (it writes the inventory, which is not edited here); the workspace
suite and the other integration tests of `rcfd-cli` (rule 7). No heavy record exists: the five tests take about one
second together.

Size (agent), actual: done; 0; 0; 0; 180000

### Decision by the registered rule

A1, A1s, A1x and A2 show no differing byte; E1, E1c and E2 give exit 3 with the registered texts; E2n, U1, H1, H2 and
B1 hold; every mutation is caught. **done**, with clause c5 completed by the lead's re-recording of the inventory at
the merge.

## For the lead: roadmap entries

**F-451, proposed status: `done`.** The note entry (also in `docs/roadmap/proposals/0783-F-451.json`, checked with
`python scripts/roadmap_proposals.py check`):

> ADR 0783, 2026-11-23, done: rcfd duct-modes reads one rcfd-surface-series file per duct section and writes the rcfd-duct-modes export (one section: kind section; two or more: the axial split into the waves towards +x and -x); on the seven sections of the ADR 0351 X1 field the export equals the library's text with 0 differing bytes of 3337 as a file, 0 of 3338 on standard output and 0 of 3336 with the reference position 0.1, and one section gives the library's section export with 0 of 2091; a face moved outside the duct and a series cut to 127 or 96 of its 128 samples are refused with exit 3, the file, the JSON path and the registered texts; 5 of 5 mutations caught; baseline hashes pass; no library or solver source changed; the help gains six lines and the Studio capability inventory seven entries (one command, six options), none removed or changed, which the lead re-records with a coverage entry (rule 3, additive golden edit).

No follow-up item is proposed.
