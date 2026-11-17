//! F-754 (ADR 0675): the transient Rhie-Chow (`ddt_correction`) term on coupled faces, the opt-in
//! `PimpleSettings::coupled_ddt_correction`. Binding case: ADR 0672's skewed periodic channel `P` against its unrolled twin `T`
//! with `ddt_correction` on (checks P1, NC1, P2, D1L, C1, D3, G0). Supporting case: ADR 0674's warped 60-degree sector `S`
//! against its unrolled annulus `A` with `ddt_correction` on (checks S1, SNC1, SD1L, D4, G0). The meshes, flows and settings
//! are those of ADR 0672 and ADR 0674 (copied here; those files are not edited). The limits are those registered in ADR 0675
//! before this code ran; each test prints every measured number and then asserts the checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::divergence;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, box_2d, cylinder_ogrid, warp, Grading};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const NU: f64 = 0.1;
const U_REF: f64 = 1.0;
const DT: f64 = 0.01;
const STEPS: usize = 200;
const PASSES: usize = 6;
const TAU: f64 = 0.3;
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// The settings of ADR 0672's / ADR 0674's full variant `F`, with `ddt_correction` and the new opt-in as given.
fn settings(ddt: bool, opt: bool, rotational: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.n_outer = 1;
    s.n_correctors = 2;
    s.non_orth_correctors = PASSES;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s.ddt_correction = ddt;
    s.coupled_body_pair_mean = true;
    s.rotational_periodic_implicit = rotational;
    s.coupled_non_orth = true;
    s.coupled_ddt_correction = opt;
    s
}

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

fn max_abs(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |a, v| a.max(finite_or_inf(v.abs())))
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

/// `max abs(phi_f + phi_g)` over the coupled pairs.
fn pair_sum(phi: &[f64], pairs: &[(usize, usize)]) -> f64 {
    pairs
        .iter()
        .fold(0.0f64, |w, &(f, g)| w.max(finite_or_inf((phi[f] + phi[g]).abs())))
}

/// `max abs(phi_h - U_h . S_h)` over the internal faces `faces` (the defect the next step's term reads).
fn defect_at(g: &FvGeometry, s: &PimpleSolver, faces: &[usize]) -> f64 {
    let zero_b = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
    let fo = face_flux(g, &s.u.values, &zero_b);
    faces
        .iter()
        .fold(0.0f64, |w, &h| w.max(finite_or_inf((s.phi[h] - fo[h]).abs())))
}

// ---- the channel (ADR 0672) ------------------------------------------------------------------------------------------------

mod channel {
    use super::*;

    pub const L: f64 = 2.0;
    pub const H: f64 = 1.0;
    pub const NX: usize = 32;
    pub const NY: usize = 16;
    pub const FX: f64 = 0.8;
    pub const GY: f64 = -2.0;

    fn tilt(x: f64) -> f64 {
        let s = x.rem_euclid(L);
        if s <= 0.25 * L || s >= 0.75 * L {
            TAU * (2.0 * PI * s / L).cos().powi(2)
        } else {
            0.0
        }
    }

    pub fn mesh(shift: f64) -> Mesh {
        let m = box_2d(NX, NY, L, H, true, false).unwrap();
        warp(&m, |p| [p[0] + tilt(p[0] + shift) * (p[1] - 0.5 * H), p[1], p[2]]).unwrap()
    }

    pub fn image(x: Vec3) -> Vec3 {
        [(x[0] - 0.5 * L).rem_euclid(L), x[1], x[2]]
    }

    fn p0(x: Vec3) -> f64 {
        GY * (x[1] - 0.5 * H)
    }

    pub fn solver(g: &Arc<FvGeometry>, ddt: bool, opt: bool, ref_cell: usize) -> PimpleSolver {
        let mut u = VectorField::uniform("U", g, [0.0; 3]);
        for w in ["ymin", "ymax"] {
            u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let p = ScalarField::from_fn("p", g, p0);
        let mut s = settings(ddt, opt, false);
        s.p_ref = (ref_cell, p0(g.mesh.cell_centres()[ref_cell]));
        let mut sv = PimpleSolver::new(g.clone(), u, p, s);
        sv.body_force = Some(BodyForce::uniform(g, [FX, GY, 0.0]));
        sv
    }

    pub fn div_scale() -> f64 {
        U_REF * (H / NY as f64) * (L / NX as f64)
    }

    pub fn vdiff(a: &[Vec3], b: &[Vec3], m: &[usize]) -> f64 {
        a.iter().enumerate().fold(0.0f64, |w, (c, v)| {
            w.max(finite_or_inf(vec3::norm(vec3::sub(*v, b[m[c]]))))
        })
    }

    pub fn sdiff(a: &[f64], b: &[f64], m: &[usize]) -> f64 {
        a.iter()
            .enumerate()
            .fold(0.0f64, |w, (c, v)| w.max(finite_or_inf((v - b[m[c]]).abs())))
    }
}

/// P1, NC1, P2, D1L, C1, D3, G0 of ADR 0675: the lockstep of `P_on`, `P_off`, `T_on`, `T_off`, `P_X`, `P_Y` over 200 steps.
#[test]
fn coupled_ddt_correction_channel_against_the_twin() {
    use channel::*;
    let t0 = Instant::now();
    let gp = geom_of(mesh(0.0));
    let gt = geom_of(mesh(0.5 * L));
    let imgs: Vec<Vec3> = gp.mesh.cell_centres().iter().map(|&c| image(c)).collect();
    let (m, match_d, match_one) = match_points(&imgs, gt.mesh.cell_centres());
    // the internal faces of T at the images of P's coupled faces
    let pairs = coupled_pairs(&gp);
    let cut_faces: Vec<Vec3> = pairs
        .iter()
        .flat_map(|&(f, g)| [f, g])
        .map(|f| image(gp.mesh.face_centres()[f]))
        .collect();
    let (t_faces, t_face_d, _) = match_points(&cut_faces, &gt.mesh.face_centres()[..gt.n_internal]);
    let (rp, rt) = (0, m[0]);
    let mut p_on = solver(&gp, true, true, rp);
    let mut p_off = solver(&gp, true, false, rp);
    let mut t_on = solver(&gt, true, true, rt);
    let mut t_off = solver(&gt, true, false, rt);
    let mut p_x = solver(&gp, false, true, rp);
    let mut p_y = solver(&gp, false, false, rp);
    let (mut g_on, mut g_off, mut p2, mut d1l, mut c1, mut c1_off, mut t_oo) =
        (vec![], vec![], vec![], vec![], vec![], vec![], 0.0f64);
    let mut defect = 0.0f64;
    let mut d3 = true;
    let sc = div_scale();
    for _ in 0..STEPS {
        for s in [&mut p_on, &mut p_off, &mut t_on, &mut t_off, &mut p_x, &mut p_y] {
            s.step(DT);
        }
        g_on.push(vdiff(&p_on.u.values, &t_on.u.values, &m) / U_REF);
        g_off.push(vdiff(&p_off.u.values, &t_on.u.values, &m) / U_REF);
        p2.push(sdiff(&p_on.p.values, &t_on.p.values, &m) / (GY.abs() * H));
        let (dvp, dvt) = (divergence(&gp, &p_on.phi), divergence(&gt, &t_on.phi));
        d1l.push(sdiff(&dvp, &dvt, &m) / sc);
        c1.push(pair_sum(&p_on.phi, &pairs) / sc);
        c1_off.push(pair_sum(&p_off.phi, &pairs) / sc);
        let ident: Vec<usize> = (0..t_on.u.values.len()).collect();
        t_oo = t_oo.max(vdiff(&t_on.u.values, &t_off.u.values, &ident) / U_REF);
        defect = defect.max(defect_at(&gt, &t_on, &t_faces) / sc);
        d3 &= bits(&p_x) == bits(&p_y);
    }
    let elapsed = t0.elapsed().as_secs_f64();
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
    println!(
        "T_on against T_off (the term on T's own cut) {t_oo:.4e} U_ref; largest old-level defect on T_on's faces at the images of \
         P's cut {defect:.4e} U_ref h dz ({} faces matched within {t_face_d:.1e}); C1 of P_off {:.3e}; elapsed {elapsed:.1} s",
        t_faces.len(),
        largest(&c1_off).0
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
            pairs.len()
        ),
    );
    check(
        &mut fails,
        "D3",
        d3,
        format!("P_X (the opt-in without ddt_correction) equals P_Y bit for bit on every step: {d3}"),
    );
    check(
        &mut fails,
        "G0",
        match_d <= 1e-12 && match_one,
        format!("nearest T cell of every translated P cell {match_d:.2e} (1e-12), one-to-one {match_one}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---- the sector (ADR 0674) -------------------------------------------------------------------------------------------------

mod sector {
    use super::*;

    pub const ALPHA: f64 = FRAC_PI_3;
    const RI: f64 = 1.0;
    const RO: f64 = 1.5;
    const RM: f64 = 1.25;
    const DZ: f64 = 0.1;
    const NT: usize = 12;
    pub const NR: usize = 8;
    pub const COPIES: usize = 6;
    const G: f64 = 2.0;
    const F0: f64 = 3.2;

    fn tilt(theta: f64) -> f64 {
        let s = theta.rem_euclid(ALPHA);
        if s <= 0.25 * ALPHA || s >= 0.75 * ALPHA {
            TAU * (2.0 * PI * s / ALPHA).cos().powi(2)
        } else {
            0.0
        }
    }

    fn w_map(p: Vec3) -> Vec3 {
        let (r, th) = (p[0].hypot(p[1]), p[1].atan2(p[0]));
        let t = th + tilt(th) * (r - RM) / RM;
        [r * t.cos(), r * t.sin(), p[2]]
    }

    pub fn sector() -> Mesh {
        warp(
            &annulus_sector(NT, NR, RI, RO, ALPHA, Grading::Uniform, DZ).unwrap(),
            w_map,
        )
        .unwrap()
    }

    pub fn annulus() -> Mesh {
        warp(
            &cylinder_ogrid(COPIES * NT, NR, RI, RO, Grading::Uniform, DZ).unwrap(),
            w_map,
        )
        .unwrap()
    }

    fn polar(x: Vec3) -> (f64, f64, Vec3, Vec3) {
        let (r, th) = (x[0].hypot(x[1]), x[1].atan2(x[0]));
        (r, th, [th.cos(), th.sin(), 0.0], [-th.sin(), th.cos(), 0.0])
    }

    fn phi(x: Vec3) -> f64 {
        let (r, th, _, _) = polar(x);
        G * RM / 6.0 * (r / RM).powi(6) * (6.0 * th).cos()
    }

    fn grad_phi(x: Vec3) -> Vec3 {
        let (r, th, rh, th_h) = polar(x);
        let a = G * (r / RM).powi(5);
        vec3::add(
            vec3::scale(rh, a * (6.0 * th).cos()),
            vec3::scale(th_h, -a * (6.0 * th).sin()),
        )
    }

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

    pub fn rot_j(j: usize) -> [[f64; 3]; 3] {
        let a = j as f64 * ALPHA;
        [[a.cos(), -a.sin(), 0.0], [a.sin(), a.cos(), 0.0], [0.0, 0.0, 1.0]]
    }

    pub fn apply(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
    }

    fn walls(g: &FvGeometry) -> [&'static str; 2] {
        if g.mesh.patch("inner").is_some() {
            ["inner", "outer"]
        } else {
            ["cylinder", "farfield"]
        }
    }

    pub fn solver(g: &Arc<FvGeometry>, opt: bool, ref_cell: usize) -> PimpleSolver {
        let mut u = VectorField::uniform("U", g, [0.0; 3]);
        for w in walls(g) {
            u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let p = ScalarField::from_fn("p", g, phi);
        let mut s = settings(true, opt, true);
        s.p_ref = (ref_cell, phi(g.mesh.cell_centres()[ref_cell]));
        let mut sv = PimpleSolver::new(g.clone(), u, p, s);
        sv.body_force = Some(body_force(g));
        sv
    }

    pub fn div_scale() -> f64 {
        U_REF * ((RO - RI) / NR as f64) * DZ
    }

    pub fn vdiff(a: &[Vec3], b: &[Vec3], maps: &[Vec<usize>]) -> f64 {
        let mut out = 0.0f64;
        for (j, m) in maps.iter().enumerate() {
            let r = rot_j(j);
            for (c, v) in a.iter().enumerate() {
                out = out.max(finite_or_inf(vec3::norm(vec3::sub(apply(&r, *v), b[m[c]]))));
            }
        }
        out
    }

    pub fn sdiff(a: &[f64], b: &[f64], maps: &[Vec<usize>]) -> f64 {
        maps.iter().fold(0.0f64, |w, m| {
            a.iter()
                .enumerate()
                .fold(w, |w, (c, v)| w.max(finite_or_inf((v - b[m[c]]).abs())))
        })
    }
}

/// S1, SNC1, SD1L, D4, G0 of ADR 0675 (supporting): the lockstep of `S_on`, `S_off`, `A_on`, `A_off` over 200 steps.
#[test]
fn coupled_ddt_correction_sector_against_the_annulus() {
    use sector::*;
    let t0 = Instant::now();
    let gs = geom_of(sector());
    let ga = geom_of(annulus());
    let (mut maps, mut match_d, mut each_one) = (Vec::new(), 0.0f64, true);
    for j in 0..COPIES {
        let r = rot_j(j);
        let imgs: Vec<Vec3> = gs.mesh.cell_centres().iter().map(|&c| apply(&r, c)).collect();
        let (mm, d, one) = match_points(&imgs, ga.mesh.cell_centres());
        match_d = match_d.max(d);
        each_one &= one;
        maps.push(mm);
    }
    let all: std::collections::HashSet<usize> = maps.iter().flatten().copied().collect();
    let bijective = all.len() == ga.n_cells() && maps.iter().map(|m| m.len()).sum::<usize>() == ga.n_cells();
    let pairs = coupled_pairs(&gs);
    let wrap: Vec<Vec3> = (0..COPIES)
        .flat_map(|j| {
            let r = rot_j(j);
            let gs = &gs;
            pairs.iter().map(move |&(f, _)| apply(&r, gs.mesh.face_centres()[f]))
        })
        .collect();
    let (a_faces, a_face_d, _) = match_points(&wrap, &ga.mesh.face_centres()[..ga.n_internal]);
    let (rs, ra) = (0, maps[0][0]);
    let mut s_on = solver(&gs, true, rs);
    let mut s_off = solver(&gs, false, rs);
    let mut a_on = solver(&ga, true, ra);
    let mut a_off = solver(&ga, false, ra);
    let (mut g_on, mut g_off, mut d1l, mut c1) = (vec![], vec![], vec![], vec![]);
    let (mut d4, mut defect) = (true, 0.0f64);
    let sc = div_scale();
    for _ in 0..STEPS {
        for s in [&mut s_on, &mut s_off, &mut a_on, &mut a_off] {
            s.step(DT);
        }
        g_on.push(vdiff(&s_on.u.values, &a_on.u.values, &maps) / U_REF);
        g_off.push(vdiff(&s_off.u.values, &a_on.u.values, &maps) / U_REF);
        let (dvs, dva) = (divergence(&gs, &s_on.phi), divergence(&ga, &a_on.phi));
        d1l.push(sdiff(&dvs, &dva, &maps) / sc);
        c1.push(pair_sum(&s_on.phi, &pairs) / sc);
        d4 &= bits(&a_on) == bits(&a_off);
        defect = defect.max(defect_at(&ga, &a_on, &a_faces) / sc);
    }
    let elapsed = t0.elapsed().as_secs_f64();
    for (name, g) in [("G_Son", &g_on), ("G_Soff", &g_off)] {
        let (mx, st) = largest(g);
        println!(
            "{name}: step 1 {:.4e}, step 2 {:.4e}, largest {mx:.4e} U_ref at step {st}, last {:.4e}",
            g[0],
            g[1],
            g[STEPS - 1]
        );
    }
    println!(
        "largest old-level defect on A_on's faces at the images of S's wrap {defect:.4e} U_ref dr dz ({} faces within \
         {a_face_d:.1e}); |phi_f + phi_g| of S_on {:.3e} U_ref dr dz (printed); largest |div| of S_on {:.3e}; elapsed {elapsed:.1} s",
        a_faces.len(),
        largest(&c1).0,
        max_abs(&divergence(&gs, &s_on.phi)) / sc
    );
    let mut fails = Fails::new();
    let (n1, s1) = largest(&g_on);
    check(
        &mut fails,
        "S1",
        n1 <= 1e-8,
        format!("max over {STEPS} steps of G_Son(n) {n1:.4e} U_ref (step {s1}) (1e-8 each step)"),
    );
    let (nc, sn) = largest(&g_off);
    check(
        &mut fails,
        "SNC1",
        nc > 1e-8,
        format!("control: largest G_Soff(n) {nc:.4e} U_ref (step {sn}) (> 1e-8 on at least one step)"),
    );
    let (dl, sl) = largest(&d1l);
    check(
        &mut fails,
        "SD1L",
        dl <= 1e-9,
        format!("max over the steps of |div_Son - div_Aon| / (U_ref dr dz) {dl:.4e} (step {sl}) (1e-9 each step)"),
    );
    check(
        &mut fails,
        "D4",
        d4,
        format!("A_on equals A_off bit for bit on every step: {d4}"),
    );
    check(
        &mut fails,
        "G0",
        match_d <= 1e-12 && each_one && bijective,
        format!("six cell maps {match_d:.2e} (1e-12), each one-to-one {each_one}, a bijection {bijective}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
