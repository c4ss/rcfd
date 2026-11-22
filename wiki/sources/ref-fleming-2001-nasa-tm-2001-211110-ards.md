---
type: source
tags: [reference, foundation, structural, rotordynamics, nasa]
sources: []
origin: https://ntrs.nasa.gov/api/citations/20010092007/downloads/20010092007.pdf
licence: NASA technical memorandum (NASA Glenn Research Center); no licence statement on the pages read. Not copied into raw/.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fleming (2001), ARDS User Manual, NASA/TM-2001-211110

**Origin:** D. P. Fleming, *ARDS User Manual*, NASA/TM-2001-211110, NASA Glenn Research Center, 2001, NTRS 20010092007. ARDS (Analysis of Rotor Dynamic Systems) is a NASA-developed rotordynamics program; this is a government report, not commercial vendor documentation.
**Kind:** NASA technical memorandum (PDF), mostly a user manual. Locators are the section "Program description" and its numbered analysis options.
**Licence:** see frontmatter; cite only.
**Read:** 2026-11-22 by job p6-structural: introduction and "Program description" (analysis options 1-10). The input and output sections and appendices were skimmed only for terms.

## Summary

Describes a finite-element rotordynamics program for multi-shaft rotors on bearings: whirl frequencies and stability, damped and undamped critical speeds, unbalance and transient response (e.g. blade loss), sensitivity coefficients and critical-speed placement. The theory is in its references 7 to 10, not in this manual.

## Key points

- **Modelling (program description):** rotating assemblies are modelled as finite elements with sub-elements; rotors are on rigid foundations except in the component mode synthesis (CMS) option for steady unbalance response, where foundation flexibility may be included.
- **Option 1, damped whirl frequencies and mode shapes:** these are the natural frequencies of the shaft system in forward and backward whirl at given shaft speeds; "Forward and backward whirl frequencies differ because of gyroscopic effects"; the output includes the real part of the whirl eigenvalue as a measure of stability.
- **Option 2, critical speeds:** "A critical speed occurs when the speed of rotation equals a system natural frequency"; because the natural frequencies themselves depend on speed through gyroscopic effects, finding a critical speed is iterative (done with sensitivity coefficients).
- **Options 3, 6, 7:** steady unbalance response over a speed range; transient response (blade loss, step turns, base acceleration, nonlinear rubs and squeeze-film dampers) by CMS only; a quick estimate of peak blade-loss response by summing modal responses, an analogue of shock-spectrum analysis.
- **Options 8-9:** sensitivity coefficients (derivatives of whirl frequencies, critical speeds and unbalance response with respect to bearing coefficients, disc inertias, shaft mass and stiffness); design usually requires critical speeds outside the operating speed range and small bearing forces.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rotordynamics-gyroscopic-effects]]
