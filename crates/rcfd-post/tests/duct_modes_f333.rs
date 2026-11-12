//! F-333 (ADR 0351): a hard-walled channel driven with one mode and solved by the linear acoustic wave solver of rcfd-fv,
//! measured by the modal decomposition of rcfd-post. S1 (mode 1, matched layer, rigid end): K1 axial wavenumber, K1b loss
//! per wavelength, S1 reflection; S1c the same on the coarse grid; P1 (plane wave, plain layer): K0, P1; M1 the open end
//! against (1 - cos)/(1 + cos); NC1 the rigid end without a layer; W1 the S1 set-up on a warped mesh; E1 the decay rate of
//! a cut-off mode. X1 and X2 the export and recovery of the modal amplitudes of synthetic fields. Every set-up number and
//! limit is the registered one of ADR 0351.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, Medium, WaveBc, WaveSponge};
use rcfd_fv::sponge::{rate_max_for_reflection, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_2d, warp};
use rcfd_post::duct_modes::{
    axial_wavenumber, decompose_section, fit_axial, fit_exponential, split_axial, AxialFit, DuctShape, HarmonicDft,
    ModalExport, Mode, ModeBasis, ModeValues, C64,
};
use std::f64::consts::PI;
use std::sync::Arc;

const RHO0: f64 = 1.2;
const C: f64 = 340.0;
const H: f64 = 0.12;
const AIR: Medium = Medium { rho0: RHO0, c: C };

/// The driven frequency: mode 1 at cos(theta) = 0.8.
fn omega() -> f64 {
    C * (PI / H) / 0.6
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Layer {
    None,
    Plain,
    Matched,
}

#[derive(Clone, Copy, Debug)]
struct Run {
    /// Cells across the height.
    n: usize,
    /// Cells along.
    nx: usize,
    /// Driven mode.
    mode: usize,
    /// End wall (true) or open end.
    wall_end: bool,
    layer: Layer,
    warped: bool,
    steps_per_period: usize,
    ramp_periods: f64,
    periods: usize,
    dft_periods: usize,
}

const S1: Run = Run {
    n: 32,
    nx: 200,
    mode: 1,
    wall_end: true,
    layer: Layer::Matched,
    warped: false,
    steps_per_period: 160,
    ramp_periods: 8.0,
    periods: 40,
    dft_periods: 4,
};

/// The time-harmonic state of a run: station positions and, per station, the amplitudes of the channel modes 0..7.
struct Measured {
    xs: Vec<f64>,
    amps: Vec<Vec<C64>>,
    max_rate_dt: f64,
    dt_over_max: f64,
}

fn psi(m: usize, y: f64) -> f64 {
    let e = if m == 0 { 1.0 } else { 2.0f64.sqrt() };
    e * (m as f64 * PI * y / H).cos()
}

fn run(r: Run) -> Measured {
    let dx = H / r.n as f64;
    let length = r.nx as f64 * dx;
    let mesh = box_2d(r.nx, r.n, length, H, false, false).unwrap();
    let mesh = if r.warped {
        warp(&mesh, |p| {
            [
                p[0],
                p[1] + 0.05 * H * (2.0 * PI * p[0] / H).sin() * (PI * p[1] / H).sin(),
                p[2],
            ]
        })
        .unwrap()
    } else {
        mesh
    };
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let w = omega();
    let k = w / C;
    let k_cut = r.mode as f64 * PI / H;
    let kz = axial_wavenumber(k, k_cut);
    let t_ramp = r.ramp_periods * 2.0 * PI / w;
    let mode = r.mode;
    let drive = move |x: [f64; 3], t: f64| -> (f64, [f64; 3]) {
        let ramp = if t < t_ramp {
            (PI * t / (2.0 * t_ramp)).sin().powi(2)
        } else {
            1.0
        };
        let a = ramp * psi(mode, x[1]);
        let p = a * (w * t).cos();
        let ux = if kz.im == 0.0 {
            kz.re / (RHO0 * w) * p
        } else {
            -kz.im / (RHO0 * w) * a * (w * t).sin()
        };
        (p, [ux, 0.0, 0.0])
    };
    let end = if r.wall_end { WaveBc::Wall } else { WaveBc::Open };
    let bcs = [
        ("xmin", WaveBc::Driven(Arc::new(drive))),
        ("xmax", end),
        ("ymin", WaveBc::Wall),
        ("ymax", WaveBc::Wall),
    ];
    let layer = SpongeLayer {
        origin: [5.0 * H, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.15,
        rate_max: rate_max_for_reflection(0.15, C, 1e-6, 2.0).unwrap(),
        exponent: 2.0,
    };
    let sponge = match r.layer {
        Layer::None => None,
        Layer::Plain => Some(WaveSponge::from_layer(&geom, &layer, false).unwrap()),
        Layer::Matched => Some(WaveSponge::from_layer(&geom, &layer, true).unwrap()),
    };
    let max_rate = sponge
        .as_ref()
        .map_or(0.0, |s| s.rate.iter().cloned().fold(0.0, f64::max));
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let mut s = AcousticWaveSolver::new(geom, AIR, &bcs).unwrap();
    s.set_sponge(sponge).unwrap();
    let period = 2.0 * PI / w;
    let dt = period / r.steps_per_period as f64;
    let dt_over_max = dt / s.max_dt(0.48);
    let total = r.periods * r.steps_per_period;
    let first_dft = total - r.dft_periods * r.steps_per_period;
    let mut dft = HarmonicDft::new(w, cc.len()).unwrap();
    for step in 0..total {
        // The time from the step count, so that the DFT samples are uniform.
        s.step(dt).unwrap();
        s.t = (step + 1) as f64 * dt;
        if step >= first_dft {
            dft.add(s.t, &s.p).unwrap();
        }
    }
    let field = dft.amplitudes().unwrap();
    // Stations: the columns of cells (x is not warped).
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); r.nx];
    for (c, x) in cc.iter().enumerate() {
        columns[(x[0] / dx).floor() as usize].push(c);
    }
    let basis = ModeBasis::new(
        DuctShape::Rectangular { a: H, b: 1.0 },
        &(0..8).map(|m| Mode { m, n: 0 }).collect::<Vec<_>>(),
    )
    .unwrap();
    let mut xs = Vec::new();
    let mut amps = Vec::new();
    for (i, col) in columns.iter().enumerate() {
        let points: Vec<[f64; 2]> = col.iter().map(|&c| [cc[c][1], 0.0]).collect();
        let weights: Vec<f64> = col.iter().map(|&c| vol[c]).collect();
        let values: Vec<C64> = col.iter().map(|&c| field[c]).collect();
        let fit = decompose_section(&basis, &points, &weights, &values).unwrap();
        xs.push((i as f64 + 0.5) * dx);
        amps.push(fit.amplitudes);
    }
    Measured {
        xs,
        amps,
        max_rate_dt: max_rate * dt,
        dt_over_max,
    }
}

/// The stations whose centre lies in `[lo, hi]` and the amplitude of `mode` there.
fn stations(m: &Measured, mode: usize, lo: f64, hi: f64) -> (Vec<f64>, Vec<C64>) {
    let mut xs = Vec::new();
    let mut a = Vec::new();
    for (i, &x) in m.xs.iter().enumerate() {
        if (lo..=hi).contains(&x) {
            xs.push(x);
            a.push(m.amps[i][mode]);
        }
    }
    (xs, a)
}

struct Waves {
    fit: AxialFit,
    kr: f64,
    loss: f64,
    r: f64,
    other_modes: f64,
}

fn waves(m: &Measured, mode: usize, kz_cf: f64, x_ref: f64) -> Waves {
    let (xs, a) = stations(m, mode, 0.5 * H, 5.0 * H);
    let fit = fit_axial(&xs, &a, kz_cf, x_ref).unwrap();
    assert!(
        !fit.at_bracket_edge,
        "the fitted wavenumber lies at its bracket edge: {:?}",
        fit.kz
    );
    let kr = fit.kz.re;
    let ki = -fit.kz.im;
    // Largest amplitude of the modes not driven, relative to the driven one, in the station region.
    let mut other: f64 = 0.0;
    let mut driven: f64 = 0.0;
    for (i, &x) in m.xs.iter().enumerate() {
        if (0.5 * H..=5.0 * H).contains(&x) {
            for q in 0..8 {
                if q == mode {
                    driven = driven.max(m.amps[i][q].abs());
                } else {
                    other = other.max(m.amps[i][q].abs());
                }
            }
        }
    }
    Waves {
        fit,
        kr,
        loss: 1.0 - (-2.0 * PI * ki / kr).exp(),
        r: (fit.minus / fit.plus).abs(),
        other_modes: other / driven,
    }
}

fn kz1() -> f64 {
    axial_wavenumber(omega() / C, PI / H).re
}

fn report(name: &str, m: &Measured, w: &Waves, kz_cf: f64) {
    println!(
        "{name}: kr = {:.6} (closed form {kz_cf:.6}, rel {:+.3e}); ki = {:.3e}; loss per wavelength {:.3e}; R = {:.3e}; \
         |plus| = {:.4}; fit residual {:.3e}; other modes {:.3e}; rate_max dt {:.3}; dt / max_dt(0.48) {:.4}",
        w.kr,
        w.kr / kz_cf - 1.0,
        -w.fit.kz.im,
        w.loss,
        w.r,
        w.fit.plus.abs(),
        w.fit.residual,
        w.other_modes,
        m.max_rate_dt,
        m.dt_over_max
    );
}

/// S1 (ADR 0351): mode 1 into a matched layer with a rigid end. K1 |kr/kz - 1| <= 1e-2; K1b loss per wavelength <= 1e-2;
/// S1 R < 1e-2.
#[test]
fn s1_mode_one_matched_layer_rigid_end() {
    let m = run(S1);
    let kz = kz1();
    let w = waves(&m, 1, kz, 5.0 * H);
    report("S1", &m, &w, kz);
    assert!((w.kr / kz - 1.0).abs() <= 1e-2, "K1");
    assert!(w.loss <= 1e-2, "K1b");
    assert!(w.r < 1e-2, "S1");
}

/// S1c (ADR 0351): the S1 set-up on the coarse grid (16 across, 100 along, 80 steps per period): |kr/kz - 1| <= 3e-2,
/// R < 1e-2.
#[test]
fn s1c_coarse_grid() {
    let m = run(Run {
        n: 16,
        nx: 100,
        steps_per_period: 80,
        ..S1
    });
    let kz = kz1();
    let w = waves(&m, 1, kz, 5.0 * H);
    report("S1c", &m, &w, kz);
    assert!((w.kr / kz - 1.0).abs() <= 3e-2, "S1c wavenumber");
    assert!(w.r < 1e-2, "S1c reflection");
}

/// P1 (ADR 0351): the plane wave into a plain layer with a rigid end. K0 |kr/k - 1| <= 1e-2; P1 R < 1e-2.
#[test]
fn p1_plane_wave_plain_layer() {
    let m = run(Run {
        mode: 0,
        layer: Layer::Plain,
        ..S1
    });
    let k = omega() / C;
    let w = waves(&m, 0, k, 5.0 * H);
    report("P1", &m, &w, k);
    assert!((w.kr / k - 1.0).abs() <= 1e-2, "K0");
    assert!(w.r < 1e-2, "P1");
}

/// M1 (ADR 0351): mode 1 against the open end without a layer: |R - 1/9| <= 2e-2.
#[test]
fn m1_open_end_reflects_the_closed_form() {
    let m = run(Run {
        wall_end: false,
        layer: Layer::None,
        ..S1
    });
    let kz = kz1();
    let w = waves(&m, 1, kz, 6.25 * H);
    report("M1", &m, &w, kz);
    assert!((w.r - 1.0 / 9.0).abs() <= 2e-2, "M1");
}

/// NC1 (ADR 0351): mode 1 against the rigid end without a layer: R >= 0.9.
#[test]
fn nc1_rigid_end_reflects() {
    let m = run(Run {
        layer: Layer::None,
        ..S1
    });
    let kz = kz1();
    let w = waves(&m, 1, kz, 6.25 * H);
    report("NC1", &m, &w, kz);
    assert!(w.r >= 0.9, "NC1");
}

/// W1 (ADR 0351): the S1 set-up on the warped mesh. W1k |kr/kz - 1| <= 1e-2; W1s R < 1e-2.
#[test]
fn w1_warped_mesh() {
    let m = run(Run { warped: true, ..S1 });
    let kz = kz1();
    let w = waves(&m, 1, kz, 5.0 * H);
    report("W1", &m, &w, kz);
    assert!((w.kr / kz - 1.0).abs() <= 1e-2, "W1k");
    assert!(w.r < 1e-2, "W1s");
}

/// E1 (ADR 0351): the cut-off mode 2 (N = 64, 3 h, rigid end): |alpha/alpha_cf - 1| <= 1e-2.
#[test]
fn e1_cut_off_mode_decays_at_the_closed_form_rate() {
    let m = run(Run {
        n: 64,
        nx: 192,
        mode: 2,
        wall_end: true,
        layer: Layer::None,
        warped: false,
        steps_per_period: 320,
        ramp_periods: 16.0,
        periods: 32,
        dft_periods: 8,
    });
    let alpha_cf = -axial_wavenumber(omega() / C, 2.0 * PI / H).im;
    let (xs, a) = stations(&m, 2, 0.25 * H, 1.25 * H);
    let (alpha, kr) = fit_exponential(&xs, &a).unwrap();
    println!(
        "E1: alpha = {alpha:.6} (closed form {alpha_cf:.6}, rel {:+.3e}); phase slope {kr:.3e}; {} stations; |A| from {:.4} to {:.4e}",
        alpha / alpha_cf - 1.0,
        xs.len(),
        a[0].abs(),
        a[a.len() - 1].abs()
    );
    assert!((alpha / alpha_cf - 1.0).abs() <= 1e-2, "E1");
}

/// The measure of X1 and X2: |a_rec - a|/|a| for a != 0, |a_rec|/max|a| for a = 0.
fn recovery_error(truth: &[C64], got: &[C64]) -> f64 {
    let big = truth.iter().map(|a| a.abs()).fold(0.0, f64::max);
    truth
        .iter()
        .zip(got)
        .map(|(t, g)| {
            if t.abs() > 0.0 {
                (*g - *t).abs() / t.abs()
            } else {
                g.abs() / big
            }
        })
        .fold(0.0, f64::max)
}

/// X1 (ADR 0351): rectangular duct 0.12 x 0.08 m, k = 60 1/m; the five propagating modes in both directions and the cut-off
/// (2,1) towards +x, sampled at 7 stations as time series; DFT, section fit, axial split, export, read back: every
/// amplitude within 1e-2.
#[test]
fn x1_rectangular_chain_export_and_recovery() {
    let shape = DuctShape::Rectangular { a: 0.12, b: 0.08 };
    let k = 60.0;
    let w = k * C;
    let basis = ModeBasis::below(shape, 80.0).unwrap();
    let expect = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (2, 1), (0, 2), (3, 0)];
    assert_eq!(basis.len(), 8, "X1 basis {:?}", basis.modes);
    for (m, n) in expect {
        assert!(basis.modes.contains(&Mode { m, n }), "X1 basis lacks ({m}, {n})");
    }
    // Synthetic waves (plus, minus) at x = 0 for every basis mode, in the order of `expect`.
    let truth_of = |md: Mode| -> (C64, C64) {
        let c = |r: f64, a: f64| C64::cis(a).scale(r);
        match (md.m, md.n) {
            (0, 0) => (c(1.0, 0.0), c(0.4, 1.1)),
            (1, 0) => (c(0.8, -0.7), c(0.3, 2.0)),
            (0, 1) => (c(0.6, 2.5), c(0.25, -1.4)),
            (1, 1) => (c(0.5, 0.9), c(0.2, 0.3)),
            (2, 0) => (c(0.35, -2.2), C64::default()),
            (2, 1) => (c(0.3, 1.7), C64::default()),
            _ => (C64::default(), C64::default()),
        }
    };
    let truth: Vec<(C64, C64)> = basis.modes.iter().map(|&md| truth_of(md)).collect();
    let kz: Vec<C64> = basis.k_cut.iter().map(|&kc| axial_wavenumber(k, kc)).collect();
    let (ny, nz) = (24, 16);
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for i in 0..ny {
        for j in 0..nz {
            points.push([0.12 * (i as f64 + 0.5) / ny as f64, 0.08 * (j as f64 + 0.5) / nz as f64]);
            weights.push(0.12 / ny as f64 * 0.08 / nz as f64);
        }
    }
    let xs: Vec<f64> = (0..7).map(|s| 0.05 * s as f64).collect();
    let period = 2.0 * PI / w;
    // Complex field per station and point, then real time series and the DFT.
    let mut station_amps: Vec<Vec<C64>> = vec![Vec::new(); basis.len()];
    let mut residual: f64 = 0.0;
    for &x in &xs {
        let field: Vec<C64> = points
            .iter()
            .map(|&pt| {
                let mut f = C64::default();
                for q in 0..basis.len() {
                    let (ap, am) = truth[q];
                    let ep = (C64::new(0.0, -1.0) * kz[q]).scale(x).exp();
                    let em = (C64::new(0.0, 1.0) * kz[q]).scale(x).exp();
                    f += (ap * ep + am * em) * basis.eval(q, pt);
                }
                f
            })
            .collect();
        let mut dft = HarmonicDft::new(w, points.len()).unwrap();
        for n in 0..128 {
            let t = n as f64 * period / 64.0;
            let e = C64::cis(w * t);
            let values: Vec<f64> = field.iter().map(|f| (*f * e).re).collect();
            dft.add(t, &values).unwrap();
        }
        let fit = decompose_section(&basis, &points, &weights, &dft.amplitudes().unwrap()).unwrap();
        residual = residual.max(fit.residual);
        for q in 0..basis.len() {
            station_amps[q].push(fit.amplitudes[q]);
        }
    }
    let pairs: Vec<(C64, C64)> = (0..basis.len())
        .map(|q| {
            let f = split_axial(&xs, &station_amps[q], kz[q], 0.0).unwrap();
            (f.plus, f.minus)
        })
        .collect();
    let export = ModalExport::axial(&basis, w / (2.0 * PI), C, RHO0, 0.0, &pairs, residual).unwrap();
    let text = export.to_json_string();
    let back = ModalExport::from_json_str(&text).unwrap();
    let mut t_all = Vec::new();
    let mut g_all = Vec::new();
    for (q, rec) in back.modes.iter().enumerate() {
        assert_eq!(rec.mode, basis.modes[q]);
        let ModeValues::Axial { plus, minus, .. } = rec.values else {
            panic!("X1: not an axial record")
        };
        t_all.extend([truth[q].0, truth[q].1]);
        g_all.extend([plus, minus]);
    }
    let err = recovery_error(&t_all, &g_all);
    println!(
        "X1: worst recovery error {err:.3e} over {} amplitudes; section residual {residual:.3e}; {} bytes",
        t_all.len(),
        text.len()
    );
    assert!(err <= 1e-2, "X1: {err:e}");
}

/// X2 (ADR 0351): circular duct R = 0.06 m, k R = 4.5; the eight propagating modes plus (4,1) and (0,2) outside the basis at
/// 0.05; polar sampling 24 x 48; section fit on the propagating modes, export, read back: every amplitude within 1e-2.
#[test]
fn x2_circular_section_with_truncation() {
    let radius = 0.06;
    let shape = DuctShape::Circular { radius };
    let k = 4.5 / radius;
    let basis = ModeBasis::below(shape, k).unwrap();
    assert_eq!(basis.len(), 8, "X2 basis {:?}", basis.modes);
    // (4,1) and (0,2) of the ADR's labels are, in the library's radial index from 0, (4, 0) and (0, 2).
    let extra = ModeBasis::new(shape, &[Mode { m: 4, n: 0 }, Mode { m: 0, n: 2 }]).unwrap();
    assert!(
        extra.k_cut.iter().all(|&kc| kc > k),
        "X2: the extra modes must be cut off: {:?}",
        extra.k_cut
    );
    let truth: Vec<C64> = (0..basis.len())
        .map(|q| C64::cis(0.7 * q as f64 - 1.3).scale(1.0 - 0.1 * q as f64))
        .collect();
    let extra_amp = [C64::cis(0.3).scale(0.05), C64::cis(-2.1).scale(0.05)];
    let (nr, nt) = (24, 48);
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for i in 0..nr {
        let (r0, r1) = (radius * i as f64 / nr as f64, radius * (i + 1) as f64 / nr as f64);
        let r = 0.5 * (r0 + r1);
        for j in 0..nt {
            let phi = 2.0 * PI * (j as f64 + 0.5) / nt as f64;
            points.push([r * phi.cos(), r * phi.sin()]);
            weights.push(0.5 * (r1 * r1 - r0 * r0) * 2.0 * PI / nt as f64);
        }
    }
    let values: Vec<C64> = points
        .iter()
        .map(|&pt| {
            let mut f = C64::default();
            for q in 0..basis.len() {
                f += truth[q] * basis.eval(q, pt);
            }
            for q in 0..2 {
                f += extra_amp[q] * extra.eval(q, pt);
            }
            f
        })
        .collect();
    let fit = decompose_section(&basis, &points, &weights, &values).unwrap();
    let export = ModalExport::section(&basis, k * C / (2.0 * PI), C, RHO0, &fit).unwrap();
    let back = ModalExport::from_json_str(&export.to_json_string()).unwrap();
    let got: Vec<C64> = back
        .modes
        .iter()
        .map(|r| match r.values {
            ModeValues::Section { amplitude } => amplitude,
            _ => panic!("X2: not a section record"),
        })
        .collect();
    let err = recovery_error(&truth, &got);
    println!(
        "X2: modes {:?}; worst recovery error {err:.3e}; residual {:.3e}",
        basis.modes.iter().map(|m| (m.m, m.n)).collect::<Vec<_>>(),
        fit.residual
    );
    assert!(err <= 1e-2, "X2: {err:e}");
}
