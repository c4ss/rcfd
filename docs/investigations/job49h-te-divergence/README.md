# Job 49H: NACA 0012 trailing-edge divergence at fine wall spacing

ADR: `docs/adr/0171-job49h-the-naca0012-trailing-edge-divergence-at-fine-wall-spacing.md` (registration, amendments 1 to 3, addendum with results).

- `results.json`: every run's end state (clean at the iteration cap, or guard stop with iteration, cell and position), the full-length window means, and
  the evaluation of every registered limit (`evaluation.<id>.met`).
- `results.csv`: one row per short run.
- Regenerate: `python scripts/w49h_te_divergence.py collect <work-folder> docs/investigations/job49h-te-divergence` (the work folder holds `runs/<id>`).
  `cases` writes the case files; `run <id>` runs one; `list` shows the registry.
- Tests: `crates/rcfd-api/tests/te_divergence_w49h.rs` (missed limits are ignored `*_known_miss` tests).

Short summary: the unlimited `linear_upwind` extrapolation diverges in the trailing-edge tip cells of the y+ 2 O-grid at 6 degrees (iteration 15); pressure-velocity
coupling, relaxation, turbulence model and mesh transform do not change it; `solver.linear_upwind_limiter = "barth_jespersen"` removes it in every run tried
and costs 3 % of `cd_eff` at 0 degrees. LA0 and LB6 rows come from `monitors.csv` (the status stream was lost with a scratch folder); see the ADR's deviations.
