---
type: source
tags: [reference, foundation, acoustics, rotor-noise, lighthill, duct-modes, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19920001380
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API read 2026-11-22). Copyable; not copied to raw/ (59 MB scan; left for the lead).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hubbard (ed.) (1991), Aeroacoustics of Flight Vehicles: Theory and Practice, Vol. 1 Noise Sources, NASA RP-1258

**Origin:** H. H. Hubbard (ed.), *Aeroacoustics of Flight Vehicles: Theory and Practice. Volume 1: Noise Sources*, NASA Reference Publication 1258, Vol. 1, August 1991. NTRS 19920001380. PDF: https://ntrs.nasa.gov/api/citations/19920001380/downloads/19920001380.pdf
**Kind:** edited NASA reference book; one chapter per noise source, each by different authors. Locators: chapter, book page, equation.
**Licence:** US Government work, public use permitted (NTRS). Copyable, but the 59 MB scan was not copied into `raw/`.
**Read:** 2026-11-22 by job p6-compressible-acoustics, from the text layer plus page images for equations: ch. 1 (Magliozzi, Hanson, Amiet, "Propeller and Propfan Noise") pp. 1-16, the spinning-mode paragraph on p. 25 and the broadband-prediction opening on p. 28; ch. 2 (Schmitz, "Rotor Noise") pp. 77-79; ch. 3 (Groeneweg, Sofrin, Rice, Gliebe, "Turbomachinery Noise") pp. 157-160; ch. 4 (Lilley, "Jet Noise Classical Theory and Experiments") pp. 211-219. Other chapters (jet noise ch. 5-6, airframe noise ch. 7 and later) not read.

## Summary

The standard NASA survey of aircraft noise sources. The parts read cover the classification of rotor noise (harmonic, broadband, narrow-band random), its mechanisms (thickness, loading, quadrupole; steady, unsteady, random sources), the linear FW-H equation and Gutin's tone formula, duct modes and cut-off in turbomachinery, and Lighthill's acoustic analogy with its eighth-power law.

## Key points by chapter (own summary)

### Ch. 1, Propeller and Propfan Noise (Magliozzi, Hanson, Amiet)
- **Noise characteristics (p. 4, figs. 4-6):** three categories: harmonic, broadband and narrow-band random. Harmonic noise of an ideal propeller with `B` blades at rotational speed `N` has fundamental frequency `B N`; "The blade-passage period is 1/BN"; the pulse is not a pure sinusoid, so harmonics occur at integer multiples of `BN`. Broadband noise is random with a continuous spectrum. Narrow-band random noise is almost periodic, with energy spread around the harmonics.
- **Steady sources (pp. 5-6):** "linear thickness, linear loading, and (nonlinear) quadrupole". Thickness noise comes from the periodic displacement of air by the blade volume, can be represented by a monopole distribution and "becomes important at high speeds"; loading noise (thrust and torque, or lift and drag) is "an important mechanism at low to moderate speeds"; quadrupole sources in the volume around the blades matter at transonic section speeds.
- **Unsteady sources (pp. 7-10):** periodic loading changes (shaft angle of attack, inflow distortion, wakes of a front rotor in a counter-rotating pair) radiate at harmonics of blade-passage frequency; their circumferential directivity has lobes set by the distortion order, not the blade count.
- **Random sources (p. 10):** broadband noise comes from inflow turbulence hitting the leading edges and from the turbulent boundary layer at the trailing edge (and tip-vortex turbulence at the tip); for full-scale propellers in flight it is "relatively unimportant".
- **Broadband prediction (p. 28):** following Curle, surface noise comes from forces (dipoles) and sources (monopoles) on the surface; random dipoles give broadband noise; "Usually there are no monopoles with a random time behavior"; turbulence away from an edge "produces quadrupole sound, which is generally a weak generation mechanism compared with dipole sound", while eddies passing the trailing edge change the airfoil loading and radiate.
- **Linear FW-H (p. 11, Eqs. 1-2):** all propeller noise methods in use "can be derived from the Ffowcs Williams and Hawkings equation", a rigorous combination of momentum, continuity and state into a wave equation; the first source term (blades parting the air) gives thickness noise, the second (blade forces) loading noise. Simplified (Eq. 2): a wave equation with a volume distribution of sources `q` (thickness) and doublets of the blade force `F` (loading), equivalent to the source/vortex picture of wing aerodynamics (fig. 8).
- **Time and frequency domain (pp. 12-15):** time-domain methods (Farassat's Formulations 1, 1A, 3) evaluate integrands at the retarded (emission) time; frequency-domain methods give harmonics at `m B` as Fourier sums with Bessel functions "which are indicators of radiation efficiency" (Eqs. 4-6).
- **Spinning modes (p. 25, Eq. 36):** each loading harmonic radiates at all sound harmonics `mB`; circumferential modes of order `n` spin about the axis at `mB/n` times the shaft speed; radiation efficiency is set by the ratio of Bessel argument to order, similar to the turbofan cut-off ratio; the spinning-mode tip Mach number is `M_s = (mB/n) M_t` (Eq. 36).

### Ch. 2, Rotor Noise (Schmitz)
- **Gutin (pp. 78-79, Eqs. 5a-5b):** with `B` equally spaced blades the fundamental becomes `B` times the rotation rate. Gutin's far-field harmonic `p'_mB = (m B Omega/(2 pi c_0 R_0)) integral_0^R (-dT/dr cos(sigma) + (c_0/(Omega r)) dD/dr) J_mB((m B Omega/c_0) r sin(sigma)) dr` (Eq. 5a), with `J_n` the Bessel function of the first kind, `m` the harmonic number, `Omega` the rotation rate, `c_0` the speed of sound, `R_0` the hub-observer distance, `dT/dr`, `dD/dr` the radial thrust and drag distributions and `sigma` the observer elevation from the rotor plane. Concentrating the loads at an effective radius `r_e` gives the rms harmonic `p'_mB = (m B Omega/(2 sqrt(2) pi c_0 R_0)) (-T_e cos(sigma) + c_0 D_e/(Omega r_e)) J_mB((m B Omega/c_0) r_e sin(sigma))` (Eq. 5b); "for low orders of harmonics, choosing r_e ~ 0.8R yielded good results". As long as the tip Mach number of the rotating source stays `<= 0.7` "no sharp impulses are expected" (p. 78).

### Ch. 3, Turbomachinery Noise (Groeneweg, Sofrin, Rice, Gliebe)
- **Coupling to the duct (pp. 157-159, Eqs. 4-8):** for harmonic time dependence `p = Re[P exp(-i omega t)]`, the pressure from a force distribution `f(r_0)` on a source plane is `P = integral G f dS_0` (Eq. 5); expanding both in the duct's orthogonal modes `psi_q` gives `P(x,r) = sum_q Lambda_q S_0 f_q G_q psi_q(r) exp{i k_xq (x - x_0)}` (Eq. 8): each mode's amplitude is the product of the forcing's modal content `f_q` and a duct transfer function `G_q`.
- **Cut-off (p. 159, Eq. 9):** above cut-off the axial wave number is real and the mode propagates; below, it is imaginary and the mode decays exponentially. Cut-off ratio `xi = k_0 r_0/(alpha_mn beta)` with `alpha_mn` the duct eigenvalue and `beta = (1 - M_D^2)^(1/2)`; `xi > 1` propagates. For rotor-stator interaction `xi_mn = |sB/(sB - kV)| M_t/(M*_mn beta)` with `m = sB - kV` (`B` blades, `V` vanes, `s`, `k` integers) (Eq. 9). Design strategy: cut off the low-order modes by blade and vane numbers, then shape the radial distribution of the forcing (p. 160).

### Ch. 4, Jet Noise Classical Theory (Lilley)
- **Pressure equation (p. 214, Eqs. 6-10):** perfect gas `p = rho R T` (Eq. 8), `c = sqrt(gamma R T)`; neglecting squares and products of perturbations gives the linearised equation for the pressure perturbation, a wave equation `d^2p/dt^2 - c_0^2 nabla^2 p = ...` with mass and force sources (Eq. 9).
- **Lighthill's equation (p. 215, Eqs. 11-14):** subtracting the divergence of the momentum equation from the time derivative of continuity gives `d^2 rho/dt^2 - c_inf^2 nabla^2 rho = A(x,t)` (Eq. 11), `A = d^2 T_ij/dx_i dx_j` (Eq. 12), `T_ij = rho v_i v_j + (p - rho c_inf^2) delta_ij - tau_ij` (Eq. 13). Source terms with `d/dt`, `d/dx_i`, `d^2/dx_i dx_j`, `d^3/...` are "labeled, respectively, monopole, dipole, quadrupole, and octopole"; Lighthill's source is a quadrupole. "Lighthill's equation is exact"; for unbounded flow `rho(x,t) = (1/(4 pi c_inf^2)) integral A(y,tau)/|x - y| dy` (Eq. 14), `tau = t - |x-y|/c_inf` the retarded time. The sources may move but the medium is at rest with constant `rho_inf`, `c_inf`.
- **Limits (pp. 211-213):** the theory is predictive only when the source field is known; it is hard to apply when the sound interacts with the flow (refraction, convection), which Goldstein treats; Curle extended it to solid surfaces.
- **Far field and scaling (pp. 216-218):** only the component of `T_ij` along the source-observer direction radiates, with amplitude proportional to its second time derivative at emission; the acoustic power is a small fraction of the jet's kinetic-energy flux; "the sound power per unit volume of the flow is proportional to the eighth power of the flow velocity"; the viscous part of `T_ij` is negligible at high Reynolds number.
- **Heated jets (p. 219):** temperature fluctuations add a dipole term, giving intensity `~ M^6` at low Mach number instead of `M^8` for the isothermal jet.

## Contradictions with the wiki

None found.

## Pages this source informs

[[rotor-tone-noise]], [[acoustic-source-types]], [[lighthill-acoustic-analogy]], [[duct-acoustic-modes]], [[acoustic-wave-equation]], [[fw-h]]
