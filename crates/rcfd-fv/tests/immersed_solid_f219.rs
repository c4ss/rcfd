//! F-219 (ADR 0218): an immersed (non-conforming) solid with modified forcing.
//!
//! Acceptance (as read in ADR 0218): the DFG 2D-1 drag and lift coefficients (Re 20) and the DFG 2D-2 Strouhal number (Re 100) of
//! the flow past a cylinder that is an immersed solid on a Cartesian mesh, each within 5 percent of the FeatFlow reference values
//! (C_D 5.57953523384, C_L 0.010618948146, St 0.30188; read on the TU Dortmund FeatFlow benchmark pages, the set-up of Schaefer
//! and Turek 1996). Verification on circular Couette flow between an immersed rotating cylinder and an immersed stationary ring
//! (exact solution), the momentum balance of the domain, refusals and build errors. The heavy tests run in release only.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// DFG set-up
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
const CD_REF: f64 = 5.579_535_233_84;
const CL_REF: f64 = 0.010_618_948_146;
const ST_REF: f64 = 0.301_88;
const DP_REF: f64 = 0.117_520_166_97;

fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn state_hash(u: &[Vec3], p: &[f64]) -> u64 {
    fnv(u
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(p.iter().map(|x| x.to_bits())))
}

/// `n` cells on `[0, len]`, geometric, the last cell of size `h`.
fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

/// `n` cells on `[0, len]`, geometric, the first cell of size `h`.
fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

/// Coordinates `lo + len (i - i0)/n`-style uniform core with the line `centre` held exactly: `n` cells on `[a, b]`.
fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

/// ADR 0218 set-up: M1 coordinates, bisected `level - 1` times (level 1, 2, 3).
fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

/// ADR 0218 T mesh of 2D-2.
fn t_mesh() -> (Vec<f64>, Vec<f64>) {
    let xs = join(&[uniform(0.0, 0.6, 240, 0.2), geo_start(84, 1.6, 0.0025)], &[0.0, 0.6]);
    let ys = uniform(0.0, H, 164, 0.2);
    (xs, ys)
}

fn cylinder() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    )
}

/// Exact face average of the inflow profile `4 U_m y (H - y)/H^2` over `[y1, y2]`.
fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

struct Dfg {
    geom: Arc<FvGeometry>,
    xs: Vec<f64>,
    ys: Vec<f64>,
    u: VectorField,
    p: ScalarField,
    ib: ImmersedForcing,
}

fn dfg(xs: Vec<f64>, ys: Vec<f64>, um: f64, modified: bool) -> Dfg {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(&xs, &ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = ImmersedSettings::new(K_DFG);
    st.modified = modified;
    let ib = ImmersedForcing::build(&geom, &[cylinder()], st, &uniform_gamma(&geom, NU)).unwrap();
    Dfg { geom, xs, ys, u, p, ib }
}

/// `(C_D, C_L)` of the cylinder from the forcing.
fn coefficients(ib: &ImmersedForcing, u: &[Vec3], ubar: f64) -> (f64, f64) {
    let (f, _) = ib.force_moment(0, u, 1.0, CENTRE);
    let q = 0.5 * ubar * ubar * DIAM * DZ;
    (f[0] / q, f[1] / q)
}

/// S3: the solid cells are mirror-symmetric about `x = 0.2` and `y = 0.2`.
fn assert_mirror_symmetric(d: &Dfg) {
    let (nx, ny) = (d.xs.len() - 1, d.ys.len() - 1);
    let ix0 =
        d.xs.iter()
            .position(|&x| (x - 0.2).abs() < 1e-12)
            .expect("x = 0.2 is a grid line");
    let iy0 =
        d.ys.iter()
            .position(|&y| (y - 0.2).abs() < 1e-12)
            .expect("y = 0.2 is a grid line");
    let solid = d.ib.solid_mask();
    let mut count = 0;
    for j in 0..ny {
        for i in 0..nx {
            if !solid[i + nx * j] {
                continue;
            }
            count += 1;
            let im = 2 * ix0 - 1 - i;
            let jm = 2 * iy0 - 1 - j;
            assert!(solid[im + nx * j], "S3: solid cell ({i}, {j}) has no x mirror");
            assert!(solid[i + nx * jm], "S3: solid cell ({i}, {j}) has no y mirror");
        }
    }
    let n_mod = d.ib.bodies[0].terms.iter().filter(|t| t.face.is_some()).count();
    let theta_min = d.ib.bodies[0]
        .terms
        .iter()
        .filter(|t| t.face.is_some())
        .map(|t| t.theta)
        .fold(f64::INFINITY, f64::min);
    println!(
        "mesh {} x {} = {} cells, {count} solid cells, {n_mod} modification terms, smallest theta {theta_min:.4e}",
        nx,
        ny,
        nx * ny
    );
}

fn max_solid_speed(ib: &ImmersedForcing, u: &[Vec3]) -> f64 {
    ib.bodies[0]
        .solid_cells
        .iter()
        .map(|&c| vec3::norm(u[c]))
        .fold(0.0, f64::max)
}

struct Steady {
    solver: SimpleSolver,
    ib: ImmersedForcing,
    xs: Vec<f64>,
    ys: Vec<f64>,
    cd: f64,
    cl: f64,
    iterations: usize,
    converged: bool,
    seconds: f64,
}

/// One 2D-1 run (ADR 0218): SIMPLEC, linear upwind, the convergence rule on `C_D`, `C_L`; `init` = the converged state of the
/// coarser mesh of the family (injected).
fn run_2d1(level: usize, init: Option<&Steady>, cap: usize, modified: bool) -> Steady {
    let (xs, ys) = dfg_family(level);
    let d = dfg(xs, ys, 0.3, modified);
    if modified {
        assert_mirror_symmetric(&d);
    }
    let (nx, ny) = (d.xs.len() - 1, d.ys.len() - 1);
    let expected = [4576, 18304, 73216][level - 1];
    assert_eq!(nx * ny, expected, "S3: cell count of M{level}");
    let ubar = 0.2;
    let mut u = d.u;
    let mut p = d.p;
    if let Some(c) = init {
        let nxc = c.xs.len() - 1;
        for j in 0..ny {
            for i in 0..nx {
                let pc = i / 2 + nxc * (j / 2);
                u.values[i + nx * j] = c.solver.u.values[pc];
                p.values[i + nx * j] = c.solver.p.values[pc];
            }
        }
    }
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.convection = ConvScheme::LinearUpwind;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(d.geom.clone(), u, p, st);
    s.immersed = Some(d.ib.clone());
    let t0 = Instant::now();
    // C_D, C_L after every 10 iterations, starting with the initial state (iteration 0)
    let mut hist: Vec<(f64, f64)> = vec![coefficients(&d.ib, &s.u.values, ubar)];
    let (mut iterations, mut converged) = (0, false);
    let (mut cd, mut cl) = (f64::NAN, f64::NAN);
    for it in 1..=cap {
        let info = s.iterate();
        iterations = it;
        if it % 10 == 0 {
            (cd, cl) = coefficients(&d.ib, &s.u.values, ubar);
            hist.push((cd, cl));
            if it % 500 == 0 {
                println!(
                    "M{level} it {it}: C_D {cd:.8} C_L {cl:.8e} res_u {:.3e} res_p {:.3e} max_du {:.3e} p_iters {} ({:.0} s)",
                    info.res_u,
                    info.res_p,
                    info.max_du,
                    info.p_iters,
                    t0.elapsed().as_secs_f64()
                );
            }
            if !cd.is_finite() {
                break;
            }
            if it >= 200 {
                let (cd0, cl0) = hist[hist.len() - 21];
                if (cd - cd0).abs() <= 1e-7 * cd.abs() && (cl - cl0).abs() <= 1e-8 {
                    converged = true;
                    break;
                }
            }
        }
    }
    let seconds = t0.elapsed().as_secs_f64();
    let last = s.history.last().copied();
    println!(
        "M{level} (modified {modified}): converged {converged} after {iterations} iterations ({seconds:.0} s); C_D {cd:.8} \
         ({:+.4} %), C_L {cl:.8e} ({:+.4} %); last {last:?}; hash {:016x}",
        100.0 * (cd - CD_REF) / CD_REF,
        100.0 * (cl - CL_REF) / CL_REF,
        state_hash(&s.u.values, &s.p.values)
    );
    Steady {
        solver: s,
        ib: d.ib,
        xs: d.xs,
        ys: d.ys,
        cd,
        cl,
        iterations,
        converged,
        seconds,
    }
}

/// Printed, no limit: the pressure difference and the recirculation length from the two rows at `y = 0.2 -+ h/2`.
fn print_dp_and_la(r: &Steady) {
    let (nx, _) = (r.xs.len() - 1, r.ys.len() - 1);
    let jy = r.ys.iter().position(|&y| (y - 0.2).abs() < 1e-12).unwrap();
    let rows = [jy - 1, jy];
    let solid = r.ib.solid_mask();
    let xc = |i: usize| 0.5 * (r.xs[i] + r.xs[i + 1]);
    let front = (0..nx).filter(|&i| xc(i) < 0.15 && !solid[i + nx * jy]).max().unwrap();
    let back = (0..nx).filter(|&i| xc(i) > 0.25 && !solid[i + nx * jy]).min().unwrap();
    let pm = |i: usize| rows.iter().map(|&j| r.solver.p.values[i + nx * j]).sum::<f64>() / 2.0;
    let dp = pm(front) - pm(back);
    let um = |i: usize| rows.iter().map(|&j| r.solver.u.values[i + nx * j][0]).sum::<f64>() / 2.0;
    let mut la = f64::NAN;
    for i in back..nx - 1 {
        if um(i) < 0.0 && um(i + 1) >= 0.0 {
            let t = -um(i) / (um(i + 1) - um(i));
            la = xc(i) + t * (xc(i + 1) - xc(i)) - 0.25;
            break;
        }
    }
    println!(
        "pressure difference (cells at x {:.5} and {:.5}) {dp:.6} (FeatFlow {DP_REF}); recirculation length {la:.5}",
        xc(front),
        xc(back)
    );
}

/// The momentum balance of the domain (ADR 0218 V2): `F_bal = -rho sum_b [phi_b u_b + p_b S_b - gamma_b lap_coeff_b (u_b - u_c)]`
/// over the non-empty boundary faces, the diffusive term on fixed-velocity faces through the boundary coefficients.
fn domain_balance(s: &SimpleSolver) -> Vec3 {
    let g = &s.geom;
    let mesh = &g.mesh;
    let n_int = g.n_internal;
    let ub = s.u.boundary_values(g);
    let pbc = rcfd_fv::pressure::pressure_bc(g, &s.u, &s.p);
    let gamma = uniform_gamma(g, NU);
    let bck: Vec<_> = (0..3).map(|k| s.u.boundary_coeffs(g, k)).collect();
    let sf = mesh.face_area_vectors();
    let mut tot = vec3::ZERO;
    for f in n_int..mesh.n_faces() {
        if g.empty[f] {
            continue;
        }
        let i = f - n_int;
        let c = mesh.owner()[f];
        let pb = pbc.coeffs[i].value(s.p.values[c]);
        let area = vec3::norm(sf[f]);
        for k in 0..3 {
            if !g.active[k] {
                continue;
            }
            let conv = s.phi[f] * ub[i][k];
            let diff = -gamma[f] * area * bck[k][i].gradient(s.u.values[c][k]);
            tot[k] += conv + diff + pb * sf[f][k];
        }
    }
    vec3::scale(tot, -1.0)
}

// ---------------------------------------------------------------------------------------------------------------------------
// 2D-1
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0218 S1, S3, E1 (M1), V2 and V2n on the converged M1 state.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn v2_m1_two_d_one_momentum_balance_and_solid() {
    let r = run_2d1(1, None, 4000, true);
    assert!(r.converged, "S1: M1 not converged in 4000 iterations");
    let v = max_solid_speed(&r.ib, &r.solver.u.values);
    println!("E1 M1: max |u| in the solid {v:.3e} = {:.3e} U_bar", v / 0.2);
    assert!(v <= 1e-3 * 0.2, "E1 (M1): {v:.3e}");
    let (f, _) = r.ib.force_moment(0, &r.solver.u.values, 1.0, CENTRE);
    let bal = domain_balance(&r.solver);
    // the forcing without its modification terms (V2n)
    let mut fp = vec3::ZERO;
    let vol = r.solver.geom.mesh.cell_volumes();
    for t in r.ib.bodies[0].terms.iter().filter(|t| t.face.is_none()) {
        for k in 0..3 {
            fp[k] += vol[t.cell] * (t.coeff * r.solver.u.values[t.cell][k] - t.source[k]);
        }
    }
    let ex = (f[0] - bal[0]).abs() / f[0].abs();
    let ey = (f[1] - bal[1]).abs() / f[0].abs();
    let exn = (fp[0] - bal[0]).abs() / f[0].abs();
    println!(
        "V2: forcing ({:.10e}, {:.10e}), balance ({:.10e}, {:.10e}): |dF_x|/|F_x| {ex:.3e}, |dF_y|/|F_x| {ey:.3e}; \
         V2n: without the modification ({:.10e}, {:.10e}), |dF_x|/|F_x| {exn:.3e}",
        f[0], f[1], bal[0], bal[1], fp[0], fp[1]
    );
    assert!(ex <= 1e-3 && ey <= 1e-3, "V2: {ex:.3e}, {ey:.3e}");
    assert!(exn >= 1e-2, "V2n (negative control) did not fire: {exn:.3e}");
}

/// ADR 0218 A1, A2 (M3), S1 (M1 to M3), S3, E1 (M3); prints the orders, the pressure difference and the recirculation length.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_a2_two_d_one_drag_and_lift_on_m3() {
    let m1 = run_2d1(1, None, 4000, true);
    assert!(m1.converged, "S1: M1 not converged");
    let m2 = run_2d1(2, Some(&m1), 4000, true);
    assert!(m2.converged, "S1: M2 not converged");
    let m3 = run_2d1(3, Some(&m2), 6000, true);
    assert!(m3.converged, "S1: M3 not converged in 6000 iterations");
    let order = |a: f64, b: f64, c: f64| ((a - b).abs() / (b - c).abs()).log2();
    println!(
        "C_D M1 {:.8} M2 {:.8} M3 {:.8} (order {:.3}); C_L M1 {:.8e} M2 {:.8e} M3 {:.8e} (order {:.3}); iterations {} {} {}, \
         seconds {:.0} {:.0} {:.0}",
        m1.cd,
        m2.cd,
        m3.cd,
        order(m1.cd, m2.cd, m3.cd),
        m1.cl,
        m2.cl,
        m3.cl,
        order(m1.cl, m2.cl, m3.cl),
        m1.iterations,
        m2.iterations,
        m3.iterations,
        m1.seconds,
        m2.seconds,
        m3.seconds
    );
    for r in [&m1, &m2, &m3] {
        print_dp_and_la(r);
    }
    let v = max_solid_speed(&m3.ib, &m3.solver.u.values);
    println!("E1 M3: max |u| in the solid {v:.3e} = {:.3e} U_bar", v / 0.2);
    let ecd = (m3.cd - CD_REF).abs() / CD_REF;
    let ecl = (m3.cl - CL_REF).abs() / CL_REF;
    println!(
        "A1: C_D {:.8} error {:.4} %; A2: C_L {:.8e} error {:.4} %",
        m3.cd,
        100.0 * ecd,
        m3.cl,
        100.0 * ecl
    );
    assert!(v <= 1e-3 * 0.2, "E1 (M3): {v:.3e}");
    assert!(ecd <= 0.05, "A1: C_D {} error {ecd:.4}", m3.cd);
    assert!(ecl <= 0.05, "A2: C_L {} error {ecl:.4}", m3.cl);
}

/// Printed, no limit (ADR 0218): the 2D-1 forces with the plain (unmodified) forcing on M1 and M2.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn two_d_one_plain_forcing_printed() {
    let m1 = run_2d1(1, None, 4000, false);
    let m2 = run_2d1(2, Some(&m1), 4000, false);
    println!(
        "plain forcing: M1 C_D {:.8} C_L {:.8e} (converged {}); M2 C_D {:.8} C_L {:.8e} (converged {})",
        m1.cd, m1.cl, m1.converged, m2.cd, m2.cl, m2.converged
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// 2D-2
// ---------------------------------------------------------------------------------------------------------------------------

/// Upward crossings of `v - mean` (linear interpolation).
fn crossings(t: &[f64], v: &[f64], mean: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            out.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    out
}

/// ADR 0218 A3, S2, S3 (T), E1 (T).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a3_two_d_two_strouhal_number() {
    let (xs, ys) = t_mesh();
    let um = 1.5;
    let ubar = 1.0;
    let d = dfg(xs, ys, um, true);
    assert_mirror_symmetric(&d);
    assert_eq!(d.geom.n_cells(), 53136, "S3: cell count of T");
    let mut u = d.u;
    let cc = d.geom.mesh.cell_centres().to_vec();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * ubar * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.n_correctors = 2;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(d.geom.clone(), u, d.p, st);
    s.immersed = Some(d.ib.clone());
    let dt = 0.001;
    let steps = 8000;
    let t0 = Instant::now();
    let (mut ts, mut cds, mut cls) = (Vec::new(), Vec::new(), Vec::new());
    for n in 1..=steps {
        let info = s.step(dt);
        assert!(s.diverged.is_none(), "2D-2 stopped: {:?}", s.diverged);
        let (cd, cl) = coefficients(&d.ib, &s.u.values, ubar);
        let t = n as f64 * dt;
        ts.push(t);
        cds.push(cd);
        cls.push(cl);
        if n % 500 == 0 {
            println!(
                "t {t:.3}: C_D {cd:.5} C_L {cl:+.5} Co {:.3} continuity {:.2e} p_iters {} ({:.0} s)",
                info.courant,
                info.continuity,
                info.p_iters,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let w0 = ts.iter().position(|&t| t >= 5.5 - 1e-12).unwrap();
    let (tw, dw, lw) = (&ts[w0..], &cds[w0..], &cls[w0..]);
    let mean = lw.iter().sum::<f64>() / lw.len() as f64;
    let cr = crossings(tw, lw, mean);
    let n = cr.len();
    assert!(n >= 6, "S2: {n} upward crossings in the window");
    let f = (n - 1) as f64 / (cr[n - 1] - cr[0]);
    let st_num = f * DIAM / ubar;
    let periods: Vec<f64> = cr.windows(2).map(|w| w[1] - w[0]).collect();
    let pm = periods.iter().sum::<f64>() / periods.len() as f64;
    let pdev = periods.iter().map(|p| (p - pm).abs() / pm).fold(0.0, f64::max);
    // per period (between successive crossings): max C_L, extremes of C_D
    let mut maxima = Vec::new();
    let (mut cd_max, mut cd_min, mut cl_max, mut cl_min) = (f64::MIN, f64::MAX, f64::MIN, f64::MAX);
    for w in cr.windows(2) {
        let idx: Vec<usize> = (0..tw.len()).filter(|&i| tw[i] >= w[0] && tw[i] < w[1]).collect();
        maxima.push(idx.iter().map(|&i| lw[i]).fold(f64::MIN, f64::max));
        for &i in &idx {
            cd_max = cd_max.max(dw[i]);
            cd_min = cd_min.min(dw[i]);
            cl_max = cl_max.max(lw[i]);
            cl_min = cl_min.min(lw[i]);
        }
    }
    let mm = maxima.iter().sum::<f64>() / maxima.len() as f64;
    let mdev = maxima.iter().map(|m| (m - mm).abs() / mm.abs()).fold(0.0, f64::max);
    let amp = lw.iter().copied().fold(f64::MIN, f64::max) - lw.iter().copied().fold(f64::MAX, f64::min);
    let cd_mean = dw.iter().sum::<f64>() / dw.len() as f64;
    let v = max_solid_speed(&d.ib, &s.u.values);
    println!(
        "2D-2: {n} crossings, St {st_num:.5} ({:+.3} % of {ST_REF}), 1/f {:.5} s, period spread {pdev:.3e}, C_L max spread {mdev:.3e}, \
         C_L amplitude {amp:.4}; C_D max {cd_max:.4} min {cd_min:.4} mean {cd_mean:.4}; C_L max {cl_max:.4} min {cl_min:.4}; \
         E1 max |u| solid {v:.3e}; {:.0} s; hash {:016x}",
        100.0 * (st_num - ST_REF) / ST_REF,
        1.0 / f,
        t0.elapsed().as_secs_f64(),
        state_hash(&s.u.values, &s.p.values)
    );
    assert!(pdev <= 0.01, "S2: period spread {pdev:.3e}");
    assert!(mdev <= 0.02, "S2: C_L maximum spread {mdev:.3e}");
    assert!(amp >= 1.0, "S2: C_L amplitude {amp:.4}");
    assert!(v <= 1e-3 * ubar, "E1 (T): {v:.3e}");
    let e = (st_num - ST_REF).abs() / ST_REF;
    assert!(e <= 0.05, "A3: St {st_num} error {e:.4}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette (CC)
// ---------------------------------------------------------------------------------------------------------------------------

const CC_C: Vec3 = [1.2, 1.2, 0.0];
const R1: f64 = 0.5;
const R2: f64 = 1.0;
const OMEGA: f64 = 1.0;

fn cc_bodies() -> Vec<ImmersedBody> {
    let mut inner = ImmersedBody::fixed(
        "inner",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R1,
        },
    );
    inner.omega = [0.0, 0.0, OMEGA];
    inner.origin = CC_C;
    let mut outer = ImmersedBody::fixed(
        "outer",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R2,
        },
    );
    outer.inverted = true;
    vec![inner, outer]
}

fn cc_exact(x: Vec3) -> Vec3 {
    let (dx, dy) = (x[0] - CC_C[0], x[1] - CC_C[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let a = -OMEGA * R1 * R1 / (R2 * R2 - R1 * R1);
    let b = OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let ut = a * r + b / r;
    [-ut * dy / r, ut * dx / r, 0.0]
}

fn cc_fields(n: usize, modified: bool) -> (Arc<FvGeometry>, VectorField, ScalarField, ImmersedForcing) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = ImmersedSettings::new(1e8);
    st.modified = modified;
    let ib = ImmersedForcing::build(&geom, &cc_bodies(), st, &uniform_gamma(&geom, 1.0)).unwrap();
    (geom, u, p, ib)
}

/// `e = sqrt(sum_F V |u - u_exact|^2 / sum_F V) / (omega R1)` over the fluid cells.
fn cc_error(geom: &FvGeometry, ib: &ImmersedForcing, u: &[Vec3]) -> f64 {
    let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
    let (mut num, mut den) = (0.0, 0.0);
    for c in 0..geom.n_cells() {
        if ib.body_of_cell[c].is_some() {
            continue;
        }
        let r = vec3::norm(vec3::sub(cc[c], [CC_C[0], CC_C[1], cc[c][2]]));
        assert!(r > R1 && r < R2, "fluid cell {c} outside the annulus (r = {r})");
        let d = vec3::sub(u[c], cc_exact(cc[c]));
        num += vol[c] * vec3::dot(d, d);
        den += vol[c];
    }
    (num / den).sqrt() / (OMEGA * R1)
}

struct CcRun {
    e: f64,
    converged: bool,
    iterations: usize,
}

fn cc_simplec(n: usize, modified: bool) -> CcRun {
    let (geom, u, p, ib) = cc_fields(n, modified);
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let (mut iterations, mut converged) = (0, false);
    for it in 1..=20000 {
        let info = s.iterate();
        iterations = it;
        if !info.max_du.is_finite() {
            break;
        }
        if info.max_du <= 1e-10 {
            converged = true;
            break;
        }
    }
    let e = cc_error(&geom, &ib, &s.u.values);
    let dz = 2.4 / n as f64;
    let (_, m_in) = ib.force_moment(0, &s.u.values, 1.0, CC_C);
    let (_, m_out) = ib.force_moment(1, &s.u.values, 1.0, CC_C);
    let exact = 4.0 * std::f64::consts::PI * OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    println!(
        "CC N {n} modified {modified}: e {e:.5e}, converged {converged} after {iterations}; torque inner {:.5} outer {:.5} (exact -+{exact:.5}); \
         solid {} cells, modification terms {}",
        m_in[2] / dz,
        m_out[2] / dz,
        ib.solid_mask().iter().filter(|&&b| b).count(),
        ib.bodies.iter().flat_map(|b| &b.terms).filter(|t| t.face.is_some()).count()
    );
    CcRun {
        e,
        converged,
        iterations,
    }
}

/// ADR 0218 V1, V1n and S4.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn v1_v1n_circular_couette_order() {
    let m: Vec<CcRun> = [24, 48, 96].iter().map(|&n| cc_simplec(n, true)).collect();
    let pl: Vec<CcRun> = [24, 48, 96].iter().map(|&n| cc_simplec(n, false)).collect();
    for r in m.iter().chain(&pl) {
        assert!(r.converged, "S4: CC run not converged ({} iterations)", r.iterations);
    }
    let order = (m[1].e / m[2].e).log2();
    let order_plain = (pl[1].e / pl[2].e).log2();
    println!(
        "V1: modified e {:.4e} {:.4e} {:.4e}, order {order:.3}; V1n: plain e {:.4e} {:.4e} {:.4e}, order {order_plain:.3}, ratio {:.3}",
        m[0].e,
        m[1].e,
        m[2].e,
        pl[0].e,
        pl[1].e,
        pl[2].e,
        pl[2].e / m[2].e
    );
    assert!(m[2].e <= 1e-2, "V1: e(96) {:.4e}", m[2].e);
    assert!(order >= 1.5, "V1: order {order:.3}");
    assert!(
        pl[2].e >= 2.0 * m[2].e,
        "V1n (negative control) did not fire: {:.4e} vs {:.4e}",
        pl[2].e,
        m[2].e
    );
}

/// ADR 0218 V3: the PISO path of the forcing on CC at N = 48.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn v3_circular_couette_with_piso() {
    let simplec = cc_simplec(48, true);
    assert!(simplec.converged, "S4: CC SIMPLEC 48 not converged");
    let (geom, u, p, ib) = cc_fields(48, true);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let mut change = f64::NAN;
    for _ in 0..400 {
        let before = s.u.values.clone();
        s.step(0.0025);
        assert!(s.diverged.is_none(), "{:?}", s.diverged);
        change =
            s.u.values
                .iter()
                .zip(&before)
                .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
                .fold(0.0, f64::max);
    }
    let e = cc_error(&geom, &ib, &s.u.values);
    println!(
        "V3: PISO e {e:.5e} (SIMPLEC {:.5e}), last step change {change:.3e}",
        simplec.e
    );
    assert!(change <= 1e-9, "V3 set-up: not steady ({change:.3e})");
    assert!(e <= 1.5 * simplec.e, "V3: e {e:.4e} against SIMPLEC {:.4e}", simplec.e);
}

// ---------------------------------------------------------------------------------------------------------------------------
// G1, B1, R1 to R3, N1
// ---------------------------------------------------------------------------------------------------------------------------

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

/// ADR 0218 G1.
#[test]
fn g1_level_sets_and_crossings() {
    let mut seed = 0x2545_F491_4F6C_DD1D_u64;
    let cyl = SolidShape::Cylinder {
        centre: [0.3, -0.2, 5.0],
        axis: [0.0, 0.0, 2.0],
        radius: 0.7,
    };
    let sph = SolidShape::Sphere {
        centre: [0.1, 0.2, 0.3],
        radius: 0.5,
    };
    let lo = [-0.5, -0.25, 0.0];
    let hi = [0.5, 0.75, 0.4];
    let cub = SolidShape::Cuboid { lo, hi };
    let mut worst: f64 = 0.0;
    for _ in 0..20 {
        let x = [0, 1, 2].map(|_| 3.0 * lcg(&mut seed) - 1.5);
        let dc = ((x[0] - 0.3).powi(2) + (x[1] + 0.2).powi(2)).sqrt() - 0.7;
        let ds = ((x[0] - 0.1).powi(2) + (x[1] - 0.2).powi(2) + (x[2] - 0.3).powi(2)).sqrt() - 0.5;
        let inside = (0..3).all(|k| x[k] > lo[k] && x[k] < hi[k]);
        let db = if inside {
            -(0..3)
                .map(|k| (x[k] - lo[k]).min(hi[k] - x[k]))
                .fold(f64::INFINITY, f64::min)
        } else {
            let q: Vec3 = [0, 1, 2].map(|k| x[k] - x[k].clamp(lo[k], hi[k]));
            vec3::norm(q)
        };
        worst = worst
            .max((cyl.signed_distance(x) - dc).abs())
            .max((sph.signed_distance(x) - ds).abs())
            .max((cub.signed_distance(x) - db).abs());
    }
    println!("G1: level sets {worst:.3e}");
    assert!(worst <= 1e-13, "G1 level sets {worst:.3e}");
    let body = ImmersedBody::fixed(
        "c",
        SolidShape::Cylinder {
            centre: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            radius: 1.0,
        },
    );
    let mut worst_t: f64 = 0.0;
    let mut k = 0;
    while k < 20 {
        let a = [3.0 * lcg(&mut seed) - 1.5, 3.0 * lcg(&mut seed) - 1.5, 0.0];
        let b = [1.4 * lcg(&mut seed) - 0.7, 1.4 * lcg(&mut seed) - 0.7, 0.0];
        if body.level(a) <= 0.0 || body.level(b) >= 0.0 {
            continue;
        }
        k += 1;
        let d = vec3::sub(b, a);
        // |a + t d|^2 = 1: t^2 d.d + 2 t a.d + a.a - 1 = 0, the root in [0, 1]
        let (qa, qb, qc) = (vec3::dot(d, d), 2.0 * vec3::dot(a, d), vec3::dot(a, a) - 1.0);
        let disc = (qb * qb - 4.0 * qa * qc).sqrt();
        let roots = [(-qb - disc) / (2.0 * qa), (-qb + disc) / (2.0 * qa)];
        let t = roots.iter().copied().find(|t| (0.0..=1.0).contains(t)).unwrap();
        worst_t = worst_t.max((body.crossing(a, b) - t).abs());
    }
    println!("G1: crossings {worst_t:.3e}");
    assert!(worst_t <= 1e-12, "G1 crossings {worst_t:.3e}");
}

/// ADR 0218 B1.
#[test]
fn b1_build_errors_are_located() {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(10, 10, 1.0, 1.0, false, false).unwrap())).unwrap());
    let gamma = uniform_gamma(&geom, 1.0);
    let circle = |r: f64| {
        ImmersedBody::fixed(
            "disc",
            SolidShape::Cylinder {
                centre: [0.5, 0.5, 0.0],
                axis: [0.0, 0.0, 1.0],
                radius: r,
            },
        )
    };
    let err =
        |bodies: &[ImmersedBody], st: ImmersedSettings| ImmersedForcing::build(&geom, bodies, st, &gamma).unwrap_err();
    let ok = ImmersedSettings::new(1e6);
    for (pen, what) in [(0.0, "penalty"), (f64::NAN, "penalty"), (-1.0, "penalty")] {
        let e = err(&[circle(0.2)], ImmersedSettings::new(pen));
        assert!(e.contains(what), "{e}");
    }
    for tm in [0.0, 1.5, f64::NAN] {
        let mut st = ok;
        st.theta_min = tm;
        let e = err(&[circle(0.2)], st);
        assert!(e.contains("theta_min"), "{e}");
    }
    for r in [0.0, -1.0, f64::INFINITY] {
        let e = err(&[circle(r)], ok);
        assert!(e.contains("radius") && e.contains("disc"), "{e}");
    }
    let mut axis0 = circle(0.2);
    axis0.shape = SolidShape::Cylinder {
        centre: [0.5, 0.5, 0.0],
        axis: [0.0; 3],
        radius: 0.2,
    };
    let e = err(&[axis0], ok);
    assert!(e.contains("axis"), "{e}");
    let mut empty = circle(0.2);
    empty.shape = SolidShape::Cuboid {
        lo: [0.2, 0.2, 0.0],
        hi: [0.2, 0.6, 1.0],
    };
    let e = err(&[empty], ok);
    assert!(e.contains("empty cuboid"), "{e}");
    let mut other = circle(0.3);
    other.name = "ring".into();
    let e = err(&[circle(0.2), other], ok);
    assert!(
        e.contains("cell") && e.contains("'disc'") && e.contains("'ring'"),
        "{e}"
    );
    let pgeom = Arc::new(FvGeometry::new(Arc::new(box_2d(10, 10, 1.0, 1.0, true, false).unwrap())).unwrap());
    let mut edge = circle(0.2);
    edge.shape = SolidShape::Cylinder {
        centre: [0.0, 0.5, 0.0],
        axis: [0.0, 0.0, 1.0],
        radius: 0.2,
    };
    let e = ImmersedForcing::build(&pgeom, &[edge], ok, &uniform_gamma(&pgeom, 1.0)).unwrap_err();
    assert!(e.contains("periodic"), "{e}");
    let e = ImmersedForcing::build(&geom, &[circle(0.2)], ok, &gamma[1..]).unwrap_err();
    assert!(e.contains("gamma"), "{e}");
    println!("B1: every build error located");
}

fn small_piso(fractional: bool) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(12, 12, 1.0, 1.0, false, false).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| [0.1 * x[1], 0.0, 0.0]);
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    for w in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(0.01);
    if fractional {
        st.fractional = Some(FractionalSettings::default());
    }
    let ib = ImmersedForcing::build(
        &geom,
        &[ImmersedBody::fixed(
            "disc",
            SolidShape::Cylinder {
                centre: [0.5, 0.5, 0.0],
                axis: [0.0, 0.0, 1.0],
                radius: 0.2,
            },
        )],
        ImmersedSettings::new(1e6),
        &uniform_gamma(&geom, 0.01),
    )
    .unwrap();
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib);
    s
}

fn bits(s: &PimpleSolver) -> u64 {
    fnv(s
        .u
        .values
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(s.p.values.iter().map(|x| x.to_bits()))
        .chain(s.phi.iter().map(|x| x.to_bits())))
}

/// ADR 0218 R1.
#[test]
fn r1_fractional_step_refuses_the_immersed_solid() {
    let mut s = small_piso(true);
    let before = bits(&s);
    s.step(0.01);
    let d = s.diverged.clone().expect("R1: the step was not stopped");
    assert_eq!(d.step, 1);
    assert_eq!(d.stage, "fractional step");
    assert_eq!(d.quantity, "immersed solid");
    assert_eq!(bits(&s), before, "R1: the state changed");
    let msg = d.message(None);
    assert!(msg.contains("ADR 0218"), "{msg}");
    println!("R1: {msg}");
}

/// ADR 0218 R2 and R3.
#[test]
fn r2_r3_moving_mesh_refuses_the_immersed_solid() {
    let mut s = small_piso(false);
    let before = bits(&s);
    let g = s.geom.clone();
    let nf = g.mesh.n_faces();
    let e = s
        .step_ale_with_walls(0.01, g.clone(), vec![0.0; nf], Vec::new())
        .unwrap_err();
    assert!(e.contains("ADR 0218"), "R2: {e}");
    assert_eq!(bits(&s), before, "R2: the state changed");
    assert!(Arc::ptr_eq(&s.geom, &g) && s.time == 0.0 && s.history.is_empty());
    let (u, p, phi) = (s.u.values.clone(), s.p.values.clone(), s.phi.clone());
    let e3 = s.replace_mesh(g, u, p, phi).unwrap_err();
    println!("R2: {e}; R3: {e3}");
    assert_eq!(bits(&s), before, "R3: the state changed");
}

/// ADR 0218 N1.
#[test]
fn n1_immersed_forcing_is_off_by_default() {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(4, 4, 1.0, 1.0, false, false).unwrap())).unwrap());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let s = SimpleSolver::new(
        geom.clone(),
        u.clone(),
        p.clone(),
        SimpleSettings::new(1.0, Coupling::Simplec),
    );
    let q = PimpleSolver::new(geom, u, p, PimpleSettings::piso(1.0));
    assert!(s.immersed.is_none() && q.immersed.is_none());
}
