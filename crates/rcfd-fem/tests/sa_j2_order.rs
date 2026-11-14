//! F-560 (ADR 0515): a reading of the Newton convergence order that fits an active-set change, on the J2 sphere and beam
//! solves of ADR 0475. The reading Q(C, F): every pair of consecutive residuals `(r_k, r_{k+1})`, `k >= 1`, of an increment
//! whose correction did not change the set of yielded integration points satisfies `r_{k+1} <= max(C r_k^2, F)`. Checks B1,
//! D1, Q1 to Q3, N2 to N4 run here; N1 (the continuum tangent) is an ignored test run in the MU1 scratch copy; B0 is
//! `sa_j2_order_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::nonlinear::{solve, LoadStep, NewtonOptions, NonlinearResult};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const SY: f64 = 2.0e8;
const RHO: f64 = 7850.0;
/// The registered constants of the reading (ADR 0515 D1).
const C_REG: f64 = 10.0;
const F_REG: f64 = 1e-13;
const INFORMATIVE: f64 = 1e-6;

fn j2(h_iso: f64, h_kin: f64) -> Material {
    Material::j2(
        Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap(),
        SY,
        h_iso,
        h_kin,
    )
    .unwrap()
}

const SPHERE_N: usize = 2;
const SPHERE_M: usize = 8;
const ELEMENTS_PER_LAYER: usize = 3 * SPHERE_N * SPHERE_N;

fn p_of_c(c: f64) -> f64 {
    2.0 * c.ln() + 2.0 / 3.0 * (1.0 - c * c * c / 8.0)
}

/// The octant of the sphere `a = 1`, `b = 2` of D5 (cubed sphere, 2 x 2 cells per face, 8 radial layers, Hex20 on the
/// spheres), element `layer * 12 + cell`; with the bore pressure `p` (Pa) if given.
fn sphere(mat: Material, p: Option<f64>) -> SolidModel {
    let (n, m) = (SPHERE_N, SPHERE_M);
    let mut index: HashMap<([usize; 3], usize), usize> = HashMap::new();
    let mut params: Vec<Vec3> = Vec::new();
    let mut node = |q: [usize; 3], k: usize, params: &mut Vec<Vec3>| -> usize {
        *index.entry((q, k)).or_insert_with(|| {
            let r = 1.0 + k as f64 / m as f64;
            params.push([
                r * q[0] as f64 / n as f64,
                r * q[1] as f64 / n as f64,
                r * q[2] as f64 / n as f64,
            ]);
            params.len() - 1
        })
    };
    let mut conn = Vec::new();
    for k in 0..m {
        for a in 0..3 {
            for j in 0..n {
                for i in 0..n {
                    let q = |i: usize, j: usize| {
                        let mut v = [0; 3];
                        v[a] = n;
                        v[(a + 1) % 3] = i;
                        v[(a + 2) % 3] = j;
                        v
                    };
                    for kk in [k, k + 1] {
                        for (ii, jj) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)] {
                            let id = node(q(ii, jj), kk, &mut params);
                            conn.push(id);
                        }
                    }
                }
            }
        }
    }
    let nn = params.len();
    let mut mesh = SolidMesh::new(params).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        let set: Vec<usize> = (0..nn).filter(|&v| mesh.nodes()[v][c] == 0.0).collect();
        mesh.add_node_set(name, set).unwrap();
    }
    mesh.add_surface_set(
        "bore",
        (0..ELEMENTS_PER_LAYER)
            .map(|e| FaceRef { element: e, face: 0 })
            .collect(),
    )
    .unwrap();
    let mesh = upgrade_to_hex20_mapped(&mesh, |p| {
        let inf = p[0].abs().max(p[1].abs()).max(p[2].abs());
        let two = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] * inf / two, p[1] * inf / two, p[2] * inf / two]
    })
    .unwrap()
    .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        model.add_constraint(Constraint::displacement(name, c, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::pressure("bore", p)).unwrap();
    }
    model
}

const BEAM_L: f64 = 0.02;
const BEAM_H: f64 = 0.1;
const BEAM_W: f64 = 0.01;

fn kappa_y() -> f64 {
    2.0 * SY / (E_MOD * BEAM_H)
}

/// The narrow beam of D6 and the prescribed displacements of the curvature `kappa`.
fn beam(kappa: f64) -> (SolidModel, Vec<Prescribed>) {
    let mesh = upgrade_to_hex20(
        &hex_box(
            [0.0, -BEAM_H / 2.0, -BEAM_W / 2.0],
            [BEAM_L, BEAM_H, BEAM_W],
            [1, 16, 1],
        )
        .unwrap(),
    )
    .unwrap()
    .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", j2(0.0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let nodes = m.mesh().nodes().to_vec();
    let at = |p: Vec3| {
        nodes
            .iter()
            .position(|q| (q[0] - p[0]).abs() < 1e-12 && (q[1] - p[1]).abs() < 1e-12 && (q[2] - p[2]).abs() < 1e-12)
            .expect("node exists")
    };
    let mut pres: Vec<Prescribed> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, -kappa * BEAM_L * nodes[n][1]))
        .collect();
    let o = at([0.0, 0.0, 0.0]);
    let top = at([0.0, BEAM_H / 2.0, 0.0]);
    pres.push(Prescribed::new(o, 1, 0.0));
    pres.push(Prescribed::new(o, 2, 0.0));
    pres.push(Prescribed::new(top, 2, 0.0));
    (m, pres)
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Every value of a nonlinear result in a fixed order: displacement, reactions, per increment the step, load factor,
/// halvings, residuals and energies, then the state values.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap_or(&[]));
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} {detail}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// What the reading found on the increments of one solve.
#[derive(Debug, Default)]
struct Reading {
    /// Read pairs that violate the bound.
    violations: usize,
    /// Read pairs.
    pairs: usize,
    /// Increments without an informative read pair.
    uninformative: usize,
    /// Increments with a set change.
    with_change: usize,
    /// The largest `r_{k+1} / r_k^2` over the read pairs above the floor (infinite if `r_k = 0`).
    max_ratio: f64,
    /// Every residual finite.
    finite: bool,
}

/// The reading Q(C, F) of ADR 0515 D1; `exempt` false reads the pairs whose correction changed the set as well (N2).
fn read(steps: &[LoadStep], c: f64, f: f64, exempt: bool) -> Reading {
    let mut out = Reading {
        finite: true,
        ..Reading::default()
    };
    for s in steps {
        let r = &s.residuals;
        let ch = &s.set_changed;
        assert_eq!(r.len(), ch.len(), "the record is as long as the residuals");
        out.finite &= r.iter().all(|x| x.is_finite());
        out.with_change += usize::from(ch.iter().any(|&x| x));
        let mut informative = false;
        for k in 1..r.len().saturating_sub(1) {
            if exempt && ch[k + 1] {
                continue;
            }
            out.pairs += 1;
            let (a, b) = (r[k], r[k + 1]);
            informative |= a >= INFORMATIVE;
            if b > f {
                let ratio = if a > 0.0 { b / (a * a) } else { f64::INFINITY };
                out.max_ratio = out.max_ratio.max(ratio);
            }
            if b.is_nan() || b > (c * a * a).max(f) {
                out.violations += 1;
            }
        }
        out.uninformative += usize::from(!informative);
    }
    out
}

/// The order of the last three residuals above `f` counted from the last set change (N4, secondary); `None` if fewer than
/// three.
fn order(s: &LoadStep, f: f64) -> Option<f64> {
    let last = s.set_changed.iter().rposition(|&x| x).unwrap_or(0);
    let above: Vec<f64> = s.residuals[last..].iter().copied().filter(|&r| r > f).collect();
    if above.len() < 3 {
        return None;
    }
    let [a, b, c] = [above[above.len() - 3], above[above.len() - 2], above[above.len() - 1]];
    Some((c / b).ln() / (b / a).ln())
}

/// The nine solves of the item: S1, S2, S3 and the six P1 beams, with the active-set record asked for or not.
fn nine(record: bool) -> Vec<(String, Result<NonlinearResult, String>)> {
    let opts = |n: usize| NewtonOptions::new(n).record_active_set(record);
    let mut out = Vec::new();
    for (id, cc, steps) in [("S1", 1.25, 8usize), ("S2", 1.5, 10)] {
        let model = sphere(j2(0.0, 0.0), Some(p_of_c(cc) * SY));
        out.push((
            id.to_string(),
            solve(&model, &[], &[], &opts(steps)).map_err(|e| e.to_string()),
        ));
    }
    let model = sphere(j2(0.0, 0.0), Some(0.98 * 2.0 * 2.0f64.ln() * SY));
    out.push((
        "S3".to_string(),
        solve(&model, &[], &[], &opts(20)).map_err(|e| e.to_string()),
    ));
    for k in [1.25, 1.5, 2.0, 3.0, 4.0, 5.0] {
        let (m, pres) = beam(k * kappa_y());
        out.push((
            format!("P1 kappa = {k} kappa_y"),
            solve(&m, &pres, &[], &opts((4.0 * k).ceil() as usize)).map_err(|e| e.to_string()),
        ));
    }
    out
}

/// The number of points with an equivalent plastic strain above zero (13 state values per point, `alpha` the last).
fn plastic_points(r: &NonlinearResult) -> usize {
    r.states
        .iter()
        .map(|s| s.values().chunks(13).filter(|p| p[12] > 0.0).count())
        .sum()
}

#[test]
fn d1_q1_q2_q3_n2_n3_n4_consistent_tangent() {
    let mut c = Checks::new();
    let solves = nine(true);
    let mut max_ratio: f64 = 0.0;
    let mut all_finite = true;
    let (mut n2_violations, mut n3_s2, mut n3_s3) = (0, 0, 0);
    for (id, r) in &solves {
        let r = r.as_ref().unwrap_or_else(|e| panic!("{id} does not converge: {e}"));
        // D1: the diagnostic.
        let mut d1 = true;
        for s in &r.steps {
            d1 &= s.yielded.len() == s.residuals.len() && s.set_changed.len() == s.residuals.len();
            d1 &= !s.set_changed[0];
            for k in 1..s.yielded.len() {
                d1 &= s.yielded[k] == s.yielded[k - 1] || s.set_changed[k];
            }
        }
        if id == "S1" || id == "S2" {
            let last = r.steps.last().unwrap();
            let n = plastic_points(r);
            d1 &= *last.yielded.last().unwrap() == n;
            println!(
                "  {id}: last yielded {} of {n} plastic points",
                last.yielded.last().unwrap()
            );
        }
        c.holds(&format!("D1 {id}"), d1, "");
        // Q1, Q2; the least C that would pass is the largest ratio above the floor.
        let q = read(&r.steps, C_REG, F_REG, true);
        println!(
            "  {id}: {} increments ({} with a set change), {} read pairs, least C {:.3}, residuals finite {}",
            r.steps.len(),
            q.with_change,
            q.pairs,
            q.max_ratio,
            q.finite
        );
        c.holds(
            &format!("Q1 {id}: pairs violating Q(10, 1e-13)"),
            q.violations == 0 && q.finite,
            &format!("{} of {}", q.violations, q.pairs),
        );
        c.holds(
            &format!("Q2 {id}: every increment informative, some increment with a set change"),
            q.uninformative == 0 && q.with_change > 0,
            &format!("{} uninformative, {} with a change", q.uninformative, q.with_change),
        );
        all_finite &= q.finite && q.max_ratio.is_finite();
        max_ratio = max_ratio.max(q.max_ratio);
        // N2, N3.
        n2_violations += read(&r.steps, C_REG, F_REG, false).violations;
        if id == "S2" {
            n3_s2 = read(&r.steps, 1.0, F_REG, true).violations;
        }
        if id == "S3" {
            n3_s3 = read(&r.steps, 1.0, F_REG, true).violations;
        }
        // N4: the order, report only.
        let orders: Vec<f64> = r.steps.iter().filter_map(|s| order(s, F_REG)).collect();
        println!(
            "  N4 {id}: order of the last three residuals above 1e-13 from the last set change: least {:.3} over {} of {} \
             increments (undefined in {})",
            orders.iter().copied().fold(f64::INFINITY, f64::min),
            orders.len(),
            r.steps.len(),
            r.steps.len() - orders.len()
        );
    }
    c.holds(
        "Q3 the largest ratio r_{k+1} / r_k^2 over the nine solves is finite and <= 10",
        all_finite && max_ratio.is_finite() && max_ratio <= 10.0,
        &format!("{max_ratio:.4}"),
    );
    c.holds(
        "N2 the reading without the exemption fails (pairs violating, all solves)",
        n2_violations > 0,
        &format!("{n2_violations}"),
    );
    c.holds(
        "N3 Q(1, 1e-13) fails on S2 and S3",
        n3_s2 > 0 && n3_s3 > 0,
        &format!("S2 {n3_s2}, S3 {n3_s3}"),
    );
    c.finish();
}

#[test]
fn b1_the_record_is_opt_in_and_changes_no_bit() {
    let mut c = Checks::new();
    let expected: [(&str, &str, u64); 5] = [
        ("S1", "S1", 0xfa47_eed3_8c0f_def8),
        ("S2", "S2", 0x3425_56fa_7f12_f899),
        ("S3", "S3", 0x54fd_cd5c_d089_c65b),
        ("P1 1.25", "P1 kappa = 1.25 kappa_y", 0x1c36_1cb4_7319_fc47),
        ("P1 5", "P1 kappa = 5 kappa_y", 0x8855_6e1f_cea7_bf0f),
    ];
    for record in [false, true] {
        let all = nine(record);
        for (id, name, want) in expected {
            let r = all.iter().find(|(n, _)| n == name).unwrap().1.as_ref().unwrap();
            let got = digest(&flat(r));
            c.holds(
                &format!("B1 {id}, record {record}: digest of B0"),
                got == want,
                &format!("{got:#018x}"),
            );
            let empty = r.steps.iter().all(|s| s.yielded.is_empty() && s.set_changed.is_empty());
            c.holds(
                &format!(
                    "B1 {id}, record {record}: vectors {}",
                    if record { "filled" } else { "empty" }
                ),
                empty != record,
                "",
            );
        }
    }
    c.finish();
}

/// N1, the negative control: the continuum tangent (MU1 of ADR 0475: `theta_bar = 1 / (1 + h / (3 mu))` in `mat_j2.rs`)
/// fails the reading on every one of the nine solves.
#[test]
#[ignore = "negative control N1 of ADR 0515: passes only in the scratch copy with mutation MU1 (mat_j2.rs line 131, theta_bar without the (1 - theta) term); on the real tangent it fails by design"]
fn n1_continuum_tangent_fails_the_reading() {
    let mut c = Checks::new();
    for (id, r) in nine(true) {
        match r {
            Err(e) => c.holds(&format!("N1 {id}: fails (does not converge)"), true, &e),
            Ok(r) => {
                let q = read(&r.steps, C_REG, F_REG, true);
                println!(
                    "  N1 {id}: {} of {} pairs violate, least C {:.3e}",
                    q.violations, q.pairs, q.max_ratio
                );
                c.holds(&format!("N1 {id}: fails the reading"), q.violations > 0, "");
            }
        }
    }
    c.finish();
}
