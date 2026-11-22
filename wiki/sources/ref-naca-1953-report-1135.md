---
type: source
tags: [reference, foundation, compressible, gas-dynamics, shock, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19930091059
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED ("Work of the US Gov. Public Use Permitted.", record API read 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Ames Research Staff (1953), Equations, Tables, and Charts for Compressible Flow, NACA Report 1135

**Origin:** Ames Research Staff, "Equations, Tables, and Charts for Compressible Flow", NACA Report 1135, 1953 (report pages 611-681). NTRS 19930091059. PDF: https://ntrs.nasa.gov/api/citations/19930091059/downloads/19930091059.pdf
**Kind:** NACA report (scanned; equations read from page images).
**Licence:** US Government work, public use permitted; full copy in [raw/naca-1953-report-1135.pdf](../raw/naca-1953-report-1135.pdf).
**Read:** 2026-11-22 by job p6-compressible-acoustics: summary, symbols and notation (pp. 613-614), "Fundamental relations" (thermodynamics, pp. 614-615), "Continuous one-dimensional flow" (pp. 615-618), "Normal shock waves" (pp. 618-619), "Prandtl-Meyer expansion" (p. 626). Tables, charts, oblique shocks, cones and imperfect-gas sections were not read. Locators are report page and equation numbers.

## Summary

A compilation of the working equations of inviscid compressible flow of air: perfect-gas thermodynamics, isentropic one-dimensional flow as functions of Mach number, normal and oblique shock waves, Prandtl-Meyer expansions, flow past wedges and cones, and imperfect-gas corrections, with tables and charts for `gamma = 7/5`.

## Key points (own summary)

- **Notation (p. 614):** equations are tagged `[perf]` (thermally and calorically perfect gas), `[therm perf]`, `[isen]` (isentropic; "may not be applied to the flow across a shock wave") and `[adiab]` (adiabatic only; may be applied across a shock). Subscript `t` means total (stagnation) conditions, the state reached if the gas were brought to rest isentropically; `*` means critical conditions where the local speed equals the local speed of sound. Mach angle `mu = sin^-1(1/M)` (symbols, p. 613).
- **Equation of state (p. 614, Eqs. 2-3):** thermally perfect gas `p = RT/v = rho R T`. Caloric relations: `du = c_v dT` (Eq. 8b), and for constant specific heats `u = c_v T + u_r` (Eq. 9).
- **Specific heats (p. 615, Eqs. 11-17):** `c_p - c_v = R` for a thermally perfect gas (Eq. 13b); `gamma = c_p/c_v` (Eq. 14); kinetic theory gives `gamma = (n+2)/n` with `n` effective degrees of freedom (Eq. 15); `c_p = gamma R/(gamma-1)`, `c_v = R/(gamma-1)` (Eqs. 16-17). Enthalpy `h = u + p v` (Eq. 18), `dh = c_p dT` (Eq. 19b).
- **First law and entropy (p. 615, Eqs. 10, 21-25):** `dq = du + p dv = dh - v dp` (Eq. 10a); `ds = (dq/T)_rev` (Eq. 21); for a perfect gas `s - s_r = c_v ln[(T/T_r)/(rho/rho_r)^(gamma-1)]` (Eq. 23b) and `p/rho^gamma = (p_r/rho_r^gamma) e^((s-s_r)/c_v)` (Eq. 24); the second law requires `s - s_r >= 0` for adiabatic processes (Eq. 25).
- **1D flow equations (pp. 615-616, Eqs. 26-28):** for inviscid, non-heat-conducting flow along a streamline: state `p/rho = RT` (Eq. 26), dynamic equation `(1/rho) dp + V dV = 0` (Eq. 27), energy `dh + V dV = 0` (Eq. 28a).
- **Speed of sound and Mach number (p. 616, Eqs. 29-31):** `a = sqrt((dp/drho)_s) = sqrt(gamma (dp/drho)_T)` (Eq. 29a); for a thermally perfect gas `a = sqrt(gamma p/rho) = sqrt(gamma R T)` (Eq. 29b); for air about `49.0 sqrt(T)` ft/s with `T` in degrees Rankine (Eq. 29c). Mach number `M = V/a` (Eq. 30). Dynamic pressure `q = rho V^2/2 = (gamma/2) p M^2` (Eqs. 31a-b).
- **Energy integral (p. 616, Eqs. 32-33):** `h + V^2/2 = h_t` (adiabatic, Eq. 32a); `c_p T + V^2/2 = c_p T_t`; `a^2/(gamma-1) + V^2/2 = a_t^2/(gamma-1)` (Eq. 32b).
- **Isentropic relations (p. 616, Eqs. 34-35, 43-46):** `p/rho^gamma = const` (Eq. 34); `p/p_t = (rho/rho_t)^gamma = (T/T_t)^(gamma/(gamma-1))` (Eq. 35). In terms of `M`: `T/T_t = (1 + (gamma-1)/2 M^2)^-1` (Eq. 43, adiabatic); `p/p_t = (1 + (gamma-1)/2 M^2)^(-gamma/(gamma-1))` (Eq. 44, isentropic); `rho/rho_t = (1 + (gamma-1)/2 M^2)^(-1/(gamma-1))` (Eq. 45); `a/a_t = (1 + (gamma-1)/2 M^2)^(-1/2)` (Eq. 46).
- **Small disturbances (p. 616, Eqs. 40-42):** for `(V - V_inf) << V_inf`, `p/p_inf ~ 1 - gamma M_inf^2 (V - V_inf)/V_inf` and `rho/rho_inf ~ 1 - M_inf^2 (V - V_inf)/V_inf` (Eqs. 41-42): the relative density change scales with `M^2`.
- **Stream-tube area (p. 618, Eqs. 79-80):** continuity `rho V A = const = rho_* a_* A_*` (Eq. 79); `A_*/A = ((gamma+1)/2)^((gamma+1)/(2(gamma-1))) M (1 + (gamma-1)/2 M^2)^(-(gamma+1)/(2(gamma-1)))` (Eq. 80).
- **Normal shock: basic equations (pp. 618-619, Eqs. 84-89):** the isentropic relations hold on either side of a shock but not across it, because "the flow quantities have discontinuities". Jump conditions: mass `rho_1 u_1 = rho_2 u_2` (Eq. 84); momentum `p_1 + rho_1 u_1^2 = p_2 + rho_2 u_2^2` (Eq. 85); energy `u_1^2/2 + h_1 = u_2^2/2 + h_2` (Eq. 86a), with the requirement that entropy does not decrease, `Delta s = s_2 - s_1 >= 0` (Eq. 87). Total enthalpy, total temperature and the critical speed of sound are constant across the shock (Eq. 88). Prandtl's relation `u_1 u_2 = a_*^2` (Eq. 89) "implies that the flow is supersonic ahead of the shock wave and subsonic behind (the reverse possibility is ruled out by the requirement of nondecreasing entropy)".
- **Rankine-Hugoniot relations (p. 619, Eqs. 90-92):** `p_2/p_1 = [(gamma+1) rho_2 - (gamma-1) rho_1]/[(gamma+1) rho_1 - (gamma-1) rho_2]` (Eq. 90); `rho_2/rho_1 = [(gamma+1) p_2 + (gamma-1) p_1]/[(gamma+1) p_1 + (gamma-1) p_2]` (Eq. 91).
- **Normal shock in terms of `M_1` (p. 619, Eqs. 93-102):** `p_2/p_1 = (2 gamma M_1^2 - (gamma-1))/(gamma+1)` (Eq. 93); `rho_2/rho_1 = u_1/u_2 = (gamma+1) M_1^2/((gamma-1) M_1^2 + 2)` (Eq. 94); `M_2^2 = ((gamma-1) M_1^2 + 2)/(2 gamma M_1^2 - (gamma-1))` (Eq. 96); total-pressure ratio `p_t2/p_t1 = e^(-Delta s/R)` (Eq. 99); entropy jump (Eq. 101). For weak shocks (Eq. 104) the entropy rise is third order in `(M_1^2 - 1)`: `Delta s/R = 2 gamma/(gamma+1)^2 (M_1^2-1)^3/3 + ...`.
- **Prandtl-Meyer expansion (p. 626, Eqs. 171-174):** a uniform supersonic stream turning round a convex bend expands isentropically; the Prandtl-Meyer angle `nu` through which the stream turns from `M = 1` to `M` is `nu = sqrt((gamma+1)/(gamma-1)) tan^-1 sqrt((gamma-1)/(gamma+1) (M^2-1)) - tan^-1 sqrt(M^2-1)` (Eq. 171c); maximum turning `130.45` degrees for `gamma = 7/5` (Eq. 172). The figures show expansions bounded by Mach lines.

## Contradictions with the wiki

None found.

## Pages this source informs

[[ideal-gas-and-thermodynamics-basics]], [[speed-of-sound-and-mach-number]], [[shock-waves]], [[euler-equations]], [[characteristics-and-hyperbolic-systems]]
