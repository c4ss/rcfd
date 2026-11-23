---
type: source
tags: [adr-thread, physics, variable-density, compressible, real-gas, cht, radiation, chemistry, combustion, mhd, heat-exchanger, viscoelastic, acoustics, fw-h]
sources: [doc-reactor-and-acoustics, doc-acoustics]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the physics models of the early waves (variable density, compressible, heat, chemistry, MHD, viscoelasticity, acoustics)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda; wave records that each carry several F-items), 18 ADRs, from the pre-wave ADRs 0005 to 0013 to wave 40A.

Each ADR here adds one physics model as a vertical slice under the same house rules: checks and limits registered before any code ran, a miss kept as an ignored `*_known_miss` test with the limit intact, a set-up defect answered by a new corrected test, and defaults bit-identical. Nearly every item ends `partial` with named gaps, usually because the roadmap acceptance names a published experiment that the wave did not compare against. The models are: variable density in the pressure-based kernel (steady, real gas, transient), boundary conditions of the density-based Euler solver, lumped zone components and radiation, the 0D reactor and FW-H acoustics, conjugate heat transfer, finite-rate and turbulent combustion, interface physics, Joule heating and melting, tabulated properties and the actuator disk, MHD and heat-exchanger macro models, transient momentum sources, viscoelasticity, stiff-chemistry acceleration, and the fan-noise library. See [[known-miss]], [[registration]], [[bit-identical]].

## Current state

- **Variable density** lives in its own module `rcfd-fv::varden`, selected by `fluid.density = "ideal_gas"` (ADR 0005) or `"real_gas"` (Peng-Robinson or Redlich-Kwong, compressible mode only, ADR 0007), transient with `solver.algorithm = "pimple"` and Euler or variable-step BDF2 plus `energy.viscous_heating` (ADR 0009). "The constant-density code is not touched, so every existing case keeps its floating-point operation order; the price is about 300 duplicated lines of the SIMPLE iteration" (ADR 0005). See [[rcfd-fv]].
- **Density-based Euler boundaries** `TotalInlet`, `PressureOutlet`, `NonReflecting`, `FarField` by 1-D characteristic analysis; "Only normally incident waves are treated as non-reflecting; oblique incidence reflects"; the NACA 0012 transonic Cp check "is registered as not attempted", so F-062 stays partial (ADR 0006).
- **Heat:** lumped fan, porous jump, heat exchanger and network outlet zones plus grey Rosseland and P1 radiation (F-113, F-130; ADR 0012); solid cell zones, coupled interfaces and thermal wall conditions (F-051; ADR 0024), first with `solver.frozen_flow = true` only, then solved flow with solids for the coupled algorithm (ADR 0029, on [[adr-thread-early-numerics-transient-accuracy]]); interface layers, Joule heating and enthalpy-porosity melting (F-215, F-213, F-212; ADR 0035); effectiveness-NTU heat exchangers, a loss closure, a dual cell and a fan boundary (F-217; ADR 0039), with an exact series for the both-unmixed crossflow (ADR 0044).
- **Chemistry:** the 0D reactor toolkit in the new crate `rcfd-reactor` (F-112; ADR 0013, see [[rcfd-reactor]]); `finite_rate`, `eddy_dissipation`, `finite_rate_eddy_dissipation` and `edc` closures on constant-density species transport (F-054; ADR 0027), wired into `rcfd run` (ADR 0037); ISAT, DRG and clustering, and mixture-fraction, flamelet and progress-variable libraries (F-223, F-129; ADR 0053). "No agreement with a published flame is claimed anywhere in this wave" (ADR 0027).
- **Sources and MHD:** tabulated properties, an actuator-disk source and per-region turbulence (F-220, F-218, F-216; ADR 0036); a Lorentz-force source (F-214; ADR 0039); the transient PISO / PIMPLE momentum-source hook (ADR 0045), whose misses led to the transient-defect work of ADR 0046.
- **Viscoelasticity and structural export:** Oldroyd-B, Giesekus, linear PTT and FENE-P with log-conformation and `ViscoelasticSolver` wrapping `PimpleSolver` (F-135); `rcfd-io::registry`, a Nastran-style deck and one-way pressure and temperature transfer (F-237) (ADR 0051).
- **Acoustics:** FW-H in ADR 0013 ("All FW-H checks passed on the first run ... which also means the limits were loose"); the fan-noise chain in `rcfd-post` and `rcfd fwh` (ADR 0066, F-243 partial); an independent retarded-time check of a rotating point force sets F-136 done (ADR 0097). See [[fw-h]], [[aeroacoustics]].

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0005](../../docs/adr/0005-variable-density-pressure-based.md) | | design + registration | F-050, F-061 | Variable density in the pressure-based kernel (`varden`, ideal gas) | accepted; misses recorded in the roadmap | 0007, 0009 |
| [0006](../../docs/adr/0006-compressible-boundary-conditions.md) | | registration | F-062 | Characteristic boundary conditions for the density-based Euler solver | accepted; check (D) not attempted, F-062 partial | |
| [0007](../../docs/adr/0007-real-gas-eos.md) | | registration + results | F-061 | Cubic real-gas EOS in `varden` | R1 misses at three points (20 of 23 pass); R2, R3 miss; choked nozzle -2.8e-4 against 1.0 % | |
| [0009](../../docs/adr/0009-transient-variable-density.md) | | registration + results | F-050, F-061 | Transient variable density and viscous heating | steady bit-identical; T3c BDF2 order 1.34 against [1.80, 2.20]; T5 misses | 0010 (T3c met with the corrected diagonal) |
| [0012](../../docs/adr/0012-lumped-zones-radiation.md) | | registration + results | F-052, F-113, F-130, F-217 | Lumped zone components and Rosseland / P1 radiation | Z3 (+0.085 % against 1e-4) and Z4 (1.45e-6 against 1e-6) known misses; the rest met | |
| [0013](../../docs/adr/0013-reactor-and-fwh.md) | | registration + results | F-112, F-136 | 0D reactor toolkit and FW-H acoustics | GRI-Mech 3.0 NASA continuity known miss (5.1e-3 in h); every FW-H check met | 0097 (F-136) |
| [0024](../../docs/adr/0024-wave17b-cht.md) | | registration + results | F-051 | Solid cell zones, coupled interfaces, thermal walls | every check within its limit, two groups only on corrected tests; solved flow with solids not delivered | 0029 |
| [0027](../../docs/adr/0027-wave18b-combustion.md) | | registration + results | F-054, F-222, F-224, F-225 | Finite-rate and turbulent-chemistry closures | K8, K10 (ii) known misses; F-054 acceptance NOT met | 0037, 0053 |
| [0035](../../docs/adr/0035-wave21a-thermal-extras.md) | | registration + results | F-212, F-213, F-215 | Interface layers, Joule heating, enthalpy-porosity melting | F-213 met; F-215 G5 and F-212 P1, P2, P8, K2 missed | 0043 |
| [0036](../../docs/adr/0036-wave21b-tables-actuator.md) | | registration + results | F-216, F-218, F-220 | Tabulated properties, actuator disk, per-region turbulence | T9 vapour known miss (2.642e-4 against 1e-4); A4c miss led to `sources.<n>.distribution` | 0044 (F-220) |
| [0039](../../docs/adr/0039-wave23a-mhd-heat-exchanger.md) | | registration + results | F-214, F-217 | Lorentz-force MHD and heat-exchanger macro models | Hartmann and counterflow acceptances met; both-unmixed crossflow H2, H3 missed (3.3 %) | 0044, 0045 |
| [0043](../../docs/adr/0043-wave25a-temporal-accuracy.md) | | diagnosis + fixes | F-210, F-212, F-215 | Temporal and nonlinear accuracy misses | F-210 periodic defect fixed; F-212 safeguarded Newton; F-215 G5 a double-precision property; see also [[adr-thread-early-numerics-transient-accuracy]] | |
| [0044](../../docs/adr/0044-wave25b-closure-accuracy-references.md) | | repairs + audit | F-217, F-220, F-227 | Closure accuracy repairs and a literature audit | exact crossflow series 7.717e-6 against 1e-4; `eval_density_ideal_scaled` 1.239e-5; W8-4 known miss | |
| [0045](../../docs/adr/0045-wave25c-transient-momentum-sources.md) | | registration + results | F-214, F-217, F-218 | Momentum sources and lumped components in transient PISO / PIMPLE | T2, T4, T6, T7 met; T1, T3, T5 missed from two pre-existing causes | 0046 |
| [0051](../../docs/adr/0051-wave27c-viscoelastic-structural-export.md) | | registration + results | F-135, F-237 | Viscoelastic models with log-conformation; format registry and one-way structural load export | V1 and V3 known misses (the recalled Giesekus formula "is wrong"); P3 sign corrected | |
| [0053](../../docs/adr/0053-wave28b-chemistry-acceleration-combustion.md) | | registration + results | F-129, F-223 | ISAT, DRG, clustering; mixture fraction, flamelets, progress variable | F-223 point met against direct integration (1.7e-3, 1.6e-3); ISAT speed-up 0.23 (a slowdown); I5 profile 1222 K against 10 K | |
| [0066](../../docs/adr/0066-wave32b-aeroacoustics.md) | | registration + results | F-136, F-243 | Fan-noise library chain (FW-H, broadband proxies, Gutin tone, SPL bands) | every check met except W5a, W5b (impermeable sphere in flow); proxies not validated; F-243 partial | |
| [0097](../../docs/adr/0097-wave40a-rotating-force-direct-reference-chained-transient.md) | | partials close-out | F-136, F-241 | Independent rotating-force far field; chained against single transient | met with no amendment; F-136 done, F-241 done | |

## Key numbers and limits

- ADR 0007: N2 300 K at 5.1, 10.1, 15.1 MPa miss the 1 percent class; R2 enthalpy differences +18.8, +15.7, +12.3 percent; NIST values came "through a page fetcher with an LLM summariser and are marked 'retrieved, not independently re-read'". See [[unverified]].
- ADR 0009: T5 at 400 steps "misses the mass-flow limit (1.64e-3 / 1.62e-3 against 1e-3) and the in/out equality limit (1.56e-5 against 1e-6)"; "Compressible transient from rest is not robust for impulsive velocity mismatches at large `dt`".
- ADR 0012: Z1 "missed on the first implementation (fan as a balanced-force body force, porous jump as a cell-based sink; -17 % at u = 0.5 m/s, limit 1e-3). A pressure-jump plane then replaced that implementation; the criteria did not change."
- ADR 0024: "The registered steady stop criterion is unattainable, so the library harness stops at 1e-11; the accuracy limits are unchanged."
- ADR 0035: F-215 G5 at dt = 0.01 "per-side enthalpy drift 2.0e-12 and 1.8e-11 against 1e-12"; F-212 P1 orders "-0.55, 0.06, 0.98 against >= 0.9".
- ADR 0039: Hartmann "profile within 2 percent, flow-rate reduction within 1 percent" at three Hartmann numbers (1-D set-up); counterflow "within 2 percent of eps-NTU".
- ADR 0044: the W8-4 continuum estimate "+27.8 percent, limit 25 percent"; c4c "confirms the cal to J factor 4.184".
- ADR 0045: a T8 deviation "attributed by inference and not bisected to wave 24 commit a9045eb".
- ADR 0066: W5a and W5b "at 7.695e-2 and 6.673e-2 against limits of 1e-2 and 1e-3"; the A-weighting table "recalled from memory".
- ADR 0097: R1 "e = 1.87e-8 to 3.4e-8 against the limit 1e-3", T1 "identical bit for bit"; the Gutin (1936) and Lowson (1965) attributions "still UNVERIFIED".

## Open questions

- ADR 0012, 0013, 0027, 0041, 0051, 0053, 0066: references "from memory" or "UNVERIFIED" (Milne extrapolation length, R2 reference digits, the 2225 K of R7, Farassat / Brentner formulas, FENE-P normalisation, Nastran field syntax, the A-weighting table). ADR 0044 audited some of them; "many items still unverified".
- ADR 0027: "GRI-Mech data are never committed (licence unverified)".
- No ADR of this thread compares a published flame, gallium melt, shell-and-tube table or cavity Cp; those are the named gaps behind most `partial` statuses.

## Related

- [[rcfd-fv]], [[rcfd-reactor]], [[fw-h]], [[aeroacoustics]], [[known-miss]], [[unverified]]
- [[adr-thread-early-numerics-transient-accuracy]] (ADRs 0029, 0043, 0046), [[adr-thread-vof-multiphase-particles]]
- [[roadmap-heat-materials-and-chemistry]], [[roadmap-compressible-flow-and-acoustics]], [[roadmap-numerics-and-solvers]]
