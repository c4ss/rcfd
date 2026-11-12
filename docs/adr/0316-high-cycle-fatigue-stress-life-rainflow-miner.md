# ADR 0316: High-cycle fatigue: stress-life curves, mean-stress corrections, harmonic and counted histories, linear damage

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-6 of the structural solver, advanced tier. Size M.

## Goal

Turn stresses into life. From a static (mean) stress field and a harmonic stress field (blade-passing response on the centrifugal
state) or a stress history: stress-life curves (Basquin, or a table in log-log, with an optional endurance limit), mean-stress
corrections (Goodman, Gerber, Soderberg), equivalent stresses for proportional loading (signed von Mises, largest principal; the
amplitude of a harmonic tensor over the phase), rainflow counting, the Palmgren-Miner sum, the narrow-band damage of a random
stress from its spectral moments, and fields of life, damage rate and safety factor.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-376, F-399.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Curves and corrections reproduce N =
(sigma_a/sigma_f)^(1/b)/2, sigma_a/(1 - sigma_m/sigma_u) (Goodman), sigma_a/(1 - (sigma_m/sigma_u)^2) (Gerber) and Soderberg
within 1e-12; a mean stress at or above the strength is refused. Harmonic amplitude: for a normal and a shear component 90 degrees
out of phase the von Mises amplitude equals the closed-form maximum over the phase within 1e-10. Cantilever with a static and a
harmonic tip force: the largest damage lies on the root surface and the life there equals the hand value from 6 P L/(b h^2) within
the registered factor that a 2 percent stress error allows. Plate with a small circular hole (radius/width 0.05) under remote
harmonic tension: stress amplitude at the hole edge within 3 percent of 3 sigma (Kirsch) and the life the Basquin life of that
stress. Counting: n cycles of constant amplitude give n cycles of that range; a large cycle carrying m small ripples gives 1 and
m; a periodic history started at its maximum closes with no residual; a history and its mirror image give the same counts; two
load blocks give n1/N1 + n2/N2 within 1e-12. Narrow band: the damage rate nu0 (sqrt(2) s)^m Gamma(1 + m/2)/C within 1e-12 as a
function, and the counted damage of a synthesised narrow-band history (registered seed, 1e5 cycles) within 5 percent of it.

## Reference to check against

Closed forms and definitions: Basquin (1910), Goodman, Gerber, Soderberg, Miner (1945); the hole in a plate (Kirsch, in Timoshenko
and Goodier); the damage of a Rayleigh distribution of peaks (Miles, 1954; one integral). Counting is defined in the registration
by the closing of hysteresis loops (Matsuishi and Endo, 1968; Downing and Socie, Int. J. Fatigue 4 (1982) 31; from memory) and
checked on constructed histories; the standard that prints a worked example (ASTM E1049) is paid and is not used.

## Files

`crates/rcfd-fem/src/fatigue.rs`, `crates/rcfd-fem/tests/sa_fatigue.rs`.

## What could make it miss

No stress-life curve of any polymer is in the repository, and such curves depend on temperature, frequency and fibre direction:
every life is as good as its input (decision A1). Non-proportional multiaxial loading (critical plane), strain-life and
notch-sensitivity factors are not included. The stress at a root fillet needs a converged mesh (ADR 0335 gives the estimate).
