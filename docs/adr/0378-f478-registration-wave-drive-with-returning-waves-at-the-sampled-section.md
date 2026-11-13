# ADR 0378: F-478 registration: the wave-solver drive with returning waves at the sampled section (two or more sampled sections, the two-wave split before the drive)

Status: **REGISTERED 2026-11-12**, written before any code of F-478 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parents: [ADR 0372](0372-f452-registration-wave-solver-driven-by-a-sampled-cfd-surface.md) (F-452: `SurfaceDrive`,
`DuctFrame`, `DriveOptions`, `HarmonicModes` in `crates/rcfd-post/src/wave_drive.rs`; question 1, from which this item comes,
recommendation (a) as the follow-up; question 2, the location of the off-plane refusal); [ADR 0351](0351-f333-registration-acoustic-wave-solver-and-modal-export.md)
(F-333: the wave solver, `duct_modes`, `split_axial`, `fit_axial`, DR8 and DR9). Roadmap item F-478 (depends on F-452).
Number 0378, branch `f478-returning-wave-drive` from main ae763e4d, worktree `w49i-work/f478`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults bit-identical; no hash table or golden
touched; no new crate, no unsafe code; unsigned commits without trailers; no push. No CLI command and no case key (F-451).

## What F-478 asks

Acceptance (`docs/roadmap.json`, binding in full):

> A drive built from two sampled sections of a duct field that holds both directions reproduces the entering wave's modal
> amplitudes at the inlet within 1 percent and ignores the returning one to 1 percent; a single section with a returning wave
> is reported by a measured residual.

Clauses: (A) a closed-form duct field holding a wave entering the wave domain and a wave returning from it is sampled on two
sections into two `rcfd-surface-series` files; a drive is built from the two files alone; the wave solver run with it
reproduces the entering wave's modal amplitudes at the inlet within 1 percent; (B) the returning wave is ignored to 1 percent
(what reaches the wave domain of the returning wave is at most 1 percent of it); (C) a drive built from a single section of
such a field is reported by a measured residual (the drive states, by a number it measures, that its entering-wave
assumption does not hold).

## What was read and run before this text (disclosed)

- **Read:** the F-478, F-452, F-451 entries of `docs/roadmap.json`; ADR 0372 in full (with its addendum);
  `crates/rcfd-post/src/wave_drive.rs` in full; `crates/rcfd-post/src/duct_modes.rs` lines 1 to 70 and 180 to 660
  (`C64`, `DuctShape`, `ModeBasis`, `axial_wavenumber`, `decompose_section`, `HarmonicDft`, `AxialFit`, `split_axial`,
  `fit_axial`); `crates/rcfd-post/tests/wave_drive_f452.rs` in full (the set-up, the measurement and the runs re-used here).
- **Arithmetic (a scratch Python script, closed forms only, no rcfd code):** for the field and sections below, the
  separations `sigma` of D3 for the eight modes at both harmonics (minimum 0.4088 at harmonic 1, mode 1, cut off,
  `tanh(14.4715 x 0.03)`; at harmonic 2 the minimum is 0.7005, mode 2); the reflection ratios of D5 at section A (0.5 at
  harmonic 1, 0.58608 at harmonic 2); the residual of E1 (0.52608, 0.88823); the spacing at which the plane wave of harmonic 2
  makes the split singular (`pi / kz = 0.072 m`). These are quoted below as estimates.
- No code of F-478 exists; nothing of it was compiled or run.

## Design

**D1. Why a second section.** At one harmonic a mode's pressure along a hard-walled duct is
`a(xi) = P e^{-i kz (xi - xi_r)} + M e^{+i kz (xi - xi_r)}` (the wave entering the wave domain, `P`, and the one returning,
`M`, at the reference plane `xi_r`; ADR 0351 DR8). Both waves have the same section shape `psi_q`, so the section field of one
plane of pressure determines only `P + M` per mode: every split is consistent with it and no residual of one section can
tell them apart. Two planes give two equations per mode and determine `P` and `M`; a third and more over-determine them and
give a residual of the two-wave model (least squares, `split_axial`). The file carries pressure only (no velocity, ADR 0372
D2), hence sections, not pressure and velocity on one.

**D2. Construction, additive.** A new associated function `SurfaceDrive::from_sections(specs: &[SeriesSpec], medium, frame,
basis, opts) -> Result<SplitDrive>` in `wave_drive.rs`. It builds the F-452 drive of each section with the unchanged
`SurfaceDrive::from_series` (all of its refusals and its report apply to each section; an error is located by prefixing
`sections[i]/` to the path), so the modal amplitudes `a_{q,h}(xi_i)` of section `i` at its plane `xi_i` come from exactly the
F-452 code. The sections share the frame, the basis, the medium and the options; each keeps its own clock (the DFT phases are
taken on each file's absolute time, ADR 0372 D3, so different `t0` or sample counts are allowed as long as each covers whole
periods) and its own faces (the grids need not match). Reference plane `xi_r = xi_0`, the plane of the first section. Per
harmonic and mode, `split_axial(xi_i, a_{q,h}(xi_i), kz_{q,h}, xi_0)` gives `P` and `M` at `xi_0` with the closed-form `kz`
(no fitted wavenumber: the CFD field's propagation is the closed form's for a medium at rest). The returned `SplitDrive`
holds `drive: SurfaceDrive`, the drive of section 0 with its modal amplitudes replaced by `P` (its `section_xi` is `xi_0`), so
that `state` and `ghost_state` play back the entering wave only, with the F-452 synthesis unchanged, plus the report of D5.
`SurfaceDrive::from_series`, `state`, `ghost_state` and every existing output are unchanged; nothing runs the new code unless
it calls `from_sections` or `entering_residual`.

**D3. Conditioning (refusal).** With `u_i = e^{-i kz (xi_i - xi_0)}`, `v_i = e^{+i kz (xi_i - xi_0)}`, the normal matrix of
the split has the normalised off-diagonal `g = |sum conj(u_i) v_i| / sqrt(sum |u_i|^2 sum |v_i|^2)`; define the separation
`sigma = sqrt(1 - g^2)`. For two sections at a spacing `d`, `sigma = |sin(kz d)|` for a propagating mode and `tanh(alpha d)`
for a cut-off one (`kz = -i alpha`); the error of the sampled amplitudes is amplified by about `1/sigma` in `P`. The drive is
refused, naming the harmonic and the mode, when `sigma < 0.05` for any mode of the basis at any harmonic kept (an
amplification above 20; this includes two sections on one plane, `sigma = 0`, and a spacing of half an axial wavelength of a
propagating mode). The test is geometric (applied to every mode of the basis, whatever its content). A split whose amplitudes
are not finite (a cut-off mode with `alpha |d| > ~350`, overflow) is refused as well. Fewer than two sections are refused.

**D4. The single-section residual (clause C).** `SurfaceDrive::entering_residual(&self, check: &SeriesSpec) -> Result<Vec<f64>>`:
the F-452 drive of one section is tested against a second sampled section `check` (same frame, basis, medium; the options
of the drive, which `from_series` now keeps in a private field, set from `opts` in one added line that changes no output).
Per harmonic, with `a^c_q` the modal amplitudes of `check` (from `from_series`) at its plane `xi_c` and `a_q` the drive's at
`xi_s`, the residual is

    r_h = sqrt( sum_q |a^c_q - a_q e^{-i kz_q (xi_c - xi_s)}|^2 / sum_q |a^c_q|^2 )   (0 when the denominator is 0),

the relative misfit of the entering-wave-only prediction at the check section. For a field with `P`, `M` per mode at `xi_s`,
the numerator's term is `|M| |e^{i kz d} - e^{-i kz d}| = 2 |M| |sin(kz d)|` (propagating, `d = xi_c - xi_s`): zero for an
entering wave only, and of the order of the returning share otherwise. The residual is measured, not refused (the caller
decides).

**D5. Report of the split.** `SplitDrive` carries: `sections_xi` (the planes); `returning: Vec<Vec<C64>>` (per harmonic, per
mode, `M` at `xi_0`); `separation: Vec<Vec<f64>>` (`sigma` per harmonic and mode) and `min_separation`; `reflection: Vec<f64>`
per harmonic, `sqrt(sum_q |M_q|^2 / sum_q |P_q|^2)` (0 when the denominator is 0; the modes are orthonormal in the area
mean, so this is the ratio of the section RMS of the two waves at `xi_0`, cut-off modes included); `split_residual: Vec<f64>`
per harmonic, the RMS of the two-wave fit residuals over the modes relative to the RMS of the sampled modal amplitudes over the
sections (zero up to round-off for two sections; a measure of the two-wave model for three or more).

**D6. Question 2 of ADR 0372 (small, registered).** In `from_series`, when a face lies off the section plane, the error now
names the face farthest from the area-weighted plane (and its offset) instead of the first face found off it. Only the path
and number of that one refusal change: the order of the checks is kept (the farthest face is looked up only once the existing
test has found a face off the plane), so every accepted series and every other refusal is unchanged. Taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation), as the brief allows.

## Set-up (fixed here)

The F-452 set-up (ADR 0372 "Set-up") unchanged where not stated: `rho0 = 1.2`, `c = 340`, channel `h = 0.12 m`, `box_2d`,
`xmin` Driven, `xmax`, `ymin`, `ymax` Wall; matched layer at `x = 5 h`, thickness 0.15 m, exponent 2, `R0 = 1e-6`; frame
`origin = (-0.02, 0, 0)`, `axis = +x`, `e1 = +y`, `e2 = +z`; basis `Rectangular { a: h, b: 1 }`, modes `(m, 0)`,
`m = 0..7`; fundamental `f0 = 1180.56 Hz`, `H = 2`, ramp `4 T0`.

**The closed-form field with both directions.** With `A_{h,m}` the F-452 amplitudes at `x = 0` (`0.6 e^{0.4 i}`,
`0.3 e^{2.0 i}`, `1.0 e^{-1.1 i}` for `(1,0)`, `(2,0)`, `(2,1)`) and returning amplitudes `B_{h,m} = R_{h,m} A_{h,m}`,
`R_{1,0} = 0.5 e^{0.7 i}`, `R_{2,0} = 0.4 e^{-0.3 i}`, `R_{2,1} = 0.6 e^{1.9 i}` (as if the CFD duct had a reflecting end
downstream):

    p(x, y, t) = 3.0 + sum_{(h,m)} Re( psi_m(y) (A_{h,m} e^{-i kz x} + B_{h,m} e^{+i kz x}) e^{i h omega0 t} ).

The entering wave (the truth of the drive) is the `A` part: amplitude `A_{h,m}` at the inlet `x = 0`, velocity
`u_x = Re(kz/(rho0 omega) ...)` (ADR 0372 D4).

**The files.** Section A: the F-452 section (24 wall-clustered faces at `x = -0.02 m`, normal `-x`, `t0 = 0.0137 s`,
`dt = T0/64`, 128 samples). Section B: 20 uniform faces at `x = -0.05 m` (`y_j = (j + 1/2) h/20`, width `h/20`, depth 1,
normal `-x`), `t0 = 0.0211 s`, the same `dt`, 192 samples (3 periods) - a different grid and clock. Section C (for M2 only):
16 uniform faces at `x = -0.08 m`, `t0 = 0.0137 s`, 128 samples. Each is written with `SeriesSpec::to_json`, serialised to
text and parsed back; the drive is built from the parsed specs only. The entering-only field (E0) is the same with `B = 0`.

**Runs.** As ADR 0372: D1 (decision, heavy, `#[ignore]`): 64 x 400 cells, 320 steps per period of harmonic 2, 40 periods,
DFT over the last 4; D1c (covering): 32 x 200, 160 steps; D2 (chain): 16 x 100, 80 steps, 12 periods, no DFT. Measurement as
ADR 0372 (per column section fit on modes 0..7, `fit_axial` over `[0.5 h, 5 h]` with `x_ref = 0`); `plus` and `minus` at the
inlet. D1 and D1c are driven by `SplitDrive::drive.ghost_state()` from sections A and B. D2 runs the solver twice, once
driven by the split drive and once by the closed-form entering wave written in the test (with the same ramp).

## Registered checks

| id | clause | quantity | limit | estimate (source) |
|---|---|---|---|---|
| B1 | A | D1: `|plus - A_{h,m}| / |A_{h,m}|` for `(1,0)`, `(2,0)`, `(2,1)` | `<= 1e-2` each | the F-452 A1 numbers, `8.85e-4`, `3.52e-3`, `2.27e-3`: the ghost state is the closed-form entering wave to round-off (B3), so the scheme sees the F-452 drive |
| B2 | B | D1: `|minus| / |B_{h,m}|` at the inlet for each component (the share of the returning wave found in the wave domain) | `<= 1e-2` each | the layer's reflection only, `|minus/plus|` of F-452 A2m (`1e-6` to `1e-5`) times `|A|/|B| <= 2.5`: `<= 3e-5` |
| B3 | A, B (drive) | `SplitDrive::drive.state` (no ramp) against the closed-form entering `(p - 3, u_x)` at the 64 inlet face centres of D1 at 64 times over `T0`: max error / max `|p_entering|`, pressure and `rho0 c` times velocity; transverse velocity | `<= 1e-9`; `<= 1e-9`; 0 | round-off amplified by `1/sigma <= 2.5`: `<= 1e-12` |
| B3r | B (drive) | for each component, `|P_{h,m} e^{-i kz (x_in - x_A)} - A_{h,m}| / |B_{h,m}|`, `x_in - x_A = 0.02 m` (the returning wave left in the drive's amplitudes, carried from section A to the inlet `x = 0`) | `<= 1e-2` each | round-off, `1e-14` |
| B4 | A (chain) | D2: `max_c |p_split - p_closed| / max_c |p_closed|` after 12 periods | `<= 1e-9` | round-off, `1e-13` (F-452 A2: `1.1e-14`) |
| B1c | A (covering) | D1c: the B1 measure for each component | `<= 3e-2` each | F-452 A1c: `3.52e-3`, `1.39e-2`, `8.97e-3` |
| E1 | C | `entering_residual` of the F-452 drive of section A of the two-way field, checked against section B: `r_1`, `r_2` against the closed form of D4 computed in the test from `A`, `B`, `kz` | relative difference `<= 1e-9` each; and each `r_h >= 0.1` | `0.52608`, `0.88823` (scratch arithmetic) |
| E0 | C (control) | the same for the entering-only field | `r_h <= 1e-12` each | round-off, `1e-15` |
| N1 | C (negative control) | the F-452 drive of section A of the two-way field (the single-section assumption): its amplitudes carried to the inlet against `A_{h,m}`, relative | equals `|R_{h,m}|` within `1e-9` (absolute) each | `A + B e^{2 i kz x_A}` (D1): error `|R|` = 0.5, 0.4, 0.6 |
| M1 | report | `reflection` (0.5, 0.58608), `min_separation` (`tanh(alpha_{1,1} 0.03)` = 0.40878, computed in the test), `split_residual` | within `1e-9` relative; within `1e-9` relative; `<= 1e-12` | round-off |
| M2 | report (more sections) | sections A, B, C: the drive's `P` against the A-and-B split, max relative; `split_residual` | `<= 1e-9`; `<= 1e-12` | round-off (the field is exactly two waves) |
| R3 | refusals | one section; sections A and A (one plane); section B moved to `x = -0.092 m` (`kz d = pi` for `(2,0)`: `sigma ~ 1e-16`, the error must name harmonic 2 and mode `(0, 0)`); section B with `c0 = 341` (the error path starts with `sections[1]/`) | an error each, with the stated content | refused (D2, D3) |
| Q2 | D6 | the F-452 off-plane case (face 5 of section A moved by `1e-3 h` along `x`): the error path is `$.faces[5].x` | exact | face 5 is the farthest (offset `1.2e-4 (1 - a_5/h)` against `1.2e-4 a_5/h` for the others) |
| S1 | defaults | the existing tests of `rcfd-post`, unedited, in release: all pass; the printed output of the fast tests of `wave_drive_f452` on the branch equal byte for byte to that on ae763e4d, except the one R2 "off plane" line (D6); `cargo test -p rcfd-api --release --test baseline_hashes` passes, the file unedited | all pass; identical | unchanged code paths |

The acceptance is decided by **B1** (clause A), **B2** and **B3r** (clause B) and **E1** with its control **E0**
(clause C). B3 and B4 check the chain from the files to the face; B1c is the fast covering run of the B1 code path (D1 is
about four minutes in release, ADR 0372: an `#[ignore]`d heavy record, run once; D1c, D2 and the rest are ordinary tests).

## Planned mutations (scratch copy, once each, not committed)

| id | mutation | expected to be caught by |
|---|---|---|
| MV1 | the split's `minus` played as the entering wave (plus and minus swapped) | B3, B3r, B4, B1c |
| MV2 | the split referenced at the last section's plane while the drive keeps `section_xi = xi_0` | B3, B3r, B4, B1c |
| MV3 | the split skipped: `from_sections` returns section 0's F-452 drive unchanged | B3, B3r, B4, B1c (by `|R|`) |
| MV4 | the separation refusal removed | R3 (the `kz d = pi` case; the one-plane case may still fail in the solve, which is accepted as caught only if the error names the harmonic and the mode) |
| MV5 | the carry `e^{-i kz (xi_c - xi_s)}` dropped from `entering_residual` | E0, E1 |
| MV6 | D6 reverted (the first face off the plane named) | Q2 |

By Rule I (the lead's rule) a registered mutation not caught by its registered check makes the item partial.

## Decision rule (registered)

- **done** when B1, B2, B3r, E1 and E0 meet their limits for every component and harmonic, B3, B4, N1, M1, M2, R3, Q2 and
  S1 meet theirs, and every registered mutation is caught by at least one of its registered checks without a post-run
  amendment.
- **partial** when any registered check misses (kept as `*_known_miss`, diagnosed), or D1 could not be made, or a registered
  mutation is not caught by its registered check, or is caught only after a post-run amendment.
- **todo** does not apply once code exists.

## Limits (what is not covered)

- One duct (the two-dimensional channel), one field (three components at two harmonics, each with a returning wave), two
  sections (three in M2); no circular or three-dimensional duct run (the code takes any `DuctShape`).
- The split uses the closed-form axial wavenumbers of a hard-walled duct with the medium at rest: a CFD section in a flow, in
  a lined or non-uniform duct, or with a swirling field is not separated correctly (the medium at rest is already refused by
  F-452 for a non-zero `flow`).
- The single-section residual needs a second sampled section; one section of pressure alone carries no information on the
  split (D1), so no residual can be measured from it alone. This is a reading of clause C, stated here (question Q1 of the
  addendum).
- Periodic signals only; static sections; library only (no case key, CLI or MCP tool, F-451).

## Sources

ADR 0351 DR5 to DR9 and ADR 0372 D2 to D6, re-used. The two-wave split of a duct mode from two or more axial stations is the
standard two-microphone / multi-microphone wave decomposition of duct acoustics (Seybert and Ross 1977, Chung and Blaser
1980, cited from memory; nothing is taken from them that is not derived in D1 and D3 from the mode's axial dependence). No
vendor code or documentation was used.
