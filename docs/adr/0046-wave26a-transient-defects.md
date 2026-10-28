# ADR 0046: Wave 26A, two pre-existing defects of the transient PISO / PIMPLE algorithms

Status: accepted. The decisions and registered checks below were written before any solver code of this wave was changed and before
any post-fix run. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the limit intact; a set-up defect gets a NEW corrected test and the old one stays. An
addendum (measured values, deviations) is appended at the end. Clean-room: first principles and published literature only (Issa 1986
for PISO); no vendor code, text or names.

Scope: `crates/rcfd-fv/src/pimple.rs` momentum matrix assembly for translationally periodic patches. Not touched: SIMPLE/SIMPLEC steady
code, `fractional.rs` (read only), the `sources.<n>` mechanism.

## Findings that precede the fix (diagnosis, added after a bisect; the registered checks below do not depend on them)

* Before-values of the wave 25C known-miss tests, rerun on the unmodified HEAD 2b73501 (`--ignored`): T1a PISO Euler ux 5.093e-1,
  uy 3.207e-4, p 1.510e-1; PIMPLE (3) 1.080e-1 / 1.276e-4 / 3.089e-2; PISO BDF2 5.091e-1 / 3.797e-4 / 1.563e-1. T1b: vs recursion
  7.587e-2 / 1.402e-2 / 7.437e-2. They reproduce the wave 25C numbers.
* Reproducer `crates/rcfd-fv/tests/w26a_couette.rs` (impulsively started plane Couette flow, nu = 1, wall speed 1, `d = nu dt / dy^2`,
  60 steps, ny = 40): with the periodic box (`nx = 4`, periodic in x) PISO is stable at d = 0.5, 1.6, 3 (peak |u| < 1) and diverges at
  d = 6, 16, 100 (peak |u| 1.7e3, 1.5e3, 2.3e3). The same case with a non-periodic x (nx = 4 and nx = 40) is stable for d up to 16
  (peak |u| 0.985). At step 0 of the periodic case an x-variation of u (6.5e-3), a pressure (0.58) and a v-velocity (1.2e-3) already
  appear in a flow that is exactly x-uniform; the non-periodic run has none (1e-14). The x-uniform flow can only be disturbed by the
  wrap cells, whose partner value is lagged (defect 1). Hypothesis tested by the fix: the instability at large d is the amplification
  of the lagged periodic coupling, not a property of the PISO splitting. A residual instability of the non-periodic nx = 4 case at
  d = 100 (not at nx = 40) is a separate observation, diagnosed after the fix.

## Decisions

### D1 Implicit periodic coupling in the transient momentum matrix

The momentum matrix of `pimple.rs` is assembled by `convection` and `laplacian`, which for a coupled (periodic) boundary face put the
owner part on the diagonal and the partner part on the right-hand side from the previous iterate (`FaceCoeffs` built from `u`
at the start of the outer iteration). The partner contribution of a coupled face is linear, with coefficient `c_f`:
`-gamma_f |S_f| / span` for the Laplacian (`span` = sum of the two boundary-face distances), plus `phi_f` for upwind convection
when `phi_f < 0`. D1: for translationally periodic patches the PISO/PIMPLE step takes these coefficients (computed from the same
`FaceCoeffs` and flux that the assembly used), removes the lagged partner term from `b` (`b += c_f q_lag`), and carries them as
extra off-diagonal links (row = owner cell, column = owner cell of the partner face) used consistently in (a) the momentum solve,
(b) the implicit under-relaxation (diagonal dominance sum), and (c) the `H` operator of the correctors. The deferred (higher-order)
convection correction on coupled faces keeps its lagged partner value (it already was explicit by design). Rotationally periodic
patches keep the lagged treatment (the partner value is rotated; named limitation, as in `transposed_stress_source`). Cells that are
their own partner (nx = 1) and duplicate pairs are handled by summing coefficients.

Not done: the pressure equation already couples the pair implicitly (`pressure.rs`), the fractional-step code is not changed.

### D2 High-diffusion-number instability

Decide defect or property after the fix by re-running the reproducer. If periodic runs become stable at all d, the instability was
the lagged coupling (defect 1 consequence) and is closed by D1. Any remaining instability (non-periodic nx = 4, d = 100) is
bisected with term toggles; a fix is made only for a proven defect, otherwise it is reported as a property with evidence.

## Registered checks (limits fixed before any post-fix run)

**C1 (the wave 25C T1a, T1b, limits unchanged, registered configurations, `tests/transient_sources.rs`).**
Doubly periodic box 8x8, a = 2, dt = 0.01, 100 steps, PISO Euler, PIMPLE n_outer 3 (Euler), PISO BDF2:
T1a `max |ux - (0.3 + 2t)| <= 1e-9`, `max |uy| <= 1e-12`, `max |p| <= 1e-9`. T1b (sink s = 2): Euler recursion `<= 1e-9`;
analytic `<= 1e-2 a/s` (Euler), `<= 2e-3 a/s` (BDF2). The ignored `*_known_miss` tests are run with `--ignored` before and
after and compared; they are not edited. If they pass after the fix, new passing tests with the same limits are added and the old
records keep their ignore with a note.

**C2 (stability, new).** Plane Couette start-up, `box_2d` ny = 40, periodic x, nx in {4, 40}, PISO Euler, nu = 1,
d in {0.5, 1.6, 3, 6, 16, 100}, 60 steps: all values finite and `peak |u_x| <= 1 + 1e-9` (the exact solution is bounded by the wall
speed 1 for the Euler diffusion step). Same for PIMPLE (n_outer 3, alpha 0.8/0.5).

**C3 (x-uniformity, new).** In the Couette case of C2, `max |u_y| <= 1e-10` and the cell-to-cell x variation of `u_x` in a row
`<= 1e-10` at every step.

**C4 (no regression).** All existing tests pass unedited, in particular `tests/baseline_hashes.rs` (3 passed / 1 ignored, hashes not
re-recorded), `tests/periodic.rs` and the periodic transient tests of `rcfd-fv`, `rcfd-api` (`transient_sources` T2, T4, T6, T7).
Non-periodic transient runs are bit-identical before and after (no link exists, the code path is unchanged): checked by hashing
`ux, uy, p` of the lid-driven cavity PISO/PIMPLE run before and after. If any registered result of a periodic transient case
changes, work stops before committing the fix and the change is reported.

## Not attempted

Rotational periodicity, the fractional-step advance, the steady code, coupled/compressible algorithms.

## Addendum A: measured results, decisions taken, deviations (written after the runs; no registered limit changed)

Release builds, `CARGO_TARGET_DIR=<target dir>`.

### D1 implemented

`pimple.rs`: `PeriodicLinks` (private to the module) is built per momentum component and outer iteration from the same flux
(`phi_conv`), face diffusivity and boundary coefficients that the assembly used. The lagged partner term is removed from `b`
(`b += c_f u_partner,lag`) and the entry is used in the momentum solve (CSR, BiCGStab/ILU(0)), the implicit under-relaxation sum, the
reported momentum residual and the `H` operator. With no translationally periodic patch no link exists and every call falls through to
the previous code, so non-periodic transient results are unchanged (`baseline_hashes` 3 passed / 1 ignored with the hashes as
registered, the cavity PISO/PIMPLE hash included).

### C1 (T1, limits unchanged, the ignored registered tests run with `--ignored`)

| Check | Before (HEAD 2b73501) | After | Limit |
|---|---|---|---|
| T1a PISO Euler | ux 5.093e-1, uy 3.207e-4, p 1.510e-1 | 3.508e-14, 0, 0 | 1e-9 / 1e-12 / 1e-9 |
| T1a PIMPLE n_outer 3 | 1.080e-1, 1.276e-4, 3.089e-2 | 1.776e-14, 0, 0 | same |
| T1a PISO BDF2 | 5.091e-1, 3.797e-4, 1.563e-1 | 7.550e-14, 0, 0 | same |
| T1b recursion (Euler PISO / PIMPLE 3) | 7.587e-2 / 1.402e-2 | 4.996e-15 / 2.010e-14 | 1e-9 |
| T1b analytic (PISO / PIMPLE 3 / BDF2), of a/s | 7.841e-2 / 1.657e-2 / 7.691e-2 | 2.554e-3 / 2.554e-3 / 1.872e-4 | 1e-2 / 1e-2 / 2e-3 |

All registered configurations pass now. The `*_known_miss` tests are kept as they were (ignored, limits intact, a note line added).
New passing tests `t1a_..._implicit_periodic_coupling` and `t1b_..._implicit_periodic_coupling` repeat the registered configurations
with the same limits. The same holds for the wave 25C T5 impulse balance (PISO Euler): T5a 3.587e-15 (limit 1e-8), T5b 2.131e-15 on the
stable step and 2.690e-14 on the registered 10 steps of dt = 0.02 (1e-8), T5c 1.241e-14 (1e-9); before 4.319e-3, 3.434e-1 (registered)
and 2.453e-1. Two new passing tests `t5_..._implicit_periodic_coupling`. The registered-step T5b pass covers only 0.2 s; the T3 runs at
that step are still unstable (below), so it does not claim that the dt = 0.02 Hartmann flow works.

### C2, C3 (Couette start-up, new, limits above): PASS

`tests/w26a_couette.rs::c2_c3_periodic_couette_startup_is_stable_and_x_uniform`: PISO and PIMPLE (3 outer, 0.8/0.5), nx 4 and 40,
d from 0.5 to 100: peak |u_x| between 0.948 and 0.9875 (limit 1 + 1e-9), max |u_y| at most 1.1e-11 (1e-10), x variation at most
4.6e-11 (1e-10). Before the fix the periodic 4x40 case diverged at d = 6, 16, 100 (peak |u| 1.7e3, 1.5e3, 2.3e3).

### C4 (no regression): PASS

`cargo test --release` of `rcfd-fv` and `rcfd-api` (all test files, `--no-fail-fast`) before the last test additions: 128 test
binaries ok, none failed; `baseline_hashes` 3 passed / 1 ignored. No baseline hash or registered result changed, so the stop condition
of the wave did not trigger. Registered misses that remain (ignored, run with `--ignored`): T3 at dt = 0.02 (last-step change 2.6e-1,
profile error 9e79), T3d at dt 2e-2 (u 6.7e1, J 8.8e-1) and at dt 2.5e-4 with 3 outer iterations (u 7.9e-3, J 3.6e-5). Their limits
are intact.

### D2 decision: the periodic part was a defect (fixed); the wall-bounded remainder is a property of the scheme

* Defect part. In the periodic Couette box of the unmodified solver an exactly x-uniform flow develops, at step 0, an x variation
  (6.5e-3), a pressure (0.58) and a cross velocity (1.2e-3); with a non-periodic x none appears (1e-14), and the non-periodic box is
  stable for d up to 16. With D1 the periodic runs are stable at every d tested (C2). The wave 25C statement "independent of
  periodicity" is explained by the second mechanism below, which exists in non-periodic boxes too.
* Property part. In a closed box (lid driven, all walls fixed, nu = 1, 20x20, d = nu dt/dy^2, 40 steps) the 2-corrector PISO step is
  accurate and bounded at small d, loses accuracy as d grows and diverges beyond about d = 20 to 30, for lid speeds 1 and 1e-3 alike (the
  growth scales with the lid speed, so it is a linear instability of the step map, not the convective nonlinearity). Peak |u| over the
  lid speed (lid 1e-3): 0.8520 (d 1), 0.8523 (2 to 4), 0.858 (6), 0.882 (8), 0.902 (10), 0.967 (20), then 7.7e4 (100) and 5.5e10 (1000).
  The converged value is 0.852, so the error of the bounded solution grows with d before the divergence: it is the splitting error of
  the corrector iteration. More correctors are stable and converge: 3 correctors give 0.862 at d = 10 and 0.894 at d = 20, 4 correctors
  0.907 / 0.923 at d = 100 / 1000, 20 correctors 0.8528 / 0.8537 at d = 100 / 1000. PIMPLE with one corrector, 40 outer iterations and
  relaxation 0.7/0.3 is stable and converged at all d (0.852). PIMPLE with 3 outer iterations and 0.8/0.5 diverges for d >= 10, and
  with 2 correctors per outer iteration and relaxation it diverges for d >= 10 (the relaxed `H` of the later corrector uses the relaxed
  diagonal). In the open-ended Couette runs, switching off the transient Rhie-Chow term, or 1 or 4 correctors, moves the threshold but
  does not remove it.
  Mechanism (first principles, not proven analytically here): the corrector uses `u = H(u)/A_D - grad p/A_D` with the diagonal
  `A_D = 1/dt + 2 nu sum 1/dx^2` only, whereas the response of a smooth velocity mode to a pressure gradient is `1/(1/dt + nu k^2)`; the
  ratio is about `1 + 2d`. The pressure correction is therefore badly scaled by a factor that grows with d (observed: pressure 600 times
  `u dy/dt` at d = 100 in the first step), and the two-corrector sequence, a truncated Jacobi-type iteration for the velocity-pressure
  system (the splitting of Issa 1986), contracts only while that factor is small. Every term of the code is the standard form of the
  method and no term was found that departs from it, so this is classified as a property (conditional stability at large diffusion
  number), not a coding defect.
* Consequence. No solver change for this part. The remedies the evidence supports: more correctors (>= 4 up to d = 1000, 20 for the
  converged value), PIMPLE with one corrector and many relaxed outer iterations, or the coupled algorithm. The registered wave 25C T3 /
  T3d dt = 0.02 set-ups (d = 32 to 128) stay known misses; the stable-step variants stay the passing records.
* Not proven: the linear amplification factor of the step map was not extracted as an eigenvalue (a homogeneous-noise probe decays to
  round-off within three steps at d = 100, so growth rates measured on it were round-off and were discarded). The threshold d = 20 to
  30 is read from forced runs only.

### Deviations

* The D2 reproducers (closed box, `nonperiodic_nx4_scan`, corrector-count scans) are ignored diagnostics in `tests/w26a_couette.rs`; their
  numbers are measurements, not limits. The passing closed-box test (limit 1 = the lid speed, 20 correctors) is a characterisation and
  not a registered criterion.
* The open-ended (zero-gradient x ends) Couette case with nx 4 and 8 also diverges at some d (30, 100, 400) while nx 40 does not; the
  open ends carry a fluid flux and the case was not analysed further (it is placed in the property class by the closed-box evidence).
* `docs/roadmap.json`: one sentence in the F-214 note (the "root cause not found" claim); statuses unchanged.

### Not resolved

Rotationally periodic patches keep the lagged partner (named limitation). The large-d conditional stability is documented, not removed.
Steady code and fractional-step code were not touched.
