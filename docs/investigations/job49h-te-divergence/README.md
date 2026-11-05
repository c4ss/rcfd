# Job 49H: NACA 0012 trailing-edge divergence at fine wall spacing

Short summary: the unlimited `linear_upwind` extrapolation diverges in the trailing-edge tip cells of the y+ 2 O-grid at
6 degrees (iteration 15). Pressure-velocity coupling, relaxation, turbulence model and mesh transform do not change it.
The limiter setting `solver.linear_upwind_limiter = "barth_jespersen"` removes it in every run tried and costs 3 % of
`cd_eff` at 0 degrees. Missed limits are ignored `*_known_miss` tests.

The ADR holds the registration, amendments 1 to 3 and the addendum with results:
`docs/adr/0171-job49h-the-naca0012-trailing-edge-divergence-at-fine-wall-spacing.md`. The LA0 and LB6 rows come from
`monitors.csv` (the status stream was lost with a scratch folder); see the ADR's deviations.

- `results.json`: every run's end state (clean at the iteration cap, or guard stop with iteration, cell and position),
  the full-length window means, and the evaluation of every registered limit (`evaluation.<id>.met`).
- `results.csv`: one row per short run.
- `crates/rcfd-api/tests/te_divergence_w49h.rs`: the tests.

To regenerate the results, run
`python scripts/w49h_te_divergence.py collect <work-folder> docs/investigations/job49h-te-divergence` (the work folder
holds `runs/<id>`). In the same script, `cases` writes the case files, `run <id>` runs one and `list` shows the registry.
