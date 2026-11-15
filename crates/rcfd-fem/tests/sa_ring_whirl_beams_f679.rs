//! F-679 (ADR 0602): an independent whirl reference for spinning beams. A free circular ring of Beam2 spinning about its axis,
//! on a cyclic sector with `CyclicOptions::beams`, its in-plane and out-of-plane forward and backward whirl from
//! `RotatingSector::whirl_modes` at nodal diameters 2 and 3, against the exact per-nodal-diameter solution of the spinning
//! Timoshenko ring in the rotating frame (D3, a 3 x 3 Hermitian gyroscopic problem per family, solved here by bisection of its
//! determinant). Checks RF1 to RF3, PR, WF, OD, SP, KA, DI, TR, AG and the negative controls NC0, NC1 of the registration (B1 and
//! B2 are the diff and `baseline_hashes`).

use rcfd_fem::geometric::Prestress;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingSector, Whirl, WhirlMode};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel,
};
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const R: f64 = 0.5;
/// Section width along the ring's axis (`e_2`) and height along the radius (`e_3`), m.
const BW: f64 = 0.03;
const HH: f64 = 0.02;
const J_T: f64 = 4.7e-8;
const KSH: f64 = 5.0 / 6.0;
const N: usize = 24;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// The two meshes of D2: elements per sector.
const MESHES: [usize; 2] = [2, 4];
const HARMONICS: [usize; 2] = [2, 3];
const EPS: f64 = f64::EPSILON;

fn area() -> f64 {
    BW * HH
}
fn i_y() -> f64 {
    BW * HH * HH * HH / 12.0
}
fn i_z() -> f64 {
    HH * BW * BW * BW / 12.0
}
fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

// ------------------------------------------------------------------------------------------- the reference (D3)

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cx {
    re: f64,
    im: f64,
}

impl Cx {
    const ZERO: Cx = Cx { re: 0.0, im: 0.0 };
    fn new(re: f64, im: f64) -> Cx {
        Cx { re, im }
    }
    fn re(re: f64) -> Cx {
        Cx { re, im: 0.0 }
    }
    fn add(self, o: Cx) -> Cx {
        Cx::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    fn scale(self, s: f64) -> Cx {
        Cx::new(s * self.re, s * self.im)
    }
    fn conj(self) -> Cx {
        Cx::new(self.re, -self.im)
    }
    fn abs2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

/// The section of the reference (scaled by `t` for the thin limit RF2): `(A, I_y, I_z, J)`.
#[derive(Clone, Copy)]
struct Sec {
    a: f64,
    iy: f64,
    iz: f64,
    j: f64,
}

fn sec(t: f64) -> Sec {
    Sec {
        a: area() * t * t,
        iy: i_y() * t.powi(4),
        iz: i_z() * t.powi(4),
        j: J_T * t.powi(4),
    }
}

type M6 = [[Cx; 6]; 6];

/// D3.2: per unit length, `K`, `K_sigma` (1 rad/s), `M`, `M_perp` (Hermitian) and `C` (real skew) of the harmonic `e^{i n phi}`,
/// fields `(u_1, u_2, u_3, theta_1, theta_2, theta_3)` in the moving frame `(e_phi, a, e_r)`.
fn ring_forms(n: usize, s: Sec) -> (M6, M6, [f64; 6], [f64; 6], [[f64; 6]; 6]) {
    let d = Cx::new(0.0, n as f64 / R);
    let lin = |terms: &[(Cx, usize)]| -> [Cx; 6] {
        let mut v = [Cx::ZERO; 6];
        for &(c, i) in terms {
            v[i] = v[i].add(c);
        }
        v
    };
    let inv_r = Cx::re(1.0 / R);
    let m_inv_r = Cx::re(-1.0 / R);
    let one = Cx::re(1.0);
    let m_one = Cx::re(-1.0);
    // (du/ds)_k and (dtheta/ds)_k.
    let du1 = lin(&[(d, 0), (inv_r, 2)]);
    let du2 = lin(&[(d, 1)]);
    let du3 = lin(&[(d, 2), (m_inv_r, 0)]);
    let dt1 = lin(&[(d, 3), (inv_r, 5)]);
    let dt2 = lin(&[(d, 4)]);
    let dt3 = lin(&[(d, 5), (m_inv_r, 3)]);
    let g2 = lin(&[(d, 1), (m_one, 5)]);
    let g3 = lin(&[(d, 2), (m_inv_r, 0), (one, 4)]);
    let form = |rows: &[([Cx; 6], f64)]| -> M6 {
        let mut k = [[Cx::ZERO; 6]; 6];
        for (r, w) in rows {
            for i in 0..6 {
                for j in 0..6 {
                    k[i][j] = k[i][j].add(r[i].conj().mul(r[j]).scale(*w));
                }
            }
        }
        k
    };
    let g = g_mod();
    let k = form(&[
        (du1, E * s.a),
        (g2, KSH * g * s.a),
        (g3, KSH * g * s.a),
        (dt1, g * s.j),
        (dt2, E * s.iy),
        (dt3, E * s.iz),
    ]);
    let n1 = RHO * s.a * R * R;
    let ks = form(&[(du1, n1), (du2, n1), (du3, n1), (dt1, n1 * (s.iy + s.iz) / s.a)]);
    let m = [
        RHO * s.a,
        RHO * s.a,
        RHO * s.a,
        RHO * (s.iy + s.iz),
        RHO * s.iy,
        RHO * s.iz,
    ];
    let mp = [RHO * s.a, 0.0, RHO * s.a, RHO * s.iz, RHO * s.iy, RHO * s.iz];
    let mut c = [[0.0; 6]; 6];
    c[0][2] = RHO * s.a;
    c[2][0] = -RHO * s.a;
    c[3][5] = RHO * s.iz;
    c[5][3] = -RHO * s.iz;
    (k, ks, m, mp, c)
}

const IN_PLANE: [usize; 3] = [0, 2, 4];
const OUT_PLANE: [usize; 3] = [1, 3, 5];

type M3 = [[Cx; 3]; 3];

struct Family {
    ke: M3,
    m: [f64; 3],
    c: [[f64; 3]; 3],
    omega: f64,
}

fn family(n: usize, omega: f64, fam: [usize; 3], stiff: bool, s: Sec) -> Family {
    let (k, ks, m, mp, c) = ring_forms(n, s);
    let sf = if stiff { 1.0 } else { 0.0 };
    let w2 = omega * omega;
    let mut ke = [[Cx::ZERO; 3]; 3];
    let mut cf = [[0.0; 3]; 3];
    let mut mf = [0.0; 3];
    for (a, &i) in fam.iter().enumerate() {
        mf[a] = m[i];
        for (b, &j) in fam.iter().enumerate() {
            let mut v = k[i][j].add(ks[i][j].scale(w2 * sf));
            if i == j {
                v = v.sub(Cx::re(w2 * mp[i]));
            }
            ke[a][b] = v;
            cf[a][b] = c[i][j];
        }
    }
    Family {
        ke,
        m: mf,
        c: cf,
        omega,
    }
}

/// `H(lambda) = K_e - lambda^2 M + i lambda 2 Omega C`.
fn h_of(f: &Family, lambda: f64) -> M3 {
    let mut h = f.ke;
    for a in 0..3 {
        h[a][a] = h[a][a].sub(Cx::re(lambda * lambda * f.m[a]));
        for b in 0..3 {
            h[a][b] = h[a][b].add(Cx::new(0.0, lambda * 2.0 * f.omega * f.c[a][b]));
        }
    }
    h
}

fn det3(h: &M3) -> f64 {
    let m = |a: Cx, b: Cx| a.mul(b);
    let t1 = m(h[0][0], m(h[1][1], h[2][2]).sub(m(h[1][2], h[2][1])));
    let t2 = m(h[0][1], m(h[1][0], h[2][2]).sub(m(h[1][2], h[2][0])));
    let t3 = m(h[0][2], m(h[1][0], h[2][1]).sub(m(h[1][1], h[2][0])));
    t1.sub(t2).add(t3).re
}

fn cross_c(a: [Cx; 3], b: [Cx; 3]) -> [Cx; 3] {
    [
        a[1].mul(b[2]).sub(a[2].mul(b[1])),
        a[2].mul(b[0]).sub(a[0].mul(b[2])),
        a[0].mul(b[1]).sub(a[1].mul(b[0])),
    ]
}

/// The first root of `det H` on the side `sign` (`+1` backward, `-1` forward), its magnitude and the library's whirl number.
fn first_root(f: &Family, sign: f64, step: f64) -> (f64, f64) {
    let d0 = det3(&h_of(f, 0.0));
    assert!(d0 > 0.0, "det K_e = {d0:e} is not positive");
    let mut lo = 0.0;
    let mut k = 1usize;
    let hi = loop {
        let x = k as f64 * step;
        if det3(&h_of(f, sign * x)) <= 0.0 {
            break x;
        }
        lo = x;
        k += 1;
        assert!(k < 10_000_000, "no root found");
    };
    let (mut a, mut b) = (lo, hi);
    loop {
        let mid = 0.5 * (a + b);
        if mid <= a || mid >= b {
            break;
        }
        if det3(&h_of(f, sign * mid)) > 0.0 {
            a = mid;
        } else {
            b = mid;
        }
    }
    let x = 0.5 * (a + b);
    let h = h_of(f, sign * x);
    let cands = [cross_c(h[0], h[1]), cross_c(h[0], h[2]), cross_c(h[1], h[2])];
    let q = *cands
        .iter()
        .max_by(|p, r| {
            let np: f64 = p.iter().map(|v| v.abs2()).sum();
            let nr: f64 = r.iter().map(|v| v.abs2()).sum();
            np.partial_cmp(&nr).unwrap()
        })
        .unwrap();
    let mut qcq = Cx::ZERO;
    let mut qmq = 0.0;
    for a in 0..3 {
        qmq += f.m[a] * q[a].abs2();
        for b in 0..3 {
            qcq = qcq.add(q[a].conj().mul(q[b]).scale(f.c[a][b]));
        }
    }
    // D3.4: kappa_b = +Im(Q^H C Q) / Q^H M Q, kappa_f = -Im(Q^H C Q) / Q^H M Q.
    let kappa = sign * qcq.im / qmq;
    (x, kappa)
}

/// The reference pair of a family: `(omega_f, omega_b, kappa_f, kappa_b)`.
fn ref_pair(n: usize, omega: f64, fam: [usize; 3], stiff: bool, s: Sec) -> (f64, f64, f64, f64) {
    let f = family(n, omega, fam, stiff, s);
    let scale = (E * s.iy / (RHO * s.a * R.powi(4))).sqrt();
    let step = scale / 1000.0;
    let (wf, kf) = first_root(&f, -1.0, step);
    let (wb, kb) = first_root(&f, 1.0, step);
    (wf, wb, kf, kb)
}

fn omega_1() -> f64 {
    let mut w = f64::INFINITY;
    for n in 2..=5 {
        for fam in [IN_PLANE, OUT_PLANE] {
            w = w.min(ref_pair(n, 0.0, fam, true, sec(1.0)).0);
        }
    }
    w
}

fn speeds(w1: f64) -> [f64; 3] {
    [1.0, 2.0, 3.0].map(|j| j * w1 / 6.0)
}

/// D3.3: the thin inextensional in-plane ring in the rotating frame.
fn thin_in_plane(n: usize, omega: f64, s: Sec) -> (f64, f64) {
    let nf = n as f64;
    let w02 = E * s.iy * nf * nf * (nf * nf - 1.0).powi(2) / (RHO * s.a * R.powi(4) * (nf * nf + 1.0));
    let root = ((1.0 + nf * nf).powi(2) * w02 + nf * nf * (nf * nf - 1.0).powi(2) * omega * omega).sqrt();
    (
        (root - 2.0 * nf * omega) / (1.0 + nf * nf),
        (root + 2.0 * nf * omega) / (1.0 + nf * nf),
    )
}

/// D3.3: the classical out-of-plane ring at rest (cited, checked as the thin limit).
fn thin_out_of_plane(n: usize, s: Sec) -> f64 {
    let nf = n as f64;
    (E * s.iz * nf * nf * (nf * nf - 1.0).powi(2) / (RHO * s.a * R.powi(4) * (nf * nf + E * s.iz / (g_mod() * s.j))))
        .sqrt()
}

/// Arithmetic 1: set (`true` = F), `j`, `n`, in-plane `(f, b, kappa_f, kappa_b)`, out-of-plane `(f, b, kappa_f, kappa_b)`.
type RefRow = (bool, usize, usize, [f64; 4], [f64; 4]);
const REF_TABLE: [RefRow; 12] = [
    (
        false,
        1,
        2,
        [269.204298684032, 352.531499597897, 0.8001548, -0.8000634],
        [410.767082988726, 411.034577829776, 2.568876e-3, -2.568104e-3],
    ),
    (
        false,
        1,
        3,
        [850.223792368553, 912.752862883168, 0.6005203, -0.6002899],
        [1230.943439746718, 1231.449944582607, 4.864924e-3, -4.862010e-3],
    ),
    (
        false,
        2,
        2,
        [222.794891003891, 389.445463492684, 0.8001807, -0.8000006],
        [410.607868193025, 411.142874238668, 2.569340e-3, -2.567796e-3],
    ),
    (
        false,
        2,
        3,
        [815.995801011409, 941.051385989798, 0.6006223, -0.6001632],
        [1230.674013215345, 1231.687040338849, 4.866465e-3, -4.860637e-3],
    ),
    (
        false,
        3,
        2,
        [173.055752657706, 423.022037357161, 0.8001915, -0.7999285],
        [410.431658892268, 411.234208872855, 2.569857e-3, -2.567541e-3],
    ),
    (
        false,
        3,
        3,
        [779.771634045630, 967.348621613690, 0.6007147, -0.6000299],
        [1230.393791281521, 1231.913375598148, 4.868062e-3, -4.859319e-3],
    ),
    (
        true,
        1,
        2,
        [276.949899625503, 360.278694082952, 0.8001712, -0.8000775],
        [423.726183298182, 423.993685518972, 2.568959e-3, -2.568162e-3],
    ),
    (
        true,
        1,
        3,
        [859.995136358259, 922.526329372697, 0.6005419, -0.6003090],
        [1240.769518818326, 1241.276037663302, 4.865071e-3, -4.862133e-3],
    ),
    (
        true,
        2,
        2,
        [253.140917571968, 419.804237434476, 0.8002508, -0.8000530],
        [460.264295641277, 460.799360726951, 2.569717e-3, -2.567986e-3],
    ),
    (
        true,
        2,
        3,
        [854.577229582325, 979.649792988307, 0.6007139, -0.6002347],
        [1269.522730521359, 1270.535869719386, 4.867095e-3, -4.861083e-3],
    ),
    (
        true,
        3,
        2,
        [239.288208400434, 489.297510006706, 0.8003583, -0.8000371],
        [515.489496260866, 516.292245512028, 2.570791e-3, -2.567883e-3],
    ),
    (
        true,
        3,
        3,
        [864.831854926126, 1052.466135853155, 0.6009312, -0.6001801],
        [1316.194508499327, 1317.714471078663, 4.869578e-3, -4.860225e-3],
    ),
];
const OMEGA_1_TABLE: f64 = 312.434403687127;

/// Arithmetic 4: the polygon model's four lowest `|lambda|` on the coarse mesh, `(set F, j, k, values)`.
const AG_TABLE: [(bool, usize, usize, [f64; 4]); 12] = [
    (
        false,
        1,
        2,
        [269.877221329041, 353.204884278410, 411.444733264837, 411.712727520844],
    ),
    (
        false,
        1,
        3,
        [852.133234015595, 914.665141646339, 1232.703398668859, 1233.210615539384],
    ),
    (
        false,
        2,
        2,
        [223.478093166842, 390.129622237774, 411.285265493855, 411.821270205628],
    ),
    (
        false,
        2,
        3,
        [817.910490572004, 942.971789402309, 1232.433616368229, 1233.448067226008],
    ),
    (
        false,
        3,
        2,
        [173.757361015913, 411.108801175707, 411.912848746281, 423.725161879525],
    ),
    (
        false,
        3,
        3,
        [781.696137103931, 969.281794492800, 1232.153039068244, 1233.674758148412],
    ),
    (
        true,
        1,
        2,
        [277.606374132585, 360.935622735102, 424.386111234562, 424.654099882103],
    ),
    (
        true,
        1,
        3,
        [861.883303688418, 924.417315795607, 1242.513846364662, 1243.021062763019],
    ),
    (
        true,
        2,
        2,
        [253.762145009690, 420.426358337451, 460.879675777895, 461.415635628995],
    ),
    (
        true,
        2,
        3,
        [856.410018758138, 981.488151845963, 1271.221898263074, 1272.236345340532],
    ),
    (
        true,
        3,
        2,
        [239.861473143049, 489.872077918155, 516.050892824875, 516.854789006668],
    ),
    (
        true,
        3,
        3,
        [
            866.582637948845,
            1054.225102212552,
            1317.824847221716,
            1319.346553541724,
        ],
    ),
];

/// Arithmetic 4: the polygon's hoop force over `rho A R^2`, minus 1, per mesh.
const PR_TABLE: [f64; 2] = [-2.851712875399e-3, -7.136922537857e-4];

/// WF: twice the largest predicted error of the mesh (index), set (F) and family (out-of-plane).
fn wf_limit(mesh: usize, set_f: bool, out: bool) -> f64 {
    match (mesh, set_f, out) {
        (0, false, false) => 8.1e-3,
        (0, false, true) => 3.3e-3,
        (0, true, false) => 4.9e-3,
        (0, true, true) => 3.1e-3,
        (1, false, false) => 2.03e-3,
        (1, false, true) => 8.3e-4,
        (1, true, false) => 1.23e-3,
        (1, true, true) => 7.8e-4,
        _ => unreachable!("two meshes"),
    }
}

#[test]
fn rf1_rf2_rf3_reference_of_the_spinning_ring() {
    let mut ok = true;
    let w1 = omega_1();
    let e1 = (w1 / OMEGA_1_TABLE - 1.0).abs();
    let mut rf1 = e1;
    let mut rf3: f64 = 0.0;
    let mut signs = true;
    let om = speeds(w1);
    for (set_f, j, n, inp, outp) in REF_TABLE {
        for (fam, row) in [(IN_PLANE, inp), (OUT_PLANE, outp)] {
            let (wf, wb, kf, kb) = ref_pair(n, om[j - 1], fam, set_f, sec(1.0));
            rf1 = rf1.max((wf / row[0] - 1.0).abs()).max((wb / row[1] - 1.0).abs());
            rf3 = rf3.max((kf / row[2] - 1.0).abs()).max((kb / row[3] - 1.0).abs());
            signs &= kf > 0.0 && kb < 0.0;
        }
    }
    println!(
        "RF1 omega_1 = {w1:.12} rad/s; the 48 roots and omega_1 against arithmetic 1: max {rf1:.2e} (limit 1e-9) {}",
        met(rf1 <= 1e-9)
    );
    ok &= rf1 <= 1e-9;
    println!(
        "RF3 whirl numbers against arithmetic 1: max {rf3:.2e} (limit 1e-6) {}; signs kappa_f > 0 > kappa_b: {} {}",
        met(rf3 <= 1e-6),
        signs,
        met(signs)
    );
    ok &= rf3 <= 1e-6 && signs;
    // RF2: the thin limit.
    for n in [2usize, 3] {
        let mut d = Vec::new();
        for t in [1.0 / 8.0, 1.0 / 16.0] {
            let s = sec(t);
            let (wf, wb, _, _) = ref_pair(n, t * om[2], IN_PLANE, true, s);
            let (tf, tb) = thin_in_plane(n, t * om[2], s);
            let (of, _, _, _) = ref_pair(n, 0.0, OUT_PLANE, true, s);
            let to = thin_out_of_plane(n, s);
            d.push([wf / tf - 1.0, wb / tb - 1.0, of / to - 1.0]);
        }
        for (q, name) in ["in-plane forward", "in-plane backward", "out-of-plane at rest"]
            .iter()
            .enumerate()
        {
            let ratio = d[0][q] / d[1][q];
            let rich = (4.0 * d[1][q] - d[0][q]) / 3.0;
            let good = (3.9..=4.1).contains(&ratio) && rich.abs() <= 1e-7;
            println!(
                "RF2 n = {n} {name}: t = 1/8 {:+.3e}, t = 1/16 {:+.3e}, ratio {ratio:.4} (limit 3.9 .. 4.1), Richardson {rich:+.2e} (limit 1e-7) {}",
                d[0][q],
                d[1][q],
                met(good)
            );
            ok &= good;
        }
    }
    assert!(ok, "a registered reference check missed (see the output)");
}

// ------------------------------------------------------------------------------------------- the ring of beams (D2)

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn p(phi: f64) -> Vec3 {
    let (a, b) = (ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + R * (c * a[k] + s * b[k]))
}

fn section() -> BeamSection {
    BeamSection::new(area(), i_y(), i_z(), J_T, KSH, KSH).unwrap()
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// The free ring sector of D2 with `nc` elements.
fn sector(nc: usize) -> SolidModel {
    let al = 2.0 * PI / N as f64;
    let nodes: Vec<Vec3> = (0..=nc).map(|i| p(i as f64 * al / nc as f64)).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..nc).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section(), axis()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..nc).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![nc]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true).beams(true),
    ))
    .unwrap();
    m
}

/// D4: the pre-stress at 1 rad/s from a copy with the tangential and axial displacement of node 1 zero.
fn prestress(nc: usize) -> Prestress {
    let mut copy = sector(nc);
    let phi1 = 2.0 * PI / (N * nc) as f64;
    let t = [0, 1, 2].map(|k| -phi1.sin() * ea()[k] + phi1.cos() * eb()[k]);
    for d in [t, axis()] {
        copy.add_constraint(Constraint::linear(
            (0..3).map(|c| LinearTerm::new(1, c, d[c])).collect(),
            0.0,
        ))
        .unwrap();
    }
    copy.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    let r = solve_static(&copy, &StaticOptions::default()).unwrap();
    Prestress::from_static(&copy, &r).unwrap()
}

/// The axial fraction of the translations of a sector-pair mode (D6).
fn axial_fraction(model: &SolidModel, m: &WhirlMode) -> f64 {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let a = axis();
    let (mut ax, mut all) = (0.0, 0.0);
    for v in [&m.re, &m.im] {
        for half in 0..2 {
            for node in 0..dofs.n_nodes() {
                let u: Vec3 = [0, 1, 2].map(|c| v[half * n + dofs.equation(node, c).unwrap()]);
                ax += dot(u, a).powi(2);
                all += dot(u, u);
            }
        }
    }
    ax / all
}

struct Pair {
    lo: WhirlMode,
    hi: WhirlMode,
    purity: f64,
}

/// The four lowest whirl modes of one harmonic sorted into the in-plane and the out-of-plane pair, the ratio of the fifth to
/// the fourth, the four lowest omegas, the unknowns and the largest frequency.
fn pairs(model: &SolidModel, rs: &RotatingSector, k: usize, omega: f64) -> (Pair, Pair, f64, [f64; 4], usize, f64) {
    let res = rs.whirl_modes(k, omega).unwrap();
    let wm = res.modes().expect("no loss of stiffness at k >= 2");
    let modes = &wm.modes;
    assert!(modes.len() >= 5, "{} modes", modes.len());
    let mut inp = Vec::new();
    let mut out = Vec::new();
    for m in &modes[..4] {
        let f = axial_fraction(model, m);
        if f < 0.5 {
            inp.push((m.clone(), f));
        } else {
            out.push((m.clone(), 1.0 - f));
        }
    }
    assert!(
        inp.len() == 2 && out.len() == 2,
        "set-up failure: {} in-plane and {} out-of-plane among the four lowest",
        inp.len(),
        out.len()
    );
    let mk = |v: Vec<(WhirlMode, f64)>| Pair {
        purity: v[0].1.max(v[1].1),
        lo: v[0].0.clone(),
        hi: v[1].0.clone(),
    };
    let four = [0, 1, 2, 3].map(|i| modes[i].omega);
    let wmax = wm.frequencies_all.iter().cloned().fold(0.0, f64::max);
    (
        mk(inp),
        mk(out),
        modes[4].omega / modes[3].omega,
        four,
        wm.unknowns,
        wmax,
    )
}

#[test]
fn nc0_the_free_ring_static_solve_is_refused() {
    let m = sector(MESHES[0]);
    let r = RotatingSector::new(&m, &RotatingOptions::new(3).coriolis(true));
    let refused = matches!(r, Err(FemError::SingularStiffness { .. }));
    println!(
        "NC0 RotatingSector::new on the free sector with stress stiffening: SingularStiffness {} {}",
        refused,
        met(refused)
    );
    assert!(refused, "expected SingularStiffness");
}

#[test]
fn nc1_no_split_without_coriolis() {
    let w1 = omega_1();
    let om = speeds(w1)[2];
    let m = sector(MESHES[0]);
    let rs = RotatingSector::new(&m, &RotatingOptions::new(3).coriolis(false).stress_stiffening(false)).unwrap();
    let mut ok = true;
    for k in HARMONICS {
        let (pi, po, _, _, nq, wmax) = pairs(&m, &rs, k, om);
        for (name, pr) in [("in-plane", &pi), ("out-of-plane", &po)] {
            let d = pr.hi.omega / pr.lo.omega - 1.0;
            let lim = 10.0 * nq as f64 * EPS * wmax / pr.lo.omega;
            println!("NC1 k = {k} {name}: omega_hi / omega_lo - 1 = {d:.2e} (limit {lim:.2e}, n_q {nq}, omega_max {wmax:.3e}) {}", met(d <= lim));
            ok &= d <= lim;
        }
    }
    assert!(ok, "NC1 missed");
}

#[test]
fn pr_wf_od_sp_ka_di_tr_ag_whirl_of_the_spinning_ring_of_beams() {
    let w1 = omega_1();
    let om = speeds(w1);
    println!("omega_1 = {w1:.9} rad/s; speeds {:.6} {:.6} {:.6}", om[0], om[1], om[2]);
    let mut ok = true;
    // errors[mesh][(set, j, k, fam, member)]
    let mut errs: Vec<Vec<f64>> = vec![Vec::new(), Vec::new()];
    let mut worst = [[[0.0f64; 2]; 2]; 2];
    let (mut sp_max, mut ka_max, mut tr_max, mut ag_max): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let mut sep_min = f64::INFINITY;
    let mut di_ok = true;
    for (mi, &nc) in MESHES.iter().enumerate() {
        let m = sector(nc);
        // PR.
        let ps = prestress(nc);
        let n1 = RHO * area() * R * R;
        let mut pr: f64 = 0.0;
        for e in 0..nc {
            let nodes = m.mesh().element_nodes(e);
            let (x0, x1) = (m.mesh().nodes()[nodes[0]], m.mesh().nodes()[nodes[1]]);
            let d = [0, 1, 2].map(|k| x1[k] - x0[k]);
            let l = dot(d, d).sqrt();
            let e1 = d.map(|v| v / l);
            let s = ps.of_element(e)[0];
            let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
            let mut sn = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    sn += e1[i] * t[i][j] * e1[j];
                }
            }
            let dev = area() * sn / n1 - 1.0;
            pr = pr.max((dev - PR_TABLE[mi]).abs());
        }
        println!(
            "PR mesh n_c = {nc}: hoop force against arithmetic 4: {pr:.2e} (limit 1e-8) {}",
            met(pr <= 1e-8)
        );
        ok &= pr <= 1e-8;
        for set_f in [false, true] {
            let opts = RotatingOptions::new(3).coriolis(true);
            let rs = if set_f {
                RotatingSector::with_unit_prestress(&m, ps.clone(), &opts).unwrap()
            } else {
                RotatingSector::new(&m, &opts.stress_stiffening(false)).unwrap()
            };
            for (j, &omega) in om.iter().enumerate() {
                for k in HARMONICS {
                    let (pi, po, sep, four, _, _) = pairs(&m, &rs, k, omega);
                    sep_min = sep_min.min(sep);
                    if mi == 0 {
                        let ag = AG_TABLE
                            .iter()
                            .find(|r| r.0 == set_f && r.1 == j + 1 && r.2 == k)
                            .unwrap();
                        for q in 0..4 {
                            ag_max = ag_max.max((four[q] / ag.3[q] - 1.0).abs());
                        }
                    }
                    for (fi, (pr_, fam, out)) in
                        [(&pi, IN_PLANE, false), (&po, OUT_PLANE, true)].into_iter().enumerate()
                    {
                        let (wf, wb, kf, kb) = ref_pair(k, omega, fam, set_f, sec(1.0));
                        let e_lo = pr_.lo.omega / wf - 1.0;
                        let e_hi = pr_.hi.omega / wb - 1.0;
                        let d = (pr_.hi.omega - pr_.lo.omega) / (wb - wf) - 1.0;
                        let ka = (pr_.lo.whirl / kf - 1.0).abs().max((pr_.hi.whirl / kb - 1.0).abs());
                        let di = pr_.lo.direction == Whirl::Forward
                            && pr_.lo.wave.unwrap_or(0.0) >= 0.99
                            && pr_.hi.direction == Whirl::Backward
                            && pr_.hi.wave.unwrap_or(0.0) <= -0.99;
                        let lim = wf_limit(mi, set_f, out);
                        let wf_ok = e_lo.abs() <= lim && e_hi.abs() <= lim;
                        println!(
                            "n_c {nc} set {} Omega_{} k {k} {:12}: lo {:.6} ({}, tau {:+.4}, kappa {:+.4e}) hi {:.6} ({}, tau {:+.4}, kappa {:+.4e}); e_lo {e_lo:+.3e} e_hi {e_hi:+.3e} (WF limit {lim:.2e}) {}; split d {d:+.2e}; kappa {ka:.2e}; purity {:.1e}; DI {}",
                            if set_f { "F" } else { "S" },
                            j + 1,
                            if out { "out-of-plane" } else { "in-plane" },
                            pr_.lo.omega,
                            pr_.lo.direction.name(),
                            pr_.lo.wave.unwrap_or(f64::NAN),
                            pr_.lo.whirl,
                            pr_.hi.omega,
                            pr_.hi.direction.name(),
                            pr_.hi.wave.unwrap_or(f64::NAN),
                            pr_.hi.whirl,
                            met(wf_ok),
                            pr_.purity,
                            met(di)
                        );
                        ok &= wf_ok;
                        di_ok &= di;
                        sp_max = sp_max.max(d.abs());
                        ka_max = ka_max.max(ka);
                        tr_max = tr_max.max(pr_.purity);
                        let w = &mut worst[mi][usize::from(set_f)][fi];
                        *w = w.max(e_lo.abs()).max(e_hi.abs());
                        errs[mi].push(e_lo);
                        errs[mi].push(e_hi);
                    }
                }
            }
        }
    }
    for (mi, &nc) in MESHES.iter().enumerate() {
        println!(
            "WF n_c {nc}: worst |e| S in {:.2e}, S out {:.2e}, F in {:.2e}, F out {:.2e}",
            worst[mi][0][0], worst[mi][0][1], worst[mi][1][0], worst[mi][1][1]
        );
    }
    // OD.
    let (mut pmin, mut pmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for (a, b) in errs[0].iter().zip(&errs[1]) {
        let p = (a.abs() / b.abs()).log2();
        pmin = pmin.min(p);
        pmax = pmax.max(p);
    }
    let od = (pmin - 2.0).abs() <= 0.5 && (pmax - 2.0).abs() <= 0.5;
    println!(
        "OD observed order of the {} frequencies: {pmin:.4} .. {pmax:.4} (limit |p - 2| <= 0.5) {}",
        errs[0].len(),
        met(od)
    );
    ok &= od;
    println!(
        "SP split, every pair, both meshes: max |d| {sp_max:.2e} (limit 1e-2) {}",
        met(sp_max <= 1e-2)
    );
    ok &= sp_max <= 1e-2;
    println!(
        "KA whirl numbers: max {ka_max:.2e} (limit 1e-2) {}",
        met(ka_max <= 1e-2)
    );
    ok &= ka_max <= 1e-2;
    println!(
        "DI directions and wave senses, every pair: {} ; fifth over fourth mode min {sep_min:.2} (limit 5) {}",
        met(di_ok),
        met(sep_min >= 5.0)
    );
    ok &= di_ok && sep_min >= 5.0;
    println!(
        "TR family purity: max {tr_max:.2e} (limit 1e-6) {}",
        met(tr_max <= 1e-6)
    );
    ok &= tr_max <= 1e-6;
    println!(
        "AG coarse mesh against the polygon model: max {ag_max:.2e} (limit 1e-8) {}",
        met(ag_max <= 1e-8)
    );
    ok &= ag_max <= 1e-8;
    assert!(ok, "a registered check missed (see the output)");
}
