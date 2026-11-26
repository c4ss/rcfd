//! F-674 (ADR 0601): `full_rotor` and the expansion of a cyclic sector with Beam2. Checks G1, O1, L0 to L3, F0 to F3, Z0,
//! ZM, P0 to P3, RQ0 to RQ3, G, B3 and the negative controls NC1, NC2, NC-O and NC-H of the registration (B1 and B2 are the
//! existing tests and `baseline_hashes`). The wheel sector W and the independently built ring are those of ADR 0595
//! (`sa_cyclic_beams_f672.rs`), rebuilt here by the same code.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::cyclic::{
    full_rotor, modal_analysis_harmonic, rotation_matrix, solve_static_harmonic, FullRotor, HarmonicLoad, Mat3,
};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const OMEGA: f64 = 100.0;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// Ring elements per sector.
const NR: usize = 4;
/// Nodes of the sector.
const NQ: usize = 7;
/// The loaded sector nodes of ADR 0595 (not the high node 4, not the hub 6).
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];
/// The nodes loaded by the load check L (the high node too).
const LOADED_L: [usize; 6] = [0, 1, 2, 3, 4, 5];

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

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a * x[k] + b * y[k])
}

/// `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`.
fn p(phi: f64, r: f64, z: f64) -> Vec3 {
    let (n, a, b) = (axis(), ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn ring_section() -> BeamSection {
    BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap()
}

fn spoke_section() -> BeamSection {
    BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap()
}

fn v_ring() -> Vec3 {
    comb(0.8, axis(), 0.6, ea())
}

fn v_spoke() -> Vec3 {
    comb(0.6, eb(), 0.8, axis())
}

fn rot(j: usize) -> Mat3 {
    rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn beam_options() -> CyclicOptions {
    CyclicOptions::new().shells(true).beams(true)
}

fn full_options() -> CyclicOptions {
    beam_options().full_rotor_beams(true)
}

/// The force and moment of sector node `q` (as `sa_cyclic_shells.rs`).
fn nodal_load(q: f64) -> (Vec3, Vec3) {
    (
        [
            1e3 * (1.3 * q + 0.2).sin(),
            1e3 * (0.7 * q + 0.5).cos(),
            1e3 * (2.1 * q + 1.1).sin(),
        ],
        [
            1e2 * (1.9 * q + 0.3).cos(),
            1e2 * (0.4 * q + 0.9).sin(),
            1e2 * (1.1 * q + 1.7).cos(),
        ],
    )
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
}

fn wheel_mesh() -> SolidMesh {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    nodes.push(p(a / 2.0, 0.65, 0.125));
    nodes.push(p(a / 2.0, 0.3, 0.25));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(ring_section(), v_ring()).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(spoke_section(), v_spoke()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..6).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![NR]).unwrap();
    mesh.add_node_set("hub", vec![6]).unwrap();
    for q in LOADED {
        mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
    }
    mesh
}

/// The wheel sector W of ADR 0595 with the cyclic constraint of `options`.
fn sector(options: CyclicOptions) -> SolidModel {
    let mut m = SolidModel::new(wheel_mesh());
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The ring node of sector node `q` in sector `j`.
fn ring_node(j: usize, q: usize) -> usize {
    match q {
        0..=NR => (NR * j + q) % (NR * N),
        5 => NR * N + j,
        6 => NR * N + N + j,
        _ => unreachable!("sector node"),
    }
}

/// The full ring of beams of ADR 0595, built independently, orientations `R^j v` in sector `j`.
fn ring() -> SolidModel {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..NR * N).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.65, 0.125));
    }
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.3, 0.25));
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    for j in 0..N {
        let r = rot(j);
        let conn: Vec<usize> = (0..NR).flat_map(|i| [ring_node(j, i), ring_node(j, i + 1)]).collect();
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(ring_section(), apply(&r, v_ring())).unwrap())
            .unwrap();
        let conn = vec![ring_node(j, 6), ring_node(j, 5), ring_node(j, 5), ring_node(j, 2)];
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(spoke_section(), apply(&r, v_spoke())).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..6 * N).collect()).unwrap();
    mesh.add_node_set("hub", (0..N).map(|j| ring_node(j, 6)).collect())
        .unwrap();
    for j in 0..N {
        for q in LOADED {
            let rn = ring_node(j, q);
            mesh.add_node_set(&format!("p{rn}"), vec![rn]).unwrap();
        }
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

/// The full-rotor model F of ADR 0601: the mesh of `rotor` with a node set `r<p>` on every node, the material and the
/// clamp.
fn rotor_model(mesh: &SolidMesh) -> SolidModel {
    let mut mesh = mesh.clone();
    for p in 0..mesh.nodes().len() {
        mesh.add_node_set(&format!("r{p}"), vec![p]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

/// `(cos(j theta), sin(j theta))`, `theta = 2 pi k / N`.
fn phase(k: usize, j: usize) -> (f64, f64) {
    let t = 2.0 * PI * ((j * k) % N) as f64 / N as f64;
    (t.cos(), t.sin())
}

/// A vector over the unknowns of F mapped to the ring's numbering by the node map `rotor.node(j, q) -> ring_node(j, q)`.
fn to_ring(rotor: &FullRotor, fmodel: &SolidModel, ring: &SolidModel, v: &[f64]) -> Vec<f64> {
    let (fd, rd) = (fmodel.dof_map(), ring.dof_map());
    let mut out = vec![0.0; rd.n_equations()];
    for j in 0..N {
        for q in 0..NQ {
            let (f, r) = (rotor.node(j, q), ring_node(j, q));
            for c in 0..6 {
                out[rd.equation(r, c).unwrap()] = v[fd.equation(f, c).unwrap()];
            }
        }
    }
    out
}

/// `(e_u, e_theta)` of two fields in the ring's numbering, relative to the max-norm of `reference`.
fn field_error(ring: &SolidModel, x: &[f64], reference: &[f64]) -> (f64, f64) {
    let rd = ring.dof_map();
    let (mut du, mut dt, mut mu, mut mt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for node in 0..ring.mesh().nodes().len() {
        for c in 0..6 {
            let e = rd.equation(node, c).unwrap();
            let d = (x[e] - reference[e]).abs();
            if c < 3 {
                du = du.max(d);
                mu = mu.max(reference[e].abs());
            } else {
                dt = dt.max(d);
                mt = mt.max(reference[e].abs());
            }
        }
    }
    (du / mu, dt / mt)
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |a, x| a.max(x.abs()))
}

fn inf_norm(a: &CsrMatrix) -> f64 {
    (0..a.n)
        .map(|r| {
            a.val[a.row_ptr[r]..a.row_ptr[r + 1]]
                .iter()
                .map(|v| v.abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max)
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The S0 loads of ADR 0595 on the ring (nodal forces and moments `R^j f`, `R^j m`, the centrifugal load).
fn ring_s0() -> SolidModel {
    let mut full = ring();
    full.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
    for q in LOADED {
        let (f, mo) = nodal_load(q as f64);
        for j in 0..N {
            let r = rot(j);
            let set = format!("p{}", ring_node(j, q));
            full.add_load(Load::nodal_force(&set, apply(&r, f))).unwrap();
            full.add_load(Load::nodal_moment(&set, apply(&r, mo))).unwrap();
        }
    }
    full
}

/// The S0 nodal loads of ADR 0595 as a vector over the sector's unknowns.
fn s0_vector(m: &SolidModel) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut v = vec![0.0; dofs.n_equations()];
    for q in LOADED {
        let (f, mo) = nodal_load(q as f64);
        for c in 0..3 {
            v[dofs.equation(q, c).unwrap()] = f[c];
            v[dofs.equation(q, 3 + c).unwrap()] = mo[c];
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------- G1, O1

#[test]
fn g1_o1_full_rotor_copies_and_orientations() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let full = ring();
    let mut fails = Vec::new();
    // G1: the node map is a bijection onto the ring's nodes, positions within 1e-14.
    let nf = rotor.mesh().nodes().len();
    let mut ring_of = vec![usize::MAX; nf];
    let mut consistent_map = nf == full.mesh().nodes().len();
    let mut dx = 0.0f64;
    for j in 0..N {
        for q in 0..NQ {
            let (f, r) = (rotor.node(j, q), ring_node(j, q));
            if ring_of[f] != usize::MAX && ring_of[f] != r {
                consistent_map = false;
            }
            ring_of[f] = r;
            let (a, b) = (rotor.mesh().nodes()[f], full.mesh().nodes()[r]);
            for c in 0..3 {
                dx = dx.max((a[c] - b[c]).abs());
            }
        }
    }
    let mut seen = ring_of.clone();
    seen.sort_unstable();
    seen.dedup();
    let bijection = consistent_map && !ring_of.contains(&usize::MAX) && seen.len() == nf;
    println!(
        "G1: {nf} nodes, bijection {bijection}, max |x_F - x_ring| {dx:.4e} (limit <= 1e-14) {}",
        met(bijection && dx <= 1e-14)
    );
    if !(bijection && dx <= 1e-14) {
        fails.push(format!("G1 {bijection} {dx:e}"));
    }
    // O1: one block per copy of each Beam2 block, orientation R^j v.
    let blocks = rotor.mesh().blocks();
    let mut ok_struct = blocks.len() == 2 * N;
    let mut copy0_bits = true;
    let mut dv = 0.0f64;
    let sector_mesh = m.mesh();
    let mut full_start = 0usize;
    for (b, (section, v)) in [(ring_section(), v_ring()), (spoke_section(), v_spoke())]
        .into_iter()
        .enumerate()
    {
        let sb = &sector_mesh.blocks()[b];
        let ne = sb.len();
        for j in 0..N {
            let fb = rotor.block(j, b);
            ok_struct &= fb == N * b + j;
            let block = &blocks[fb];
            ok_struct &= block.kind() == ElementKind::Beam2 && block.len() == ne;
            let mapped: Vec<usize> = sb.connectivity().iter().map(|&q| rotor.node(j, q)).collect();
            ok_struct &= block.connectivity() == mapped.as_slice();
            for l in 0..ne {
                let e = full_start + j * ne + l;
                ok_struct &= rotor.mesh().block_of(e) == fb;
                ok_struct &= rotor.mesh().element_nodes(e) == &mapped[2 * l..2 * l + 2];
            }
            match block.property() {
                Some(BlockProperty::Beam {
                    section: s,
                    orientation,
                    ..
                }) => {
                    ok_struct &= *s == section;
                    if j == 0 {
                        copy0_bits &= (0..3).all(|c| orientation[c].to_bits() == v[c].to_bits());
                    } else {
                        let w = apply(&rot(j), v);
                        for c in 0..3 {
                            dv = dv.max((orientation[c] - w[c]).abs());
                        }
                    }
                }
                other => {
                    ok_struct = false;
                    println!("O1: block {fb} property {other:?}");
                }
            }
        }
        full_start += ne * N;
    }
    let o1 = ok_struct && copy0_bits && dv <= 1e-15;
    println!(
        "O1: blocks {} (2 N), structure {ok_struct}, copy 0 bits {copy0_bits}, max |v_j - R^j v| {dv:.4e} (limit <= 1e-15) {}",
        blocks.len(),
        met(o1)
    );
    if !o1 {
        fails.push(format!("O1 {ok_struct} {copy0_bits} {dv:e}"));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- L0 to L3

#[test]
fn l_expand_load_against_the_ring_load() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let fmodel = rotor_model(rotor.mesh());
    let full = ring();
    let (sd, fd, rd) = (m.dof_map(), fmodel.dof_map(), full.dof_map());
    let n = sd.n_equations();
    let mut fails = Vec::new();
    for k in 0..=N / 2 {
        let doubled = k != 0 && 2 * k != N;
        let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
        for q in LOADED_L {
            let (f, mo) = nodal_load(q as f64);
            let (g, go) = nodal_load(q as f64 + 7.0);
            for c in 0..3 {
                fa[sd.equation(q, c).unwrap()] = f[c];
                fa[sd.equation(q, 3 + c).unwrap()] = mo[c];
                if doubled {
                    fb[sd.equation(q, c).unwrap()] = g[c];
                    fb[sd.equation(q, 3 + c).unwrap()] = go[c];
                }
            }
        }
        let load = to_ring(&rotor, &fmodel, &full, &rotor.expand_load(&sd, &fd, k, &fa, &fb));
        // The ring's load by hand, in the ring's numbering: R^j (c f_a - s f_b) at the image of every sector node.
        let mut hand = vec![0.0; rd.n_equations()];
        for j in 0..N {
            let r = rot(j);
            let (c, s) = phase(k, j);
            for q in 0..NQ {
                for base in [0, 3] {
                    let v: Vec3 = [0, 1, 2].map(|t| {
                        let e = sd.equation(q, base + t).unwrap();
                        c * fa[e] - s * fb[e]
                    });
                    let w = apply(&r, v);
                    for t in 0..3 {
                        hand[rd.equation(ring_node(j, q), base + t).unwrap()] += w[t];
                    }
                }
            }
        }
        let d: Vec<f64> = load.iter().zip(&hand).map(|(a, b)| a - b).collect();
        let x = inf(&d) / inf(&hand);
        println!("L{k}: {x:.4e} (limit <= 1e-10) {}", met(x <= 1e-10));
        if x > 1e-10 {
            fails.push(format!("L{k}: {x:e}"));
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- F0 to F3

#[test]
fn f_expand_field_against_the_ring_field() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let fmodel = rotor_model(rotor.mesh());
    let (sd, fd) = (m.dof_map(), fmodel.dof_map());
    let n = sd.n_equations();
    let mut fails = Vec::new();
    // F0: the S0 solution of the sector (solve_static_mpc), expanded, against the ring's S0.
    {
        let mut ms = sector(full_options());
        ms.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
        for q in LOADED {
            let (f, mo) = nodal_load(q as f64);
            ms.add_load(Load::nodal_force(&format!("p{q}"), f)).unwrap();
            ms.add_load(Load::nodal_moment(&format!("p{q}"), mo)).unwrap();
        }
        let s = solve_static_mpc(&ms, &[], &[], &[], &StaticOptions::default()).unwrap();
        let us = s.result.displacement_values().to_vec();
        let zero = vec![0.0; us.len()];
        let full = ring_s0();
        let ur = solve_static(&full, &StaticOptions::default()).unwrap();
        let field = to_ring(&rotor, &fmodel, &full, &rotor.expand_field(&sd, &fd, 0, &us, &zero));
        let (eu, et) = field_error(&full, &field, ur.displacement_values());
        let e = eu.max(et);
        println!(
            "F0: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
            met(e <= 1e-10)
        );
        if e > 1e-10 {
            fails.push(format!("F0: {e:e}"));
        }
    }
    // F1 to F3: the harmonic static solutions (the S-k loads of ADR 0595).
    for k in 1..=N / 2 {
        let doubled = 2 * k < N;
        let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
        for q in LOADED {
            for (part, v) in [(0.0, &mut fa), (7.0, &mut fb)] {
                if part != 0.0 && !doubled {
                    continue;
                }
                let (f, mo) = nodal_load(q as f64 + part);
                for c in 0..3 {
                    v[sd.equation(q, c).unwrap()] = f[c];
                    v[sd.equation(q, 3 + c).unwrap()] = mo[c];
                }
            }
        }
        let load = HarmonicLoad::new(k, fa.clone(), fb.clone());
        let hs = solve_static_harmonic(&m, &[], &load, &StaticOptions::default()).unwrap();
        let mut full = ring();
        for q in LOADED {
            let get = |v: &[f64], base: usize| -> Vec3 { [0, 1, 2].map(|c| v[sd.equation(q, base + c).unwrap()]) };
            for j in 0..N {
                let (c, s) = phase(k, j);
                let r = rot(j);
                let part = |base: usize| -> Vec3 {
                    let (x, y) = (get(&fa, base), get(&fb, base));
                    apply(&r, [0, 1, 2].map(|t| c * x[t] - s * y[t]))
                };
                let set = format!("p{}", ring_node(j, q));
                full.add_load(Load::nodal_force(&set, part(0))).unwrap();
                full.add_load(Load::nodal_moment(&set, part(3))).unwrap();
            }
        }
        let ur = solve_static(&full, &StaticOptions::default()).unwrap();
        let field = to_ring(
            &rotor,
            &fmodel,
            &full,
            &rotor.expand_field(&sd, &fd, k, &hs.cos, &hs.sin),
        );
        let (eu, et) = field_error(&full, &field, ur.displacement_values());
        let e = eu.max(et);
        println!(
            "F{k}: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
            met(e <= 1e-10)
        );
        if e > 1e-10 {
            fails.push(format!("F{k}: {e:e}"));
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- Z0, NC-O, ZM

/// F under `expand_load` of the S0 nodal loads (on `r<p>`) and the centrifugal load, solved, in the ring's numbering.
fn z0_field(m: &SolidModel, rotor: &FullRotor, mesh: &SolidMesh, full: &SolidModel) -> Vec<f64> {
    let mut fmodel = rotor_model(mesh);
    let (sd, fd) = (m.dof_map(), fmodel.dof_map());
    let fa = s0_vector(m);
    let zero = vec![0.0; fa.len()];
    let load = rotor.expand_load(&sd, &fd, 0, &fa, &zero);
    for p in 0..mesh.nodes().len() {
        let f: Vec3 = [0, 1, 2].map(|t| load[fd.equation(p, t).unwrap()]);
        let mo: Vec3 = [0, 1, 2].map(|t| load[fd.equation(p, 3 + t).unwrap()]);
        fmodel.add_load(Load::nodal_force(&format!("r{p}"), f)).unwrap();
        fmodel.add_load(Load::nodal_moment(&format!("r{p}"), mo)).unwrap();
    }
    fmodel.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
    let u = solve_static(&fmodel, &StaticOptions::default()).unwrap();
    to_ring(rotor, &fmodel, full, u.displacement_values())
}

#[test]
fn z0_nc_o_full_rotor_model_static_against_the_ring() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let full = ring_s0();
    let ur = solve_static(&full, &StaticOptions::default()).unwrap();
    let mut fails = Vec::new();
    let x = z0_field(&m, &rotor, rotor.mesh(), &full);
    let (eu, et) = field_error(&full, &x, ur.displacement_values());
    let e = eu.max(et);
    println!(
        "Z0: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
        met(e <= 1e-10)
    );
    if e > 1e-10 {
        fails.push(format!("Z0: {e:e}"));
    }
    // NC-O: F with every Beam2 copy reset to the sector's v.
    let mut mesh = rotor.mesh().clone();
    for (b, (section, v)) in [(ring_section(), v_ring()), (spoke_section(), v_spoke())]
        .into_iter()
        .enumerate()
    {
        for j in 0..N {
            mesh.set_block_property(rotor.block(j, b), BlockProperty::beam(section, v).unwrap())
                .unwrap();
        }
    }
    let x = z0_field(&m, &rotor, &mesh, &full);
    let (eu, et) = field_error(&full, &x, ur.displacement_values());
    let e = eu.max(et);
    println!(
        "NC-O: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit >= 1e-3) {}",
        met(e >= 1e-3)
    );
    if e < 1e-3 {
        fails.push(format!("NC-O: {e:e}"));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

#[test]
fn zm_full_rotor_model_modes_against_the_ring() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let fmodel = rotor_model(rotor.mesh());
    let rf = modal_analysis(&fmodel, &modal_options(30)).unwrap();
    let rr = modal_analysis(&ring(), &modal_options(30)).unwrap();
    let d = rf.eigenvalues[..18]
        .iter()
        .zip(&rr.eigenvalues[..18])
        .map(|(a, b)| (a / b - 1.0).abs())
        .fold(0.0f64, f64::max);
    println!(
        "ZM: 18 lowest, max |lambda_F / lambda_r - 1| {d:.4e} (limit <= 1e-10) {}; iterations F {} ring {}",
        met(d <= 1e-10),
        rf.iterations,
        rr.iterations
    );
    assert!(d <= 1e-10, "ZM: {d:e}");
}

// ---------------------------------------------------------------------------------------------- P, RQ, G, NC-H

#[test]
fn p_rq_g_nc_h_expanded_modes_in_the_ring() {
    let m = sector(full_options());
    let rotor = full_rotor(&m).unwrap();
    let fmodel = rotor_model(rotor.mesh());
    let full = ring();
    let (sd, fd, rd) = (m.dof_map(), fmodel.dof_map(), full.dof_map());
    let harmonics: Vec<_> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(&m, &[], k, &modal_options(4)).unwrap())
        .collect();
    let rr = modal_analysis(&full, &modal_options(30)).unwrap();
    let lc = (1.0 - 1e-6) * harmonics.iter().map(|h| h.values[3]).fold(f64::INFINITY, f64::min);
    let kf = stiffness(&full).unwrap();
    let mf = consistent(&full).unwrap();
    let (nk, nm) = (inf_norm(&kf), inf_norm(&mf));
    let fixed: Vec<bool> = (0..rd.n_equations())
        .map(|e| rd.node_component(e).unwrap().0 >= NR * N + N)
        .collect();
    let mut fails = Vec::new();
    let mut all = Vec::new();
    for h in &harmonics {
        let (mut worst_p, mut worst_rq, mut count) = (0.0f64, 0.0f64, 0usize);
        for (lam, mode) in h.eigenvalues.iter().zip(&h.modes) {
            if *lam >= lc {
                continue;
            }
            count += 1;
            let phi = to_ring(&rotor, &fmodel, &full, &rotor.expand(&sd, &fd, h.harmonic, mode));
            let (kp, mp) = (matvec(&kf, &phi), matvec(&mf, &phi));
            let r = (0..phi.len())
                .filter(|&e| !fixed[e])
                .map(|e| (kp[e] - lam * mp[e]).abs())
                .fold(0.0f64, f64::max);
            worst_p = worst_p.max(r / ((nk + lam.abs() * nm) * inf(&phi)));
            let rq = dot(&phi, &kp) / dot(&phi, &mp);
            let near = rr
                .eigenvalues
                .iter()
                .copied()
                .min_by(|a, b| (a - rq).abs().total_cmp(&(b - rq).abs()))
                .unwrap();
            worst_rq = worst_rq.max((rq / near - 1.0).abs());
            all.push((phi, mp));
        }
        println!(
            "P{}: backward error {worst_p:.4e} over {count} modes (limit <= 1e-10) {}",
            h.harmonic,
            met(worst_p <= 1e-10)
        );
        println!(
            "RQ{}: max |RQ / lambda_ring - 1| {worst_rq:.4e} (limit <= 1e-10) {}",
            h.harmonic,
            met(worst_rq <= 1e-10)
        );
        if worst_p > 1e-10 || worst_rq > 1e-10 || count == 0 {
            fails.push(format!("P/RQ{}: {worst_p:e} {worst_rq:e} ({count})", h.harmonic));
        }
    }
    // G: the M-Gram matrix of every expanded mode below the cut-off.
    let mut g = 0.0f64;
    for (i, (pi, _)) in all.iter().enumerate() {
        for (l, (_, ml)) in all.iter().enumerate() {
            let target = if i == l { 1.0 } else { 0.0 };
            g = g.max((dot(pi, ml) - target).abs());
        }
    }
    println!(
        "G: {} modes, max |G - I| {g:.4e} (limit <= 1e-10) {}",
        all.len(),
        met(g <= 1e-10)
    );
    if g > 1e-10 {
        fails.push(format!("G: {g:e}"));
    }
    // NC-H: the first k = 1 mode expanded with the harmonic argument 2.
    let h1 = &harmonics[1];
    let phi = to_ring(&rotor, &fmodel, &full, &rotor.expand(&sd, &fd, 2, &h1.modes[0]));
    let rq = dot(&phi, &matvec(&kf, &phi)) / dot(&phi, &matvec(&mf, &phi));
    let x = (rq / h1.eigenvalues[0] - 1.0).abs();
    println!("NC-H: |RQ / lambda - 1| {x:.4e} (limit >= 1e-3) {}", met(x >= 1e-3));
    if x < 1e-3 {
        fails.push(format!("NC-H: {x:e}"));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- B3: the shell twin

const NC: usize = 2;
const NA: usize = 3;

/// The ADR 0556 D3 frustum point at column `i` and generator parameter `s`.
fn frustum_point(i: usize, s: f64) -> Vec3 {
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    p(phi, 1.0 - 0.4 * s, 0.8 * s)
}

fn shell_sector(options: CyclicOptions) -> SolidModel {
    let id = |i: usize, a: usize| a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..=NC {
            nodes.push(frustum_point(i, a as f64 / NA as f64));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            conn.extend([id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    mesh.add_element_set("all", (0..NA * NC).collect()).unwrap();
    mesh.add_node_set("hub", (0..=NC).map(|i| id(i, 0)).collect()).unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

fn shell_rotor_values(m: &SolidModel) -> (FullRotor, Vec<f64>) {
    let rotor = full_rotor(m).unwrap();
    let fmodel = rotor_model(rotor.mesh());
    let (sd, fd) = (m.dof_map(), fmodel.dof_map());
    let n = sd.n_equations();
    let fa: Vec<f64> = (0..n).map(|e| (0.37 * e as f64 + 0.1).sin()).collect();
    let fb: Vec<f64> = (0..n).map(|e| (0.53 * e as f64 + 0.4).cos()).collect();
    let mut out: Vec<f64> = rotor.mesh().nodes().iter().flatten().copied().collect();
    out.extend(rotor.expand_load(&sd, &fd, 1, &fa, &fb));
    out.extend(rotor.expand_field(&sd, &fd, 1, &fa, &fb));
    (rotor, out)
}

#[test]
fn b3_shell_full_rotor_bits_with_and_without_the_option() {
    let (ra, a) = shell_rotor_values(&shell_sector(CyclicOptions::new().shells(true)));
    let (rb, b) = shell_rotor_values(&shell_sector(full_options()));
    let same = ra == rb && a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("B3: rotor equal and {} values bit-equal: {same} {}", a.len(), met(same));
    assert!(same, "B3");
}

// ---------------------------------------------------------------------------------------------- NC1, NC2

#[test]
#[ignore = "ADR 0799: retired record, NC1 asserts the full_rotor refusal without full_rotor_beams that is lifted by default; restated in sa_beam_opt_ins_default_f672.rs"]
fn nc1_nc2_refusals() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // NC1: beams without full_rotor_beams: full_rotor refuses Beam2 as before.
    let r = full_rotor(&sector(beam_options())).map(|_| ());
    hold(
        "NC1 full_rotor without full_rotor_beams",
        matches!(&r, Err(FemError::UnsupportedKind { kind: "Beam2", .. })),
        format!("{r:?}"),
    );
    // NC2: full_rotor_beams without beams, refused by add_constraint.
    let mut m = SolidModel::new(wheel_mesh());
    m.add_material("all", mat()).unwrap();
    let r = m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true).full_rotor_beams(true),
    ));
    hold(
        "NC2 add_constraint",
        matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("ADR 0601")),
        format!("{r:?}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
