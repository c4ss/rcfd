"""ADR 0084 (wave 37A): analysis of the finite-difference Jacobians written by crates/rcfd-fv/tests/w37a_jacobian.rs.

Usage: python scripts/w37a_jacobian_analysis.py DIR TAG [TAG ...]
       python scripts/w37a_jacobian_analysis.py DIR --compare TAG_A TAG_B   (relative difference of two Jacobians)

Per Jacobian: the eigenvalues in the scaled variables (velocity / U, pressure / P, flux / (U A)); the neutral modes (|lambda - 1| < 1e-6
with a velocity block that is a mean velocity, |sum u| / sqrt(N sum u^2) > 0.9 for a component: the free mean velocity of a doubly periodic box); rho = the largest |lambda| of the rest; for the
leading modes the block energy fractions (u, v, p, phi), the dominant x-Fourier index m of the velocity block (cells labelled (i, j) by the
harness), the energy fraction on the seam rows/columns, the pressure checkerboard index; the transient growth max_k ||J_d^k||_2 of the
deflated map J_d = J (I - P_neutral) over k in {1, 2, 5, 10, 20, 50, 100, 200, 400}.
"""
import sys
import numpy as np


def load(d, tag):
    meta = {}
    cells, faces = [], []
    for line in open(f"{d}/{tag}.meta"):
        k, *v = line.split()
        if k == "cell":
            cells.append((int(v[0]), int(v[1])))
        elif k == "face":
            faces.append(int(v[0]))
        elif k == "scales":
            meta[k] = [float(x) for x in v]
        else:
            meta[k] = int(v[0])
    n = meta["n"]
    j = np.fromfile(f"{d}/{tag}.bin", dtype="<f8").reshape(n, n)
    return j, meta, np.array(cells), np.array(faces)


def scaling(meta):
    nc, nb = meta["n_cells"], meta["comps"]
    su, sp, sf = meta["scales"]
    s = np.concatenate([np.full(nb * nc, su), np.full(nc, sp), np.full(meta["n_faces"], sf)])
    return s


def describe(v, meta, cells, faces):
    nc, nb, nx, ny = meta["n_cells"], meta["comps"], meta["nx"], meta["ny"]
    e = np.abs(v) ** 2
    tot = e.sum()
    blocks = [e[b * nc:(b + 1) * nc].sum() / tot for b in range(nb)]
    blocks.append(e[nb * nc:(nb + 1) * nc].sum() / tot)
    blocks.append(e[(nb + 1) * nc:].sum() / tot)
    # x-Fourier energy of the velocity block
    em = np.zeros(nx)
    seam = 0.0
    for b in range(nb):
        g = np.zeros((nx, ny), dtype=complex)
        g[cells[:, 0], cells[:, 1]] = v[b * nc:(b + 1) * nc]
        em += (np.abs(np.fft.fft(g, axis=0)) ** 2).sum(axis=1)
        ge = np.abs(g) ** 2
        mask = np.zeros((nx, ny), dtype=bool)
        mask[[0, nx - 1], :] = True
        mask[:, [0, ny - 1]] = True
        seam += ge[mask].sum()
    ue = sum(e[b * nc:(b + 1) * nc].sum() for b in range(nb))
    seam_frac = seam / max(ue, 1e-300)
    seam_uniform = (2 * ny + 2 * nx - 4) / (nx * ny)
    mfold = np.zeros(nx // 2 + 1)
    for m in range(nx):
        mfold[min(m, nx - m)] += em[m]
    mdom = int(np.argmax(mfold))
    pv = v[nb * nc:(nb + 1) * nc]
    sign = np.where((cells[:, 0] + cells[:, 1]) % 2 == 0, 1.0, -1.0)
    chk = abs((sign * pv).sum()) / max(np.abs(pv).sum(), 1e-300)
    means = []
    for b in range(nb):
        ub = v[b * nc:(b + 1) * nc]
        means.append(abs(ub.sum()) / max(np.sqrt(nc * (np.abs(ub) ** 2).sum()), 1e-300))
    return blocks, mdom, mfold[mdom] / mfold.sum(), seam_frac, seam_uniform, chk, means


def analyse(d, tag, top=6, kmax=400):
    j, meta, cells, faces = load(d, tag)
    s = scaling(meta)
    js = j * s[None, :] / s[:, None]  # D^-1 J D, D = diag(scales)
    w, vr = np.linalg.eig(js)
    nc, nb = meta["n_cells"], meta["comps"]
    neutral = []
    for i in range(len(w)):
        if abs(w[i] - 1.0) < 1e-6:
            if max(describe(vr[:, i], meta, cells, faces)[6]) > 0.9:
                neutral.append(i)
    rest = [i for i in range(len(w)) if i not in neutral]
    order = sorted(rest, key=lambda i: -abs(w[i]))
    rho = abs(w[order[0]])
    print(f"== {tag}: n = {meta['n']}, neutral modes {len(neutral)} "
          f"(eigenvalues {[complex(round(w[i].real, 9), round(w[i].imag, 9)) for i in neutral]}), rho = {rho:.6f}")
    shown = 0
    seen = set()
    for i in order:
        key = (round(abs(w[i]), 8), round(abs(w[i].imag), 8))
        if key in seen:
            continue
        seen.add(key)
        blocks, m, mfrac, seam, seam_u, chk, means = describe(vr[:, i], meta, cells, faces)
        print(f"   lambda {w[i].real:+.6f}{w[i].imag:+.6f}i |{abs(w[i]):.6f}| blocks u/v/p/phi "
              f"{' '.join(f'{x:.3f}' for x in blocks)} m {m} ({mfrac:.2f}) seam {seam:.3f} (uniform {seam_u:.3f}) checker {chk:.3f} mean-velocity fraction {' '.join(f'{x:.3f}' for x in means)}")
        shown += 1
        if shown >= top:
            break
    # largest |lambda| per Fourier index m
    per_m = {}
    for i in rest:
        _, m, mfrac, *_ = describe(vr[:, i], meta, cells, faces)
        per_m[m] = max(per_m.get(m, 0.0), abs(w[i]))
    print("   rho per x-Fourier index m: " + ", ".join(f"{m}: {per_m[m]:.5f}" for m in sorted(per_m)))
    # deflated transient growth
    if neutral:
        vl = np.linalg.inv(vr)
        pn = (vr[:, neutral] @ vl[neutral, :]).real
        jd = js @ (np.eye(len(w)) - pn)
    else:
        jd = js
    p = np.eye(len(w))
    best, kbest = 0.0, 0
    marks = {}
    for k in range(1, kmax + 1):
        p = jd @ p
        if k in (1, 2, 5, 10, 20, 50, 100, 200, 400):
            nk = np.linalg.norm(p, 2)
            marks[k] = nk
            if nk > best:
                best, kbest = nk, k
    print(f"   transient growth: max_k ||J_d^k||_2 = {best:.4e} at k = {kbest}; "
          + ", ".join(f"k={k}: {v:.3e}" for k, v in marks.items()))
    return rho


def compare(d, a, b):
    ja = load(d, a)[0]
    jb = load(d, b)[0]
    rel = np.linalg.norm(ja - jb) / np.linalg.norm(ja)
    print(f"== compare {a} vs {b}: relative Frobenius difference {rel:.3e}")


if __name__ == "__main__":
    d = sys.argv[1]
    if sys.argv[2] == "--compare":
        compare(d, sys.argv[3], sys.argv[4])
    else:
        for t in sys.argv[2:]:
            analyse(d, t)
