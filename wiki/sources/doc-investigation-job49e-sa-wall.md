---
type: source
tags: [investigation, spalart-allmaras, wall-treatment, flat-plate, naca0012, y-plus]
sources: []
origin: docs/investigations/job49e-sa-wall/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 49E: SA wall treatment with the wall cell in the buffer layer

**Origin:** [docs/investigations/job49e-sa-wall/README.md](../../docs/investigations/job49e-sa-wall/README.md) at origin/main 2e94e5ef (80 lines).
**Kind:** investigation results folder. Set-ups, limits, amendment 1 and evaluation: [ADR 0168](../../docs/adr/0168-job49e-sa-wall-treatment-in-the-buffer-layer.md).

## Summary

"SA's default wall mode `auto` over-predicts the friction when the wall cell lies in the buffer layer": on the flat plate +20 % at wall y+ 5 to 8, +14 % at 12 and +11 % at 15, against +3 % to +4 % at y+ 1 and 40; on the NACA 0012 at 0 degrees +17 % in friction at median wall y+ 5 and +9.5 % at 15. Cause: "linear interpolation of the eddy viscosity to the face between the wall cell and the next cell, across the steep, non-linear buffer-layer profile". The wall function itself is consistent. The opt-in fix `turbulence.sa.wall_mode = "auto_sa_law"` (as amended) brings the plate within +2.3 % to +4.4 % of TMR at every y+ from 1 to 60; "No constant was tuned."

## Key points

- A 1-D model of exactly that discretisation reproduces the default curve within 1.2 points at all 10 y+ (limit LH1).
- `auto_sa_law` as amended (amendment 1): the SA model's own inner-layer law; nuTilda = kappa u_tau y in every wall cell; on the wall cell's wall-normal face the law's series-resistance viscosity and tangential velocity; on all other internal faces nut of the interpolated nuTilda.
- As registered, the opt-in had a y+ 5 switch that made it bistable at y+ 5 (E05 +7.5 %, a known miss) and over-produced nuTilda in the second cell; amendment 1 fixed both before its runs.
- Plate Cf(0.97) against TMR SA 2.70565e-3, default / amendment 1: y+ 1 +3.1 % / +2.3 %; 2 +4.9 % / +3.2 %; 3 +7.7 % / +4.4 %; 5 **+19.9 %** / +4.2 %; 8 **+19.7 %** / +3.1 %; 12 +14.2 % / +2.6 %; 15 +11.4 % / +2.5 %; 30 +5.7 % / +2.4 %; 40 +4.3 % / +2.4 %; 60 +2.9 % / +2.4 %.
- Schlichting's local law gives 0.002881 at x = 0.97; TMR's SA value is 6.1 % below it, "a model property". The plate set-up has a baseline of about +2 % to +3 % that no wall treatment removes (no run-up section, streamwise resolution, first-order nuTilda convection). `viscous_effective` equals the integral of the wall-law Cf within 0.6 % (LP3).
- NACA 0012, alpha 0, Re 6e6 (ADR 0164's F00 set-up), cd_eff against TMR 0.008192: M1w05 (median y+ 5.4) default +17.8 %, registered +5.1 %, amendment 1 **+2.0 %**; M1w15 (15.6) +9.3 % / +1.9 % / **+1.3 %**; M1w (42.1) +2.6 % / +2.9 % / **+2.3 %**. Spread over the three meshes 1.0 % of TMR (amendment 1) against 3.2 % (registered) and 15.3 % (default).
- Rebuild: `scripts/w49e_sa_wall.py` (`mesh`, `cases`, `run`, `collect`, `model`).

## Folder contents (not ingested)

- `results.json`: every run (exit, iterations, window statistics, plate wall data and profile diagnostics, aerofoil wall y+), the bit comparisons with ADR 0164, the evaluation of every limit (registered and amendment 1) and the 1-D model.
- `results.csv`: one row per run.
- `cf_vs_yplus.png`: plate Cf(0.97) error against wall y+ (default, as registered, amendment 1, 1-D model).

## Contradictions with the wiki

None. [[doc-case-schema-turbulence]] lists `auto_sa_law` as opt-in (ADR 0168), consistent with this README.

## Pages this source informs

- [[spalart-allmaras]], [[flat-plate]], [[naca0012]], [[doc-case-schema-turbulence]]
- [[doc-investigation-job49a-sa-drag]] (which found the y+ 5 excess)
