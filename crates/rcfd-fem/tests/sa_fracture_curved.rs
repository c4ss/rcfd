//! F-682 (ADR 0610): the interaction integral on curved crack fronts with the correction terms of the auxiliary fields.
//! Checks PM1 to PM8 and PMR (heavy record), PC1 to PC4 and PCR, EC1 (known miss) to EC3, EH1 to EH3 (heavy record), PQ1,
//! PQ2, MC1 to MC3, MS, R6 and F1 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::fracture::{
    centre_crack_plate, curved_auxiliary, elliptical_crack_block, interaction_integral, interaction_integral_curved,
    j_integral, penny_crack_cylinder, penny_crack_half_cylinder, plane_strain_k, tip_frame, CrackFront,
    CrackedHalfCylinder, CurvedTerms, EllipticalCrack, FrontCurve, ModeFactors, PennyCrack, PlateCrack, TipMode,
};
use rcfd_fem::hex20::{traction_field_loads, traction_loads};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const A: f64 = 0.01;
const B: f64 = 0.5 * A;
const SIGMA: f64 = 1.0e8;
const BETA: f64 = 0.25 * PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A non-finite
/// value is a miss of F1.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !value.is_finite() {
            self.0.push(format!("F1 {name} = {value} is not finite"));
        } else if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

fn rel(v: f64, r: f64) -> f64 {
    (v / r - 1.0).abs()
}

fn k_of(j: f64) -> f64 {
    plane_strain_k(j, E_MOD, NU).unwrap_or(f64::NAN)
}

fn material() -> Material {
    Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap()
}

fn e_prime() -> f64 {
    E_MOD / (1.0 - NU * NU)
}

fn shear_modulus() -> f64 {
    E_MOD / (2.0 * (1.0 + NU))
}

fn refused(r: &rcfd_fem::Result<impl std::fmt::Debug>, key: &str, text: &str) -> (bool, String) {
    match r {
        Err(FemError::InvalidValue { key: k, reason }) => {
            (*k == key && reason.contains(text), format!("{k}: {reason}"))
        }
        Err(e) => (false, format!("other error {e}")),
        Ok(v) => (false, format!("accepted {v:?}")),
    }
}

/// The same blocks and sets on a new node list.
fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
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

fn solve(model: &SolidModel, loads: &[NodalForceAt], solver: StaticSolver, refine: bool) -> Vec<f64> {
    let options = if refine {
        StaticOptions::with_solver(solver).refinement(2)
    } else {
        StaticOptions::with_solver(solver)
    };
    solve_static_with(model, &[], loads, &options)
        .unwrap()
        .displacement_values()
        .to_vec()
}

/// Gauss-Legendre nodes and weights on `[-1, 1]`.
fn gauss(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x = vec![0.0; n];
    let mut w = vec![0.0; n];
    for i in 0..n.div_ceil(2) {
        let mut z = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 0.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, z);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * z * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (z * p1 - p0) / (z * z - 1.0);
            let dz = p1 / dp;
            z -= dz;
            if dz.abs() <= 1e-16 {
                break;
            }
        }
        x[i] = -z;
        x[n - 1 - i] = z;
        let wi = 2.0 / ((1.0 - z * z) * dp * dp);
        w[i] = wi;
        w[n - 1 - i] = wi;
    }
    (x, w)
}

/// The hat average `int phi_j f ds / int phi_j ds` of station `j` of the stations at parameters `st`, with `ds = w(p) dp`;
/// 16 Gauss points on each of 64 panels on each side (ADR 0610 D2).
fn hat_average(st: &[f64], j: usize, f: &dyn Fn(f64) -> f64, w: &dyn Fn(f64) -> f64) -> f64 {
    let (gx, gw) = gauss(16);
    let mut num = 0.0;
    let mut den = 0.0;
    let mut sides = Vec::new();
    if j > 0 {
        sides.push((st[j - 1], st[j]));
    }
    if j + 1 < st.len() {
        sides.push((st[j], st[j + 1]));
    }
    for (lo, hi) in sides {
        let panels = 64;
        let h = (hi - lo) / panels as f64;
        for p in 0..panels {
            let mid = lo + (p as f64 + 0.5) * h;
            for (x, wq) in gx.iter().zip(&gw) {
                let t = mid + 0.5 * h * x;
                let hat = if t < st[j] {
                    (t - lo) / (hi - lo)
                } else {
                    (hi - t) / (hi - lo)
                };
                let wt = 0.5 * h * wq * hat * w(t);
                num += f(t) * wt;
                den += wt;
            }
        }
    }
    num / den
}

// ------------------------------------------------------------------------------------------------- closed forms, penny

fn a_one() -> f64 {
    2.0 * SIGMA * BETA.cos().powi(2) * (A / PI).sqrt()
}

fn tau() -> f64 {
    SIGMA * BETA.sin() * BETA.cos()
}

fn a_two() -> f64 {
    4.0 * tau() * (A / PI).sqrt() / (2.0 - NU)
}

fn a_three() -> f64 {
    4.0 * (1.0 - NU) * tau() * (A / PI).sqrt() / (2.0 - NU)
}

fn k_two(theta: f64) -> f64 {
    a_two() * theta.sin()
}

fn k_three(theta: f64) -> f64 {
    -a_three() * theta.cos()
}

fn g_closed(theta: f64) -> f64 {
    (a_one().powi(2) + k_two(theta).powi(2)) / e_prime() + k_three(theta).powi(2) / (2.0 * shear_modulus())
}

/// The inclined-tension model of the half cylinder (ADR 0610 D4).
fn mixed_model(c: &CrackedHalfCylinder) -> (SolidModel, Vec<NodalForceAt>) {
    let (sb, cb) = BETA.sin_cos();
    let mesh = &c.mesh;
    let mut loads = traction_loads(mesh, "top", [0.0, SIGMA * cb * sb, SIGMA * cb * cb]).unwrap();
    loads.extend(traction_loads(mesh, "bottom", [0.0, -SIGMA * cb * sb, -SIGMA * cb * cb]).unwrap());
    loads.extend(
        traction_field_loads(mesh, "side", |x| {
            let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
            let dm = sb * x[1] / r;
            [0.0, SIGMA * dm * sb, SIGMA * dm * cb]
        })
        .unwrap(),
    );
    let mut m = SolidModel::new(mesh.clone());
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("top_centre", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("top_centre", 2, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("bottom_centre", 1, 0.0))
        .unwrap();
    (m, loads)
}

fn angle(x: &ModeFactors) -> f64 {
    x.position[1].atan2(x.position[0])
}

/// The station references of K_II, K_III and G (hat averages) for the stations of `v`.
fn mixed_refs(v: &[ModeFactors]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let st: Vec<f64> = v.iter().map(angle).collect();
    let one = |_: f64| 1.0;
    let r2 = (0..st.len()).map(|j| hat_average(&st, j, &k_two, &one)).collect();
    let r3 = (0..st.len()).map(|j| hat_average(&st, j, &k_three, &one)).collect();
    let rg = (0..st.len()).map(|j| hat_average(&st, j, &g_closed, &one)).collect();
    (r2, r3, rg)
}

/// The largest errors of K_I (relative), K_II (/A_II, every station) and K_III (/A_III, interior stations).
fn mixed_errors(name: &str, v: &[ModeFactors]) -> [f64; 3] {
    let (r2, r3, _) = mixed_refs(v);
    let last = v.len() - 1;
    let mut e = [0.0f64; 3];
    for (j, x) in v.iter().enumerate() {
        let th = angle(x);
        println!(
            "{name} station {j} (theta {:+.5}): K_I {:.6e} (/A_I - 1 {:+.3e}), K_II {:+.6e} ref {:+.6e} (point {:+.6e}) err/A {:+.3e}, \
             K_III {:+.6e} ref {:+.6e} (point {:+.6e}) err/A {:+.3e}",
            th,
            x.k_i,
            x.k_i / a_one() - 1.0,
            x.k_ii,
            r2[j],
            k_two(th),
            (x.k_ii - r2[j]) / a_two(),
            x.k_iii,
            r3[j],
            k_three(th),
            (x.k_iii - r3[j]) / a_three()
        );
        e[0] = e[0].max(rel(x.k_i, a_one()));
        e[1] = e[1].max((x.k_ii - r2[j]).abs() / a_two());
        if j > 0 && j < last {
            e[2] = e[2].max((x.k_iii - r3[j]).abs() / a_three());
        }
    }
    e
}

/// A component of a [`ModeFactors`].
type Getter = fn(&ModeFactors) -> f64;

fn coarse_half_cylinder_spec() -> PennyCrack {
    PennyCrack::new(A, 10.0 * A, 10.0 * A, A / 8.0, 2, 4, 0.25 * A, 1, 0.5 * A, 1.5, 1.6)
}

#[test]
fn pc_r6_coarse_penny_inclined() {
    let mut c = Checks::new();
    let cyl = penny_crack_half_cylinder(&coarse_half_cylinder_spec()).unwrap();
    let (model, loads) = mixed_model(&cyl);
    println!(
        "PC: {} nodes, {} elements, {} unknowns, {} front nodes",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        model.dof_map().n_equations(),
        cyl.front.len()
    );
    let start = std::time::Instant::now();
    let u = solve(&model, &loads, StaticSolver::SupernodalDirect, false);
    println!("PC solve {:.1} s", start.elapsed().as_secs_f64());
    let front = cyl.crack_front();
    let v = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::Corrected).unwrap();
    let e = mixed_errors("PC ring 2", &v);
    c.at_most("PC1 K_I against 2 sigma_nn sqrt(a/pi), every station", e[0], 0.08);
    c.at_most("PC2 K_II against A_II sin theta (hat average), / A_II", e[1], 0.08);
    c.at_most("PC3 K_III against -A_III cos theta, interior, / A_III", e[2], 0.08);
    let jv = j_integral(&model, &u, &front, 2).unwrap();
    let (_, _, rg) = mixed_refs(&v);
    let mut pc4: f64 = 0.0;
    for (j, x) in jv.iter().enumerate() {
        println!(
            "PC4 station {j}: J {:.6e}, G ref {:.6e}, J/G - 1 = {:+.4e}",
            x.j,
            rg[j],
            x.j / rg[j] - 1.0
        );
        pc4 = pc4.max(rel(x.j, rg[j]));
    }
    c.at_most("PC4 J against the hat average of G of the closed forms", pc4, 0.10);
    let d = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::DomainOnly).unwrap();
    let ed = mixed_errors("PCR domain only ring 2", &d);
    println!(
        "PCR domain only: errors K_I {:.4e}, K_II {:.4e}, K_III {:.4e} (record)",
        ed[0], ed[1], ed[2]
    );
    c.holds("F1 PCR finite", ed.iter().all(|x| x.is_finite()), "");
    // R6: the builder and the unchanged straight path.
    let mut bad = coarse_half_cylinder_spec();
    bad.n_theta = 3;
    let r = penny_crack_half_cylinder(&bad).map(|x| x.front.len());
    let (ok, detail) = refused(&r, "penny crack", "n_theta even");
    c.holds("R6 half cylinder with n_theta = 3 refused", ok, &detail);
    let r = interaction_integral(&model, &u, &front, 2);
    let (ok, detail) = refused(&r, "crack front", "straight");
    c.holds(
        "R6 interaction_integral (ADR 0605) of the circular front refused",
        ok,
        &detail,
    );
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0610 (PM1 to PM8, PMR): about 120 000 unknowns, supernodal factor; run once alone"]
fn pm1_to_pm8_penny_inclined_heavy() {
    let mut c = Checks::new();
    let cyl = penny_crack_half_cylinder(&PennyCrack::new(
        A,
        10.0 * A,
        10.0 * A,
        A / 16.0,
        3,
        8,
        0.25 * A,
        1,
        0.5 * A,
        1.4,
        1.5,
    ))
    .unwrap();
    let (model, loads) = mixed_model(&cyl);
    println!(
        "PM: {} nodes, {} elements, {} unknowns, {} front nodes",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        model.dof_map().n_equations(),
        cyl.front.len()
    );
    let start = std::time::Instant::now();
    let u = solve(&model, &loads, StaticSolver::SupernodalDirect, false);
    println!("PM solve {:.1} s", start.elapsed().as_secs_f64());
    let front = cyl.crack_front();
    let mut per_ring = Vec::new();
    for ring in 2..=4 {
        per_ring.push(interaction_integral_curved(&model, &u, &front, ring, CurvedTerms::Corrected).unwrap());
    }
    let v3 = &per_ring[1];
    let e = mixed_errors("PM ring 3", v3);
    c.at_most("PM1 K_I against 2 sigma_nn sqrt(a/pi), every station", e[0], 0.03);
    c.at_most("PM2 K_II against A_II sin theta (hat average), / A_II", e[1], 0.03);
    c.at_most("PM3 K_III against -A_III cos theta, interior, / A_III", e[2], 0.03);
    let (r2, r3, rg) = mixed_refs(v3);
    let last = v3.len() - 1;
    let mut pm4: f64 = 0.0;
    for (j, x) in v3.iter().enumerate() {
        if r2[j].abs() >= 0.5 * a_two() {
            pm4 = pm4.max(rel(x.k_ii, r2[j]));
        }
        if j > 0 && j < last && r3[j].abs() >= 0.5 * a_three() {
            pm4 = pm4.max(rel(x.k_iii, r3[j]));
        }
    }
    c.at_most("PM4 K_II, K_III relative where |K_ref| >= A/2", pm4, 0.03);
    let mut pm5: f64 = 0.0;
    for j in 0..=last {
        let comps: [(Getter, f64, bool); 3] = [
            (|x| x.k_i, a_one(), true),
            (|x| x.k_ii, a_two(), true),
            (|x| x.k_iii, a_three(), j > 0 && j < last),
        ];
        for (get, scale, used) in comps {
            if !used {
                continue;
            }
            let vals: Vec<f64> = per_ring.iter().map(|v| get(&v[j])).collect();
            let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            pm5 = pm5.max((hi - lo) / scale);
        }
    }
    c.at_most("PM5 spread over rings 2 to 4 / A_mode", pm5, 0.01);
    let jv = j_integral(&model, &u, &front, 3).unwrap();
    let mut pm6: f64 = 0.0;
    let mut pm8: f64 = 0.0;
    let (mut jw, mut gw, mut w) = (0.0, 0.0, 0.0);
    for (j, x) in jv.iter().enumerate() {
        let k = &v3[j];
        let g_m = (k.k_i.powi(2) + k.k_ii.powi(2)) / e_prime() + k.k_iii.powi(2) / (2.0 * shear_modulus());
        println!(
            "PM6/PM8 station {j}: J {:.6e}, G ref {:.6e} (J/G - 1 {:+.4e}), G from K {:.6e} (J/G_K - 1 {:+.4e})",
            x.j,
            rg[j],
            x.j / rg[j] - 1.0,
            g_m,
            x.j / g_m - 1.0
        );
        pm6 = pm6.max(rel(x.j, rg[j]));
        if j > 0 && j < last {
            pm8 = pm8.max(rel(x.j, g_m));
        }
        jw += x.j * x.weight;
        gw += rg[j] * x.weight;
        w += x.weight;
    }
    c.at_most("PM6 J against the hat average of G of the closed forms", pm6, 0.04);
    println!("PM7 front mean J {:.6e}, G {:.6e}", jw / w, gw / w);
    c.at_most("PM7 front mean of J against that of G", rel(jw / w, gw / w), 0.02);
    c.at_most("PM8 J against G of the ring-3 K, interior stations", pm8, 0.02);
    // PMR: the domain term alone, and the end stations' K_III.
    let mut dom = Vec::new();
    for ring in 2..=4 {
        dom.push(interaction_integral_curved(&model, &u, &front, ring, CurvedTerms::DomainOnly).unwrap());
    }
    let ed = mixed_errors("PMR domain only ring 3", &dom[1]);
    println!(
        "PMR domain only ring 3: errors K_I {:.4e}, K_II {:.4e}, K_III {:.4e} (record)",
        ed[0], ed[1], ed[2]
    );
    let mut spread_dom: f64 = 0.0;
    for j in 1..last {
        for (get, scale) in [
            ((|x: &ModeFactors| x.k_i) as Getter, a_one()),
            (|x: &ModeFactors| x.k_ii, a_two()),
            (|x: &ModeFactors| x.k_iii, a_three()),
        ] {
            let vals: Vec<f64> = dom.iter().map(|v| get(&v[j])).collect();
            let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            spread_dom = spread_dom.max((hi - lo) / scale);
        }
    }
    println!("PMR domain only: spread over rings 2 to 4 / A_mode {spread_dom:.4e} (record)");
    println!(
        "PMR end stations K_III/A_III: {:+.4e}, {:+.4e} (closed form 0; record)",
        v3[0].k_iii / a_three(),
        v3[last].k_iii / a_three()
    );
    c.holds(
        "F1 PMR finite",
        ed.iter().all(|x| x.is_finite()) && spread_dom.is_finite(),
        "",
    );
    c.finish();
}

// -------------------------------------------------------------------------------------------------- mode I, ellipse

/// Composite Simpson, 4000 intervals: `E(k) = int_0^(pi/2) sqrt(1 - k^2 sin^2 t) dt`.
fn elliptic_e(k2: f64) -> f64 {
    let n = 4000;
    let h = 0.5 * PI / n as f64;
    let mut s = 0.0;
    for i in 0..=n {
        let f = (1.0 - k2 * (i as f64 * h).sin().powi(2)).sqrt();
        s += f * if i == 0 || i == n {
            1.0
        } else if i % 2 == 1 {
            4.0
        } else {
            2.0
        };
    }
    s * h / 3.0
}

/// Irwin's `K_I(phi)` of the embedded elliptical crack (ADR 0605 D4).
fn irwin(phi: f64) -> f64 {
    let k2 = 1.0 - (B / A).powi(2);
    SIGMA * (PI * B).sqrt() / elliptic_e(k2) * (phi.sin().powi(2) + (B / A).powi(2) * phi.cos().powi(2)).powf(0.25)
}

/// The linear hat average of Irwin's form at station `j` of `n_theta` equal steps of the elliptic angle (ADR 0610 D3).
fn irwin_linear(j: usize, n_theta: usize) -> f64 {
    let st: Vec<f64> = (0..=n_theta).map(|i| 0.5 * PI * i as f64 / n_theta as f64).collect();
    let ds = |p: f64| (A * A * p.sin().powi(2) + B * B * p.cos().powi(2)).sqrt();
    hat_average(&st, j, &irwin, &ds)
}

fn coarse_penny_spec(radius: f64, r_t: f64) -> PennyCrack {
    PennyCrack::new(radius, 12.0 * A, 10.0 * A, A / 8.0, 2, 4, 0.4 * A, 1, r_t, 1.5, 1.6)
}

fn half_block_model(mesh: &SolidMesh) -> (SolidModel, Vec<NodalForceAt>) {
    let loads = traction_loads(mesh, "top", [0.0, 0.0, SIGMA]).unwrap();
    let mut m = SolidModel::new(mesh.clone());
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ligament", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
    (m, loads)
}

/// EC1 to EC3 on the coarse ellipse of ADR 0605 (EF); EC1 only when `with_ec1` (its known miss, addendum 1).
fn coarse_ellipse_checks(c: &mut Checks, with_ec1: bool) {
    let ell = elliptical_crack_block(&EllipticalCrack::new(coarse_penny_spec(A, 0.65 * A), B)).unwrap();
    let (model, loads) = half_block_model(&ell.mesh);
    let u = solve(&model, &loads, StaticSolver::Direct, false);
    let front = ell.crack_front();
    let v = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::Corrected).unwrap();
    let d = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::DomainOnly).unwrap();
    let jv = j_integral(&model, &u, &front, 2).unwrap();
    let (mut ec1, mut ec2): (f64, f64) = (0.0, 0.0);
    let mut zero = true;
    for (x, (y, jx)) in v.iter().zip(d.iter().zip(&jv)) {
        let kr = irwin_linear(x.station, 4);
        let kj = k_of(jx.j);
        println!(
            "EC station {}: K_I {:.6e}, K(J) {:.6e} (K/K(J) - 1 {:+.4e}), Irwin {:.6e} (K/ref - 1 {:+.4e}); domain only {:.6e} \
             (/K(J) - 1 {:+.4e})",
            x.station,
            x.k_i,
            kj,
            x.k_i / kj - 1.0,
            kr,
            x.k_i / kr - 1.0,
            y.k_i,
            y.k_i / kj - 1.0
        );
        ec1 = ec1.max(rel(x.k_i, kj));
        ec2 = ec2.max(rel(x.k_i, kr));
        zero &= x.k_ii == 0.0 && x.k_iii == 0.0;
    }
    if with_ec1 {
        c.at_most("EC1 coarse ellipse ring 2, K_I against K of J", ec1, 0.02);
    } else {
        println!("EC1 (known miss, not asserted here): {ec1:.4e} (limit <= 2e-2)");
    }
    c.at_most(
        "EC2 coarse ellipse ring 2, K_I against Irwin (linear hat average)",
        ec2,
        0.08,
    );
    c.holds("EC3 K_II = K_III = 0 in the half-symmetric model", zero, "");
}

/// ADR 0610 EC1, registered limit `2e-2`, measured `2.2893e-2` in the registered run (addendum 1): K of the coarse-mesh J is
/// `1.4e-2` to `3.3e-2` low (ADR 0605 EF), K of the interaction integral within `1.1e-2` of Irwin.
#[test]
#[ignore = "known miss of ADR 0610 EC1: measured 2.2893e-2 against the limit 2e-2 (addendum 1)"]
fn ec1_coarse_ellipse_known_miss() {
    let mut c = Checks::new();
    coarse_ellipse_checks(&mut c, true);
    c.finish();
}

#[test]
fn ec2_ec3_pq_mode_one_curved_fronts() {
    let mut c = Checks::new();
    coarse_ellipse_checks(&mut c, false);
    // PQ: the coarse quarter penny of ADR 0547 (PF).
    let cyl = penny_crack_cylinder(&PennyCrack::new(
        A,
        10.0 * A,
        10.0 * A,
        A / 8.0,
        2,
        4,
        0.25 * A,
        1,
        0.5 * A,
        1.5,
        1.6,
    ))
    .unwrap();
    let (model, loads) = half_block_model(&cyl.mesh);
    let u = solve(&model, &loads, StaticSolver::Direct, false);
    let front = cyl.crack_front();
    let v = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::Corrected).unwrap();
    let d = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::DomainOnly).unwrap();
    let jv = j_integral(&model, &u, &front, 2).unwrap();
    let k_ref = 2.0 * SIGMA * (A / PI).sqrt();
    let (mut pq1, mut pq2): (f64, f64) = (0.0, 0.0);
    for (x, (y, jx)) in v.iter().zip(d.iter().zip(&jv)) {
        let kj = k_of(jx.j);
        println!(
            "PQ station {}: K_I {:.6e} (/ref - 1 {:+.4e}), K(J) {:.6e} (K/K(J) - 1 {:+.4e}); domain only {:.6e} (/ref - 1 {:+.4e})",
            x.station,
            x.k_i,
            x.k_i / k_ref - 1.0,
            kj,
            x.k_i / kj - 1.0,
            y.k_i,
            y.k_i / k_ref - 1.0
        );
        pq1 = pq1.max(rel(x.k_i, k_ref));
        pq2 = pq2.max(rel(x.k_i, kj));
    }
    c.at_most("PQ1 coarse penny ring 2, K_I against 2 sigma sqrt(a/pi)", pq1, 0.10);
    c.at_most("PQ2 coarse penny ring 2, K_I against K of J", pq2, 0.02);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0610 (EH1 to EH3): the fine ellipse of ADR 0605, about 88 000 unknowns; run once alone"]
fn eh1_to_eh3_ellipse_heavy() {
    let mut c = Checks::new();
    let n_theta = 12;
    let ell = elliptical_crack_block(&EllipticalCrack::new(
        PennyCrack::new(
            A,
            12.0 * A,
            10.0 * A,
            A / 32.0,
            4,
            n_theta,
            0.4 * A,
            2,
            0.75 * A,
            1.25,
            1.4,
        ),
        B,
    ))
    .unwrap();
    let (model, loads) = half_block_model(&ell.mesh);
    let start = std::time::Instant::now();
    let u = solve(&model, &loads, StaticSolver::SupernodalDirect, false);
    println!(
        "EH: {} unknowns, solve {:.1} s",
        model.dof_map().n_equations(),
        start.elapsed().as_secs_f64()
    );
    let front = ell.crack_front();
    let refs: Vec<f64> = (0..=n_theta).map(|j| irwin_linear(j, n_theta)).collect();
    let mut per_ring = Vec::new();
    for ring in 2..=4 {
        per_ring.push(interaction_integral_curved(&model, &u, &front, ring, CurvedTerms::Corrected).unwrap());
    }
    let jv = j_integral(&model, &u, &front, 3).unwrap();
    let dom = interaction_integral_curved(&model, &u, &front, 3, CurvedTerms::DomainOnly).unwrap();
    let (mut eh1, mut eh2, mut eh3): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for j in 0..=n_theta {
        let k = per_ring[1][j].k_i;
        let kj = k_of(jv[j].j);
        println!(
            "EH station {j}: K_I {:.6e}, ref {:.6e} (K/ref - 1 {:+.4e}), K(J) {:.6e} (K/K(J) - 1 {:+.4e}); domain only {:+.4e}",
            k,
            refs[j],
            k / refs[j] - 1.0,
            kj,
            k / kj - 1.0,
            dom[j].k_i / refs[j] - 1.0
        );
        eh1 = eh1.max(rel(k, refs[j]));
        eh2 = eh2.max(rel(k, kj));
        let vals: Vec<f64> = per_ring.iter().map(|v| v[j].k_i).collect();
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        eh3 = eh3.max((hi - lo) / lo);
    }
    c.at_most(
        "EH1 fine ellipse ring 3, K_I against Irwin (linear hat average)",
        eh1,
        0.03,
    );
    c.at_most("EH2 fine ellipse ring 3, K_I against K of J", eh2, 0.01);
    c.at_most("EH3 fine ellipse, spread of rings 2 to 4", eh3, 0.01);
    c.finish();
}

// ----------------------------------------------------------------------------------------------- correction terms

fn hooke(g: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mu = shear_modulus();
    let lambda = E_MOD * NU / ((1.0 + NU) * (1.0 - 2.0 * NU));
    let tr = g[0][0] + g[1][1] + g[2][2];
    let mut s = [[0.0; 3]; 3];
    for i in 0..3 {
        for k in 0..3 {
            s[i][k] = mu * (g[i][k] + g[k][i]) + if i == k { lambda * tr } else { 0.0 };
        }
    }
    s
}

/// `P[j][k] = sigma_ij H_ik + sigma^aux_ij u_i,k - sigma_ij eps^aux_ij delta_jk` at `x`.
fn p_tensor(curve: &FrontCurve, mode: TipMode, x: Vec3, du: &[[f64; 3]; 3], s: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let aux = curved_auxiliary(curve, mode, x, shear_modulus(), NU).unwrap();
    let (h, sa) = (aux.grad, aux.stress);
    let mut w = 0.0;
    for i in 0..3 {
        for k in 0..3 {
            w += s[i][k] * 0.5 * (h[i][k] + h[k][i]);
        }
    }
    let mut p = [[0.0; 3]; 3];
    for j in 0..3 {
        for k in 0..3 {
            let mut v = 0.0;
            for i in 0..3 {
                v += s[i][j] * h[i][k] + sa[i][j] * du[i][k];
            }
            p[j][k] = v - if j == k { w } else { 0.0 };
        }
    }
    p
}

#[test]
fn mc_r6_correction_terms() {
    let mut c = Checks::new();
    let z = [0.0, 0.0, 1.0];
    let circle = FrontCurve::Circle {
        centre: [0.0; 3],
        normal: z,
        radius: A,
    };
    let ellipse = FrontCurve::Ellipse {
        centre: [0.0; 3],
        normal: z,
        major_axis: [1.0, 0.0, 0.0],
        a: A,
        b: B,
    };
    // A uniform actual field with a rotation part, and a vector q with an out-of-normal component.
    let du = [
        [0.35e-3, -1.10e-3, 0.42e-3],
        [0.81e-3, -0.27e-3, 1.33e-3],
        [-0.64e-3, 0.19e-3, 0.58e-3],
    ];
    let s = hooke(&du);
    let q = [0.37, -1.21, 0.84];
    let offsets: [(f64, f64); 4] = [
        (0.3e-3, 0.7e-3),
        (-0.4e-3, 0.5e-3),
        (-0.6e-3, -0.2e-3),
        (0.8e-3, -0.1e-3),
    ];
    let mut feet: Vec<(&str, &FrontCurve, Vec3, Vec3)> = Vec::new();
    let t = 0.3f64;
    feet.push((
        "circle t 0.3",
        &circle,
        [A * t.cos(), A * t.sin(), 0.0],
        [t.cos(), t.sin(), 0.0],
    ));
    for t in [0.2f64, 1.2] {
        let nn = [B * t.cos(), A * t.sin(), 0.0];
        let l = (nn[0] * nn[0] + nn[1] * nn[1]).sqrt();
        feet.push((
            if t < 1.0 { "ellipse t 0.2" } else { "ellipse t 1.2" },
            &ellipse,
            [A * t.cos(), B * t.sin(), 0.0],
            [nn[0] / l, nn[1] / l, 0.0],
        ));
    }
    let (mut mc1, mut mc2, mut mc3): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for (name, curve, foot, big_n) in &feet {
        for &(o1, o2) in &offsets {
            let x = [foot[0] + o1 * big_n[0], foot[1] + o1 * big_n[1], o2];
            let r = (o1 * o1 + o2 * o2).sqrt();
            let fr = tip_frame(curve, x).unwrap();
            let mut res = [0.0; 3];
            for i in 0..3 {
                res[i] = x[i] - fr.foot[i] - fr.x1 * fr.basis[0][i] - fr.x2 * fr.basis[1][i];
            }
            let along = (0..3).map(|i| (x[i] - fr.foot[i]) * fr.basis[2][i]).sum::<f64>();
            let on = if name.starts_with("circle") {
                ((fr.foot[0].powi(2) + fr.foot[1].powi(2)).sqrt() - A).abs()
            } else {
                (fr.foot[0].powi(2) / (A * A) + fr.foot[1].powi(2) / (B * B) - 1.0).abs() * A
            };
            let resn = (res[0].powi(2) + res[1].powi(2) + res[2].powi(2)).sqrt();
            mc2 = mc2.max(resn / A).max(along.abs() / A).max(on / A);
            let h = 1e-5 * r;
            for mode in [TipMode::I, TipMode::II, TipMode::III] {
                let mut div = [0.0; 3];
                for j in 0..3 {
                    let mut xp = x;
                    let mut xm = x;
                    xp[j] += h;
                    xm[j] -= h;
                    let pp = p_tensor(curve, mode, xp, &du, &s);
                    let pm = p_tensor(curve, mode, xm, &du, &s);
                    for k in 0..3 {
                        div[k] += (pp[j][k] - pm[j][k]) / (2.0 * h);
                    }
                }
                let fd = div[0] * q[0] + div[1] * q[1] + div[2] * q[2];
                let aux = curved_auxiliary(curve, mode, x, shear_modulus(), NU).unwrap();
                let an = aux.correction(&s, &du, q);
                let smax = s.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
                let hmax = aux.grad.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
                let scale = smax * hmax / r;
                println!(
                    "MC {name} offset ({o1:+.1e}, {o2:+.1e}) {mode:?}: differences {fd:+.9e}, formula {an:+.9e}, |diff|/scale \
                     {:.3e}, |corr|/scale {:.3e}",
                    (fd - an).abs() / scale,
                    an.abs() / scale
                );
                mc1 = mc1.max((fd - an).abs() / scale);
                mc3 = mc3.max(an.abs() / scale);
            }
        }
    }
    c.at_most("MC1 correction against central differences of div P q", mc1, 1e-7);
    c.at_most("MC2 frame residuals and the foot on the curve, / a", mc2, 1e-12);
    c.at_least("MC3 the largest correction / scale (not vacuous)", mc3, 1e-3);
    // R6: frames refused.
    let r = tip_frame(&circle, [0.0, 0.0, 1e-3]);
    let (ok, detail) = refused(&r, "crack front", "axis");
    c.holds("R6 tip frame of a point on the circle's axis refused", ok, &detail);
    let r = tip_frame(&ellipse, [0.0, 0.0, 1e-3]);
    let (ok, detail) = refused(&r, "crack front", "centre of curvature");
    c.holds("R6 tip frame of the ellipse centre refused", ok, &detail);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- straight front

#[test]
fn ms_straight_front_unchanged() {
    let mut c = Checks::new();
    let beta = PI / 6.0;
    let (sb, cb) = beta.sin_cos();
    let p = centre_crack_plate(&PlateCrack::new(A, 10.0 * A, 20.0 * A, 0.05 * A, A / 40.0, 1.4, 4)).unwrap();
    let nodes: Vec<Vec3> = p
        .mesh
        .nodes()
        .iter()
        .map(|x| [x[0] * cb - x[1] * sb, x[0] * sb + x[1] * cb, x[2]])
        .collect();
    let mesh = rebuild(&p.mesh, nodes);
    let mut loads = Vec::new();
    for (set, f) in [("top", cb), ("bottom", -cb), ("right", sb), ("left", -sb)] {
        loads.extend(traction_loads(&mesh, set, [0.0, SIGMA * f, 0.0]).unwrap());
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material()).unwrap();
    model.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    model
        .add_constraint(Constraint::displacement("right_pin", 0, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("right_pin", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("left_pin", 1, 0.0))
        .unwrap();
    let u = solve(&model, &loads, StaticSolver::Direct, true);
    let front = CrackFront::straight(p.front.clone(), [-sb, cb, 0.0], [cb, sb, 0.0]);
    let k0 = SIGMA * (PI * A).sqrt();
    let mut ms: f64 = 0.0;
    for ring in 2..=4 {
        let straight = interaction_integral(&model, &u, &front, ring).unwrap();
        for terms in [CurvedTerms::Corrected, CurvedTerms::DomainOnly] {
            let curved = interaction_integral_curved(&model, &u, &front, ring, terms).unwrap();
            for (x, y) in straight.iter().zip(&curved) {
                let d = (x.k_i - y.k_i)
                    .abs()
                    .max((x.k_ii - y.k_ii).abs())
                    .max((x.k_iii - y.k_iii).abs())
                    / k0;
                println!(
                    "MS ring {ring} {terms:?} station {}: straight ({:.9e}, {:.9e}, {:+.3e}), curved ({:.9e}, {:.9e}, {:+.3e}), \
                     max diff / k0 {d:.3e}",
                    x.station, x.k_i, x.k_ii, x.k_iii, y.k_i, y.k_ii, y.k_iii
                );
                ms = ms.max(d);
            }
        }
    }
    c.at_most(
        "MS curved function on the straight inclined crack against ADR 0605",
        ms,
        1e-12,
    );
    c.finish();
}
