//! F-568 (ADR 0616) B1: the bits of every value of the F-432 estimate (`element_errors`, `estimate`, `solution_norm`,
//! `relative`, `ranked` and the recovered nodal stress) on the two-layer plate with a hole and the case-D manufactured
//! solution of F-378 on Hex8, Tet10 and Hex20 at n = 4, recorded on main b61032f4 before any library line of F-568 was
//! written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::estimate::{energy_error, PatchOptions};
use rcfd_fem::hex20::{faces, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::body_force_loads;
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use std::collections::BTreeSet;
use std::f64::consts::PI;

/// The FNV-1a hash of a sequence of 64-bit words.
fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn bits(v: &[f64]) -> impl Iterator<Item = u64> + '_ {
    v.iter().map(|x| x.to_bits())
}

const HR: f64 = 0.005;
const HA: f64 = 0.1;
const HB: f64 = 0.05;
const HT: f64 = 0.001;
const S0: f64 = 50e6;

/// The map of ADR 0449 (sa_fatigue.rs).
fn plate_map(p: Vec3) -> Vec3 {
    let (s, q) = (p[0], p[1]);
    let (circle, outer) = if q == 1.0 {
        ([0.0, HR], [0.0, HB])
    } else {
        let th = 0.5 * PI * q;
        let ell = q * (HA + HB);
        let o = if ell <= HB { [HA, ell] } else { [HA - (ell - HB), HB] };
        ([HR * th.cos(), HR * th.sin()], o)
    };
    let g = ((4.0 * s).exp() - 1.0) / (4f64.exp() - 1.0);
    [
        circle[0] + g * (outer[0] - circle[0]),
        circle[1] + g * (outer[1] - circle[1]),
        p[2],
    ]
}

/// The plate of ADR 0449 with two layers through the thickness (ADR 0478 reading a6; as in sa_estimate.rs).
fn plate() -> SolidModel {
    let (mut mesh, _) =
        upgrade_to_hex20_mapped(&hex_box([0.0; 3], [1.0, 1.0, HT], [24, 24, 2]).unwrap(), plate_map).unwrap();
    let right: Vec<_> = mesh
        .surface_set("xmax")
        .unwrap()
        .iter()
        .copied()
        .filter(|f| {
            let nodes = mesh.element_nodes(f.element);
            faces()[f.face]
                .iter()
                .all(|&a| (mesh.nodes()[nodes[a]][0] - HA).abs() <= 1e-12)
        })
        .collect();
    mesh.add_surface_set("right", right).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("right", [S0, 0.0, 0.0])).unwrap();
    m
}

const K: [[f64; 3]; 3] = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: [[f64; 3]; 3] = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];
const NU: f64 = 0.3;

fn trig_u(x: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| (0..3).map(|j| (PI * K[i][j] * x[j] + PHI[i][j]).sin()).product())
}

/// The body force of F-378 (sb_mms.rs arithmetic 3) from the analytic second derivatives.
fn body_force(x: Vec3) -> Vec3 {
    let lambda = NU / ((1.0 + NU) * (1.0 - 2.0 * NU));
    let mu = 1.0 / (2.0 * (1.0 + NU));
    let mut h = [[[0.0; 3]; 3]; 3];
    for (i, hi) in h.iter_mut().enumerate() {
        let k: [f64; 3] = std::array::from_fn(|j| PI * K[i][j]);
        let s: [f64; 3] = std::array::from_fn(|j| (k[j] * x[j] + PHI[i][j]).sin());
        let c: [f64; 3] = std::array::from_fn(|j| (k[j] * x[j] + PHI[i][j]).cos());
        for m in 0..3 {
            for n in 0..3 {
                let mut p = 1.0;
                if m == n {
                    p *= -k[m] * k[m];
                    for sj in s {
                        p *= sj;
                    }
                } else {
                    p *= k[m] * c[m] * k[n] * c[n];
                    for (j, sj) in s.iter().enumerate() {
                        if j != m && j != n {
                            p *= sj;
                        }
                    }
                }
                hi[m][n] = p;
            }
        }
    }
    std::array::from_fn(|i| {
        let grad_div: f64 = (0..3).map(|k| h[k][i][k]).sum();
        let lap: f64 = (0..3).map(|j| h[i][j][j]).sum();
        -((lambda + mu) * grad_div + mu * lap)
    })
}

/// The six digests of one model's estimate.
fn estimate_digests(model: &SolidModel, u: &[f64]) -> [u64; 6] {
    let (rec, est) = energy_error(model, u, &PatchOptions::new()).unwrap();
    let nodal: Vec<f64> = rec.nodal().iter().flat_map(|x| x.unwrap_or([f64::NAN; 6])).collect();
    [
        fnv(bits(est.element_errors())),
        fnv([est.estimate().to_bits()]),
        fnv([est.solution_norm().to_bits()]),
        fnv([est.relative().to_bits()]),
        fnv(est.ranked().iter().map(|&e| e as u64)),
        fnv(bits(&nodal)),
    ]
}

/// Case D of F-378 at n = 4 on one mesh (the default solver).
fn case_d(mesh: SolidMesh) -> (SolidModel, Vec<f64>) {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(1.0, NU, 0.0, 0.0).unwrap())
        .unwrap();
    let mut fixed = BTreeSet::new();
    for f in BOX_FACES {
        fixed.extend(m.mesh().node_set(f).unwrap().iter().copied());
    }
    let mut pres = Vec::new();
    for &node in &fixed {
        for (k, &v) in trig_u(m.mesh().nodes()[node]).iter().enumerate() {
            pres.push(Prescribed::new(node, k, v));
        }
    }
    let forces = body_force_loads(&m, body_force).unwrap();
    let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
    let u = r.displacement_values().to_vec();
    (m, u)
}

const NAMES: [&str; 4] = ["plate", "Hex8 n = 4", "Tet10 n = 4", "Hex20 n = 4"];
const VALUES: [&str; 6] = [
    "element_errors",
    "estimate",
    "solution_norm",
    "relative",
    "ranked",
    "nodal",
];

fn digests() -> [[u64; 6]; 4] {
    let p = plate();
    let r = solve_static(&p, &StaticOptions::with_solver(StaticSolver::SparseDirect)).unwrap();
    let up = r.displacement_values().to_vec();
    let (o, l, d) = ([0.0; 3], [1.0; 3], [4; 3]);
    let meshes = [
        hex_box(o, l, d).unwrap(),
        upgrade_to_tet10(&tet_box(o, l, d).unwrap()).unwrap().0,
        upgrade_to_hex20(&hex_box(o, l, d).unwrap()).unwrap().0,
    ];
    let mut out = [[0; 6]; 4];
    out[0] = estimate_digests(&p, &up);
    for (k, mesh) in meshes.into_iter().enumerate() {
        let (m, u) = case_d(mesh);
        out[k + 1] = estimate_digests(&m, &u);
    }
    out
}

/// Prints the digests (run at main b61032f4 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0616); run at the base to record BASE"]
fn print_base_digests() {
    for (name, row) in NAMES.iter().zip(digests()) {
        println!("// {name}");
        println!("[{}],", row.map(|d| format!("0x{d:016x}")).join(", "));
    }
}

/// The digests printed at main b61032f4 (ADR 0616 D4).
const BASE: [[u64; 6]; 4] = [
    // plate
    [
        0xa5dd62fa1037c1c8,
        0x9964e32b31e965ec,
        0xba35f24cd14f9b02,
        0xd35a41faafc6256a,
        0x6409503443013361,
        0x6b2d3a38838a6cf9,
    ],
    // Hex8 n = 4
    [
        0x98fec1593340b9fe,
        0xaa0e74bb185515b7,
        0x1ad89d2cbc3e1e77,
        0x3dc3ddb540700c99,
        0xf6b45e95951b9585,
        0xa76187b83eb3d83e,
    ],
    // Tet10 n = 4
    [
        0xfcb8335f386c37e5,
        0x8fa2a43f6542d53f,
        0x047f292f2c2843d0,
        0x7a8f7af6463257de,
        0x0dbcf93e77f77e11,
        0x51818c8e54383de2,
    ],
    // Hex20 n = 4
    [
        0xda91f19676e7fd3c,
        0xd72679938db4ffdd,
        0x9b22a8858dbf1555,
        0xc0da1f6f07f22ba3,
        0xb4391c0588cf4685,
        0xbb4cabb5de8283de,
    ],
];

#[test]
fn b1_estimate_bits_unchanged() {
    let got = digests();
    let mut bad = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        for (j, value) in VALUES.iter().enumerate() {
            if got[i][j] != BASE[i][j] {
                bad.push(format!("{name} {value}: 0x{:016x} != 0x{:016x}", got[i][j], BASE[i][j]));
            }
        }
    }
    println!("B1: {} of 24 digests equal", 24 - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
