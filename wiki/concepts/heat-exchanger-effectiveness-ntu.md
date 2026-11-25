---
type: concept
tags: [foundation, heat-transfer, heat-exchanger]
sources: [ref-lienhard-ahtt-6e]
builds_on: [convective-heat-transfer]
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# Heat exchanger effectiveness and NTU

## In short

The effectiveness-NTU method rates a heat exchanger without solving its internal temperature field: the effectiveness $\varepsilon$ is the heat actually transferred over the most that could be transferred, and it is a known function of the number of transfer units (NTU) and the heat capacity rate ratio for each flow arrangement ([[ref-lienhard-ahtt-6e]] sec. 3.3).

## Explanation

**Definitions.** With hot and cold stream capacity rates $C_h$, $C_c$ (mass flow times specific heat) and the smaller one $C_{min}$:

$$\varepsilon = \frac{C_h(T_{h,in} - T_{h,out})}{C_{min}(T_{h,in} - T_{c,in})} = \frac{C_c(T_{c,out} - T_{c,in})}{C_{min}(T_{h,in} - T_{c,in})}$$ (Lienhard eqs. 3.16, 3.17)

so the duty is $Q = \varepsilon C_{min}(T_{h,in} - T_{c,in})$. The size of the exchanger enters through

$$NTU = \frac{UA}{C_{min}}$$ (Lienhard eq. 3.18)

with the overall heat transfer coefficient $U$ and area $A$ (built from the convection coefficients of [[convective-heat-transfer]]). The definition goes back to Nusselt; Kays and London developed the method in full in 1955; it works if $U$ is fairly uniform ([[ref-lienhard-ahtt-6e]] sec. 3.3).

**Formulas.** With $C_r = C_{min}/C_{max}$:

$$\varepsilon_{parallel} = \frac{1 - e^{-(1+C_r)NTU}}{1 + C_r}$$ (Lienhard eq. 3.20)

$$\varepsilon_{counter} = \frac{1 - e^{-(1-C_r)NTU}}{1 - C_r\,e^{-(1-C_r)NTU}}$$ (Lienhard eq. 3.21)

Example 3.5: parallel flow with $NTU = 1.5$ and $C_r = 0.5$ gives $\varepsilon = 0.596$. Table 3.1 gives other arrangements (crossflow, shell and tube) ([[ref-lienhard-ahtt-6e]] sec. 3.3).

## Builds on

- [[convective-heat-transfer]]: the heat transfer coefficients inside $U$.
- Floor: energy balance of a stream, $Q = C\,\Delta T$.

## Leads to

- rcfd's lumped heat-exchanger zones and heat-exchanger macro models

## How rcfd uses it

- F-217 (ADR 0039) heat-exchanger macro models: counterflow "within 2 percent of eps-NTU"; the both-unmixed crossflow curve fit missed by 3.3 % and was replaced by an exact series in ADR 0044 (7.717e-6 against 1e-4) ([[adr-thread-early-physics-models]], [[doc-reference-data-wave23a-mhd-heat-exchanger]]).
- Lumped heat-exchanger zone with $Q = \varepsilon C_{min}(T_{sec} - T_{in})$ (ADR 0012) ([[doc-reference-data-lumped-radiation]]).
- rcfd recorded the parallel and counterflow formulas and the effectiveness definition as "citation UNVERIFIED (memory)"; Lienhard eqs. 3.17, 3.20 and 3.21 state the same forms ([[doc-reference-data-wave23a-mhd-heat-exchanger]], [[ref-lienhard-ahtt-6e]]).
- Update 2026-11-25 (3b8f12c4, F-886, ADR 0796): read by the repo and compared with the code, at most 3.2e-14 at 96 points; the marks are struck. Table 3.1 (crossflow, shell and tube) is a proposed follow-up ([[adr-thread-vof-multiphase-particles]]).
