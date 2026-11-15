//! F-618 (ADR 0542): B, the default-bits record of the Newton driver before the supernodal option. The digests were printed
//! by this file run on 498b5417 plus ADR 0542 (`f618-scratch/bits-base.log`) and written into `EXPECTED` before any library
//! code of F-618. G1 is ADR 0542 Q5's cantilever, plain and with D5's halving device; G2 is ADR 0517's hub at `n = 4` with the
//! total Lagrangian body and `d = 1e-4 a` (a copy of `fit_model` of `tests/sa_contact_hub_f604.rs`); G3 is the error text of
//! the same hub with ADR 0517's `d = 1e-3 a`.

use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::ContactResult;
use rcfd_fem::nonlinear::{solve, solve_with, Contribution, DriverExtensions, NewtonOptions, NonlinearResult};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use std::cell::RefCell;

/// Digests of 498b5417 plus ADR 0542 (unchanged library) in the order of the run, from `f618-scratch/bits-base.log`.
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("G1 cantilever, plain and with the device", 0xa2c6_6c48_e61b_b15b),
    ("G2 total Lagrangian hub n = 4, d = 1e-4", 0x711b_5ca9_95f1_e02a),
    (
        "G3 total Lagrangian hub n = 4, d = 1e-3, error text",
        0xe535_7101_0808_74de,
    ),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// FNV-1a 64 over bytes.
fn digest_bytes(v: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &b in v {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// The displacement, the reactions, every contactor node's force, pressure, gap and local coordinates, and every Newton
/// residual, energy, penetration, convex count and active set (ADR 0533's `flat`).
fn flat(r: &ContactResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    if let Some(re) = r.result.reactions() {
        v.extend_from_slice(re);
    }
    for n in &r.nodes {
        v.extend([n.force, n.pressure, n.gap.unwrap_or(f64::NAN), n.local[0], n.local[1]]);
    }
    for s in &r.steps {
        for x in &s.solves {
            v.extend_from_slice(&x.residuals);
            v.extend_from_slice(&x.energies);
            v.push(x.penetration);
            v.push(x.convex_iterations as f64);
            for a in &x.active_sets {
                v.push(a.len() as f64);
                v.extend(a.iter().map(|&i| i as f64));
            }
        }
    }
    v
}

/// The displacement, the reactions, and per converged increment the step, the load factor, the halvings, the residuals
/// and the energies.
fn flat_nl(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    if let Some(re) = r.result.reactions() {
        v.extend_from_slice(re);
    }
    for s in &r.steps {
        v.extend([s.step as f64, s.load_factor, s.halvings as f64]);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    v
}

/// ADR 0542 D5: no force; the tangent entry `-kick` at `eq` once `|u_eq - u_eq,start| > tau`, `u_eq,start` taken at the first
/// call with the attempt's load factor.
struct Spoiler {
    eq: usize,
    tau: f64,
    kick: f64,
    start: RefCell<Option<(u64, f64)>>,
}

impl Contribution for Spoiler {
    fn name(&self) -> &str {
        "spoiler"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        _force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::error::Result<()> {
        let mut s = self.start.borrow_mut();
        if s.map(|x| x.0) != Some(load_factor.to_bits()) {
            *s = Some((load_factor.to_bits(), u[self.eq]));
        }
        let start = s.expect("set").1;
        if (u[self.eq] - start).abs() > self.tau {
            tangent.push((self.eq, self.eq, -self.kick));
        }
        Ok(())
    }
}

/// ADR 0542 Q5: the Hex8 cantilever `10 x 1 x 1` of `40 x 4 x 4` cells, clamped at `x = 0`, a dead traction on `x = 10` with
/// `P L^2/(E I) = 1`; and the equation of the tip's `u_y` at `(10, 0.5, 0.5)`.
fn cantilever() -> (SolidModel, usize) {
    let (l, h, e) = (10.0, 1.0, 2.0e11);
    let mesh = hex_box([0.0; 3], [l, h, h], [40, 4, 4]).unwrap();
    let tip = (0..mesh.nodes().len())
        .find(|&n| {
            let p = mesh.nodes()[n];
            (p[0] - l).abs() < 1e-9 && (p[1] - 0.5).abs() < 1e-9 && (p[2] - 0.5).abs() < 1e-9
        })
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(e, 0.3, 1.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let i = h * h * h * h / 12.0;
    let p = e * i / (l * l);
    m.add_load(Load::traction("xmax", [0.0, p / (h * h), 0.0])).unwrap();
    let eq = m.dof_map().equation(tip, 1).unwrap();
    (m, eq)
}

/// G1: the plain solve and the solve with the device (`tau = 0.35` of the plain tip deflection, `kick = 10 K_qq`).
fn g1() -> Vec<f64> {
    let (m, eq) = cantilever();
    let o = NewtonOptions::new(2);
    let plain = solve(&m, &[], &[], &o).unwrap();
    let k = rcfd_fem::assemble::stiffness(&m).unwrap().diag()[eq];
    let dev = Spoiler {
        eq,
        tau: 0.35 * plain.result.displacement_values()[eq],
        kick: 10.0 * k,
        start: RefCell::new(None),
    };
    let spoiled = solve_with(
        &m,
        &[],
        &[],
        &o,
        DriverExtensions {
            contributions: vec![&dev],
            ..Default::default()
        },
    )
    .unwrap();
    let mut v = flat_nl(&plain);
    v.extend(flat_nl(&spoiled));
    v
}

#[allow(dead_code)]
mod hub {
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Target};
    use rcfd_fem::nonlinear::NewtonOptions;
    use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
    use std::f64::consts::PI;

    const E: f64 = 2.0e11;
    const A: f64 = 1.0;
    const B: f64 = 2.0;
    /// Cells over 90 degrees (ADR 0517 D3).
    const NTH: usize = 15;
    /// Cells per side of the shaft's square core.
    const NC: usize = 8;
    /// Side of the shaft's square core.
    const CORE: f64 = 0.4 * A;
    /// The axial offset of the shaft's node planes, as a fraction of the next cell (ADR 0517 D3).
    const SHIFT: f64 = 0.05;

    #[derive(Clone, Copy, Debug)]
    struct Setup {
        /// Cells through `b - a` (the bore cell `h = (b - a)/n`).
        n: usize,
        /// Hub length.
        l: f64,
        /// Shaft extension beyond each face.
        e: f64,
        nu: f64,
        d: f64,
        /// `u_z = 0` at every node (control CPE).
        all_uz: bool,
        /// The axial offset of the shaft's planes.
        shift: f64,
    }

    const BASE: Setup = Setup {
        n: 8,
        l: 4.0 * (B - A),
        e: 2.0 * A,
        nu: 0.3,
        d: 1e-3 * A,
        all_uz: false,
        shift: SHIFT,
    };

    /// `p_0 = E d (b^2 - a^2)/(2 a b^2)` (ADR 0517 D1, `sigma_zz = 0`).
    fn p_free(s: &Setup) -> f64 {
        E * s.d * (B * B - A * A) / (2.0 * A * B * B)
    }

    /// `p_pe = p_0/(1 - nu^2)` (ADR 0517 D1, plane strain).
    fn p_plane_strain(s: &Setup) -> f64 {
        p_free(s) / (1.0 - s.nu * s.nu)
    }

    /// ADR 0517 D3's axial positions from a face (0) to `len`: cells of `h` over `0.5 (b - a)`, `m = round(1/(h + 0.25))` cells
    /// rising linearly from `h` to `0.25 (b - a)` over the next `0.5 (b - a)`, then cells of `0.25 (b - a)`.
    fn axial(len: f64, h: f64) -> Vec<f64> {
        let w = 0.5 * (B - A);
        let hc = 0.25 * (B - A);
        let rest = len - 2.0 * w;
        let nrest = (rest / hc).round() as usize;
        assert!(
            rest >= -1e-12 && (nrest as f64 * hc - rest).abs() < 1e-12,
            "length {len}"
        );
        let nz = (w / h).round() as usize;
        let mut x: Vec<f64> = (0..=nz).map(|i| w * i as f64 / nz as f64).collect();
        let m = (1.0 / (h / (B - A) + 0.25)).round() as usize;
        let sizes: Vec<f64> = (0..m).map(|i| h + (hc - h) * (i as f64 + 0.5) / m as f64).collect();
        let sum: f64 = sizes.iter().sum();
        let mut z = w;
        for (i, sz) in sizes.iter().enumerate() {
            z += sz * w / sum;
            x.push(if i == m - 1 { 2.0 * w } else { z });
        }
        for i in 1..=nrest {
            x.push(2.0 * w + i as f64 * hc);
        }
        x
    }

    struct Fit {
        setup: Setup,
        model: SolidModel,
        /// The hub's bore nodes by row (plane `k`, `z = zh[k]`) and angle `j`.
        bore: Vec<Vec<usize>>,
        zh: Vec<f64>,
    }

    fn fit_model(s: Setup) -> Fit {
        let h = (B - A) / s.n as f64;
        let half = 0.5 * s.l;
        let mut zh: Vec<f64> = axial(half, h).iter().rev().map(|v| half - v).collect();
        zh[0] = 0.0;
        let last = zh.len() - 1;
        zh[last] = half;
        let mut zm = zh.clone();
        zm.extend(axial(s.e, h).iter().skip(1).map(|v| half + v));
        let nzm = zm.len();
        zm[nzm - 1] = half + s.e;
        let zs: Vec<f64> = (0..nzm)
            .map(|k| {
                if k == 0 || k == nzm - 1 {
                    zm[k]
                } else {
                    zm[k] + s.shift * (zm[k + 1] - zm[k])
                }
            })
            .collect();
        let dth = 0.5 * PI / NTH as f64;
        // The shaft's cross-section: the core, then the band onto the polygon.
        let mut sp: Vec<[f64; 2]> = Vec::new();
        let core = |i: usize, j: usize| i + (NC + 1) * j;
        for j in 0..=NC {
            for i in 0..=NC {
                sp.push([CORE * i as f64 / NC as f64, CORE * j as f64 / NC as f64]);
            }
        }
        let nper = NTH + 2;
        assert_eq!(nper - 1, 2 * NC);
        let cb: Vec<usize> = (0..nper)
            .map(|k| if k <= NC { core(NC, k) } else { core(2 * NC - k, NC) })
            .collect();
        let per: Vec<[f64; 2]> = (0..nper)
            .map(|k| {
                if k == 0 {
                    [A, 0.0]
                } else if k == nper - 1 {
                    [0.0, A]
                } else {
                    let th = (k as f64 - 0.5) * dth;
                    let rv = A / (0.5 * dth).cos();
                    [rv * th.cos(), rv * th.sin()]
                }
            })
            .collect();
        let nb = ((0.6 * s.n as f64).ceil() as usize).max(3);
        let mut band = vec![cb.clone()];
        for l in 1..=nb {
            let t = l as f64 / nb as f64;
            let mut row = Vec::new();
            for k in 0..nper {
                let c = sp[cb[k]];
                let mut p = [(1.0 - t) * c[0] + t * per[k][0], (1.0 - t) * c[1] + t * per[k][1]];
                if k == 0 {
                    p[1] = 0.0;
                }
                if k == nper - 1 {
                    p[0] = 0.0;
                }
                sp.push(p);
                row.push(sp.len() - 1);
            }
            band.push(row);
        }
        let ns2 = sp.len();
        // The hub's cross-section: n uniform cells from a - d to b, angles j Delta.
        let mut hp: Vec<[f64; 2]> = Vec::new();
        for k in 0..=s.n {
            let r = (A - s.d) + k as f64 * (B - A + s.d) / s.n as f64;
            for j in 0..=NTH {
                let th = j as f64 * dth;
                hp.push(if j == 0 {
                    [r, 0.0]
                } else if j == NTH {
                    [0.0, r]
                } else {
                    [r * th.cos(), r * th.sin()]
                });
            }
        }
        let nh2 = hp.len();
        let hub = |k: usize, j: usize| k * (NTH + 1) + j;
        let mut nodes: Vec<Vec3> = Vec::new();
        for &z in &zs {
            nodes.extend(sp.iter().map(|p| [p[0], p[1], z]));
        }
        let hoff = nodes.len();
        for &z in &zh {
            nodes.extend(hp.iter().map(|p| [p[0], p[1], z]));
        }
        let sid = |pl: usize, i: usize| pl * ns2 + i;
        let hid = |pl: usize, i: usize| hoff + pl * nh2 + i;
        let mut conn = Vec::new();
        let (mut shaft_out, mut bore_in) = (Vec::new(), Vec::new());
        let mut e = 0;
        for pl in 0..zs.len() - 1 {
            let hx = |q: [usize; 4]| {
                [
                    sid(pl, q[0]),
                    sid(pl, q[1]),
                    sid(pl, q[2]),
                    sid(pl, q[3]),
                    sid(pl + 1, q[0]),
                    sid(pl + 1, q[1]),
                    sid(pl + 1, q[2]),
                    sid(pl + 1, q[3]),
                ]
            };
            for j in 0..NC {
                for i in 0..NC {
                    conn.extend_from_slice(&hx([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
                    e += 1;
                }
            }
            for l in 0..nb {
                for k in 0..nper - 1 {
                    conn.extend_from_slice(&hx([band[l][k], band[l + 1][k], band[l + 1][k + 1], band[l][k + 1]]));
                    if l == nb - 1 {
                        shaft_out.push(FaceRef { element: e, face: 3 });
                    }
                    e += 1;
                }
            }
        }
        for pl in 0..zh.len() - 1 {
            let hx = |q: [usize; 4]| {
                [
                    hid(pl, q[0]),
                    hid(pl, q[1]),
                    hid(pl, q[2]),
                    hid(pl, q[3]),
                    hid(pl + 1, q[0]),
                    hid(pl + 1, q[1]),
                    hid(pl + 1, q[2]),
                    hid(pl + 1, q[3]),
                ]
            };
            for k in 0..s.n {
                for j in 0..NTH {
                    conn.extend_from_slice(&hx([hub(k, j), hub(k + 1, j), hub(k + 1, j + 1), hub(k, j + 1)]));
                    if k == 0 {
                        bore_in.push(FaceRef { element: e, face: 5 });
                    }
                    e += 1;
                }
            }
        }
        let (mut xs, mut ys, mut zset) = (Vec::new(), Vec::new(), Vec::new());
        for (v, p) in nodes.iter().enumerate() {
            if p[0] == 0.0 {
                xs.push(v);
            }
            if p[1] == 0.0 {
                ys.push(v);
            }
            if p[2] == 0.0 || s.all_uz {
                zset.push(v);
            }
        }
        let bore: Vec<Vec<usize>> = (0..zh.len())
            .map(|pl| (0..=NTH).map(|j| hid(pl, hub(0, j))).collect())
            .collect();
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, conn).unwrap();
        let ne = mesh.n_elements();
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        mesh.add_surface_set("bore", bore_in).unwrap();
        mesh.add_surface_set("shaft_out", shaft_out).unwrap();
        mesh.add_node_set("x0", xs).unwrap();
        mesh.add_node_set("y0", ys).unwrap();
        mesh.add_node_set("z0", zset).unwrap();
        let mut model = SolidModel::new(mesh);
        model
            .add_material("all", Material::isotropic_elastic(E, s.nu, 1.0, 0.0).unwrap())
            .unwrap();
        model.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
        model.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
        model.add_constraint(Constraint::displacement("z0", 2, 0.0)).unwrap();
        println!(
            "{s:?}: {} nodes, {ne} elements, {} hub planes, {} shaft planes, {nb} band layers",
            model.mesh().nodes().len(),
            zh.len(),
            zs.len()
        );
        Fit {
            setup: s,
            model,
            bore,
            zh,
        }
    }

    /// ADR 0542 D4: the hub at `n` with the total Lagrangian body and the interference `d`, ADR 0517's options.
    pub fn tl(n: usize, d: f64) -> Result<ContactResult, rcfd_fem::error::FemError> {
        let s = Setup { n, d, ..BASE };
        let fit = fit_model(s);
        let h = (B - A) / s.n as f64;
        let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E / h);
        let o = ContactOptions::new(1, 1e-6 * s.d)
            .newton(NewtonOptions::new(1).max_iterations(30))
            .kinematics(rcfd_fem::contact::Kinematics::TotalLagrangian);
        solve(&fit.model, &[], &[], &[pair], &o)
    }
}

/// B: the three solves give the bits they gave before F-618.
#[test]
fn b_newton_default_bits_f618() {
    let g3 = hub::tl(4, 1e-3)
        .expect_err("ADR 0542 Q1: refused at the first tangent")
        .to_string();
    println!("G3 error: {g3}");
    let got = [
        ("G1 cantilever, plain and with the device", digest(&g1())),
        (
            "G2 total Lagrangian hub n = 4, d = 1e-4",
            digest(&flat(&hub::tl(4, 1e-4).unwrap())),
        ),
        (
            "G3 total Lagrangian hub n = 4, d = 1e-3, error text",
            digest_bytes(g3.as_bytes()),
        ),
    ];
    for (name, d) in &got {
        println!("B digest {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("the digests of the unchanged library are written in before the library code");
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, ed, "B {name}: {d:#018x} against {ed:#018x}");
    }
}
