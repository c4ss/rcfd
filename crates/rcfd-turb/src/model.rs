//! Turbulence-model interface and registry (F-044, ADR 0014).
//!
//! [`TurbulenceModel`] is what the solver layer needs from a RANS closure: one sweep per iteration on the current
//! velocity and flux, the effective face viscosity, the eddy viscosity, the transported scalars, the wall `y+`, and the
//! list of coefficients with their ranges so that they can be shown and edited in the settings tree. The built-in models
//! (k-epsilon family, k-omega SST, Spalart-Allmaras) are registered under `k_epsilon`, `k_omega_sst` and
//! `spalart_allmaras`; an external crate adds its own with [`register_model`] and selects it with
//! `turbulence.model = "plugin"`, `turbulence.plugin = "<key>"`.

use crate::eddy::patch_faces;
use crate::kepsilon::{KEpsilon, KEpsilonVariant, WallFunction, WallTreatment};
use crate::sa::{SLimiter, SaWallMode, SpalartAllmaras};
use crate::sst::{KOmegaSst, SstVariant, SstWallMode};
use crate::wall::Roughness;
use rcfd_core::{Error, Result};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::{Arc, Mutex, OnceLock};

/// A model coefficient as shown in the settings tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CoefficientInfo {
    /// Name (the key under `turbulence.coefficients.<model>`).
    pub name: &'static str,
    /// Current value.
    pub value: f64,
    /// Default (published) value.
    pub default: f64,
    /// Smallest accepted value.
    pub min: f64,
    /// Largest accepted value.
    pub max: f64,
    /// One-line description.
    pub doc: &'static str,
}

/// What a model factory gets.
#[derive(Clone)]
pub struct ModelContext {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Wall patch names.
    pub walls: Vec<String>,
    /// Initial `k`.
    pub k0: f64,
    /// Initial epsilon (k-epsilon).
    pub eps0: f64,
    /// Initial omega (SST).
    pub omega0: f64,
    /// Initial `nuT` (Spalart-Allmaras).
    pub nutilde0: f64,
    /// k-epsilon variant.
    pub variant: KEpsilonVariant,
    /// k-epsilon wall-cell treatment.
    pub wall_treatment: WallTreatment,
    /// Spalart-Allmaras wall mode.
    pub sa_wall_mode: SaWallMode,
    /// Spalart-Allmaras `S~` limiter.
    pub sa_limiter: SLimiter,
    /// Spalart-Allmaras negative-`nuT` treatment.
    pub sa_negative: bool,
    /// Log-law constant.
    pub kappa: f64,
    /// Log-law roughness parameter.
    pub e_wall: f64,
    /// Convection scheme of the transported scalars.
    pub convection: ConvScheme,
    /// Implicit under-relaxation.
    pub relax_turb: f64,
    /// Eddy-viscosity under-relaxation.
    pub relax_nut: f64,
}

/// The near-wall options of a case (F-203, F-043, ADR 0015): `turbulence.wall_function`, `roughness_height`,
/// `roughness_constant`, `wall_omega`, `sst_variant`. The settings-tree reading lives in `rcfd-api`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallOptions {
    /// k-epsilon wall treatment.
    pub wall_function: WallFunction,
    /// Sand-grain roughness height (0: smooth).
    pub roughness_height: f64,
    /// Roughness constant `C_s` (0: derived from the smooth log law, the Nikuradse value).
    pub roughness_constant: f64,
    /// SST wall omega treatment.
    pub wall_omega: SstWallMode,
    /// Menter variant.
    pub sst_variant: SstVariant,
}

impl WallOptions {
    /// Roughness for a log law with the given constants.
    pub fn roughness(&self, kappa: f64, e: f64) -> Roughness {
        if self.roughness_height > 0.0 {
            if self.roughness_constant > 0.0 {
                Roughness {
                    ks: self.roughness_height,
                    cs: self.roughness_constant,
                }
            } else {
                Roughness::new(self.roughness_height, kappa, e)
            }
        } else {
            Roughness::SMOOTH
        }
    }
}

/// Put the options on a k-epsilon model (uses its own log-law constants for the derived roughness constant).
pub fn apply_kepsilon(o: &WallOptions, m: &mut KEpsilon) {
    m.wall_function = o.wall_function;
    m.roughness = o.roughness(m.wc.kappa, m.wc.e);
}

/// Put the options on a k-omega SST model.
pub fn apply_sst(o: &WallOptions, m: &mut KOmegaSst) -> Result<()> {
    m.variant = o.sst_variant;
    m.roughness = o.roughness(m.law.kappa, m.law.e);
    m.set_wall_mode(o.wall_omega)
}

/// A RANS turbulence closure.
pub trait TurbulenceModel: Send {
    /// Registry key (`k_epsilon`, `k_omega_sst`, `spalart_allmaras`, or a plugin's).
    fn key(&self) -> &str;
    /// One sweep on the velocity `u` and flux `phi`; returns the largest relative change of the eddy viscosity.
    fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64;
    /// Effective face viscosity `nu + nu_t` for the momentum equation.
    fn nu_face(&self) -> Vec<f64>;
    /// Eddy viscosity of the cells.
    fn nut(&self) -> &[f64];
    /// Wall distance of the cells.
    fn wall_distance(&self) -> &[f64];
    /// Transported scalars `(field name, field)`.
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)>;
    /// Mutable access to a transported scalar by name.
    fn scalar_mut(&mut self, name: &str) -> Option<&mut ScalarField>;
    /// Wall-face `y+` in the order of the wall patches' faces.
    fn y_plus_faces(&self) -> Vec<f64>;
    /// Wall faces (global indices) matching [`TurbulenceModel::y_plus_faces`].
    fn wall_faces(&self) -> Vec<usize>;
    /// Coefficients with current values, defaults and ranges.
    fn coefficients(&self) -> Vec<CoefficientInfo>;
    /// Set a coefficient; `Err` carries the reason (unknown name, out of range, not overridable).
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String>;
    /// Start from the eddy viscosity of another model.
    fn initialize_from_nut(&mut self, nut: &[f64]);
    /// Apply the near-wall options of the case (called once after construction). The default accepts the
    /// default options only for models without wall options; an option set that the model cannot honour is
    /// reported, not ignored.
    fn apply_wall_options(&mut self, _o: &WallOptions) -> std::result::Result<(), String> {
        Ok(())
    }
}

fn check_range(infos: &[CoefficientInfo], name: &str, v: f64) -> std::result::Result<usize, String> {
    let i = infos
        .iter()
        .position(|c| c.name == name)
        .ok_or_else(|| format!("unknown coefficient '{name}'"))?;
    let c = &infos[i];
    if !v.is_finite() || v < c.min || v > c.max {
        return Err(format!("{name} = {v} is outside [{}, {}]", c.min, c.max));
    }
    Ok(i)
}

// ---------------------------------------------------------------- k-epsilon

/// k-epsilon family behind the interface.
pub struct KEpsilonModel {
    /// The model.
    pub m: KEpsilon,
    wall_faces: Vec<usize>,
}

fn ke_defaults(v: KEpsilonVariant) -> [f64; 5] {
    match v {
        KEpsilonVariant::Standard => [0.09, 1.44, 1.92, 1.0, 1.3],
        KEpsilonVariant::Rng => [0.0845, 1.42, 1.68, 0.7194, 0.7194],
        KEpsilonVariant::Realizable => [0.09, 1.44, 1.9, 1.0, 1.2],
    }
}

impl KEpsilonModel {
    /// Build from a context.
    pub fn new(ctx: &ModelContext) -> Result<Self> {
        let walls: Vec<&str> = ctx.walls.iter().map(String::as_str).collect();
        let mut m = KEpsilon::new(ctx.variant, ctx.geom.clone(), ctx.nu, &walls, ctx.k0, ctx.eps0)?;
        m.wc.kappa = ctx.kappa;
        m.wc.e = ctx.e_wall;
        m.wall_treatment = ctx.wall_treatment;
        m.relax_turb = ctx.relax_turb;
        m.relax_nut = ctx.relax_nut;
        m.convection = ctx.convection;
        let wall_faces = patch_faces(&ctx.geom, &walls);
        Ok(KEpsilonModel { m, wall_faces })
    }
}

impl TurbulenceModel for KEpsilonModel {
    fn apply_wall_options(&mut self, o: &WallOptions) -> std::result::Result<(), String> {
        apply_kepsilon(o, &mut self.m);
        Ok(())
    }
    fn key(&self) -> &str {
        "k_epsilon"
    }
    fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        self.m.update(u, phi)
    }
    fn nu_face(&self) -> Vec<f64> {
        self.m.nu_face()
    }
    fn nut(&self) -> &[f64] {
        &self.m.nut
    }
    fn wall_distance(&self) -> &[f64] {
        self.m.wall_distance()
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        vec![("k", &self.m.k), ("epsilon", &self.m.eps)]
    }
    fn scalar_mut(&mut self, name: &str) -> Option<&mut ScalarField> {
        match name {
            "k" => Some(&mut self.m.k),
            "epsilon" => Some(&mut self.m.eps),
            _ => None,
        }
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        let g = &self.m.geom;
        let c25 = self.m.c_mu.powf(0.25);
        self.wall_faces
            .iter()
            .map(|&f| {
                let c = g.mesh.owner()[f];
                c25 * self.m.k.values[c].max(0.0).sqrt() * g.dn[f - g.n_internal] / self.m.nu
            })
            .collect()
    }
    fn wall_faces(&self) -> Vec<usize> {
        self.wall_faces.clone()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        let d = ke_defaults(self.m.variant);
        let m = &self.m;
        let mk = |name, value, default, min, max, doc| CoefficientInfo {
            name,
            value,
            default,
            min,
            max,
            doc,
        };
        vec![
            mk(
                "c_mu",
                m.c_mu,
                d[0],
                0.01,
                0.2,
                "eddy-viscosity constant Cmu (also the wall-function constant)",
            ),
            mk(
                "c1",
                m.c1,
                d[1],
                0.5,
                3.0,
                "epsilon production constant C1 (standard, rng; the realizable C1 is variable)",
            ),
            mk("c2", m.c2, d[2], 0.5, 4.0, "epsilon destruction constant C2"),
            mk("sigma_k", m.sigma_k, d[3], 0.1, 5.0, "turbulent Prandtl number of k"),
            mk(
                "sigma_e",
                m.sigma_e,
                d[4],
                0.1,
                5.0,
                "turbulent Prandtl number of epsilon",
            ),
        ]
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String> {
        check_range(&self.coefficients(), name, value)?;
        match name {
            "c_mu" => {
                self.m.c_mu = value;
                self.m.wc.cmu = value;
            }
            "c1" => {
                if self.m.variant == KEpsilonVariant::Realizable {
                    return Err(
                        "c1 does not apply to the realizable variant (its C1 = max(0.43, eta/(eta+5)) is variable)"
                            .into(),
                    );
                }
                self.m.c1 = value;
            }
            "c2" => self.m.c2 = value,
            "sigma_k" => self.m.sigma_k = value,
            _ => self.m.sigma_e = value,
        }
        Ok(())
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        for (c, &v) in nut.iter().enumerate() {
            let k = self.m.k.values[c].max(1e-12);
            self.m.eps.values[c] = (self.m.c_mu * k * k / v.max(1e-30)).max(1e-10);
        }
        self.m.nut = nut.to_vec();
    }
}

// ---------------------------------------------------------------- SST

/// Menter 2003 values (TMR) shown for the SST model. The constants are `const` items in `sst.rs`; this table repeats
/// them for display (a value other than the default is rejected, see [`SstModel::set_coefficient`]).
pub const SST_COEFFICIENTS: [(&str, f64, &str); 11] = [
    ("beta_star", 0.09, "beta*, k destruction and the k-epsilon Cmu"),
    (
        "a1",
        0.31,
        "a1 of the shear-stress limiter nu_t = a1 k/max(a1 omega, S F2)",
    ),
    ("sigma_k1", 0.85, "k diffusion constant, inner (k-omega) set"),
    ("sigma_w1", 0.5, "omega diffusion constant, inner set"),
    ("beta1", 0.075, "omega destruction constant, inner set"),
    ("gamma1", 5.0 / 9.0, "omega production constant, inner set"),
    ("sigma_k2", 1.0, "k diffusion constant, outer (k-epsilon) set"),
    (
        "sigma_w2",
        0.856,
        "omega diffusion constant, outer set (also in the cross-diffusion term)",
    ),
    ("beta2", 0.0828, "omega destruction constant, outer set"),
    ("gamma2", 0.44, "omega production constant, outer set"),
    (
        "production_limiter",
        10.0,
        "production limiter factor: P <= factor beta* k omega",
    ),
];

/// k-omega SST behind the interface.
pub struct SstModel {
    /// The model.
    pub m: KOmegaSst,
    wall_faces: Vec<usize>,
}

impl SstModel {
    /// Build from a context.
    pub fn new(ctx: &ModelContext) -> Result<Self> {
        let walls: Vec<&str> = ctx.walls.iter().map(String::as_str).collect();
        let mut m = KOmegaSst::new(ctx.geom.clone(), ctx.nu, &walls, ctx.k0, ctx.omega0)?;
        m.law.kappa = ctx.kappa;
        m.law.e = ctx.e_wall;
        m.relax_turb = ctx.relax_turb;
        m.relax_nut = ctx.relax_nut;
        m.convection = ctx.convection;
        let wall_faces = patch_faces(&ctx.geom, &walls);
        Ok(SstModel { m, wall_faces })
    }
}

impl TurbulenceModel for SstModel {
    fn apply_wall_options(&mut self, o: &WallOptions) -> std::result::Result<(), String> {
        apply_sst(o, &mut self.m).map_err(|e| e.to_string())
    }
    fn key(&self) -> &str {
        "k_omega_sst"
    }
    fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        self.m.update(u, phi)
    }
    fn nu_face(&self) -> Vec<f64> {
        self.m.nu_face()
    }
    fn nut(&self) -> &[f64] {
        &self.m.nut
    }
    fn wall_distance(&self) -> &[f64] {
        self.m.wall_distance()
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        vec![("k", &self.m.k), ("omega", &self.m.omega)]
    }
    fn scalar_mut(&mut self, name: &str) -> Option<&mut ScalarField> {
        match name {
            "k" => Some(&mut self.m.k),
            "omega" => Some(&mut self.m.omega),
            _ => None,
        }
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        self.m.y_plus().to_vec()
    }
    fn wall_faces(&self) -> Vec<usize> {
        self.wall_faces.clone()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        SST_COEFFICIENTS
            .iter()
            .map(|&(name, v, doc)| CoefficientInfo {
                name,
                value: v,
                default: v,
                min: 0.0,
                max: 100.0,
                doc,
            })
            .collect()
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String> {
        let infos = self.coefficients();
        let i = check_range(&infos, name, value)?;
        if value == infos[i].default {
            Ok(())
        } else {
            Err(format!(
                "the k-omega SST coefficients are fixed at their published (TMR, Menter 2003) values in this version; {name} = {value} differs from {}",
                infos[i].default
            ))
        }
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        for (c, &v) in nut.iter().enumerate() {
            self.m.omega.values[c] = (self.m.k.values[c].max(1e-12) / v.max(1e-30)).max(1e-10);
        }
        self.m.nut = nut.to_vec();
    }
}

/// k-epsilon coefficient list for the settings schema: value and default 0 stand for "the variant's own value".
pub fn ke_coefficient_table() -> Vec<CoefficientInfo> {
    let std = ke_defaults(KEpsilonVariant::Standard);
    let _ = std;
    let mk = |name, min, max, doc| CoefficientInfo {
        name,
        value: 0.0,
        default: 0.0,
        min,
        max,
        doc,
    };
    vec![
        mk(
            "c_mu",
            0.01,
            0.2,
            "eddy-viscosity constant Cmu (0 = variant default: 0.09, RNG 0.0845)",
        ),
        mk(
            "c1",
            0.5,
            3.0,
            "epsilon production constant C1 (0 = variant default: 1.44, RNG 1.42; not for realizable)",
        ),
        mk(
            "c2",
            0.5,
            4.0,
            "epsilon destruction constant C2 (0 = variant default: standard 1.92, RNG 1.68, realizable 1.9)",
        ),
        mk(
            "sigma_k",
            0.1,
            5.0,
            "turbulent Prandtl number of k (0 = variant default: 1.0, RNG 0.7194)",
        ),
        mk(
            "sigma_e",
            0.1,
            5.0,
            "turbulent Prandtl number of epsilon (0 = variant default: standard 1.3, RNG 0.7194, realizable 1.2)",
        ),
    ]
}

/// Coefficient list of the SST model for the settings schema.
pub fn sst_coefficient_table() -> Vec<CoefficientInfo> {
    SST_COEFFICIENTS
        .iter()
        .map(|&(name, v, doc)| CoefficientInfo {
            name,
            value: v,
            default: v,
            min: 0.0,
            max: 100.0,
            doc,
        })
        .collect()
}

// ---------------------------------------------------------------- SA

/// Coefficient list of the Spalart-Allmaras model for the given values (defaults are the TMR values).
pub fn sa_infos(c: crate::sa::SaCoefficients) -> Vec<CoefficientInfo> {
    let d = crate::sa::SaCoefficients::default();
    let mk = |name, value, default, min, max, doc| CoefficientInfo {
        name,
        value,
        default,
        min,
        max,
        doc,
    };
    vec![
        mk("sigma", c.sigma, d.sigma, 0.1, 5.0, "diffusion constant sigma (2/3)"),
        mk("cb1", c.cb1, d.cb1, 0.0, 1.0, "production constant cb1"),
        mk("cb2", c.cb2, d.cb2, 0.0, 5.0, "cb2 of the |grad nuT|^2 term"),
        mk(
            "kappa",
            c.kappa,
            d.kappa,
            0.2,
            0.6,
            "von Karman constant inside the model (cw1 follows from it)",
        ),
        mk("cw2", c.cw2, d.cw2, 0.0, 2.0, "cw2 of the fw function"),
        mk("cw3", c.cw3, d.cw3, 0.5, 10.0, "cw3 of the fw function"),
        mk("cv1", c.cv1, d.cv1, 1.0, 20.0, "cv1 of the viscous damping fv1"),
        mk(
            "cn1",
            c.cn1,
            d.cn1,
            1.0,
            100.0,
            "cn1 of the negative-nuT diffusion factor fn",
        ),
        mk(
            "c2",
            c.c2,
            d.c2,
            0.0,
            1.0,
            "c2 of the Allmaras-Johnson-Spalart S~ limiter",
        ),
        mk(
            "c3",
            c.c3,
            d.c3,
            0.0,
            1.5,
            "c3 of the Allmaras-Johnson-Spalart S~ limiter",
        ),
    ]
}

/// Build the Spalart-Allmaras model from a context.
pub fn new_spalart_allmaras(ctx: &ModelContext) -> Result<SpalartAllmaras> {
    let walls: Vec<&str> = ctx.walls.iter().map(String::as_str).collect();
    let mut m = SpalartAllmaras::new(ctx.geom.clone(), ctx.nu, &walls, ctx.nutilde0)?;
    m.law.kappa = ctx.kappa;
    m.law.e = ctx.e_wall;
    m.relax_turb = ctx.relax_turb;
    m.relax_nut = ctx.relax_nut;
    m.convection = ctx.convection;
    m.wall_mode = ctx.sa_wall_mode;
    m.limiter = ctx.sa_limiter;
    m.negative_nutilde = ctx.sa_negative;
    Ok(m)
}

impl TurbulenceModel for SpalartAllmaras {
    fn key(&self) -> &str {
        "spalart_allmaras"
    }
    fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        SpalartAllmaras::update(self, u, phi)
    }
    fn nu_face(&self) -> Vec<f64> {
        SpalartAllmaras::nu_face(self)
    }
    fn nut(&self) -> &[f64] {
        &self.nut
    }
    fn wall_distance(&self) -> &[f64] {
        SpalartAllmaras::wall_distance(self)
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        vec![("nuTilda", &self.nutilde)]
    }
    fn scalar_mut(&mut self, name: &str) -> Option<&mut ScalarField> {
        (name == "nuTilda").then_some(&mut self.nutilde)
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        self.y_plus().to_vec()
    }
    fn wall_faces(&self) -> Vec<usize> {
        SpalartAllmaras::wall_faces(self).to_vec()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        sa_infos(self.coef)
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String> {
        check_range(&self.coefficients(), name, value)?;
        match name {
            "sigma" => self.coef.sigma = value,
            "cb1" => self.coef.cb1 = value,
            "cb2" => self.coef.cb2 = value,
            "kappa" => self.coef.kappa = value,
            "cw2" => self.coef.cw2 = value,
            "cw3" => self.coef.cw3 = value,
            "cv1" => self.coef.cv1 = value,
            "cn1" => self.coef.cn1 = value,
            "c2" => self.coef.c2 = value,
            _ => self.coef.c3 = value,
        }
        Ok(())
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        SpalartAllmaras::initialize_from_nut(self, nut)
    }
}

// ---------------------------------------------------------------- registry

/// A model factory.
pub type ModelFactory = Arc<dyn Fn(&ModelContext) -> Result<Box<dyn TurbulenceModel>> + Send + Sync>;

struct Entry {
    key: String,
    doc: String,
    factory: ModelFactory,
}

fn registry() -> &'static Mutex<Vec<Entry>> {
    static REG: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    REG.get_or_init(|| {
        let b = |key: &str, doc: &str, f: ModelFactory| Entry {
            key: key.into(),
            doc: doc.into(),
            factory: f,
        };
        Mutex::new(vec![
            b(
                "k_epsilon",
                "k-epsilon family (standard, RNG, realizable) with wall functions",
                Arc::new(|c| Ok(Box::new(KEpsilonModel::new(c)?) as Box<dyn TurbulenceModel>)),
            ),
            b(
                "k_omega_sst",
                "k-omega SST (Menter 2003) with automatic wall treatment",
                Arc::new(|c| Ok(Box::new(SstModel::new(c)?) as Box<dyn TurbulenceModel>)),
            ),
            b(
                "spalart_allmaras",
                "Spalart-Allmaras one-equation model (SA-noft2, SA-neg)",
                Arc::new(|c| Ok(Box::new(new_spalart_allmaras(c)?) as Box<dyn TurbulenceModel>)),
            ),
        ])
    })
}

/// Register (or replace) a model under `key`.
pub fn register_model(key: &str, doc: &str, factory: ModelFactory) {
    let mut r = registry().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = r.iter_mut().find(|e| e.key == key) {
        e.doc = doc.into();
        e.factory = factory;
    } else {
        r.push(Entry {
            key: key.into(),
            doc: doc.into(),
            factory,
        });
    }
}

/// Keys and descriptions of the registered models.
pub fn model_keys() -> Vec<(String, String)> {
    let r = registry().lock().unwrap_or_else(|e| e.into_inner());
    r.iter().map(|e| (e.key.clone(), e.doc.clone())).collect()
}

/// Is a model registered under `key`?
pub fn is_registered(key: &str) -> bool {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|e| e.key == key)
}

/// Build the model registered under `key`.
pub fn create_model(key: &str, ctx: &ModelContext) -> Result<Box<dyn TurbulenceModel>> {
    let f = {
        let r = registry().lock().unwrap_or_else(|e| e.into_inner());
        r.iter().find(|e| e.key == key).map(|e| e.factory.clone())
    };
    match f {
        Some(f) => f(ctx),
        None => Err(Error::InvalidValue {
            path: "turbulence.plugin".into(),
            reason: format!(
                "no turbulence model registered as '{key}' (registered: {})",
                model_keys().iter().map(|k| k.0.clone()).collect::<Vec<_>>().join(", ")
            ),
        }),
    }
}
