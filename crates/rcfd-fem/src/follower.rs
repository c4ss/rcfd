//! Follower pressure and its load stiffness, the deformation-dependent centrifugal load, buckling with the load stiffness,
//! and the modes about a nonlinear equilibrium. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0480
//! (the registration of ADR 0327, F-424).
//!
//! * [`FollowerPressure`] (ADR 0480 D1): a uniform pressure on the current (deformed) faces of a surface set of solid
//!   elements, `f_a = -p int N_a (x_u x x_v) du dv` with the rule and the functions of the dead `Load::Pressure`
//!   ([`crate::loads::face_rule`], [`crate::loads::face_shape_functions`]), so that at zero displacement it is the dead load
//!   bit for bit; its load stiffness `K_p = d f / d u` from `d(x_u x x_v)/d x_bj = N_b,u (e_j x x_v) + N_b,v (x_u x e_j)`
//!   ([`FollowerPressure::load_stiffness`], the full unsymmetric matrix). As a [`Contribution`] of the Newton driver it adds
//!   `-f` and the symmetric part `-(K_p + K_p^T)/2`: exact when the antisymmetric part, a term on the edges of the loaded
//!   surface, couples only held unknowns (a closed surface, or edges on symmetry planes with the normal component and the
//!   out-of-plane component held); elsewhere the iteration converges at a reduced rate to the same solution.
//! * [`FollowerPressure::on_shells`] (ADR 0487, F-573): the follower form of the shell pressure on the current mid-surface of
//!   Shell4 and Shell3 elements, `f_a = +p int N_a (x_u x x_v) du dv` on the translations, with the same load stiffness; the
//!   Newton driver does not take shells yet (ADR 0487 D0), the linear buckling path does.
//! * [`CentrifugalFollower`] (D2): the part `Omega^2 M_perp u` of the centrifugal load on the deformed position, with its
//!   load stiffness `-Omega^2 M_perp` (the spin softening of [`crate::rotating`]) for the model's `Load::Centrifugal`.
//! * [`buckling_factors`] (D3): `(K + mu (K_sigma - K_p,sym)) x = 0` by the path of [`crate::buckling::factors`].
//! * [`tangent_modes`] (D7): the modes of the tangent of a converged nonlinear state (with the contributions' tangents), `K_T
//!   phi = omega^2 M phi`, by the eigensolver of [`crate::geometric::prestressed_modes`].

use crate::buckling::{BucklingOptions, BucklingResult};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce, Prescribed};
use crate::element::ElementState;
use crate::error::{FemError, Result, SetKind};
use crate::geometric::Prestress;
use crate::loads::{face_rule, face_shape_functions};
use crate::mass::consistent_with;
use crate::mesh::ElementKind;
use crate::modal::{ModalOptions, ModalResult};
use crate::model::{Load, SolidModel};
use crate::nonlinear::{add_triplets, Contribution};
use rcfd_core::vec3::{cross, Vec3};
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// One loaded face: its nodes' equations (three per node, node-major) and reference coordinates.
#[derive(Debug, Clone)]
struct Face {
    eqs: Vec<usize>,
    x0: Vec<Vec3>,
}

/// A uniform pressure on the deformed faces of a surface set (ADR 0480 D1), positive when it presses on the solid, scaled
/// by the load factor.
#[derive(Debug, Clone)]
pub struct FollowerPressure {
    name: String,
    pressure: f64,
    faces: Vec<Face>,
    n: usize,
    /// The faces are Shell4 or Shell3 elements (ADR 0487 D1): the face routines receive the pressure negated.
    shell: bool,
}

impl FollowerPressure {
    /// The pressure `pressure` on the faces of the surface set `set` of `model`. The faces must belong to solid elements
    /// (Hex8, Hex20, Tet4, Tet10); a non-finite pressure is refused (key `"pressure"`).
    pub fn new(model: &SolidModel, set: &str, pressure: f64) -> Result<FollowerPressure> {
        if !pressure.is_finite() {
            return Err(FemError::InvalidValue {
                key: "pressure",
                reason: format!("{pressure:e} is not finite"),
            });
        }
        let mesh = model.mesh();
        let dofs = model.dof_map();
        let refs = mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Surface,
            name: set.to_string(),
        })?;
        let mut faces = Vec::with_capacity(refs.len());
        for &face in refs {
            let kind = mesh.element_kind(face.element);
            if !kind.is_solid() {
                return Err(FemError::InvalidValue {
                    key: "follower pressure",
                    reason: format!(
                        "surface set '{set}': element {} is a {} element; a follower pressure takes faces of solid elements \
                         (ADR 0480 D1)",
                        face.element,
                        kind.name()
                    ),
                });
            }
            let nodes = mesh.face_nodes(face);
            let mut eqs = Vec::with_capacity(3 * nodes.len());
            for &node in &nodes {
                for c in 0..3 {
                    eqs.push(dofs.equation(node, c).ok_or(FemError::MissingComponent {
                        role: "follower pressure",
                        node,
                        component: c,
                        available: dofs.components(node),
                    })?);
                }
            }
            faces.push(Face {
                eqs,
                x0: nodes.iter().map(|&n| mesh.nodes()[n]).collect(),
            });
        }
        Ok(FollowerPressure {
            name: format!("follower pressure on '{set}'"),
            pressure,
            faces,
            n: dofs.n_equations(),
            shell: false,
        })
    }

    /// The pressure `pressure` on the current mid-surface of the elements of the element set `set` of `model`, each a Shell4
    /// or a Shell3 (ADR 0487 D1): the follower form of [`Load::ShellPressure`], positive pushing the surface towards `+e_3`
    /// (the right-hand normal of the node order, as the dead load). On the interpolated current mid-surface `x = sum N_a (X_a
    /// + u_a)` the forces are `f_a = +p int N_a (x_u x x_v) du dv` on the translations, no moments; the load stiffness the
    /// same derivative as for solid faces. Another element kind is refused (key `"follower pressure"`), a non-finite pressure
    /// (key `"pressure"`).
    ///
    /// The Newton driver of [`crate::nonlinear`] does not take shell models (it has no geometrically nonlinear shell;
    /// ADR 0487 D0), so on shells the [`Contribution`] waits for a driver that takes them; [`FollowerPressure::force`],
    /// [`FollowerPressure::load_stiffness`] and [`buckling_factors`] (the linear buckling path takes shells) serve now.
    pub fn on_shells(model: &SolidModel, set: &str, pressure: f64) -> Result<FollowerPressure> {
        if !pressure.is_finite() {
            return Err(FemError::InvalidValue {
                key: "pressure",
                reason: format!("{pressure:e} is not finite"),
            });
        }
        let mesh = model.mesh();
        let dofs = model.dof_map();
        let elements = mesh.element_set(set).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Element,
            name: set.to_string(),
        })?;
        let mut faces = Vec::with_capacity(elements.len());
        for &e in elements {
            let kind = mesh.element_kind(e);
            if !matches!(kind, ElementKind::Shell4 | ElementKind::Shell3) {
                return Err(FemError::InvalidValue {
                    key: "follower pressure",
                    reason: format!(
                        "element {e} of the set '{set}' is a {} element; a follower shell pressure takes Shell4 and Shell3                          elements (ADR 0487 D1)",
                        kind.name()
                    ),
                });
            }
            let nodes = mesh.element_nodes(e);
            let mut eqs = Vec::with_capacity(3 * nodes.len());
            for &node in nodes {
                for c in 0..3 {
                    eqs.push(dofs.equation(node, c).ok_or(FemError::MissingComponent {
                        role: "follower pressure",
                        node,
                        component: c,
                        available: dofs.components(node),
                    })?);
                }
            }
            faces.push(Face {
                eqs,
                x0: nodes.iter().map(|&n| mesh.nodes()[n]).collect(),
            });
        }
        Ok(FollowerPressure {
            name: format!("follower pressure on the shells of '{set}'"),
            pressure,
            faces,
            n: dofs.n_equations(),
            shell: true,
        })
    }

    /// The pressure the face routines receive at the load factor: the solid faces' `-p` convention gives `+p` on shells
    /// when it is negated (exact in floating point).
    fn face_pressure(&self, load_factor: f64) -> f64 {
        if self.shell {
            -(load_factor * self.pressure)
        } else {
            load_factor * self.pressure
        }
    }

    /// The pressure at load factor 1.
    pub fn pressure(&self) -> f64 {
        self.pressure
    }

    fn check(&self, u: &[f64]) -> Result<()> {
        if u.len() != self.n {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: self.n,
                found: u.len(),
            });
        }
        Ok(())
    }

    /// The face's current nodal positions.
    fn positions(face: &Face, u: &[f64]) -> Vec<Vec3> {
        face.x0
            .iter()
            .enumerate()
            .map(|(a, x)| {
                [
                    x[0] + u[face.eqs[3 * a]],
                    x[1] + u[face.eqs[3 * a + 1]],
                    x[2] + u[face.eqs[3 * a + 2]],
                ]
            })
            .collect()
    }

    /// The load of one face at the positions `x` (the operations of the dead `Load::Pressure`, `loads.rs`).
    fn face_force(x: &[Vec3], p: f64) -> Result<Vec<Vec3>> {
        let mut fe = vec![[0.0; 3]; x.len()];
        for (uv, w) in face_rule(x.len())? {
            let (n, d) = face_shape_functions(x.len(), uv)?;
            let mut xu = [0.0; 3];
            let mut xv = [0.0; 3];
            for (xa, da) in x.iter().zip(&d) {
                for i in 0..3 {
                    xu[i] += xa[i] * da[0];
                    xv[i] += xa[i] * da[1];
                }
            }
            let c = cross(xu, xv);
            let t = [-p * c[0], -p * c[1], -p * c[2]];
            for (fa, na) in fe.iter_mut().zip(&n) {
                for i in 0..3 {
                    fa[i] += w * na * t[i];
                }
            }
        }
        Ok(fe)
    }

    /// The load stiffness `d f_ai / d x_bj` of one face (row-major, `3 n x 3 n`).
    fn face_stiffness(x: &[Vec3], p: f64) -> Result<Vec<f64>> {
        let nn = x.len();
        let size = 3 * nn;
        let mut k = vec![0.0; size * size];
        for (uv, w) in face_rule(nn)? {
            let (n, d) = face_shape_functions(nn, uv)?;
            let mut xu = [0.0; 3];
            let mut xv = [0.0; 3];
            for (xa, da) in x.iter().zip(&d) {
                for i in 0..3 {
                    xu[i] += xa[i] * da[0];
                    xv[i] += xa[i] * da[1];
                }
            }
            for (b, db) in d.iter().enumerate() {
                for j in 0..3 {
                    let mut e = [0.0; 3];
                    e[j] = 1.0;
                    let t1 = cross(e, xv);
                    let t2 = cross(xu, e);
                    let dc = [
                        db[0] * t1[0] + db[1] * t2[0],
                        db[0] * t1[1] + db[1] * t2[1],
                        db[0] * t1[2] + db[1] * t2[2],
                    ];
                    let col = 3 * b + j;
                    for (a, na) in n.iter().enumerate() {
                        for i in 0..3 {
                            k[(3 * a + i) * size + col] += -p * w * na * dc[i];
                        }
                    }
                }
            }
        }
        Ok(k)
    }

    /// The load vector `f(u)` at the load factor (one value per equation of the model's `DofMap`).
    pub fn force(&self, u: &[f64], load_factor: f64) -> Result<Vec<f64>> {
        self.check(u)?;
        let p = self.face_pressure(load_factor);
        let mut f = vec![0.0; self.n];
        for face in &self.faces {
            let x = Self::positions(face, u);
            let fe = Self::face_force(&x, p)?;
            for (a, fa) in fe.iter().enumerate() {
                for i in 0..3 {
                    f[face.eqs[3 * a + i]] += fa[i];
                }
            }
        }
        Ok(f)
    }

    /// The load stiffness `d f / d u` at `u` and the load factor, the full (unsymmetric) matrix as `(row, column, value)`
    /// triplets in equations, face by face.
    pub fn load_stiffness(&self, u: &[f64], load_factor: f64) -> Result<Vec<(usize, usize, f64)>> {
        self.check(u)?;
        let p = self.face_pressure(load_factor);
        let mut out = Vec::new();
        for face in &self.faces {
            let x = Self::positions(face, u);
            let k = Self::face_stiffness(&x, p)?;
            let size = face.eqs.len();
            for r in 0..size {
                for c in 0..size {
                    out.push((face.eqs[r], face.eqs[c], k[r * size + c]));
                }
            }
        }
        Ok(out)
    }

    /// The triplets of `-(K_p + K_p^T)/2` face by face (the tangent of the contribution).
    fn symmetric_negative(&self, u: &[f64], load_factor: f64, out: &mut Vec<(usize, usize, f64)>) -> Result<()> {
        let p = self.face_pressure(load_factor);
        for face in &self.faces {
            let x = Self::positions(face, u);
            let k = Self::face_stiffness(&x, p)?;
            let size = face.eqs.len();
            for r in 0..size {
                for c in 0..size {
                    out.push((face.eqs[r], face.eqs[c], -0.5 * (k[r * size + c] + k[c * size + r])));
                }
            }
        }
        Ok(())
    }
}

impl Contribution for FollowerPressure {
    fn name(&self) -> &str {
        &self.name
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> Result<()> {
        let f = self.force(u, load_factor)?;
        for (a, b) in force.iter_mut().zip(&f) {
            *a -= b;
        }
        self.symmetric_negative(u, load_factor, tangent)
    }
}

/// The part of the centrifugal load that depends on the displacement (ADR 0480 D2): `lambda Omega^2 M_perp u` for the
/// model's `Load::Centrifugal`, with the load stiffness `lambda Omega^2 M_perp`. Given to the Newton driver with that load,
/// the two make the centrifugal force on the deformed position.
#[derive(Debug, Clone)]
pub struct CentrifugalFollower {
    m_perp: CsrMatrix,
    omega2: f64,
}

impl CentrifugalFollower {
    /// For the single `Load::Centrifugal` of `model` (none or more than one refused, key `"centrifugal"`); `threads` for the
    /// assembly of `M_perp` ([`crate::rotating::spin_softening_mass`]).
    pub fn new(model: &SolidModel, threads: Option<usize>) -> Result<CentrifugalFollower> {
        let loads: Vec<(Vec3, f64)> = model
            .loads()
            .iter()
            .filter_map(|l| match l {
                Load::Centrifugal { axis, omega, .. } => Some((*axis, *omega)),
                _ => None,
            })
            .collect();
        if loads.len() != 1 {
            return Err(FemError::InvalidValue {
                key: "centrifugal",
                reason: format!(
                    "{} centrifugal loads in the model: the deformation-dependent part needs exactly one (ADR 0480 D2)",
                    loads.len()
                ),
            });
        }
        let (axis, omega) = loads[0];
        Ok(CentrifugalFollower {
            m_perp: crate::rotating::spin_softening_mass(model, axis, threads)?,
            omega2: omega * omega,
        })
    }
}

impl Contribution for CentrifugalFollower {
    fn name(&self) -> &str {
        "centrifugal follower"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> Result<()> {
        let m = &self.m_perp;
        if u.len() != m.n {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: m.n,
                found: u.len(),
            });
        }
        let s = load_factor * self.omega2;
        for r in 0..m.n {
            let mut v = 0.0;
            for k in m.row_ptr[r]..m.row_ptr[r + 1] {
                v += m.val[k] * u[m.col[k]];
                tangent.push((r, m.col[k], -s * m.val[k]));
            }
            force[r] -= s * v;
        }
        Ok(())
    }
}

/// The lowest positive buckling factors with the load stiffness of follower pressures (ADR 0480 D3): `(K + mu (K_sigma -
/// sum K_p,sym)) x = 0`, `K_p` of each pressure at zero displacement and load factor 1, by the path of
/// [`crate::buckling::factors`] (its refusals; not on the transformation path). The pre-stress is that of the same reference
/// pressure (for instance the linear static solve with the dead pressure).
pub fn buckling_factors(
    model: &SolidModel,
    prestress: &Prestress,
    pressures: &[&FollowerPressure],
    options: &BucklingOptions,
) -> Result<BucklingResult> {
    let zero = vec![0.0; model.dof_map().n_equations()];
    let mut extra = Vec::new();
    for p in pressures {
        p.symmetric_negative(&zero, 1.0, &mut extra)?;
    }
    crate::buckling::factors_with(model, prestress, &extra, options)
}

/// The modes about a nonlinear state (ADR 0480 D7): `K_T phi = omega^2 M phi` with `K_T` the tangent of the driver at the
/// displacement `u` and the element `states` ([`crate::nonlinear::tangent`]) plus the tangent of each contribution at `(u,
/// load_factor)`, `M` the consistent mass, both reduced on the model's prescribed equations and `prescribed` (the motions
/// about the state hold them; their values are not used). The shift rule, the mass test and the result are those of
/// [`crate::geometric::prestressed_modes`]; a tangent that is not symmetric after the reduction is refused by the
/// eigensolver.
#[allow(clippy::too_many_arguments)]
pub fn tangent_modes(
    model: &SolidModel,
    prescribed: &[Prescribed],
    u: &[f64],
    states: &[ElementState],
    load_factor: f64,
    contributions: &[&dyn Contribution],
    options: &ModalOptions,
) -> Result<ModalResult> {
    model.validate()?;
    model.check_supported_nonlinear()?;
    crate::nonlinear::check_driver_model(model)?;
    let dofs = model.dof_map();
    let fixed: Vec<(usize, f64)> = prescribed_equations(model, prescribed)?
        .into_iter()
        .map(|(eq, _)| (eq, 0.0))
        .collect();
    let rigid_free = match check_rigid_body_motions(model, &fixed) {
        Ok(()) => false,
        Err(FemError::RigidBodyMotionFree { .. }) => true,
        Err(e) => return Err(e),
    };
    let mut k = crate::nonlinear::tangent(model, u, states, options.threads)?;
    if !contributions.is_empty() {
        let mut triplets = Vec::new();
        let mut scratch = vec![0.0; u.len()];
        for c in contributions {
            c.add(u, load_factor, &mut scratch, &mut triplets)
                .map_err(|e| FemError::InvalidValue {
                    key: "contribution",
                    reason: format!("contribution '{}': {e}", c.name()),
                })?;
        }
        add_triplets(&mut k, &triplets);
    }
    let m = consistent_with(model, options.threads)?;
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed)?;
    let mr = reduce(&m, &zero, &fixed)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.modes == 0 || options.modes >= nf {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (i, d) in mr.matrix.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("node {node} component {component} has no mass (diagonal {d:e})"),
            });
        }
    }
    let shift = options
        .shift
        .unwrap_or(if rigid_free { Shift::Auto } else { Shift::Value(0.0) });
    let opts = SubspaceOptions {
        shift,
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr.matrix, &mr.matrix, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let modes = eig
        .vectors
        .iter()
        .map(|v| {
            let mut full = vec![0.0; k.n];
            for (&eq, &x) in free.iter().zip(v) {
                full[eq] = x;
            }
            full
        })
        .collect();
    Ok(ModalResult::from_parts(eig, modes, nf, rigid_free, dofs))
}
