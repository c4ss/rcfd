//! F-695 (ADR 0627): the hole criterion of the error density `eta_e/|K|^{1/2}` (F-568, ADR 0616) on a two-layer Hex20 plate
//! with a hole whose first ring has one radial size at every angle, and the recovered peak stress against Kirsch. Checks U1,
//! U2, P3, NC1, NC2, P2, P2r and FW24 of the registration; FW and P2b are a heavy record (ignored, run once).
//!
//! The plate, `plate_map`, `on_hole`, `hole_edge` and `Checks` are copied verbatim from `sa_density.rs` (F-568), which takes
//! them from `sa_estimate.rs` (F-432, ADR 0478) and `sa_fatigue.rs` (ADR 0449). Only the spacing along the node lines differs
//! (`uniform_map`, ADR 0627 D1).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::estimate::{energy_error, EnergyError, PatchOptions, Recovery};
use rcfd_fem::hex20::{faces, upgrade_to_hex20_mapped};
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The plate with a hole (ADR 0449) and its two maps

const HR: f64 = 0.005;
const HA: f64 = 0.1;
const HB: f64 = 0.05;
const HT: f64 = 0.001;
const S0: f64 = 50e6;

/// Howland's gross factor at d/W = 0.1 by Peterson's polynomial (from memory, ADR 0627 arithmetic 6).
const KTG_HOWLAND: f64 = 3.023492;

/// The map of ADR 0449 / 0478 / 0616, verbatim.
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

/// The end points `C(q)` and `O(q)` of the node line `q`, as in `plate_map`.
fn line_ends(q: f64) -> ([f64; 2], [f64; 2]) {
    if q == 1.0 {
        ([0.0, HR], [0.0, HB])
    } else {
        let th = 0.5 * PI * q;
        let ell = q * (HA + HB);
        let o = if ell <= HB { [HA, ell] } else { [HA - (ell - HB), HB] };
        ([HR * th.cos(), HR * th.sin()], o)
    }
}

/// The first-ring radial size of `plate_map` at 90 degrees, `(B - r) g(1/24)`.
fn h1() -> f64 {
    (HB - HR) * ((4.0f64 / 24.0).exp() - 1.0) / (4f64.exp() - 1.0)
}

/// The exponent `a(q)` of the spacing `d(s) = L (e^{a s} - 1)/(e^a - 1)` on line `q` such that the node at `s = 1/24` lies at
/// radius `r + h_1` (ADR 0627 arithmetic 1): bisection, 200 halvings in `[1e-6, 20]`.
fn exponent(q: f64) -> (f64, [f64; 2], [f64; 2], f64) {
    let (c, o) = line_ends(q);
    let l = (o[0] - c[0]).hypot(o[1] - c[1]);
    let u = [(o[0] - c[0]) / l, (o[1] - c[1]) / l];
    let cc = (c[0] * u[0] + c[1] * u[1]) / HR;
    let h = h1();
    let d1 = -HR * cc + (HR * HR * cc * cc + 2.0 * HR * h + h * h).sqrt();
    let f = |a: f64| l * ((a / 24.0).exp() - 1.0) / (a.exp() - 1.0) - d1;
    let (mut lo, mut hi) = (1e-6, 20.0);
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if f(m) > 0.0 {
            lo = m;
        } else {
            hi = m;
        }
    }
    (0.5 * (lo + hi), c, o, l)
}

/// ADR 0627 D1: the lines of `plate_map`, spaced so that the first ring has the radial size `h_1` on every line.
fn uniform_map(p: Vec3) -> Vec3 {
    let (s, q) = (p[0], p[1]);
    let (a, c, o, l) = exponent(q);
    if s == 0.0 {
        return [c[0], c[1], p[2]];
    }
    if s == 1.0 {
        return [o[0], o[1], p[2]];
    }
    let d = l * ((a * s).exp() - 1.0) / (a.exp() - 1.0);
    [c[0] + d * (o[0] - c[0]) / l, c[1] + d * (o[1] - c[1]) / l, p[2]]
}

fn plate_with(map: fn(Vec3) -> Vec3, cells: usize, layers: usize) -> SolidModel {
    let (mut mesh, _) =
        upgrade_to_hex20_mapped(&hex_box([0.0; 3], [1.0, 1.0, HT], [cells, cells, layers]).unwrap(), map).unwrap();
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

fn solved(map: fn(Vec3) -> Vec3, cells: usize, layers: usize, solver: StaticSolver) -> (SolidModel, Vec<f64>) {
    let t0 = Instant::now();
    let model = plate_with(map, cells, layers);
    let r = solve_static(&model, &StaticOptions::with_solver(solver)).unwrap();
    let u = r.displacement_values().to_vec();
    println!(
        "  plate {cells} x {cells} x {layers}: {} elements, {} equations, solved in {:.2} s",
        model.mesh().n_elements(),
        u.len(),
        t0.elapsed().as_secs_f64()
    );
    (model, u)
}

fn on_hole(x: Vec3) -> bool {
    (x[0].hypot(x[1]) - HR).abs() <= 1e-9
}

/// The hole-edge elements of the plate (every element with a face on the hole).
fn hole_edge(mesh: &SolidMesh) -> Vec<usize> {
    (0..mesh.n_elements())
        .filter(|&e| {
            let nodes = mesh.element_nodes(e);
            mesh.element_kind(e)
                .faces()
                .iter()
                .any(|f| f.iter().all(|&a| on_hole(mesh.nodes()[nodes[a]])))
        })
        .collect()
}

/// The radial sizes `|x| - r` of the off-hole corner nodes of the first ring (elements `e` with `e % 24 == 0`), each node once.
fn first_ring_sizes(mesh: &SolidMesh) -> Vec<f64> {
    let mut nodes = BTreeSet::new();
    for e in (0..mesh.n_elements()).filter(|e| e % 24 == 0) {
        for &a in &mesh.element_nodes(e)[..8] {
            if !on_hole(mesh.nodes()[a]) {
                nodes.insert(a);
            }
        }
    }
    nodes
        .iter()
        .map(|&a| mesh.nodes()[a][0].hypot(mesh.nodes()[a][1]) - HR)
        .collect()
}

/// Hole-edge elements outside the tenth of `ranked`, and their sorted ranks.
fn outside(ranked: &[usize], edge: &[usize]) -> (usize, Vec<usize>) {
    let tenth = ranked.len().div_ceil(10);
    let top: BTreeSet<usize> = ranked[..tenth].iter().copied().collect();
    let mut ranks: Vec<usize> = edge
        .iter()
        .map(|&e| ranked.iter().position(|&r| r == e).unwrap())
        .collect();
    ranks.sort_unstable();
    (edge.iter().filter(|e| !top.contains(e)).count(), ranks)
}

fn angle(mesh: &SolidMesh, e: usize) -> f64 {
    let nodes = mesh.element_nodes(e);
    let (mut x, mut y) = (0.0, 0.0);
    for &a in nodes {
        x += mesh.nodes()[a][0];
        y += mesh.nodes()[a][1];
    }
    y.atan2(x).to_degrees()
}

/// The largest recovered nodal `sigma_xx` and its node.
fn recovered_peak(mesh: &SolidMesh, rec: &Recovery) -> (usize, f64) {
    let mut best = (0, f64::NEG_INFINITY);
    for n in 0..mesh.nodes().len() {
        let s = rec.nodal()[n].unwrap()[0];
        if s > best.1 {
            best = (n, s);
        }
    }
    best
}

/// The nodal average (F-376) `sigma_xx` at the node `(0, r, 0)`, over `sigma_0`.
fn nodal_average_at_peak(model: &SolidModel, u: &[f64]) -> f64 {
    let mesh = model.mesh();
    let field = IntegrationPointField::compute(model, u).unwrap();
    let avg = nodal_average(model, &field, field.stress(), None).unwrap();
    let node = (0..mesh.nodes().len())
        .find(|&n| {
            let x = mesh.nodes()[n];
            x[0].abs() <= 1e-12 && (x[1] - HR).abs() <= 1e-12 && x[2].abs() <= 1e-12
        })
        .unwrap();
    avg[node].unwrap()[0] / S0
}

fn estimate_of(model: &SolidModel, u: &[f64]) -> (Recovery, EnergyError) {
    energy_error(model, u, &PatchOptions::new()).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------
// U1, U2, P3, NC2, P2, P2r

#[test]
fn u1_u2_p3_nc2_p2_uniform_plate() {
    let mut c = Checks::new();
    let (model, u) = solved(uniform_map, 24, 2, StaticSolver::SparseDirect);
    let mesh = model.mesh();
    // U1
    let h = h1();
    let sizes = first_ring_sizes(mesh);
    let dev = sizes.iter().map(|s| (s - h).abs() / h).fold(0.0, f64::max);
    println!(
        "U1 record: h_1 = {h:.6e} m = {:.5} r, {} first-ring corner nodes",
        h / HR,
        sizes.len()
    );
    c.holds(
        "U1 first-ring corner nodes counted",
        sizes.len() == 75,
        &format!("{}", sizes.len()),
    );
    c.at_most("U1 largest ||x| - r - h_1| / h_1", dev, 1e-9);
    // U2
    let (rec, est) = estimate_of(&model, &u);
    let exact = HT * (HA * HB - PI * HR * HR / 4.0);
    let sum: f64 = est.element_volumes().iter().sum();
    c.at_most(
        "U2 sum |K| against t (A B - pi r^2/4), relative",
        (sum / exact - 1.0).abs(),
        1e-6,
    );
    let edge = hole_edge(mesh);
    c.holds(
        "U2 1152 elements, 48 hole-edge elements",
        mesh.n_elements() == 1152 && edge.len() == 48,
        &format!("{} elements, {} hole-edge", mesh.n_elements(), edge.len()),
    );
    // P3
    let by_density = est.ranked_by_density();
    let den = est.element_densities();
    let tenth = mesh.n_elements().div_ceil(10);
    let (missing, ranks) = outside(&by_density, &edge);
    let edge_min = edge.iter().map(|&e| den[e]).fold(f64::INFINITY, f64::min);
    println!("P3 record: density ranks of the hole-edge elements {ranks:?}");
    println!(
        "P3 record: largest density {:.4e} (element {}, angle {:.2}), density of rank {tenth} {:.4e}, smallest at the hole \
         edge {edge_min:.4e}",
        den[by_density[0]],
        by_density[0],
        angle(mesh, by_density[0]),
        den[by_density[tenth - 1]]
    );
    for &e in edge.iter().filter(|&&e| e < 576) {
        let rank = by_density.iter().position(|&r| r == e).unwrap();
        println!(
            "P3 record: hole edge e {e} cell {} angle {:.2} density {:.4e} rank {rank}",
            e / 24,
            angle(mesh, e),
            den[e]
        );
    }
    let mut per_ring = [0usize; 24];
    for &e in &by_density[..tenth] {
        per_ring[e % 24] += 1;
    }
    println!("P3 record: elements of each ring among the top {tenth}: {per_ring:?}");
    c.holds(
        "P3 the tenth of largest density contains every element with a face on the hole",
        edge.len() == 48 && missing == 0,
        &format!("{missing} of {} hole-edge elements outside the tenth", edge.len()),
    );
    // NC2
    let (missing_abs, ranks_abs) = outside(&est.ranked(), &edge);
    println!("NC2 record: absolute ranks of the hole-edge elements {ranks_abs:?}");
    c.holds(
        "NC2 ranked by eta_e alone the criterion fails (>= 1 hole-edge element outside the tenth)",
        missing_abs >= 1,
        &format!("{missing_abs} of {} outside", edge.len()),
    );
    // P2, P2r
    let (node, peak) = recovered_peak(mesh, &rec);
    println!(
        "P2 record: recovered peak sigma_xx {peak:.6e} at node {node} {:?}, {:+.4e} of 3 sigma_0",
        mesh.nodes()[node],
        peak / (3.0 * S0) - 1.0
    );
    c.at_most(
        "P2 recovered peak sigma_xx against 3 sigma_0",
        (peak / (3.0 * S0) - 1.0).abs(),
        2e-2,
    );
    println!(
        "P2r record: recovered peak {:+.4e} of K_tg sigma_0 (K_tg = {KTG_HOWLAND}); nodal average (F-376) at (0, r, 0) {:+.4e} \
         of 3 sigma_0",
        peak / (KTG_HOWLAND * S0) - 1.0,
        nodal_average_at_peak(&model, &u) / 3.0 - 1.0
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// NC1: the ADR 0616 mesh in this file

#[test]
fn nc1_adr0616_mesh_misses() {
    let mut c = Checks::new();
    let (model, u) = solved(plate_map, 24, 2, StaticSolver::SparseDirect);
    let mesh = model.mesh();
    let (_, est) = estimate_of(&model, &u);
    let edge = hole_edge(mesh);
    let by_density = est.ranked_by_density();
    let (missing, ranks) = outside(&by_density, &edge);
    let tenth = mesh.n_elements().div_ceil(10);
    let d116 = est.element_densities()[by_density[tenth - 1]];
    let sizes = first_ring_sizes(mesh);
    let (lo, hi) = sizes
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    println!(
        "NC1 record: first-ring radial size {:.5} r to {:.5} r, ratio {:.4}; density ranks {ranks:?}",
        lo / HR,
        hi / HR,
        hi / lo
    );
    c.holds(
        "NC1 the ADR 0616 mesh: 4 hole-edge elements outside, ranks 126, 127, 140, 141",
        missing == 4 && ranks[44..] == [126, 127, 140, 141],
        &format!(
            "{missing} outside, last ranks {:?}",
            &ranks[ranks.len().saturating_sub(4)..]
        ),
    );
    c.at_most(
        "NC1 density of rank 116 against 2.5082e-1 (F-568 log)",
        (d116 - 2.5082e-1).abs(),
        5e-5,
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// FW24 (fast path), FW and P2b (heavy)

#[test]
fn fw24_nodal_average_single_layer() {
    let (model, u) = solved(uniform_map, 24, 1, StaticSolver::SupernodalDirect);
    let k = nodal_average_at_peak(&model, &u);
    println!(
        "FW24 record: nodal average sigma_xx at (0, r, 0) over sigma_0 {k:.6} ({:+.4e} of 3, {:+.4e} of K_tg)",
        k / 3.0 - 1.0,
        k / KTG_HOWLAND - 1.0
    );
    assert!(
        k.is_finite() && k > 2.9 && k < 3.2,
        "FW24 out of any plausible range: {k}"
    );
}

#[test]
#[ignore = "heavy record of ADR 0627 FW and P2b (single-layer meshes of 24, 48 and 96 cells, 196 425 equations at 96, SupernodalDirect), run once"]
fn fw_p2b_finite_width_heavy() {
    let mut c = Checks::new();
    let mut k = Vec::new();
    for n in [24, 48, 96] {
        let (model, u) = solved(uniform_map, n, 1, StaticSolver::SupernodalDirect);
        let kn = nodal_average_at_peak(&model, &u);
        println!("FW record: n = {n}: K = {kn:.6}");
        k.push(kn);
    }
    let p = ((k[0] - k[1]) / (k[1] - k[2])).log2();
    let rich = k[2] + (k[2] - k[1]) / (2f64.powf(p) - 1.0);
    println!(
        "FW record: observed order {p:.3}, Richardson value {rich:.6} ({:+.4e} of K_tg, {:+.4e} of Heywood 3.032222)",
        rich / KTG_HOWLAND - 1.0,
        rich / 3.032222 - 1.0
    );
    c.at_most("FW |K_96 / K_tg - 1|", (k[2] / KTG_HOWLAND - 1.0).abs(), 5e-3);
    let (model, u) = solved(uniform_map, 24, 2, StaticSolver::SparseDirect);
    let rec = rcfd_fem::estimate::recover(&model, &u, &PatchOptions::new()).unwrap();
    let (_, peak) = recovered_peak(model.mesh(), &rec);
    c.at_most(
        "P2b recovered peak against K_96 sigma_0",
        (peak / (k[2] * S0) - 1.0).abs(),
        1e-2,
    );
    println!("P2b signed: {:+.4e}", peak / (k[2] * S0) - 1.0);
    c.finish();
}
