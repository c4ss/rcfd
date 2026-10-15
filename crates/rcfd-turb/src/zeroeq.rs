//! Algebraic zero-equation closure: Prandtl mixing length (F-206, ADR 0026).
//!
//! `nu_t = l^2 |S|` with `|S| = sqrt(2 S_ij S_ij)`. The length is either linear, `l = min(kappa d f, l_max_ratio R)`, or the
//! Nikuradse pipe profile `l = R (0.14 - 0.08 (1 - d/R)^2 - 0.06 (1 - d/R)^4) f`, with the optional van Driest damping
//! `f = 1 - exp(-y+/A+)`. Wall-resolved: the eddy viscosity on the wall faces is zero. Steady use relaxes `nu_t` (Picard
//! iteration, as the plain map is only neutrally stable); transient use is explicit.

use crate::les::{relative_change, strain_magnitude, van_driest, velocity_gradients, WallInfo};
use crate::model::{CoefficientInfo, ModelContext, TurbulenceModel};
use rcfd_core::Result;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// Shape of the mixing-length profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixingProfile {
    /// `l = min(kappa d, l_max_ratio R)`.
    Linear,
    /// Nikuradse's pipe formula in `d/R`.
    Nikuradse,
}

/// Default van Driest constant of the mixing length.
pub const A_PLUS_ML: f64 = 26.0;

/// Nikuradse's pipe mixing length divided by the radius, `l/R` at `xi = d/R` (clamped to `xi <= 1`).
pub fn nikuradse_l_over_r(xi: f64) -> f64 {
    let e = 1.0 - xi.clamp(0.0, 1.0);
    0.14 - 0.08 * e * e - 0.06 * e.powi(4)
}

/// Mixing length for wall distance `d` (without damping).
pub fn mixing_length(profile: MixingProfile, kappa: f64, l_max_ratio: f64, r: f64, d: f64) -> f64 {
    match profile {
        MixingProfile::Linear => (kappa * d).min(l_max_ratio * r),
        MixingProfile::Nikuradse => r * nikuradse_l_over_r(d / r),
    }
}

/// Mixing-length model behind the turbulence interface.
pub struct MixingLengthModel {
    /// von Karman constant (linear profile).
    pub kappa: f64,
    /// Upper limit of the linear profile in units of `length_scale`.
    pub l_max_ratio: f64,
    /// van Driest constant.
    pub a_plus: f64,
    /// Profile shape.
    pub profile: MixingProfile,
    /// Length scale `R` (channel half height or pipe radius) [m].
    pub length_scale: f64,
    /// Van Driest damping of the length.
    pub damping: bool,
    /// Under-relaxation of `nu_t` (steady Picard iteration); 1 for transient use.
    pub relax_nut: f64,
    geom: Arc<FvGeometry>,
    wall: WallInfo,
    nut: Vec<f64>,
    started: bool,
}

impl MixingLengthModel {
    /// Build from a context (the model needs at least one wall patch for its distance).
    pub fn new(ctx: &ModelContext) -> Result<Self> {
        let walls: Vec<&str> = ctx.walls.iter().map(String::as_str).collect();
        let wall = WallInfo::new(&ctx.geom, &walls, ctx.nu)?;
        Ok(MixingLengthModel {
            kappa: 0.41,
            l_max_ratio: 0.09,
            a_plus: A_PLUS_ML,
            profile: ctx.mixing_profile,
            length_scale: ctx.length_scale,
            damping: ctx.wall_damping,
            relax_nut: if ctx.time_accurate { 1.0 } else { ctx.relax_nut },
            nut: vec![0.0; ctx.geom.n_cells()],
            geom: ctx.geom.clone(),
            wall,
            started: false,
        })
    }

    /// Mixing length of every cell for the current wall shear (damping included).
    pub fn lengths(&self) -> Vec<f64> {
        let yp = self.wall.y_plus_cells();
        (0..self.nut.len())
            .map(|c| {
                let l = mixing_length(
                    self.profile,
                    self.kappa,
                    self.l_max_ratio,
                    self.length_scale,
                    self.wall.dist[c],
                );
                if self.damping {
                    // the damping multiplies the profile length; the linear cap applies to the damped length
                    let f = van_driest(yp[c], self.a_plus);
                    match self.profile {
                        MixingProfile::Linear => {
                            (self.kappa * self.wall.dist[c] * f).min(self.l_max_ratio * self.length_scale)
                        }
                        MixingProfile::Nikuradse => l * f,
                    }
                } else {
                    l
                }
            })
            .collect()
    }
}

/// Coefficient list of the mixing-length model for the given values.
pub fn mixing_length_infos(kappa: f64, l_max_ratio: f64, a_plus: f64) -> Vec<CoefficientInfo> {
    vec![
        CoefficientInfo {
            name: "kappa",
            value: kappa,
            default: 0.41,
            min: 0.2,
            max: 0.6,
            doc: "von Karman constant of the linear profile l = kappa d",
        },
        CoefficientInfo {
            name: "l_max_ratio",
            value: l_max_ratio,
            default: 0.09,
            min: 0.0,
            max: 1.0,
            doc: "upper limit of the linear profile in units of turbulence.length_scale (0.09 delta); 0 gives no eddy viscosity",
        },
        CoefficientInfo {
            name: "a_plus",
            value: a_plus,
            default: A_PLUS_ML,
            min: 1.0,
            max: 100.0,
            doc: "van Driest constant A+ of the optional wall damping 1 - exp(-y+/A+)",
        },
    ]
}

impl TurbulenceModel for MixingLengthModel {
    fn key(&self) -> &str {
        "mixing_length"
    }
    fn update(&mut self, u: &VectorField, _phi: &[f64]) -> f64 {
        self.wall.update_shear(u);
        let l = self.lengths();
        let g = velocity_gradients(&self.geom, u);
        let new: Vec<f64> = g.iter().zip(&l).map(|(gc, &l)| l * l * strain_magnitude(gc)).collect();
        let alpha = if self.started { self.relax_nut } else { 1.0 };
        let mixed: Vec<f64> = self
            .nut
            .iter()
            .zip(&new)
            .map(|(&o, &n)| if alpha >= 1.0 { n } else { (1.0 - alpha) * o + alpha * n })
            .collect();
        let change = relative_change(&self.nut, &mixed);
        self.nut = mixed;
        self.started = true;
        change
    }
    fn nu_face(&self) -> Vec<f64> {
        self.wall.nu_face(&self.nut)
    }
    fn nut(&self) -> &[f64] {
        &self.nut
    }
    fn wall_distance(&self) -> &[f64] {
        &self.wall.dist
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        Vec::new()
    }
    fn scalar_mut(&mut self, _name: &str) -> Option<&mut ScalarField> {
        None
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        self.wall.y_plus_faces()
    }
    fn wall_faces(&self) -> Vec<usize> {
        self.wall.faces.clone()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        mixing_length_infos(self.kappa, self.l_max_ratio, self.a_plus)
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String> {
        let infos = self.coefficients();
        let c = infos
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| format!("unknown coefficient '{name}'"))?;
        if !value.is_finite() || value < c.min || value > c.max {
            return Err(format!("{name} = {value} is outside [{}, {}]", c.min, c.max));
        }
        match name {
            "kappa" => self.kappa = value,
            "l_max_ratio" => self.l_max_ratio = value,
            _ => self.a_plus = value,
        }
        Ok(())
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        self.nut = nut.to_vec();
    }
}
