# Reactor cases (`rcfd reactor`, F-112) and FW-H acoustics (`rcfd acoustics`, F-136)

This page describes two stand-alone commands that read their own JSON input rather than a CFD case: a zero-dimensional
chemical reactor and a far-field acoustic calculation from a permeable data surface. It is for users who want to run either
command; it gives what each computes, its limits, and the input file with every key. The reactor work is partial (it has no
flame-speed solver); the acoustic command works from a permeable surface in a medium at rest and leaves out the volume
quadrupole. For moving blade surfaces, uniform flow, rotor tones and noise metrics read [acoustics.md](acoustics.md)
instead.

Checks, limits and results: `docs/adr/0013-reactor-and-fwh.md`. Both commands read JSON, print a summary or CSV and
report every problem with its JSON path (exit code 3 for an invalid input, 1 for I/O failures, 2 for usage).

## `rcfd reactor <case.json> [--out <file.csv>]`

Limits: ideal gas only, constant-area constant-pressure plug flow without pressure drop, no heat exchange model, no
surface chemistry, no 1D flame speed (so F-112 stays partial), no sensitivity analysis.

### Reactor case file

```json
{
  "mechanism": {"chemkin": "grimech30.dat", "thermo": "thermo30.dat"},
  "reactor": {
    "type": "constant_volume",
    "energy": "adiabatic",
    "temperature": 1100, "pressure": 101325,
    "composition": {"mole": {"H2": 2, "O2": 1, "N2": 3.76}},
    "end_time": 0.01
  },
  "tolerances": {"rtol": 1e-8, "atol_t": 1e-8, "atol_y": 1e-14}
}
```

* `mechanism.chemkin` (required) and `mechanism.thermo` (optional if the mechanism file has an inline THERMO section):
  paths relative to the case file. The CHEMKIN-II text format is read: ELEMENTS (optional `NAME/weight/`), SPECIES,
  THERMO (NASA-7, fixed columns), REACTIONS with `CAL/MOLE` (default), `KCAL/MOLE`, `JOULES/MOLE`, `KJOULES/MOLE`,
  `KELVINS`, `EVOLTS`; third bodies and efficiencies, `(+M)`/`(+X)` falloff with `LOW` and `TROE`, `REV`, `DUPLICATE`.
  `SRI`, `PLOG`, `FORD`, `LT`, `HIGH`, `MOLECULES` are rejected with a line number. No mechanism ships with rcfd;
  `python validation-data/fetch.py fetch gri-mech-30` downloads GRI-Mech 3.0 (licence status: citation requested,
  recorded as unverified in `docs/validation-data.md`).
* `reactor.type`: `constant_pressure`, `constant_volume` (need `end_time`), `plug_flow` (needs `length`, `velocity`),
  `psr` (needs `residence_time`; optional `inlet_temperature`, default `temperature`, and `blowout`), `equilibrium`
  (`constraint` `TP`, `HP` (default) or `UV`).
* `energy`: `adiabatic` (default) or `isothermal`. `composition` is `{"mole": {...}}` or `{"mass": {...}}`.
* Batch output: `ignition_delay_max_dTdt` (time of the maximum dT/dt, null if the temperature rises by less than
  `ignition.temperature_rise`, default 400 K), `ignition_delay_temperature_rise`, final state; `--out` writes the
  accepted-step trajectory (decimated to `output_points`, default 200) with all mass fractions.
* PSR: the steady state is found by pseudo-time integration from the adiabatic equilibrium of the inlet mixture.
  `"blowout": {"tau_start": 0.01, "bracket": 0.05}` instead continues downwards in residence time (factor 0.9) and
  bisects until the burning/extinct residence times are within the bracket; burning means
  `T > T_in + 0.4 (T_eq - T_in)`.
* The integrator is the in-repo extrapolated linearly implicit Euler method (order 6 columns, forward-difference
  Jacobian, dense LU); this costs about 80 right-hand-side evaluations per step, adequate for mechanisms of tens of
  species. Large mechanisms (hundreds of species) would need sparse algebra.

## `rcfd acoustics <surface.json> --observer x,y,z [--observer ...] [--times t0:t1:n] [--out <file.csv>]`

Far field of the Ffowcs Williams-Hawkings analogy, Farassat/Brentner formulation 1A, from a permeable data surface
in a medium at rest. Output columns: observer index, observer time, thickness, loading and total acoustic pressure.
Without `--times` the whole valid window (the span of observer times whose retarded times lie inside the record) is
written at the sample interval. The volume quadrupole is not included.

### Surface file

Surface file (`rcfd-fwh-surface` version 1):

```json
{"format": "rcfd-fwh-surface", "version": 1, "rho0": 1.2, "c0": 340.0, "t0": 0.0, "dt": 1e-4,
 "elements": [
   {"area": 0.01,
    "x0": [1, 0, 0], "n0": [1, 0, 0],
    "p": [..], "rho": [..], "u": [[ux, uy, uz], ..]}
 ]}
```

Per element: `area`; the position either as `x0` (fixed) or `x` (one `[x,y,z]` per sample); the unit outward normal as
`n0` or `n`; optional surface velocity `v` per sample (zero if omitted; must stay subsonic); `p` (acoustic pressure
p', Pa) per sample; `rho` (density perturbation) and `u` (fluid velocity) per sample, zero if omitted. All series have
the same number of samples (at least 8) on the uniform axis `t0 + i dt`. Observer times must keep every retarded time
three samples inside the record; the error message names the valid window.

## Where to go next

* [acoustics.md](acoustics.md): the moving-surface FW-H solver, rotor tones and noise metrics for fans.
* [validation-data.md](validation-data.md): how to fetch reference data such as the mechanism files.
