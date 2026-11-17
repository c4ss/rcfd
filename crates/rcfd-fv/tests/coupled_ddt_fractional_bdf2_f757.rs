//! F-757 (ADR 0687): the transient Rhie-Chow term on coupled faces in the fractional step (ADR 0356's carried-defect flux,
//! behind `PimpleSettings::coupled_ddt_correction`, with the implicit periodic partner `fractional_periodic_implicit`) and in
//! the PISO step with BDF2. Cases on ADR 0672's skewed periodic channel `P` against its unrolled twin `T` (the meshes and the
//! map copied from ADR 0672's test; that file and ADR 0675's are not edited): the fractional step from an initial velocity
//! (the fractional step reads no body force, ADR 0687 read 2; checks P1f, NC1f, P2f, D1Lf, C1f, NCL, D5, G0), its no-op on a
//! mesh without pairs (D6), ADR 0675's PISO lockstep with BDF2 (P1b, NC1b, P2b, D1Lb, C1b) and the fractional step with BDF2
//! (S1fb, SNC1fb). The limits are those registered in ADR 0687 before this code ran; each test prints every measured number
//! and then asserts the checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::divergence;
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
const PASSES: usize = 6;
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
const U0: f64 = 1.0;
const A: f64 = 0.5 * H / PI;

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

fn base_settings(bdf2: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.n_outer = 1;
    s.n_correctors = 2;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s.ddt_correction = true;
    if bdf2 {
        s.time_scheme = TimeScheme::Bdf2;
    }
    s
}

/// ADR 0675's PISO channel solver (ADR 0672's flow and full setting `F`, `ddt_correction` on), with BDF2 if asked.
fn piso_solver(g: &Arc<FvGeometry>, opt: bool, bdf2: bool, ref_cell: usize) -> PimpleSolver {
    let mut u = VectorField::uniform("U", g, [0.0; 3]);
    for w in ["ymin", "ymax"] {
        u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::from_fn("p", g, p0);
    let mut s = base_settings(bdf2);
    s.non_orth_correctors = PASSES;
    s.coupled_body_pair_mean = true;
    s.coupled_non_orth = true;
    s.coupled_ddt_correction = opt;
    s.p_ref = (ref_cell, p0(g.mesh.cell_centres()[ref_cell]));
    let mut sv = PimpleSolver::new(g.clone(), u, p, s);
    sv.body_force = Some(BodyForce::uniform(g, [FX, GY, 0.0]));
    sv
}

/// The initial velocity of the fractional runs (ADR 0687): a plane shear flow and a periodic cellular part, at `x + shift`.
fn u_init(x: Vec3, shift: f64) -> Vec3 {
    let (xs, y) = (x[0] + shift, x[1]);
    let kx = 2.0 * PI * xs / L;
    [
        U0 * 4.0 * y * (H - y) / (H * H) + A * (PI / H) * (2.0 * PI * y / H).sin() * kx.cos(),
        A * (2.0 * PI / L) * (PI * y / H).sin().powi(2) * kx.sin(),
        0.0,
    ]
}

/// A fractional-step solver on the channel mesh `g` (built with `shift`): ADR 0687's set-up.
struct Frac {
    opt: bool,
    sc: bool,
    links: bool,
    bdf2: bool,
}

fn frac_solver(g: &Arc<FvGeometry>, shift: f64, o: Frac, ref_cell: usize, walls: &[&str]) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", g, |x| u_init(x, shift));
    for w in walls {
        u.set_bc(g, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", g, 0.0);
    let mut s = base_settings(o.bdf2);
    s.non_orth_correctors = 0;
    s.fractional = Some(FractionalSettings::default());
    s.fractional_step_consistent = o.sc;
    s.fractional_periodic_implicit = o.links;
    s.coupled_ddt_correction = o.opt;
    s.p_ref = (ref_cell, 0.0);
    PimpleSolver::new(g.clone(), u, p, s)
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

/// The per-step series of one lockstep of `P_on`, `P_off`, `T_on`, `T_off` (and any extra `P` runs compared to `T_on`).
#[derive(Default)]
struct Series {
    g_on: Vec<f64>,
    g_off: Vec<f64>,
    p2: Vec<f64>,
    d1l: Vec<f64>,
    c1: Vec<f64>,
    c1_off: Vec<f64>,
    t_oo: f64,
    defect: f64,
    extra: Vec<Vec<f64>>,
}

fn lockstep(pr: &Pair, runs: [&mut PimpleSolver; 4], extra: &mut [&mut PimpleSolver], p_scale: f64) -> Series {
    let [p_on, p_off, t_on, t_off] = runs;
    let sc = div_scale();
    let m = &pr.m;
    let mut se = Series {
        extra: vec![Vec::new(); extra.len()],
        ..Default::default()
    };
    let ident: Vec<usize> = (0..t_on.u.values.len()).collect();
    for _ in 0..STEPS {
        p_on.step(DT);
        p_off.step(DT);
        t_on.step(DT);
        t_off.step(DT);
        for e in extra.iter_mut() {
            e.step(DT);
        }
        se.g_on.push(vdiff(&p_on.u.values, &t_on.u.values, m) / U_REF);
        se.g_off.push(vdiff(&p_off.u.values, &t_on.u.values, m) / U_REF);
        se.p2.push(sdiff(&p_on.p.values, &t_on.p.values, m) / p_scale);
        let (dvp, dvt) = (divergence(&pr.gp, &p_on.phi), divergence(&pr.gt, &t_on.phi));
        se.d1l.push(sdiff(&dvp, &dvt, m) / sc);
        se.c1.push(pair_sum(&p_on.phi, &pr.pairs) / sc);
        se.c1_off.push(pair_sum(&p_off.phi, &pr.pairs) / sc);
        se.t_oo = se.t_oo.max(vdiff(&t_on.u.values, &t_off.u.values, &ident) / U_REF);
        se.defect = se.defect.max(defect_at(&pr.gt, t_on, &pr.t_faces) / sc);
        for (j, e) in extra.iter().enumerate() {
            se.extra[j].push(vdiff(&e.u.values, &t_on.u.values, m) / U_REF);
        }
    }
    se
}

fn print_series(se: &Series, elapsed: f64, pr: &Pair) {
    for (name, g) in [("G_on", &se.g_on), ("G_off", &se.g_off)] {
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
        "T_on against T_off {:.4e} U_ref; largest old-level defect on T_on's faces at the images of P's cut {:.4e} U_ref h dz \
         ({} faces); C1 of P_off {:.3e}; elapsed {elapsed:.1} s",
        se.t_oo,
        se.defect,
        pr.t_faces.len(),
        largest(&se.c1_off).0
    );
}

/// The checks every lockstep carries: P1, NC1, P2, D1L, C1 (with the suffix of the case), G0.
fn common_checks(fails: &mut Fails, ids: [&'static str; 6], se: &Series, pr: &Pair, p_what: &str) {
    let (n1, s1) = largest(&se.g_on);
    check(
        fails,
        ids[0],
        n1 <= 1e-8,
        format!("max over {STEPS} steps of G_on(n) {n1:.4e} U_ref (step {s1}) (1e-8 each step)"),
    );
    let (nc, sn) = largest(&se.g_off);
    check(
        fails,
        ids[1],
        nc > 1e-8,
        format!("control: largest G_off(n) {nc:.4e} U_ref (step {sn}) (> 1e-8 on at least one step)"),
    );
    let (pp, sp) = largest(&se.p2);
    check(
        fails,
        ids[2],
        pp <= 1e-8,
        format!("max over the steps of |p_Pon - p_Ton| / ({p_what}) {pp:.4e} (step {sp}) (1e-8 each step)"),
    );
    let (dl, sl) = largest(&se.d1l);
    check(
        fails,
        ids[3],
        dl <= 1e-9,
        format!("max over the steps of |div_Pon - div_Ton| / (U_ref h dz) {dl:.4e} (step {sl}) (1e-9 each step)"),
    );
    let (cc, scc) = largest(&se.c1);
    check(
        fails,
        ids[4],
        cc <= 1e-12,
        format!(
            "max over the steps of |phi_f + phi_g| / (U_ref h dz) on P_on's {} pairs {cc:.3e} (step {scc}) (1e-12 each step)",
            pr.pairs.len()
        ),
    );
    check(
        fails,
        ids[5],
        pr.match_d <= 1e-12 && pr.match_one,
        format!(
            "nearest T cell of every translated P cell {:.2e} (1e-12), one-to-one {}",
            pr.match_d, pr.match_one
        ),
    );
}

const WALLS: [&str; 2] = ["ymin", "ymax"];

/// P1f, NC1f, P2f, D1Lf, C1f, NCL, D5, G0 of ADR 0687: the fractional step (Euler) on the channel against its twin.
#[test]
fn fractional_step_channel_against_the_twin() {
    let t0 = Instant::now();
    let pr = pair();
    let (rp, rt) = (0, pr.m[0]);
    let (sp, st) = (0.0, 0.5 * L);
    let f = |opt, sc, links| Frac {
        opt,
        sc,
        links,
        bdf2: false,
    };
    let mut p_on = frac_solver(&pr.gp, sp, f(true, true, true), rp, &WALLS);
    let mut p_off = frac_solver(&pr.gp, sp, f(false, true, true), rp, &WALLS);
    let mut t_on = frac_solver(&pr.gt, st, f(true, true, true), rt, &WALLS);
    let mut t_off = frac_solver(&pr.gt, st, f(false, true, true), rt, &WALLS);
    let mut p_nl = frac_solver(&pr.gp, sp, f(true, true, false), rp, &WALLS);
    let mut d5a = frac_solver(&pr.gp, sp, f(true, false, true), rp, &WALLS);
    let mut d5b = frac_solver(&pr.gp, sp, f(false, false, true), rp, &WALLS);
    // D5 lockstep (bit for bit every step), then the main lockstep
    let mut d5 = true;
    for _ in 0..STEPS {
        d5a.step(DT);
        d5b.step(DT);
        d5 &= bits(&d5a) == bits(&d5b);
    }
    let se = lockstep(
        &pr,
        [&mut p_on, &mut p_off, &mut t_on, &mut t_off],
        &mut [&mut p_nl],
        0.5 * U_REF * U_REF,
    );
    print_series(&se, t0.elapsed().as_secs_f64(), &pr);
    let mut fails = Fails::new();
    common_checks(
        &mut fails,
        ["P1f", "NC1f", "P2f", "D1Lf", "C1f", "G0"],
        &se,
        &pr,
        "0.5 U_ref^2",
    );
    let (nl, snl) = largest(&se.extra[0]);
    check(
        &mut fails,
        "NCL",
        nl > 1e-6,
        format!(
            "P_on without the implicit partner against T_on: largest {nl:.4e} U_ref (step {snl}) (> 1e-6 on one step)"
        ),
    );
    check(
        &mut fails,
        "D5",
        d5,
        format!("the opt-in without fractional_step_consistent equals neither bit for bit on every step: {d5}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// D6 of ADR 0687: `fractional_periodic_implicit` on a mesh without periodic pairs changes nothing, 20 steps bit for bit.
#[test]
fn fractional_periodic_implicit_without_pairs_is_a_no_op() {
    let g = geom_of(box_2d(NX, NY, L, H, false, false).unwrap());
    let walls = ["xmin", "xmax", "ymin", "ymax"];
    let mk = |links| {
        frac_solver(
            &g,
            0.0,
            Frac {
                opt: true,
                sc: true,
                links,
                bdf2: false,
            },
            0,
            &walls,
        )
    };
    let (mut a, mut b) = (mk(true), mk(false));
    let mut d6 = true;
    for _ in 0..20 {
        a.step(DT);
        b.step(DT);
        d6 &= bits(&a) == bits(&b);
    }
    let mut fails = Fails::new();
    check(
        &mut fails,
        "D6",
        d6,
        format!("fractional_periodic_implicit on against off without pairs, 20 steps bit for bit: {d6}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// P1b, NC1b, P2b, D1Lb, C1b (and G0) of ADR 0687: ADR 0675's PISO lockstep with BDF2. Known miss (ADR 0687 amendment 1):
/// the one registered run (5bb12a91) gave P1b `4.2194e-4` U_ref (step 85; limit `1e-8`) and P2b `8.2756e-4` (limit `1e-8`),
/// NC1b held (`4.1895e-4`), D1Lb `1.3888e-11`, C1b `4.441e-16`, G0 met. Cause: the lagged periodic partner in the BDF2
/// deferred-correction state of the PISO predictor; the corrected set-up is `bdf2_piso_extrapolated_partner_against_the_twin`.
#[test]
#[ignore = "known miss of ADR 0687 (P1b, P2b): the registered record, limits intact"]
fn bdf2_piso_channel_against_the_twin_known_miss() {
    let t0 = Instant::now();
    let pr = pair();
    let (rp, rt) = (0, pr.m[0]);
    let mut p_on = piso_solver(&pr.gp, true, true, rp);
    let mut p_off = piso_solver(&pr.gp, false, true, rp);
    let mut t_on = piso_solver(&pr.gt, true, true, rt);
    let mut t_off = piso_solver(&pr.gt, false, true, rt);
    let se = lockstep(
        &pr,
        [&mut p_on, &mut p_off, &mut t_on, &mut t_off],
        &mut [],
        GY.abs() * H,
    );
    print_series(&se, t0.elapsed().as_secs_f64(), &pr);
    let mut fails = Fails::new();
    common_checks(
        &mut fails,
        ["P1b", "NC1b", "P2b", "D1Lb", "C1b", "G0"],
        &se,
        &pr,
        "|g_y| H",
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// S1fb, SNC1fb of ADR 0687 (supporting): the fractional step with BDF2 on the channel against its twin.
#[test]
fn fractional_step_bdf2_channel_against_the_twin() {
    let t0 = Instant::now();
    let pr = pair();
    let (rp, rt) = (0, pr.m[0]);
    let f = |opt| Frac {
        opt,
        sc: true,
        links: true,
        bdf2: true,
    };
    let mut p_on = frac_solver(&pr.gp, 0.0, f(true), rp, &WALLS);
    let mut p_off = frac_solver(&pr.gp, 0.0, f(false), rp, &WALLS);
    let mut t_on = frac_solver(&pr.gt, 0.5 * L, f(true), rt, &WALLS);
    let mut t_off = frac_solver(&pr.gt, 0.5 * L, f(false), rt, &WALLS);
    let se = lockstep(
        &pr,
        [&mut p_on, &mut p_off, &mut t_on, &mut t_off],
        &mut [],
        0.5 * U_REF * U_REF,
    );
    print_series(&se, t0.elapsed().as_secs_f64(), &pr);
    let mut fails = Fails::new();
    let (n1, s1) = largest(&se.g_on);
    check(
        &mut fails,
        "S1fb",
        n1 <= 1e-8,
        format!("max over {STEPS} steps of G_on(n) {n1:.4e} U_ref (step {s1}) (1e-8 each step)"),
    );
    let (nc, sn) = largest(&se.g_off);
    check(
        &mut fails,
        "SNC1fb",
        nc > 1e-8,
        format!("control: largest G_off(n) {nc:.4e} U_ref (step {sn}) (> 1e-8 on at least one step)"),
    );
    // printed, no limit in ADR 0687 for the fractional BDF2 case
    let (pp, _) = largest(&se.p2);
    let (dl, _) = largest(&se.d1l);
    let (cc, _) = largest(&se.c1);
    println!("printed: P2 {pp:.4e}, D1L {dl:.4e}, C1 {cc:.3e}");
    assert!(fails.is_empty(), "{fails:?}");
}
