//! F-637 (ADR 0568): the pressure drift of the F-bar Hex8 under creep at finite strain, the growth of ADR 0549's F4. The
//! checks L0, K1 to K4, X1 to X7, J1 to J4, R0, R1 of the registration: its scaling with p/E (the library at p, p/10, p/100
//! and the pair of ADR 0563), its presence in a one-dimensional F-bar proxy, and a J-based volumetric part of the law in the
//! proxy (written here; nothing in the library changes). The helpers of the first section are copied verbatim from
//! `si_fbar_bore.rs`, which copied them from `si_fbar.rs` (a test binary cannot import another's functions; neither file is
//! edited); the ones not used here are left out. Every measure is required finite before it is compared.

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

// ------------------------------------------------------------------------------------------------ F-637 (ADR 0568)

/// The volumetric part of the law in the proxy (ADR 0568 D2).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Law {
    /// The library's law: `kappa tr(E - eps_c)` (the copied `ref_return`).
    Library,
    /// The `J`-based variant: `kappa ln J C^-1` and the library's deviatoric return.
    JBased,
}

/// The `J`-based law of ADR 0568 D2 on the normal components `(rr, theta theta, zz)` of the Green-Lagrange strain `e` from
/// the creep strain `c` (updated): `S = kappa ln J C^-1 + s`, `s` the radial return of `2 G (dev E - c)`.
fn j_return(e: [f64; 3], c: &mut [f64; 3], a: f64, dt: f64) -> [f64; 3] {
    let g = REF_E / (2.0 * (1.0 + REF_NU));
    let kappa = REF_E / (3.0 * (1.0 - 2.0 * REF_NU));
    let lam2: [f64; 3] = std::array::from_fn(|i| 1.0 + 2.0 * e[i]);
    let ln_j = 0.5 * (lam2[0] * lam2[1] * lam2[2]).ln();
    let vol: [f64; 3] = std::array::from_fn(|i| kappa * ln_j / lam2[i]);
    let tr = e[0] + e[1] + e[2];
    let s: [f64; 3] = std::array::from_fn(|i| 2.0 * g * (e[i] - tr / 3.0 - c[i]));
    let qtr = (1.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2])).sqrt();
    if dt == 0.0 || qtr == 0.0 {
        return std::array::from_fn(|i| vol[i] + s[i]);
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
    assert!(converged, "J law: local iteration not converged");
    let de = (qtr - q) / (3.0 * g);
    for (ci, si) in c.iter_mut().zip(&s) {
        *ci += de * 1.5 * si / qtr;
    }
    std::array::from_fn(|i| vol[i] + s[i] - 3.0 * g * de * s[i] / qtr)
}

/// The Newton stop of the references and proxies of ADR 0568 (arithmetic 4).
const STOP: f64 = 1e-10;

/// The proxy of ADR 0568 D3: the stress over `p` at the points of every step end (as a `Reference`, its radii the points')
/// and `kappa (Q - Q_mean)/p` per point (`Q = tr E_bar - ln J_bar`).
struct Proxy {
    stress: Reference,
    mismatch: Vec<Vec<f64>>,
}

/// One element of the proxy at `(u0, u1)`: the force, and per point the stress, the creep strain after the step and `Q`.
struct ProxyElement {
    f: [f64; 2],
    s: Vec<[f64; 3]>,
    c: Vec<[f64; 3]>,
    q: Vec<f64>,
}

/// The one-dimensional proxy of ADR 0568 D3: `nel` graded linear elements, `points` (1: the midpoint, 2: Gauss) per element,
/// the variational F-bar if `fbar`, the law `law`, at the bore pressure `p`.
fn proxy(p: f64, nel: usize, points: usize, fbar: bool, law: Law) -> Proxy {
    let a_law = 1.0 / (REF_E * p.powi(3));
    let kappa = REF_E / (3.0 * (1.0 - 2.0 * REF_NU));
    let r: Vec<f64> = (0..=nel)
        .map(|i| {
            let s = i as f64 / nel as f64;
            CYL_A + (CYL_B - CYL_A) * ((2.0 * s).exp() - 1.0) / (2f64.exp() - 1.0)
        })
        .collect();
    let h: Vec<f64> = (0..nel).map(|i| r[i + 1] - r[i]).collect();
    let (xi, wq): (Vec<f64>, Vec<f64>) = if points == 1 {
        (vec![0.0], vec![2.0])
    } else {
        let g = 1.0 / 3f64.sqrt();
        (vec![-g, g], vec![1.0, 1.0])
    };
    let np = xi.len();
    let shape: Vec<[f64; 2]> = xi.iter().map(|x| [(1.0 - x) / 2.0, (1.0 + x) / 2.0]).collect();
    let rg: Vec<Vec<f64>> = (0..nel)
        .map(|i| shape.iter().map(|n| n[0] * r[i] + n[1] * r[i + 1]).collect())
        .collect();
    let wt: Vec<Vec<f64>> = (0..nel)
        .map(|i| (0..np).map(|k| rg[i][k] * h[i] / 2.0 * wq[k]).collect())
        .collect();
    let elem = |i: usize, u0: f64, u1: f64, c: &[[f64; 3]], dt: f64| {
        // The stretches and d ln(lambda)/du_j at each point.
        let mut lam = vec![[0.0; 3]; np];
        let mut dl = vec![[[0.0; 2]; 3]; np];
        for k in 0..np {
            let lr = 1.0 + (u1 - u0) / h[i];
            let lt = 1.0 + (shape[k][0] * u0 + shape[k][1] * u1) / rg[i][k];
            lam[k] = [lr, lt, 1.0];
            dl[k][0] = [-1.0 / h[i] / lr, 1.0 / h[i] / lr];
            dl[k][1] = [shape[k][0] / rg[i][k] / lt, shape[k][1] / rg[i][k] / lt];
        }
        let jac: Vec<f64> = lam.iter().map(|l| l[0] * l[1] * l[2]).collect();
        let wsum: f64 = wt[i].iter().sum();
        let jbar = (0..np).map(|k| wt[i][k] * jac[k]).sum::<f64>() / wsum;
        if fbar {
            let dlnj: Vec<[f64; 2]> = (0..np)
                .map(|k| std::array::from_fn(|j| dl[k][0][j] + dl[k][1][j] + dl[k][2][j]))
                .collect();
            let dlnjbar: [f64; 2] =
                std::array::from_fn(|j| (0..np).map(|k| wt[i][k] * jac[k] * dlnj[k][j]).sum::<f64>() / wsum / jbar);
            for k in 0..np {
                let al = (jbar / jac[k]).cbrt();
                for m in 0..3 {
                    lam[k][m] *= al;
                    for j in 0..2 {
                        dl[k][m][j] += (dlnjbar[j] - dlnj[k][j]) / 3.0;
                    }
                }
            }
        }
        let mut f = [0.0; 2];
        let mut s_out = Vec::with_capacity(np);
        let mut c_out = Vec::with_capacity(np);
        let mut q_out = Vec::with_capacity(np);
        for k in 0..np {
            let e: [f64; 3] = std::array::from_fn(|m| 0.5 * (lam[k][m] * lam[k][m] - 1.0));
            let mut cn = c[k];
            let s = match law {
                Law::JBased => j_return(e, &mut cn, a_law, dt),
                Law::Library => ref_return(e, &mut cn, a_law, dt),
            };
            for (j, fj) in f.iter_mut().enumerate() {
                *fj += wt[i][k] * (0..3).map(|m| lam[k][m] * lam[k][m] * dl[k][m][j] * s[m]).sum::<f64>();
            }
            let jb = lam[k][0] * lam[k][1] * lam[k][2];
            q_out.push(e[0] + e[1] + e[2] - jb.ln());
            s_out.push(s);
            c_out.push(cn);
        }
        ProxyElement {
            f,
            s: s_out,
            c: c_out,
            q: q_out,
        }
    };
    let mut times = vec![0.0];
    times.extend(creep_times());
    let nn = nel + 1;
    let mut u = vec![0.0; nn];
    let mut c = vec![vec![[0.0; 3]; np]; nel];
    let mut out = Proxy {
        stress: Reference {
            rm: rg.iter().flatten().copied().collect(),
            steps: Vec::new(),
            eq_max: Vec::new(),
            ua: Vec::new(),
        },
        mismatch: Vec::new(),
    };
    let hh = 1e-7 * p / REF_E;
    for k in 0..times.len() {
        let dt = if k == 0 { 0.0 } else { times[k] - times[k - 1] };
        let mut converged = false;
        for _ in 0..40 {
            let mut res = vec![0.0; nn];
            let (mut lo, mut dg, mut up) = (vec![0.0; nn], vec![0.0; nn], vec![0.0; nn]);
            for i in 0..nel {
                let f = elem(i, u[i], u[i + 1], &c[i], dt).f;
                res[i] += f[0];
                res[i + 1] += f[1];
                let fp = elem(i, u[i] + hh, u[i + 1], &c[i], dt).f;
                let fm = elem(i, u[i] - hh, u[i + 1], &c[i], dt).f;
                dg[i] += (fp[0] - fm[0]) / (2.0 * hh);
                lo[i + 1] += (fp[1] - fm[1]) / (2.0 * hh);
                let fp = elem(i, u[i], u[i + 1] + hh, &c[i], dt).f;
                let fm = elem(i, u[i], u[i + 1] - hh, &c[i], dt).f;
                up[i] += (fp[0] - fm[0]) / (2.0 * hh);
                dg[i + 1] += (fp[1] - fm[1]) / (2.0 * hh);
            }
            res[0] -= p * CYL_A;
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
            if max_abs(&du) <= STOP * max_abs(&u) {
                converged = true;
                break;
            }
        }
        assert!(
            converged,
            "proxy: Newton not converged at step {k} (p = {p:e}, N = {nel})"
        );
        let mut stress = Vec::with_capacity(nel * np);
        let mut mismatch = Vec::with_capacity(nel * np);
        for i in 0..nel {
            let el = elem(i, u[i], u[i + 1], &c[i], dt);
            c[i] = el.c;
            let qm = (0..np).map(|k| wt[i][k] * el.q[k]).sum::<f64>() / wt[i].iter().sum::<f64>();
            for kk in 0..np {
                stress.push([el.s[kk][0] / p, el.s[kk][1] / p, el.s[kk][2] / p]);
                mismatch.push(kappa * (el.q[kk] - qm) / p);
            }
        }
        out.stress.steps.push(stress);
        out.mismatch.push(mismatch);
        out.stress.ua.push(u[0]);
    }
    out
}

/// The growth `d_70 - d_40` per point of the proxy `x` against the reference `r` (both over their own `p`).
fn proxy_growth(x: &Reference, r: &Reference) -> Vec<[f64; 2]> {
    x.rm.iter()
        .enumerate()
        .map(|(i, &rad)| {
            let g: [f64; 2] = std::array::from_fn(|c| {
                let d70 = x.steps[70][i][c] - ref_at(r, 70, rad)[c];
                let d40 = x.steps[40][i][c] - ref_at(r, 40, rad)[c];
                d70 - d40
            });
            g
        })
        .collect()
}

/// The growth `d_70 - d_40` per point of a library run against the reference `r`.
fn library_growth(
    model: &SolidModel,
    h: &rcfd_fem::nonlinear::TimeHistory,
    r: &Reference,
    p: f64,
) -> (Vec<[f64; 2]>, Vec<f64>) {
    let pts = |i: usize| polar_points(model, BBAR, &h.steps[i].u, &h.steps[i].states);
    let (b40, b70) = (pts(40), pts(70));
    let d40 = ref_differences(&b40, r, 40, p);
    let d70 = ref_differences(&b70, r, 70, p);
    let g = d70.iter().zip(&d40).map(|(a, b)| [a[0] - b[0], a[1] - b[1]]).collect();
    let rad = b70.iter().map(|pt| pt.r).collect();
    let pr = b70
        .iter()
        .map(|pt| (pt.state[6], pt.q / p, pt.szz / p))
        .fold((0.0f64, 0.0f64, f64::INFINITY), |acc, v| {
            (acc.0.max(v.0), acc.1.max(v.1), acc.2.min(v.2))
        });
    println!(
        "library p = {p:e} at 20 t* (reported): largest equivalent creep strain {:.4e}, largest von Mises / p {:.4e}, smallest sigma_z / p {:.4e}",
        pr.0, pr.1, pr.2
    );
    (g, rad)
}

/// The largest `|g|_inf` over the points, the radius of its point and its signed components (NaN if any is not finite).
fn largest(g: &[[f64; 2]], radii: &[f64]) -> (f64, f64, [f64; 2]) {
    let mut best = (-1.0, f64::NAN, [f64::NAN; 2]);
    for (v, &r) in g.iter().zip(radii) {
        let m = v[0].abs().max(v[1].abs());
        if !m.is_finite() {
            return (f64::NAN, f64::NAN, [f64::NAN; 2]);
        }
        if m > best.0 {
            best = (m, r, *v);
        }
    }
    best
}

/// The pair of ADR 0563 on growths: `g(x) - g(q)` per point.
fn pair(gx: &[[f64; 2]], gq: &[[f64; 2]]) -> Vec<[f64; 2]> {
    gx.iter().zip(gq).map(|(a, b)| [a[0] - b[0], a[1] - b[1]]).collect()
}

/// The largest growth of the mismatch between steps 40 and 70 and the radius of its point.
fn mismatch_growth(x: &Proxy) -> (f64, f64) {
    let g: Vec<[f64; 2]> = x.mismatch[70]
        .iter()
        .zip(&x.mismatch[40])
        .map(|(a, b)| [a - b, 0.0])
        .collect();
    let (m, r, _) = largest(&g, &x.stress.rm);
    (m, r)
}

/// The largest difference of two references over steps `ks` at the radii of the first.
fn ref_gap(a: &Reference, b: &Reference, ks: &[usize]) -> f64 {
    let mut v = Vec::new();
    for &k in ks {
        for (i, &rad) in a.rm.iter().enumerate() {
            let y = ref_at(b, k, rad);
            for c in 0..3 {
                v.push((a.steps[k][i][c] - y[c]).abs());
            }
        }
    }
    worst(v)
}

/// L0, K1 to K4, X1 to X7, J1 to J4, R0, R1 of ADR 0568 (U1, S0, S1 are checked outside the test).
#[test]
fn f637_pressure_drift_of_the_f_bar_hex8_under_creep() {
    let p = CR_P;
    let p10 = CR_P / 10.0;
    let q = CR_P / 100.0;
    let mut ch = Checks::new();
    // L0
    let (mp, hp) = creep_run(BBAR, p);
    let (m10, h10) = creep_run(BBAR, p10);
    let (mq, hq) = creep_run(BBAR, q);
    let (rp, rp8) = (reference_with(p, 400, STOP, false), reference_with(p, 800, STOP, false));
    let r10 = reference_with(p10, 400, STOP, false);
    let rq = reference_with(q, 400, STOP, false);
    println!(
        "reference largest equivalent creep strain at 20 t*: p {:.4e}, p/10 {:.4e}, p/100 {:.4e} (reported)",
        rp.eq_max[70], r10.eq_max[70], rq.eq_max[70]
    );
    let (gp, rad) = library_growth(&mp, &hp, &rp, p);
    let (g10, _) = library_growth(&m10, &h10, &r10, p10);
    let (gq, _) = library_growth(&mq, &hq, &rq, q);
    let f4p = largest(&gp, &rad);
    let f410 = largest(&g10, &rad);
    let f4q = largest(&gq, &rad);
    println!(
        "library F4: p {:.4e} at r = {:.4} ({:+.4e}, {:+.4e}); p/10 {:.4e} at r = {:.4} ({:+.4e}, {:+.4e}); p/100 {:.4e} at r = {:.4} ({:+.4e}, {:+.4e}) (reported)",
        f4p.0, f4p.1, f4p.2[0], f4p.2[1], f410.0, f410.1, f410.2[0], f410.2[1], f4q.0, f4q.1, f4q.2[0], f4q.2[1]
    );
    ch.holds(
        "L0 the three BBar runs to 20 t*",
        hp.steps.len() == 71 && h10.steps.len() == 71 && hq.steps.len() == 71,
        &format!("{}, {}, {} step ends", hp.steps.len(), h10.steps.len(), hq.steps.len()),
    );
    ch.at_most(
        "L0 |F4_lib(p) / 4.396e-3 - 1| (ADR 0549's bits)",
        (f4p.0 / 4.396e-3 - 1.0).abs(),
        2e-4,
    );
    // K1 to K4
    let gfs_p = largest(&pair(&gp, &gq), &rad);
    let gfs_10 = largest(&pair(&g10, &gq), &rad);
    println!(
        "library Gfs: p {:.4e} at r = {:.4}; p/10 {:.4e} at r = {:.4} (reported)",
        gfs_p.0, gfs_p.1, gfs_10.0, gfs_10.1
    );
    let rho = gfs_10.0 / gfs_p.0;
    println!("rho = {rho:.6} (linear in p/E: 0.090909)");
    ch.at_most("K1 |rho / 0.090909 - 1|", (rho / (0.09 / 0.99) - 1.0).abs(), 5e-2);
    ch.at_most(
        "K2 |Gfs_lib(p) / (0.99 F4_lib(p)) - 1|",
        (gfs_p.0 / (0.99 * f4p.0) - 1.0).abs(),
        5e-2,
    );
    ch.at_most("K3 F4_lib(p/100)", f4q.0, 2e-4);
    ch.holds(
        "K4 the worst point of Gfs_lib(p) in [1.04, 1.07]",
        (1.04..=1.07).contains(&gfs_p.1),
        &format!("r = {:.4}", gfs_p.1),
    );
    // X1 to X4, X6, J1 to J3
    let fb = |x: f64, nel: usize, law: Law| proxy(x, nel, 2, true, law);
    let xp = fb(p, 48, Law::Library);
    let xq = fb(q, 48, Law::Library);
    let gp = proxy_growth(&xp.stress, &rp);
    let gq = proxy_growth(&xq.stress, &rq);
    let f4 = largest(&gp, &xp.stress.rm);
    println!(
        "proxy F4(p) {:.4e} at r = {:.4}, components ({:+.4e}, {:+.4e}) (reported)",
        f4.0, f4.1, f4.2[0], f4.2[1]
    );
    let g_fs = largest(&pair(&gp, &gq), &xp.stress.rm);
    println!("proxy Gfs(p) {:.4e} at r = {:.4} (reported)", g_fs.0, g_fs.1);
    ch.at_most("X1 |F4_proxy(p) / F4_lib(p) - 1|", (f4.0 / f4p.0 - 1.0).abs(), 5e-2);
    ch.at_most("X2 |r(F4_proxy) - r(F4_lib)|", (f4.1 - f4p.1).abs(), 5e-3);
    ch.at_most(
        "X3 |Gfs_proxy(p) / Gfs_lib(p) - 1|",
        (g_fs.0 / gfs_p.0 - 1.0).abs(),
        5e-2,
    );
    let (m, mr) = mismatch_growth(&xp);
    println!("proxy M(p) {m:.4e} at r = {mr:.4} (reported)");
    ch.at_most("X4 |F4_proxy(p) / M(p) - 1|", (f4.0 / m - 1.0).abs(), 5e-2);
    ch.at_most("X4 |r(F4_proxy) - r(M)|", (f4.1 - mr).abs(), 5e-3);
    let x96 = fb(p, 96, Law::Library);
    let f96 = largest(&proxy_growth(&x96.stress, &rp), &x96.stress.rm);
    println!(
        "proxy F4(p) on 96 elements {:.4e} at r = {:.4} (reported)",
        f96.0, f96.1
    );
    let ratio = f96.0 / f4.0;
    ch.holds(
        "X6 F4_proxy(96) / F4_proxy(48) in [0.45, 0.55]",
        ratio.is_finite() && (0.45..=0.55).contains(&ratio),
        &format!("{ratio:.5}"),
    );
    // J1, J2
    let jp = proxy(p, 400, 1, false, Law::JBased);
    let jq = proxy(q, 400, 1, false, Law::JBased);
    let yp = fb(p, 48, Law::JBased);
    let yq = fb(q, 48, Law::JBased);
    let jgp = proxy_growth(&yp.stress, &jp.stress);
    let jgq = proxy_growth(&yq.stress, &jq.stress);
    let f4j = largest(&jgp, &yp.stress.rm);
    println!(
        "J law F4_J(p) {:.4e} at r = {:.4}, components ({:+.4e}, {:+.4e}) (reported)",
        f4j.0, f4j.1, f4j.2[0], f4j.2[1]
    );
    ch.at_most("J1 F4_J(p), F-bar proxy against the J reference", f4j.0, 3e-4);
    let gj = largest(&pair(&jgp, &jgq), &yp.stress.rm);
    println!("J law Gfs_J(p) at r = {:.4} (reported)", gj.1);
    ch.at_most("J2 Gfs_J(p)", gj.0, 3e-4);
    let fq = largest(&jgq, &yq.stress.rm);
    println!("J law F4_J(p/100) {:.4e} at r = {:.4} (reported)", fq.0, fq.1);
    let (mj, mjr) = mismatch_growth(&yp);
    println!("J law M_J(p) at r = {mjr:.4} (reported)");
    ch.at_least("J3 M_J(p) (the F-bar mismatch is still in the kinematics)", mj, 3e-3);
    // X5
    let x1 = proxy(p, 48, 1, false, Law::Library);
    let f1 = largest(&proxy_growth(&x1.stress, &rp), &x1.stress.rm);
    println!("one midpoint, no F-bar: at r = {:.4} (reported)", f1.1);
    ch.at_most("X5 F4 of the one-midpoint proxy without F-bar", f1.0, 1e-4);
    // X7
    let x10 = fb(p10, 48, Law::Library);
    let f10 = largest(&proxy_growth(&x10.stress, &r10), &x10.stress.rm);
    let ratio = f10.0 / f4.0;
    println!("proxy F4(p/10) {:.4e} at r = {:.4} (reported)", f10.0, f10.1);
    ch.holds(
        "X7 F4_proxy(p/10) / F4_proxy(p) in [0.095, 0.105]",
        ratio.is_finite() && (0.095..=0.105).contains(&ratio),
        &format!("{ratio:.5}"),
    );
    // J4
    ch.at_most(
        "J4 the J reference against the library-law reference at p/100",
        ref_gap(&jq.stress, &rq, &[0, 40, 70]),
        1e-5,
    );
    println!(
        "J reference against the library-law reference at p, 20 t* (reported): {:.4e}",
        ref_gap(&jp.stress, &rp, &[70])
    );
    // R0
    let x400 = proxy(p, 400, 1, false, Law::Library);
    ch.at_most(
        "R0 the one-midpoint proxy against the copied reference",
        ref_gap(&x400.stress, &rp, &[0, 40, 70]),
        1e-6,
    );
    println!(
        "bore displacement at 20 t* (reported): reference {:.6e}, one-midpoint proxy {:.6e}",
        rp.ua[70], x400.stress.ua[70]
    );
    // R1
    let pts = |i: usize| polar_points(&mp, BBAR, &hp.steps[i].u, &hp.steps[i].states);
    let mut r1 = Vec::new();
    for k in [40usize, 70] {
        for pt in pts(k) {
            let (x, y) = (ref_at(&rp, k, pt.r), ref_at(&rp8, k, pt.r));
            r1.push((x[0] - y[0]).abs());
            r1.push((x[1] - y[1]).abs());
        }
    }
    ch.at_most(
        "R1 reference N = 400 against N = 800 at the library's points",
        worst(r1),
        1e-4,
    );
    let jp8 = proxy(p, 800, 1, false, Law::JBased);
    let mut r1j = Vec::new();
    for k in [40usize, 70] {
        for &rad in &yp.stress.rm {
            let (a, b) = (ref_at(&jp.stress, k, rad), ref_at(&jp8.stress, k, rad));
            r1j.push((a[0] - b[0]).abs());
            r1j.push((a[1] - b[1]).abs());
        }
    }
    ch.at_most(
        "R1 J reference N = 400 against N = 800 at the proxy's points",
        worst(r1j),
        1e-4,
    );
    ch.finish();
}
