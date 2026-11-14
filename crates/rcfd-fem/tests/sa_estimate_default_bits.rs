//! F-432 (ADR 0478) D1: the bits of the default paths next to the patch recovery (the static solve, the integration-point
//! stress of F-376 and its nodal average) on the two-layer plate with a hole and the Hex8 manufactured solution at n = 4,
//! recorded on main 57808e24 before any library line of F-432 was written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::hex20::{faces, upgrade_to_hex20_mapped};
use rcfd_fem::loads::body_force_loads;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
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

/// The plate of ADR 0449 with two layers through the thickness (ADR 0478 reading a6).
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

fn flat(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flat_map(|x| x.iter().copied()).collect()
}

fn flat_opt(v: &[Option<[f64; 6]>]) -> Vec<f64> {
    v.iter().flat_map(|x| x.unwrap_or([f64::NAN; 6])).collect()
}

fn digests() -> [u64; 6] {
    let p = plate();
    let r = solve_static(&p, &StaticOptions::with_solver(StaticSolver::SparseDirect)).unwrap();
    let up = r.displacement_values().to_vec();
    let fp = IntegrationPointField::compute(&p, &up).unwrap();
    let np = nodal_average(&p, &fp, fp.stress(), None).unwrap();

    let mesh = hex_box([0.0; 3], [1.0; 3], [4; 3]).unwrap();
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
    let rm = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
    let um = rm.displacement_values().to_vec();
    let fm = IntegrationPointField::compute(&m, &um).unwrap();
    let nm = nodal_average(&m, &fm, fm.stress(), None).unwrap();
    [
        fnv(bits(&up)),
        fnv(bits(&flat(fp.stress()))),
        fnv(bits(&flat_opt(&np))),
        fnv(bits(&um)),
        fnv(bits(&flat(fm.stress()))),
        fnv(bits(&flat_opt(&nm))),
    ]
}

/// Prints the digests (run at main 57808e24 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D1 (ADR 0478); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 57808e24 (ADR 0478 D8).
const BASE: [u64; 6] = [
    0xe658_25d3_d747_c1c8,
    0x76ec_3650_b237_fc64,
    0xdc45_73c1_1da5_eeff,
    0x2a2b_33ac_721b_7d64,
    0x7f52_36f9_e7f6_e747,
    0xb5af_cc25_dfd2_b362,
];

#[test]
fn d1_default_paths_unchanged() {
    let got = digests();
    let names = [
        "plate u",
        "plate point stress",
        "plate nodal average",
        "mms u",
        "mms point stress",
        "mms nodal average",
    ];
    let mut bad = Vec::new();
    for i in 0..6 {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", names[i], got[i], BASE[i]));
        }
    }
    println!("D1: {} of 6 digests equal", 6 - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
