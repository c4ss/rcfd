//! ADR 0346: the default flip of the two wall-pressure keys (ADR 0269 Q1 and ADR 0342 Q1, approved by the maintainer in
//! person on 2026-11-12). Without the key the body force of the six-DOF driver (`[dynamic_mesh.six_dof.coupling]
//! wall_pressure`) and the force reports on moving walls (`[dynamic_mesh.force_reports] wall_pressure`) take
//! `prescribed_gradient`; the explicit old value gives the old default's bits; the numbers move as ADR 0269 and ADR 0342
//! listed; where the new value is refused the old behaviour is kept silently.
//!
//! The limits, set-ups and mutations were registered in ADR 0346 before this file existed. The texts are those of
//! `strong_coupling_case_f350.rs`, `wall_pressure_case_f358.rs` and `force_reports_moving_wall_f440.rs` (copied).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use std::f64::consts::TAU;

/// Added mass of the column, `rho A L0`.
const M_A: f64 = 1.0 * 0.04 * 1.0;
/// Amplitude (initial displacement) of the piston.
const X: f64 = 0.01;
const BODY_KEY: &str = "dynamic_mesh.six_dof.coupling.wall_pressure";
const REPORT_KEY: &str = "dynamic_mesh.force_reports.wall_pressure";
const BODY_OLD: &str = "[dynamic_mesh.six_dof.coupling]\nwall_pressure = \"extrapolated\"\n";
const BODY_NEW: &str = "[dynamic_mesh.six_dof.coupling]\nwall_pressure = \"prescribed_gradient\"\n";
const REPORT_OLD: &str = "[dynamic_mesh.force_reports]\nwall_pressure = \"boundary_value\"\n";
const REPORT_NEW: &str = "[dynamic_mesh.force_reports]\nwall_pressure = \"prescribed_gradient\"\n";

/// X0, measured once on the unchanged tree b511eb75 (old defaults) from the texts without the key (ADR 0346 X0):
/// the digests of X1 a to e.
const OLD_DIGESTS: [u64; 5] = [
    0xbe15_ac65_4a67_efb0,
    0xe08d_76bb_b57f_d846,
    0x0bb5_aeca_4ff4_60d8,
    0x3f8d_213a_7c31_7800,
    0x1adc_11ed_ae8a_fc94,
];
/// X0: the settings hashes of the piston (`mu = 0.2`, `T/100`), `t4` and the resting column without tables.
const OLD_HASHES: [u64; 3] = [0x8013_3491_493e_8dde, 0xc3f1_f3f1_fbed_789b, 0xd9b0_6e96_1713_1355];

fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

/// ADR 0266's piston text: `box_3d` of `lx = L0 - X` with the body at `X`, so the column is `L0 - x`.
fn piston(mu: f64, dt: f64, table: &str) -> String {
    let (m, k) = (mass(mu), stiffness(mu));
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = 20
ny = 2
nz = 2
lx = 0.99
ly = 0.2
lz = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[boundaries.zmin]
velocity = "zero_gradient"
[boundaries.zmax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = {dt:?}
end_time = 11.5
momentum_rel_tol = 1e-13
momentum_max_iter = 5000
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
pressure_max_iter = 5000
[dynamic_mesh]
enabled = true
motion = "six_dof"
moving_patches = ["xmin"]
fixed_patches = ["xmax"]
deformation = "smoothing"
[dynamic_mesh.six_dof]
mass = {m:?}
inertia = [1.0, 1.0, 1.0]
position = [{X:?}, 0.0, 0.0]
translation_locked = [false, true, true]
stiffness = [{k:?}, 0.0, 0.0]
{table}
"#
    )
}

/// ADR 0250's registered strong settings as keys (ADR 0266), with `extra` lines in the table.
fn strong_table(mu: f64, extra: &str) -> String {
    format!(
        "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nrelaxation = \"aitken\"\nadded_mass = {M_A:?}\ntolerance = 1e-8\nforce_scale = {:?}\nmax_iterations = 20\n{extra}",
        stiffness(mu) * X
    )
}

/// `rcfd-api/tests/dynamic_mesh.rs` T4 (copied by ADR 0269): the constant-force piston of mass 0.2.
fn t4(table: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
gravity = [5.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 2.0

[dynamic_mesh]
enabled = true
motion = "six_dof"
moving_patches = ["xmin"]
fixed_patches = ["xmax"]
deformation = "smoothing"
[dynamic_mesh.six_dof]
mass = 0.2
inertia = [1.0, 1.0, 1.0]
translation_locked = [false, true, true]
gravity = true
{table}
"#
    )
}

/// ADR 0269 AU1b's rigid column with `xmin` at rest or following and the five reports of ADR 0342.
fn column(wall: &str, table: &str) -> String {
    let patches = if wall == "rest" {
        "rest_patches = [\"xmin\"]"
    } else {
        "moving_patches = [\"xmin\"]"
    };
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = 20
ny = 2
nz = 2
lx = 1.0
ly = 0.2
lz = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[boundaries.zmin]
velocity = "zero_gradient"
[boundaries.zmax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.01
end_time = 1.0
momentum_rel_tol = 1e-13
momentum_max_iter = 5000
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
pressure_max_iter = 5000
[reports.fp]
type = "force"
patch = "xmin"
component = "pressure"
direction = [1.0, 0.0, 0.0]
[reports.ft]
type = "force"
patch = "xmin"
component = "total"
direction = [1.0, 0.0, 0.0]
[reports.my]
type = "moment"
patch = "xmin"
direction = [0.0, 1.0, 0.0]
origin = [0.0, 0.0, 0.0]
[reports.cf]
type = "force_coefficient"
patch = "xmin"
component = "pressure"
direction = [1.0, 0.0, 0.0]
u_ref = 2.0
a_ref = 0.04
[reports.fo]
type = "force"
patch = "xmax"
component = "total"
direction = [1.0, 0.0, 0.0]
[dynamic_mesh]
enabled = true
deformation = "rigid"
{patches}
displacement_expression_x = "0.01 [m] * (cos(2 * pi * t / 1 [s]) - 1)"
{table}
"#
    )
}

fn parse(text: &str) -> Case {
    let c = Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}"));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    c
}

/// Diagnostic paths of a text: parse errors, then the static checks.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => c.semantic_errors().iter().map(|x| x.path.clone()).collect(),
    }
}

/// FNV-1a 64 over the bits of a sequence of numbers.
#[derive(Clone, Copy)]
struct Digest(u64);

impl Digest {
    fn new() -> Self {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn f(&mut self, x: f64) {
        self.word(x.to_bits());
    }
}

fn add_flow(d: &mut Digest, s: &Simulation) {
    let f = s.pimple_solver().unwrap();
    for v in f.u.values.iter().flatten() {
        d.f(*v);
    }
    for v in &f.p.values {
        d.f(*v);
    }
    for v in &f.phi {
        d.f(*v);
    }
}

/// The report values sorted by name.
fn reports(s: &Simulation) -> Vec<f64> {
    let mut v: Vec<(String, f64)> = s
        .report_values()
        .into_iter()
        .map(|(n, r)| (n, r.unwrap_or_else(|e| panic!("{e}")).value))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().map(|x| x.1).collect()
}

/// A six-DOF run: the digest of `u`, `p`, `phi`, `x`, `v` after every step, then the iteration counts.
fn body_digest(text: &str, steps: usize) -> u64 {
    let mut s = Simulation::new(&parse(text)).unwrap();
    let mut d = Digest::new();
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        add_flow(&mut d, &s);
        let b = s.dynamic_body().unwrap();
        for k in 0..3 {
            d.f(b.state.x[k]);
            d.f(b.state.v[k]);
        }
    }
    for r in s.coupling_records() {
        d.word(r.iterations as u64);
    }
    d.0
}

/// A column run: the digest of the five reports before the first and after every step, and of the flow.
fn column_digest(text: &str, steps: usize) -> u64 {
    let mut s = Simulation::new(&parse(text)).unwrap();
    let mut d = Digest::new();
    for v in reports(&s) {
        d.f(v);
    }
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        for v in reports(&s) {
            d.f(v);
        }
        add_flow(&mut d, &s);
    }
    d.0
}

/// X1 a to e with the given tables: (body table for a and c, strong extra line for b, report table for d and e).
fn x_digests(body: &str, strong_extra: &str, report: &str) -> [u64; 5] {
    [
        body_digest(&piston(0.2, 0.01, body), 100),
        body_digest(&piston(4.0, 0.01, &strong_table(4.0, strong_extra)), 40),
        body_digest(&t4(body), 92),
        column_digest(&column("rest", report), 12),
        column_digest(&column("follow", report), 12),
    ]
}

fn texts_without_tables() -> [String; 3] {
    [piston(0.2, 0.01, ""), t4(""), column("rest", "")]
}

/// ADR 0346 X0: run once on the unchanged tree (the old defaults) to record [`OLD_DIGESTS`] and [`OLD_HASHES`].
#[test]
#[ignore = "record X0 of ADR 0346: run once on the unchanged tree b511eb75 before any code"]
fn x0_record_of_the_old_default() {
    let none = x_digests("", "", "");
    let old = x_digests(BODY_OLD, "wall_pressure = \"extrapolated\"\n", REPORT_OLD);
    let hashes: Vec<u64> = texts_without_tables()
        .iter()
        .map(|t| parse(t).tree.content_hash())
        .collect();
    println!("X0 without the key: {:#018x?}", none);
    println!("X0 explicit old value: {:#018x?}", old);
    println!("X0 settings hashes: {:#018x?}", hashes);
    assert_eq!(none, old);
}

/// ADR 0346 X1: the explicit old value gives the old default's bits.
#[test]
fn x1_explicit_old_value_equals_the_old_default() {
    let old = x_digests(BODY_OLD, "wall_pressure = \"extrapolated\"\n", REPORT_OLD);
    println!("X1: {:#018x?} against X0 {:#018x?}", old, OLD_DIGESTS);
    assert_eq!(old, OLD_DIGESTS);
}

/// ADR 0346 Y: without the key the run equals the explicit new value bit for bit; the control X1 a differs from Y a.
#[test]
fn y_no_key_equals_the_explicit_new_value() {
    let none = [
        body_digest(&piston(0.2, 0.01, ""), 100),
        body_digest(&piston(4.0, 0.01, &strong_table(4.0, "")), 40),
        body_digest(&t4(""), 92),
        column_digest(&column("rest", ""), 12),
    ];
    let new = [
        body_digest(&piston(0.2, 0.01, BODY_NEW), 100),
        body_digest(
            &piston(
                4.0,
                0.01,
                &strong_table(4.0, "wall_pressure = \"prescribed_gradient\"\n"),
            ),
            40,
        ),
        body_digest(&t4(BODY_NEW), 92),
        column_digest(&column("rest", REPORT_NEW), 12),
    ];
    println!("Y without the key {:#018x?}; explicit new value {:#018x?}", none, new);
    println!("Y control: X1 a {:#018x} against Y a {:#018x}", OLD_DIGESTS[0], none[0]);
    assert_eq!(none, new);
    assert_ne!(none[0], OLD_DIGESTS[0]);
}

/// ADR 0346 D0: the texts without tables are unchanged, their hashes are the old ones, the keys read the new default; a
/// written strong table without the key now carries the new value (disclosed, ADR 0346 D1).
#[test]
fn d0_texts_and_defaults() {
    let hashes: Vec<u64> = texts_without_tables()
        .iter()
        .map(|t| {
            let c = parse(t);
            let (tt, jt) = (c.to_toml(), c.to_json_string());
            for w in ["coupling", "wall_pressure", "force_reports"] {
                assert!(!tt.contains(w) && !jt.contains(w), "{w}");
            }
            assert_eq!(c.get(BODY_KEY).unwrap(), "prescribed_gradient");
            assert_eq!(c.get(REPORT_KEY).unwrap(), "prescribed_gradient");
            c.tree.content_hash()
        })
        .collect();
    println!("D0: settings hashes {:#018x?} against X0 {:#018x?}", hashes, OLD_HASHES);
    assert_eq!(hashes, OLD_HASHES);
    let strong = parse(&piston(4.0, 0.01, &strong_table(4.0, "")));
    assert!(strong.to_toml().contains("wall_pressure = \"prescribed_gradient\""));
}

// ---- P1 to P6 ----

fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

/// `omega_m/omega` (the period is 1) of a run of steps of `dt`, the body starting at `X`; and the crossings.
fn omega_ratio(dt: f64, x: &[f64]) -> (f64, usize) {
    let mut xs = vec![X];
    xs.extend_from_slice(x);
    let t: Vec<f64> = (0..xs.len()).map(|n| n as f64 * dt).collect();
    let c = upward_crossings(&t, &xs);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]) / TAU, c.len())
}

fn worst_growth(x: &[f64]) -> f64 {
    let mut maxima = vec![x[0]];
    let mut minima = Vec::new();
    for i in 1..x.len() - 1 {
        let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
        let curv = a - 2.0 * b + c;
        if b > a && b >= c && curv != 0.0 {
            maxima.push(b - (c - a).powi(2) / (8.0 * curv));
        } else if b < a && b <= c && curv != 0.0 {
            minima.push(b - (c - a).powi(2) / (8.0 * curv));
        }
    }
    let mut worst: f64 = 0.0;
    for w in maxima.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in minima.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    worst
}

/// Body positions after each of `steps` steps, all converged when strong.
fn positions(text: &str, steps: usize) -> (Vec<f64>, bool) {
    let mut s = Simulation::new(&parse(text)).unwrap();
    let mut x = Vec::new();
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        x.push(s.dynamic_body().unwrap().state.x[0]);
    }
    (x, s.coupling_records().iter().all(|r| r.converged))
}

/// ADR 0346 P1: SP of ADR 0211 (mass ratio 0.2) without the table now gives ADR 0269 A1's number.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p1_spring_mounted_piston_without_the_table() {
    let (x, _) = positions(&piston(0.2, 0.01, ""), 650);
    let (w, n) = omega_ratio(0.01, &x);
    println!("P1: omega_m/omega {w:.7} ({n} crossings); limit |. - 1.000264| <= 1e-6, old 1.001307");
    assert!((w - 1.000264).abs() <= 1e-6, "{w}");
    assert!((w - 1.001307).abs() > 5e-4, "{w}");
}

/// ADR 0346 P2: ADR 0266's strong piston (mass ratio 4) without the key now gives ADR 0269 A4's numbers.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p2_strong_piston_without_the_key() {
    let (x, conv) = positions(&piston(4.0, 0.01, &strong_table(4.0, "")), 1150);
    let (w, n) = omega_ratio(0.01, &x);
    let mut xs = vec![X];
    xs.extend_from_slice(&x);
    let g = worst_growth(&xs);
    println!(
        "P2: omega_m/omega {w:.7} ({n} crossings), largest turning ratio {g:.6}, all converged {conv}; limits 1e-6 of 1.000042, 1e-5 of 0.92469; old 1.005081, 0.92452"
    );
    assert!((w - 1.000042).abs() <= 1e-6, "{w}");
    assert!((g - 0.92469).abs() <= 1e-5, "{g}");
    assert!(conv);
}

/// ADR 0346 P3: T4 of `dynamic_mesh.rs` without the table, ADR 0269 L's numbers.
#[test]
fn p3_t4_without_the_table() {
    let mut s = Simulation::new(&parse(&t4(""))).unwrap();
    let m = &s.geom.mesh;
    let area: f64 = m
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| vec3::norm(m.face_area_vectors()[f]))
        .sum();
    let mut steps = 0;
    while s.dynamic_body().unwrap().state.x[0] < 0.5 && steps < 400 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        steps += 1;
    }
    let b = s.dynamic_body().unwrap();
    let (x, v) = (b.state.x[0], b.state.v[0]);
    let m0 = 0.2 + area;
    let v2 = -(2.0 * 1.0 / area) * (1.0 - area * x / m0).ln();
    let ratio = v * v / v2;
    println!("P3: {steps} steps, x = {x:.7}, v = {v:.7}, v^2/v_exact^2 = {ratio:.7}; ADR 0269 L 92, 0.506503, 2.209188, 0.999498");
    assert_eq!(steps, 92);
    assert!((x - 0.506503).abs() <= 1e-6, "{x}");
    assert!((v - 2.209188).abs() <= 1e-6, "{v}");
    assert!((ratio - 0.999498).abs() <= 1e-6, "{ratio}");
}

/// ADR 0346 P4: the loose piston at mass ratio 4 without the table (ADR 0266 L): `10 X` at step 28, stop at 35.
#[test]
fn p4_loose_piston_at_mass_ratio_4_without_the_table() {
    let mut s = Simulation::new(&parse(&piston(4.0, 0.01, ""))).unwrap();
    let (mut big, mut stop) = (None, None);
    for n in 1..=1150 {
        s.step();
        if s.failure().is_some() || s.stop_reason().is_some() {
            stop = Some(n);
            break;
        }
        if big.is_none() && s.dynamic_body().unwrap().state.x[0].abs() >= 10.0 * X {
            big = Some(n);
        }
    }
    println!("P4: 10 X at step {big:?}, stopped at step {stop:?}; ADR 0269 L 28, 35 (old 28, 34)");
    assert_eq!(big, Some(28));
    assert_eq!(stop, Some(35));
}

/// ADR 0346 P5: ADR 0266 Z's case (mass ratio 0.2, 100 loose steps) without the table: `x` after 100 steps.
#[test]
fn p5_piston_at_mass_ratio_0_2_after_100_steps() {
    let (x, _) = positions(&piston(0.2, 0.01, ""), 100);
    println!("P5: x = {:.9e}; ADR 0269 L 9.524133e-3 (old 9.527268e-3)", x[99]);
    assert!((x[99] - 9.524133e-3).abs() <= 1e-9, "{}", x[99]);
}

/// ADR 0346 P6: the resting column of ADR 0342 without the table: worst deficits of `fp` and `ft` over steps 3 to 12.
#[test]
fn p6_force_reports_without_the_table() {
    let mut s = Simulation::new(&parse(&column("rest", ""))).unwrap();
    let (mut wp, mut wt): (f64, f64) = (0.0, 0.0);
    for n in 1..=12 {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        if n < 3 {
            continue;
        }
        let g = &s.geom;
        let w = s.dynamic_wall_gradients();
        let (mut area, mut gsum, mut k) = (0.0, 0.0, 0.0);
        for f in g.mesh.patch("xmin").unwrap().faces() {
            area += vec3::norm(g.mesh.face_area_vectors()[f]);
            gsum += w[f - g.n_internal];
            k += 1.0;
        }
        let fx = -(gsum / k) * area;
        let r = reports(&s);
        let (dp, dt) = (1.0 - r[2] / fx, 1.0 - r[3] / fx);
        if dp.abs() > wp.abs() {
            wp = dp;
        }
        if dt.abs() > wt.abs() {
            wt = dt;
        }
    }
    println!("P6: worst deficit fp {wp:.6e}, ft {wt:.6e}; ADR 0342 4.597e-6, 4.369e-6 (old 2.5004597e-2)");
    assert!((wp - 4.597e-6).abs() <= 1e-9, "{wp}");
    assert!((wt - 4.369e-6).abs() <= 1e-9, "{wt}");
}

// ---- R ----

/// The piston with a prescribed motion: the `six_dof` table removed, `table` at the end.
fn prescribed_piston(table: &str) -> String {
    let t = piston(0.2, 0.01, "");
    let cut = t.find("[dynamic_mesh.six_dof]").unwrap();
    format!("{}{table}", &t[..cut]).replace(
        "motion = \"six_dof\"",
        "motion = \"prescribed\"\ndisplacement_expression_x = \"0.01 [m] * sin(t / 1 [s])\"",
    )
}

/// ADR 0346 R: where the new value is refused, a case without the table is what it was with the old value; the explicit
/// new value is still refused at the key.
#[test]
fn r_refused_contexts_keep_the_old_behaviour() {
    let layering = |t: &str| column("follow", t).replace("deformation = \"rigid\"", "deformation = \"layering\"");
    let comp = |t: &str| column("rest", t).replace("algorithm = \"piso\"", "algorithm = \"compressible\"");
    type Text = Box<dyn Fn(&str) -> String>;
    let cases: [(&str, Text, &str, &str, &str); 3] = [
        ("layering", Box::new(layering), REPORT_OLD, REPORT_NEW, REPORT_KEY),
        ("compressible", Box::new(comp), REPORT_OLD, REPORT_NEW, REPORT_KEY),
        (
            "prescribed motion",
            Box::new(prescribed_piston),
            BODY_OLD,
            BODY_NEW,
            BODY_KEY,
        ),
    ];
    for (name, text, old, new, key) in &cases {
        let (pn, po, pw) = (paths(&text("")), paths(&text(old)), paths(&text(new)));
        assert!(text("").contains(match *name {
            "layering" => "\"layering\"",
            "compressible" => "\"compressible\"",
            _ => "\"prescribed\"",
        }));
        println!("R {name}: without the table {pn:?}; old value {po:?}; new value {pw:?}");
        assert_eq!(pn, po, "{name}");
        assert!(!pn.iter().any(|p| p == key), "{name}");
        assert!(pw.iter().any(|p| p == key), "{name}");
    }
}
