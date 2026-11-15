//! F-635 (ADR 0574): the warping unknown on the harmonic and transient paths. Checks H1 to H8, H6b, H7', HW, HB, HE, TR1 to
//! TR4, TR3b, TR5' and R of the registration (with amendment 1). Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    load_vector, modal_analysis, solve_static, Warping, WarpingHarmonic, WarpingHarmonicOptions, WarpingSection,
    WarpingTransient,
};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::modal::ModalOptions;
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

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
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
    m.add_material("all", steel()).unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// The fork beam with the torque `T` about `x` on the node set `at`.
fn loaded(n: usize, section: &BeamSection, at: &str) -> SolidModel {
    let mut m = fork(n, section);
    m.add_load(Load::nodal_moment(at, [T, 0.0, 0.0])).unwrap();
    m
}

/// `(G J k_m^2 + E C_w k_m^4)/(rho I_p)`, `k_m = m pi/L`.
fn c3(s: &BeamSection, cw: f64, m: usize) -> f64 {
    let k = m as f64 * PI / L;
    (g_mod() * s.torsion() * k * k + E * cw * k.powi(4)) / (RHO * (s.i_y() + s.i_z()))
}

/// The continuous drive-point receptance at midspan (ADR 0574 arithmetic 2), 4000 terms: `(re, im)` of `sum_m (2 T sin^2(k_m
/// L/2)/(rho I_p L)) / (omega_m^2 (1 + i eta) - omega^2 + i omega (alpha + beta omega_m^2))`.
fn series(s: &BeamSection, cw: f64, omega: f64, eta: f64, alpha: f64, beta: f64) -> (f64, f64) {
    let rip = RHO * (s.i_y() + s.i_z());
    let (mut re, mut im) = (0.0, 0.0);
    for m in 1..=4000 {
        let sn = (m as f64 * PI / 2.0).sin();
        let c = 2.0 * T * sn * sn / (rip * L);
        let wm2 = c3(s, cw, m);
        let dr = wm2 - omega * omega;
        let di = eta * wm2 + omega * (alpha + beta * wm2);
        let d = dr * dr + di * di;
        re += c * dr / d;
        im -= c * di / d;
    }
    (re, im)
}

/// The continuous midspan twist under the step torque at `t` (4000 terms).
fn series_step(s: &BeamSection, t: f64) -> f64 {
    let rip = RHO * (s.i_y() + s.i_z());
    let (_, ws) = i_parts();
    let mut x = 0.0;
    for m in 1..=4000 {
        let sn = (m as f64 * PI / 2.0).sin();
        let wm2 = c3(s, ws.c_w(), m);
        x += 2.0 * T * sn * sn / (rip * L * wm2) * (1.0 - (wm2.sqrt() * t).cos());
    }
    x
}

fn twist_eq(m: &SolidModel, node: usize) -> usize {
    m.dof_map().equation(node, 3).unwrap()
}

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::FieldLength { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        other => format!("{other:?}"),
    }
}

/// D5: the secant search of the zero of `g(s) = 1/Re X_d(s)`; `x_d(omega)` returns the undamped drive-point response or the
/// error. Returns `(s, steps)` or `None` after 20 steps.
fn resonance(x_d: impl Fn(f64) -> Result<f64, FemError>, s0: f64, s1: f64) -> Option<(f64, usize)> {
    let g = |s: f64| -> Result<f64, FemError> { Ok(1.0 / x_d(s.sqrt())?) };
    let (mut s0, mut s1) = (s0, s1);
    let mut g0 = g(s0).ok()?;
    let mut g1 = match g(s1) {
        Ok(v) => v,
        Err(e) if key_of(&e) == "frequency" => return Some((s1, 0)),
        Err(_) => return None,
    };
    for step in 1..=20 {
        let s2 = s1 - g1 * (s1 - s0) / (g1 - g0);
        println!("    secant step {step}: s = {s2:.12e}");
        if (s2 - s1).abs() <= 1e-6 * s1.abs() {
            return Some((s2, step));
        }
        s0 = s1;
        g0 = g1;
        s1 = s2;
        g1 = match g(s1) {
            Ok(v) => v,
            Err(e) if key_of(&e) == "frequency" => return Some((s1, step)),
            Err(_) => return None,
        };
    }
    None
}

fn drive_point<'a>(
    h: &'a WarpingHarmonic,
    load: &HarmonicLoad,
    eq: usize,
) -> impl Fn(f64) -> Result<f64, FemError> + 'a {
    let load = load.clone();
    move |w: f64| Ok(h.response(&load, w)?.re[eq])
}

/// The index of the mode whose twist at node `node` is largest.
fn torsional_mode(m: &SolidModel, modes: &[Vec<f64>], node: usize) -> usize {
    let eq = twist_eq(m, node);
    (0..modes.len())
        .max_by(|&a, &b| modes[a][eq].abs().total_cmp(&modes[b][eq].abs()))
        .unwrap()
}

#[test]
fn h_harmonic_response() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w_closed = c3(&s, ws.c_w(), 1).sqrt();
    let w = Warping::new().block(0, ws);
    let m = loaded(16, &s, "mid");
    let eq = twist_eq(&m, 8);
    let load = HarmonicLoad::real(load_vector(&m, &w).unwrap()).unwrap();
    let h = WarpingHarmonic::new(&m, &w, &WarpingHarmonicOptions::new()).unwrap();

    // H2 (acceptance): the resonance of D5.
    let s_c = w_closed * w_closed;
    let (s_r, steps) = resonance(drive_point(&h, &load, eq), 0.98 * s_c, 1.02 * s_c).unwrap_or((f64::NAN, 21));
    let w_r = s_r.sqrt();
    println!("H2 resonance {w_r:.12e} rad/s after {steps} steps, closed {w_closed:.12e}");
    c.at_most("H2 resonance against sqrt(C3) (acceptance)", rel(w_r, w_closed), 1e-3);

    // H3: against the torsional mode of modal_analysis.
    let md = modal_analysis(&fork(16, &s), &w, &ModalOptions::new(4)).unwrap();
    let it = torsional_mode(&m, &md.modes, 8);
    println!("H3 modal eigenvalues {:?}, torsional mode {}", md.eigenvalues, it + 1);
    let lam = md.eigenvalues[it];
    c.at_most("H3 resonance against the modal eigenvalue", rel(s_r, lam), 1e-8);

    // H1: undamped at 0.5 sqrt(C3).
    let r1 = h.response(&load, 0.5 * w_closed).unwrap();
    let (x1, _) = series(&s, ws.c_w(), 0.5 * w_closed, 0.0, 0.0, 0.0);
    println!("H1 twist {:.12e}, series {x1:.12e}", r1.re[eq]);
    c.at_most("H1 undamped at 0.5 omega_1", rel(r1.re[eq], x1), 1e-5);

    // HB: no bending in the response.
    let dofs = m.dof_map();
    let mut bend = 0.0f64;
    for node in 0..=16 {
        for comp in [0, 1, 2, 4, 5] {
            if let Some(e) = dofs.equation(node, comp) {
                bend = bend.max(r1.re[e].abs()).max(r1.im[e].abs());
            }
        }
    }
    let tmax = (0..=16).map(|node| r1.re[twist_eq(&m, node)].abs()).fold(0.0, f64::max);
    c.at_most("HB bending over twist", bend / tmax, 1e-10);

    // H4: omega = 0 against solve_static.
    let st = solve_static(&m, &w, None).unwrap();
    let r0 = h.response(&load, 0.0).unwrap();
    let umax = st.displacement.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let dmax = r0
        .re
        .iter()
        .zip(&st.displacement)
        .fold(0.0f64, |a, (x, u)| a.max((x - u).abs()));
    c.at_most("H4 omega = 0 against solve_static", dmax / umax, 1e-10);
    c.holds(
        "H4 imaginary part zero",
        r0.im.iter().all(|&v| v == 0.0),
        "every imaginary value 0",
    );

    // H5, H6, H6b: damping against the series.
    let z = 0.02;
    let (alpha, beta) = (z * w_closed, z / w_closed);
    let cases = [
        ("H5 loss factor 0.02 at omega_1", 0.02, None, w_closed, 2e-4),
        (
            "H6 Rayleigh zeta 0.02 at omega_1",
            0.0,
            Some((alpha, beta)),
            w_closed,
            1e-4,
        ),
        (
            "H6b loss factor and Rayleigh at 0.5 omega_1",
            0.02,
            Some((alpha, beta)),
            0.5 * w_closed,
            1e-5,
        ),
    ];
    for (name, eta, ray, omega, limit) in cases {
        let mut o = WarpingHarmonicOptions::new().loss_factor(eta);
        if let Some((a, b)) = ray {
            o = o.rayleigh(Rayleigh { mass: a, stiffness: b });
        }
        let hd = WarpingHarmonic::new(&m, &w, &o).unwrap();
        let r = hd.response(&load, omega).unwrap();
        let (a, b) = ray.unwrap_or((0.0, 0.0));
        let (xr, xi) = series(&s, ws.c_w(), omega, eta, a, b);
        let (fr, fi) = (r.re[eq], r.im[eq]);
        // |X/X_c - 1| = |X - X_c|/|X_c|
        let err = (fr - xr).hypot(fi - xi) / xr.hypot(xi);
        println!("{name}: ({fr:.10e}, {fi:.10e}) against ({xr:.10e}, {xi:.10e})");
        c.at_most(name, err, limit);
    }

    // H7': the Saint-Venant section on the same element.
    let sv = Warping::new().block(0, WarpingSection::saint_venant());
    let hsv = WarpingHarmonic::new(&m, &sv, &WarpingHarmonicOptions::new()).unwrap();
    let lsv = HarmonicLoad::real(load_vector(&m, &sv).unwrap()).unwrap();
    let (s_sv, steps_sv) = resonance(drive_point(&hsv, &lsv, eq), 0.98 * s_c, 1.02 * s_c).unwrap_or((f64::NAN, 21));
    println!("H7' Saint-Venant resonance {:.10e} after {steps_sv} steps", s_sv.sqrt());
    c.above(
        "H7' Saint-Venant resonance off sqrt(C3)",
        rel(s_sv.sqrt(), w_closed),
        0.1,
    );

    // H8: 8 elements.
    let m8 = loaded(8, &s, "mid");
    let h8 = WarpingHarmonic::new(&m8, &w, &WarpingHarmonicOptions::new()).unwrap();
    let l8 = HarmonicLoad::real(load_vector(&m8, &w).unwrap()).unwrap();
    let (s8, _) = resonance(drive_point(&h8, &l8, twist_eq(&m8, 4)), 0.98 * s_c, 1.02 * s_c).unwrap_or((f64::NAN, 21));
    c.at_most("H8 resonance at 8 elements", rel(s8.sqrt(), w_closed), 1e-4);

    // HW: warping held at both ends.
    let wh = Warping::new().block(0, ws).restrain("x0").restrain("xl");
    let mdh = modal_analysis(&fork(16, &s), &wh, &ModalOptions::new(4)).unwrap();
    let ih = torsional_mode(&m, &mdh.modes, 8);
    let lam_h = mdh.eigenvalues[ih];
    println!("HW held eigenvalues {:?}, torsional mode {}", mdh.eigenvalues, ih + 1);
    let hh = WarpingHarmonic::new(&m, &wh, &WarpingHarmonicOptions::new()).unwrap();
    let lh = HarmonicLoad::real(load_vector(&m, &wh).unwrap()).unwrap();
    let (s_h, _) = resonance(drive_point(&hh, &lh, eq), 0.98 * lam_h, 1.02 * lam_h).unwrap_or((f64::NAN, 21));
    c.at_most("HW resonance against the held eigenvalue", rel(s_h, lam_h), 1e-8);
    c.holds(
        "HW held eigenvalue above 1.2 of the free",
        lam_h > 1.2 * lam,
        &format!("{:.6e} of the free", lam_h / lam),
    );

    // HE: the torque on the end node.
    let me = loaded(16, &s, "xl");
    let he = WarpingHarmonic::new(&me, &w, &WarpingHarmonicOptions::new()).unwrap();
    let le = HarmonicLoad::real(load_vector(&me, &w).unwrap()).unwrap();
    let re = he.response(&le, 0.5 * w_closed).unwrap();
    c.holds(
        "HE end torque: the response is zero",
        re.re.iter().chain(&re.im).all(|&v| v == 0.0),
        "every value 0",
    );
    c.finish();
}

#[test]
fn tr_transient_twist() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w_closed = c3(&s, ws.c_w(), 1).sqrt();
    let t1 = 2.0 * PI / w_closed;
    let dt = t1 / 400.0;
    let w = Warping::new().block(0, ws);
    let m = loaded(16, &s, "mid");
    let eq = twist_eq(&m, 8);
    let stat_c = series(&s, ws.c_w(), 0.0, 0.0, 0.0, 0.0).0;
    let run_step = |warping: &Warping, model: &SolidModel, steps: u64, opts: TransientOptions| {
        let f = load_vector(model, warping).unwrap();
        let lh = LoadHistory::new(f.clone(), LoadTable::constant(1.0, 0.0, 10.0).unwrap());
        let mut tr = WarpingTransient::new(model, warping, &[lh], &opts).unwrap();
        tr.start_at_rest(0.0).unwrap();
        (tr, f, steps)
    };

    // TR1, TR2, TR4.
    let (mut tr, f, steps) = run_step(&w, &m, 800, TransientOptions::new(dt));
    let us = solve_static(&m, &w, None).unwrap();
    let e0 = 0.5 * us.displacement.iter().zip(&f).map(|(a, b)| a * b).sum::<f64>();
    let (mut err, mut peak, mut drift) = (0.0f64, 0.0f64, 0.0f64);
    for n in 0..=steps {
        if n > 0 {
            tr.run(1).unwrap();
        }
        let u = tr.displacement().unwrap();
        let t = n as f64 * dt;
        err = err.max((u[eq] - series_step(&s, t)).abs());
        peak = peak.max(u[eq]);
        let it = tr.integrator();
        let fu: f64 = u.iter().zip(&f).map(|(a, b)| a * b).sum();
        let en = it.kinetic_energy().unwrap() + it.strain_energy().unwrap() - fu + e0;
        drift = drift.max((en / e0 - 1.0).abs());
    }
    println!("TR1 static (series) {stat_c:.10e}, peak {peak:.10e}");
    c.at_most(
        "TR1 transient twist against the series (acceptance)",
        err / stat_c,
        1.5e-2,
    );
    let ratio = peak / stat_c;
    c.holds(
        "TR2 peak over the static twist in [1.9, 2.05]",
        ratio.is_finite() && (1.9..=2.05).contains(&ratio),
        &format!("{ratio:.6}"),
    );
    c.at_most("TR4 energy drift", drift, 1e-9);

    // TR3, TR3b: mass-proportional damping to the static twist.
    let alpha = 2.0 * w_closed;
    let steps3 = (25.0 / (w_closed * dt)).ceil() as u64;
    println!("TR3 steps {steps3}");
    for (name, warping, limit) in [
        ("TR3 damped final twist against solve_static", w.clone(), 1e-5),
        (
            "TR3b damped final twist, warping held, against solve_static",
            Warping::new().block(0, ws).restrain("x0").restrain("xl"),
            2e-5,
        ),
    ] {
        let opts = TransientOptions::new(dt).damping(Rayleigh {
            mass: alpha,
            stiffness: 0.0,
        });
        let (mut trd, _, _) = run_step(&warping, &m, steps3, opts);
        trd.run(steps3).unwrap();
        let u = trd.displacement().unwrap();
        let st = solve_static(&m, &warping, None).unwrap();
        println!("{name}: {:.10e} against {:.10e}", u[eq], st.displacement[eq]);
        c.at_most(name, rel(u[eq], st.displacement[eq]), limit);
    }

    // TR5': the Saint-Venant section on the same element.
    let sv = Warping::new().block(0, WarpingSection::saint_venant());
    let (mut trs, _, _) = run_step(&sv, &m, 800, TransientOptions::new(dt));
    let mut err_sv = 0.0f64;
    for n in 0..=800u64 {
        if n > 0 {
            trs.run(1).unwrap();
        }
        let u = trs.displacement().unwrap();
        err_sv = err_sv.max((u[eq] - series_step(&s, n as f64 * dt)).abs());
    }
    c.above("TR5' Saint-Venant transient off the series", err_sv / stat_c, 0.5);
    c.finish();
}

#[test]
fn r_refusals() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let m = loaded(16, &s, "mid");
    let mut expect = |name: &str, r: Result<(), FemError>, key: &str| {
        let got = match &r {
            Ok(()) => "accepted".to_string(),
            Err(e) => key_of(e),
        };
        c.holds(name, got == key, &format!("key {got} (want {key})"));
    };
    let ho = WarpingHarmonicOptions::new();
    expect(
        "R harmonic without a warping block (RF1)",
        WarpingHarmonic::new(&m, &Warping::new(), &ho).map(|_| ()),
        "warping",
    );
    let mut mp = m.clone();
    mp.add_constraint(Constraint::displacement("mid", 0, 1e-3)).unwrap();
    expect(
        "R harmonic with a nonzero prescribed value",
        WarpingHarmonic::new(&mp, &w, &ho).map(|_| ()),
        "constraint",
    );
    expect(
        "R harmonic with a negative loss factor",
        WarpingHarmonic::new(&m, &w, &WarpingHarmonicOptions::new().loss_factor(-0.1)).map(|_| ()),
        "damping",
    );
    let h = WarpingHarmonic::new(&m, &w, &ho).unwrap();
    let load = HarmonicLoad::real(load_vector(&m, &w).unwrap()).unwrap();
    expect(
        "R response at a negative frequency",
        h.response(&load, -1.0).map(|_| ()),
        "frequency",
    );
    let six = HarmonicLoad::from_model(&m).unwrap();
    expect(
        "R response with a load of the six-unknown length",
        h.response(&six, 100.0).map(|_| ()),
        "harmonic load",
    );
    let md = modal_analysis(&fork(16, &s), &w, &ModalOptions::new(4)).unwrap();
    let it = torsional_mode(&m, &md.modes, 8);
    expect(
        "R undamped response at the eigenvalue",
        h.response(&load, md.eigenvalues[it].sqrt()).map(|_| ()),
        "frequency",
    );
    let dt = 1e-4;
    let six_lh = LoadHistory::model_loads(&m, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    expect(
        "R transient with a pattern of the six-unknown length",
        WarpingTransient::new(&m, &w, &[six_lh], &TransientOptions::new(dt)).map(|_| ()),
        "load pattern",
    );
    let lh = LoadHistory::new(
        load_vector(&m, &w).unwrap(),
        LoadTable::constant(1.0, 0.0, 1.0).unwrap(),
    );
    expect(
        "R transient with a nonzero prescribed value",
        WarpingTransient::new(&mp, &w, std::slice::from_ref(&lh), &TransientOptions::new(dt)).map(|_| ()),
        "constraint",
    );
    let mut tr = WarpingTransient::new(&m, &w, &[lh], &TransientOptions::new(dt)).unwrap();
    let n = tr.dofs().n_equations();
    let mut u0 = vec![0.0; n];
    u0[twist_eq(&m, 0)] = 1e-3;
    expect(
        "R start with a displacement at a held twist",
        tr.start(0.0, &u0, &vec![0.0; n]),
        "initial state",
    );
    c.finish();
}
