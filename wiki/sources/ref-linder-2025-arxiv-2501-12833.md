---
type: source
tags: [reference, foundation, structural, dynamics, substructuring, arxiv, open-licence]
sources: []
origin: https://arxiv.org/abs/2501.12833
licence: CC BY 4.0 (arXiv abstract page licence link, verified 2026-11-22). Copying into raw/ would be allowed; not copied by this job.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Linder, Gross and Krack (2025), A coupled FE-BE multi-scale method for the dynamics of jointed structures

**Origin:** H. D. Linder, J. Gross, M. Krack (University of Stuttgart), arXiv:2501.12833v1 [eess.SY], 22 January 2025.
**Kind:** research paper (arXiv preprint). Used here for its self-contained statement of the Craig-Bampton method.
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-structural: abstract and sec. 2.2 (Craig-Bampton reduction, eqs. 7-12). The boundary-element coupling and results were not read beyond the remark on the 3000 constraint modes in sec. 3.

## Summary

A multi-scale method for friction damping in bolted joints couples a reduced finite-element model of the parts with a boundary-element model of the contact. The parts are reduced by Craig-Bampton component mode synthesis, which the paper restates.

## Key points

- **Sec. 2.2, why Craig-Bampton:** "by far the most popular component mode synthesis method"; for jointed structures the contacts mostly stick, "so that fixed-interface methods are expected to converge better than free-interface methods".
- **Sec. 2.2, eq. (8):** split the coordinates into boundary $\mathbf q_b$ and inner $\mathbf q_i$ and approximate $\begin{bmatrix}\mathbf q_b\\ \mathbf q_i\end{bmatrix} \approx \begin{bmatrix}\mathbf I & \mathbf 0\\ \boldsymbol\Psi & \boldsymbol\Phi\end{bmatrix}\begin{bmatrix}\mathbf q_b\\ \boldsymbol\eta\end{bmatrix}$; the boundary coordinates are kept.
- **Constraint modes:** $\boldsymbol\Psi = -\mathbf K_{ii}^{-1}\mathbf K_{ib}$; column $j$ is the static deflection of the inner coordinates for a unit displacement of boundary coordinate $j$ with the other boundary coordinates fixed.
- **Fixed-interface normal modes:** $\boldsymbol\Phi$ holds the $n_m$ lowest modes of $(\mathbf K_{ii} - \omega_m^2\mathbf M_{ii})\boldsymbol\phi_m = 0$, mass-normalised.
- **Reduced model, eqs. (9)-(12):** Galerkin projection with $\mathbf R$ gives $\tilde{\mathbf M}\ddot{\tilde{\mathbf q}} + \tilde{\mathbf K}\tilde{\mathbf q} = \mathbf R^T\mathbf W\boldsymbol\lambda + \tilde{\mathbf f}_{ex}$ with $\tilde{\mathbf K} = \mathbf R^T\mathbf K\mathbf R = \mathrm{blockdiag}(\tilde{\mathbf K}_{bb}, \mathrm{diag}\,\omega_m^2)$ (11) and $\tilde{\mathbf M}$ with an identity block for the modal coordinates (10).
- **Property:** the reduced model reproduces the static compliance for loads applied at the boundary exactly as the parent model; the retained modes must cover the frequency range of the response, and "a very small number of normal modes is sufficient" for most vibration problems. In the paper's example 25 fixed-interface modes were kept against 3000 static constraint modes (sec. 3).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[substructuring-and-craig-bampton]]
