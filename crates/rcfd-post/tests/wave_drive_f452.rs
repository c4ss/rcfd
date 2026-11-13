//! F-452 (ADR 0372): the acoustic wave solver driven by a sampled surface. A closed-form channel field (the plane wave at
//! the fundamental, the plane wave and mode 1 at the second harmonic, a 3 Pa mean) is sampled on a non-uniform section
//! 0.02 m upstream of the wave mesh into an `rcfd-surface-series` document, written as JSON text and parsed back; the drive is
//! built from the parsed document alone. A1 (D1, heavy) and A1c (D1c): the modal amplitudes measured in the duct at the
//! inlet against the closed form; A2 (D2): the file-driven run against the run driven by the closed form; A3: the drive's
//! state against the closed form; R0, R1, R2: acceptance and refusals; M0: the report. Every number is the registered one.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, GhostState, Medium, WaveBc, WaveSponge};
use rcfd_fv::sponge::{rate_max_for_reflection, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_post::duct_modes::{
    axial_wavenumber, decompose_section, fit_axial, DuctShape, HarmonicDft, Mode, ModeBasis, C64,
};
use rcfd_post::fwh::SourceMode;
use rcfd_post::surface_series::{FaceSpec, Motion, SeriesSpec};
use rcfd_post::wave_drive::{DriveOptions, DuctFrame, SurfaceDrive};
use std::f64::consts::PI;
use std::sync::Arc;

const RHO0: f64 = 1.2;
const C: f64 = 340.0;
const H: f64 = 0.12;
const AIR: Medium = Medium { rho0: RHO0, c: C };
const X_S: f64 = -0.02;
const T0_FILE: f64 = 0.0137;
const MEAN: f64 = 3.0;

/// The fundamental angular frequency: mode 1 propagates at harmonic 2 with cos theta = 0.8.
fn omega0() -> f64 {
    C * (PI / H) / 1.2
}

fn period0() -> f64 {
    2.0 * PI / omega0()
}

/// The components `(harmonic, mode, amplitude at x = 0)`.
fn components() -> [(usize, usize, C64); 3] {
    [
        (1, 0, C64::cis(0.4).scale(0.6)),
        (2, 0, C64::cis(2.0).scale(0.3)),
        (2, 1, C64::cis(-1.1).scale(1.0)),
    ]
}

fn psi(m: usize, y: f64) -> f64 {
    let e = if m == 0 { 1.0 } else { 2.0f64.sqrt() };
    e * (m as f64 * PI * y / H).cos()
}

fn kz(h: usize, m: usize) -> C64 {
    axial_wavenumber(h as f64 * omega0() / C, m as f64 * PI / H)
}

/// The closed-form entering field without the mean: `(p, u_x)` at `(x, y)` and `t`.
fn closed_form(x: f64, y: f64, t: f64) -> (f64, f64) {
    let (mut p, mut u) = (0.0, 0.0);
    for (h, m, a) in components() {
        let w = h as f64 * omega0();
        let k = kz(h, m);
        let e = ((C64::new(0.0, -1.0) * k).scale(x) + C64::new(0.0, w * t)).exp();
        let v = a.scale(psi(m, y)) * e;
        p += v.re;
        u += (k * v).re / (RHO0 * w);
    }
    (p, u)
}

/// The file: 24 faces of a wall-clustered section at x = X_S, `n` samples of T0/64 from T0_FILE, via JSON text.
fn series(n: usize) -> SeriesSpec {
    let dt = period0() / 64.0;
    let edges: Vec<f64> = (0..=24)
        .map(|j| {
            let s = j as f64 / 24.0;
            H * (s - 0.5 * (2.0 * PI * s).sin() / (2.0 * PI))
        })
        .collect();
    let faces = (0..24)
        .map(|j| {
            let y = 0.5 * (edges[j] + edges[j + 1]);
            FaceSpec {
                area: edges[j + 1] - edges[j],
                x: [X_S, y, 0.5],
                n: [-1.0, 0.0, 0.0],
                p: (0..n)
                    .map(|i| MEAN + closed_form(X_S, y, T0_FILE + i as f64 * dt).0)
                    .collect(),
            }
        })
        .collect();
    let spec = SeriesSpec {
        rho0: RHO0,
        c0: C,
        t0: T0_FILE,
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

fn frame() -> DuctFrame {
    DuctFrame {
        origin: [X_S, 0.0, 0.0],
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

fn drive_from(spec: &SeriesSpec) -> rcfd_core::Result<SurfaceDrive> {
    SurfaceDrive::from_series(spec, AIR, frame(), basis(), options())
}

/// The closed-form entering wave as a ghost state, with the drive's ramp (the F-333 way).
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
        let (p, u) = closed_form(x[0], x[1], t);
        (w * p, [w * u, 0.0, 0.0])
    })
}

struct Grid {
    n: usize,
    nx: usize,
    /// Steps per period of harmonic 2.
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

/// Run 40 periods of harmonic 2, DFT over the last 4 at both harmonics, fit each driven component at x_ref = 0.
/// Returns per component `(error against the closed form, |minus/plus|, plus, fitted kz)`.
fn measure(g: &Grid) -> Vec<(f64, f64, C64, C64)> {
    let drive = drive_from(&series(128)).unwrap();
    let (mut s, dt) = solver(g, drive.ghost_state());
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
    let b = ModeBasis::new(
        DuctShape::Rectangular { a: H, b: 1.0 },
        &(0..8).map(|m| Mode { m, n: 0 }).collect::<Vec<_>>(),
    )
    .unwrap();
    let mut out = Vec::new();
    for (h, m, a) in components() {
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
        let r = (fit.minus / fit.plus).abs();
        println!(
            "({h},{m}): plus = {:.6} {:+.6} i (closed form {:.6} {:+.6} i), error {err:.3e}; |minus/plus| {r:.3e}; \
             kr/kz_cf - 1 = {:+.3e}; ki = {:.3e}; fit residual {:.3e}",
            fit.plus.re,
            fit.plus.im,
            a.re,
            a.im,
            fit.kz.re / kz(h, m).re - 1.0,
            -fit.kz.im,
            fit.residual
        );
        out.push((err, r, fit.plus, fit.kz));
    }
    out
}

/// A1 and A2m (ADR 0372, run D1, heavy): 64 across; every component within 1e-2 at the inlet; |minus/plus| < 1e-2.
#[test]
#[ignore = "heavy record (D1, 25600 cells x 12800 steps): run with --ignored"]
fn d1_a1_modal_amplitudes_reproduced_at_the_inlet() {
    let res = measure(&Grid {
        n: 64,
        nx: 400,
        steps: 320,
    });
    for (i, (err, r, _, _)) in res.iter().enumerate() {
        assert!(*err <= 1e-2, "A1 component {i}: {err:e}");
        assert!(*r < 1e-2, "A2m component {i}: {r:e}");
    }
}

/// A1c (ADR 0372, run D1c, covering): 32 across; every component within 3e-2.
#[test]
fn d1c_a1c_covering_run() {
    let res = measure(&Grid {
        n: 32,
        nx: 200,
        steps: 160,
    });
    for (i, (err, _, _, _)) in res.iter().enumerate() {
        assert!(*err <= 3e-2, "A1c component {i}: {err:e}");
    }
}

/// A2 (ADR 0372, run D2): the file-driven run against the closed-form-driven run, 16 x 100, 12 periods of harmonic 2:
/// max |p_file - p_closed| / max |p_closed| <= 1e-9.
#[test]
fn d2_a2_file_drive_equals_closed_form_drive() {
    let g = Grid {
        n: 16,
        nx: 100,
        steps: 80,
    };
    let drive = drive_from(&series(128)).unwrap();
    let (mut a, dt) = solver(&g, drive.ghost_state());
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
        "A2: max |p_file - p_closed| / max |p_closed| = {:.3e} (max |p| {big:.4})",
        diff / big
    );
    assert!(big > 0.1, "A2: the run did not develop a field");
    assert!(diff / big <= 1e-9, "A2: {:e}", diff / big);
}

/// A3 (ADR 0372): the drive's state against the closed form at the 64 inlet face centres of D1 at 64 times over T0.
#[test]
fn a3_drive_state_equals_closed_form() {
    let drive = drive_from(&series(128)).unwrap();
    let dx = H / 64.0;
    let (mut big, mut ep, mut eu, mut et) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for j in 0..64 {
        let x = [0.0, (j as f64 + 0.5) * dx, 0.5 * dx];
        for k in 0..64 {
            let t = 0.3 + k as f64 * period0() / 64.0;
            let (p, u) = drive.state(x, t);
            let (pc, uc) = closed_form(0.0, x[1], t);
            big = big.max(pc.abs());
            ep = ep.max((p - pc).abs());
            eu = eu.max(RHO0 * C * (u[0] - uc).abs());
            et = et.max(u[1].abs().max(u[2].abs()));
        }
    }
    println!(
        "A3: pressure {:.3e}, velocity {:.3e} (of max |p| {big:.4}); transverse {et:e}",
        ep / big,
        eu / big
    );
    assert!(ep / big <= 1e-9, "A3 pressure");
    assert!(eu / big <= 1e-9, "A3 velocity");
    assert_eq!(et, 0.0, "A3 transverse velocity");
}

/// M0 (ADR 0372): the report of the drive: mean 3.0, two periods, nothing unrepresented.
#[test]
fn m0_report() {
    let drive = drive_from(&series(128)).unwrap();
    println!(
        "M0: mean {:.15}, periods {}, unrepresented {:.3e}, section residuals {:?}",
        drive.mean,
        drive.periods,
        drive.unrepresented,
        drive.harmonics.iter().map(|h| h.residual).collect::<Vec<_>>()
    );
    assert!((drive.mean - MEAN).abs() <= 1e-12, "M0 mean");
    assert_eq!(drive.periods, 2, "M0 periods");
    assert!(drive.unrepresented <= 1e-12, "M0 unrepresented");
}

/// R0 and R1 (ADR 0372): the whole two periods are accepted; 127 and 96 samples are refused, naming the periods.
#[test]
fn r0_r1_whole_periods_only() {
    let spec = series(128);
    assert_eq!(drive_from(&spec).unwrap().periods, 2, "R0");
    for n in [127, 96] {
        let mut s = spec.clone();
        for f in s.faces.iter_mut() {
            f.p.truncate(n);
        }
        let e = drive_from(&s).expect_err("R1: a series of a non-whole number of periods was accepted");
        let msg = e.to_string();
        println!("R1 ({n} samples): {msg}");
        assert!(msg.contains("not a whole number of periods"), "R1 message: {msg}");
    }
}

/// R2 (ADR 0372): the other refusals.
#[test]
fn r2_refusals() {
    let spec = series(128);
    let check = |name: &str, r: rcfd_core::Result<SurfaceDrive>| {
        let e = r.err().unwrap_or_else(|| panic!("R2 {name}: accepted"));
        println!("R2 {name}: {e}");
    };
    let with = |f: &dyn Fn(&mut SeriesSpec)| {
        let mut s = spec.clone();
        f(&mut s);
        s
    };
    let opts = |o: DriveOptions| SurfaceDrive::from_series(&spec, AIR, frame(), basis(), o);
    check(
        "Nyquist",
        opts(DriveOptions {
            max_harmonic: 32,
            ..options()
        }),
    );
    check("c0", drive_from(&with(&|s| s.c0 = 341.0)));
    check("off plane", drive_from(&with(&|s| s.faces[5].x[0] += 1e-3 * H)));
    check("outside", drive_from(&with(&|s| s.faces[5].x[1] = 1.01 * H)));
    check("normal", drive_from(&with(&|s| s.faces[5].n = [0.0, 1.0, 0.0])));
    check("flow", drive_from(&with(&|s| s.flow = [1.0, 0.0, 0.0])));
    check(
        "motion",
        drive_from(&with(&|s| {
            s.motion = Motion::Rotation {
                axis: [0.0, 0.0, 1.0],
                origin: [0.0; 3],
                omega: 1.0,
                angle0: 0.0,
                velocity: [0.0; 3],
            }
        })),
    );
    check(
        "frequency",
        opts(DriveOptions {
            frequency: 0.0,
            ..options()
        }),
    );
    check(
        "no harmonic",
        opts(DriveOptions {
            max_harmonic: 0,
            ..options()
        }),
    );
    let big = ModeBasis::new(
        DuctShape::Rectangular { a: H, b: 1.0 },
        &(0..30).map(|m| Mode { m, n: 0 }).collect::<Vec<_>>(),
    )
    .unwrap();
    check("basis", SurfaceDrive::from_series(&spec, AIR, frame(), big, options()));
    check(
        "frame",
        SurfaceDrive::from_series(
            &spec,
            AIR,
            DuctFrame {
                e1: [0.6, 0.8, 0.0],
                ..frame()
            },
            basis(),
            options(),
        ),
    );
}
