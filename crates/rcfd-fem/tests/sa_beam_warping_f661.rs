//! F-661 (ADR 0585): the pre-stress on the warping harmonic path. Checks P0, H2, H2T, H3, H8, H1, H5, H6, H6b, H7, HS, HB-
//! and R of the registration (B0 is `sa_beam_warping_f661_bits.rs`). Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    load_vector, prestressed_modes, solve_static, Warping, WarpingHarmonic, WarpingHarmonicOptions, WarpingPrestress,
    WarpingSection,
};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::transient::Rayleigh;
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

/// C4: `(G J k_m^2 + N I_p/A k_m^2 + E C_w k_m^4)/(rho I_p)`; `None` for `N = 0` gives C3. Returns (elastic part, geometric
/// part) of `omega_m^2`.
fn c4_parts(s: &BeamSection, cw: f64, m: usize, n_ax: f64) -> (f64, f64) {
    let k = m as f64 * PI / L;
    let ip = s.i_y() + s.i_z();
    (
        (g_mod() * s.torsion() * k * k + E * cw * k.powi(4)) / (RHO * ip),
        n_ax * ip / s.area() * k * k / (RHO * ip),
    )
}

fn c4(s: &BeamSection, cw: f64, n_ax: f64) -> f64 {
    let (a, b) = c4_parts(s, cw, 1, n_ax);
    a + b
}

/// The continuous drive-point receptance at midspan with `N` (ADR 0585 arithmetic 3), 4000 terms: the loss factor on the
/// elastic part only, the Rayleigh stiffness on the total.
fn series(s: &BeamSection, cw: f64, n_ax: f64, omega: f64, eta: f64, alpha: f64, beta: f64) -> (f64, f64) {
    let rip = RHO * (s.i_y() + s.i_z());
    let (mut re, mut im) = (0.0, 0.0);
    for m in 1..=4000 {
        let sn = (m as f64 * PI / 2.0).sin();
        let c = 2.0 * T * sn * sn / (rip * L);
        let (ke, kg) = c4_parts(s, cw, m, n_ax);
        let dr = ke + kg - omega * omega;
        let di = eta * ke + omega * (alpha + beta * (ke + kg));
        let d = dr * dr + di * di;
        re += c * dr / d;
        im -= c * di / d;
    }
    (re, im)
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

/// D4 (ADR 0574 D5): the secant search of the zero of `g(s) = 1/Re X_d(s)`. Returns `(s, steps)` or `None` after 20 steps
/// or when the first evaluation fails.
fn resonance(x_d: impl Fn(f64) -> Result<f64, FemError>, s0: f64, s1: f64) -> Option<(f64, usize)> {
    let g = |s: f64| -> Result<f64, FemError> { Ok(1.0 / x_d(s.sqrt())?) };
    let (mut s0, mut s1) = (s0, s1);
    let mut g0 = match g(s0) {
        Ok(v) => v,
        Err(e) => {
            println!("    first evaluation refused: {e}");
            return None;
        }
    };
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
            Err(e) if key_of(&e) == "frequency" => {
                println!("    refused at step {step}: {e}");
                return Some((s1, step));
            }
            Err(_) => return None,
        };
    }
    None
}

/// The searched resonance `s` of the midspan twist of `m` (a loaded model) with `options`, started at 0.98 and 1.02 of
/// `s_c`.
fn search(m: &SolidModel, w: &Warping, options: &WarpingHarmonicOptions, s_c: f64) -> f64 {
    let eq = twist_eq(m, m.mesh().nodes().len() / 2);
    let load = HarmonicLoad::real(load_vector(m, w).unwrap()).unwrap();
    let h = WarpingHarmonic::new(m, w, options).unwrap();
    let (s, steps) =
        resonance(|om: f64| Ok(h.response(&load, om)?.re[eq]), 0.98 * s_c, 1.02 * s_c).unwrap_or((f64::NAN, 21));
    println!("    resonance s {s:.12e} after {steps} steps (C4 {s_c:.12e})");
    s
}

fn frac_name(frac: f64) -> &'static str {
    if frac > 0.0 {
        "+"
    } else {
        "-"
    }
}

#[test]
fn h2_h3_h8_hs_resonances() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    println!("|N_T| {nt:.10e}");
    let mut w_r = Vec::new();
    for frac in [0.5, -0.5] {
        let tag = frac_name(frac);
        let n_ax = frac * nt;
        let s_c = c4(&s, ws.c_w(), n_ax);
        let p16 = axial_prestress(16, &s, &w, n_ax);
        let opts = WarpingHarmonicOptions::new().prestress(p16.clone());

        // H2 (acceptance): the full fork beam, default resonance pivot.
        println!("H2{tag} full fork beam, N = {n_ax:.6e}");
        let sr = search(&with_torque(fork(16, &s)), &w, &opts, s_c);
        c.at_most(
            &format!("H2{tag} resonance against sqrt(C4) (acceptance)"),
            rel(sr.sqrt(), s_c.sqrt()),
            1e-3,
        );
        w_r.push(sr.sqrt());

        // H2T: the torsion-only model.
        println!("H2T{tag} torsion-only model");
        let srt = search(&with_torque(torsion_only(16, &s)), &w, &opts, s_c);
        c.at_most(
            &format!("H2T{tag} torsion-only resonance against sqrt(C4)"),
            rel(srt.sqrt(), s_c.sqrt()),
            1e-3,
        );

        // H3: the search without the resonance pivot against the pre-stressed modes.
        let opts0 = opts.clone().resonance_pivot(0.0);
        if frac > 0.0 {
            println!("H3+ full beam, pivot 0");
            let s3 = search(&with_torque(fork(16, &s)), &w, &opts0, s_c);
            let md = prestressed_modes(&fork(16, &s), &p16, &w, &ModalOptions::new(4)).unwrap();
            let eq = twist_eq(&fork(16, &s), 8);
            let it = (0..md.modes.len())
                .max_by(|&a, &b| md.modes[a][eq].abs().total_cmp(&md.modes[b][eq].abs()))
                .unwrap();
            println!(
                "H3+ pre-stressed eigenvalues {:?}, torsional mode {}",
                md.eigenvalues,
                it + 1
            );
            c.at_most(
                "H3+ resonance against the pre-stressed eigenvalue",
                rel(s3, md.eigenvalues[it]),
                1e-8,
            );
        } else {
            println!("H3- torsion-only model, pivot 0");
            let s3 = search(&with_torque(torsion_only(16, &s)), &w, &opts0, s_c);
            let md = prestressed_modes(&torsion_only(16, &s), &p16, &w, &ModalOptions::new(1)).unwrap();
            println!("H3- pre-stressed eigenvalue {:?}", md.eigenvalues);
            c.at_most(
                "H3- resonance against the pre-stressed eigenvalue",
                rel(s3, md.eigenvalues[0]),
                1e-8,
            );
        }

        // H8: 8 elements.
        println!("H8{tag} 8 elements");
        let p8 = axial_prestress(8, &s, &w, n_ax);
        let s8 = search(
            &with_torque(fork(8, &s)),
            &w,
            &WarpingHarmonicOptions::new().prestress(p8),
            s_c,
        );
        c.at_most(
            &format!("H8{tag} resonance at 8 elements"),
            rel(s8.sqrt(), s_c.sqrt()),
            1e-4,
        );
    }
    // HS: the sign of N.
    c.at_most(
        "HS tension over compression resonance against sqrt(3)",
        rel(w_r[0] / w_r[1], 3f64.sqrt()),
        1e-5,
    );
    c.finish();
}

#[test]
fn h1_h5_h6_h7_hb_responses() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    let m = with_torque(fork(16, &s));
    let eq = twist_eq(&m, 8);
    let load = HarmonicLoad::real(load_vector(&m, &w).unwrap()).unwrap();
    let plain = WarpingHarmonic::new(&m, &w, &WarpingHarmonicOptions::new()).unwrap();
    for frac in [0.5, -0.5] {
        let tag = frac_name(frac);
        let n_ax = frac * nt;
        let wn = c4(&s, ws.c_w(), n_ax).sqrt();
        let p = axial_prestress(16, &s, &w, n_ax);
        let z = 0.02;
        let (alpha, beta) = (z * wn, z / wn);
        let cases: [(&str, f64, Option<Rayleigh>, f64, f64); 4] = [
            ("H1", 0.0, None, 0.5 * wn, 3e-5),
            ("H5", 0.02, None, wn, 2e-4),
            (
                "H6",
                0.0,
                Some(Rayleigh {
                    mass: alpha,
                    stiffness: beta,
                }),
                wn,
                2e-4,
            ),
            (
                "H6b",
                0.02,
                Some(Rayleigh {
                    mass: alpha,
                    stiffness: beta,
                }),
                0.5 * wn,
                3e-5,
            ),
        ];
        for (name, eta, ray, om, limit) in cases {
            let mut o = WarpingHarmonicOptions::new().loss_factor(eta).prestress(p.clone());
            if let Some(r) = ray {
                o = o.rayleigh(r);
            }
            let h = WarpingHarmonic::new(&m, &w, &o).unwrap();
            let r = h.response(&load, om).unwrap();
            let (ar, ai) = (r.re[eq], r.im[eq]);
            let (br, bi) = series(
                &s,
                ws.c_w(),
                n_ax,
                om,
                eta,
                ray.map_or(0.0, |r| r.mass),
                ray.map_or(0.0, |r| r.stiffness),
            );
            let err = ((ar - br).powi(2) + (ai - bi).powi(2)).sqrt() / (br * br + bi * bi).sqrt();
            println!("{name}{tag} twist ({ar:.12e}, {ai:.12e}), series ({br:.12e}, {bi:.12e})");
            c.at_most(&format!("{name}{tag} against the series"), err, limit);
            if name == "H1" && frac < 0.0 {
                // HB-: no bending in the response of the indefinite case.
                let dofs = m.dof_map();
                let mut bend = 0.0f64;
                for node in 0..=16 {
                    for comp in [0, 1, 2, 4, 5] {
                        if let Some(e) = dofs.equation(node, comp) {
                            bend = bend.max(r.re[e].abs()).max(r.im[e].abs());
                        }
                    }
                }
                let tmax = (0..=16).map(|node| r.re[twist_eq(&m, node)].abs()).fold(0.0, f64::max);
                c.at_most("HB- bending over twist", bend / tmax, 1e-10);
            }
        }
        // H7: without the option.
        let r0 = plain.response(&load, 0.5 * wn).unwrap();
        let (br, _) = series(&s, ws.c_w(), n_ax, 0.5 * wn, 0.0, 0.0, 0.0);
        c.above(
            &format!("H7{tag} without the option against the series with N"),
            rel(r0.re[eq], br),
            0.3,
        );
    }
    c.finish();
}

#[test]
fn p0_zero_prestress_is_bit_equal() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let m = with_torque(fork(16, &s));
    let w1 = c4(&s, ws.c_w(), 0.0).sqrt();
    let zero = WarpingPrestress::torsion_free(Prestress::uniform(&m, [0.0; 6]).unwrap());
    let load = HarmonicLoad::real(load_vector(&m, &w).unwrap()).unwrap();
    let ray = Rayleigh {
        mass: 0.02 * w1,
        stiffness: 0.02 / w1,
    };
    let cases = [
        ("undamped at 0.5 omega_1", WarpingHarmonicOptions::new(), 0.5 * w1),
        (
            "loss factor at omega_1",
            WarpingHarmonicOptions::new().loss_factor(0.02),
            w1,
        ),
        ("Rayleigh at omega_1", WarpingHarmonicOptions::new().rayleigh(ray), w1),
        ("undamped at 0", WarpingHarmonicOptions::new(), 0.0),
    ];
    for (name, o, om) in cases {
        let a = WarpingHarmonic::new(&m, &w, &o).unwrap().response(&load, om).unwrap();
        let b = WarpingHarmonic::new(&m, &w, &o.clone().prestress(zero.clone()))
            .unwrap()
            .response(&load, om)
            .unwrap();
        let same = a.re.len() == b.re.len()
            && a.re.iter().zip(&b.re).all(|(x, y)| x.to_bits() == y.to_bits())
            && a.im.iter().zip(&b.im).all(|(x, y)| x.to_bits() == y.to_bits());
        c.holds(
            &format!("P0 {name}"),
            same,
            "bit-equal to the response without the option",
        );
    }
    c.finish();
}

#[test]
fn r_refusals_with_the_option() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let m = with_torque(fork(16, &s));
    let mut check = |name: &str, r: Result<WarpingHarmonic, FemError>, key: &str| {
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
        WarpingHarmonic::new(
            &m,
            &w,
            &WarpingHarmonicOptions::new().prestress(WarpingPrestress::torsion_free(sv)),
        ),
        "warping",
    );
    // RF10: a from_static pre-stress of another section.
    let p = axial_prestress(16, &s, &w, 1e5);
    let other = Warping::new().block(0, WarpingSection::saint_venant());
    check(
        "R RF10 other section",
        WarpingHarmonic::new(&m, &other, &WarpingHarmonicOptions::new().prestress(p)),
        "warping",
    );
    // RF10: a from_static pre-stress of the 8-element model.
    let p8 = axial_prestress(8, &s, &w, 1e5);
    check(
        "R RF10 other numbering",
        WarpingHarmonic::new(&m, &w, &WarpingHarmonicOptions::new().prestress(p8)),
        "warping",
    );
    c.finish();
}
