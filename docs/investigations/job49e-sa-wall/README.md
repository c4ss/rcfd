# SA wall treatment with the wall cell in the buffer layer (job 49E, ADR 0168)

The set-ups, limits, amendment 1 and the evaluation are in
[ADR 0168](../../adr/0168-job49e-sa-wall-treatment-in-the-buffer-layer.md). This folder holds the results only. No run or
mesh enters git. To build them again, use `scripts/w49e_sa_wall.py` (`mesh`, `cases`, `run`, `collect`, `model`) in an
empty work folder.

## Finding

SA's default wall mode `auto` over-predicts the friction when the wall cell lies in the buffer layer.
- On the flat plate the error is +20 % at wall y+ 5 to 8, +14 % at 12 and +11 % at 15, against +3 % to +4 % at y+ 1 and
  y+ 40.
- On the NACA 0012 at 0 degrees it is +17 % in friction on a mesh of median wall y+ 5, and +9.5 % at 15.

**Cause.** The cause is the linear interpolation of the eddy viscosity to the face between the wall cell and the next
cell, across the steep, non-linear buffer-layer profile.
- The wall function itself is consistent. Its u_tau iteration, wall-face viscosity and wall nuTilda are not the cause.
- A 1-D model of exactly that discretisation reproduces the default curve within 1.2 points at all 10 y+ (limit LH1).

**The opt-in fix.** `turbulence.sa.wall_mode = "auto_sa_law"`, as amended (amendment 1):
- the wall law is the SA model's own inner-layer law;
- nuTilda = kappa u_tau y in every wall cell;
- on the wall cell's wall-normal face, the law's series-resistance viscosity and the law's tangential velocity in the
  velocity gradient;
- on all other internal faces, nut of the interpolated nuTilda.

It brings the plate within +2.3 % to +4.4 % of TMR at every y+ from 1 to 60, within 2.0 % of its own y+ 1 value. The
aerofoil's cd is within 1.3 % to 2.3 % of TMR at wall y+ 5, 15 and 40. No constant was tuned.

As registered, the opt-in had a y+ 5 switch that made it bistable at y+ 5 (E05 +7.5 %, a known miss) and over-produced
nuTilda in the second cell. Amendment 1 fixed both before its runs.

## Flat plate: Cf(0.97) against TMR SA 2.70565e-3 (wall-law value; y+ = wall-cell y+ at x = 0.97)

| Target y+ | Default (D): y+ / error | As registered (E): y+ / error | Amendment 1 (F): y+ / error | F: mean C_F vs Kármán-Schoenherr | F: second-cell nuTilda/(kappa u_tau y) |
|---|---|---|---|---|---|
| 1 | 0.99 / +3.1 % | 0.98 / +2.3 % | 0.98 / **+2.3 %** | -0.1 % | 1.08 |
| 2 | 1.99 / +4.9 % | 1.97 / +2.9 % | 1.97 / **+3.2 %** | +0.9 % | 1.15 |
| 3 | 3.02 / +7.7 % | 2.96 / +3.4 % | 2.97 / **+4.4 %** | +2.0 % | 1.07 |
| 5 | 5.31 / **+19.9 %** | 5.03 / +7.5 % (pinned at 5) | 4.95 / **+4.2 %** | +1.8 % | 1.06 |
| 8 | 8.49 / **+19.7 %** | 7.91 / +3.9 % | 7.88 / **+3.1 %** | +1.0 % | 1.04 |
| 12 | 12.44 / +14.2 % | 11.83 / +3.2 % | 11.79 / **+2.6 %** | +0.8 % | 1.01 |
| 15 | 15.36 / +11.4 % | 14.77 / +3.0 % | 14.74 / **+2.5 %** | +0.9 % | 1.00 |
| 30 | 29.93 / +5.7 % | 29.49 / +2.7 % | 29.45 / **+2.4 %** | +1.2 % | 0.97 |
| 40 | 39.63 / +4.3 % | 39.31 / +2.6 % | 39.26 / **+2.4 %** | +1.4 % | 0.95 |
| 60 | 59.04 / +2.9 % | 59.00 / +2.7 % | 58.90 / **+2.4 %** | +1.9 % | 0.92 |

- Schlichting's local law gives 0.002881 at x = 0.97. TMR's SA value is 6.1 % below it, a model property; the F plates are
  2 % to 4 % below Schlichting.
- The `viscous_effective` report equals the integral of the wall-law Cf within 0.6 % on every opt-in plate (LP3).
- The plate set-up has a baseline of about +2 % to +3 % that no wall treatment removes. It shows at y+ 1, where the cells
  resolve the layer: no run-up section, the streamwise resolution, first-order nuTilda convection.
- See `cf_vs_yplus.png`. The dotted curves are the 1-D model (D7). Its opt-in curve is the registered version, with the
  switch.

## NACA 0012, alpha 0, Re 6e6 (ADR 0164's F00 set-up: far-field role `freestream`, `total_effective`)

| Mesh (wall y+ median, range) | Arm | Iterations | cd_eff (vs TMR 0.008192) | friction (vs 0.006831) | pressure (vs 0.001361) |
|---|---|---|---|---|---|
| M1w05 (5.4; 0.8-7.5) | default AD05 | 8000 (cap) | 0.009652 (+17.8 %) | +17.4 % | +20.1 % |
| | registered AE05 | 8000 (cap) | 0.008610 (+5.1 %) | +5.6 % | +2.8 % |
| | **amendment 1 AF05** | 8000 (cap) | **0.008358 (+2.0 %)** | +2.6 % | -0.8 % |
| M1w15 (15.6; 2.1-21.6) | default AD15 | 335 | 0.008957 (+9.3 %) | +9.5 % | +8.6 % |
| | registered AE15 | 322 | 0.008345 (+1.9 %) | +2.1 % | +0.8 % |
| | **amendment 1 AF15** | 321 | **0.008295 (+1.3 %)** | +1.5 % | 0.0 % |
| M1w (42.1; 5.5-60.8) | default AD40 (= ADR 0164 F00) | 261 | 0.008402 (+2.6 %) | +2.5 % | +3.0 % |
| | registered AE40 | 8000 (cap) | 0.008427 (+2.9 %) | +2.6 % | +4.0 % |
| | **amendment 1 AF40** | 259 | **0.008378 (+2.3 %)** | +2.1 % | +3.3 % |

The spread of the opt-in cd over the three meshes is 1.0 % of TMR (amendment 1) against 3.2 % (as registered) and 15.3 %
(default).

## Files

| File | Contents |
|---|---|
| `results.json` | every run: exit, iterations, window statistics, plate wall data and profile diagnostics, aerofoil wall y+; the bit comparisons with ADR 0164; the evaluation of every limit (registered and amendment 1) and the 1-D model |
| `results.csv` | one row per run |
| `cf_vs_yplus.png` | plate Cf(0.97) error against wall y+: default, as registered, amendment 1 and the 1-D model |
