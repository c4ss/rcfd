//! F-464 (ADR 0362): the moved boxes of ADR 0358 measured on the node coordinates they store. The checker reads
//! `SolidMesh::nodes()` of the mesh handed to the solver against a freshly built regular mesh; it uses no value of the
//! generator. Checks S0 to S5 and the controls C1, C2 of the registration. `sb_patch_moved.rs` is not edited; the generator
//! `moved` and the seed are copied from it.

use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::mesh::SolidMesh;
use std::collections::BTreeSet;

const SEED: u64 = 0xF461_0358;

// D2: the registered configurations.
const NS: [usize; 3] = [3, 4, 6];
const ALPHAS: [f64; 2] = [0.15, 0.05];
const DRAWS: [u64; 3] = [0, 1, 2];

// Limits (checks table of the registration).
const MOVE_FRACTION: f64 = 0.3;
const RMS_FLOOR: f64 = 1e-3;
const NODE_FLOOR: f64 = 1e-5;
const OFFSET_SLACK: f64 = 1e-9;

/// SplitMix64, a deterministic source only (as in sb_patch_moved.rs).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// The builder mesh with its interior nodes (in no face node set) moved by `alpha h (2 r - 1)` per component, the uniforms
/// `r` drawn from `Rng::new(tag)` in ascending node order; blocks and sets copied.
fn moved(mesh: &SolidMesh, alpha: f64, n: usize, tag: u64) -> SolidMesh {
    let h = 1.0 / n as f64;
    let mut boundary = BTreeSet::new();
    for name in BOX_FACES {
        boundary.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut rng = Rng::new(tag);
    let mut nodes = mesh.nodes().to_vec();
    for (v, x) in nodes.iter_mut().enumerate() {
        if boundary.contains(&v) {
            continue;
        }
        for xc in x.iter_mut() {
            let d = alpha * h * (2.0 * rng.uniform() - 1.0);
            *xc += d;
        }
    }
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

/// The mesh handed to the solver for a configuration: the builder mesh moved with the built amplitude.
fn cube(n: usize, built_alpha: f64, draw: u64) -> SolidMesh {
    let regular = hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    moved(&regular, built_alpha, n, 1000 * draw + n as u64)
}

/// The 18 registered meshes as (cells per edge, registered amplitude, draw).
fn configs() -> Vec<(usize, f64, u64)> {
    let mut out = Vec::new();
    for &draw in &DRAWS {
        for &alpha in &ALPHAS {
            for &n in &NS {
                out.push((n, alpha, draw));
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Kind {
    AtLeast,
    AtMost,
    Equal,
}

/// One measured value with its limit.
struct Measure {
    name: &'static str,
    value: f64,
    limit: f64,
    kind: Kind,
}

impl Measure {
    fn met(&self) -> bool {
        match self.kind {
            Kind::AtLeast => self.value >= self.limit,
            Kind::AtMost => self.value <= self.limit,
            Kind::Equal => self.value == self.limit,
        }
    }
    fn print(&self, tag: &str) {
        let rel = match self.kind {
            Kind::AtLeast => ">=",
            Kind::AtMost => "<=",
            Kind::Equal => "==",
        };
        println!(
            "{} {tag}: {:.4e} (limit {rel} {:e}) {}",
            self.name,
            self.value,
            self.limit,
            if self.met() { "met" } else { "MISSED" }
        );
    }
}

fn measure(name: &'static str, kind: Kind, value: f64, limit: f64) -> Measure {
    Measure {
        name,
        value,
        limit,
        kind,
    }
}

/// S0 to S5 on the stored mesh, against the regular builder mesh of `n` cells per edge and the registered amplitude
/// `expected_alpha`. Only `stored.nodes()` and `stored.blocks()` are read.
fn stored_checks(stored: &SolidMesh, n: usize, expected_alpha: f64) -> Vec<Measure> {
    let regular = hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    let h = 1.0 / n as f64;
    let mut boundary = BTreeSet::new();
    for name in BOX_FACES {
        boundary.extend(regular.node_set(name).unwrap().iter().copied());
    }
    let (sn, rn) = (stored.nodes(), regular.nodes());
    let mut out = Vec::new();

    // S0: the node count and the interior count.
    out.push(measure("S0a", Kind::Equal, sn.len() as f64, rn.len() as f64));
    let interior: Vec<usize> = (0..rn.len().min(sn.len())).filter(|v| !boundary.contains(v)).collect();
    let want_interior = ((n - 1) * (n - 1) * (n - 1)) as f64;
    out.push(measure("S0b", Kind::Equal, interior.len() as f64, want_interior));

    // S1: boundary nodes unchanged bit for bit.
    let changed = boundary
        .iter()
        .filter(|&&v| v < sn.len() && (0..3).any(|k| sn[v][k].to_bits() != rn[v][k].to_bits()))
        .count();
    out.push(measure("S1", Kind::Equal, changed as f64, 0.0));

    // S2, S2b, S3, S4: offsets of the interior nodes read from the stored coordinates.
    let (mut sq, mut count) = (0.0f64, 0usize);
    let mut max_offset = 0.0f64;
    let mut min_node_max = f64::INFINITY;
    for &v in &interior {
        let mut node_max = 0.0f64;
        for k in 0..3 {
            let d = (sn[v][k] - rn[v][k]) / h;
            sq += d * d;
            count += 1;
            node_max = node_max.max(d.abs());
        }
        max_offset = max_offset.max(node_max);
        min_node_max = min_node_max.min(node_max);
    }
    let rms = (sq / count.max(1) as f64).sqrt();
    out.push(measure("S2", Kind::AtLeast, rms, MOVE_FRACTION * expected_alpha));
    out.push(measure("S2b", Kind::AtLeast, rms, RMS_FLOOR));
    out.push(measure(
        "S3",
        Kind::AtMost,
        max_offset,
        expected_alpha * (1.0 + OFFSET_SLACK),
    ));
    out.push(measure("S4", Kind::AtLeast, min_node_max, NODE_FLOOR));

    // S5: the connectivity of every block equals the regular mesh's.
    let mut differing = 0usize;
    if stored.blocks().len() != regular.blocks().len() {
        differing += 1;
    }
    for (a, b) in stored.blocks().iter().zip(regular.blocks()) {
        if a.kind() != b.kind() || a.connectivity() != b.connectivity() {
            differing += 1;
        }
    }
    out.push(measure("S5", Kind::Equal, differing as f64, 0.0));
    out
}

fn tag_of(n: usize, alpha: f64, draw: u64) -> String {
    format!("n = {n} alpha = {alpha} draw {draw}")
}

#[test]
fn s_stored_mesh_of_the_moved_boxes() {
    let mut missed = Vec::new();
    for (n, alpha, draw) in configs() {
        let mesh = cube(n, alpha, draw);
        let tag = tag_of(n, alpha, draw);
        for m in stored_checks(&mesh, n, alpha) {
            m.print(&tag);
            if !m.met() {
                missed.push(format!("{} {tag}: {:e} against {:e}", m.name, m.value, m.limit));
            }
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}

#[test]
fn c1_unmoved_mesh_fails_the_stored_checks() {
    let mut wrong = Vec::new();
    for (n, alpha, draw) in configs() {
        // The same code, built with amplitude 0, checked against the registered amplitude.
        let mesh = cube(n, 0.0, draw);
        let tag = tag_of(n, alpha, draw);
        for m in stored_checks(&mesh, n, alpha) {
            m.print(&format!("(unmoved, registered {tag})"));
            let must_miss = matches!(m.name, "S2" | "S2b" | "S4");
            if m.met() == must_miss {
                wrong.push(format!("{} {tag}: met = {}", m.name, m.met()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "control behaved otherwise than registered: {wrong:#?}"
    );
}

#[test]
fn c2_unmoved_mesh_with_zero_expected_amplitude() {
    let mut wrong = Vec::new();
    for (n, alpha, draw) in configs() {
        let mesh = cube(n, 0.0, draw);
        let tag = tag_of(n, alpha, draw);
        for m in stored_checks(&mesh, n, 0.0) {
            m.print(&format!("(unmoved, expected 0, {tag})"));
            // S2 is met trivially at expected amplitude 0 (0 >= 0, recorded); S2b and S4 must miss.
            let must_miss = matches!(m.name, "S2b" | "S4");
            let may_either = m.name == "S2";
            if !may_either && m.met() == must_miss {
                wrong.push(format!("{} {tag}: met = {}", m.name, m.met()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "control behaved otherwise than registered: {wrong:#?}"
    );
}
