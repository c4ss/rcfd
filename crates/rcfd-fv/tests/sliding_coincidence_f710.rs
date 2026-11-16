//! F-710 (ADR 0638): the sliding interface at coinciding nodes. Checks A1, A1c (uniform flow on ADR 0216's pair P with a
//! step that lands on a node coincidence every whole rotor pitch), B1, B2 (`sliding_step` against a replica of the base
//! step of 76661520 wherever no coincidence occurs), S1, S2 (the coincidence steps) and N1 (the base step as negative
//! control). The limits are those registered in ADR 0638 before this code ran; each test prints every measured number and
//! then asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sliding::{interface_balance, sliding_step, SlidingStep};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d, cylinder_ogrid, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use std::f64::consts::TAU;
use std::sync::Arc;

const Z: Vec3 = [0.0, 0.0, 1.0];
/// UC's step: the rotor pitch over 8.
const DT_UC: f64 = TAU / 144.0 / 8.0;
const STEPS_UC: usize = 50;

/// P of ADR 0216.
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, 0.1).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, 0.1).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// P3 of ADR 0216.
fn pair_p3() -> SlidingInterface {
    let a = annulus_3d(48, 4, 2, 1.0, 1.5, 0.4, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(40, 4, 3, 1.5, 2.0, 0.4, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap()
}

/// LP of ADR 0638 (equal axial layers).
fn pair_lp() -> SlidingInterface {
    let a = annulus_3d(48, 4, 2, 1.0, 1.5, 0.4, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(40, 4, 2, 1.5, 2.0, 0.4, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap()
}

/// R of ADR 0638: the M3/M4 part of the base `sliding_step` (76661520), the same operations in the same order, returning
/// `(mesh_flux, phi_old, old_defect)`.
fn replica(
    old_geom: &FvGeometry,
    old: &SlidingMesh,
    new: &SlidingMesh,
    phi_old: &[f64],
    u_old: &[Vec3],
    dt: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let map = new.face_map_from(old).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(new.mesh.clone())).unwrap());
    let mesh = &geom.mesh;
    let (n, nf, ni) = (mesh.n_cells(), mesh.n_faces(), mesh.n_internal_faces());
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let faces = new.report.interface_faces.clone();
    let w = new.rotor_swept_volumes(old.angle).unwrap();
    let mut m: Vec<f64> = w.iter().map(|x| x / dt).collect();
    for (f, x) in m.iter_mut().enumerate() {
        if geom.empty[f] || new.is_interface(f) {
            *x = 0.0;
        }
    }
    let vol_old = old_geom.mesh.cell_volumes().to_vec();
    let vol_new = mesh.cell_volumes();
    let mut t: Vec<f64> = (0..n).map(|c| (vol_new[c] - vol_old[c]) / dt).collect();
    for f in 0..nf {
        if !new.is_interface(f) {
            t[own[f]] -= m[f];
            if f < ni {
                t[nei[f]] += m[f];
            }
        }
    }
    let gcl = interface_balance(mesh, &faces, &t).unwrap();
    for (k, &f) in faces.iter().enumerate() {
        m[f] = gcl.values[k];
    }
    let interp_new = face_flux(&geom, u_old, &vec![vec3::ZERO; mesh.n_boundary_faces()]);
    let interp_old = face_flux(old_geom, u_old, &vec![vec3::ZERO; old.mesh.n_boundary_faces()]);
    let mut phi = vec![0.0; nf];
    let mut old_defect = vec![0.0; ni];
    for f in 0..nf {
        match map[f] {
            Some(g) => {
                phi[f] = phi_old[g];
                if f < ni {
                    old_defect[f] = phi_old[g] - interp_old[g];
                }
            }
            None => phi[f] = interp_new[f],
        }
    }
    let mut e = vec![0.0; n];
    for f in 0..nf {
        let x = if new.is_interface(f) { 0.0 } else { phi[f] };
        e[own[f]] -= x;
        if f < ni {
            e[nei[f]] += x;
        }
    }
    for &f in &faces {
        e[own[f]] -= interp_new[f];
        e[nei[f]] += interp_new[f];
    }
    let cor = interface_balance(mesh, &faces, &e).unwrap();
    for (k, &f) in faces.iter().enumerate() {
        phi[f] = interp_new[f] + cor.values[k];
    }
    (m, phi, old_defect)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Whether the step equals the replica bit for bit.
fn equals_replica(st: &SlidingStep, r: &(Vec<f64>, Vec<f64>, Vec<f64>)) -> bool {
    bits(&st.mesh_flux) == bits(&r.0) && bits(&st.phi_old) == bits(&r.1) && bits(&st.old_defect) == bits(&r.2)
}

/// The coupling faces of D1, counted here from the step's mesh (an independent union-find).
fn n_coupling(st: &SlidingStep) -> usize {
    let m = &st.geom.mesh;
    let (own, nei) = (m.owner(), m.neighbour());
    let mut part: Vec<usize> = (0..m.n_cells()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        p[i] = r;
        r
    }
    let mut touched = vec![false; m.n_cells()];
    let mut on = vec![false; m.n_faces()];
    for &f in &st.interface_faces {
        on[f] = true;
        touched[own[f]] = true;
        touched[nei[f]] = true;
        let (a, b) = (find(&mut part, own[f]), find(&mut part, nei[f]));
        part[a] = b;
    }
    (0..m.n_internal_faces())
        .filter(|&f| !on[f] && touched[own[f]] && touched[nei[f]] && find(&mut part, own[f]) != find(&mut part, nei[f]))
        .count()
}

/// PS of ADR 0216 with `nu = 0.1`.
fn settings() -> PimpleSettings {
    let tight = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter: 5000,
    };
    let mut s = PimpleSettings::piso(0.1);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 2;
    s.momentum_solver = tight;
    s.pressure_solver = tight;
    s
}

/// Per-step record of a uniform run.
#[derive(Debug, Clone)]
struct StepRec {
    step: usize,
    faces: usize,
    eu: f64,
    ep: f64,
    equal: bool,
    flux: f64,
    gcl: f64,
    coupling: usize,
}

/// ADR 0216 U2's loop (`uniform_run`) on `sc` with `u0`, `dt`, `steps`; with `use_replica` the step data are the
/// replica's (N1), otherwise `sliding_step`'s, each step compared with the replica.
fn uniform_run(sc: &SlidingInterface, u0: Vec3, dt: f64, steps: usize, use_replica: bool) -> Vec<StepRec> {
    let mut cur = sc.mesh_at(0.0).unwrap();
    let g = Arc::new(FvGeometry::new(Arc::new(cur.mesh.clone())).unwrap());
    let names: Vec<String> = cur
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != rcfd_mesh::PatchKind::Empty)
        .map(|p| p.name.clone())
        .collect();
    let mut u = VectorField::uniform("U", &g, u0);
    for n in &names {
        u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSolver::new(g, u, p, settings());
    let un = vec3::norm(u0);
    let mut out = vec![];
    for k in 1..=steps {
        let new = sc.mesh_at(k as f64 * dt).unwrap();
        let mut st = sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let r = replica(&s.geom, &cur, &new, &s.phi, &s.u.values, dt);
        let equal = equals_replica(&st, &r);
        let coupling = n_coupling(&st);
        let (flux, gcl) = (st.flux_residual, st.gcl_residual);
        if use_replica {
            st.mesh_flux = r.0;
            st.phi_old = r.1;
            st.old_defect = r.2;
        }
        s.step_sliding(dt, st, vec![]).unwrap();
        let eu =
            s.u.values
                .iter()
                .fold(0.0f64, |a, v| a.max(vec3::norm(vec3::sub(*v, u0))))
                / un;
        let p0 = s.p.values[0];
        let ep = s.p.values.iter().fold(0.0f64, |a, x| a.max((x - p0).abs())) / (un * un);
        out.push(StepRec {
            step: k,
            faces: new.report.interface_faces.len(),
            eu,
            ep,
            equal,
            flux,
            gcl,
            coupling,
        });
        cur = new;
    }
    out
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

fn require(fails: &Fails, ids: &[&str]) {
    let bad: Vec<_> = fails.iter().filter(|x| ids.contains(&x.0)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

/// A1, A1c, B1, S1, S2 on UC.
#[test]
fn uc_uniform_flow_through_node_coincidences() {
    let recs = uniform_run(&pair_p(), [1.0, 0.5, 0.0], DT_UC, STEPS_UC, false);
    for r in &recs {
        println!(
            "UC step {:2}: {} interface faces, velocity {:.2e}, pressure {:.2e}, replica equal {}, flux residual {:.2e}, GCL {:.2e}, coupling faces {}",
            r.step, r.faces, r.eu, r.ep, r.equal, r.flux, r.gcl, r.coupling
        );
    }
    let coinc: Vec<&StepRec> = recs.iter().filter(|r| r.step % 8 == 0).collect();
    let regain: Vec<&StepRec> = recs.iter().filter(|r| r.step % 8 == 1 && r.step > 1).collect();
    let mut fails = Fails::new();
    let eu = recs.iter().fold(0.0f64, |a, r| a.max(r.eu));
    let ep = recs.iter().fold(0.0f64, |a, r| a.max(r.ep));
    check(
        &mut fails,
        "A1",
        eu <= 1e-10 && ep <= 1e-10,
        format!(
            "UC, {} steps: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)",
            recs.len()
        ),
    );
    let worst = |v: &[&StepRec]| v.iter().fold((0.0f64, 0.0f64), |a, r| (a.0.max(r.eu), a.1.max(r.ep)));
    let (cu, cp) = worst(&coinc);
    let (gu, gp) = worst(&regain);
    check(
        &mut fails,
        "A1c",
        coinc.len() == 6 && regain.len() == 6 && cu <= 1e-10 && cp <= 1e-10 && gu <= 1e-10 && gp <= 1e-10,
        format!(
            "{} coincidence steps {:?}: velocity {cu:.2e}, pressure {cp:.2e}; {} regain steps {:?}: velocity {gu:.2e}, pressure {gp:.2e} (1e-10 each)",
            coinc.len(),
            coinc.iter().map(|r| r.step).collect::<Vec<_>>(),
            regain.len(),
            regain.iter().map(|r| r.step).collect::<Vec<_>>()
        ),
    );
    let differ: Vec<usize> = recs.iter().filter(|r| !r.equal).map(|r| r.step).collect();
    let expected: Vec<usize> = (1..=STEPS_UC).filter(|k| k % 8 == 0).collect();
    check(
        &mut fails,
        "B1",
        differ == expected,
        format!(
            "steps where sliding_step differs from the replica {differ:?} (exactly {expected:?}); {} steps bit-equal",
            recs.len() - differ.len()
        ),
    );
    let (fl, gc) = coinc
        .iter()
        .fold((0.0f64, 0.0f64), |a, r| (a.0.max(r.flux), a.1.max(r.gcl)));
    check(
        &mut fails,
        "S1",
        fl <= 1e-12 && gc <= 1e-12,
        format!("coincidence steps: flux residual {fl:.2e}, GCL residual {gc:.2e} (1e-12 each)"),
    );
    let faces: Vec<usize> = coinc.iter().map(|r| r.faces).collect();
    let cpl: Vec<usize> = coinc.iter().map(|r| r.coupling).collect();
    check(
        &mut fails,
        "S2",
        cpl.iter().all(|&c| c == 48),
        format!("coincidence steps: interface faces {faces:?}, coupling faces {cpl:?} (48 each)"),
    );
    require(&fails, &["A1", "A1c", "B1", "S1", "S2"]);
}

/// B2: U2 of ADR 0216 (P, `dt = 0.005`, 100 steps), U3 (P3, `dt = 0.01`, 40 steps) and LP (`dt = 0.01`, 20 steps), every
/// step against the replica.
#[test]
fn b2_no_coincidence_runs_equal_the_base_step() {
    let mut fails = Fails::new();
    for (name, sc, u0, dt, steps) in [
        ("U2", pair_p(), [1.0, 0.5, 0.0], 0.005, 100),
        ("U3", pair_p3(), [1.0, 0.5, 0.3], 0.01, 40),
        ("LP", pair_lp(), [1.0, 0.5, 0.3], 0.01, 20),
    ] {
        let recs = uniform_run(&sc, u0, dt, steps, false);
        let differ: Vec<usize> = recs.iter().filter(|r| !r.equal).map(|r| r.step).collect();
        let eu = recs.iter().fold(0.0f64, |a, r| a.max(r.eu));
        let faces: Vec<usize> = recs.iter().map(|r| r.faces).collect();
        let (fmin, fmax) = (faces.iter().min().unwrap(), faces.iter().max().unwrap());
        check(
            &mut fails,
            "B2",
            differ.is_empty(),
            format!(
                "{name}: {} steps, differ from the replica on {differ:?} (none); interface faces {fmin} to {fmax}; velocity {eu:.2e}",
                recs.len()
            ),
        );
    }
    require(&fails, &["B2"]);
}

/// N1: UC with the replica's step data (the base step).
#[test]
fn n1_the_base_step_loses_the_uniform_state() {
    let recs = uniform_run(&pair_p(), [1.0, 0.5, 0.0], DT_UC, STEPS_UC, true);
    let eu = recs.iter().fold(0.0f64, |a, r| a.max(r.eu));
    let first = recs.iter().find(|r| r.eu > 1e-10).map(|r| r.step);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "N1",
        eu >= 1e-6,
        format!("UC with the base step: velocity {eu:.2e} |U| (>= 1e-6), first step above 1e-10: {first:?}"),
    );
    require(&fails, &["N1"]);
}
