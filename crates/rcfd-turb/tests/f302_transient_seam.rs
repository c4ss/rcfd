//! ADR 0227 (F-302), library checks of one diffusivity per periodic seam pair on the transient branches that `pimple` runs
//! (F-294): K1t, the k budget of one k-epsilon sweep after `begin_step(0.05, euler)`, and A1t, the `nuTilda` budget of one
//! Spalart-Allmaras sweep after `begin_time_step(0.05, euler)` with `nuTilda < 0` at seam cells, each with its negative control
//! (`owner`). The set-ups, the lag term and the budget are those of ADR 0212 K1 and A1 (`f296_seam_diffusivity.rs`); the limits
//! are those registered in ADR 0227 before this code ran.
//!
//! BOX: `box_2d(16, 4, 1, 1)` periodic in x. GBOX (ADR 0205 amendment 2): BOX graded by 4 in x (`w = 0.8`).

use rcfd_core::vec3;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, Grading};
use rcfd_turb::sa::{fv2, fw, r_function, s_tilde};
use rcfd_turb::{
    wall_cell_sources, KEpsilon, KEpsilonVariant, SLimiter, SaWallMode, SeamDiffusivity, SpalartAllmaras, WallTreatment,
};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.1;
/// The step of K1t and A1t.
const DT: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mesh {
    Box,
    GBox,
}

fn geometry(mesh: Mesh) -> Arc<FvGeometry> {
    let m = match mesh {
        Mesh::Box => box_2d(16, 4, 1.0, 1.0, true, false).unwrap(),
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

const WALLS: [&str; 2] = ["ymin", "ymax"];

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

fn weight(geom: &FvGeometry, f: usize, g: usize) -> f64 {
    let (dn_f, dn_g) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
    dn_g / (dn_f + dn_g)
}

fn nut_field(geom: &FvGeometry) -> Vec<f64> {
    geom.mesh
        .cell_centres()
        .iter()
        .map(|&c| 0.1 * (1.0 + 0.5 * (2.0 * PI * c[0]).sin()))
        .collect()
}

fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    }
}

fn frozen_u(geom: &FvGeometry) -> VectorField {
    VectorField::from_fn("U", geom, |p| [0.0, 1.0 + 0.5 * (2.0 * PI * p[0] + 0.5).sin(), 0.0])
}

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

/// K1t: one transient k-epsilon sweep (k) after `begin_step(dt, euler)`. Returns the k sweep and the smallest k after it.
fn k1t_run(mesh: Mesh, mode: SeamDiffusivity, dt: f64) -> (Arc<FvGeometry>, Sweep, f64) {
    let geom = geometry(mesh);
    let mut m = KEpsilon::new(KEpsilonVariant::Standard, geom.clone(), NU, &WALLS, 1.0, 1.0).unwrap();
    m.solver = tight();
    m.k = ScalarField::from_fn("k", &geom, |p| 1.0 + 0.5 * (2.0 * PI * p[0]).sin());
    m.eps = ScalarField::from_fn("epsilon", &geom, |p| 1.0 + 0.3 * (2.0 * PI * p[0] + 1.0).sin());
    m.nut = nut_field(&geom);
    m.set_periodic_diffusivity(mode).unwrap();
    let u = frozen_u(&geom);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (ks, es, nuts) = (m.k.values.clone(), m.eps.values.clone(), m.nut.clone());
    let s = rcfd_turb::eddy::strain_rate(&geom, &u);
    m.begin_step(dt, TimeScheme::ImplicitEuler);
    assert!(m.is_transient(), "DEFECTIVE SET-UP: the step did not start");
    m.update(&u, &phi);
    let kn = m.k.values.clone();
    let vol = geom.mesh.cell_volumes();
    let own = geom.mesh.owner();
    let n = geom.n_cells();
    let mut wf = vec![usize::MAX; n];
    for w in WALLS {
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
            p * vol[c] - e / ks[c].max(1e-30) * vol[c] * kn[c] - vol[c] * (kn[c] - ks[c]) / dt
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

/// A1t: one transient Spalart-Allmaras sweep (`nuTilda`) after `begin_time_step(dt, euler)`; also the number of seam owner cells
/// with `nuTilda < 0`.
fn a1t_run(mesh: Mesh, mode: SeamDiffusivity, dt: f64) -> (Arc<FvGeometry>, Sweep, usize) {
    let geom = geometry(mesh);
    let mut m = SpalartAllmaras::new(geom.clone(), NU, &WALLS, 0.1).unwrap();
    m.wall_mode = SaWallMode::Resolved;
    m.solver = tight();
    m.nutilde = ScalarField::from_fn("nuTilda", &geom, |p| 0.1 * ((2.0 * PI * p[0]).sin() - 0.1));
    for w in WALLS {
        m.nutilde.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    m.set_periodic_diffusivity(mode).unwrap();
    let u = frozen_u(&geom);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let xs = m.nutilde.values.clone();
    let nb = m.nutilde.boundary_values(&geom);
    let gnt = green_gauss(&geom, &xs, &nb, 0);
    let gr = rcfd_turb::les::velocity_gradients(&geom, &u);
    let d: Vec<f64> = m.wall_distance().iter().map(|&x| x.max(1e-30)).collect();
    m.begin_time_step(dt, TimeScheme::ImplicitEuler);
    assert!(m.is_transient(), "DEFECTIVE SET-UP: the step did not start");
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

/// The registered limits of a sweep budget with a negative control (as ADR 0212 `check_sweep`).
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

/// K1t: the k budget of a transient k-epsilon sweep closes with one face eddy viscosity per pair; the owner sweep misses it.
#[test]
fn k1t_transient_kepsilon_sweep_budget() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair, kmin_p) = k1t_run(mesh, SeamDiffusivity::Pair, DT);
        let (_, owner, kmin_o) = k1t_run(mesh, SeamDiffusivity::Owner, DT);
        println!("K1t {mesh:?}: smallest k after the sweep: pair {kmin_p:.4e}, owner {kmin_o:.4e} (> 1e-10)");
        assert!(kmin_p > 1e-10 && kmin_o > 1e-10, "K1t {mesh:?}: the floor of k acted");
        check_sweep(&format!("K1t {mesh:?}"), &geom, &pair, &owner);
    }
}

/// A1t: with `nuTilda < 0` at seam cells the transient SA sweep closes its budget with the D1 value on both faces of every pair;
/// the owner sweep misses it.
#[test]
fn a1t_transient_sa_sweep_budget_with_negative_nutilde_at_the_seam() {
    for mesh in [Mesh::Box, Mesh::GBox] {
        let (geom, pair, neg) = a1t_run(mesh, SeamDiffusivity::Pair, DT);
        let (_, owner, _) = a1t_run(mesh, SeamDiffusivity::Owner, DT);
        println!("A1t {mesh:?}: {neg} seam owner cells with nuTilda < 0");
        assert!(
            neg > 0,
            "A1t {mesh:?}: DEFECTIVE SET-UP, no negative nuTilda at the seam"
        );
        check_sweep(&format!("A1t {mesh:?}"), &geom, &pair, &owner);
    }
}
