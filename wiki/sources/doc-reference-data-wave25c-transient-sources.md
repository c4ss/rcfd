---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 25C transient momentum sources (F-214, F-217, F-218, ADR 0045)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 25C transient momentum sources (F-214, F-217, F-218, ADR 0045)" (lines 1010-1036) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 25C transient momentum source references (ADR 0045).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No literature source was consulted in wave 25C. Every reference value below is derived from first principles in this section or reused from the earlier waves named in ADR 0045. Nothing is credited to a book or paper.

**Impulsively accelerated plane Poiseuille flow (check T2).** A uniform body acceleration $G$ along $x$ acts from $t = 0$ on a fluid at rest between no-slip plates at $y = 0$ and $y = h$: $u_t = G + \nu u_{yy}$, $u(0,t) = u(h,t) = 0$, $u(y,0) = 0$. The steady solution is $u_s = G y (h - y) / (2\nu)$. The difference $v = u - u_s$ satisfies the homogeneous heat equation with $v(y,0) = -u_s(y)$, so $v = \sum_n b_n \sin(n\pi y/h)\, \exp(-n^2\pi^2\nu t/h^2)$ with $b_n = -\frac{2}{h}\int_0^h u_s \sin(n\pi y/h)\, dy = -\frac{G}{\nu h}\int_0^h y (h - y) \sin(n\pi y/h)\, dy$. Two integrations by parts give $\int_0^h y (h - y) \sin(n\pi y/h)\, dy = 2 h^3 (1 - \cos n\pi)/(n\pi)^3$, which is $4 h^3/(n\pi)^3$ for odd $n$ and zero for even $n$. Hence

$$u(y,t) = \frac{G y (h - y)}{2\nu} - \frac{4 G h^2}{\nu \pi^3} \sum_{n\ \text{odd}} \frac{\sin(n\pi y/h)\, \exp(-n^2\pi^2\nu t/h^2)}{n^3}.$$

The derivation was checked by substitution (each term solves the heat equation and vanishes at both plates) and numerically. For $G = 8$, $\nu = h = 1$ with the 100 odd terms used by the test, the numerical check gave $\max \lvert u(y_c, 0)\rvert = 5.0 \times 10^{-7}$ over the 40 cell centres of the T2 mesh (the truncation of the series at $t = 0$) and $u(h/2, 5) = 1.000000 = G h^2/(8\nu)$.

**Uniform acceleration with a linear sink (check T1b).** The equation $du/dt = a - s u$ has the solution $u = a/s + (u_0 - a/s)\, \exp(-s t)$, and the implicit Euler recursion is $u_{n+1} = (u_n + a\, dt)/(1 + s\, dt)$. Both are exact algebra, no data.

**Hartmann limits (T3).** The short-circuit profile and flow rate of `rcfd_fv::lorentz::hartmann` are reused from wave 23A (docs/reference-data.md, F-214 section).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
