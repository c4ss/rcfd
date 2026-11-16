# ADR 0671: F-750 registration: moving immersed shapes at lock-in where the lift does not cancel

Status: **REGISTERED 2026-11-16, written before any code of F-750 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-750 was written, compiled or run; the arithmetic below was done by
hand and with a few lines of Python (pure arithmetic of the case numbers, no solver). No solver run of any kind was made for
F-750 before this registration. Results go in the amendments and addenda at the end; nothing above them is changed after a run.

Parent: [ADR 0657](0657-f731-registration-moving-immersed-shapes-at-lock-in.md) (F-731: the lock-in case at Re 100, A/D 0.5,
f D/U 0.14, its test file `crates/rcfd-fv/tests/immersed_lockin_f731.rs`, its measures D3 to D5, the allowance D4, its
addendum 1 with the near cancellation of the first lift harmonic, its questions Q3 and Q4 from which F-750 was made), and through
it ADR 0648 (F-707), ADR 0631 (F-308, `MovingImmersed`), ADR 0218, ADR 0249, ADR 0451, ADR 0060. Roadmap item F-750 (deps F-731).
Number 0671 given by the lead. Branch `f750-work` from 6600f7c9 (F-731 / ADR 0657 merged), worktree `w49i-work/f750`, scratch
`w49i-work/f750-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; existing tests and
ignored records not edited (the F-731 file is not touched); defaults bit-identical; no edit of `baseline_hashes.rs`, of the case
and mesh hash tables or of any golden file; no new dependency, no unsafe code, crate graph unchanged; clean-room. The roadmap
files are not edited. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-750 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A cylinder oscillating across a uniform stream at Re 100 inside the lock-in range, at a forcing point where the first harmonic of the lift is at least half the fixed cylinder's lift amplitude in the body-fitted reference, gives the shedding locked to the forcing, the mean drag, the lift amplitude and the phase of the lift against a body-fitted moving-mesh run of this repository within registered limits that allow for the natural-frequency difference of the two discretisations, with the lift limits stated against a fixed force scale as well as relative; the force series are kept for diagnosis; a negative control with the body velocity left out of the forcing misses the lift amplitude and phase.

Clauses: **c0** every limit registered here; **c1** Re 100, a cylinder oscillating across a uniform stream, inside the lock-in
range; **c2** at a forcing point where the first lift harmonic of the body-fitted reference is at least half the fixed
cylinder's lift amplitude of the body-fitted reference; **c3** the shedding locked to the forcing; **c4** mean drag, **c5** lift
amplitude, **c6** lift phase against a body-fitted moving-mesh run of this repository, within registered limits that allow for
the natural-frequency difference; **c7** the lift limits stated against a fixed force scale as well as relative; **c8** the
force series kept for diagnosis; **c9** the negative control with the body velocity left out of the forcing misses the lift
amplitude and the phase. The brief adds: choose the forcing point from F-731's own data or a cheap registered body-fitted scan
disclosed before the main runs; heavy runs ignored records run once with fast tests over the same path; negative controls and
mutations registered before the run; reuse the F-731 harness.

### Readings, registered

- **The reference, the measures, the allowance, the lock measures** are those of ADR 0657 (D2 to D5) without change: the
  body-fitted swept-volume ALE run of this repository on the 256 x 128 O-grid of Expansion(20) mapped to the square `[-8, 8]^2`,
  started on the grid at the initial pose; the immersed run on the Cartesian D/40 core (G40); `Freestream` on the whole edge;
  PISO with linear upwind; `dt = T / 700` for every run of one forcing point; 20 periods, the window W the last 4; drag, lift
  harmonic, phase `arg(Lhat / Yhat)`, L1 and L2; the natural frequencies from fixed runs and the allowance `|s_X (r_IB - r_BF)|`
  from the body-fitted bracket runs at 720 and 680 steps per period.
- **"The fixed cylinder's lift amplitude in the body-fitted reference"** is `S`, the lift half range `(max - min) / 2` of the
  fixed body-fitted run BF0 over its last four shedding cycles (ADR 0657 D3, `amp_halves.1`), run at the selected point's `dt`.
  c2 is the check **G1** `|Lhat_BF| >= 0.5 S`, measured in the main record (the scan only chooses the point).
- **"A fixed force scale"** is `F_L = S + m_a A omega^2` with `m_a = rho pi r^2 = 0.785398` (per unit depth): the fixed
  cylinder's lift amplitude plus the potential-flow added-mass force amplitude of the imposed motion. Both terms are fixed by the
  case (the fixed run and the kinematics), not by the compared lift, so a near cancellation of the total lift cannot make a check
  ill-conditioned; and the scale grows with the added-mass force where that dominates the lift, so a limit on it is not tighter
  than the discretisation can give there (arithmetic 5). Every lift check is stated twice: relative to `|Lhat_BF|` and against
  `F_L`; both must be met (c7). The numbers against `S` alone are reported.
- **"Inside the lock-in range"** is shown by L1 in every compared forced run (ADR 0657 D5) and, at the screening stage, by L1 of
  the two neighbouring frequencies of the chosen point (the point is at least one neighbour step inside the range on either
  side).
- **"The force series are kept"** (c8): every run of the scan and of the main record writes its force series (step, `F_x`,
  `F_y`, `F_z`, shortest round-trip decimals) to a file in the folder given by the environment variable
  `RCFD_F750_SERIES_DIR` (default: Cargo's `CARGO_TARGET_TMPDIR`); the file is read back and must give the in-memory series bit
  for bit (check K1), and the FNV-1a hash of the bits is printed and recorded in the addendum. The record is run with the folder
  `w49i-work/f750-scratch/series`.
- **"Misses the lift amplitude and phase"** (c9): the negative control's lift amplitude lies outside both amplitude limits (A2,
  A2L) and its phase outside the phase limit (A3) that the immersed run must meet, with the immersed run's allowance (N1b, N1c).
  Its complex harmonic must also miss (N1d). Its drag is reported only (ADR 0657 Q4, recommendation (a), written into this
  acceptance).
- **The forcing point is chosen by a registered body-fitted scan** (stage S below), with a mechanical selection rule fixed
  here, its results disclosed in an amendment committed before any main run. F-731's own data do not hold a point with a large
  first harmonic: at A/D 0.5 the harmonic falls from 0.057 to 0.036 over `r = 0.826` to `0.875`, and a linear extrapolation to
  `0.5 S = 0.089` needs `r` about 0.75, at or beyond the lower edge of the lock-in range (arithmetic 1).

## What was read

`w49i-work/AGENT_RULES.md`; the brief; ADR 0657 (all, with addendum 1 and the "For the lead" section); the F-731 test file (all);
the F-731 record log `w49i-work/f731-scratch/logs/heavy_record.log` (timings, hashes); `crates/rcfd-fv/src/immersed_motion.rs`
(`bodies_at`, `forcing_at`, `step`); the newest ADRs 0665 and 0666 (form).

## Arithmetic before this registration (disclosed)

1. **The F-731 lift, decomposed.** With `y_c = -A cos(omega t)`, `Yhat = -A`; the potential-flow added-mass force
   `-m_a yddot = m_a omega^2 y_c` has the harmonic `-m_a A omega^2` (real, phase 0 against the displacement). At F-731's point
   (A 0.5, f 0.14, `omega^2 = 0.77377`) that is `-0.30387`; the body-fitted harmonic was `(+0.010988, +0.046567)`, so the rest
   (vortex lift and the viscous and wake parts) is about `(+0.315, +0.047)`, magnitude 0.318: the two parts nearly cancel. The
   immersed run differed by 0.0096 (complex), 3.0 per cent of the vortex part or 2.0 per cent of `F_L(731) = 0.1776 + 0.3039 =
   0.4815`; the coarse reference (R1) by 0.0041, 0.85 per cent of `F_L`. The harmonic over the bracket: 0.0568 (r 0.826), 0.0478
   (0.850), 0.0364 (0.875); the linear extrapolation reaches 0.089 at `r = 0.850 - 0.041 / 0.42 = 0.75`.
2. **Candidates.** Two ways out of the cancellation (ADR 0657 Q3): (a) a smaller amplitude at about the same `r`, where the
   added-mass part halves (A/D 0.25: `m_a A omega^2 = 0.152` at f 0.14) while the locked vortex lift need not; (b) a forcing
   above the natural frequency at A/D 0.5, where the added-mass part grows as `omega^2` (0.502 at f 0.18) and, past the phase
   change of the vortex lift known from forced-wake studies (Koopmann 1967 and the general picture, cited from memory and used
   only for this framing), the two parts no longer oppose. Neither can be told from F-731's data, hence the scan. Families and
   frequencies (with the F-731 `f0 = 0.1648`):

   | Family | A/D | f D/U | r = f / 0.1648 | `m_a A omega^2` | dt = T / 700 |
   | --- | --- | --- | --- | --- | --- |
   | A50 | 0.5 | 0.17, **0.18**, **0.19**, 0.20 | 1.032, 1.092, 1.153, 1.214 | 0.448, 0.502, 0.560, 0.620 | 8.4034e-3, 7.9365e-3, 7.5188e-3, 7.1429e-3 |
   | A25 | 0.25 | 0.13, **0.14**, **0.15**, 0.16 | 0.789, 0.850, 0.910, 0.971 | 0.131, 0.152, 0.174, 0.198 | 1.0989e-2, 1.0204e-2, 9.5238e-3, 8.9286e-3 |

   The candidate points are the bold middle ones, C1 = (0.5, 0.18), C2 = (0.5, 0.19), C3 = (0.25, 0.14), C4 = (0.25, 0.15); each
   has a neighbour 0.01 below and above in its family (5 to 8 per cent of the frequency), wider than the main bracket of +-3 per
   cent.
3. **The negative control may not separate in phase where the added mass dominates.** F-731's N1 harmonic `(-0.894, -0.470)` has
   the phase +27.8 deg against the displacement; an added-mass dominated reference lies near 0 deg. If N1's phase stayed near +28
   deg at C1 or C2, N1c (more than about 8 deg off) could hold or fail by a small margin. The scan therefore also runs N1 on the
   coarse immersed mesh G20 (D/20) at each candidate, and the rule requires a separation (S4 below). This uses only the negative
   control and the reference, never the compared immersed run; it is disclosed here as a choice of the experiment so that c9 can
   discriminate, not as a choice of the result.
4. **Time steps and run lengths.** Every run of one point uses `dt = (1 / f) / 700`; forced runs 20 of their own periods (14 000
   steps at 700 per period; the bracket 14 400 and 13 600); fixed runs `700 ceil(20 f / 0.14)` steps so that they reach `t >=
   142.86` as in F-731 and the `f0` window `t >= 80` holds about ten cycles: 14 000 (f 0.14), 15 400 (0.15), 18 200 (0.18), 19
   600 (0.19). Courant (implicit Euler): BF at A50, f 0.20, about `(1.5 + 0.63) x 0.00714 / 0.0147 = 1.03`, below F-731's 1.45;
   diffusion numbers below F-731's (smaller `dt`). Largest body speed per step `A omega dt = 2 pi A / 700 = 0.0045` (A50), 0.18
   of an IB cell.
5. **Error estimates behind the base limits.** From arithmetic 1: the immersed lift error about 3 per cent of the vortex part plus,
   for the added-mass part, the effective-diameter error of the staircase (`h/4` to `h/2` on `r`, so 1 to 2.5 per cent on
   `r^2`): at C1, with a vortex part of order 0.3, `0.009 + 0.010 = 0.019`, that is 2.8 per cent of `F_L = 0.178 + 0.502`; at C3
   about `0.009 + 0.003 = 0.012`, 3.6 per cent of `F_L = 0.330`. Relative to `|Lhat_BF|`, with `|Lhat_BF| >= 0.75 S` guaranteed
   only at the scan (S2) and expected 0.15 to 0.6: 3 to 10 per cent. The F-731 base limits (drag 0.04, amplitude 0.08, phase 8
   deg, complex 0.12) are kept for the relative checks; the limits against `F_L` are about twice the upper estimate: amplitude
   0.06, complex 0.08; the coarse reference 0.05. Phase: `0.019 / 0.15 rad` at worst, about 7 deg; usually 2 to 4 deg.
6. **L2 of the immersed run.** F-731: L2 0.129 of a lift rms of about 0.04, a residual (the harmonics above the fifth, the cell
   crossings of the staircase surface) of about 0.005, 1.1 per cent of `F_L(731)`. If it scales with the body speed (1.29 times
   at C1) and the lift rms is at least `0.75 S / sqrt 2 = 0.094`: L2 at most 0.075, residual about 1 per cent of `F_L`. The
   immersed run's L2 is limited at 0.10 relative and its residual at 0.03 of `F_L` (L2L); the body-fitted runs keep 0.05.
7. **Cost.** F-731 timings on the shared machine: BFc about 0.18 s per step, BF 0.65 to 1.1, IB (G40) 0.95 to 1.05; G20 about a
   quarter of G40's cells. Stage S: 8 BFc runs of 14 000 steps (about 45 min each) and 4 G20 N1 runs (about 1 h), twelve threads,
   wall time 1 to 2.5 h on 16 logical cores shared with other agents. Stage M: nine runs as F-731, the longest (BF0 or IB0 of up
   to 19 600 steps) 4 to 7 h. Memory below 1 GB per run (30 GB free).

## Design

### D1. No library change

All of F-750 is test code in a new file `crates/rcfd-fv/tests/immersed_lockin_f750.rs`. The helpers of the F-731 file are copied
(that file is not edited); the case constants become fields of a forcing point `Point { amp, f }` from which every run
specification derives its `dt`, steps and motion. No library line changes; the default bits are unchanged by construction.

### D2. Stage S, the scan (heavy record `f750_scan_record`, run once)

Runs: BFc (128 x 64) forced at the eight points of arithmetic 2, from the uniform stream, 20 periods; N1 on G20 (h 0.05, the
F-707 coarse mesh), velocity left out of the forcing, at C1 to C4, 20 periods. Measures over W as ADR 0657 D5. Screening scale:
`S731 = 0.177620` (F-731 BF0, ADR 0657 addendum 1). A candidate `c` with neighbours `n-`, `n+` (its family, 0.01 below and above)
**qualifies** when all of:

- **S1 (locked with margin)**: L1 <= 0.01 in the BFc runs at `c`, `n-` and `n+`;
- **S2 (large harmonic)**: `|Lhat_c| >= 0.75 S731 = 0.133215` (BFc);
- **S3 (well conditioned)**: for each neighbour, `| |Lhat_n| - |Lhat_c| | / |Lhat_c| <= 0.25` and `|wrap(phi_n - phi_c)| <= 30
  deg`;
- **S4 (the control can separate)**: the G20 N1 at `c` against BFc at `c`: `|wrap(phi_N1 - phi_c)| >= 24 deg` (three times the
  phase base limit) and `| |Lhat_N1| - |Lhat_c| | / |Lhat_c| >= 0.24` (three times the amplitude base limit).

**Selection:** among the qualifying candidates the one with the smallest `m_c = max_n | |Lhat_n| - |Lhat_c| | / |Lhat_c|`; a tie
(within 1e-12) goes to the earlier of C1, C2, C3, C4. The rule is implemented as a function and tested on synthetic tables (T8).
If no candidate qualifies, no main record is run, the scan is reported, and the decision is partial (amendment 1 says so).

Amendment 1 (written after the scan and committed before any main run) records the scan's numbers, the rule's outcome and the
selected point, and sets the point constant in the test file in the same commit; nothing else of this registration changes.

### D3. Stage M, the main record (heavy record `f750_lockin_record`, run once at the selected point)

The nine runs of ADR 0657 at `(A*, f*)` with `dt = (1 / f*) / 700`: BF0, BFc0, IB0 (fixed, perturbed start, `700 ceil(20 f* /
0.14)` steps); BF, BF- (720 steps per period), BF+ (680), BFc, IB, N1 (G40, velocity left out) forced, 20 periods. Measures,
natural frequencies, slopes and allowances exactly as ADR 0657 D3 to D5, plus the fixed-scale slopes `s_A^L = (|Lhat+| -
|Lhat-|) / (F_L w_r)` and `s_C^L = |Lhat+ - Lhat-| / (F_L w_r)` with allowances `|s^L Delta r|`; the L2 residual
`R2 = rms over W of (F_y - mean - harmonics 1 to 5)`. Every series is written and read back (K1).

### D4. Not built

Other amplitudes or frequencies than the scan's; a map of the lock-in range or of the phase jump; rotating, deforming or freely
moving bodies; BDF2; second-order immersed faces; three dimensions; a case-file key (ADR 0648 Q1).

## Checks

Stage S (reported, and the rule of D2; no pass or fail beyond **SS1**: every scan run finishes without a stop with finite forces,
and its series is written and read back bit for bit).

Stage M, heavy record (run once, prints every measure before it asserts). `Delta r = r_IB - r_BF`, `Delta r_c = r_BFc - r_BF`;
N1 uses the IB allowance.

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| G1 | the forcing point (c2): `|Lhat_BF| / S` | >= 0.5 | 0.75 to 3 (the scan's S2, BFc against BF within 1 per cent of `F_L`) |
| F1 | fixed runs BF0, BFc0, IB0: at least 9 upward crossings after t = 80; first and last four cycles: frequency within 0.005, half range within 0.02 (relative) | as stated | F-731: 1e-5 |
| F0 | `abs(f0_IB / f0_BF - 1)`, `abs(f0_BFc / f0_BF - 1)` | <= 0.045 each | F-731: 4.4e-4, 3.9e-3 |
| F2 | reported: `St` of each, `r_BF`, `r_IB`, `r_BFc`, the bracket | reported | |
| L1 | lock, lagged: IB, BF, BF-, BF+, BFc (c1, c3) | <= 0.03 each | F-731: below 1e-8 |
| L2 | lock, spectral: BF, BF-, BF+, BFc | <= 0.05 each | F-731: 1e-4 |
| L2I | IB: L2 relative; and `R2 / F_L` (L2L) | <= 0.10; <= 0.03 | <= 0.075; about 0.01 (arithmetic 6) |
| X1 | L1 of the fixed BF0, IB0 at the forcing period over their last `4 x 700` steps | > 0.1 each | `2 abs(sin(pi f0 / f))`: 0.5 to 1.1 |
| J1 | across the bracket: `abs(wrap(phi+ - phi-))`; `abs(abs(Lhat+) - abs(Lhat-)) / abs(Lhat_BF)` | <= 25 deg; <= 0.25 | the scan's S3 over a wider step |
| A1 | mean drag `abs(Fbar_x,IB - Fbar_x,BF) / Fbar_x,BF` (c4) | <= 0.04 + `abs(s_D Delta r)` | F-731: 0.0037 |
| A2 | lift amplitude relative `abs(abs(Lhat_IB) - abs(Lhat_BF)) / abs(Lhat_BF)` (c5) | <= 0.08 + `abs(s_A Delta r)` | 0.03 to 0.10 (5) |
| A2L | lift amplitude against the fixed scale `abs(abs(Lhat_IB) - abs(Lhat_BF)) / F_L` (c5, c7) | <= 0.06 + `abs(s_A^L Delta r)` | 0.01 to 0.03 (5) |
| A3 | lift phase `abs(wrap(phi_IB - phi_BF))` (c6) | <= 8 deg + `abs(s_P Delta r)` | 2 to 7 deg (5) |
| A4 | lift harmonic, complex, relative `abs(Lhat_IB - Lhat_BF) / abs(Lhat_BF)` (c5, c6) | <= 0.12 + `abs(s_C Delta r)` | 0.03 to 0.10 |
| A4L | lift harmonic, complex, against `F_L` (c7) | <= 0.08 + `abs(s_C^L Delta r)` | 0.015 to 0.04 |
| A5 | mean lift `abs(Fbar_y,IB - Fbar_y,BF) / F_L` | <= 0.02 | F-731: 2e-6 of `F_L` |
| R1 | BFc against BF: drag as A1; complex relative as A4; complex against `F_L`, each with `Delta r_c` | <= 0.03 + allow.; <= 0.08 + allow.; <= 0.05 + allow. | F-731: 0.004; 0.086; 0.0085 |
| N1b | velocity left out (G40): lift amplitude as A2 and as A2L (c9) | > the A2 limit and > the A2L limit (must exceed) | F-731: 20, i.e. 2.0 of `F_L` |
| N1c | N1: lift phase as A3 (c9) | > the A3 limit | the scan's S4: at least 24 deg at G20 |
| N1d | N1: complex harmonic as A4 and as A4L | > the A4 and A4L limits | |
| N1a | N1: mean drag as A1 | reported | F-731: 0.022 |
| K1 | every run of both stages: the series file read back equals the in-memory series bit for bit; its FNV-1a hash printed (c8) | bit for bit | |
| S1 | every run: no divergence stop, finite force and fields; largest step continuity and Courant reported | no stop | |
| P1 | reported: the measures per period of W, `C_a`, `C_v`, the numbers against `S` alone, L1 and L2 of N1 | reported | |

Fast tests (not ignored; release):

| Id | Check | Limit |
| --- | --- | --- |
| T1 | the BF, BFc and smoke meshes mapped to the square (as ADR 0657 T1); the smallest cell volume positive at the poses `-A, 0, A` for A 0.5 and 0.25 | as ADR 0657 T1 |
| T2 | for each of the eight scan points and its bracket: `dt` bit-equal to `(1/f)/700`, integer steps per period, fixed steps `700 ceil(20 f / 0.14)` (14 000, 15 400, 18 200, 19 600 at f 0.14, 0.15, 0.18, 0.19); the measures on the synthetic series of ADR 0657 T2 on each point's step grid; L1 of the off-frequency sinusoid against the closed form; `f0` of D3; the allowance; `F_L` against `S + pi r^2 A omega^2` on given numbers; `R2` of a series with a known sixth harmonic `b cos(6 omega t)` against `b / sqrt 2` | 1e-12 relative (L1, L2 and R2 of periodic series absolute 1e-12); 1e-9 deg; closed-form L1 1e-3; `f0` 1e-6; allowance and `F_L` 1e-14 |
| T3 | IB smoke (G20, each of C1 to C4, 40 steps, with and without the velocity): no stop, finite force, the forcing after each step equals `forcing_at(solver.time)`, the solid cells hold `(0, ydot)` (or 0) within `1e-3 A omega` | as stated |
| T4 | BF smoke (64 x 32, C1 and C3, 40 steps): continuity <= 1e-6; the `cylinder` points' mean `(0, y_c(t_n))` within 1e-12; far field unchanged bit for bit | as stated |
| T5 | the body-fitted solver starts on the grid at the initial pose, for A 0.5 and 0.25: the solver's points equal `y0 + y_c(0) w` bit for bit, cylinder centre `(0, -A)` within 1e-12, largest difference `A` within 1e-12; the fixed specification starts on the unmoved points | as stated |
| T6 | fixed set-ups smoke (G20 IB and 64 x 32 BF at the dt of C1, 20 steps), the initial field the perturbation bit for bit | as ADR 0657 T6 |
| T7 | the series file: a synthetic 3-component series (with values such as 1/3, -0.0, 1e-300, 1.2345678901234567e17) written and read back bit for bit, one line per step, the hash of the read-back equal to the hash of the original | bit for bit |
| T8 | the selection rule of D2 on synthetic scan tables: a table where all four candidates qualify picks the smallest `m_c`; a tie picks the earlier; and one table each where the otherwise best candidate fails exactly one of S1 (a neighbour's L1 0.011), S2 (`|Lhat| = 0.133`), S3 (a neighbour's phase 31 deg off), S4 (N1 phase 23 deg off), so that the next is picked; a table where none qualifies gives none | exact |

### Negative controls

N1 (heavy, c9) at G40 and its screening at G20 in stage S; its fast half in T3 (velocity left out). X1 is the control of the
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
| M8 | selection S1 without the neighbours' lock | T8 |
| M9 | selection without S4 | T8 |
| M10 | library (scratch copy of `immersed_motion.rs`): `bodies_at` leaves out the motion velocity regardless of the flag | T3 |
| M11 | the run specification's motion amplitude fixed at 0.5 regardless of the point | T5 |

## Decision rule

- **done**: G1, F1, F0, L1, L2, L2I, X1, J1, A1, A2, A2L, A3, A4, A4L, A5, R1 met; N1b, N1c, N1d exceeded; K1 and S1 for every
  run of both stages (SS1); T1 to T8 pass; at least ten of the eleven mutations caught, M1 among them; no library line changed.
- **partial**: any of these missed (with the diagnosed cause), a run stopped, no candidate qualified at the scan, or a check met
  only after an amendment other than amendment 1 of D2.
- **todo**: the case cannot be built or run.
- F2, N1a and P1 are reported and do not change the decision.

## Limits (what this does not cover)

One forcing point compared (the scan's eight points only in the coarse reference), Re 100, two dimensions, one mesh pair, the
square domain of half side 8 D with its short wake; the time step shared by all runs of one point, so the time error is common
and not measured; the reference is this repository's body-fitted run, not a published data set; the lock-in range and the phase
jump are not mapped. The scan's screening uses the coarse reference and the coarse immersed mesh for the negative control; the
binding checks are on the fine meshes. The allowance assumes the response depends on the forcing through `f / f0` near the point,
as ADR 0657. First-order forcing of ADR 0218. Items of D4.

## Open questions

- **Q1. A case-file key for moving immersed bodies.** As ADR 0648 Q1. Open (schema golden), not taken.
- **Q2. The fixed force scale.** Options: (a) `F_L = S + m_a A omega^2` as registered; (b) `S` alone (tighter where the added mass
  dominates, by up to a factor 4 at C2, below what the staircase gives on the added-mass part); (c) the added-mass scale alone (zero
  meaning at A = 0). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation); the numbers
  against `S` are reported.
- **Q3. The screening of the negative control (S4).** Options: (a) as registered, the coarse N1 in the scan so that c9 can
  discriminate at the chosen point; (b) no screening. Taken (a) on the standing instruction, disclosed in arithmetic 3.

## Amendment T3 (2026-11-16, written after the first run of the fast tests and before the corrected run)

The fast tests were run once on the test file as first written (log `w49i-work/f750-scratch/logs/fast_tests_run1.log`): T1, T2,
T4 to T8 passed; **T3 failed** at C3 (A/D 0.25, f 0.14, velocity in the forcing): the solid cells hold the body velocity within
`1.29e-3 A omega`, against the registered `1e-3 A omega`. At C1 and C2 (A/D 0.5) the same measure is `6.36e-4` and `6.34e-4`.

Diagnosis: the residual is the penalty error of the forcing, about `|u| / (K dt)` with `K dt = 1e6 x dt` of order 1e4 and `|u|`
of order the free stream, so it is fixed in absolute terms: `6.36e-4 x 0.5 x 1.131 = 3.6e-4` (C1), `1.29e-3 x 0.25 x 0.880 =
2.8e-4` (C3), and `6.43e-4 x 0.440 = 2.8e-4` in F-731's T3 at A/D 0.5, f 0.14. Normalised by the body speed `A omega` it doubles
when the amplitude halves. The limit of T3 was carried over from ADR 0657 T3, where only A/D 0.5 was tested; its normalisation is
defective for the smaller amplitude, not the forcing.

Correction (registered now, before the corrected run): **T3b** is T3 with the limit `max |u_c - u_b| <= 1e-3 U` (absolute, the
free stream speed `U = 1`) at every candidate, with and without the velocity; the estimate is 2.8e-4 to 3.6e-4. The quantity
`max |u_c - u_b| / (A omega)` is still printed. The first T3 stays in the file as the ignored record
`t3_immersed_smoke_at_the_candidates_known_miss` with its limit intact and the measured 1.29e-3 recorded. M10 is checked against
T3b (with the motion velocity left out the solid cells hold 0 while the body moves at up to `A omega` = 0.22 to 0.60, far above
1e-3). By the decision rule T3 counts as a check met only after an amendment (the rule's partial clause); this is stated here and
not changed.
