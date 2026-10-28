# Backward-facing step: laminar flow at Re 800 and Re 400

## What it is

A channel suddenly doubles its height at a step (expansion ratio 2). The flow separates at the step lip, a recirculating
bubble forms behind the step and the flow reattaches on the floor further downstream; at higher Reynolds numbers a second
bubble forms on the upper wall. This is Gartling's laminar benchmark (1990): channel height H = 1 m downstream of the
step, step height and inlet height h = H / 2, a parabolic inflow of mean velocity 1 m/s, Re = U_mean H / nu.

* `cases/step-re800.toml`: Re = 800 (Gartling's condition). `cases/step-re400.toml`: the variant, nu doubled (Re = 400).
* Domain: Gartling's formulation starts the domain at the step, so there is no upstream channel and no visible corner in
  the picture: the lower half of the left boundary (y < 0.5 m) is the step face (zero velocity, a wall) and the upper half
  is the inlet with the parabolic profile `24 (y - 0.5)(1 - y)` (one expression covers both). The channel is 15 H long
  (Gartling used 30 H).
* Mesh: the repository's `box_2d` generator, 200 x 40 uniform cells (8000 cells).
* Solver: SIMPLEC, second-order upwind convection, until the residual targets 1e-6 are met. Stored: `ux`, `uy`, `p` every
  40 iterations, single precision. Report: `p_inlet`, the mean pressure on the left boundary (the outlet is at p = 0). It
  comes out negative: the flow slows down behind the expansion and its pressure rises more than friction takes away
  over the 15 H.

## What to click

1. Select the run of `cases/step-re800.toml`; choose `ux` in the "field" box: the darkest colours along the floor behind
   the step are the reversed flow of the main bubble, those along the upper wall further downstream the second bubble.
2. Hover along the floor to find where `ux` changes sign: that is the reattachment point.
3. Slide over the iterations to see the bubbles form while the solver converges (iterations, not time).
4. Select the run of `cases/step-re400.toml`: a shorter main bubble and a much smaller upper one.

## How it relates to the benchmark, and the limits

The repository's descriptor `../../cases/backward_step/case.json` is a different flow: Driver and Seegmiller's turbulent
step (expansion ratio 1.125, Re_h 37500, k-epsilon or SST). This example is the laminar expansion-ratio-2 step that the
repository checks in `../../crates/rcfd-fv/tests/step_laminar.rs` with its own step mesh and in
`../../docs/bfs-convergence-findings.md`. The published values that the repository quotes for Re 800 (secondary sources,
see `../../docs/reference-data.md`): main reattachment at 12.20 step heights (6.10 H), upper bubble from 9.70 to 20.96
step heights (4.85 H to 10.48 H). `scripts/make_example_projects.py` reads the reattachment points of these runs from
their last stored `ux` (sign changes along the first cell row above the floor and below the upper wall, linear
interpolation) into the run table of `notes/README.md` and `designs/variants.ledger.jsonl` (`x_reattach`,
`x_top_separation`, `x_top_reattach`, in metres, so in units of H). They are not a validation: one coarse mesh, the outlet
at 15 H instead of 30 H, values read from single-precision snapshots, no uncertainty estimate.
