//! F-752 (ADR 0672): the pressure part of the non-orthogonal correction on coupled faces (ADR 0667's
//! `PimpleSettings::coupled_non_orth`) on a skewed translational periodic channel `P` against its unrolled twin `T` (the same
//! cells with the periodic cut moved to where the faces are orthogonal), under a body force across the channel that gives a
//! tangential pressure gradient on every coupled face. Variants: the full setting `F`, the setting without its pressure part
//! `M` (`coupled_non_orth_without_pressure`, the control), the setting off `O`, the new field alone `X`. Checks P1, NC1, P2,
//! D1L, D2, D3, G0 (the lockstep, 200 steps), D1 (the converged flux, 5 steps), H1 and G0 (the meshes). The limits are those
//! registered in ADR 0672 before this code ran; each test prints every measured number and then asserts the checks it
//! carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{divergence, CoupledNonOrth};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, warp};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const L: f64 = 2.0;
const H: f64 = 1.0;
const NX: usize = 32;
const NY: usize = 16;
/// The tilt of the lines `x = const` at the cut of `P`.
const TAU: f64 = 0.3;
const NU: f64 = 0.1;
/// The body acceleration: `FX` drives the start-up of plane Poiseuille flow, `GY` the hydrostatic pressure across.
const FX: f64 = 0.8;
const GY: f64 = -2.0;
/// The steady Poiseuille centreline `FX H^2 / (8 NU)`.
const U_REF: f64 = 1.0;
const DT: f64 = 0.01;
const STEPS: usize = 200;
const PASSES: usize = 6;
const D1_STEPS: usize = 5;
const D1_PASSES: usize = 100;
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// The tilt `t(x)`: `TAU cos^2(2 pi s / L)` for `s = x mod L` within `L / 4` of the cut, zero in the middle half.
fn tilt(x: f64) -> f64 {
    let s = x.rem_euclid(L);
    if s <= 0.25 * L || s >= 0.75 * L {
        TAU * (2.0 * PI * s / L).cos().powi(2)
    } else {
        0.0
    }
}

/// `P` (`shift = 0`) or `T` (`shift = L / 2`): the periodic box with `x' = x + t(x + shift) (y - H / 2)`.
fn mesh(shift: f64) -> Mesh {
    let m = box_2d(NX, NY, L, H, true, false).unwrap();
    warp(&m, |p| [p[0] + tilt(p[0] + shift) * (p[1] - 0.5 * H), p[1], p[2]]).unwrap()
}

fn geom_of(m: Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// The image in `T` of a point of `P`: translated by `-L / 2` modulo `L`.
fn image(x: Vec3) -> Vec3 {
    [(x[0] - 0.5 * L).rem_euclid(L), x[1], x[2]]
}

/// The hydrostatic start pressure.
fn p0(x: Vec3) -> f64 {
    GY * (x[1] - 0.5 * H)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// ADR 0667's setting.
    F,
    /// The setting without its pressure part (F-752's control).
    M,
    /// The setting off.
    O,
    /// The new field alone (no effect without `coupled_non_orth`).
    X,
}

fn settings(mode: Mode, passes: usize) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.n_outer = 1;
    s.n_correctors = 2;
    s.non_orth_correctors = passes;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s.ddt_correction = false;
    s.coupled_body_pair_mean = true;
    s.coupled_non_orth = matches!(mode, Mode::F | Mode::M);
    s.coupled_non_orth_without_pressure = matches!(mode, Mode::M | Mode::X);
    s
}

fn fields(g: &FvGeometry) -> (VectorField, ScalarField) {
    let mut u = VectorField::uniform("U", g, [0.0; 3]);
    for w in ["ymin", "ymax"] {
        u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    (u, ScalarField::from_fn("p", g, p0))
}

fn solver(g: &Arc<FvGeometry>, mode: Mode, passes: usize, ref_cell: usize) -> PimpleSolver {
    let (u, p) = fields(g);
    let mut s = settings(mode, passes);
    s.p_ref = (ref_cell, p0(g.mesh.cell_centres()[ref_cell]));
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    sv.body_force = Some(BodyForce::uniform(g, [FX, GY, 0.0]));
    sv
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

/// The meshes and the cell map `m(c)` of `P` onto `T` (G0).
struct Pair {
    gp: Arc<FvGeometry>,
    gt: Arc<FvGeometry>,
    m: Vec<usize>,
    match_d: f64,
    match_one: bool,
}

fn pair() -> Pair {
    let gp = geom_of(mesh(0.0));
    let gt = geom_of(mesh(0.5 * L));
    let imgs: Vec<Vec3> = gp.mesh.cell_centres().iter().map(|&c| image(c)).collect();
    let (m, match_d, match_one) = match_points(&imgs, gt.mesh.cell_centres());
    Pair {
        gp,
        gt,
        m,
        match_d,
        match_one,
    }
}

/// The scale of the per-cell divergence, `U_ref h dz` (`h = H / NY`, the depth `dz = L / NX` of `box_2d`).
fn div_scale() -> f64 {
    U_REF * (H / NY as f64) * (L / NX as f64)
}

fn max_abs(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |a, v| a.max(finite_or_inf(v.abs())))
}

/// `max_c |a_c - b_m(c)|` of the velocity.
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

fn smallest(x: &[f64]) -> (f64, usize) {
    x.iter().enumerate().fold((f64::INFINITY, 0), |b, (s, &v)| {
        let v = if v.is_nan() { 0.0 } else { v };
        if v < b.0 {
            (v, s + 1)
        } else {
            b
        }
    })
}

/// P1, NC1, P2, D1L, D2, D3, G0 of ADR 0672: the lockstep of `P_F`, `P_M`, `P_O`, `P_X`, `T_F`, `T_O` over 200 steps.
#[test]
fn pressure_part_lockstep_against_the_twin() {
    let t0 = Instant::now();
    let pr = pair();
    let (rp, rt) = (0, pr.m[0]);
    let mut pf = solver(&pr.gp, Mode::F, PASSES, rp);
    let mut pm = solver(&pr.gp, Mode::M, PASSES, rp);
    let mut po = solver(&pr.gp, Mode::O, PASSES, rp);
    let mut px = solver(&pr.gp, Mode::X, PASSES, rp);
    let mut tf = solver(&pr.gt, Mode::F, PASSES, rt);
    let mut to = solver(&pr.gt, Mode::O, PASSES, rt);
    let (mut g_f, mut g_m, mut g_o, mut p2, mut d1l, mut d2) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let (mut abs_div, mut pm_p) = ([0.0f64; 6], 0.0f64);
    let mut d3 = true;
    let sc = div_scale();
    for _ in 0..STEPS {
        for s in [&mut pf, &mut pm, &mut po, &mut px, &mut tf, &mut to] {
            s.step(DT);
        }
        let m = &pr.m;
        g_f.push(vdiff(&pf.u.values, &tf.u.values, m) / U_REF);
        g_m.push(vdiff(&pm.u.values, &tf.u.values, m) / U_REF);
        g_o.push(vdiff(&po.u.values, &tf.u.values, m) / U_REF);
        p2.push(sdiff(&pf.p.values, &tf.p.values, m) / (GY.abs() * H));
        pm_p = pm_p.max(sdiff(&pm.p.values, &tf.p.values, m));
        let (dvp, dvt) = (divergence(&pr.gp, &pf.phi), divergence(&pr.gt, &tf.phi));
        d1l.push(sdiff(&dvp, &dvt, m) / sc);
        let ident: Vec<usize> = (0..tf.u.values.len()).collect();
        d2.push(vdiff(&tf.u.values, &to.u.values, &ident) / U_REF);
        d3 &= bits(&px) == bits(&po);
        for (i, (s, g)) in [
            (&pf, &pr.gp),
            (&pm, &pr.gp),
            (&po, &pr.gp),
            (&px, &pr.gp),
            (&tf, &pr.gt),
            (&to, &pr.gt),
        ]
        .iter()
        .enumerate()
        {
            abs_div[i] = abs_div[i].max(max_abs(&divergence(g, &s.phi)) / sc);
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    for (name, g) in [("G_F", &g_f), ("G_M", &g_m), ("G_O", &g_o)] {
        let (mx, st) = largest(g);
        println!(
            "{name}: step 1 {:.4e}, largest {mx:.4e} U_ref at step {st}, last {:.4e}",
            g[0],
            g[STEPS - 1]
        );
    }
    println!(
        "largest per-cell divergence / (U_ref h dz) over the steps: P_F {:.3e}, P_M {:.3e}, P_O {:.3e}, P_X {:.3e}, T_F {:.3e}, \
         T_O {:.3e}",
        abs_div[0], abs_div[1], abs_div[2], abs_div[3], abs_div[4], abs_div[5]
    );
    println!("largest |p_PM - p_TF| over the steps {pm_p:.4e} (the control's pressure defect)");
    let ux = pf.u.values.iter().fold(0.0f64, |a, v| a.max(v[0]));
    println!("largest u_x of P_F at the last step {ux:.4} U_ref; elapsed {elapsed:.1} s");
    let mut fails = Fails::new();
    let (n1, s1) = largest(&g_f);
    check(
        &mut fails,
        "P1",
        n1 <= 1e-8,
        format!("max over {STEPS} steps of G_F(n) {n1:.4e} U_ref (step {s1}) (1e-8 each step)"),
    );
    let (nc, sn) = smallest(&g_m);
    check(
        &mut fails,
        "NC1",
        nc >= 1e-4,
        format!("control: min over {STEPS} steps of G_M(n) {nc:.4e} U_ref (step {sn}) (>= 1e-4 each step)"),
    );
    let (pp, sp) = largest(&p2);
    check(
        &mut fails,
        "P2",
        pp <= 1e-8,
        format!("max over the steps of |p_PF - p_TF| / (|g_y| H) {pp:.4e} (step {sp}) (1e-8 each step)"),
    );
    let (dl, sl) = largest(&d1l);
    check(
        &mut fails,
        "D1L",
        dl <= 1e-9,
        format!("max over the steps of |div_PF - div_TF| / (U_ref h dz) {dl:.4e} (step {sl}) (1e-9 each step)"),
    );
    let (dd, sd) = largest(&d2);
    check(
        &mut fails,
        "D2",
        dd <= 1e-10,
        format!("max over the steps of |u_TF - u_TO| / U_ref {dd:.4e} (step {sd}) (1e-10 each step)"),
    );
    check(
        &mut fails,
        "D3",
        d3,
        format!("P_X (the new field alone) equals P_O bit for bit on every step: {d3}"),
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

/// D1 of ADR 0672: the corrected flux of `P_F` with the explicit iteration converged (100 passes) is divergence-free per cell;
/// `P_M` printed (the default pressure pair is consistent too).
#[test]
fn converged_corrected_flux_is_divergence_free() {
    let t0 = Instant::now();
    let pr = pair();
    let sc = div_scale();
    let mut out = [0.0f64; 2];
    for (i, mode) in [Mode::F, Mode::M].into_iter().enumerate() {
        let mut s = solver(&pr.gp, mode, D1_PASSES, 0);
        for _ in 0..D1_STEPS {
            s.step(DT);
            out[i] = out[i].max(max_abs(&divergence(&pr.gp, &s.phi)) / sc);
        }
    }
    println!(
        "D1: largest per-cell divergence / (U_ref h dz) over {D1_STEPS} steps with {D1_PASSES} passes: P_F {:.3e}, P_M {:.3e}; \
         elapsed {:.1} s",
        out[0],
        out[1],
        t0.elapsed().as_secs_f64()
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "D1",
        out[0] <= 1e-10,
        format!("P_F converged flux, max_c |div| / (U_ref h dz) {:.3e} (1e-10)", out[0]),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// The non-orthogonal remainder `S - d |S|^2 / (S . d)`.
fn k_of(s: Vec3, d: Vec3) -> Vec3 {
    vec3::sub(s, vec3::scale(d, vec3::dot(s, s) / vec3::dot(s, d)))
}

/// H1 and G0 of ADR 0672: the setting applies to `P`'s translational pairs, every coupled face of `P` is an internal face of
/// `T` with the same `S`, `d`, `k`, and `T`'s own coupled faces are orthogonal.
#[test]
fn h1_coupled_faces_are_the_twin_faces() {
    let pr = pair();
    let (gp, gt) = (&pr.gp, &pr.gt);
    let (u, p) = fields(gp);
    let cn_u = CoupledNonOrth::build(gp, |pi| u.bc(pi).is_coupled());
    let cn_p = CoupledNonOrth::build(gp, |pi| p.bc(pi).is_coupled());
    let translational =
        gp.periodic_rot.iter().all(|r| r.is_none()) && gp.periodic_partner.iter().filter(|m| m.is_some()).count() == 2;
    let (mp, mt) = (&gp.mesh, &gt.mesh);
    let fa: Vec<Vec3> = mt.face_centres()[..gt.n_internal].to_vec();
    let (st, ot, nt) = (mt.face_area_vectors(), mt.owner(), mt.neighbour());
    let (mut worst_c, mut worst_s, mut worst_d, mut worst_k, mut kmax) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut unmatched, mut misoriented) = (0usize, 0usize);
    let same_lists = cn_u.geometry().iter().map(|e| e.0).collect::<Vec<_>>()
        == cn_p.geometry().iter().map(|e| e.0).collect::<Vec<_>>();
    for (f, d, k) in cn_u.geometry() {
        let (idx, dist, _) = match_points(&[image(mp.face_centres()[f])], &fa);
        worst_c = worst_c.max(dist);
        if dist > 1e-9 {
            unmatched += 1;
            continue;
        }
        let h = idx[0];
        let r = pr.m[mp.owner()[f]];
        let sign = if ot[h] == r {
            1.0
        } else if nt[h] == r {
            -1.0
        } else {
            misoriented += 1;
            continue;
        };
        let s_t = vec3::scale(st[h], sign);
        let d_t = vec3::scale(gt.delta[h], sign);
        let k_t = k_of(s_t, d_t);
        let s_p = mp.face_area_vectors()[f];
        worst_s = worst_s.max(vec3::norm(vec3::sub(s_p, s_t)));
        worst_d = worst_d.max(vec3::norm(vec3::sub(d, d_t)));
        worst_k = worst_k.max(vec3::norm(vec3::sub(k, k_t)));
        kmax = kmax.max(vec3::norm(k) / vec3::norm(s_p));
    }
    let cn_t = CoupledNonOrth::build(gt, |pi| fields(gt).0.bc(pi).is_coupled());
    let kt = cn_t
        .geometry()
        .iter()
        .map(|e| vec3::norm(e.2) / vec3::norm(mt.face_area_vectors()[e.0]))
        .fold(0.0f64, f64::max);
    println!(
        "H1: P velocity {} and pressure {} coupled faces (same list {same_lists}), translational {translational}; centre \
         {worst_c:.2e}, unmatched {unmatched}, misoriented {misoriented}; S {worst_s:.2e}, d {worst_d:.2e}, k {worst_k:.2e}; \
         largest |k| / |S| of P {kmax:.5}; of T's {} coupled faces {kt:.2e}",
        cn_u.geometry().len(),
        cn_p.geometry().len(),
        cn_t.geometry().len()
    );
    let mut fails = Fails::new();
    let n_ok = cn_u.geometry().len() == 2 * NY && cn_p.geometry().len() == 2 * NY && same_lists;
    check(
        &mut fails,
        "H1",
        n_ok && translational
            && unmatched == 0
            && misoriented == 0
            && worst_c <= 1e-12
            && worst_s <= 1e-12
            && worst_d <= 1e-12
            && worst_k <= 1e-12
            && (0.25..=0.35).contains(&kmax)
            && kt <= 1e-12,
        format!(
            "{} and {} faces (32), translational {translational}; centre {worst_c:.2e}, S {worst_s:.2e}, d {worst_d:.2e}, k \
             {worst_k:.2e} (1e-12 each), {unmatched} unmatched, {misoriented} misoriented; P |k|/|S| {kmax:.4} ([0.25, \
             0.35]); T {kt:.2e} (1e-12)",
            cn_u.geometry().len(),
            cn_p.geometry().len()
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
