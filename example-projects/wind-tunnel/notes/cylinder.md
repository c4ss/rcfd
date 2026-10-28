# Flow past a cylinder: vortex shedding at Re 100 and Re 200

## What it is

A circular cylinder of diameter D = 1 m in a uniform stream U = 1 m/s, two-dimensional and laminar. Above a Reynolds number
of about 47 the wake becomes unstable and sheds vortices alternately from the top and the bottom (a von Karman vortex
street); the lift force then oscillates at the shedding frequency f, given as the Strouhal number St = f D / U.

* `cases/cylinder-re100.toml`: Re = U D / nu = 100. `cases/cylinder-re200.toml`: the variant, nu halved (Re = 200).
* Mesh: the repository's `cylinder_ogrid` generator, 64 cells around and 48 radial cells graded towards the wall, far field
  at 15 D (3072 cells), with free-stream conditions on the far field.
* Solver: PISO, second-order upwind convection, time step 0.0625 D / U, 80 D / U (1280 steps). The start is uniform flow
  plus a sideways push behind the cylinder, so shedding starts within about 40 D / U instead of waiting for round-off.
* Stored: `ux`, `uy`, `p` and `vorticity_z` once per D / U from t = 1 (80 snapshots), single precision.
* Reports: drag and lift coefficients `cd` and `cl` (reference area D x depth), every step.

## What to click

1. Select the run of `cases/cylinder-re100.toml` in the Runs panel; the Viewport shows `p` at the last snapshot.
2. Choose `vorticity_z` in the "field" box and "whole run" as the range, so the colours stay fixed while you scrub.
3. Press first, then play: the push behind the cylinder, the wake starting to swing, and from about t = 40 the regular
   street. One shedding period is about 6.6 D / U, so about 6 to 7 stored frames per period: the animation is coarse in time.
4. In the Plots panel, `cl` swings around zero and `cd` oscillates at twice that frequency; the line marks the shown step.
5. Select the run of `cases/cylinder-re200.toml` and compare: shorter period, stronger lift swing.

## How it relates to published data, and the limits

The repository's validation runs of this flow (ADR 0062, ADR 0069, `crates/rcfd-api/tests/validation_cylinder_re100*.rs`,
128 x 100 cells, far field at 20 D, 160 D / U) measured St 0.159 to 0.165 and a mean drag coefficient 1.31 to 1.34 at
Re 100. This example uses about a quarter of those cells and half the time; its Strouhal number and mean drag are in the
run table of `notes/README.md` and in `designs/variants.ledger.jsonl` (computed by `scripts/make_example_projects.py` from
the `cl` and `cd` series over the whole shedding periods of the second half of the run, ADR 0142 amendment 1; only about
six periods fit, so the Strouhal number is known to a few percent at best). They are not a validation: one coarse mesh, one time step, no
uncertainty estimate. A real cylinder wake becomes three-dimensional near Re 190, which a two-dimensional run cannot show,
so the Re 200 run is a two-dimensional model of that flow.
