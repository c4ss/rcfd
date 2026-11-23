---
type: source
tags: [reference, foundation, structural, dynamics, random-vibration, nasa]
sources: []
origin: https://ntrs.nasa.gov/api/citations/19710009806/downloads/19710009806.pdf
licence: NASA special publication (space vehicle design criteria monograph); no licence statement on the pages read. Not copied into raw/.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NASA SP-8050 (1970), Structural Vibration Prediction (NASA Space Vehicle Design Criteria, Structures)

**Origin:** NASA SP-8050, *Structural Vibration Prediction*, NASA Space Vehicle Design Criteria (Structures), 1970, NTRS 19710009806.
**Kind:** NASA design-criteria monograph (PDF). Locators are section numbers.
**Licence:** see frontmatter; cite only.
**Read:** 2026-11-22 by job p6-structural: table of contents, sec. 2.1.2.1 (end, substructure and finite-element remarks) and sec. 2.1.2.2 "Response Formulation and Solution". Other sections were not read.

## Summary

A state-of-the-art review (as of 1970) of predicting the vibration of launch-vehicle structures: extrapolation from measured data, deterministic analysis of mathematical models, statistical energy analysis, and tests.

## Key points

- **Sec. 2.1.2.1:** substructure methods suit large structures made of dissimilar parts (multistage vehicles) and extract more accurate high-frequency modes than a single model of the same size; the finite-element technique is "the most generally applicable of the available analytical methods", its drawback being large matrices and computer time.
- **Sec. 2.1.2.2, equations of motion:** linear vibration leads to second-order linear equations whose stiffness, damping and mass matrices multiply displacement, velocity and acceleration; when sufficiently nonlinear, practice is to use a linear model with equivalent stiffness and damping.
- **Sec. 2.1.2.2, solution:** the equations are commonly uncoupled by a transformation built from the natural modes of the undamped structure, then solved by the mode-displacement method, the mode-acceleration method, or mechanical admittance / impedance methods; the response at each frequency is summed over the orthogonal modes.
- **Sec. 2.1.2.2, random loads:** when the applied loading is random, the response is random too and is expressed against frequency as a spectral density; acceleration spectral density follows from the displacement spectral density; internal loads and stresses follow from root-mean-square displacements; fatigue-related data (number of response cycles, probability of peak levels) can also be predicted.
- **Sec. 2.1.2.2, damping:** finding the right amount and type of damping per mode is "a major problem"; viscous damping is assumed because it is compatible with linear analysis, although real aerospace damping is material, friction (dominant in bolted and riveted structures) and acoustic radiation; prediction relies heavily on tests of similar structures.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[harmonic-and-random-response]], [[structural-dynamics]], [[substructuring-and-craig-bampton]]
