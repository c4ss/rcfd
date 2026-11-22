---
type: concept
tags: [acoustics, fw-h, noise]
sources: [doc-acoustics, doc-reactor-and-acoustics, adr-thread-acoustic-wave-solver, ref-farassat-2007-nasa-tm-214853, ref-hubbard-1991-nasa-rp-1258-vol1]
builds_on: [lighthill-acoustic-analogy, greens-function-and-retarded-time]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# FW-H (Ffowcs Williams-Hawkings acoustic analogy)

An integral method that gives the far-field acoustic pressure from the flow on a surface: thickness and loading integrals evaluated at the retarded time, plus a volume (quadrupole) term. rcfd uses Formulation 1A (Brentner and Farassat) and computes no volume term ([[doc-acoustics]]).

## In short

The Ffowcs Williams-Hawkings equation extends Lighthill's acoustic analogy to surfaces moving through the fluid: a wave equation for the acoustic pressure with a thickness source and a loading source on the surface and Lighthill's quadrupole in the volume outside it. Formulations 1 and 1A of Farassat are its retarded-time integral solutions with surface sources only, for a surface moving at subsonic speed ([[ref-farassat-2007-nasa-tm-214853]] summary, sec. 2). All propeller noise methods in use "can be derived from the Ffowcs Williams and Hawkings equation" ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 11).

$$\Box^2 p' = \frac{\partial}{\partial t}\left[\rho_0 v_n \delta(f)\right] - \frac{\partial}{\partial x_i}\left[p\, n_i\, \delta(f)\right] + \frac{\partial^2}{\partial x_i \partial x_j}\left[H(f)\, T_{ij}\right]$$ (Farassat 2007, eq. 1)

with the surface $f = 0$, $\nabla f = \mathbf{n}$, $v_n$ its normal velocity, $H$ the Heaviside function and $T_{ij}$ the Lighthill stress tensor. For a permeable data surface the sources become $\rho_0 U_n$ and $L_i$ (Farassat eqs. 2-4); the retarded time and the Doppler factor $1 - M_r$ are explained on [[greens-function-and-retarded-time]]. (Section added by p6-compressible-acoustics, 2026-11-22.)

## Builds on

- [[lighthill-acoustic-analogy]]: the volume quadrupole term and the idea of an exact wave equation with flow sources.
- [[greens-function-and-retarded-time]]: the free-space Green's function, emission time and Doppler factor used to integrate it.
- [[acoustic-source-types]]: the thickness and loading terms are often called monopole and dipole, a terminology Farassat calls "misleading" for moving surfaces ([[ref-farassat-2007-nasa-tm-214853]] sec. 2).

## How rcfd uses it

- **Two entry points:**
  - `rcfd acoustics <surface.json>` (F-136, ADR 0013): permeable data surface in a medium at rest, input `rcfd-fwh-surface` v1 ([[doc-reactor-and-acoustics]]).
  - `rcfd fwh series.json` (wave 32B, ADR 0066): moving impermeable surfaces and uniform flow, input `rcfd-surface-series` v1, output signal CSV and spectra ([[doc-acoustics]]).
- **Impermeable case for fans:** `u = v`, `rho' = 0` on the blade, so `U_i = v_i`, `L_i = p' n_i`; `loading` mode sets `U = 0` ([[doc-acoustics]]).
- **Numerics:** retarded time by Newton per element and observer time; 4-point Lagrange interpolation; fourth-order time derivatives; a valid-window guard refuses out-of-record times and non-subsonic surfaces ([[doc-acoustics]]).
- **Uniform stream:** evaluated in the medium frame with a moving observer, "exact for 1A in a medium at rest"; requires `|U| < c0` ([[doc-acoustics]]).
- **Verification:** fourth-order convergence; pulsating sphere; compact dipole; W5d permeable monopole in flow 4.5e-6. W5a/W5b impermeable sphere in flow MISSED (7.7e-2, 6.7e-2 vs 1e-2, 1e-3), attributed to the test construction, kept as `*_known_miss` ([[doc-acoustics]]).
- **Gutin-type tones** are derived independently and match FW-H on a point-force rotor within 4e-5 ([[doc-acoustics]]).
- **Assumptions:** subsonic, no quadrupole ("small for subsonic tip Mach numbers, not zero"), free field; result only as good as the sampled surface pressure, which must resolve the blade-passing frequency ([[doc-acoustics]]).
- **Wave-solver drive:** the `rcfd-surface-series` files of the FW-H chain can drive the linear acoustic wave solver as a boundary (F-452), with the split of entering and returning waves from two or more sampled sections (F-478); an optional axial-velocity series in the format is approved as F-480 ([[adr-thread-acoustic-wave-solver]]).

## Related

- [[aeroacoustics]]
- [[rcfd-post]]
- [[known-miss]]

## Open questions

- No comparison with a measured fan was made ([[doc-acoustics]]).
