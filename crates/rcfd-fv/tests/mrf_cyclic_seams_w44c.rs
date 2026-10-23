//! Wave 44C (ADR 0113): MRF (absolute-velocity frozen-rotor) zones with rotationally cyclic seams. A 60-degree sector that is an
//! exact sixth of a full annulus is compared with that annulus: solid-body rotation (K1), Taylor-Couette (K2), an axial
//! through-flow with a rotor band (K3); the empty passage rotating as a whole is an exact discrete solution (K4); the ADR 0109
//! sector joined on a cylinder runs with its inner block as the MRF zone (K5). Construction checks G, negative controls D1 / D4.
//! Limits registered in ADR 0113 before this code ran.

use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::MrfZone;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_3d, annulus_sector, box_mesh, cylinder_ogrid, warp, Grading};
use rcfd_mesh::non_conformal::join_non_conformal_cylindrical;
use rcfd_mesh::Mesh;
use std::f64::consts::FRAC_PI_3;
use std::sync::Arc;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const NU: f64 = 0.1;
const W_I: f64 = 1.0;
const V_I: f64 = 0.5;
/// Sector-against-annulus limit as a fraction of the velocity scale: the smallest frame-change difference of ADR 0008 M3.
const L_DIFF: f64 = 4.3e-4;

fn tc(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -W_I * RI * RI / d * r + W_I * RI * RI * RO * RO / d / r
}

fn full_2d() -> Mesh {
    cylinder_ogrid(48, 16, RI, RO, Grading::Uniform, 0.1).unwrap()
}

fn sector_2d() -> Mesh {
    annulus_sector(8, 16, RI, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap()
}

/// 3-D 60-degree sector, 8 radial x 8 around x 12 axial over 0 <= z <= 1.2: a box (r, theta, z) bent about the z axis; patches
/// xmin = inner, xmax = outer, ymin / ymax = the cyclic pair, zmin, zmax.
fn sector_3d() -> Mesh {
    let b = box_mesh([8, 8, 12], [1.0, 1.0, 1.2], [Grading::Uniform; 3]).unwrap();
    let mut m = warp(&b, |p| {
        let (r, th) = (RI + p[0] * (RO - RI), p[1] * FRAC_PI_3);
        [r * th.cos(), r * th.sin(), p[2]]
    })
    .unwrap();
    m.set_periodic_rotation("ymin", "ymax", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
        .unwrap();
    m
}

fn full_3d() -> Mesh {
    annulus_3d(48, 8, 12, RI, RO, 1.2, 0.0, Grading::Uniform).unwrap()
}

/// The ADR 0109 G2 sector: inner block 8 x 8 (r 0.5-0.75) joined to 6 x 8 (r 0.75-1) on the cylinder, cyclic again; with the
/// interface faces.
fn joined() -> (Mesh, Vec<usize>) {
    let a = annulus_sector(8, 8, RI, 0.75, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let b = annulus_sector(6, 8, 0.75, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let (mut m, rep) =
        join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
    m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
        .unwrap();
    (m, rep.interface_faces)
}

fn mrf_zone(omega: f64, seams: bool) -> MrfZone {
    MrfZone {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega,
        cyclic_seams: seams,
    }
}

type ZoneFn = fn(&FvGeometry) -> Vec<bool>;

fn all(g: &FvGeometry) -> Vec<bool> {
    vec![true; g.n_cells()]
}

fn band(g: &FvGeometry) -> Vec<bool> {
    g.mesh.cell_centres().iter().map(|c| c[2] > 0.4 && c[2] < 0.8).collect()
}

fn inner_block(g: &FvGeometry) -> Vec<bool> {
    g.mesh.cell_centres().iter().map(|c| c[0].hypot(c[1]) < 0.75).collect()
}

struct Case<'a> {
    /// (patch, absolute angular velocity, axial velocity) of fixed-velocity walls.
    walls: &'a [(&'a str, f64, f64)],
    /// (inlet, outlet, axial speed): fixed inlet velocity, zero-gradient outlet at p = 0; also the initial field.
    through: Option<(&'a str, &'a str, f64)>,
    /// MRF zone: (Omega, zone, cyclic_seams).
    mrf: Option<(f64, ZoneFn, bool)>,
    /// Iteration cap; `exact` = run all of them (no residual stop).
    iters: usize,
    exact: bool,
    /// F1 on whatever the source (amendment 3: the inertial sector arms of K3c and K3i).
    f1: bool,
}

struct Run {
    solver: SimpleSolver,
    converged: bool,
    last: rcfd_fv::simple::IterationInfo,
}

/// Steady SIMPLEC (C2b template of ADR 0071), the MRF zone wired as `sim.rs` wires it: frame flux on the convecting flux,
/// component-diagonal correction, lagged -Omega x u every iteration, no wall-pressure trapezoid, no pair mean; with seams also F1
/// (`coupled_offdiag_in_h1`, ADR 0113 amendment 2).
fn solve(mesh: Mesh, c: &Case) -> Run {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let u0 = c.through.map_or([0.0; 3], |t| [0.0, 0.0, t.2]);
    let mut u = VectorField::uniform("U", &geom, u0);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    let cf = geom.mesh.face_centres();
    for &(name, w, uz) in c.walls {
        let v: Vec<_> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| [-w * cf[f][1], w * cf[f][0], uz])
            .collect();
        u.set_bc(&geom, name, Box::new(FixedValue(Profile::PerFace(v))))
            .unwrap();
    }
    if let Some((inlet, outlet, uz)) = c.through {
        u.set_bc(&geom, inlet, Box::new(FixedValue::vector([0.0, 0.0, uz])))
            .unwrap();
        u.set_bc(&geom, outlet, Box::new(ZeroGradient)).unwrap();
        p.set_bc(&geom, outlet, Box::new(FixedValue::scalar(0.0))).unwrap();
    }
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let mrf = c.mrf.map(|(omega, zf, seams)| (mrf_zone(omega, seams), zf(&geom)));
    solver.coupled_offdiag_in_h1 = c.f1;
    if let Some((m, zone)) = &mrf {
        let mut off = vec![0.0; geom.mesh.n_faces()];
        m.add_frame_flux(&geom, zone, &mut off);
        solver.conv_flux_offset = Some(off);
        solver.component_diag_correction = true;
        // amendment 2, F1: as the API sets it for an MRF zone with cyclic seams
        solver.coupled_offdiag_in_h1 = c.f1 || m.cyclic_seams;
    }
    let mut last = solver.iterate();
    let mut converged = false;
    for _ in 0..c.iters {
        if let Some((m, zone)) = &mrf {
            let ub = solver.u.boundary_values(&geom);
            let mut f = BodyForce::uniform(&geom, [0.0; 3]);
            m.add_force(&geom, &solver.u.values, &ub, zone, &mut f);
            solver.body_force = Some(f);
        }
        last = solver.iterate();
        if !c.exact && last.res_u < 1e-10 && last.continuity < 1e-9 {
            converged = true;
            break;
        }
    }
    Run {
        solver,
        converged,
        last,
    }
}

/// For each cell of `a`, the cell of `b` at the same centre (within 1e-9).
fn matching(a: &Run, b: &Run) -> Vec<usize> {
    let (ca, cb) = (a.solver.geom.mesh.cell_centres(), b.solver.geom.mesh.cell_centres());
    ca.iter()
        .map(|x| {
            let j = (0..cb.len())
                .min_by(|&i, &j| {
                    let d = |k: usize| rcfd_core::vec3::norm(rcfd_core::vec3::sub(cb[k], *x));
                    d(i).total_cmp(&d(j))
                })
                .unwrap();
            assert!(
                rcfd_core::vec3::norm(rcfd_core::vec3::sub(cb[j], *x)) <= 1e-9,
                "no matching cell"
            );
            j
        })
        .collect()
}

/// (max |du|, max |d(p - p_0)|) of `a` against the matching cells of `b`.
fn difference(a: &Run, b: &Run) -> (f64, f64) {
    let m = matching(a, b);
    let (ua, ub) = (&a.solver.u.values, &b.solver.u.values);
    let (pa, pb) = (&a.solver.p.values, &b.solver.p.values);
    let (mut du, mut dp): (f64, f64) = (0.0, 0.0);
    for (i, &j) in m.iter().enumerate() {
        for k in 0..3 {
            du = du.max((ua[i][k] - ub[j][k]).abs());
        }
        dp = dp.max(((pa[i] - pa[0]) - (pb[j] - pb[m[0]])).abs());
    }
    (du, dp)
}

/// |sum phi(a) + sum phi(b)| / A(a) over a cyclic pair.
fn seam_balance(r: &Run, a: &str, b: &str) -> f64 {
    let (m, phi) = (&r.solver.geom.mesh, &r.solver.phi);
    let s = |n: &str| m.patch(n).unwrap().faces().map(|f| phi[f]).sum::<f64>();
    let area: f64 = m.patch(a).unwrap().faces().map(|f| m.face_area(f)).sum();
    (s(a) + s(b)).abs() / area
}

fn theta_error(r: &Run) -> f64 {
    let cc = r.solver.geom.mesh.cell_centres();
    r.solver
        .u
        .values
        .iter()
        .enumerate()
        .map(|(c, v)| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let rr = x.hypot(y);
            ((-y * v[0] + x * v[1]) / rr - tc(rr)).abs()
        })
        .fold(0.0, f64::max)
}

fn check(fail: &mut Vec<String>, name: &str, value: f64, limit: f64) {
    let ok = value <= limit;
    println!(
        "  {name}: {value:.3e} (limit {limit:.3e}) {}",
        if ok { "pass" } else { "MISS" }
    );
    if !ok {
        fail.push(format!("{name} {value:.3e} > {limit:.3e}"));
    }
}

fn require(fail: Vec<String>) {
    assert!(fail.is_empty(), "{fail:?}");
}

// ---------------------------------------------------------------------------------------------------------------- G

/// Per-cell closure of the frame-flux offset, worst over the zone cells, relative to sum |Omega| r_perp |S|.
fn closure(geom: &FvGeometry, m: &MrfZone, zone: &[bool]) -> f64 {
    let mesh = &geom.mesh;
    let mut off = vec![0.0; mesh.n_faces()];
    m.add_frame_flux(geom, zone, &mut off);
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let mut worst: f64 = 0.0;
    for c in (0..mesh.n_cells()).filter(|&c| zone[c]) {
        let (mut s, mut d) = (0.0f64, 0.0f64);
        for &f in mesh.cell_faces(c) {
            if geom.empty[f] {
                continue;
            }
            s += if mesh.owner()[f] == c { off[f] } else { -off[f] };
            d += m.omega.abs() * fc[f][0].hypot(fc[f][1]) * rcfd_core::vec3::norm(sf[f]);
        }
        worst = worst.max(s.abs() / d);
    }
    worst
}

/// Largest pair mismatch |F_f + F_g| / (|Omega| r |S|) over the seam faces of `patch`.
fn pair_mismatch(geom: &FvGeometry, m: &MrfZone, patch: &str) -> f64 {
    let mesh = &geom.mesh;
    let pi = mesh.patches().iter().position(|p| p.name == patch).unwrap();
    let map = geom.periodic_partner[pi].as_ref().unwrap();
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let flux = |f: usize| rcfd_core::vec3::dot(m.frame_velocity(fc[f]), sf[f]);
    mesh.patch(patch)
        .unwrap()
        .faces()
        .zip(map)
        .map(|(f, &g)| (flux(f) + flux(g)).abs() / (m.omega.abs() * fc[f][0].hypot(fc[f][1]) * mesh.face_area(f)))
        .fold(0.0, f64::max)
}

#[test]
fn g_construction_seam_pairs_and_closure() {
    let mut fail = Vec::new();
    let sets: [(&str, Mesh, &str, ZoneFn); 4] = [
        ("2-D sector", sector_2d(), "theta_min", all),
        ("3-D sector", sector_3d(), "ymin", all),
        ("3-D sector, band", sector_3d(), "ymin", band),
        ("K5 joined", joined().0, "theta_min", inner_block),
    ];
    for (name, mesh, seam, zf) in sets {
        let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
        let zone = zf(&geom);
        let on = mrf_zone(2.0, true);
        println!("{name}:");
        check(&mut fail, "pair mismatch", pair_mismatch(&geom, &on, seam), 1e-12);
        check(&mut fail, "closure with seams", closure(&geom, &on, &zone), 1e-12);
        check(
            &mut fail,
            "revolution check with seams",
            on.worst_non_axisymmetry(&geom, &zone).0,
            1e-6,
        );
        let off = mrf_zone(2.0, false).worst_non_axisymmetry(&geom, &zone).0;
        println!("  revolution check without seams: {off:.3e} (must exceed 1e-6)");
        if off <= 1e-6 {
            fail.push(format!("{name}: refusal without seams lost"));
        }
    }
    require(fail);
}

fn on_seam(geom: &FvGeometry, f: Option<usize>, seams: [&str; 2]) -> bool {
    f.and_then(|f| geom.mesh.patch_of_face(f))
        .is_some_and(|p| seams.contains(&p.name.as_str()))
}

fn tilted(t: f64) -> MrfZone {
    MrfZone {
        axis: [0.0, -t.sin(), t.cos()],
        ..mrf_zone(2.0, true)
    }
}

/// G guards as registered (3-D sector). Defective set-up (amendment 2): the tilt also makes the end planes non-axisymmetric, and a
/// one-sided zone is bounded by a plane through the axis that reads 1.0 like the unpaired seam.
#[test]
#[ignore = "known miss (defective set-up, ADR 0113 amendment 2): the worst face is an end plane (tilt) or the zone's own theta plane"]
fn g_guards_as_registered_known_miss() {
    let geom = FvGeometry::new(Arc::new(sector_3d())).unwrap();
    let (w, at) = tilted(1e-3).worst_non_axisymmetry(&geom, &all(&geom));
    assert!(w > 1e-6 && on_seam(&geom, at, ["ymin", "ymax"]), "tilted axis: {w:.3e}");
    let half: Vec<bool> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| c[1].atan2(c[0]) < FRAC_PI_3 / 2.0)
        .collect();
    let (w, at) = mrf_zone(2.0, true).worst_non_axisymmetry(&geom, &half);
    assert!(
        w > 1e-6 && on_seam(&geom, at, ["ymin", "ymax"]),
        "one-sided zone: {w:.3e}"
    );
}

/// Guards as corrected in amendment 2. Known miss (amendment 3): the tilted axis also breaks the walls, so the worst face is a wall.
#[test]
#[ignore = "known miss (defective set-up, ADR 0113 amendment 3): worst face of the tilted 2-D sector is a wall (1.0e-4), refusal holds"]
fn g_guards_amendment2_known_miss() {
    let geom = FvGeometry::new(Arc::new(sector_2d())).unwrap();
    let (w, at) = tilted(1e-3).worst_non_axisymmetry(&geom, &all(&geom));
    assert!(
        w > 1e-6 && on_seam(&geom, at, ["theta_min", "theta_max"]),
        "tilted axis"
    );
}

/// Guards, second correction (amendment 3): a misaligned axis and a one-sided zone are refused; the seam pair term alone detects the
/// misalignment.
#[test]
fn g_guards_refuse_misaligned_axis_and_one_sided_zone() {
    let mut fail = Vec::new();
    for (name, mesh, seam) in [("2-D", sector_2d(), "theta_min"), ("3-D", sector_3d(), "ymin")] {
        let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
        let t = tilted(1e-3);
        let w = t.worst_non_axisymmetry(&geom, &all(&geom)).0;
        let pm = pair_mismatch(&geom, &t, seam);
        println!("tilted axis ({name}): revolution check {w:.3e}, seam pair term {pm:.3e} (each must exceed 1e-6)");
        if !(w > 1e-6 && pm > 1e-6) {
            fail.push(format!("tilted axis {name} not refused"));
        }
    }
    let g3 = FvGeometry::new(Arc::new(sector_3d())).unwrap();
    let half: Vec<bool> = g3
        .mesh
        .cell_centres()
        .iter()
        .map(|c| c[1].atan2(c[0]) < FRAC_PI_3 / 2.0)
        .collect();
    let w = mrf_zone(2.0, true).worst_non_axisymmetry(&g3, &half).0;
    println!("one-sided zone: {w:.3e} (must exceed 1e-6)");
    if w <= 1e-6 {
        fail.push("one-sided zone not refused".into());
    }
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- K1, D1

fn k1(seams: bool) -> (Run, Run) {
    let w = 2.0;
    let sector = solve(
        sector_2d(),
        &Case {
            walls: &[("inner", w, 0.0), ("outer", w, 0.0)],
            through: None,
            mrf: Some((w, all, seams)),
            iters: 4000,
            exact: false,
            f1: false,
        },
    );
    let full = solve(
        full_2d(),
        &Case {
            walls: &[("cylinder", w, 0.0), ("farfield", w, 0.0)],
            through: None,
            mrf: Some((w, all, false)),
            iters: 4000,
            exact: false,
            f1: false,
        },
    );
    (sector, full)
}

#[test]
fn k1_solid_body_rotation_sector_returns_the_full_annulus() {
    let (w, wr) = (2.0, 2.0 * RO);
    let range = w * w * (RO * RO - RI * RI) / 2.0;
    let (sector, full) = k1(true);
    println!(
        "K1: sector converged {} ({} it), full converged {} ({} it)",
        sector.converged, sector.last.iteration, full.converged, full.last.iteration
    );
    let mut fail = Vec::new();
    if !(sector.converged && full.converged) {
        fail.push("not converged".into());
    }
    let cc = sector.solver.geom.mesh.cell_centres();
    let r2 = |c: usize| cc[c][0] * cc[c][0] + cc[c][1] * cc[c][1];
    let (mut eu, mut ep): (f64, f64) = (0.0, 0.0);
    for (c, v) in sector.solver.u.values.iter().enumerate() {
        eu = eu.max((v[0] + w * cc[c][1]).hypot(v[1] - w * cc[c][0]).hypot(v[2]));
        let dp = sector.solver.p.values[c] - sector.solver.p.values[0];
        ep = ep.max((dp - w * w * (r2(c) - r2(0)) / 2.0).abs());
    }
    check(&mut fail, "velocity / (W r_o)", eu / wr, 2e-2);
    check(&mut fail, "pressure / range", ep / range, 5e-2);
    let (du, dp) = difference(&sector, &full);
    check(&mut fail, "sector - annulus velocity / (W r_o)", du / wr, L_DIFF);
    check(&mut fail, "sector - annulus pressure / range", dp / range, L_DIFF);
    check(
        &mut fail,
        "seam balance / (W r_o)",
        seam_balance(&sector, "theta_min", "theta_max") / wr,
        1e-8,
    );
    require(fail);
}

/// Amendment 2, falsifier of F1 (run first): with F1 the K1 sector equals the annulus to the level of the no-force arm.
#[test]
fn f1_falsifier_k1_sector_equals_annulus() {
    let (sector, full) = k1(true);
    let (du, dp) = difference(&sector, &full);
    let mut fail = Vec::new();
    println!(
        "F1 falsifier (K1 with F1): converged {} {}",
        sector.converged, full.converged
    );
    check(&mut fail, "sector - annulus velocity / (W r_o)", du / (2.0 * RO), 1e-8);
    check(
        &mut fail,
        "sector - annulus pressure / range",
        dp / (2.0 * 2.0 * (RO * RO - RI * RI) / 2.0),
        1e-8,
    );
    require(fail);
}

#[test]
fn d1_solid_body_without_seam_frame_flux_is_wrong() {
    let wr = 2.0 * RO;
    let (sector, full) = k1(false);
    let (du, _) = difference(&sector, &full);
    println!(
        "D1: sector converged {} ({} it); sector - annulus velocity {:.3e} W r_o (must exceed 10 x {L_DIFF:.1e})",
        sector.converged,
        sector.last.iteration,
        du / wr
    );
    assert!(du / wr >= 10.0 * L_DIFF || !du.is_finite());
}

// ---------------------------------------------------------------------------------------------------------------- K2

#[test]
fn k2_taylor_couette_sector_returns_the_full_annulus() {
    let run = |mesh: Mesh, walls: &[(&str, f64, f64)], mrf: Option<(f64, ZoneFn, bool)>| {
        solve(
            mesh,
            &Case {
                walls,
                through: None,
                mrf,
                iters: 4000,
                exact: false,
                f1: false,
            },
        )
    };
    let sw = [("inner", W_I, 0.0), ("outer", 0.0, 0.0)];
    let fw = [("cylinder", W_I, 0.0), ("farfield", 0.0, 0.0)];
    let sector = run(sector_2d(), &sw, Some((W_I, all, true)));
    let full = run(full_2d(), &fw, Some((W_I, all, false)));
    let (si, fi) = (run(sector_2d(), &sw, None), run(full_2d(), &fw, None));
    println!(
        "K2: sector converged {} ({} it), full converged {} ({} it); inertial pair difference (reported) {:.3e} V_i",
        sector.converged,
        sector.last.iteration,
        full.converged,
        full.last.iteration,
        difference(&si, &fi).0 / V_I
    );
    let mut fail = Vec::new();
    if !(sector.converged && full.converged) {
        fail.push("not converged".into());
    }
    check(
        &mut fail,
        "sector analytic error / V_i",
        theta_error(&sector) / V_I,
        1.5e-2,
    );
    println!("  (full annulus analytic error {:.3e} V_i)", theta_error(&full) / V_I);
    check(
        &mut fail,
        "sector - annulus / V_i",
        difference(&sector, &full).0 / V_I,
        L_DIFF,
    );
    check(
        &mut fail,
        "seam balance / V_i",
        seam_balance(&sector, "theta_min", "theta_max") / V_I,
        1e-8,
    );
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- K3

type Walls = &'static [(&'static str, f64, f64)];

fn k3_case(walls: Walls, mrf: Option<(f64, ZoneFn, bool)>) -> Case<'static> {
    Case {
        walls,
        through: Some(("zmin", "zmax", 1.0)),
        mrf,
        iters: 4000,
        exact: false,
        f1: false,
    }
}

const SW: Walls = &[("xmin", 0.0, 0.0), ("xmax", 0.0, 0.0)];
const FW: Walls = &[("inner", 0.0, 0.0), ("outer", 0.0, 0.0)];

/// K3 limits other than the sector-annulus one; returns the failed criteria.
fn k3_common(fail: &mut Vec<String>, sector: &Run, refr: &Run) {
    let u = 1.0;
    let (m, phi) = (&sector.solver.geom.mesh, &sector.solver.phi);
    let flow = |n: &str| m.patch(n).unwrap().faces().map(|f| phi[f]).sum::<f64>();
    let (inflow, outflow) = (-flow("zmin"), flow("zmax"));
    check(
        fail,
        "|inflow - outflow| / inflow",
        (inflow - outflow).abs() / inflow,
        1e-9,
    );
    let cc = m.cell_centres();
    let (mut ut, mut duz): (f64, f64) = (0.0, 0.0);
    for (c, v) in sector.solver.u.values.iter().enumerate() {
        let r = cc[c][0].hypot(cc[c][1]);
        ut = ut.max(((-cc[c][1] * v[0] + cc[c][0] * v[1]) / r).abs());
        duz = duz.max((v[2] - refr.solver.u.values[c][2]).abs());
    }
    check(fail, "max |u_theta| / U", ut / u, 2e-2);
    check(fail, "max |u_z - u_z,ref| / U", duz / u, 2e-2);
    check(fail, "seam balance / U", seam_balance(sector, "ymin", "ymax") / u, 1e-8);
}

/// K3 as registered (amendment 3: met with F1; amendment 2 had called it defective by a wrong attribution).
#[test]
fn k3_axial_through_flow_with_a_rotor_band() {
    let sector = solve(sector_3d(), &k3_case(SW, Some((5.0, band, true))));
    let refr = solve(sector_3d(), &k3_case(SW, None));
    let full = solve(full_3d(), &k3_case(FW, Some((5.0, band, false))));
    let mut fail = Vec::new();
    if !(sector.converged && refr.converged && full.converged) {
        fail.push("not converged".into());
    }
    k3_common(&mut fail, &sector, &refr);
    check(&mut fail, "sector - annulus / U", difference(&sector, &full).0, L_DIFF);
    require(fail);
}

/// K3b (amendment 2): the effect of the MRF band is the same in the sector as in the annulus. Defective set-up (amendment 3): its
/// inertial sector reference runs without F1.
#[test]
#[ignore = "known miss (defective set-up, ADR 0113 amendment 3): 4.64e-3 U against 4.3e-4 U, the reference sector lacks F1"]
fn k3b_rotor_band_effect_sector_equals_annulus_known_miss() {
    k3_effect(false);
}

/// K3c (amendment 3): K3b with F1 on the inertial sector reference too.
#[test]
fn k3c_rotor_band_effect_sector_equals_annulus() {
    k3_effect(true);
}

/// K3i (amendment 3, reported): the inertial sector with F1 against the inertial annulus.
#[test]
fn k3i_inertial_sector_with_f1_reported() {
    let mut c = k3_case(SW, None);
    c.f1 = true;
    let s = solve(sector_3d(), &c);
    let f = solve(full_3d(), &k3_case(FW, None));
    println!(
        "K3i: inertial sector with F1 against the inertial annulus {:.3e} U (converged {} {})",
        difference(&s, &f).0,
        s.converged,
        f.converged
    );
}

fn k3_effect(f1_reference: bool) {
    let sector = solve(sector_3d(), &k3_case(SW, Some((5.0, band, true))));
    let mut rc = k3_case(SW, None);
    rc.f1 = f1_reference;
    let refr = solve(sector_3d(), &rc);
    let full = solve(full_3d(), &k3_case(FW, Some((5.0, band, false))));
    let full_ref = solve(full_3d(), &k3_case(FW, None));
    println!(
        "K3b/c: converged sector {} ({} it), sector reference {}, annulus {}, annulus reference {}",
        sector.converged, sector.last.iteration, refr.converged, full.converged, full_ref.converged
    );
    let mut fail = Vec::new();
    if !(sector.converged && refr.converged && full.converged && full_ref.converged) {
        fail.push("not converged".into());
    }
    k3_common(&mut fail, &sector, &refr);
    let m = matching(&sector, &full);
    let mut worst: f64 = 0.0;
    for (i, &j) in m.iter().enumerate() {
        for k in 0..3 {
            let es = sector.solver.u.values[i][k] - refr.solver.u.values[i][k];
            let ea = full.solver.u.values[j][k] - full_ref.solver.u.values[j][k];
            worst = worst.max((es - ea).abs());
        }
    }
    check(&mut fail, "band effect, sector - annulus / U", worst, L_DIFF);
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- K4, D4

fn k4(seams: bool) -> (f64, f64) {
    let u = 1.0;
    let r = solve(
        sector_3d(),
        &Case {
            walls: &[("xmin", 0.0, u), ("xmax", 0.0, u)],
            through: Some(("zmin", "zmax", u)),
            mrf: Some((5.0, all, seams)),
            iters: 300,
            exact: true,
            f1: false,
        },
    );
    let du = r
        .solver
        .u
        .values
        .iter()
        .map(|v| v[0].abs().max(v[1].abs()).max((v[2] - u).abs()))
        .fold(0.0, f64::max);
    let p = r.solver.p.values.iter().map(|p| p.abs()).fold(0.0, f64::max);
    (du / u, p / (u * u))
}

#[test]
fn k4_empty_passage_keeps_the_exact_swirl_free_state() {
    let (du, p) = k4(true);
    let mut fail = Vec::new();
    println!("K4 (300 iterations):");
    check(&mut fail, "max |u - (0, 0, U)| / U", du, 1e-8);
    check(&mut fail, "max |p| / U^2", p, 1e-8);
    require(fail);
}

#[test]
fn d4_empty_passage_without_seam_frame_flux_is_wrong() {
    let (du, _) = k4(false);
    println!("D4: max |u - (0, 0, U)| {du:.3e} U (must exceed 10 x 1e-8)");
    assert!(du >= 1e-7 || !du.is_finite());
}

// ---------------------------------------------------------------------------------------------------------------- K5

#[test]
fn k5_joined_sector_with_the_rotor_block_as_mrf_zone() {
    let walls = [("inner", W_I, 0.0), ("outer", 0.0, 0.0)];
    let case = Case {
        walls: &walls,
        through: None,
        mrf: Some((W_I, inner_block, true)),
        iters: 4000,
        exact: false,
        f1: false,
    };
    let ctrl: Vec<f64> = [8, 6]
        .iter()
        .map(|&n| {
            let r = solve(
                annulus_sector(n, 16, RI, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap(),
                &case,
            );
            println!(
                "K5 control {n} x 16: converged {}, error {:.3e} V_i",
                r.converged,
                theta_error(&r) / V_I
            );
            assert!(r.converged, "control {n} not converged");
            theta_error(&r)
        })
        .collect();
    let (mesh, faces) = joined();
    let run = solve(mesh, &case);
    println!("K5: converged {} ({} it)", run.converged, run.last.iteration);
    let mut fail = Vec::new();
    if !run.converged {
        fail.push("not converged".into());
    }
    let e = theta_error(&run);
    check(&mut fail, "error / V_i", e / V_I, 2e-2);
    check(
        &mut fail,
        "error / (1.5 x max control)",
        e / (1.5 * ctrl[0].max(ctrl[1])),
        1.0,
    );
    let (m, phi) = (&run.solver.geom.mesh, &run.solver.phi);
    let area: f64 = faces.iter().map(|&f| m.face_area(f)).sum();
    let net: f64 = faces.iter().map(|&f| phi[f]).sum();
    let abs: f64 = faces.iter().map(|&f| phi[f].abs()).sum();
    check(
        &mut fail,
        "|net interface flux| / (V_i A)",
        net.abs() / (V_I * area),
        1e-8,
    );
    println!("  sum |interface flux| (reported) {:.3e} V_i A", abs / (V_I * area));
    check(
        &mut fail,
        "seam balance / V_i",
        seam_balance(&run, "theta_min", "theta_max") / V_I,
        1e-8,
    );
    require(fail);
}

/// Smoke of the seams-on arms (rule d, ADR 0113 amendment): construction plus 30 iterations; only finiteness and continuity are
/// looked at.
#[test]
#[ignore = "smoke of ADR 0113; run on demand"]
fn smoke_w44c_seam_arms() {
    let w2 = [("inner", 2.0, 0.0), ("outer", 2.0, 0.0)];
    let tcw = [("inner", W_I, 0.0), ("outer", 0.0, 0.0)];
    let rest = [("xmin", 0.0, 0.0), ("xmax", 0.0, 0.0)];
    let mov = [("xmin", 0.0, 1.0), ("xmax", 0.0, 1.0)];
    let arms: [(&str, Mesh, Case); 5] = [
        (
            "K1",
            sector_2d(),
            Case {
                walls: &w2,
                through: None,
                mrf: Some((2.0, all, true)),
                iters: 30,
                exact: true,
                f1: false,
            },
        ),
        (
            "K2",
            sector_2d(),
            Case {
                walls: &tcw,
                through: None,
                mrf: Some((W_I, all, true)),
                iters: 30,
                exact: true,
                f1: false,
            },
        ),
        (
            "K3",
            sector_3d(),
            Case {
                walls: &rest,
                through: Some(("zmin", "zmax", 1.0)),
                mrf: Some((5.0, band, true)),
                iters: 30,
                exact: true,
                f1: false,
            },
        ),
        (
            "K4",
            sector_3d(),
            Case {
                walls: &mov,
                through: Some(("zmin", "zmax", 1.0)),
                mrf: Some((5.0, all, true)),
                iters: 30,
                exact: true,
                f1: false,
            },
        ),
        (
            "K5",
            joined().0,
            Case {
                walls: &tcw,
                through: None,
                mrf: Some((W_I, inner_block, true)),
                iters: 30,
                exact: true,
                f1: false,
            },
        ),
    ];
    for (name, mesh, case) in arms {
        let r = solve(mesh, &case);
        let finite = r.solver.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        println!("smoke {name}: finite {finite}, continuity {:.2e}", r.last.continuity);
    }
}
