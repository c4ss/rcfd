//! F-372 (ADR 0273): the element library. Checks A1 to A10, the extensions E1 to E6 and the negative controls N1 to N3 of
//! the registration, on the reference elements and on randomly distorted ones (seed 0xF372_0273).

use rcfd_core::vec3::Vec3;
use rcfd_fem::element::element_for;
use rcfd_fem::material::{apply, PointContext};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::quadrature::{self, Domain, QuadratureRule};
use rcfd_fem::{FemError, Material};

const SEED: u64 = 0xF372_0273;
const N_POINTS: usize = 100;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;

/// SplitMix64, a deterministic source only (ADR 0273 D6).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    /// Uniform in [-r, r).
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

fn rotation(rng: &mut Rng) -> [[f64; 3]; 3] {
    loop {
        let q = [rng.sym(1.0), rng.sym(1.0), rng.sym(1.0), rng.sym(1.0)];
        let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if n < 0.1 {
            continue;
        }
        let (w, x, y, z) = (q[0] / n, q[1] / n, q[2] / n, q[3] / n);
        return [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - w * z),
                2.0 * (x * z + w * y),
            ],
            [
                2.0 * (x * y + w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - w * x),
            ],
            [
                2.0 * (x * z - w * y),
                2.0 * (y * z + w * x),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ];
    }
}

fn rotate(r: &[[f64; 3]; 3], p: Vec3) -> Vec3 {
    [
        r[0][0] * p[0] + r[0][1] * p[1] + r[0][2] * p[2],
        r[1][0] * p[0] + r[1][1] * p[1] + r[1][2] * p[2],
        r[2][0] * p[0] + r[2][1] * p[1] + r[2][2] * p[2],
    ]
}

struct Case {
    label: String,
    coords: Vec<Vec3>,
    distorted: bool,
    curved: bool,
}

fn distorted(rng: &mut Rng, kind: ElementKind, curved: bool) -> Vec<Vec3> {
    let refn = kind.reference_nodes();
    let r = if kind == ElementKind::Hex8 { 0.2 } else { 0.1 };
    let corners = kind.corners();
    let mut x: Vec<Vec3> = refn[..corners]
        .iter()
        .map(|p| [p[0] + rng.sym(r), p[1] + rng.sym(r), p[2] + rng.sym(r)])
        .collect();
    if kind == ElementKind::Tet10 {
        for [i, j] in rcfd_fem::mesh::TET_EDGES {
            let mut m = [0.0; 3];
            for k in 0..3 {
                m[k] = 0.5 * (x[i][k] + x[j][k]);
            }
            if curved {
                for v in m.iter_mut() {
                    *v += rng.sym(0.03);
                }
            }
            x.push(m);
        }
    }
    let rot = rotation(rng);
    x.into_iter().map(|p| rotate(&rot, p)).collect()
}

/// The reference element and the distorted ones of D6, each validated through the mesh of ADR 0271.
fn cases(kind: ElementKind) -> Vec<Case> {
    let mut out = vec![Case {
        label: "reference".into(),
        coords: kind.reference_nodes().to_vec(),
        distorted: false,
        curved: false,
    }];
    let mut rng = Rng::new(10 + kind_index(kind));
    for i in 0..10 {
        let curved = kind == ElementKind::Tet10;
        out.push(Case {
            label: format!("{} {i}", if curved { "curved" } else { "distorted" }),
            coords: distorted(&mut rng, kind, curved),
            distorted: true,
            curved,
        });
    }
    if kind == ElementKind::Tet10 {
        for i in 0..5 {
            out.push(Case {
                label: format!("straight {i}"),
                coords: distorted(&mut rng, kind, false),
                distorted: true,
                curved: false,
            });
        }
    }
    for c in &out {
        let mut mesh = SolidMesh::new(c.coords.clone()).unwrap();
        let conn: Vec<usize> = (0..c.coords.len()).collect();
        mesh.add_block(kind, conn)
            .unwrap_or_else(|e| panic!("{} {}: refused by the mesh: {e}", kind.name(), c.label));
    }
    out
}

/// 100 random reference points per case, a fresh set per case in case order.
fn points(kind: ElementKind, n_cases: usize) -> Vec<Vec<[f64; 3]>> {
    let mut rng = Rng::new(20 + kind_index(kind));
    (0..n_cases)
        .map(|_| {
            let mut pts = Vec::with_capacity(N_POINTS);
            while pts.len() < N_POINTS {
                if kind == ElementKind::Hex8 {
                    pts.push([rng.sym(1.0), rng.sym(1.0), rng.sym(1.0)]);
                } else {
                    let p = [rng.uniform(), rng.uniform(), rng.uniform()];
                    if p[0] + p[1] + p[2] <= 1.0 {
                        pts.push(p);
                    }
                }
            }
            pts
        })
        .collect()
}

fn d_matrix() -> [[f64; 6]; 6] {
    Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0)
        .unwrap()
        .elasticity_matrix()
        .unwrap()
}

/// `v <= limit`, false for NaN.
fn within(v: f64, limit: f64) -> bool {
    v <= limit
}

/// `v > limit`, false for NaN.
fn above(v: f64, limit: f64) -> bool {
    v > limit
}

fn inf(v: &[f64; 3]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

// ---------------------------------------------------------------------------------------------------------------------
// A1, A2: partition of unity and sum of derivatives.

#[test]
fn a1_a2_partition_of_unity() {
    let mut fail = Vec::new();
    for kind in KINDS {
        let el = element_for(kind);
        let cs = cases(kind);
        let pts = points(kind, cs.len());
        let (mut a1, mut a2_ref, mut a2_dist) = (0.0f64, 0.0f64, 0.0f64);
        for (c, ps) in cs.iter().zip(&pts) {
            for &xi in ps {
                let n = el.shape_functions(xi);
                a1 = a1.max((n.iter().sum::<f64>() - 1.0).abs());
                if c.distorted {
                    let p = el.point(&c.coords, xi).unwrap();
                    let mut s = [0.0; 3];
                    let mut gmax = 0.0f64;
                    for g in &p.grad {
                        for k in 0..3 {
                            s[k] += g[k];
                        }
                        gmax = gmax.max(inf(g));
                    }
                    a2_dist = a2_dist.max(inf(&s) / gmax);
                } else {
                    let mut s = [0.0; 3];
                    for g in el.shape_derivatives(xi) {
                        for k in 0..3 {
                            s[k] += g[k];
                        }
                    }
                    a2_ref = a2_ref.max(inf(&s));
                }
            }
        }
        println!(
            "A1 {}: max |sum N - 1| = {a1:.3e}; A2 reference max |sum dN/dxi| = {a2_ref:.3e}; distorted max |sum grad N| / max |grad N| = {a2_dist:.3e} (limit 1e-14)",
            kind.name()
        );
        if !within(a1, 1e-14) {
            fail.push(format!("A1 {}", kind.name()));
        }
        if !(within(a2_ref, 1e-14) && within(a2_dist, 1e-14)) {
            fail.push(format!("A2 {}", kind.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// A3: analytic derivatives against central differences.

#[test]
fn a3_central_differences() {
    let h = 1e-5;
    let mut fail = Vec::new();
    for kind in KINDS {
        let el = element_for(kind);
        let cs = cases(kind);
        let pts = points(kind, cs.len());
        let (mut e_ref, mut e_dist) = (0.0f64, 0.0f64);
        for (c, ps) in cs.iter().zip(&pts) {
            for &xi in ps {
                let dn = el.shape_derivatives(xi);
                let geo = if c.distorted {
                    Some(el.point(&c.coords, xi).unwrap())
                } else {
                    None
                };
                for j in 0..3 {
                    let mut xp = xi;
                    let mut xm = xi;
                    xp[j] += h;
                    xm[j] -= h;
                    let np = el.shape_functions(xp);
                    let nm = el.shape_functions(xm);
                    let fd: Vec<f64> = np.iter().zip(&nm).map(|(a, b)| (a - b) / (2.0 * h)).collect();
                    match &geo {
                        None => {
                            for a in 0..dn.len() {
                                e_ref = e_ref.max((dn[a][j] - fd[a]).abs());
                            }
                        }
                        Some(p) => {
                            let mut t = [0.0; 3];
                            for (a, x) in c.coords.iter().enumerate() {
                                for k in 0..3 {
                                    t[k] += (np[a] - nm[a]) * x[k] / (2.0 * h);
                                }
                            }
                            for a in 0..dn.len() {
                                let g = p.grad[a];
                                let along = g[0] * t[0] + g[1] * t[1] + g[2] * t[2];
                                e_dist = e_dist.max((along - fd[a]).abs());
                            }
                        }
                    }
                }
            }
        }
        println!(
            "A3 {}: reference max |dN/dxi - FD| = {e_ref:.3e}; distorted max |grad N . T - FD| = {e_dist:.3e} (limit 1e-7)",
            kind.name()
        );
        if !(within(e_ref, 1e-7) && within(e_dist, 1e-7)) {
            fail.push(format!("A3 {}", kind.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// A4, N1: monomials.

fn fact(n: u32) -> f64 {
    (1..=n).map(|k| k as f64).product()
}

fn exact_monomial(domain: Domain, e: [u32; 3]) -> f64 {
    match domain {
        Domain::Tetrahedron => fact(e[0]) * fact(e[1]) * fact(e[2]) / fact(e[0] + e[1] + e[2] + 3),
        _ => e
            .iter()
            .map(|&k| if k % 2 == 0 { 2.0 / (k as f64 + 1.0) } else { 0.0 })
            .product(),
    }
}

fn rule_monomial(rule: &QuadratureRule, e: [u32; 3]) -> f64 {
    rule.points()
        .iter()
        .zip(rule.weights())
        .map(|(p, w)| w * p[0].powi(e[0] as i32) * p[1].powi(e[1] as i32) * p[2].powi(e[2] as i32))
        .sum()
}

/// Monomials up to the degree `d` (total for the tetrahedron, per variable for the cube); `exact_degree` keeps only
/// those of total degree exactly `d` (tetrahedron) or with a largest exponent exactly `d` (cube).
fn monomials(domain: Domain, d: u32, exact_degree: bool) -> Vec<[u32; 3]> {
    let mut out = Vec::new();
    for a in 0..=d {
        for b in 0..=d {
            for c in 0..=d {
                let keep = match domain {
                    Domain::Tetrahedron => {
                        if exact_degree {
                            a + b + c == d
                        } else {
                            a + b + c <= d
                        }
                    }
                    _ => !exact_degree || a.max(b).max(c) == d,
                };
                if keep {
                    out.push([a, b, c]);
                }
            }
        }
    }
    out
}

#[test]
fn a4_quadrature_monomials() {
    let mut fail = Vec::new();
    for rule in quadrature::all_rules() {
        let d = rule.degree();
        let mono = monomials(rule.domain(), d, false);
        let err = mono
            .iter()
            .map(|&e| (rule_monomial(rule, e) - exact_monomial(rule.domain(), e)).abs())
            .fold(0.0f64, f64::max);
        let err_above = monomials(rule.domain(), d + 1, true)
            .iter()
            .map(|&e| (rule_monomial(rule, e) - exact_monomial(rule.domain(), e)).abs())
            .fold(0.0f64, f64::max);
        println!(
            "A4 {} ({} points, degree {d}, {} monomials): max error {err:.3e} (limit 1e-13); N1 at degree {}: max error {err_above:.3e} (must be > 1e-6)",
            rule.name(),
            rule.len(),
            mono.len(),
            d + 1
        );
        if !within(err, 1e-13) {
            fail.push(format!("A4 {}", rule.name()));
        }
        if !above(err_above, 1e-6) {
            fail.push(format!("N1 {}", rule.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// A5, A6, A7, N2, N3: the element stiffness.

fn frob(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn rank(k: &[f64], n: usize) -> (usize, f64, f64) {
    let eig = rcfd_linalg::eigen::symmetric_eigen(k, n).unwrap();
    let lmax = eig.values[n - 1];
    let r = eig.values.iter().filter(|&&l| l > 1e-9 * lmax).count();
    (r, eig.values[5] / lmax, eig.values[6] / lmax)
}

fn rigid_motions(coords: &[Vec3]) -> Vec<Vec<f64>> {
    let mut out = Vec::new();
    for k in 0..3 {
        let mut r = vec![0.0; 3 * coords.len()];
        for a in 0..coords.len() {
            r[3 * a + k] = 1.0;
        }
        out.push(r);
    }
    for k in 0..3 {
        let mut w = [0.0; 3];
        w[k] = 1.0;
        let mut r = vec![0.0; 3 * coords.len()];
        for (a, x) in coords.iter().enumerate() {
            let c = rcfd_core::vec3::cross(w, *x);
            r[3 * a..3 * a + 3].copy_from_slice(&c);
        }
        out.push(r);
    }
    out
}

fn unit_strain_field(coords: &[Vec3], eps: &[f64; 6]) -> Vec<f64> {
    let t = [
        [eps[0], 0.5 * eps[3], 0.5 * eps[5]],
        [0.5 * eps[3], eps[1], 0.5 * eps[4]],
        [0.5 * eps[5], 0.5 * eps[4], eps[2]],
    ];
    let mut u = Vec::with_capacity(3 * coords.len());
    for x in coords {
        for row in &t {
            u.push(row[0] * x[0] + row[1] * x[1] + row[2] * x[2]);
        }
    }
    u
}

#[test]
fn a5_a6_a7_stiffness() {
    let d = d_matrix();
    let mut fail = Vec::new();
    for kind in KINDS {
        let el = element_for(kind);
        let n = 3 * el.n_nodes();
        let (mut sym, mut rig, mut l6, mut l7) = (0.0f64, 0.0f64, 0.0f64, f64::INFINITY);
        let mut ranks = Vec::new();
        for c in cases(kind) {
            let k = el.stiffness(&c.coords, &d).unwrap();
            let kmax = k.values().iter().fold(0.0f64, |m, v| m.max(v.abs()));
            for i in 0..n {
                for j in 0..n {
                    sym = sym.max((k.get(i, j) - k.get(j, i)).abs() / kmax);
                }
            }
            let kf = frob(k.values());
            for r in rigid_motions(&c.coords) {
                rig = rig.max(frob(&k.matvec(&r)) / (kf * frob(&r)));
            }
            let (rk, r6, r7) = rank(k.values(), n);
            ranks.push(rk);
            l6 = l6.max(r6);
            l7 = l7.min(r7);
            if rk != n - 6 {
                fail.push(format!("A7 {} {}: rank {rk}", kind.name(), c.label));
            }
        }
        println!(
            "A5 {}: max |K - K^T| / max |K| = {sym:.3e} (limit 1e-13); A6 max |K r| / (|K|_F |r|) = {rig:.3e} (limit 1e-11); A7 ranks {ranks:?} (n - 6 = {}), max lambda_6/lambda_max = {l6:.3e}, min lambda_7/lambda_max = {l7:.3e}",
            kind.name(),
            n - 6
        );
        if !within(sym, 1e-13) {
            fail.push(format!("A5 {}", kind.name()));
        }
        if !within(rig, 1e-11) {
            fail.push(format!("A6 {}", kind.name()));
        }
        // N3: a unit strain is not a rigid motion.
        let refc = kind.reference_nodes().to_vec();
        let k = el.stiffness(&refc, &d).unwrap();
        let u = unit_strain_field(&refc, &[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let n3 = frob(&k.matvec(&u)) / (frob(k.values()) * frob(&u));
        println!(
            "N3 {}: A6 measure of the unit strain eps_xx = {n3:.3e} (must be > 1e-3)",
            kind.name()
        );
        if !above(n3, 1e-3) {
            fail.push(format!("N3 {}", kind.name()));
        }
    }
    // N2: reduced rules make the rank deficient.
    for (kind, rule) in [
        (ElementKind::Hex8, quadrature::gauss_1()),
        (ElementKind::Tet10, quadrature::tet_1()),
    ] {
        let el = element_for(kind);
        let n = 3 * el.n_nodes();
        let k = el.stiffness_with_rule(kind.reference_nodes(), &d, rule).unwrap();
        let (rk, _, _) = rank(k.values(), n);
        println!("N2 {} with {}: rank {rk} (must be 6)", kind.name(), rule.name());
        if rk != 6 {
            fail.push(format!("N2 {}", kind.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// A8, E3, E6: strain energy of the six unit strain states.

#[test]
fn a8_strain_energy() {
    let d = d_matrix();
    let mut fail = Vec::new();
    for kind in KINDS {
        let el = element_for(kind);
        let cs = cases(kind);
        let pts = points(kind, cs.len());
        let (mut a8, mut e3, mut e6) = (0.0f64, 0.0f64, 0.0f64);
        for (c, ps) in cs.iter().zip(&pts) {
            let k = el.stiffness(&c.coords, &d).unwrap();
            let v: f64 = el
                .integration_points(&c.coords, el.stiffness_rule())
                .unwrap()
                .iter()
                .map(|p| p.weight)
                .sum();
            for s in 0..6 {
                let mut eps = [0.0; 6];
                eps[s] = 1.0;
                let u = unit_strain_field(&c.coords, &eps);
                let ku = k.matvec(&u);
                let w_k = 0.5 * u.iter().zip(&ku).map(|(a, b)| a * b).sum::<f64>();
                let sig = apply(&d, &eps);
                let w_e = 0.5 * v * eps.iter().zip(&sig).map(|(a, b)| a * b).sum::<f64>();
                a8 = a8.max((w_k - w_e).abs() / w_e);
                for &xi in ps {
                    let st = el.strain_at(&c.coords, &u, xi).unwrap();
                    for m in 0..6 {
                        e3 = e3.max((st[m] - eps[m]).abs());
                    }
                }
            }
            if c.curved {
                let mut mesh = SolidMesh::new(c.coords.clone()).unwrap();
                mesh.add_block(kind, (0..c.coords.len()).collect()).unwrap();
                let vx = mesh.element_volume(0);
                e6 = e6.max((v - vx).abs() / vx);
            }
        }
        println!(
            "A8 {}: max |u^T K u/2 - V/2 eps D eps| / (V/2 eps D eps) = {a8:.3e} (limit 1e-12); E3 max |strain_at - eps| = {e3:.3e} (limit 1e-12); E6 curved |V_rule - V_exact| / V_exact = {e6:.3e} (record)",
            kind.name()
        );
        if !within(a8, 1e-12) {
            fail.push(format!("A8 {}", kind.name()));
        }
        if !within(e3, 1e-12) {
            fail.push(format!("E3 {}", kind.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// A9, A10, E4: Hooke's law.

#[test]
fn a9_a10_hooke() {
    let s = 1e-3;
    let gamma = 1e-3;
    let mut fail = Vec::new();
    for nu in [0.0, 0.3, 0.45, -0.5] {
        let mat = Material::isotropic_elastic(E_MOD, nu, 7800.0, 1.2e-5).unwrap();
        let d = mat.elasticity_matrix().unwrap();
        let g = E_MOD / (2.0 * (1.0 + nu));
        let (mut a9, mut a10) = (0.0f64, 0.0f64);
        for axis in 0..3 {
            let mut eps = [-nu * s, -nu * s, -nu * s, 0.0, 0.0, 0.0];
            eps[axis] = s;
            let (st, tangent) = mat.stress(&eps, &PointContext::default(), &[]).unwrap();
            assert_eq!(tangent, d);
            for sig in [apply(&d, &eps), st] {
                for k in 0..6 {
                    let want = if k == axis { E_MOD * s } else { 0.0 };
                    a9 = a9.max((sig[k] - want).abs() / (E_MOD * s));
                }
            }
        }
        for slot in 3..6 {
            let mut eps = [0.0; 6];
            eps[slot] = gamma;
            let (st, _) = mat.stress(&eps, &PointContext::default(), &[]).unwrap();
            for sig in [apply(&d, &eps), st] {
                for k in 0..6 {
                    let want = if k == slot { g * gamma } else { 0.0 };
                    a10 = a10.max((sig[k] - want).abs() / (g * gamma));
                }
            }
        }
        // E4: D S = I with the closed-form compliance.
        let mut sm = [[0.0; 6]; 6];
        for i in 0..3 {
            for j in 0..3 {
                sm[i][j] = if i == j { 1.0 / E_MOD } else { -nu / E_MOD };
            }
        }
        for k in 3..6 {
            sm[k][k] = 1.0 / g;
        }
        let mut e4 = 0.0f64;
        for i in 0..6 {
            for j in 0..6 {
                let v: f64 = (0..6).map(|m| d[i][m] * sm[m][j]).sum();
                e4 = e4.max((v - if i == j { 1.0 } else { 0.0 }).abs());
            }
        }
        println!(
            "nu = {nu}: A9 uniaxial max error / (E s) = {a9:.3e}; A10 shear max error / (G gamma) = {a10:.3e} (limit 1e-14); E4 max |D S - I| = {e4:.3e} (limit 1e-13)"
        );
        if !within(a9, 1e-14) {
            fail.push(format!("A9 nu = {nu}"));
        }
        if !within(a10, 1e-14) {
            fail.push(format!("A10 nu = {nu}"));
        }
        if !within(e4, 1e-13) {
            fail.push(format!("E4 nu = {nu}"));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// E1, E2: Kronecker property and linear reproduction.

#[test]
fn e1_kronecker_e2_linear_reproduction() {
    let mut fail = Vec::new();
    for kind in KINDS {
        let el = element_for(kind);
        let mut e1 = 0.0f64;
        for (b, &xb) in kind.reference_nodes().iter().enumerate() {
            for (a, na) in el.shape_functions(xb).iter().enumerate() {
                e1 = e1.max((na - if a == b { 1.0 } else { 0.0 }).abs());
            }
        }
        let cs = cases(kind);
        let pts = points(kind, cs.len());
        let mut e2 = 0.0f64;
        for (c, ps) in cs.iter().zip(&pts) {
            if !c.distorted {
                continue;
            }
            for &xi in ps {
                let p = el.point(&c.coords, xi).unwrap();
                for i in 0..3 {
                    for k in 0..3 {
                        let s: f64 = c.coords.iter().zip(&p.grad).map(|(x, g)| x[i] * g[k]).sum();
                        e2 = e2.max((s - if i == k { 1.0 } else { 0.0 }).abs());
                    }
                }
            }
        }
        println!(
            "E1 {}: max |N_a(x_b) - delta_ab| = {e1:.3e} (limit 1e-15); E2 max |sum x_a grad N_a - I| = {e2:.3e} (limit 1e-13)",
            kind.name()
        );
        if !within(e1, 1e-15) {
            fail.push(format!("E1 {}", kind.name()));
        }
        if !within(e2, 1e-13) {
            fail.push(format!("E2 {}", kind.name()));
        }
    }
    assert!(fail.is_empty(), "missed: {fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// E5: refusals.

#[test]
fn e5_refusals() {
    let tet = element_for(ElementKind::Tet4);
    let d = d_matrix();
    let refc = ElementKind::Tet4.reference_nodes().to_vec();
    match tet.stiffness(&refc[..3], &d) {
        Err(FemError::FieldLength { key: "coordinates", .. }) => {}
        other => panic!("coordinates: {other:?}"),
    }
    match tet.strain_at(&refc, &[0.0; 11], [0.25; 3]) {
        Err(FemError::FieldLength {
            key: "displacement", ..
        }) => {}
        other => panic!("displacement: {other:?}"),
    }
    let mut inv = refc.clone();
    inv.swap(1, 2);
    match tet.stiffness(&inv, &d) {
        Err(e @ FemError::InvalidValue { key: "jacobian", .. }) => {
            let m = e.to_string();
            println!("E5 inverted Tet4: {m}");
            assert!(m.contains("(0.25, 0.25, 0.25)"), "{m}");
        }
        other => panic!("jacobian: {other:?}"),
    }
    let mat = Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap();
    match mat.stress(&[0.0; 6], &PointContext::default(), &[1.0]) {
        Err(FemError::FieldLength { key: "state", .. }) => {}
        other => panic!("state: {other:?}"),
    }
    println!("E5: coordinates, displacement, jacobian and state refused as registered");
}
