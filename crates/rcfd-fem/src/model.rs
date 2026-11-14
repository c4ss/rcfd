//! The solid model (ADR 0271 D5): numbering of the unknowns, materials, constraints, loads, the model and its result.
//!
//! Every type is built through a constructor that validates; structs have private fields and enums are non-exhaustive,
//! so that a field or variant added by a later tier breaks no caller (the request of ADR 0290 section 2.4). Loads and
//! constraints are plain data naming sets, readable from a file; there are no closures.

use crate::error::{FemError, Result, SetKind};
use crate::material::{
    finite_material, positive_material, HyperelasticEnergy, MaterialAxes, PronyTerm, TimeTemperatureShift,
};
use crate::mesh::SolidMesh;
use rcfd_core::vec3::Vec3;

/// Numbering of the unknowns: (node, component) to equation. The only place that knows how many unknowns a node has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofMap {
    offsets: Vec<usize>,
}

impl DofMap {
    /// `per_node` unknowns at each of `n_nodes` nodes, numbered node by node.
    pub fn uniform(n_nodes: usize, per_node: usize) -> DofMap {
        DofMap {
            offsets: (0..=n_nodes).map(|n| n * per_node).collect(),
        }
    }

    /// The given number of unknowns at each node, numbered node by node.
    pub fn from_counts(counts: &[usize]) -> DofMap {
        let mut offsets = Vec::with_capacity(counts.len() + 1);
        let mut s = 0;
        offsets.push(0);
        for &c in counts {
            s += c;
            offsets.push(s);
        }
        DofMap { offsets }
    }

    /// The numbering a mesh needs: three translations per node, and six unknowns (the translations, then the small
    /// rotations about x, y, z) at each node of a Beam2 element (ADR 0417 D1), of a Shell4 element (ADR 0432 D1) or of a
    /// Shell3 element (ADR 0440 D4). A mesh without a Beam2, Shell4 or Shell3 block gets `DofMap::uniform(n, 3)`.
    pub fn for_mesh(mesh: &SolidMesh) -> DofMap {
        use crate::mesh::ElementKind::{Beam2, Shell3, Shell4};
        if mesh
            .blocks()
            .iter()
            .all(|b| !matches!(b.kind(), Beam2 | Shell4 | Shell3))
        {
            return DofMap::uniform(mesh.nodes().len(), 3);
        }
        let mut counts = vec![3; mesh.nodes().len()];
        for e in 0..mesh.n_elements() {
            if matches!(mesh.element_kind(e), Beam2 | Shell4 | Shell3) {
                for &n in mesh.element_nodes(e) {
                    counts[n] = 6;
                }
            }
        }
        DofMap::from_counts(&counts)
    }

    /// Number of nodes.
    pub fn n_nodes(&self) -> usize {
        self.offsets.len() - 1
    }
    /// Number of unknowns.
    pub fn n_equations(&self) -> usize {
        *self.offsets.last().expect("offsets has n_nodes + 1 entries")
    }
    /// Number of unknowns of a node (0 for a node that does not exist).
    pub fn components(&self, node: usize) -> usize {
        if node + 1 < self.offsets.len() {
            self.offsets[node + 1] - self.offsets[node]
        } else {
            0
        }
    }
    /// The equation of (node, component), or `None` if the node or the component does not exist.
    pub fn equation(&self, node: usize, component: usize) -> Option<usize> {
        (component < self.components(node)).then(|| self.offsets[node] + component)
    }
    /// The (node, component) of an equation, or `None` beyond the last.
    pub fn node_component(&self, equation: usize) -> Option<(usize, usize)> {
        if equation >= self.n_equations() {
            return None;
        }
        let node = self.offsets.partition_point(|&o| o <= equation) - 1;
        Some((node, equation - self.offsets[node]))
    }
}

/// A material. Non-exhaustive. The isotropic elastic material is the one of the earlier tiers; the advanced tier's variants
/// (ADR 0310 section 2.3, declared by ADR 0440 D2) are plain data refused by every solve until their package lands
/// ([`Material::pending`]).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Material {
    /// Isotropic linear elasticity.
    #[non_exhaustive]
    IsotropicElastic {
        /// Young's modulus, Pa, positive.
        e: f64,
        /// Poisson ratio, in the open interval (-1, 0.5).
        nu: f64,
        /// Density, kg/m^3, non-negative.
        density: f64,
        /// Linear expansion coefficient, 1/K.
        expansion: f64,
    },
    /// Orthotropic linear elasticity in material axes (ADR 0312, F-409).
    #[non_exhaustive]
    Orthotropic {
        /// `E_1, E_2, E_3`, Pa.
        e: [f64; 3],
        /// `nu_12, nu_13, nu_23`.
        nu: [f64; 3],
        /// `G_12, G_13, G_23`, Pa.
        g: [f64; 3],
        /// Density, kg/m^3.
        density: f64,
        /// Expansion coefficients along the material axes, 1/K.
        expansion: [f64; 3],
        /// The rule giving the material axes.
        axes: MaterialAxes,
    },
    /// An elastic base with properties against temperature (ADR 0312, F-409): named columns over increasing temperatures.
    #[non_exhaustive]
    Tabulated {
        /// The elastic base (isotropic or orthotropic).
        base: Box<Material>,
        /// Temperatures, K, strictly increasing.
        temperatures: Vec<f64>,
        /// `(property, values at the temperatures)`.
        columns: Vec<(String, Vec<f64>)>,
    },
    /// Linear viscoelasticity: an elastic base relaxing by a Prony series (ADR 0313, F-410).
    #[non_exhaustive]
    Viscoelastic {
        /// The instantaneous elastic base.
        base: Box<Material>,
        /// The Prony terms.
        terms: Vec<PronyTerm>,
        /// The time-temperature shift, if any.
        shift: Option<TimeTemperatureShift>,
    },
    /// Power-law creep with time hardening, `d eps_c / dt = a sigma^n t^m`, on an elastic base (ADR 0314, F-411).
    #[non_exhaustive]
    Creep {
        /// The elastic base.
        base: Box<Material>,
        /// `a`, 1/(s Pa^n s^m).
        a: f64,
        /// The stress exponent `n`.
        n: f64,
        /// The time exponent `m`.
        m: f64,
    },
    /// J2 plasticity with linear isotropic and kinematic hardening on an isotropic base (ADR 0325, F-422).
    #[non_exhaustive]
    J2 {
        /// The isotropic elastic base.
        base: Box<Material>,
        /// Initial yield stress, Pa.
        yield_stress: f64,
        /// Isotropic hardening modulus, Pa.
        isotropic: f64,
        /// Kinematic hardening modulus, Pa.
        kinematic: f64,
    },
    /// A hyperelastic material (ADR 0328, F-425).
    #[non_exhaustive]
    Hyperelastic {
        /// The strain energy.
        energy: HyperelasticEnergy,
        /// Density, kg/m^3.
        density: f64,
    },
}

fn elastic_base(base: &Material, isotropic_only: bool) -> Result<()> {
    let ok = match base {
        Material::IsotropicElastic { .. } => true,
        Material::Orthotropic { .. } => !isotropic_only,
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key: "base",
            reason: format!(
                "a {} base: the base must be {}",
                base.name(),
                if isotropic_only {
                    "IsotropicElastic"
                } else {
                    "IsotropicElastic or Orthotropic"
                }
            ),
        })
    }
}

fn density_ok(density: f64) -> Result<()> {
    if density >= 0.0 && density.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidMaterial {
            key: "density",
            value: density,
            reason: "density must be non-negative and finite",
        })
    }
}

impl Material {
    /// An isotropic linear elastic material. Refuses (naming the key) `e <= 0`, `nu` outside (-1, 0.5), a negative
    /// density and any non-finite value.
    pub fn isotropic_elastic(e: f64, nu: f64, density: f64, expansion: f64) -> Result<Material> {
        if !(e > 0.0 && e.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key: "e",
                value: e,
                reason: "Young's modulus must be positive and finite",
            });
        }
        if !(nu > -1.0 && nu < 0.5) {
            return Err(FemError::InvalidMaterial {
                key: "nu",
                value: nu,
                reason: "Poisson ratio must lie in the open interval (-1, 0.5)",
            });
        }
        if !(density >= 0.0 && density.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key: "density",
                value: density,
                reason: "density must be non-negative and finite",
            });
        }
        if !expansion.is_finite() {
            return Err(FemError::InvalidMaterial {
                key: "expansion",
                value: expansion,
                reason: "expansion coefficient must be finite",
            });
        }
        Ok(Material::IsotropicElastic {
            e,
            nu,
            density,
            expansion,
        })
    }

    /// An orthotropic material (ADR 0440 D2): refuses a modulus that is not positive and finite, a non-finite ratio or
    /// expansion, a negative or non-finite density, and (ADR 0452 F2) constants whose compliance is not positive definite,
    /// naming the ratio ([`crate::mat_ortho::check_positive_definite`]).
    pub fn orthotropic(
        e: [f64; 3],
        nu: [f64; 3],
        g: [f64; 3],
        density: f64,
        expansion: [f64; 3],
        axes: MaterialAxes,
    ) -> Result<Material> {
        for (key, v) in [("e_1", e[0]), ("e_2", e[1]), ("e_3", e[2])] {
            positive_material(key, v)?;
        }
        for (key, v) in [("g_12", g[0]), ("g_13", g[1]), ("g_23", g[2])] {
            positive_material(key, v)?;
        }
        for (key, v) in [("nu_12", nu[0]), ("nu_13", nu[1]), ("nu_23", nu[2])] {
            finite_material(key, v)?;
        }
        for v in expansion {
            finite_material("expansion", v)?;
        }
        density_ok(density)?;
        crate::mat_ortho::check_positive_definite(e, nu, g)?;
        Ok(Material::Orthotropic {
            e,
            nu,
            g,
            density,
            expansion,
            axes,
        })
    }

    /// An elastic base with property columns against temperature (ADR 0440 D2): refuses a base that is not isotropic or
    /// orthotropic, fewer than two temperatures, temperatures not finite or not strictly increasing, no column, an empty
    /// column name, a column of the wrong length or with a non-finite entry.
    pub fn tabulated(base: Material, temperatures: Vec<f64>, columns: Vec<(String, Vec<f64>)>) -> Result<Material> {
        elastic_base(&base, false)?;
        let bad = |key: &'static str, reason: String| FemError::InvalidValue { key, reason };
        if temperatures.len() < 2 {
            return Err(bad(
                "temperatures",
                format!("{} temperatures: at least 2", temperatures.len()),
            ));
        }
        if !temperatures.iter().all(|t| t.is_finite()) || temperatures.windows(2).any(|w| w[1] <= w[0]) {
            return Err(bad(
                "temperatures",
                format!("{temperatures:?} must be finite and strictly increasing"),
            ));
        }
        if columns.is_empty() {
            return Err(bad("columns", "a table needs at least one property column".to_string()));
        }
        for (name, values) in &columns {
            if name.is_empty() {
                return Err(bad("columns", "a column has an empty property name".to_string()));
            }
            if values.len() != temperatures.len() {
                return Err(FemError::FieldLength {
                    key: "columns",
                    expected: temperatures.len(),
                    found: values.len(),
                });
            }
            if !values.iter().all(|v| v.is_finite()) {
                return Err(bad("columns", format!("column '{name}' has a non-finite value")));
            }
        }
        Ok(Material::Tabulated {
            base: Box::new(base),
            temperatures,
            columns,
        })
    }

    /// A viscoelastic material (ADR 0440 D2): refuses a base that is not isotropic or orthotropic, no term, and a sum of
    /// shear or of bulk weights above 1.
    pub fn viscoelastic(
        base: Material,
        terms: Vec<PronyTerm>,
        shift: Option<TimeTemperatureShift>,
    ) -> Result<Material> {
        elastic_base(&base, false)?;
        if terms.is_empty() {
            return Err(FemError::InvalidValue {
                key: "terms",
                reason: "a Prony series needs at least one term".to_string(),
            });
        }
        for (key, sum) in [
            ("shear", terms.iter().map(|t| t.shear).sum::<f64>()),
            ("bulk", terms.iter().map(|t| t.bulk).sum::<f64>()),
        ] {
            if sum > 1.0 {
                return Err(FemError::InvalidMaterial {
                    key,
                    value: sum,
                    reason: "the Prony weights must sum to at most 1",
                });
            }
        }
        Ok(Material::Viscoelastic {
            base: Box::new(base),
            terms,
            shift,
        })
    }

    /// A creeping material (ADR 0440 D2): refuses a base that is not isotropic or orthotropic, `a` or `n` not positive
    /// and finite, `m` not finite or `m <= -1`.
    pub fn creep(base: Material, a: f64, n: f64, m: f64) -> Result<Material> {
        elastic_base(&base, false)?;
        positive_material("a", a)?;
        positive_material("n", n)?;
        if !(m > -1.0 && m.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key: "m",
                value: m,
                reason: "the time exponent must be finite and above -1",
            });
        }
        Ok(Material::Creep {
            base: Box::new(base),
            a,
            n,
            m,
        })
    }

    /// A J2 plastic material (ADR 0440 D2): refuses a base that is not isotropic, a yield stress not positive and finite,
    /// a hardening modulus negative or not finite.
    pub fn j2(base: Material, yield_stress: f64, isotropic: f64, kinematic: f64) -> Result<Material> {
        elastic_base(&base, true)?;
        positive_material("yield_stress", yield_stress)?;
        for (key, v) in [("isotropic", isotropic), ("kinematic", kinematic)] {
            if !(v >= 0.0 && v.is_finite()) {
                return Err(FemError::InvalidMaterial {
                    key,
                    value: v,
                    reason: "a hardening modulus must be non-negative and finite",
                });
            }
        }
        Ok(Material::J2 {
            base: Box::new(base),
            yield_stress,
            isotropic,
            kinematic,
        })
    }

    /// A hyperelastic material (ADR 0440 D2): the energy (validated by its constructor) and a non-negative density.
    pub fn hyperelastic(energy: HyperelasticEnergy, density: f64) -> Result<Material> {
        density_ok(density)?;
        Ok(Material::Hyperelastic { energy, density })
    }

    /// Name of the variant.
    pub fn name(&self) -> &'static str {
        match self {
            Material::IsotropicElastic { .. } => "IsotropicElastic",
            Material::Orthotropic { .. } => "Orthotropic",
            Material::Tabulated { .. } => "Tabulated",
            Material::Viscoelastic { .. } => "Viscoelastic",
            Material::Creep { .. } => "Creep",
            Material::J2 { .. } => "J2",
            Material::Hyperelastic { .. } => "Hyperelastic",
        }
    }

    /// The work package that adds the variant, while it has not landed (`None` for the isotropic material; ADR 0440 D2).
    pub fn pending(&self) -> Option<&'static str> {
        match self {
            Material::IsotropicElastic { .. } => None,
            Material::Orthotropic { .. } => crate::mat_ortho::PENDING,
            Material::Tabulated { .. } => crate::mat_table::PENDING,
            Material::Viscoelastic { .. } => crate::mat_visco::PENDING,
            Material::Creep { .. } => crate::mat_creep::PENDING,
            Material::J2 { .. } => crate::mat_j2::PENDING,
            Material::Hyperelastic { .. } => crate::mat_hyper::PENDING,
        }
    }

    /// The elastic base of a wrapping variant (`Tabulated`, `Viscoelastic`, `Creep`, `J2`).
    pub fn base(&self) -> Option<&Material> {
        match self {
            Material::Tabulated { base, .. }
            | Material::Viscoelastic { base, .. }
            | Material::Creep { base, .. }
            | Material::J2 { base, .. } => Some(base),
            _ => None,
        }
    }

    /// Young's modulus (the base's for a wrapping variant; NaN for a variant without a single value).
    pub fn youngs_modulus(&self) -> f64 {
        match self {
            Material::IsotropicElastic { e, .. } => *e,
            _ => self.base().map_or(f64::NAN, |b| b.youngs_modulus()),
        }
    }
    /// Poisson ratio (the base's for a wrapping variant; NaN for a variant without a single value).
    pub fn poisson_ratio(&self) -> f64 {
        match self {
            Material::IsotropicElastic { nu, .. } => *nu,
            _ => self.base().map_or(f64::NAN, |b| b.poisson_ratio()),
        }
    }
    /// Density.
    pub fn density(&self) -> f64 {
        match self {
            Material::IsotropicElastic { density, .. } => *density,
            Material::Orthotropic { density, .. } | Material::Hyperelastic { density, .. } => *density,
            _ => self.base().map_or(f64::NAN, |b| b.density()),
        }
    }
    /// Linear expansion coefficient (the base's for a wrapping variant; NaN for a variant without a single value).
    pub fn expansion(&self) -> f64 {
        match self {
            Material::IsotropicElastic { expansion, .. } => *expansion,
            _ => self.base().map_or(f64::NAN, |b| b.expansion()),
        }
    }
}

/// A constraint. Non-exhaustive: the intermediate tier adds multi-point, rigid and cyclic constraints (ADR 0290).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Constraint {
    /// Prescribed value of one component of the displacement at every node of a node set.
    #[non_exhaustive]
    Displacement {
        /// Node set name.
        set: String,
        /// Component (0, 1, 2 for the translations x, y, z).
        component: usize,
        /// Prescribed value, m.
        value: f64,
    },
    /// A linear constraint `sum_i a_i u_i = b` over (node, component) pairs (ADR 0296; refused until it lands).
    #[non_exhaustive]
    Linear {
        /// The terms `a_i u_i`.
        terms: Vec<LinearTerm>,
        /// The right-hand side `b`, m.
        value: f64,
    },
    /// The nodes of a node set follow a reference point rigidly, with small rotations (ADR 0296; refused until it lands).
    #[non_exhaustive]
    RigidSet {
        /// Node set name.
        set: String,
        /// The reference point.
        reference: Vec3,
    },
    /// Cyclic symmetry of one sector: the surface set `high` is `low` rotated by `2 pi / sectors` about the axis (ADR
    /// 0297; refused until it lands).
    #[non_exhaustive]
    Cyclic {
        /// Surface set of the low boundary.
        low: String,
        /// Surface set of the high boundary.
        high: String,
        /// Direction of the axis (non-zero).
        axis: Vec3,
        /// A point on the axis.
        origin: Vec3,
        /// Number of sectors of the full rotor, at least 2.
        sectors: usize,
        /// The extensions of ADR 0456 (non-matching boundaries, nodes on the axis); the default is the matched boundary of
        /// ADR 0386.
        options: CyclicOptions,
    },
}

/// The opt-in extensions of a cyclic constraint (ADR 0456 D4). The default (no interpolation, no axis nodes) is the
/// node-for-node match of ADR 0386.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct CyclicOptions {
    /// Non-matching boundaries: each high node follows the low faces rotated by `2 pi / N` by the tie method and tolerance.
    pub interpolation: Option<crate::tie::TieOptions>,
    /// Nodes on both boundaries are accepted on the axis and constrained by the harmonic rule of ADR 0456 arithmetic 5.
    pub axis_nodes: bool,
}

impl CyclicOptions {
    /// The default options.
    pub fn new() -> CyclicOptions {
        CyclicOptions::default()
    }
    /// With the interpolation of non-matching boundaries.
    pub fn interpolation(mut self, options: crate::tie::TieOptions) -> CyclicOptions {
        self.interpolation = Some(options);
        self
    }
    /// With nodes on the axis.
    pub fn axis_nodes(mut self, on: bool) -> CyclicOptions {
        self.axis_nodes = on;
        self
    }
}

/// One term `a u` of a [`Constraint::Linear`]: the coefficient of one component of one node.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct LinearTerm {
    /// The node.
    pub node: usize,
    /// The component.
    pub component: usize,
    /// The coefficient.
    pub coefficient: f64,
}

impl LinearTerm {
    /// A term.
    pub fn new(node: usize, component: usize, coefficient: f64) -> LinearTerm {
        LinearTerm {
            node,
            component,
            coefficient,
        }
    }
}

impl Constraint {
    /// A prescribed displacement component on a node set.
    pub fn displacement(set: &str, component: usize, value: f64) -> Constraint {
        Constraint::Displacement {
            set: set.to_string(),
            component,
            value,
        }
    }
    /// A linear constraint `sum a_i u_i = value`.
    pub fn linear(terms: Vec<LinearTerm>, value: f64) -> Constraint {
        Constraint::Linear { terms, value }
    }
    /// A rigid set of nodes following a reference point.
    pub fn rigid_set(set: &str, reference: Vec3) -> Constraint {
        Constraint::RigidSet {
            set: set.to_string(),
            reference,
        }
    }
    /// The cyclic condition between two boundary surface sets of a sector.
    pub fn cyclic(low: &str, high: &str, axis: Vec3, origin: Vec3, sectors: usize) -> Constraint {
        Constraint::cyclic_with(low, high, axis, origin, sectors, CyclicOptions::default())
    }
    /// The cyclic condition with the extensions of ADR 0456.
    pub fn cyclic_with(
        low: &str,
        high: &str,
        axis: Vec3,
        origin: Vec3,
        sectors: usize,
        options: CyclicOptions,
    ) -> Constraint {
        Constraint::Cyclic {
            low: low.to_string(),
            high: high.to_string(),
            axis,
            origin,
            sectors,
            options,
        }
    }
    /// Name of the variant.
    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Displacement { .. } => "Displacement",
            Constraint::Linear { .. } => "Linear",
            Constraint::RigidSet { .. } => "RigidSet",
            Constraint::Cyclic { .. } => "Cyclic",
        }
    }
}

/// An elastic support to ground (ADR 0290 section 2.3; ADR 0296, refused until it lands). Non-exhaustive.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ElasticSupport {
    /// A spring to ground on one component at every node of a node set, N/m per node.
    #[non_exhaustive]
    NodalSpring {
        /// Node set name.
        set: String,
        /// Component.
        component: usize,
        /// Stiffness, positive.
        stiffness: f64,
    },
    /// A foundation on a surface set: a normal stiffness per area, Pa/m.
    #[non_exhaustive]
    Foundation {
        /// Surface set name.
        set: String,
        /// Stiffness per area, positive.
        stiffness: f64,
    },
}

impl ElasticSupport {
    /// A spring per node and component on a node set.
    pub fn nodal_spring(set: &str, component: usize, stiffness: f64) -> ElasticSupport {
        ElasticSupport::NodalSpring {
            set: set.to_string(),
            component,
            stiffness,
        }
    }
    /// A foundation with a normal stiffness per area on a surface set.
    pub fn foundation(set: &str, stiffness: f64) -> ElasticSupport {
        ElasticSupport::Foundation {
            set: set.to_string(),
            stiffness,
        }
    }
    /// Name of the variant.
    pub fn name(&self) -> &'static str {
        match self {
            ElasticSupport::NodalSpring { .. } => "NodalSpring",
            ElasticSupport::Foundation { .. } => "Foundation",
        }
    }
}

/// A load. Non-exhaustive; plain data naming sets (ADR 0270 section 2.4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Load {
    /// The same force on each node of a node set, N.
    #[non_exhaustive]
    NodalForce {
        /// Node set name.
        set: String,
        /// Force on each node.
        force: Vec3,
    },
    /// The same moment on each node of a node set, N m; every node must carry rotations (ADR 0417 D2).
    #[non_exhaustive]
    NodalMoment {
        /// Node set name.
        set: String,
        /// Moment on each node, about the global axes.
        moment: Vec3,
    },
    /// A uniform force per unit length, N/m, about the global axes, on every element of an element set; every element of
    /// the set must be a Beam2 element (ADR 0422 D1). Its nodal loads are the consistent loads of
    /// [`crate::beam::line_load_vector`].
    #[non_exhaustive]
    LineLoad {
        /// Element set name.
        set: String,
        /// Force per unit length on each element.
        force: Vec3,
    },
    /// A uniform pressure, Pa, on every element of an element set of Shell4 elements, along each element's normal `e_3`
    /// (positive pushes the surface towards `+e_3`; ADR 0432 D5). Its nodal loads are the consistent loads of
    /// [`crate::shell::pressure_load_vector`].
    #[non_exhaustive]
    ShellPressure {
        /// Element set name.
        set: String,
        /// Pressure on each element.
        pressure: f64,
    },
    /// A uniform traction on a surface set, Pa.
    #[non_exhaustive]
    Traction {
        /// Surface set name.
        set: String,
        /// Traction vector.
        traction: Vec3,
    },
    /// A uniform pressure on a surface set, Pa, positive when it presses on the solid.
    #[non_exhaustive]
    Pressure {
        /// Surface set name.
        set: String,
        /// Pressure.
        pressure: f64,
    },
    /// Gravity on the whole model, m/s^2.
    #[non_exhaustive]
    Gravity {
        /// Acceleration of gravity.
        acceleration: Vec3,
    },
    /// The centrifugal load of the whole model rotating at constant speed about an axis.
    #[non_exhaustive]
    Centrifugal {
        /// Direction of the axis (non-zero; not necessarily of unit length).
        axis: Vec3,
        /// A point on the axis.
        origin: Vec3,
        /// Angular speed, rad/s.
        omega: f64,
    },
    /// A body force per unit volume given at the nodes, N/m^3.
    #[non_exhaustive]
    BodyForceNodal {
        /// One vector per node.
        values: Vec<Vec3>,
    },
    /// A nodal temperature field for the thermal strain, K.
    #[non_exhaustive]
    Temperature {
        /// One temperature per node.
        nodal: Vec<f64>,
        /// Reference (strain-free) temperature.
        reference: f64,
    },
}

impl Load {
    /// The same force on each node of a node set.
    pub fn nodal_force(set: &str, force: Vec3) -> Load {
        Load::NodalForce {
            set: set.to_string(),
            force,
        }
    }
    /// The same moment on each node of a node set (nodes with rotations only).
    pub fn nodal_moment(set: &str, moment: Vec3) -> Load {
        Load::NodalMoment {
            set: set.to_string(),
            moment,
        }
    }
    /// A uniform force per unit length on every element of an element set of Beam2 elements (ADR 0422).
    pub fn line_load(set: &str, force: Vec3) -> Load {
        Load::LineLoad {
            set: set.to_string(),
            force,
        }
    }
    /// A uniform pressure along the element normals on every element of an element set of Shell4 elements (ADR 0432).
    pub fn shell_pressure(set: &str, pressure: f64) -> Load {
        Load::ShellPressure {
            set: set.to_string(),
            pressure,
        }
    }
    /// A uniform traction on a surface set.
    pub fn traction(set: &str, traction: Vec3) -> Load {
        Load::Traction {
            set: set.to_string(),
            traction,
        }
    }
    /// A uniform pressure on a surface set (positive pressing on the solid).
    pub fn pressure(set: &str, pressure: f64) -> Load {
        Load::Pressure {
            set: set.to_string(),
            pressure,
        }
    }
    /// Gravity on the whole model.
    pub fn gravity(acceleration: Vec3) -> Load {
        Load::Gravity { acceleration }
    }
    /// The centrifugal load of a rotation about `axis` through `origin` at `omega`.
    pub fn centrifugal(axis: Vec3, origin: Vec3, omega: f64) -> Load {
        Load::Centrifugal { axis, origin, omega }
    }
    /// A body force per unit volume at the nodes.
    pub fn body_force_nodal(values: Vec<Vec3>) -> Load {
        Load::BodyForceNodal { values }
    }
    /// A nodal temperature field with its reference temperature.
    pub fn temperature(nodal: Vec<f64>, reference: f64) -> Load {
        Load::Temperature { nodal, reference }
    }
}

fn finite_vec(key: &'static str, v: Vec3) -> Result<()> {
    if v.iter().all(|c| c.is_finite()) {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v:?} is not finite"),
        })
    }
}

fn finite(key: &'static str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v} is not finite"),
        })
    }
}

fn positive_finite(key: &'static str, v: f64) -> Result<()> {
    if v > 0.0 && v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v} must be positive and finite"),
        })
    }
}

/// A solid model: mesh, materials on element sets, constraints, loads and elastic supports.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidModel {
    mesh: SolidMesh,
    materials: Vec<(String, Material)>,
    constraints: Vec<Constraint>,
    loads: Vec<Load>,
    supports: Vec<ElasticSupport>,
}

impl SolidModel {
    /// A model on a mesh, with no material, constraint or load yet.
    pub fn new(mesh: SolidMesh) -> SolidModel {
        SolidModel {
            mesh,
            materials: Vec::new(),
            constraints: Vec::new(),
            loads: Vec::new(),
            supports: Vec::new(),
        }
    }

    /// The mesh.
    pub fn mesh(&self) -> &SolidMesh {
        &self.mesh
    }
    /// Materials with the element sets they are assigned to, in the order added.
    pub fn materials(&self) -> &[(String, Material)] {
        &self.materials
    }
    /// The constraints, in the order added.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    /// The loads, in the order added.
    pub fn loads(&self) -> &[Load] {
        &self.loads
    }
    /// The elastic supports, in the order added.
    pub fn supports(&self) -> &[ElasticSupport] {
        &self.supports
    }
    /// The numbering of the unknowns of this model.
    pub fn dof_map(&self) -> DofMap {
        DofMap::for_mesh(&self.mesh)
    }

    fn need_set(&self, kind: SetKind, name: &str) -> Result<()> {
        let exists = match kind {
            SetKind::Node => self.mesh.node_set(name).is_some(),
            SetKind::Element => self.mesh.element_set(name).is_some(),
            SetKind::Surface => self.mesh.surface_set(name).is_some(),
        };
        if exists {
            Ok(())
        } else {
            Err(FemError::UnknownSet {
                kind,
                name: name.to_string(),
            })
        }
    }

    /// Assigns a material to an element set.
    pub fn add_material(&mut self, element_set: &str, material: Material) -> Result<()> {
        self.need_set(SetKind::Element, element_set)?;
        self.materials.push((element_set.to_string(), material));
        Ok(())
    }

    /// Adds a constraint; its set must exist and every node of it must have the component.
    pub fn add_constraint(&mut self, c: Constraint) -> Result<()> {
        match &c {
            Constraint::Displacement { set, component, value } => {
                self.need_set(SetKind::Node, set)?;
                finite("value", *value)?;
                let dofs = self.dof_map();
                for &node in self.mesh.node_set(set).expect("checked") {
                    if dofs.equation(node, *component).is_none() {
                        return Err(FemError::ComponentOutOfRange {
                            set: set.clone(),
                            node,
                            component: *component,
                            available: dofs.components(node),
                        });
                    }
                }
            }
            Constraint::Linear { terms, value } => {
                if terms.is_empty() {
                    return Err(FemError::InvalidValue {
                        key: "terms",
                        reason: "a linear constraint needs at least one term".into(),
                    });
                }
                let dofs = self.dof_map();
                for t in terms {
                    if t.node >= dofs.n_nodes() {
                        return Err(FemError::MissingNode {
                            role: "linear constraint",
                            node: t.node,
                            n_nodes: dofs.n_nodes(),
                        });
                    }
                    if dofs.equation(t.node, t.component).is_none() {
                        return Err(FemError::MissingComponent {
                            role: "linear constraint",
                            node: t.node,
                            component: t.component,
                            available: dofs.components(t.node),
                        });
                    }
                    finite("coefficient", t.coefficient)?;
                }
                if terms.iter().all(|t| t.coefficient == 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "coefficient",
                        reason: "every coefficient of the linear constraint is zero".into(),
                    });
                }
                finite("value", *value)?;
            }
            Constraint::RigidSet { set, reference } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("reference", *reference)?;
            }
            Constraint::Cyclic {
                low,
                high,
                axis,
                origin,
                sectors,
                options,
            } => {
                if let Some(t) = options.interpolation.and_then(|i| i.tolerance) {
                    positive_finite("tolerance", t)?;
                }
                self.need_set(SetKind::Surface, low)?;
                self.need_set(SetKind::Surface, high)?;
                finite_vec("axis", *axis)?;
                if axis.iter().all(|&c| c == 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "axis",
                        reason: "the axis is the zero vector".into(),
                    });
                }
                finite_vec("origin", *origin)?;
                if *sectors < 2 {
                    return Err(FemError::InvalidValue {
                        key: "sectors",
                        reason: format!("{sectors} sectors: a cyclic rotor has at least 2"),
                    });
                }
            }
        }
        self.constraints.push(c);
        Ok(())
    }

    /// Adds an elastic support; its set must exist with the right kind, its component exist at every node and its
    /// stiffness be positive and finite.
    pub fn add_support(&mut self, s: ElasticSupport) -> Result<()> {
        match &s {
            ElasticSupport::NodalSpring {
                set,
                component,
                stiffness,
            } => {
                self.need_set(SetKind::Node, set)?;
                let dofs = self.dof_map();
                for &node in self.mesh.node_set(set).expect("checked") {
                    if dofs.equation(node, *component).is_none() {
                        return Err(FemError::ComponentOutOfRange {
                            set: set.clone(),
                            node,
                            component: *component,
                            available: dofs.components(node),
                        });
                    }
                }
                positive_finite("stiffness", *stiffness)?;
            }
            ElasticSupport::Foundation { set, stiffness } => {
                self.need_set(SetKind::Surface, set)?;
                positive_finite("stiffness", *stiffness)?;
            }
        }
        self.supports.push(s);
        Ok(())
    }

    /// Refuses an element kind or a solid formulation whose work package has not landed, naming it and its block (ADR
    /// 0291 D6), a Beam2 block, which only the paths of ADR 0417 D4 take, naming [`crate::beam::FOLLOW_UP`], and a Shell4
    /// block, which only the paths of ADR 0432 D3 take, naming [`crate::shell::FOLLOW_UP`]. A model of the basic kinds with
    /// the default formulation passes.
    pub fn check_elements_supported(&self) -> Result<()> {
        self.check_elements(false, false, false)
    }

    /// As [`SolidModel::check_elements_supported`] for a path that takes beams (ADR 0417 D4): a Beam2 block passes, and the
    /// model's beams are checked by [`crate::beam::check_beam_model`] (a Beam property on every Beam2 block, no node shared
    /// with a solid element, a frame for every beam element). A Shell4 block is refused.
    pub fn check_elements_supported_with_beams(&self) -> Result<()> {
        self.check_elements(true, false, false)?;
        crate::beam::check_beam_model(self)
    }

    /// As [`SolidModel::check_elements_supported_with_beams`] for a path that also takes shells (ADR 0432 D3): a Shell4
    /// block passes, and the model's shells are checked by [`crate::shell::check_shell_model`].
    pub fn check_elements_supported_with_structures(&self) -> Result<()> {
        self.check_elements(true, true, false)?;
        crate::beam::check_beam_model(self)?;
        crate::shell::check_shell_model(self)
    }

    /// As [`SolidModel::check_elements_supported_with_structures`] for the static solve, which also takes the orthotropic
    /// and the tabulated material on solid elements (ADR 0452 D6), then [`crate::mat_ortho::check_model`].
    pub fn check_elements_supported_with_materials(&self) -> Result<()> {
        self.check_elements(true, true, true)?;
        crate::beam::check_beam_model(self)?;
        crate::shell::check_shell_model(self)?;
        crate::mat_ortho::check_model(self)
    }

    fn check_elements(&self, beams: bool, shells: bool, point_materials: bool) -> Result<()> {
        for (b, block) in self.mesh.blocks().iter().enumerate() {
            if let Some(package) = block.kind().pending() {
                return Err(FemError::NotYetSupported {
                    item: "element kind",
                    variant: block.kind().name(),
                    location: format!("block {b}"),
                    package,
                });
            }
            let formulation = block.formulation();
            if let Some(package) = formulation.pending() {
                return Err(FemError::NotYetSupported {
                    item: "formulation",
                    variant: formulation.name(),
                    location: format!("block {b}"),
                    package,
                });
            }
            // ADR 0440 D5: a layup is refused on every path until its package lands.
            if let Some(crate::mesh::BlockProperty::Layup { .. }) = block.property() {
                if let Some(package) = crate::laminate::PENDING {
                    return Err(FemError::NotYetSupported {
                        item: "shell property",
                        variant: "Layup",
                        location: format!("block {b}"),
                        package,
                    });
                }
            }
            // ADR 0417 D4: a path that does not take beams refuses them.
            if !beams && block.kind() == crate::mesh::ElementKind::Beam2 {
                return Err(FemError::NotYetSupported {
                    item: "element kind",
                    variant: block.kind().name(),
                    location: format!("block {b}"),
                    package: crate::beam::FOLLOW_UP,
                });
            }
            // ADR 0432 D3: a path that does not take shells refuses them.
            if !shells && block.kind() == crate::mesh::ElementKind::Shell4 {
                return Err(FemError::NotYetSupported {
                    item: "element kind",
                    variant: block.kind().name(),
                    location: format!("block {b}"),
                    package: crate::shell::FOLLOW_UP,
                });
            }
        }
        // ADR 0440 D5: a material variant whose package has not landed is refused naming it and its set.
        for (i, (set, m)) in self.materials.iter().enumerate() {
            if let Some(package) = m.pending() {
                return Err(FemError::NotYetSupported {
                    item: "material",
                    variant: m.name(),
                    location: format!("material {i}, element set '{set}'"),
                    package,
                });
            }
            // ADR 0452 D6: a path that does not take the per-point materials refuses them.
            if !point_materials && crate::mat_ortho::is_point_material(m) {
                return Err(FemError::NotYetSupported {
                    item: "material",
                    variant: m.name(),
                    location: format!("material {i}, element set '{set}'"),
                    package: crate::mat_ortho::FOLLOW_UP,
                });
            }
        }
        Ok(())
    }

    /// [`SolidModel::check_elements_supported`], then refuses an elastic support whose work package has not landed,
    /// naming it and its set (ADR 0291 D6). Constraints are refused where they are read
    /// ([`crate::constraint::prescribed_equations`]).
    pub fn check_supported(&self) -> Result<()> {
        self.check_elements_supported()?;
        self.check_supports()
    }

    /// As [`SolidModel::check_supported`] for a path that takes beams (ADR 0417 D4).
    pub fn check_supported_with_beams(&self) -> Result<()> {
        self.check_elements_supported_with_beams()?;
        self.check_supports()
    }

    /// As [`SolidModel::check_supported`] for a path that takes beams and shells (ADR 0432 D3).
    pub fn check_supported_with_structures(&self) -> Result<()> {
        self.check_elements_supported_with_structures()?;
        self.check_supports()
    }

    /// As [`SolidModel::check_supported_with_structures`] for the static solve, which also takes the orthotropic and the
    /// tabulated material on solid elements (ADR 0452 D6).
    pub fn check_supported_with_materials(&self) -> Result<()> {
        self.check_elements_supported_with_materials()?;
        self.check_supports()
    }

    fn check_supports(&self) -> Result<()> {
        for (i, s) in self.supports.iter().enumerate() {
            if let Some(package) = crate::mpc::PENDING {
                let location = match s {
                    ElasticSupport::NodalSpring { set, .. } => format!("support {i}, node set '{set}'"),
                    ElasticSupport::Foundation { set, .. } => format!("support {i}, surface set '{set}'"),
                };
                return Err(FemError::NotYetSupported {
                    item: "support",
                    variant: s.name(),
                    location,
                    package,
                });
            }
        }
        Ok(())
    }

    /// Adds a load; the sets it names must exist with the right kind, its values be finite and its fields of the node
    /// count.
    pub fn add_load(&mut self, l: Load) -> Result<()> {
        let n = self.mesh.nodes().len();
        match &l {
            Load::NodalForce { set, force } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("force", *force)?;
            }
            Load::NodalMoment { set, moment } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("moment", *moment)?;
                let dofs = self.dof_map();
                for &node in self.mesh.node_set(set).expect("checked") {
                    if dofs.components(node) < 6 {
                        return Err(FemError::ComponentOutOfRange {
                            set: set.clone(),
                            node,
                            component: 3,
                            available: dofs.components(node),
                        });
                    }
                }
            }
            Load::LineLoad { set, force } => {
                self.need_set(SetKind::Element, set)?;
                finite_vec("force per length", *force)?;
                for &e in self.mesh.element_set(set).expect("checked") {
                    let kind = self.mesh.element_kind(e);
                    if kind != crate::mesh::ElementKind::Beam2 {
                        return Err(FemError::InvalidValue {
                            key: "line load",
                            reason: format!(
                                "element {e} of the set '{set}' is a {} element: a line load acts on Beam2 elements",
                                kind.name()
                            ),
                        });
                    }
                }
            }
            Load::ShellPressure { set, pressure } => {
                self.need_set(SetKind::Element, set)?;
                finite("pressure", *pressure)?;
                for &e in self.mesh.element_set(set).expect("checked") {
                    let kind = self.mesh.element_kind(e);
                    if kind != crate::mesh::ElementKind::Shell4 {
                        return Err(FemError::InvalidValue {
                            key: "shell pressure",
                            reason: format!(
                                "element {e} of the set '{set}' is a {} element: a shell pressure acts on Shell4 elements",
                                kind.name()
                            ),
                        });
                    }
                }
            }
            Load::Traction { set, traction } => {
                self.need_set(SetKind::Surface, set)?;
                finite_vec("traction", *traction)?;
            }
            Load::Pressure { set, pressure } => {
                self.need_set(SetKind::Surface, set)?;
                finite("pressure", *pressure)?;
            }
            Load::Gravity { acceleration } => finite_vec("acceleration", *acceleration)?,
            Load::Centrifugal { axis, origin, omega } => {
                finite_vec("axis", *axis)?;
                if axis.iter().all(|&c| c == 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "axis",
                        reason: "the axis is the zero vector".into(),
                    });
                }
                finite_vec("origin", *origin)?;
                finite("omega", *omega)?;
            }
            Load::BodyForceNodal { values } => {
                if values.len() != n {
                    return Err(FemError::FieldLength {
                        key: "values",
                        expected: n,
                        found: values.len(),
                    });
                }
                for v in values {
                    finite_vec("values", *v)?;
                }
            }
            Load::Temperature { nodal, reference } => {
                if nodal.len() != n {
                    return Err(FemError::FieldLength {
                        key: "nodal",
                        expected: n,
                        found: nodal.len(),
                    });
                }
                for &t in nodal {
                    finite("nodal", t)?;
                }
                finite("reference", *reference)?;
            }
        }
        self.loads.push(l);
        Ok(())
    }

    /// Index into [`SolidModel::materials`] of each element; refuses an element without material or with two.
    pub fn material_indices(&self) -> Result<Vec<usize>> {
        let mut of: Vec<Option<usize>> = vec![None; self.mesh.n_elements()];
        for (k, (set, _)) in self.materials.iter().enumerate() {
            for &e in self.mesh.element_set(set).expect("checked when added") {
                if let Some(first) = of[e] {
                    return Err(FemError::MaterialOverlap {
                        element: e,
                        first: self.materials[first].0.clone(),
                        second: set.clone(),
                    });
                }
                of[e] = Some(k);
            }
        }
        of.into_iter()
            .enumerate()
            .map(|(element, m)| m.ok_or(FemError::MaterialUnassigned { element }))
            .collect()
    }

    /// Checks the model as a whole: every element has exactly one material.
    pub fn validate(&self) -> Result<()> {
        self.material_indices().map(|_| ())
    }

    /// The material of an element, if it has exactly one.
    pub fn material_of(&self, element: usize) -> Option<&Material> {
        let mut found = None;
        for (set, m) in &self.materials {
            if self.mesh.element_set(set).is_some_and(|s| s.contains(&element)) {
                if found.is_some() {
                    return None;
                }
                found = Some(m);
            }
        }
        found
    }
}

/// Statistics of a solve.
#[derive(Debug, Clone, PartialEq)]
pub struct SolverStats {
    solver: String,
    unknowns: usize,
    iterations: usize,
    relative_residual: f64,
}

impl SolverStats {
    /// Statistics of a solve by the named solver.
    pub fn new(solver: &str, unknowns: usize, iterations: usize, relative_residual: f64) -> SolverStats {
        SolverStats {
            solver: solver.to_string(),
            unknowns,
            iterations,
            relative_residual,
        }
    }
    /// Solver name.
    pub fn solver(&self) -> &str {
        &self.solver
    }
    /// Number of unknowns solved for.
    pub fn unknowns(&self) -> usize {
        self.unknowns
    }
    /// Iterations (0 for a direct solve).
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    /// Final relative residual.
    pub fn relative_residual(&self) -> f64 {
        self.relative_residual
    }
}

/// The result of a solve: displacement and reactions per unknown, strain and stress at the integration points, solver
/// statistics. Derived fields come from `recover` (ADR 0277).
#[derive(Debug, Clone, PartialEq)]
pub struct SolidResult {
    dofs: DofMap,
    displacement: Vec<f64>,
    reactions: Option<Vec<f64>>,
    ip_offsets: Vec<usize>,
    strain: Vec<[f64; 6]>,
    stress: Vec<[f64; 6]>,
    stats: Option<SolverStats>,
}

impl SolidResult {
    /// A result with the displacement of every unknown of `dofs`.
    pub fn new(dofs: DofMap, displacement: Vec<f64>) -> Result<SolidResult> {
        if displacement.len() != dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: dofs.n_equations(),
                found: displacement.len(),
            });
        }
        Ok(SolidResult {
            dofs,
            displacement,
            reactions: None,
            ip_offsets: vec![0],
            strain: Vec::new(),
            stress: Vec::new(),
            stats: None,
        })
    }

    /// Adds the reactions, one per unknown.
    pub fn with_reactions(mut self, reactions: Vec<f64>) -> Result<SolidResult> {
        if reactions.len() != self.dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "reactions",
                expected: self.dofs.n_equations(),
                found: reactions.len(),
            });
        }
        self.reactions = Some(reactions);
        Ok(self)
    }

    /// Adds strain and stress at the integration points: `offsets[e]..offsets[e + 1]` are the points of element `e`
    /// (offsets start at 0, never decrease and end at the point count); six components xx, yy, zz, xy, yz, zx, strains
    /// with engineering shear.
    pub fn with_integration_points(
        mut self,
        offsets: Vec<usize>,
        strain: Vec<[f64; 6]>,
        stress: Vec<[f64; 6]>,
    ) -> Result<SolidResult> {
        let ok = offsets.first() == Some(&0) && offsets.windows(2).all(|w| w[0] <= w[1]);
        if !ok {
            return Err(FemError::InvalidValue {
                key: "offsets",
                reason: "must start at 0 and never decrease".into(),
            });
        }
        let np = *offsets.last().expect("non-empty");
        if strain.len() != np {
            return Err(FemError::FieldLength {
                key: "strain",
                expected: np,
                found: strain.len(),
            });
        }
        if stress.len() != np {
            return Err(FemError::FieldLength {
                key: "stress",
                expected: np,
                found: stress.len(),
            });
        }
        self.ip_offsets = offsets;
        self.strain = strain;
        self.stress = stress;
        Ok(self)
    }

    /// Adds the solver statistics.
    pub fn with_stats(mut self, stats: SolverStats) -> SolidResult {
        self.stats = Some(stats);
        self
    }

    /// The numbering of the unknowns.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }
    /// Displacement of every unknown.
    pub fn displacement_values(&self) -> &[f64] {
        &self.displacement
    }
    /// The values of a node's unknowns (empty for a node that does not exist).
    pub fn displacement(&self, node: usize) -> &[f64] {
        match self.dofs.equation(node, 0) {
            Some(start) => &self.displacement[start..start + self.dofs.components(node)],
            None => &[],
        }
    }
    /// Reactions per unknown, if recovered.
    pub fn reactions(&self) -> Option<&[f64]> {
        self.reactions.as_deref()
    }
    /// Integration-point offsets per element (`[0]` when none were stored).
    pub fn integration_point_offsets(&self) -> &[usize] {
        &self.ip_offsets
    }
    /// Strain at the integration points.
    pub fn strain(&self) -> &[[f64; 6]] {
        &self.strain
    }
    /// Stress at the integration points.
    pub fn stress(&self) -> &[[f64; 6]] {
        &self.stress
    }
    /// Solver statistics, if given.
    pub fn stats(&self) -> Option<&SolverStats> {
        self.stats.as_ref()
    }
}
