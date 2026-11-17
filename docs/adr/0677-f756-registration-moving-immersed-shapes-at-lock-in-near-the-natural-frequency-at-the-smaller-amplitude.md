# ADR 0677: F-756 registration: moving immersed shapes at lock-in near the natural frequency at the smaller amplitude

Status: **REGISTERED 2026-11-17, written before any code of F-756 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-756 was written, compiled or run; the arithmetic below was done by
hand and with a few lines of Python (pure arithmetic of the case numbers and of the published numbers of ADR 0657 and ADR 0671,
no solver, no reading of a force series). Results go in the amendments and addenda at the end; nothing above them is changed
after a run.

Parents: [ADR 0671](0671-f750-registration-moving-immersed-shapes-at-lock-in-where-the-lift-does-not-cancel.md) (F-750: the
test file `crates/rcfd-fv/tests/immersed_lockin_f750.rs`, its scan, selection rule, the fixed force scale `F_L`, amendment T3 and
its corrected smoke test T3b, addendum 1 with its questions Q4 and Q5 from which F-756 was made) and
[ADR 0657](0657-f731-registration-moving-immersed-shapes-at-lock-in.md) (F-731: the set-up, the measures D3 to D5, the allowance
D4, the natural frequencies of addendum 1), and through them ADR 0648 (F-707), ADR 0631 (F-308, `MovingImmersed`), ADR 0218,
ADR 0249, ADR 0451, ADR 0060. Roadmap item F-756 (deps F-750). Number 0677 given by the lead. Branch `f756-work` from ed0249da
(F-731 / ADR 0657 and F-750 / ADR 0671 merged), worktree `w49i-work/f756`, scratch `w49i-work/f756-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; existing tests and
ignored records not edited (the F-731 and F-750 files are not touched); defaults bit-identical; no edit of `baseline_hashes.rs`,
of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, crate graph unchanged; clean-room. The
roadmap files are not edited. Every choice below that the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-756 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A cylinder oscillating across a uniform stream at Re 100 with A/D 0.25, at a forcing point between the natural frequency and 10 per cent above it chosen by a registered body-fitted scan whose conditioning is measured over the comparison's own frequency bracket, where the first harmonic of the lift is at least half the fixed cylinder's lift amplitude in the body-fitted reference, gives the shedding locked to the forcing, the mean drag, the lift amplitude and the phase of the lift against a body-fitted moving-mesh run of this repository within registered limits that allow for the natural-frequency difference of the two discretisations, stated relative and against a fixed force scale; the force series are kept for diagnosis; a negative control with the body velocity left out of the forcing misses the lift amplitude and phase.

Clauses: **c0** every limit registered here; **c1** Re 100, a cylinder oscillating across a uniform stream, A/D 0.25; **c2** the
forcing point between the natural frequency and 10 per cent above it; **c3** chosen by a registered body-fitted scan whose
conditioning is measured over the comparison's own frequency bracket; **c4** the first lift harmonic of the body-fitted reference
at least half the fixed cylinder's lift amplitude of the body-fitted reference; **c5** the shedding locked to the forcing; **c6**
mean drag, **c7** lift amplitude, **c8** lift phase, against a body-fitted moving-mesh run of this repository, within registered
limits that allow for the natural-frequency difference; **c9** the limits stated relative and against a fixed force scale;
**c10** the force series kept; **c11** the negative control with the body velocity left out of the forcing misses the lift
amplitude and the phase. The brief adds: ADR 0671 Q4 recommendation (a) (a scan at A/D 0.25 at and above the natural frequency,
the conditioning over the +-3 per cent bracket, reusing F-750's f 0.15 and 0.16 runs) and Q5 (a) (keep the phase clause, choose
points where the reference phase is far from the control's); verify the hashes of the reused series; heavy runs as ignored
records run once with fast tests over the same path; negative controls and mutations registered before the run; if the scan
finds no qualifying point, follow the rule and recommend, and do not move the bracket after seeing results.

### Readings, registered

- **The set-up, the reference, the measures and the allowance** are those of ADR 0671 (and through it ADR 0657 D2 to D5)
  without change: the body-fitted swept-volume ALE run on the 256 x 128 O-grid of Expansion(20) mapped to the square `[-8, 8]^2`
  (BF), its 128 x 64 version (BFc), started on the grid at the initial pose; the immersed run on the Cartesian D/40 core (G40);
  `Freestream` on the whole edge; PISO with linear upwind; `dt = T / 700` for every run of one point; forced runs 20 periods from
  the uniform stream, the window W their last 4; fixed runs `700 ceil(20 f / 0.14)` steps from the perturbed start; drag, lift
  harmonic `Lhat`, phase `arg(Lhat / Yhat)`, L1, L2, `R2`; `f0` from the upward crossings after `t = 80`; the slopes and the
  allowance `|s_X (r_IB - r_BF)|`.
- **"The comparison's own frequency bracket"** is the bracket of the main record: the body-fitted runs at the forcing periods of
  720 and 680 steps of the point's own `dt` (`f- = f 700 / 720`, -2.78 per cent; `f+ = f 700 / 680`, +2.94 per cent), from which
  the slopes and J1 are taken. The scan runs these two bracket runs (on BFc) at each candidate with exactly the specification of
  the main record (`Spec::forced(point, 720)` and `(point, 680)`: the candidate's `dt`, integer steps per period), and measures
  the conditioning across them (S3 below). Nothing else is used as the conditioning; in particular not the neighbours at 0.01 of
  F-750, which are 5 to 8 per cent away.
- **"Between the natural frequency and 10 per cent above it"** (c2) is the ratio `r_BF = f / f0_BF` of the forcing to the natural
  frequency of the fine body-fitted reference, measured in the main record by its fixed run BF0 at the point's `dt`: check **G0**
  `1.0 <= r_BF <= 1.1`. At the scan stage, before any fixed run at the candidate's `dt`, the screen **S0** uses the F-731 value
  `f0_731 = 0.164794` (ADR 0657 addendum 1, BF0 at `dt = 0.0102`) with a margin of half a per cent on either side: `1.005 <=
  f / f0_731 <= 1.095` (arithmetic 2 for the margin).
- **"The fixed cylinder's lift amplitude in the body-fitted reference"** is `S`, the lift half range of BF0 over its last four
  shedding cycles (ADR 0671 readings); c4 is the check **G1** `|Lhat_BF| >= 0.5 S` in the main record.
- **"A fixed force scale"** is `F_L = S + m_a A omega^2`, `m_a = rho pi r^2` (ADR 0671 readings and its Q2 (a)); every lift
  limit is stated relative to `|Lhat_BF|` and against `F_L`, both binding (c9); drag is relative to the reference drag, which is
  itself a fixed scale of the drag (no cancellation is possible in a mean of order 0.8). The numbers against `S` alone are
  reported.
- **"Locked"** (c5): L1 and L2 of every compared forced run (ADR 0657 D5), and in the scan L1 of the candidate and of both its
  bracket runs.
- **"Kept"** (c10): every run of both stages writes its force series to the folder `RCFD_F756_SERIES_DIR` (default
  `CARGO_TARGET_TMPDIR`), read back bit for bit (K1), FNV-1a hash printed and recorded. The records are run with the folder
  `w49i-work/f756-scratch/series`.
- **"Misses the lift amplitude and phase"** (c11): the negative control's lift amplitude lies outside both amplitude limits (A2,
  A2L) and its phase outside the phase limit (A3) that the immersed run must meet, with the immersed run's allowance (N1b, N1c);
  its complex harmonic also misses (N1d). Its drag is reported (ADR 0657 Q4 (a)). Following ADR 0671 Q5 (a), the scan screens the
  point so that the control can separate (S4).
- **The reuse of F-750's runs** (brief, ADR 0671 Q4 (a)): F-750's coarse A/D 0.25 runs at f 0.15 and 0.16 (and its other ten
  scan series, copied to `w49i-work/f756-scratch/f750-series`) are read, their FNV-1a hash compared with the hashes of ADR 0671
  amendment 1 (check **SR**), and their measures recomputed from the file. They are reported as the trend below the natural
  frequency and are not used by the selection rule: their frequencies (0.15, 0.16) lie below `f0` and they are not runs of any
  candidate's bracket (a different `dt`). A file whose hash differs is not used and is reported.

## What was read

`w49i-work/AGENT_RULES.md`; the brief; ADR 0671 (all, with amendments T3 and 1, addendum 1 and the "For the lead" section);
ADR 0657 (all); the F-750 test file (all); the F-750 logs `w49i-work/f750-scratch/logs/scan_record.log` (the run times, the S1
lines) and `fast_tests.log` (the duration of the fast tests, 53 s). The force series of F-750 were listed, not read.

## Arithmetic before this registration (disclosed)

1. **What F-750 found at A/D 0.25** (ADR 0671 amendment 1, BFc): `|Lhat|` 0.029, 0.052, 0.092, 0.154 at f 0.13 to 0.16 (r
   0.79 to 0.97 of `f0_731`), phase -178.5, +148.8, +122.0, +97.0 deg, L1 at most 1.8e-3; the coarse negative control (G20) at
   f 0.14 and 0.15: +46.6 and +37.1 deg, `|Lhat|` 1.06 and 1.04. The harmonic grows toward `f0` and the phase falls by 25 to 33
   deg per 0.01. Extrapolated by hand (not a check, only the framing of the candidates): at f 0.17 to 0.18 the harmonic of order
   0.2 to 0.4 if the growth continues past `f0`, the phase of order +40 to +75 deg, the control's phase of order +10 to +25 deg,
   so a separation of order 30 to 60 deg; the lock-in range at A/D 0.25 above `f0` is not known from this repository's data
   (published pictures of forced wakes, cited from memory and used only for this framing, put its upper edge at A/D 0.25 and Re
   100 of order 10 to 20 per cent above `f0`).
2. **Candidates and the margin of S0.** Candidates `C1 = 0.17`, `C2 = 0.175`, `C3 = 0.18` (A/D 0.25), a step of 0.005 as ADR
   0671 Q4 (a) proposed; `f / f0_731` 1.0316, 1.0619, 1.0923. The value 0.165 of Q4 (a) has `f / f0_731 = 1.00125`, so a shift of
   `f0` by more than 0.13 per cent between the F-731 `dt` (0.0102) and the point's `dt` (0.00866), or the difference between BF0
   and BFc0 (0.39 per cent in F-731), could put it below `f0`; S0 with a half-per-cent margin excludes it, and it is not run. The
   F-750 run at 0.16 (r 0.971) stays the lowest data point. The margin: implicit Euler's frequency error on an oscillator is
   of order `(omega dt)^2 / 3`, 4e-5 at these steps; the wake's own response to `dt` is not measured here and is taken as at most
   a few tenths of a per cent, so half a per cent; G0 checks `r_BF` at the chosen point in the main record anyway.

   | Point | f | `f / f0_731` | dt = T / 700 | bracket `f-`, `f+` | their `f / f0_731` | `m_a A omega^2` | `F_L` with `S731` | fixed steps |
   | --- | --- | --- | --- | --- | --- | --- | --- | --- |
   | C1 | 0.170 | 1.0316 | 8.40336e-3 | 0.165278, 0.175000 | 1.0029, 1.0619 | 0.2240 | 0.4016 | 17 500 |
   | C2 | 0.175 | 1.0619 | 8.16327e-3 | 0.170139, 0.180147 | 1.0324, 1.0932 | 0.2374 | 0.4150 | 17 500 |
   | C3 | 0.180 | 1.0923 | 7.93651e-3 | 0.175000, 0.185294 | 1.0619, 1.1244 | 0.2512 | 0.4288 | 18 200 |

   The bracket of C1 reaches down to `r = 1.003` and that of C3 up to 1.124; only the point itself is held to c2 (G0).
3. **Time steps and stability.** Courant on BFc about `(1.5 + A omega) dt / 0.0183`: 0.81 (C1) to 0.77 (C3); F-750's A/D 0.25
   BFc runs reached at most 0.76 at a larger `dt`. BF: the F-731 Courant 1.45 at `dt = 0.0102` scales to at most 1.2. Diffusion
   numbers below F-731's. Body speed `A omega` 0.267 to 0.283; per step `2 pi A / 700 = 0.0022`, 0.09 of an IB cell. The lock
   measure discriminates less near `f0`: X1 of a fixed run is about `2 abs(sin(pi f0 / f))`, 0.19 at C1, 0.36 at C2, 0.53 at C3
   (with `f0_731`); at C1 a rise of `f0` by 1.3 per cent would bring it to the limit 0.1. This is a property of the measure at
   that point, disclosed here, not a reason to change the limit.
4. **Error estimates behind the limits** (as ADR 0671 arithmetic 5 for its C4-like point): immersed lift error about 3 per cent of
   the vortex part plus 1 to 2.5 per cent of the added-mass part (0.22 to 0.25): about `0.009 + 0.004 = 0.013`, 3.2 per cent of
   `F_L` (about 0.40 to 0.43); relative to `|Lhat_BF|` (expected 0.15 to 0.4) 3 to 9 per cent. Near the natural frequency the
   response is more sensitive to `r` than at F-731's point; the allowance takes the measured slope times `Delta r` (F-731: `Delta
   r = 3.8e-4`), so the base limits are kept from ADR 0671: drag 0.04; amplitude 0.08 relative and 0.06 of `F_L`; phase 8 deg;
   complex 0.12 relative and 0.08 of `F_L`; mean lift 0.02 of `F_L`; the coarse reference 0.03 (drag), 0.08 (complex, relative),
   0.05 (complex, `F_L`). L2 of the immersed run 0.10 and `R2 / F_L` 0.03 (ADR 0671 arithmetic 6; the body speed is 0.6 of
   F-731's, the cell crossings fewer). The scan's conditioning limit (S3) is set at 0.8 of the J1 limits (0.20 and 20 deg), so
   that the BFc-to-BF difference (F-731 R1: 0.086 of `|Lhat|`) cannot alone turn a screened bracket into a J1 miss.
5. **The negative control's separation in the scan (S4)** as ADR 0671 D2: three times the base limits, 24 deg and 0.24.
6. **Cost.** From the F-750 scan log: BFc at A/D 0.25, 14 000 steps, 3 145 to 3 351 s each with twelve runs in parallel; G20
   N1 4 058 to 4 342 s. Stage S (twelve runs: nine BFc of 13 600 to 14 400 steps, three G20 N1 of 14 000) about 1.2 to 2 h wall.
   Stage M as F-731 (15 427 s wall, the slowest BF- of 14 400 steps), here with fixed runs of 17 500 to 18 200 steps: 4 to 8 h
   wall. Memory below 1 GB per run (31 GB free at registration).

## Design

### D1. No library change

All of F-756 is test code in a new file `crates/rcfd-fv/tests/immersed_lockin_f756.rs`. The helpers of the F-750 file are copied
(that file and the F-731 file are not edited); the forcing point, candidates, scan and selection rule are those of this ADR. No
library line changes; the default bits are unchanged by construction.

### D2. Stage S, the scan (heavy record `f756_scan_record`, run once)

Runs, all at A/D 0.25 from the uniform stream, 20 of their own periods: for each candidate `c` in C1, C2, C3, BFc (128 x 64) at
`Spec::forced(c, 700)`, `Spec::forced(c, 720)` and `Spec::forced(c, 680)` (nine runs); N1 on G20 (velocity left out of the
forcing) at `Spec::forced(c, 700)` (three runs). Measures over W. Also SR: the twelve F-750 series in
`w49i-work/f756-scratch/f750-series` (path from `RCFD_F756_F750_SERIES`; SR is skipped and reported when the variable is unset),
hashed and compared with ADR 0671 amendment 1, their measures printed for the A/D 0.25 files.

A candidate `c` with bracket runs `c-` (720 steps per period) and `c+` (680) **qualifies** when all of:

- **S0 (between `f0` and 10 per cent above, with margin)**: `1.005 <= f_c / 0.164794 <= 1.095`;
- **S1 (locked with margin)**: L1 <= 0.01 in BFc at `c`, `c-` and `c+`;
- **S2 (large harmonic)**: `|Lhat_c| >= 0.75 S731`, `S731 = 0.177620`, i.e. `>= 0.133215` (BFc);
- **S3 (well conditioned over the bracket)**: `m_c = | |Lhat_c+| - |Lhat_c-| | / |Lhat_c| <= 0.20` and `|wrap(phi_c+ -
  phi_c-)| <= 20 deg`;
- **S4 (the control can separate)**: N1 (G20) at `c` against BFc at `c`: `|wrap(phi_N1 - phi_c)| >= 24 deg` and `| |Lhat_N1| -
  |Lhat_c| | / |Lhat_c| >= 0.24`.

**Selection:** among the qualifying candidates the one with the smallest `m_c`; a tie (within 1e-12) goes to the lower
frequency. Implemented as a function and tested on synthetic tables (T8). If no candidate qualifies, no main record is run, the
scan is reported, and the decision is partial; the candidates and the bracket are not changed after the scan (a new scan needs a
new registration).

Amendment 1 (written after the scan and committed before any main run) records the scan's numbers, the rule's outcome and the
selected point, and sets the point constant in the test file in the same commit; nothing else of this registration changes.

### D3. Stage M, the main record (heavy record `f756_lockin_record`, run once at the selected point)

The nine runs of ADR 0671 D3 at `(0.25, f*)` with `dt = (1 / f*) / 700`: BF0, BFc0, IB0 (fixed, perturbed start, `700 ceil(20
f* / 0.14)` steps); BF, BF- (720 steps per period), BF+ (680), BFc, IB, N1 (G40, velocity left out) forced, 20 periods.
Measures, natural frequencies, slopes, allowances, the fixed-scale slopes and `R2` exactly as ADR 0671 D3, plus G0. Every series
is written and read back (K1). The main BFc forced run has the same specification as the scan's run at `f*`; whether its series
hash equals the scan's is reported (K2, the determinism of the path; not a check of the decision).

### D4. Not built

Other amplitudes or frequencies than the scan's three candidates and their brackets; a map of the lock-in range or of the phase
jump; rotating, deforming or freely moving bodies; BDF2; second-order immersed faces; three dimensions; a case-file key (ADR 0648
Q1).

## Checks

Stage S: **SS1** every scan run finishes without a stop with finite forces, its series written and read back bit for bit (K1);
**SR** reported (hashes of the reused F-750 series); the rule of D2.

Stage M, heavy record (run once, prints every measure before it asserts). `Delta r = r_IB - r_BF`, `Delta r_c = r_BFc - r_BF`;
N1 uses the IB allowance.

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| G0 | the forcing point (c2): `r_BF = f* / f0_BF` | `1.0 <= r_BF <= 1.1` | 1.03 to 1.09 (arithmetic 2) |
| G1 | the forcing point (c4): `|Lhat_BF| / S` | >= 0.5 | 0.75 to 2.5 (the scan's S2) |
| F1 | fixed runs BF0, BFc0, IB0: at least 9 upward crossings after t = 80; first and last four cycles: frequency within 0.005, half range within 0.02 (relative) | as stated | F-731: 1e-5 |
| F0 | `abs(f0_IB / f0_BF - 1)`, `abs(f0_BFc / f0_BF - 1)` | <= 0.045 each | F-731: 4.4e-4, 3.9e-3 |
| F2 | reported: `St` of each, `r_BF`, `r_IB`, `r_BFc`, the bracket | reported | |
| L1 | lock, lagged: IB, BF, BF-, BF+, BFc (c5) | <= 0.03 each | the scan's S1 (<= 0.01) |
| L2 | lock, spectral: BF, BF-, BF+, BFc | <= 0.05 each | F-731: 1e-4 |
| L2I | IB: L2 relative; `R2 / F_L` | <= 0.10; <= 0.03 | below 0.05; about 0.01 |
| X1 | L1 of the fixed BF0, IB0 at the forcing period over their last `4 x 700` steps | > 0.1 each | `2 abs(sin(pi f0 / f))`: 0.19 (C1) to 0.53 (C3) |
| J1 | across the bracket: `abs(wrap(phi+ - phi-))`; `abs(abs(Lhat+) - abs(Lhat-)) / abs(Lhat_BF)` | <= 25 deg; <= 0.25 | the scan's S3 (<= 20 deg; <= 0.20) |
| A1 | mean drag `abs(Fbar_x,IB - Fbar_x,BF) / Fbar_x,BF` (c6) | <= 0.04 + `abs(s_D Delta r)` | F-731: 0.0037 |
| A2 | lift amplitude relative (c7, c9) | <= 0.08 + `abs(s_A Delta r)` | 0.03 to 0.09 |
| A2L | lift amplitude against `F_L` (c7, c9) | <= 0.06 + `abs(s_A^L Delta r)` | 0.01 to 0.03 |
| A3 | lift phase `abs(wrap(phi_IB - phi_BF))` (c8) | <= 8 deg + `abs(s_P Delta r)` | 2 to 6 deg |
| A4 | lift harmonic, complex, relative (c7, c8, c9) | <= 0.12 + `abs(s_C Delta r)` | 0.03 to 0.10 |
| A4L | lift harmonic, complex, against `F_L` (c9) | <= 0.08 + `abs(s_C^L Delta r)` | 0.015 to 0.04 |
| A5 | mean lift `abs(Fbar_y,IB - Fbar_y,BF) / F_L` | <= 0.02 | F-731: 2e-6 of `F_L` |
| R1 | BFc against BF: drag as A1; complex relative as A4; complex against `F_L`, each with `Delta r_c` | <= 0.03 + allow.; <= 0.08 + allow.; <= 0.05 + allow. | F-731: 0.004; 0.086; 0.0085 |
| N1b | velocity left out (G40): lift amplitude as A2 and as A2L (c11) | > the A2 and > the A2L limit (must exceed) | the scan's S4 (N1 about 1.0 against 0.15 to 0.4) |
| N1c | N1: lift phase as A3 (c11) | > the A3 limit | the scan's S4: at least 24 deg at G20 |
| N1d | N1: complex harmonic as A4 and as A4L | > the A4 and A4L limits | |
| N1a | N1: mean drag as A1 | reported | F-731: 0.022 |
| K1 | every run of both stages: the series file read back equals the in-memory series bit for bit; its FNV-1a hash printed (c10) | bit for bit | |
| K2 | the main BFc series hash against the scan's at `f*` | reported | equal (the path is deterministic in F-731 and F-750's repeated fast tests) |
| S1 | every run: no divergence stop, finite force and fields; largest step continuity and Courant reported | no stop | |
| P1 | reported: the measures per period of W, `C_a`, `C_v`, the numbers against `S` alone, L1 and L2 of N1 | reported | |

Fast tests (not ignored; release):

| Id | Check | Limit |
| --- | --- | --- |
| T1 | the BF, BFc and smoke meshes mapped to the square (as ADR 0671 T1); the smallest cell volume positive at the poses `-0.25, 0, 0.25` | as ADR 0671 T1 |
| T2 | for each candidate and its bracket: `dt` bit-equal to `(1/f_c)/700` (the candidate's, also for the bracket runs), integer steps per period, the bracket frequencies `f_c 700 / 720` and `f_c 700 / 680` within 1e-15 relative, fixed steps `700 ceil(20 f / 0.14)` (17 500, 17 500, 18 200 at f 0.17, 0.175, 0.18), the motion amplitude 0.25; the measures on the synthetic series of ADR 0671 T2 on each step grid; L1 of the off-frequency sinusoid against the closed form; `f0` of D3; the allowance; `F_L` against `S + pi r^2 A omega^2`; `R2` of a known sixth harmonic | as ADR 0671 T2 (1e-12; 1e-9 deg; 1e-3; 1e-6; 1e-14) |
| T3 | IB smoke (G20, each of C1 to C3, 40 steps, with and without the velocity): no stop, finite force, the forcing after each step equals `forcing_at(solver.time)`, the solid cells hold `(0, ydot)` (or 0) within `1e-3 U` (the absolute limit of ADR 0671 amendment T3) | as stated; estimate 2.8e-4 to 3.8e-4 |
| T4 | BF smoke (64 x 32, C1 and C3, 40 steps): continuity <= 1e-6; the `cylinder` points' mean `(0, y_c(t_n))` within 1e-12; far field unchanged bit for bit | as stated |
| T5 | the body-fitted solver starts on the grid at the initial pose (C1, C3 and a bracket specification of C1): the solver's points equal `y0 + y_c(0) w` bit for bit, centre `(0, -0.25)` within 1e-12, largest difference 0.25 within 1e-12; the fixed specification starts on the unmoved points | as stated |
| T6 | fixed set-ups smoke (G20 IB and 64 x 32 BF at the dt of C1, 20 steps), the initial field the perturbation bit for bit, the IB solid cells within 1e-3 after the first step | as ADR 0671 T6 |
| T7 | the series file round trip (as ADR 0671 T7) | bit for bit |
| T8 | the selection rule of D2 on synthetic tables: all three qualify, the smallest `m_c` picked; a tie picks the lower frequency; one table each where the otherwise best candidate fails exactly one of S1 (a bracket run's L1 0.011, once for `c-` and once for `c+`), S2 (`|Lhat| = 0.133`), S3 (the bracket's amplitude change 0.21 of `|Lhat_c|`; and its phase change 21 deg), S4 (N1 phase 23 deg off), so that the next is picked; S0 of the frequencies 0.165 and 0.181 false, of 0.17, 0.175, 0.18 true; a table where none qualifies gives none | exact |

### Negative controls

N1 (heavy, c11) at G40 and its screening at G20 in stage S; its fast half in T3 (velocity left out). X1 is the control of the
lock measure. T8's single-criterion tables are the controls of the selection rule.

### Planned mutations (each run once against the fast tests, on a scratch copy)

| Id | Mutation | Expected to catch |
| --- | --- | --- |
| M1 | BF solver built on the unmoved grid (the F-707 survivor) | T5 |
| M2 | L1 with the lag `N_p - 1` | T2 |
| M3 | `f0` of D3 dividing 8 cycles by the span of 7 | T2 |
| M4 | `F_L` without the added-mass term | T2 |
| M5 | fixed steps with `floor` instead of `ceil` | T2 |
| M6 | the series writer with 6 significant digits | T7 |
| M7 | selection S2 against `0.5 S731` instead of `0.75 S731` | T8 |
| M8 | selection S1 without the bracket runs' lock | T8 |
| M9 | selection without S4 | T8 |
| M10 | library (scratch copy of `immersed_motion.rs`): `bodies_at` leaves out the motion velocity regardless of the flag | T3 |
| M11 | the run specification's motion amplitude fixed at 0.5 regardless of the point | T2, T5 |
| M12 | the bracket specification given its own `dt = T- / 700` instead of the candidate's | T2 |
| M13 | S3 measured from `c+` to `c` only (the change `| |Lhat_c+| - |Lhat_c| |`), leaving out `c-` | T8 |
| M14 | S0 left out (every frequency accepted) | T8 |

## Decision rule

- **done**: G0, G1, F1, F0, L1, L2, L2I, X1, J1, A1, A2, A2L, A3, A4, A4L, A5, R1 met; N1b, N1c, N1d exceeded; K1 and S1 for
  every run of both stages (SS1); T1 to T8 pass on their first run; at least twelve of the fourteen mutations caught, M1 among
  them; no library line changed.
- **partial**: any of these missed (with the diagnosed cause), a run stopped, no candidate qualified at the scan, or a check met
  only after an amendment other than amendment 1 of D2.
- **todo**: the case cannot be built or run.
- F2, K2, N1a, P1 and SR are reported and do not change the decision.

## Limits (what this does not cover)

One forcing point compared (the scan's three candidates and their brackets only in the coarse reference), A/D 0.25, Re 100, two
dimensions, one mesh pair, the square domain of half side 8 D with its short wake; the time step shared by all runs of one point,
so the time error is common and not measured; the reference is this repository's body-fitted run, not a published data set; the
lock-in range and the phase jump are not mapped. The screening uses the coarse reference, the coarse immersed mesh for the
negative control and the F-731 natural frequency for S0; the binding checks are on the fine meshes. The allowance assumes the
response depends on the forcing through `f / f0` near the point. First-order forcing of ADR 0218. Items of D4.

## Open questions

- **Q1. A case-file key for moving immersed bodies.** As ADR 0648 Q1. Open (schema golden), not taken.
- **Q2. The candidates.** Options: (a) 0.17, 0.175, 0.18 with the S0 margin, leaving out 0.165 (arithmetic 2); (b) 0.165 to 0.18
  with S0 against `f0_731` without margin. Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation): (b) would let the rule pick a point that G0 may then fail by a few tenths of a per cent.
- **Q3. The conditioning screen.** Options: (a) the bracket's end-to-end change at 0.8 of the J1 limits, as registered; (b) the
  J1 limits themselves. Taken (a) on the standing instruction (arithmetic 4).

## Amendment 1 (2026-11-17, the scan of D2; written after the scan and before any main run)

The scan record `f756_scan_record` was run once (test file 8a8a666c, release, twelve threads, 3 438 s wall on the shared machine;
log `w49i-work/f756-scratch/logs/scan_record.log`). Every run finished (SS1): largest step continuity 7.2e-10, largest Courant
0.59; every series written to `w49i-work/f756-scratch/series` and read back bit for bit (K1). SR: the twelve F-750 series all
have the hashes of ADR 0671 amendment 1, and the measures recomputed from the A/D 0.25 files equal ADR 0671's printed digits.

| Run | f (dt of) | L1 | `abs(Lhat)` | `/ S731` | phase (deg) | mean drag | `C_a`, `C_v` | series hash |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| BFc C1 | 0.170 (0.170) | 6.6e-5 | 0.24923 | 1.403 | +72.09 | 0.8309 | 0.34, -0.47 | c4f87f3591a650f4 |
| BFc C1- | 0.165278 (0.170) | 3.4e-6 | 0.19817 | 1.116 | +84.01 | 0.8132 | 0.10, -0.39 | b718d6edfdee2845 |
| BFc C1+ | 0.175000 (0.170) | 6.3e-4 | 0.32074 | 1.806 | +58.61 | 0.8467 | 0.70, -0.55 | 012e42eee3f0a7dc |
| BFc C2 | 0.175 (0.175) | 6.3e-4 | 0.31974 | 1.800 | +58.75 | 0.8466 | 0.70, -0.55 | 9ef2d21ebbc6a854 |
| BFc C2- | 0.170139 (0.175) | 7.0e-5 | 0.25018 | 1.409 | +71.84 | 0.8312 | 0.35, -0.48 | 2a703fb17a545ecf |
| BFc C2+ | 0.180147 (0.175) | 1.9e-3 | 0.43508 | 2.450 | +40.95 | 0.8534 | 1.31, -0.57 | 38b9af7f307c0678 |
| BFc C3 | 0.180 (0.180) | 2.0e-3 | 0.42921 | 2.417 | +41.79 | 0.8536 | 1.27, -0.57 | edf6cda85cd915ff |
| BFc C3- | 0.175000 (0.180) | 6.2e-4 | 0.31881 | 1.795 | +58.88 | 0.8465 | 0.69, -0.55 | 3021c78fc1036546 |
| BFc C3+ | 0.185294 (0.180) | 2.2e-2 | 0.63051 | 3.550 | +1.37 | 0.7707 | 2.37, -0.03 | bfc604227c817292 |
| N1 G20 C1 | 0.170 | 9.4e-3 | 0.78286 | | +11.87 | 0.8710 | 3.42, -0.32 | fe71184bbdb4fd9e |
| N1 G20 C2 | 0.175 | 8.8e-3 | 0.72154 | | +9.20 | 0.8554 | 3.00, -0.23 | ff0fbc3ee2b8b4c2 |
| N1 G20 C3 | 0.180 | 7.1e-3 | 0.67205 | | +8.34 | 0.8444 | 2.65, -0.19 | 36f35a8eb9021646 |

The rule of D2:

| Candidate | S0 | S1 (L1 <= 0.01, c and bracket) | S2 (`>= 0.133215`) | S3 (bracket: `m_c <= 0.20`, phase <= 20 deg) | S4 (N1: >= 24 deg, >= 0.24) | qualifies |
| --- | --- | --- | --- | --- | --- | --- |
| C1 (0.170) | yes (1.0316) | yes | yes (0.249) | **no** (`m_c` 0.492, phase -25.4 deg) | yes (60.2 deg, 2.14) | no |
| C2 (0.175) | yes (1.0619) | yes | yes (0.320) | **no** (`m_c` 0.578, phase -30.9 deg) | yes (49.6 deg, 1.26) | no |
| C3 (0.180) | yes (1.0923) | **no** (`c+` 2.2e-2) | yes (0.429) | **no** (`m_c` 0.726, phase -57.5 deg) | yes (33.5 deg, 0.57) | no |

**No candidate qualifies.** By D2 no main record is run and the decision is partial; `SELECTED` stays `None` in the test file and
`f756_lockin_record` stays ignored and not run. The candidates and the bracket are not changed. Nothing else of this registration
changes.
