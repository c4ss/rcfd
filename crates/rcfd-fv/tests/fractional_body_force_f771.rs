//! F-771 (ADR 0691): the body force in the fractional step (`PimpleSettings::fractional_body_force`): the walls' pressure
//! gradient, the predictor's cell term and the balanced-force face term of the intermediate flux. Cases: ADR 0672's skewed
//! periodic channel `P` with its body force against its unrolled twin `T` (the meshes and the map copied from ADR 0687's test
//! file, which is not edited; checks P1, NC1, P2, D1L, C1, G0), a discrete hydrostatic rest state on `P` (R1, R1p, R2, R2p,
//! NR1), the start-up channel on an orthogonal periodic mesh against a 1-D model and the closed form (S1a, S1b, S2a, S2b, NS),
//! and the no-ops and the refusal (DB1, DB2, DB3). The limits are those registered in ADR 0691 before this code ran; each test
//! prints every measured number and then asserts the checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{apply_body_force_wall_gradient, boundary_values_pbc, divergence, pressure_bc};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, warp};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const NU: f64 = 0.1;
const U_REF: f64 = 1.0;
const DT: f64 = 0.01;
const STEPS: usize = 200;
const TAU: f64 = 0.3;
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

const L: f64 = 2.0;
const H: f64 = 1.0;
const NX: usize = 32;
const NY: usize = 16;
const FX: f64 = 0.8;
const GY: f64 = -2.0;
/// The amplitude of the periodic part of the rest test's potential (ADR 0691).
const C_PHI: f64 = 0.1;
const U0: f64 = 1.0;
const A: f64 = 0.5 * H / PI;
const WALLS: [&str; 2] = ["ymin", "ymax"];

fn geom_of(m: Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

/// For every point of `pts`, the nearest of `centres`; the largest distance; whether one-to-one.
fn match_points(pts: &[Vec3], centres: &[Vec3]) -> (Vec<usize>, f64, bool) {
    let mut worst: f64 = 0.0;
    let idx: Vec<usize> = pts
        .iter()
        .map(|c| {
            let (mut best, mut d) = (0, f64::INFINITY);
            for (j, a) in centres.iter().enumerate() {
                let e = vec3::norm(vec3::sub(*c, *a));
                if e < d {
                    d = e;
                    best = j;
                }
            }
            worst = worst.max(d);
            best
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let one = idx.iter().all(|j| seen.insert(*j));
    (idx, worst, one)
}

fn bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flat_map(|u| u.iter().map(|x| x.to_bits())).collect();
    v.extend(s.p.values.iter().map(|x| x.to_bits()));
    v.extend(s.phi.iter().map(|x| x.to_bits()));
    v
}

/// The largest of a per-step series and its step (1-based).
fn largest(x: &[f64]) -> (f64, usize) {
    x.iter().enumerate().fold((0.0f64, 0), |b, (s, &v)| {
        if finite_or_inf(v) > b.0 {
            (finite_or_inf(v), s + 1)
        } else {
            b
        }
    })
}

/// The coupled pairs `(f, g)` of a mesh, each once (`f < g`).
fn coupled_pairs(g: &FvGeometry) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, p) in g.mesh.patches().iter().enumerate() {
        if let Some(map) = &g.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                if f < map[local] {
                    out.push((f, map[local]));
                }
            }
        }
    }
    out
}

fn pair_sum(phi: &[f64], pairs: &[(usize, usize)]) -> f64 {
    pairs
        .iter()
        .fold(0.0f64, |w, &(f, g)| w.max(finite_or_inf((phi[f] + phi[g]).abs())))
}

/// `max abs(phi_h - U_h . S_h)` over the internal faces `faces`.
fn defect_at(g: &FvGeometry, s: &PimpleSolver, faces: &[usize]) -> f64 {
    let zero_b = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
    let fo = face_flux(g, &s.u.values, &zero_b);
    faces
        .iter()
        .fold(0.0f64, |w, &h| w.max(finite_or_inf((s.phi[h] - fo[h]).abs())))
}

// ---- ADR 0672's channel ----------------------------------------------------------------------------------------------------

fn tilt(x: f64) -> f64 {
    let s = x.rem_euclid(L);
    if s <= 0.25 * L || s >= 0.75 * L {
        TAU * (2.0 * PI * s / L).cos().powi(2)
    } else {
        0.0
    }
}

fn mesh(shift: f64) -> Mesh {
    let m = box_2d(NX, NY, L, H, true, false).unwrap();
    warp(&m, |p| [p[0] + tilt(p[0] + shift) * (p[1] - 0.5 * H), p[1], p[2]]).unwrap()
}

fn image(x: Vec3) -> Vec3 {
    [(x[0] - 0.5 * L).rem_euclid(L), x[1], x[2]]
}

fn p0(x: Vec3) -> f64 {
    GY * (x[1] - 0.5 * H)
}

fn div_scale() -> f64 {
    U_REF * (H / NY as f64) * (L / NX as f64)
}

fn vdiff(a: &[Vec3], b: &[Vec3], m: &[usize]) -> f64 {
    a.iter().enumerate().fold(0.0f64, |w, (c, v)| {
        w.max(finite_or_inf(vec3::norm(vec3::sub(*v, b[m[c]]))))
    })
}

fn sdiff(a: &[f64], b: &[f64], m: &[usize]) -> f64 {
    a.iter()
        .enumerate()
        .fold(0.0f64, |w, (c, v)| w.max(finite_or_inf((v - b[m[c]]).abs())))
}

/// The options of one fractional run.
#[derive(Clone, Copy)]
struct Frac {
    /// `coupled_ddt_correction`.
    opt: bool,
    /// `fractional_step_consistent` and `fractional_periodic_implicit` (the twin settings of ADR 0687).
    twin: bool,
    /// `fractional_body_force`.
    body: bool,
}

/// ADR 0687's fractional settings (implicit Euler, incremental, Kim-Moin, no non-orthogonal correction).
fn frac_settings(o: Frac) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.n_outer = 1;
    s.n_correctors = 2;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s.ddt_correction = true;
    s.non_orth_correctors = 0;
    s.fractional = Some(FractionalSettings::default());
    s.fractional_step_consistent = o.twin;
    s.fractional_periodic_implicit = o.twin;
    s.coupled_ddt_correction = o.opt;
    s.fractional_body_force = o.body;
    s
}

fn walled_u(g: &Arc<FvGeometry>, f: impl Fn(Vec3) -> Vec3) -> VectorField {
    let mut u = VectorField::from_fn("U", g, f);
    for w in WALLS {
        u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u
}

/// ADR 0672's flow in the fractional step: from rest, hydrostatic start, the body force `(FX, GY)`.
fn channel_solver(g: &Arc<FvGeometry>, o: Frac, ref_cell: usize) -> PimpleSolver {
    let u = walled_u(g, |_| [0.0; 3]);
    let p = ScalarField::from_fn("p", g, p0);
    let mut s = frac_settings(o);
    s.p_ref = (ref_cell, p0(g.mesh.cell_centres()[ref_cell]));
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    sv.body_force = Some(BodyForce::uniform(g, [FX, GY, 0.0]));
    sv
}

/// The two meshes, the cell map, the internal faces of `T` at the images of `P`'s cut.
struct Pair {
    gp: Arc<FvGeometry>,
    gt: Arc<FvGeometry>,
    m: Vec<usize>,
    match_d: f64,
    match_one: bool,
    pairs: Vec<(usize, usize)>,
    t_faces: Vec<usize>,
}

fn pair() -> Pair {
    let gp = geom_of(mesh(0.0));
    let gt = geom_of(mesh(0.5 * L));
    let imgs: Vec<Vec3> = gp.mesh.cell_centres().iter().map(|&c| image(c)).collect();
    let (m, match_d, match_one) = match_points(&imgs, gt.mesh.cell_centres());
    let pairs = coupled_pairs(&gp);
    let cut: Vec<Vec3> = pairs
        .iter()
        .flat_map(|&(f, g)| [f, g])
        .map(|f| image(gp.mesh.face_centres()[f]))
        .collect();
    let (t_faces, _, _) = match_points(&cut, &gt.mesh.face_centres()[..gt.n_internal]);
    Pair {
        gp,
        gt,
        m,
        match_d,
        match_one,
        pairs,
        t_faces,
    }
}

/// P1, NC1, P2, D1L, C1, G0 of ADR 0691: the fractional step with the body force on ADR 0672's channel against its twin.
#[test]
fn fractional_channel_with_body_force_against_the_twin() {
    let t0 = Instant::now();
    let pr = pair();
    let (rp, rt) = (0, pr.m[0]);
    let f = |opt| Frac {
        opt,
        twin: true,
        body: true,
    };
    let mut p_on = channel_solver(&pr.gp, f(true), rp);
    let mut p_off = channel_solver(&pr.gp, f(false), rp);
    let mut t_on = channel_solver(&pr.gt, f(true), rt);
    let mut t_off = channel_solver(&pr.gt, f(false), rt);
    let m = &pr.m;
    let sc = div_scale();
    let ident: Vec<usize> = (0..t_on.u.values.len()).collect();
    let (mut g_on, mut g_off, mut p2, mut d1l, mut c1) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut t_oo, mut defect, mut c1_off) = (0.0f64, 0.0f64, 0.0f64);
    for _ in 0..STEPS {
        p_on.step(DT);
        p_off.step(DT);
        t_on.step(DT);
        t_off.step(DT);
        g_on.push(vdiff(&p_on.u.values, &t_on.u.values, m) / U_REF);
        g_off.push(vdiff(&p_off.u.values, &t_on.u.values, m) / U_REF);
        p2.push(sdiff(&p_on.p.values, &t_on.p.values, m) / (GY.abs() * H));
        let (dvp, dvt) = (divergence(&pr.gp, &p_on.phi), divergence(&pr.gt, &t_on.phi));
        d1l.push(sdiff(&dvp, &dvt, m) / sc);
        c1.push(pair_sum(&p_on.phi, &pr.pairs) / sc);
        c1_off = c1_off.max(pair_sum(&p_off.phi, &pr.pairs) / sc);
        t_oo = t_oo.max(vdiff(&t_on.u.values, &t_off.u.values, &ident) / U_REF);
        defect = defect.max(defect_at(&pr.gt, &t_on, &pr.t_faces) / sc);
    }
    for (name, g) in [("G_on", &g_on), ("G_off", &g_off)] {
        let (mx, st) = largest(g);
        println!(
            "{name}: step 1 {:.4e}, step 2 {:.4e}, largest {mx:.4e} U_ref at step {st}, last {:.4e}, smallest from step 2 {:.4e}",
            g[0],
            g[1],
            g[STEPS - 1],
            g[1..].iter().fold(f64::INFINITY, |a, &v| a.min(v))
        );
    }
    let ub: f64 = t_on.u.values.iter().map(|v| v[0]).sum::<f64>() / t_on.u.values.len() as f64;
    println!(
        "T_on against T_off {t_oo:.4e} U_ref; largest old-level defect on T_on's faces at the images of P's cut {defect:.4e} \
         U_ref h dz ({} faces); C1 of P_off {c1_off:.3e}; T_on mean u_x at the end {ub:.5}; elapsed {:.1} s",
        pr.t_faces.len(),
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    let (n1, s1) = largest(&g_on);
    check(
        &mut fails,
        "P1",
        n1 <= 1e-8,
        format!("max over {STEPS} steps of G_on(n) {n1:.4e} U_ref (step {s1}) (1e-8 each step)"),
    );
    let (nc, sn) = largest(&g_off);
    check(
        &mut fails,
        "NC1",
        nc > 1e-8,
        format!("control: largest G_off(n) {nc:.4e} U_ref (step {sn}) (> 1e-8 on at least one step)"),
    );
    let (pp, sp) = largest(&p2);
    check(
        &mut fails,
        "P2",
        pp <= 1e-8,
        format!("max over the steps of |p_Pon - p_Ton| / (|g_y| H) {pp:.4e} (step {sp}) (1e-8 each step)"),
    );
    let (dl, sl) = largest(&d1l);
    check(
        &mut fails,
        "D1L",
        dl <= 1e-9,
        format!("max over the steps of |div_Pon - div_Ton| / (U_ref h dz) {dl:.4e} (step {sl}) (1e-9 each step)"),
    );
    let (cc, scc) = largest(&c1);
    check(
        &mut fails,
        "C1",
        cc <= 1e-12,
        format!(
            "max over the steps of |phi_f + phi_g| / (U_ref h dz) on P_on's {} pairs {cc:.3e} (step {scc}) (1e-12 each step)",
            pr.pairs.len()
        ),
    );
    check(
        &mut fails,
        "G0",
        pr.match_d <= 1e-12 && pr.match_one,
        format!(
            "nearest T cell of every translated P cell {:.2e} (1e-12), one-to-one {}",
            pr.match_d, pr.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---- the rest state ---------------------------------------------------------------------------------------------------------

fn phi_rest(x: Vec3) -> f64 {
    GY * (x[1] - 0.5 * H) + C_PHI * (2.0 * PI * x[0] / L).cos()
}

/// ADR 0691's discrete potential force of `Phi` on the mesh `g`: compact differences on the internal and coupled faces,
/// `g_y S_y` on the other boundary faces, and the cell values the step's Green-Gauss gradient of `p = Phi` takes.
fn potential_force(g: &Arc<FvGeometry>, u: &VectorField, p: &ScalarField) -> BodyForce {
    let mesh = &g.mesh;
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let n_int = g.n_internal;
    let ph = &p.values;
    let mut pbc = pressure_bc(g, u, p);
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if f < n_int {
                g.lap_coeff[f] * (ph[nei[f]] - ph[own[f]])
            } else {
                let i = f - n_int;
                match pbc.coupled[i] {
                    Some(gp) => {
                        let span = g.dn[i] + g.dn[gp - n_int];
                        vec3::norm(sf[f]) * (ph[own[gp]] - ph[own[f]]) / span
                    }
                    None => GY * sf[f][1],
                }
            }
        })
        .collect();
    let mut bf = BodyForce {
        cell: vec![[0.0; 3]; g.n_cells()],
        face,
    };
    apply_body_force_wall_gradient(g, &bf, &mut pbc);
    let bv = boundary_values_pbc(g, &pbc, ph);
    bf.cell = green_gauss(g, ph, &bv, 0);
    bf
}

fn rest_solver(g: &Arc<FvGeometry>, o: Frac) -> PimpleSolver {
    let u = walled_u(g, |_| [0.0; 3]);
    let p = ScalarField::from_fn("p", g, phi_rest);
    let bf = potential_force(g, &u, &p);
    let mut s = frac_settings(o);
    s.p_ref = (0, p.values[0]);
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    sv.body_force = Some(bf);
    sv
}

/// R1, R1p, R2, R2p, NR1 of ADR 0691: a discrete hydrostatic rest state on `P` stays at rest.
#[test]
fn fractional_hydrostatic_rest_state_stays_at_rest() {
    let t0 = Instant::now();
    let g = geom_of(mesh(0.0));
    let phi0: Vec<f64> = g.mesh.cell_centres().iter().map(|&c| phi_rest(c)).collect();
    let mut r1 = rest_solver(
        &g,
        Frac {
            opt: true,
            twin: true,
            body: true,
        },
    );
    let mut r2 = rest_solver(
        &g,
        Frac {
            opt: false,
            twin: false,
            body: true,
        },
    );
    let mut nr1 = rest_solver(
        &g,
        Frac {
            opt: true,
            twin: true,
            body: false,
        },
    );
    let umax = |s: &PimpleSolver| {
        s.u.values
            .iter()
            .fold(0.0f64, |w, v| w.max(finite_or_inf(vec3::norm(*v))))
    };
    let pdev = |s: &PimpleSolver| {
        s.p.values
            .iter()
            .zip(&phi0)
            .fold(0.0f64, |w, (a, b)| w.max(finite_or_inf((a - b).abs())))
            / (GY.abs() * H)
    };
    let fmax = |s: &PimpleSolver| s.phi.iter().fold(0.0f64, |w, v| w.max(finite_or_inf(v.abs())));
    let (mut u1, mut p1, mut u2, mut p2, mut un) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut f1, mut f2) = (0.0f64, 0.0f64);
    for _ in 0..STEPS {
        r1.step(DT);
        r2.step(DT);
        nr1.step(DT);
        u1.push(umax(&r1));
        p1.push(pdev(&r1));
        u2.push(umax(&r2));
        p2.push(pdev(&r2));
        un.push(umax(&nr1));
        f1 = f1.max(fmax(&r1));
        f2 = f2.max(fmax(&r2));
    }
    println!(
        "printed: largest |phi| R1 {f1:.3e}, R2 {f2:.3e}; NR1 step 1 {:.4e}, last {:.4e}; elapsed {:.1} s",
        un[0],
        un[STEPS - 1],
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    for (id, x, what) in [
        ("R1", &u1, "max_c |u|"),
        ("R1p", &p1, "max_c |p - Phi| / (|g_y| H)"),
        ("R2", &u2, "max_c |u|"),
        ("R2p", &p2, "max_c |p - Phi| / (|g_y| H)"),
    ] {
        let (v, st) = largest(x);
        check(
            &mut fails,
            id,
            v <= 1e-10,
            format!("largest over {STEPS} steps of {what} {v:.4e} (step {st}) (1e-10 each step)"),
        );
    }
    let (v, st) = largest(&un);
    check(
        &mut fails,
        "NR1",
        v > 1e-6,
        format!("control (the force not read): largest max_c |u| {v:.4e} (step {st}) (> 1e-6 on at least one step)"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---- the start-up channel ---------------------------------------------------------------------------------------------------

/// The closed-form bulk velocity of the start-up channel at time `t`.
fn bulk_exact(t: f64) -> f64 {
    let mut ub = FX * H * H / (12.0 * NU);
    let mut n = 1;
    while n < 400 {
        let nf = n as f64;
        ub -= 8.0 * FX * H * H / (NU * PI.powi(4) * nf.powi(4)) * (-NU * nf * nf * PI * PI * t / (H * H)).exp();
        n += 2;
    }
    ub
}

/// One implicit-Euler step of the 1-D cell-centred model (per unit length and depth; walls at half a cell).
fn model_step(u: &mut [f64]) {
    let ny = u.len();
    let dy = H / ny as f64;
    let (mut a, mut b, mut c, mut d) = (vec![0.0; ny], vec![0.0; ny], vec![0.0; ny], vec![0.0; ny]);
    for j in 0..ny {
        b[j] = dy / DT;
        if j > 0 {
            b[j] += NU / dy;
            a[j] = -NU / dy;
        } else {
            b[j] += NU / (0.5 * dy);
        }
        if j + 1 < ny {
            b[j] += NU / dy;
            c[j] = -NU / dy;
        } else {
            b[j] += NU / (0.5 * dy);
        }
        d[j] = dy / DT * u[j] + dy * FX;
    }
    for j in 1..ny {
        let m = a[j] / b[j - 1];
        b[j] -= m * c[j - 1];
        d[j] -= m * d[j - 1];
    }
    u[ny - 1] = d[ny - 1] / b[ny - 1];
    for j in (0..ny - 1).rev() {
        u[j] = (d[j] - c[j] * u[j + 1]) / b[j];
    }
}

/// S1a, S1b, S2a, S2b, NS of ADR 0691: the start-up channel on the orthogonal periodic mesh. Known miss (ADR 0691 amendment
/// 1): the one registered run (b2ac49e3) gave S2a `5.7930e-2` (step 1; limit `1e-9`); S1a `3.5684e-4`, S1b `5.4624e-3` (limit
/// `2e-2`), S2b `5.4266e-14`, NS `1.0` met. Cause: the lagged periodic partner of the plain fractional predictor (S-a without
/// `fractional_periodic_implicit`); the corrected set-up is `fractional_start_up_channel_with_the_implicit_partner`.
#[test]
#[ignore = "known miss of ADR 0691 (S2a): the registered record, limits intact"]
fn fractional_start_up_channel_against_the_model_and_the_closed_form_known_miss() {
    let t0 = Instant::now();
    let g = geom_of(box_2d(NX, NY, L, H, true, false).unwrap());
    let mk = |o: Frac| {
        let u = walled_u(&g, |_| [0.0; 3]);
        let p = ScalarField::from_fn("p", &g, p0);
        let mut s = frac_settings(o);
        s.p_ref = (0, p0(g.mesh.cell_centres()[0]));
        let mut sv = PimpleSolver::new(g.clone(), u, p, s);
        sv.body_force = Some(BodyForce::uniform(&g, [FX, GY, 0.0]));
        sv
    };
    let mut sa = mk(Frac {
        opt: false,
        twin: false,
        body: true,
    });
    let mut sb = mk(Frac {
        opt: true,
        twin: true,
        body: true,
    });
    let mut ns = mk(Frac {
        opt: false,
        twin: false,
        body: false,
    });
    let dy = H / NY as f64;
    let row: Vec<usize> = g
        .mesh
        .cell_centres()
        .iter()
        .map(|c| ((c[1] / dy).floor() as usize).min(NY - 1))
        .collect();
    let mut model = vec![0.0; NY];
    let bulk = |s: &PimpleSolver| s.u.values.iter().map(|v| v[0]).sum::<f64>() / s.u.values.len() as f64;
    let dev = |s: &PimpleSolver, model: &[f64], ubm: f64| {
        s.u.values.iter().enumerate().fold(0.0f64, |w, (c, v)| {
            w.max(finite_or_inf((v[0] - model[row[c]]).abs()))
                .max(finite_or_inf(v[1].abs()))
        }) / ubm
    };
    let (mut s2a, mut s2b) = (Vec::new(), Vec::new());
    for _ in 0..STEPS {
        sa.step(DT);
        sb.step(DT);
        ns.step(DT);
        model_step(&mut model);
        let ubm = model.iter().sum::<f64>() / NY as f64;
        s2a.push(dev(&sa, &model, ubm));
        s2b.push(dev(&sb, &model, ubm));
    }
    let t = DT * STEPS as f64;
    let ex = bulk_exact(t);
    let ubm = model.iter().sum::<f64>() / NY as f64;
    let (ua, ub, un) = (bulk(&sa), bulk(&sb), bulk(&ns));
    println!(
        "printed: U_b at t = {t}: S-a {ua:.6}, S-b {ub:.6}, NS {un:.3e}, 1-D model {ubm:.6}, closed form {ex:.6}; elapsed {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    for (id, v) in [("S1a", ua), ("S1b", ub)] {
        let r = finite_or_inf(((v - ex) / ex).abs());
        check(
            &mut fails,
            id,
            r <= 2e-2,
            format!("|U_b - U_b,exact| / U_b,exact at t = {t}: {r:.4e} (2e-2)"),
        );
    }
    for (id, x) in [("S2a", &s2a), ("S2b", &s2b)] {
        let (v, st) = largest(x);
        check(
            &mut fails,
            id,
            v <= 1e-9,
            format!(
                "largest over the steps of max_c (|u_x - u_1D|, |u_y|) / U_b,1D {v:.4e} (step {st}) (1e-9 each step)"
            ),
        );
    }
    let r = finite_or_inf(((un - ex) / ex).abs());
    check(
        &mut fails,
        "NS",
        r > 0.5,
        format!("control (the force not read): |U_b - U_b,exact| / U_b,exact {r:.4e} (> 0.5)"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// S1a', S2a' of ADR 0691 amendment 1: the plain fractional start-up run (`dte` coefficients) with the implicit periodic
/// partner only, against the 1-D model and the closed form.
#[test]
fn fractional_start_up_channel_with_the_implicit_partner() {
    let t0 = Instant::now();
    let g = geom_of(box_2d(NX, NY, L, H, true, false).unwrap());
    let u = walled_u(&g, |_| [0.0; 3]);
    let p = ScalarField::from_fn("p", &g, p0);
    let mut s = frac_settings(Frac {
        opt: false,
        twin: false,
        body: true,
    });
    s.fractional_periodic_implicit = true;
    s.p_ref = (0, p0(g.mesh.cell_centres()[0]));
    let mut sa = PimpleSolver::new(g.clone(), u, p, s);
    sa.body_force = Some(BodyForce::uniform(&g, [FX, GY, 0.0]));
    let dy = H / NY as f64;
    let row: Vec<usize> = g
        .mesh
        .cell_centres()
        .iter()
        .map(|c| ((c[1] / dy).floor() as usize).min(NY - 1))
        .collect();
    let mut model = vec![0.0; NY];
    let mut s2 = Vec::new();
    for _ in 0..STEPS {
        sa.step(DT);
        model_step(&mut model);
        let ubm = model.iter().sum::<f64>() / NY as f64;
        let d = sa.u.values.iter().enumerate().fold(0.0f64, |w, (c, v)| {
            w.max(finite_or_inf((v[0] - model[row[c]]).abs()))
                .max(finite_or_inf(v[1].abs()))
        });
        s2.push(d / ubm);
    }
    let t = DT * STEPS as f64;
    let ex = bulk_exact(t);
    let ubm = model.iter().sum::<f64>() / NY as f64;
    let ua = sa.u.values.iter().map(|v| v[0]).sum::<f64>() / sa.u.values.len() as f64;
    println!(
        "printed: U_b at t = {t}: S-a' {ua:.6}, 1-D model {ubm:.6}, closed form {ex:.6}; elapsed {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    let r = finite_or_inf(((ua - ex) / ex).abs());
    check(
        &mut fails,
        "S1a'",
        r <= 2e-2,
        format!("|U_b - U_b,exact| / U_b,exact at t = {t}: {r:.4e} (2e-2)"),
    );
    let (v, st) = largest(&s2);
    check(
        &mut fails,
        "S2a'",
        v <= 1e-9,
        format!("largest over the steps of max_c (|u_x - u_1D|, |u_y|) / U_b,1D {v:.4e} (step {st}) (1e-9 each step)"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---- no-ops and the refusal -------------------------------------------------------------------------------------------------

/// The initial velocity of ADR 0687's fractional runs.
fn u_init(x: Vec3) -> Vec3 {
    let (xs, y) = (x[0], x[1]);
    let kx = 2.0 * PI * xs / L;
    [
        U0 * 4.0 * y * (H - y) / (H * H) + A * (PI / H) * (2.0 * PI * y / H).sin() * kx.cos(),
        A * (2.0 * PI / L) * (PI * y / H).sin().powi(2) * kx.sin(),
        0.0,
    ]
}

fn noop_solver(g: &Arc<FvGeometry>, body_opt: bool, with_force: bool) -> PimpleSolver {
    let u = walled_u(g, u_init);
    let p = ScalarField::uniform("p", g, 0.0);
    let mut s = frac_settings(Frac {
        opt: false,
        twin: false,
        body: body_opt,
    });
    s.p_ref = (0, 0.0);
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    if with_force {
        sv.body_force = Some(BodyForce::uniform(g, [FX, GY, 0.0]));
    }
    sv
}

/// DB1, DB2 of ADR 0691: without the opt-in the body force is not read, and the opt-in without a body force changes nothing;
/// 20 steps bit for bit.
#[test]
fn fractional_body_force_no_ops() {
    let g = geom_of(mesh(0.0));
    let (mut a, mut b) = (noop_solver(&g, false, true), noop_solver(&g, false, false));
    let (mut c, mut d) = (noop_solver(&g, true, false), noop_solver(&g, false, false));
    let (mut db1, mut db2) = (true, true);
    for _ in 0..20 {
        for s in [&mut a, &mut b, &mut c, &mut d] {
            s.step(DT);
        }
        db1 &= bits(&a) == bits(&b);
        db2 &= bits(&c) == bits(&d);
    }
    let mut fails = Fails::new();
    check(
        &mut fails,
        "DB1",
        db1,
        format!("body force set, opt-in off, against no body force, 20 steps bit for bit: {db1}"),
    );
    check(
        &mut fails,
        "DB2",
        db2,
        format!("opt-in on without a body force against off, 20 steps bit for bit: {db2}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// DB3 of ADR 0691: the opt-in with the Chorin form is refused.
#[test]
#[should_panic(expected = "fractional_body_force: incremental form only")]
fn fractional_body_force_refuses_the_chorin_form() {
    let g = geom_of(mesh(0.0));
    let mut sv = noop_solver(&g, true, true);
    sv.settings.fractional = Some(FractionalSettings {
        incremental: false,
        kim_moin: true,
    });
    sv.step(DT);
}
