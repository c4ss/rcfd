#!/usr/bin/env python3
"""Regenerate the Graetz-problem reference tables from first principles.

Writes validation-data/reference/graetz_parallel_plates.csv, laminar_internal_flow_limits.csv
and graetz_tube_cwt_eigenvalues.csv. Needs numpy. Run: python validation-data/generate_graetz.py

Method: finite-volume semi-discretisation of the cross-stream operator, exact in the streamwise
coordinate through an eigen-decomposition. Parallel plates, fully developed laminar flow,
x+ = x / (Dh Re Pr), Dh = 4b (b = half gap):
    1.5 (1 - eta^2) d(theta)/dx+ = 16 d2(theta)/d(eta)2, eta in [0, 1], symmetry at eta = 0.
Tube, constant wall temperature: (Y phi')' + lambda^2 Y (1 - Y^2) phi = 0, phi'(0) = 0, phi(1) = 0,
Richardson-extrapolated from N = 1000 and 2000 cells. Rows agree with the classical series
solutions (Shah and London, Convective heat transfer in ducts) to the digits printed.
"""
import pathlib
import numpy as np

OUT = pathlib.Path(__file__).parent / "reference"
XS_TW = [1e-4, 5e-4, 1e-3, 3e-3, 5e-3, 1e-2, 2e-2, 5e-2, 0.1, 0.2]
XS_Q = [1e-4, 5e-4, 1e-3, 5e-3, 1e-2, 3e-2, 5e-2, 0.1, 0.2]


def plates(n=3000):
    h = 1.0 / n
    lo = np.arange(n) * h
    hi = lo + h
    w = 1.5 * ((hi - hi**3 / 3) - (lo - lo**3 / 3))  # exact integral of u over each cell
    s = 1 / np.sqrt(w)

    def lap(dirichlet):
        a = np.zeros((n, n))
        d = 16 / h
        i = np.arange(n - 1)
        a[i, i + 1] += d
        a[i + 1, i] += d
        a[i, i] -= d
        a[i + 1, i + 1] -= d
        if dirichlet:
            a[n - 1, n - 1] -= 16 / (h / 2)
        return a

    rows = []
    lam, v = np.linalg.eigh(s[:, None] * lap(True) * s[None, :])
    z0 = v.T @ (np.sqrt(w) * np.ones(n))
    for x in XS_TW:
        th = s * (v @ (np.exp(lam * x) * z0))
        tb = w @ th / w.sum()
        rows.append(f"Tw,{x:g},{tb:.5f},{4 * (th[-1] / (h / 2)) / tb:.4f}")
    lam_q, v = np.linalg.eigh(s[:, None] * lap(False) * s[None, :])
    b = np.zeros(n)
    b[-1] = 16.0
    g = v.T @ (s * b)
    for x in XS_Q:
        small = np.abs(lam_q) < 1e-9
        f = np.where(small, x, np.expm1(lam_q * x) / np.where(small, 1, lam_q))
        th = s * (v @ (f * g))
        tb = w @ th / w.sum()
        rows.append(f"q,{x:g},,{4 / (th[-1] + h / 2 - tb):.4f}")
    return rows, -lam.max() / 4  # energy balance d(theta_b)/dx+ = 16 theta'(1): slowest mode lam0 gives Nu = -lam0 / 4


def tube_eigs(n):
    h = 1 / n
    lo = np.arange(n) * h
    hi = lo + h
    w = (hi**2 / 2 - hi**4 / 4) - (lo**2 / 2 - lo**4 / 4)
    a = np.zeros((n, n))
    i = np.arange(n - 1)
    f = hi[:-1] / h
    a[i, i + 1] += f
    a[i + 1, i] += f
    a[i, i] -= f
    a[i + 1, i + 1] -= f
    a[n - 1, n - 1] -= 1 / (h / 2)
    s = 1 / np.sqrt(w)
    return np.sqrt(np.sort(-np.linalg.eigvalsh(s[:, None] * a * s[None, :]))[:5])


def write(name, text):
    with open(OUT / name, "w", newline="
") as f:  # LF on every platform so the checksums agree
        f.write(text)


def main():
    rows, nu_tw = plates()
    write("graetz_parallel_plates.csv",
        "bc,x_plus,theta_b,Nu_x\n" + "\n".join(rows) + "\n")
    byu = (OUT / "laminar_internal_flow_limits.csv").read_text().splitlines()[1:]  # BYU rows are kept as they are
    limits = [r for r in byu if "parallel_plates" not in r] + [
        f"Nu_Dh_parallel_plates_constant_wall_T,{nu_tw:.4f},computed here: -lambda_0/4 of the slowest Tw mode",
        "Nu_Dh_parallel_plates_constant_heat_flux,8.2353,exact 140/17",
    ]
    write("laminar_internal_flow_limits.csv",
        "quantity,value,source\n" + "\n".join(limits) + "\n")
    a, b = tube_eigs(1000), tube_eigs(2000)
    e = b + (b - a) / 3
    write("graetz_tube_cwt_eigenvalues.csv",
        "n,lambda_n,lambda_n_squared\n"
        + "".join(f"{k + 1},{v:.4f},{v * v:.3f}\n" for k, v in enumerate(e)))


if __name__ == "__main__":
    main()
