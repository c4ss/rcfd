# ADR 0197: F-254 follow-up: a named steel preset of the erosion constants (ADR 0194 Q3)

Status: **REGISTERED 2026-11-06**, written before any code of this ADR. Not yet accepted by the maintainer. The only code run
before this text was the record test R5 (check G8): it uses the code of main as it is and was run on it to make the record
below. Its first run stopped at its set-up (case D was read without its injections, which the case tree requires); the
injections were added as ADR 0194 writes them, and the second run made the record. Results go in the addendum at the end.

Parents: [ADR 0194](0194-f254-registration-particle-wall-interaction.md) (F-254: the erosion model, its keys S2, its outputs
S3, its checks D1 to D4 and E1 to E4, the records R3 and R4, and the questions Q3 and Q4),
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6 (`hash_omit_default`),
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (optional objects, the record R0 of `case_hashes_w48u.rs`),
[ADR 0163](0163-job48ac-case-hashes-h1-after-the-corrector-control-key.md) (the H1 normalisation and its control C2),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-254 (partial).

Read at main 2f60baf. Number: 0195 is the highest on main and no branch holds a 0196 or a 0197; the number 0197 was assigned
to this work by the lead, 0196 being assigned elsewhere.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0194):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- every new code path is opt-in: a case without the new key runs, hashes and writes as before, bit for bit;
- the new key uses `hash_omit_default`; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0 is
  re-recorded, and no existing test is edited (H1 below);
- no new third-party crate, no commit trailers.

## The decision this implements

ADR 0194 Q3 asked how `K`, `n` and the target density are given: (a) inputs with no default, with Table 7-2 in the docs, or
(b) defaults to the steel grades of Table 7-2. The recommendation was (a). The maintainer decided (b) on 2026-11-06, against
the recommendation. The decision reached this work through the lead, in this form: a named preset, chosen by a new optional
key (suggested name `particles.erosion.material`), that fills `K`, `n` and the wall density from the Table 7-2 values already
quoted in `docs/reference-data.md`; explicit `k`, `n`, `density` keys override it; the preset never fills in silently (the
case file names it, and the report and `particle_erosion.csv` record its name and the values used); without the key nothing
changes. The provenance warning (P7) and the checks to register came with it: the preset values, the rate bit for bit in
the library and in the case tree, the override precedence, the unknown name, the defaults and a can-fail mutation (G1 to G5,
G8 and M1 below); G6 and G7 are registered here.

So the preset is opt-in, not a default: a case that leaves `k`, `n` or `density` unset and names no preset is refused as
before. Everything else below that no instruction fixes is marked "registered here".

## What the code read shows (main 2f60baf, nothing run but R5)

1. **The keys.** `particles.erosion` is an optional object (`.optional().hash_omit_default()`) with `enabled`, `patches`,
   `k`, `n`, `density`; the three numbers default to the sentinel `-1` (unset). `check_erosion` (`rcfd-api` `particles.rs`)
   runs only with `particles.erosion.enabled` and gives a located error at `particles.erosion.<key>` for each of `k`, `n`,
   `density` not above 0. `build_tracker` passes the three values unchanged to `ErosionSpec {patches, k, n, density}`.
2. **The library.** `ErosionSpec` has four public fields and no constructor; `Erosion::new` refuses values not above 0;
   `Erosion::book` uses `k` and `n`, `depth_rate` uses `density`; `Erosion::csv` writes the header
   `face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate` and one row per face. Nothing in the library knows where
   the constants came from.
3. **The run path.** `ParticleRun::new(case, tracker, t0)` sees the case; `ParticleRun::report` adds `erosion {faces, hits,
   impact_mass, eroded, max_rate, max_depth_rate}` only when erosion is on; `write_outputs` writes `particle_erosion.csv` as
   `Erosion::csv` gives it. The service method `particles.report` uses the same `report`.
4. **Hashes.** `SettingsTree::content_hash` leaves out a field with `hash_omit_default` while it holds its default (ADR 0156
   D6). An optional object, once in the data, holds every one of its fields, so a field added to `particles.erosion` without
   `hash_omit_default` would change the hash of every case that writes the table.
5. **H1.** `case_hashes_w48u.rs` compares every case file and every parsing TOML block of `docs/` recorded in R0 (made at
   6867e40, when `particles.erosion` was an unknown key). No case file, fixture or `docs/` TOML block of main writes
   `particles.erosion` (searched for `erosion` in every `.toml`, `.json` and `.md` file of the repository: only ADRs and
   prose docs mention it, in no TOML block). A `DEFAULT_KEYS` entry for the new key would make H1 fail: its control C2
   requires every recorded valid source to hold the default of each entry, and no recorded source holds
   `particles.erosion`.

## Source

Only the Table 7-2 values already in the repository are used. `docs/reference-data.md` (F-254 table) and ADR 0194 "Sources"
quote DNV RP O501 "Erosive wear in piping systems", Revision 4.2-2007 (amended January 2011), Table 7-2, read on 2026-11-06
from a third-party copy (wiki.pengtools.com); the binding version found through dnv.com and the later DNVGL-RP-O501 editions
were not read (ADR 0194 Q4). The row used here, as quoted there: **steel grades, `K = 2.0e-9` (m/s)^-n, `n = 2.6`, density
7800 kg/m^3**. Nothing was looked up for this ADR, and no number below is new.

The angle function `F` of ADR 0194 E2 is the one DNV gives for steel grades. That is why the preset is the steel row only:
the GRP rows go with the brittle curve, which has no formula in the source and is not delivered, and the titanium row is not
steel (Q2).

## The preset

**P1. The key (registered here: the value set and the default).** `particles.erosion.material`, an enumeration with the
default `"none"` and the values `"none"` and `"dnv_o501_2007_steel"`, with `hash_omit_default`. `"none"` is the behaviour of
ADR 0194: the three constants are required. The name says the source, its edition and the row, so that a preset from another
edition can be added beside it later without renaming this one (Q1).

**P2. The table (library).** One preset:

| Name | Row of Table 7-2 | `K` [(m/s)^-n] | `n` | density [kg/m^3] |
| --- | --- | --- | --- | --- |
| `dnv_o501_2007_steel` | steel grades | `2.0e-9` | `2.6` | `7800.0` |

typed as these literals, so their values are the nearest doubles of the decimal texts, exactly as a case file that writes
`k = 2.0e-9`, `n = 2.6`, `density = 7800.0` gives them.

**P3. Precedence.** With a preset, each of `k`, `n`, `density` is the case value when the case writes a value other than the
sentinel `-1`, and the preset value otherwise. An explicit `-1` reads as unset, as everywhere in the particle keys. The case
data is never changed: the keys left unset stay `-1` in the data and in the written case, and the case file shows the
preset by its `material` line.

**P4. What a partial override may leave out (registered here).** `K` has the unit (m/s)^-n and Table 7-2 gives `K` and `n` as
a pair; a `K` with the `n` of a preset (or the reverse) is a pair that no source gives, and the model is then not specified by
the case. So, with a preset, `k` and `n` are overridden together or not at all; setting one of them alone is refused,
located at the key left unset. `density` enters only the depth rate (ADR 0194 E4) and may be overridden alone. An explicit
value not above 0 is refused at its key, with or without a preset.

**P5. Located errors.** An unknown preset name is refused when the case is read (the enumeration), at
`particles.erosion.material`, whether erosion is enabled or not. The others are made by `check_erosion`, only with
`particles.erosion.enabled` (as all erosion checks): with a preset, (i) `k` set and `n` unset, at `particles.erosion.n`;
(ii) `n` set and `k` unset, at `particles.erosion.k`; (iii) an explicit value not above 0, at its key. Without a preset the
checks and their messages are those of ADR 0194, unchanged.

**P6. Records (run path).** Only with a preset:

- `particle_report.json` gains `erosion.material {preset, row, source, k, n, density, from {k, n, density}, provenance}`:
  the preset name, `"steel grades"`, `"DNV RP O501 Rev 4.2-2007 Table 7-2"`, the three values the tracker uses, for each
  value `"preset"` or `"case"`, and the provenance text of P7;
- `particle_erosion.csv` starts with three comment lines before its header, which is unchanged:

      # erosion preset dnv_o501_2007_steel: steel grades, DNV RP O501 Rev 4.2-2007 Table 7-2
      # values used: k = 2e-9 (preset), n = 2.6 (preset), density = 7800.0 (preset)
      # <the provenance text of P7>

  with the values written as Rust's `{:?}` writes an `f64` (shortest round trip) and `(case)` for a value the case sets.

Without a preset the report and the CSV are those of ADR 0194, byte for byte (the CSV starts with its header).

**P7. The provenance text (exact).** "The Table 7-2 values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has
not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate." It is the
library constant `rcfd_particles::erosion::PRESET_PROVENANCE`, and it is written wherever a preset appears: the doc of the
preset table and of `ErosionSpec::from_preset`, the schema doc of `particles.erosion.material` (and so the JSON Schema),
the report's `provenance`, the third line of the CSV, `docs/particles.md`, `docs/case-schema.md`, `docs/reference-data.md`
and this ADR.

**P8. Library API.** In `rcfd_particles::erosion`: `ErosionPreset {name, row, source, k, n, density}`, the table
`EROSION_PRESETS`, `erosion_preset(name) -> Option<&'static ErosionPreset>`, `PRESET_PROVENANCE` and
`ErosionSpec::from_preset(patches, &ErosionPreset)`. `ErosionSpec`, `Erosion`, the booking, the CSV and the tracker are
unchanged; the library does not record a preset (the run path does, P6).

**P9. Case-tree API.** `particles::erosion_constants(case)` returns the three values and, with a preset, the preset and which
values came from it; `build_tracker` builds `ErosionSpec` from it, so with `"none"` it passes the same three `gf` reads as
before. `ParticleRun` keeps the record of P6 from `ParticleRun::new`.

**P10. Hashes and written text.** With `hash_omit_default` and the default `"none"`, the settings hash of every case without
the key, or with `material = "none"`, is unchanged (R5). The written text (`Case::to_toml`, `Case::to_json_string`) of a case
that writes `particles.erosion` now lists `material = "none"`, as ADR 0156 D6 lists `corrector_control`; no H1 source writes
the table, so H1 is not touched (read 5), and `case_hashes_w48u.rs` is not edited.

## Registered checks

All tests run in release. Library checks in `crates/rcfd-particles/tests/erosion_presets.rs`, case-tree and run-path checks
in `crates/rcfd-api/tests/erosion_presets_f254.rs`. "Bit for bit" means equal `f64::to_bits` (or equal bytes). Geometry P,
the streams of B and case D are those of ADR 0194, copied into the new test files unchanged (case D with the constants
`k = 2.0e-9`, `n = 2.6`, `density = 7800.0` written as there).

- **G1 (preset values equal Table 7-2 exactly).** `EROSION_PRESETS` has one entry, named `dnv_o501_2007_steel`, row
  `steel grades`; its `k`, `n`, `density` equal `2.0e-9`, `2.6`, `7800.0` by `==`; and they equal, by `==`, the three
  numbers parsed from the text `steel grades 2.0e-9 (m/s)^-n, 2.6, 7800 kg/m^3` of the Table 7-2 row of
  `docs/reference-data.md`, which the test finds in that file. `erosion_preset` returns it for its name and `None` for
  `"steel"`, `"none"`, `""` and `"DNV_O501_2007_STEEL"`. `ErosionSpec::from_preset` copies the three values bit for bit.
- **G2 (library: preset rate equals explicit rate bit for bit).** The run of ADR 0194 B (three streams on the plate of P,
  600 steps) once with `ErosionSpec::from_preset(["ymin"], preset)` and once with `ErosionSpec {k: 2.0e-9, n: 2.6,
  density: 7800.0}` typed in the test: the tracker fingerprints are equal word for word; for every face `rate` and
  `depth_rate` at the tracker's time are equal bit for bit; `Erosion::csv` gives the same bytes.
- **G3 (case tree: preset rate equals explicit rate bit for bit).** Case D with `material = "dnv_o501_2007_steel"` and the
  lines `k`, `n`, `density` removed, against case D: (i) `particles::track_frozen` fingerprints equal word for word; (ii)
  through the run path (`runctl::drive` and `write_results`), the preset run's `particle_erosion.csv` without its first three
  lines equals the explicit run's file byte for byte, `particles.vtp` is byte-equal, and `particle_report.json` is equal as
  JSON once `erosion.material` is removed from the preset run's report.
- **G4 (override precedence).** Through `build_tracker` (`Simulation::new` then `particles::track_frozen`), case D with the
  preset and: (a) `density = 2700.0`; (b) `k = 3.0e-9` and `n = 2.4`; (c) `k = 3.0e-9`, `n = 2.4` and `density = 2700.0`. The
  tracker's `Erosion::k()`, `n()`, `density()` are, by `==`: (a) `2.0e-9`, `2.6`, `2700.0`; (b) `3.0e-9`, `2.4`, `7800.0`;
  (c) `3.0e-9`, `2.4`, `2700.0`; each run's fingerprint equals word for word that of case D with the same three values
  written explicitly and no `material`; the fingerprint of (b) differs from that of the preset alone (the override acts).
  The report's `erosion.material.from` is (a) `{preset, preset, case}`, (b) `{case, case, preset}`, (c) `{case, case, case}`,
  and its `preset` is the preset name in all three.
- **G5 (located errors).** Each input gives a diagnostic at the stated path (from `Case::parse_toml`, `semantic_errors` or
  `Simulation::new`): `material = "steel"` and `material = ""` at `particles.erosion.material`; the preset with `k = 3.0e-9`
  only at `particles.erosion.n`; with `n = 2.4` only at `particles.erosion.k`; with `k = 0.0` and `n = 2.4` at
  `particles.erosion.k`; with `k = 3.0e-9` and `n = 0.0` at `particles.erosion.n`; with `density = -2.0` at
  `particles.erosion.density`. Controls with no diagnostic: the preset alone and the three inputs of G4. ADR 0194 D1 passes
  unedited.
- **G6 (records).** The preset alone through the run path: the report's `erosion.material` has `preset`
  `"dnv_o501_2007_steel"`, `row` `"steel grades"`, `source` `"DNV RP O501 Rev 4.2-2007 Table 7-2"`, `k`, `n`, `density`
  equal to the tracker's, `from` `{preset, preset, preset}` and `provenance` equal to `PRESET_PROVENANCE`; the first four
  lines of `particle_erosion.csv` are, exactly, the three lines of P6 (with `2e-9`, `2.6`, `7800.0`, all `(preset)`) and the
  unchanged header. G4 (a) through the run path: the second line is `# values used: k = 2e-9 (preset), n = 2.6 (preset),
  density = 2700.0 (case)`. Case D (no key): the report's `erosion` has no `material` and the first line of the CSV is the
  header. The schema doc of `particles.erosion.material` (`SettingsTree::schema_at`) contains `PRESET_PROVENANCE`.
- **G7 (the warning in the docs).** After collapsing every run of white space to one space, each of `docs/particles.md`,
  `docs/case-schema.md`, `docs/reference-data.md` and this ADR contains `dnv_o501_2007_steel`, "has not been checked against
  the maintainer's edition" and "indicative, not certified".
- **G8 (defaults unchanged).** (i) R3 of ADR 0194 (`wall_interaction.rs`, unedited) stays `fe94f34ac9fc5dbd`; (ii) R4
  (`wall_interaction_f254.rs`, unedited) stays `5514d6332741d3a4`; both compared on Windows (ADR 0137). (iii) R5, recorded on
  main 2f60baf before any code of this ADR (Windows; a settings hash has no platform value): the settings hash of case D is
  `03a6467e53b50ec2`, that of the R4 case `4b830c0bb7976b35`, and that of the R4 case with `[particles.erosion]` and
  `enabled = false` only is `4b830c0bb7976b35` too (as ADR 0194 E4 measured on its branch). After the code: the same three
  values; `4b830c0bb7976b35` for the R4 case with an inert table that also writes `material = "none"`; and
  `03a6467e53b50ec2` for case D with `material = "none"` written. (iv) ADR 0194's and ADR
  0192's suites (`wall_interaction`, `wall_interaction_f254`, `wall_film`, `wall_film_f253`), `case_hashes_w48u` H1 and
  `baseline_hashes` pass unedited, as do the particle suites ADR 0194 E3 lists.
- **M1 (can fail).** One mutation, restored afterwards and checked byte for byte against the kept copy: in
  `erosion_constants`, the preset value wins over an explicit one. Then G4 must fail. `scripts/limit_audit.py` is run on the
  two new test files and each hit is read.
- **Gate.** `cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, every test in release on a
  `git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
  `E:/projects/cfd/w49i-work/target-ero`, started only when no other gate has run for 15 minutes and about 30 GB of memory
  is free. A failure of a known flaky test is rerun alone and reported with its name.

## Not delivered

Presets for the titanium and GRP rows of Table 7-2 (Q2), presets from another DNV edition, a setup-rule warning
(`rules/setup_rules.json` is unchanged; the record is the case file, the report and the CSV), a preset in the library's own
`Erosion` record or in `Erosion::csv`, any change to the model of ADR 0194.

## Open questions and choices for the maintainer

- **Q1. The name.** `dnv_o501_2007_steel` (source, edition, row). Options: (a) keep it; (b) a shorter name such as `steel`.
  Recommendation: (a): the name carries the provenance, and a later edition can get its own name.
- **Q2. The other rows.** Titanium grades (`2.0e-9`, `2.6`, 4500) and the two GRP rows are quoted in the docs but are not
  presets. Options: (a) steel only, as delivered; (b) add titanium (same `K` and `n`, the steel angle function); (c) add GRP
  too. Recommendation: (a): the instruction named steel presets, and the GRP rows need the brittle angle function, which is
  not delivered; whether DNV pairs titanium with the steel angle function was not read here.
- **Q3. The pairing rule of P4.** Options: (a) `k` and `n` together, `density` alone, as registered; (b) each key alone.
  Recommendation: (a), for the reason in P4.
- **Q4. The edition (ADR 0194 Q4, still open).** The preset rests on a third-party copy. Recommendation: check the three
  values against the edition the maintainer holds; until then the warning of P7 stays.
