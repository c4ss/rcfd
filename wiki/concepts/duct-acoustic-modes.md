---
type: concept
tags: [foundation, acoustics, duct-modes, fan-noise]
sources: [ref-hubbard-1991-nasa-rp-1258-vol2, ref-hubbard-1991-nasa-rp-1258-vol1]
builds_on: [acoustic-wave-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Duct acoustic modes

## In short

Inside a duct, sound can only travel in certain patterns (modes) that fit the walls. In a circular duct each mode is a Bessel-function shape across the radius times a pattern of $m$ lobes round the circumference that spins, carried along the axis as a wave. A mode propagates only above its cut-on frequency; below it, it decays exponentially and carries no power. Fan designers use this to keep tones inside a duct from escaping ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 105-109; [[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 3, p. 159).

## Explanation

**Set-up.** Take a uniform circular duct of radius $R$ with hard walls and uniform mean flow of Mach number $M$. The acoustic pressure obeys the convected wave equation $\left(\partial_t + M\partial_x\right)^2 p = \nabla^2 p$ (Eversman eq. 13), with no normal velocity at the wall, $\partial p/\partial r = 0$ at $r = 1$ (radius scaled by $R$) ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 105-106; see [[acoustic-wave-equation]]).

**Separation of variables.** For a harmonic source $p = P(x, r, \theta)\,e^{i\eta t}$ with $\eta = \omega R/c_r$, the convected Helmholtz equation (eq. 14) has travelling-wave solutions $P(r)\,e^{\pm i m\theta}\,e^{-i k_x x}$ whose radial part satisfies Bessel's equation; the solutions finite at the axis are $J_m(\kappa r)$, and the wall condition picks the eigenvalues ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 107):

$$J'_m(\kappa_{mn}) = 0$$ (Eversman, eq. 16)

$$p_{mn}(x, r, \theta, t) = P_{mn}\, J_m(\kappa_{mn} r)\, \exp\!\left[i(\eta t \pm m\theta - k_{x,mn} x)\right]$$ (Eversman, eq. 17)

$m$ is the circumferential order (number of lobes) and $n$ the radial order. At a fixed $x$ the pattern $e^{i(\eta t \pm m\theta)}$ is a spinning mode; at fixed $\theta$ it is an axial travelling wave.

**Axial wave number and cut-off.** ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 107-109)

$$\frac{k_{x,mn}}{\eta} = \frac{1}{1 - M^2}\left[-M \pm \sqrt{1 - (1 - M^2)\left(\frac{\kappa_{mn}}{\eta}\right)^2}\,\right]$$ (Eversman, eq. 15)

If $(1 - M^2)(\kappa_{mn}/\eta)^2 < 1$, $k_x$ is real and the mode propagates ("cut on"). If it is $> 1$, $k_x$ is complex: the wave is attenuated with distance and an energy argument shows "no acoustic power is associated with these modes" ("cutoff"). The cut-off ratio

$$\xi_{mn} = \frac{\eta}{\kappa_{mn}\sqrt{1 - M^2}}$$ (Eversman, p. 109)

exceeds one for cut-on modes. A physical picture: a mode is a pair of plane waves bouncing between the walls at an angle set by the cut-off ratio; at cut-off they travel straight across the duct (90 degrees) and carry nothing along it (p. 109). For a source at $x = 0$ in an infinite duct, only the waves that travel or decay away from it are kept on each side (p. 109).

**Modal content of a source.** For a force distribution on a source plane, the duct pressure is a sum over modes, each amplitude the product of the source's modal content $f_q$ and a duct transfer function $G_q$ ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 3, pp. 157-159, eq. 8):

$$P(x, \mathbf{r}) = \sum_q \Lambda_q S_0\, f_q\, G_q\, \psi_q(\mathbf{r})\, e^{i k_{xq}(x - x_0)}$$ (Groeneweg et al., eq. 8)

**Rotor-stator interaction (Tyler and Sofrin).** A rotor of $B$ blades interacting with $V$ vanes produces circumferential orders $m = sB - kV$ at the $s$th harmonic of blade passing ($s$, $k$ integers), with cut-off ratio $\xi_{mn} = \left|\frac{sB}{sB - kV}\right| \frac{M_t}{M^*_{mn}\beta}$, $M_t$ the blade-tip Mach number, $M^*_{mn} = \alpha_{mn}/m$, $\beta = \sqrt{1 - M_D^2}$ (eq. 9) ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 3, p. 159). Choosing blade and vane numbers so that the low orders are cut off is the classic way to keep the tone in the duct, "feasible for, at most, a very few circumferential modes" (p. 160); an isolated rotor can in principle make only cut-off modes, but struts and vanes add interaction tones, and a finite duct lets the reactive field radiate some power ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 109).

**Lined ducts.** With an acoustic lining the wall condition uses the wall impedance, $p/v = Z/(\rho_r c_r)$ (eq. 23), and the axial wave numbers become complex, giving attenuation ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 111-112).

**What it adds.** The [[acoustic-wave-equation]] in a bounded cross-section has only discrete solutions; this page gives them for a circular duct and the cut-off rule that decides which ones carry sound.

## Builds on

- [[acoustic-wave-equation]]: the convected wave and Helmholtz equations solved in the duct.

## Leads to

- [[aeroacoustics]], [[adr-thread-acoustic-wave-solver]]

## How rcfd uses it

- The modal decomposition and export of hard-walled duct fields is in `rcfd-post` (`duct_modes.rs`), with the linear acoustic wave solver (F-333, ADR 0351) ([[adr-thread-acoustic-wave-solver]]).
- Circular ducts on an O-grid (F-453, ADR 0366): spinning and radial modes with a matched layer; the counter-rotating `(-1, 0)` content missed ("1.2e-5 against 1e-6"), kept as a known miss ([[adr-thread-acoustic-wave-solver]]).
- The command `rcfd duct-modes` (F-451) is an approved follow-up ([[adr-thread-acoustic-wave-solver]]).
