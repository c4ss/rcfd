# ADR 0372: F-452 registration: the acoustic wave solver driven by a sampled CFD surface (an rcfd-surface-series file)

Status: **REGISTERED 2026-11-12**, written before any code of F-452 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parents: [ADR 0351](0351-f333-registration-acoustic-wave-solver-and-modal-export.md) (F-333: `AcousticWaveSolver`, its
`WaveBc::Driven` ghost state, `duct_modes`; question 2, from which this item comes, recommendation (a) taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation)); [ADR 0366](0366-f453-registration-circular-and-three-dimensional-ducts-through-the-wave-solver.md)
(F-453, circular O-grid ducts) and [ADR 0371](0371-f473-registration-wave-solver-conservation-with-a-non-zero-boundary-mean.md)
(F-473), read for the state of the solver; [ADR 0066](0066-wave32b-aeroacoustics.md) (the `rcfd-surface-series` v1 format of
the FW-H chain). Roadmap item F-452 (depends on F-333). Number 0372, branch `f452-wave-from-cfd-surface` from main b624bd0a,
worktree `w49i-work/f452`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults bit-identical; no hash table or golden
touched; no new crate, no unsafe code; unsigned commits without trailers; no push. No CLI command and no case key (F-451 and
the wiring need the maintainer).

## What F-452 asks

Acceptance (`docs/roadmap.json`, binding in full):

> A driven boundary built from an rcfd-surface-series file reproduces, in a duct, the modal amplitudes of a closed-form duct
> field sampled into that file within 1 percent; a series that does not cover a whole number of periods is refused.

Clauses: (A) a closed-form duct field is sampled into an `rcfd-surface-series` file; a `WaveBc::Driven` boundary is built
from that file alone; the wave solver run in a duct with it reproduces the field's modal amplitudes within 1 percent; (B) a
series that does not cover a whole number of periods is refused.

## What was read and run before this text (disclosed)

- **Read:** the F-452, F-333, F-451, F-453, F-473 entries of `docs/roadmap.json`; ADR 0351 in full; the head of ADR 0371;
  `crates/rcfd-post/src/surface_series.rs` in full (`SeriesSpec`, `FaceSpec`, `from_json`, `to_json`, `validate`; the file
  carries per face `area`, `x`, `n` and a pressure series `p` on the uniform axis `t0 + i dt`, plus `rho0`, `c0`, `flow`,
  `motion`; **no velocity**); `crates/rcfd-fv/src/acoustic_wave.rs` lines 1 to 200 and 330 to 520 (`GhostState =
  Arc<dyn Fn(Vec3, f64) -> (f64, Vec3)>`, the driven face flux, lines 467 to 474); `crates/rcfd-post/src/duct_modes.rs` lines
  180 to 660 (`DuctShape`, `Mode`, `ModeBasis`, `axial_wavenumber`, `decompose_section`, `HarmonicDft`, `split_axial`,
  `fit_axial`); `crates/rcfd-post/tests/duct_modes_f333.rs` in full (the S1 set-up and measurement, reused here);
  `rcfd_mesh::gen::box_2d`.
- **Run (an existing test, not a check of this item):** `cargo test --release -p rcfd-post --test duct_modes_f333 --
  s1_mode_one_matched_layer_rigid_end s1c_coarse_grid --nocapture`, to read numbers the F-333 addendum rounded. Printed:
  S1 (32 cells across) `|plus| = 0.9836` at `x_ref = 5 h`, `ki = 1.265e-2 1/m`; S1c (16 across) `|plus| = 0.9110`,
  `ki = 9.772e-2 1/m`. Arithmetic by hand from them: the driven mode's amplitude extrapolated to the inlet with the fitted
  decay, `|plus(0)| = |plus(5h)| e^{ki 5h}`, is `0.9836 x 1.00762 = 0.9911` (S1, `-8.9e-3` below the drive of 1) and
  `0.9110 x 1.0604 = 0.966` (S1c, `-3.4e-2`): the driven boundary injects the mode about 0.9 percent low at 32 cells
  across, second order (ratio 3.8 for a halving). **This sets the grid of the decision run (64 across) below; on the F-333
  grid the 1 percent clause would have been met by a margin of 1.1 only.**
- No code of F-452 exists; nothing of it was compiled or run.

## Design

**D1. Where.** A new module `crates/rcfd-post/src/wave_drive.rs` (`SurfaceDrive`), one `pub mod` line in
`crates/rcfd-post/src/lib.rs`. It needs `surface_series` and `duct_modes` (rcfd-post) and builds a `GhostState` of rcfd-fv
(already a dependency of rcfd-post); no change to `rcfd-fv`, no change of any existing function. Opt-in: nothing runs new
code unless it constructs a `SurfaceDrive`. Library only (no case key, no CLI), taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation) and as the brief says.

**D2. What the file can and cannot tell.** The file gives the pressure on a section; the driven face needs the ghost state
`(p, u)` of the wave **entering** the domain (DR3 of ADR 0351: the face takes the incoming characteristic `w = p + Z u_ax`
from the ghost). Pressure alone does not give `u`, and the plane-wave impedance `u = p/Z` is wrong for a duct mode: a mode at
`cos theta = kz/k` has `u_ax = (kz/(rho0 omega)) p`, so the plane-wave guess injects it with `2/(1 + cos theta)` of its
amplitude (`+11 %` at `cos theta = 0.8`, the negative control MU1). The drive therefore decomposes the sampled field into duct
modes per harmonic and gives each mode its own axial impedance. **Assumption, stated:** the sampled pressure is the wave
travelling into the wave domain only (no returning wave at the sampling section); a section field with both directions
cannot be split from one section of pressure. This is a limit, not checked.

**D3. Periodic playback, hence whole periods (clause B).** The series is taken as `P` periods of a periodic signal of the
fundamental frequency `f` given by the caller, and played back periodically. Harmonic `h` (`omega_h = 2 pi h f`,
`h = 1 .. H`, `H` given by the caller) has the face amplitudes

    A_{f,h} = (2/N) sum_{n=0}^{N-1} p_f(t_n) e^{-i omega_h t_n},   t_n = t0 + n dt,

(`HarmonicDft`, ADR 0351 DR7) with the absolute file times, so that playback at the solver time `t` uses the file's clock
(the solver time and the file time are the same axis). This is exact (and rejects the mean and every other harmonic) only
when `N dt = P / f` for an integer `P >= 1`: otherwise the mean leaks into every harmonic and the harmonics leak into each
other (the DFT of a non-integer number of cycles). Hence the refusal: with `r = N dt f`, the series is refused when
`round(r) < 1` or `|r - round(r)| > 1e-6 max(1, r)`. Tolerance: a file written with shortest round-trip `f64` (`to_json`)
has `|r - P|` of order `N eps = 1e-14`; a solver writing `dt` to seven significant digits stays below `5e-7 r`; a missing
sample is off by `P/N` (`1/64` here). Aliasing: refused when `H P >= N/2` (harmonic `H` at or above the Nyquist bin). The
mean (`h = 0`, the steady pressure of the CFD run) is not acoustic and is not played back; it is reported.

**D4. Modal decomposition and synthesis.** The caller gives a `DuctFrame` (`origin`, unit `axis` pointing into the wave
domain, unit section axes `e1`, `e2`; the section coordinates of a point `x` are `((x - origin).e1, (x - origin).e2)`, the
axial coordinate `xi = (x - origin).axis`) and a `ModeBasis` (shape and modes, ADR 0351 DR5/DR6). Per harmonic the face
amplitudes are fitted on the basis with the face areas as weights (`decompose_section`, DR7): modal amplitudes `a_{q,h}` at the
sampling plane `xi_s` (the area-weighted mean `xi` of the faces). The ghost state at a face centre `x`, time `t`:

    p_g = w(t) sum_h sum_q Re( a_{q,h} psi_q(s(x)) e^{i (omega_h t - kz_{q,h} (xi(x) - xi_s))} ),
    u_g = axis w(t) sum_h sum_q Re( (kz_{q,h} / (rho0 omega_h)) a_{q,h} psi_q(s(x)) e^{i (omega_h t - kz_{q,h} (xi(x) - xi_s))} ),

with `kz_{q,h} = axial_wavenumber(omega_h/c, k_cut_q)` (propagating: real; cut off: `-i alpha`, the decaying field). The
velocity follows from `rho0 u_t = -dp/dxi`: `i omega rho0 u = i kz p`. The factor `e^{-i kz (xi - xi_s)}` carries the
sampled wave from the sampling plane to the wave mesh's driven faces (the two need not coincide). `w(t)` is the ramp
`sin^2(pi t/(2 T_r))` for `0 <= t < T_r`, 1 after, 0 before 0, `T_r` given by the caller (ADR 0351 set-up). The returned
`GhostState` caches `psi_q(s(x))` and `xi(x)` per face centre (keyed by the bits of `x`) so that Bessel functions are not
re-evaluated every stage.

**D5. Refusals (located errors).** Besides D3: a fundamental not finite and positive; `H = 0`; `T_r` negative or not finite;
the file's `rho0` or `c0` differing from the wave solver's medium by more than `1e-9` relative (`$.rho0`, `$.c0`); a
non-zero `flow` (`$.flow`, the medium is at rest) or a rotating `motion` (`$.motion`); a frame whose axes are not unit and
orthogonal within `1e-9`; a face normal not along the axis (`|n.axis| < 1 - 1e-6`, `$.faces[k].n`); faces not on one
section plane (`|xi - xi_s| > 1e-6 L`, `L` the duct size `max(a, b)` or the radius, `$.faces[k].x`); a face outside the
section by more than `1e-6 L` (`$.faces[k].x`); a basis with more modes than faces.

**D6. Report.** `SurfaceDrive` exposes the periods `P`, the area-weighted mean pressure, the section residual of each
harmonic's fit relative to the RMS of that harmonic's face amplitudes, the fraction of the series' variance about the mean
not represented by the harmonics kept, and the modal amplitudes and `kz` per harmonic. Reported, not refused.

## Set-up (fixed here)

Channel as F-333: `rho0 = 1.2`, `c = 340`, `h = 0.12 m`, `box_2d`, patches `xmin` (Driven from the file), `ymin`, `ymax`
(Wall), `xmax` (Wall), `frontAndBack` (empty); matched layer at `x = 5 h`, thickness 0.15 m, exponent 2, `R0 = 1e-6`,
normal `+x` (as S1). Frame: `origin = (x_s, 0, 0)`, `axis = (1,0,0)`, `e1 = (0,1,0)`, `e2 = (0,0,1)`; basis
`Rectangular { a: h, b: 1 }`, modes `(m, 0)`, `m = 0..7`.

**The closed-form field.** Fundamental `f0 = c (pi/h) / 1.2 = 1180.56 Hz` (`omega0`), so that mode 1 propagates at harmonic
2 with `cos theta = 0.8` (the F-333 frequency, `2361.1 Hz`) and only the plane wave propagates at harmonic 1 (mode 1 cut off,
`k = 21.82 < 26.18 1/m`). Defined with amplitudes at `x = 0` (the wave mesh's inlet):

    p(x, y, t) = 3.0 + sum_{(h, m)} Re( A_{h,m} psi_m(y) e^{i (h omega0 t - kz_{h,m} x)} ),
    A_{1,0} = 0.6 e^{0.4 i},  A_{2,0} = 0.3 e^{2.0 i},  A_{2,1} = 1.0 e^{-1.1 i}        [Pa],

`psi_m` the normalised channel modes, `kz` the closed forms; the mean 3.0 Pa stands for the steady CFD pressure.

**The file.** Sampled at a section **upstream** of the wave mesh, `x_s = -0.02 m`, on 24 faces of a non-uniform section grid
(edges `y_j = h (j/24 - 0.5 sin(2 pi j/24)/(2 pi))`, `j = 0..24`, clustered at both walls like a wall-refined CFD mesh; face
centre the midpoint of its edges, `z = 0.5`, area the width times depth 1, normal `(-1, 0, 0)`), `t0 = 0.0137 s`,
`dt = T0/64`, `N = 128` samples (`P = 2`), `rho0`, `c0` as above, static, no flow. Written with `SeriesSpec::to_json`,
serialised to text, parsed back with `SeriesSpec::from_json`; the drive is built from the parsed spec only. `H = 2`,
`T_r = 4 T0` (= 8 periods of harmonic 2, as S1).

**Truth.** The amplitude of mode `m` at harmonic `h` at `x = 0` is `A_{h,m}`; the drive must carry it there from
`x_s = -0.02 m` (phase `kz 0.02`: 0.44 rad for `(1,0)`, 1.40 rad for `(2,1)`).

**Runs.**

| run | cells across x along | steps per period of harmonic 2 | periods of harmonic 2 | DFT window | used by |
|---|---|---|---|---|---|
| D1 (decision) | 64 x 400 (0.75 m) | 320 (`c dt/dx = 0.24`) | 40 | the last 4 (= 2 T0) | A1, A2m |
| D1c (covering) | 32 x 200 | 160 | 40 | the last 4 | A1c |
| D2 (chain) | 16 x 100 | 80 | 12, no DFT | none | A2 |

Measurement (DR9 of ADR 0351, the S1 code): per cell DFT at `omega0` and at `2 omega0` over the window; per column of cells a
section fit on modes 0..7; per driven `(h, m)` the two-wave fit `fit_axial` with complex `kz` over the stations in
`[0.5 h, 5 h]`, guess the closed form, **`x_ref = 0`**: `plus` is the measured amplitude at the inlet (the fitted decay and
dispersion of the scheme are taken out by the fit, so the comparison is of the injected amplitude, not of the scheme's
propagation, which F-333 checked). A fit at its bracket edge fails the test.

D2 runs the solver twice on the same mesh from rest, once driven by the file (`SurfaceDrive::ghost_state`) and once by the
closed-form incoming wave written directly in the test (the F-333 way: `p` and `u_x = Re(kz/(rho0 omega) ...)` of the three
components at the face centre, same ramp), and compares the pressure of every cell after 12 periods.

## Registered checks

| id | clause | quantity | limit | estimate (source) |
|---|---|---|---|---|
| A1 | A | D1: `|plus - A_{h,m}| / |A_{h,m}|` (complex, amplitude and phase) for each of `(1,0)`, `(2,0)`, `(2,1)` | `<= 1e-2` each | `(2,1)`: `2.3e-3` (the `-8.9e-3` of S1 at 32 across, divided by 3.8, the S1/S1c ratio, second order); `(2,0)`, `(1,0)`: not modelled, assumed `<= 2.3e-3` (plane waves; `(1,0)` at half the wavenumber) |
| R1 | B | the 128-sample file truncated to 127 samples (`r = 1.984`) and to 96 samples (`r = 1.5`): `from_series` refused with an error naming the number of periods | an error each | refused (D3) |
| R0 | B (control) | the 128-sample file: accepted, `P = 2` reported | accepted | — |
| A3 | A (chain) | `SurfaceDrive::state` (no ramp) against the closed-form incoming `(p - 3, u_x)` at the 64 inlet face centres of D1 at 64 times over one `T0`: max error / max `|p|` (and `rho0 c` times the velocity error / max `|p|`) | `<= 1e-9` | round-off, `1e-14` (the field lies in the span of the basis, the DFT is exact over whole periods) |
| A2 | A (chain) | D2: `max_c |p_file - p_closed| / max_c |p_closed|` after 12 periods | `<= 1e-9` | round-off, `1e-13` |
| A1c | A (covering) | D1c: the A1 measure for each driven component | `<= 3e-2` each | `(2,1)`: `8.9e-3` (S1) |
| A2m | method | D1: `|minus/plus|` for each driven component (the layer) | `< 1e-2` | S1 `9e-5`; `(1,0)`: the layer is half a wavelength thick, `<= 1e-3` |
| R2 | refusals | D5 and the Nyquist bin: `H = 32` on the 128-sample file (`H P = 64 = N/2`); `c0` of the file 341; a face moved by `1e-3 h` along `x`; a face with `y = 1.01 h`; a face normal `(0, 1, 0)`; `flow = (1, 0, 0)`; a rotating motion; `f = 0`; `H = 0`; a basis of 30 modes on 24 faces; a non-orthogonal frame | an error each | refused |
| M0 | report | D1: the reported mean (3.0), `P` (2), the unrepresented variance fraction | mean within `1e-12` of 3.0; fraction `<= 1e-12` | round-off |

The acceptance is decided by **A1** (clause A) and **R1** (clause B). A3 and A2 check the chain from the file to the face
without the scheme's error; A1c is the fast covering run of the A1 code path (D1 is expected near two minutes in release
and is therefore an `#[ignore]`d heavy record, run once; D1c and D2 are ordinary tests); A2m validates that the layer leaves a
single wave to fit.

## Planned mutations (scratch copy, once each, not committed)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the modal impedance replaced by the plane-wave one, `u = p/(rho0 c)` for every mode | A3, A2, A1 and A1c for `(2,1)` (`+11 %`) |
| MU2 | the DFT phase taken from `n dt` instead of `t0 + n dt` (file clock lost) | A3, A2, A1, A1c (`omega0 t0` mod `2 pi` = 1.07 rad) |
| MU3 | the whole-period test removed | R1 |
| MU4 | the axial carry with `e^{+i kz (xi - xi_s)}` | A3, A2, A1, A1c (phase `2 kz 0.02`) |
| MU5 | every harmonic played back at `omega0` (harmonic number dropped in the synthesis) | A3, A2, A1 and A1c for `(2,0)`, `(2,1)` |
| MU6 | the Nyquist test removed | R2 (the `H = 32` case) |

A mutation that survives its registered checks is reported and diagnosed; by Rule I (the lead's rule) a registered mutation
not caught by its registered check makes the item partial.

## Decision rule (registered)

- **done** when A1 meets its limit for all three components, R1 refuses both truncated series and R0 accepts the whole one,
  A3 and A2 meet theirs, R2 refuses every case, the baseline hashes are unchanged, and every registered mutation is caught by
  at least one of its registered checks.
- **partial** when A1 misses for any component (kept as `*_known_miss`, diagnosed), or R1 fails, or a registered check
  misses, or a registered mutation is not caught by its registered checks, or is caught only after a post-run amendment, or
  the D1 run could not be made.
- **todo** does not apply once code exists.
- A1c, A2m and M0 do not decide the status by themselves; a miss there is kept as `*_known_miss` and named in the note (it is
  a registered check, so by Rule I it makes the item partial).

## Limits (what is not covered)

- One duct (the two-dimensional channel), one field (three components at two harmonics), one sampling section; no circular
  or three-dimensional duct run (the code takes any `DuctShape`; the circular O-grid of F-453 is not run here, cost).
- The sampled field must be the wave entering the wave domain (D2); a CFD section with reflected waves is not separated.
- Periodic signals only (whole periods of a given fundamental); a broadband or transient CFD signal is not played back.
- Medium at rest; static surfaces only; the CFD run itself is not part of the check (the "CFD surface" is a closed-form field
  written in the CFD chain's file format).
- Library only: no case key, CLI or MCP tool.

## Sources

ADR 0351 DR1 to DR9 (the solver, the modes, the DFT, the fits), re-used; the impedance of a duct mode from the linear
momentum equation (derived in D4). The DFT orthogonality over whole periods is elementary (geometric sums). Pierce,
*Acoustics*, 1989, ch. 7, cited from memory for duct modes; nothing is taken from it that is not derived. No vendor code or
documentation was used.
