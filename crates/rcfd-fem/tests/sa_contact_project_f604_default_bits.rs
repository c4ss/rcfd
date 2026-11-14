//! F-604 (ADR 0517 amendment 1): B1, the default-bits record of the closest-point fix in `contact.rs` `project`, and PJ1, a
//! small face far from the origin. B1's digests were printed by this file run on the unchanged library (8028545c,
//! `f604-scratch/b1-base.log`) and written into `EXPECTED` before the fix. The two models are copies of ADR 0513's
//! `fit_model` (`tests/sa_contact_fit_f599.rs`) and ADR 0517's (`tests/sa_contact_hub_f604.rs`); the blocks are ADR 0507
//! B1's (`tests/sa_contact.rs`).

use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, ContactSet, Target};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, Load, Material, SolidModel};

/// Digests of 8028545c (unchanged library) in the order of the run, from `f604-scratch/b1-base.log`.
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("blocks pressed", 0xf561_9b60_0e92_0c5f),
    ("ring fit (ADR 0507 set-up)", 0x639f_c374_b3a9_e917),
    ("hub n = 4", 0x5f0b_2378_5337_e56e),
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

/// The displacement, the reactions, every contactor node's force, pressure, gap and local coordinates, and every Newton
/// residual and penetration.
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
            v.push(x.penetration);
        }
    }
    v
}

fn elastic(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, 1.0, 0.0).unwrap()
}

/// ADR 0507 B1: two blocks pressed.
fn blocks_pressed() -> ContactResult {
    let e = 2.0e11;
    let lower = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let upper = hex_box([0.0, 0.0, 1.0], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(e, 0.3)).unwrap();
    m.add_material("upper.all", elastic(e, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("lower.zmin", 2, 0.0))
        .unwrap();
    for p in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{p}.xmin"), 0, 0.0))
            .unwrap();
        m.add_constraint(Constraint::displacement(&format!("{p}.ymin"), 1, 0.0))
            .unwrap();
    }
    m.add_load(Load::pressure("upper.zmax", 1.0e8)).unwrap();
    let pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * e / 0.5);
    let o = ContactOptions::new(2, 1e-12).newton(NewtonOptions::new(1).max_iterations(30));
    solve(&m, &[], &[], &[pair], &o).unwrap()
}

#[allow(dead_code)]
mod ring {
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Target};
    use rcfd_fem::nonlinear::NewtonOptions;
    use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
    use std::f64::consts::PI;

    const E_FIT: f64 = 2.0e11;
    const A_FIT: f64 = 1.0;
    const B_FIT: f64 = 2.0;
    const T_FIT: f64 = 0.1;
    const N_FIT: usize = 64;

    /// The `u_z` constraints of ADR 0513's probe.
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Zbc {
        /// ADR 0507: `u_z = 0` on every node of `z = 0`.
        Bottom,
        /// Plane strain: `u_z = 0` on every node of `z = 0` and of `z = T`.
        Both,
        /// `u_z` free: `u_z = 0` at three bottom nodes of each body only.
        Minimal,
        /// `u_z = 0` on the mid-plane nodes (an even number of layers).
        Mid,
    }

    #[derive(Clone, Copy, Debug)]
    struct Setup {
        nz: usize,
        nr: usize,
        zbc: Zbc,
        nu: f64,
        d: f64,
    }

    const ADR_0507: Setup = Setup {
        nz: 1,
        nr: 8,
        zbc: Zbc::Bottom,
        nu: 0.3,
        d: 1e-3,
    };

    struct Fit {
        setup: Setup,
        model: SolidModel,
        ring: Vec<usize>,
        shaft: Vec<usize>,
        /// Nodes per plane.
        n2: usize,
    }

    /// ADR 0507's `fit_model` with the parameters of `s` (ADR 0513 D4); `ADR_0507` is ADR 0507's model.
    fn fit_model(s: Setup) -> Fit {
        let (nz, nr, d) = (s.nz, s.nr, s.d);
        let dth = 2.0 * PI / N_FIT as f64;
        let mut pts: Vec<[f64; 2]> = Vec::new();
        let m = 16;
        let core = |i: usize, j: usize| i + (m + 1) * j;
        for j in 0..=m {
            for i in 0..=m {
                pts.push([-0.5 + i as f64 / m as f64, -0.5 + j as f64 / m as f64]);
            }
        }
        let mut per = Vec::new();
        for j in 8..=m {
            per.push(core(m, j));
        }
        for i in (0..m).rev() {
            per.push(core(i, m));
        }
        for j in (0..m).rev() {
            per.push(core(0, j));
        }
        for i in 1..=m {
            per.push(core(i, 0));
        }
        for j in 1..8 {
            per.push(core(m, j));
        }
        assert_eq!(per.len(), N_FIT);
        let av = A_FIT / (dth / 2.0).cos();
        let layers = 6;
        let mut band = vec![per.clone()];
        for l in 1..=layers {
            let t = l as f64 / layers as f64;
            let mut row = Vec::new();
            for (p, &q) in per.iter().enumerate() {
                let th = (p as f64 + 0.5) * dth;
                let c = pts[q];
                pts.push([
                    (1.0 - t) * c[0] + t * av * th.cos(),
                    (1.0 - t) * c[1] + t * av * th.sin(),
                ]);
                row.push(pts.len() - 1);
            }
            band.push(row);
        }
        let ring0 = pts.len();
        let rk = |k: usize| (A_FIT - d) + k as f64 * (B_FIT - A_FIT + d) / nr as f64;
        let ring = |j: usize, k: usize| ring0 + (j % N_FIT) + N_FIT * k;
        for k in 0..=nr {
            for j in 0..N_FIT {
                let th = j as f64 * dth;
                pts.push([rk(k) * th.cos(), rk(k) * th.sin()]);
            }
        }
        let n2 = pts.len();
        let mut nodes: Vec<Vec3> = Vec::new();
        for l in 0..=nz {
            let z = T_FIT * l as f64 / nz as f64;
            nodes.extend(pts.iter().map(|p| [p[0], p[1], z]));
        }
        let mut conn = Vec::new();
        let (mut shaft_el, mut ring_el, mut shaft_out, mut ring_in) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut e = 0;
        for lz in 0..nz {
            let o = lz * n2;
            let hex = |q: [usize; 4]| {
                [
                    q[0] + o,
                    q[1] + o,
                    q[2] + o,
                    q[3] + o,
                    q[0] + o + n2,
                    q[1] + o + n2,
                    q[2] + o + n2,
                    q[3] + o + n2,
                ]
            };
            for j in 0..m {
                for i in 0..m {
                    conn.extend_from_slice(&hex([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
                    shaft_el.push(e);
                    e += 1;
                }
            }
            for l in 0..layers {
                for p in 0..N_FIT {
                    let p1 = (p + 1) % N_FIT;
                    conn.extend_from_slice(&hex([band[l][p], band[l + 1][p], band[l + 1][p1], band[l][p1]]));
                    shaft_el.push(e);
                    if l == layers - 1 {
                        shaft_out.push(FaceRef { element: e, face: 3 });
                    }
                    e += 1;
                }
            }
            for k in 0..nr {
                for j in 0..N_FIT {
                    conn.extend_from_slice(&hex([ring(j, k), ring(j, k + 1), ring(j + 1, k + 1), ring(j + 1, k)]));
                    ring_el.push(e);
                    if k == 0 {
                        ring_in.push(FaceRef { element: e, face: 5 });
                    }
                    e += 1;
                }
            }
        }
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, conn).unwrap();
        let ne = mesh.n_elements();
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        mesh.add_surface_set("ring_in", ring_in).unwrap();
        mesh.add_surface_set("shaft_out", shaft_out).unwrap();
        let column = |v: usize| (0..=nz).map(|l| v + l * n2).collect::<Vec<_>>();
        let zset: Vec<usize> = match s.zbc {
            Zbc::Bottom => (0..n2).collect(),
            Zbc::Both => (0..n2).chain(nz * n2..(nz + 1) * n2).collect(),
            Zbc::Minimal => vec![
                core(8, 8),
                core(m, 8),
                core(8, m),
                ring(0, nr),
                ring(16, nr),
                ring(32, nr),
            ],
            Zbc::Mid => {
                assert!(nz % 2 == 0, "Mid needs an even number of layers");
                (nz / 2 * n2..(nz / 2 + 1) * n2).collect()
            }
        };
        mesh.add_node_set("zfix", zset).unwrap();
        mesh.add_node_set("centre", column(core(8, 8))).unwrap();
        mesh.add_node_set("shaft_x", column(core(m, 8))).unwrap();
        mesh.add_node_set("ring_0", column(ring(0, nr))).unwrap();
        let mut r90 = column(ring(16, nr));
        r90.extend(column(ring(48, nr)));
        mesh.add_node_set("ring_90_270", r90).unwrap();
        let mut model = SolidModel::new(mesh);
        model
            .add_material("all", Material::isotropic_elastic(E_FIT, s.nu, 1.0, 0.0).unwrap())
            .unwrap();
        model.add_constraint(Constraint::displacement("zfix", 2, 0.0)).unwrap();
        for (set, c) in [
            ("centre", 0),
            ("centre", 1),
            ("shaft_x", 1),
            ("ring_0", 1),
            ("ring_90_270", 0),
        ] {
            model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
        }
        Fit {
            setup: s,
            model,
            ring: ring_el,
            shaft: shaft_el,
            n2,
        }
    }

    /// ADR 0513's solve of ADR 0507's set-up.
    pub fn solved() -> ContactResult {
        let s = ADR_0507;
        let fit = fit_model(s);
        let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
        let o = ContactOptions::new(1, 1e-6 * s.d).newton(NewtonOptions::new(1).max_iterations(30));
        solve(&fit.model, &[], &[], &[pair], &o).unwrap()
    }
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

    /// ADR 0517's solve at `n = 4`.
    pub fn solved() -> ContactResult {
        let s = Setup { n: 4, ..BASE };
        let fit = fit_model(s);
        let h = (B - A) / s.n as f64;
        let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E / h);
        let o = ContactOptions::new(1, 1e-6 * s.d).newton(NewtonOptions::new(1).max_iterations(30));
        solve(&fit.model, &[], &[], &[pair], &o).unwrap()
    }
}

/// B1: the three contact solves give the bits they gave before the fix.
#[test]
fn b1_contact_default_bits() {
    let got = [
        ("blocks pressed", digest(&flat(&blocks_pressed()))),
        ("ring fit (ADR 0507 set-up)", digest(&flat(&ring::solved()))),
        ("hub n = 4", digest(&flat(&hub::solved()))),
    ];
    for (name, d) in &got {
        println!("B1 digest {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("the digests of the unchanged library are written in before the fix");
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, ed, "B1 {name}: {d:#018x} against {ed:#018x}");
    }
}

/// PJ1: a contactor over a small face far from the origin is found, with its gap.
#[test]
fn pj1_small_face_far_from_the_origin() {
    let s = 1e-3;
    let lower = hex_box([10.0, 10.0, 10.0 - s], [2.0 * s, 2.0 * s, s], [1, 1, 1]).unwrap();
    let upper = hex_box([10.0 + 0.3 * s, 10.0 + 0.3 * s, 10.0 - 1e-3 * s], [s, s, s], [1, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(2.0e11, 0.3)).unwrap();
    m.add_material("upper.all", elastic(2.0e11, 0.3)).unwrap();
    let pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 1.0e14);
    let o = ContactOptions::new(1, 1e-12);
    let set = ContactSet::new(&m, &[pair], &o).unwrap();
    let u = vec![0.0; m.dof_map().n_equations()];
    let nodes = set.evaluate(&u).unwrap();
    assert_eq!(nodes.len(), 4);
    let g0 = -1e-3 * s;
    let mut missed = Vec::new();
    for n in &nodes {
        let e = n.gap.map_or(f64::INFINITY, |g| (g / g0 - 1.0).abs());
        println!("PJ1 node {}: gap {:?}, relative error {e:.3e}", n.node, n.gap);
        if !(e.is_finite() && e <= 1e-9) {
            missed.push((n.node, n.gap));
        }
    }
    assert!(missed.is_empty(), "PJ1 missed: {missed:?}");
}
