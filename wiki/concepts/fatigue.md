---
type: concept
tags: [foundation, structural, fatigue]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, ref-nasa-sp-8050-structural-vibration-prediction, adr-thread-structural-fatigue-and-radiated-sound]
builds_on: [stress]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fatigue (S-N curves, mean stress, cumulative damage)

## In short

A part can fail under a cyclic stress far below its static strength: tiny cracks start and grow a little each cycle. Fatigue design compares the stress amplitude with a stress-life (S-N) curve measured at constant amplitude, corrects for mean stress, and sums the damage of a mixed load history with Miner's rule ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Fatigue").

## Explanation

**S-N curve.** Stress amplitude against the logarithm of cycles to failure at constant amplitude. Ferrous alloys show an endurance limit below which failure does not occur; aluminium alloys do not ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Load ratio and mean stress.** A cycle is described by its alternating and mean stress, or by $R = \sigma_{min}/\sigma_{max}$; $R = -1$ is fully reversed. A tensile mean lowers the allowable amplitude. The Goodman diagram is a straight line in the mean-alternating plane from the endurance limit (zero mean) to the ultimate tensile strength (zero amplitude); points below it are safe ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Cumulative damage.** For a history with $n_i$ cycles at levels whose lives are $N_i$, Miner's law predicts failure at $$\sum_i \frac{n_i}{N_i} = 1$$ Roylance advises viewing it "skeptically": load order matters and the rule ignores it ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Crack growth.** Once a crack exists, its growth per cycle follows the Paris law $da/dN = A\,\Delta K^m$ in the stress-intensity range $\Delta K$ ([[ref-roylance-mit-3-11-mechanics-of-materials]]; [[fracture-mechanics]]).

**Random loading.** For random vibration, fatigue-related quantities (number of response cycles, probability of peak levels) can be predicted from the response spectral density ([[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.2; [[harmonic-and-random-response]]).

**Not covered by the sources read.** The Basquin power-law fit, the Gerber and Soderberg corrections, rainflow counting and spectral damage methods (narrow-band, Dirlik) were not described in the sources read.

## Builds on

- [[stress]]: stress components and their cyclic amplitude and mean; equivalent stresses such as von Mises.

## Leads to

- [[fracture-mechanics]] (crack growth), [[harmonic-and-random-response]]

## How rcfd uses it

- High-cycle fatigue: Basquin, power and log-log table S-N curves with an endurance limit, Goodman, Gerber and Soderberg corrections, harmonic amplitude, rainflow, linear and narrow-band damage (F-413, ADR 0449, done) ([[adr-thread-structural-fatigue-and-radiated-sound]]).
- Dirlik damage rate from the spectral moments `m0, m1, m2, m4` (F-566, ADR 0497, partial) ([[adr-thread-structural-fatigue-and-radiated-sound]]).
- A `[fatigue]` case-file key (F-605) approved 2026-11-18, still todo at this snapshot ([[adr-thread-structural-fatigue-and-radiated-sound]]).
