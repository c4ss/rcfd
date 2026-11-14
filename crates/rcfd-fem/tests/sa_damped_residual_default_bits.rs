//! F-532 (ADR 0464): the bit record of `damped_modes` without the residual vectors (check D1 of the registration).
//!
//! This file uses only the API of main 80ebbd1e: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-532, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The set-ups are F-414's (ADR 0450): the bar with a tip dashpot on 40, 20 and 10 modes, the bar
//! with dashpots between two nodes, the cantilever with Rayleigh damping and with a loss factor.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

/// Every field of a result, as a flat list of numbers (flags as 0 and 1, counts as numbers, notes by their bytes).
fn flat(d: &DampedModes) -> Vec<f64> {
    let mut v = Vec::new();
    v.push(d.modes.len() as f64);
    for m in &d.modes {
        v.extend([
            m.eigenvalue.re,
            m.eigenvalue.im,
            m.decay_rate,
            m.damped_omega,
            m.frequency,
            m.damping_ratio,
            f64::from(u8::from(m.oscillating)),
            m.residual,
        ]);
        for s in &m.shape {
            v.extend([s.re, s.im]);
        }
    }
    v.push(d.roots.len() as f64);
    for r in &d.roots {
        v.extend([r.re, r.im]);
    }
    v.extend(d.omegas.iter().copied());
    v.extend([d.basis as f64, d.oscillating as f64, d.non_oscillating as f64]);
    v.push(d.defects.len() as f64);
    for g in &d.defects {
        v.extend(g.indices.iter().map(|&i| i as f64));
        v.extend([g.mean.re, g.mean.im, g.spread]);
    }
    for n in &d.notes {
        v.push(n.len() as f64);
        v.extend(n.bytes().map(f64::from));
    }
    v
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn modal(m: usize) -> ModalOptions {
    ModalOptions::new(m)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap());
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn bar() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.01, 0.01], [100, 1, 1]).unwrap());
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn impedance() -> f64 {
    1e-4 * (E_MOD * RHO).sqrt()
}

fn tip(c: f64, modes: usize) -> DampedModes {
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(modes));
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

/// The node of the bar at `x = 0.5` with the `y`, `z` of node `a`.
fn mid_node(model: &SolidModel, a: usize) -> usize {
    let nodes = model.mesh().nodes();
    let p = nodes[a];
    nodes
        .iter()
        .position(|q| (q[0] - 0.5).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-12 && (q[2] - p[2]).abs() < 1e-12)
        .unwrap()
}

fn pairs(c: f64, modes: usize) -> DampedModes {
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(modes));
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::between(a, mid_node(&model, a), 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

fn rayleigh() -> DampedModes {
    let model = cantilever();
    let u = modal_analysis(&model, &modal(12)).unwrap();
    let w: Vec<f64> = u.eigenvalues.iter().map(|l| l.sqrt()).collect();
    let o = DampedOptions::with_modal(modal(12))
        .rayleigh(0.2 * w[0], 0.5 / w[11])
        .threads(4);
    damped_modes(&model, &o).unwrap()
}

fn loss() -> DampedModes {
    let o = DampedOptions::with_modal(modal(12)).loss_factor("all", LossFactor::Constant(0.04));
    damped_modes(&cantilever(), &o).unwrap()
}

/// Printed by this file at main 80ebbd1e (before any library code of F-532).
const BASE: [(&str, u64); 9] = [
    ("bar c = Z/2, 40 modes", 0x69d4bba9ff8a7a34),
    ("bar c = Z/2, 20 modes", 0xb556534410ce88a3),
    ("bar c = Z/2, 10 modes", 0xa8cad80cbfb2a934),
    ("bar c = 2 Z, 40 modes", 0x24c56a77f5f378b2),
    ("bar c = 2 Z, 20 modes", 0xae5f958378b1e232),
    ("bar c = 0.9 Z, 20 modes", 0x31ffd91883263cb8),
    ("bar two-node dashpots 0.9 Z, 20 modes", 0x0c7c69e9306605d0),
    ("cantilever Rayleigh, 4 threads", 0x5f5ac9710c88e28d),
    ("cantilever loss factor 0.04", 0x4991afdaba6db050),
];

#[test]
fn d1_default_results_bit_identical_to_f414() {
    let z = impedance();
    let got = [
        digest(flat(&tip(0.5 * z, 40))),
        digest(flat(&tip(0.5 * z, 20))),
        digest(flat(&tip(0.5 * z, 10))),
        digest(flat(&tip(2.0 * z, 40))),
        digest(flat(&tip(2.0 * z, 20))),
        digest(flat(&tip(0.9 * z, 20))),
        digest(flat(&pairs(0.9 * z, 20))),
        digest(flat(&rayleigh())),
        digest(flat(&loss())),
    ];
    let mut same = 0;
    for ((name, want), g) in BASE.iter().zip(got) {
        println!("    (\"{name}\", 0x{g:016x}),");
        if *want == g {
            same += 1;
        }
    }
    println!("D1 {same} of {} digests equal to main 80ebbd1e", BASE.len());
    assert_eq!(same, BASE.len());
}
