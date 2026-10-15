//! ADR 0026 S1: closed forms of the pointwise subgrid functions. Limits as registered in the ADR (written before this file
//! was run); expected digits come from the hand derivations of the ADR.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_turb::les::{filter_width, smagorinsky_nu, van_driest, wale_nu};

const CS: f64 = 0.17;
const CW: f64 = 0.325;
const D: f64 = 0.1;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn tensor(entries: &[((usize, usize), f64)]) -> [[f64; 3]; 3] {
    let mut g = [[0.0; 3]; 3];
    for &((i, j), v) in entries {
        g[i][j] = v;
    }
    g
}

#[test]
fn s1a_pure_shear() {
    let g = tensor(&[((0, 1), 2.0)]);
    assert!(rel(smagorinsky_nu(&g, D, CS), 5.78e-4) <= 1e-12);
    assert_eq!(wale_nu(&g, D, CW), 0.0);
}

#[test]
fn s1b_pure_strain() {
    let g = tensor(&[((0, 0), 1.5), ((1, 1), -1.5)]);
    assert!(rel(smagorinsky_nu(&g, D, CS), 8.670e-4) <= 1e-12);
    assert!(
        rel(wale_nu(&g, D, CW), 1.377838789072e-4) <= 1e-12,
        "{}",
        wale_nu(&g, D, CW)
    );
}

#[test]
fn s1c_uniform_flow() {
    let g = [[0.0; 3]; 3];
    assert_eq!(smagorinsky_nu(&g, D, CS), 0.0);
    let w = wale_nu(&g, D, CW);
    assert!(w == 0.0 && w.is_finite());
}

#[test]
fn s1d_rigid_rotation() {
    let g = tensor(&[((0, 1), -3.0), ((1, 0), 3.0)]);
    assert_eq!(smagorinsky_nu(&g, D, CS), 0.0);
    assert!(
        rel(wale_nu(&g, D, CW), 2.863288848939e-3) <= 1e-12,
        "{}",
        wale_nu(&g, D, CW)
    );
}

#[test]
fn s1e_general_tensor() {
    let g = [[0.4, -0.7, 0.2], [1.1, -0.3, 0.5], [-0.6, 0.8, -0.1]];
    assert!(rel(smagorinsky_nu(&g, D, CS), 4.596826405250e-4) <= 1e-12);
    assert!(rel(wale_nu(&g, D, CW), 2.189365993742e-4) <= 1e-12);
}

/// Gradient of `u = (a y + b y z, 0, c y)` at `(y, z)`.
fn near_wall(y: f64, z: f64) -> [[f64; 3]; 3] {
    let (a, b, c) = (1.0, 1.0, 1.0);
    tensor(&[((0, 1), a + b * z), ((0, 2), b * y), ((2, 1), c)])
}

#[test]
fn s1f_near_wall_scaling() {
    let (a, b, c) = (1.0_f64, 1.0_f64, 1.0_f64);
    let _ = a;
    let y = 1e-3;
    let lead = (CW * D).powi(2) * (b * c * y).powi(3) / (2.0_f64.powf(1.5) * 1.0);
    let r = wale_nu(&near_wall(y, 0.0), D, CW) / lead - 1.0;
    assert!(r.abs() <= 1e-5, "WALE leading-order ratio - 1 = {r:e}");
    let slope = |f: &dyn Fn(f64) -> f64| {
        let ys: [f64; 3] = [1e-4, 1e-3, 1e-2];
        let (xs, vs): (Vec<f64>, Vec<f64>) = ys.iter().map(|&y| (y.ln(), f(y).ln())).unzip();
        let mx = xs.iter().sum::<f64>() / 3.0;
        let mv = vs.iter().sum::<f64>() / 3.0;
        xs.iter().zip(&vs).map(|(x, v)| (x - mx) * (v - mv)).sum::<f64>()
            / xs.iter().map(|x| (x - mx).powi(2)).sum::<f64>()
    };
    let sw = slope(&|y| wale_nu(&near_wall(y, 0.0), D, CW));
    let ss = slope(&|y| smagorinsky_nu(&near_wall(y, 0.0), D, CS));
    println!("S1f WALE slope {sw:.6}, Smagorinsky slope {ss:.6}");
    assert!((sw - 3.0).abs() <= 1e-3, "{sw}");
    assert!(ss.abs() <= 1e-3, "{ss}");
    // van Driest: exact factor and exponent 2 at the wall
    for yp in [0.3, 5.0, 40.0] {
        let g = near_wall(1e-3, 0.0);
        let plain = smagorinsky_nu(&g, D, CS);
        let damped = smagorinsky_nu(&g, D * van_driest(yp, 25.0), CS);
        assert!(rel(damped, plain * (1.0 - (-yp / 25.0_f64).exp()).powi(2)) <= 1e-12);
    }
    let sv = {
        let (x1, x2) = (1e-3_f64, 1e-2_f64);
        let f = |yp: f64| van_driest(yp, 25.0).powi(2);
        (f(x2).ln() - f(x1).ln()) / (x2.ln() - x1.ln())
    };
    assert!((sv - 2.0).abs() <= 1e-3, "{sv}");
}

#[test]
fn s1g_filter_width() {
    for (n, l) in [([4, 3, 2], [2.0, 1.5, 1.0]), ([8, 2, 1], [2.0, 1.0, 1.0])] {
        let m = box_mesh(n, l, [Grading::Uniform; 3]).unwrap();
        for &v in m.cell_volumes() {
            assert!(rel(filter_width(v), 0.5) <= 1e-14);
        }
    }
}
