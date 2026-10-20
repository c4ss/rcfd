//! ADR 0084 (wave 37A): linearisation of the steady SIMPLEC iteration on uniformly sheared meshes. Ignored diagnostics; their output is
//! recorded in the ADR. The state of one outer iteration is x = (active cell velocity components, cell pressure, face flux of every non-empty
//! face); `SimpleSolver::iterate` is the map F (everything else it reads is fixed for these set-ups: no walls with body forces, no face jumps,
//! no driver, no pseudo time). The Jacobian dF/dx is formed column by column with central differences (step `h` times the scale of the
//! variable type) and written as a dense matrix for `scripts/w37a_jacobian_analysis.py` (eigenvalues, deflation of the neutral mean-velocity
//! modes, Fourier labels, transient growth).

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver, SteadyGuard};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, channel, warp, Grading};
use std::io::Write;
use std::sync::Arc;

/// Settings of one map.
#[derive(Clone, Copy, Debug)]
struct Var {
coupling: Coupling,
alpha_u: f64,
alpha_p: f64,
correctors: usize,
compensation: bool,
    /// Linear solves to 1e-13 relative (the linearised map); `false` keeps the default 1e-3.
tight: bool,
central: bool,
}

const BASE: Var = Var {
    coupling: Coupling::Simplec,
    alpha_u: 0.9,
    alpha_p: 1.0,
    correctors: 0,
    compensation: false,
    tight: true,
    central: false,
};

fn settings(nu: f64, v: Var) -> SimpleSettings {
    let mut st = SimpleSettings::new(nu, v.coupling);
    st.convection = if v.central { ConvScheme::Central } else { ConvScheme::Upwind };
    st.alpha_u = v.alpha_u;
    st.alpha_p = v.alpha_p;
    st.non_orth_correctors = v.correctors;
    st.simplec_non_orth_compensation = v.compensation;
    if v.tight {
        st.momentum_solver = SolveControl {
            tol_abs: 1e-16,
            tol_rel: 1e-13,
            max_iter: 10000,
        };
        st.pressure_solver = SolveControl {
            tol_abs: 1e-16,
            tol_rel: 1e-13,
            max_iter: 10000,
        };
    }
    st
}

fn shear(m: rcfd_mesh::Mesh, deg: f64) -> Arc<FvGeometry> {
    let t = deg.to_radians().tan();
    let m = warp(&m, |p| [p[0] + t * p[1], p[1], p[2]]).unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// Doubly periodic Kolmogorov box `lx` x 1 on `nx` x `ny` cells (ADR 0081 V7 for 40 x 10, 4 x 1), body force nu k^2 sin(k y), k = 2 pi.
fn kolmogorov(nx: usize, ny: usize, lx: f64, deg: f64, nu: f64, v: Var) -> SimpleSolver {
    let geom = shear(box_2d(nx, ny, lx, 1.0, true, true).unwrap(), deg);
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSolver::new(geom, u, p, settings(nu, v));
    let k = 2.0 * std::f64::consts::PI;
    let cells: Vec<[f64; 3]> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| [nu * k * k * (k * c[1]).sin(), 0.0, 0.0])
        .collect();
    s.body_force = Some(BodyForce::from_cells(&s.geom, cells));
    s
}

/// ADR 0081 set-up B (the ADR 0077 S3 channel) at viscosity `nu`.
fn channel_b(deg: f64, nu: f64, v: Var) -> SimpleSolver {
    let geom = shear(
        channel([40, 10, 1], [4.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap(),
        deg,
    );
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    SimpleSolver::new(geom, u, p, settings(nu, v))
}

/// Packing of the state vector.
struct Layout {
comps: Vec<usize>,
n_cells: usize,
faces: Vec<usize>,
}

impl Layout {
fn of(s: &SimpleSolver) -> Self {
        let g = &s.geom;
        Layout {
            comps: (0..3).filter(|&k| g.active[k]).collect(),
            n_cells: g.n_cells(),
            faces: (0..g.mesh.n_faces()).filter(|&f| !g.empty[f]).collect(),
        }
    }
fn len(&self) -> usize {
        (self.comps.len() + 1) * self.n_cells + self.faces.len()
    }
fn get(&self, s: &SimpleSolver) -> Vec<f64> {
        let mut x = Vec::with_capacity(self.len());
        for &k in &self.comps {
            x.extend(s.u.values.iter().map(|u| u[k]));
        }
        x.extend_from_slice(&s.p.values);
        x.extend(self.faces.iter().map(|&f| s.phi[f]));
        x
    }
fn set(&self, s: &mut SimpleSolver, x: &[f64]) {
        let n = self.n_cells;
        for (b, &k) in self.comps.iter().enumerate() {
            for c in 0..n {
                s.u.values[c][k] = x[b * n + c];
            }
        }
        let o = self.comps.len() * n;
        s.p.values.copy_from_slice(&x[o..o + n]);
        for (i, &f) in self.faces.iter().enumerate() {
            s.phi[f] = x[o + n + i];
        }
    }
    /// Block of entry `i`: 0.. velocity components, then pressure, then flux.
fn block(&self, i: usize) -> usize {
        let nb = self.comps.len();
        (i / self.n_cells).min(nb + 1)
    }
}

/// F(x): one outer iteration from state x.
fn map(s: &mut SimpleSolver, l: &Layout, x: &[f64]) -> Vec<f64> {
    l.set(s, x);
    s.iterate();
    s.history.clear();
    l.get(s)
}

/// Scales of the variable types (velocity, pressure, flux) at state x.
fn scales(l: &Layout, x: &[f64], s: &SimpleSolver) -> Vec<f64> {
    let n = l.n_cells;
    let nb = l.comps.len();
    let umax = x[..nb * n].iter().fold(0.0f64, |a, v| a.max(v.abs())).max(1.0);
    let pmax = x[nb * n..(nb + 1) * n]
        .iter()
        .fold(0.0f64, |a, v| a.max(v.abs()))
        .max(umax * umax);
    let sf = s.geom.mesh.face_area_vectors();
    let amax = l.faces.iter().map(|&f| vec3::norm(sf[f])).fold(0.0, f64::max);
    vec![umax, pmax, umax * amax]
}

/// Central-difference Jacobian, row-major `n x n`, with step `h * scale(type of column)`.
fn jacobian(s: &mut SimpleSolver, l: &Layout, x: &[f64], h: f64) -> Vec<f64> {
    let n = l.len();
    let sc = scales(l, x, s);
    let nb = l.comps.len();
    let mut j = vec![0.0; n * n];
    let mut xp = x.to_vec();
    for col in 0..n {
        let b = l.block(col);
        let step = h * if b < nb { sc[0] } else if b == nb { sc[1] } else { sc[2] };
        xp[col] = x[col] + step;
        let fp = map(s, l, &xp);
        xp[col] = x[col] - step;
        let fm = map(s, l, &xp);
        xp[col] = x[col];
        for row in 0..n {
            j[row * n + col] = (fp[row] - fm[row]) / (2.0 * step);
        }
    }
    j
}

/// Output directory of the dumps.
fn out_dir() -> std::path::PathBuf {
    let d = std::env::var("W37A_OUT").unwrap_or_else(|_| "<scratch dir>/jac".into());
    let p = std::path::PathBuf::from(d);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Write J and the layout metadata (cell (i, j) labels for the Fourier analysis, scales, the seam faces).
fn dump(tag: &str, s: &SimpleSolver, l: &Layout, x: &[f64], j: &[f64], nx: usize, ny: usize, lx: f64, deg: f64) {
    let dir = out_dir();
    let mut f = std::fs::File::create(dir.join(format!("{tag}.bin"))).unwrap();
    let mut buf = Vec::with_capacity(j.len() * 8);
    for v in j {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    f.write_all(&buf).unwrap();
    let t = deg.to_radians().tan();
    let (dx, dy) = (lx / nx as f64, 1.0 / ny as f64);
    let mut m = format!(
        "n {}\nn_cells {}\ncomps {}\nn_faces {}\nnx {nx}\nny {ny}\n",
        l.len(),
        l.n_cells,
        l.comps.len(),
        l.faces.len()
    );
    let sc = scales(l, x, s);
    m.push_str(&format!("scales {} {} {}\n", sc[0], sc[1], sc[2]));
    for c in s.geom.mesh.cell_centres() {
        let jj = ((c[1] / dy).floor() as i64).clamp(0, ny as i64 - 1);
        let ii = (((c[0] - t * c[1]) / dx).floor() as i64).rem_euclid(nx as i64);
        m.push_str(&format!("cell {ii} {jj}\n"));
    }
    let n_int = s.geom.n_internal;
    for &fc in &l.faces {
        m.push_str(&format!("face {}\n", if fc >= n_int { 1 } else { 0 }));
    }
    std::fs::write(dir.join(format!("{tag}.meta")), m).unwrap();
    std::fs::write(
        dir.join(format!("{tag}.x")),
        x.iter().map(|v| format!("{v:e}\n")).collect::<String>(),
    )
    .unwrap();
}

/// Iterate to res_u, res_p <= tol (cap), returning the iterations used (None at the cap).
fn converge(s: &mut SimpleSolver, tol: f64, cap: usize) -> Option<usize> {
    for it in 1..=cap {
        let i = s.iterate();
        if i.res_u <= tol && i.res_p <= tol {
            return Some(it);
        }
    }
    None
}

/// Largest cell speed.
fn max_speed(s: &SimpleSolver) -> f64 {
    s.u.values
        .iter()
        .map(|u| vec3::norm(*u))
        .fold(0.0f64, |a, x| if x.is_nan() { f64::INFINITY } else { a.max(x) })
}

/// The ADR 0081 run classification over 1000 iterations: (stable, summary).
fn classify(mut s: SimpleSolver, u_ref: f64) -> (bool, String) {
    let mut g = SteadyGuard::new(true);
    let (mut umax, mut c10, mut c_late, mut finite) = (0.0f64, f64::NAN, 0.0f64, true);
    let mut conv = None;
    for it in 1..=1000 {
        let info = s.iterate();
        if let Some(d) = g.check(&s.geom.clone(), &info, &s.u, &s.p.values, &s.phi) {
            return (false, format!("trip {} at {it}", d.kind));
        }
        if it >= 10 {
            umax = umax.max(max_speed(&s));
        }
        if !info.continuity.is_finite() {
            finite = false;
        }
        if it == 10 {
            c10 = info.continuity;
        }
        if it >= 900 {
            c_late = c_late.max(info.continuity);
        }
        if conv.is_none() && info.res_u < 1e-6 && info.res_p < 1e-6 {
            conv = Some(it);
        }
    }
    let stable = umax <= 3.0 * u_ref && finite && c_late <= c10;
    (
        stable,
        format!("umax {umax:.2e}, c10 {c10:.1e}, c900+ {c_late:.1e}, res 1e-6 at {conv:?}"),
    )
}

/// Harness smoke (rule (d)): the map is stateless (two evaluations bit-identical) and the Jacobian of a 6 x 3 box builds.
#[test]
#[ignore = "harness smoke of ADR 0084"]
fn harness_smoke() {
    let mut s = kolmogorov(6, 3, 2.0, 60.0, 0.5, BASE);
    let l = Layout::of(&s);
    for _ in 0..5 {
        s.iterate();
    }
    let x = l.get(&s);
    let a = map(&mut s, &l, &x);
    let b = map(&mut s, &l, &x);
    assert!(a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits()), "map not stateless");
    let j = jacobian(&mut s, &l, &x, 1e-6);
    dump("smoke", &s, &l, &x, &j, 6, 3, 2.0, 60.0);
    println!("smoke: n = {}, |J|_max = {:e}", l.len(), j.iter().fold(0.0f64, |a, v| a.max(v.abs())));
}

/// Set-up smoke (rule (d)): converged states at 0.9/0.3 (tight), the base-map residual of one iteration from them, and the tight channel scan.
#[test]
#[ignore = "set-up smoke of ADR 0084"]
fn state_smoke() {
    let v03 = Var { alpha_p: 0.3, ..BASE };
    for deg in [55.0, 60.0, 70.0] {
        let mut s = kolmogorov(40, 10, 4.0, deg, 0.5, v03);
        let it = converge(&mut s, 1e-12, 20000);
        let l = Layout::of(&s);
        let x = l.get(&s);
        s.settings = settings(0.5, BASE);
        let y = map(&mut s, &l, &x);
        let d = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        println!("V7 {deg}: 0.9/0.3 res 1e-12 at {it:?}; base map moves x* by {d:.2e}; umax {:.4}", max_speed(&s));
        let mut s = channel_b(deg, 0.01, v03);
        let it = converge(&mut s, 1e-12, 20000);
        let l = Layout::of(&s);
        let x = l.get(&s);
        s.settings = settings(0.01, BASE);
        let y = map(&mut s, &l, &x);
        let d = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        println!("B {deg}: 0.9/0.3 res 1e-12 at {it:?}; base map moves x* by {d:.2e}; n = {}", l.len());
    }
    for deg in [45.0, 50.0, 52.5, 55.0, 57.5, 60.0] {
        let (st, why) = classify(channel_b(deg, 0.01, BASE), 1.0);
        let (st2, why2) = classify(channel_b(deg, 0.01, Var { tight: false, ..BASE }), 1.0);
        println!("B tight {deg}: {st} ({why}); default tolerances: {st2} ({why2})");
    }
}

/// A registered set-up: K40 / K20 / K10 / KS / KC (Kolmogorov boxes) or B (channel B).
#[derive(Clone, Copy, Debug)]
enum Setup {
    K { nx: usize, ny: usize, lx: f64, nu: f64 },
    B,
}

const K40: Setup = Setup::K { nx: 40, ny: 10, lx: 4.0, nu: 0.5 };

impl Setup {
fn build(self, deg: f64, v: Var) -> SimpleSolver {
        match self {
            Setup::K { nx, ny, lx, nu } => kolmogorov(nx, ny, lx, deg, nu, v),
            Setup::B => channel_b(deg, 0.01, v),
        }
    }
fn dims(self) -> (usize, usize, f64) {
        match self {
            Setup::K { nx, ny, lx, .. } => (nx, ny, lx),
            Setup::B => (40, 10, 4.0),
        }
    }
fn nu(self) -> f64 {
        match self {
            Setup::K { nu, .. } => nu,
            Setup::B => 0.01,
        }
    }
}

/// x*: the 0.9/0.3 tight iteration (res 1e-12 or 20 000 iterations), then switched to the map `v`; accepted iff one map iteration moves it by
/// at most 1e-12 (registered). Returns the solver positioned at x* with settings `v`.
fn fixed_point(set: Setup, deg: f64, v: Var) -> SimpleSolver {
    let mut s = set.build(deg, Var { alpha_p: 0.3, ..v });
    let it = converge(&mut s, 1e-12, 20000);
    s.settings = settings(set.nu(), v);
    let l = Layout::of(&s);
    let x = l.get(&s);
    let y = map(&mut s, &l, &x);
    let d = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    l.set(&mut s, &x);
    println!("x* {set:?} {deg} deg: res 1e-12 at {it:?}; one map iteration moves it by {d:.2e} ({})", if d <= 1e-12 { "accepted" } else { "NOT A FIXED POINT" });
    s
}

/// Form, check statelessness (M2) and dump one Jacobian.
fn jac_run(tag: &str, set: Setup, deg: f64, s: &mut SimpleSolver, h: f64) {
    let l = Layout::of(s);
    let x = l.get(s);
    let a = map(s, &l, &x);
    let b = map(s, &l, &x);
    let stateless = a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits());
    let t0 = std::time::Instant::now();
    let j = jacobian(s, &l, &x, h);
    l.set(s, &x);
    let (nx, ny, lx) = set.dims();
    dump(tag, s, &l, &x, &j, nx, ny, lx, deg);
    println!("J {tag}: n = {}, M2 stateless {stateless}, {:.1} s", l.len(), t0.elapsed().as_secs_f64());
}

/// M3: growth of a 1e-8 perturbation of x* under the map (geometric mean over the iterations where max|x_k - x*| lies in [1e-7, 1e-3]).
fn m3(tag: &str, s: &mut SimpleSolver) {
    let l = Layout::of(s);
    let xs = l.get(s);
    let n = l.n_cells;
    let ux = l.comps.iter().position(|&k| k == 0).unwrap();
    let w: Vec<f64> = (0..n).map(|c| xs[ux * n + c] * (c as f64).sin()).collect();
    let mean = w.iter().sum::<f64>() / n as f64;
    let mut x = xs.clone();
    for c in 0..n {
        x[ux * n + c] += 1e-8 * (w[c] - mean);
    }
    let mut win: Vec<(usize, f64)> = Vec::new();
    for k in 1..=3000 {
        x = map(s, &l, &x);
        let d = x.iter().zip(&xs).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        if (1e-7..=1e-3).contains(&d) {
            win.push((k, d));
        }
        if !d.is_finite() || d > 1e-2 {
            break;
        }
    }
    l.set(s, &xs);
    if win.len() >= 2 {
        let (k0, d0) = win[0];
        let (k1, d1) = win[win.len() - 1];
        let g = (d1 / d0).powf(1.0 / (k1 - k0).max(1) as f64);
        println!("M3 {tag}: growth {g:.5} over iterations {k0}..{k1} ({d0:.2e} -> {d1:.2e})");
    } else {
        println!("M3 {tag}: the perturbation never entered [1e-7, 1e-3] within 3000 iterations");
    }
}

fn only(tag: &str) -> bool {
    std::env::var("W37A_ONLY").map_or(true, |o| o.split(',').any(|p| tag.starts_with(p)))
}

/// The registered Jacobians J1-J10 and the method checks M1-M3 of ADR 0084 (each formed once). Select with W37A_ONLY=J1,J8 (prefixes).
#[test]
#[ignore = "registered linearisation runs of ADR 0084 (dumps for scripts/w37a_jacobian_analysis.py)"]
fn registered_jacobians() {
    let h = 1e-6;
    let a = |alpha_p| Var { alpha_p, ..BASE };
    if only("J1") {
        for deg in [50.0, 55.0, 57.5, 60.0, 65.0, 70.0] {
            let mut s = fixed_point(K40, deg, BASE);
            jac_run(&format!("J1_K40_{deg}"), K40, deg, &mut s, h);
            if deg == 60.0 && only("J1M") {
                jac_run("M1_K40_60_h1e-7", K40, deg, &mut s, 1e-7);
            }
            if deg >= 60.0 {
                m3(&format!("J1_K40_{deg}"), &mut s);
            }
        }
    }
    if only("J2") {
        for deg in [60.0, 70.0] {
            let mut s = K40.build(deg, BASE);
            jac_run(&format!("J2_K40_{deg}_x0"), K40, deg, &mut s, h);
        }
    }
    if only("J3") {
        for deg in [60.0, 65.0, 70.0] {
            let mut s = fixed_point(K40, deg, a(0.7));
            jac_run(&format!("J3_K40_{deg}_ap0.7"), K40, deg, &mut s, h);
            if deg == 70.0 {
                m3("J3_K40_70_ap0.7", &mut s);
            }
        }
    }
    if only("J4") {
        for deg in [60.0, 70.0] {
            let mut s = fixed_point(K40, deg, a(0.5));
            jac_run(&format!("J4_K40_{deg}_ap0.5"), K40, deg, &mut s, h);
        }
    }
    if only("J5") {
        for (name, set) in [
            ("K20", Setup::K { nx: 20, ny: 10, lx: 2.0, nu: 0.5 }),
            ("K10", Setup::K { nx: 10, ny: 10, lx: 1.0, nu: 0.5 }),
        ] {
            let mut s = fixed_point(set, 60.0, BASE);
            jac_run(&format!("J5_{name}_60"), set, 60.0, &mut s, h);
        }
    }
    if only("J6") {
        let ks = Setup::K { nx: 40, ny: 10, lx: 4.0, nu: 50.0 };
        let mut s = fixed_point(ks, 60.0, BASE);
        jac_run("J6_KS_60", ks, 60.0, &mut s, h);
        let mut s = ks.build(60.0, BASE);
        jac_run("J6_KS_60_x0", ks, 60.0, &mut s, h);
    }
    if only("J7") {
        let v = Var { central: true, ..BASE };
        let mut s = fixed_point(K40, 60.0, v);
        jac_run("J7_KC_60", K40, 60.0, &mut s, h);
    }
    if only("J8") {
        for deg in [50.0, 55.0, 60.0] {
            let mut s = fixed_point(Setup::B, deg, BASE);
            jac_run(&format!("J8_B_{deg}"), Setup::B, deg, &mut s, h);
            if deg == 55.0 && only("J8M") {
                jac_run("M1_B_55_h1e-7", Setup::B, deg, &mut s, 1e-7);
            }
        }
    }
    if only("J9") {
        let mut s = Setup::B.build(55.0, BASE);
        let mut done = 0;
        for k in [0usize, 1, 5, 10, 20] {
            while done < k {
                s.iterate();
                done += 1;
            }
            println!("J9 x{k}: max speed {:.3}", max_speed(&s));
            jac_run(&format!("J9_B_55_x{k}"), Setup::B, 55.0, &mut s, h);
        }
    }
    if only("J10") {
        for deg in [0.0, 30.0] {
            let mut s = fixed_point(K40, deg, BASE);
            jac_run(&format!("J10_K40_{deg}"), K40, deg, &mut s, h);
        }
    }
}
