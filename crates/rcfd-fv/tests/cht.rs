//! Conjugate heat transfer library checks (F-051, ADR 0024 groups B, C, D, E). Limits are the ones registered in
//! `docs/adr/0024-wave17b-cht.md` before any run.

mod cht_common;

use cht_common::*;
use rcfd_fv::cht::{
    conductive_face_flows, contact_faces, enthalpy, group_net_outflow, patch_heat_flow_out, ChtMaterials,
};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::{ConvectiveWall, FixedGradient, FixedValue, FvGeometry, Profile, ScalarField};
use rcfd_mesh::gen::{box_2d, box_2d_graded, channel, warp, Grading};
use rcfd_mesh::non_conformal::join_non_conformal;
use std::sync::Arc;

fn labels_by_x(geom: &FvGeometry, cuts: &[f64]) -> Vec<usize> {
    geom.mesh
        .cell_centres()
        .iter()
        .map(|c| cuts.iter().filter(|&&x| c[0] >= x).count())
        .collect()
}

/// Convective wall on `patch` with the owner-cell conductivity of every face.
fn conv_wall(geom: &FvGeometry, mats: &ChtMaterials, patch: &str, h: f64, t_inf: f64) -> ConvectiveWall {
    let own = geom.mesh.owner();
    ConvectiveWall {
        k: geom
            .mesh
            .patch(patch)
            .unwrap()
            .faces()
            .map(|f| mats.k_cell[own[f]])
            .collect(),
        h,
        t_inf,
        wall_resistance: 0.0,
        emissivity: 0.0,
        t_rad: 0.0,
    }
}

/// x of the faces between differing labels (1-D layered meshes), ascending.
fn interface_positions(geom: &FvGeometry, label: &[usize]) -> Vec<f64> {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let mut x: Vec<f64> = (0..geom.n_internal)
        .filter(|&f| label[own[f]] != label[nei[f]])
        .map(|f| geom.mesh.face_centres()[f][0])
        .collect();
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    x
}

#[allow(dead_code)]
struct Wall3 {
    geom: Arc<FvGeometry>,
    mats: ChtMaterials,
    en: EnergySolver,
    label: Vec<usize>,
    kf: Vec<f64>,
}

const K3: [f64; 3] = [1.0, 10.0, 0.1];
const RCP3: [f64; 3] = [3.0, 1.0, 7.0];

fn wall3(mesh: rcfd_mesh::Mesh, source: Option<f64>, rcp_ref: f64) -> Wall3 {
    let geom = geometry(mesh);
    let label = labels_by_x(&geom, &[0.2, 0.5]);
    let mats = labelled_materials(&label, &K3, &RCP3, rcp_ref);
    let mut t = ScalarField::uniform("T", &geom, 350.0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(400.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(conv_wall(&geom, &mats, "xmax", 5.0, 300.0)))
        .unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    if let Some(q) = source {
        let qv: Vec<f64> = label.iter().map(|&l| if l == 1 { q } else { 0.0 }).collect();
        en.source = Some(mats.source_in_kelvin_per_second(&qv));
    }
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (_, change) = converge(&mut en, &phi, 1e-11, 400);
    assert!(change < 1e-10, "not converged {change}");
    let kf = mats.face_conductivity(&geom);
    Wall3 {
        geom,
        mats,
        en,
        label,
        kf,
    }
}

/// Exact temperature of the three-layer wall with the layer interfaces at `xi`.
fn wall3_exact(xi: &[f64], lx: f64) -> impl Fn(f64) -> f64 {
    let xi = xi.to_vec();
    let edges = [0.0, xi[0], xi[1], lx];
    let rtot: f64 = (0..3).map(|i| (edges[i + 1] - edges[i]) / K3[i]).sum::<f64>() + 1.0 / 5.0;
    let q = 100.0 / rtot;
    move |x: f64| {
        let mut r = 0.0;
        for i in 0..3 {
            let (a, b) = (edges[i], edges[i + 1]);
            if x <= b {
                r += (x - a) / K3[i];
                break;
            }
            r += (b - a) / K3[i];
        }
        400.0 - q * r
    }
}

fn cell_error(w: &Wall3, exact: impl Fn(f64) -> f64) -> f64 {
    w.geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&w.en.t.values)
        .map(|(c, t)| (t - exact(c[0])).abs())
        .fold(0.0, f64::max)
}

#[test]
fn b1_three_layer_wall_uniform_and_graded() {
    for (name, mesh) in [
        ("uniform", box_2d(60, 1, 1.0, 1.0, false, false).unwrap()),
        (
            "graded",
            box_2d_graded(
                60,
                1,
                1.0,
                1.0,
                [Grading::Symmetric(3.0), Grading::Uniform],
                false,
                false,
            )
            .unwrap(),
        ),
    ] {
        let w = wall3(mesh, None, 2.0);
        let xi = interface_positions(&w.geom, &w.label);
        assert_eq!(xi.len(), 2);
        let err = cell_error(&w, wall3_exact(&xi, 1.0)) / 100.0;
        println!("B1 {name}: interfaces {xi:?}, max error/dT {err:.3e}");
        assert!(err <= 1e-10, "B1 {name}: {err}");
    }
}

#[test]
fn b2_interface_flux_continuity() {
    let w = wall3(box_2d(60, 1, 1.0, 1.0, false, false).unwrap(), None, 2.0);
    let flows = conductive_face_flows(&w.geom, &w.en.t, &w.kf, false);
    let left = patch_heat_flow_out(&w.geom, &flows, "xmin").unwrap().abs();
    let (own, nei) = (w.geom.mesh.owner(), w.geom.mesh.neighbour());
    let mut n = 0;
    for f in 0..w.geom.n_internal {
        if w.label[own[f]] != w.label[nei[f]] {
            let rel = (flows[f].abs() - left).abs() / left;
            println!("B2 interface face {f}: relative difference {rel:.3e}");
            assert!(rel <= 1e-10, "B2 interface {f}: {rel}");
            n += 1;
        }
    }
    assert_eq!(n, 2);
}

fn zone_balance(w: &Wall3, q: f64, non_orth: bool) -> (Vec<f64>, f64) {
    let flows = conductive_face_flows(&w.geom, &w.en.t, &w.kf, non_orth);
    let out = group_net_outflow(&w.geom, &flows, &w.label, 3);
    let vol = w.geom.mesh.cell_volumes();
    let v1: f64 = (0..w.label.len()).filter(|&c| w.label[c] == 1).map(|c| vol[c]).sum();
    (out, q * v1)
}

#[test]
fn b3_energy_balance_across_zones() {
    let q = 5e4;
    let w = wall3(box_2d(60, 1, 1.0, 1.0, false, false).unwrap(), Some(q), 2.0);
    let (out, qv) = zone_balance(&w, q, false);
    let errs = [out[0].abs() / qv, (out[1] - qv).abs() / qv, out[2].abs() / qv];
    let global = ((out[0] + out[1] + out[2]) - qv).abs() / qv;
    println!("B3 1-D: zone imbalances {errs:?}, global {global:.3e}");
    assert!(errs.iter().all(|&e| e <= 1e-9) && global <= 1e-9);
}

#[test]
fn b3b_energy_balance_on_warped_mesh() {
    let q = 5e3;
    let ly = 0.5;
    let base = box_2d(30, 15, 1.0, ly, false, false).unwrap();
    let mesh = warp(&base, |p| {
        [p[0] + 0.04 * (2.0 * std::f64::consts::PI * p[1] / ly).sin(), p[1], p[2]]
    })
    .unwrap();
    let geom = geometry(mesh);
    let label = labels_by_x(&geom, &[0.3, 0.6]);
    let mats = labelled_materials(&label, &K3, &RCP3, 2.0);
    let mut t = ScalarField::uniform("T", &geom, 330.0);
    t.set_bc(&geom, "xmin", Box::new(conv_wall(&geom, &mats, "xmin", 8.0, 300.0)))
        .unwrap();
    t.set_bc(&geom, "xmax", Box::new(conv_wall(&geom, &mats, "xmax", 12.0, 350.0)))
        .unwrap();
    t.set_bc(&geom, "ymin", Box::new(conv_wall(&geom, &mats, "ymin", 5.0, 320.0)))
        .unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::LinearUpwind);
    let qv: Vec<f64> = label.iter().map(|&l| if l == 1 { q } else { 0.0 }).collect();
    en.source = Some(mats.source_in_kelvin_per_second(&qv));
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (sweeps, change) = converge(&mut en, &phi, 1e-11, 800);
    println!("B3b converged in {sweeps} sweeps, change {change:.2e}");
    let kf = mats.face_conductivity(&geom);
    let w = Wall3 {
        geom,
        mats,
        en,
        label,
        kf,
    };
    let (out, qvol) = zone_balance(&w, q, true);
    let errs = [out[0].abs() / qvol, (out[1] - qvol).abs() / qvol, out[2].abs() / qvol];
    let global = ((out[0] + out[1] + out[2]) - qvol).abs() / qvol;
    println!("B3b warped: zone imbalances {errs:?}, global {global:.3e}");
    assert!(errs.iter().all(|&e| e <= 1e-9) && global <= 1e-9);
}

fn contact_run(n: usize, steps: usize) -> f64 {
    let (t1, t2) = (400.0, 300.0);
    let (k1, r1, k2, r2): (f64, f64, f64, f64) = (2.0, 1.0, 0.5, 2.0);
    let (e1, e2) = ((k1 * r1).sqrt(), (k2 * r2).sqrt());
    let ti = (e1 * t1 + e2 * t2) / (e1 + e2);
    let (a1, a2) = (k1 / r1, k2 / r2);
    let tend = 0.2;
    let base = box_2d(n, 1, 16.0, 1.0, false, false).unwrap();
    let mesh = warp(&base, |p| [p[0] - 8.0, p[1], p[2]]).unwrap();
    let geom = geometry(mesh);
    let label: Vec<usize> = geom.mesh.cell_centres().iter().map(|c| (c[0] > 0.0) as usize).collect();
    let mats = labelled_materials(&label, &[k1, k2], &[r1, r2], r1);
    let t0: Vec<f64> = label.iter().map(|&l| if l == 0 { t1 } else { t2 }).collect();
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    t.values = t0;
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    // the history was created from the uniform field in `EnergySolver::new`
    en.history = rcfd_fv::ddt::History::new(en.t.values.clone());
    let phi = vec![0.0; geom.mesh.n_faces()];
    let dt = tend / steps as f64;
    for _ in 0..steps {
        en.advance(&phi, dt, TimeScheme::Bdf2);
    }
    geom.mesh
        .cell_centres()
        .iter()
        .zip(&en.t.values)
        .map(|(c, t)| {
            let x = c[0];
            let exact = if x < 0.0 {
                t1 - (t1 - ti) * erfc(x.abs() / (2.0 * (a1 * tend).sqrt()))
            } else {
                t2 + (ti - t2) * erfc(x / (2.0 * (a2 * tend).sqrt()))
            };
            (t - exact).abs()
        })
        .fold(0.0, f64::max)
        / (t1 - t2)
}

#[test]
fn b4_transient_contact_of_semi_infinite_solids() {
    let e = [contact_run(320, 40), contact_run(640, 80), contact_run(1280, 160)];
    let orders = [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()];
    println!("B4 errors {e:?}, orders {orders:?}");
    assert!(e[2] <= 1e-3, "B4 finest error {}", e[2]);
    assert!(orders.iter().all(|&o| o >= 1.5), "B4 orders {orders:?}");
}

fn b5_run(scheme: TimeScheme) -> f64 {
    let geom = geometry(box_2d(60, 1, 1.0, 1.0, false, false).unwrap());
    let label = labels_by_x(&geom, &[0.2, 0.5]);
    let mats = labelled_materials(&label, &K3, &RCP3, 2.0);
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    t.values = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| 300.0 + 100.0 * (7.0 * c[0]).sin().powi(2))
        .collect();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    en.history = rcfd_fv::ddt::History::new(en.t.values.clone());
    let phi = vec![0.0; geom.mesh.n_faces()];
    let h0 = enthalpy(&geom, &mats.rho_cp_cell, &en.t.values);
    let mut worst: f64 = 0.0;
    for _ in 0..50 {
        en.advance(&phi, 0.05, scheme);
        let h = enthalpy(&geom, &mats.rho_cp_cell, &en.t.values);
        worst = worst.max((h - h0).abs() / h0.abs());
    }
    worst
}

#[test]
fn b5_adiabatic_enthalpy_conservation() {
    for (name, s) in [("euler", TimeScheme::ImplicitEuler), ("bdf2", TimeScheme::Bdf2)] {
        let w = b5_run(s);
        println!("B5 {name}: worst relative enthalpy drift {w:.3e}");
        assert!(w <= 1e-12, "B5 {name}: {w}");
    }
}

// ---------------------------------------------------------------- C. non-conformal

fn nonconformal_run(nl: [usize; 2], nr: [usize; 2]) -> (f64, f64, f64) {
    let block = |n: [usize; 2], x0: f64| {
        let m = channel([n[0], n[1], 1], [0.5, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
        warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
    };
    let (a, b) = (block(nl, 0.0), block(nr, 0.5));
    let (mesh, _) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
    let geom = geometry(mesh);
    let label = labels_by_x(&geom, &[0.5]);
    let mats = labelled_materials(&label, &[1.0, 5.0], &[1.0, 1.0], 1.0);
    let mut t = ScalarField::uniform("T", &geom, 350.0);
    t.set_bc(&geom, "inlet", Box::new(FixedValue::scalar(400.0))).unwrap();
    t.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(300.0))).unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::LinearUpwind);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (sweeps, change) = converge(&mut en, &phi, 1e-11, 1500);
    println!("C1 {nl:?}|{nr:?}: {sweeps} sweeps, change {change:.2e}");
    let kf = mats.face_conductivity(&geom);
    let q = 100.0 / (0.5 / 1.0 + 0.5 / 5.0);
    let ti = 400.0 - q * 0.5;
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&en.t.values)
        .map(|(c, t)| {
            let x = c[0];
            let ex = if x < 0.5 {
                400.0 - q * x
            } else {
                ti - q / 5.0 * (x - 0.5)
            };
            (t - ex).abs()
        })
        .fold(0.0, f64::max)
        / 100.0;
    let flows = conductive_face_flows(&geom, &en.t, &kf, true);
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let iface: f64 = (0..geom.n_internal)
        .filter(|&f| label[own[f]] != label[nei[f]])
        .map(|f| flows[f])
        .sum();
    let inlet = -patch_heat_flow_out(&geom, &flows, "inlet").unwrap();
    let outlet = patch_heat_flow_out(&geom, &flows, "outlet").unwrap();
    let cons = (iface - inlet).abs().max((iface - outlet).abs()) / iface.abs();
    (err, cons, iface)
}

#[test]
fn c1_non_conformal_composite_wall() {
    let (e1, cons1, _) = nonconformal_run([10, 20], [13, 27]);
    let (e2, cons2, _) = nonconformal_run([20, 40], [26, 54]);
    println!(
        "C1 error/dT {e1:.3e} -> {e2:.3e} (ratio {:.2}); conservation {cons1:.2e} {cons2:.2e}",
        e1 / e2
    );
    assert!(cons1 <= 1e-9 && cons2 <= 1e-9, "C1 conservation {cons1} {cons2}");
    assert!(e1 <= 1e-2, "C1 error {e1}");
    assert!(e1 / e2 >= 1.5, "C1 refinement ratio {}", e1 / e2);
}

// ---------------------------------------------------------------- D. duct with solid wall

const H: f64 = 1.0;
const TS: f64 = 0.5;
const KF: f64 = 1.0;
const KS: f64 = 10.0;
const UM: f64 = 1.0;
const QW: f64 = 1.0;
const T0: f64 = 300.0;

fn theta(y: f64) -> f64 {
    let beta = 3.0 * QW / (2.0 * KF * H);
    let eta = y / H;
    if y <= H {
        T0 + beta * H * H * (eta * eta / 2.0 - eta.powi(4) / 12.0)
    } else {
        theta(H) + QW / KS * (y - H)
    }
}

fn t_exact(x: f64, y: f64) -> f64 {
    // axial slope A = q / (rho cp U H) with the fluid capacity rho cp = 1
    x * QW / (UM * H) + theta(y)
}

/// Integral of the Poiseuille velocity over `[y0, y1]` within the fluid (zero in the solid).
fn u_integral(y0: f64, y1: f64) -> f64 {
    if y0 >= H - 1e-12 {
        return 0.0;
    }
    1.5 * UM * ((y1 - y0) - (y1.powi(3) - y0.powi(3)) / (3.0 * H * H))
}

struct Duct {
    geom: Arc<FvGeometry>,
    en: EnergySolver,
    phi: Vec<f64>,
    mats: ChtMaterials,
}

fn duct(nf: usize, midpoint_flux: bool, scheme: ConvScheme) -> Duct {
    let (nx, lx) = (8usize, 2.0);
    let ns = nf / 2;
    let mesh = box_2d(nx, nf + ns, lx, H + TS, false, false).unwrap();
    let geom = geometry(mesh);
    let cc = geom.mesh.cell_centres();
    let label: Vec<usize> = cc.iter().map(|c| (c[1] > H) as usize).collect();
    let mats = labelled_materials(&label, &[KF, KS], &[1.0, 4.0], 1.0);
    let mut t = ScalarField::uniform("T", &geom, T0 + 1.0);
    let cf = geom.mesh.face_centres();
    for (patch, x) in [("xmin", 0.0), ("xmax", lx)] {
        let prof: Vec<[f64; 3]> = geom
            .mesh
            .patch(patch)
            .unwrap()
            .faces()
            .map(|f| [t_exact(x, cf[f][1]), 0.0, 0.0])
            .collect();
        t.set_bc(&geom, patch, Box::new(FixedValue(Profile::PerFace(prof))))
            .unwrap();
    }
    t.set_bc(&geom, "ymax", Box::new(FixedGradient(Profile::scalar(QW / KS))))
        .unwrap();
    let mut en = cht_solver(&geom, t, &mats, scheme);
    let sf = geom.mesh.face_area_vectors();
    let depth = lx / nx as f64;
    let mut phi = vec![0.0; geom.mesh.n_faces()];
    for (f, p) in phi.iter_mut().enumerate() {
        if geom.empty[f] || sf[f][0].abs() < 1e-14 {
            continue;
        }
        let dy = sf[f][0].abs() / depth;
        let (yc, sign) = (cf[f][1], sf[f][0].signum());
        let (y0, y1) = (yc - 0.5 * dy, yc + 0.5 * dy);
        let u = if midpoint_flux {
            if yc < H {
                1.5 * UM * (1.0 - yc * yc / (H * H)) * dy
            } else {
                0.0
            }
        } else {
            u_integral(y0, y1)
        };
        *p = sign * u * depth;
    }
    let (_, change) = converge(&mut en, &phi, 1e-11, 60);
    assert!(change < 1e-9, "duct not converged: {change}");
    Duct { geom, en, phi, mats }
}

/// Relative error of the outer-surface-minus-bulk temperature difference at mid-length.
fn duct_error(d: &Duct) -> f64 {
    let geom = &d.geom;
    let cc = geom.mesh.cell_centres();
    let xc = cc.iter().map(|c| c[0]).fold(
        f64::INFINITY,
        |a, b| if (b - 1.0).abs() < (a - 1.0).abs() { b } else { a },
    );
    let own = geom.mesh.owner();
    let (mut num, mut den) = (0.0, 0.0);
    for f in 0..geom.n_internal {
        // x-faces on the right of a fluid cell in the column: cell flux
        let c = own[f];
        if (cc[c][0] - xc).abs() < 1e-9 && cc[c][1] < H && d.phi[f] > 0.0 && geom.mesh.face_area_vectors()[f][0] > 0.0 {
            num += d.phi[f] * d.en.t.values[c];
            den += d.phi[f];
        }
    }
    let tb = num / den;
    let bv = d.en.t.boundary_values(geom);
    let cf = geom.mesh.face_centres();
    let top = geom
        .mesh
        .patch("ymax")
        .unwrap()
        .faces()
        .find(|&f| (cf[f][0] - xc).abs() < 1e-9)
        .unwrap();
    let ttop = bv[top - geom.n_internal];
    let dt_num = ttop - tb;
    let dt_exact = QW * (17.0 * H / (35.0 * KF) + TS / KS);
    (dt_num - dt_exact).abs() / dt_exact
}

#[test]
#[ignore = "set-up defect (ADR 0024 addendum): upwind is not exact for the linear axial profile at the inlet face (the boundary face value is the face value, the interior faces carry the upstream cell value); kept as a record, corrected in *_central"]
fn d1_d2_nusselt_duct_acceptance_and_order() {
    let e = [16, 32, 64].map(|n| duct_error(&duct(n, false, ConvScheme::Upwind)));
    let o = [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()];
    println!("D1/D2 errors {e:?}, orders {o:?}");
    assert!(e[0] <= 1e-2, "D1 error at n_f = 16: {}", e[0]);
    assert!(o.iter().all(|&x| x >= 1.8), "D2 orders {o:?}");
}

#[test]
#[ignore = "set-up defect (ADR 0024 addendum), see d1_d2_nusselt_duct_acceptance_and_order; kept as a record, corrected in *_central"]
fn d3_duct_balance_and_interface_flux() {
    let d = duct(32, false, ConvScheme::Upwind);
    let geom = &d.geom;
    let kf = d.mats.face_conductivity(geom);
    let flows = conductive_face_flows(geom, &d.en.t, &kf, false);
    let bv = d.en.t.boundary_values(geom);
    let mut conv = 0.0;
    let mut cond = 0.0;
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        conv += d.phi[f] * bv[f - geom.n_internal] * d.mats.rho_cp_ref;
        cond += flows[f];
    }
    let top_area = patch_area(geom, "ymax");
    let imb = (conv + cond).abs() / (QW * top_area);
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let cc = geom.mesh.cell_centres();
    let iface: f64 = (0..geom.n_internal)
        .filter(|&f| (cc[own[f]][1] < H) != (cc[nei[f]][1] < H))
        .map(|f| flows[f])
        .sum();
    let rel_if = (iface.abs() - QW * top_area).abs() / (QW * top_area);
    println!("D3 balance {imb:.3e}, interface flux relative error {rel_if:.3e}");
    assert!(imb <= 1e-9, "D3 balance {imb}");
    assert!(rel_if <= 1e-3, "D3 interface flux {rel_if}");
}

#[test]
fn d1_d2_nusselt_duct_acceptance_and_order_central() {
    let e = [16, 32, 64].map(|n| duct_error(&duct(n, false, ConvScheme::Central)));
    let o = [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()];
    println!("D1/D2 errors {e:?}, orders {o:?}");
    assert!(e[0] <= 1e-2, "D1 error at n_f = 16: {}", e[0]);
    assert!(o.iter().all(|&x| x >= 1.8), "D2 orders {o:?}");
}

#[test]
fn d3_duct_balance_and_interface_flux_central() {
    let d = duct(32, false, ConvScheme::Central);
    let geom = &d.geom;
    let kf = d.mats.face_conductivity(geom);
    let flows = conductive_face_flows(geom, &d.en.t, &kf, false);
    let bv = d.en.t.boundary_values(geom);
    let mut conv = 0.0;
    let mut cond = 0.0;
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        conv += d.phi[f] * bv[f - geom.n_internal] * d.mats.rho_cp_ref;
        cond += flows[f];
    }
    let top_area = patch_area(geom, "ymax");
    let imb = (conv + cond).abs() / (QW * top_area);
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let cc = geom.mesh.cell_centres();
    let iface: f64 = (0..geom.n_internal)
        .filter(|&f| (cc[own[f]][1] < H) != (cc[nei[f]][1] < H))
        .map(|f| flows[f])
        .sum();
    let rel_if = (iface.abs() - QW * top_area).abs() / (QW * top_area);
    println!("D3 balance {imb:.3e}, interface flux relative error {rel_if:.3e}");
    assert!(imb <= 1e-9, "D3 balance {imb}");
    assert!(rel_if <= 1e-3, "D3 interface flux {rel_if}");
}

// ---------------------------------------------------------------- E. contact resistance, layered thin wall

#[test]
fn e1_contact_resistance_jump() {
    let (k1, k2, l, r) = (1.0, 10.0, 0.1, 0.02);
    let geom = geometry(box_2d(40, 1, l, 1.0, false, false).unwrap());
    let label = labels_by_x(&geom, &[0.05]);
    let mut mats = labelled_materials(&label, &[k1, k2], &[1.0, 1.0], 1.0);
    mats.contacts = contact_faces(&geom, &label, 0, 1, r);
    assert_eq!(mats.contacts.len(), 1);
    let mut t = ScalarField::uniform("T", &geom, 350.0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(400.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(300.0))).unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    let phi = vec![0.0; geom.mesh.n_faces()];
    converge(&mut en, &phi, 1e-11, 100);
    let q = 100.0 / (0.05 / k1 + r + 0.05 / k2);
    let ex = |x: f64| {
        if x < 0.05 {
            400.0 - q * x / k1
        } else {
            400.0 - q * 0.05 / k1 - q * r - q * (x - 0.05) / k2
        }
    };
    let cc = geom.mesh.cell_centres();
    let err = cc
        .iter()
        .zip(&en.t.values)
        .map(|(c, t)| (t - ex(c[0])).abs())
        .fold(0.0, f64::max)
        / 100.0;
    // extrapolated jump from the two cells next to the interface on each side
    let i = (0..40).find(|&c| label[c] == 1).unwrap();
    let tl = en.t.values[i - 1] + 0.5 * (en.t.values[i - 1] - en.t.values[i - 2]);
    let tr = en.t.values[i] - 0.5 * (en.t.values[i + 1] - en.t.values[i]);
    let jump = tl - tr;
    let rel = (jump - q * r).abs() / (q * r);
    println!(
        "E1 error/dT {err:.3e}, jump {jump:.9} vs {:.9}, relative {rel:.3e}",
        q * r
    );
    assert!(err <= 1e-10, "E1 temperature error {err}");
    assert!(rel <= 1e-10, "E1 jump {rel}");
}

#[test]
fn e2_thin_wall_equals_resolved_layers() {
    let (h, tinf, tr) = (25.0, 350.0, 290.0);
    let (kslab, lslab) = (1.5, 0.05);
    // thin wall version
    let thin = {
        let geom = geometry(box_2d(25, 1, lslab, 1.0, false, false).unwrap());
        let mats = uniform_materials(&geom, kslab, 1.0);
        let mut t = ScalarField::uniform("T", &geom, 320.0);
        let mut w = conv_wall(&geom, &mats, "xmin", h, tinf);
        w.wall_resistance = 0.002 / 0.5 + 0.001 / 45.0;
        t.set_bc(&geom, "xmin", Box::new(w)).unwrap();
        t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(tr))).unwrap();
        let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
        let phi = vec![0.0; geom.mesh.n_faces()];
        converge(&mut en, &phi, 1e-11, 100);
        (geom, en)
    };
    // resolved version: cells of width 0.001 (k 45), 0.002 (k 0.5), then 25 slab cells
    let resolved = {
        let n = 27usize;
        let base = box_2d(n, 1, 1.0, 1.0, false, false).unwrap();
        let (b1, b2) = (1.0 / n as f64, 2.0 / n as f64);
        let mesh = warp(&base, |p| {
            let u = p[0];
            let x = if u <= b1 {
                u / b1 * 0.001
            } else if u <= b2 {
                0.001 + (u - b1) / (b2 - b1) * 0.002
            } else {
                0.003 + (u - b2) / (1.0 - b2) * lslab
            };
            [x, p[1], p[2]]
        })
        .unwrap();
        let geom = geometry(mesh);
        let label: Vec<usize> = geom
            .mesh
            .cell_centres()
            .iter()
            .map(|c| {
                if c[0] < 0.001 {
                    0
                } else if c[0] < 0.003 {
                    1
                } else {
                    2
                }
            })
            .collect();
        let mats = labelled_materials(&label, &[45.0, 0.5, kslab], &[1.0, 1.0, 1.0], 1.0);
        let mut t = ScalarField::uniform("T", &geom, 320.0);
        t.set_bc(&geom, "xmin", Box::new(conv_wall(&geom, &mats, "xmin", h, tinf)))
            .unwrap();
        t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(tr))).unwrap();
        let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
        let phi = vec![0.0; geom.mesh.n_faces()];
        converge(&mut en, &phi, 1e-11, 100);
        (geom, en)
    };
    let slab_cells: Vec<f64> = resolved.1.t.values[2..].to_vec();
    let err = thin
        .1
        .t
        .values
        .iter()
        .zip(&slab_cells)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
        / (tinf - tr);
    // the cell centres of the slab coincide after the shift by the wall thickness
    let cx_thin = thin.0.mesh.cell_centres()[0][0];
    let cx_res = resolved.0.mesh.cell_centres()[2][0];
    assert!((cx_res - cx_thin - 0.003).abs() < 1e-12);
    println!("E2 thin wall versus resolved layers: max difference/dT {err:.3e}");
    assert!(err <= 1e-10, "E2 {err}");
}
