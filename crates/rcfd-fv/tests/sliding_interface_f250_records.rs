//! F-250 (ADR 0216) records R1 to R3: the bits of three library runs that F-250 must not change, recorded on main 08316f7
//! before any code of F-250 (Windows; printed elsewhere, ADR 0137). R1 is ADR 0033's rigidly rotating annulus G6 (the
//! incompressible PISO step on a moving mesh of fixed topology, 20 steps); R2 the cylindrical join of ADR 0109 on the
//! rotor/stator pair of ADR 0216 at two rotor angles (content hashes and join reports, with the interface
//! non-orthogonality printed); R3 a PISO Taylor-Couette start on the static joined pair (20 steps).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d, cylinder_ogrid, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::non_conformal::{interface_non_orthogonality, join_non_conformal_cylindrical};
use rcfd_mesh::Mesh;
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn push(bytes: &mut Vec<u8>, x: f64) {
    bytes.extend_from_slice(&x.to_bits().to_le_bytes());
}

fn check(name: &str, bytes: &[u8], want: &str) {
    let got = format!("{:016x}", rcfd_core::hash::fnv64(bytes));
    println!("{name}: {got}");
    if cfg!(windows) {
        assert_eq!(got, want, "{name}");
    }
}

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn push_solver(bytes: &mut Vec<u8>, s: &PimpleSolver) {
    for v in &s.u.values {
        for x in v {
            push(bytes, *x);
        }
    }
    for x in s.p.values.iter().chain(&s.phi) {
        push(bytes, *x);
    }
    push(bytes, s.time);
    bytes.extend_from_slice(&(s.history.len() as u64).to_le_bytes());
}

/// The rotor/stator pair of ADR 0216 (2-D, one cell deep): rotor `cylinder_ogrid(144, 8, 1, 1.5)` (inner wall `cylinder`,
/// interface `farfield`), stator `cylinder_ogrid(120, 8, 1.5, 2)` (interface `cylinder`, outer boundary `farfield`).
fn pair() -> (Mesh, Mesh) {
    (
        cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, 0.1).unwrap(),
        cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, 0.1).unwrap(),
    )
}

fn rotated(m: &Mesh, angle: f64) -> Mesh {
    let pose = Pose {
        origin: vec3::ZERO,
        axis: [0.0, 0.0, 1.0],
        angle,
        translation: vec3::ZERO,
    };
    moved_mesh(m, m.points().iter().map(|&q| pose.apply(q)).collect()).unwrap()
}

/// R1: ADR 0033 G6, `annulus_3d(48, 8, 2, 0.5, 1, 0.2)` rotating rigidly with the fluid (`W = 1`), walls following the mesh,
/// PISO with central convection and tight solvers, `nu = 0.01`, `dt = 0.02`, 20 steps. Hash: the bits of every cell velocity,
/// pressure and face flux, the time and the step count.
#[test]
fn r1_rigidly_rotating_annulus_record() {
    let w = 1.0;
    let mesh = annulus_3d(48, 8, 2, 0.5, 1.0, 0.2, 0.0, Grading::Uniform).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [-w * c[1], w * c[0], 0.0]);
    let p = ScalarField::from_fn("p", &geom, |c| 0.5 * w * w * (c[0] * c[0] + c[1] * c[1]));
    let mut s = PimpleSolver::new(geom, u, p, settings(0.01));
    let dt = 0.02;
    let names: Vec<String> = ["inner", "outer", "zmin", "zmax"].iter().map(|s| s.to_string()).collect();
    for n in 1..=20 {
        let pose = Pose {
            origin: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            angle: w * n as f64 * dt,
            translation: vec3::ZERO,
        };
        let pts: Vec<Vec3> = mesh.points().iter().map(|&q| pose.apply(q)).collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
    }
    let mut bytes = Vec::new();
    push_solver(&mut bytes, &s);
    check("R1", &bytes, "e4bc6f7b23554262");
}

/// R2: `join_non_conformal_cylindrical` of the ADR 0216 pair with the rotor at 0 and at 0.0123 rad about z (rotated with
/// `motion::moved_mesh`), tolerance 1e-8. Hash: both content hashes, the overlap and dropped counts, the split-edge count,
/// the bits of the tiling error, the interface face indices and the B cell offset. Printed: the worst interface
/// non-orthogonality (ADR 0112) at both angles.
#[test]
fn r2_cylindrical_join_of_the_pair_record() {
    let (a, b) = pair();
    let mut bytes = Vec::new();
    for angle in [0.0, 0.0123] {
        let ra = rotated(&a, angle);
        let (m, rep) =
            join_non_conformal_cylindrical(&ra, "farfield", &b, "cylinder", vec3::ZERO, [0.0, 0.0, 1.0], 1e-8).unwrap();
        bytes.extend_from_slice(&m.content_hash().to_le_bytes());
        for k in [rep.n_overlap_faces, rep.n_dropped, rep.n_split_edges, rep.b_cell_offset] {
            bytes.extend_from_slice(&(k as u64).to_le_bytes());
        }
        push(&mut bytes, rep.max_tiling_error);
        for f in &rep.interface_faces {
            bytes.extend_from_slice(&(*f as u64).to_le_bytes());
        }
        let no = interface_non_orthogonality(&m, &rep.interface_faces, 70.0).unwrap();
        println!(
            "R2 angle {angle}: {} cells, {} overlaps, dropped {}, tiling {:.2e}, worst interface non-orthogonality {:.2} deg",
            m.n_cells(),
            rep.n_overlap_faces,
            rep.n_dropped,
            rep.max_tiling_error,
            no.worst_deg
        );
    }
    check("R2", &bytes, "77a3c29e4ad6f213");
}

/// R3: PISO Taylor-Couette from rest on the static joined pair (rotor at 0): inner wall `cylinder` at `W = 1` (velocity
/// `W z x x_f` per face), outer `farfield` at rest, `nu = 1`, central convection, two non-orthogonal correctors, tight
/// solvers, `dt = 0.005`, 20 steps. Hash as R1.
#[test]
fn r3_static_joined_pair_taylor_couette_record() {
    let (a, b) = pair();
    let (m, _) = join_non_conformal_cylindrical(&a, "farfield", &b, "cylinder", vec3::ZERO, [0.0, 0.0, 1.0], 1e-8).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    let cf = geom.mesh.face_centres();
    let inner: Vec<Vec3> = geom.mesh.patch("cylinder").unwrap().faces().map(|f| [-cf[f][1], cf[f][0], 0.0]).collect();
    u.set_bc(&geom, "cylinder", Box::new(FixedValue(Profile::PerFace(inner)))).unwrap();
    u.set_bc(&geom, "farfield", Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = settings(1.0);
    st.non_orth_correctors = 2;
    let mut s = PimpleSolver::new(geom, u, p, st);
    for _ in 0..20 {
        s.step(0.005);
    }
    let mut bytes = Vec::new();
    push_solver(&mut bytes, &s);
    check("R3", &bytes, "6378ceeeaacb6799");
}
