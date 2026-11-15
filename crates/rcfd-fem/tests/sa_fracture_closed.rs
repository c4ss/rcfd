//! F-689 (ADR 0618): the interaction integral on closed curved fronts and at mirror-plane stations. Checks WC1 to WC5 and
//! WCR, WM1 to WM7 and WMR (heavy record), MW1 to MW4 and MWR, CL1, CL2, WE1, WE2, R1 to R7 and F1 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::fracture::{
    centre_crack_plate, interaction_integral, interaction_integral_curved, j_integral, penny_crack_half_cylinder,
    penny_crack_whole_cylinder, CrackFront, CrackedHalfCylinder, CurvedTerms, ModeFactors, PennyCrack, PlateCrack,
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
/// The in-plane direction of the shear, 35 deg (ADR 0618 D4).
const PSI: f64 = 7.0 * PI / 36.0;

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

fn solve(model: &SolidModel, loads: &[NodalForceAt]) -> Vec<f64> {
    solve_static_with(
        model,
        &[],
        loads,
        &StaticOptions::with_solver(StaticSolver::SupernodalDirect),
    )
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

/// The angles of the stations along the front, unwrapped (`atan2(Y/b, X/a)` with `b = a` for a circle).
fn unwrapped(v: &[ModeFactors], a: f64, b: f64) -> Vec<f64> {
    let mut th: Vec<f64> = Vec::with_capacity(v.len());
    for x in v {
        let t = (x.position[1] / b).atan2(x.position[0] / a);
        if let Some(&prev) = th.last() {
            let d = t - prev;
            th.push(prev + d - 2.0 * PI * (d / (2.0 * PI)).round());
        } else {
            th.push(t);
        }
    }
    th
}

/// The periodic hat average `int phi_j f ds / int phi_j ds` of station `j` of a closed front with station angles `st`
/// (unwrapped), `ds = w(p) dp`; 16 Gauss points on each of 64 panels on each side (ADR 0618 D4).
fn periodic_hat_average(st: &[f64], j: usize, f: &dyn Fn(f64) -> f64, w: &dyn Fn(f64) -> f64) -> f64 {
    let (gx, gw) = gauss(16);
    let m = st.len();
    let left = if j == 0 { st[m - 1] - 2.0 * PI } else { st[j - 1] };
    let right = if j + 1 == m { st[0] + 2.0 * PI } else { st[j + 1] };
    let mut num = 0.0;
    let mut den = 0.0;
    for (lo, hi) in [(left, st[j]), (st[j], right)] {
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

/// `K_II = A_II cos(theta - psi)` (ADR 0618 D4).
fn k_two(theta: f64) -> f64 {
    a_two() * (theta - PSI).cos()
}

/// `K_III = A_III sin(theta - psi)` (ADR 0618 D4).
fn k_three(theta: f64) -> f64 {
    a_three() * (theta - PSI).sin()
}

fn g_closed(theta: f64) -> f64 {
    (a_one().powi(2) + k_two(theta).powi(2)) / e_prime() + k_three(theta).powi(2) / (2.0 * shear_modulus())
}

/// The supports of the whole (free) body (ADR 0618 D3).
fn whole_supports(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("top_centre", c, 0.0))
            .unwrap();
    }
    for c in 0..2 {
        m.add_constraint(Constraint::displacement("bottom_centre", c, 0.0))
            .unwrap();
    }
    m.add_constraint(Constraint::displacement("top_rim", 0, 0.0)).unwrap();
}

/// The tractions of the uniform state `sigma d d` on `top`, `bottom` and `side`.
fn uniaxial_loads(mesh: &SolidMesh, d: Vec3) -> Vec<NodalForceAt> {
    let cb = d[2];
    let mut loads = traction_loads(mesh, "top", [SIGMA * cb * d[0], SIGMA * cb * d[1], SIGMA * cb * d[2]]).unwrap();
    loads.extend(
        traction_loads(
            mesh,
            "bottom",
            [-SIGMA * cb * d[0], -SIGMA * cb * d[1], -SIGMA * cb * d[2]],
        )
        .unwrap(),
    );
    loads.extend(
        traction_field_loads(mesh, "side", |x| {
            let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
            let dm = (d[0] * x[0] + d[1] * x[1]) / r;
            [SIGMA * dm * d[0], SIGMA * dm * d[1], SIGMA * dm * d[2]]
        })
        .unwrap(),
    );
    loads
}

/// The whole-cylinder model under uniaxial tension along `d`.
fn whole_model(mesh: &SolidMesh, d: Vec3) -> (SolidModel, Vec<NodalForceAt>) {
    let loads = uniaxial_loads(mesh, d);
    let mut m = SolidModel::new(mesh.clone());
    m.add_material("all", material()).unwrap();
    whole_supports(&mut m);
    (m, loads)
}

fn general_direction() -> Vec3 {
    let (sb, cb) = BETA.sin_cos();
    [sb * PSI.cos(), sb * PSI.sin(), cb]
}

/// The inclined-tension model of the half cylinder of ADR 0610 D4 (its test set-up, copied).
fn half_mixed_model(c: &CrackedHalfCylinder) -> (SolidModel, Vec<NodalForceAt>) {
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

/// The station references of K_II, K_III and G (periodic hat averages) for the stations of `v`.
fn closed_refs(v: &[ModeFactors]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let st = unwrapped(v, A, A);
    let one = |_: f64| 1.0;
    let r2 = (0..st.len())
        .map(|j| periodic_hat_average(&st, j, &k_two, &one))
        .collect();
    let r3 = (0..st.len())
        .map(|j| periodic_hat_average(&st, j, &k_three, &one))
        .collect();
    let rg = (0..st.len())
        .map(|j| periodic_hat_average(&st, j, &g_closed, &one))
        .collect();
    (r2, r3, rg)
}

/// The largest errors of K_I (relative), K_II (/A_II), K_III (/A_III), every station, and the relative errors of K_II and
/// K_III where `|K_ref| >= A/2`.
fn closed_errors(name: &str, v: &[ModeFactors]) -> [f64; 4] {
    let (r2, r3, _) = closed_refs(v);
    let st = unwrapped(v, A, A);
    let mut e = [0.0f64; 4];
    for (j, x) in v.iter().enumerate() {
        let th = st[j];
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
        e[2] = e[2].max((x.k_iii - r3[j]).abs() / a_three());
        if r2[j].abs() >= 0.5 * a_two() {
            e[3] = e[3].max(rel(x.k_ii, r2[j]));
        }
        if r3[j].abs() >= 0.5 * a_three() {
            e[3] = e[3].max(rel(x.k_iii, r3[j]));
        }
    }
    e
}

/// A component of a [`ModeFactors`].
type Getter = fn(&ModeFactors) -> f64;

/// ADR 0610 PC's spec: the coarse cylinder.
fn coarse_spec() -> PennyCrack {
    PennyCrack::new(A, 10.0 * A, 10.0 * A, A / 8.0, 2, 4, 0.25 * A, 1, 0.5 * A, 1.5, 1.6)
}

/// ADR 0610 PM's spec: the medium cylinder.
fn medium_spec() -> PennyCrack {
    PennyCrack::new(A, 10.0 * A, 10.0 * A, A / 16.0, 3, 8, 0.25 * A, 1, 0.5 * A, 1.4, 1.5)
}

#[test]
fn wc_coarse_whole_penny_general_direction() {
    let mut c = Checks::new();
    let cyl = penny_crack_whole_cylinder(&coarse_spec()).unwrap();
    let (model, loads) = whole_model(&cyl.mesh, general_direction());
    println!(
        "WC: {} nodes, {} elements, {} unknowns, {} front nodes",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        model.dof_map().n_equations(),
        cyl.front.len()
    );
    let start = std::time::Instant::now();
    let u = solve(&model, &loads);
    println!("WC solve {:.1} s", start.elapsed().as_secs_f64());
    let front = cyl.crack_front();
    let v = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::Corrected).unwrap();
    let e = closed_errors("WC ring 2", &v);
    c.at_most("WC1 K_I against 2 sigma_nn sqrt(a/pi), every station", e[0], 0.03);
    c.at_most("WC2 K_II against A_II cos(theta - psi), / A_II", e[1], 0.03);
    c.at_most("WC3 K_III against A_III sin(theta - psi), / A_III", e[2], 0.03);
    c.at_most("WC4 K_II, K_III relative where |K_ref| >= A/2", e[3], 0.03);
    let jv = j_integral(&model, &u, &front, 2).unwrap();
    let (_, _, rg) = closed_refs(&v);
    let mut wc5: f64 = 0.0;
    for (j, x) in jv.iter().enumerate() {
        println!(
            "WC5 station {j}: J {:.6e}, G ref {:.6e}, J/G - 1 = {:+.4e}, weight {:.6e}",
            x.j,
            rg[j],
            x.j / rg[j] - 1.0,
            x.weight
        );
        wc5 = wc5.max(rel(x.j, rg[j]));
    }
    c.at_most("WC5 J against the hat average of G of the closed forms", wc5, 0.10);
    let d = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::DomainOnly).unwrap();
    let ed = closed_errors("WCR domain only ring 2", &d);
    println!(
        "WCR domain only: errors K_I {:.4e}, K_II {:.4e}, K_III {:.4e} (record)",
        ed[0], ed[1], ed[2]
    );
    c.holds("F1 WCR finite", ed.iter().all(|x| x.is_finite()), "");
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0618 (WM1 to WM7, WMR): the medium whole cylinder, about 240 000 unknowns; run once alone"]
fn wm1_to_wm7_whole_penny_heavy() {
    let mut c = Checks::new();
    let cyl = penny_crack_whole_cylinder(&medium_spec()).unwrap();
    let (model, loads) = whole_model(&cyl.mesh, general_direction());
    println!(
        "WM: {} nodes, {} elements, {} unknowns, {} front nodes",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        model.dof_map().n_equations(),
        cyl.front.len()
    );
    let start = std::time::Instant::now();
    let u = solve(&model, &loads);
    println!("WM solve {:.1} s", start.elapsed().as_secs_f64());
    let front = cyl.crack_front();
    let mut per_ring = Vec::new();
    for ring in 2..=4 {
        per_ring.push(interaction_integral_curved(&model, &u, &front, ring, CurvedTerms::Corrected).unwrap());
    }
    let v3 = &per_ring[1];
    let e = closed_errors("WM ring 3", v3);
    c.at_most("WM1 K_I against 2 sigma_nn sqrt(a/pi), every station", e[0], 0.03);
    c.at_most("WM2 K_II against A_II cos(theta - psi), / A_II", e[1], 0.03);
    c.at_most("WM3 K_III against A_III sin(theta - psi), / A_III", e[2], 0.03);
    c.at_most("WM4 K_II, K_III relative where |K_ref| >= A/2", e[3], 0.03);
    let mut wm5: f64 = 0.0;
    for j in 0..v3.len() {
        let comps: [(Getter, f64); 3] = [(|x| x.k_i, a_one()), (|x| x.k_ii, a_two()), (|x| x.k_iii, a_three())];
        for (get, scale) in comps {
            let vals: Vec<f64> = per_ring.iter().map(|v| get(&v[j])).collect();
            let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            wm5 = wm5.max((hi - lo) / scale);
        }
    }
    c.at_most("WM5 spread over rings 2 to 4 / A_mode", wm5, 0.01);
    let jv = j_integral(&model, &u, &front, 3).unwrap();
    let (_, _, rg) = closed_refs(v3);
    let mut wm6: f64 = 0.0;
    let (mut jw, mut gw, mut w) = (0.0, 0.0, 0.0);
    for (j, x) in jv.iter().enumerate() {
        println!(
            "WM6 station {j}: J {:.6e}, G ref {:.6e} (J/G - 1 {:+.4e})",
            x.j,
            rg[j],
            x.j / rg[j] - 1.0
        );
        wm6 = wm6.max(rel(x.j, rg[j]));
        jw += x.j * x.weight;
        gw += rg[j] * x.weight;
        w += x.weight;
    }
    c.at_most("WM6 J against the hat average of G of the closed forms", wm6, 0.04);
    println!("WM7 front mean J {:.6e}, G {:.6e}", jw / w, gw / w);
    c.at_most("WM7 front mean of J against that of G", rel(jw / w, gw / w), 0.02);
    let dom = interaction_integral_curved(&model, &u, &front, 3, CurvedTerms::DomainOnly).unwrap();
    let ed = closed_errors("WMR domain only ring 3", &dom);
    println!(
        "WMR domain only ring 3: errors K_I {:.4e}, K_II {:.4e}, K_III {:.4e} (record)",
        ed[0], ed[1], ed[2]
    );
    c.holds("F1 WMR finite", ed.iter().all(|x| x.is_finite()), "");
    c.finish();
}

// ----------------------------------------------------------------------------------------------------- mirror plane

#[test]
fn mw_mirror_plane_option() {
    let mut c = Checks::new();
    let half = penny_crack_half_cylinder(&coarse_spec()).unwrap();
    let (hm, hl) = half_mixed_model(&half);
    let uh = solve(&hm, &hl);
    let plain = half.crack_front();
    let mirrored = half.crack_front().mirror_plane([0.0; 3], [1.0, 0.0, 0.0]);
    let v = interaction_integral_curved(&hm, &uh, &plain, 2, CurvedTerms::Corrected).unwrap();
    let vm = interaction_integral_curved(&hm, &uh, &mirrored, 2, CurvedTerms::Corrected).unwrap();
    let last = v.len() - 1;
    c.holds(
        "MW1 K_III at the two mirror-plane stations",
        vm[0].k_iii == 0.0 && vm[last].k_iii == 0.0,
        &format!("{:e}, {:e}", vm[0].k_iii, vm[last].k_iii),
    );
    let mut same = true;
    for (j, (x, y)) in v.iter().zip(&vm).enumerate() {
        same &= x.station == y.station
            && x.node == y.node
            && x.position == y.position
            && x.s == y.s
            && x.weight == y.weight
            && x.k_i == y.k_i
            && x.k_ii == y.k_ii
            && (j == 0 || j == last || x.k_iii == y.k_iii);
    }
    c.holds(
        "MW2 every other value bit-identical to the run without the option",
        same,
        "",
    );
    println!(
        "MWR without the option, end stations K_III/A_III: {:+.4e}, {:+.4e} (record; closed form 0)",
        v[0].k_iii / a_three(),
        v[last].k_iii / a_three()
    );
    // The whole cylinder under ADR 0610's load (psi = 90 deg, d_x = 0 exactly).
    let whole = penny_crack_whole_cylinder(&coarse_spec()).unwrap();
    let (sb, cb) = BETA.sin_cos();
    let (wm, wl) = whole_model(&whole.mesh, [0.0, sb, cb]);
    println!(
        "MW: half {} unknowns, whole {} unknowns",
        hm.dof_map().n_equations(),
        wm.dof_map().n_equations()
    );
    let uw = solve(&wm, &wl);
    let vw = interaction_integral_curved(&wm, &uw, &whole.crack_front(), 2, CurvedTerms::Corrected).unwrap();
    let mut mw3: f64 = 0.0;
    let mut positions = true;
    for (j, x) in vm.iter().enumerate() {
        let y = &vw[j];
        positions &= x.position == y.position;
        let d = [
            (x.k_i - y.k_i).abs() / a_one(),
            (x.k_ii - y.k_ii).abs() / a_two(),
            (x.k_iii - y.k_iii).abs() / a_three(),
        ];
        println!(
            "MW3 station {j}: half+mirror ({:.9e}, {:+.9e}, {:+.9e}), whole ({:.9e}, {:+.9e}, {:+.9e}), diff/A ({:.2e}, {:.2e}, \
             {:.2e})",
            x.k_i, x.k_ii, x.k_iii, y.k_i, y.k_ii, y.k_iii, d[0], d[1], d[2]
        );
        mw3 = mw3.max(d[0]).max(d[1]).max(d[2]);
    }
    c.holds(
        "MW3 the stations of the half are the first stations of the whole",
        positions,
        "",
    );
    c.at_most(
        "MW3 whole (psi = 90 deg) against the half with the mirror option, max |dK|/A",
        mw3,
        1e-6,
    );
    let mw4 = (vw[0].k_iii.abs()).max(vw[last].k_iii.abs()) / a_three();
    c.at_most("MW4 whole: |K_III|/A_III at theta = +/- pi/2", mw4, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ whole ellipse

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

/// ADR 0605's coarse ellipse spec (EF).
fn coarse_ellipse_spec() -> PennyCrack {
    PennyCrack::new(A, 12.0 * A, 10.0 * A, A / 8.0, 2, 4, 0.4 * A, 1, 0.65 * A, 1.5, 1.6)
}

/// The whole cylinder of `spec` mapped onto an elliptical crack of semi-axes `a` (x), `b` (y) by the map of
/// `elliptical_crack_block` (ADR 0605 D3).
fn whole_ellipse(spec: &PennyCrack) -> (SolidMesh, Vec<usize>) {
    let cyl = penny_crack_whole_cylinder(spec).unwrap();
    let r_t = spec.transition_radius;
    let c = (A * A - B * B).sqrt();
    let mu0 = (B / A).atanh();
    let mu_t = mu0 + (r_t / A).ln();
    let (at, bt) = (c * mu_t.cosh(), c * mu_t.sinh());
    let mapped: Vec<Vec3> = cyl
        .mesh
        .nodes()
        .iter()
        .map(|p| {
            let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
            if rho >= r_t * (1.0 - 1e-12) {
                let mu = mu0 + (rho / A).ln();
                [c * mu.cosh() * p[0] / rho, c * mu.sinh() * p[1] / rho, p[2]]
            } else {
                [p[0] * at / r_t, p[1] * bt / r_t, p[2]]
            }
        })
        .collect();
    (rebuild(&cyl.mesh, mapped), cyl.front)
}

fn ellipse_front(nodes: &[usize]) -> CrackFront {
    CrackFront::elliptical(nodes.to_vec(), [0.0, 0.0, 1.0], [0.0; 3], [1.0, 0.0, 0.0], A, B).closed()
}

#[test]
fn we_whole_ellipse_mode_one() {
    let mut c = Checks::new();
    let (mesh, nodes) = whole_ellipse(&coarse_ellipse_spec());
    let mut loads = traction_loads(&mesh, "top", [0.0, 0.0, SIGMA]).unwrap();
    loads.extend(traction_loads(&mesh, "bottom", [0.0, 0.0, -SIGMA]).unwrap());
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material()).unwrap();
    whole_supports(&mut model);
    println!("WE: {} unknowns", model.dof_map().n_equations());
    let u = solve(&model, &loads);
    let front = ellipse_front(&nodes);
    let v = interaction_integral_curved(&model, &u, &front, 2, CurvedTerms::Corrected).unwrap();
    let st = unwrapped(&v, A, B);
    let ds = |p: f64| (A * A * p.sin().powi(2) + B * B * p.cos().powi(2)).sqrt();
    let refs: Vec<f64> = (0..st.len())
        .map(|j| periodic_hat_average(&st, j, &irwin, &ds))
        .collect();
    let kmax = refs.iter().copied().fold(0.0, f64::max);
    let (mut we1, mut we2): (f64, f64) = (0.0, 0.0);
    for (j, x) in v.iter().enumerate() {
        println!(
            "WE station {j} (phi {:+.5}): K_I {:.6e}, Irwin {:.6e} (K/ref - 1 {:+.4e}), K_II {:+.3e}, K_III {:+.3e}",
            st[j],
            x.k_i,
            refs[j],
            x.k_i / refs[j] - 1.0,
            x.k_ii,
            x.k_iii
        );
        we1 = we1.max(rel(x.k_i, refs[j]));
        we2 = we2.max(x.k_ii.abs().max(x.k_iii.abs()) / kmax);
    }
    c.at_most(
        "WE1 whole ellipse ring 2, K_I against Irwin (periodic linear hat average)",
        we1,
        0.08,
    );
    c.at_most("WE2 whole ellipse, max(|K_II|, |K_III|)/max K_ref", we2, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------- weights and refusals

#[test]
fn cl_r_closed_weights_and_refusals() {
    let mut c = Checks::new();
    // CL1: the closed circle.
    let whole = penny_crack_whole_cylinder(&coarse_spec()).unwrap();
    let mut wm = SolidModel::new(whole.mesh.clone());
    wm.add_material("all", material()).unwrap();
    let zero = vec![0.0; wm.dof_map().n_equations()];
    let front = whole.crack_front();
    let jv = j_integral(&wm, &zero, &front, 2).unwrap();
    let sum: f64 = jv.iter().map(|x| x.weight).sum();
    let each = jv.iter().map(|x| rel(x.weight, A * PI / 8.0)).fold(0.0, f64::max);
    println!(
        "CL1: {} stations, sum of weights {sum:.15e} (2 pi a {:.15e})",
        jv.len(),
        2.0 * PI * A
    );
    c.holds("CL1 16 stations", jv.len() == 16, &format!("{}", jv.len()));
    c.at_most(
        "CL1 closed circle: |sum w/(2 pi a) - 1|, max |w_j/(a pi/8) - 1|",
        rel(sum, 2.0 * PI * A).max(each),
        1e-12,
    );
    // CL2: the closed ellipse.
    let (emesh, enodes) = whole_ellipse(&coarse_ellipse_spec());
    let mut em = SolidModel::new(emesh);
    em.add_material("all", material()).unwrap();
    let ezero = vec![0.0; em.dof_map().n_equations()];
    let ev = j_integral(&em, &ezero, &ellipse_front(&enodes), 2).unwrap();
    let esum: f64 = ev.iter().map(|x| x.weight).sum();
    let perimeter = 4.0 * A * elliptic_e(1.0 - (B / A).powi(2));
    println!("CL2: sum of weights {esum:.12e}, 4 a E(k) {perimeter:.12e}");
    c.at_most("CL2 closed ellipse: |sum w/(4 a E(k)) - 1|", rel(esum, perimeter), 1e-4);
    // R1: a straight front closed.
    let p = centre_crack_plate(&PlateCrack::new(A, 10.0 * A, 20.0 * A, 0.05 * A, A / 8.0, 1.5, 2)).unwrap();
    let mut pm = SolidModel::new(p.mesh.clone());
    pm.add_material("all", material()).unwrap();
    let pzero = vec![0.0; pm.dof_map().n_equations()];
    let r = j_integral(&pm, &pzero, &p.crack_front().closed(), 2);
    let (ok, detail) = refused(&r, "crack front", "closed");
    c.holds("R1 a straight front closed refused", ok, &detail);
    // R2: an odd number of nodes.
    let mut odd = whole.front.clone();
    odd.pop();
    let r = j_integral(
        &wm,
        &zero,
        &CrackFront::circular(odd, [0.0, 0.0, 1.0], [0.0; 3]).closed(),
        2,
    );
    let (ok, detail) = refused(&r, "crack front", "closed");
    c.holds("R2 a closed front of an odd number of nodes refused", ok, &detail);
    // R3: two corners swapped.
    let mut swapped = whole.front.clone();
    swapped.swap(2, 4);
    let r = j_integral(
        &wm,
        &zero,
        &CrackFront::circular(swapped, [0.0, 0.0, 1.0], [0.0; 3]).closed(),
        2,
    );
    let (ok, detail) = refused(&r, "crack front", "order");
    c.holds("R3 a closed front out of order refused", ok, &detail);
    // R4, R5: mirror planes of the half.
    let half = penny_crack_half_cylinder(&coarse_spec()).unwrap();
    let (hm, _) = half_mixed_model(&half);
    let hzero = vec![0.0; hm.dof_map().n_equations()];
    let r = interaction_integral_curved(
        &hm,
        &hzero,
        &half.crack_front().mirror_plane([0.0, 0.0, A], [0.0, 0.0, 1.0]),
        2,
        CurvedTerms::Corrected,
    );
    let (ok, detail) = refused(&r, "crack front", "mirror plane");
    c.holds("R4 a mirror plane without a station refused", ok, &detail);
    let s2 = 0.5f64.sqrt();
    let r = interaction_integral_curved(
        &hm,
        &hzero,
        &half.crack_front().mirror_plane([0.0, -A, 0.0], [s2, s2, 0.0]),
        2,
        CurvedTerms::Corrected,
    );
    let (ok, detail) = refused(&r, "crack front", "mirror plane");
    c.holds(
        "R5 a mirror plane not normal to the front refused",
        ok && detail.contains("not normal"),
        &detail,
    );
    // R6: a mirror plane on a closed front.
    let r = interaction_integral_curved(
        &wm,
        &zero,
        &front.clone().mirror_plane([0.0; 3], [1.0, 0.0, 0.0]),
        2,
        CurvedTerms::Corrected,
    );
    let (ok, detail) = refused(&r, "crack front", "mirror plane");
    c.holds("R6 a mirror plane on a closed front refused", ok, &detail);
    // R7: interaction_integral (ADR 0605) with a mirror plane.
    let r = interaction_integral(&pm, &pzero, &p.crack_front().mirror_plane([0.0; 3], [0.0, 0.0, 1.0]), 2);
    let (ok, detail) = refused(&r, "crack front", "mirror plane");
    c.holds("R7 interaction_integral with a mirror plane refused", ok, &detail);
    c.finish();
}
