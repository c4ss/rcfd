//! ADR 0212 (F-296), library checks of one diffusivity per periodic seam pair in the other models: F2 the momentum face values of
//! every model (one value per pair, the bits of D1, nothing else changed), G2 the global diffusion budget of those values, R3' the
//! refusal of a seam map that is not a pairing, M1 the momentum-source budget of the Reynolds stress model, and the sweep budgets
//! K1 (k-epsilon, k), A1 / A1p (Spalart-Allmaras, `nuTilda`) and Q1 (Reynolds stress model, the stresses and epsilon) with their
//! negative controls (`owner`). The limits are those registered in ADR 0212 before this code ran.
//!
//! BOX: `box_2d(16, 4, 1, 1)` periodic in x. SEC: `annulus_3d_sector(8, 6, 4, 0.5, 1.0, 0.5, 40 degrees)`, a rotational seam.
//! GBOX (ADR 0205 amendment 2): BOX graded by 4 in x, so the two faces of a pair are at different distances (`w = 0.8`).

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, box_2d, box_2d_graded, Grading};
use rcfd_turb::rsm::{homogeneous_rates, new_rsm, stabilisation, tensor_divergence, trace};
use rcfd_turb::sa::{fv2, fw, r_function, s_tilde};
use rcfd_turb::{
    create_model, wall_cell_sources, KEpsilon, KEpsilonVariant, MixingProfile, ModelContext, ReynoldsStress, SLimiter,
    SaWallMode, SeamDiffusivity, SpalartAllmaras, TurbulenceModel, WallTreatment,
};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.1;
/// The sector angle of SEC.
const SEC_ANGLE_DEG: f64 = 40.0;
/// The models of F2, G2 and R3'.
const MODELS: [&str; 6] = [
    "k_epsilon",
    "spalart_allmaras",
    "smagorinsky",
    "wale",
    "mixing_length",
    "rsm",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mesh {
    Box,
    Sec,
    GBox,
}

const MESHES: [Mesh; 3] = [Mesh::Box, Mesh::Sec, Mesh::GBox];

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

/// The weight `w = dn_g / (dn_f + dn_g)` of the owner of `f`.
fn weight(geom: &FvGeometry, f: usize, g: usize) -> f64 {
    let (dn_f, dn_g) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
    dn_g / (dn_f + dn_g)
}

/// The context of the F2, G2, R3' and M1 models: `nu = 0.1`, the model walls, `k0 = 1`, `eps0 = 1`, `nutilde0 = 0.1`,
/// `length_scale = 0.5`, the case-tree defaults otherwise.
fn ctx(geom: &Arc<FvGeometry>, mesh: Mesh) -> ModelContext {
    ModelContext {
        geom: geom.clone(),
        nu: NU,
        walls: walls(mesh).iter().map(|s| s.to_string()).collect(),
        k0: 1.0,
        eps0: 1.0,
        omega0: 10.0,
        nutilde0: 0.1,
        variant: KEpsilonVariant::Standard,
        wall_treatment: WallTreatment::Smooth,
        sa_wall_mode: SaWallMode::Auto,
        sa_limiter: SLimiter::Allmaras,
        sa_negative: true,
        kappa: 0.41,
        e_wall: 9.8,
        convection: ConvScheme::Upwind,
        relax_turb: 0.7,
        relax_nut: 0.5,
        wall_damping: false,
        length_scale: 0.5,
        mixing_profile: MixingProfile::Linear,
        time_accurate: false,
    }
}

/// The F field `0.1 (1 + 0.5 sin(2 pi s))` of every cell.
fn nut_field(geom: &FvGeometry, mesh: Mesh) -> Vec<f64> {
    geom.mesh
        .cell_centres()
        .iter()
        .map(|&c| 0.1 * (1.0 + 0.5 * (2.0 * PI * s_of(mesh, c)).sin()))
        .collect()
}

/// The G velocity of ADR 0205 G1, `(sin(2 pi x) + 0.1 y, 0.5 sin(2 pi x + 1), 0)`.
fn g_velocity(geom: &FvGeometry) -> VectorField {
    VectorField::from_fn("U", geom, |c| {
        [
            (2.0 * PI * c[0]).sin() + 0.1 * c[1],
            0.5 * (2.0 * PI * c[0] + 1.0).sin(),
            0.0,
        ]
    })
}

// ---------------------------------------------------------------------------------------------------------------
// F2, G2, R3'
// ---------------------------------------------------------------------------------------------------------------

/// `nu_face` of the F state in `owner` and in `pair` mode (the same model state), and the model's `nut`.
fn face_viscosities(key: &str, mesh: Mesh) -> (Arc<FvGeometry>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let geom = geometry(mesh);
    let mut m = create_model(key, &ctx(&geom, mesh)).unwrap();
    m.initialize_from_nut(&nut_field(&geom, mesh));
    let owner = m.nu_face();
    m.set_option("periodic_diffusivity", "pair")
        .unwrap_or_else(|e| panic!("F2 {key} {mesh:?}: pair refused: {e}"));
    let pair = m.nu_face();
    (geom, m.nut().to_vec(), owner, pair)
}

fn f2(key: &str, mesh: Mesh) {
    let (geom, nut, owner, pair) = face_viscosities(key, mesh);
    let own = geom.mesh.owner();
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
        let w = weight(&geom, f, g);
        let d1 = NU + w * nut[own[f]] + (1.0 - w) * nut[own[g]];
        if pair[f].to_bits() != d1.to_bits() {
            not_d1 += 1;
        }
        owner_diff = owner_diff.max((owner[f] - owner[g]).abs());
    }
    let changed_elsewhere = (0..geom.mesh.n_faces())
        .filter(|&x| !on_seam[x] && pair[x].to_bits() != owner[x].to_bits())
        .count();
    let big = owner.iter().cloned().fold(0.0, f64::max);
    println!(
        "F2 {key} {mesh:?}: {} pairs; pair: {unequal} with unequal bits, {not_d1} not the D1 bits, {changed_elsewhere} other faces changed; owner: largest pair difference {:.4e} of the largest nu_face {big:.4e} (must be >= 1e-3)",
        seam.len(),
        owner_diff / big
    );
    assert!(!seam.is_empty(), "F2 {key} {mesh:?}: no seam");
    assert_eq!(unequal, 0, "F2 {key} {mesh:?}: pairs with unequal bits");
    assert_eq!(not_d1, 0, "F2 {key} {mesh:?}: pairs off the D1 bits");
    assert_eq!(changed_elsewhere, 0, "F2 {key} {mesh:?}: faces off the seam changed");
    assert!(
        owner_diff >= 1e-3 * big,
        "F2 {key} {mesh:?}: vacuous, {:.3e}",
        owner_diff / big
    );
}

/// F2: one value per pair, the bits of D1, nothing else changed, for every model and mesh; `owner` differs across the seam.
#[test]
fn f2_one_momentum_face_viscosity_per_pair_for_every_model() {
    for key in MODELS {
        for mesh in MESHES {
            f2(key, mesh);
        }
    }
}

/// G of ADR 0205 G1: `abs(sum_c (A x - b)_c) / sum_f abs(F_f)` with the field's own boundary coefficients.
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
            flux += (gamma[f] * vec3::norm(mesh.face_area_vectors()[f]) * co.gradient(x[own[f]])).abs();
        }
    }
    total.abs() / flux
}

/// The G fields: `q` (every mesh), `u_x` and `u_y` of the G velocity (BOX, GBOX).
fn g_fields(geom: &Arc<FvGeometry>, mesh: Mesh) -> Vec<(String, Vec<f64>, Vec<rcfd_fv::FaceCoeffs>)> {
    let mut fields = Vec::new();
    let q = ScalarField::from_fn("q", geom, |p| {
        1.0 + 0.5 * (2.0 * PI * s_of(mesh, p)).sin() + 0.2 * if mesh == Mesh::Sec { p[2] } else { p[1] }
    });
    fields.push(("q".to_string(), q.values.clone(), q.boundary_coeffs(geom)));
    if mesh != Mesh::Sec {
        let u = g_velocity(geom);
        for (k, name) in [(0usize, "u_x"), (1, "u_y")] {
            let x: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            fields.push((name.to_string(), x, u.boundary_coeffs(geom, k)));
        }
    }
    fields
}

/// G2: the global diffusion budget with the momentum face viscosity closes with one value per pair and misses with the owner
/// values, for every model and mesh.
#[test]
fn g2_global_diffusion_budget_for_every_model() {
    for key in MODELS {
        for mesh in MESHES {
            let (geom, _, owner, pair) = face_viscosities(key, mesh);
            for (name, x, bc) in g_fields(&geom, mesh) {
                let (gp, go) = (budget(&geom, &pair, &x, &bc), budget(&geom, &owner, &x, &bc));
                println!("G2 {key} {mesh:?} {name}: pair {gp:.3e} (limit 1e-13), owner {go:.3e} (must be >= 1e-5)");
                assert!(gp <= 1e-13, "G2 {key} {mesh:?} {name}: pair {gp:.3e}");
                assert!(
                    go >= 1e-5,
                    "G2 {key} {mesh:?} {name}: the owner control did not fire, {go:.3e}"
                );
            }
        }
    }
}

/// R3': a seam map that is not a pairing is refused for `pair` by every model, which keeps the owner values; `owner` is accepted.
#[test]
fn r3_a_broken_seam_map_is_refused_by_every_model() {
    let mut g = (*geometry(Mesh::Box)).clone();
    let pi = g.mesh.patches().iter().position(|p| p.name == "xmin").unwrap();
    let map = g.periodic_partner[pi].as_mut().unwrap();
    // two xmin faces with the same partner: the partner of map[1] is not xmin face 0 (ADR 0205 R3)
    map[0] = map[1];
    let geom = Arc::new(g);
    for key in MODELS {
        let mut m = create_model(key, &ctx(&geom, Mesh::Box)).unwrap();
        m.initialize_from_nut(&nut_field(&geom, Mesh::Box));
        let before = m.nu_face();
        let e = m
            .set_option("periodic_diffusivity", "pair")
            .expect_err("R3': pair accepted a broken map");
        let after = m.nu_face();
        let same = before.iter().zip(&after).all(|(a, b)| a.to_bits() == b.to_bits());
        println!("R3' {key}: set_option(pair): {e}; nu_face unchanged: {same}");
        assert!(same, "R3' {key}: the refused option changed nu_face");
        m.set_option("periodic_diffusivity", "owner")
            .unwrap_or_else(|e| panic!("R3' {key}: owner refused: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// M1: the momentum source of the Reynolds stress model
// ---------------------------------------------------------------------------------------------------------------

/// `abs(sum_c V_c (s_c + d_c)_i) / sum_c abs(V_c (s_c + d_c)_i)` for `i = x, y`.
fn m1_run(mesh: Mesh, pair: bool) -> [f64; 2] {
    let geom = geometry(mesh);
    let mut m: ReynoldsStress = new_rsm(&ctx(&geom, mesh)).unwrap();
    m.initialize_from_nut(&nut_field(&geom, mesh));
    if pair {
        TurbulenceModel::set_option(&mut m, "periodic_diffusivity", "pair").unwrap();
    }
    let u = g_velocity(&geom);
    let src = m.momentum_source(&u);
    let n = geom.n_cells();
    let st: Vec<_> = (0..n).map(|c| m.stress(c)).collect();
    let div = tensor_divergence(&geom, &st);
    let vol = geom.mesh.cell_volumes();
    [0, 1].map(|k| {
        let v: Vec<f64> = (0..n).map(|c| vol[c] * (src[c][k] + div[c][k])).collect();
        v.iter().sum::<f64>().abs() / v.iter().map(|x| x.abs()).sum::<f64>()
    })
}

/// M1: the Laplacian part of the RSM momentum source is conservative with one viscosity per pair, not with the owner values.
#[test]
fn m1_rsm_momentum_source_budget() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (p, o) = (m1_run(mesh, true), m1_run(mesh, false));
        println!(
            "M1 {mesh:?}: pair x {:.3e}, y {:.3e} (limit 1e-13); owner x {:.3e}, y {:.3e} (must be >= 1e-5)",
            p[0], p[1], o[0], o[1]
        );
        for k in 0..2 {
            assert!(p[k] <= 1e-13, "M1 {mesh:?} component {k}: pair {:.3e}", p[k]);
            assert!(
                o[k] >= 1e-5,
                "M1 {mesh:?} component {k}: the owner control did not fire, {:.3e}",
                o[k]
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Sweep budgets K1, A1, A1p, Q1
// ---------------------------------------------------------------------------------------------------------------

fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    }
}

/// The frozen velocity of the sweep budgets, `(0, 1 + 0.5 sin(2 pi x + 0.5), 0)`, normal to the walls ymin / ymax.
fn frozen_u(geom: &FvGeometry) -> VectorField {
    VectorField::from_fn("U", geom, |p| [0.0, 1.0 + 0.5 * (2.0 * PI * p[0] + 0.5).sin(), 0.0])
}

/// The seam lag term `L = sum_pairs [G_f |S_f| (x'_P - x*_Q) + G_g |S_g| (x'_Q - x*_P)] / (dn_f + dn_g)` for the face values
/// `(f, g, G_f, G_g)` of every pair.
fn lag_term(geom: &FvGeometry, pairs: &[(usize, usize, f64, f64)], xs: &[f64], xn: &[f64]) -> f64 {
    let own = geom.mesh.owner();
    let sf = geom.mesh.face_area_vectors();
    let mut l = 0.0;
    for &(f, g, gf, gg) in pairs {
        let (p, q) = (own[f], own[g]);
        let s = geom.dn[f - geom.n_internal] + geom.dn[g - geom.n_internal];
        l += (gf * vec3::norm(sf[f]) * (xn[p] - xs[q]) + gg * vec3::norm(sf[g]) * (xn[q] - xs[p])) / s;
    }
    l
}

/// One equation of one sweep: the cell terms, the field before and after, and the face values of the existing rule and of D1.
struct Sweep {
    r: Vec<f64>,
    xs: Vec<f64>,
    xn: Vec<f64>,
    owner: Vec<(usize, usize, f64, f64)>,
    d1: Vec<(usize, usize, f64, f64)>,
}

impl Sweep {
    fn b(&self, geom: &FvGeometry, pairs: &[(usize, usize, f64, f64)]) -> f64 {
        let sum: f64 = self.r.iter().sum();
        let n: f64 = self.r.iter().map(|x| x.abs()).sum();
        (sum - lag_term(geom, pairs, &self.xs, &self.xn)).abs() / n
    }
    /// `(B(L_owner), B(L_D1), max abs(x' - x*))`.
    fn budgets(&self, geom: &FvGeometry) -> (f64, f64, f64) {
        let step = self
            .xn
            .iter()
            .zip(&self.xs)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        (self.b(geom, &self.owner), self.b(geom, &self.d1), step)
    }
}

/// K1: one k-epsilon sweep (k). Returns the k sweep and the smallest k after it.
fn k1_run(mesh: Mesh, mode: SeamDiffusivity) -> (Arc<FvGeometry>, Sweep, f64) {
    let geom = geometry(mesh);
    let mut m = KEpsilon::new(KEpsilonVariant::Standard, geom.clone(), NU, walls(mesh), 1.0, 1.0).unwrap();
    m.relax_turb = 1.0;
    m.relax_nut = 1.0;
    m.solver = tight();
    m.k = ScalarField::from_fn("k", &geom, |p| 1.0 + 0.5 * (2.0 * PI * p[0]).sin());
    m.eps = ScalarField::from_fn("epsilon", &geom, |p| 1.0 + 0.3 * (2.0 * PI * p[0] + 1.0).sin());
    m.nut = nut_field(&geom, mesh);
    m.set_periodic_diffusivity(mode).unwrap();
    let u = frozen_u(&geom);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (ks, es, nuts) = (m.k.values.clone(), m.eps.values.clone(), m.nut.clone());
    let s = rcfd_turb::eddy::strain_rate(&geom, &u);
    m.update(&u, &phi);
    let kn = m.k.values.clone();
    let vol = geom.mesh.cell_volumes();
    let own = geom.mesh.owner();
    // the nearest wall face of every wall cell (as the model)
    let n = geom.n_cells();
    let mut wf = vec![usize::MAX; n];
    for w in walls(mesh) {
        for f in geom.mesh.patch(w).unwrap().faces() {
            let c = own[f];
            if wf[c] == usize::MAX || geom.dn[f - geom.n_internal] < geom.dn[wf[c] - geom.n_internal] {
                wf[c] = f;
            }
        }
    }
    let r: Vec<f64> = (0..n)
        .map(|c| {
            let (p, e) = if wf[c] != usize::MAX {
                let f = wf[c];
                let nrm = geom.normal(f);
                let v = u.values[c];
                let ut = vec3::norm(vec3::sub(v, vec3::scale(nrm, vec3::dot(v, nrm))));
                let y = geom.dn[f - geom.n_internal];
                wall_cell_sources(WallTreatment::Smooth, &mut false, &m.wc, NU, ks[c], y, ut)
            } else {
                (nuts[c] * s[c] * s[c], es[c])
            };
            p * vol[c] - e / ks[c].max(1e-30) * vol[c] * kn[c]
        })
        .collect();
    let sk = m.sigma_k;
    let seam = seam_faces(&geom);
    let owner = seam
        .iter()
        .map(|&(f, g)| (f, g, NU + nuts[own[f]] / sk, NU + nuts[own[g]] / sk))
        .collect();
    let d1 = seam
        .iter()
        .map(|&(f, g)| {
            let w = weight(&geom, f, g);
            let v = NU + (w * nuts[own[f]] + (1.0 - w) * nuts[own[g]]) / sk;
            (f, g, v, v)
        })
        .collect();
    let kmin = kn.iter().cloned().fold(f64::INFINITY, f64::min);
    (
        geom,
        Sweep {
            r,
            xs: ks,
            xn: kn,
            owner,
            d1,
        },
        kmin,
    )
}

/// The registered limits of a sweep budget with a negative control (K1, A1, Q1).
fn check_sweep(id: &str, geom: &FvGeometry, pair: &Sweep, owner: &Sweep) {
    let (p_own, p_d1, p_step) = pair.budgets(geom);
    let (o_own, o_d1, o_step) = owner.budgets(geom);
    println!(
        "{id}: pair B(L_D1) {p_d1:.3e} (limit 1e-12), B(L_owner) {p_own:.3e}; owner B(L_owner) {o_own:.3e} (harness, <= 1e-12), B(L_D1) {o_d1:.3e} (must be >= 1e-6); step change pair {p_step:.3e}, owner {o_step:.3e} (>= 1e-3)"
    );
    assert!(
        o_own <= 1e-12,
        "{id}: DEFECTIVE SET-UP, the harness does not close the owner sweep: {o_own:.3e}"
    );
    assert!(p_step >= 1e-3 && o_step >= 1e-3, "{id}: vacuous step");
    assert!(o_d1 >= 1e-6, "{id}: the owner control did not fire, {o_d1:.3e}");
    assert!(p_d1 <= 1e-12, "{id}: pair B(L_D1) {p_d1:.3e}");
}

/// K1: the k budget of a k-epsilon sweep closes with one face eddy viscosity per pair; the owner sweep misses it.
#[test]
fn k1_kepsilon_sweep_budget() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair, kmin_p) = k1_run(mesh, SeamDiffusivity::Pair);
        let (_, owner, kmin_o) = k1_run(mesh, SeamDiffusivity::Owner);
        println!("K1 {mesh:?}: smallest k after the sweep: pair {kmin_p:.4e}, owner {kmin_o:.4e} (> 1e-10)");
        assert!(kmin_p > 1e-10 && kmin_o > 1e-10, "K1 {mesh:?}: the floor of k acted");
        check_sweep(&format!("K1 {mesh:?}"), &geom, &pair, &owner);
    }
}

/// A1 / A1p: one Spalart-Allmaras sweep (`nuTilda`) after `begin_step(0.05)` from `nuTilda = init(x)`; also the number of seam
/// owner cells with `nuTilda < 0`.
fn a1_run(mesh: Mesh, mode: SeamDiffusivity, init: impl Fn(f64) -> f64) -> (Arc<FvGeometry>, Sweep, usize) {
    let geom = geometry(mesh);
    let mut m = SpalartAllmaras::new(geom.clone(), NU, walls(mesh), 0.1).unwrap();
    m.wall_mode = SaWallMode::Resolved;
    m.solver = tight();
    m.nutilde = ScalarField::from_fn("nuTilda", &geom, |p| init(p[0]));
    for w in walls(mesh) {
        m.nutilde.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    m.set_periodic_diffusivity(mode).unwrap();
    let u = frozen_u(&geom);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let dt = 0.05;
    let xs = m.nutilde.values.clone();
    let nb = m.nutilde.boundary_values(&geom);
    let gnt = green_gauss(&geom, &xs, &nb, 0);
    let gr = rcfd_turb::les::velocity_gradients(&geom, &u);
    let d: Vec<f64> = m.wall_distance().iter().map(|&x| x.max(1e-30)).collect();
    m.begin_step(dt);
    m.update(&u, &phi);
    let xn = m.nutilde.values.clone();
    let cf = m.coef;
    let cw1 = cf.cw1();
    let vol = geom.mesh.cell_volumes();
    let n = geom.n_cells();
    let fnv = |x: f64| -> f64 {
        if x < 0.0 {
            let chi3 = (x / NU).powi(3);
            (cf.cn1 + chi3) / (cf.cn1 - chi3)
        } else {
            1.0
        }
    };
    let r: Vec<f64> = (0..n)
        .map(|c| {
            let mut w2 = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    let wij = 0.5 * (gr[c][i][j] - gr[c][j][i]);
                    w2 += wij * wij;
                }
            }
            let vort = (2.0 * w2).sqrt();
            let x = xs[c];
            let g2 = vec3::dot(gnt[c], gnt[c]);
            let mut b = cf.cb2 / cf.sigma * g2 * vol[c];
            let mut diag = 0.0;
            if x >= 0.0 {
                let chi = x / NU;
                let sb = x * fv2(chi, cf.cv1) / (cf.kappa * cf.kappa * d[c] * d[c]);
                let st = s_tilde(vort, sb, SLimiter::Allmaras, cf.c2, cf.c3);
                let rr = r_function(x, st, cf.kappa, d[c]);
                let fwv = fw(rr, cf.cw2, cf.cw3);
                b += cf.cb1 * st.max(0.0) * x * vol[c];
                diag = cw1 * fwv * x / (d[c] * d[c]) * vol[c];
            } else {
                b += (cf.cb1 * vort * x + cw1 * x * x / (d[c] * d[c])) * vol[c];
            }
            b - diag * xn[c] - vol[c] * (xn[c] - xs[c]) / dt
        })
        .collect();
    let own = geom.mesh.owner();
    let ni = geom.n_internal;
    let seam = seam_faces(&geom);
    let owner = seam
        .iter()
        .map(|&(f, g)| {
            let (bf, bg) = (nb[f - ni], nb[g - ni]);
            (f, g, (NU + fnv(bf) * bf) / cf.sigma, (NU + fnv(bg) * bg) / cf.sigma)
        })
        .collect();
    let d1 = seam
        .iter()
        .map(|&(f, g)| {
            let w = weight(&geom, f, g);
            let (a, b) = (fnv(xs[own[f]]) * xs[own[f]], fnv(xs[own[g]]) * xs[own[g]]);
            let v = (NU + w * a + (1.0 - w) * b) / cf.sigma;
            (f, g, v, v)
        })
        .collect();
    let negative = seam
        .iter()
        .flat_map(|&(f, g)| [own[f], own[g]])
        .filter(|&c| xs[c] < 0.0)
        .count();
    (geom, Sweep { r, xs, xn, owner, d1 }, negative)
}

fn a1_init(x: f64) -> f64 {
    0.1 * ((2.0 * PI * x).sin() - 0.1)
}

fn a1p_init(x: f64) -> f64 {
    0.1 * (1.0 + 0.5 * (2.0 * PI * x).sin())
}

/// A1: with `nuTilda < 0` at seam cells the SA sweep closes its budget with the D1 value on both faces of every pair; the owner
/// sweep (one value per pair too, but the value at the periodic face) misses it.
#[test]
fn a1_sa_sweep_budget_with_negative_nutilde_at_the_seam() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair, neg) = a1_run(mesh, SeamDiffusivity::Pair, a1_init);
        let (_, owner, _) = a1_run(mesh, SeamDiffusivity::Owner, a1_init);
        println!("A1 {mesh:?}: {neg} seam owner cells with nuTilda < 0");
        assert!(
            neg > 0,
            "A1 {mesh:?}: DEFECTIVE SET-UP, no negative nuTilda at the seam"
        );
        check_sweep(&format!("A1 {mesh:?}"), &geom, &pair, &owner);
    }
}

/// A1p: with positive `nuTilda` the existing rule is D1 (ADR 0212 item 2): both sweeps close both budgets.
#[test]
fn a1p_sa_sweep_budget_with_positive_nutilde() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair, _) = a1_run(mesh, SeamDiffusivity::Pair, a1p_init);
        let (_, owner, _) = a1_run(mesh, SeamDiffusivity::Owner, a1p_init);
        let (_, p_d1, p_step) = pair.budgets(&geom);
        let (o_own, o_d1, o_step) = owner.budgets(&geom);
        println!(
            "A1p {mesh:?}: pair B(L_D1) {p_d1:.3e} (<= 1e-12); owner B(L_owner) {o_own:.3e}, B(L_D1) {o_d1:.3e} (both <= 1e-12); step change {p_step:.3e}, {o_step:.3e} (>= 1e-3)"
        );
        assert!(p_step >= 1e-3 && o_step >= 1e-3, "A1p {mesh:?}: vacuous step");
        assert!(p_d1 <= 1e-12, "A1p {mesh:?}: pair {p_d1:.3e}");
        assert!(
            o_own <= 1e-12 && o_d1 <= 1e-12,
            "A1p {mesh:?}: owner {o_own:.3e} {o_d1:.3e}"
        );
    }
}

/// The stress components `xx, yy, zz, xy, xz, yz` of the model (`COMPS` of rsm.rs).
const COMPS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)];

/// Q1: one sweep of the Reynolds stress model after `begin_step(0.05)`; one [`Sweep`] per equation (6 stresses, epsilon).
fn q1_run(mesh: Mesh, mode: SeamDiffusivity) -> (Arc<FvGeometry>, Vec<Sweep>) {
    let geom = geometry(mesh);
    let mut m = ReynoldsStress::new(geom.clone(), NU, 1.0, 1.0);
    m.solver = tight();
    let kf = |x: f64| 1.0 + 0.5 * (2.0 * PI * x).sin();
    for i in 0..3 {
        m.r[i] = ScalarField::from_fn(["Rxx", "Ryy", "Rzz"][i], &geom, |p| 2.0 / 3.0 * kf(p[0]));
    }
    m.eps = ScalarField::from_fn("epsilon", &geom, |p| 1.0 + 0.3 * (2.0 * PI * p[0] + 1.0).sin());
    m.set_periodic_diffusivity(mode).unwrap();
    let u = frozen_u(&geom);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let dt = 0.05;
    let n = geom.n_cells();
    let field = |m: &ReynoldsStress, q: usize| {
        if q < 6 {
            m.r[q].values.clone()
        } else {
            m.eps.values.clone()
        }
    };
    let xs: Vec<Vec<f64>> = (0..7).map(|q| field(&m, q)).collect();
    let stresses: Vec<_> = (0..n).map(|c| m.stress(c)).collect();
    let gr = rcfd_turb::les::velocity_gradients(&geom, &u);
    let (ps, cf) = (m.ps, m.coef);
    let mut rate = vec![[0.0; 7]; n];
    let mut lam = vec![[0.0; 7]; n];
    let mut koe = vec![0.0; n];
    for c in 0..n {
        let t = stresses[c];
        let eps = xs[6][c];
        let (dr, deps) = homogeneous_rates(ps, &cf, &t, eps, &gr[c]);
        let l = stabilisation(ps, &cf, &t, eps, &gr[c]);
        for (q, &(i, j)) in COMPS.iter().enumerate() {
            rate[c][q] = dr[i][j];
            lam[c][q] = l[i][j];
        }
        rate[c][6] = deps;
        lam[c][6] = cf.ce2(ps) * eps / (0.5 * trace(&t)).max(1e-30);
        koe[c] = (0.5 * trace(&t)).max(1e-30) / eps.max(1e-30);
    }
    m.begin_step(dt);
    m.update(&u, &phi);
    let xn: Vec<Vec<f64>> = (0..7).map(|q| field(&m, q)).collect();
    let vol = geom.mesh.cell_volumes();
    let own = geom.mesh.owner();
    let seam = seam_faces(&geom);
    // the normal stress of cell c on face f (before the sweep)
    let rnn = |c: usize, f: usize| -> f64 {
        let nrm = geom.normal(f);
        let mut s = 0.0;
        for (q, &(i, j)) in COMPS.iter().enumerate() {
            let fac = if i == j { 1.0 } else { 2.0 };
            s += fac * nrm[i] * nrm[j] * xs[q][c];
        }
        s
    };
    let sweeps = (0..7)
        .map(|q| {
            let cc = if q < 6 { cf.c_s } else { cf.c_eps };
            let r = (0..n)
                .map(|c| vol[c] * (rate[c][q] - lam[c][q] * (xn[q][c] - xs[q][c]) - (xn[q][c] - xs[q][c]) / dt))
                .collect();
            let owner = seam
                .iter()
                .map(|&(f, g)| {
                    let (a, b) = (own[f], own[g]);
                    (
                        f,
                        g,
                        NU + cc * koe[a] * rnn(a, f).max(0.0),
                        NU + cc * koe[b] * rnn(b, g).max(0.0),
                    )
                })
                .collect();
            let d1 = seam
                .iter()
                .map(|&(f, g)| {
                    let (a, b) = (own[f], own[g]);
                    let w = weight(&geom, f, g);
                    let v =
                        NU + cc * (w * koe[a] + (1.0 - w) * koe[b]) * (w * rnn(a, f) + (1.0 - w) * rnn(b, g)).max(0.0);
                    (f, g, v, v)
                })
                .collect();
            Sweep {
                r,
                xs: xs[q].clone(),
                xn: xn[q].clone(),
                owner,
                d1,
            }
        })
        .collect();
    (geom.clone(), sweeps)
}

/// Q1: the stress and epsilon budgets of a Reynolds stress sweep close with one diffusivity per pair; the owner sweep misses
/// them. `Rxz` and `Ryz` stay zero in this set-up and are not checked.
#[test]
fn q1_rsm_sweep_budgets() {
    const NAMES: [&str; 7] = ["Rxx", "Ryy", "Rzz", "Rxy", "Rxz", "Ryz", "epsilon"];
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair) = q1_run(mesh, SeamDiffusivity::Pair);
        let (_, owner) = q1_run(mesh, SeamDiffusivity::Owner);
        for q in 0..7 {
            let zero = pair[q].r.iter().chain(&owner[q].r).all(|&x| x == 0.0);
            if q == 4 || q == 5 {
                println!("Q1 {mesh:?} {}: all cell terms zero: {zero} (not checked)", NAMES[q]);
                assert!(zero, "Q1 {mesh:?} {}: DEFECTIVE SET-UP, not zero", NAMES[q]);
                continue;
            }
            assert!(!zero, "Q1 {mesh:?} {}: DEFECTIVE SET-UP, all zero", NAMES[q]);
            check_sweep(&format!("Q1 {mesh:?} {}", NAMES[q]), &geom, &pair[q], &owner[q]);
        }
    }
}
