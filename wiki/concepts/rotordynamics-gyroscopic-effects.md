---
type: concept
tags: [foundation, structural, rotordynamics]
sources: [ref-wikipedia-rotordynamics, ref-fleming-2001-nasa-tm-2001-211110-ards, ref-cline-variational-mechanics, adr-thread-structural-rotordynamics, doc-reference-data-f415-coriolis-campbell]
builds_on: [rotating-reference-frame, modal-analysis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rotordynamics and gyroscopic effects (whirl, critical speeds)

## In short

A spinning shaft or disc that vibrates sideways traces an orbit, called whirl. Spin couples the two lateral directions through gyroscopic (Coriolis) forces, so each natural frequency splits into a forward-whirl branch (orbit in the spin direction) and a backward-whirl branch, which separate as speed rises. A critical speed is a speed at which the rotation rate equals a natural frequency, where unbalance can drive a large response ([[ref-wikipedia-rotordynamics]]; [[ref-fleming-2001-nasa-tm-2001-211110-ards]]).

## Explanation

**Equation of motion.** For an axially symmetric rotor at constant spin speed $\Omega$, $$\mathbf M\ddot{\mathbf q} + (\mathbf C + \mathbf G)\,\dot{\mathbf q} + (\mathbf K + \mathbf N)\,\mathbf q = \mathbf f(t)$$ with symmetric mass, damping and bearing stiffness matrices, a skew-symmetric gyroscopic matrix $\mathbf G$ proportional to $\Omega$, and $\mathbf N$ for deflection-dependent (e.g. centrifugal) effects; the eigenvectors are complex and depend on speed, and positive cross-coupled stiffness can destabilise the rotor ([[ref-wikipedia-rotordynamics]], "Basic principles"). The velocity-proportional, skew coupling has the same structure as the Coriolis force $-2m\,\boldsymbol\omega\times\mathbf v_{rot}$ seen in a rotating frame, which is perpendicular to the velocity and does no work ([[ref-cline-variational-mechanics]], sec. 12.8, eq. 12.41; [[rotating-reference-frame]]).

**Whirl frequencies.** Solving the eigenproblem at each spin speed gives the damped whirl frequencies and mode shapes, "in forward and backward whirl"; "Forward and backward whirl frequencies differ because of gyroscopic effects", and the real part of each eigenvalue indicates stability ([[ref-fleming-2001-nasa-tm-2001-211110-ards]], option 1).

**Critical speeds.** "A critical speed occurs when the speed of rotation equals a system natural frequency"; since the natural frequencies themselves depend on speed, finding one is iterative ([[ref-fleming-2001-nasa-tm-2001-211110-ards]], option 2). On a Campbell diagram they are the crossings of the whirl curves with the synchronous line ([[ref-wikipedia-rotordynamics]], "Campbell diagram"; [[campbell-diagram]]). Design usually keeps critical speeds outside the operating speed range ([[ref-fleming-2001-nasa-tm-2001-211110-ards]], option 9).

**Jeffcott rotor.** The simplest model: a massless flexible shaft on two bearings with a rigid disc at mid-span, lateral motion only; with eccentricity $\varepsilon$, $m\ddot r + kr = m\varepsilon\Omega^2\sin\Omega t$, an unbalance force growing with $\Omega^2$ ([[ref-wikipedia-rotordynamics]], "Jeffcott rotor").

**Unbalance and transients.** Rotordynamic programs compute steady unbalance response over a speed range and transients such as blade loss ([[ref-fleming-2001-nasa-tm-2001-211110-ards]], options 3, 6, 7).

**Depth note.** The sources read state the gyroscopic matrix and its properties but do not derive it from the element kinetic energy; that derivation is not on this page.

## Builds on

- [[rotating-reference-frame]]: Coriolis and centrifugal terms in a rotating frame.
- [[modal-analysis]]: natural frequencies and modes, here speed-dependent and complex.

## Leads to

- [[campbell-diagram]], [[cyclic-symmetry]] (rotating sectors), [[linear-buckling-and-stress-stiffening]] (centrifugal stress stiffening)

## How rcfd uses it

- Free vibration in the rotating frame `M u_tt + 2 Omega C u_t + K_eff u = 0` with `C` skew; the gyroscopic matrix and forward / backward whirl in the Campbell table (F-499, ADR 0411) ([[doc-reference-data-f415-coriolis-campbell]]; [[campbell-diagram]]).
- Whirl above the loss of stiffness by a state-matrix eigensolver (F-505), whirl stability per cyclic index (F-511), shift-invert skew Lanczos for large parts (F-506) ([[adr-thread-structural-rotordynamics]]).
- A flexible rotor of Beam2 shafts on bearings in the stationary frame (F-416), speed-dependent bearing tables (F-563), damped critical speeds as unbalance-response peaks and checked Campbell crossings (F-564, ADR 0501, done) ([[adr-thread-structural-rotordynamics]]).
