//! F-753 (ADR 0674): the pressure part of the non-orthogonal correction on coupled faces (ADR 0667's
//! `PimpleSettings::coupled_non_orth`) on rotational pairs: a warped 60-degree sector `S` (wrap faces tilted by `0.3 r / r_m`)
//! against its unrolled full annulus `A` (six rotated copies of the same cells, no coupled face), under a balanced body force
//! `grad phi`, `phi = (G r_m / 6) (r / r_m)^6 cos 6 theta` (a radial, i.e. face-tangential, pressure gradient on the wrap faces)
//! and an axisymmetric swirl driver `F0 theta_hat`. Variants: the full setting `F`, the setting without its pressure part `M`
//! (ADR 0672's `coupled_non_orth_without_pressure`, the control), the setting off `O`, the control field alone `X`. Checks P1,
//! NC1, P2, D1L, D2, D3, G0 (the lockstep, 200 steps), D1 (the converged flux, 5 steps), H1, H3 and G0 (the meshes). The limits
//! are those registered in ADR 0674 before this code ran; each test prints every measured number and then asserts the checks it
//! carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{divergence, CoupledNonOrth};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, cylinder_ogrid, warp, Grading};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const ALPHA: f64 = FRAC_PI_3;
const RI: f64 = 1.0;
const RO: f64 = 1.5;
const RM: f64 = 1.25;
const DZ: f64 = 0.1;
const NT: usize = 12;
const NR: usize = 8;
const COPIES: usize = 6;
/// The tilt of the lines `theta = const` at the wrap (as a tangential slope at `r_m`).
const TAU: f64 = 0.3;
const NU: f64 = 0.1;
/// The balanced potential's scale: `|grad phi| = G (r / r_m)^5`.
const G: f64 = 2.0;
/// The swirl driver `F0 theta_hat`.
const F0: f64 = 3.2;
/// The plane estimate of the gap centre speed `F0 h^2 / (8 nu)`.
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

/// The tilt `t(theta)`: `TAU cos^2(2 pi s / alpha)` for `s = theta mod alpha` within `alpha / 4` of the wrap, zero otherwise.
fn tilt(theta: f64) -> f64 {
    let s = theta.rem_euclid(ALPHA);
    if s <= 0.25 * ALPHA || s >= 0.75 * ALPHA {
        TAU * (2.0 * PI * s / ALPHA).cos().powi(2)
    } else {
        0.0
    }
}

/// The map `W`: `theta' = theta + t(theta) (r - r_m) / r_m`.
fn w_map(p: Vec3) -> Vec3 {
    let (r, th) = (p[0].hypot(p[1]), p[1].atan2(p[0]));
    let t = th + tilt(th) * (r - RM) / RM;
    [r * t.cos(), r * t.sin(), p[2]]
}

fn sector() -> Mesh {
    warp(
        &annulus_sector(NT, NR, RI, RO, ALPHA, Grading::Uniform, DZ).unwrap(),
        w_map,
    )
    .unwrap()
}

fn annulus() -> Mesh {
    warp(
        &cylinder_ogrid(COPIES * NT, NR, RI, RO, Grading::Uniform, DZ).unwrap(),
        w_map,
    )
    .unwrap()
}

fn geom_of(m: Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

fn polar(x: Vec3) -> (f64, f64, Vec3, Vec3) {
    let (r, th) = (x[0].hypot(x[1]), x[1].atan2(x[0]));
    (r, th, [th.cos(), th.sin(), 0.0], [-th.sin(), th.cos(), 0.0])
}

/// The balanced potential.
fn phi(x: Vec3) -> f64 {
    let (r, th, _, _) = polar(x);
    G * RM / 6.0 * (r / RM).powi(6) * (6.0 * th).cos()
}

/// `grad phi = G (r / r_m)^5 (cos 6 theta r_hat - sin 6 theta theta_hat)`.
fn grad_phi(x: Vec3) -> Vec3 {
    let (r, th, rh, th_h) = polar(x);
    let a = G * (r / RM).powi(5);
    vec3::add(
        vec3::scale(rh, a * (6.0 * th).cos()),
        vec3::scale(th_h, -a * (6.0 * th).sin()),
    )
}

/// The body acceleration `grad phi + F0 theta_hat`.
fn accel(x: Vec3) -> Vec3 {
    let (_, _, _, th_h) = polar(x);
    vec3::add(grad_phi(x), vec3::scale(th_h, F0))
}

fn body_force(g: &FvGeometry) -> BodyForce {
    let m = &g.mesh;
    BodyForce {
        cell: m.cell_centres().iter().map(|&c| accel(c)).collect(),
        face: m
            .face_centres()
            .iter()
            .zip(m.face_area_vectors())
            .map(|(&x, s)| vec3::dot(accel(x), *s))
            .collect(),
    }
}

/// The rotation by `j alpha` about `z`.
fn rot_j(j: usize) -> [[f64; 3]; 3] {
    let a = j as f64 * ALPHA;
    [[a.cos(), -a.sin(), 0.0], [a.sin(), a.cos(), 0.0], [0.0, 0.0, 1.0]]
}

fn apply(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// ADR 0667's setting.
    F,
    /// The setting without its pressure part (ADR 0672's control).
    M,
    /// The setting off.
    O,
    /// The control field alone (no effect without `coupled_non_orth`).
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
    s.rotational_periodic_implicit = true;
    s.coupled_non_orth = matches!(mode, Mode::F | Mode::M);
    s.coupled_non_orth_without_pressure = matches!(mode, Mode::M | Mode::X);
    s
}

/// The wall patches of `S` or `A`.
fn walls(g: &FvGeometry) -> [&'static str; 2] {
    if g.mesh.patch("inner").is_some() {
        ["inner", "outer"]
    } else {
        ["cylinder", "farfield"]
    }
}

fn fields(g: &FvGeometry) -> (VectorField, ScalarField) {
    let mut u = VectorField::uniform("U", g, [0.0; 3]);
    for w in walls(g) {
        u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    (u, ScalarField::from_fn("p", g, phi))
}

fn solver(g: &Arc<FvGeometry>, mode: Mode, passes: usize, ref_cell: usize) -> PimpleSolver {
    let (u, p) = fields(g);
    let mut s = settings(mode, passes);
    s.p_ref = (ref_cell, phi(g.mesh.cell_centres()[ref_cell]));
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    sv.body_force = Some(body_force(g));
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

/// The meshes and the six cell maps `m_j(c)` of `S` onto `A` (G0).
struct Pair {
    gs: Arc<FvGeometry>,
    ga: Arc<FvGeometry>,
    maps: Vec<Vec<usize>>,
    match_d: f64,
    each_one: bool,
    bijective: bool,
}

fn pair() -> Pair {
    let gs = geom_of(sector());
    let ga = geom_of(annulus());
    let (mut maps, mut match_d, mut each_one) = (Vec::new(), 0.0f64, true);
    for j in 0..COPIES {
        let r = rot_j(j);
        let imgs: Vec<Vec3> = gs.mesh.cell_centres().iter().map(|&c| apply(&r, c)).collect();
        let (m, d, one) = match_points(&imgs, ga.mesh.cell_centres());
        match_d = match_d.max(d);
        each_one &= one;
        maps.push(m);
    }
    let all: std::collections::HashSet<usize> = maps.iter().flatten().copied().collect();
    let bijective = all.len() == ga.n_cells() && maps.iter().map(|m| m.len()).sum::<usize>() == ga.n_cells();
    Pair {
        gs,
        ga,
        maps,
        match_d,
        each_one,
        bijective,
    }
}

/// The scale of the per-cell divergence, `U_ref dr dz`.
fn div_scale() -> f64 {
    U_REF * ((RO - RI) / NR as f64) * DZ
}

fn max_abs(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |a, v| a.max(finite_or_inf(v.abs())))
}

/// `max_j max_c |R_j a_c - b_{m_j(c)}|` of the velocity.
fn vdiff(a: &[Vec3], b: &[Vec3], maps: &[Vec<usize>]) -> (f64, usize) {
    let mut out = (0.0f64, 0);
    for (j, m) in maps.iter().enumerate() {
        let r = rot_j(j);
        for (c, v) in a.iter().enumerate() {
            let e = finite_or_inf(vec3::norm(vec3::sub(apply(&r, *v), b[m[c]])));
            if e > out.0 {
                out = (e, j);
            }
        }
    }
    out
}

/// `max_j max_c |a_c - b_{m_j(c)}|` of a scalar.
fn sdiff(a: &[f64], b: &[f64], maps: &[Vec<usize>]) -> f64 {
    maps.iter().fold(0.0f64, |w, m| {
        a.iter()
            .enumerate()
            .fold(w, |w, (c, v)| w.max(finite_or_inf((v - b[m[c]]).abs())))
    })
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

/// P1, NC1, P2, D1L, D2, D3, G0 of ADR 0674: the lockstep of `S_F`, `S_M`, `S_O`, `S_X`, `A_F`, `A_O` over 200 steps.
#[test]
fn pressure_part_on_rotational_pairs_lockstep_against_the_annulus() {
    let t0 = Instant::now();
    let pr = pair();
    let (rs, ra) = (0, pr.maps[0][0]);
    let mut sf = solver(&pr.gs, Mode::F, PASSES, rs);
    let mut sm = solver(&pr.gs, Mode::M, PASSES, rs);
    let mut so = solver(&pr.gs, Mode::O, PASSES, rs);
    let mut sx = solver(&pr.gs, Mode::X, PASSES, rs);
    let mut af = solver(&pr.ga, Mode::F, PASSES, ra);
    let mut ao = solver(&pr.ga, Mode::O, PASSES, ra);
    let (mut g_f, mut g_m, mut g_o, mut p2, mut d1l, mut d2) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let (mut abs_div, mut sm_p, mut gf_copy) = ([0.0f64; 6], 0.0f64, (0.0f64, 0usize));
    let (mut d3, mut d2_bits) = (true, true);
    let sc = div_scale();
    let maps = &pr.maps;
    for _ in 0..STEPS {
        for s in [&mut sf, &mut sm, &mut so, &mut sx, &mut af, &mut ao] {
            s.step(DT);
        }
        let gf = vdiff(&sf.u.values, &af.u.values, maps);
        if gf.0 > gf_copy.0 {
            gf_copy = gf;
        }
        g_f.push(gf.0 / U_REF);
        g_m.push(vdiff(&sm.u.values, &af.u.values, maps).0 / U_REF);
        g_o.push(vdiff(&so.u.values, &af.u.values, maps).0 / U_REF);
        p2.push(sdiff(&sf.p.values, &af.p.values, maps) / (G * RM));
        sm_p = sm_p.max(sdiff(&sm.p.values, &af.p.values, maps));
        let (dvs, dva) = (divergence(&pr.gs, &sf.phi), divergence(&pr.ga, &af.phi));
        d1l.push(sdiff(&dvs, &dva, maps) / sc);
        let d2v =
            af.u.values
                .iter()
                .zip(&ao.u.values)
                .fold(0.0f64, |w, (a, b)| w.max(finite_or_inf(vec3::norm(vec3::sub(*a, *b)))));
        d2.push(d2v / U_REF);
        d2_bits &= bits(&af) == bits(&ao);
        d3 &= bits(&sx) == bits(&so);
        for (i, (s, g)) in [
            (&sf, &pr.gs),
            (&sm, &pr.gs),
            (&so, &pr.gs),
            (&sx, &pr.gs),
            (&af, &pr.ga),
            (&ao, &pr.ga),
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
    println!("G_F largest in copy {}", gf_copy.1);
    println!(
        "largest per-cell divergence / (U_ref dr dz) over the steps: S_F {:.3e}, S_M {:.3e}, S_O {:.3e}, S_X {:.3e}, A_F {:.3e}, \
         A_O {:.3e}",
        abs_div[0], abs_div[1], abs_div[2], abs_div[3], abs_div[4], abs_div[5]
    );
    println!("largest |p_SM - p_AF| over the steps {sm_p:.4e} (the control's pressure defect); A_F equals A_O bit for bit: {d2_bits}");
    let umax = sf.u.values.iter().fold(0.0f64, |a, v| a.max(vec3::norm(*v)));
    println!("largest |u| of S_F at the last step {umax:.4} U_ref; elapsed {elapsed:.1} s");
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
        format!("max over the steps of |p_SF - p_AF| / (G r_m) {pp:.4e} (step {sp}) (1e-8 each step)"),
    );
    let (dl, sl) = largest(&d1l);
    check(
        &mut fails,
        "D1L",
        dl <= 1e-9,
        format!("max over the steps of |div_SF - div_AF| / (U_ref dr dz) {dl:.4e} (step {sl}) (1e-9 each step)"),
    );
    let (dd, sd) = largest(&d2);
    check(
        &mut fails,
        "D2",
        dd <= 1e-10,
        format!("max over the steps of |u_AF - u_AO| / U_ref {dd:.4e} (step {sd}) (1e-10 each step)"),
    );
    check(
        &mut fails,
        "D3",
        d3,
        format!("S_X (the control field alone) equals S_O bit for bit on every step: {d3}"),
    );
    check(
        &mut fails,
        "G0",
        pr.match_d <= 1e-12 && pr.each_one && pr.bijective,
        format!(
            "nearest A cell of every rotated S cell {:.2e} (1e-12), each copy one-to-one {}, a bijection onto A {}",
            pr.match_d, pr.each_one, pr.bijective
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// D1 of ADR 0674: the corrected flux of `S_F` with the explicit iteration converged (100 passes) is divergence-free per cell;
/// `S_M` printed.
#[test]
fn converged_corrected_flux_on_the_sector_is_divergence_free() {
    let t0 = Instant::now();
    let gs = geom_of(sector());
    let sc = div_scale();
    let mut out = [0.0f64; 2];
    for (i, mode) in [Mode::F, Mode::M].into_iter().enumerate() {
        let mut s = solver(&gs, mode, D1_PASSES, 0);
        for _ in 0..D1_STEPS {
            s.step(DT);
            out[i] = out[i].max(max_abs(&divergence(&gs, &s.phi)) / sc);
        }
    }
    println!(
        "D1: largest per-cell divergence / (U_ref dr dz) over {D1_STEPS} steps with {D1_PASSES} passes: S_F {:.3e}, S_M {:.3e}; \
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
        format!("S_F converged flux, max_c |div| / (U_ref dr dz) {:.3e} (1e-10)", out[0]),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// The non-orthogonal remainder `S - d |S|^2 / (S . d)`.
fn k_of(s: Vec3, d: Vec3) -> Vec3 {
    vec3::sub(s, vec3::scale(d, vec3::dot(s, s) / vec3::dot(s, d)))
}

/// H1, H3 and G0 of ADR 0674: the sector's wrap faces are rotational pairs by `alpha`, each an internal face of the annulus with
/// the same centre, `S`, `d`, `k`; the rotation carries a registered share of the pressure part's face term on `p0 = phi`.
#[test]
fn h1_h3_wrap_faces_are_the_annulus_faces_and_the_rotation_matters() {
    let pr = pair();
    let (gs, ga) = (&pr.gs, &pr.ga);
    let (u, p) = fields(gs);
    let cn_u = CoupledNonOrth::build(gs, |pi| u.bc(pi).is_coupled());
    let cn_p = CoupledNonOrth::build(gs, |pi| p.bc(pi).is_coupled());
    let same_lists = cn_u.geometry().iter().map(|e| e.0).collect::<Vec<_>>()
        == cn_p.geometry().iter().map(|e| e.0).collect::<Vec<_>>();
    let (ms, ma) = (&gs.mesh, &ga.mesh);
    let (own, fc, sf, cc) = (ms.owner(), ms.face_centres(), ms.face_area_vectors(), ms.cell_centres());
    let nint = gs.n_internal;
    // the rotation of every pair, and H3's share on `p0 = phi`
    let (ca, sa) = (ALPHA.cos(), ALPHA.sin());
    let (mut worst_r, mut worst_map, mut n_rot) = (0.0f64, 0.0f64, 0usize);
    let (mut rho_min, mut rho_max) = (f64::INFINITY, 0.0f64);
    let kmap: std::collections::HashMap<usize, Vec3> = cn_p.geometry().iter().map(|e| (e.0, e.2)).collect();
    for (pi, patch) in ms.patches().iter().enumerate() {
        let Some(map) = &gs.periodic_partner[pi] else {
            continue;
        };
        let Some(r) = gs.periodic_rot[pi] else {
            continue;
        };
        n_rot += 1;
        let e = [
            (r[2][2] - 1.0).abs(),
            (r[0][0] - ca).abs(),
            (r[1][1] - ca).abs(),
            (r[0][1].abs() - sa).abs(),
            (r[1][0].abs() - sa).abs(),
            r[0][2].abs(),
            r[1][2].abs(),
            r[2][0].abs(),
            r[2][1].abs(),
        ];
        worst_r = e.iter().fold(worst_r, |a, &x| a.max(x));
        for (local, f) in patch.faces().enumerate() {
            if gs.empty[f] {
                continue;
            }
            let g = map[local];
            worst_map = worst_map.max(vec3::norm(vec3::sub(apply(&r, fc[g]), fc[f])));
            let Some(&k) = kmap.get(&f) else {
                continue;
            };
            let (dp, dq) = (gs.dn[f - nint], gs.dn[g - nint]);
            let w = dq / (dp + dq);
            let gc = grad_phi(cc[own[g]]);
            let share = vec3::scale(vec3::sub(apply(&r, gc), gc), 1.0 - w);
            let rho = vec3::dot(k, share).abs() / vec3::dot(k, grad_phi(fc[f])).abs();
            rho_min = rho_min.min(rho);
            rho_max = rho_max.max(finite_or_inf(rho));
        }
    }
    // every wrap face an annulus internal face with the same centre, S, d, k
    let fa: Vec<Vec3> = ma.face_centres()[..ga.n_internal].to_vec();
    let (st, ot, nt) = (ma.face_area_vectors(), ma.owner(), ma.neighbour());
    let (mut worst_c, mut worst_s, mut worst_d, mut worst_k, mut kmax) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut unmatched, mut misoriented) = (0usize, 0usize);
    for (f, d, k) in cn_u.geometry() {
        let (idx, dist, _) = match_points(&[fc[f]], &fa);
        worst_c = worst_c.max(dist);
        if dist > 1e-9 {
            unmatched += 1;
            continue;
        }
        let h = idx[0];
        let r = pr.maps[0][own[f]];
        let sign = if ot[h] == r {
            1.0
        } else if nt[h] == r {
            -1.0
        } else {
            misoriented += 1;
            continue;
        };
        let s_a = vec3::scale(st[h], sign);
        let d_a = vec3::scale(ga.delta[h], sign);
        let k_a = k_of(s_a, d_a);
        worst_s = worst_s.max(vec3::norm(vec3::sub(sf[f], s_a)));
        worst_d = worst_d.max(vec3::norm(vec3::sub(d, d_a)));
        worst_k = worst_k.max(vec3::norm(vec3::sub(k, k_a)));
        kmax = kmax.max(vec3::norm(k) / vec3::norm(sf[f]));
    }
    let n_a = CoupledNonOrth::build(ga, |pi| fields(ga).0.bc(pi).is_coupled())
        .geometry()
        .len();
    println!(
        "H1: S velocity {} and pressure {} coupled faces (same list {same_lists}), {n_rot} rotational patches; R against a turn \
         by alpha {worst_r:.2e}, R x_g - x_f {worst_map:.2e}; centre {worst_c:.2e}, unmatched {unmatched}, misoriented \
         {misoriented}; S {worst_s:.2e}, d {worst_d:.2e}, k {worst_k:.2e}; largest |k| / |S| {kmax:.5}; A coupled faces {n_a}",
        cn_u.geometry().len(),
        cn_p.geometry().len(),
    );
    println!("H3: rho_f smallest {rho_min:.4}, largest {rho_max:.4}");
    let mut fails = Fails::new();
    let n_ok = cn_u.geometry().len() == 2 * NR && cn_p.geometry().len() == 2 * NR && same_lists && n_rot == 2;
    check(
        &mut fails,
        "H1",
        n_ok && unmatched == 0
            && misoriented == 0
            && worst_r <= 1e-12
            && worst_map <= 1e-12
            && worst_c <= 1e-12
            && worst_s <= 1e-12
            && worst_d <= 1e-12
            && worst_k <= 1e-12
            && (0.32..=0.39).contains(&kmax),
        format!(
            "{} and {} faces (16), {n_rot} rotational patches (2); R {worst_r:.2e}, R x_g - x_f {worst_map:.2e}, centre \
             {worst_c:.2e}, S {worst_s:.2e}, d {worst_d:.2e}, k {worst_k:.2e} (1e-12 each), {unmatched} unmatched, \
             {misoriented} misoriented; largest |k|/|S| {kmax:.4} ([0.32, 0.39])",
            cn_u.geometry().len(),
            cn_p.geometry().len()
        ),
    );
    check(
        &mut fails,
        "H3",
        rho_max >= 0.1,
        format!("largest share of the rotation in the pressure part's face term {rho_max:.4} (>= 0.1)"),
    );
    check(
        &mut fails,
        "G0",
        pr.match_d <= 1e-12 && pr.each_one && pr.bijective,
        format!(
            "nearest A cell of every rotated S cell {:.2e} (1e-12), each copy one-to-one {}, a bijection onto A {}",
            pr.match_d, pr.each_one, pr.bijective
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
