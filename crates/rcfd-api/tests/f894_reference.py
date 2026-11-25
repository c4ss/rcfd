"""Reference of ADR 0792 (F-894), rows A1 and D1: the closed form of the inverse CDF of a normal distribution open on one
side, in 760-digit decimal arithmetic, from first principles (the standard library only).

    python f894_reference.py > f894_reference.tsv

Columns: kind (grid, uq-mc, uq-lhs, sobol), mean, sd, lo, hi, index, u, x (the double nearest to the closed form). It shares
nothing with the code under test: erf from its alternating Maclaurin series at every argument, P = Phi(a) + u (Phi(b) -
Phi(a)) without loss, Newton's method in the same digits; the draws of the drivers from its own transcription of the
generator (xoshiro256** seeded by SplitMix64) and of the Latin hypercube.
"""
import math
import sys
from decimal import Decimal as D, getcontext

getcontext().prec = 760
INF = math.inf
V = 2.0 ** -53
CLAMP = (D(2.0 ** -1022), 1 - D(V))


def machin():
    def arctan_inverse(n):
        s, t, k = D(0), D(1) / n, 0
        while t > D(10) ** -770:
            s += t / (2 * k + 1) * (-1) ** k
            t /= n * n
            k += 1
        return s

    return 4 * (4 * arctan_inverse(5) - arctan_inverse(239))


PI = machin()
TWO_OVER_SQRT_PI = 2 / PI.sqrt()
SQRT_2 = D(2).sqrt()


def erf_alternating(x):
    """(2 / sqrt(pi)) sum (-1)^n x^(2n+1) / (n! (2n+1))"""
    x2, t, s, n = x * x, x, x, 0
    while True:
        n += 1
        t *= -x2 / n
        s += t / (2 * n + 1)
        if n > x2 and abs(t) < D(10) ** -700:
            return TWO_OVER_SQRT_PI * s


def erf_positive(x):
    """(2 / sqrt(pi)) exp(-x^2) sum 2^n x^(2n+1) / (1 3 ... (2n+1)): the cross-check"""
    x2, t, s, n = x * x, x, x, 0
    while abs(t) > abs(s) * D(10) ** -700:
        n += 1
        t *= 2 * x2 / (2 * n + 1)
        s += t
    return TWO_OVER_SQRT_PI * (-x2).exp() * s


def phi(z):
    return (-z * z / 2).exp() / (2 * PI).sqrt()


def upper(z):
    """Q(z) = 1 - Phi(z) for a Decimal z"""
    return (1 - erf_alternating(z / SQRT_2)) / 2


def cdf(z):
    if z in (D("Infinity"), D("-Infinity")):
        return D(1) if z > 0 else D(0)
    return 1 - upper(z)


def start(p, q):
    """z to about 15 digits: bisection on the smaller tail with the C library's erfc"""
    tail = float(min(p, q))
    lo, hi = 0.0, 40.0
    for _ in range(200):
        m = 0.5 * (lo + hi)
        if 0.5 * math.erfc(m / math.sqrt(2.0)) > tail:
            lo = m
        else:
            hi = m
    return D(lo) if q < p else D(-lo)


def quantile(p):
    q = 1 - p
    z = start(p, q)
    for _ in range(12):
        step = (cdf(z) - p) / phi(z)
        z -= step
        if abs(step) < D(10) ** -40:
            break
    else:
        sys.exit("Newton did not converge at %s" % float(p))
    c = cdf(z)
    if abs(c / p - 1) > D(10) ** -30 or abs((1 - c) / q - 1) > D(10) ** -30:
        sys.exit("residual at %s" % float(p))
    return z


CACHE = {}


def closed_form(mean, sd, lo, hi, u):
    key = (mean, sd, lo, hi)
    if key not in CACHE:
        CACHE[key] = (cdf((D(lo) - D(mean)) / D(sd)), cdf((D(hi) - D(mean)) / D(sd)))
    fa, fb = CACHE[key]
    uc = min(max(D(u), CLAMP[0]), CLAMP[1])
    p = max(fa + uc * (fb - fa), CLAMP[0])
    return D(mean) + D(sd) * quantile(p)


MASK = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        st, self.s = seed, []
        for _ in range(4):
            st = (st + 0x9E3779B97F4A7C15) & MASK
            z = st
            z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
            z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
            self.s.append(z ^ (z >> 31))

    @staticmethod
    def rotl(x, k):
        return ((x << k) | (x >> (64 - k))) & MASK

    def next_u64(self):
        s = self.s
        r = (self.rotl((s[1] * 5) & MASK, 7) * 9) & MASK
        t = (s[1] << 17) & MASK
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = self.rotl(s[3], 45)
        return r

    def uniform(self):
        return (self.next_u64() >> 11) * V

    def centred(self):
        """the draw of both drivers: ((next >> 11) + 0.5) / 2^53 in doubles"""
        return (float(self.next_u64() >> 11) + 0.5) / 2.0 ** 53


def latin_hypercube_1d(n, seed):
    rng, perm, out = Rng(seed), list(range(n)), []
    for i in range(n - 1, 0, -1):
        j = rng.next_u64() % (i + 1)
        perm[i], perm[j] = perm[j], perm[i]
    for i in range(n):
        x = (perm[i] + rng.uniform()) / n
        out.append(min(x, 1.0 - V))
    return out


def row(kind, mean, sd, lo, hi, k, u):
    x = closed_form(mean, sd, lo, hi, u)
    print("\t".join([kind] + [repr(v) for v in (mean, sd, lo, hi)] + [str(k), repr(u), repr(float(x))]), flush=True)


def main():
    sys.stdout.reconfigure(newline="\n")
    for k in (0.001, 0.1, 0.5, 1.0, 2.0, 2.5, 3.0, 5.0, 8.0, 26.0):
        a, b = erf_alternating(D(k)), erf_positive(D(k))
        if abs((1 - a) / (1 - b) - 1) > D(10) ** -30:
            sys.exit("the two series differ at %s" % k)
    ks = [-30.0, -8.0, -3.0, -1.0, -0.25, 0.0, 0.25, 1.0, 3.0, 5.0, 7.0, 7.034483685]
    us = [0.0, 2.0 ** -54, 1e-9, 1e-3, 0.1, 0.25, 0.5, 0.75, 0.9, 0.999, 1.0 - 1e-9, 1.0 - V, 1.0]
    for mean, sd in ((0.0, 1.0), (0.1, 0.3), (1013.25, 0.037)):
        for side in (1.0, -1.0):
            for k in ks:
                if k > 7.0 and mean != 0.0:
                    continue
                bound = mean + sd * (side * k)
                lo, hi = (bound, INF) if side > 0 else (-INF, bound)
                for i, u in enumerate(us):
                    row("grid", mean, sd, lo, hi, i, u)
    seed = 792001
    for mean, sd, lo, hi in ((0.1, 0.3, -INF, 0.25), (0.1, 0.3, -0.2, INF), (0.0, 1.0, 7.0, INF), (0.0, 1.0, -INF, -7.0)):
        rng = Rng(seed)
        for i in range(10):
            row("uq-mc", mean, sd, lo, hi, i, rng.centred())
        rng = Rng(seed)
        u = latin_hypercube_1d(5, rng.next_u64()) + latin_hypercube_1d(5, rng.next_u64())
        for i in range(10):
            row("uq-lhs", mean, sd, lo, hi, i, u[i])
        rng = Rng(seed)
        for j in range(3):
            a, b = rng.centred(), rng.centred()
            for i, v in enumerate((a, b, b)):
                row("sobol", mean, sd, lo, hi, 3 * j + i, v)


main()
