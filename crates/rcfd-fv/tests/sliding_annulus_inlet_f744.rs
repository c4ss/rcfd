//! F-744 (ADR 0667): the explicit non-orthogonal correction on the coupled (seam and wrap) faces
//! (`PimpleSettings::coupled_non_orth`) on the lockstep of ADR 0661 (the sector SS through `periodic_sliding_step_coupled`
//! against the full annulus P), the lag removed by ADR 0664's `rotational_periodic_implicit` with one outer iteration:
//! `V**(1, 6)` against ADR 0661's `V(6, 0)`, with ADR 0664's `V*(1, 6)` (the correction off) as the control. Checks N1J, N1fJ,
//! D0J, D2, NC1J, G0 (from `u0`, 16 steps; fast), H2 (geometry, fast) and N1, N2, N1f, D0h, NC1, A1s, G0 (from `u0`, 400
//! steps; heavy record). The limits are those registered in ADR 0667 before this code ran; each test prints every measured
//! number and then asserts the checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::CoupledNonOrth;
use rcfd_fv::sliding::sliding_step;
use rcfd_fv::sliding_periodic::periodic_sliding_step_coupled;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, cylinder_ogrid, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};
const U_IN: f64 = 1.0;
const RI: f64 = 1.0;
/// ADR 0641's LS: the rotor pitch over 8, 400 steps.
const DT: f64 = PI / 576.0;
const STEPS: usize = 400;
/// The fast form: the first two coincidence steps.
const J_STEPS: usize = 16;
/// The variants: `V**(1, 6)`, `V(6, 0)`, `V*(1, 6)` as `(n_outer, non_orth_correctors, implicit pair, coupled correction)`.
const VARIANTS: [(usize, usize, bool, bool); 3] = [(1, 6, true, true), (6, 0, false, false), (1, 6, true, false)];

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// ADR 0216's pair P (the full annulus).
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// PS6 of ADR 0629 with `n_outer = k`, `non_orth_correctors = n`, ADR 0664's opt-in `implicit` and ADR 0667's `coupled`.
fn settings(k: usize, n: usize, implicit: bool, coupled: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.1);
    s.rotational_periodic_implicit = implicit;
    s.coupled_non_orth = coupled;
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = n;
    s.n_outer = k;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

/// The inlet of ADR 0641, `U_in (1 + 0.5 sin 6 theta) x_hat`.
fn modulation(x: Vec3) -> f64 {
    1.0 + 0.5 * (6.0 * x[1].atan2(x[0])).sin()
}

fn inlet(x: Vec3) -> Vec3 {
    vec3::scale(xhat(x), U_IN * modulation(x))
}

/// The start `u0 = U_in (1 + 0.5 sin 6 theta) (r_i / r) x_hat` (consistent with the inlet, divergence-free; ADR 0661).
fn start(x: Vec3) -> Vec3 {
    vec3::scale(xhat(x), U_IN * modulation(x) * RI / x[0].hypot(x[1]))
}

fn per_face(g: &FvGeometry, patch: &str) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| inlet(cf[k])).collect(),
    )))
}

fn solver(g: Arc<FvGeometry>, inlet_patch: &str, outlet: &str, s: PimpleSettings) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", &g, start);
    u.set_bc(&g, inlet_patch, per_face(&g, inlet_patch)).unwrap();
    u.set_bc(&g, outlet, Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, outlet, Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(g, u, p, s)
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

/// One variant: the coupled sector C and the annulus A with the same settings.
struct Variant<'a> {
    name: String,
    sc: &'a PeriodicSlidingInterface,
    c_cur: PeriodicSlidingMesh,
    c: PimpleSolver,
    a_sc: SlidingInterface,
    a_cur: SlidingMesh,
    a: PimpleSolver,
}

impl<'a> Variant<'a> {
    fn new(sc: &'a PeriodicSlidingInterface, (k, n, implicit, coupled): (usize, usize, bool, bool)) -> Self {
        let c_cur = sc.mesh_turned(0.0).unwrap();
        let c = solver(
            geom_of(&c_cur.mesh),
            "inner",
            "outer",
            settings(k, n, implicit, coupled),
        );
        let a_sc = pair_p();
        let a_cur = a_sc.mesh_at(0.0).unwrap();
        let a = solver(
            geom_of(&a_cur.mesh),
            "cylinder",
            "farfield",
            settings(k, n, implicit, coupled),
        );
        let stars = match (implicit, coupled) {
            (true, true) => "**",
            (true, false) => "*",
            _ => "",
        };
        Variant {
            name: format!("V{stars}({k}, {n})"),
            sc,
            c_cur,
            c,
            a_sc,
            a_cur,
            a,
        }
    }

    /// One step of both solvers: `u_C - u_A` at every sector cell (against its nearest annulus cell), the match, and C's
    /// remapped flux residual.
    fn step(&mut self, step_no: usize) -> (Vec<Vec3>, f64, bool, f64) {
        let theta = step_no as f64 * DT;
        let new_c = self.sc.mesh_turned(theta).unwrap();
        let st_c = periodic_sliding_step_coupled(&self.c.geom, &self.c_cur, &new_c, &self.c.phi, &self.c.u.values, DT)
            .unwrap();
        let new_a = self.a_sc.mesh_at(theta).unwrap();
        let st_a = sliding_step(&self.a.geom, &self.a_cur, &new_a, &self.a.phi, &self.a.u.values, DT).unwrap();
        let flux_residual = finite_or_inf(st_c.flux_residual);
        let gc = st_c.geom.clone();
        self.c.u.set_bc(&gc, "inner", per_face(&gc, "inner")).unwrap();
        self.c.step_sliding_periodic(DT, st_c, Vec::new()).unwrap();
        self.c_cur = new_c;
        let ga = st_a.geom.clone();
        self.a.u.set_bc(&ga, "cylinder", per_face(&ga, "cylinder")).unwrap();
        self.a.step_sliding(DT, st_a, Vec::new()).unwrap();
        self.a_cur = new_a;
        let (idx, match_d, match_one) = match_points(self.c.geom.mesh.cell_centres(), self.a.geom.mesh.cell_centres());
        let delta = idx
            .iter()
            .enumerate()
            .map(|(c, &j)| vec3::sub(self.c.u.values[c], self.a.u.values[j]))
            .collect();
        (delta, match_d, match_one, flux_residual)
    }
}

fn max_norm(x: &[Vec3]) -> f64 {
    x.iter().fold(0.0f64, |m, v| m.max(finite_or_inf(vec3::norm(*v)))) / U_IN
}

fn max_diff(x: &[Vec3], y: &[Vec3]) -> f64 {
    x.iter()
        .zip(y)
        .fold(0.0f64, |m, (a, b)| m.max(finite_or_inf(vec3::norm(vec3::sub(*a, *b)))))
        / U_IN
}

/// The bits of the annulus solver's velocity and pressure.
fn annulus_bits(v: &Variant) -> Vec<u64> {
    let mut b: Vec<u64> =
        v.a.u
            .values
            .iter()
            .flat_map(|u| u.iter().map(|x| x.to_bits()))
            .collect();
    b.extend(v.a.p.values.iter().map(|x| x.to_bits()));
    b
}

/// The lockstep of [`VARIANTS`] from `u0` over `steps` steps.
struct Run {
    /// `G_V(n)` of every variant at every step (index `n - 1`).
    g: Vec<Vec<f64>>,
    /// The largest over the steps of `max_c |delta_V - delta_W|` of every pair of variants.
    pair: Vec<Vec<f64>>,
    match_d: f64,
    match_one: bool,
    /// C's flux residual of variant 0 from step 2.
    a1s: f64,
    /// Whether the annulus of variant 0 and that of variant 2 were equal bit for bit on every step.
    annulus_same: bool,
    elapsed: f64,
}

impl Run {
    /// The largest over the steps of `|G_i(n) - G_j(n)|`, its signed value and its step.
    fn gdiff(&self, i: usize, j: usize) -> (f64, f64, usize) {
        let mut out = (0.0, 0.0, 0);
        for (s, (a, b)) in self.g[i].iter().zip(&self.g[j]).enumerate() {
            let d = a - b;
            if finite_or_inf(d.abs()) > out.0 {
                out = (finite_or_inf(d.abs()), d, s + 1);
            }
        }
        out
    }

    fn gmax(&self, i: usize) -> (f64, usize) {
        self.g[i].iter().enumerate().fold((0.0f64, 0), |b, (s, &x)| {
            if finite_or_inf(x) > b.0 {
                (finite_or_inf(x), s + 1)
            } else {
                b
            }
        })
    }
}

fn run(steps: usize) -> Run {
    let t0 = Instant::now();
    let sc = pair_ss();
    let mut var: Vec<Variant> = VARIANTS.iter().map(|&v| Variant::new(&sc, v)).collect();
    let nv = var.len();
    let mut out = Run {
        g: vec![Vec::new(); nv],
        pair: vec![vec![0.0; nv]; nv],
        match_d: 0.0,
        match_one: true,
        a1s: 0.0,
        annulus_same: true,
        elapsed: 0.0,
    };
    for n in 1..=steps {
        let outs: Vec<(Vec<Vec3>, f64, bool, f64)> = var.iter_mut().map(|v| v.step(n)).collect();
        for (i, o) in outs.iter().enumerate() {
            out.match_d = out.match_d.max(o.1);
            out.match_one &= o.2;
            out.g[i].push(max_norm(&o.0));
            for (j, p) in outs.iter().enumerate().skip(i + 1) {
                let d = max_diff(&o.0, &p.0);
                out.pair[i][j] = out.pair[i][j].max(d);
                out.pair[j][i] = out.pair[i][j];
            }
        }
        if n >= 2 {
            out.a1s = out.a1s.max(outs[0].3);
        }
        out.annulus_same &= annulus_bits(&var[0]) == annulus_bits(&var[2]);
    }
    out.elapsed = t0.elapsed().as_secs_f64();
    for (i, v) in var.iter().enumerate() {
        let (gmax, step) = out.gmax(i);
        println!(
            "{}: G at step 1 {:.9e}, largest {gmax:.6e} U_in at step {step}",
            v.name, out.g[i][0]
        );
    }
    for i in 0..nv {
        for j in i + 1..nv {
            let (d, sd, st) = out.gdiff(i, j);
            println!(
                "{} against {}: max_n |G - G| {d:.4e} (signed {sd:.4e}, step {st}); max |delta - delta| {:.4e}",
                var[i].name, var[j].name, out.pair[i][j]
            );
        }
    }
    out
}

/// N1J, N1fJ, D0J, D2, NC1J, G0 of ADR 0667: the three variants from `u0` over 16 steps.
#[test]
fn coupled_correction_from_u0_fast() {
    let r = run(J_STEPS);
    println!("elapsed {:.1} s", r.elapsed);
    let mut fails = Fails::new();
    let n1 = r.gdiff(0, 1);
    check(
        &mut fails,
        "N1J",
        n1.0 <= 3e-4,
        format!(
            "max over {J_STEPS} steps of |G_**(n) - G_(6,0)(n)| {:.4e} U_in (signed {:.4e}, step {}) (3e-4 each step)",
            n1.0, n1.1, n1.2
        ),
    );
    check(
        &mut fails,
        "N1fJ",
        r.pair[0][1] <= 5e-4,
        format!(
            "max over {J_STEPS} steps of max_c |delta_** - delta_(6,0)| {:.4e} U_in (5e-4 each step)",
            r.pair[0][1]
        ),
    );
    check(
        &mut fails,
        "D0J",
        (r.g[2][0] - 7.90e-4).abs() <= 5e-7,
        format!(
            "V*(1, 6) (correction off) step 1 {:.6e} U_in (ADR 0664's 7.90e-4 within 5e-7)",
            r.g[2][0]
        ),
    );
    check(
        &mut fails,
        "D2",
        r.annulus_same,
        format!(
            "the annulus with the correction on equals the annulus with it off bit for bit on every step: {}",
            r.annulus_same
        ),
    );
    let nc = r.gdiff(2, 1);
    check(
        &mut fails,
        "NC1J",
        nc.0 >= 5e-4,
        format!(
            "control: max over {J_STEPS} steps of |G_*(n) - G_(6,0)(n)| {:.4e} U_in (step {}) (>= 5e-4)",
            nc.0, nc.2
        ),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// The non-orthogonal remainder `S - d |S|^2 / (S . d)`.
fn k_of(s: Vec3, d: Vec3) -> Vec3 {
    vec3::sub(s, vec3::scale(d, vec3::dot(s, s) / vec3::dot(s, d)))
}

/// H2 of ADR 0667: on the meshes of steps 0 to 16, every coupled face of the sector is an internal face of the annulus with
/// the same centre, and its `d` and `k` are the annulus face's (oriented from the matched owner).
#[test]
fn h2_coupled_faces_are_the_annulus_faces() {
    let sc = pair_ss();
    let ap = pair_p();
    let (mut worst_c, mut worst_d, mut worst_k, mut worst_s, mut kmax) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut n_faces, mut unmatched, mut misoriented) = (0usize, 0usize, 0usize);
    for n in 0..=J_STEPS {
        let theta = n as f64 * DT;
        let gc = geom_of(&sc.mesh_turned(theta).unwrap().mesh);
        let ga = geom_of(&ap.mesh_at(theta).unwrap().mesh);
        let cn = CoupledNonOrth::build(&gc, |_| true);
        let (ma, mc) = (&ga.mesh, &gc.mesh);
        let fa: Vec<Vec3> = ma.face_centres()[..ga.n_internal].to_vec();
        let (cell_idx, _, _) = match_points(mc.cell_centres(), ma.cell_centres());
        let (sa, oa, na) = (ma.face_area_vectors(), ma.owner(), ma.neighbour());
        for (f, d, k) in cn.geometry() {
            n_faces += 1;
            let x = mc.face_centres()[f];
            let (idx, dist, _) = match_points(&[x], &fa);
            worst_c = worst_c.max(dist);
            if dist > 1e-9 {
                unmatched += 1;
                continue;
            }
            let h = idx[0];
            let r = cell_idx[mc.owner()[f]];
            let sign = if oa[h] == r {
                1.0
            } else if na[h] == r {
                -1.0
            } else {
                misoriented += 1;
                continue;
            };
            let da = vec3::scale(ga.delta[h], sign);
            let s_a = vec3::scale(sa[h], sign);
            let ka = k_of(s_a, da);
            worst_d = worst_d.max(vec3::norm(vec3::sub(d, da)));
            worst_k = worst_k.max(vec3::norm(vec3::sub(k, ka)));
            worst_s = worst_s.max(vec3::norm(vec3::sub(mc.face_area_vectors()[f], s_a)));
            kmax = kmax.max(vec3::norm(k) / vec3::norm(s_a));
        }
    }
    println!(
        "H2: {n_faces} coupled faces over {} meshes; centre {worst_c:.2e}, unmatched {unmatched}, owner not on the face \
         {misoriented}; d {worst_d:.2e}, k {worst_k:.2e}, S {worst_s:.2e}; largest |k| / |S| {kmax:.4}",
        J_STEPS + 1
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "H2",
        n_faces > 0 && unmatched == 0 && misoriented == 0 && worst_c <= 1e-9 && worst_d <= 1e-10 && worst_k <= 1e-10,
        format!(
            "every coupled face an annulus face (centre {worst_c:.2e} <= 1e-9, {unmatched} unmatched, {misoriented} \
             misoriented); d {worst_d:.2e}, k {worst_k:.2e} (1e-10 each)"
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// N1, N2, N1f, D0h, NC1, A1s, G0 of ADR 0667: the three variants from `u0` over 400 steps.
#[test]
#[ignore = "heavy record (about 600 s estimated), run once (ADR 0667); coupled_correction_from_u0_fast covers the same path fast"]
fn coupled_correction_against_v60_from_u0() {
    let r = run(STEPS);
    println!("elapsed {:.1} s", r.elapsed);
    let mut fails = Fails::new();
    let n1 = r.gdiff(0, 1);
    check(
        &mut fails,
        "N1",
        n1.0 <= 3e-4,
        format!(
            "max over the {STEPS} steps of |G_**(n) - G_(6,0)(n)| {:.4e} U_in (signed {:.4e}, step {}) (3e-4 each step)",
            n1.0, n1.1, n1.2
        ),
    );
    let (g0, s0) = r.gmax(0);
    check(
        &mut fails,
        "N2",
        g0 <= 4.5e-4,
        format!("largest G_** {g0:.4e} U_in at step {s0} (4.5e-4; 6.0e-3 without the correction)"),
    );
    check(
        &mut fails,
        "N1f",
        r.pair[0][1] <= 5e-4,
        format!(
            "max over the steps of max_c |delta_** - delta_(6,0)| {:.4e} U_in (5e-4 each step)",
            r.pair[0][1]
        ),
    );
    let ((g1, _), (g2, s2)) = (r.gmax(1), r.gmax(2));
    check(
        &mut fails,
        "D0h",
        (g1 - 1.48e-4).abs() <= 5e-7 && (g2 - 5.891e-3).abs() <= 5e-7,
        format!(
            "largest G_(6,0) {g1:.6e} (ADR 0661's 1.48e-4 within 5e-7); largest G_* {g2:.6e} at step {s2} (ADR 0664's \
             5.891e-3 within 5e-7)"
        ),
    );
    let nc = r.gdiff(2, 1);
    check(
        &mut fails,
        "NC1",
        nc.0 >= 3e-3,
        format!(
            "control: max over the steps of |G_*(n) - G_(6,0)(n)| {:.4e} U_in (step {}) (>= 3e-3)",
            nc.0, nc.2
        ),
    );
    check(
        &mut fails,
        "A1s",
        r.a1s <= 1e-12,
        format!(
            "C flux_residual of V**(1, 6), steps 2 to {STEPS}: {:.2e} (1e-12)",
            r.a1s
        ),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
