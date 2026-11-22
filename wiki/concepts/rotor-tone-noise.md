---
type: concept
tags: [foundation, acoustics, rotor-noise, fan-noise, blade-passing-frequency]
sources: [ref-hubbard-1991-nasa-rp-1258-vol1]
builds_on: [acoustic-source-types, fourier-analysis-and-spectra]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rotor tone noise (blade-passing frequency)

## In short

A rotor with $B$ blades turning at $N$ revolutions per second sends a pressure pulse past a listener $B N$ times a second. The sound is periodic, so its spectrum is a set of tones: the blade-passing frequency $BN$ and its harmonics. Steady blade loads and blade thickness give these tones; unsteady loads add more; random loads give broadband noise between them ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, pp. 4-10).

## Explanation

**Harmonic, broadband, narrow-band.** Propeller noise falls into three categories ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 4, figs. 4-6):

- *Harmonic* noise is periodic. For an ideal rotor with $B$ blades at rotational speed $N$ the fundamental is $f_1 = BN$, "The blade-passage period is $1/BN$"; because the pulse is not a pure sinusoid, harmonics appear at integer multiples $mBN$ ([[fourier-analysis-and-spectra]]).
- *Broadband* noise is random, with a continuous spectrum.
- *Narrow-band random* noise is almost periodic, its energy spread around the harmonics (for example when the rotor ingests a slowly wandering vortex, p. 10).

**Steady sources.** Loads that are constant in the blade frame still radiate because they rotate. They split into linear thickness (monopole-like, important at high speed), linear loading (thrust and torque, important at low to moderate speed) and nonlinear quadrupole (transonic sections) ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, pp. 5-6; see [[acoustic-source-types]]).

**Gutin's formula.** Treating thrust and drag as forces on a radial line of rotating points, Gutin found the far-field pressure of the $m$th harmonic of blade passing ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 2, p. 78):

$$p'_{mB} = \frac{mB\Omega}{2\pi c_0 \mathfrak{R}_0} \int_0^R \left(-\frac{dT}{dr}\cos\sigma + \frac{c_0}{\Omega r}\frac{dD}{dr}\right) J_{mB}\!\left(\frac{mB\Omega}{c_0}\, r \sin\sigma\right) dr$$ (Schmitz, eq. 5a)

with $\Omega$ the rotation rate, $c_0$ the speed of sound, $\mathfrak{R}_0$ the distance from hub to observer, $dT/dr$ and $dD/dr$ the radial distributions of thrust and drag, $\sigma$ the observer's elevation from the rotor plane and $J_n$ the Bessel function of the first kind. With the loads lumped at an effective radius $r_e$ the rms harmonic is (p. 79)

$$p'_{mB} = \frac{mB\Omega}{2\sqrt{2}\,\pi c_0 \mathfrak{R}_0}\left(-T_e\cos\sigma + \frac{c_0 D_e}{\Omega r_e}\right) J_{mB}\!\left(\frac{mB\Omega}{c_0}\, r_e\sin\sigma\right)$$ (Schmitz, eq. 5b)

and "for low orders of harmonics, choosing $r_e \approx 0.8R$ yielded good results". The Bessel function sets how efficiently each harmonic radiates: radiation is inefficient when the ratio of its argument to its order is below one and efficient above ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 25, fig. 14), so high harmonics of a slow rotor are weak. "As long as the tip Mach number of the rotating source remains $\le 0.7$, no sharp impulses are expected", and the harmonics fall off fast; near tip Mach 1 they roll off slowly (pp. 78-79).

**Unsteady loading and spinning modes.** Periodic load changes on the blades (inflow distortion, angle of attack, wakes of an upstream blade row) also radiate at harmonics of blade passing; their circumferential directivity has lobes set by the distortion, not the blade count ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, pp. 7-8). Each loading harmonic radiates at all sound harmonics $mB$; the circumferential pattern of order $n$ spins at $mB/n$ times shaft speed, and its radiation efficiency is governed by the spinning-mode tip Mach number $M_s = (mB/n) M_t$ (p. 25, eq. 36), the free-field analogue of duct cut-off ([[duct-acoustic-modes]]).

**Broadband sources.** Random forces on the blades give broadband noise: inflow turbulence hitting the leading edges, and the turbulent boundary layer passing the trailing edge (and tip-vortex turbulence at the tip) ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, pp. 10, 28). For full-scale propellers in flight broadband noise was found relatively unimportant (p. 11).

**What it adds.** [[acoustic-source-types]] names the sources; [[fourier-analysis-and-spectra]] turns a periodic signal into harmonics. A rotor joins them: rotating forces and volumes make a periodic signal whose harmonics sit at multiples of $BN$.

## Builds on

- [[acoustic-source-types]]: thickness (monopole-like) and loading (dipole-like) sources of a blade.
- [[fourier-analysis-and-spectra]]: periodic signals and their harmonic spectra.

## Leads to

- [[duct-acoustic-modes]] (rotor-stator interaction modes), [[sound-levels-and-metrics]]
- [[aeroacoustics]], [[fw-h]]

## How rcfd uses it

- rcfd derives Gutin-type tones independently from the dipole far field and checks them against FW-H on a point-force rotor (errors `<= 4e-5`); "The original Gutin (1936) formula was not read" ([[fw-h]], [[doc-acoustics]]).
- The FW-H result is only as good as the sampled surface pressure, "which must resolve the blade-passing frequency"; "A steady RANS run gives no tonal pressure history" ([[fw-h]], [[aeroacoustics]]).
- The broadband proxies (Proudman, Curle, trailing edge) are "NOT validated"; "Until then these numbers are not predictions" ([[doc-acoustics]]).
