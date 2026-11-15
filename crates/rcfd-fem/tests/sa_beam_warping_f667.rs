//! F-667 (ADR 0591): the pre-stress on the warping transient path. Checks P0, TR1, TR1T, TR2, TR4, TRD, TR3, TR5, HB, LG and
//! R of the registration (B0 is `sa_beam_warping_f667_bits.rs`). Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{load_vector, solve_static, Warping, WarpingPrestress, WarpingSection, WarpingTransient};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::transient::{LoadHistory, LoadTable, Rayleigh, TransientOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite is a miss.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const L: f64 = 3.0;
const T: f64 = 1000.0;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

/// The fork beam of `n` elements along `x`: `u = 0` at node 0, `v = w = phi = 0` at both ends; node sets `x0`, `xl`, `mid`,
/// `nodes`. No load.
fn fork(n: usize, section: &BeamSection) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    mesh.add_node_set("nodes", (0..=n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// The torsion-only model of ADR 0548 F1: the fork beam with the components 0, 1, 2, 4, 5 held at every node.
fn torsion_only(n: usize, section: &BeamSection) -> SolidModel {
    let mut m = fork(n, section);
    for c in [0, 1, 2, 4, 5] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m
}

fn with_torque(mut m: SolidModel) -> SolidModel {
    m.add_load(Load::nodal_moment("mid", [T, 0.0, 0.0])).unwrap();
    m
}

/// The pre-stress of the readings: the warping static solve of the `n`-element fork beam with the force `(n_ax, 0, 0)` at
/// `xl` (tension positive), `WarpingPrestress::from_static`.
fn axial_prestress(n: usize, s: &BeamSection, w: &Warping, n_ax: f64) -> WarpingPrestress {
    let mut l = fork(n, s);
    l.add_load(Load::nodal_force("xl", [n_ax, 0.0, 0.0])).unwrap();
    let r = solve_static(&l, w, None).unwrap();
    WarpingPrestress::from_static(&l, w, &r, [0.0; 2]).unwrap()
}

/// `|N_T| = (G J + E C_w k_1^2) A/I_p`.
fn n_t(s: &BeamSection, ws: &WarpingSection) -> f64 {
    let k = PI / L;
    (g_mod() * s.torsion() + E * ws.c_w() * k * k) * s.area() / (s.i_y() + s.i_z())
}

/// C4: `omega_m^2 = (G J k_m^2 + N I_p/A k_m^2 + E C_w k_m^4)/(rho I_p)`.
fn c4(s: &BeamSection, cw: f64, m: usize, n_ax: f64) -> f64 {
    let k = m as f64 * PI / L;
    let ip = s.i_y() + s.i_z();
    ((g_mod() * s.torsion() + n_ax * ip / s.area()) * k * k + E * cw * k.powi(4)) / (RHO * ip)
}

/// C5 and C5d: the continuous midspan twist under the step torque at `t` with `N`, Rayleigh `C = alpha M + beta (K +
/// K_sigma)` (4000 terms; the overdamped modes in the overdamped form).
fn series_step(s: &BeamSection, cw: f64, n_ax: f64, t: f64, alpha: f64, beta: f64) -> f64 {
    let rip = RHO * (s.i_y() + s.i_z());
    let mut x = 0.0;
    for m in 1..=4000 {
        let sn = (m as f64 * PI / 2.0).sin();
        let sn2 = sn * sn;
        if sn2 < 1e-30 {
            continue;
        }
        let w2 = c4(s, cw, m, n_ax);
        let w = w2.sqrt();
        let st = 2.0 * T * sn2 / (rip * L * w2);
        let z = (alpha + beta * w2) / (2.0 * w);
        let shape = if z == 0.0 {
            1.0 - (w * t).cos()
        } else if z < 1.0 {
            let q = (1.0 - z * z).sqrt();
            1.0 - (-z * w * t).exp() * ((w * q * t).cos() + z / q * (w * q * t).sin())
        } else {
            let q = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w / (z + q), -w * (z + q));
            1.0 + (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r1 - r2)
        };
        x += st * shape;
    }
    x
}

/// The continuous static twist `phi_c,s` with `N`.
fn series_static(s: &BeamSection, cw: f64, n_ax: f64) -> f64 {
    let rip = RHO * (s.i_y() + s.i_z());
    (1..=4000)
        .map(|m| {
            let sn = (m as f64 * PI / 2.0).sin();
            2.0 * T * sn * sn / (rip * L * c4(s, cw, m, n_ax))
        })
        .sum()
}

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::FieldLength { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        other => format!("{other:?}"),
    }
}

fn sign_name(frac: f64) -> &'static str {
    if frac > 0.0 {
        "+"
    } else {
        "-"
    }
}

/// The integrator with the pre-stress and the step load of `model`, started from rest.
fn step_run(
    model: &SolidModel,
    w: &Warping,
    p: &WarpingPrestress,
    opts: &TransientOptions,
) -> (WarpingTransient, Vec<f64>) {
    let f = load_vector(model, w).unwrap();
    let lh = LoadHistory::new(f.clone(), LoadTable::constant(1.0, 0.0, 10.0).unwrap());
    let mut tr = WarpingTransient::with_prestress(model, w, &[lh], opts, p).unwrap();
    tr.start_at_rest(0.0).unwrap();
    (tr, f)
}

/// FNV-1a 64 over the count and the `to_bits()` of each value (ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

#[test]
fn p0_zero_prestress_keeps_the_bits() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let m = with_torque(fork(16, &s));
    let w1 = c4(&s, ws.c_w(), 1, 0.0).sqrt();
    let dt = 2.0 * PI / w1 / 400.0;
    let zero = WarpingPrestress::torsion_free(Prestress::uniform(&m, [0.0; 6]).unwrap());
    let f = load_vector(&m, &w).unwrap();
    let lh = LoadHistory::new(f, LoadTable::constant(1.0, 0.0, 10.0).unwrap());
    for (name, opts) in [
        ("P0 undamped", TransientOptions::new(dt)),
        (
            "P0 Rayleigh",
            TransientOptions::new(dt).damping(Rayleigh {
                mass: 0.02 * w1,
                stiffness: 0.02 / w1,
            }),
        ),
    ] {
        let mut a = WarpingTransient::new(&m, &w, std::slice::from_ref(&lh), &opts).unwrap();
        let mut b = WarpingTransient::with_prestress(&m, &w, std::slice::from_ref(&lh), &opts, &zero).unwrap();
        a.start_at_rest(0.0).unwrap();
        b.start_at_rest(0.0).unwrap();
        a.run(100).unwrap();
        b.run(100).unwrap();
        let da = digest(a.displacement().unwrap().into_iter().chain(a.velocity().unwrap()));
        let db = digest(b.displacement().unwrap().into_iter().chain(b.velocity().unwrap()));
        c.holds(name, da == db, &format!("0x{da:016x} against 0x{db:016x}"));
    }
    c.finish();
}

#[test]
fn tr1_step_torque_under_the_axial_force() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    println!("|N_T| {nt:.10e}");
    for frac in [0.5, -0.5] {
        let sg = sign_name(frac);
        let n_ax = frac * nt;
        let w1 = c4(&s, ws.c_w(), 1, n_ax).sqrt();
        let t1 = 2.0 * PI / w1;
        let dt = t1 / 400.0;
        let stat_c = series_static(&s, ws.c_w(), n_ax);
        println!("N {n_ax:.6e}: omega_1 {w1:.8}, T_1 {t1:.8e}, static (series) {stat_c:.10e}");
        let p = axial_prestress(16, &s, &w, n_ax);
        let series: Vec<f64> = (0..=800)
            .map(|n| series_step(&s, ws.c_w(), n_ax, n as f64 * dt, 0.0, 0.0))
            .collect();
        for (label, model) in [
            ("TR1", with_torque(fork(16, &s))),
            ("TR1T", with_torque(torsion_only(16, &s))),
        ] {
            let eq = model.dof_map().equation(8, 3).unwrap();
            let bend: Vec<usize> = (0..=16)
                .flat_map(|node| [0, 1, 2, 4, 5].map(|cmp| model.dof_map().equation(node, cmp)))
                .flatten()
                .collect();
            let (mut tr, f) = step_run(&model, &w, &p, &TransientOptions::new(dt));
            let e_s = 0.5 * T * stat_c;
            let (mut err, mut peak, mut drift, mut bmax, mut tmax) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
            for (n, phi_c) in series.iter().enumerate() {
                if n > 0 {
                    tr.run(1).unwrap();
                }
                let u = tr.displacement().unwrap();
                err = err.max((u[eq] - phi_c).abs());
                peak = peak.max(u[eq]);
                tmax = tmax.max(u[eq].abs());
                for &b in &bend {
                    bmax = bmax.max(u[b].abs());
                }
                let it = tr.integrator();
                let fu: f64 = u.iter().zip(&f).map(|(a, b)| a * b).sum();
                let en = it.kinetic_energy().unwrap() + it.strain_energy().unwrap() - fu;
                drift = drift.max(en.abs() / e_s);
            }
            println!("{label}{sg}: peak {peak:.10e}");
            c.at_most(
                &format!("{label}{sg} twist against the series (acceptance)"),
                err / stat_c,
                1.5e-2,
            );
            if label == "TR1" {
                let ratio = peak / stat_c;
                c.holds(
                    &format!("TR2{sg} peak over the static twist in [1.9, 2.05]"),
                    ratio.is_finite() && (1.9..=2.05).contains(&ratio),
                    &format!("{ratio:.6}"),
                );
                c.at_most(&format!("TR4{sg} energy"), drift, 1e-9);
                c.at_most(&format!("HB{sg} bending over twist"), bmax / tmax, 1e-10);
            }
        }
        // TR5: the default constructor (no pre-stress) against the series with N.
        let m = with_torque(fork(16, &s));
        let eq = m.dof_map().equation(8, 3).unwrap();
        let f = load_vector(&m, &w).unwrap();
        let lh = LoadHistory::new(f, LoadTable::constant(1.0, 0.0, 10.0).unwrap());
        let mut tr = WarpingTransient::new(&m, &w, &[lh], &TransientOptions::new(dt)).unwrap();
        tr.start_at_rest(0.0).unwrap();
        let mut err = 0.0f64;
        for (n, phi_c) in series.iter().enumerate() {
            if n > 0 {
                tr.run(1).unwrap();
            }
            err = err.max((tr.displacement().unwrap()[eq] - phi_c).abs());
        }
        c.above(&format!("TR5{sg} no pre-stress off the series"), err / stat_c, 0.5);
    }
    c.finish();
}

#[test]
fn trd_tr3_damped_runs() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    let m = with_torque(fork(16, &s));
    let eq = m.dof_map().equation(8, 3).unwrap();
    for frac in [0.5, -0.5] {
        let sg = sign_name(frac);
        let n_ax = frac * nt;
        let w1 = c4(&s, ws.c_w(), 1, n_ax).sqrt();
        let dt = 2.0 * PI / w1 / 400.0;
        let stat_c = series_static(&s, ws.c_w(), n_ax);
        let p = axial_prestress(16, &s, &w, n_ax);
        // TRD: Rayleigh zeta = 0.02 on mode 1, beta on K + K_sigma.
        let (alpha, beta) = (0.02 * w1, 0.02 / w1);
        let opts = TransientOptions::new(dt).damping(Rayleigh {
            mass: alpha,
            stiffness: beta,
        });
        let (mut tr, _) = step_run(&m, &w, &p, &opts);
        let mut err = 0.0f64;
        for n in 0..=800u64 {
            if n > 0 {
                tr.run(1).unwrap();
            }
            let phi_c = series_step(&s, ws.c_w(), n_ax, n as f64 * dt, alpha, beta);
            err = err.max((tr.displacement().unwrap()[eq] - phi_c).abs());
        }
        c.at_most(
            &format!("TRD{sg} Rayleigh against the damped series"),
            err / stat_c,
            2e-3,
        );
        // TR3: mass-proportional alpha = 2 omega_1(N) to the static twist.
        let steps3 = (25.0 / (w1 * dt)).ceil() as u64;
        let opts = TransientOptions::new(dt).damping(Rayleigh {
            mass: 2.0 * w1,
            stiffness: 0.0,
        });
        let (mut tr, _) = step_run(&m, &w, &p, &opts);
        tr.run(steps3).unwrap();
        let u = tr.displacement().unwrap()[eq];
        println!("TR3{sg} steps {steps3}: final {u:.10e} against {stat_c:.10e}");
        c.at_most(
            &format!("TR3{sg} damped final twist against the series static"),
            (u / stat_c - 1.0).abs(),
            2e-5,
        );
    }
    c.finish();
}

#[test]
fn lg_lateral_step_force_under_the_axial_force() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    let mut m = fork(16, &s);
    m.add_load(Load::nodal_force("mid", [0.0, 100.0, 100.0])).unwrap();
    let (ev, ew) = (m.dof_map().equation(8, 1).unwrap(), m.dof_map().equation(8, 2).unwrap());
    for frac in [0.5, -0.5] {
        let sg = sign_name(frac);
        let n_ax = frac * nt;
        let w1 = c4(&s, ws.c_w(), 1, n_ax).sqrt();
        let dt = 2.0 * PI / w1 / 400.0;
        let p = axial_prestress(16, &s, &w, n_ax);
        let (mut tr, _) = step_run(&m, &w, &p, &TransientOptions::new(dt));
        let (mut first, mut second) = (0.0f64, 0.0f64);
        for n in 1..=800u64 {
            tr.run(1).unwrap();
            let u = tr.displacement().unwrap();
            let lat = u[ev].hypot(u[ew]);
            if n <= 400 {
                first = first.max(lat);
            } else {
                second = second.max(lat);
            }
        }
        let r = second / first;
        println!("LG{sg}: first period {first:.6e}, second {second:.6e}");
        if frac < 0.0 {
            c.above("LG- lateral growth in compression", r, 10.0);
        } else {
            c.at_most("LG+ lateral bounded in tension", r, 1.5);
        }
    }
    c.finish();
}

#[test]
fn r_refusals_with_the_prestress() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let m = with_torque(fork(16, &s));
    let w1 = c4(&s, ws.c_w(), 1, 0.0).sqrt();
    let opts = TransientOptions::new(2.0 * PI / w1 / 400.0);
    let f = load_vector(&m, &w).unwrap();
    let lh = LoadHistory::new(f, LoadTable::constant(1.0, 0.0, 1.0).unwrap());
    let mut check = |name: &str, r: Result<WarpingTransient, FemError>, key: &str| {
        let got = match r {
            Ok(_) => "accepted".to_string(),
            Err(e) => {
                println!("    {name}: {e}");
                key_of(&e)
            }
        };
        c.holds(name, got == key, &format!("key {got} (want {key})"));
    };
    // RF8: a torsion-free pre-stress with the torque of the plain static solve.
    let r = rcfd_fem::solve::solve_static(&m, &StaticOptions::default()).unwrap();
    let sv = Prestress::from_static(&m, &r)
        .unwrap()
        .with_beam_resultants(&m, r.displacement_values(), [0.0; 2])
        .unwrap();
    check(
        "R RF8 torque in a torsion-free pre-stress",
        WarpingTransient::with_prestress(
            &m,
            &w,
            std::slice::from_ref(&lh),
            &opts,
            &WarpingPrestress::torsion_free(sv),
        ),
        "warping",
    );
    // RF10: a from_static pre-stress of another section.
    let p = axial_prestress(16, &s, &w, 1e5);
    let other = Warping::new().block(0, WarpingSection::saint_venant());
    check(
        "R RF10 other section",
        WarpingTransient::with_prestress(&m, &other, std::slice::from_ref(&lh), &opts, &p),
        "warping",
    );
    // RF10: a from_static pre-stress of the 8-element model.
    let p8 = axial_prestress(8, &s, &w, 1e5);
    check(
        "R RF10 other numbering",
        WarpingTransient::with_prestress(&m, &w, std::slice::from_ref(&lh), &opts, &p8),
        "warping",
    );
    // A load pattern of the six-unknown length.
    let six = LoadHistory::model_loads(&m, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    check(
        "R six-unknown load pattern",
        WarpingTransient::with_prestress(&m, &w, &[six], &opts, &p),
        "load pattern",
    );
    c.finish();
}
