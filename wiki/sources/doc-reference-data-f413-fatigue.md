---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-413 high-cycle fatigue: stress-life curves, mean stress, rainflow, Miner, narrow band (ADR 0449)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-413 high-cycle fatigue: stress-life curves, mean stress, rainflow, Miner, narrow band (ADR 0449)" (lines 1638-1657) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-413 high-cycle fatigue references (ADR 0449).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-413; every definition is cited from memory, and every number checked comes from a closed form derived in ADR 0449 or from the library's own finite-element models (Kirsch and the beam formula as the references). The material values used ($\sigma_f = 900$ MPa, $b = -0.09$, $\sigma_u = 600$ MPa, $\sigma_y = 400$ MPa) are illustrative, not data of a material.

| Item | Status | Source |
|---|---|---|
| Basquin $\sigma_a = \sigma_f (2 N)^b$, so $N = (\sigma_a/\sigma_f)^{1/b}/2$ (the factor 2 on the reversals); power-law form $m = -1/b$, $C = \sigma_f^m/2$ | derived | Basquin, "The exponential law of endurance tests", Proc. ASTM 10 (1910) 625-630, in the reversal notation of Morrow: NOT read; ADR 0449 closed forms |
| Goodman $\sigma_a/(1 - \sigma_m/\sigma_u)$, Gerber $\sigma_a/(1 - (\sigma_m/\sigma_u)^2)$, Soderberg $\sigma_a/(1 - \sigma_m/\sigma_y)$; no credit for a compressive mean by default | from memory | Goodman (1899), Gerber (1874), Soderberg, Trans. ASME 52 (1930); standard texts (e.g. Dowling, Mechanical Behavior of Materials): NOT read; derived as the lines (parabola) through the strength in ADR 0449 |
| Von Mises amplitude of $\operatorname{Re}((A + i B) e^{i w t})$: $\sqrt{(q_A + q_B)/2 + \sqrt{((q_A - q_B)/2)^2 + p_{AB}^2}}$ | derived | first principles, ADR 0449; checked against $\max(s, \sqrt{3} t)$ and 200 sampled cases |
| Rainflow counting: four-point closing of hysteresis loops; periodic history rotated to its turning point of largest modulus | from memory | Matsuishi and Endo (1968); Downing and Socie, Int. J. Fatigue 4 (1982) 31-40; ASTM E1049-85 (2017) section 5.4.4: NOT read (the standard's worked example not used); properties derived and checked in ADR 0449 (`f413-scratch/rainflow.py`) |
| Palmgren-Miner linear damage $D = \sum n_i/N_i$ | from memory | Palmgren (1924); Miner, "Cumulative damage in fatigue", J. Appl. Mech. 12 (1945) A159-A164: NOT read |
| Narrow-band damage rate $\nu_{0} (\sqrt{2} s)^m \Gamma(1 + m/2)/C$; $\nu_{0} = \sqrt{m_{2}/m_{0}}$, $s = \sqrt{m_{0}}$ | derived | Rice, Bell Syst. Tech. J. 23 (1944) and 24 (1945); Miles, J. Aeronaut. Sci. 21 (1954) 753-762; Bendat (1964): NOT read; the Rayleigh integral evaluated in ADR 0449 |
| $\ln \Gamma$ by Stirling's series with $B_2 \dots B_{14}$ for $x \ge 15$ and the recurrence below | from memory | Abramowitz and Stegun, Handbook of Mathematical Functions, 6.1.40: NOT read; Bernoulli numbers exact rationals; checked against factorials and half-integer values (B1) |
| Kirsch: hoop stress $3 \sigma_0$ at the edge of a circular hole, 90 degrees from remote tension | from memory | Kirsch, Z. VDI 42 (1898) 797-807; Timoshenko and Goodier, Theory of Elasticity: NOT read; measured 1.0123 on the quarter plate of ADR 0449 ($r/W = 0.05$) |
| Finite-width factor of a hole, $K_{tn} = 3 - 3.14 (d/W) + 3.667 (d/W)^2 - 1.527 (d/W)^3$ (gross 3.023 at $d/W = 0.1$) | from memory | Peterson's Stress Concentration Factors after Howland (1930): NOT read; used only for the estimate of K1 in ADR 0449, not for a limit |
| Cantilever root stress $6 P L/(b h^2)$ | derived | Euler-Bernoulli beam; the support of beam theory of ADR 0433 D2 |
| Benasciutti-Tovo broadband damage rate $[b + (1 - b) \alpha_{2}^{m-1}] D_{\mathrm{NB}}$, $\alpha_{1} = m_{1}/\sqrt{m_{0} m_{2}}$, $\alpha_{2} = m_{2}/\sqrt{m_{0} m_{4}}$, $b = (\alpha_{1} - \alpha_{2}) [1.112 (1 + \alpha_{1} \alpha_{2} - (\alpha_{1} + \alpha_{2})) \exp(2.11 \alpha_{2}) + (\alpha_{1} - \alpha_{2})]/(\alpha_{2} - 1)^2$, $D_{\mathrm{NB}}$ at the up-crossing rate $\sqrt{m_{2}/m_{0}}$ (ADR 0524) | from memory, **fitted $b$ unverified** | Benasciutti and Tovo, Int. J. Fatigue 27 (2005) 867-877: NOT read; the simpler $b = (\alpha_{1} - \alpha_{2})/(1 - \alpha_{1})$ (other paper) is not used; corroborated by BT/Dirlik = 0.99948 on the two-mode case of ADR 0497 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
