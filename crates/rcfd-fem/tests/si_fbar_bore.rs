//! F-636 (ADR 0563): the bore-zone witness for the finite-strain growth of the B-bar creep cylinder of ADR 0549. The checks
//! A0, B1, B2, G, D, R1B, R4, R5 of the registration. The helpers below the first section are copied verbatim from
//! `si_fbar.rs` (a test binary cannot import another's functions; `si_fbar.rs` is not edited); the one change is that the
//! copied finite-strain reference also records the bore displacement `ua`. Every measure is required finite before it is
//! compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::nonlinear::{hex8_second_piola_kirchhoff, solve_in_time, NewtonOptions, TimeSteppingOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::f64::consts::PI;

const BBAR: SolidFormulation = SolidFormulation::BBar;

// ------------------------------------------------------------------------------------- copied from si_fbar.rs

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
        if !ok {
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

/// The largest of finite values; NaN if any is not finite (so that the check misses).
fn worst<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = 0.0f64;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    w
}

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

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

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

/// The element unknowns of `u` (one per equation; every node of a solid has three).
fn element_u(model: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    model
        .mesh()
        .element_nodes(e)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect()
}

fn newton(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps).threads(4)
}

const CYL_A: f64 = 1.0;
const CYL_B: f64 = 2.0;

/// The graded plane-strain quarter cylinder of ADR 0506 D6: `nr x nt` Hex8, one layer of 0.02.
fn cylinder(nr: usize, nt: usize, f: SolidFormulation, material: Material, p: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 0.02], [nr, nt, 1]).unwrap();
    let nodes = base
        .nodes()
        .iter()
        .map(|q| {
            let r = CYL_A + (CYL_B - CYL_A) * ((2.0 * q[0]).exp() - 1.0) / (2f64.exp() - 1.0);
            let th = 0.5 * PI * q[1];
            [r * th.cos(), r * th.sin(), q[2]]
        })
        .collect();
    let mut mesh = with_formulation(rebuild(&base, nodes), f);
    let nn = mesh.nodes().len();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", p)).unwrap();
    m
}

/// One point: radius, sigma_r, sigma_theta, sigma_z, and the state values of the point.
struct PolarPoint {
    r: f64,
    srr: f64,
    stt: f64,
    szz: f64,
    /// The von Mises stress of the point.
    q: f64,
    state: Vec<f64>,
}

fn polar_points(model: &SolidModel, f: SolidFormulation, u: &[f64], states: &[ElementState]) -> Vec<PolarPoint> {
    let mesh = model.mesh();
    let material = &model.materials()[0].1;
    let el = element_for(ElementKind::Hex8);
    let ns = material.state_size();
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        let coords = element_coords(mesh, e);
        let ue = element_u(model, u, e);
        let s = hex8_second_piola_kirchhoff(f, &coords, material, &ue, &states[e]).unwrap();
        let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
        for (q, (s, ip)) in s.iter().zip(&ips).enumerate() {
            let x = ip.geometry.x;
            let r = x[0].hypot(x[1]);
            let (cs, sn) = (x[0] / r, x[1] / r);
            out.push(PolarPoint {
                r,
                srr: cs * cs * s[0] + sn * sn * s[1] + 2.0 * cs * sn * s[3],
                stt: sn * sn * s[0] + cs * cs * s[1] - 2.0 * cs * sn * s[3],
                szz: s[2],
                q: {
                    let mean = (s[0] + s[1] + s[2]) / 3.0;
                    let (a, b, c) = (s[0] - mean, s[1] - mean, s[2] - mean);
                    (1.5 * (a * a + b * b + c * c + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]))).sqrt()
                },
                state: states[e].values()[q * ns..(q + 1) * ns].to_vec(),
            });
        }
    }
    out
}
const CR_P: f64 = 2e5;
const CR_N: f64 = 4.0;

fn creep_material(p: f64) -> Material {
    let base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    Material::creep(base, 1.0 / (2.0e9 * p.powi(3)), CR_N, 0.0).unwrap()
}

/// The times of ADR 0506 D6: the 40 of ADR 0489 to `5 t*`, then 30 of `0.5 t*` to `20 t*`.
fn creep_times() -> Vec<f64> {
    let dt0 = 5.0 * 0.05 / (1.05f64.powi(40) - 1.0);
    let mut ts: Vec<f64> = (1..40).map(|k| dt0 * (1.05f64.powi(k) - 1.0) / 0.05).collect();
    ts.push(5.0);
    ts.extend((1..=30).map(|k| 5.0 + 0.5 * k as f64));
    ts
}

/// The creep cylinder at bore pressure `p`: the model and its history.
fn creep_run(f: SolidFormulation, p: f64) -> (SolidModel, rcfd_fem::nonlinear::TimeHistory) {
    let m = cylinder(48, 24, f, creep_material(p), p);
    let opts = TimeSteppingOptions::new(5e-4).newton(newton(1));
    let h = solve_in_time(&m, &[], &creep_times(), &opts).unwrap();
    let halvings: usize = h.steps.iter().map(|s| s.halvings).sum();
    let largest = worst(h.steps.iter().map(|s| s.max_creep_increment));
    println!(
        "C {} p = {p:e}: {halvings} halvings, largest creep-strain increment {largest:.4e}, t_end {}",
        f.name(),
        h.steps.last().unwrap().time
    );
    assert_eq!(h.steps[40].time, 5.0);
    assert_eq!(h.steps[70].time, 20.0);
    (m, h)
}

/// The radial displacement of the bore nodes (r = a) of a solution, by node.
fn bore_radial(model: &SolidModel, u: &[f64]) -> std::collections::BTreeMap<usize, f64> {
    let mesh = model.mesh();
    let mut out = std::collections::BTreeMap::new();
    for e in 0..mesh.n_elements() {
        let ue = element_u(model, u, e);
        for (k, &n) in mesh.element_nodes(e).iter().enumerate() {
            let x = mesh.nodes()[n];
            let r = x[0].hypot(x[1]);
            if (r - CYL_A).abs() < 1e-9 {
                out.insert(n, (x[0] * ue[3 * k] + x[1] * ue[3 * k + 1]) / r);
            }
        }
    }
    out
}

const REF_E: f64 = 2.0e9;
const REF_NU: f64 = 0.3;

/// The creep law of ADR 0549 arithmetic 2, written from the law (not by calling `mat_creep`): the backward Euler radial
/// return of the normal components `(rr, theta theta, zz)` of the Green-Lagrange strain `e` from the creep strain `c`
/// (updated) over a step `dt`, `m = 0`, `n = CR_N`. Returns the second Piola-Kirchhoff stress.
fn ref_return(e: [f64; 3], c: &mut [f64; 3], a: f64, dt: f64) -> [f64; 3] {
    let g = REF_E / (2.0 * (1.0 + REF_NU));
    let lam = REF_E * REF_NU / ((1.0 + REF_NU) * (1.0 - 2.0 * REF_NU));
    let el: [f64; 3] = std::array::from_fn(|i| e[i] - c[i]);
    let tr = el[0] + el[1] + el[2];
    let sig: [f64; 3] = std::array::from_fn(|i| lam * tr + 2.0 * g * el[i]);
    let mean = (sig[0] + sig[1] + sig[2]) / 3.0;
    let s: [f64; 3] = std::array::from_fn(|i| sig[i] - mean);
    let qtr = (1.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2])).sqrt();
    if dt == 0.0 || qtr == 0.0 {
        return sig;
    }
    let k = 3.0 * g * a * dt;
    let mut q = qtr;
    let mut converged = false;
    for _ in 0..60 {
        let f = qtr - q - k * q.powf(CR_N);
        let df = -1.0 - k * CR_N * q.powf(CR_N - 1.0);
        let dq = -f / df;
        q += dq;
        if dq.abs() <= 1e-15 * qtr {
            converged = true;
            break;
        }
    }
    assert!(converged, "reference: local iteration not converged");
    let de = (qtr - q) / (3.0 * g);
    for (ci, si) in c.iter_mut().zip(&s) {
        *ci += de * 1.5 * si / qtr;
    }
    std::array::from_fn(|i| sig[i] - 3.0 * g * de * s[i] / qtr)
}

/// One element of the reference: the force `[f0, f1]`, the stress, the creep strain and the equivalent creep strain after
/// the step.
struct RefElement {
    f: [f64; 2],
    s: [f64; 3],
    c: [f64; 3],
    eq: f64,
}

/// The finite-strain reference of ADR 0549 arithmetic 2: `nel` graded linear elements in R, one point at the midpoint.
struct Reference {
    /// The midpoint radii.
    rm: Vec<f64>,
    /// The stress `(S_RR, S_ThTh, S_zz) / p` at the midpoints at every step end (`t = 0` and `creep_times`).
    steps: Vec<Vec<[f64; 3]>>,
    /// The largest equivalent creep strain at every step end.
    eq_max: Vec<f64>,
    /// F-636 (the one change of the copy): the radial displacement of the node at `R = a` at every step end.
    ua: Vec<f64>,
}

/// The reference with the Newton stop `max |du| <= tol max |u|`; `verbose` prints the ratio of every iteration of step 0.
fn reference_with(p: f64, nel: usize, tol: f64, verbose: bool) -> Reference {
    let a_law = 1.0 / (REF_E * p.powi(3));
    let r: Vec<f64> = (0..=nel)
        .map(|i| {
            let s = i as f64 / nel as f64;
            CYL_A + (CYL_B - CYL_A) * ((2.0 * s).exp() - 1.0) / (2f64.exp() - 1.0)
        })
        .collect();
    let h: Vec<f64> = (0..nel).map(|i| r[i + 1] - r[i]).collect();
    let rm: Vec<f64> = (0..nel).map(|i| 0.5 * (r[i] + r[i + 1])).collect();
    let elem = |i: usize, u0: f64, u1: f64, c: &[f64; 3], eq: f64, dt: f64| {
        let rp = 1.0 + (u1 - u0) / h[i];
        let rr = 1.0 + 0.5 * (u0 + u1) / rm[i];
        let mut cn = *c;
        let s = ref_return([0.5 * (rp * rp - 1.0), 0.5 * (rr * rr - 1.0), 0.0], &mut cn, a_law, dt);
        let (prr, ptt) = (rp * s[0], rr * s[1]);
        let w = rm[i] * h[i];
        let dev: [f64; 3] = std::array::from_fn(|k| cn[k] - c[k]);
        let de = (2.0 / 3.0 * (dev[0] * dev[0] + dev[1] * dev[1] + dev[2] * dev[2])).sqrt();
        RefElement {
            f: [
                w * (-prr / h[i] + 0.5 * ptt / rm[i]),
                w * (prr / h[i] + 0.5 * ptt / rm[i]),
            ],
            s,
            c: cn,
            eq: eq + de,
        }
    };
    let mut times = vec![0.0];
    times.extend(creep_times());
    let nn = nel + 1;
    let mut u = vec![0.0; nn];
    let mut c = vec![[0.0; 3]; nel];
    let mut eq = vec![0.0; nel];
    let mut out = Reference {
        rm: rm.clone(),
        steps: Vec::new(),
        eq_max: Vec::new(),
        ua: Vec::new(),
    };
    let hh = 1e-7 * p / REF_E;
    for k in 0..times.len() {
        let dt = if k == 0 { 0.0 } else { times[k] - times[k - 1] };
        let mut converged = false;
        for _ in 0..40 {
            let mut res = vec![0.0; nn];
            let (mut lo, mut dg, mut up) = (vec![0.0; nn], vec![0.0; nn], vec![0.0; nn]);
            for i in 0..nel {
                let f = elem(i, u[i], u[i + 1], &c[i], eq[i], dt).f;
                res[i] += f[0];
                res[i + 1] += f[1];
                let fp = elem(i, u[i] + hh, u[i + 1], &c[i], eq[i], dt).f;
                let fm = elem(i, u[i] - hh, u[i + 1], &c[i], eq[i], dt).f;
                dg[i] += (fp[0] - fm[0]) / (2.0 * hh);
                lo[i + 1] += (fp[1] - fm[1]) / (2.0 * hh);
                let fp = elem(i, u[i], u[i + 1] + hh, &c[i], eq[i], dt).f;
                let fm = elem(i, u[i], u[i + 1] - hh, &c[i], eq[i], dt).f;
                up[i] += (fp[0] - fm[0]) / (2.0 * hh);
                dg[i + 1] += (fp[1] - fm[1]) / (2.0 * hh);
            }
            res[0] -= p * CYL_A;
            // The Thomas algorithm for J du = -res.
            let mut cp = vec![0.0; nn];
            let mut dp = vec![0.0; nn];
            cp[0] = up[0] / dg[0];
            dp[0] = -res[0] / dg[0];
            for i in 1..nn {
                let m = dg[i] - lo[i] * cp[i - 1];
                cp[i] = if i < nel { up[i] / m } else { 0.0 };
                dp[i] = (-res[i] - lo[i] * dp[i - 1]) / m;
            }
            let mut du = vec![0.0; nn];
            du[nel] = dp[nel];
            for i in (0..nel).rev() {
                du[i] = dp[i] - cp[i] * du[i + 1];
            }
            for (ui, d) in u.iter_mut().zip(&du) {
                *ui += d;
            }
            if verbose && k == 0 {
                println!(
                    "reference p = {p:e} N = {nel} step 0: max |du| / max |u| = {:.3e}",
                    max_abs(&du) / max_abs(&u)
                );
            }
            if max_abs(&du) <= tol * max_abs(&u) {
                converged = true;
                break;
            }
        }
        assert!(converged, "reference: Newton not converged at step {k}");
        let mut stress = Vec::with_capacity(nel);
        for i in 0..nel {
            let el = elem(i, u[i], u[i + 1], &c[i], eq[i], dt);
            c[i] = el.c;
            eq[i] = el.eq;
            stress.push([el.s[0] / p, el.s[1] / p, el.s[2] / p]);
        }
        out.steps.push(stress);
        out.eq_max.push(worst(eq.iter().copied()));
        out.ua.push(u[0]);
    }
    out
}

/// The reference stress over `p` at the reference radius `radius` at step `k` (linear between midpoints, clamped).
fn ref_at(r: &Reference, k: usize, radius: f64) -> [f64; 3] {
    let rm = &r.rm;
    let s = &r.steps[k];
    let last = rm.len() - 1;
    if radius <= rm[0] {
        return s[0];
    }
    if radius >= rm[last] {
        return s[last];
    }
    let j = rm.partition_point(|&x| x <= radius) - 1;
    let w = (radius - rm[j]) / (rm[j + 1] - rm[j]);
    std::array::from_fn(|i| (1.0 - w) * s[j][i] + w * s[j + 1][i])
}

/// The difference `(sigma_r, sigma_theta) / p - reference` at every point.
fn ref_differences(pts: &[PolarPoint], r: &Reference, k: usize, p: f64) -> Vec<[f64; 2]> {
    pts.iter()
        .map(|pt| {
            let s = ref_at(r, k, pt.r);
            [pt.srr / p - s[0], pt.stt / p - s[1]]
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ F-636 (ADR 0563)

/// The bore zone of ADR 0563: the points with reference radius below `1.1 a`.
const BORE_ZONE: f64 = 1.1 * CYL_A;

/// `e_B`: the point-wise error over `p` against the reference at the bore-zone points, and the radius of the worst point.
fn bore_error(pts: &[PolarPoint], r: &Reference, k: usize, p: f64) -> (f64, f64) {
    let d = ref_differences(pts, r, k, p);
    let mut wr = f64::NAN;
    let mut wv = -1.0;
    let mut values = Vec::new();
    for (pt, dk) in pts.iter().zip(&d) {
        if pt.r < BORE_ZONE {
            let v = dk[0].abs().max(dk[1].abs());
            values.push(v);
            if v > wv {
                wv = v;
                wr = pt.r;
            }
        }
    }
    (worst(values), wr)
}

/// The mean and the spread (largest minus smallest) of the radial displacement of the bore nodes.
fn bore_mean(model: &SolidModel, u: &[f64]) -> (f64, f64) {
    let b = bore_radial(model, u);
    let mean = b.values().sum::<f64>() / b.len() as f64;
    let lo = b.values().copied().fold(f64::INFINITY, f64::min);
    let hi = b.values().copied().fold(f64::NEG_INFINITY, f64::max);
    (mean, hi - lo)
}

/// A0, B1, B2, G, D, R1B, R4, R5 of ADR 0563 (the rest reported).
#[test]
fn f636_bore_zone_witness_of_the_b_bar_creep_cylinder() {
    let p = CR_P;
    let q = CR_P / 100.0;
    let mut ch = Checks::new();
    let (mp, hp) = creep_run(BBAR, p);
    let (mq, hq) = creep_run(BBAR, q);
    let reference = |x: f64, nel: usize| reference_with(x, nel, 1e-9, false);
    let (rp4, rp8) = (reference(p, 400), reference(p, 800));
    let (rq4, rq8) = (reference(q, 400), reference(q, 800));
    println!(
        "reference largest equivalent creep strain at 20 t*: p {:.4e}, q {:.4e}",
        rp4.eq_max[70], rq4.eq_max[70]
    );
    let pts = |i: usize| polar_points(&mp, BBAR, &hp.steps[i].u, &hp.steps[i].states);
    let (b5, b20) = (pts(40), pts(70));
    // A0
    let nb = b5.iter().filter(|pt| pt.r < BORE_ZONE).count();
    ch.holds(
        "A0 both BBar runs to 20 t*, bore-zone points",
        hp.steps.len() == 71 && hq.steps.len() == 71 && nb >= 16,
        &format!(
            "{} and {} step ends, {nb} points with r < 1.1 a",
            hp.steps.len(),
            hq.steps.len()
        ),
    );
    let bore_pts = b20.iter().filter(|pt| pt.r < BORE_ZONE);
    println!(
        "bore zone at 20 t* (reported): largest equivalent creep strain {:.4e}, largest von Mises / p {:.4e}, sigma_z / p in [{:.4e}, {:.4e}]",
        worst(bore_pts.clone().map(|pt| pt.state[6])),
        worst(bore_pts.clone().map(|pt| pt.q / p)),
        bore_pts.clone().map(|pt| pt.szz / p).fold(f64::INFINITY, f64::min),
        bore_pts.map(|pt| pt.szz / p).fold(f64::NEG_INFINITY, f64::max)
    );
    // B1, B2, G
    let (e5, w5) = bore_error(&b5, &rp4, 40, p);
    let (e20, w20) = bore_error(&b20, &rp4, 70, p);
    for i in [50, 60] {
        let (e, w) = bore_error(&pts(i), &rp4, i, p);
        println!("e_B at t = {}: {e:.4e} at r = {w:.4} (reported)", hp.steps[i].time);
    }
    println!("e_B worst points: 5 t* r = {w5:.4}, 20 t* r = {w20:.4}");
    ch.at_most("B1 e_B(5 t*) against the finite-strain reference", e5, 2e-2);
    ch.at_most("B2 e_B(20 t*)", e20, 2e-2);
    ch.at_most("G |e_B(20 t*) - e_B(5 t*)|", (e20 - e5).abs(), 6e-3);
    let d5 = ref_differences(&b5, &rp4, 40, p);
    let d20 = ref_differences(&b20, &rp4, 70, p);
    let (mut f4, mut f4r) = (0.0, f64::NAN);
    for ((a, b), pt) in d20.iter().zip(&d5).zip(&b20) {
        let v = (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
        if pt.r < BORE_ZONE && v > f4 {
            f4 = v;
            f4r = pt.r;
        }
    }
    println!("bore-zone max |d(20 t*) - d(5 t*)| (reported): {f4:.4e} at r = {f4r:.4}");
    // R1B
    let mut r1 = Vec::new();
    for (k, pts) in [(40usize, &b5), (70, &b20)] {
        for pt in pts.iter().filter(|pt| pt.r < BORE_ZONE) {
            let (x, y) = (ref_at(&rp4, k, pt.r), ref_at(&rp8, k, pt.r));
            r1.push((x[0] - y[0]).abs());
            r1.push((x[1] - y[1]).abs());
        }
    }
    ch.at_most(
        "R1B reference N = 400 against N = 800 at the bore-zone points",
        worst(r1),
        1e-4,
    );
    // D
    let ua = |m: &SolidModel, h: &rcfd_fem::nonlinear::TimeHistory, i: usize| {
        let (mean, spread) = bore_mean(m, &h.steps[i].u);
        println!(
            "FE bore displacement at t = {}: mean {mean:.6e}, spread {spread:.3e} (reported)",
            h.steps[i].time
        );
        mean
    };
    let du_fe_p = ua(&mp, &hp, 70) - ua(&mp, &hp, 40);
    let du_fe_q = ua(&mq, &hq, 70) - ua(&mq, &hq, 40);
    let du_ref = |r: &Reference| r.ua[70] - r.ua[40];
    let delta_fe = du_fe_p / p - du_fe_q / q;
    let delta_ref = du_ref(&rp4) / p - du_ref(&rq4) / q;
    let delta_ref8 = du_ref(&rp8) / p - du_ref(&rq8) / q;
    println!(
        "Du / x (reported): FE p {:.6e}, FE q {:.6e}, reference p {:.6e}, reference q {:.6e}; FE against reference: p {:+.3e}, q {:+.3e}",
        du_fe_p / p,
        du_fe_q / q,
        du_ref(&rp4) / p,
        du_ref(&rq4) / q,
        du_fe_p / du_ref(&rp4) - 1.0,
        du_fe_q / du_ref(&rq4) - 1.0
    );
    let d20_fe = ua(&mp, &hp, 70) / p - ua(&mq, &hq, 70) / q;
    let d20_ref = rp4.ua[70] / p - rq4.ua[70] / q;
    println!(
        "delta (5 t* to 20 t*): FE {delta_fe:.6e}, reference {delta_ref:.6e}; drift from t = 0 at 20 t*: FE {d20_fe:.6e}, reference {d20_ref:.6e} (reported)"
    );
    ch.at_most(
        "D |delta_FE / delta_ref - 1| (finite-strain drift of the bore displacement)",
        (delta_fe / delta_ref - 1.0).abs(),
        5e-2,
    );
    // R4, R5
    ch.at_most(
        "R4 |delta_ref(400) / delta_ref(800) - 1|",
        (delta_ref / delta_ref8 - 1.0).abs(),
        1e-2,
    );
    ch.at_least(
        "R5 |delta_ref| / |Du_ref(q) / q| (the witness measures finite strain)",
        (delta_ref / (du_ref(&rq4) / q)).abs(),
        1e-3,
    );
    ch.finish();
}
