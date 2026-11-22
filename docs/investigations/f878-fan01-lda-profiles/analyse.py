"""ADR 0765 (F-878): the comparison of the simulated profiles with the measured LDA profiles, the bands, the Euler work and the
division of the missing pressure rise, by the rules the ADR registered before the runs. Standard library only.

usage: python analyse.py lda_profiles.csv B1.log B2.log B3.log [rust]

The logs are those of `f878_family_run` of crates/rcfd-api/tests/f878_fan01_lda_profiles.rs (lines F878S, F878P, F878H, F878A).
With a fifth argument `rust` the constants of the recorded state are printed in the form of the test file.
"""
import math
import sys

RHO, RPM, R_DUCT, R_HUB = 1.2, 1486.0, 0.250, 0.124
OMEGA = RPM * 2.0 * math.pi / 60.0
Q_DESIGN, DP_MEAS, P_SHAFT = 1.4032, 126.46, 333.30
DP_SIM_0762, TORQUE_0762 = 73.9881, 2.098164
FS, R21 = 1.25, (38184.0 / 13627.0) ** (1.0 / 3.0)
ITER = [7000, 7250, 7500, 7750, 8000]
X_UP, X_LO, X_DOWN, X_HI = 20.0, 88.7, 90.0, 91.3
U_IN = Q_DESIGN / (math.pi * (R_DUCT ** 2 - R_HUB ** 2))


def trapezoid(r, f):
    return sum(0.5 * (2 * math.pi * r[i] * f[i] + 2 * math.pi * r[i + 1] * f[i + 1]) * (r[i + 1] - r[i]) for i in range(len(r) - 1))


def midpoint(r, f):
    return sum(2 * math.pi * ri * fi * 1e-3 for ri, fi in zip(r, f))


def head(r, cx, cxct, ke, rule=trapezoid):
    flow = rule(r, cx)
    power = RHO * OMEGA * rule(r, [ri * f for ri, f in zip(r, cxct)])
    return dict(flow=flow, power=power, e=power / flow, q=RHO * rule(r, ke) / flow)


def read_lda(path):
    rows = [[float(v) for v in l.split(',')] for l in open(path) if not l.startswith('#') and not l.startswith('x_mm')]
    out = {}
    for x in (X_UP, X_DOWN):
        p = [r for r in rows if r[0] == x]
        r = [v[1] * R_DUCT for v in p]
        ux, ut, ur = [v[2] for v in p], [v[3] for v in p], [v[4] for v in p]
        prof = head(r, ux, [a * b for a, b in zip(ux, ut)], [0.5 * a * (a * a + b * b + c * c) for a, b, c in zip(ux, ut, ur)])
        point = head(r, [v[8] for v in p], [v[11] for v in p], [v[12] for v in p])
        out[x] = dict(rn=[v[1] for v in p], r=r, ux=ux, ut=ut, ur=ur, prof=prof, point=point)
    return out


def read_log(path):
    """-> state {iteration: (p_in, torque, dp)}, rings {(iteration, x): [ring]}, annulus {x: [ring]}, result line"""
    state, rings, annulus, result, outside = {}, {}, {}, None, []
    for line in open(path, errors='replace'):
        w = line.split()
        if not w:
            continue
        if w[0] == 'F878S':
            state[int(w[2])] = (float(w[4]), float(w[6]), float(w[8]))
        elif w[0] in ('F878P', 'F878A'):
            keys = ('r', 'fluid', 'cx', 'ct', 'cr', 'cxct', 'ke', 'pcx')
            ring = dict(zip(keys, (float(v) for v in w[4:12])))
            if w[0] == 'F878P':
                rings.setdefault((int(w[2]), float(w[3])), []).append(ring)
            else:
                annulus.setdefault(float(w[3]), []).append(ring)
        elif w[0] == 'F878' and 'RESULT:' in w:
            result = line.strip()
        elif w[0] == 'F878' and 'points not in the fluid' in line:
            outside.append(line.strip())
    return dict(state=state, rings=rings, annulus=annulus, result=result, outside=outside)


def solved(rings, p_in, rule=trapezoid, weighted=False):
    r = [m['r'] for m in rings]
    wt = [m['fluid'] if weighted else 1.0 for m in rings]
    col = lambda f: [w * f(m) for w, m in zip(wt, rings)]
    cx = col(lambda m: m['cx'])
    prof = head(r, cx, col(lambda m: m['cx'] * m['ct']), col(lambda m: 0.5 * m['cx'] * (m['cx'] ** 2 + m['ct'] ** 2 + m['cr'] ** 2)), rule)
    point = head(r, cx, col(lambda m: m['cxct']), col(lambda m: m['ke']), rule)
    pi = RHO * (rule(r, col(lambda m: m['pcx'])) / point['flow'] - p_in - 0.5 * U_IN * U_IN)
    return prof, point, pi


def u_num(f1, f2):
    return FS * abs(f2 - f1) / (R21 - 1.0)


def half_range(v):
    return 0.5 * (max(v) - min(v))


def division(meas, pi_s, u_pi, eq_s, u_eq_s):
    prof, point = meas['prof'], meas['point']
    d = DP_MEAS - DP_SIM_0762
    r_lo, r_hi = DP_MEAS - point['q'] - pi_s, point['e'] - point['q'] - pi_s
    u_lo = u_pi + abs(prof['q'] - point['q'])
    u_data_hi = abs((prof['e'] - prof['q']) - (point['e'] - point['q']))
    u_hi = u_pi + u_data_hi
    verdict = 'Installation' if r_hi + u_hi < 0.5 * d else 'Rotor' if r_lo - u_lo > 0.5 * d else 'CannotBeTold'
    return dict(d=d, r_lo=r_lo, r_hi=r_hi, u_lo=u_lo, u_hi=u_hi, verdict=verdict, r_eq=(point['e'] - point['q']) - eq_s, u_eq=u_eq_s + u_data_hi)


def main():
    lda = read_lda(sys.argv[1])
    logs = {k: read_log(p) for k, p in zip(('B1', 'B2', 'B3'), sys.argv[2:5])}
    rust = len(sys.argv) > 5 and sys.argv[5] == 'rust'
    for k, lg in logs.items():
        print(k, lg['result'])
        for l in lg['outside']:
            print('  ', l)
    m90 = lda[X_DOWN]
    print('\nMeasured, x = 90 mm: profile %s\n                     ensemble %s' % (m90['prof'], m90['point']))
    print('Measured, x = 20 mm: profile %s\n                     ensemble %s' % (lda[X_UP]['prof'], lda[X_UP]['point']))

    b1, b2, b3 = logs['B1'], logs['B2'], logs['B3']
    at = lambda lg, it, x, key: [m[key] for m in lg['rings'][(it, x)]]

    # ---- the profiles at x = 90 mm against the band (C1, C2), and at x = 20 mm (R3)
    record = {}
    for x, name in ((X_DOWN, 'x = 90 mm (C1, C2)'), (X_UP, 'x = 20 mm (R3, report)')):
        print('\nProfiles at %s. Columns: r/R; then for c_x and for c_t: LDA, B1, B2, B3, U_num, S, T, band, B1 - LDA, verdict' % name)
        for key, meas in (('cx', lda[x]['ux']), ('ct', lda[x]['ut'])):
            f1, f2, f3 = at(b1, 8000, x, key), at(b2, 8000, x, key), at(b3, 8000, x, key)
            un = [u_num(a, b) for a, b in zip(f1, f2)]
            s = [half_range([at(b1, it, x, key)[i] for it in ITER]) for i in range(26)]
            t = [half_range([at(b1, 8000, xx, key)[i] for xx in (X_LO, X_DOWN, X_HI)]) for i in range(26)] if x == X_DOWN else [0.0] * 26
            band = [a + b + c for a, b, c in zip(un, s, t)]
            ok = [abs(a - b) <= c for a, b, c in zip(f1, meas, band)]
            if x == X_DOWN:
                record[key] = dict(b1=f1, b2=f2, s=s, t=t, ok=ok, band=band)
            print(' %s:' % key)
            for i in range(26):
                print('  %.3f  %6.2f %7.3f %7.3f %7.3f  %6.3f %6.4f %6.3f  %6.3f  %+7.3f  %s' % (
                    lda[x]['rn'][i], meas[i], f1[i], f2[i], f3[i], un[i], s[i], t[i], band[i], f1[i] - meas[i], 'within' if ok[i] else 'MISS'))
            miss = [i for i in range(26) if not ok[i]]
            print('  %s: %d of 26 within the band; misses at r/R %s; largest distance %.3f m/s; largest excess over the band %.3f m/s' % (
                key, 26 - len(miss), [lda[x]['rn'][i] for i in miss], max(abs(a - b) for a, b in zip(f1, meas)),
                max([abs(f1[i] - meas[i]) - band[i] for i in miss] or [0.0])))
        print(' c_r (report): r/R, LDA, B1')
        print('  ' + '; '.join('%.3f %.2f %.2f' % (lda[x]['rn'][i], lda[x]['ur'][i], at(b1, 8000, x, 'cr')[i]) for i in range(26)))
    print('\nPoints in the fluid, share, B1 at x = 90 mm at the measured radii: min %.4f' % min(at(b1, 8000, X_DOWN, 'fluid')))

    # ---- the scalars of the solve (C3) and their band terms (R2)
    def scal(lg, it, x):
        prof, point, pi = solved(lg['rings'][(it, x)], lg['state'][it][0])
        return dict(prof=prof, point=point, pi=pi, eq=point['e'] - point['q'])

    print('\nEuler work and kinetic energy over the measured span at x = 90 mm (C3); measured shaft power %.2f W = %.2f Pa per design flow' % (P_SHAFT, P_SHAFT / Q_DESIGN))
    print('  measured: flow %.4f / %.4f m^3/s, P_E %.2f / %.2f W, e %.2f / %.2f Pa, q %.2f / %.2f Pa (profile / ensemble)' % (
        m90['prof']['flow'], m90['point']['flow'], m90['prof']['power'], m90['point']['power'], m90['prof']['e'], m90['point']['e'], m90['prof']['q'], m90['point']['q']))
    sc = {}
    for k, lg in logs.items():
        s = sc[k] = scal(lg, 8000, X_DOWN)
        up = scal(lg, 8000, X_UP)
        st = lg['state'][8000]
        print('  %s: flow %.4f m^3/s, P_E %.2f / %.2f W, e %.2f / %.2f Pa, q %.2f / %.2f Pa (profile / pointwise), Pi_s %.2f Pa; torque x omega %.2f W, dp_ts %.4f Pa; '
              'L_s = e - q - Pi %.2f Pa, Rec_s = dp_ts - Pi %.2f Pa; x = 20 mm: P_E %.2f W (should be 0), flow %.4f' % (
                  k, s['point']['flow'], s['prof']['power'], s['point']['power'], s['prof']['e'], s['point']['e'], s['prof']['q'], s['point']['q'], s['pi'],
                  st[1] * OMEGA, st[2], s['eq'] - s['pi'], st[2] - s['pi'], up['point']['power'], up['point']['flow']))
    # whole annulus (V5, u_cov)
    ann = {}
    for k, lg in logs.items():
        prof, point, pi = solved(lg['annulus'][X_DOWN], lg['state'][8000][0], midpoint, True)
        ann[k] = dict(prof=prof, point=point, pi=pi, eq=point['e'] - point['q'])
        pu, ptu, _ = solved(lg['annulus'][X_UP], lg['state'][8000][0], midpoint, True)
        st = lg['state'][8000]
        print('  %s whole annulus (V5): x = 90: flow %.4f m^3/s (%.2f %% of %.4f), P_E %.2f / %.2f W against torque x omega %.2f W (%.1f %% / %.1f %%), e %.2f, q %.2f, Pi_s %.2f Pa; '
              'x = 20: flow %.4f (%.2f %%), P_E %.2f W; least share of fluid points on a ring at x = 90: %.3f' % (
                  k, point['flow'], 100 * (point['flow'] / Q_DESIGN - 1), Q_DESIGN, prof['power'], point['power'], st[1] * OMEGA,
                  100 * (prof['power'] / (st[1] * OMEGA) - 1), 100 * (point['power'] / (st[1] * OMEGA) - 1), point['e'], point['q'], pi,
                  ptu['flow'], 100 * (ptu['flow'] / Q_DESIGN - 1), ptu['power'], min(m['fluid'] for m in lg['annulus'][X_DOWN])))

    terms = {}
    for key in ('pi', 'eq'):
        s = half_range([scal(b1, it, X_DOWN)[key] for it in ITER])
        t = half_range([scal(b1, 8000, xx)[key] for xx in (X_LO, X_DOWN, X_HI)])
        cov = abs(ann['B1'][key] - sc['B1'][key])
        un = u_num(sc['B1'][key], sc['B2'][key])
        terms[key] = dict(un=un, s=s, t=t, cov=cov, total=un + s + t + cov)
        print('  band terms of %s (R2): B1 %.3f, B2 %.3f, B3 %.3f Pa; U_num %.3f, S %.4f, T %.3f, u_cov %.3f: sum %.3f Pa' % (
            'Pi_s' if key == 'pi' else 'e_s - q_s', sc['B1'][key], sc['B2'][key], sc['B3'][key], un, s, t, cov, terms[key]['total']))

    # ---- the division (C4)
    d = division(m90, sc['B1']['pi'], terms['pi']['total'], sc['B1']['eq'], terms['eq']['total'])
    print('\nThe division (C4): D = %.2f Pa, D/2 = %.2f Pa' % (d['d'], 0.5 * d['d']))
    print('  rotor share between R_lo = %.2f Pa (band %.2f) and R_hi = %.2f Pa (band %.2f): with the band [%.2f, %.2f] Pa; installation share I = D - R in [%.2f, %.2f] Pa' % (
        d['r_lo'], d['u_lo'], d['r_hi'], d['u_hi'], d['r_lo'] - d['u_lo'], d['r_hi'] + d['u_hi'], d['d'] - d['r_hi'] - d['u_hi'], d['d'] - d['r_lo'] + d['u_lo']))
    print('  verdict: %s' % d['verdict'])
    print('  conditional (L_m = L_s): R_eq = %.2f Pa, I_eq = %.2f Pa, band %.2f Pa' % (d['r_eq'], d['d'] - d['r_eq'], d['u_eq']))
    d0 = division(m90, sc['B1']['pi'], 0.0, sc['B1']['eq'], 0.0)
    d2 = division(m90, sc['B2']['pi'], terms['pi']['total'], sc['B2']['eq'], terms['eq']['total'])
    print('  R2: verdict without the band terms of the solve: %s; with B2 in the place of B1: %s (R_lo %.2f, R_hi %.2f, R_eq %.2f Pa)' % (
        d0['verdict'], d2['verdict'], d2['r_lo'], d2['r_hi'], d2['r_eq']))
    q_m, e_m = m90['point']['q'], m90['point']['e']
    rec_s = DP_SIM_0762 - sc['B1']['pi']
    print('  report: Rec_m - L_m = %.2f Pa; bound I <= q_m - Rec_s = %.2f Pa; L_s %.2f Pa, Rec_s %.2f Pa; e_m - e_s %.2f Pa, q_m - q_s %.2f Pa' % (
        DP_MEAS - (e_m - q_m), q_m - rec_s, sc['B1']['eq'] - sc['B1']['pi'], rec_s, e_m - sc['B1']['point']['e'], q_m - sc['B1']['point']['q']))

    if rust:
        arr = lambda name, v, f='%.6f': 'const %s: [f64; %d] = [%s];' % (name, len(v), ', '.join(f % a for a in v))
        four = lambda s: [s['point']['e'], s['point']['q'], s['pi'], s['point']['flow']]
        print()
        print(arr('B1_SCALARS', four(sc['B1'])))
        print(arr('B2_SCALARS', four(sc['B2'])))
        print(arr('S_T_COV_PI', [terms['pi'][k] for k in ('s', 't', 'cov')]))
        print(arr('S_T_COV_EQ', [terms['eq'][k] for k in ('s', 't', 'cov')]))
        for key, tag in (('cx', 'CX'), ('ct', 'CT')):
            print(arr('B1_' + tag, record[key]['b1']))
        for key, tag in (('cx', 'CX'), ('ct', 'CT')):
            print(arr('B2_' + tag, record[key]['b2']))
        for key, tag in (('cx', 'CX'), ('ct', 'CT')):
            print(arr('S_' + tag, record[key]['s']))
        for key, tag in (('cx', 'CX'), ('ct', 'CT')):
            print(arr('T_' + tag, record[key]['t']))
        for key, tag in (('cx', 'CX'), ('ct', 'CT')):
            print('const WITHIN_%s_RECORD: [bool; 26] = [%s];' % (tag, ', '.join('true' if b else 'false' for b in record[key]['ok'])))
        print('const VERDICT_RECORD: Verdict = Verdict::%s;' % d['verdict'])
        print(arr('DIVISION_RECORD', [d['r_lo'], d['r_hi'], d['u_lo'], d['u_hi'], d['r_eq'], d['u_eq']], '%.2f'))


main()
