---
type: source
tags: [investigation, naca0012, spalart-allmaras, flat-plate, wall-treatment, far-field]
sources: []
origin: docs/investigations/job49a-sa-drag/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 49A: why the steady SA drag of the NACA 0012 was wrong

**Origin:** [docs/investigations/job49a-sa-drag/README.md](../../docs/investigations/job49a-sa-drag/README.md) at origin/main 2e94e5ef (84 lines).
**Kind:** investigation results folder. Set-up, limits, amendment 1 and evaluation: [ADR 0164](../../docs/adr/0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md).

## Summary

"ADR 0159's SA drag had two independent causes: the force report at 0 degrees and the far-field nuTilda from 4 degrees up." Neither is the SA model, its constants or its wall law. With the opt-in component `total_effective`, cd at 0 degrees is +3.6 % against the TMR reference; with the opt-in far-field role as well, cd at 10 degrees is +26 % on the 50-chord far field and +10 % on 500 chords, "and the remaining +10 % is not attributed". At y+ 5 SA's wall mode `auto` over-predicts friction by 20 %, a separate item (job 49E, [[doc-investigation-job49e-sa-wall]]).

## Key points

- Reference: NASA TMR NACA 0012 SA validation case (CFL3D, 897 x 257 grid, 500 chords, nuTilda = 3 nu): cd 0.00819 at 0 degrees (friction 0.00683, pressure 0.00136); cl 1.0909 and cd 0.01231 at 10 degrees. Files in `validation-data/reference/w49a_*`.
- Cause 1, the force report: the default `force_coefficient` takes the viscous stress with the molecular viscosity and the wall cell's u_P/dn; at y+ about 40 the solver applies the wall-function shear through nu + nu_t,w. Reported friction is 0.32 of the applied friction on the aerofoil, 0.35 on the y+ 40 plate; at y+ 1 they agree to 0.5 %. With `total_effective`, cd at 0 degrees is 0.00848 (+3.6 %). SST's 0.0036 is the same effect (0.003637 default, 0.008317 effective).
- Cause 2, the far-field nuTilda: a `velocity = "freestream"` far field got the turbulence role `zero_gradient`, so nuTilda grew to 6.0e4 nu in the outer flow at 10 degrees (24 nu at 0), an outer eddy viscosity of 0.01 U c; the cd drift is this growth (A10: cd +19 % between iterations 1001-2000 and 7001-8000).
- Fix 2 (amendment 1): `boundaries.farfield.turbulence = "freestream"` (3 nu on inflow faces, zero gradient on outflow). Outer nuTilda stays 3.000 nu; targets met at iteration 577; at 10 degrees cl 0.982 to 1.055 (TMR -3.3 %), cd 0.0445 to 0.0155 (+26 %). On 500 chords (C10): cl 1.065 (-2.4 %), cd 0.0135 (+10 %). No point-vortex correction.
- The registered far field `turbulence = "inlet"` (arms B00, B10) fixes nuTilda on outflow faces too; nut reached the cap of 1e5 nu and nuTilda grew without bound.
- Untested candidates for the remaining +10 %: the wall law at y+ 40, buffer-layer wall cells, first-order nuTilda convection, the O-grid's resolution.
- Flat plate (SA, Re 5e6 per m, against TMR Cf(0.97) 2.70565e-3): P01 y+ 0.99 +3.1 %; P05 y+ 5.3 **+19.9 %**; P40 y+ 39.6 +4.3 % with molecular / wall-law 0.352. M1w's wall cells span y+ 5.5 to 61 (median 42).
- A00 and A10 reproduce ADR 0159's SA-a00 and SA-a10 bit for bit (10 314 and 16 000 values).
- Rebuild: `scripts/w49a_sa_drag.py` (`mesh`, `cases`, `run`, `adopt`, `collect`).

## Folder contents (not ingested)

- `results.json`: every run (exit, iterations, window statistics of every report, outer nuTilda, plate wall data, the bit comparison with ADR 0159) and the evaluation of every limit.
- `results.csv`: one row per run.
- `cd_histories.png`: cd against iteration (default and `total_effective`): A10's drift, F10 and C10 settling.

## Contradictions with the wiki

None. The README calls both fixes opt-in; they became defaults afterwards: ADR 0166 makes the effective wall shear and the SA far-field `freestream` role defaults, and ADR 0169 extends the far-field default to SST and k-epsilon ([[doc-case-schema-turbulence]]). This is a supersession, not a contradiction.

## Pages this source informs

- [[naca0012]], [[spalart-allmaras]], [[flat-plate]], [[doc-case-schema-turbulence]]
- [[doc-investigation-job48y-aoa-sweep]] (the drag it explains), [[doc-investigation-job49d-aoa-sweep-fixed]] (the sweep with the fixes)
