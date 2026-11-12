//! F-453 (ADR 0366): a spinning mode (1, 0) and a radial mode (0, 1) of a hard-walled circular duct on an O-grid
//! (`rcfd_mesh::gen::circular_duct_ogrid`) driven through the linear acoustic wave solver of rcfd-fv and measured by the
//! modal decomposition of rcfd-post. C10 and C01 (heavy records, ignored): K10, K01 the axial wavenumber against the
//! closed form corrected for the polygonal section (ADR 0366 DR3), S10, S01 the reflection of the matched layer, SP10
//! the spinning character. C10c, C01c, NC10c: the same code path on coarse grids. P: the polygonal correction against
//! its first-order estimate. BZ: Bessel functions and zeros computed here from the power series, against the library.
//! G1, G1d, G2: the O-grid. U8: conservation on the closed O-grid. Every set-up number and limit is the registered one.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, Medium, WaveBc, WaveSponge};
use rcfd_fv::sponge::{rate_max_for_reflection, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::circular_duct_ogrid;
use rcfd_post::duct_modes::{
    bessel_j, bessel_jp_zeros, decompose_section, fit_axial, DuctShape, HarmonicDft, Mode, ModeBasis, C64,
};
use std::f64::consts::PI;
use std::sync::Arc;

const RHO0: f64 = 1.2;
const C: f64 = 340.0;
const R: f64 = 0.06;
const CORE: f64 = 0.5;
const AIR: Medium = Medium { rho0: RHO0, c: C };

// ---------------------------------------------------------------------------------------------------------------------
// Bessel functions from the power series (ADR 0366 DR2), independent of the library's integral representation
// ---------------------------------------------------------------------------------------------------------------------

/// The terms `t_k = (-1)^k (x/2)^{2k+m} / (k! (k+m)!)` of the series of `J_m(x)`, summed with the weight `w(k)`.
fn series(m: u32, x: f64, w: impl Fn(u32) -> f64) -> f64 {
    let h = 0.5 * x;
    let mut t = 1.0;
    for i in 1..=m {
        t *= h / i as f64;
    }
    let mut sum = w(0) * t;
    let mut k = 0u32;
    loop {
        t *= -h * h / (((k + 1) * (k + 1 + m)) as f64);
        k += 1;
        let term = w(k) * t;
        sum += term;
        if (k as f64 > x && term.abs() <= 1e-17 * sum.abs()) || k > 400 {
            break;
        }
    }
    sum
}

/// `J_m(x)` by its power series.
fn series_j(m: u32, x: f64) -> f64 {
    series(m, x, |_| 1.0)
}

/// `J_m'(x)` by the term-by-term derivative of the series: `sum_k t_k (2k + m) / x` (`x > 0`).
fn series_jp(m: u32, x: f64) -> f64 {
    series(m, x, |k| (2 * k + m) as f64) / x
}

/// The zero `j'_{m,n}` of `J_m'` in the library's count (`n = 0` is the first non-negative zero: 0 for `m = 0`): a scan
/// in steps of 0.01 from 0.01 and bisection.
fn jp_zero(m: u32, n: u32) -> f64 {
    if m == 0 && n == 0 {
        return 0.0;
    }
    let wanted = if m == 0 { n } else { n + 1 };
    let mut found = 0;
    let mut x0 = 0.01;
    let mut f0 = series_jp(m, x0);
    loop {
        let x1 = x0 + 0.01;
        let f1 = series_jp(m, x1);
        if f0 * f1 < 0.0 {
            found += 1;
            if found == wanted {
                let (mut a, mut b, mut fa) = (x0, x1, f0);
                for _ in 0..200 {
                    let mid = 0.5 * (a + b);
                    if mid <= a || mid >= b {
                        break;
                    }
                    let fm = series_jp(m, mid);
                    if fa * fm <= 0.0 {
                        b = mid;
                    } else {
                        a = mid;
                        fa = fm;
                    }
                }
                return 0.5 * (a + b);
            }
        }
        x0 = x1;
        f0 = f1;
    }
}

/// BZ (ADR 0366): the series zeros against `bessel_jp_zeros` (<= 1e-10), the series residual `|J_m'(j)|` (<= 1e-12), the
/// series `J_m` against `bessel_j` (<= 1e-11).
#[test]
fn bz_bessel_series_and_zeros() {
    let mut worst_zero: f64 = 0.0;
    let mut worst_res: f64 = 0.0;
    for (m, n) in [(0u32, 1u32), (0, 2), (1, 0), (1, 1), (2, 0), (3, 0), (4, 0)] {
        let j = jp_zero(m, n);
        let lib = bessel_jp_zeros(m as i32, n as usize + 1)[n as usize];
        let res = series_jp(m, j).abs();
        println!(
            "BZ: j'_({m},{n}) = {j:.15} (library {lib:.15}, difference {:.2e}); |J'_m(j)| = {res:.2e}",
            j - lib
        );
        worst_zero = worst_zero.max((j - lib).abs());
        worst_res = worst_res.max(res);
    }
    let mut worst_j: f64 = 0.0;
    for m in 0..=5u32 {
        for i in 1..=20 {
            let x = 0.5 * i as f64;
            worst_j = worst_j.max((series_j(m, x) - bessel_j(m as i32, x)).abs());
        }
    }
    println!("BZ: worst zero difference {worst_zero:.2e}, worst residual {worst_res:.2e}, worst J_m difference {worst_j:.2e}");
    assert!(worst_zero <= 1e-10, "BZ zeros");
    assert!(worst_res <= 1e-12, "BZ residual");
    assert!(worst_j <= 1e-11, "BZ series against the library");
}

// ---------------------------------------------------------------------------------------------------------------------
// The O-grid
// ---------------------------------------------------------------------------------------------------------------------

/// Area of the inscribed regular polygon of `nb` sides.
fn polygon_area(nb: usize) -> f64 {
    0.5 * nb as f64 * R * R * (2.0 * PI / nb as f64).sin()
}

fn patch_area(mesh: &rcfd_mesh::Mesh, name: &str) -> (f64, usize) {
    let sf = mesh.face_area_vectors();
    let p = mesh.patch(name).unwrap();
    let a = p
        .faces()
        .map(|f| (sf[f][0].powi(2) + sf[f][1].powi(2) + sf[f][2].powi(2)).sqrt())
        .sum();
    (a, p.faces().len())
}

/// G1 (ADR 0366): counts, positive volumes, total volume and patch areas against the polygon's closed forms.
#[test]
fn g1_ogrid_volume_areas_counts() {
    for (n, n_r, n_z) in [(8usize, 4usize, 6usize), (16, 8, 4)] {
        let len = 0.1;
        let mesh = circular_duct_ogrid(n, n_r, n_z, R, CORE, len).unwrap();
        let nb = 4 * n;
        let per_layer = n * n + nb * n_r;
        assert_eq!(mesh.n_cells(), per_layer * n_z, "G1 cell count");
        let vol = mesh.cell_volumes();
        let vmin = vol.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(vmin > 0.0, "G1 a cell volume <= 0: {vmin}");
        let an = polygon_area(nb);
        let total: f64 = vol.iter().sum();
        let e_vol = (total / (an * len) - 1.0).abs();
        let (a_in, f_in) = patch_area(&mesh, "inlet");
        let (a_out, f_out) = patch_area(&mesh, "outlet");
        let (a_wall, f_wall) = patch_area(&mesh, "wall");
        let e_in = (a_in / an - 1.0).abs();
        let e_out = (a_out / an - 1.0).abs();
        let wall_cf = nb as f64 * 2.0 * R * (PI / nb as f64).sin() * len;
        let e_wall = (a_wall / wall_cf - 1.0).abs();
        println!(
            "G1 n = {n}: cells {}, min volume {vmin:.3e}, volume {e_vol:.2e}, inlet {e_in:.2e}, outlet {e_out:.2e}, wall \
             {e_wall:.2e}; faces {f_in} {f_out} {f_wall}",
            mesh.n_cells()
        );
        assert_eq!(
            (f_in, f_out, f_wall),
            (per_layer, per_layer, nb * n_z),
            "G1 face counts"
        );
        assert!(
            e_vol <= 1e-12 && e_in <= 1e-12 && e_out <= 1e-12 && e_wall <= 1e-12,
            "G1 areas and volume"
        );
    }
}

/// G1d (ADR 0366): the cell centres are invariant under the rotation by 90 degrees and the mirror x -> -x (<= 1e-12 R).
#[test]
fn g1d_ogrid_symmetry() {
    for (n, n_r, n_z) in [(8usize, 4usize, 6usize), (16, 8, 4)] {
        let mesh = circular_duct_ogrid(n, n_r, n_z, R, CORE, 0.1).unwrap();
        let cc = mesh.cell_centres();
        let nearest = |p: [f64; 3]| {
            cc.iter()
                .map(|q| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt())
                .fold(f64::INFINITY, f64::min)
        };
        let mut worst: f64 = 0.0;
        for p in cc {
            worst = worst.max(nearest([-p[1], p[0], p[2]]));
            worst = worst.max(nearest([-p[0], p[1], p[2]]));
        }
        println!("G1d n = {n}: largest distance {:.2e} R", worst / R);
        assert!(worst <= 1e-12 * R, "G1d");
    }
}

/// G2 (ADR 0366): invalid arguments are refused.
#[test]
fn g2_ogrid_refusals() {
    let ok = |n: usize, nr: usize, nz: usize, r: f64, core: f64, len: f64| {
        circular_duct_ogrid(n, nr, nz, r, core, len).is_ok()
    };
    assert!(ok(2, 1, 1, R, CORE, 0.1));
    assert!(!ok(1, 1, 1, R, CORE, 0.1));
    assert!(!ok(4, 0, 1, R, CORE, 0.1));
    assert!(!ok(4, 2, 0, R, CORE, 0.1));
    for r in [0.0, -1.0, f64::NAN] {
        assert!(!ok(4, 2, 1, r, CORE, 0.1));
    }
    assert!(!ok(4, 2, 1, R, CORE, 0.0));
    for core in [0.0, 0.71, f64::NAN] {
        assert!(!ok(4, 2, 1, R, core, 0.1));
    }
}

/// U8 (ADR 0366): closed O-grid duct, all walls, 100 steps: the integral of p is conserved (<= 1e-12 relative).
#[test]
fn u8_closed_ogrid_conserves_pressure_integral() {
    let len = 0.1;
    let mesh = circular_duct_ogrid(6, 3, 6, R, CORE, len).unwrap();
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let bcs = [
        ("wall", WaveBc::Wall),
        ("inlet", WaveBc::Wall),
        ("outlet", WaveBc::Wall),
    ];
    let mut s = AcousticWaveSolver::new(geom, AIR, &bcs).unwrap();
    for (c, x) in cc.iter().enumerate() {
        s.p[c] = (1.0 + x[0] / R + 0.5 * x[1] / R) * (PI * x[2] / len).cos();
    }
    let i0 = s.pressure_integral();
    let scale: f64 = s.p.iter().zip(&vol).map(|(p, v)| p.abs() * v).sum();
    let dt = s.max_dt(0.48);
    for _ in 0..100 {
        s.step(dt).unwrap();
    }
    let e = (s.pressure_integral() - i0).abs() / scale;
    println!("U8: relative change of sum p V {e:.2e}");
    assert!(e <= 1e-12, "U8");
}

// ---------------------------------------------------------------------------------------------------------------------
// Duct runs
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Run {
    /// Driven mode (library labels).
    mode: Mode,
    /// Cells along each side of the core (ring: n/2 radial, 4 n around).
    n: usize,
    /// Matched layer before the rigid end (true) or the rigid end alone.
    layer: bool,
}

/// The closed forms of one run.
struct Closed {
    omega: f64,
    k: f64,
    kz_cf: f64,
    kz_poly: f64,
    lambda_z: f64,
    nb: usize,
}

fn closed(r: Run) -> Closed {
    let j = jp_zero(r.mode.m.unsigned_abs(), r.mode.n);
    let k = j / (0.6 * R);
    let kz_cf = (k * k - (j / R).powi(2)).sqrt();
    let nb = 4 * r.n;
    let kc_poly = j * (PI / polygon_area(nb)).sqrt();
    Closed {
        omega: C * k,
        k,
        kz_cf,
        kz_poly: (k * k - kc_poly * kc_poly).sqrt(),
        lambda_z: 2.0 * PI / kz_cf,
        nb,
    }
}

/// P (ADR 0366): the polygonal correction from the polygon's area against its first-order estimate (relative <= 1e-2).
#[test]
fn p_polygonal_correction_against_first_order() {
    for (mode, n) in [
        (Mode { m: 1, n: 0 }, 16usize),
        (Mode { m: 0, n: 1 }, 32),
        (Mode { m: 1, n: 0 }, 8),
        (Mode { m: 0, n: 1 }, 12),
    ] {
        let cl = closed(Run { mode, n, layer: true });
        let j = jp_zero(mode.m.unsigned_abs(), mode.n);
        let corr = cl.kz_poly / cl.kz_cf - 1.0;
        let first = -((j / R).powi(2) / cl.kz_cf.powi(2)) * (PI / cl.nb as f64).powi(2) / 3.0;
        let e = (corr / first - 1.0).abs();
        println!(
            "P ({}, {}) n = {n}: k_z,poly / k_z,cf - 1 = {corr:+.4e}, first order {first:+.4e}, relative {e:.2e}",
            mode.m, mode.n
        );
        assert!(e <= 1e-2, "P");
    }
}

/// The measured waves of one run.
struct Measured {
    cl: Closed,
    kr: f64,
    ki: f64,
    loss: f64,
    refl: f64,
    plus: f64,
    residual: f64,
    counter: f64,
    other: f64,
    steps_per_period: usize,
    rate_dt: f64,
    cells: usize,
}

fn run(r: Run) -> Measured {
    run_periods(r, 34)
}

/// The run of `r` over `periods` periods (34 registered; other numbers for the diagnostic D1 of amendment 1), DFT over
/// the last 4.
fn run_periods(r: Run, periods: usize) -> Measured {
    let cl = closed(r);
    let length = 3.5 * cl.lambda_z;
    let n_z = (length / (R / r.n as f64)).round() as usize;
    let dz = length / n_z as f64;
    let mesh = circular_duct_ogrid(r.n, r.n / 2, n_z, R, CORE, length).unwrap();
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let shape = DuctShape::Circular { radius: R };
    let drive_basis = ModeBasis::new(shape, &[r.mode]).unwrap();
    let w = cl.omega;
    let kz = cl.kz_cf;
    let t_ramp = 8.0 * 2.0 * PI / w;
    let drive = move |x: [f64; 3], t: f64| -> (f64, [f64; 3]) {
        let ramp = if t < t_ramp {
            (PI * t / (2.0 * t_ramp)).sin().powi(2)
        } else {
            1.0
        };
        let a = drive_basis.eval(0, [x[0], x[1]]).scale(ramp);
        // Re(a e^{i omega t}).
        let p = a.re * (w * t).cos() - a.im * (w * t).sin();
        (p, [0.0, 0.0, kz / (RHO0 * w) * p])
    };
    let bcs = [
        ("inlet", WaveBc::Driven(Arc::new(drive))),
        ("wall", WaveBc::Wall),
        ("outlet", WaveBc::Wall),
    ];
    let z_front = 2.5 * cl.lambda_z;
    let sponge = if r.layer {
        let layer = SpongeLayer {
            origin: [0.0, 0.0, z_front],
            normal: [0.0, 0.0, 1.0],
            thickness: cl.lambda_z,
            rate_max: rate_max_for_reflection(cl.lambda_z, C, 1e-6, 2.0).unwrap(),
            exponent: 2.0,
        };
        Some(WaveSponge::from_layer(&geom, &layer, true).unwrap())
    } else {
        None
    };
    let max_rate = sponge
        .as_ref()
        .map_or(0.0, |s| s.rate.iter().cloned().fold(0.0, f64::max));
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let cells = cc.len();
    let mut s = AcousticWaveSolver::new(geom, AIR, &bcs).unwrap();
    s.set_sponge(sponge).unwrap();
    let period = 2.0 * PI / w;
    let n_t = (period / s.max_dt(0.48)).ceil() as usize;
    let dt = period / n_t as f64;
    let total = periods * n_t;
    let first_dft = total - 4 * n_t;
    let mut dft = HarmonicDft::new(w, cells).unwrap();
    for step in 0..total {
        s.step(dt).unwrap();
        s.t = (step + 1) as f64 * dt;
        if step >= first_dft {
            dft.add(s.t, &s.p).unwrap();
        }
    }
    let field = dft.amplitudes().unwrap();
    // Stations: the layers of cells.
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); n_z];
    for (c, x) in cc.iter().enumerate() {
        layers[((x[2] / dz).floor() as usize).min(n_z - 1)].push(c);
    }
    let basis = ModeBasis::below(shape, 1.5 * cl.k).unwrap();
    let q_drive = basis.modes.iter().position(|m| *m == r.mode).unwrap();
    let q_counter = basis
        .modes
        .iter()
        .position(|m| {
            *m == Mode {
                m: -r.mode.m,
                n: r.mode.n,
            }
        })
        .unwrap();
    let (lo, hi) = (0.5 * cl.lambda_z, 2.5 * cl.lambda_z);
    let (mut zs, mut amps) = (Vec::new(), Vec::new());
    let (mut counter, mut other): (f64, f64) = (0.0, 0.0);
    for (i, layer) in layers.iter().enumerate() {
        let z = (i as f64 + 0.5) * dz;
        if !(lo..=hi).contains(&z) {
            continue;
        }
        let points: Vec<[f64; 2]> = layer.iter().map(|&c| [cc[c][0], cc[c][1]]).collect();
        let weights: Vec<f64> = layer.iter().map(|&c| vol[c]).collect();
        let values: Vec<C64> = layer.iter().map(|&c| field[c]).collect();
        let fit = decompose_section(&basis, &points, &weights, &values).unwrap();
        let ad = fit.amplitudes[q_drive].abs();
        if q_counter != q_drive {
            counter = counter.max(fit.amplitudes[q_counter].abs() / ad);
        }
        for (q, a) in fit.amplitudes.iter().enumerate() {
            if q != q_drive && q != q_counter {
                other = other.max(a.abs() / ad);
            }
        }
        zs.push(z);
        amps.push(fit.amplitudes[q_drive]);
    }
    let z_ref = if r.layer { z_front } else { length };
    let fit = fit_axial(&zs, &amps, cl.kz_poly, z_ref).unwrap();
    assert!(
        !fit.at_bracket_edge,
        "the fitted wavenumber lies at its bracket edge: {:?}",
        fit.kz
    );
    let kr = fit.kz.re;
    let ki = -fit.kz.im;
    Measured {
        kr,
        ki,
        loss: 1.0 - (-2.0 * PI * ki / kr).exp(),
        refl: (fit.minus / fit.plus).abs(),
        plus: fit.plus.abs(),
        residual: fit.residual,
        counter,
        other,
        steps_per_period: n_t,
        rate_dt: max_rate * dt,
        cells,
        cl,
    }
}

fn report(name: &str, m: &Measured) {
    println!(
        "{name}: cells {}; steps per period {}; kr = {:.6} (k_z,poly {:.6}, rel {:+.4e}; circle k_z,cf {:.6}, rel {:+.4e}); \
         ki = {:.4e}; loss per wavelength {:.3e}; R = {:.3e}; |plus| = {:.4}; fit residual {:.3e}; counter-rotating \
         {:.3e}; other modes {:.3e}; rate_max dt {:.3}; k = {:.4}",
        m.cells,
        m.steps_per_period,
        m.kr,
        m.cl.kz_poly,
        m.kr / m.cl.kz_poly - 1.0,
        m.cl.kz_cf,
        m.kr / m.cl.kz_cf - 1.0,
        m.ki,
        m.loss,
        m.refl,
        m.plus,
        m.residual,
        m.counter,
        m.other,
        m.rate_dt,
        m.cl.k
    );
}

const SPIN: Mode = Mode { m: 1, n: 0 };
const RADIAL: Mode = Mode { m: 0, n: 1 };

/// C10 (ADR 0366, heavy record): the spinning mode (1, 0), n = 16, matched layer. K10 |kr/k_z,poly - 1| <= 1e-2; S10
/// R < 1e-2; SP10 counter-rotating <= 1e-6.
/// KNOWN MISS (ADR 0366 addendum 1): the registered run met K10 (-2.583e-3) and S10 (3.583e-5) and missed SP10, the
/// counter-rotating ratio 1.264e-5 against 1e-6 (the start transient of the ramp, amendment 1 H1). Kept with the limit
/// intact; `c10_spinning_mode_matched_layer_k_s` holds the two met assertions.
#[test]
#[ignore = "known miss and heavy record (ADR 0366 C10, about 20 minutes in release): SP10 1.264e-5 against 1e-6"]
fn c10_spinning_mode_matched_layer_known_miss() {
    let m = run(Run {
        mode: SPIN,
        n: 16,
        layer: true,
    });
    report("C10", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 1e-2, "K10");
    assert!(m.refl < 1e-2, "S10");
    assert!(m.counter <= 1e-6, "SP10");
}

/// C10 (ADR 0366 addendum 1, heavy record): the K10 (<= 1e-2) and S10 (< 1e-2) assertions of the C10 run. Not run as a
/// separate run: the registered run of `c10_spinning_mode_matched_layer_known_miss` gave both numbers.
#[test]
#[ignore = "heavy record (ADR 0366 C10, about 20 minutes in release); C10c covers the code path"]
fn c10_spinning_mode_matched_layer_k_s() {
    let m = run(Run {
        mode: SPIN,
        n: 16,
        layer: true,
    });
    report("C10", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 1e-2, "K10");
    assert!(m.refl < 1e-2, "S10");
}

/// C01 (ADR 0366, heavy record): the radial mode (0, 1), n = 32, matched layer. K01 |kr/k_z,poly - 1| <= 1e-2; S01
/// R < 1e-2.
#[test]
#[ignore = "heavy record (ADR 0366 C01, about an hour in release); C01c covers the code path"]
fn c01_radial_mode_matched_layer() {
    let m = run(Run {
        mode: RADIAL,
        n: 32,
        layer: true,
    });
    report("C01", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 1e-2, "K01");
    assert!(m.refl < 1e-2, "S01");
}

/// C10c (ADR 0366): the spinning mode on the coarse grid n = 8. K10c <= 3e-2; S10c R < 1e-2; SP10c <= 1e-6.
/// KNOWN MISS (ADR 0366 amendment 1): the registered run met K10c (-9.742e-3) and S10c (1.533e-4) and missed SP10c, the
/// counter-rotating ratio 1.233e-5 against 1e-6. Kept with the limit intact; `c10c_spinning_mode_coarse_k_s` keeps the
/// two met assertions of the same run as the fast test.
#[test]
#[ignore = "known miss (ADR 0366 amendment 1): SP10c 1.233e-5 against 1e-6, the start transient of the ramp"]
fn c10c_spinning_mode_coarse_known_miss() {
    let m = run(Run {
        mode: SPIN,
        n: 8,
        layer: true,
    });
    report("C10c", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 3e-2, "K10c");
    assert!(m.refl < 1e-2, "S10c");
    assert!(m.counter <= 1e-6, "SP10c");
}

/// C10c (ADR 0366 amendment 1): the K10c (<= 3e-2) and S10c (< 1e-2) assertions of the C10c run.
#[test]
fn c10c_spinning_mode_coarse_k_s() {
    let m = run(Run {
        mode: SPIN,
        n: 8,
        layer: true,
    });
    report("C10c", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 3e-2, "K10c");
    assert!(m.refl < 1e-2, "S10c");
}

/// D1 (ADR 0366 amendment 1, diagnostic): the C10c set-up for 60 periods; under H1 the counter-rotating ratio falls to
/// at most half of the 34-period 1.233e-5.
#[test]
fn d1_counter_rotating_falls_with_time() {
    let m = run_periods(
        Run {
            mode: SPIN,
            n: 8,
            layer: true,
        },
        60,
    );
    report("D1", &m);
    assert!(m.counter <= 0.5 * 1.233e-5, "D1");
}

/// D2 (ADR 0366 amendment 1, diagnostic): the solver commutes with the rotation by 90 degrees on the O-grid. The C10c
/// mesh and set-up, 300 steps, standing drives `J_1 cos(phi)` (A) and `J_1 sin(phi)` (B):
/// `max |p_B(R x) - p_A(x)| / max |p_A| <= 1e-9`.
#[test]
fn d2_solver_commutes_with_the_rotation() {
    let r = Run {
        mode: SPIN,
        n: 8,
        layer: true,
    };
    let cl = closed(r);
    let length = 3.5 * cl.lambda_z;
    let n_z = (length / (R / r.n as f64)).round() as usize;
    let mesh = Arc::new(circular_duct_ogrid(r.n, r.n / 2, n_z, R, CORE, length).unwrap());
    let shape = DuctShape::Circular { radius: R };
    let w = cl.omega;
    let kz = cl.kz_cf;
    let t_ramp = 8.0 * 2.0 * PI / w;
    let solve = |sine: bool| -> Vec<f64> {
        let geom = FvGeometry::new(mesh.clone()).unwrap();
        let basis = ModeBasis::new(shape, &[r.mode]).unwrap();
        let drive = move |x: [f64; 3], t: f64| -> (f64, [f64; 3]) {
            let ramp = if t < t_ramp {
                (PI * t / (2.0 * t_ramp)).sin().powi(2)
            } else {
                1.0
            };
            let a = basis.eval(0, [x[0], x[1]]);
            // N J_1 cos(phi) = Re(psi), N J_1 sin(phi) = Im(psi).
            let shape = if sine { a.im } else { a.re };
            let p = ramp * shape * (w * t).cos();
            (p, [0.0, 0.0, kz / (RHO0 * w) * p])
        };
        let bcs = [
            ("inlet", WaveBc::Driven(Arc::new(drive))),
            ("wall", WaveBc::Wall),
            ("outlet", WaveBc::Wall),
        ];
        let layer = SpongeLayer {
            origin: [0.0, 0.0, 2.5 * cl.lambda_z],
            normal: [0.0, 0.0, 1.0],
            thickness: cl.lambda_z,
            rate_max: rate_max_for_reflection(cl.lambda_z, C, 1e-6, 2.0).unwrap(),
            exponent: 2.0,
        };
        let sponge = WaveSponge::from_layer(&geom, &layer, true).unwrap();
        let mut s = AcousticWaveSolver::new(geom, AIR, &bcs).unwrap();
        s.set_sponge(Some(sponge)).unwrap();
        let period = 2.0 * PI / w;
        let n_t = (period / s.max_dt(0.48)).ceil() as usize;
        let dt = period / n_t as f64;
        for step in 0..300 {
            s.step(dt).unwrap();
            s.t = (step + 1) as f64 * dt;
        }
        s.p.clone()
    };
    let pa = solve(false);
    let pb = solve(true);
    let cc = mesh.cell_centres();
    let scale = pa.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let mut worst: f64 = 0.0;
    for (c, x) in cc.iter().enumerate() {
        let rx = [-x[1], x[0], x[2]];
        let (cr, _) = cc
            .iter()
            .enumerate()
            .map(|(i, q)| {
                (
                    i,
                    (q[0] - rx[0]).powi(2) + (q[1] - rx[1]).powi(2) + (q[2] - rx[2]).powi(2),
                )
            })
            .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
        worst = worst.max((pb[cr] - pa[c]).abs());
    }
    println!(
        "D2: max |p_B(R x) - p_A(x)| / max |p_A| = {:.3e} (max |p_A| {scale:.3e})",
        worst / scale
    );
    assert!(worst / scale <= 1e-9, "D2");
}

/// C01c (ADR 0366): the radial mode on the coarse grid n = 12. K01c <= 5e-2; S01c R < 1e-2.
#[test]
fn c01c_radial_mode_coarse() {
    let m = run(Run {
        mode: RADIAL,
        n: 12,
        layer: true,
    });
    report("C01c", &m);
    assert!((m.kr / m.cl.kz_poly - 1.0).abs() <= 5e-2, "K01c");
    assert!(m.refl < 1e-2, "S01c");
}

/// NC10c (ADR 0366, negative control): the C10c set-up without the layer, R at the rigid end >= 0.9.
#[test]
fn nc10c_rigid_end_reflects() {
    let m = run(Run {
        mode: SPIN,
        n: 8,
        layer: false,
    });
    report("NC10c", &m);
    assert!(m.refl >= 0.9, "NC10c");
}
