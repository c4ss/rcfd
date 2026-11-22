"""ADR 0769 (F-883): the reading of the casing pressure at x = 90 mm and the identity of ADR 0765 with it.

Usage: python -I wall_pressure_share.py <kulite_means.csv> <lda_profiles.csv of ADR 0765>
Every rule is the one registered in ADR 0769; the output is analysis.txt beside this file.
"""
import csv
import math
import sys

RHO, R_WALL = 1.2, 0.250
P_TS, DP_S = 126.46, 73.99           # measured pressure rise; B1 pressure rise (ADR 0762), as ADR 0765 forms D
D = P_TS - DP_S                      # 52.47 Pa
PI_S, U_S = 61.709, 21.683           # B1 Pi_s and its band (ADR 0765, analysis.txt: U_num + S + T + u_cov)
EM_QM, QM = 120.819, 99.196          # e_m - q_m and q_m, pointwise (ADR 0765)


def table(path):
    lines = [l for l in open(path, encoding='utf-8') if not l.startswith('#')]
    return list(csv.DictReader(lines))


def batch_se(b):
    m = sum(b) / len(b)
    return math.sqrt(sum((x - m) ** 2 for x in b) / (len(b) - 1) / len(b))


def wall_at(rows, lo, w_lo):
    """value and 2 s.e. of w_lo p_lo + (1 - w_lo) p_hi, the batch means combined the same way."""
    a, b = rows[lo], rows[lo + 1]
    mean = w_lo * float(a['mean']) + (1 - w_lo) * float(b['mean'])
    bm = [w_lo * float(a['b%02d' % k]) + (1 - w_lo) * float(b['b%02d' % k]) for k in range(1, 31)]
    return mean, 2 * batch_se(bm), 0.13 * abs(float(b['mean']) - float(a['mean']))


def correction(lda, full):
    d = [x for x in lda if x['x_mm'] == '90']
    r = [float(x['radius_norm']) * R_WALL for x in d]
    ux = [float(x['ux_mean']) for x in d]
    sq = lambda m, s: float(m) ** 2 + (float(s) ** 2 if full else 0.0)
    ct2 = [sq(x['uphi_mean'], x['uphi_rms']) for x in d]
    cr2 = [sq(x['ur_mean'], x['ur_rms']) for x in d]
    f = [RHO * (a - b) / rr for a, b, rr in zip(ct2, cr2, r)]
    end = 0.5 * f[-1] * (R_WALL - r[-1])
    tz = lambda y: sum(0.5 * (y[j] + y[j + 1]) * (r[j + 1] - r[j]) for j in range(len(r) - 1))
    g = [sum(0.5 * (f[j] + f[j + 1]) * (r[j + 1] - r[j]) for j in range(i, len(r) - 1)) + end + RHO * cr2[i]
         for i in range(len(r))]
    w = [u * 2 * math.pi * rr for u, rr in zip(ux, r)]
    return tz([a * b for a, b in zip(g, w)]) / tz(w), end


def main():
    km, lda = table(sys.argv[1]), table(sys.argv[2])
    rows = {int(x['row']): x for x in km}
    out = []
    p = lambda *a: out.append(' '.join(str(x) for x in a))
    p('ADR 0769 (F-883): wall pressure of the FAN-01 record and the identity of ADR 0765')
    p('Means of kulite_pressure_in_Pa (against the chamber), Pa:')
    for i in range(15):
        x = rows[i]
        p('  row %2d x = %5s mm: mean %10.4f  2 s.e. %.4f  std %9.4f  rms %9.4f' % (
            i, x['x_mm'] or '-', float(x['mean']), 2 * batch_se([float(x['b%02d' % k]) for k in range(1, 31)]),
            float(x['std']), float(x['rms'])))
    # K1
    std = [float(rows[i]['std']) for i in range(1, 15)]
    rms = [float(rows[i]['rms']) for i in range(1, 15)]
    k1_std = round(float(rows[0]['std']), 2) == 0.16 and round(min(std)) == 11 and round(max(std)) == 184
    k1_rms = round(float(rows[0]['rms']), 2) == 0.16 and round(min(rms)) == 11 and round(max(rms)) == 184
    k1 = abs(float(rows[0]['mean'])) < 0.5 and (k1_std or k1_rms)
    p('K1: row 0 mean %.4f, std %.4f, rms %.4f; rows 1-14 std %.2f..%.2f, rms %.2f..%.2f; met %s (reading: %s)' % (
        float(rows[0]['mean']), float(rows[0]['std']), float(rows[0]['rms']), min(std), max(std), min(rms), max(rms),
        k1, 'std about the mean' if k1_std else ('root mean square' if k1_rms else 'neither')))
    k2 = all(float(rows[i]['mean']) <= 2 * batch_se([float(rows[i]['b%02d' % k]) for k in range(1, 31)])
             for i in range(1, 6))
    p('K2: rows 1-5 means %s; each <= u_stat: %s' % ([round(float(rows[i]['mean']), 3) for i in range(1, 6)], k2))
    k3 = float(rows[1]['mean']) > -P_TS
    p('K3: row 1 mean %.4f > -126.46: %s' % (float(rows[1]['mean']), k3))
    # the station: rows 12 and 13 at x = 87 and 97 mm; x = 90 -> weight 0.7 on row 12
    pw, ustat, ux = wall_at(rows, 12, 0.7)
    p('p_w(90) = %.4f Pa, u_stat %.4f, u_x %.4f' % (pw, ustat, ux))
    for lo, wl, what in ((11, 0.7, 'row n at x = 10 n - 23 mm (rows 11 and 12 enclose x = 90)'),
                         (13, 0.7, 'row n at x = 10 n - 43 mm (rows 13 and 14 enclose x = 90)')):
        p('  reported, not banded: %s: %.4f Pa' % (what, wall_at(rows, lo, wl)[0]))
    gbar, end = correction(lda, True)
    gbar0, _ = correction(lda, False)
    uread = abs(gbar - gbar0)
    pim = pw - gbar
    um = ustat + ux + end + uread
    p('Gbar %.4f Pa (means alone %.4f), u_gap %.4f, u_read %.4f, u_meas 0 (not stated)' % (gbar, gbar0, end, uread))
    p('Pi_m = %.4f Pa, U_m = %.4f Pa' % (pim, um))
    rr = pim - PI_S
    u = um + U_S
    ii = D - rr
    if rr + u < D / 2:
        verdict = 'Installation'
    elif rr - u > D / 2:
        verdict = 'Rotor'
    else:
        verdict = 'CannotBeTold'
    b1 = 27.264 - um <= pim <= EM_QM + um
    p('R = %.4f Pa, I = %.4f Pa, U = %.4f Pa; R - U %.4f, R + U %.4f against D/2 %.4f: verdict %s' % (
        rr, ii, u, rr - u, rr + u, D / 2, verdict))
    p('L_m = %.4f Pa, Rec_m = %.4f Pa; B1 (27.264 - U_m <= Pi_m <= 120.819 + U_m): %s' % (EM_QM - pim, P_TS - pim, b1))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
