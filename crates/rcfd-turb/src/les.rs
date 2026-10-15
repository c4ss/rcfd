//! Algebraic subgrid-scale eddy-viscosity models for large-eddy simulation (F-045, ADR 0026): Smagorinsky and WALE.
//!
//! Both are evaluated explicitly from the velocity gradient of the current cell-centred velocity; the filter width is the
//! cube root of the cell volume. The pointwise functions ([`smagorinsky_nu`], [`wale_nu`], [`van_driest`]) are public so that
//! their closed forms can be tested without a mesh. The wall helper [`WallInfo`] (nearest wall face, wall shear velocity,
//! `y+`) is shared with the mixing-length closure in `zeroeq.rs`.

use crate::eddy::{effective_viscosity, patch_faces};
use crate::model::{CoefficientInfo, ModelContext, TurbulenceModel};
use crate::wall::wall_distance;
use rcfd_core::{vec3, Result};
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// Velocity gradient tensor `g[c][i][j] = d u_i / d x_j` of every cell (Green-Gauss, boundary values from the
/// velocity boundary conditions).
pub fn velocity_gradients(geom: &FvGeometry, u: &VectorField) -> Vec<[[f64; 3]; 3]> {
    let ub = u.boundary_values(geom);
    let g: Vec<Vec<[f64; 3]>> = (0..3)
        .map(|k| {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            green_gauss(geom, &cells, &bv, 0)
        })
        .collect();
    (0..geom.n_cells())
        .map(|c| {
            let mut t = [[0.0; 3]; 3];
            for (i, row) in t.iter_mut().enumerate() {
                *row = g[i][c];
            }
            t
        })
        .collect()
}

/// Filter width of a cell, `V^(1/3)`.
pub fn filter_width(volume: f64) -> f64 {
    volume.cbrt()
}

/// `sqrt(2 S_ij S_ij)` of a velocity gradient `g_ij = d u_i / d x_j`.
pub fn strain_magnitude(g: &[[f64; 3]; 3]) -> f64 {
    let mut s2 = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            let s = 0.5 * (g[i][j] + g[j][i]);
            s2 += s * s;
        }
    }
    (2.0 * s2).sqrt()
}

/// Smagorinsky subgrid viscosity `(Cs Delta)^2 |S|`.
pub fn smagorinsky_nu(g: &[[f64; 3]; 3], delta: f64, cs: f64) -> f64 {
    let l = cs * delta;
    l * l * strain_magnitude(g)
}

/// WALE subgrid viscosity (Nicoud and Ducros 1999). Zero when the denominator is exactly zero (uniform flow).
pub fn wale_nu(g: &[[f64; 3]; 3], delta: f64, cw: f64) -> f64 {
    let mut g2 = [[0.0; 3]; 3];
    for (i, row) in g2.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| g[i][k] * g[k][j]).sum();
        }
    }
    let tr = g2[0][0] + g2[1][1] + g2[2][2];
    let (mut sd2, mut ss) = (0.0, 0.0);
    for i in 0..3 {
        for j in 0..3 {
            let mut sd = 0.5 * (g2[i][j] + g2[j][i]);
            if i == j {
                sd -= tr / 3.0;
            }
            sd2 += sd * sd;
            let s = 0.5 * (g[i][j] + g[j][i]);
            ss += s * s;
        }
    }
    let den = ss.powf(2.5) + sd2.powf(1.25);
    if den == 0.0 {
        return 0.0;
    }
    let l = cw * delta;
    l * l * sd2.powf(1.5) / den
}

/// Van Driest damping factor `1 - exp(-y+/A+)`.
pub fn van_driest(y_plus: f64, a_plus: f64) -> f64 {
    1.0 - (-y_plus / a_plus).exp()
}

/// Wall bookkeeping shared by the algebraic models: wall faces, the exact wall distance of the cells, the nearest wall
/// face of every cell and the wall shear velocity of the wall faces. All empty when the model has no wall patch.
pub struct WallInfo {
    /// Wall faces (global indices).
    pub faces: Vec<usize>,
    /// Exact wall distance of the cells (`1e30` when there is no wall).
    pub dist: Vec<f64>,
    /// For every cell the index into `faces` of the nearest wall face (by face-centre distance).
    pub nearest: Vec<usize>,
    /// Wall shear velocity of the wall faces from the last [`WallInfo::update_shear`].
    pub u_tau: Vec<f64>,
    nu: f64,
    geom: Arc<FvGeometry>,
}

impl WallInfo {
    /// Build for the named wall patches (may be empty).
    pub fn new(geom: &Arc<FvGeometry>, walls: &[&str], nu: f64) -> Result<Self> {
        let faces = patch_faces(geom, walls);
        let n = geom.n_cells();
        let (dist, nearest) = if faces.is_empty() {
            (vec![1e30; n], Vec::new())
        } else {
            let d = wall_distance(&geom.mesh, walls)?;
            let cc = geom.mesh.cell_centres();
            let fc = geom.mesh.face_centres();
            let near = (0..n)
                .map(|c| {
                    let mut best = (f64::INFINITY, 0);
                    for (i, &f) in faces.iter().enumerate() {
                        let r = vec3::sub(cc[c], fc[f]);
                        let d2 = vec3::dot(r, r);
                        if d2 < best.0 {
                            best = (d2, i);
                        }
                    }
                    best.1
                })
                .collect();
            (d, near)
        };
        Ok(WallInfo {
            u_tau: vec![0.0; faces.len()],
            faces,
            dist,
            nearest,
            nu,
            geom: geom.clone(),
        })
    }

    /// Recompute the wall shear velocity of the wall faces: `u_tau^2 = nu |u_t,c - u_t,w| / d_n`, the tangential velocity
    /// of the wall cell relative to the boundary value.
    pub fn update_shear(&mut self, u: &VectorField) {
        if self.faces.is_empty() {
            return;
        }
        let g = &self.geom;
        let ub = u.boundary_values(g);
        for (i, &f) in self.faces.iter().enumerate() {
            let c = g.mesh.owner()[f];
            let n = g.normal(f);
            let rel = vec3::sub(u.values[c], ub[f - g.n_internal]);
            let t = vec3::sub(rel, vec3::scale(n, vec3::dot(rel, n)));
            let dn = g.dn[f - g.n_internal];
            self.u_tau[i] = (self.nu * vec3::norm(t) / dn).sqrt();
        }
    }

    /// `y+` of every cell, `d u_tau / nu` with the shear velocity of the nearest wall face (0 without walls).
    pub fn y_plus_cells(&self) -> Vec<f64> {
        if self.faces.is_empty() {
            return vec![0.0; self.dist.len()];
        }
        (0..self.dist.len())
            .map(|c| self.dist[c] * self.u_tau[self.nearest[c]] / self.nu)
            .collect()
    }

    /// `y+` of the wall faces (first-cell centre distance).
    pub fn y_plus_faces(&self) -> Vec<f64> {
        let g = &self.geom;
        self.faces
            .iter()
            .zip(&self.u_tau)
            .map(|(&f, &ut)| g.dn[f - g.n_internal] * ut / self.nu)
            .collect()
    }

    /// Effective face viscosity `nu + nu_t` with `nu_t = 0` on the wall faces (wall-resolved treatment).
    pub fn nu_face(&self, nut: &[f64]) -> Vec<f64> {
        let wall_nut: Vec<(usize, f64)> = self.faces.iter().map(|&f| (f, 0.0)).collect();
        effective_viscosity(&self.geom, self.nu, nut, &wall_nut)
    }
}

/// Largest change of `new` against `old`, relative to the largest new value (0 when everything is 0).
pub(crate) fn relative_change(old: &[f64], new: &[f64]) -> f64 {
    let scale = new.iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
    if scale == 0.0 {
        return old.iter().fold(0.0_f64, |a, &b| a.max(b.abs())).min(1.0);
    }
    old.iter().zip(new).fold(0.0_f64, |a, (&o, &n)| a.max((n - o).abs())) / scale
}

/// Which subgrid model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgsKind {
    /// Smagorinsky (1963).
    Smagorinsky,
    /// Wall-adapting local eddy viscosity (Nicoud and Ducros 1999).
    Wale,
}

/// Default Smagorinsky constant.
pub const CS_DEFAULT: f64 = 0.17;
/// Default WALE constant.
pub const CW_DEFAULT: f64 = 0.325;
/// Default van Driest constant of the subgrid model.
pub const A_PLUS_SGS: f64 = 25.0;

/// Algebraic subgrid-scale model behind the turbulence interface.
pub struct AlgebraicSgs {
    /// Model.
    pub kind: SgsKind,
    /// Smagorinsky constant.
    pub cs: f64,
    /// WALE constant.
    pub cw: f64,
    /// Van Driest constant (Smagorinsky with damping).
    pub a_plus: f64,
    /// Van Driest damping of the Smagorinsky length (needs a wall).
    pub damping: bool,
    geom: Arc<FvGeometry>,
    delta: Vec<f64>,
    wall: WallInfo,
    nut: Vec<f64>,
}

impl AlgebraicSgs {
    /// Build from a context.
    pub fn new(kind: SgsKind, ctx: &ModelContext) -> Result<Self> {
        let walls: Vec<&str> = ctx.walls.iter().map(String::as_str).collect();
        let wall = WallInfo::new(&ctx.geom, &walls, ctx.nu)?;
        let delta = ctx.geom.mesh.cell_volumes().iter().map(|&v| filter_width(v)).collect();
        Ok(AlgebraicSgs {
            kind,
            cs: CS_DEFAULT,
            cw: CW_DEFAULT,
            a_plus: A_PLUS_SGS,
            damping: ctx.wall_damping && kind == SgsKind::Smagorinsky,
            nut: vec![0.0; ctx.geom.n_cells()],
            geom: ctx.geom.clone(),
            delta,
            wall,
        })
    }

    /// Filter width of the cells.
    pub fn filter_widths(&self) -> &[f64] {
        &self.delta
    }
}

/// Coefficient list of a subgrid model for the given values.
pub fn sgs_infos(kind: SgsKind, cs: f64, cw: f64, a_plus: f64) -> Vec<CoefficientInfo> {
    match kind {
        SgsKind::Smagorinsky => vec![
            CoefficientInfo {
                name: "cs",
                value: cs,
                default: CS_DEFAULT,
                min: 0.0,
                max: 0.5,
                doc: "Smagorinsky constant Cs (nu_sgs = (Cs Delta)^2 |S|, Delta = cell volume^(1/3)); 0 switches the model off",
            },
            CoefficientInfo {
                name: "a_plus",
                value: a_plus,
                default: A_PLUS_SGS,
                min: 1.0,
                max: 100.0,
                doc: "van Driest constant A+ of the optional wall damping 1 - exp(-y+/A+)",
            },
        ],
        SgsKind::Wale => vec![CoefficientInfo {
            name: "cw",
            value: cw,
            default: CW_DEFAULT,
            min: 0.0,
            max: 1.0,
            doc: "WALE constant Cw (nu_sgs = (Cw Delta)^2 (Sd:Sd)^(3/2) / ((S:S)^(5/2) + (Sd:Sd)^(5/4)), Delta = cell volume^(1/3))",
        }],
    }
}

impl TurbulenceModel for AlgebraicSgs {
    fn key(&self) -> &str {
        match self.kind {
            SgsKind::Smagorinsky => "smagorinsky",
            SgsKind::Wale => "wale",
        }
    }
    fn update(&mut self, u: &VectorField, _phi: &[f64]) -> f64 {
        let g = velocity_gradients(&self.geom, u);
        let new: Vec<f64> = if self.damping {
            self.wall.update_shear(u);
            let yp = self.wall.y_plus_cells();
            g.iter()
                .enumerate()
                .map(|(c, gc)| {
                    let f = van_driest(yp[c], self.a_plus);
                    smagorinsky_nu(gc, self.delta[c] * f, self.cs)
                })
                .collect()
        } else {
            self.wall.update_shear(u);
            g.iter()
                .enumerate()
                .map(|(c, gc)| match self.kind {
                    SgsKind::Smagorinsky => smagorinsky_nu(gc, self.delta[c], self.cs),
                    SgsKind::Wale => wale_nu(gc, self.delta[c], self.cw),
                })
                .collect()
        };
        let change = relative_change(&self.nut, &new);
        self.nut = new;
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
        sgs_infos(self.kind, self.cs, self.cw, self.a_plus)
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
            "cs" => self.cs = value,
            "cw" => self.cw = value,
            _ => self.a_plus = value,
        }
        Ok(())
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        self.nut = nut.to_vec();
    }
}
