//! Studio render core (L4 of `docs/ui-restart-plan.md`, ADR 0125): wgpu offscreen rendering of surface meshes.
//!
//! A [`SurfaceMesh`] (boundary patches or face zones of a volume mesh, or a hand-built surface) with a per-face or
//! per-point scalar [`Field`] is uploaded once ([`Renderer::upload`]) and drawn with a [`View`]: orbit
//! [`Camera`] (perspective or orthographic), colour map and range, unlit / flat / smooth shading, per-part
//! colour and visibility, a cut plane clipped per fragment in the shader. [`Renderer::render`] returns an RGBA
//! image (PNG through [`rcfd_post::render::Image::to_png`]); [`Renderer::pick`] returns the visible face and
//! field value per pixel. No window, no MSAA: pixels are deterministic on a given adapter.

pub mod camera;
mod gpu;
pub mod surface;

pub use camera::{Camera, Projection};
pub use gpu::{adapters, adapters_of, GpuMesh, PartStyle, PickImage, Renderer, Shading, View, VERTEX_BYTES};
pub use rcfd_post::render::Image;
pub use surface::{Field, SurfaceMesh};
pub use wgpu;

/// Piecewise-linear colour map: evenly spaced RGB stops (2 to 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorMap(pub Vec<[u8; 3]>);

impl ColorMap {
    /// The viridis map of `rcfd_post::render::colormap`. Its 9 stops are integers, so sampling it at `k / 8`
    /// returns them exactly.
    pub fn viridis() -> ColorMap {
        ColorMap((0..9).map(|k| rcfd_post::render::colormap(k as f64 / 8.0)).collect())
    }

    /// Colour at `t` in [0, 1] (clamped; non-finite gives the first stop), rounded to the nearest integer.
    pub fn eval(&self, t: f64) -> [u8; 3] {
        let s = &self.0;
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let x = t * (s.len() - 1) as f64;
        let i = (x.floor() as usize).min(s.len() - 2);
        let f = x - i as f64;
        [0, 1, 2].map(|k| (s[i][k] as f64 * (1.0 - f) + s[i + 1][k] as f64 * f).round() as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viridis_matches_rcfd_post() {
        let m = ColorMap::viridis();
        for k in 0..=1000 {
            let t = k as f64 / 1000.0;
            assert_eq!(m.eval(t), rcfd_post::render::colormap(t), "t = {t}");
        }
    }
}
