//! F-478 (ADR 0378): the wave-solver drive with returning waves at the sampled section. A closed-form channel field holding
//! the F-452 entering components and a returning wave of each (reflection factors `R`) is sampled on two sections (24
//! wall-clustered faces at x = -0.02 m, 20 uniform faces at x = -0.05 m, different clocks) into `rcfd-surface-series`
//! documents, written as JSON text and parsed back. B1 (D1, heavy), B1c (D1c) and B2: the entering amplitudes and the
//! returning share measured in the duct at the inlet; B3, B3r: the split drive's state and amplitudes against the closed
//! form; B4 (D2): the split-driven run against the run driven by the closed-form entering wave; E1, E0, N1: the
//! single-section residual and the single-section error; M1, M2: the report; R3, Q2: refusals. Every number is the
//! registered one.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, GhostState, Medium, WaveBc, WaveSponge};
use rcfd_fv::sponge::{rate_max_for_reflection, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_post::duct_modes::{
    axial_wavenumber, decompose_section, fit_axial, DuctShape, HarmonicDft, Mode, ModeBasis, C64,
};
use rcfd_post::fwh::SourceMode;
use rcfd_post::surface_series::{FaceSpec, Motion, SeriesSpec};
use rcfd_post::wave_drive::{DriveOptions, DuctFrame, SplitDrive, SurfaceDrive};
use std::f64::consts::PI;
use std::sync::Arc;

const RHO0: f64 = 1.2;
const C: f64 = 340.0;
const H: f64 = 0.12;
const AIR: Medium = Medium { rho0: RHO0, c: C };
const X_A: f64 = -0.02;
const X_B: f64 = -0.05;
const X_C: f64 = -0.08;
const MEAN: f64 = 3.0;

fn omega0() -> f64 {
    C * (PI / H) / 1.2
}

fn period0() -> f64 {
    2.0 * PI / omega0()
}

/// `(harmonic, mode, entering amplitude at x = 0, reflection factor R)`; the returning amplitude at x = 0 is `R A`.
fn components() -> [(usize, usize, C64, C64); 3] {
    [
        (1, 0, C64::cis(0.4).scale(0.6), C64::cis(0.7).scale(0.5)),
        (2, 0, C64::cis(2.0).scale(0.3), C64::cis(-0.3).scale(0.4)),
        (2, 1, C64::cis(-1.1).scale(1.0), C64::cis(1.9).scale(0.6)),
    ]
}

fn psi(m: usize, y: f64) -> f64 {
    let e = if m == 0 { 1.0 } else { 2.0f64.sqrt() };
    e * (m as f64 * PI * y / H).cos()
}

fn kz(h: usize, m: usize) -> C64 {
    axial_wavenumber(h as f64 * omega0() / C, m as f64 * PI / H)
}

fn ex(k: C64, x: f64) -> C64 {
    (C64::new(0.0, -1.0) * k).scale(x).exp()
}

/// Modal amplitude of a component at x: entering, returning (if `back`).
fn modal(h: usize, m: usize, x: f64, back: bool) -> C64 {
    let (_, _, a, r) = components().into_iter().find(|c| c.0 == h && c.1 == m).unwrap();
    let k = kz(h, m);
    let mut v = a * ex(k, x);
    if back {
        v += a * r * ex(k, -x);
    }
    v
}

/// The closed-form pressure without the mean, entering plus (if `back`) returning.
fn pressure(x: f64, y: f64, t: f64, back: bool) -> f64 {
    components()
        .iter()
        .map(|&(h, m, _, _)| (modal(h, m, x, back) * C64::cis(h as f64 * omega0() * t)).re * psi(m, y))
        .sum()
}

/// The closed-form entering wave `(p, u_x)` without the mean.
fn entering(x: f64, y: f64, t: f64) -> (f64, f64) {
    let (mut p, mut u) = (0.0, 0.0);
    for (h, m, a, _) in components() {
        let w = h as f64 * omega0();
        let k = kz(h, m);
        let v = a.scale(psi(m, y)) * ex(k, x) * C64::cis(w * t);
        p += v.re;
        u += (k * v).re / (RHO0 * w);
    }
    (p, u)
}

/// A section at `xs` with the face edges `edges`, `n` samples of T0/64 from `t0`, via JSON text.
fn section(xs: f64, edges: &[f64], t0: f64, n: usize, back: bool) -> SeriesSpec {
    let dt = period0() / 64.0;
    let faces = edges
        .windows(2)
        .map(|e| {
            let y = 0.5 * (e[0] + e[1]);
            FaceSpec {
                area: e[1] - e[0],
                x: [xs, y, 0.5],
                n: [-1.0, 0.0, 0.0],
                p: (0..n)
                    .map(|i| MEAN + pressure(xs, y, t0 + i as f64 * dt, back))
                    .collect(),
            }
        })
        .collect();
    let spec = SeriesSpec {
        rho0: RHO0,
        c0: C,
        t0,
        dt,
        mode: SourceMode::Impermeable,
        flow: [0.0; 3],
        motion: Motion::Static,
        faces,
    };
    let text = spec.to_json().to_string();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    SeriesSpec::from_json(&doc, None).unwrap()
}

fn clustered() -> Vec<f64> {
    (0..=24)
        .map(|j| {
            let s = j as f64 / 24.0;
            H * (s - 0.5 * (2.0 * PI * s).sin() / (2.0 * PI))
        })
        .collect()
}

fn uniform(n: usize) -> Vec<f64> {
    (0..=n).map(|j| j as f64 * H / n as f64).collect()
}

fn section_a(back: bool) -> SeriesSpec {
    section(X_A, &clustered(), 0.0137, 128, back)
}

fn section_b_at(xs: f64, back: bool) -> SeriesSpec {
    section(xs, &uniform(20), 0.0211, 192, back)
}

fn section_b(back: bool) -> SeriesSpec {
    section_b_at(X_B, back)
}

fn section_c(back: bool) -> SeriesSpec {
    section(X_C, &uniform(16), 0.0137, 128, back)
}

fn frame() -> DuctFrame {
    DuctFrame {
        origin: [X_A, 0.0, 0.0],
        axis: [1.0, 0.0, 0.0],
        e1: [0.0, 1.0, 0.0],
        e2: [0.0, 0.0, 1.0],
    }
}

fn basis() -> ModeBasis {
    ModeBasis::new(
        DuctShape::Rectangular { a: H, b: 1.0 },
        &(0..8).map(|m| Mode { m, n: 0 }).collect::<Vec<_>>(),
    )
    .unwrap()
}

fn options() -> DriveOptions {
    DriveOptions {
        frequency: omega0() / (2.0 * PI),
        max_harmonic: 2,
        ramp_time: 4.0 * period0(),
    }
}

fn split(specs: &[SeriesSpec]) -> rcfd_core::Result<SplitDrive> {
    SurfaceDrive::from_sections(specs, AIR, frame(), basis(), options())
}

fn split_ab() -> SplitDrive {
    split(&[section_a(true), section_b(true)]).unwrap()
}

fn single(spec: &SeriesSpec) -> SurfaceDrive {
    SurfaceDrive::from_series(spec, AIR, frame(), basis(), options()).unwrap()
}

/// A drive's amplitude of a component carried from its plane (axial 0 of the frame, x = X_A) to the inlet x = 0.
fn at_inlet(d: &SurfaceDrive, h: usize, m: usize) -> C64 {
    d.harmonics[h - 1].amplitudes[m] * ex(kz(h, m), -X_A)
}

fn closed_form_ghost() -> GhostState {
    let tr = 4.0 * period0();
    Arc::new(move |x: [f64; 3], t: f64| {
        let w = if t < 0.0 {
            0.0
        } else if t < tr {
            (PI * t / (2.0 * tr)).sin().powi(2)
        } else {
            1.0
        };
        let (p, u) = entering(x[0], x[1], t);
        (w * p, [w * u, 0.0, 0.0])
    })
}

struct Grid {
    n: usize,
    nx: usize,
    steps: usize,
}

fn solver(g: &Grid, ghost: GhostState) -> (AcousticWaveSolver, f64) {
    let dx = H / g.n as f64;
    let mesh = box_2d(g.nx, g.n, g.nx as f64 * dx, H, false, false).unwrap();
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let layer = SpongeLayer {
        origin: [5.0 * H, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.15,
        rate_max: rate_max_for_reflection(0.15, C, 1e-6, 2.0).unwrap(),
        exponent: 2.0,
    };
    let sponge = WaveSponge::from_layer(&geom, &layer, true).unwrap();
    let bcs = [
        ("xmin", WaveBc::Driven(ghost)),
        ("xmax", WaveBc::Wall),
        ("ymin", WaveBc::Wall),
        ("ymax", WaveBc::Wall),
    ];
    let mut s = AcousticWaveSolver::new(geom, AIR, &bcs).unwrap();
    s.set_sponge(Some(sponge)).unwrap();
    (s, 0.5 * period0() / g.steps as f64)
}

/// Run 40 periods of harmonic 2 driven by the A-B split, DFT over the last 4, fit each component at x_ref = 0.
/// Returns per component `(|plus - A| / |A|, |minus| / |R A|)`.
fn measure(g: &Grid) -> Vec<(f64, f64)> {
    let sd = split_ab();
    let (mut s, dt) = solver(g, sd.drive.ghost_state());
    let dx = H / g.n as f64;
    let cc = s.geometry().mesh.cell_centres().to_vec();
    let vol = s.geometry().mesh.cell_volumes().to_vec();
    let total = 40 * g.steps;
    let first = total - 4 * g.steps;
    let mut dfts = [
        HarmonicDft::new(omega0(), cc.len()).unwrap(),
        HarmonicDft::new(2.0 * omega0(), cc.len()).unwrap(),
    ];
    for step in 0..total {
        s.step(dt).unwrap();
        s.t = (step + 1) as f64 * dt;
        if step >= first {
            for d in dfts.iter_mut() {
                d.add(s.t, &s.p).unwrap();
            }
        }
    }
    let fields = [dfts[0].amplitudes().unwrap(), dfts[1].amplitudes().unwrap()];
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); g.nx];
    for (c, x) in cc.iter().enumerate() {
        columns[(x[0] / dx).floor() as usize].push(c);
    }
    let b = basis();
    let mut out = Vec::new();
    for (h, m, a, r) in components() {
        let mut xs = Vec::new();
        let mut amps = Vec::new();
        for (i, col) in columns.iter().enumerate() {
            let x = (i as f64 + 0.5) * dx;
            if !(0.5 * H..=5.0 * H).contains(&x) {
                continue;
            }
            let points: Vec<[f64; 2]> = col.iter().map(|&c| [cc[c][1], 0.0]).collect();
            let weights: Vec<f64> = col.iter().map(|&c| vol[c]).collect();
            let values: Vec<C64> = col.iter().map(|&c| fields[h - 1][c]).collect();
            let fit = decompose_section(&b, &points, &weights, &values).unwrap();
            xs.push(x);
            amps.push(fit.amplitudes[m]);
        }
        let fit = fit_axial(&xs, &amps, kz(h, m).re, 0.0).unwrap();
        assert!(
            !fit.at_bracket_edge,
            "({h},{m}): fitted wavenumber at its bracket edge: {:?}",
            fit.kz
        );
        let err = (fit.plus - a).abs() / a.abs();
        let back = fit.minus.abs() / (a * r).abs();
        println!(
            "({h},{m}): plus = {:.6} {:+.6} i (closed form {:.6} {:+.6} i), error {err:.3e}; |minus| / |R A| {back:.3e}; \
             kr/kz_cf - 1 = {:+.3e}; ki = {:.3e}; fit residual {:.3e}",
            fit.plus.re,
            fit.plus.im,
            a.re,
            a.im,
            fit.kz.re / kz(h, m).re - 1.0,
            -fit.kz.im,
            fit.residual
        );
        out.push((err, back));
    }
    out
}

/// B1 and B2 (ADR 0378, run D1, heavy): 64 across; every entering component within 1e-2 at the inlet; the returning share
/// at the inlet within 1e-2 of the returning wave.
#[test]
#[ignore = "heavy record (D1, 25600 cells x 12800 steps): run with --ignored"]
fn d1_b1_b2_entering_reproduced_returning_ignored() {
    let res = measure(&Grid {
        n: 64,
        nx: 400,
        steps: 320,
    });
    for (i, (err, back)) in res.iter().enumerate() {
        assert!(*err <= 1e-2, "B1 component {i}: {err:e}");
        assert!(*back <= 1e-2, "B2 component {i}: {back:e}");
    }
}

/// B1c (ADR 0378, run D1c, covering): 32 across; every entering component within 3e-2.
#[test]
fn d1c_b1c_covering_run() {
    let res = measure(&Grid {
        n: 32,
        nx: 200,
        steps: 160,
    });
    for (i, (err, _)) in res.iter().enumerate() {
        assert!(*err <= 3e-2, "B1c component {i}: {err:e}");
    }
}

/// B4 (ADR 0378, run D2): the split-driven run against the closed-form-entering-driven run, 16 x 100, 12 periods.
#[test]
fn d2_b4_split_drive_equals_closed_form_entering_drive() {
    let g = Grid {
        n: 16,
        nx: 100,
        steps: 80,
    };
    let sd = split_ab();
    let (mut a, dt) = solver(&g, sd.drive.ghost_state());
    let (mut b, _) = solver(&g, closed_form_ghost());
    for step in 0..12 * g.steps {
        a.step(dt).unwrap();
        b.step(dt).unwrap();
        a.t = (step + 1) as f64 * dt;
        b.t = a.t;
    }
    let big = b.p.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let diff = a.p.iter().zip(&b.p).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    println!(
        "B4: max |p_split - p_closed| / max |p_closed| = {:.3e} (max |p| {big:.4})",
        diff / big
    );
    assert!(big > 0.1, "B4: the run did not develop a field");
    assert!(diff / big <= 1e-9, "B4: {:e}", diff / big);
}

/// B3 and B3r (ADR 0378): the split drive's state against the closed-form entering wave at the 64 inlet face centres of D1
/// at 64 times over T0; its amplitudes at the inlet against A, relative to the returning wave.
#[test]
fn b3_b3r_split_drive_is_the_entering_wave() {
    let sd = split_ab();
    let dx = H / 64.0;
    let (mut big, mut ep, mut eu, mut et) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for j in 0..64 {
        let x = [0.0, (j as f64 + 0.5) * dx, 0.5 * dx];
        for k in 0..64 {
            let t = 0.3 + k as f64 * period0() / 64.0;
            let (p, u) = sd.drive.state(x, t);
            let (pc, uc) = entering(0.0, x[1], t);
            big = big.max(pc.abs());
            ep = ep.max((p - pc).abs());
            eu = eu.max(RHO0 * C * (u[0] - uc).abs());
            et = et.max(u[1].abs().max(u[2].abs()));
        }
    }
    println!(
        "B3: pressure {:.3e}, velocity {:.3e} (of max |p| {big:.4}); transverse {et:e}",
        ep / big,
        eu / big
    );
    assert!(ep / big <= 1e-9, "B3 pressure");
    assert!(eu / big <= 1e-9, "B3 velocity");
    assert_eq!(et, 0.0, "B3 transverse velocity");
    for (h, m, a, r) in components() {
        let e = (at_inlet(&sd.drive, h, m) - a).abs() / (a * r).abs();
        println!("B3r ({h},{m}): {e:.3e}");
        assert!(e <= 1e-2, "B3r ({h},{m}): {e:e}");
    }
}

/// The closed-form E1 residual per harmonic (ADR 0378 D4) between sections at `xa` and `xb`.
fn residual_closed_form(xa: f64, xb: f64, back: bool) -> [f64; 2] {
    let mut out = [0.0; 2];
    for (h, o) in out.iter_mut().enumerate() {
        let (mut num, mut den) = (0.0, 0.0);
        for (hh, m, _, _) in components() {
            if hh != h + 1 {
                continue;
            }
            let aa = modal(hh, m, xa, back);
            let ab = modal(hh, m, xb, back);
            num += (ab - aa * ex(kz(hh, m), xb - xa)).norm_sqr();
            den += ab.norm_sqr();
        }
        *o = (num / den).sqrt();
    }
    out
}

/// E1 and E0 (ADR 0378): the single-section residual against the check section, with and without a returning wave.
#[test]
fn e1_e0_single_section_residual() {
    let r1 = single(&section_a(true)).entering_residual(&section_b(true)).unwrap();
    let cf = residual_closed_form(X_A, X_B, true);
    println!("E1: measured {r1:?}, closed form {cf:?}");
    for h in 0..2 {
        let rel = (r1[h] - cf[h]).abs() / cf[h];
        println!("E1 harmonic {}: relative difference {rel:.3e}", h + 1);
        assert!(rel <= 1e-9, "E1 harmonic {}: {rel:e}", h + 1);
        assert!(r1[h] >= 0.1, "E1 harmonic {}: {}", h + 1, r1[h]);
    }
    let r0 = single(&section_a(false)).entering_residual(&section_b(false)).unwrap();
    println!("E0: {r0:?}");
    for (h, r) in r0.iter().enumerate() {
        assert!(*r <= 1e-12, "E0 harmonic {}: {r:e}", h + 1);
    }
}

/// N1 (ADR 0378, negative control): the single-section drive of the two-way field is off by |R| at the inlet.
#[test]
fn n1_single_section_takes_the_returning_wave() {
    let d = single(&section_a(true));
    for (h, m, a, r) in components() {
        let e = (at_inlet(&d, h, m) - a).abs() / a.abs();
        println!("N1 ({h},{m}): error {e:.12}, |R| {:.12}", r.abs());
        assert!((e - r.abs()).abs() <= 1e-9, "N1 ({h},{m}): {e} against {}", r.abs());
    }
}

/// M1 and M2 (ADR 0378): the report of the split, and three sections against two.
#[test]
fn m1_m2_report_and_three_sections() {
    let sd = split_ab();
    let refl_cf = [
        0.5,
        ((0.4f64 * 0.3).powi(2) + 0.6f64.powi(2)).sqrt() / (0.3f64.powi(2) + 1.0).sqrt(),
    ];
    let alpha = ((PI / H).powi(2) - (omega0() / C).powi(2)).sqrt();
    let sep_cf = (alpha * (X_A - X_B)).tanh();
    println!(
        "M1: reflection {:?} (closed form {refl_cf:?}); min separation {:.12} (closed form {sep_cf:.12}); split residual {:?}",
        sd.reflection, sd.min_separation, sd.split_residual
    );
    for h in 0..2 {
        assert!((sd.reflection[h] / refl_cf[h] - 1.0).abs() <= 1e-9, "M1 reflection {h}");
        assert!(sd.split_residual[h] <= 1e-12, "M1 split residual {h}");
    }
    assert!((sd.min_separation / sep_cf - 1.0).abs() <= 1e-9, "M1 min separation");
    let s3 = split(&[section_a(true), section_b(true), section_c(true)]).unwrap();
    let mut worst = 0.0f64;
    for (h2, h3) in sd.drive.harmonics.iter().zip(&s3.drive.harmonics) {
        let big = h2.amplitudes.iter().fold(0.0f64, |m, a| m.max(a.abs()));
        for (a2, a3) in h2.amplitudes.iter().zip(&h3.amplitudes) {
            worst = worst.max((*a3 - *a2).abs() / big);
        }
    }
    println!(
        "M2: three against two sections {worst:.3e}; split residual {:?}",
        s3.split_residual
    );
    assert!(worst <= 1e-9, "M2 amplitudes");
    for (h, r) in s3.split_residual.iter().enumerate() {
        assert!(*r <= 1e-12, "M2 split residual {h}: {r:e}");
    }
}

/// R3 (ADR 0378): the refusals of the split.
#[test]
fn r3_refusals() {
    let a = section_a(true);
    let e = split(std::slice::from_ref(&a)).expect_err("R3 one section accepted");
    println!("R3 one section: {e}");
    let e = split(&[a.clone(), a.clone()]).expect_err("R3 one plane accepted");
    println!("R3 one plane: {e}");
    let e = split(&[a.clone(), section_b_at(X_A - 0.072, true)]).expect_err("R3 kz d = pi accepted");
    let msg = e.to_string();
    println!("R3 kz d = pi: {msg}");
    assert!(msg.contains("harmonic 2 mode (0, 0)"), "R3 kz d = pi: {msg}");
    let mut b = section_b(true);
    b.c0 = 341.0;
    let e = split(&[a, b]).expect_err("R3 c0 accepted");
    let msg = e.to_string();
    println!("R3 c0: {msg}");
    assert!(msg.contains("'sections[1]/"), "R3 c0: {msg}");
}

/// Q2 (ADR 0378 D6): the off-plane refusal names the farthest face.
#[test]
fn q2_off_plane_names_the_farthest_face() {
    let mut s = section_a(false);
    s.faces[5].x[0] += 1e-3 * H;
    let e = SurfaceDrive::from_series(&s, AIR, frame(), basis(), options()).expect_err("Q2 accepted");
    let msg = e.to_string();
    println!("Q2: {msg}");
    assert!(msg.contains("'$.faces[5].x'"), "Q2: {msg}");
}
