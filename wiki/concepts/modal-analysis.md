---
type: concept
tags: [foundation, structural, dynamics, eigenvalues]
sources: [ref-bathe-mit-2-092-fea-notes, ref-bai-2000-templates-eigenvalue, adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-basic-tier, doc-structural-solver]
builds_on: [structural-dynamics, eigenvalue-solvers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Modal analysis (natural frequencies and mode shapes)

## In short

Every structure has preferred ways to vibrate, its modes, each with a natural frequency. They come from the generalised eigenvalue problem $\mathbf K\boldsymbol\phi = \omega^2\mathbf M\boldsymbol\phi$. Modes are the basis for judging resonance (is a forcing frequency near a natural frequency?) and for solving dynamic problems cheaply by superposing a few modes ([[ref-bathe-mit-2-092-fea-notes]], lectures 15-16, 19, 23).

## Explanation

**The eigenproblem.** For free undamped vibration try $\mathbf U = \boldsymbol\phi\sin\omega(t - t_0)$ in $\mathbf M\ddot{\mathbf U} + \mathbf K\mathbf U = 0$: $$\mathbf K\boldsymbol\phi = \omega^2\mathbf M\boldsymbol\phi, \qquad \det(\mathbf K - \lambda\mathbf M) = 0,\ \lambda = \omega^2$$ with $0 \le \omega_1^2 \le \omega_2^2 \le \dots \le \omega_n^2$ (Bathe, lecture 15). Each $\boldsymbol\phi_i$ is a mode shape; normalised so that $\boldsymbol\phi_i^T\mathbf M\boldsymbol\phi_i = 1$, the modes are M-orthonormal and K-orthogonal: $$\boldsymbol\phi_i^T\mathbf M\boldsymbol\phi_j = \delta_{ij}, \qquad \boldsymbol\phi_i^T\mathbf K\boldsymbol\phi_j = \omega_i^2\delta_{ij}$$ ([[ref-bathe-mit-2-092-fea-notes]]).

**Rigid-body modes.** An unsupported body has $\omega = 0$ modes, rigid motions with no strain energy ([[ref-bathe-mit-2-092-fea-notes]], lecture 15).

**Rayleigh quotient.** $\rho(\mathbf v) = \mathbf v^T\mathbf K\mathbf v/\mathbf v^T\mathbf M\mathbf v$ lies between $\omega_1^2$ and $\omega_n^2$; $\omega_1^2$ is the least strain energy (times two) storable with $\mathbf v^T\mathbf M\mathbf v = 1$ ([[ref-bathe-mit-2-092-fea-notes]], lectures 16, 23).

**Mode superposition.** With $\mathbf U = \boldsymbol\Phi\mathbf X$ and damping diagonal in the modes, the equations decouple into $$\ddot x_i + 2\omega_i\xi_i\dot x_i + \omega_i^2 x_i = \boldsymbol\phi_i^T\mathbf R = r_i$$ (lecture 16, eq. 5), with initial conditions ${}^0\mathbf X = \boldsymbol\Phi^T\mathbf M\,{}^0\mathbf U$ (eq. 7). A structure released in one mode vibrates only in that mode. How many modes: those excited by the initial conditions and by the spatial distribution and frequency content of the load, up to a cut-off frequency above the highest load frequency; the residual of the truncated solution measures the error, and a static correction recovers the quasi-static part carried by the omitted modes (lecture 19). It suits low-frequency excitation, not wave propagation ([[ref-bathe-mit-2-092-fea-notes]]).

**Computing a few modes.** Large models need only the lowest $p$ eigenpairs. Rayleigh-Ritz projects onto trial vectors $\boldsymbol\Psi$, $(\boldsymbol\Psi^T\mathbf K\boldsymbol\Psi)\mathbf x = \rho(\boldsymbol\Psi^T\mathbf M\boldsymbol\Psi)\mathbf x$; subspace iteration alternates inverse iteration $\mathbf K\bar{\mathbf X}_{k+1} = \mathbf M\mathbf X_k$ on $q$ vectors with a Rayleigh-Ritz step, converging at rate $\lambda_i/\lambda_{q+1}$, with $q = \max(2p, p + 8)$ called "quite conservative" ([[ref-bathe-mit-2-092-fea-notes]], lecture 23). The power, inverse, subspace and Lanczos iterations are on [[eigenvalue-solvers]] ([[ref-bai-2000-templates-eigenvalue]]).

## Builds on

- [[structural-dynamics]]: the matrices $\mathbf M$, $\mathbf K$, $\mathbf C$.
- [[eigenvalue-solvers]]: algorithms for large symmetric eigenproblems.

## Leads to

- [[harmonic-and-random-response]], [[linear-buckling-and-stress-stiffening]], [[rotordynamics-gyroscopic-effects]], [[substructuring-and-craig-bampton]]
- [[campbell-diagram]], [[cyclic-symmetry]], [[mpc-transformation]]

## How rcfd uses it

- Modal analysis is part of the basic tier (ADR 0270), with an eigensolver registered among the basic-tier ADRs ([[adr-thread-structural-basic-tier]]); the fan-blade demonstration's first frequency is 188.8 Hz coarse and 186.1 Hz fine ([[doc-structural-solver]]).
- Scale-aware convergence of the subspace iteration (F-457, ADR 0354); modal analysis on the transformation with zero modes and the count of free rigid motions (F-486, F-488) ([[adr-thread-structural-modal-buckling-cyclic]]).
- Modes per harmonic index on a sector ([[cyclic-symmetry]]), tracked against speed ([[campbell-diagram]]).
