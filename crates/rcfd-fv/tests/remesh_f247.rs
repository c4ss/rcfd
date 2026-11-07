//! F-247 checks of ADR 0215: ADR 0090's piston on an unstructured tetrahedral mesh with local remeshing instead of
//! layering (P0, A1, A2, B1v, B1m, B1p, B1e, B4, Q, B2s, B3, L1, R1, R2 and the negative control N1), and the same piston
//! without remeshing (the negative control N2). The limits are the registered ones; the piston moves with the band of
//! amendment 2 (`w = 2 h0`).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::FvGeometry;
use rcfd_mesh::remesh::{box_planes, remap, OpCounts, RemeshSettings, TetRemesher};
use rcfd_mesh::surface::box_surface;
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const T: f64 = 16.0;
const H0: f64 = 0.035;
const BAND: f64 = 2.0 * H0;
const Q_MIN: f64 = 0.2;
const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

/// M0 of ADR 0215.
fn m0() -> TetMesh {
    let s = box_surface([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]);
    let opt = TetOptions {
        size: Some(H0),
        quality_sweeps: 3,
        ..TetOptions::default()
    };
    tetrahedralize(&s, &opt).unwrap()
}

/// FNV-1a over the point bits and the cell vertex lists (the record of ADR 0215).
fn hash(m: &TetMesh) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |x: u64| {
        h ^= x;
        h = h.wrapping_mul(0x100000001b3);
    };
    for p in &m.points {
        for c in p {
            eat(c.to_bits());
        }
    }
    for c in &m.cells {
        for &v in c {
            eat(v as u64);
        }
    }
    h
}

fn remesher(m: &TetMesh) -> TetRemesher {
    TetRemesher::from_tet_mesh(m, box_planes([0.0; 3], [1.0, 0.1, 0.1]), RemeshSettings::for_size(H0)).unwrap()
}

/// The solver geometry of the remesher's mesh; P0 asks for a valid mesh with the six patches in order.
fn geom(r: &TetRemesher) -> FvGeometry {
    let m = r.mesh().expect("P0: a valid mesh");
    let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, PATCHES, "P0: the six patches in order");
    FvGeometry::new(Arc::new(m)).unwrap()
}

fn rest() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    }
}

fn piston_x(t: f64) -> f64 {
    1.0 - 0.3 * (1.0 - (PI * t / T).cos())
}

/// Volume-averaged pressure and total volume.
fn mean_pressure(s: &CompressibleSolver) -> (f64, f64) {
    let v = s.geom.mesh.cell_volumes();
    let vol: f64 = v.iter().sum();
    let pv: f64 = s.primitives().iter().zip(v).map(|(p, v)| p.p * v).sum();
    (pv / vol, vol)
}

/// `[sum V, sum rho V, P_x, P_y, P_z, sum rho E V]` and the momentum scale `sum_c |m_c| V_c`.
fn totals(s: &CompressibleSolver) -> ([f64; 6], f64) {
    let t = s.totals();
    let v = s.geom.mesh.cell_volumes();
    let scale: f64 = s.u.iter().zip(v).map(|(c, v)| vec3::norm([c[1], c[2], c[3]]) * v).sum();
    ([v.iter().sum(), t[0], t[1], t[2], t[3], t[4]], scale)
}

/// Smallest dihedral angle of the remesher's cells, degrees.
fn min_dihedral(r: &TetRemesher) -> f64 {
    const FACE: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];
    let mut m: f64 = 180.0;
    for c in r.cells() {
        let p: [Vec3; 4] = c.map(|v| r.points()[v]);
        let n: Vec<Vec3> = FACE
            .iter()
            .map(|f| vec3::unit(vec3::cross(vec3::sub(p[f[1]], p[f[0]]), vec3::sub(p[f[2]], p[f[0]]))))
            .collect();
        for i in 0..4 {
            for j in i + 1..4 {
                m = m.min((PI - vec3::dot(n[i], n[j]).clamp(-1.0, 1.0).acos()).to_degrees());
            }
        }
    }
    m
}

fn add_ops(a: &mut OpCounts, b: &OpCounts) {
    a.splits += b.splits;
    a.collapses += b.collapses;
    a.removals += b.removals;
    a.flips += b.flips;
    a.relocations += b.relocations;
}

/// P-T of ADR 0215: ADR 0090's piston on M0 with local remeshing.
#[test]
fn p_tetrahedral_piston_with_local_remeshing() {
    let clock = std::time::Instant::now();
    let tm = m0();
    let h = hash(&tm);
    println!(
        "P0: M0 hash {h:016x}, {} cells, {} points",
        tm.cells.len(),
        tm.points.len()
    );
    assert_eq!(h, 0xaaf8f4c36f6446e4, "P0: the M0 record");
    assert_eq!(tm.cells.len(), 1711, "P0: M0 cells");
    let mut r = remesher(&tm);
    let st = *r.settings();
    let (lo, hi) = r.edge_range();
    println!(
        "P0: mean ratio min {:.4}, edges [{lo:.4}, {hi:.4}] in [{}, {}]",
        r.min_quality(),
        st.l_min,
        st.l_max
    );
    assert!(r.min_quality() >= 0.56, "P0: M0 mean ratio");
    assert!(lo >= st.l_min && hi <= st.l_max, "P0: M0 edges");
    assert!(!r.needs_event(), "P0: M0 triggers no event");
    let xmax = r.plane_index("xmax").unwrap();
    let gamma = Gas::AIR.gamma;
    let mut s = CompressibleSolver::new(geom(&r), Gas::AIR, CompSettings::default(), |_| rest()).unwrap();
    let (p0, v0) = mean_pressure(&s);
    let inv0 = p0 * v0.powf(gamma);
    let m_start = s.totals()[0];
    let mut q_run = r.min_quality();
    let (mut b1v, mut b1m, mut b1p, mut b1e) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut b2s, mut b3, mut drift, mut diag) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut l1, mut r2, mut n1) = (f64::INFINITY, 0.0f64, 0.0f64);
    let (mut events, mut idle, mut unresolved, mut steps) = (0usize, 0usize, 0usize, 0usize);
    let mut ops = OpCounts::default();
    let mut a = [f64::NAN; 2];
    let mut cells_at = [0usize; 2];
    let mut spread = (f64::NAN, f64::NAN);
    let mut t = 0.0;
    for (stroke, target) in [T, 2.0 * T].into_iter().enumerate() {
        while t < target {
            let mut dt = s.max_dt();
            let last = t + dt >= target - 1e-9;
            if last {
                dt = target - t;
            }
            let t_new = if last { target } else { t + dt };
            let m_before = s.totals()[0];
            r.move_plane_with_band(xmax, piston_x(t_new), BAND).unwrap();
            s.step_ale(geom(&r), dt).unwrap();
            t = t_new;
            steps += 1;
            let (tot, _) = totals(&s);
            b2s = b2s.max((tot[1] / m_before - 1.0).abs());
            drift = drift.max((tot[1] / m_start - 1.0).abs());
            b3 = b3.max((tot[0] / (0.01 * piston_x(t)) - 1.0).abs());
            q_run = q_run.min(r.min_quality());
            let (pm, vm) = mean_pressure(&s);
            diag = diag.max((pm * vm.powf(gamma) / inv0 - 1.0).abs());
            // the trigger, then the event (what remesh() does), so that a triggered event that changes nothing is counted
            if !r.needs_event() {
                continue;
            }
            let Some(ch) = r.event().expect("R1: no event returns an error") else {
                idle += 1;
                continue;
            };
            let (before, scale) = totals(&s);
            let g = geom(&r);
            // N1: every new cell takes the density of its largest source
            let vn = g.mesh.cell_volumes();
            let bad: f64 = ch
                .sources
                .iter()
                .zip(vn)
                .map(|(src, v)| {
                    let big = src
                        .iter()
                        .fold((0usize, -1.0f64), |m, &(o, f)| if f > m.1 { (o, f) } else { m });
                    s.u[big.0][0] * v
                })
                .sum();
            n1 = n1.max((bad / before[1] - 1.0).abs());
            let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
            s.replace_mesh(g, u).unwrap();
            let (after, _) = totals(&s);
            b1v = b1v.max((after[0] / before[0] - 1.0).abs());
            b1m = b1m.max((after[1] / before[1] - 1.0).abs());
            for k in 2..5 {
                b1p = b1p.max((after[k] - before[k]).abs() / scale);
            }
            b1e = b1e.max((after[5] / before[5] - 1.0).abs());
            b3 = b3.max((after[0] / (0.01 * piston_x(t)) - 1.0).abs());
            q_run = q_run.min(r.min_quality());
            l1 = l1.min(ch.lo[0]);
            r2 = r2.max(ch.mismatch);
            add_ops(&mut ops, &ch.ops);
            events += 1;
            if r.needs_event() {
                unresolved += 1;
            }
        }
        let (pm, vm) = mean_pressure(&s);
        a[stroke] = pm * vm.powf(gamma) / inv0 - 1.0;
        cells_at[stroke] = r.cells().len();
        if stroke == 0 {
            let q: Vec<f64> = s.primitives().iter().map(|p| p.p / p.rho.powf(gamma)).collect();
            spread = q.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                (lo.min(x), hi.max(x))
            });
            println!(
                "at T: V/V0 = {:.6}, p_bar = {pm:.6} against the isentrope's {:.6}, {} cells",
                vm / v0,
                p0 * (v0 / vm).powf(gamma),
                r.cells().len()
            );
        }
    }
    println!(
        "A1 {:.3e}  A2 {:.3e}  (limit 2e-2; largest over the steps {diag:.3e})",
        a[0], a[1]
    );
    println!("B1v {b1v:.3e}  B1m {b1m:.3e}  B1p {b1p:.3e}  B1e {b1e:.3e}  (limit 1e-13)");
    println!("B4 events {events} (limit >= 5); triggered without change {idle}; a trigger left after {unresolved}");
    println!(
        "Q smallest mean ratio {q_run:.4} (limit 0.2); final mesh: mean ratio {:.4}, smallest dihedral {:.2} deg",
        r.min_quality(),
        min_dihedral(&r)
    );
    println!(
        "B2s {b2s:.3e} (limit 1e-13)  B3 {b3:.3e} (limit 1e-13)  L1 {l1:.4} (limit >= 0.2)  R2 {r2:.3e} (limit 1e-11)"
    );
    println!("N1 {n1:.3e} (limit >= 1e-10); mass drift over the run {drift:.3e} (no limit)");
    println!(
        "operators {ops:?}; cells at T {} and 2T {}; p/rho^gamma at T in [{:.6}, {:.6}]",
        cells_at[0], cells_at[1], spread.0, spread.1
    );
    println!("{steps} steps, {:.1} s", clock.elapsed().as_secs_f64());
    assert!(a[0].abs() <= 2e-2, "A1 {:e}", a[0]);
    assert!(a[1].abs() <= 2e-2, "A2 {:e}", a[1]);
    assert!(b1v <= 1e-13, "B1v {b1v:e}");
    assert!(b1m <= 1e-13, "B1m {b1m:e}");
    assert!(b1p <= 1e-13, "B1p {b1p:e}");
    assert!(b1e <= 1e-13, "B1e {b1e:e}");
    assert!(events >= 5, "B4 {events}");
    assert!(q_run >= Q_MIN, "Q {q_run}");
    assert!(b2s <= 1e-13, "B2s {b2s:e}");
    assert!(b3 <= 1e-13, "B3 {b3:e}");
    assert!(l1 >= 0.2, "L1 {l1}");
    assert!(r2 <= 1e-11, "R2 {r2:e}");
    assert!(n1 >= 1e-10, "N1 {n1:e}");
}

/// N2 of ADR 0215: the same piston without remeshing breaks the mesh before full compression.
#[test]
fn n2_without_remeshing_the_piston_breaks_the_mesh_before_full_compression() {
    let tm = m0();
    let mut r = remesher(&tm);
    let xmax = r.plane_index("xmax").unwrap();
    let mut s = CompressibleSolver::new(geom(&r), Gas::AIR, CompSettings::default(), |_| rest()).unwrap();
    let mut t = 0.0;
    let mut why = String::new();
    while t < T {
        let mut dt = s.max_dt();
        if t + dt >= T - 1e-9 {
            dt = T - t;
        }
        r.move_plane_with_band(xmax, piston_x(t + dt), BAND).unwrap();
        let mesh: Mesh = match r.mesh() {
            Ok(m) => m,
            Err(e) => {
                why = format!("mesh: {e}");
                break;
            }
        };
        let g = match FvGeometry::new(Arc::new(mesh)) {
            Ok(g) => g,
            Err(e) => {
                why = format!("geometry: {e}");
                break;
            }
        };
        if let Err(e) = s.step_ale(g, dt) {
            why = format!("step: {e}");
            break;
        }
        t += dt;
        let q = r.min_quality();
        if q < Q_MIN {
            why = format!("mean ratio {q:.4}");
            break;
        }
    }
    println!(
        "N2: without remeshing the run breaks at t = {t:.4} (x_p = {:.4}): {why}",
        piston_x(t)
    );
    assert!(t < T && !why.is_empty(), "N2: the run reached T without remeshing");
}
