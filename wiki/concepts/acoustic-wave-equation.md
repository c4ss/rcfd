---
type: concept
tags: [foundation, acoustics, wave-equation]
sources: [ref-hubbard-1991-nasa-rp-1258-vol2, ref-hubbard-1991-nasa-rp-1258-vol1, ref-hu-1995-icase-95-70, ref-farassat-2007-nasa-tm-214853]
builds_on: [euler-equations, speed-of-sound-and-mach-number]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Acoustic wave equation

## In short

Sound is a very small disturbance of pressure, density and velocity on top of a steady state. Keeping only first-order terms in the disturbances turns the [[euler-equations]] into the linearised Euler (acoustic) equations, and for a medium at rest these combine into the wave equation: pressure disturbances travel in every direction at the speed of sound $c_0$ ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 103-105).

## Explanation

**Linearisation.** Write each quantity as mean plus perturbation, $\rho^* = \rho_0 + \rho$, $p^* = p_0 + p$, $\mathbf{V}^* = \mathbf{V}_0 + \mathbf{V}$, insert into the inviscid equations and drop second-order terms. This gives the acoustic continuity, momentum and energy equations (Eversman eqs. 5-7) and the acoustic equation of state ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 104):

$$p = \gamma \frac{p_0}{\rho_0}\,\rho = c_0^2\, \rho$$ (Eversman, eq. 8)

so a pressure perturbation and a density perturbation are proportional, with $c_0$ the local speed of sound of the mean flow ([[speed-of-sound-and-mach-number]]).

**Linearised Euler equations with a uniform flow.** For a uniform mean flow of Mach number $M$ along $x$, nondimensionalised by the mean density and $c_0$, Hu writes ([[ref-hu-1995-icase-95-70]] sec. 2):

$$\frac{\partial u}{\partial t} + M\frac{\partial u}{\partial x} + \frac{\partial p}{\partial x} = 0, \qquad \frac{\partial v}{\partial t} + M\frac{\partial v}{\partial x} + \frac{\partial p}{\partial y} = 0, \qquad \frac{\partial p}{\partial t} + M\frac{\partial p}{\partial x} + \frac{\partial u}{\partial x} + \frac{\partial v}{\partial y} = 0$$ (Hu 1995, eqs. 1.1-1.3)

plus an equation for the density perturbation (eq. 1.4). They carry acoustic waves at the speed of sound relative to the flow and vorticity and entropy waves that move with it (sec. 1).

**The wave equation.** In a uniform duct with uniform mean Mach number $M$ the acoustic equations combine into the convected wave equation (nondimensional) ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 105):

$$\left(\frac{\partial}{\partial t} + M\frac{\partial}{\partial x}\right)^2 p = \nabla^2 p$$ (Eversman, eq. 13)

which "simplifies to the classic wave equation in the absence of mean flow". In dimensional form for a medium at rest, $\frac{\partial^2 p}{\partial t^2} - c_0^2\nabla^2 p = 0$; with sources on the right it is the starting point of aeroacoustics: Lilley's linearised pressure equation has mass and force sources (eq. 9) ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 214), and the FW-H equation writes the wave operator $\Box^2 p'$ on the left ([[ref-farassat-2007-nasa-tm-214853]] eq. 1).

**Harmonic waves.** With time dependence $e^{i\omega t}$ the wave equation becomes a Helmholtz equation for the amplitude; Eversman writes $p = P(x, r, \theta)\,e^{i\eta t}$ with $\eta = \omega R/c_r$ and obtains the convected Helmholtz equation of a duct (eq. 14) ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 106-107). See [[duct-acoustic-modes]].

**Spreading.** Far from a compact source the wave fronts spread spherically; the level of a point source drops 6 dB per doubling of distance, for every frequency; a line of incoherent sources (cylindrical spreading) drops 3 dB per doubling ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 12, p. 55). In the free-space solution this is the $1/r$ of the Green's function ([[greens-function-and-retarded-time]]).

**Impedance.** At a wall lined with a locally reacting material the ratio of acoustic pressure to the normal particle velocity is the wall's impedance; Eversman writes it nondimensionally as $p/v = Z/(\rho_r c_r)$, the specific impedance (eq. 23) ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 112). A hard wall is the limit of zero normal velocity, $\partial p/\partial r = 0$ (p. 106). Impedance is the language of absorbing and non-reflecting boundaries ([[non-reflecting-boundaries-and-sponge-layers]]).

**What it adds.** The [[euler-equations]] are nonlinear and carry shocks; for sound amplitudes the nonlinear terms are negligible, and the result is a linear equation whose solutions superpose. [[speed-of-sound-and-mach-number]] supplies $c_0$, the only speed in it.

## Builds on

- [[euler-equations]]: the inviscid conservation laws that are linearised.
- [[speed-of-sound-and-mach-number]]: $c_0^2 = \gamma p_0/\rho_0$ links pressure and density perturbations.

## Leads to

- [[greens-function-and-retarded-time]], [[lighthill-acoustic-analogy]], [[duct-acoustic-modes]], [[non-reflecting-boundaries-and-sponge-layers]], `structural-sound-radiation`
- [[adr-thread-acoustic-wave-solver]], [[aeroacoustics]]

## How rcfd uses it

- `AcousticWaveSolver` (F-333, ADR 0351) solves "the linear acoustic equations about a uniform medium at rest, first order in `p` and `u`", the small-amplitude limit of the Euler equations of `CompressibleSolver`, on the polyhedral finite-volume mesh with an exact linear Riemann flux ([[adr-thread-acoustic-wave-solver]]).
- The wave solver is library only and "not coupled to a CFD run"; it can be driven from a sampled CFD surface (F-452, F-478) ([[aeroacoustics]]).
- The impedance boundary `CompBc::Impedance { p, z }` of the compressible solver (F-243, ADR 0233) uses the same impedance idea; `z == 0.0` returns `pressure_outlet` bit for bit ([[adr-thread-compressible-acoustic-boundaries]]).
