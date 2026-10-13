//! Boundary-layer inflation for block meshes (F-108): prismatic (here hexahedral) wall layers
//! from a first-cell height or a target `y+`, with a growth ratio, plus the layer report.
//!
//! The first-cell height follows from the friction velocity: the wall-adjacent cell centre sits at
//! `y_P = h_1 / 2`, so a target `y+ = u_tau y_P / nu` needs `h_1 = 2 y+ nu / u_tau`. The friction
//! velocity of a flat plate is estimated from a skin-friction correlation,
//! `u_tau = U sqrt(Cf / 2)` with White's `Cf = 0.455 / ln^2(0.06 Re_x)` (see
//! docs/reference-data.md), or supplied directly.

use rcfd_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Local skin-friction coefficient of a turbulent flat plate, `Cf = 0.455 / ln^2(0.06 Re_x)`
/// (White's fit to the Karman-Schoenberg law; about 2 percent accurate for 1e5 < Re_x < 1e9).
pub fn flat_plate_cf(re_x: f64) -> f64 {
    let l = (0.06 * re_x).ln();
    0.455 / (l * l)
}

/// Friction velocity of a flat plate at distance `x` from the leading edge, free-stream speed `u`
/// and kinematic viscosity `nu`.
pub fn flat_plate_u_tau(u: f64, nu: f64, x: f64) -> f64 {
    u * (0.5 * flat_plate_cf(u * x / nu)).sqrt()
}

/// Height `h_1` of the wall-adjacent cell whose centre (at `h_1 / 2`) has the given `y+`.
pub fn first_cell_height_from_y_plus(y_plus: f64, u_tau: f64, nu: f64) -> f64 {
    2.0 * y_plus * nu / u_tau
}

/// `y+` of the centre of a wall-adjacent cell of height `h_1`.
pub fn y_plus_of_first_cell(h1: f64, u_tau: f64, nu: f64) -> f64 {
    0.5 * h1 * u_tau / nu
}

/// Cell sizes of one wall segment of `n` cells and length `len` starting at the wall:
/// `layers` cells `first * growth^i`, then `n - layers` bulk cells `h_L r^j` (`j = 1..`), with `r`
/// solved so that the sizes add up to `len` (`r` is the bulk growth ratio, returned too).
pub fn wall_segment_sizes(n: usize, len: f64, first: f64, growth: f64, layers: usize) -> Result<(Vec<f64>, f64)> {
    if !(first > 0.0 && growth > 0.0 && len > 0.0) {
        return Err(Error::Mesh(
            "wall layers need a positive first height, growth ratio and length".into(),
        ));
    }
    if layers == 0 || layers >= n {
        return Err(Error::Mesh(format!(
            "wall layers: {layers} layers need at least one more bulk cell than layers (axis has {n} cells)"
        )));
    }
    let mut sizes: Vec<f64> = (0..layers).map(|i| first * growth.powi(i as i32)).collect();
    let thick: f64 = sizes.iter().sum();
    let rest = len - thick;
    if rest <= 0.0 {
        return Err(Error::Mesh(format!(
            "wall layers: the {layers} layers are {thick:.4e} thick, not less than the axis length {len:.4e}"
        )));
    }
    let h_l = *sizes.last().expect("layers >= 1");
    let m = n - layers;
    let total = |r: f64| -> f64 {
        let (mut s, mut h) = (0.0, h_l);
        for _ in 0..m {
            h *= r;
            s += h;
        }
        s
    };
    // total(r) increases with r: bisect on a logarithmic scale
    let (mut lo, mut hi) = (1e-6_f64, 1e6_f64);
    for _ in 0..300 {
        let mid = (lo * hi).sqrt();
        if total(mid) < rest {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let r = (lo * hi).sqrt();
    let mut h = h_l;
    for _ in 0..m {
        h *= r;
        sizes.push(h);
    }
    // absorb the bisection residual
    let sum: f64 = sizes.iter().sum();
    let k = len / sum;
    sizes.iter_mut().for_each(|s| *s *= k);
    Ok((sizes, r))
}

/// Cell sizes of a whole axis for [`crate::gen::Grading::WallLayers`].
pub fn wall_layer_axis_sizes(
    n: usize,
    len: f64,
    first: f64,
    growth: f64,
    layers: usize,
    both: bool,
) -> Result<Vec<f64>> {
    if !both {
        return Ok(wall_segment_sizes(n, len, first, growth, layers)?.0);
    }
    if n % 2 != 0 {
        return Err(Error::Mesh(
            "wall layers at both walls need an even number of cells".into(),
        ));
    }
    let (mut half, _) = wall_segment_sizes(n / 2, 0.5 * len, first, growth, layers)?;
    let mirror: Vec<f64> = half.iter().rev().copied().collect();
    half.extend(mirror);
    Ok(half)
}

/// Summary of the inflation layers of an axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerReport {
    /// Height of the wall-adjacent cell.
    pub first_height: f64,
    /// Number of inflation layers examined.
    pub layers: usize,
    /// Total thickness of the layers.
    pub thickness: f64,
    /// Smallest ratio of successive layer heights.
    pub growth_min: f64,
    /// Largest ratio of successive layer heights.
    pub growth_max: f64,
    /// Ratio of the first bulk cell to the last layer (1 = smooth transition).
    pub transition_ratio: f64,
    /// Largest ratio of neighbouring cell heights over the whole axis.
    pub max_neighbour_ratio: f64,
}

/// Layer report of axis coordinates measured from the wall at `coords[0]`.
pub fn layer_report(coords: &[f64], layers: usize) -> Option<LayerReport> {
    if layers < 2 || coords.len() < layers + 2 {
        return None;
    }
    let h: Vec<f64> = coords.windows(2).map(|w| w[1] - w[0]).collect();
    let ratios: Vec<f64> = h[..layers].windows(2).map(|w| w[1] / w[0]).collect();
    let nbr = h.windows(2).map(|w| (w[1] / w[0]).max(w[0] / w[1])).fold(1.0, f64::max);
    Some(LayerReport {
        first_height: h[0],
        layers,
        thickness: coords[layers] - coords[0],
        growth_min: ratios.iter().copied().fold(f64::INFINITY, f64::min),
        growth_max: ratios.iter().copied().fold(0.0, f64::max),
        transition_ratio: h[layers] / h[layers - 1],
        max_neighbour_ratio: nbr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{axis_coords, box_2d_graded, channel, Grading};
    use crate::quality::check_mesh;

    fn wl(first: f64, growth: f64, layers: usize, both: bool) -> Grading {
        Grading::WallLayers {
            first,
            growth,
            layers,
            both,
        }
    }

    #[test]
    fn layer_heights_follow_first_height_and_growth() {
        let x = axis_coords(40, 1.0, wl(1e-3, 1.2, 10, false)).unwrap();
        assert_eq!(x.len(), 41);
        assert!((x[40] - 1.0).abs() < 1e-15 && x[0] == 0.0);
        let h: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
        // the total length is honoured by a tiny rescale of all sizes
        assert!((h[0] / 1e-3 - 1.0).abs() < 1e-6, "{}", h[0]);
        for i in 1..10 {
            assert!((h[i] / h[i - 1] - 1.2).abs() < 1e-5, "layer {i}: {}", h[i] / h[i - 1]);
        }
        let rep = layer_report(&x, 10).unwrap();
        assert!((rep.growth_min - 1.2).abs() < 1e-5 && (rep.growth_max - 1.2).abs() < 1e-5);
        let thick: f64 = (0..10).map(|i| 1e-3 * 1.2f64.powi(i)).sum();
        assert!((rep.thickness - thick).abs() < 1e-5 * thick);
        assert!(h.iter().all(|&s| s > 0.0));
        assert!(h[39] > 50.0 * h[0]);
        assert!(rep.max_neighbour_ratio < 1.5, "{}", rep.max_neighbour_ratio);
        // bad inputs
        assert!(axis_coords(10, 1.0, wl(0.0, 1.2, 3, false)).is_err());
        assert!(axis_coords(5, 1.0, wl(0.1, 1.2, 5, false)).is_err());
        assert!(axis_coords(20, 0.05, wl(0.01, 1.5, 10, false)).is_err());
        assert!(axis_coords(9, 1.0, wl(0.01, 1.2, 2, true)).is_err());
    }

    #[test]
    fn both_walls_are_mirror_images() {
        let x = axis_coords(40, 2.0, wl(2e-3, 1.15, 8, true)).unwrap();
        let h: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
        for i in 0..20 {
            assert!((h[i] - h[39 - i]).abs() < 1e-12);
        }
        assert!((h[0] / 2e-3 - 1.0).abs() < 1e-6);
        assert!((h[1] / h[0] - 1.15).abs() < 1e-5);
    }

    #[test]
    fn y_plus_targeting_round_trip() {
        let (u, nu, x) = (10.0, 1.5e-5, 1.0);
        let ut = flat_plate_u_tau(u, nu, x);
        // Re_x = 6.7e5: Cf about 0.0037 -> u_tau about 0.43 m/s
        assert!((0.38..0.46).contains(&ut), "{ut}");
        let h1 = first_cell_height_from_y_plus(1.0, ut, nu);
        assert!((y_plus_of_first_cell(h1, ut, nu) - 1.0).abs() < 1e-12);
        // the centre of the first cell of a generated mesh has the target y+
        let m = box_2d_graded(
            8,
            60,
            1.0,
            0.05,
            [Grading::Uniform, wl(h1, 1.15, 20, false)],
            false,
            false,
        )
        .unwrap();
        let yc = m.cell_centres().iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
        assert!((yc * ut / nu - 1.0).abs() < 1e-5, "y+ {}", yc * ut / nu);
    }

    #[test]
    fn inflated_meshes_conserve_volume_and_pass_the_mesh_check() {
        let g = wl(1e-3, 1.2, 12, true);
        let c = channel(
            [10, 40, 1],
            [5.0, 1.0, 0.1],
            [Grading::Uniform, g, Grading::Uniform],
            true,
        )
        .unwrap();
        assert!((c.total_volume() - 0.5).abs() < 1e-12);
        let r = check_mesh(&c);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(c.n_cells(), 400);
        // largest aspect ratio is dx / h_1 = 0.5 / 1e-3 and stays below the warning limit
        assert!((r.max_aspect_ratio - 0.5 / 1e-3).abs() < 1.0, "{}", r.max_aspect_ratio);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let b = box_2d_graded(
            20,
            30,
            2.0,
            0.5,
            [Grading::Uniform, wl(5e-4, 1.25, 10, false)],
            false,
            false,
        )
        .unwrap();
        assert!((b.total_volume() - 2.0 * 0.5 * (2.0 / 20.0)).abs() < 1e-12);
        assert!(check_mesh(&b).is_ok());
    }
}
