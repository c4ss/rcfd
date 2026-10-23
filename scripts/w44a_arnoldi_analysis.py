"""ADR 0111: Ritz analysis of the matrix-free Arnoldi dumps written by crates/rcfd-api/tests/fan01_stall_w43c.rs (w44a_lin_*).

usage: python scripts/w44a_arnoldi_analysis.py <dump dir> <tag> [<tag> ...]

Per tag: the converged Ritz values (residual estimate h_{k+1,k} |y_k| <= 1e-2 |theta|), rho (largest converged modulus), rho_neg (the same
over negative real parts), rho from the first 150 steps (nested check M-A), the localisation of the leading Ritz vector (scaled velocity
energy in the five leading-edge cells and in n5, its five most energetic cells, the block shares). Writes <tag>.lead (the real part of the
leading Ritz vector, unit 2-norm, little-endian f64) for check M3.
"""
import sys

import numpy as np


def ritz(h, k):
    th, y = np.linalg.eig(h[:k, :k])
    y = y / np.linalg.norm(y, axis=0)
    res = np.abs(h[k, k - 1]) * np.abs(y[k - 1, :])
    return th, y, res


def main():
    d, tags = sys.argv[1], sys.argv[2:]
    for tag in tags:
        meta = dict(line.split(" ", 1) for line in open(f"{d}/{tag}.meta").read().splitlines())
        n, m, steps, nc, nf = (int(meta[k]) for k in ("n", "m", "steps", "n_cells", "n_faces"))
        le5 = [int(x) for x in meta["le5"].split()]
        n5 = [int(x) for x in meta["n5"].split()]
        h = np.fromfile(f"{d}/{tag}.h", "<f8").reshape(m + 1, m)
        v = np.fromfile(f"{d}/{tag}.v", "<f8").reshape(-1, n)
        k = steps
        th, y, res = ritz(h, k)
        conv = res <= 1e-2 * np.abs(th)
        order = np.argsort(-np.abs(th))
        print(f"== {tag}: n {n}, steps {steps}, h {meta['h']}, converged Ritz values {conv.sum()} of {k}")
        shown = 0
        for i in order:
            if shown >= 10:
                break
            print(f"   theta {th[i].real:+.6f} {th[i].imag:+.6f}i  |theta| {abs(th[i]):.6f}  residual {res[i]:.2e}  {'conv' if conv[i] else 'not conv'}")
            shown += 1
        ci = [i for i in order if conv[i]]
        if not ci:
            print("   no converged Ritz value")
            continue
        lead = ci[0]
        rho = abs(th[lead])
        neg = [i for i in ci if th[i].real < 0]
        rho_neg = abs(th[neg[0]]) if neg else float("nan")
        k2 = min(150, k)
        th2, _, res2 = ritz(h, k2)
        c2 = res2 <= 1e-2 * np.abs(th2)
        rho150 = np.max(np.abs(th2[c2])) if c2.any() else float("nan")
        above = [th[i] for i in ci if abs(th[i]) > 1.0]
        print(f"   rho {rho:.6f} (theta {th[lead].real:+.6f} {th[lead].imag:+.6f}i, residual {res[lead]:.2e}); rho_neg {rho_neg:.6f}; "
              f"rho from {k2} steps {rho150:.6f} (|diff| {abs(rho - rho150):.2e})")
        print(f"   converged values above 1: {', '.join(f'{z.real:+.5f}{z.imag:+.5f}i' for z in above) or 'none'}")
        z = v[:k].T @ y[:, lead]
        e = (np.abs(z[: 3 * nc].reshape(3, nc)) ** 2).sum(axis=0)
        tot = (np.abs(z) ** 2).sum()
        blocks = [np.abs(z[: 3 * nc]) ** 2, np.abs(z[3 * nc : 4 * nc]) ** 2, np.abs(z[4 * nc : 4 * nc + nf]) ** 2, np.abs(z[4 * nc + nf :]) ** 2]
        top = np.argsort(-e)[:5]
        print(f"   leading vector: velocity energy in the five cells {e[le5].sum() / e.sum():.3f}, in n5 ({len(n5)} cells) {e[n5].sum() / e.sum():.3f}; "
              f"top cells {', '.join(f'c{c} {e[c] / e.sum():.3f}' for c in top)}; block shares u/p/phi/c "
              f"{' / '.join(f'{b.sum() / tot:.3f}' for b in blocks)}")
        lv = z.real / np.linalg.norm(z.real)
        lv.astype("<f8").tofile(f"{d}/{tag}.lead")


if __name__ == "__main__":
    main()
