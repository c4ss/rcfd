//! ADR 0205 (F-295), library checks of one diffusivity per periodic seam pair in k-omega SST: P0 the pairs of a translational
//! and a rotational seam, R3 the refusal of a seam map that is not a pairing, F1 the face values (one value per pair, the bits of
//! D1, nothing else changed), G1 the global diffusion budget of the operator with those values, and the transient budgets C1p
//! and C2p of the periodic box of ADR 0199 C1 with their negative controls C1pN and C2pN (`owner`), and the report-only C1p30.
//! The limits are those registered in ADR 0205 before this code ran.
//!
//! BOX: `box_2d(16, 4, 1, 1)` periodic in x. SEC: `annulus_3d_sector(8, 6, 4, 0.5, 1.0, 0.5, 40 degrees)`, a rotational seam.
//! GBOX (amendment 2, F1g): BOX graded by 4 in x, so the two faces of a pair are at different distances (`w = 0.8`).

use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, box_2d, box_2d_graded, Grading};
use rcfd_turb::{KOmegaSst, SeamDiffusivity};
use std::collections::HashSet;
use std::f64::consts::PI;
use std::sync::Arc;

const BETA_STAR: f64 = 0.09;
const BETA1: f64 = 0.075;
const NU: f64 = 0.1;
/// The sector angle of SEC.
const SEC_ANGLE_DEG: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mesh {
    Box,
    Sec,
    /// Amendment 2.
    GBox,
}

fn geometry(mesh: Mesh) -> Arc<FvGeometry> {
    let m = match mesh {
        Mesh::Box => box_2d(16, 4, 1.0, 1.0, true, false).unwrap(),
        Mesh::Sec => annulus_3d_sector(8, 6, 4, 0.5, 1.0, 0.5, SEC_ANGLE_DEG.to_radians(), Grading::Uniform).unwrap(),
        Mesh::GBox => box_2d_graded(
            16,
            4,
            1.0,
            1.0,
            [Grading::Expansion(4.0), Grading::Uniform],
            true,
            false,
        )
        .unwrap(),
    };
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

fn walls(mesh: Mesh) -> &'static [&'static str] {
    match mesh {
        Mesh::Box | Mesh::GBox => &["ymin", "ymax"],
        Mesh::Sec => &["inner", "outer"],
    }
}

fn model(geom: &Arc<FvGeometry>, mesh: Mesh, mode: SeamDiffusivity) -> KOmegaSst {
    let mut m = KOmegaSst::new(geom.clone(), NU, walls(mesh), 1.0, 10.0).unwrap();
    m.set_periodic_diffusivity(mode).unwrap();
    m
}

/// The coordinate `s` of a point: `x` (BOX, GBOX) or `theta / angle` (SEC).
fn s_of(mesh: Mesh, p: [f64; 3]) -> f64 {
    match mesh {
        Mesh::Box | Mesh::GBox => p[0],
        Mesh::Sec => p[1].atan2(p[0]) / SEC_ANGLE_DEG.to_radians(),
    }
}

/// The pairs `(f, g)`, `f < g`, read from the partner maps of the geometry (independently of `periodic_pairs`).
fn seam_faces(geom: &FvGeometry) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, p) in geom.mesh.patches().iter().enumerate() {
        if let Some(map) = &geom.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                if f < map[local] {
                    out.push((f, map[local]));
                }
            }
        }
    }
    out
}

/// D1 computed in the test: `nu + w a_P + (1 - w) a_Q`, `w = dn_g / (dn_f + dn_g)`, `P`, `Q` the owners of `f < g`.
fn d1_value(geom: &FvGeometry, f: usize, g: usize, a: &[f64]) -> f64 {
    let own = geom.mesh.owner();
    let (dn_f, dn_g) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
    let w = dn_g / (dn_f + dn_g);
    NU + w * a[own[f]] + (1.0 - w) * a[own[g]]
}

// ---------------------------------------------------------------------------------------------------------------
// P0 and R3
// ---------------------------------------------------------------------------------------------------------------

/// P0: every periodic face in exactly one pair, each the other's partner, matched areas, positive distances.
fn p0(mesh: Mesh, expected: usize) {
    let geom = geometry(mesh);
    let pairs = geom.periodic_pairs().unwrap();
    let mut partner = std::collections::HashMap::new();
    let mut n_faces = 0;
    for (pi, p) in geom.mesh.patches().iter().enumerate() {
        if let Some(map) = &geom.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                partner.insert(f, map[local]);
                n_faces += 1;
            }
        }
    }
    let mut seen = HashSet::new();
    let sf = geom.mesh.face_area_vectors();
    let mut worst_area: f64 = 0.0;
    for p in &pairs {
        assert!(p.f < p.g, "P0 {mesh:?}: pair {p:?} not ordered");
        assert!(
            seen.insert(p.f) && seen.insert(p.g),
            "P0 {mesh:?}: a face in two pairs: {p:?}"
        );
        assert_eq!(partner.get(&p.f), Some(&p.g), "P0 {mesh:?}: {p:?}");
        assert_eq!(partner.get(&p.g), Some(&p.f), "P0 {mesh:?}: {p:?}");
        let (a, b) = (rcfd_core::vec3::norm(sf[p.f]), rcfd_core::vec3::norm(sf[p.g]));
        worst_area = worst_area.max((a - b).abs() / a);
        assert!((a - b).abs() <= 1e-12 * a, "P0 {mesh:?}: areas {a} {b}");
        let (dn_f, dn_g) = (geom.dn[p.f - geom.n_internal], geom.dn[p.g - geom.n_internal]);
        assert!(dn_f > 0.0 && dn_g > 0.0, "P0 {mesh:?}: dn {dn_f} {dn_g}");
    }
    assert_eq!(
        seen.len(),
        n_faces,
        "P0 {mesh:?}: {} faces in pairs of {n_faces}",
        seen.len()
    );
    assert_eq!(pairs.len(), expected, "P0 {mesh:?}: {} pairs", pairs.len());
    println!(
        "P0 {mesh:?}: {} pairs, {n_faces} periodic faces, largest relative area difference {worst_area:.3e}",
        pairs.len()
    );
}

/// P0: the pairs of BOX (4) and SEC (24).
#[test]
fn p0_periodic_pairs_of_both_meshes() {
    p0(Mesh::Box, 4);
    p0(Mesh::Sec, 24);
}

/// R3: a seam map that is not a pairing is refused for `pair`; `owner` does not read it.
#[test]
fn r3_a_broken_seam_map_is_refused() {
    let mut g = (*geometry(Mesh::Box)).clone();
    let pi = g.mesh.patches().iter().position(|p| p.name == "xmin").unwrap();
    let map = g.periodic_partner[pi].as_mut().unwrap();
    // two xmin faces with the same partner: the partner of map[1] is not xmin face 0
    map[0] = map[1];
    let geom = Arc::new(g);
    let e = geom
        .periodic_pairs()
        .expect_err("R3: periodic_pairs accepted a broken map");
    println!("R3: periodic_pairs: {e}");
    let mut m = KOmegaSst::new(geom.clone(), NU, walls(Mesh::Box), 1.0, 10.0).unwrap();
    let e = m
        .set_periodic_diffusivity(SeamDiffusivity::Pair)
        .expect_err("R3: pair accepted a broken map");
    println!("R3: set_periodic_diffusivity(Pair): {e}");
    assert_eq!(m.periodic_diffusivity(), SeamDiffusivity::Owner);
    m.set_periodic_diffusivity(SeamDiffusivity::Owner)
        .expect("R3: owner refused");
}

// ---------------------------------------------------------------------------------------------------------------
// F1 and G1
// ---------------------------------------------------------------------------------------------------------------

/// The F1 / G1 eddy viscosity `0.1 (1 + 0.5 sin(2 pi s))` of every cell.
fn nut_field(geom: &FvGeometry, mesh: Mesh) -> Vec<f64> {
    geom.mesh
        .cell_centres()
        .iter()
        .map(|&c| 0.1 * (1.0 + 0.5 * (2.0 * PI * s_of(mesh, c)).sin()))
        .collect()
}

/// `nu_face` of the F1 state in `owner` and in `pair` mode (the same model state).
fn face_viscosities(mesh: Mesh) -> (Arc<FvGeometry>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let geom = geometry(mesh);
    let mut m = model(&geom, mesh, SeamDiffusivity::Owner);
    m.nut = nut_field(&geom, mesh);
    let owner = m.nu_face();
    m.set_periodic_diffusivity(SeamDiffusivity::Pair).unwrap();
    let pair = m.nu_face();
    let nut = m.nut.clone();
    (geom, nut, owner, pair)
}

fn f1(mesh: Mesh) {
    let (geom, nut, owner, pair) = face_viscosities(mesh);
    let seam = seam_faces(&geom);
    let mut on_seam = vec![false; geom.mesh.n_faces()];
    let (mut unequal, mut not_d1) = (0, 0);
    let mut owner_diff: f64 = 0.0;
    for &(f, g) in &seam {
        on_seam[f] = true;
        on_seam[g] = true;
        if pair[f].to_bits() != pair[g].to_bits() {
            unequal += 1;
        }
        if pair[f].to_bits() != d1_value(&geom, f, g, &nut).to_bits() {
            not_d1 += 1;
        }
        owner_diff = owner_diff.max((owner[f] - owner[g]).abs());
    }
    let changed_elsewhere = (0..geom.mesh.n_faces())
        .filter(|&x| !on_seam[x] && pair[x].to_bits() != owner[x].to_bits())
        .count();
    let big = owner.iter().cloned().fold(0.0, f64::max);
    println!(
        "F1 {mesh:?}: {} pairs; pair: {unequal} with unequal bits, {not_d1} not the D1 bits, {changed_elsewhere} other faces changed; owner: largest pair difference {:.4e} of the largest nu_face {big:.4e} (must be >= 1e-3)",
        seam.len(),
        owner_diff / big
    );
    assert_eq!(unequal, 0, "F1 {mesh:?}: pairs with unequal bits");
    assert_eq!(not_d1, 0, "F1 {mesh:?}: pairs off the D1 bits");
    assert_eq!(changed_elsewhere, 0, "F1 {mesh:?}: faces off the seam changed");
    assert!(
        owner_diff >= 1e-3 * big,
        "F1 {mesh:?}: vacuous, {:.3e}",
        owner_diff / big
    );
}

/// F1: one value per pair, the bits of D1, nothing else changed; `owner` differs across the seam.
#[test]
fn f1_one_value_per_pair_and_the_d1_bits() {
    f1(Mesh::Box);
    f1(Mesh::Sec);
}

/// F1g (amendment 2): F1 on the graded box, where the weight of a pair is not 1/2.
#[test]
fn f1g_one_value_per_pair_on_a_graded_seam() {
    let geom = geometry(Mesh::GBox);
    let w: Vec<f64> = geom.periodic_pairs().unwrap().iter().map(|p| p.w).collect();
    println!("F1g: pair weights {w:?}");
    f1(Mesh::GBox);
}

/// G1 of one field: `abs(sum_c (A x - b)_c) / sum_f abs(F_f)` with the field's own boundary coefficients.
fn budget(geom: &FvGeometry, gamma: &[f64], x: &[f64], bc: &[rcfd_fv::FaceCoeffs]) -> f64 {
    let m = laplacian(geom, gamma, bc, None);
    let r = m.residual(x); // b - A x
    let total: f64 = r.iter().sum();
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut flux = 0.0;
    for f in 0..mesh.n_faces() {
        if f < geom.n_internal {
            flux += (gamma[f] * geom.lap_coeff[f] * (x[nei[f]] - x[own[f]])).abs();
        } else if !geom.empty[f] {
            let co = &bc[f - geom.n_internal];
            flux += (gamma[f] * rcfd_core::vec3::norm(mesh.face_area_vectors()[f]) * co.gradient(x[own[f]])).abs();
        }
    }
    total.abs() / flux
}

fn g1(mesh: Mesh) {
    let (geom, _, owner, pair) = face_viscosities(mesh);
    let cc = geom.mesh.cell_centres().to_vec();
    let mut fields: Vec<(String, Vec<f64>, Vec<rcfd_fv::FaceCoeffs>)> = Vec::new();
    let q = ScalarField::from_fn("q", &geom, |p| {
        1.0 + 0.5 * (2.0 * PI * s_of(mesh, p)).sin() + 0.2 * if mesh == Mesh::Box { p[1] } else { p[2] }
    });
    fields.push(("q".into(), q.values.clone(), q.boundary_coeffs(&geom)));
    if mesh == Mesh::Box {
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for (v, c) in u.values.iter_mut().zip(&cc) {
            *v = [
                (2.0 * PI * c[0]).sin() + 0.1 * c[1],
                0.5 * (2.0 * PI * c[0] + 1.0).sin(),
                0.0,
            ];
        }
        for (k, name) in [(0usize, "u_x"), (1, "u_y")] {
            let x: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            fields.push((name.into(), x, u.boundary_coeffs(&geom, k)));
        }
    }
    for (name, x, bc) in &fields {
        let (gp, go) = (budget(&geom, &pair, x, bc), budget(&geom, &owner, x, bc));
        println!("G1 {mesh:?} {name}: pair {gp:.3e} (limit 1e-13), owner {go:.3e} (must be >= 1e-5)");
        assert!(gp <= 1e-13, "G1 {mesh:?} {name}: pair {gp:.3e}");
        assert!(
            go >= 1e-5,
            "G1 {mesh:?} {name}: the owner control did not fire, {go:.3e}"
        );
    }
}

/// G1: the global diffusion budget closes with one value per pair and misses with the owner values.
#[test]
fn g1_global_diffusion_budget() {
    g1(Mesh::Box);
    g1(Mesh::Sec);
}

// ---------------------------------------------------------------------------------------------------------------
// C1p, C1pN, C2p, C2pN, C1p30: the transient budgets on the periodic box of ADR 0199 C1
// ---------------------------------------------------------------------------------------------------------------

/// Sweeps per step of C1p and C2p (ADR 0205 item 10: 30 leaves the seam lag at 1e-10 to 5e-10).
const M: usize = 60;

fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    }
}

/// BDF2 weights `(c0, c1, c2)` of a step of size `dt` after one of size `dt_old` (Euler without an old step).
fn weights(scheme: TimeScheme, dt: f64, dt_old: Option<f64>) -> (f64, f64, f64) {
    match (scheme, dt_old) {
        (TimeScheme::Bdf2, Some(d)) => {
            let r = dt / d;
            ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r))
        }
        _ => (1.0, 1.0, 0.0),
    }
}

/// One step of a C run.
#[derive(Debug, Clone, Copy)]
struct Step {
    /// `abs(B_k) / D_k`.
    bk: f64,
    /// `abs(B_w) / D_w`.
    bw: f64,
    /// Last-sweep change of k and omega, relative to their largest values.
    last_k: f64,
    last_w: f64,
    /// `max abs(k' - k^n)`, `max abs(w' - w^n)`.
    step_k: f64,
    step_w: f64,
    /// Largest seam `abs(nu_t,P - nu_t,Q)` after the step.
    seam_nut: f64,
    /// F1 = 1 in every cell of every sweep of the step.
    f1_one: bool,
}

fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

fn max_of(v: &[f64]) -> f64 {
    v.iter().cloned().fold(0.0, f64::max)
}

/// The C run: BOX, walls ymin / ymax with k, omega (and velocity) zero gradient, `u = (1, 0, 0)`, `k = 1 + 0.5 sin(2 pi x)`,
/// omega `10` or `10 - 5 sin(2 pi x)` (`opposed`), `dt = 0.05`, 10 steps, `sweeps` per step.
fn c_run(scheme: TimeScheme, conv: ConvScheme, mode: SeamDiffusivity, opposed: bool, sweeps: usize) -> Vec<Step> {
    let geom = geometry(Mesh::Box);
    let mut m = model(&geom, Mesh::Box, mode);
    m.solver = tight();
    m.convection = conv;
    m.k = ScalarField::from_fn("k", &geom, |x| 1.0 + 0.5 * (2.0 * PI * x[0]).sin());
    if opposed {
        m.omega = ScalarField::from_fn("omega", &geom, |x| 10.0 - 5.0 * (2.0 * PI * x[0]).sin());
    }
    for w in walls(Mesh::Box) {
        m.k.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
        m.omega.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    let u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    let phi = face_flux(&geom, &u.values, &u.boundary_values(&geom));
    let vol = geom.mesh.cell_volumes().to_vec();
    let own = geom.mesh.owner().to_vec();
    let seam = seam_faces(&geom);
    let dt = 0.05;
    let (mut k_old, mut k_n): (Option<Vec<f64>>, Vec<f64>) = (None, m.k.values.clone());
    let (mut w_old, mut w_n): (Option<Vec<f64>>, Vec<f64>) = (None, m.omega.values.clone());
    let mut dt_old = None;
    let mut out = Vec::new();
    for _ in 1..=10 {
        m.begin_step(dt, scheme);
        let mut w_star = m.omega.values.clone();
        let (mut last_k, mut last_w) = (0.0, 0.0);
        let mut f1_one = true;
        for s in 0..sweeps {
            if s + 1 == sweeps {
                w_star = m.omega.values.clone();
            }
            let (kb, wb) = (m.k.values.clone(), m.omega.values.clone());
            m.update(&u, &phi);
            f1_one &= m.f1().iter().all(|&f| f == 1.0);
            last_k = max_abs_diff(&m.k.values, &kb) / max_of(&m.k.values);
            last_w = max_abs_diff(&m.omega.values, &wb) / max_of(&m.omega.values);
        }
        let (c0, c1, c2) = weights(scheme, dt, dt_old);
        let (k, w) = (&m.k.values, &m.omega.values);
        let (mut bk, mut dk, mut bw, mut dw) = (0.0, 0.0, 0.0, 0.0);
        for c in 0..k.len() {
            let ko = k_old.as_ref().map_or(0.0, |o| o[c]);
            let wo = w_old.as_ref().map_or(0.0, |o| o[c]);
            bk += vol[c] * ((c0 * k[c] - c1 * k_n[c] + c2 * ko) / dt + BETA_STAR * w_star[c] * k[c]);
            dk += vol[c] * BETA_STAR * w_star[c] * k[c];
            bw += vol[c] * ((c0 * w[c] - c1 * w_n[c] + c2 * wo) / dt + BETA1 * w_star[c] * w[c]);
            dw += vol[c] * BETA1 * w_star[c] * w[c];
        }
        let seam_nut = seam
            .iter()
            .map(|&(f, g)| (m.nut[own[f]] - m.nut[own[g]]).abs())
            .fold(0.0, f64::max);
        out.push(Step {
            bk: bk.abs() / dk,
            bw: bw.abs() / dw,
            last_k,
            last_w,
            step_k: max_abs_diff(k, &k_n),
            step_w: max_abs_diff(w, &w_n),
            seam_nut,
            f1_one,
        });
        k_old = Some(std::mem::replace(&mut k_n, k.clone()));
        w_old = Some(std::mem::replace(&mut w_n, w.clone()));
        dt_old = Some(dt);
    }
    out
}

const SCHEMES: [TimeScheme; 2] = [TimeScheme::ImplicitEuler, TimeScheme::Bdf2];
const CONVS: [ConvScheme; 2] = [ConvScheme::Upwind, ConvScheme::LinearUpwind];

fn worst(steps: &[Step], f: impl Fn(&Step) -> f64) -> f64 {
    steps.iter().map(f).fold(0.0, f64::max)
}

fn least(steps: &[Step], f: impl Fn(&Step) -> f64) -> f64 {
    steps.iter().map(f).fold(f64::INFINITY, f64::min)
}

/// C1p / C2p with `pair`: the budgets close; preconditions and non-vacuity as registered.
fn c_pair(opposed: bool) {
    let id = if opposed { "C2p" } else { "C1p" };
    for scheme in SCHEMES {
        for conv in CONVS {
            let st = c_run(scheme, conv, SeamDiffusivity::Pair, opposed, M);
            for (i, s) in st.iter().enumerate() {
                println!(
                    "  {id} {scheme:?} {conv:?} step {}: B_k {:.3e}, B_w {:.3e}; last sweep k {:.1e}, w {:.1e}; step change k {:.3e}, w {:.3e}; seam nut {:.3e}; F1 = 1 {}",
                    i + 1,
                    s.bk,
                    s.bw,
                    s.last_k,
                    s.last_w,
                    s.step_k,
                    s.step_w,
                    s.seam_nut,
                    s.f1_one
                );
            }
            let (bk, bw) = (worst(&st, |s| s.bk), worst(&st, |s| s.bw));
            println!(
                "{id} {scheme:?} {conv:?}: largest |B_k| / D_k {bk:.3e}{} (limit 1e-10); largest last-sweep change k {:.1e}, w {:.1e} (<= 1e-13); smallest step change k {:.3e}{} (>= 1e-3); largest seam nut difference {:.3e} (>= 1e-3)",
                if opposed { format!(", |B_w| / D_w {bw:.3e}") } else { String::new() },
                worst(&st, |s| s.last_k),
                worst(&st, |s| s.last_w),
                least(&st, |s| s.step_k),
                if opposed { format!(", w {:.3e}", least(&st, |s| s.step_w)) } else { String::new() },
                worst(&st, |s| s.seam_nut)
            );
            // preconditions (a miss is a defective set-up)
            assert!(
                st.iter().all(|s| s.f1_one),
                "{id} {scheme:?} {conv:?}: DEFECTIVE SET-UP, F1 is not 1"
            );
            assert!(
                st.iter().all(|s| s.last_k <= 1e-13 && s.last_w <= 1e-13),
                "{id} {scheme:?} {conv:?}: DEFECTIVE SET-UP, the sweeps did not converge"
            );
            // non-vacuity
            assert!(least(&st, |s| s.step_k) >= 1e-3, "{id} {scheme:?} {conv:?}: vacuous k");
            if opposed {
                assert!(
                    least(&st, |s| s.step_w) >= 1e-3,
                    "{id} {scheme:?} {conv:?}: vacuous omega"
                );
            }
            assert!(
                worst(&st, |s| s.seam_nut) >= 1e-3,
                "{id} {scheme:?} {conv:?}: no seam difference"
            );
            // the limits
            assert!(bk <= 1e-10, "{id} {scheme:?} {conv:?}: B_k {bk:.3e}");
            if opposed {
                assert!(bw <= 1e-10, "{id} {scheme:?} {conv:?}: B_w {bw:.3e}");
            }
        }
    }
}

/// C1pN / C2pN with `owner`: the budgets miss at every step.
fn c_owner(opposed: bool) {
    let id = if opposed { "C2pN" } else { "C1pN" };
    for scheme in SCHEMES {
        for conv in CONVS {
            let st = c_run(scheme, conv, SeamDiffusivity::Owner, opposed, M);
            let (lk, wk) = (least(&st, |s| s.bk), worst(&st, |s| s.bk));
            let (lw, ww) = (least(&st, |s| s.bw), worst(&st, |s| s.bw));
            println!(
                "{id} {scheme:?} {conv:?}: |B_k| / D_k smallest {lk:.3e} (>= 1e-8), largest {wk:.3e} (>= 1e-3){}",
                if opposed {
                    format!("; |B_w| / D_w smallest {lw:.3e} (>= 1e-8), largest {ww:.3e} (>= 1e-3)")
                } else {
                    String::new()
                }
            );
            assert!(
                lk >= 1e-8 && wk >= 1e-3,
                "{id} {scheme:?} {conv:?}: the k control did not fire"
            );
            if opposed {
                assert!(
                    lw >= 1e-8 && ww >= 1e-3,
                    "{id} {scheme:?} {conv:?}: the omega control did not fire"
                );
            }
        }
    }
}

/// C1p: the k budget of ADR 0199 C1's periodic box closes with one diffusivity per pair.
#[test]
fn c1p_k_budget_on_the_periodic_box() {
    c_pair(false)
}

/// C1pN: with the owner diffusivities the k budget misses at every step (C1p can fail).
#[test]
fn c1pn_owner_misses_the_k_budget() {
    c_owner(false)
}

/// C2p: with opposed k and omega gradients both budgets close (the omega diffusion acts).
#[test]
fn c2p_k_and_omega_budgets_with_opposed_gradients() {
    c_pair(true)
}

/// C2pN: with the owner diffusivities both budgets miss at every step (C2p can fail).
#[test]
fn c2pn_owner_misses_both_budgets() {
    c_owner(true)
}

/// C1p30 (report only): C1p with 30 sweeps per step, the seam lag in the budgets.
#[test]
#[ignore = "report only (ADR 0205 C1p30): C1p with 30 sweeps per step"]
fn c1p30_report_with_30_sweeps() {
    for scheme in SCHEMES {
        for conv in CONVS {
            let st = c_run(scheme, conv, SeamDiffusivity::Pair, false, 30);
            println!(
                "C1p30 {scheme:?} {conv:?}: |B_k| / D_k largest {:.3e}, |B_w| / D_w largest {:.3e}; largest last-sweep change k {:.1e}, w {:.1e}",
                worst(&st, |s| s.bk),
                worst(&st, |s| s.bw),
                worst(&st, |s| s.last_k),
                worst(&st, |s| s.last_w)
            );
        }
    }
}
