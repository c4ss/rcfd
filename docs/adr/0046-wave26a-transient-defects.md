# ADR 0046: Wave 26A, two pre-existing defects of the transient PISO / PIMPLE algorithms

Status: accepted. The decisions and registered checks below were written before any solver code of this wave was changed and before
any post-fix run. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the limit intact; a set-up defect gets a NEW corrected test and the old one stays. An
addendum (measured values, deviations) is appended at the end. Clean-room: first principles and published literature only (Issa 1986
for PISO); no vendor code, text or names.

Scope: `crates/rcfd-fv/src/pimple.rs` momentum matrix assembly for translationally periodic patches. Not touched: SIMPLE/SIMPLEC steady
code, `fractional.rs` (read only), the `sources.<n>` mechanism.

## Findings that precede the fix (diagnosis, added after a bisect; the registered checks below do not depend on them)

* Before-values of the wave 25C known-miss tests, rerun on the unmodified HEAD 51f1980 (`--ignored`): T1a PISO Euler ux 5.093e-1,
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
