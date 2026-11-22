"""ADR 0765 (F-878): the measured LDA profiles of the FAN-01 record (Zenodo doi:10.5281/zenodo.23943494, v6, lda_data.zip, CC BY 4.0) and
the integrals of the registration, from the measured data alone. Standard library only.

usage: python -I lda_measured.py <folder of mat5.py> <folder lda_data of the unpacked archive> [csv to write]
"""
import math
import sys

sys.path.insert(0, sys.argv[1])
import mat5  # noqa: E402

ROOT = sys.argv[2]
RHO, RPM, R_DUCT = 1.2, 1486.0, 0.250
OMEGA = RPM * 2.0 * math.pi / 60.0
Q_DESIGN, DP_MEAS, P_SHAFT = 1.4032, 126.46, 333.30


def trapz(r, f):
    return sum(0.5 * (f[i] + f[i + 1]) * (r[i + 1] - r[i]) for i in range(len(r) - 1))


def time_averaged(side):
    d = mat5.read('%s/time_averaged/n1ug_1_4_time_averaged_%s_side.mat' % (ROOT, side))
    g = lambda k: d[k][1]
    return dict(rn=g('radius_norm'), x=g('x_in_mm'), cx=g('ux_mean_in_m_per_s'), ct=g('uphi_mean_in_m_per_s'), cr=g('ur_mean_in_m_per_s'),
                sx=g('ux_rms_in_m_per_s'), st=g('uphi_rms_in_m_per_s'), sr=g('ur_rms_in_m_per_s'))


def ensemble(side):
    d = mat5.read('%s/ensemble_averaged/n1ug_1_4_ensemble_averaged_%s_side.mat' % (ROOT, side))
    g = lambda k: d[k][1]
    rn, cx, ct, cr = g('radius_norm'), g('ux_mean_in_m_per_s'), g('uphi_mean_in_m_per_s'), g('ur_mean_in_m_per_s')
    assert len(rn) == 26 * 180
    out = dict(rn=[], cx=[], ct=[], cr=[], cxct=[], ke=[])
    for i in range(26):
        s = slice(i * 180, (i + 1) * 180)
        assert max(rn[s]) == min(rn[s]), 'not radius-major'
        a, b, c = cx[s], ct[s], cr[s]
        out['rn'].append(rn[s][0])
        out['cx'].append(sum(a) / 180)
        out['ct'].append(sum(b) / 180)
        out['cr'].append(sum(c) / 180)
        out['cxct'].append(sum(u * v for u, v in zip(a, b)) / 180)  # bin mean of the product
        out['ke'].append(sum(0.5 * (u * u + v * v + w * w) * u for u, v, w in zip(a, b, c)) / 180)
    return out


def integrals(rn, cx, cxct, ke):
    """covered span, trapezoid on the measured radii: flow [m^3/s], angular-momentum flux / rho [m^5/s^2], kinetic-energy flux / rho"""
    r = [v * R_DUCT for v in rn]
    q = trapz(r, [2 * math.pi * r[i] * cx[i] for i in range(26)])
    a = trapz(r, [2 * math.pi * r[i] * r[i] * cxct[i] for i in range(26)])
    k = trapz(r, [2 * math.pi * r[i] * ke[i] for i in range(26)])
    return q, a, k


def main():
    res = {}
    rows = []
    for side, x in (('suction', 20), ('pressure', 90)):
        t, e = time_averaged(side), ensemble(side)
        assert all(abs(v - x) < 1e-12 for v in t['x'])
        assert all(abs(a - b) < 1e-12 for a, b in zip(t['rn'], e['rn']))
        dmax = [max(abs(a - b) for a, b in zip(t[k], e[k])) for k in ('cx', 'ct', 'cr')]
        print('x = %d mm: largest difference of the bin mean of the ensemble file from the time-averaged file: cx %.3f ct %.3f cr %.3f m/s' % (x, *dmax))
        tp = integrals(t['rn'], t['cx'], [a * b for a, b in zip(t['cx'], t['ct'])],
                       [0.5 * (a * a + b * b + c * c) * a for a, b, c in zip(t['cx'], t['ct'], t['cr'])])
        te = integrals(e['rn'], e['cx'], e['cxct'], e['ke'])
        res[x] = (tp, te)
        for i in range(26):
            rows.append((x, t['rn'][i], t['cx'][i], t['ct'][i], t['cr'][i], t['sx'][i], t['st'][i], t['sr'][i], e['cx'][i], e['ct'][i], e['cr'][i], e['cxct'][i], e['ke'][i]))
    for name, j in (('profiles (product of the time means)', 0), ('ensemble (bin mean of the products)', 1)):
        (q20, a20, k20), (q90, a90, k90) = res[20][j], res[90][j]
        p_e = RHO * OMEGA * (a90 - a20)
        e_head = RHO * OMEGA * (a90 / q90 - a20 / q20)
        print('%s:\n  covered flow x=20 %.4f, x=90 %.4f m^3/s (design %.4f; covered area share %.4f)' % (name, q20, q90, Q_DESIGN, (0.246**2 - 0.133**2) / (0.25**2 - 0.124**2)))
        print('  Euler power rho omega (A90 - A20) = %.2f W (A90 part %.2f W, A20 part %.2f W); measured shaft power %.2f W' % (p_e, RHO * OMEGA * a90, RHO * OMEGA * a20, P_SHAFT))
        print('  Euler head e = %.2f Pa (x=90 part %.2f, x=20 part %.2f); shaft power / design flow %.2f Pa' % (e_head, RHO * OMEGA * a90 / q90, RHO * OMEGA * a20 / q20, P_SHAFT / Q_DESIGN))
        print('  kinetic energy flux per flow q: x=20 %.2f Pa, x=90 %.2f Pa' % (RHO * k20 / q20, RHO * k90 / q90))
    if len(sys.argv) > 3:
        with open(sys.argv[3], 'w', newline='\n') as f:
            f.write('# FAN-01 LDA profiles at the design point (1.4 m^3/s), transcribed for ADR 0765 (F-878) from lda_data.zip of the Zenodo record\n')
            f.write('# "FAN-01 Benchmark: Low-pressure axial fan in a short duct" (v6), doi:10.5281/zenodo.23943494 (concept doi:10.5281/zenodo.8418340),\n')
            f.write('# Kroemer, Junger, Czwielong, Heidegger, Kaltenbacher, Schoder, Becker, licence CC BY 4.0. Archive md5 4cafb1602e81dc18f1681e6facd495a0,\n')
            f.write('# sha256 3c527f49b02e9e1b80718915a79468381e927dfb8239ba68f7a33d575df4f707, fetched 2026-11-22. x_mm from the upstream hub face; r = radius_norm * 250 mm.\n')
            f.write('# Columns 3 to 8: the time-averaged files as stored (ux = meridional/axial, uphi = circumferential, ur = radial; mean and rms, m/s).\n')
            f.write('# Columns 9 to 13: DERIVED here from the ensemble-averaged files (180 rotor-angle bins per radius): bin means, the bin mean of ux*uphi\n')
            f.write('# [m^2/s^2] and the bin mean of ux*(ux^2+uphi^2+ur^2)/2 [m^3/s^3]. Written by lda_measured.py.\n')
            f.write('x_mm,radius_norm,ux_mean,uphi_mean,ur_mean,ux_rms,uphi_rms,ur_rms,ens_ux,ens_uphi,ens_ur,ens_ux_uphi,ens_ke_flux\n')
            for r in rows:
                f.write('%d,%.3f,%.2f,%.2f,%.2f,%.2f,%.2f,%.2f,%.6f,%.6f,%.6f,%.6f,%.6f\n' % r)


main()
