//! F-632 (ADR 0605): the domain integral on an elliptical front and the separation of K_I, K_II and K_III. Checks EH1 to EH4
//! (heavy record), EF, EU1, EU2, MI1 to MI5, M31 to M33, MT1, MT2, MR, MA, R5 and F1 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::fracture::{
    centre_crack_plate, elliptical_crack_block, interaction_integral, j_integral, near_tip_gradient, opening_k,
    penny_crack_cylinder, plane_strain_k, CrackFront, CrackedEllipse, CrackedPlate, EllipticalCrack, ModeFactors,
    OpeningPair, PennyCrack, PlateCrack, TipMode,
};
use rcfd_fem::hex20::traction_loads;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const A: f64 = 0.01;
const SIGMA: f64 = 1.0e8;

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

// ------------------------------------------------------------------------------------------------------------ ellipse

const B: f64 = 0.5 * A;

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

/// `K_ref,j = sqrt(<K_Irwin^2>_j)`: the hat average of station `j` of `n_theta` equal steps of the elliptic angle over the
/// exact ellipse (ADR 0605 D4), 16 Gauss points on each of 64 panels on each side of the station.
fn k_ref_station(j: usize, n_theta: usize) -> f64 {
    let (gx, gw) = gauss(16);
    let st = |i: usize| 0.5 * PI * i as f64 / n_theta as f64;
    let ds = |p: f64| (A * A * p.sin().powi(2) + B * B * p.cos().powi(2)).sqrt();
    let mut num = 0.0;
    let mut den = 0.0;
    let mut sides = Vec::new();
    if j > 0 {
        sides.push((st(j - 1), st(j)));
    }
    if j < n_theta {
        sides.push((st(j), st(j + 1)));
    }
    for (lo, hi) in sides {
        let panels = 64;
        let h = (hi - lo) / panels as f64;
        for p in 0..panels {
            let mid = lo + (p as f64 + 0.5) * h;
            for (x, w) in gx.iter().zip(&gw) {
                let phi = mid + 0.5 * h * x;
                let hat = if phi < st(j) {
                    (phi - lo) / (hi - lo)
                } else {
                    (hi - phi) / (hi - lo)
                };
                let wt = 0.5 * h * w * hat * ds(phi);
                num += irwin(phi).powi(2) * wt;
                den += wt;
            }
        }
    }
    (num / den).sqrt()
}

fn coarse_penny_spec(radius: f64, r_t: f64) -> PennyCrack {
    PennyCrack::new(radius, 12.0 * A, 10.0 * A, A / 8.0, 2, 4, 0.4 * A, 1, r_t, 1.5, 1.6)
}

fn coarse_ellipse() -> CrackedEllipse {
    elliptical_crack_block(&EllipticalCrack::new(coarse_penny_spec(A, 0.65 * A), B)).unwrap()
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

/// EU: the isotropic expansion `u = eps x` prescribed; `max_j |J_j| / (|p| h0)` over rings 1 to 3 (ADR 0605 D2).
fn uniform_expansion(c: &mut Checks, name: &str, mesh: &SolidMesh, front: &CrackFront, h0: f64) {
    let mut model = SolidModel::new(mesh.clone());
    model.add_material("all", material()).unwrap();
    let dofs = model.dof_map();
    let eps = 1e-3;
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in mesh.nodes().iter().enumerate() {
        for comp in 0..3 {
            u[dofs.equation(n, comp).unwrap()] = eps * x[comp];
        }
    }
    let bulk = E_MOD / (3.0 * (1.0 - 2.0 * NU));
    let p = -1.5 * bulk * eps * eps;
    for ring in 1..=3 {
        let v = j_integral(&model, &u, front, ring).unwrap();
        let eta = v.iter().map(|x| x.j.abs()).fold(0.0, f64::max) / (p.abs() * h0);
        let per: Vec<String> = v.iter().map(|x| format!("{:+.3e}", x.j / (p.abs() * h0))).collect();
        println!("{name} ring {ring}: J/(|p| h0) per station {}", per.join(" "));
        c.at_most(&format!("{name} ring {ring}, max |J|/(|p| h0)"), eta, 1e-8);
    }
}

#[test]
fn ef_eu1_r5_coarse_ellipse() {
    let mut c = Checks::new();
    let ell = coarse_ellipse();
    let (model, loads) = half_block_model(&ell.mesh);
    println!(
        "EF: {} nodes, {} elements, {} unknowns",
        ell.mesh.nodes().len(),
        ell.mesh.n_elements(),
        model.dof_map().n_equations()
    );
    let u = solve(&model, &loads, StaticSolver::Direct, false);
    let v = j_integral(&model, &u, &ell.crack_front(), 2).unwrap();
    let mut ef: f64 = 0.0;
    for x in &v {
        let kr = k_ref_station(x.station, 4);
        let phi = 0.5 * PI * x.station as f64 / 4.0;
        println!(
            "EF station {} (nu {:.6} against {:.6}): K {:.6e}, K_ref {:.6e} (point {:.6e}), K/K_ref - 1 = {:+.4e}",
            x.station,
            x.s,
            phi,
            k_of(x.j),
            kr,
            irwin(phi),
            k_of(x.j) / kr - 1.0
        );
        ef = ef.max(rel(k_of(x.j), kr));
    }
    c.at_most("EF coarse ellipse, ring 2, every station", ef, 0.08);
    uniform_expansion(&mut c, "EU1 coarse ellipse", &ell.mesh, &ell.crack_front(), A / 8.0);
    // R5: elliptical fronts and the builder.
    let z = [0.0, 0.0, 1.0];
    let x_axis = [1.0, 0.0, 0.0];
    let r = j_integral(
        &model,
        &u,
        &CrackFront::elliptical(ell.front.clone(), z, [0.0; 3], x_axis, A, A).half_symmetric(),
        2,
    );
    let (ok, detail) = refused(&r, "crack front", "a > b > 0");
    c.holds("R5 elliptical front with b = a refused", ok, &detail);
    let tilted = [1.0 / 2f64.sqrt(), 0.0, 1.0 / 2f64.sqrt()];
    let r = j_integral(
        &model,
        &u,
        &CrackFront::elliptical(ell.front.clone(), z, [0.0; 3], tilted, A, B).half_symmetric(),
        2,
    );
    let (ok, detail) = refused(&r, "crack front", "major axis");
    c.holds("R5 major axis not in the plane refused", ok, &detail);
    for (name, shift, expect_refused) in [("1e-4 a", 1e-4 * A, true), ("1e-8 a", 1e-8 * A, false)] {
        let mut nodes = ell.mesh.nodes().to_vec();
        nodes[ell.front[3]][0] += shift;
        let moved = rebuild(&ell.mesh, nodes);
        let (m, _) = half_block_model(&moved);
        let zero = vec![0.0; m.dof_map().n_equations()];
        let r = j_integral(&m, &zero, &ell.crack_front(), 2);
        if expect_refused {
            let (ok, detail) = refused(&r, "crack front", "ellipse");
            c.holds(&format!("R5 a front node moved by {name}: refused"), ok, &detail);
        } else {
            let ok = r.as_ref().is_ok_and(|v| v.iter().all(|x| x.j.is_finite()));
            c.holds(
                &format!("R5 a front node moved by {name}: accepted"),
                ok,
                &format!("{:?}", r.as_ref().map(|v| v.len()).map_err(|e| e.to_string())),
            );
        }
    }
    let pairs = [OpeningPair {
        upper: ell.front[0],
        lower: None,
    }];
    let r = opening_k(&model, &u, &ell.crack_front(), &pairs, 0.0, A);
    let (ok, detail) = refused(&r, "opening", "straight or circular");
    c.holds("R5 opening_k of an elliptical front refused", ok, &detail);
    let r = elliptical_crack_block(&EllipticalCrack::new(coarse_penny_spec(A, 0.65 * A), A));
    let (ok, detail) = refused(&r.map(|e| e.front.len()), "elliptical crack", "0 < b < a");
    c.holds("R5 builder with b = a refused", ok, &detail);
    let mut spec = coarse_penny_spec(A, 0.55 * A);
    spec.core_side = 0.3 * A;
    let r = elliptical_crack_block(&EllipticalCrack::new(spec, B));
    let (ok, detail) = refused(&r.map(|e| e.front.len()), "elliptical crack", "focal segment");
    c.holds("R5 builder with r_t <= a c/(a + b) refused", ok, &detail);
    c.finish();
}

#[test]
fn eu2_r5_coarse_penny() {
    let mut c = Checks::new();
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
    uniform_expansion(&mut c, "EU2 coarse penny", &cyl.mesh, &cyl.crack_front(), A / 8.0);
    let (model, _) = half_block_model(&cyl.mesh);
    let zero = vec![0.0; model.dof_map().n_equations()];
    let r = interaction_integral(
        &model,
        &zero,
        &CrackFront::circular(cyl.front.clone(), [0.0, 0.0, 1.0], [0.0; 3]),
        2,
    );
    let (ok, detail) = refused(&r, "crack front", "straight");
    c.holds("R5 interaction integral of a circular front refused", ok, &detail);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0605 (EH1 to EH4): about 86 000 unknowns, supernodal factor; run once alone"]
fn eh1_to_eh4_ellipse_heavy() {
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
    println!(
        "EH: {} nodes, {} elements, {} unknowns",
        ell.mesh.nodes().len(),
        ell.mesh.n_elements(),
        model.dof_map().n_equations()
    );
    let start = std::time::Instant::now();
    let u = solve(&model, &loads, StaticSolver::SupernodalDirect, false);
    println!("EH solve {:.1} s", start.elapsed().as_secs_f64());
    let refs: Vec<f64> = (0..=n_theta).map(|j| k_ref_station(j, n_theta)).collect();
    let mut per_ring = Vec::new();
    for k in 2..=4 {
        let v = j_integral(&model, &u, &ell.crack_front(), k).unwrap();
        let ks: Vec<f64> = v.iter().map(|x| k_of(x.j)).collect();
        for (x, kk) in v.iter().zip(&ks) {
            let phi = 0.5 * PI * x.station as f64 / n_theta as f64;
            println!(
                "ring {k} station {} (nu {:.6}): K {:.6e}, K_ref {:.6e} (point {:.6e}), K/K_ref - 1 = {:+.4e}",
                x.station,
                x.s,
                kk,
                refs[x.station],
                irwin(phi),
                kk / refs[x.station] - 1.0
            );
        }
        per_ring.push(ks);
    }
    let k3 = &per_ring[1];
    let eh1 = k3.iter().zip(&refs).map(|(&k, &r)| rel(k, r)).fold(0.0, f64::max);
    c.at_most("EH1 ring-3 K at every station against Irwin (hat average)", eh1, 0.03);
    let mut eh2: f64 = 0.0;
    for j in 0..=n_theta {
        let vals = [per_ring[0][j], per_ring[1][j], per_ring[2][j]];
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        eh2 = eh2.max((hi - lo) / lo);
    }
    c.at_most("EH2 spread of rings 2 to 4, every station", eh2, 0.01);
    let ratio = k3[0] / k3[n_theta];
    let ratio_ref = refs[0] / refs[n_theta];
    println!("EH3 K_0/K_12 = {ratio:.6}, reference {ratio_ref:.6}");
    c.at_most("EH3 K_0/K_12 against the reference ratio", rel(ratio, ratio_ref), 0.02);
    for (k, ks) in [(2, &per_ring[0]), (4, &per_ring[2])] {
        let e = ks.iter().zip(&refs).map(|(&v, &r)| rel(v, r)).fold(0.0, f64::max);
        println!("EH4 ring {k}: largest error {e:.4e} (printed, no limit)");
        c.holds(&format!("F1 EH4 ring {k} finite"), e.is_finite(), "");
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- near-tip fields

#[test]
fn ma_near_tip_fields() {
    let mut c = Checks::new();
    let mu = shear_modulus();
    let lambda = E_MOD * NU / ((1.0 + NU) * (1.0 - 2.0 * NU));
    let hooke = |g: &[[f64; 3]; 3]| {
        let tr = g[0][0] + g[1][1] + g[2][2];
        let mut s = [[0.0; 3]; 3];
        for i in 0..3 {
            for k in 0..3 {
                s[i][k] = mu * (g[i][k] + g[k][i]) + if i == k { lambda * tr } else { 0.0 };
            }
        }
        s
    };
    let at = |mode: TipMode, x: f64, y: f64| {
        let (r, th) = ((x * x + y * y).sqrt(), y.atan2(x));
        near_tip_gradient(mode, r, th, mu, NU)
    };
    let points: [(f64, f64); 4] = [
        (0.3e-3, 0.7e-3),
        (-0.4e-3, 0.5e-3),
        (-0.6e-3, -0.2e-3),
        (0.8e-3, -0.1e-3),
    ];
    for (mode, unit) in [
        (TipMode::I, [1.0, 0.0, 0.0]),
        (TipMode::II, [0.0, 1.0, 0.0]),
        (TipMode::III, [0.0, 0.0, 1.0]),
    ] {
        let (mut g_err, mut div_err): (f64, f64) = (0.0, 0.0);
        for &(x, y) in &points {
            let r = (x * x + y * y).sqrt();
            let h = 1e-6 * r;
            let (_, g) = at(mode, x, y);
            let gmax = g.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
            for (k, (dx, dy)) in [(h, 0.0), (0.0, h)].into_iter().enumerate() {
                let (up, _) = at(mode, x + dx, y + dy);
                let (um, _) = at(mode, x - dx, y - dy);
                for i in 0..3 {
                    g_err = g_err.max(((up[i] - um[i]) / (2.0 * h) - g[i][k]).abs() / gmax);
                }
            }
            let s0 = hooke(&g);
            let smax = s0.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
            let sp_x = hooke(&at(mode, x + h, y).1);
            let sm_x = hooke(&at(mode, x - h, y).1);
            let sp_y = hooke(&at(mode, x, y + h).1);
            let sm_y = hooke(&at(mode, x, y - h).1);
            for i in 0..3 {
                let div = (sp_x[i][0] - sm_x[i][0]) / (2.0 * h) + (sp_y[i][1] - sm_y[i][1]) / (2.0 * h);
                div_err = div_err.max(div.abs() / (smax / r));
            }
        }
        c.at_most(&format!("MA {mode:?} gradient against differences"), g_err, 1e-6);
        c.at_most(&format!("MA {mode:?} div sigma / (|sigma|/r)"), div_err, 1e-5);
        let mut face: f64 = 0.0;
        for th in [PI - 1e-9, -(PI - 1e-9)] {
            let (_, g) = near_tip_gradient(mode, 1e-3, th, mu, NU);
            let s = hooke(&g);
            let smax = s.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
            // Traction on the face of normal x2: (s_12, s_22, s_32); ahead of the tip the mode's own components scale it.
            let (_, g0) = near_tip_gradient(mode, 1e-3, 0.0, mu, NU);
            let s_ref = hooke(&g0).iter().flatten().map(|v| v.abs()).fold(smax, f64::max);
            for t in [s[0][1], s[1][1], s[2][1]] {
                face = face.max(t.abs() / s_ref);
            }
        }
        c.at_most(&format!("MA {mode:?} traction on the faces"), face, 1e-6);
        let r = 1e-6;
        let s = hooke(&near_tip_gradient(mode, r, 0.0, mu, NU).1);
        let f = (2.0 * PI * r).sqrt();
        let got = [s[1][1] * f, s[0][1] * f, s[1][2] * f];
        let err = (0..3).map(|i| (got[i] - unit[i]).abs()).fold(0.0, f64::max);
        c.at_most(&format!("MA {mode:?} normalisation ahead"), err, 1e-12);
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------- plates

fn plate() -> CrackedPlate {
    centre_crack_plate(&PlateCrack::new(A, 10.0 * A, 20.0 * A, 0.05 * A, A / 40.0, 1.4, 4)).unwrap()
}

fn front_mean(v: &[ModeFactors]) -> [f64; 3] {
    let w: f64 = v.iter().map(|x| x.weight).sum();
    [
        v.iter().map(|x| x.k_i * x.weight).sum::<f64>() / w,
        v.iter().map(|x| x.k_ii * x.weight).sum::<f64>() / w,
        v.iter().map(|x| x.k_iii * x.weight).sum::<f64>() / w,
    ]
}

fn j_mean(model: &SolidModel, u: &[f64], front: &CrackFront, ring: usize) -> f64 {
    let v = j_integral(model, u, front, ring).unwrap();
    let w: f64 = v.iter().map(|x| x.weight).sum();
    v.iter().map(|x| x.j * x.weight).sum::<f64>() / w
}

fn record(name: &str, v: &[ModeFactors], scale: f64) {
    for x in v {
        println!(
            "MR {name} station {}: K_I {:+.6e}, K_II {:+.6e}, K_III {:+.6e} (/ scale {:+.4e} {:+.4e} {:+.4e})",
            x.station,
            x.k_i,
            x.k_ii,
            x.k_iii,
            x.k_i / scale,
            x.k_ii / scale,
            x.k_iii / scale
        );
    }
}

#[test]
fn mi1_to_mi5_inclined_crack() {
    let mut c = Checks::new();
    let beta = PI / 6.0;
    let (sb, cb) = beta.sin_cos();
    let p = plate();
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
    let mut means = Vec::new();
    for ring in 2..=4 {
        let v = interaction_integral(&model, &u, &front, ring).unwrap();
        record(&format!("I ring {ring}"), &v, k0);
        let m = front_mean(&v);
        println!(
            "I ring {ring}: front mean K_I {:.6e} ({:.6}), K_II {:.6e} ({:.6}), K_III {:+.3e}",
            m[0],
            m[0] / k0,
            m[1],
            m[1] / k0,
            m[2]
        );
        means.push(m);
    }
    let m3 = means[1];
    c.at_most(
        "MI1 K_I against sigma sqrt(pi a) cos^2 beta",
        rel(m3[0], k0 * cb * cb),
        0.02,
    );
    c.at_most(
        "MI2 K_II against sigma sqrt(pi a) sin beta cos beta",
        rel(m3[1], k0 * sb * cb),
        0.02,
    );
    c.at_most("MI3 |K_III|/(sigma sqrt(pi a))", m3[2].abs() / k0, 1e-8);
    let j = j_mean(&model, &u, &front, 3);
    let j_k = (m3[0] * m3[0] + m3[1] * m3[1]) / e_prime();
    println!("MI4 J {j:.6e}, (K_I^2 + K_II^2)/E' {j_k:.6e}");
    c.at_most("MI4 J against (K_I^2 + K_II^2)/E'", rel(j, j_k), 0.02);
    let mut spread: f64 = 0.0;
    for comp in 0..2 {
        let vals: Vec<f64> = means.iter().map(|m| m[comp]).collect();
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        spread = spread.max((hi - lo) / hi);
    }
    c.at_most("MI5 spread of K_I and K_II over rings 2 to 4", spread, 0.01);
    c.finish();
}

#[test]
fn m31_to_m33_antiplane() {
    let mut c = Checks::new();
    let p = plate();
    let mut loads = traction_loads(&p.mesh, "top", [0.0, 0.0, SIGMA]).unwrap();
    loads.extend(traction_loads(&p.mesh, "bottom", [0.0, 0.0, -SIGMA]).unwrap());
    let mut model = SolidModel::new(p.mesh.clone());
    model.add_material("all", material()).unwrap();
    model.add_constraint(Constraint::displacement("all", 0, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    model
        .add_constraint(Constraint::displacement("right_pin", 2, 0.0))
        .unwrap();
    let u = solve(&model, &loads, StaticSolver::Direct, true);
    let front = p.crack_front();
    let k0 = SIGMA * (PI * A).sqrt();
    let v = interaction_integral(&model, &u, &front, 3).unwrap();
    record("III ring 3", &v, k0);
    let m = front_mean(&v);
    println!(
        "III front mean: K_I {:+.3e}, K_II {:+.3e}, K_III {:.6e} ({:.6})",
        m[0],
        m[1],
        m[2],
        m[2] / k0
    );
    c.at_most("M31 K_III against tau sqrt(pi a)", rel(m[2], k0), 0.02);
    c.at_most(
        "M32 max(|K_I|, |K_II|)/(tau sqrt(pi a))",
        m[0].abs().max(m[1].abs()) / k0,
        1e-8,
    );
    let j = j_mean(&model, &u, &front, 3);
    let k_j = (2.0 * shear_modulus() * j).sqrt();
    println!("M33 sqrt(2 mu J) {k_j:.6e}");
    c.at_most("M33 sqrt(2 mu J) against K_III", rel(k_j, m[2]), 0.01);
    c.finish();
}

#[test]
fn mt1_mt2_r5_mode_one_plate() {
    let mut c = Checks::new();
    let p = plate();
    let mut loads = traction_loads(&p.mesh, "top", [0.0, SIGMA, 0.0]).unwrap();
    loads.extend(traction_loads(&p.mesh, "bottom", [0.0, -SIGMA, 0.0]).unwrap());
    let plate_model = |mesh: SolidMesh, two: bool| {
        let mut m = SolidModel::new(mesh);
        if two {
            m.add_material("rest", material()).unwrap();
            m.add_material("one", Material::isotropic_elastic(1.1 * E_MOD, NU, 0.0, 0.0).unwrap())
                .unwrap();
        } else {
            m.add_material("all", material()).unwrap();
        }
        m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("right_pin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("right_pin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("left_pin", 1, 0.0)).unwrap();
        m
    };
    let model = plate_model(p.mesh.clone(), false);
    let u = solve(&model, &loads, StaticSolver::Direct, true);
    let front = p.crack_front();
    let v = interaction_integral(&model, &u, &front, 3).unwrap();
    let m = front_mean(&v);
    record("T ring 3", &v, m[0]);
    let k_j = k_of(j_mean(&model, &u, &front, 3));
    println!(
        "MT front mean K_I {:.6e}, K from J {k_j:.6e}, K_II {:+.3e}, K_III {:+.3e}",
        m[0], m[1], m[2]
    );
    c.at_most("MT1 K_I against plane_strain_k(J)", rel(m[0], k_j), 0.01);
    c.at_most("MT2 max(|K_II|, |K_III|)/K_I", m[1].abs().max(m[2].abs()) / m[0], 1e-8);
    // R5: a half-symmetric front and a second material in the domain.
    let r = interaction_integral(&model, &u, &front.clone().half_symmetric(), 3);
    let (ok, detail) = refused(&r, "crack front", "half-symmetric");
    c.holds("R5 interaction integral of a half-symmetric front refused", ok, &detail);
    let tip = p.front[0];
    let one = (0..p.mesh.n_elements())
        .find(|&e| p.mesh.element_nodes(e).contains(&tip))
        .unwrap();
    let mut mesh = p.mesh.clone();
    mesh.add_element_set("one", vec![one]).unwrap();
    mesh.add_element_set("rest", (0..p.mesh.n_elements()).filter(|&e| e != one).collect())
        .unwrap();
    let two = plate_model(mesh, true);
    let r = interaction_integral(&two, &u, &front, 3);
    let (ok, detail) = refused(&r, "element", "one material");
    c.holds("R5 a ring element of another material refused", ok, &detail);
    c.finish();
}
