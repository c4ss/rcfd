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

# Reference data: F-416 a flexible rotor on bearings in the stationary frame (ADR 0476)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-416 a flexible rotor on bearings in the stationary frame (ADR 0476)" (lines 1839-1858) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-416 flexible rotor on bearings in the stationary frame (ADR 0476).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-416; every item is cited from memory and derived in ADR 0476 (set-ups and arithmetic 1 to 5; numpy checks `f416-scratch/py/estimates.py`, `r_set.py`, `j_set.py`, `o_set.py`, `a_set.py`, `k_set.py`, `u_set.py`; the zero-speed probe `f416-scratch/probe`). The references of the checks are closed forms, the F-391 beam model (`modal_analysis`) and the rigid rotor of ADR 0120 (`rotordyn`).

| Item | Status | Source |
|---|---|---|
| Gyroscopic moments of a spinning section $m_x = \rho I_p \Omega \theta_{y,t}$, $m_y = -\rho I_p \Omega \theta_{x,t}$; the element matrix $G = \begin{bmatrix} 0 & P \\ -P & 0 \end{bmatrix}$, $P = \int \rho I_p N_{\psi}^T N_{\psi}$ | derived | the rate of the angular momentum $\rho I_p \Omega (\theta_y, -\theta_x, 1)$ (ADR 0476 D2); H. D. Nelson, J. M. McVaugh, J. Eng. Ind. 98 (1976) 593 and M. Lalanne, G. Ferraris, Rotordynamics Prediction in Engineering (1998), for the names only: NOT read |
| Forward and backward by the sense of the orbit: $\operatorname{Im}(X \operatorname{conj} Y) > 0$ forward; the mass-weighted whirl number in `[-1, 1]` | derived | the averaged $x y_t - y x_t$ (ADR 0476 D5) |
| Undamped critical speeds $K z = \Omega^2 (M - i G) z$, `M - i G` Hermitian | derived | the synchronous root $\lambda = i \Omega$ (ADR 0476 D6) |
| Jeffcott rotor: critical speed $\sqrt{k/m}$, unbalance amplitude $e r^2/\sqrt{(1 - r^2)^2 + (2 \zeta r)^2}$ | derived | $m w_{tt} + c w_t + k w = m e \Omega^2 e^{i \Omega t}$ in $w = x + i y$; G. Genta, Dynamics of Rotating Systems (2005), for the name only: NOT read |
| Timoshenko shaft under a central load: $k_s = 1/(L^3/(48 E I) + L/(4 \kappa G A))$ | derived | bending and shear compliance of a simply supported beam (ADR 0476 set-up J) |
| Cross-coupled stiffness on a damped disc: threshold $k_c = c w_n$ exactly | derived | $m s^2 + c s + k - i k_c = 0$ at $s = i w$ (ADR 0476 set-up X); D. W. Childs, Turbomachinery Rotordynamics (1993), for the name only: NOT read |
| Spinning Rayleigh beam on pins: $E I k^4 - \rho A w^2 - \rho I k^2 w^2 \pm 2 \rho I k^2 \Omega w = 0$, $k = n \pi/L$ | derived | $E I w'''' + \rho A w_{tt} - \rho I w_{tt}'' + i \rho I_p \Omega w_t'' = 0$ (ADR 0476 set-up R) |
| Overhung disc on a cantilever, the two-unknown model: $\det \begin{bmatrix} k_{11} - m \lambda^2 & k_{12} \\ k_{12} & k_{22} - I_t \lambda^2 + I_p \Omega \lambda \end{bmatrix} = 0$ with the Timoshenko tip flexibility $a_{11} = L^3/(3 E I) + L/(\kappa G A)$, $a_{12} = L^2/(2 E I)$, $a_{22} = L/(E I)$ | derived | ADR 0476 set-up O; M. I. Friswell et al., Dynamics of Rotating Machines (2010), for the name only: NOT read |
| Fujiwara's bound on polynomial roots $2 \max \lvert a_{n-k}/a_n \rvert^{1/k}$ (with $\lvert a_0/(2 a_n) \rvert^{1/n}$) | from memory | used only for the scan range of the quartic's roots (ADR 0476 amendment 1) |
| Cyclic Jacobi rotation $t = \operatorname{sgn}(\tau)/(\lvert \tau \rvert + \sqrt{1 + \tau^2})$ and its Hermitian form by a phase | from memory | G. H. Golub, C. F. Van Loan, Matrix Computations: NOT read; checked through the closed forms of the checks |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
