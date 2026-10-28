# Job 48T findings: pressure spokes on the refined cylinder O-grid

Registration, amendments, limits and the full results: [ADR 0154](../../adr/0154-job48t-cylinder-ogrid-pressure-spokes.md).
Work folder (runs, dumps, metrics, figures; outside the repository): `E:\projects\cfd\w48t-work`. Figures:
`E:\projects\cfd\w48t-work\report\figures`, Studio-path renders: `E:\projects\cfd\w48t-work\report\studio-renders`.

## Answer

**(b) a numerical artefact of the solver at high wall resolution.** The default transient algorithm of the case (PISO: one
outer iteration, two pressure correctors) loses its pressure-velocity coupling in the thin wall cells of the graded O-grid. With
`grading = 100` and `ny = 256` the first radial cell is 0.00262 D, five times thinner than on the committed 64 x 48 mesh
(0.01366 D). Refining at a fixed Courant number lowers dt only like the cell size, so the wall diffusion number
nu dt / dr1^2 rises like 1/h: 3.35 on 64 x 48, 6.5 on 128 x 96, 22.7 on the user's 256 x 256 at dt 0.0156. Above an onset
between 6.5 (clean) and 11.4 (spoky), the error the two correctors leave behind is amplified from step to step (measured
pressure-lag amplification 2.42 per step on the user's set-up, 0.36 on 64 x 48). The amplified mode lives in the first
0.2 D at the wall and is grid-scale around the cylinder (dominant wavelength 4.5 to 6 cells, broadband from 2 to about 20 cells,
the same number of cells at 256 and 512 cells around): these are the spokes. It saturates at about 50 free-stream dynamic
pressures (p reaches -231 .. 294 near the wall where the physical field spans -0.74 .. 0.58), makes cd jump by 10 to 25 from one
step to the next, does not decay (the user's dt 0.0625 run held it from t = 2 to 71 and then diverged), and on other set-ups
(64 x 256, no push, a ramped start, PIMPLE with weak relaxation) it grows until the divergence guard stops the run.

It is **not**: the Studio's save of the case (the resolved settings differ only in nx, ny, dt; the two files give byte-identical
runs), the display (every rendered pixel shows exactly the stored cell value), the Courant number (256 x 48 at Courant 10 is
clean; 64 x 256 at Courant 0.64 diverges), the transient Rhie-Chow term (`ddt_correction = false` still spoky), the convection
scheme (upwind still spoky), non-orthogonal correction (the mesh is orthogonal to 1e-11), the linear-solver tolerances (1e-12
still spoky; pressure solves take about 10 iterations against a cap of 3000), or the start (a restart from the developed 64 x 48
field and a ramped start both fail).

## What to do on the 256 x 256 case (workaround, no code change)

Change one setting of the case; each line below was run to t = 10 and is clean at t = 1, 3 and 10 (`M_hf` is the registered
spoke measure: the RMS of the grid-scale part of p at r = 0.55 to 0.70 D, in free-stream dynamic pressures; the 64 x 48 control
has 0.0057 at t = 3, the user's run 52.7):

| Setting (everything else as the user's case) | M_hf t = 1 / 3 / 10 | cd at t = 3 / 10 | Wall time per simulated second (one thread, shared machine) |
|---|---|---|---|
| `[solver] time_advance = "fractional_step"` (recommended: the cheapest accepted) | 1.9e-7 / 7.9e-8 / 4.1e-8 | 1.466 / 1.291 | 29 s |
| `[solver] n_correctors = 3` (stays PISO), dt 1/64 | 2.7e-7 / 9.2e-8 / 5.3e-8 | 1.466 / 1.296 | 49 s |
| `[solver] n_correctors = 3` with the user's dt 0.0156 | 2.8e-7 / 9.1e-8 / 4.3e-8 | 1.466 / 1.296 | 49 s |

For comparison the 128 x 96 run (clean) has cd 1.468 / 1.286. Also clean, measured to t = 3 or 10: `n_correctors = 4`; dt 1/256
(4 times the steps); and keeping the defaults but lowering `mesh.grading` to about 11 at ny = 256, which keeps the 64 x 48
first cell and puts the added radial cells into the field instead of the wall (closer to "a higher resolution far field").
Do not use: PIMPLE with `n_outer = 2` or `4` and `alpha_u = alpha_p = 0.9` (both diverged), `ddt_correction = false`,
`convection = "upwind"`, `time_scheme = "bdf2"` (still spoky, M_hf 15). Rule of thumb for the default PISO on this O-grid: keep
nu dt / dr1^2 below about 6, or use one of the settings above.

What to expect when it is right: at Re = 100 the impulsive start gives one large pressure impulse in the first step (cd 33 at
dt 1/16, larger at smaller dt), the force then settles within about one time unit (cd 1.47 at t = 3, 1.29 at t = 10 on every
clean mesh), the pressure field shows the stagnation point in front, suction at the sides and a low-pressure wake, and shedding
starts later (about 40 D/U with the push). Spokes, p values of hundreds, or cd jumping by tens between steps are never physical
in this case. The Studio colours each cell flat with its stored value and scales the colours to the snapshot's minimum and
maximum, so a few extreme wall cells make the rest of the domain one flat colour; that is the display showing a wrong field
faithfully, not the cause.

## Measured classification (registered rule L3: clean if M_hf <= max(2 x control, 0.01) at every checked time)

| Run | Mesh, dt, coupling | nu dt/dr1^2 | M_hf t = 1 / 3 / 10 | cd change per step (RMS, t in [2, 3]) | Class |
|---|---|---|---|---|---|
| R64 | 64 x 48, 1/16, PISO-2 (committed case) | 3.35 | 0.0028 / 0.0057 / 0.0008 | 0.004 | clean |
| R128 | 128 x 96, 1/32 | 6.51 | 8e-6 / 4.4e-5 / 1.3e-5 | 0.002 | clean |
| R256 | 256 x 256, 1/64 | 22.8 | 42.6 / 48.3 / 44.0 (44.9 at t = 30) | 16.2 | spoky |
| RU | 256 x 256, 0.0156 (bit-identical to the user's run) | 22.7 | 46.2 / 52.7 / 10.4 | 18.3 | spoky |
| B1 | 256 x 48, 1/64 (nx only) | 0.84 | 1.2e-6 / 1.1e-6 / 9e-7 | 0.001 | clean |
| B1b | 256 x 48, 1/16 (Courant 10) | 3.35 | 1.5e-6 / 1.1e-6 / 9e-7 | 0.004 | clean |
| B2 | 64 x 256, 1/16 (ny only) | 91 | 6.6e5 at t = 1 | - | diverged (step 19) |
| B2b | 64 x 256, 1/64 (Courant 0.64) | 22.8 | - | - | diverged (step 30) |
| B2c | 64 x 256, 1/64, 3 correctors | 22.8 | 253 / 282 | 69.9 | spoky |
| B3 | 256 x 256, grading 10.88, 1/64 | 0.84 | 3.8e-7 / 3.1e-7 / 2.5e-7 | 0.001 | clean |
| B4a, B4b, B4c | 256 x 256, dt 1/16, 1/128, 1/256 | 91, 11.4, 5.7 | 28 / 57; 30 / 26; 1.2e-7 / 1.1e-7 | 24.7; 11.9; 0.0003 | spoky; spoky; clean |
| B5 | R256 without the push | 22.8 | - | - | diverged (step 32) |
| B6 | R256 restarted from the developed 64 x 48 field at t = 10 | 22.8 | 43.3 / 49.9 (t0 + 1, + 3) | 14.5 | spoky |
| B6r | R256 started from rest with a ramped far field | 22.8 | - | - | diverged (step 46) |
| B7, B9, B10, B11, B12 | R256 + upwind; + 1 non-orth. corrector; + tolerances 1e-12; + `ddt_correction = false`; + BDF2 | 22.8 | 40 / 45 / 45 / 49 / 15 at t = 3 | 13.5 / 17.4 / 12.4 / 12.7 / 6.2 | all spoky |
| B8a, B8b | R256 + 3, 4 correctors | 22.8 | 9.2e-8, 9.1e-8 at t = 3 | 0.001 | clean |
| W1, W2 | R256 + PIMPLE n_outer 2, 4 (alpha 0.9) | 22.8 | -, 78.6 at t = 1 | - | diverged (steps 39, 81) |
| W3, W5, W6 | fractional step; 3 correctors; 3 correctors at dt 0.0156 | 22.8 | see the workaround table | 0.001 | clean |

The user's own stores: 64 x 48 (0.0057 at t = 3, identical to R64); 256 x 256 dt 0.0625 (28 / 57 / 53, 48 to 69 up to t = 71,
then diverged); 256 x 256 dt 0.0156 (46 / 53); 512 x 512 dt 0.0625 (369 at t = 1, diverged at step 22).

## Mechanism (diagnostic DG1, amendment 1)

The repository's ADR 0073 pressure-lag estimate (power iteration of the linearised step map of a lagged pressure error) is enabled
by the case path only with one corrector. Run through a harness that reproduces the case's run bit for bit (E1), with the
arm's correctors:

| Arm | Twin (class) | nu dt/dr1^2 | rho at step 1 | rho near t = 1 | Largest mode entry |
|---|---|---|---|---|---|
| G64 | R64 (clean) | 3.35 | 0.358 | 0.281 | wall cell, r = 0.506 |
| G128 | R128 (clean) | 6.51 | 0.746 | 0.675 | wall cell, r = 0.503 |
| G256x48 | B1 (clean) | 0.84 | 0.221 | 0.233 | r = 0.57 to 0.67 |
| G256g | B3 (clean) | 0.84 | 0.219 | 0.217 | r = 0.51 to 0.61 |
| G256c4 | B8b (clean) | 22.8 | 0.563 | 0.522 | wall cell, r = 0.501 |
| G256s | B4c (clean) | 5.69 | 1.166 | 1.071 | wall cell, r = 0.501 |
| G256 | R256 (spoky) | 22.8 | 2.418 | 1.679 | wall cell, r = 0.501 |
| G64x256 | B2b (diverged) | 22.8 | 4.359 | 1.947 (t = 0.25) | wall cell, r = 0.501 |
| G256p2 | W1 (diverged) | 22.8 | 2.433 | stopped at step 39 | wall cell, r = 0.501 |

Every failing arm has rho >= 1.68 and every clean arm rho <= 1.17; the registered threshold rho = 1 is crossed by one clean arm
(G256s, 1.07 to 1.17), so the registered reading is "rejected" in its strict form, while rho is a strong separator. The mode sits
in the first cell layer at the wall in every failing arm.

## Images (paths relative to `E:\projects\cfd\w48t-work`, outside the repository)

* The user's run as the Studio draws it at t = 3: `report\studio-renders\U3-t3-zoom.png`, `U3-t3-domain.png`.
* Resolution series through the Studio path, t = 1, 3, 10: `R64-t*-zoom.png`, `R128-t*-zoom.png`, `R256-t*-zoom.png`; the
  workarounds: `W3-t*-zoom.png`, `W5-t*-zoom.png` (and `-domain` views); per-render check data `*.json` (0 mismatching pixels on
  five adapters).
* CPU maps of the stored p (r against theta, near-wall band; full range and 1-99 percentile range):
  `report\figures\<run>-p-polar-t<t>.png` for R64, R128, R256, RU, B2c, W3, W5.
* Ring spectra at t = 3: `report\figures\spectra-t3.png`; M_hf against time: `report\figures\m_hf-vs-time.png`.

## Tools

`scripts/w48t_run_matrix.py` (the registered runs, through the command line `rcfd-runner` builds), `scripts/w48t_spoke_metrics.py`
(metrics, screen, classification table, figures), `crates/rcfd-studio/tests/w48t_cylinder_spokes.rs` (store dump; the Studio-path
display check D1), `crates/rcfd-api/tests/w48t_cylinder_spokes.rs` (replica harness E1, pressure-lag diagnostic DG1, known misses
K1 and K2, workaround tests W3 and W5; all ignored, run with `cargo test --release -p rcfd-api --test w48t_cylinder_spokes --
--ignored`).
